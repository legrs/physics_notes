//! BM25 index + the web's full lexical scoring (`search.html`
//! `_buildBM25Index` / `_bm25` / `_scoreItem` / `_typoScore` / `_ngramScore`),
//! ported term for term. Corpus tokens come from the pre-tokenized
//! `search_text` (lowercased, whitespace-split) — never re-tokenized.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::{BM25_B, BM25_K1, RELATED_BOOST};
use crate::model::Corpus;
use crate::query::QueryTerms;

const INDEX_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct TermEntry {
    pub df: u32,
    /// doc index → term frequency
    pub docs: HashMap<u32, u32>,
}

/// Serializable BM25 index (`bm25_index.bin`, CLAUDE.md §4). Carries the
/// query-tokenizer tag and the data hash; a mismatch on either forces a
/// rebuild (§3).
#[derive(Debug, Serialize, Deserialize)]
pub struct Bm25Index {
    pub schema_version: u32,
    pub tokenizer_tag: String,
    pub data_hash: String,
    pub n_docs: u32,
    pub avgdl: f64,
    pub doc_len: Vec<u32>,
    pub terms: HashMap<String, TermEntry>,
}

impl Bm25Index {
    /// Port of `_buildBM25Index`: tokens are
    /// `search_text.toLowerCase().split(/\s+/).filter(Boolean)`.
    pub fn build(corpus: &Corpus, tokenizer_tag: &str, data_hash: &str) -> Self {
        let mut terms: HashMap<String, TermEntry> = HashMap::new();
        let mut total_len: u64 = 0;
        let mut doc_len = Vec::with_capacity(corpus.len());

        for (i, tokens) in corpus.st_tokens.iter().enumerate() {
            total_len += tokens.len() as u64;
            doc_len.push(tokens.len() as u32);
            let mut tf: HashMap<&str, u32> = HashMap::new();
            for t in tokens {
                *tf.entry(t.as_str()).or_insert(0) += 1;
            }
            for (t, freq) in tf {
                let entry = terms.entry(t.to_string()).or_insert_with(|| TermEntry {
                    df: 0,
                    docs: HashMap::new(),
                });
                entry.df += 1;
                entry.docs.insert(i as u32, freq);
            }
        }

        let n = corpus.len();
        Self {
            schema_version: INDEX_SCHEMA_VERSION,
            tokenizer_tag: tokenizer_tag.to_string(),
            data_hash: data_hash.to_string(),
            n_docs: n as u32,
            // `data.length ? totalLen / data.length : 1`
            avgdl: if n > 0 {
                total_len as f64 / n as f64
            } else {
                1.0
            },
            doc_len,
            terms,
        }
    }

    /// Port of `_bm25(term, itemId, docLen, idx)` with k1=1.2, b=0.75.
    pub fn term_score(&self, term: &str, doc: u32, doc_len: f64) -> f64 {
        let Some(entry) = self.terms.get(term) else {
            return 0.0;
        };
        let Some(&tf) = entry.docs.get(&doc) else {
            return 0.0;
        };
        let tf = tf as f64;
        let df = entry.df as f64;
        let n = self.n_docs as f64;
        let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
        let tfn =
            tf * (BM25_K1 + 1.0) / (tf + BM25_K1 * (1.0 - BM25_B + BM25_B * doc_len / self.avgdl));
        idf * tfn
    }

    /// Port of `_getCandidates`: docs containing any expanded term;
    /// if none of the terms is in the index, every doc is a candidate.
    pub fn candidates(&self, expanded: &[String]) -> Vec<u32> {
        let mut ids: HashSet<u32> = HashSet::new();
        let mut any_term_known = false;
        for w in expanded {
            if let Some(entry) = self.terms.get(w) {
                any_term_known = true;
                ids.extend(entry.docs.keys().copied());
            }
        }
        if !any_term_known && ids.is_empty() {
            return (0..self.n_docs).collect();
        }
        // Preserve data order like the web's `data.filter(...)`.
        let mut v: Vec<u32> = (0..self.n_docs).filter(|i| ids.contains(i)).collect();
        v.sort_unstable();
        v
    }

