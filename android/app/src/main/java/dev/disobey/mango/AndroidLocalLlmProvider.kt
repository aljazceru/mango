package dev.disobey.mango

import android.app.ActivityManager
import android.content.Context
import android.os.Build
import android.util.Log
import dev.disobey.mango.rust.DeviceCapability
import dev.disobey.mango.rust.LocalLlmCapabilityStatus
import dev.disobey.mango.rust.LocalGenerationContext
import dev.disobey.mango.rust.LocalLlmException
import dev.disobey.mango.rust.LocalLlmProvider
import dev.disobey.mango.rust.LocalModelDownloadContext
import dev.disobey.mango.rust.PlatformHttpHeader
import dev.disobey.mango.rust.PlatformHttpRequest
import dev.disobey.mango.rust.PlatformHttpResponse
import java.io.BufferedInputStream
import java.io.File
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL
import kotlin.math.max
import kotlin.math.min

private const val TAG = "AndroidLocalLlm"
private const val LOCAL_MAX_TOKENS = 192
private const val LOCAL_CONTEXT_TOKENS = 2048
private const val MAX_MODEL_REDIRECTS = 10
private const val MODEL_RESPONSE_SNIFF_BYTES = 8192
private const val MIN_API_LEVEL_FOR_LOCAL_LLM = 31
private const val LOCAL_STORAGE_RESERVE_BYTES = 512L * 1024L * 1024L
private const val LOCAL_STORAGE_MARGIN_PERCENT = 25L
private val GGUF_MAGIC = byteArrayOf(0x47, 0x47, 0x55, 0x46)

class AndroidLocalLlmProvider(context: Context) : LocalLlmProvider {
    private val appContext = context.applicationContext
    private val capability = probeCapability(appContext)
    private val inferenceThreads = localThreadCount()
    private var loadedPath: String? = null

    override fun downloadModelFile(
        url: String,
        destinationPath: String,
        context: LocalModelDownloadContext,
    ) {
        try {
            val destination = File(destinationPath)
            destination.parentFile?.mkdirs()
            var current = URL(url)

            repeat(MAX_MODEL_REDIRECTS) {
                requireAllowedModelDownloadUrl(current)
                val connection = (current.openConnection() as HttpURLConnection).apply {
                    instanceFollowRedirects = false
                    connectTimeout = 20_000
                    readTimeout = 30_000
                    requestMethod = "GET"
                    setRequestProperty("User-Agent", "Mango")
                    setRequestProperty("Accept", "*/*")
                    setRequestProperty("Accept-Encoding", "identity")
                }
                try {
                    val code = connection.responseCode
                    val contentLength = connection.contentLengthLong
                    val contentType = connection.contentType ?: ""
                    Log.i(
                        TAG,
                        "Model download response host=${current.host} code=$code length=$contentLength type=$contentType",
                    )
                    if (code in 300..399) {
                        val location = connection.getHeaderField("Location")
                            ?: throw LocalLlmException.LoadFailed("Model download redirect missing Location")
                        current = URL(current, location)
                        requireAllowedModelDownloadUrl(current)
                        return@repeat
                    }
                    if (code !in 200..299) {
                        val reason = connection.responseMessage ?: "HTTP $code"
                        throw LocalLlmException.LoadFailed("Model download failed: $reason")
                    }
                    if (isBridgeResponse(current, contentType, contentLength)) {
                        val body = connection.inputStream.use { input ->
                            input.readBytes().toString(Charsets.UTF_8)
                        }
                        current = parseBridgeUrl(body, current)
                        return@repeat
                    }

                    val total = contentLength
                        .takeIf { it > 0L }
                        ?.toULong()
                    context.emitProgress(0UL, total)
                    BufferedInputStream(connection.inputStream).use { input ->
                        validateModelResponse(input, contentType, contentLength, current)
                        FileOutputStream(destination).use { output ->
                            val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
                            var downloaded = 0L
                            var lastProgress = 0L
                            while (true) {
                                val read = input.read(buffer)
                                if (read < 0) break
                                output.write(buffer, 0, read)
                                downloaded += read.toLong()
                                if (downloaded - lastProgress >= 1024L * 1024L) {
                                    lastProgress = downloaded
                                    context.emitProgress(downloaded.toULong(), total)
                                }
                            }
                            output.fd.sync()
                            context.emitProgress(downloaded.toULong(), total)
                            if (contentLength > 0L && downloaded != contentLength) {
                                throw LocalLlmException.LoadFailed(
                                    "Model download ended early: $downloaded of $contentLength bytes"
                                )
                            }
                        }
                    }
                    Log.i(TAG, "Downloaded local model bytes to ${destination.name}")
                    return
                } finally {
                    connection.disconnect()
                }
            }

            throw LocalLlmException.LoadFailed("Model download followed too many redirects")
        } catch (error: LocalLlmException) {
            throw error
        } catch (error: Throwable) {
            val message = error.message ?: error.javaClass.simpleName
            throw LocalLlmException.LoadFailed(message)
        }
    }

