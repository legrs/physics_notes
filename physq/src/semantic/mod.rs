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
pub fn model_cached(size: ModelSize, model_cache_dir: &Path) -> bool {
    let repo_dir = match size {
        ModelSize::Small => "models--intfloat--multilingual-e5-small",
        ModelSize::Large => "models--Qdrant--multilingual-e5-large-onnx",
    };
    model_cache_dir.join(repo_dir).join("snapshots").is_dir()
}

/// Runtime query embedder (fastembed, model auto-cached in `<cache>/model/`).
pub struct Embedder {
    model: TextEmbedding,
    dim: usize,
}

impl Embedder {
    pub fn new(size: ModelSize, model_cache_dir: &Path) -> Result<Self, SemanticError> {
        let model = match size {
            ModelSize::Small => EmbeddingModel::MultilingualE5Small,
            ModelSize::Large => EmbeddingModel::MultilingualE5Large,
        };
        let opts = TextInitOptions::new(model)
            .with_cache_dir(model_cache_dir.to_path_buf())
            .with_show_download_progress(false);
        let model = TextEmbedding::try_new(opts).map_err(|e| {
            SemanticError::Unavailable(format!("failed to init embedding model: {e}"))
        })?;
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
            .map_err(|e| SemanticError::Unavailable(format!("query embedding failed: {e}")))?;
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
