//! `physq eval` — machine-readable ranking evaluation for the search
//! self-improvement loop (`../scripts/self_improve.py` at the repo root).
//!
//! For each case (a query plus the id of the record that *should* win) it
//! reports where that target ranks in every method — BM25, each loaded e5
//! model, and the RRF hybrid — as one JSON line. Two modes:
//!
//! - **batch** (`--cases <file>`): evaluate every JSONL case line, then
//!   append a `{"type":"summary",…}` line with per-method top-1/top-3/
//!   top-10/MRR aggregates.
//! - **serve** (`--serve`): read case/command lines from stdin until EOF,
//!   answering one line per input line. This is the long-running mode the
//!   optimizer drives: embedding models load once per night, query vectors
//!   are embedded once ever (in-memory cache), and `{"cmd":"reload_data",
//!   "path":…}` swaps in an edited dataset between rounds (the BM25 index
//!   rebuilds in memory; corpus embeddings are fixed upstream artifacts and
//!   are never recomputed — CLAUDE.md §2).
//!
//! `--data` / `--embeddings` point at local working copies so candidate
//! dataset edits can be measured without touching the fetch cache. Ranking
//! logic is entirely reused from `bm25` / `semantic` / `rank` — this module
//! adds only orchestration and I/O, so scores here are exactly what the TUI
//! and `physq search` produce.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::bm25::{self, Bm25Index};
use crate::config::{Config, CustomWeights, ModelSize, RRF_K, TOKENIZER_TAG};
use crate::data::sha256_hex;
use crate::engine::Engine;
use crate::model::{Corpus, normalize_id};
use crate::query::{LinderaIpadic, QueryTokenizer, expand_query, prepare_query};
use crate::rank::rrf_merge_weighted;
use crate::semantic::{CorpusEmbeddings, Embedder, SemanticError, semantic_rank};

/// Arguments for `physq eval` (wired up in `cli.rs`).
pub struct EvalArgs {
    /// JSONL case file (`{"query":…,"target":…}` per line); `-` = stdin.
    pub cases: Option<PathBuf>,
    /// Long-running stdin/stdout mode (see module docs).
    pub serve: bool,
    /// Local `q_and_a_data.json` override (skips the network/cache flow).
    pub data: Option<PathBuf>,
    /// Local `embeddings.json` override.
    pub embeddings: Option<PathBuf>,
    /// RRF weight overrides as `"<bm25>,<small>,<large>"`.
    pub weights: Option<String>,
    /// How many top hybrid ids to include per result line.
    pub top: usize,
}

/// One loaded semantic model plus its in-memory query-vector cache. The cache
/// is what makes `--serve` cheap across rounds: dataset edits never change
/// query embeddings, so each distinct query is embedded exactly once.
struct EvalModel {
    size: ModelSize,
    embedder: Embedder,
    embeddings: CorpusEmbeddings,
    qcache: HashMap<String, Vec<f64>>,
}

impl EvalModel {
    fn ensure_query_vec(&mut self, q_lower: &str) -> Result<(), SemanticError> {
        if !self.qcache.contains_key(q_lower) {
            let v = self.embedder.embed_query(q_lower)?;
            self.qcache.insert(q_lower.to_string(), v);
        }
        Ok(())
    }
}

struct EvalCtx {
    corpus: Arc<Corpus>,
    index: Arc<Bm25Index>,
    tokenizer: Arc<dyn QueryTokenizer>,
    /// record id (already normalized at corpus load) → doc index.
    id_to_doc: HashMap<String, u32>,
    /// In `ModelSel::sizes()` order (small, large). Empty for `--model none`.
    models: Vec<EvalModel>,
    weights: CustomWeights,
    top: usize,
}

pub fn run(cfg: Config, args: EvalArgs) -> Result<()> {
    if !args.serve && args.cases.is_none() {
        bail!("`physq eval` needs --cases <file> (batch) or --serve");
    }
    let weights = match &args.weights {
        Some(s) => parse_weights(s)?,
        None => CustomWeights::default(),
    };
    let mut ctx = load_ctx(&cfg, &args, weights)?;
    if args.serve {
        serve(&mut ctx)
    } else {
        batch(&mut ctx, args.cases.as_deref().unwrap())
    }
}

