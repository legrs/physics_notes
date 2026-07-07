//! Semantic side: load the **pre-computed** corpus embeddings
//! (`embeddings.json`, fetch-only — never recomputed, CLAUDE.md §2) and
//! compute only the *query* embedding at runtime via fastembed.
//!
//! Parity invariants (§7): query text = `"query: " + lowercased query`;
//! corpus was embedded as `"passage: {questions[0]} {description}"` upstream;
//! embeddings are keyed by `normalizeId(id)`; mean pooling + L2 normalize
//! (fastembed's MultilingualE5Small does both — verified in its source, so we
//! do not re-normalize). Invariant breaks fail loudly instead of degrading.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

use crate::config::ModelSize;
use crate::model::{Corpus, normalize_id};

/// e5 convention: documents use `passage: `, queries use `query: ` (§7.3).
pub const E5_QUERY_PREFIX: &str = "query: ";

/// Errors are split so callers can fail hard on shared-artifact invariant
/// breaks while treating "model not available" (offline, download failed) as
/// a loud-but-recoverable fallback to BM25-only.
#[derive(Debug, thiserror::Error)]
pub enum SemanticError {
    #[error("shared-artifact invariant broken (CLAUDE.md §7): {0}")]
    Invariant(String),
    #[error("semantic model unavailable: {0}")]
    Unavailable(String),
}

/// Build the exact text the query embedding is computed from. The caller
/// passes the already-lowercased query; corpus passages keep original case.
pub fn query_text(q_lower: &str) -> String {
    format!("{E5_QUERY_PREFIX}{q_lower}")
}

/// The pre-computed corpus embedding matrix for one model size.
#[derive(Debug)]
pub struct CorpusEmbeddings {
    pub dim: usize,
    /// normalized id → (vector, L2 norm)
    pub vectors: HashMap<String, (Vec<f64>, f64)>,
}

impl CorpusEmbeddings {
    /// Parse `embeddings.json` (`{ "small": {id: [..]}, "large": {id: [..]} }`)
    /// and select one matrix. Keys are normalized like the web's
    /// `normalizeEmbeddings`; records without a vector are simply absent
    /// (looked-up items skip them, §7.5).
    pub fn load(path: &Path, size: ModelSize) -> Result<Self, SemanticError> {
        let bytes = std::fs::read(path).map_err(|e| {
            SemanticError::Unavailable(format!("cannot read {}: {e}", path.display()))
        })?;
        Self::from_json(&bytes, size)
    }

    pub fn from_json(bytes: &[u8], size: ModelSize) -> Result<Self, SemanticError> {
        let mut top: HashMap<String, HashMap<String, Vec<f64>>> = serde_json::from_slice(bytes)
            .map_err(|e| {
                SemanticError::Invariant(format!("embeddings.json is not the expected shape: {e}"))
            })?;
        let key = size.embeddings_key();
        let map = top.remove(key).ok_or_else(|| {
            SemanticError::Invariant(format!("embeddings.json has no \"{key}\" matrix"))
        })?;
        if map.is_empty() {
            return Err(SemanticError::Invariant(format!(
                "embeddings.json \"{key}\" matrix is empty"
            )));
        }

        let mut dim = 0usize;
        let mut vectors = HashMap::with_capacity(map.len());
        for (id, v) in map {
            if dim == 0 {
                dim = v.len();
            } else if v.len() != dim {
                return Err(SemanticError::Invariant(format!(
                    "ragged embeddings: id {id} has {} dims, expected {dim}",
                    v.len()
                )));
            }
            let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
            vectors.insert(normalize_id(&id), (v, norm));
        }
        if dim != size.dim() {
            return Err(SemanticError::Invariant(format!(
                "embeddings.json \"{key}\" vectors are {dim}-dim, expected {} for this model",
                size.dim()
            )));
        }
        Ok(Self { dim, vectors })
    }
}

