/// HNSW vector index wrapper around the usearch crate.
///
/// Phase 8 (D-16, D-17, D-18, LRAG-03, LRAG-04): provides an in-process
/// HNSW index for approximate nearest-neighbour search over 384-dim f32 vectors.
/// The index is stored on disk in `{data_dir}/embeddings.usearch` and
/// loaded automatically if it exists when `new()` is called.
///
/// Phase 28 (ENC-02, ENC-03): optional AES-256-GCM encryption via DEK parameter.
/// When a DEK is provided, the index file is encrypted with MGO1 magic header.
/// Legacy unencrypted files are detected and loaded transparently.
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

use super::super::crypto::file_crypto;
use super::super::embedding::EMBEDDING_DIM;

/// On-disk filename for the serialised HNSW index.
const INDEX_FILENAME: &str = "embeddings.usearch";

/// MGO1 magic header identifying encrypted files.
const MAGIC: &[u8; 4] = b"MGO1";

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
    ///
    /// When `dek` is `Some`, the on-disk file is expected to be encrypted with
    /// `crypto::file_crypto::encrypt_file`. Legacy unencrypted files (no MGO1 header)
    /// are loaded transparently regardless of whether `dek` is provided.
    pub fn new(data_dir: &str, dek: Option<&[u8; 32]>) -> anyhow::Result<Self> {
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
            let file_bytes = std::fs::read(&path)?;

            // Detect MGO1 magic header to determine if file is encrypted
            let is_encrypted = file_bytes.len() >= 4 && &file_bytes[..4] == MAGIC;

            if is_encrypted {
                match dek {
                    Some(key) => {
                        // Decrypt to a temp file, load, then delete (T-28-15)
                        let plaintext = file_crypto::decrypt_file(key, &file_bytes)?;
                        let tmp_path = format!("{}.tmp", path);
                        write_restricted(&tmp_path, &plaintext)?;
                        let load_result = index.load(&tmp_path);
                        let _ = std::fs::remove_file(&tmp_path);
                        load_result?;
                    }
                    None => {
                        anyhow::bail!(
                            "index file at {} is encrypted (MGO1 header) but no DEK was provided",
                            path
                        );
                    }
                }
            } else {
                // Legacy unencrypted file -- load directly
                index.load(&path)?;
            }
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
        let pairs = results.keys.into_iter().zip(results.distances).collect();
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
    /// When `dek` is `Some`, saves to a temp file, reads bytes, encrypts with
    /// AES-256-GCM (MGO1 format), then writes the encrypted blob to `self.path`.
    /// The temp file is deleted immediately after encryption (T-28-15).
    ///
    /// When `dek` is `None`, saves directly (backwards-compatible unencrypted).
    pub fn save(&self, dek: Option<&[u8; 32]>) -> anyhow::Result<()> {
        match dek {
            Some(key) => {
                let tmp_path = format!("{}.tmp", self.path);
                // Save unencrypted to temp file (T-28-15: restrictive permissions)
                self.inner.save(&tmp_path)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ =
                        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600));
                }
                let plaintext = std::fs::read(&tmp_path);
                let _ = std::fs::remove_file(&tmp_path);
                let plaintext = plaintext?;

                // Encrypt and write final file
                let ciphertext = file_crypto::encrypt_file(key, &plaintext);
                std::fs::write(&self.path, &ciphertext)?;
            }
            None => {
                self.inner.save(&self.path)?;
            }
        }
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