/// `"1,2,2"` → weights. Default (no flag) matches the shipped hybrid: BM25 1,
/// each semantic list `RRF_SEMANTIC_WEIGHT` (see `rank::rrf_merge_hybrid`).
fn parse_weights(s: &str) -> Result<CustomWeights> {
    let parts: Vec<f64> = s
        .split(',')
        .map(|p| p.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .with_context(|| format!("--weights \"{s}\" is not numeric"))?;
    let [bm25, small, large] = parts.as_slice() else {
        bail!("--weights must be \"<bm25>,<small>,<large>\", e.g. \"1,2,2\"");
    };
    Ok(CustomWeights {
        bm25: *bm25,
        small: *small,
        large: *large,
    })
}

fn id_map(corpus: &Corpus) -> HashMap<String, u32> {
    corpus
        .records
        .iter()
        .enumerate()
        .map(|(i, r)| (r.id.clone(), i as u32))
        .collect()
}

/// Read a local `q_and_a_data.json` and build its BM25 index in memory only —
/// working copies must never poison the fetch cache's `bm25_index.bin`.
fn load_local_data(path: &Path) -> Result<(Arc<Corpus>, Arc<Bm25Index>)> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let corpus = Arc::new(Corpus::from_json(&bytes)?);
    let hash = sha256_hex(&bytes);
    let index = Arc::new(Bm25Index::build(&corpus, TOKENIZER_TAG, &hash));
    Ok((corpus, index))
}

fn load_ctx(cfg: &Config, args: &EvalArgs, weights: CustomWeights) -> Result<EvalCtx> {
    let progress = |s: &str| eprintln!("physq eval: {s}");

    let (corpus, index, tokenizer) = match &args.data {
        Some(p) => {
            progress(&format!("loading local data {}…", p.display()));
            let (corpus, index) = load_local_data(p)?;
            let tokenizer: Arc<dyn QueryTokenizer> =
                Arc::new(LinderaIpadic::new().context("failed to init lindera IPADIC")?);
            (corpus, index, tokenizer)
        }
        None => {
            let engine = Engine::load_blocking(cfg, &progress)?;
            for w in &engine.warnings {
                eprintln!("warning: {w}");
            }
            (engine.corpus, engine.index, engine.tokenizer)
        }
    };

    let emb_path = args
        .embeddings
        .clone()
        .unwrap_or_else(|| cfg.embeddings_path());
    let mut models = Vec::new();
    for &size in cfg.model.sizes() {
        progress(&format!(
            "loading e5-{} model + embeddings…",
            size.embeddings_key()
        ));
        let embedder = Embedder::new(size, &cfg.model_dir())?;
        let embeddings = CorpusEmbeddings::load(&emb_path, size).with_context(|| {
            format!(
                "loading {} (pass --embeddings to point at a local copy)",
                emb_path.display()
            )
        })?;
        models.push(EvalModel {
            size,
            embedder,
            embeddings,
            qcache: HashMap::new(),
        });
    }

    let id_to_doc = id_map(&corpus);
    Ok(EvalCtx {
        corpus,
        index,
        tokenizer,
        id_to_doc,
        models,
        weights,
        top: args.top,
    })
}

/// 1-based rank and score of `doc` in a ranked list.
fn rank_of(list: &[(u32, f64)], doc: u32) -> Option<(usize, f64)> {
    list.iter()
        .position(|&(d, _)| d == doc)
        .map(|i| (i + 1, list[i].1))
}

fn method_json(hit: Option<(usize, f64)>, n: usize) -> Value {
    match hit {
        Some((rank, score)) => json!({"rank": rank, "score": score, "n": n}),
        None => json!({"rank": null, "score": null, "n": n}),
    }
}

