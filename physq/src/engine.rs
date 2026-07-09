//! UI-agnostic search engine facade.
//!
//! Everything a frontend needs, with zero UI concerns: the §5 startup flow
//! (fetch → corpus → BM25 index load-or-build → query tokenizer), the BM25
//! stage, and the semantic stage. Both the TUI and the one-shot CLI are thin
//! consumers of this module — swapping or polishing a UI must never touch
//! ranking logic.
//!
//! Progress reporting is a plain `Fn(&str)` callback so frontends decide how
//! to render it (spinner label, status bar, log line…).

use std::sync::Arc;

use anyhow::{Context, Result};

use crate::bm25::{self, Bm25Index};
use crate::config::{Config, CustomWeights, ModelSize, RRF_K};
use crate::data::{ensure_data, sha256_hex};
use crate::model::Corpus;
use crate::query::{LinderaIpadic, QueryTokenizer, expand_query};
use crate::rank::{rrf_merge_hybrid, rrf_merge_weighted};
use crate::semantic::{CorpusEmbeddings, Embedder, SemanticError, semantic_rank};

/// Ranked hits: `(index into corpus.records, score)`, best first.
pub type Ranked = Vec<(u32, f64)>;

/// Loaded lexical search state (data + BM25 index + query tokenizer).
/// Cheap to clone across threads — everything is behind `Arc`.
#[derive(Clone)]
pub struct Engine {
    pub corpus: Arc<Corpus>,
    pub index: Arc<Bm25Index>,
    pub tokenizer: Arc<dyn QueryTokenizer>,
    /// Non-fatal notes from the startup flow (offline fallback etc.).
    pub warnings: Vec<String>,
}

impl Engine {
    /// The §5 startup flow. Network + parsing happen here; `progress`
    /// receives phase labels. CPU-heavy parts run on a blocking task so an
    /// async caller's runtime stays responsive.
    pub async fn load(cfg: &Config, progress: &(dyn Fn(&str) + Send + Sync)) -> Result<Engine> {
        let files = ensure_data(cfg, progress).await?;
        progress("Loading corpus…");
        let warnings = files.warnings.clone();
        let qa_path = files.qa_path.clone();
        let index_path = cfg.bm25_index_path();
        let tokenizer_tag = files.tokenizer_tag.clone();

        let mut engine = tokio::task::spawn_blocking(move || -> Result<Engine> {
            let bytes = std::fs::read(&qa_path)
                .with_context(|| format!("reading {}", qa_path.display()))?;
            let data_hash = sha256_hex(&bytes);
            let corpus = Arc::new(Corpus::from_json(&bytes)?);
            let tokenizer: Arc<dyn QueryTokenizer> =
                Arc::new(LinderaIpadic::new().context("failed to init lindera IPADIC")?);
            let index = match Bm25Index::load_if_valid(&index_path, &tokenizer_tag, &data_hash) {
                Some(idx) => Arc::new(idx),
                None => {
                    let idx = Bm25Index::build(&corpus, &tokenizer_tag, &data_hash);
                    // Cache write failures are non-fatal; the index is in memory.
                    let _ = idx.save(&index_path);
                    Arc::new(idx)
                }
            };
            Ok(Engine {
                corpus,
                index,
                tokenizer,
                warnings: Vec::new(),
            })
        })
        .await
        .context("corpus load task panicked")??;
        engine.warnings = warnings;
        Ok(engine)
    }

    /// Blocking variant for synchronous frontends.
    pub fn load_blocking(cfg: &Config, progress: &(dyn Fn(&str) + Send + Sync)) -> Result<Engine> {
        let runtime = tokio::runtime::Runtime::new().context("failed to start async runtime")?;
        runtime.block_on(Self::load(cfg, progress))
    }

    /// The full BM25 stage (§6) for an already-lowercased query
    /// (`query::prepare_query`): expansion, scoring with all field boosts,
    /// priority, related boost — the exact list RRF consumes.
    pub fn bm25(&self, q_lower: &str) -> Ranked {
        let terms = expand_query(q_lower, self.tokenizer.as_ref());
        bm25::search(&self.corpus, &self.index, &terms)
    }
}