    override fun platformHttpRequest(request: PlatformHttpRequest): PlatformHttpResponse {
        try {
            val url = URL(request.url)
            if (url.protocol != "https") {
                throw LocalLlmException.LoadFailed("Platform HTTP only allows HTTPS URLs")
            }
            val method = request.method.ifBlank { "GET" }.uppercase()
            val timeoutMillis = request.timeoutSecs
                .coerceAtLeast(1UL)
                .coerceAtMost(120UL)
                .toLong()
                .times(1000L)
                .toInt()
            val connection = (url.openConnection() as HttpURLConnection).apply {
                instanceFollowRedirects = false
                connectTimeout = timeoutMillis
                readTimeout = timeoutMillis
                requestMethod = method
                setRequestProperty("User-Agent", "Mango")
                request.headers.forEach { header ->
                    if (header.name.isNotBlank()) {
                        setRequestProperty(header.name, header.value)
                    }
                }
                if (request.body.isNotEmpty()) {
                    doOutput = true
                    setFixedLengthStreamingMode(request.body.size)
                }
            }

            try {
                if (request.body.isNotEmpty()) {
                    connection.outputStream.use { output ->
                        output.write(request.body)
                    }
                }
                val code = connection.responseCode
                val body = (if (code in 200..399) {
                    connection.inputStream
                } else {
                    connection.errorStream ?: connection.inputStream
                }).use { input ->
                    input.readBytes()
                }
                val headers = connection.headerFields
                    .flatMap { (name, values) ->
                        if (name == null) {
                            emptyList()
                        } else {
                            values.orEmpty().map { value ->
                                PlatformHttpHeader(name = name, value = value)
                            }
                        }
                    }
                return PlatformHttpResponse(
                    statusCode = code.toUShort(),
                    headers = headers,
                    body = body,
                )
            } finally {
                connection.disconnect()
            }
        } catch (error: LocalLlmException) {
            throw error
        } catch (error: Throwable) {
            val message = error.message ?: error.javaClass.simpleName
            throw LocalLlmException.LoadFailed(message)
        }
    }

    @Synchronized
    override fun loadModel(modelPath: String) {
        val file = File(modelPath)
        if (!file.isFile) {
            throw LocalLlmException.ModelMissing(modelPath)
        }
        if (capability.status != LocalLlmCapabilityStatus.SUPPORTED) {
            throw LocalLlmException.Unsupported(capability.reason ?: "local inference is unavailable")
        }

        val storageBytes = availableStorageBytes(appContext)
        val modelBytes = file.length().toULong()
        val requiredStorage = (modelBytes * (100UL + LOCAL_STORAGE_MARGIN_PERCENT.toULong())) / 100UL
        val requiredTotal = requiredStorage + LOCAL_STORAGE_RESERVE_BYTES.toULong()
        if (storageBytes < requiredTotal) {
            throw LocalLlmException.Unsupported(
                "Not enough free storage: required ${formatBytes(requiredTotal)} (including ${LOCAL_STORAGE_MARGIN_PERCENT}% margin), available ${formatBytes(storageBytes)}"
            )
        }

        try {
            AndroidLlamaEngine.ensureInitialized(appContext)
            val code = AndroidLlamaEngine.nativeLoadModel(
                file.absolutePath,
                LOCAL_CONTEXT_TOKENS,
                inferenceThreads,
            )
            if (code != 0) {
                val reason = AndroidLlamaEngine.nativeLastError()
                    ?: "llama.cpp load failed with code $code"
                throw LocalLlmException.LoadFailed(reason)
            }
            loadedPath = file.absolutePath
            Log.i(TAG, "Loaded local model ${file.name}")
        } catch (error: LocalLlmException) {
            throw error
        } catch (error: Throwable) {
            unload()
            throw LocalLlmException.LoadFailed(error.message ?: error.javaClass.simpleName)
        }
    }