/// Evaluate one case. Returns a `result` line, or an `error` line for
/// unknown targets; only shared-artifact invariant breaks are hard errors.
fn eval_case(ctx: &mut EvalCtx, case_id: Option<&str>, query: &str, target: &str) -> Result<Value> {
    let Some(&doc) = ctx.id_to_doc.get(&normalize_id(target)) else {
        return Ok(json!({
            "type": "error", "id": case_id, "query": query,
            "error": format!("target id not found in corpus: {target}"),
        }));
    };

    let q = prepare_query(query);
    let terms = expand_query(&q, ctx.tokenizer.as_ref());
    let bm25_list = bm25::search(&ctx.corpus, &ctx.index, &terms);

    let mut sem_lists: Vec<(&'static str, Vec<(u32, f64)>)> = Vec::new();
    for m in &mut ctx.models {
        m.ensure_query_vec(&q)?;
        let qv = &m.qcache[&q];
        let ranked = semantic_rank(&ctx.corpus, &m.embeddings, qv)?;
        sem_lists.push((m.size.embeddings_key(), ranked));
    }

    let mut lists: Vec<(&[(u32, f64)], f64)> = vec![(bm25_list.as_slice(), ctx.weights.bm25)];
    for (key, l) in &sem_lists {
        let w = match *key {
            "small" => ctx.weights.small,
            _ => ctx.weights.large,
        };
        lists.push((l.as_slice(), w));
    }
    let hybrid = rrf_merge_weighted(&lists, RRF_K);

    let mut methods = serde_json::Map::new();
    methods.insert(
        "bm25".into(),
        method_json(rank_of(&bm25_list, doc), bm25_list.len()),
    );
    for (key, l) in &sem_lists {
        methods.insert((*key).into(), method_json(rank_of(l, doc), l.len()));
    }
    methods.insert(
        "hybrid".into(),
        method_json(rank_of(&hybrid, doc), hybrid.len()),
    );

    let top: Vec<Value> = hybrid
        .iter()
        .take(ctx.top)
        .map(|&(d, s)| json!({"id": ctx.corpus.records[d as usize].id, "score": s}))
        .collect();

    Ok(json!({
        "type": "result", "id": case_id, "query": query, "target": target,
        "methods": Value::Object(methods), "top": top,
    }))
}

/// Handle one input line (serve mode): a command object or a case. Malformed
/// input yields an `error` line rather than killing a long overnight run.
fn handle_line(ctx: &mut EvalCtx, line: &str) -> Result<Value> {
    let v: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return Ok(json!({"type": "error", "error": format!("bad JSON: {e}")})),
    };
    if let Some(cmd) = v.get("cmd").and_then(Value::as_str) {
        return handle_cmd(ctx, cmd, &v);
    }
    let (Some(query), Some(target)) = (
        v.get("query").and_then(Value::as_str),
        v.get("target").and_then(Value::as_str),
    ) else {
        return Ok(json!({
            "type": "error",
            "error": "expected {\"query\":…,\"target\":…} or {\"cmd\":…}",
        }));
    };
    let case_id = v.get("id").and_then(Value::as_str);
    eval_case(ctx, case_id, query, target)
}

fn handle_cmd(ctx: &mut EvalCtx, cmd: &str, v: &Value) -> Result<Value> {
    match cmd {
        // Swap in an edited dataset: reparse + rebuild the BM25 index in
        // memory. Embeddings/query cache stay — the optimizer only ever edits
        // fields outside the embedded text (questions[1+], keywords, synonyms).
        "reload_data" => {
            let Some(path) = v.get("path").and_then(Value::as_str) else {
                return Ok(json!({"type": "error", "error": "reload_data needs \"path\""}));
            };
            match load_local_data(Path::new(path)) {
                Ok((corpus, index)) => {
                    ctx.id_to_doc = id_map(&corpus);
                    ctx.corpus = corpus;
                    ctx.index = index;
                    Ok(json!({
                        "type": "ok", "cmd": "reload_data", "records": ctx.corpus.len(),
                    }))
                }
                Err(e) => Ok(json!({
                    "type": "error", "error": format!("reload_data failed: {e:#}"),
                })),
            }
        }
        // Retune RRF weights without reloading anything (weight sweeps).
        "weights" => {
            let get = |k: &str, cur: f64| v.get(k).and_then(Value::as_f64).unwrap_or(cur);
            ctx.weights = CustomWeights {
                bm25: get("bm25", ctx.weights.bm25),
                small: get("small", ctx.weights.small),
                large: get("large", ctx.weights.large),
            };
            Ok(json!({"type": "ok", "cmd": "weights", "weights": weights_json(&ctx.weights)}))
        }
        "ping" => Ok(json!({"type": "ok", "cmd": "ping"})),
        other => Ok(json!({"type": "error", "error": format!("unknown cmd: {other}")})),
    }
}

