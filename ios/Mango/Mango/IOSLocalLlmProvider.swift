import Foundation
import LlamaSwift
import os

private let localLlmLogger = Logger(subsystem: "dev.disobey.mango", category: "IOSLocalLlm")
private let iosLocalMaxTokens: Int32 = 192
private let iosLocalContextTokens: Int32 = 2048
private let LOCAL_STORAGE_RESERVE_BYTES: UInt64 = 512 * 1024 * 1024
private let LOCAL_STORAGE_MARGIN_PERCENT: UInt64 = 25

private func isAllowedModelDownloadUrl(_ url: URL) -> Bool {
    guard url.scheme?.lowercased() == "https",
          let host = url.host?.lowercased()
    else {
        return false
    }
    return host == "huggingface.co"
        || host == "modelscope.cn"
        || host == "cdn-lfs-cn-1.modelscope.cn"
        || host.hasSuffix(".xethub.hf.co")
        || host.hasSuffix(".hf.co")
}

private func requireAllowedModelDownloadUrl(_ url: URL) throws {
    guard isAllowedModelDownloadUrl(url) else {
        throw LocalLlmError.LoadFailed(reason: "Model download redirected to unexpected target: \(url.scheme ?? "unknown")://\(url.host ?? "unknown")")
    }
}

private final class IOSLocalModelDownloadDelegate: NSObject, URLSessionDownloadDelegate {
    private let context: LocalModelDownloadContext

    init(context: LocalModelDownloadContext) {
        self.context = context
    }

    func urlSession(
        _ session: URLSession,
        task: URLSessionTask,
        willPerformHTTPRedirection response: HTTPURLResponse,
        newRequest request: URLRequest,
        completionHandler: @escaping (URLRequest?) -> Void
    ) {
        guard let url = request.url, isAllowedModelDownloadUrl(url) else {
            completionHandler(nil)
            return
        }
        completionHandler(request)
    }

    func urlSession(
        _ session: URLSession,
        downloadTask: URLSessionDownloadTask,
        didWriteData bytesWritten: Int64,
        totalBytesWritten: Int64,
        totalBytesExpectedToWrite: Int64
    ) {
        let total = totalBytesExpectedToWrite > 0 ? UInt64(totalBytesExpectedToWrite) : nil
        context.emitProgress(downloadedBytes: UInt64(max(0, totalBytesWritten)), totalBytes: total)
    }

    func urlSession(
        _ session: URLSession,
        downloadTask: URLSessionDownloadTask,
        didFinishDownloadingTo location: URL
    ) {
        // The download task completion handler owns validation and moving the temp file.
    }
}

private struct IOSLocalPromptRequest: Decodable {
    struct Message: Decodable {
        let role: String
        let content: String
    }

    let messages: [Message]
}

private final class IOSLlamaSession {
    let modelPath: String
    private let model: OpaquePointer
    private let context: OpaquePointer
    private let vocab: OpaquePointer
    private let sampler: UnsafeMutablePointer<llama_sampler>
    private let chatTemplate: String
    private var batch: llama_batch
    private var pendingUtf8: [CChar] = []