/// One loaded semantic model: its query embedder plus the matching
/// pre-computed corpus matrix.
struct LoadedModel {
    embedder: Embedder,
    embeddings: CorpusEmbeddings,
}

/// Loaded semantic search state — one or more query embedders each paired with
/// its pre-computed corpus matrix. `Single` selections load one; `max` loads
/// both e5 models. Constructing this may download models on first run — do it
/// off any UI thread.
pub struct SemanticEngine {
    models: Vec<LoadedModel>,
    corpus: Arc<Corpus>,
}

impl SemanticEngine {
    /// Loads every model in `cfg.model.sizes()`. Honors `--offline`: never
    /// starts a model download in offline mode (reports `Unavailable` if any
    /// required model isn't cached, so frontends fall back to BM25-only — the
    /// `max` ensemble is all-or-nothing rather than silently degrading to one
    /// model). Shared-artifact invariant breaks surface as `Invariant` —
    /// frontends must fail loudly on those (CLAUDE.md §7).
    pub fn load(cfg: &Config, corpus: Arc<Corpus>) -> Result<Self, SemanticError> {
        let sizes = cfg.model.sizes();
        if sizes.is_empty() {
            return Err(SemanticError::Unavailable(
                "semantic search disabled (--model none / --bm25-only)".to_string(),
            ));
        }
        let mut models = Vec::with_capacity(sizes.len());
        for &size in sizes {
            if cfg.offline && !crate::semantic::model_cached(size, &cfg.model_dir()) {
                return Err(SemanticError::Unavailable(format!(
                    "offline mode and the {} embedding model is not downloaded yet",
                    size.embeddings_key()
                )));
            }
            let embedder = Embedder::new(size, &cfg.model_dir())?;
            let embeddings = CorpusEmbeddings::load(&cfg.embeddings_path(), size)?;
            models.push(LoadedModel {
                embedder,
                embeddings,
            });
        }
        Ok(Self { models, corpus })
    }

    /// Rank the whole embedded corpus with every configured model, for an
    /// already-lowercased query (§7: `query: ` prefix applied inside). Returns
    /// one ranked list per model in `ModelSel::sizes()` order; the caller
    /// RRF-fuses them with BM25. The query is embedded once per model because
    /// small (384d) and large (1024d) need their own query vectors.
    pub fn rank(&mut self, q_lower: &str) -> Result<Vec<Ranked>, SemanticError> {
        let mut out = Vec::with_capacity(self.models.len());
        for m in &mut self.models {
            let qv = m.embedder.embed_query(q_lower)?;
            out.push(semantic_rank(&self.corpus, &m.embeddings, &qv)?);
        }
        Ok(out)
    }
}

/// RRF fusion of BM25 with one or more semantic rankings (§6), each at its
/// own model's weight (`ModelSize::rrf_weight`). `sizes` must be
/// index-aligned with `semantics` (both in `ModelSel::sizes()` order — see
/// `SemanticEngine::rank`). One semantic list reproduces the confirmed web
/// ordering; two = the `max` ensemble.
pub fn hybrid(bm25: &Ranked, semantics: &[Ranked], sizes: &[ModelSize]) -> Ranked {
    rrf_merge_hybrid(bm25, semantics, sizes)
}

/// RRF fusion for the `custom` (`--debug`) mode: BM25 and each semantic model
/// carry their own tunable weight. `semantics` are in `ModelSel::sizes()`
/// order (small, large), matching `weights.small` / `weights.large`.
pub fn hybrid_custom(bm25: &Ranked, semantics: &[Ranked], weights: &CustomWeights) -> Ranked {
    let sem_weights = [weights.small, weights.large];
    let mut lists: Vec<(&[(u32, f64)], f64)> = Vec::with_capacity(1 + semantics.len());
    lists.push((bm25.as_slice(), weights.bm25));
    for (i, s) in semantics.iter().enumerate() {
        lists.push((
            s.as_slice(),
            sem_weights.get(i).copied().unwrap_or(weights.large),
        ));
    }
    rrf_merge_weighted(&lists, RRF_K)
}