    // Cache round-trip ---------------------------------------------------

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = bincode::serialize(self).context("failed to serialize BM25 index")?;
        let tmp = path.with_extension("bin.tmp");
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Load the cached index if its schema, tokenizer tag and data hash all
    /// still match; otherwise `None` → the caller rebuilds.
    pub fn load_if_valid(path: &Path, tokenizer_tag: &str, data_hash: &str) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        let idx: Bm25Index = bincode::deserialize(&bytes).ok()?;
        (idx.schema_version == INDEX_SCHEMA_VERSION
            && idx.tokenizer_tag == tokenizer_tag
            && idx.data_hash == data_hash)
            .then_some(idx)
    }
}

/// Levenshtein distance over chars (the web computes it over UTF-16 code
/// units; identical for all BMP text, which this corpus is).
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=a.len()).collect();
    let mut cur = vec![0usize; a.len() + 1];
    for i in 1..=b.len() {
        cur[0] = i;
        for j in 1..=a.len() {
            cur[j] = if b[i - 1] == a[j - 1] {
                prev[j - 1]
            } else {
                (prev[j - 1] + 1).min(cur[j - 1] + 1).min(prev[j] + 1)
            };
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[a.len()]
}

/// Port of `_typoScore(word, text)`: min Levenshtein of `word` vs each
/// whitespace token of the (lowercased) search_text → +2 if 1, +1 if 2,
/// else 0. Note an exact match (distance 0) scores 0, like the web.
pub fn typo_score(word: &str, st_tokens: &[String]) -> f64 {
    let wlen = word.chars().count();
    let mut best = usize::MAX;
    for t in st_tokens {
        // |len(word) − len(token)| is a lower bound on the distance; skipping
        // tokens that cannot reach distance ≤ 2 only avoids work when the
        // token could not affect the score — unless a distance-0 exact match
        // (which must force the score to 0) is possible, i.e. equal lengths.
        let tlen = t.chars().count();
        if wlen.abs_diff(tlen) > 2 {
            continue;
        }
        let d = levenshtein(word, t);
        if d < best {
            best = d;
            if best == 0 {
                break;
            }
        }
    }
    match best {
        1 => 2.0,
        2 => 1.0,
        _ => 0.0,
    }
}

/// Port of `_ngramScore(word, text)`: +0.5 for each character bigram of
/// `word` found as a substring of the (lowercased) search_text.
pub fn ngram_score(word: &str, st_lower: &str) -> f64 {
    let chars: Vec<char> = word.chars().collect();
    let mut s = 0.0;
    for w in chars.windows(2) {
        let bigram: String = w.iter().collect();
        if st_lower.contains(&bigram) {
            s += 0.5;
        }
    }
    s
}

/// Port of `_scoreItem(item, words, expandedWords, q, bm25idx)`: BM25 body
/// over the expanded terms, field boosts over the raw words, then the
/// priority multiplier.
pub fn score_item(corpus: &Corpus, index: &Bm25Index, doc: u32, terms: &QueryTerms) -> f64 {
    let i = doc as usize;
    let record = &corpus.records[i];
    let st = &corpus.st_lower[i];
    let st_tokens = &corpus.st_tokens[i];
    let doc_len = st_tokens.len() as f64;
    let q = &terms.q;

    let mut score = 0.0;

    // BM25 本体（展開済みワード全体で集計）
    for w in &terms.expanded {
        score += index.term_score(w, doc, doc_len);
    }

    // フィールドブースト
    if corpus.questions_lower[i].iter().any(|qq| qq == q) {
        score += 10.0;
    }
    if st.contains(q.as_str()) {
        score += 3.0;
    }
    for qq in &corpus.questions_lower[i] {
        if qq.contains(q.as_str()) {
            score += 3.0;
        }
    }
    for w in &terms.words {
        for k in &corpus.keywords_lower[i] {
            if k.contains(w.as_str()) {
                score += 1.0;
            }
        }
        score += typo_score(w, st_tokens);
        score += ngram_score(w, st);
    }
    for s in &corpus.synonyms_lower[i] {
        for w in &terms.words {
            if s.contains(w.as_str()) {
                score += 1.0;
            }
        }
    }
    // フレーズボーナス（隣接単語ペア、スペースなしで連結）
    for pair in terms.words.windows(2) {
        let joined = format!("{}{}", pair[0], pair[1]);
        if st.contains(&joined) {
            score += 2.0;
        }
    }

    score * record.effective_priority()
}

/// Full BM25 stage: candidates → score → keep `score > 0` → sort desc →
/// +0.5 to ids related from the top-3 → re-sort. This is exactly the
/// `bm25Results` list the web feeds into RRF (fully boosted, priority-scaled,
/// related-boosted & sorted).
pub fn search(corpus: &Corpus, index: &Bm25Index, terms: &QueryTerms) -> Vec<(u32, f64)> {
    if terms.words.is_empty() {
        return Vec::new();
    }
    let mut results: Vec<(u32, f64)> = index
        .candidates(&terms.expanded)
        .into_iter()
        .map(|doc| (doc, score_item(corpus, index, doc, terms)))
        .filter(|&(_, s)| s > 0.0)
        .collect();
    // Stable sort keeps ties in data order, like the web's Array.sort.
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let top_related: HashSet<&str> = results
        .iter()
        .take(3)
        .flat_map(|&(doc, _)| corpus.records[doc as usize].related.iter())
        .map(String::as_str)
        .collect();
    if !top_related.is_empty() {
        for r in &mut results {
            if top_related.contains(corpus.records[r.0 as usize].id.as_str()) {
                r.1 += RELATED_BOOST;
            }
        }
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Record;
    use crate::query::QueryTerms;

    fn record(id: &str, search_text: &str) -> Record {
        serde_json::from_str(&format!(
            r#"{{"id":"{id}","questions":[],"search_text":"{search_text}"}}"#
        ))
        .unwrap()
    }

    fn corpus3() -> Corpus {
        Corpus::new(vec![
            record("a", "電磁 誘導 法則"),
            record("b", "電磁 電磁 波"),
            record("c", "運動 方程式"),
        ])
    }

    fn terms(q: &str, words: &[&str], expanded: &[&str]) -> QueryTerms {
        QueryTerms {
            q: q.to_string(),
            words: words.iter().map(|s| s.to_string()).collect(),
            expanded: expanded.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn bm25_matches_hand_computed_values() {
        // N=3, avgdl=(3+3+2)/3=8/3. Term 電磁: df=2; doc a tf=1 len3, doc b tf=2 len3.
        let corpus = corpus3();
        let idx = Bm25Index::build(&corpus, "test", "hash");
        assert_eq!(idx.n_docs, 3);
        assert!((idx.avgdl - 8.0 / 3.0).abs() < 1e-12);

        let idf = ((3.0 - 2.0 + 0.5) / (2.0 + 0.5) + 1.0f64).ln(); // ln(1.6)
        let tfn_a = 1.0 * 2.2 / (1.0 + 1.2 * (0.25 + 0.75 * 3.0 / (8.0 / 3.0)));
        let expected_a = idf * tfn_a;
        assert!((idx.term_score("電磁", 0, 3.0) - expected_a).abs() < 1e-12);

        let tfn_b = 2.0 * 2.2 / (2.0 + 1.2 * (0.25 + 0.75 * 3.0 / (8.0 / 3.0)));
        let expected_b = idf * tfn_b;
        assert!((idx.term_score("電磁", 1, 3.0) - expected_b).abs() < 1e-12);

        // df=1 term
        let idf1 = ((3.0 - 1.0 + 0.5) / (1.0 + 0.5) + 1.0f64).ln(); // ln(8/3)
        let tfn_c = 1.0 * 2.2 / (1.0 + 1.2 * (0.25 + 0.75 * 2.0 / (8.0 / 3.0)));
        assert!((idx.term_score("運動", 2, 2.0) - idf1 * tfn_c).abs() < 1e-12);

        // unknown term / absent doc → 0
        assert_eq!(idx.term_score("光", 0, 3.0), 0.0);
        assert_eq!(idx.term_score("運動", 0, 3.0), 0.0);
    }

    #[test]
    fn candidates_fall_back_to_all_docs_when_no_term_known() {
        let corpus = corpus3();
        let idx = Bm25Index::build(&corpus, "test", "hash");
        assert_eq!(idx.candidates(&["電磁".to_string()]), vec![0, 1]);
        assert_eq!(idx.candidates(&["未知語".to_string()]), vec![0, 1, 2]);
    }

    #[test]
    fn levenshtein_basics() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("同じ", "同じ"), 0);
        assert_eq!(levenshtein("電磁", "電波"), 1);
    }

    #[test]
    fn typo_score_matches_web_thresholds() {
        let tokens: Vec<String> = ["bat", "hat"].iter().map(|s| s.to_string()).collect();
        assert_eq!(typo_score("cat", &tokens), 2.0); // best distance 1
        let tokens: Vec<String> = ["abcd"].iter().map(|s| s.to_string()).collect();
        assert_eq!(typo_score("ab", &tokens), 1.0); // distance 2
        let tokens: Vec<String> = ["xyzzy"].iter().map(|s| s.to_string()).collect();
        assert_eq!(typo_score("ab", &tokens), 0.0); // too far
                                                    // exact match present → best = 0 → score 0 (web behavior)
        let tokens: Vec<String> = ["cat", "cut"].iter().map(|s| s.to_string()).collect();
        assert_eq!(typo_score("cat", &tokens), 0.0);
    }

    #[test]
    fn ngram_score_counts_half_point_per_found_bigram() {
        assert_eq!(ngram_score("abc", "xx abc yy"), 1.0); // ab, bc
        assert_eq!(ngram_score("abcd", "xxbcxx"), 0.5); // only bc
        assert_eq!(ngram_score("a", "aaaa"), 0.0); // no bigram in a 1-char word
        assert_eq!(ngram_score("電磁誘導", "電磁 と 誘導"), 1.0); // 電磁+誘導, not 磁誘
    }

    #[test]
    fn score_item_applies_field_boosts_and_priority() {
        let mut records = vec![record("a", "電磁 誘導"), record("b", "運動 方程式")];
        records[0].questions = vec!["電磁誘導とは？".to_string()];
        records[0].keywords = vec!["電磁誘導".to_string()];
        records[0].synonyms = vec!["でんじゆうどう".to_string()];
        let corpus = Corpus::new(records);
        let idx = Bm25Index::build(&corpus, "t", "h");

        // q == "電磁誘導とは？" matches questions[0] exactly → +10, and the
        // "questions contains q" boost also fires → +3.
        let t = terms("電磁誘導とは？", &["電磁誘導とは？"], &["電磁"]);
        let s = score_item(&corpus, &idx, 0, &t);
        let bm25 = idx.term_score("電磁", 0, 2.0);
        // ngram bigrams of 電磁誘導とは？: 電磁(yes) 磁誘(no) 誘導(yes) 導と(no) とは(no) は？(no) → 1.0
        let expected = bm25 + 10.0 + 3.0 + 1.0 + 1.0; // +1 keyword contains? no: keyword "電磁誘導" does not contain the full word 電磁誘導とは？
                                                      // keyword boost: k.contains(w)? "電磁誘導".contains("電磁誘導とは？") = false → 0
                                                      // typo score vs tokens [電磁, 誘導]: distances ≥ 5 → 0
                                                      // synonym contains word? "でんじゆうどう".contains(...) = false → 0
        let _ = expected;
        assert!((s - (bm25 + 10.0 + 3.0 + 1.0)).abs() < 1e-9, "got {s}");

        // priority multiplies the whole thing
        let mut records2 = vec![record("a", "電磁 誘導")];
        records2[0].questions = vec!["電磁誘導とは？".to_string()];
        records2[0].priority = 2.0;
        let corpus2 = Corpus::new(records2);
        let idx2 = Bm25Index::build(&corpus2, "t", "h");
        let s2 = score_item(&corpus2, &idx2, 0, &t);
        let bm25_2 = idx2.term_score("電磁", 0, 2.0);
        assert!(
            (s2 - 2.0 * (bm25_2 + 10.0 + 3.0 + 1.0)).abs() < 1e-9,
            "got {s2}"
        );
    }

    #[test]
    fn keyword_and_synonym_boosts_count_per_entry() {
        let mut records = vec![record("a", "zzz")];
        records[0].keywords = vec!["電磁気学".to_string(), "電磁波".to_string()];
        records[0].synonyms = vec!["電磁誘導".to_string()];
        let corpus = Corpus::new(records);
        let idx = Bm25Index::build(&corpus, "t", "h");
        // word 電磁 is contained in both keywords (+1 each) and the synonym (+1)
        let t = terms("電磁", &["電磁"], &["電磁"]);
        let s = score_item(&corpus, &idx, 0, &t);
        // bm25 0 (term not in doc); typo vs [zzz] far; ngram: 電磁 not in "zzz"
        assert!((s - 3.0).abs() < 1e-9, "got {s}");
    }

    #[test]
    fn adjacent_pair_boost_concatenates_without_space() {
        let records = vec![record("a", "電磁誘導 の 法則")];
        let corpus = Corpus::new(records);
        let idx = Bm25Index::build(&corpus, "t", "h");
        let t = terms("電磁 誘導", &["電磁", "誘導"], &[]);
        let s = score_item(&corpus, &idx, 0, &t);
        // pair "電磁誘導" appears → +2; ngram: 電磁 in st +0.5, 誘導 in st +0.5
        // (1-char-bigram words 電磁/誘導 each have exactly one bigram)
        // typo: tokens [電磁誘導, の, 法則]; 電磁 vs の len diff 1 → lev 2 → +1 …
        // compute precisely: lev(電磁, 電磁誘導)=2 → +1; lev(誘導, 電磁誘導)=2,
        // lev(誘導, の)=2, lev(誘導, 法則)=2 → +1
        assert!((s - (2.0 + 0.5 + 0.5 + 1.0 + 1.0)).abs() < 1e-9, "got {s}");
    }

    #[test]
    fn search_applies_related_boost_from_top3() {
        // doc a matches strongly and lists c as related; c must get +0.5.
        let mut records = vec![
            record("a", "電磁 誘導 電磁 誘導"),
            record("b", "電磁 波"),
            record("c", "運動 電磁"),
        ];
        records[0].related = vec!["c".to_string()];
        let corpus = Corpus::new(records);
        let idx = Bm25Index::build(&corpus, "t", "h");
        let t = terms("電磁", &["電磁"], &["電磁"]);

        let base: Vec<(u32, f64)> = idx
            .candidates(&t.expanded)
            .into_iter()
            .map(|d| (d, score_item(&corpus, &idx, d, &t)))
            .collect();
        let results = search(&corpus, &idx, &t);
        let c_base = base.iter().find(|(d, _)| *d == 2).unwrap().1;
        let c_final = results.iter().find(|(d, _)| *d == 2).unwrap().1;
        assert!((c_final - (c_base + 0.5)).abs() < 1e-9);
    }

    #[test]
    fn empty_query_returns_nothing() {
        let corpus = corpus3();
        let idx = Bm25Index::build(&corpus, "t", "h");
        let t = terms("", &[], &[]);
        assert!(search(&corpus, &idx, &t).is_empty());
    }

    #[test]
    fn index_cache_roundtrip_and_invalidation() {
        let corpus = corpus3();
        let idx = Bm25Index::build(&corpus, "lindera-ipadic", "hash1");
        let dir = std::env::temp_dir().join(format!("physq-test-{}", std::process::id()));
        let path = dir.join("bm25_index.bin");
        idx.save(&path).unwrap();
        assert!(Bm25Index::load_if_valid(&path, "lindera-ipadic", "hash1").is_some());
        assert!(Bm25Index::load_if_valid(&path, "lindera-ipadic", "hash2").is_none());
        assert!(Bm25Index::load_if_valid(&path, "other-tokenizer", "hash1").is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