    init(modelPath: String) throws {
        llama_backend_init()

        var modelParams = llama_model_default_params()
#if targetEnvironment(simulator)
        modelParams.n_gpu_layers = 0
#endif

        guard let loadedModel = llama_model_load_from_file(modelPath, modelParams) else {
            llama_backend_free()
            throw LocalLlmError.LoadFailed(reason: "llama.cpp could not load \(modelPath)")
        }
        guard let embeddedTemplate = llama_model_chat_template(loadedModel, nil) else {
            llama_model_free(loadedModel)
            llama_backend_free()
            throw LocalLlmError.LoadFailed(
                reason: "GGUF does not contain tokenizer.chat_template metadata"
            )
        }
        let loadedChatTemplate = String(cString: embeddedTemplate)
        guard !loadedChatTemplate.isEmpty else {
            llama_model_free(loadedModel)
            llama_backend_free()
            throw LocalLlmError.LoadFailed(reason: "GGUF contains an empty chat template")
        }

        var contextParams = llama_context_default_params()
        contextParams.n_ctx = UInt32(iosLocalContextTokens)
        contextParams.n_batch = UInt32(iosLocalContextTokens)
        let threadCount = max(1, min(8, ProcessInfo.processInfo.processorCount - 2))
        contextParams.n_threads = Int32(threadCount)
        contextParams.n_threads_batch = Int32(threadCount)

        guard let loadedContext = llama_init_from_model(loadedModel, contextParams) else {
            llama_model_free(loadedModel)
            llama_backend_free()
            throw LocalLlmError.LoadFailed(reason: "llama.cpp could not create context")
        }

        let samplerParams = llama_sampler_chain_default_params()
        guard let samplerChain = llama_sampler_chain_init(samplerParams) else {
            llama_free(loadedContext)
            llama_model_free(loadedModel)
            llama_backend_free()
            throw LocalLlmError.LoadFailed(reason: "llama.cpp could not create sampler")
        }
        llama_sampler_chain_add(samplerChain, llama_sampler_init_temp(0.4))
        llama_sampler_chain_add(samplerChain, llama_sampler_init_dist(1234))

        self.modelPath = modelPath
        self.model = loadedModel
        self.context = loadedContext
        self.vocab = llama_model_get_vocab(loadedModel)
        self.sampler = samplerChain
        self.chatTemplate = loadedChatTemplate
        self.batch = llama_batch_init(iosLocalContextTokens, 0, 1)

        localLlmLogger.info("Loaded llama.cpp model \(URL(fileURLWithPath: modelPath).lastPathComponent, privacy: .public) threads=\(threadCount, privacy: .public) embeddedTemplate=true")
    }

    deinit {
        llama_sampler_free(sampler)
        llama_batch_free(batch)
        llama_free(context)
        llama_model_free(model)
        llama_backend_free()
    }

    func generate(
        messages: [IOSLocalPromptRequest.Message],
        context generationContext: LocalGenerationContext
    ) throws {
        pendingUtf8.removeAll()
        llama_memory_clear(llama_get_memory(context), true)

        let prompt = try formatPrompt(messages)
        var promptTokens = tokenize(prompt, addBos: true)
        let maxPromptTokens = max(1, Int(iosLocalContextTokens - iosLocalMaxTokens))
        if promptTokens.count > maxPromptTokens {
            promptTokens = Array(promptTokens.suffix(maxPromptTokens))
        }
        guard !promptTokens.isEmpty else {
            throw LocalLlmError.GenerationFailed(reason: "prompt tokenization returned no tokens")
        }

        clearBatch(&batch)
        for (index, token) in promptTokens.enumerated() {
            addToBatch(&batch, token, position: llama_pos(index), logits: false)
        }
        batch.logits[Int(batch.n_tokens) - 1] = 1

        guard llama_decode(context, batch) == 0 else {
            throw LocalLlmError.GenerationFailed(reason: "llama.cpp prompt decode failed")
        }

        var position = batch.n_tokens
        for _ in 0..<iosLocalMaxTokens {
            if generationContext.isCancelled() {
                throw LocalLlmError.Cancelled
            }

            let nextToken = llama_sampler_sample(sampler, context, batch.n_tokens - 1)
            llama_sampler_accept(sampler, nextToken)
            if llama_vocab_is_eog(vocab, nextToken) || position >= iosLocalContextTokens {
                emitPendingUtf8(to: generationContext)
                return
            }

            let piece = tokenPiece(nextToken)
            if !piece.isEmpty {
                pendingUtf8.append(contentsOf: piece)
                emitValidUtf8(to: generationContext)
            }

            clearBatch(&batch)
            addToBatch(&batch, nextToken, position: position, logits: true)
            position += 1

            guard llama_decode(context, batch) == 0 else {
                throw LocalLlmError.GenerationFailed(reason: "llama.cpp token decode failed")
            }
        }

        emitPendingUtf8(to: generationContext)
    }