fn weights_json(w: &CustomWeights) -> Value {
    json!({"bm25": w.bm25, "small": w.small, "large": w.large})
}

fn ready_line(ctx: &EvalCtx) -> Value {
    let models: Vec<&str> = ctx.models.iter().map(|m| m.size.embeddings_key()).collect();
    json!({
        "type": "ready", "records": ctx.corpus.len(), "models": models,
        "weights": weights_json(&ctx.weights),
    })
}

fn serve(ctx: &mut EvalCtx) -> Result<()> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "{}", ready_line(ctx))?;
    out.flush()?;
    for line in std::io::stdin().lock().lines() {
        let line = line.context("reading stdin")?;
        if line.trim().is_empty() {
            continue;
        }
        let resp = handle_line(ctx, line.trim())?;
        writeln!(out, "{resp}")?;
        out.flush()?;
    }
    Ok(())
}

/// Per-method rank aggregates for the batch summary line.
#[derive(Default)]
struct MethodStats {
    ranks: Vec<Option<usize>>,
}

impl MethodStats {
    fn push(&mut self, rank: Option<usize>) {
        self.ranks.push(rank);
    }

    fn json(&self) -> Value {
        let n = self.ranks.len();
        let hits = |k: usize| {
            self.ranks
                .iter()
                .filter(|r| r.is_some_and(|r| r <= k))
                .count()
        };
        let mrr = if n == 0 {
            0.0
        } else {
            self.ranks
                .iter()
                .map(|r| r.map_or(0.0, |r| 1.0 / r as f64))
                .sum::<f64>()
                / n as f64
        };
        json!({
            "cases": n, "top1": hits(1), "top3": hits(3), "top10": hits(10),
            "top1_rate": if n == 0 { 0.0 } else { hits(1) as f64 / n as f64 },
            "mrr": mrr,
        })
    }
}

