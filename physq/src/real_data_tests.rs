//! End-to-end sanity against the real shared artifacts. The crate lives
//! inside the data repo, so `../q_and_a_data.json` and `../embeddings.json`
//! are the same files the web serves. Tests skip (with a note) if the files
//! are absent, e.g. when the crate is vendored elsewhere.
//!
//! Lives in the lib (not `tests/`) so `cargo test` links one binary instead
//! of two — the embedded IPADIC dictionary makes every extra binary ~100 MB.
#![cfg(test)]

use crate::bm25::{self, Bm25Index};
use crate::config::ModelSize;
use crate::model::Corpus;
use crate::query::{LinderaIpadic, expand_query, prepare_query};
use crate::semantic::{CorpusEmbeddings, semantic_rank};

fn repo_file(name: &str) -> Option<Vec<u8>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(name);
    std::fs::read(path).ok()
}

fn real_corpus() -> Option<Corpus> {
    let bytes = repo_file("q_and_a_data.json")?;
    Some(Corpus::from_json(&bytes).expect("real q_and_a_data.json must parse"))
}

#[test]
fn real_queries_rank_relevant_records_on_top() {
    let Some(corpus) = real_corpus() else {
        eprintln!("real q_and_a_data.json not present; skipping");
        return;
    };
    let tokenizer = LinderaIpadic::new().expect("lindera IPADIC init");
    let index = Bm25Index::build(&corpus, "lindera-ipadic", "test-hash");

    // (query, substring expected in one of the top-3 results' text)
    let cases = [
        ("電磁誘導", "電磁誘導"),
        ("クーロンの法則", "クーロン"),
        ("運動方程式", "運動方程式"),
        // kana query must still match via the kana readings in search_text
        ("でんじゆうどう", "電磁誘導"),
    ];
    for (query, expect) in cases {
        let q = prepare_query(query);
        let terms = expand_query(&q, &tokenizer);
        let results = bm25::search(&corpus, &index, &terms);
        assert!(!results.is_empty(), "no BM25 results for {query}");
        let top: Vec<String> = results
            .iter()
            .take(3)
            .map(|&(d, _)| {
                let r = &corpus.records[d as usize];
                format!("{} {}", r.questions.join(" "), r.search_text)
            })
            .collect();
        assert!(
            top.iter().any(|t| t.contains(expect)),
            "top-3 for {query:?} did not mention {expect:?}: {:?}",
            results
                .iter()
                .take(3)
                .map(|&(d, _)| corpus.records[d as usize].questions.first().cloned())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn real_embeddings_are_hit_by_raw_record_ids() {
    let (Some(corpus), Some(emb_bytes)) = (real_corpus(), repo_file("embeddings.json")) else {
        eprintln!("real data files not present; skipping");
        return;
    };
    let emb = CorpusEmbeddings::from_json(&emb_bytes, ModelSize::Small)
        .expect("embeddings.json must load with 384-dim small matrix");

    // §7.5: ids are UUIDs, normalizeId is a pass-through, lookups by raw
    // item.id must hit. Some records may legitimately lack a vector.
    let hits = corpus
        .records
        .iter()
        .filter(|r| emb.vectors.contains_key(&r.id))
        .count();
    assert!(
        hits * 10 >= corpus.len() * 9,
        "embedding lookup by raw id hit only {hits}/{} records — id handling is broken",
        corpus.len()
    );
    assert_eq!(emb.dim, 384);
}

#[test]
fn real_semantic_rank_is_self_consistent() {
    // Uses a real corpus vector as the query vector: the record it belongs
    // to must rank #1 (cosine ≈ 1), exercising load → id lookup → cosine →
    // ordering on the real artifact without needing the model download.
    let (Some(corpus), Some(emb_bytes)) = (real_corpus(), repo_file("embeddings.json")) else {
        eprintln!("real data files not present; skipping");
        return;
    };
    let emb = CorpusEmbeddings::from_json(&emb_bytes, ModelSize::Small).unwrap();
    let (probe_idx, probe_vec) = corpus
        .records
        .iter()
        .enumerate()
        .find_map(|(i, r)| emb.vectors.get(&r.id).map(|(v, _)| (i, v.clone())))
        .expect("at least one record has an embedding");

    let ranked = semantic_rank(&corpus, &emb, &probe_vec).unwrap();
    assert_eq!(ranked[0].0 as usize, probe_idx, "self-similarity must win");
    assert!(
        (ranked[0].1 - 1.0).abs() < 1e-6,
        "cosine of a vector with itself ≈ 1"
    );
    // and the whole embedded corpus is ranked, not just BM25 candidates
    assert_eq!(
        ranked.len(),
        corpus
            .records
            .iter()
            .filter(|r| emb.vectors.contains_key(&r.id))
            .count()
    );
}

#[test]
fn real_search_text_is_used_as_shipped_never_retokenized() {
    let Some(corpus) = real_corpus() else {
        eprintln!("real q_and_a_data.json not present; skipping");
        return;
    };
    // The BM25 corpus tokens must be exactly the whitespace tokens of the
    // lowercased search_text — nothing added, nothing re-segmented.
    let index = Bm25Index::build(&corpus, "lindera-ipadic", "test-hash");
    let expected_total: usize = corpus.st_tokens.iter().map(Vec::len).sum();
    let expected_avgdl = expected_total as f64 / corpus.len() as f64;
    assert!((index.avgdl - expected_avgdl).abs() < 1e-9);
    for (i, tokens) in corpus.st_tokens.iter().enumerate() {
        assert_eq!(index.doc_len[i] as usize, tokens.len());
    }
}
