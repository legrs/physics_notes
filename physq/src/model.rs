//! Q&A corpus records (`q_and_a_data.json`, CLAUDE.md §10) and the id
//! normalization rule shared with `build.js` / `search.html`.

use std::fmt;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde::de::{self, Deserializer, SeqAccess, Visitor};

/// Byte-identical port of `normalizeId` in `build.js` / `search.html`:
/// trim, then strip leading zeros on all-digit ids (`"00001"` → `"1"`,
/// `"000"` → `"0"`); anything else (UUIDs) passes through unchanged.
pub fn normalize_id(id: &str) -> String {
    let s = id.trim();
    if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
        let stripped = s.trim_start_matches('0');
        if stripped.is_empty() {
            "0".to_string()
        } else {
            stripped.to_string()
        }
    } else {
        s.to_string()
    }
}

/// `build.js` calls `String(id)` before normalizing, so numeric JSON ids are
/// accepted too.
fn deserialize_id<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        other => Err(de::Error::custom(format!("unsupported id type: {other}"))),
    }
}

/// `category` is an array today but was a plain string (possibly
/// `"a | b"`-separated) historically; mirror the web's `normalizeCategory`:
/// arrays are trimmed and empties dropped, strings split on `|`.
fn deserialize_string_or_vec<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct V;
    impl<'de> Visitor<'de> for V {
        type Value = Vec<String>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a string or an array of strings")
        }
        fn visit_str<E: de::Error>(self, s: &str) -> std::result::Result<Vec<String>, E> {
            Ok(s.split('|')
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .map(str::to_string)
                .collect())
        }
        fn visit_seq<A: SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Vec<String>, A::Error> {
            let mut out = Vec::new();
            while let Some(s) = seq.next_element::<String>()? {
                let s = s.trim().to_string();
                if !s.is_empty() {
                    out.push(s);
                }
            }
            Ok(out)
        }
    }
    deserializer.deserialize_any(V)
}

#[derive(Debug, Clone, Deserialize)]
pub struct Record {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: String,
    #[serde(default)]
    pub questions: Vec<String>,
    #[serde(default)]
    pub answer: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub synonyms: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub category: Vec<String>,
    #[serde(default)]
    pub difficulty: String,
    /// JS applies `score * (priority || 1)`, so 0/absent both mean 1.
    #[serde(default)]
    pub priority: f64,
    #[serde(default)]
    pub related: Vec<String>,
    #[serde(default)]
    pub updated_at: String,
    /// Pre-tokenized upstream (kuromoji morphemes + kana readings). The BM25
    /// source of truth — never re-tokenized here (CLAUDE.md §3, §10).
    #[serde(default)]
    pub search_text: String,
}

impl Record {
    /// `score * (item.priority || 1)` — JS `||` treats 0 as falsy.
    pub fn effective_priority(&self) -> f64 {
        if self.priority == 0.0 {
            1.0
        } else {
            self.priority
        }
    }
}

/// Corpus with the per-record lowercased strings `_scoreItem` works on,
/// precomputed once (the web lowercases on every call; same values).
pub struct Corpus {
    pub records: Vec<Record>,
    /// `search_text.toLowerCase()` per record.
    pub st_lower: Vec<String>,
    /// Whitespace tokens of `st_lower` — the BM25 corpus tokens
    /// (`search_text.toLowerCase().split(/\s+/)`, `search.html` `_buildBM25Index`).
    pub st_tokens: Vec<Vec<String>>,
    pub questions_lower: Vec<Vec<String>>,
    pub keywords_lower: Vec<Vec<String>>,
    pub synonyms_lower: Vec<Vec<String>>,
}

impl Corpus {
    /// Parse `q_and_a_data.json` bytes and normalize ids/related in place,
    /// mirroring the web's `normalizeData`.
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        let mut records: Vec<Record> =
            serde_json::from_slice(bytes).context("failed to parse q_and_a_data.json")?;
        for r in &mut records {
            r.id = normalize_id(&r.id);
            for rel in &mut r.related {
                *rel = normalize_id(rel);
            }
        }
        Ok(Self::new(records))
    }

    pub fn new(records: Vec<Record>) -> Self {
        let st_lower: Vec<String> = records
            .iter()
            .map(|r| r.search_text.to_lowercase())
            .collect();
        let st_tokens: Vec<Vec<String>> = st_lower
            .iter()
            .map(|st| st.split_whitespace().map(str::to_string).collect())
            .collect();
        let questions_lower = records
            .iter()
            .map(|r| r.questions.iter().map(|s| s.to_lowercase()).collect())
            .collect();
        let keywords_lower = records
            .iter()
            .map(|r| r.keywords.iter().map(|s| s.to_lowercase()).collect())
            .collect();
        let synonyms_lower = records
            .iter()
            .map(|r| r.synonyms.iter().map(|s| s.to_lowercase()).collect())
            .collect();
        Self {
            records,
            st_lower,
            st_tokens,
            questions_lower,
            keywords_lower,
            synonyms_lower,
        }
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_id_strips_leading_zeros_on_numeric_ids() {
        assert_eq!(normalize_id("00001"), "1");
        assert_eq!(normalize_id("00100"), "100");
        assert_eq!(normalize_id("42"), "42");
        // /^0+(?=\d)/ keeps the final digit: "000" → "0"
        assert_eq!(normalize_id("000"), "0");
        assert_eq!(normalize_id("0"), "0");
    }

    #[test]
    fn normalize_id_passes_non_numeric_through() {
        assert_eq!(
            normalize_id("2e7f2483-54ac-4c28-9b19-e3f2e58fdc04"),
            "2e7f2483-54ac-4c28-9b19-e3f2e58fdc04"
        );
        assert_eq!(normalize_id("1e3"), "1e3"); // not all digits → untouched
        assert_eq!(normalize_id("00a"), "00a");
        assert_eq!(normalize_id(""), "");
    }

    #[test]
    fn normalize_id_trims_like_build_js() {
        assert_eq!(normalize_id(" 007 "), "7");
        assert_eq!(normalize_id("  uuid-x "), "uuid-x");
    }

    #[test]
    fn effective_priority_treats_zero_and_missing_as_one() {
        let mut r: Record = serde_json::from_str(r#"{"id":"x"}"#).unwrap();
        assert_eq!(r.effective_priority(), 1.0);
        r.priority = 0.0;
        assert_eq!(r.effective_priority(), 1.0);
        r.priority = 3.0;
        assert_eq!(r.effective_priority(), 3.0);
    }

    #[test]
    fn category_accepts_string_or_array() {
        let r: Record = serde_json::from_str(r#"{"id":"x","category":"力学"}"#).unwrap();
        assert_eq!(r.category, vec!["力学"]);
        let r: Record = serde_json::from_str(r#"{"id":"x","category":["力学","波動"]}"#).unwrap();
        assert_eq!(r.category, vec!["力学", "波動"]);
        // legacy " | "-separated string form, like the web's normalizeCategory
        let r: Record = serde_json::from_str(r#"{"id":"x","category":"力学 | 波動"}"#).unwrap();
        assert_eq!(r.category, vec!["力学", "波動"]);
    }

    #[test]
    fn numeric_json_ids_are_stringified() {
        let r: Record = serde_json::from_str(r#"{"id":7}"#).unwrap();
        assert_eq!(r.id, "7");
    }
}