/// Write `data` to `path` with restricted permissions (0600 on Unix).
///
/// Used for temp files holding decrypted index bytes (T-28-15).
fn write_restricted(path: &str, data: &[u8]) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(data)?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, data)?;
    }
    Ok(())
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

    fn test_dek() -> [u8; 32] {
        [42u8; 32]
    }

    fn wrong_dek() -> [u8; 32] {
        [99u8; 32]
    }

    #[test]
    fn test_create_empty_index() {
        let dir = tempdir();
        let index = VectorIndex::new(&dir, None).expect("Should create index");
        assert_eq!(index.size(), 0);
    }

    #[test]
    fn test_add_and_search() {
        let dir = tempdir();
        let index = VectorIndex::new(&dir, None).expect("Should create index");

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
        let index = VectorIndex::new(&dir, None).expect("Should create index");

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
            let index = VectorIndex::new(&dir, None).expect("Should create index");
            index.add(100, &make_direction_vec(10)).unwrap(); // points in dim 10
            index.add(200, &make_direction_vec(300)).unwrap(); // points in dim 300
            index.save(None).expect("Should save index");
        }

        // Load from disk and verify same results
        {
            let index = VectorIndex::new(&dir, None).expect("Should load index");
            assert_eq!(index.size(), 2, "Loaded index should have 2 vectors");

            let results = index.search(&make_direction_vec(10), 1).unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(
                results[0].0, 100,
                "After round-trip, nearest to dim-10 direction should be key=100"
            );
        }
    }

    /// Encrypted round-trip: save with DEK, load with same DEK, vectors intact.
    #[test]
    fn test_encrypted_save_and_load_round_trip() {
        let dir = tempdir();
        let dek = test_dek();

        {
            let index = VectorIndex::new(&dir, None).expect("Should create index");
            index.add(100, &make_direction_vec(10)).unwrap();
            index.add(200, &make_direction_vec(300)).unwrap();
            index.save(Some(&dek)).expect("Should save encrypted");
        }

        // Verify the file has MGO1 header
        let file_path = format!("{}/{}", dir, INDEX_FILENAME);
        let file_bytes = std::fs::read(&file_path).unwrap();
        assert_eq!(&file_bytes[..4], MAGIC, "File should have MGO1 header");

        {
            let index = VectorIndex::new(&dir, Some(&dek)).expect("Should load encrypted index");
            assert_eq!(
                index.size(),
                2,
                "Loaded encrypted index should have 2 vectors"
            );

            let results = index.search(&make_direction_vec(10), 1).unwrap();
            assert_eq!(results.len(), 1);
            assert_eq!(
                results[0].0, 100,
                "After encrypted round-trip, nearest to dim-10 direction should be key=100"
            );
        }
    }

    /// Loading with a wrong DEK returns an error.
    #[test]
    fn test_wrong_dek_returns_error() {
        let dir = tempdir();
        let dek = test_dek();
        let bad = wrong_dek();

        {
            let index = VectorIndex::new(&dir, None).expect("Should create index");
            index.add(1, &make_direction_vec(0)).unwrap();
            index.save(Some(&dek)).expect("Should save encrypted");
        }

        let result = VectorIndex::new(&dir, Some(&bad));
        assert!(result.is_err(), "Loading with wrong DEK should fail");
    }

    /// Legacy unencrypted file (no MGO1 header) loads transparently even when dek is Some.
    #[test]
    fn test_legacy_unencrypted_loads_transparently() {
        let dir = tempdir();

        // Save without DEK (legacy format)
        {
            let index = VectorIndex::new(&dir, None).expect("Should create index");
            index.add(50, &make_direction_vec(50)).unwrap();
            index.save(None).expect("Should save unencrypted");
        }

        // Verify no MGO1 header
        let file_path = format!("{}/{}", dir, INDEX_FILENAME);
        let file_bytes = std::fs::read(&file_path).unwrap();
        assert_ne!(
            &file_bytes[..4],
            MAGIC,
            "Legacy file should not have MGO1 header"
        );

        // Load with a DEK -- should detect legacy format and load transparently
        let dek = test_dek();
        let index =
            VectorIndex::new(&dir, Some(&dek)).expect("Legacy file should load transparently");
        assert_eq!(index.size(), 1, "Legacy index should load with 1 vector");
    }

    /// Encrypted file with no DEK returns error.
    #[test]
    fn test_encrypted_file_no_dek_returns_error() {
        let dir = tempdir();
        let dek = test_dek();

        {
            let index = VectorIndex::new(&dir, None).expect("Should create index");
            index.add(1, &make_direction_vec(0)).unwrap();
            index.save(Some(&dek)).expect("Should save encrypted");
        }

        let result = VectorIndex::new(&dir, None);
        assert!(
            result.is_err(),
            "Encrypted file with no DEK should return error"
        );
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
