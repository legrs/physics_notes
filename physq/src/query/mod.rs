//! Query-side tokenization and expansion.
//!
//! The corpus side is **never** re-tokenized — `search_text` already ships
//! kuromoji-tokenized (CLAUDE.md §3, §10). The query side uses lindera +
//! IPADIC morphemes (owner-approved deviation from the web's CJK-bigram
//! hack, §6) plus the same hiragana↔katakana expansion the web applies.
//! No CJK-bigram expansion, no synonym expansion, no user dictionary.

use anyhow::Result;
use lindera::dictionary::load_dictionary;
use lindera::mode::Mode;
use lindera::segmenter::Segmenter;
use lindera::tokenizer::Tokenizer;

/// The two term sets `_scoreItem(item, words, expandedWords, q, idx)` needs.
/// They must stay separate: the BM25 body sums over `expanded`; the field
/// boosts use the raw `words` (duplicates preserved, as in the web).
#[derive(Debug, Clone, PartialEq)]
pub struct QueryTerms {
    /// The full query, lowercased once and used everywhere (§6).
    pub q: String,
    /// Raw whitespace-split words of `q` (duplicates preserved).
    pub words: Vec<String>,
    /// Deduplicated expansion: words ∪ morphemes ∪ kana variants.
    pub expanded: Vec<String>,
}

/// Query tokenizer abstraction (kept small so a user dictionary or another
/// dictionary could be swapped in later — CLAUDE.md §8).
pub trait QueryTokenizer: Send + Sync {
    /// Morpheme surface forms for `text`, trimmed, unfiltered.
    fn morphemes(&self, text: &str) -> Vec<String>;
    /// Tag recorded in the BM25 index cache for invalidation.
    fn tag(&self) -> &'static str;
}

/// lindera + embedded IPADIC (locked decision, CLAUDE.md §3).
pub struct LinderaIpadic {
    tokenizer: Tokenizer,
}

impl LinderaIpadic {
    pub fn new() -> Result<Self> {
        let dictionary = load_dictionary("embedded://ipadic")
            .map_err(|e| anyhow::anyhow!("failed to load embedded IPADIC dictionary: {e}"))?;
        let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
        Ok(Self {
            tokenizer: Tokenizer::new(segmenter),
        })
    }
}

impl QueryTokenizer for LinderaIpadic {
    fn morphemes(&self, text: &str) -> Vec<String> {
        match self.tokenizer.tokenize(text) {
            Ok(tokens) => tokens
                .into_iter()
                .map(|t| t.surface.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            // A tokenizer failure must not take the search down; BM25 still
            // works on the raw words.
            Err(_) => Vec::new(),
        }
    }

    fn tag(&self) -> &'static str {
        crate::config::TOKENIZER_TAG
    }
}

/// The `build.js` morpheme filter, applied to query morphemes so they line up
/// with the corpus tokens: `t.length >= 2 || /[a-zA-Z0-9]/.test(t)` —
/// drop JP tokens shorter than 2 chars, keep 1-char ASCII alphanumerics
/// (§10). JS `length` counts UTF-16 code units, so we do too.
pub fn keep_token(t: &str) -> bool {
    t.encode_utf16().count() >= 2 || t.chars().any(|c| c.is_ascii_alphanumeric())
}