    @Synchronized
    override fun generate(promptJson: String, context: LocalGenerationContext) {
        loadedPath ?: throw LocalLlmException.NotLoaded()

        try {
            if (context.isCancelled()) {
                throw LocalLlmException.Cancelled()
            }
            // Prompt formatting (ChatML/Llama-3/Gemma/Phi...) is handled in native
            // via common_chat_templates_apply using each model's embedded template.
            val promptCode = AndroidLlamaEngine.nativeProcessPrompt(
                promptJson,
                LOCAL_MAX_TOKENS,
            )
            if (promptCode != 0) {
                val reason = AndroidLlamaEngine.nativeLastError()
                    ?: "llama.cpp prompt processing failed with code $promptCode"
                throw LocalLlmException.GenerationFailed(reason)
            }

            while (!context.isCancelled()) {
                val token = AndroidLlamaEngine.nativeNextToken() ?: break
                if (token.isNotEmpty()) {
                    context.emitToken(token)
                }
                val nativeError = AndroidLlamaEngine.nativeLastError()
                if (nativeError != null) {
                    throw LocalLlmException.GenerationFailed(nativeError)
                }
            }
            AndroidLlamaEngine.nativeLastError()?.let { nativeError ->
                throw LocalLlmException.GenerationFailed(nativeError)
            }
            if (context.isCancelled()) throw LocalLlmException.Cancelled()
        } catch (error: LocalLlmException) {
            throw error
        } catch (error: Throwable) {
            val message = error.message ?: error.javaClass.simpleName
            context.emitError("Local generation failed: $message")
            throw LocalLlmException.GenerationFailed(message)
        }
    }

    @Synchronized
    override fun unload() {
        loadedPath = null
        try {
            AndroidLlamaEngine.unloadIfInitialized()
        } catch (error: Throwable) {
            Log.w(TAG, "Failed to unload llama.cpp runtime cleanly", error)
        }
    }

    @Synchronized
    override fun loadedModelPath(): String? = loadedPath

    override fun deviceCapability(): DeviceCapability = capability
}

private fun isBridgeResponse(url: URL, contentType: String, contentLength: Long): Boolean {
    if (!isAllowedModelDownloadUrl(url)) return false
    if (contentType.contains("text/html", ignoreCase = true)) return true
    if (contentType.contains("text/plain", ignoreCase = true)) return true
    return url.host.equals("modelscope.cn", ignoreCase = true) &&
        contentLength in 0L..MODEL_RESPONSE_SNIFF_BYTES.toLong()
}

private fun validateModelResponse(
    input: BufferedInputStream,
    contentType: String,
    contentLength: Long,
    url: URL,
) {
    input.mark(MODEL_RESPONSE_SNIFF_BYTES)
    val probe = ByteArray(MODEL_RESPONSE_SNIFF_BYTES)
    val read = input.read(probe)
    input.reset()

    if (read >= GGUF_MAGIC.size && probe.copyOfRange(0, GGUF_MAGIC.size).contentEquals(GGUF_MAGIC)) {
        return
    }

    val prefix = if (read > 0) {
        probe.copyOfRange(0, read).toString(Charsets.UTF_8)
    } else {
        ""
    }
    val bridgeUrl = runCatching { parseBridgeUrl(prefix, url) }.getOrNull()
    if (bridgeUrl != null) {
        throw LocalLlmException.LoadFailed(
            "Model download returned an embedded redirect instead of GGUF bytes: $bridgeUrl"
        )
    }
    val snippet = prefix.take(180).replace(Regex("\\s+"), " ")
    throw LocalLlmException.LoadFailed(
        "Model download returned non-GGUF bytes from ${url.host} " +
            "(length=$contentLength type=${contentType.ifBlank { "unknown" }}): $snippet"
    )
}

private fun parseBridgeUrl(body: String, baseUrl: URL): URL {
    val anchorHref = body
        .substringAfter("<a href=\"", missingDelimiterValue = "")
        .substringBefore('"', missingDelimiterValue = "")
    val refreshUrl = body
        .indexOf("url=", ignoreCase = true)
        .takeIf { it >= 0 }
        ?.let { index ->
            body.substring(index + "url=".length)
                .substringBefore('"')
                .substringBefore('\'')
                .substringBefore('>')
                .trim()
        }
        .orEmpty()
    val quotedUrl = Regex("""https://[^"' <>\)]+""")
        .find(body)
        ?.value
        .orEmpty()
    val redirectUrl = anchorHref.ifBlank { refreshUrl }.ifBlank { quotedUrl }
    if (redirectUrl.isBlank()) {
        val snippet = body.take(160)
        throw LocalLlmException.LoadFailed("Model download returned a non-model response: $snippet")
    }
    val normalized = redirectUrl.replace("&amp;", "&")
    val parsed = URL(baseUrl, normalized)
    if (!isAllowedModelDownloadUrl(parsed)) {
        throw LocalLlmException.LoadFailed(
            "Model download redirected to unexpected target: ${parsed.protocol}://${parsed.host}"
        )
    }
    return parsed
}