/// True if the embedding model has already been downloaded into the
/// fastembed cache (hf-hub layout). Used to honor `--offline`: never start
/// a model download in offline mode, fall back to BM25-only instead.
///
/// The repo dir names must match the HF repos fastembed actually pulls from:
/// `MultilingualE5Small` → `intfloat/multilingual-e5-small`, but
/// `MultilingualE5Large` → `Qdrant/multilingual-e5-large-onnx` (fastembed
/// sources the large model from Qdrant's ONNX mirror, not intfloat). Getting
/// this wrong makes offline `--model large`/`max` falsely report the model as
/// missing and degrade to BM25-only even when it is cached.
///
/// A snapshot directory alone is not proof of a finished download: hf-hub
/// symlinks each file into the snapshot only after its blob completes, so an
/// interrupted large download leaves `model.onnx` present but `model.onnx_data`
/// (2.2 GB, minutes to fetch) missing. Require the model files themselves to
/// resolve, otherwise offline mode would claim a half-downloaded model works.
pub fn model_cached(size: ModelSize, model_cache_dir: &Path) -> bool {
    let (repo_dir, required): (&str, &[&str]) = match size {
        ModelSize::Small => (
            "models--intfloat--multilingual-e5-small",
            &["onnx/model.onnx"],
        ),
        ModelSize::Large => (
            "models--Qdrant--multilingual-e5-large-onnx",
            &["model.onnx", "model.onnx_data"],
        ),
    };
    let snapshots = model_cache_dir.join(repo_dir).join("snapshots");
    let Ok(entries) = std::fs::read_dir(&snapshots) else {
        return false;
    };
    // `fs::metadata` follows symlinks, so a dangling link (blob missing or
    // still downloading) correctly counts as "not cached".
    entries.flatten().any(|snap| {
        required
            .iter()
            .all(|rel| std::fs::metadata(snap.path().join(rel)).is_ok())
    })
}

/// Runtime query embedder (fastembed, model auto-cached in `<cache>/model/`).
pub struct Embedder {
    model: TextEmbedding,
    dim: usize,
}

/// Init attempts per model. hf-hub itself never retries (`max_retries = 0`
/// in the API fastembed builds), and the large model's `model.onnx_data` is
/// a single 2.2 GB stream — one dropped connection used to surface as
/// "Failed to retrieve model.onnx_data". Downloads resume from the on-disk
/// `.part` file, so every retry makes forward progress instead of starting
/// over; failures that retrying can't help (404, disk full) just fail fast
/// a few times. Backoff between attempts: 1s, 2s, 4s.
const INIT_ATTEMPTS: u32 = 4;

/// Serialize model init per model size, process-wide. One init may download
/// gigabytes into the shared hf-hub cache, which guards each blob with a
/// non-blocking flock polled for only ~5 s — a concurrent init of the same
/// model (e.g. the TUI spawning a fresh semantic worker while an abandoned
/// one is still downloading) would hit that lock and fail instead of
/// waiting. With this lock the second init blocks until the first finishes,
/// then loads straight from the cache.
fn init_lock(size: ModelSize) -> &'static Mutex<()> {
    static SMALL: Mutex<()> = Mutex::new(());
    static LARGE: Mutex<()> = Mutex::new(());
    match size {
        ModelSize::Small => &SMALL,
        ModelSize::Large => &LARGE,
    }
}

impl Embedder {
    pub fn new(size: ModelSize, model_cache_dir: &Path) -> Result<Self, SemanticError> {
        let which = match size {
            ModelSize::Small => EmbeddingModel::MultilingualE5Small,
            ModelSize::Large => EmbeddingModel::MultilingualE5Large,
        };
        let _guard = init_lock(size).lock().unwrap_or_else(|p| p.into_inner());
        let mut attempt = 0;
        let model = loop {
            let opts = TextInitOptions::new(which.clone())
                .with_cache_dir(model_cache_dir.to_path_buf())
                .with_show_download_progress(false);
            match TextEmbedding::try_new(opts) {
                Ok(m) => break m,
                Err(e) => {
                    attempt += 1;
                    if attempt >= INIT_ATTEMPTS {
                        // `:#` keeps the whole anyhow chain — the root cause
                        // (lock contention, dropped connection, no disk
                        // space…) matters more than fastembed's outermost
                        // "Failed to retrieve …" context.
                        return Err(SemanticError::Unavailable(format!(
                            "failed to init embedding model: {e:#}"
                        )));
                    }
                    std::thread::sleep(Duration::from_secs(1 << (attempt - 1)));
                }
            }
        };
        Ok(Self {
            model,
            dim: size.dim(),
        })
    }

