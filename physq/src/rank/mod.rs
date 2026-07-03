//! RRF fusion of the (fully boosted, sorted) BM25 list with the semantic
//! ranking — port of `search.html` `_rrfMerge(bm25Results, semanticRanked,
//! k = 60, semanticWeight = 2.0)`.

use std::collections::HashMap;

use crate::config::{RRF_K, RRF_SEMANTIC_WEIGHT};

/// Merge two ranked lists of `(doc, score)` (only the order matters here).
/// `rrf[id] = Σ 1/(k+i_bm25+1) + Σ w/(k+i_sem+1)`; the union of ids is
/// (BM25 results with score>0) ∪ (all semantically ranked docs). Ties keep
/// first-seen order (BM25 list first), like the web's insertion-ordered Set.
pub fn rrf_merge(
    bm25: &[(u32, f64)],
    semantic: &[(u32, f64)],
    k: f64,
    semantic_weight: f64,
) -> Vec<(u32, f64)> {
    let mut order: Vec<u32> = Vec::with_capacity(bm25.len() + semantic.len());
    let mut scores: HashMap<u32, f64> = HashMap::with_capacity(bm25.len() + semantic.len());
    for &(doc, _) in bm25.iter().chain(semantic.iter()) {
        scores.entry(doc).or_insert_with(|| {
            order.push(doc);
            0.0
        });
    }
    for (i, &(doc, _)) in bm25.iter().enumerate() {
        *scores.get_mut(&doc).unwrap() += 1.0 / (k + i as f64 + 1.0);
    }
    for (i, &(doc, _)) in semantic.iter().enumerate() {
        *scores.get_mut(&doc).unwrap() += semantic_weight / (k + i as f64 + 1.0);
    }
    let mut merged: Vec<(u32, f64)> = order.into_iter().map(|d| (d, scores[&d])).collect();
    merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    merged
}

/// The confirmed web defaults (§6).
pub fn rrf_merge_default(bm25: &[(u32, f64)], semantic: &[(u32, f64)]) -> Vec<(u32, f64)> {
    rrf_merge(bm25, semantic, RRF_K, RRF_SEMANTIC_WEIGHT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rrf_matches_hand_computed_fusion() {
        // bm25 ranks: A(0), B(1), C(2); semantic ranks: B(0), D(1)
        let bm25 = vec![(0, 9.0), (1, 5.0), (2, 1.0)];
        let semantic = vec![(1, 0.9), (3, 0.8)];
        let merged = rrf_merge(&bm25, &semantic, 60.0, 2.0);

        let get = |doc: u32| merged.iter().find(|(d, _)| *d == doc).unwrap().1;
        assert!((get(0) - 1.0 / 61.0).abs() < 1e-12);
        assert!((get(1) - (1.0 / 62.0 + 2.0 / 61.0)).abs() < 1e-12);
        assert!((get(2) - 1.0 / 63.0).abs() < 1e-12);
        assert!((get(3) - 2.0 / 62.0).abs() < 1e-12);

        // order: B > D > A > C
        let order: Vec<u32> = merged.iter().map(|(d, _)| *d).collect();
        assert_eq!(order, vec![1, 3, 0, 2]);
    }

    #[test]
    fn semantic_counts_double() {
        // same rank position, semantic-only doc must score 2x the bm25-only doc
        let merged = rrf_merge(&[(0, 1.0)], &[(1, 1.0)], 60.0, 2.0);
        let a = merged.iter().find(|(d, _)| *d == 0).unwrap().1;
        let b = merged.iter().find(|(d, _)| *d == 1).unwrap().1;
        assert!((b / a - 2.0).abs() < 1e-12);
        assert_eq!(merged[0].0, 1);
    }

    #[test]
    fn union_includes_docs_from_either_side() {
        let merged = rrf_merge(&[(7, 1.0)], &[(9, 1.0)], 60.0, 2.0);
        let docs: Vec<u32> = merged.iter().map(|(d, _)| *d).collect();
        assert!(docs.contains(&7) && docs.contains(&9));
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn empty_semantic_keeps_bm25_order() {
        let merged = rrf_merge(&[(2, 5.0), (0, 3.0), (1, 1.0)], &[], 60.0, 2.0);
        let docs: Vec<u32> = merged.iter().map(|(d, _)| *d).collect();
        assert_eq!(docs, vec![2, 0, 1]);
    }
}