private fun requireAllowedModelDownloadUrl(url: URL) {
    if (!isAllowedModelDownloadUrl(url)) {
        throw LocalLlmException.LoadFailed(
            "Model download redirected to unexpected target: ${url.protocol}://${url.host}"
        )
    }
}

private fun isAllowedModelDownloadUrl(url: URL): Boolean {
    val host = url.host.lowercase()
    return url.protocol == "https" &&
        (host == "huggingface.co" ||
            host == "modelscope.cn" ||
            host == "cdn-lfs-cn-1.modelscope.cn" ||
            host.endsWith(".xethub.hf.co") ||
            host.endsWith(".hf.co"))
}

private fun probeCapability(context: Context): DeviceCapability {
    val abi = Build.SUPPORTED_ABIS.firstOrNull() ?: "unknown"
    val totalRam = totalRamBytes(context)
    val availRam = availableRamBytes(context)
    val supportsArm64 = Build.SUPPORTED_ABIS.any { it == "arm64-v8a" }
    val supportsApiLevel = Build.VERSION.SDK_INT >= MIN_API_LEVEL_FOR_LOCAL_LLM
    val is64BitProcess = android.os.Process.is64Bit()
    val availableStorage = availableStorageBytes(context)
    val hasStorage = availableStorage > LOCAL_STORAGE_RESERVE_BYTES.toULong()
    val supportedRuntime = supportsApiLevel && supportsArm64 && is64BitProcess && hasStorage
    val supportsMmap = supportedRuntime
    val status = when {
        !FeatureFlags.LOCAL_LLM_ENABLED -> LocalLlmCapabilityStatus.DISABLED_BY_FEATURE_FLAG
        !supportsApiLevel -> LocalLlmCapabilityStatus.UNSUPPORTED_API_LEVEL
        !supportsArm64 -> LocalLlmCapabilityStatus.UNSUPPORTED_ARCHITECTURE
        !is64BitProcess -> LocalLlmCapabilityStatus.UNSUPPORTED_PROCESS_BITNESS
        !hasStorage -> LocalLlmCapabilityStatus.INSUFFICIENT_STORAGE
        totalRam <= 0L -> LocalLlmCapabilityStatus.UNKNOWN
        else -> LocalLlmCapabilityStatus.SUPPORTED
    }
    val reason = when (status) {
        LocalLlmCapabilityStatus.DISABLED_BY_FEATURE_FLAG -> "Local LLM disabled by feature flag"
        LocalLlmCapabilityStatus.UNSUPPORTED_API_LEVEL ->
            "Unsupported API level: ${Build.VERSION.SDK_INT}"
        LocalLlmCapabilityStatus.UNSUPPORTED_ARCHITECTURE -> "Unsupported ABI: $abi"
        LocalLlmCapabilityStatus.UNSUPPORTED_PROCESS_BITNESS -> "Local LLM requires a 64-bit process"
        LocalLlmCapabilityStatus.INSUFFICIENT_STORAGE -> "Insufficient free storage for LocalLLM"
        LocalLlmCapabilityStatus.UNKNOWN -> "Unable to determine device RAM"
        LocalLlmCapabilityStatus.RUNTIME_NOT_PACKAGED,
        LocalLlmCapabilityStatus.RUNTIME_LOAD_FAILED,
        LocalLlmCapabilityStatus.UNSUPPORTED_CPU_FEATURES,
        LocalLlmCapabilityStatus.SUPPORTED -> null
        LocalLlmCapabilityStatus.PROBE_UNAVAILABLE -> "Local LLM probe is not available"
        LocalLlmCapabilityStatus.INSUFFICIENT_MEMORY -> null
    }

    val reasonCode = when (status) {
        LocalLlmCapabilityStatus.DISABLED_BY_FEATURE_FLAG -> "disabled_by_feature_flag"
        LocalLlmCapabilityStatus.UNSUPPORTED_API_LEVEL -> "unsupported_api_level"
        LocalLlmCapabilityStatus.UNSUPPORTED_ARCHITECTURE -> "unsupported_architecture"
        LocalLlmCapabilityStatus.UNSUPPORTED_PROCESS_BITNESS -> "unsupported_process_bitness"
        LocalLlmCapabilityStatus.INSUFFICIENT_MEMORY -> "insufficient_memory"
        LocalLlmCapabilityStatus.INSUFFICIENT_STORAGE -> "insufficient_storage"
        LocalLlmCapabilityStatus.UNSUPPORTED_CPU_FEATURES -> "unsupported_cpu_features"
        LocalLlmCapabilityStatus.PROBE_UNAVAILABLE -> "probe_unavailable"
        LocalLlmCapabilityStatus.RUNTIME_NOT_PACKAGED -> "runtime_not_packaged"
        LocalLlmCapabilityStatus.RUNTIME_LOAD_FAILED -> "runtime_load_failed"
        LocalLlmCapabilityStatus.SUPPORTED -> "supported"
        LocalLlmCapabilityStatus.UNKNOWN -> "unknown"
    }

    val stableSupported =
        status == LocalLlmCapabilityStatus.SUPPORTED && FeatureFlags.LOCAL_LLM_ENABLED && supportedRuntime

    return DeviceCapability(
        abi = abi,
        totalRamBytes = totalRam.coerceAtLeast(0L).toULong(),
        availableRamBytes = if (stableSupported) availRam.toULong() else 0UL,
        supportsMmap = supportsMmap,
        status = if (stableSupported) LocalLlmCapabilityStatus.SUPPORTED else status,
        reasonCode = reasonCode,
        reason = reason,
        availableStorageBytes = availableStorage
    )
}

