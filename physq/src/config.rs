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

/// How often a normal (non-`--offline`) run is willing to hit the network to
/// check `version.json`, once the cache is already complete. `0` (the
/// default) checks on *every* launch, matching the original always-check
/// behavior — owner-confirmed: a manual `physq search`/TUI launch checking
/// every time is fine day-to-day. Set via `--refresh-interval SECONDS` /
/// `PHYSQ_REFRESH_INTERVAL_SECS` when doing many quick repeated launches in a
/// short session (e.g. manually spot-checking search quality while iterating
/// on the self-improvement loop) — a burst of manual runs can otherwise rack
/// up enough requests to get rate-limited by the data host (observed: HTTP
/// 429). `--offline` always skips the network regardless of this value.
pub const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 0;

/// Tag stored in the BM25 index cache; a mismatch (or a different tag in
/// `version.json`) forces a rebuild (CLAUDE.md §3).
pub const TOKENIZER_TAG: &str = "lindera-ipadic";

/// Ranking constants confirmed from `search.html` (CLAUDE.md §6).
pub const BM25_K1: f64 = 1.2;
pub const BM25_B: f64 = 0.75;
pub const RRF_K: f64 = 60.0;
/// RRF weight for the BM25 list. Shared by every mode (`Single`, `Max`) and
/// the starting value for `Custom`'s tunable `weights.bm25` (`CustomWeights`
/// below). Edit this to retune ranking — keep `search.html`'s matching
/// `RRF_WEIGHT_BM25` in sync (CLAUDE.md §6 parity).
pub const RRF_WEIGHT_BM25: f64 = 1.0;
/// RRF weight for the e5-small semantic list: used whenever `Single(Small)`
/// is active, and for small's slot in the `Max` ensemble. Starting value for
/// `Custom`'s `weights.small`. Keep `search.html`'s `RRF_WEIGHTS.small` synced.
pub const RRF_WEIGHT_SMALL: f64 = 2.0;
/// RRF weight for the e5-large semantic list: used whenever `Single(Large)`
/// is active, and for large's slot in the `Max` ensemble. Starting value for
/// `Custom`'s `weights.large`. Keep `search.html`'s `RRF_WEIGHTS.large` synced.
pub const RRF_WEIGHT_LARGE: f64 = 2.0;
pub const RELATED_BOOST: f64 = 0.5;

/// One physical embedding model / matrix.
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

    /// This model's RRF weight outside `Custom` mode (`Single`, and its slot
    /// in the `Max` ensemble). `Custom` starts from the same values
    /// (`CustomWeights::default`) but can retune them live in `/config`.
    pub fn rrf_weight(self) -> f64 {
        match self {
            ModelSize::Small => RRF_WEIGHT_SMALL,
            ModelSize::Large => RRF_WEIGHT_LARGE,
        }
    }
}

/// How the semantic stage is configured for a run. `Max` is the CLI-only
/// ensemble mode: it ranks with **both** e5 models (small + large) and fuses
/// each list into the RRF alongside BM25, so a hit both models rank 2nd–3rd
/// can outrank one that a single model puts 1st. There is no web equivalent —
/// this is an accuracy-over-parity deviation on the same shared artifacts.
/// `Custom` is the `--debug`-only variant of `Max`: same two models, but the
/// RRF weights (BM25 / small / large) are user-tunable at runtime (`weights`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSel {
    /// Semantic stage off (BM25-only): no model download, no `embeddings.json`.
    Off,
    /// A single model.
    Single(ModelSize),
    /// Ensemble of every model (small + large) with the fixed default weights.
    Max,
    /// Ensemble like `Max`, but with user-tunable RRF weights (`--debug`).
    Custom,
}