fn batch(ctx: &mut EvalCtx, cases: &Path) -> Result<()> {
    let reader: Box<dyn BufRead> = if cases == Path::new("-") {
        Box::new(std::io::stdin().lock())
    } else {
        Box::new(std::io::BufReader::new(
            std::fs::File::open(cases).with_context(|| format!("opening {}", cases.display()))?,
        ))
    };
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    let mut stats: HashMap<String, MethodStats> = HashMap::new();
    let mut errors = 0usize;
    for line in reader.lines() {
        let line = line.context("reading cases")?;
        if line.trim().is_empty() {
            continue;
        }
        let resp = handle_line(ctx, line.trim())?;
        if resp["type"] == "result" {
            if let Some(methods) = resp["methods"].as_object() {
                for (k, m) in methods {
                    let rank = m["rank"].as_u64().map(|r| r as usize);
                    stats.entry(k.clone()).or_default().push(rank);
                }
            }
        } else {
            errors += 1;
        }
        writeln!(out, "{resp}")?;
    }

    let methods: serde_json::Map<String, Value> =
        stats.iter().map(|(k, s)| (k.clone(), s.json())).collect();
    writeln!(
        out,
        "{}",
        json!({"type": "summary", "errors": errors, "methods": Value::Object(methods)})
    )?;
    out.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Record;

    struct FakeTokenizer;
    impl QueryTokenizer for FakeTokenizer {
        fn morphemes(&self, text: &str) -> Vec<String> {
            text.split_whitespace().map(str::to_string).collect()
        }
        fn tag(&self) -> &'static str {
            "fake"
        }
    }

    fn record(id: &str, question: &str, search_text: &str) -> Record {
        serde_json::from_str(&format!(
            r#"{{"id":"{id}","questions":["{question}"],"search_text":"{search_text}"}}"#
        ))
        .unwrap()
    }

    fn bm25_only_ctx() -> EvalCtx {
        let corpus = Arc::new(Corpus::new(vec![
            record("aaa", "電磁誘導とは", "電磁 誘導 法則 コイル"),
            record("bbb", "クーロンの法則", "クーロン 法則 電荷"),
        ]));
        let index = Arc::new(Bm25Index::build(&corpus, "fake", "hash"));
        let id_to_doc = id_map(&corpus);
        EvalCtx {
            corpus,
            index,
            tokenizer: Arc::new(FakeTokenizer),
            id_to_doc,
            models: Vec::new(),
            weights: CustomWeights::default(),
            top: 3,
        }
    }

    #[test]
    fn eval_case_reports_target_rank_per_method() {
        let mut ctx = bm25_only_ctx();
        let out = eval_case(&mut ctx, Some("c1"), "クーロン 電荷", "bbb").unwrap();
        assert_eq!(out["type"], "result");
        assert_eq!(out["id"], "c1");
        assert_eq!(out["methods"]["bm25"]["rank"], 1);
        assert_eq!(out["methods"]["hybrid"]["rank"], 1);
        assert_eq!(out["top"][0]["id"], "bbb");
    }

    #[test]
    fn eval_case_unknown_target_is_a_soft_error() {
        let mut ctx = bm25_only_ctx();
        let out = eval_case(&mut ctx, None, "クーロン", "nope").unwrap();
        assert_eq!(out["type"], "error");
    }

    #[test]
    fn eval_case_missing_target_reports_null_rank() {
        let mut ctx = bm25_only_ctx();
        // "aaa" never matches a クーロン query → absent from the BM25 list.
        let out = eval_case(&mut ctx, None, "クーロン 電荷", "aaa").unwrap();
        assert_eq!(out["methods"]["bm25"]["rank"], Value::Null);
    }

    #[test]
    fn handle_line_dispatches_commands_and_cases() {
        let mut ctx = bm25_only_ctx();
        let ok = handle_line(&mut ctx, r#"{"cmd":"weights","bm25":0.5}"#).unwrap();
        assert_eq!(ok["type"], "ok");
        assert_eq!(ctx.weights.bm25, 0.5);
        assert_eq!(ctx.weights.small, CustomWeights::default().small);

        let bad = handle_line(&mut ctx, "not json").unwrap();
        assert_eq!(bad["type"], "error");

        let case =
            handle_line(&mut ctx, r#"{"id":"x","query":"クーロン","target":"bbb"}"#).unwrap();
        assert_eq!(case["type"], "result");
    }

    #[test]
    fn parse_weights_accepts_three_numbers() {
        let w = parse_weights("1, 2.5,0").unwrap();
        assert_eq!((w.bm25, w.small, w.large), (1.0, 2.5, 0.0));
        assert!(parse_weights("1,2").is_err());
        assert!(parse_weights("a,b,c").is_err());
    }

    #[test]
    fn method_stats_aggregates_top_k_and_mrr() {
        let mut s = MethodStats::default();
        s.push(Some(1));
        s.push(Some(4));
        s.push(None);
        let j = s.json();
        assert_eq!(j["cases"], 3);
        assert_eq!(j["top1"], 1);
        assert_eq!(j["top3"], 1);
        assert_eq!(j["top10"], 2);
        let mrr = j["mrr"].as_f64().unwrap();
        assert!((mrr - (1.0 + 0.25) / 3.0).abs() < 1e-12);
    }
}
