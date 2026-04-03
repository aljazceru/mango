/// HNSW vector index wrapper around the usearch crate.
///
/// Phase 8 (D-16, D-17, D-18, LRAG-03, LRAG-04): provides an in-process
/// HNSW index for approximate nearest-neighbour search over 384-dim f32 vectors.
/// The index is stored on disk in `{data_dir}/embeddings.usearch` and
/// loaded automatically if it exists when `new()` is called.
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

use super::super::embedding::EMBEDDING_DIM;

/// On-disk filename for the serialised HNSW index.
const INDEX_FILENAME: &str = "embeddings.usearch";

/// HNSW vector index for RAG embedding storage and retrieval.
///
/// `add`, `remove`, and `search` are O(log n). The index must be saved
/// explicitly via `save()` -- it is not auto-saved on drop.
pub struct VectorIndex {
    inner: Index,
    path: String,
}

impl VectorIndex {
    /// Open (or create) the HNSW index at `{data_dir}/embeddings.usearch`.
    ///
    /// If the file exists, loads the persisted index. Otherwise creates a fresh
    /// in-memory index with cosine distance and f32 scalar quantisation.
    pub fn new(data_dir: &str) -> anyhow::Result<Self> {
        let path = format!("{}/{}", data_dir, INDEX_FILENAME);

        let options = IndexOptions {
            dimensions: EMBEDDING_DIM,
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            connectivity: 0,
            expansion_add: 0,
            expansion_search: 0,
            multi: false,
        };

        let index = Index::new(&options)?;

        if std::path::Path::new(&path).exists() {
            index.load(&path)?;
        }

        Ok(Self { inner: index, path })
    }

    /// Add a single vector with the given key.
    ///
    /// `key` must be unique. If the same key is added twice, the behaviour is
    /// undefined (usearch does not deduplicate). The caller is responsible for
    /// ensuring uniqueness (use the SQLite chunk rowid as the key).
    pub fn add(&self, key: u64, embedding: &[f32]) -> anyhow::Result<()> {
        self.inner.reserve(self.inner.size() + 1)?;
        self.inner.add(key, embedding)?;
        Ok(())
    }

    /// Search for the `top_k` nearest neighbours to `query`.
    ///
    /// Returns a Vec of `(key, distance)` pairs sorted by distance ascending.
    /// Distance is cosine distance (0.0 = identical, 2.0 = opposite).
    pub fn search(&self, query: &[f32], top_k: usize) -> anyhow::Result<Vec<(u64, f32)>> {
        let results = self.inner.search(query, top_k)?;
        let pairs = results
            .keys
            .into_iter()
            .zip(results.distances.into_iter())
            .collect();
        Ok(pairs)
    }

    /// Remove a vector by key.
    ///
    /// No-op if the key does not exist.
    pub fn remove(&self, key: u64) -> anyhow::Result<()> {
        self.inner.remove(key)?;
        Ok(())
    }

    /// Persist the index to `{data_dir}/embeddings.usearch`.
    ///
    /// Must be called explicitly; the index is not auto-saved.
    pub fn save(&self) -> anyhow::Result<()> {
        self.inner.save(&self.path)?;
        Ok(())
    }

    /// Number of vectors currently in the index.
    pub fn size(&self) -> usize {
        self.inner.size()
    }

    /// The path where this index is (or will be) saved.
    pub fn path(&self) -> &str {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::embedding::EMBEDDING_DIM;
    use super::*;

    /// Build a unit vector pointing mostly in the direction of dimension `d`.
    ///
    /// Places 1.0 at index `d` and 0.01 at all other indices.
    /// This ensures cosine distance distinguishes between different unit directions.
    fn make_direction_vec(d: usize) -> Vec<f32> {
        let mut v = vec![0.01f32; EMBEDDING_DIM];
        v[d % EMBEDDING_DIM] = 1.0;
        // Normalise
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter().map(|x| x / norm).collect()
    }

    #[test]
    fn test_create_empty_index() {
        let dir = tempdir();
        let index = VectorIndex::new(&dir).expect("Should create index");
        assert_eq!(index.size(), 0);
    }

    #[test]
    fn test_add_and_search() {
        let dir = tempdir();
        let index = VectorIndex::new(&dir).expect("Should create index");

        // Add 3 vectors pointing in different directions
        index.add(1, &make_direction_vec(0)).unwrap(); // points in dim 0
        index.add(2, &make_direction_vec(100)).unwrap(); // points in dim 100
        index.add(3, &make_direction_vec(200)).unwrap(); // points in dim 200
        assert_eq!(index.size(), 3);

        // Search for the vector closest to dim 0 direction -- should be key=1
        let results = index.search(&make_direction_vec(0), 1).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].0, 1,
            "Nearest to dim-0 direction should be key=1"
        );
    }

    #[test]
    fn test_remove_vector() {
        let dir = tempdir();
        let index = VectorIndex::new(&dir).expect("Should create index");

        index.add(10, &make_direction_vec(0)).unwrap();
        index.add(20, &make_direction_vec(100)).unwrap();
        assert_eq!(index.size(), 2);

        index.remove(10).unwrap();
        assert_eq!(index.size(), 1);

        // Search should not return key=10 anymore
        let results = index.search(&make_direction_vec(0), 2).unwrap();
        let keys: Vec<u64> = results.iter().map(|(k, _)| *k).collect();
        assert!(
            !keys.contains(&10),
            "Removed key should not appear in search results"
        );
    }

    #[test]
    fn test_save_and_load_round_trip() {
        let dir = tempdir();

        // Create and populate index with directionally distinct vectors
        {
            let index = VectorIndex::new(&dir).expect("Should create index");
            index.add(100, &make_direction_vec(10)).unwrap(); // points in dim 10
            index.add(200, &make_direction_vec(300)).unwrap(); // points in dim 300
            index.save().expect("Should save index");
        }

        // Load from disk and verify same results
        {
            let index = VectorIndex::new(&dir).expect("Should load index");
            assert_eq!(index.size(), 2, "Loaded index should have 2 vectors");

            let results = index.search(&make_direction_vec(10), 1).unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(
                results[0].0, 100,
                "After round-trip, nearest to dim-10 direction should be key=100"
            );
        }
    }

    fn tempdir() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let path = format!("/tmp/test_vector_index_{}", nonce);
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}