    private func formatPrompt(_ messages: [IOSLocalPromptRequest.Message]) throws -> String {
        var normalized: [(role: String, content: String)] = []
        for message in messages {
            let role: String
            switch message.role.lowercased() {
            case "system":
                role = "system"
            case "assistant":
                role = "assistant"
            default:
                role = "user"
            }

            let content = message.content.trimmingCharacters(in: .whitespacesAndNewlines)
            if !content.isEmpty {
                normalized.append((role: role, content: content))
            }
        }
        if normalized.isEmpty {
            normalized.append((role: "user", content: "Reply to the user."))
        }

        // NSString owns stable, null-terminated UTF-8 storage for the duration of
        // llama_chat_apply_template. The template itself comes from this GGUF, so
        // Gemma/Phi/Llama/Qwen receive their own control tokens automatically.
        let roleStorage = normalized.map { $0.role as NSString }
        let contentStorage = normalized.map { $0.content as NSString }
        let chat = normalized.indices.map { index in
            llama_chat_message(
                role: roleStorage[index].utf8String,
                content: contentStorage[index].utf8String
            )
        }

        return try chatTemplate.withCString { template in
            let requiredLength = chat.withUnsafeBufferPointer { chatBuffer in
                llama_chat_apply_template(
                    template,
                    chatBuffer.baseAddress,
                    chatBuffer.count,
                    true,
                    nil,
                    0
                )
            }
            guard requiredLength >= 0 else {
                throw LocalLlmError.GenerationFailed(
                    reason: "this GGUF's embedded chat template is not supported by the iOS llama.cpp runtime"
                )
            }
            guard requiredLength < Int32.max else {
                throw LocalLlmError.GenerationFailed(reason: "formatted local prompt is too large")
            }

            var buffer = [CChar](repeating: 0, count: Int(requiredLength) + 1)
            let written = chat.withUnsafeBufferPointer { chatBuffer in
                buffer.withUnsafeMutableBufferPointer { promptBuffer in
                    llama_chat_apply_template(
                        template,
                        chatBuffer.baseAddress,
                        chatBuffer.count,
                        true,
                        promptBuffer.baseAddress,
                        Int32(promptBuffer.count)
                    )
                }
            }
            guard written >= 0, written <= requiredLength else {
                throw LocalLlmError.GenerationFailed(reason: "failed to apply the GGUF chat template")
            }

            let utf8 = buffer.prefix(Int(written)).map { UInt8(bitPattern: $0) }
            guard let prompt = String(bytes: utf8, encoding: .utf8) else {
                throw LocalLlmError.GenerationFailed(reason: "GGUF chat template produced invalid UTF-8")
            }
            return prompt
        }
    }

    private func tokenize(_ text: String, addBos: Bool) -> [llama_token] {
        let utf8Count = text.utf8.count
        var capacity = utf8Count + (addBos ? 2 : 1)
        var tokens = [llama_token](repeating: 0, count: capacity)
        var tokenCount = llama_tokenize(
            vocab,
            text,
            Int32(utf8Count),
            &tokens,
            Int32(tokens.count),
            addBos,
            true
        )

        if tokenCount < 0 {
            capacity = Int(-tokenCount)
            tokens = [llama_token](repeating: 0, count: capacity)
            tokenCount = llama_tokenize(
                vocab,
                text,
                Int32(utf8Count),
                &tokens,
                Int32(tokens.count),
                addBos,
                true
            )
        }

        guard tokenCount > 0 else { return [] }
        return Array(tokens.prefix(Int(tokenCount)))
    }

    private func tokenPiece(_ token: llama_token) -> [CChar] {
        var buffer = [CChar](repeating: 0, count: 16)
        let count = llama_token_to_piece(vocab, token, &buffer, Int32(buffer.count), 0, false)
        if count >= 0 {
            return Array(buffer.prefix(Int(count)))
        }

        var largeBuffer = [CChar](repeating: 0, count: Int(-count))
        let largeCount = llama_token_to_piece(
            vocab,
            token,
            &largeBuffer,
            Int32(largeBuffer.count),
            0,
            false
        )
        guard largeCount > 0 else { return [] }
        return Array(largeBuffer.prefix(Int(largeCount)))
    }

    private func emitValidUtf8(to context: LocalGenerationContext) {
        if let text = String(validatingUTF8: pendingUtf8 + [0]) {
            pendingUtf8.removeAll()
            if !text.isEmpty {
                context.emitToken(token: text)
            }
        }
    }