private fun totalRamBytes(context: Context): Long {
    val activityManager = context.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager
        ?: return 0L
    val info = ActivityManager.MemoryInfo()
    activityManager.getMemoryInfo(info)
    return info.totalMem
}

private fun availableRamBytes(context: Context): Long {
    val activityManager = context.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager
        ?: return 0L
    val info = ActivityManager.MemoryInfo()
    activityManager.getMemoryInfo(info)
    return info.availMem
}

private fun availableStorageBytes(context: Context): ULong {
    return try {
        val statFs = android.os.StatFs(context.filesDir.absolutePath)
        statFs.availableBytes.toULong()
    } catch (_: Throwable) {
        0UL
    }
}

private fun formatBytes(bytes: ULong): String = when {
    bytes >= 1024UL * 1024UL * 1024UL ->
        String.format("%.1f GiB", bytes.toDouble() / (1024.0 * 1024.0 * 1024.0))
    bytes >= 1024UL * 1024UL ->
        String.format("%.1f MiB", bytes.toDouble() / (1024.0 * 1024.0))
    else -> "$bytes B"
}

private fun localThreadCount(): Int {
    val cores = Runtime.getRuntime().availableProcessors()
    return max(1, min(4, cores - 2))
}

private object AndroidLlamaEngine {
    @Volatile
    private var initialized = false
    @Volatile
    private var terminalFailure: String? = null

    @Synchronized
    fun ensureInitialized(context: Context) {
        terminalFailure?.let { failure ->
            throw LocalLlmException.Unsupported(failure)
        }
        if (initialized) return
        try {
            System.loadLibrary("ggml-base")
            System.loadLibrary("ggml-cpu")
            System.loadLibrary("ggml")
            System.loadLibrary("llama")
            System.loadLibrary("mango_local_llama")
            nativeInit(context.applicationInfo.nativeLibraryDir)
            initialized = true
        } catch (error: UnsatisfiedLinkError) {
            terminalFailure = error.message
            throw LocalLlmException.Unsupported(error.message ?: "Local LLM runtime is unavailable")
        } catch (error: Throwable) {
            terminalFailure = error.message
            throw LocalLlmException.LoadFailed(error.message ?: error.javaClass.simpleName)
        }
    }

    external fun nativeInit(nativeLibDir: String)
    external fun nativeLoadModel(modelPath: String, contextSize: Int, threadCount: Int): Int
    external fun nativeProcessPrompt(prompt: String, maxTokens: Int): Int
    external fun nativeNextToken(): String?
    external fun nativeLastError(): String?
    external fun nativeUnload()

    @Synchronized
    fun unloadIfInitialized() {
        if (!initialized) return
        try {
            nativeUnload()
        } finally {
            initialized = false
        }
    }
}
