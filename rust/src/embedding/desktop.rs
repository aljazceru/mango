/// Desktop embedding provider using fastembed + ONNX Runtime (CPU execution provider).
///
/// Phase 8 (D-13): fastembed auto-downloads the AllMiniLML6V2Q model on first use
/// (quantised INT8 version, ~23 MB). This is acceptable on desktop where the user
/// has internet access and disk space. Mobile platforms use bundled models via the
/// native EmbeddingProvider callback, so this module is cfg-gated to non-mobile.
use std::sync::Mutex;

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

use super::EmbeddingProvider;

/// Desktop embedding provider wrapping fastembed's TextEmbedding.
///
/// `TextEmbedding::embed` takes `&mut self`, so we wrap it in a `Mutex` to satisfy
/// the `EmbeddingProvider: Send + Sync` bound while allowing `&self` in the trait method.
///
/// Construct once per app lifetime and store in ActorState.
/// fastembed loads the ONNX model into memory on `new()`.
pub struct DesktopEmbeddingProvider {
    model: Mutex<TextEmbedding>,
}

impl DesktopEmbeddingProvider {
    /// Create a new DesktopEmbeddingProvider using AllMiniLML6V2Q.
    ///
    /// Downloads the model on first call (~23 MB) and caches it in fastembed's
    /// default cache directory (`~/.cache/huggingface/hub` on Linux/macOS).
    /// Subsequent calls are fast (model is already on disk).
    pub fn new() -> anyhow::Result<Self> {
        let model = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::AllMiniLML6V2Q))?;
        Ok(Self {
            model: Mutex::new(model),
        })
    }
}

impl EmbeddingProvider for DesktopEmbeddingProvider {
    /// Embed a batch of texts using the AllMiniLML6V2Q model.
    ///
    /// Returns a flat Vec<f32> of length `texts.len() * EMBEDDING_DIM`.
    /// Each 384-dimensional embedding is appended sequentially.
    fn embed(&self, texts: Vec<String>) -> Vec<f32> {
        let mut model = match self.model.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("[DesktopEmbeddingProvider] mutex poisoned: {e}");
                return vec![];
            }
        };

        match model.embed(texts, None) {
            Ok(embeddings) => {
                // fastembed returns Vec<Vec<f32>>; flatten to Vec<f32>
                embeddings.into_iter().flatten().collect()
            }
            Err(e) => {
                // Log the error and return empty vec as a safe fallback.
                eprintln!("[DesktopEmbeddingProvider] embed error: {e}");
                vec![]
            }
        }
    }
}

// Mutex<TextEmbedding> is Send+Sync since Mutex implements Send+Sync when T: Send.
// TextEmbedding is Send (fastembed guarantees this for ONNX Runtime sessions).
// The unsafe impls below are not needed since Mutex provides Send+Sync automatically,
// but we confirm this is intentional.

#[cfg(test)]
mod tests {
    /// Integration test: skipped in CI (requires model download).
    /// Run manually with: cargo test -p mango_core --lib -- embedding::desktop::tests -- --ignored
    #[test]
    #[ignore = "requires fastembed model download (~23 MB), run manually"]
    fn test_desktop_provider_embed() {
        use super::super::EmbeddingProvider;
        use super::{super::EMBEDDING_DIM, DesktopEmbeddingProvider};

        let provider = DesktopEmbeddingProvider::new().expect("Should create provider");
        let result = provider.embed(vec!["hello world".into()]);
        assert_eq!(
            result.len(),
            EMBEDDING_DIM,
            "Should return 384-dim embedding"
        );
        // At least some values should be non-zero for real embeddings
        assert!(
            result.iter().any(|&v| v != 0.0),
            "Real embeddings should not be all zeros"
        );
    }
}
