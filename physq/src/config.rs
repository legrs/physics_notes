//! Configuration: data host, cache layout, model selection.

use std::path::PathBuf;

use anyhow::{Context, Result};

/// Data host (CONFIRMED, CLAUDE.md §5). Overridable via `--base-url` /
/// `PHYSQ_BASE_URL`; never hardcode anywhere else.
pub const DATA_BASE_URL: &str =
    "https://raw.githubusercontent.com/legrs/physics_notes/refs/heads/master/";

pub const QA_DATA_FILE: &str = "q_and_a_data.json";
pub const EMBEDDINGS_FILE: &str = "embeddings.json";
pub const VERSION_FILE: &str = "version.json";

/// Tag stored in the BM25 index cache; a mismatch (or a different tag in
/// `version.json`) forces a rebuild (CLAUDE.md §3).
pub const TOKENIZER_TAG: &str = "lindera-ipadic";

/// Ranking constants confirmed from `search.html` (CLAUDE.md §6).
pub const BM25_K1: f64 = 1.2;
pub const BM25_B: f64 = 0.75;
pub const RRF_K: f64 = 60.0;
pub const RRF_SEMANTIC_WEIGHT: f64 = 2.0;
pub const RELATED_BOOST: f64 = 0.5;

/// Which pre-computed embedding matrix (and query model) to use.
/// `embeddings.json = { "small": …, "large": … }` (CLAUDE.md §7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSize {
    Small,
    Large,
}

impl ModelSize {
    pub fn embeddings_key(self) -> &'static str {
        match self {
            ModelSize::Small => "small",
            ModelSize::Large => "large",
        }
    }

    pub fn dim(self) -> usize {
        match self {
            ModelSize::Small => 384,
            ModelSize::Large => 1024,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub base_url: String,
    pub cache_root: PathBuf,
    pub model: ModelSize,
    pub offline: bool,
}

impl Config {
    /// Resolve configuration. Precedence: CLI flag > environment > default.
    pub fn resolve(
        base_url: Option<String>,
        cache_dir: Option<PathBuf>,
        model: ModelSize,
        offline: bool,
    ) -> Result<Self> {
        let base_url = base_url
            .or_else(|| std::env::var("PHYSQ_BASE_URL").ok())
            .unwrap_or_else(|| DATA_BASE_URL.to_string());
        let cache_root =
            match cache_dir.or_else(|| std::env::var("PHYSQ_CACHE_DIR").ok().map(PathBuf::from)) {
                Some(dir) => dir,
                None => dirs::cache_dir()
                    .context("could not determine the OS cache directory")?
                    .join("physics-notes"),
            };
        Ok(Self {
            base_url,
            cache_root,
            model,
            offline,
        })
    }

    pub fn data_dir(&self) -> PathBuf {
        self.cache_root.join("data")
    }

    pub fn index_dir(&self) -> PathBuf {
        self.cache_root.join("index")
    }

    pub fn model_dir(&self) -> PathBuf {
        self.cache_root.join("model")
    }

    pub fn qa_data_path(&self) -> PathBuf {
        self.data_dir().join(QA_DATA_FILE)
    }

    pub fn embeddings_path(&self) -> PathBuf {
        self.data_dir().join(EMBEDDINGS_FILE)
    }

    pub fn bm25_index_path(&self) -> PathBuf {
        self.index_dir().join("bm25_index.bin")
    }

    pub fn file_url(&self, name: &str) -> String {
        let mut url = self.base_url.clone();
        if !url.ends_with('/') {
            url.push('/');
        }
        url.push_str(name);
        url
    }
}
