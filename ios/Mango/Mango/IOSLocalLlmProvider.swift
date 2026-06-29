import Foundation
import LlamaSwift
import os

private let localLlmLogger = Logger(subsystem: "dev.disobey.mango", category: "IOSLocalLlm")
private let iosLocalMaxTokens: Int32 = 192
private let iosLocalContextTokens: Int32 = 2048

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
        self.batch = llama_batch_init(iosLocalContextTokens, 0, 1)

        localLlmLogger.info("Loaded llama.cpp model \(URL(fileURLWithPath: modelPath).lastPathComponent, privacy: .public) threads=\(threadCount, privacy: .public)")
    }

    deinit {
        llama_sampler_free(sampler)
        llama_batch_free(batch)
        llama_free(context)
        llama_model_free(model)
        llama_backend_free()
    }

    func generate(prompt: String, context generationContext: LocalGenerationContext) throws {
        pendingUtf8.removeAll()
        llama_memory_clear(llama_get_memory(context), true)

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
        if capability.maxModelBytes == 0 {
            throw LocalLlmError.Unsupported(reason: capability.reason ?? "local inference is unavailable")
        }
        if let size = fileSize(modelPath), UInt64(size) > capability.maxModelBytes {
            throw LocalLlmError.Unsupported(reason: "Model is too large for this device: \(size) bytes")
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

        let prompt: String
        do {
            prompt = try Self.prompt(from: promptJson)
        } catch {
            throw LocalLlmError.GenerationFailed(reason: "invalid local prompt JSON: \(error.localizedDescription)")
        }

        do {
            try session.generate(prompt: prompt, context: context)
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

    private static func prompt(from promptJson: String) throws -> String {
        let data = Data(promptJson.utf8)
        let request = try JSONDecoder().decode(IOSLocalPromptRequest.self, from: data)
        var prompt = ""
        var includedMessage = false

        for message in request.messages {
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
            guard !content.isEmpty else { continue }
            prompt += "<|im_start|>\(role)\n\(content)<|im_end|>\n"
            includedMessage = true
        }

        if !includedMessage {
            prompt += "<|im_start|>user\nReply to the user.<|im_end|>\n"
        }
        prompt += "<|im_start|>assistant\n"
        return prompt
    }

    private static func probeCapability() -> DeviceCapability {
        let totalRam = ProcessInfo.processInfo.physicalMemory
        let abi = machineIdentifier()

#if targetEnvironment(simulator)
        return DeviceCapability(
            abi: abi,
            totalRamBytes: totalRam,
            maxModelBytes: 0,
            supportsMmap: false,
            reason: "iOS simulator local LLM runtime is disabled"
        )
#else
        let supported = abi == "arm64" || abi == "arm64e"
        let maxModelBytes = supported ? min(totalRam / 3, 1_500_000_000) : 0
        return DeviceCapability(
            abi: abi,
            totalRamBytes: totalRam,
            maxModelBytes: maxModelBytes,
            supportsMmap: supported,
            reason: supported ? nil : "Unsupported ABI: \(abi)"
        )
#endif
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
}
