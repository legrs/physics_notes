//! clap CLI: `physq` (TUI), `physq search "<q>"`, `physq cache clean|path`.

use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use crate::config::{Config, ModelSize};
use crate::engine::{hybrid, Engine, SemanticEngine};
use crate::model::Corpus;
use crate::query::prepare_query;
use crate::semantic::SemanticError;
use crate::spinner::StderrSpinner;
use crate::tui;

#[derive(Parser)]
#[command(
    name = "physq",
    version,
    about = "Local hybrid search (BM25 + e5 semantic, RRF) over the Physics Notes Q&A corpus",
    long_about = None
)]
struct Cli {
    /// Data host base URL (default: the Physics Notes GitHub raw URL;
    /// also settable via PHYSQ_BASE_URL)
    #[arg(long, global = true, value_name = "URL")]
    base_url: Option<String>,

    /// Embedding model / matrix ("small" = e5-small 384d, "large" = e5-large 1024d)
    #[arg(long, global = true, value_enum, default_value_t = ModelArg::Small)]
    model: ModelArg,

    /// Never touch the network; use the local cache only
    #[arg(long, global = true)]
    offline: bool,

    /// Override the cache directory (default: OS cache dir /physics-notes;
    /// also settable via PHYSQ_CACHE_DIR)
    #[arg(long, global = true, value_name = "DIR")]
    cache_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ModelArg {
    Small,
    Large,
}

impl From<ModelArg> for ModelSize {
    fn from(m: ModelArg) -> Self {
        match m {
            ModelArg::Small => ModelSize::Small,
            ModelArg::Large => ModelSize::Large,
        }
    }
}

#[derive(Subcommand)]
enum Cmd {
    /// One-shot search: hybrid BM25 + semantic (RRF-fused) ranked results
    Search {
        /// The query (Japanese or English)
        query: String,
        /// Max results to print
        #[arg(short = 'n', long, default_value_t = 10)]
        limit: usize,
        /// Force plain tab-separated output (automatic when stdout is piped)
        #[arg(long)]
        plain: bool,
        /// Skip the semantic stage (no model download; lexical ranking only)
        #[arg(long)]
        bm25_only: bool,
    },
    /// Cache utilities
    Cache {
        #[command(subcommand)]
        cmd: CacheCmd,
    },
}

#[derive(Subcommand)]
enum CacheCmd {
    /// Print the cache directory path
    Path,
    /// Remove cached data and the BM25 index (keeps the downloaded model
    /// unless --all is given)
    Clean {
        /// Also remove the downloaded embedding model
        #[arg(long)]
        all: bool,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let cfg = Config::resolve(
        cli.base_url.clone(),
        cli.cache_dir.clone(),
        cli.model.into(),
        cli.offline,
    )?;

    match cli.command {
        None => tui::run(cfg),
        Some(Cmd::Search {
            query,
            limit,
            plain,
            bm25_only,
        }) => run_search(cfg, &query, limit, plain, bm25_only),
        Some(Cmd::Cache { cmd }) => run_cache(cfg, cmd),
    }
}

fn run_cache(cfg: Config, cmd: CacheCmd) -> Result<()> {
    match cmd {
        CacheCmd::Path => {
            println!("{}", cfg.cache_root.display());
            Ok(())
        }
        CacheCmd::Clean { all } => {
            let mut targets = vec![cfg.data_dir(), cfg.index_dir()];
            if all {
                targets.push(cfg.model_dir());
            }
            for dir in targets {
                if dir.exists() {
                    std::fs::remove_dir_all(&dir)
                        .with_context(|| format!("removing {}", dir.display()))?;
                    println!("removed {}", dir.display());
                } else {
                    println!("already clean: {}", dir.display());
                }
            }
            if !all {
                println!("(model kept; use `physq cache clean --all` to remove it)");
            }
            Ok(())
        }
    }
}

/// §5 startup flow + one query, then print. All ranking logic lives in
/// `engine`; this function is only spinner, error policy, and output.
/// Composes with pipes: plain TSV (`rank\tscore\tid\tquestion`) when stdout
/// is not a terminal.
fn run_search(cfg: Config, query: &str, limit: usize, plain: bool, bm25_only: bool) -> Result<()> {
    let spinner = StderrSpinner::start("Fetching data…");
    let engine = {
        let progress = |s: &str| spinner.set_label(s);
        Engine::load_blocking(&cfg, &progress)?
    };

    let q = prepare_query(query);
    let bm25_results = engine.bm25(&q);

    let mut mode = "BM25-only";
    let mut results = bm25_results.clone();
    let mut sem_warning: Option<String> = None;

    if !bm25_only && !q.is_empty() {
        spinner.set_label("Preparing semantic model… (downloads once on first run)");
        let sem = SemanticEngine::load(&cfg, engine.corpus.clone()).and_then(|mut s| {
            spinner.set_label(""); // short waits get the whimsical verbs (§11)
            s.rank(&q)
        });
        match sem {
            Ok(semantic_ranked) => {
                results = hybrid(&bm25_results, &semantic_ranked);
                mode = "hybrid (BM25 + semantic, RRF)";
            }
            // Shared-artifact invariant breaks fail loudly (CLAUDE.md §7).
            Err(e @ SemanticError::Invariant(_)) => {
                drop(spinner);
                return Err(anyhow::Error::new(e));
            }
            Err(SemanticError::Unavailable(e)) => {
                sem_warning = Some(format!(
                    "semantic stage unavailable ({e}); showing BM25-only results"
                ));
            }
        }
    }

    spinner.finish();
    for w in &engine.warnings {
        eprintln!("warning: {w}");
    }
    if let Some(w) = sem_warning {
        eprintln!("warning: {w}");
    }
    eprintln!("mode: {mode}");

    print_results(&engine.corpus, &results, limit, plain);
    Ok(())
}

fn print_results(corpus: &Corpus, results: &[(u32, f64)], limit: usize, plain: bool) {
    let pretty = !plain && std::io::stdout().is_terminal();
    if results.is_empty() {
        if pretty {
            println!("no results");
        }
        return;
    }
    for (i, &(doc, score)) in results.iter().take(limit).enumerate() {
        let r = &corpus.records[doc as usize];
        let question = r
            .questions
            .first()
            .map(String::as_str)
            .unwrap_or("(no question)");
        if pretty {
            let bold = "\x1b[1m";
            let dim = "\x1b[2m";
            let reset = "\x1b[0m";
            println!("{bold}{:>2}. {question}{reset}", i + 1);
            let desc: String = r.description.chars().take(120).collect();
            if !desc.is_empty() {
                println!("    {desc}");
            }
            println!(
                "    {dim}score {score:.6} · {} · {} · id {}{reset}",
                r.category.join(", "),
                r.difficulty,
                r.id
            );
        } else {
            println!("{}\t{score:.6}\t{}\t{question}", i + 1, r.id);
        }
    }
}