/// Katakana → hiragana, exactly the web's `/[ァ-ヶ]/` − 0x60.
pub fn kata_to_hira(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{30a1}'..='\u{30f6}' => char::from_u32(c as u32 - 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// Hiragana → katakana, exactly the web's `/[ぁ-ゖ]/` + 0x60.
pub fn hira_to_kata(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{3041}'..='\u{3096}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// Build the CLI query term sets from an already-lowercased query (§6 CLI
/// deviation): raw whitespace words, plus lindera morphemes filtered like
/// `build.js`, plus hiragana↔katakana variants of every term. The web's
/// CJK-bigram expansion is intentionally dropped.
pub fn expand_query(q_lower: &str, tokenizer: &dyn QueryTokenizer) -> QueryTerms {
    let words: Vec<String> = q_lower.split_whitespace().map(str::to_string).collect();

    let mut expanded: Vec<String> = Vec::new();
    let push_unique = |set: &mut Vec<String>, term: String| {
        if !term.is_empty() && !set.contains(&term) {
            set.push(term);
        }
    };

    for w in &words {
        push_unique(&mut expanded, w.clone());
    }
    for m in tokenizer.morphemes(q_lower) {
        if keep_token(&m) {
            push_unique(&mut expanded, m);
        }
    }
    // Kana variants of every term (the corpus search_text has katakana and
    // hiragana readings appended, §10, so both variants can earn matches).
    let base: Vec<String> = expanded.clone();
    for term in &base {
        let hira = kata_to_hira(term);
        if hira != *term {
            push_unique(&mut expanded, hira);
        }
        let kata = hira_to_kata(term);
        if kata != *term {
            push_unique(&mut expanded, kata);
        }
    }

    QueryTerms {
        q: q_lower.to_string(),
        words,
        expanded,
    }
}

/// Lowercase the query once (the web does `decodeURIComponent(...).toLowerCase()`
/// and reuses that string everywhere — §6).
pub fn prepare_query(raw: &str) -> String {
    raw.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeTokenizer(Vec<&'static str>);
    impl QueryTokenizer for FakeTokenizer {
        fn morphemes(&self, _text: &str) -> Vec<String> {
            self.0.iter().map(|s| s.to_string()).collect()
        }
        fn tag(&self) -> &'static str {
            "fake"
        }
    }

    #[test]
    fn keep_token_matches_build_js_filter() {
        // 2+ char JP tokens kept
        assert!(keep_token("する"));
        assert!(keep_token("静電気"));
        // 1-char JP particles/symbols dropped
        assert!(!keep_token("の"));
        assert!(!keep_token("。"));
        assert!(!keep_token("、"));
        // 1-char ASCII alphanumerics kept (/[a-zA-Z0-9]/)
        assert!(keep_token("v"));
        assert!(keep_token("9"));
        assert!(keep_token("F"));
        // 1-char full-width digit: not ASCII, length 1 → dropped (same as JS)
        assert!(!keep_token("１"));
        // mixed alnum tokens kept regardless of length
        assert!(keep_token("x2"));
        assert!(!keep_token(""));
    }

    #[test]
    fn kana_conversion_matches_web_ranges() {
        assert_eq!(hira_to_kata("ぶつり"), "ブツリ");
        assert_eq!(kata_to_hira("ブツリ"), "ぶつり");
        // Range boundaries: ゖ U+3096 ↔ ヶ U+30F6
        assert_eq!(hira_to_kata("ゖ"), "ヶ");
        assert_eq!(kata_to_hira("ヶ"), "ゖ");
        // ヷ U+30F7 is outside the web's regex range → untouched
        assert_eq!(kata_to_hira("ヷ"), "ヷ");
        // Non-kana untouched
        assert_eq!(hira_to_kata("物理abc"), "物理abc");
        assert_eq!(kata_to_hira("物理abc"), "物理abc");
    }

    #[test]
    fn expand_query_keeps_words_and_filtered_morphemes_and_kana_variants() {
        let tok = FakeTokenizer(vec!["静電気", "力", "の", "せいでんき"]);
        let terms = expand_query("静電気力の せいでんき", &tok);
        assert_eq!(terms.q, "静電気力の せいでんき");
        assert_eq!(terms.words, vec!["静電気力の", "せいでんき"]);
        // raw words present
        assert!(terms.expanded.contains(&"静電気力の".to_string()));
        // 2+ char morpheme kept, 1-char JP morphemes dropped
        assert!(terms.expanded.contains(&"静電気".to_string()));
        assert!(!terms.expanded.contains(&"力".to_string()));
        assert!(!terms.expanded.contains(&"の".to_string()));
        // kana variant of the hiragana word/morpheme added
        assert!(terms.expanded.contains(&"セイデンキ".to_string()));
        // no duplicates
        let mut dedup = terms.expanded.clone();
        dedup.sort();
        dedup.dedup();
        assert_eq!(dedup.len(), terms.expanded.len());
    }

    #[test]
    fn expand_query_adds_no_cjk_bigrams() {
        // The web would add 電磁/磁誘/誘導 bigrams; the CLI must not.
        let tok = FakeTokenizer(vec!["電磁", "誘導"]);
        let terms = expand_query("電磁誘導", &tok);
        assert!(!terms.expanded.contains(&"磁誘".to_string()));
        assert!(terms.expanded.contains(&"電磁".to_string()));
        assert!(terms.expanded.contains(&"誘導".to_string()));
        assert!(terms.expanded.contains(&"電磁誘導".to_string()));
    }

    #[test]
    fn prepare_query_lowercases_once() {
        assert_eq!(prepare_query("  Lenz's LAW  "), "lenz's law");
    }

    #[test]
    fn words_preserve_duplicates_expanded_dedups() {
        let tok = FakeTokenizer(vec![]);
        let terms = expand_query("物理 物理", &tok);
        assert_eq!(terms.words, vec!["物理", "物理"]);
        assert_eq!(terms.expanded, vec!["物理"]);
    }
}