impl ModelSel {
    /// The physical models to load and rank with, in fusion order. Empty for
    /// `Off`; the ensemble order (small, large) for `Max`/`Custom` doubles as
    /// the weight order in [`CustomWeights`].
    pub fn sizes(self) -> &'static [ModelSize] {
        const SMALL: &[ModelSize] = &[ModelSize::Small];
        const LARGE: &[ModelSize] = &[ModelSize::Large];
        const BOTH: &[ModelSize] = &[ModelSize::Small, ModelSize::Large];
        match self {
            ModelSel::Off => &[],
            ModelSel::Single(ModelSize::Small) => SMALL,
            ModelSel::Single(ModelSize::Large) => LARGE,
            ModelSel::Max | ModelSel::Custom => BOTH,
        }
    }

    /// Whether the semantic stage runs at all.
    pub fn is_enabled(self) -> bool {
        !matches!(self, ModelSel::Off)
    }

    /// Stable short label for status lines / config screen / CLI parsing.
    pub fn label(self) -> &'static str {
        match self {
            ModelSel::Off => "none",
            ModelSel::Single(ModelSize::Small) => "small",
            ModelSel::Single(ModelSize::Large) => "large",
            ModelSel::Max => "max",
            ModelSel::Custom => "custom",
        }
    }
}

/// TUI keybinding scheme. `Normal` is the default arrow-key/PgUp-style map;
/// `Vim` turns the home row into modal (INSERT/NORMAL/VISUAL) navigation —
/// set at launch with `--vim`, or switched live from `/config` / `/vim`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMode {
    Normal,
    Vim,
}

impl KeyMode {
    /// Stable short label for the `/config` screen.
    pub fn label(self) -> &'static str {
        match self {
            KeyMode::Normal => "normal",
            KeyMode::Vim => "vim",
        }
    }
}

/// Per-model RRF weights for `ModelSel::Custom` (the `--debug` tuning mode).
/// Order mirrors the fusion: BM25 first, then each `ModelSel::sizes()` model
/// (small, large). Defaults match `Max` (BM25 weight 1, each semantic 2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CustomWeights {
    pub bm25: f64,
    pub small: f64,
    pub large: f64,
}

impl Default for CustomWeights {
    fn default() -> Self {
        Self {
            bm25: RRF_WEIGHT_BM25,
            small: RRF_WEIGHT_SMALL,
            large: RRF_WEIGHT_LARGE,
        }
    }
}

impl CustomWeights {
    /// Adjustment step and inclusive bounds for the `/config` weight editor,
    /// mirroring the web debug slider (step 0.1). BM25 may go to 0 to disable
    /// the lexical contribution entirely.
    pub const STEP: f64 = 0.1;
    pub const MIN: f64 = 0.0;
    pub const MAX: f64 = 5.0;
}

#[derive(Debug, Clone)]
pub struct Config {
    pub base_url: String,
    pub cache_root: PathBuf,
    /// `ModelSel::Off` disables the semantic stage entirely (BM25-only): no
    /// model download, no `embeddings.json` load. Set via `--model none` or
    /// `--bm25-only`. `ModelSel::Max` runs the small+large ensemble.
    pub model: ModelSel,
    pub offline: bool,
    /// `--debug`: unlocks the `custom` semantic mode + its weight editor in the
    /// `/config` TUI screen. Nothing else changes.
    pub debug: bool,
    /// RRF weights used when `model == ModelSel::Custom` (tuned live in
    /// `/config` under `--debug`).
    pub weights: CustomWeights,
    /// TUI keybinding scheme (`--vim`, or toggled live in `/config`).
    pub keys: KeyMode,
    /// Minimum seconds between `version.json` network checks once the cache
    /// is complete (`--refresh-interval` / `PHYSQ_REFRESH_INTERVAL_SECS`).
    /// `--offline` bypasses this (it never touches the network at all).
    pub refresh_interval_secs: u64,
}

impl Config {
    /// Resolve configuration. Precedence: CLI flag > environment > default.
    #[allow(clippy::too_many_arguments)]
    pub fn resolve(
        base_url: Option<String>,
        cache_dir: Option<PathBuf>,
        model: ModelSel,
        offline: bool,
        debug: bool,
        vim: bool,
        refresh_interval_secs: Option<u64>,
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
        let refresh_interval_secs = refresh_interval_secs
            .or_else(|| {
                std::env::var("PHYSQ_REFRESH_INTERVAL_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
            })
            .unwrap_or(DEFAULT_REFRESH_INTERVAL_SECS);
        Ok(Self {
            base_url,
            cache_root,
            model,
            offline,
            debug,
            weights: CustomWeights::default(),
            keys: if vim { KeyMode::Vim } else { KeyMode::Normal },
            refresh_interval_secs,
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