    private func emitPendingUtf8(to context: LocalGenerationContext) {
        guard !pendingUtf8.isEmpty else { return }
        let text = String(cString: pendingUtf8 + [0])
        pendingUtf8.removeAll()
        if !text.isEmpty {
            context.emitToken(token: text)
        }
    }

    private func clearBatch(_ batch: inout llama_batch) {
        batch.n_tokens = 0
    }

    private func addToBatch(
        _ batch: inout llama_batch,
        _ token: llama_token,
        position: llama_pos,
        logits: Bool
    ) {
        let index = Int(batch.n_tokens)
        batch.token[index] = token
        batch.pos[index] = position
        batch.n_seq_id[index] = 1
        batch.seq_id[index]![0] = 0
        batch.logits[index] = logits ? 1 : 0
        batch.n_tokens += 1
    }
}

final class IOSLocalLlmProvider: LocalLlmProvider, @unchecked Sendable {
    private let lock = NSLock()
    private var session: IOSLlamaSession?
    private let capability: DeviceCapability

    init() {
        self.capability = IOSLocalLlmProvider.probeCapability()
    }

    func loadModel(modelPath: String) throws {
        lock.lock()
        defer { lock.unlock() }

        if session?.modelPath == modelPath {
            return
        }
        unloadLocked()

        guard FileManager.default.fileExists(atPath: modelPath) else {
            throw LocalLlmError.ModelMissing(path: modelPath)
        }
        guard capability.status == .supported else {
            throw LocalLlmError.Unsupported(reason: capability.reason ?? "local inference is unavailable")
        }
        if let storageBytes = Self.availableStorageBytes(),
            let fileSize = fileSize(modelPath) {
            let modelBytes = UInt64(fileSize)
            let marginBytes = (modelBytes * LOCAL_STORAGE_MARGIN_PERCENT) / 100
            let requiredBytes = modelBytes + marginBytes + LOCAL_STORAGE_RESERVE_BYTES
            if storageBytes < requiredBytes {
                throw LocalLlmError.Unsupported(
                    reason: "Not enough free storage: required \(requiredBytes) bytes, available \(storageBytes) bytes"
                )
            }
        }

        session = try IOSLlamaSession(modelPath: modelPath)
    }

    func downloadModelFile(
        url: String,
        destinationPath: String,
        context: LocalModelDownloadContext
    ) throws {
        guard let source = URL(string: url) else {
            throw LocalLlmError.LoadFailed(reason: "invalid model URL")
        }
        try requireAllowedModelDownloadUrl(source)

        let destination = URL(fileURLWithPath: destinationPath)
        do {
            try FileManager.default.createDirectory(
                at: destination.deletingLastPathComponent(),
                withIntermediateDirectories: true
            )
        } catch {
            throw LocalLlmError.LoadFailed(reason: error.localizedDescription)
        }

        let semaphore = DispatchSemaphore(value: 0)
        var result: Result<Void, Error> = .failure(LocalLlmError.LoadFailed(reason: "download did not start"))
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = 30
        config.timeoutIntervalForResource = 30 * 60
        config.waitsForConnectivity = true
        let delegate = IOSLocalModelDownloadDelegate(context: context)
        let session = URLSession(configuration: config, delegate: delegate, delegateQueue: nil)
        defer {
            session.finishTasksAndInvalidate()
        }
        let task = session.downloadTask(with: source) { tempUrl, response, error in
            defer { semaphore.signal() }
            if let error {
                result = .failure(LocalLlmError.LoadFailed(reason: error.localizedDescription))
                return
            }
            if let finalUrl = response?.url {
                do {
                    try requireAllowedModelDownloadUrl(finalUrl)
                } catch {
                    result = .failure(error)
                    return
                }
            }
            if let http = response as? HTTPURLResponse, !(200...299).contains(http.statusCode) {
                result = .failure(LocalLlmError.LoadFailed(reason: "model download failed with HTTP \(http.statusCode)"))
                return
            }
            guard let tempUrl else {
                result = .failure(LocalLlmError.LoadFailed(reason: "download produced no file"))
                return
            }
            do {
                if FileManager.default.fileExists(atPath: destination.path) {
                    try FileManager.default.removeItem(at: destination)
                }
                try FileManager.default.moveItem(at: tempUrl, to: destination)
                result = .success(())
            } catch {
                result = .failure(LocalLlmError.LoadFailed(reason: error.localizedDescription))
            }
        }
        task.resume()
        semaphore.wait()
        do {
            try result.get()
        } catch let error as LocalLlmError {
            throw error
        } catch {
            throw LocalLlmError.LoadFailed(reason: error.localizedDescription)
        }
    }

