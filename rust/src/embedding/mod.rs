/// Local embedding abstraction for the RAG pipeline.
///
/// Phase 8: EmbeddingProvider is a UniFFI callback_interface so mobile platforms
/// can inject their own native implementation (Core ML on iOS, XNNPACK on Android)
/// while the Rust core owns the chunking, indexing, and retrieval logic.
///
/// Desktop uses DesktopEmbeddingProvider (fastembed + ONNX Runtime CPU EP).
/// Mobile platforms register a native callback via UniFFI.

/// Embedding dimension for all-MiniLM-L6-v2 (quantised and full variants).
pub const EMBEDDING_DIM: usize = 384;

/// Capability bridge for embedding generation (per D-12, D-14, LRAG-01, LRAG-08).
///
/// Implementors must be Send + Sync because the actor runs on a dedicated thread.
/// Mobile platforms implement this interface in Swift/Kotlin and register via UniFFI.
/// Desktop uses DesktopEmbeddingProvider.
///
/// embed() takes a batch of texts and returns a flat Vec<f32> of length
/// texts.len() * EMBEDDING_DIM (all embeddings concatenated).
#[uniffi::export(callback_interface)]
pub trait EmbeddingProvider: Send + Sync + 'static {
    /// Embed a batch of texts. Returns a flat Vec<f32> of length
    /// `texts.len() * EMBEDDING_DIM`. Each embedding is 384 f32 values.
    fn embed(&self, texts: Vec<String>) -> Vec<f32>;
}

/// Null embedding provider that returns zero-vectors.
///
/// Used as a placeholder when no EmbeddingProvider has been registered yet
/// (e.g. during startup before the mobile native bridge connects, or in tests
/// that only exercise chunking/context logic without real embeddings).
pub struct NullEmbeddingProvider;

impl EmbeddingProvider for NullEmbeddingProvider {
    fn embed(&self, texts: Vec<String>) -> Vec<f32> {
        vec![0.0f32; texts.len() * EMBEDDING_DIM]
    }
}

/// Desktop embedding provider using fastembed + ONNX Runtime (CPU EP).
///
/// Only compiled on non-iOS, non-Android targets. Mobile platforms inject
/// their own EmbeddingProvider via the UniFFI callback_interface.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub mod desktop;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_provider_single_text() {
        let provider = NullEmbeddingProvider;
        let result = provider.embed(vec!["hello".into()]);
        assert_eq!(
            result.len(),
            EMBEDDING_DIM,
            "Single text should return 384 floats"
        );
        assert!(
            result.iter().all(|&v| v == 0.0),
            "NullEmbeddingProvider should return zero vectors"
        );
    }

    #[test]
    fn test_null_provider_batch() {
        let provider = NullEmbeddingProvider;
        let result = provider.embed(vec!["a".into(), "b".into()]);
        assert_eq!(
            result.len(),
            768,
            "Two texts should return 768 floats (2 * 384)"
        );
    }

    #[test]
    fn test_null_provider_empty_batch() {
        let provider = NullEmbeddingProvider;
        let result = provider.embed(vec![]);
        assert_eq!(result.len(), 0, "Empty batch should return empty vec");
    }
}
