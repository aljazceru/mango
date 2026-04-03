package dev.disobey.mango

import android.content.Context
import ai.onnxruntime.OnnxTensor
import ai.onnxruntime.OrtEnvironment
import ai.onnxruntime.OrtSession
import dev.disobey.mango.rust.EmbeddingProvider
import java.io.File
import java.nio.LongBuffer
import kotlin.math.max
import kotlin.math.sqrt

/**
 * On-device embedding provider using ONNX Runtime with XNNPACK execution provider.
 *
 * Runs all-MiniLM-L6-v2 INT8 quantized model to produce L2-normalized 384-dimensional
 * embeddings for use in the RAG pipeline. Implements the UniFFI EmbeddingProvider
 * callback interface so the Rust core can dispatch embed() calls into native Kotlin.
 *
 * Phase 11, EMBD-03/EMBD-04/EMBD-05.
 */
class MobileEmbeddingProvider(context: Context) : EmbeddingProvider {
    private val ortEnv = OrtEnvironment.getEnvironment()
    private val session: OrtSession
    private val tokenizer: WordPieceTokenizer
    private val maxLength = 256
    private val embeddingDim = 384

    init {
        // ONNX Runtime requires a file path (not an InputStream) for session creation.
        // Copy model from assets to internal storage on first launch. Uses double-checked
        // locking so concurrent calls during app startup do not race on the file write.
        val modelFile = File(context.filesDir, "model_optimized.onnx")
        if (!modelFile.exists()) {
            synchronized(MobileEmbeddingProvider::class.java) {
                if (!modelFile.exists()) {
                    context.assets.open("model_optimized.onnx").use { input ->
                        modelFile.outputStream().use { output -> input.copyTo(output) }
                    }
                }
            }
        }

        // Build session options. Attempt to enable XNNPACK EP which accelerates
        // quantized INT8 ops on ARM CPUs. Falls back silently to default CPU EP
        // on devices where XNNPACK is unavailable.
        val options = OrtSession.SessionOptions()
        try {
            options.addXnnpack(
                mapOf("intra_op_num_threads" to Runtime.getRuntime().availableProcessors().toString())
            )
            android.util.Log.i("MobileEmbeddingProvider", "XNNPACK EP enabled")
        } catch (e: Exception) {
            android.util.Log.w(
                "MobileEmbeddingProvider",
                "XNNPACK EP unavailable, using CPU EP: ${e.message}"
            )
        }

        session = ortEnv.createSession(modelFile.absolutePath, options)

        // Load tokenizer vocabulary from bundled assets JSON
        val tokenizerJson = context.assets.open("tokenizer.json")
            .bufferedReader()
            .use { it.readText() }
        tokenizer = WordPieceTokenizer(tokenizerJson)

        android.util.Log.i(
            "MobileEmbeddingProvider",
            "Initialised: model=${modelFile.length() / 1_048_576}MB, vocab=${tokenizerJson.length / 1024}KB"
        )
    }

    /**
     * Embed a batch of texts. Returns a flat List<Float> of length texts.size * 384.
     * Each 384-float slice is an L2-normalised embedding for the corresponding text.
     * Returns 384 zero floats per text on per-text inference errors (does not crash).
     */
    override fun embed(texts: List<String>): List<Float> {
        val result = mutableListOf<Float>()
        for (text in texts) {
            try {
                val tokens = tokenizer.tokenize(text, maxLength)
                val embedding = runInference(tokens)
                for (v in embedding) result.add(v)
            } catch (e: Exception) {
                android.util.Log.e("MobileEmbeddingProvider", "Inference error for text: ${e.message}")
                // Fallback: return zero vector for this text so the pipeline does not crash
                repeat(embeddingDim) { result.add(0.0f) }
            }
        }
        return result
    }

    /**
     * Run a single-text ONNX inference pass and return a 384-float L2-normalised embedding.
     *
     * The model (all-MiniLM-L6-v2 ARM64 INT8) outputs last_hidden_state of shape
     * [1, seq_len, 384]. Mean pooling over unmasked token positions then L2 normalisation
     * produces a single embedding comparable to those from fastembed (DesktopEmbeddingProvider).
     */
    private fun runInference(tokens: TokenizerOutput): FloatArray {
        val shape = longArrayOf(1L, maxLength.toLong())

        val inputIdsTensor = OnnxTensor.createTensor(
            ortEnv, LongBuffer.wrap(tokens.inputIds), shape
        )
        val attentionMaskTensor = OnnxTensor.createTensor(
            ortEnv, LongBuffer.wrap(tokens.attentionMask), shape
        )
        val tokenTypeIdsTensor = OnnxTensor.createTensor(
            ortEnv, LongBuffer.wrap(tokens.tokenTypeIds), shape
        )

        val inputs = mapOf(
            "input_ids" to inputIdsTensor,
            "attention_mask" to attentionMaskTensor,
            "token_type_ids" to tokenTypeIdsTensor,
        )

        val results = session.run(inputs)

        // Release input tensors immediately after run to avoid GC pressure
        inputIdsTensor.close()
        attentionMaskTensor.close()
        tokenTypeIdsTensor.close()

        return try {
            // last_hidden_state: Array<Array<FloatArray>> shape [1][seq_len][384]
            @Suppress("UNCHECKED_CAST")
            val hiddenState = results[0].value as Array<Array<FloatArray>>
            val tokenEmbeddings = hiddenState[0]  // shape [seq_len][384]
            meanPoolAndNormalize(tokenEmbeddings, tokens.attentionMask)
        } finally {
            results.close()
        }
    }

    /**
     * Mean pool token embeddings over positions where attentionMask == 1L,
     * then L2-normalise the result. Matches fastembed mean pooling behaviour.
     */
    private fun meanPoolAndNormalize(
        tokenEmbeddings: Array<FloatArray>,
        attentionMask: LongArray
    ): FloatArray {
        val mean = FloatArray(embeddingDim)
        var count = 0f
        val seqLen = minOf(tokenEmbeddings.size, attentionMask.size)
        for (i in 0 until seqLen) {
            if (attentionMask[i] == 1L) {
                val tok = tokenEmbeddings[i]
                for (d in 0 until embeddingDim) {
                    mean[d] += tok[d]
                }
                count += 1f
            }
        }
        val divisor = max(count, 1e-9f)
        for (d in 0 until embeddingDim) {
            mean[d] /= divisor
        }
        return l2Normalize(mean)
    }

    /**
     * L2-normalise a vector in-place. Guards against division by near-zero norm
     * (epsilon 1e-12) to avoid NaN propagation when all token embeddings are zero.
     */
    private fun l2Normalize(vec: FloatArray): FloatArray {
        var norm = 0f
        for (v in vec) {
            norm += v * v
        }
        norm = sqrt(norm)
        val divisor = max(norm, 1e-12f)
        for (d in vec.indices) {
            vec[d] /= divisor
        }
        return vec
    }
}
