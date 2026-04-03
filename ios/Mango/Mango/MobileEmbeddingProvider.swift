import Foundation
import onnxruntime_objc

// MARK: - MobileEmbeddingProvider

/// On-device embedding provider for iOS using ONNX Runtime with CoreML execution provider.
///
/// Runs all-MiniLM-L6-v2 INT8 quantized (ARM64) via the Apple Neural Engine / CPU.
/// Conforms to the UniFFI-generated EmbeddingProvider protocol.
///
/// Model I/O:
///   Inputs:  input_ids [1, 256], attention_mask [1, 256], token_type_ids [1, 256] (int64)
///   Output:  last_hidden_state [1, 256, 384] (float32) — requires mean pooling + L2 norm
///
/// The output tensor name is last_hidden_state (shape [batch, seq_len, 384]).
/// Mean pooling is applied masked by attention_mask, then L2 normalization produces
/// a unit-norm 384-dimensional embedding suitable for cosine similarity search.
class MobileEmbeddingProvider: EmbeddingProvider {
    private static let embeddingDim = 384
    private static let maxLength = 256

    private let session: ORTSession
    private let tokenizer: WordPieceTokenizer

    /// Initialize the ONNX Runtime session with CoreML EP and load the tokenizer.
    ///
    /// Loads model_optimized.onnx and tokenizer.json from the app bundle.
    /// Throws EmbeddingError if either resource is missing or fails to load.
    init() throws {
        // Locate bundled model and tokenizer resources
        guard let modelURL = Bundle.main.url(forResource: "model_optimized", withExtension: "onnx") else {
            throw EmbeddingError.modelNotFound
        }
        guard let tokenizerURL = Bundle.main.url(forResource: "tokenizer", withExtension: "json") else {
            throw EmbeddingError.tokenizerNotFound
        }

        // Initialize tokenizer from bundled tokenizer.json
        self.tokenizer = try WordPieceTokenizer(tokenizerURL: tokenizerURL)

        // Create ORT environment with warning-level logging
        let env = try ORTEnv(loggingLevel: .warning)

        // Configure session options with CoreML execution provider
        // CPUAndNeuralEngine: routes compute to Apple Neural Engine when available,
        // falls back to CPU for unsupported ops. MLProgram format is required for ANE.
        let options = try ORTSessionOptions()
        try options.appendCoreMLExecutionProvider(withOptionsV2: [
            "MLComputeUnits": "CPUAndNeuralEngine",
            "ModelFormat": "MLProgram"
        ])

        // Create the ONNX Runtime session
        self.session = try ORTSession(env: env, modelPath: modelURL.path, sessionOptions: options)

        print("[MobileEmbeddingProvider] Loaded ONNX model with CoreML EP")
    }

    // MARK: - EmbeddingProvider Conformance

    /// Embed a batch of texts. Returns a flat [Float] of length texts.count * 384.
    ///
    /// Called synchronously from the Rust actor thread via UniFFI (Rust wraps in spawn_blocking).
    /// On any per-text error, returns 384 zero floats for that text — does not crash or throw.
    func embed(texts: [String]) -> [Float] {
        var result = [Float]()
        result.reserveCapacity(texts.count * Self.embeddingDim)

        for text in texts {
            let embedding = embedSingle(text)
            result.append(contentsOf: embedding)
        }

        return result
    }

    // MARK: - Private Inference

    /// Run inference for a single text. Returns 384-dim L2-normalized embedding,
    /// or 384 zero floats on any error.
    private func embedSingle(_ text: String) -> [Float] {
        let zeros = [Float](repeating: 0.0, count: Self.embeddingDim)

        do {
            let tokens = tokenizer.tokenize(text, maxLength: Self.maxLength)

            // Create ORTValue tensors from Int64 arrays
            let inputIdsTensor = try makeTensor(tokens.inputIds)
            let attentionMaskTensor = try makeTensor(tokens.attentionMask)
            let tokenTypeIdsTensor = try makeTensor(tokens.tokenTypeIds)

            // Run ONNX Runtime inference
            let inputs: [String: ORTValue] = [
                "input_ids": inputIdsTensor,
                "attention_mask": attentionMaskTensor,
                "token_type_ids": tokenTypeIdsTensor
            ]
            let outputNames: Set<String> = ["last_hidden_state"]
            let outputs = try session.run(
                withInputs: inputs,
                outputNames: outputNames,
                runOptions: nil
            )

            guard let outputTensor = outputs["last_hidden_state"] else {
                return zeros
            }

            // Apply mean pooling masked by attention_mask, then L2 normalize
            return try meanPoolAndNormalize(outputTensor, attentionMask: tokens.attentionMask)
        } catch {
            print("[MobileEmbeddingProvider] embed error: \(error)")
            return zeros
        }
    }

    /// Create an ORTValue int64 tensor from an [Int64] array with shape [1, array.count].
    private func makeTensor(_ values: [Int64]) throws -> ORTValue {
        var mutableValues = values
        let data = NSMutableData(
            bytes: &mutableValues,
            length: mutableValues.count * MemoryLayout<Int64>.size
        )
        return try ORTValue(
            tensorData: data,
            elementType: .int64,
            shape: [1, NSNumber(value: values.count)]
        )
    }

    // MARK: - Mean Pooling + L2 Normalization

    /// Extract last_hidden_state [1, seqLen, 384], apply masked mean pooling,
    /// then L2-normalize to produce a unit-norm 384-dim embedding.
    private func meanPoolAndNormalize(_ outputTensor: ORTValue, attentionMask: [Int64]) throws -> [Float] {
        let embeddingDim = Self.embeddingDim
        let seqLen = Self.maxLength

        // Extract raw float32 data from [1, seqLen, 384] output tensor
        let rawData = try outputTensor.tensorData() as Data
        let floats = rawData.withUnsafeBytes { Array($0.bindMemory(to: Float.self)) }

        // Mean pool: sum token embeddings where attention_mask == 1, divide by count
        var mean = [Float](repeating: 0.0, count: embeddingDim)
        var count: Float = 0.0

        for i in 0..<seqLen {
            guard i < attentionMask.count, attentionMask[i] == 1 else { continue }
            let offset = i * embeddingDim
            guard offset + embeddingDim <= floats.count else { break }
            for d in 0..<embeddingDim {
                mean[d] += floats[offset + d]
            }
            count += 1.0
        }

        // Divide by attended token count (guard against zero-length input)
        let divisor = max(count, 1e-9)
        for d in 0..<embeddingDim {
            mean[d] /= divisor
        }

        // L2 normalize to unit norm (required for cosine similarity search)
        var norm: Float = 0.0
        for d in 0..<embeddingDim {
            norm += mean[d] * mean[d]
        }
        norm = sqrt(norm)
        let normDivisor = max(norm, 1e-12)
        for d in 0..<embeddingDim {
            mean[d] /= normDivisor
        }

        return mean
    }
}