    func platformHttpRequest(request: PlatformHttpRequest) throws -> PlatformHttpResponse {
        guard let url = URL(string: request.url), url.scheme?.lowercased() == "https" else {
            throw LocalLlmError.LoadFailed(reason: "Platform HTTP only allows HTTPS URLs")
        }

        var urlRequest = URLRequest(url: url)
        urlRequest.httpMethod = request.method.isEmpty ? "GET" : request.method.uppercased()
        let timeoutSecs = min(UInt64(120), max(UInt64(1), request.timeoutSecs))
        urlRequest.timeoutInterval = TimeInterval(timeoutSecs)
        for header in request.headers where !header.name.isEmpty {
            urlRequest.setValue(header.value, forHTTPHeaderField: header.name)
        }
        if !request.body.isEmpty {
            urlRequest.httpBody = request.body
        }

        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = urlRequest.timeoutInterval
        config.timeoutIntervalForResource = urlRequest.timeoutInterval
        config.httpShouldSetCookies = false
        let session = URLSession(configuration: config)
        defer { session.invalidateAndCancel() }

        let semaphore = DispatchSemaphore(value: 0)
        var result: Result<PlatformHttpResponse, Error> = .failure(
            LocalLlmError.LoadFailed(reason: "platform HTTP request did not start")
        )
        let task = session.dataTask(with: urlRequest) { data, response, error in
            defer { semaphore.signal() }
            if let error {
                result = .failure(LocalLlmError.LoadFailed(reason: error.localizedDescription))
                return
            }
            guard let http = response as? HTTPURLResponse else {
                result = .failure(LocalLlmError.LoadFailed(reason: "platform HTTP response was not HTTP"))
                return
            }
            let headers = http.allHeaderFields.compactMap { key, value -> PlatformHttpHeader? in
                guard let name = key as? String else { return nil }
                return PlatformHttpHeader(name: name, value: "\(value)")
            }
            result = .success(
                PlatformHttpResponse(
                    statusCode: UInt16(http.statusCode),
                    headers: headers,
                    body: data ?? Data()
                )
            )
        }
        task.resume()
        semaphore.wait()

        do {
            return try result.get()
        } catch let error as LocalLlmError {
            throw error
        } catch {
            throw LocalLlmError.LoadFailed(reason: error.localizedDescription)
        }
    }

    func generate(promptJson: String, context: LocalGenerationContext) throws {
        lock.lock()
        defer { lock.unlock() }

        guard let session else {
            throw LocalLlmError.NotLoaded
        }
        if context.isCancelled() {
            throw LocalLlmError.Cancelled
        }

        let request: IOSLocalPromptRequest
        do {
            let data = Data(promptJson.utf8)
            request = try JSONDecoder().decode(IOSLocalPromptRequest.self, from: data)
        } catch {
            throw LocalLlmError.GenerationFailed(reason: "invalid local prompt JSON: \(error.localizedDescription)")
        }

        do {
            try session.generate(messages: request.messages, context: context)
        } catch let error as LocalLlmError {
            throw error
        } catch {
            throw LocalLlmError.GenerationFailed(reason: error.localizedDescription)
        }
    }

    func unload() {
        lock.lock()
        defer { lock.unlock() }
        unloadLocked()
    }

    func loadedModelPath() -> String? {
        lock.lock()
        defer { lock.unlock() }
        return session?.modelPath
    }

    func deviceCapability() -> DeviceCapability {
        capability
    }

    private func unloadLocked() {
        session = nil
    }

