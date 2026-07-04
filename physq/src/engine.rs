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
use crate::config::Config;
use crate::data::{ensure_data, sha256_hex};
use crate::model::Corpus;
use crate::query::{expand_query, LinderaIpadic, QueryTokenizer};
use crate::rank::rrf_merge_default;
use crate::semantic::{semantic_rank, CorpusEmbeddings, Embedder, SemanticError};

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

/// Loaded semantic search state (query embedder + pre-computed corpus
/// matrix). Constructing this may download the model on first run — do it
/// off any UI thread.
pub struct SemanticEngine {
    embedder: Embedder,
    embeddings: CorpusEmbeddings,
    corpus: Arc<Corpus>,
}

impl SemanticEngine {
    /// Honors `--offline`: never starts a model download in offline mode
    /// (reports `Unavailable` instead, so frontends fall back to BM25-only).
    /// Shared-artifact invariant breaks surface as `Invariant` — frontends
    /// must fail loudly on those, not degrade silently (CLAUDE.md §7).
    pub fn load(cfg: &Config, corpus: Arc<Corpus>) -> Result<Self, SemanticError> {
        let Some(model) = cfg.model else {
            return Err(SemanticError::Unavailable(
                "semantic search disabled (--model none / --bm25-only)".to_string(),
            ));
        };
        if cfg.offline && !crate::semantic::model_cached(model, &cfg.model_dir()) {
            return Err(SemanticError::Unavailable(
                "offline mode and the embedding model is not downloaded yet".to_string(),
            ));
        }
        let embedder = Embedder::new(model, &cfg.model_dir())?;
        let embeddings = CorpusEmbeddings::load(&cfg.embeddings_path(), model)?;
        Ok(Self {
            embedder,
            embeddings,
            corpus,
        })
    }

    /// Semantic ranking of the whole embedded corpus for an
    /// already-lowercased query (§7: `query: ` prefix applied inside).
    pub fn rank(&mut self, q_lower: &str) -> Result<Ranked, SemanticError> {
        let qv = self.embedder.embed_query(q_lower)?;
        semantic_rank(&self.corpus, &self.embeddings, &qv)
    }
}

/// RRF fusion of the two stages with the confirmed web constants (§6).
pub fn hybrid(bm25: &Ranked, semantic: &Ranked) -> Ranked {
    rrf_merge_default(bm25, semantic)
}