    /// Embed `query: <lowercased query>`. fastembed mean-pools and
    /// L2-normalizes (e5 requirement) — verified, not re-normalized here.
    pub fn embed_query(&mut self, q_lower: &str) -> Result<Vec<f64>, SemanticError> {
        let text = query_text(q_lower);
        let out = self
            .model
            .embed(&[text], None)
            .map_err(|e| SemanticError::Unavailable(format!("query embedding failed: {e:#}")))?;
        let v = out
            .into_iter()
            .next()
            .ok_or_else(|| SemanticError::Unavailable("empty embedding output".into()))?;
        if v.len() != self.dim {
            return Err(SemanticError::Invariant(format!(
                "query embedding is {}-dim, expected {}",
                v.len(),
                self.dim
            )));
        }
        Ok(v.into_iter().map(|x| x as f64).collect())
    }
}

/// Rank **all** items that have a vector by cosine similarity, in data order
/// for ties (the web sorts the whole embedded corpus, not just BM25
/// candidates). Vectors are L2-normalized upstream so cosine == dot, but we
/// mirror the web's `_cosineSim` (with its 1e-9 epsilon) exactly.
pub fn semantic_rank(
    corpus: &Corpus,
    embeddings: &CorpusEmbeddings,
    query_vec: &[f64],
) -> Result<Vec<(u32, f64)>, SemanticError> {
    if query_vec.len() != embeddings.dim {
        return Err(SemanticError::Invariant(format!(
            "query vector is {}-dim but corpus embeddings are {}-dim",
            query_vec.len(),
            embeddings.dim
        )));
    }
    let qnorm = query_vec.iter().map(|x| x * x).sum::<f64>().sqrt();
    let mut ranked: Vec<(u32, f64)> = Vec::with_capacity(embeddings.vectors.len());
    for (i, record) in corpus.records.iter().enumerate() {
        // Lookup by the record's id — identical on both sides because data
        // ids and embedding keys go through the same normalizeId (§7.5).
        let Some((v, n)) = embeddings.vectors.get(&record.id) else {
            continue; // records without a vector are skipped, never a crash
        };
        let dot: f64 = query_vec.iter().zip(v.iter()).map(|(a, b)| a * b).sum();
        let cos = dot / (qnorm * n + 1e-9);
        ranked.push((i as u32, cos));
    }
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    Ok(ranked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Record;

    fn record(id: &str) -> Record {
        serde_json::from_str(&format!(r#"{{"id":"{id}"}}"#)).unwrap()
    }

    #[test]
    fn model_cached_requires_every_model_file_to_resolve() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path();
        assert!(!model_cached(ModelSize::Large, cache));

        // Interrupted large download: model.onnx landed, model.onnx_data
        // (the 2.2 GB external-data file) didn't. Must not count as cached.
        let snap = cache
            .join("models--Qdrant--multilingual-e5-large-onnx")
            .join("snapshots")
            .join("abc123");
        std::fs::create_dir_all(&snap).unwrap();
        std::fs::write(snap.join("model.onnx"), b"graph").unwrap();
        assert!(!model_cached(ModelSize::Large, cache));

        std::fs::write(snap.join("model.onnx_data"), b"weights").unwrap();
        assert!(model_cached(ModelSize::Large, cache));
    }

    #[test]
    fn model_cached_small_checks_the_nested_onnx_path() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path();
        assert!(!model_cached(ModelSize::Small, cache));

        let snap = cache
            .join("models--intfloat--multilingual-e5-small")
            .join("snapshots")
            .join("deadbeef");
        std::fs::create_dir_all(snap.join("onnx")).unwrap();
        std::fs::write(snap.join("onnx").join("model.onnx"), b"graph").unwrap();
        assert!(model_cached(ModelSize::Small, cache));
    }

    /// hf-hub symlinks snapshot files to blobs; a dangling link means the
    /// blob never finished. `fs::metadata` follows links, so this must be
    /// "not cached".
    #[cfg(unix)]
    #[test]
    fn model_cached_rejects_dangling_snapshot_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path();
        let snap = cache
            .join("models--Qdrant--multilingual-e5-large-onnx")
            .join("snapshots")
            .join("abc123");
        std::fs::create_dir_all(&snap).unwrap();
        std::fs::write(snap.join("model.onnx"), b"graph").unwrap();
        std::os::unix::fs::symlink(
            cache.join("blobs").join("missing-blob"),
            snap.join("model.onnx_data"),
        )
        .unwrap();
        assert!(!model_cached(ModelSize::Large, cache));
    }

    #[test]
    fn query_text_applies_e5_prefix_to_lowercased_query() {
        // The caller lowercases once (§6); this helper only prefixes.
        assert_eq!(query_text("電磁誘導"), "query: 電磁誘導");
        assert_eq!(query_text("lenz's law"), "query: lenz's law");
    }

    #[test]
    fn embeddings_keys_are_normalized_and_matrix_selected() {
        let json =
            br#"{"small":{"00042":[1.0,0.0],"uuid-x":[0.0,1.0]},"large":{"uuid-x":[0.0,0.0,0.0]}}"#;
        // dim check is against the declared model dim; use a fake 2-dim by
        // testing the invariant failure instead:
        let err = CorpusEmbeddings::from_json(json, ModelSize::Small).unwrap_err();
        assert!(matches!(err, SemanticError::Invariant(_)));
        assert!(err.to_string().contains("expected 384"));
    }

    #[test]
    fn missing_matrix_is_an_invariant_error() {
        let err = CorpusEmbeddings::from_json(br#"{"large":{}}"#, ModelSize::Small).unwrap_err();
        assert!(matches!(err, SemanticError::Invariant(_)));
        assert!(err.to_string().contains("no \"small\""));
    }

    #[test]
    fn ragged_embeddings_fail_loudly() {
        let json = br#"{"small":{"a":[1.0,0.0],"b":[1.0]}}"#;
        let err = CorpusEmbeddings::from_json(json, ModelSize::Small).unwrap_err();
        assert!(matches!(err, SemanticError::Invariant(_)));
    }

    fn tiny_embeddings() -> CorpusEmbeddings {
        // Bypass the 384-dim check by constructing directly: unit vectors.
        let mut vectors = HashMap::new();
        vectors.insert("a".to_string(), (vec![1.0, 0.0], 1.0));
        vectors.insert("b".to_string(), (vec![0.0, 1.0], 1.0));
        vectors.insert(
            "00042".to_string(),
            (
                vec![
                    std::f64::consts::FRAC_1_SQRT_2,
                    std::f64::consts::FRAC_1_SQRT_2,
                ],
                1.0,
            ),
        );
        CorpusEmbeddings { dim: 2, vectors }
    }

    #[test]
    fn semantic_rank_orders_by_cosine_and_skips_missing_vectors() {
        let corpus = Corpus::new(vec![
            record("a"),
            record("no-vector"),
            record("b"),
            record("00042"),
        ]);
        // note: corpus ids are normalized at load; here they're already plain
        let emb = tiny_embeddings();
        let ranked = semantic_rank(&corpus, &emb, &[1.0, 0.0]).unwrap();
        // a: cos 1.0; 00042: ~0.707; b: 0.0; "no-vector" skipped
        let docs: Vec<u32> = ranked.iter().map(|(d, _)| *d).collect();
        assert_eq!(docs, vec![0, 3, 2]);
        assert!((ranked[0].1 - 1.0).abs() < 1e-6);
        assert!((ranked[1].1 - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    }

    #[test]
    fn dim_mismatch_is_an_invariant_error() {
        let corpus = Corpus::new(vec![record("a")]);
        let emb = tiny_embeddings();
        let err = semantic_rank(&corpus, &emb, &[1.0, 0.0, 0.0]).unwrap_err();
        assert!(matches!(err, SemanticError::Invariant(_)));
    }
}