    private static func probeCapability() -> DeviceCapability {
        let totalRam = ProcessInfo.processInfo.physicalMemory
        let abi = machineIdentifier()

#if targetEnvironment(simulator)
        return DeviceCapability(
            abi: abi,
            totalRamBytes: totalRam,
            availableRamBytes: 0,
            supportsMmap: false,
            status: .unsupportedArchitecture,
            reasonCode: "unsupported_architecture",
            reason: "iOS simulator local LLM runtime is disabled",
            availableStorageBytes: 0
        )
#else
        let supportedArchitecture = abi == "arm64" || abi == "arm64e"
        let availableStorage = availableStorageBytes() ?? 0
        let hasStorage = availableStorage > LOCAL_STORAGE_RESERVE_BYTES
        let availableRam = availableRamBytes()
        let status: LocalLlmCapabilityStatus = if !FeatureFlags.localLlmEnabled {
            .disabledByFeatureFlag
        } else if !supportedArchitecture {
            .unsupportedArchitecture
        } else if !hasStorage {
            .insufficientStorage
        } else {
            .supported
        }
        let reason = reasonForStatus(status, abi: abi, totalRam: totalRam)
        let reasonCode = reasonCode(for: status)
        let stableSupported = status == .supported && FeatureFlags.localLlmEnabled

        return DeviceCapability(
            abi: abi,
            totalRamBytes: totalRam,
            availableRamBytes: stableSupported ? availableRam : 0,
            supportsMmap: stableSupported,
            status: stableSupported ? .supported : status,
            reasonCode: reasonCode,
            reason: reason,
            availableStorageBytes: availableStorage
        )
#endif
    }

    private static func reasonForStatus(
        _ status: LocalLlmCapabilityStatus,
        abi: String,
        totalRam: UInt64,
    ) -> String? {
        switch status {
        case .supported, .runtimeNotPackaged, .runtimeLoadFailed, .probeUnavailable:
            return nil
        case .disabledByFeatureFlag:
            return "Local LLM is disabled by feature flag"
        case .unsupportedApiLevel:
            return "Unsupported iOS version: \(ProcessInfo.processInfo.operatingSystemVersionString)"
        case .unsupportedArchitecture:
            return "Unsupported ABI: \(abi)"
        case .unsupportedProcessBitness:
            return "iOS requires 64-bit process"
        case .unsupportedCpuFeatures:
            return "Unsupported CPU features for local inference"
        case .insufficientMemory:
            return "Insufficient RAM for Local LLM (\(totalRam) bytes)"
        case .insufficientStorage:
            return "Insufficient free storage for Local LLM"
        }
    }

    private static func reasonCode(for status: LocalLlmCapabilityStatus) -> String {
        switch status {
        case .unknown:
            return "unknown"
        case .supported:
            return "supported"
        case .disabledByFeatureFlag:
            return "disabled_by_feature_flag"
        case .unsupportedApiLevel:
            return "unsupported_api_level"
        case .unsupportedArchitecture:
            return "unsupported_architecture"
        case .unsupportedProcessBitness:
            return "unsupported_process_bitness"
        case .unsupportedCpuFeatures:
            return "unsupported_cpu_features"
        case .insufficientMemory:
            return "insufficient_memory"
        case .insufficientStorage:
            return "insufficient_storage"
        case .probeUnavailable:
            return "probe_unavailable"
        case .runtimeNotPackaged:
            return "runtime_not_packaged"
        case .runtimeLoadFailed:
            return "runtime_load_failed"
        }
    }

    private static func machineIdentifier() -> String {
        var systemInfo = utsname()
        uname(&systemInfo)
        return withUnsafePointer(to: &systemInfo.machine) { pointer in
            pointer.withMemoryRebound(to: CChar.self, capacity: 1) { machine in
                String(cString: machine)
            }
        }
    }

    private func fileSize(_ path: String) -> UInt64? {
        guard let size = try? FileManager.default.attributesOfItem(atPath: path)[.size] as? NSNumber else {
            return nil
        }
        return size.uint64Value
    }

    private static func availableStorageBytes() -> UInt64? {
        guard let supportDir = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first else {
            return nil
        }
        let attributes = try? FileManager.default.attributesOfFileSystem(forPath: supportDir.path)
        return (attributes?[.systemFreeSize] as? NSNumber)?.uint64Value
    }

    private static func availableRamBytes() -> UInt64 {
        return UInt64(os_proc_available_memory())
    }
}
