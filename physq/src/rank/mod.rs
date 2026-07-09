//! RRF fusion of the (fully boosted, sorted) BM25 list with one or more
//! semantic rankings — a generalization of `search.html`
//! `_rrfMerge(bm25Results, semanticRanked, k = 60, semanticWeight = 2.0)`.
//! With a single semantic list it is byte-identical to the web; the `max`
//! ensemble mode (CLAUDE.md §6 accuracy deviation) passes two semantic lists.

use std::collections::HashMap;

use crate::config::{ModelSize, RRF_K, RRF_WEIGHT_BM25, RRF_WEIGHT_SMALL};

/// Merge N ranked lists of `(doc, score)`, each with its own RRF weight (only
/// each list's *order* matters). `rrf[id] = Σ_l w_l/(k + i_l + 1)`. The union
/// of ids is the first-seen order across `lists` in the given order (ties keep
/// that order), mirroring the web's insertion-ordered Set: pass the BM25 list
/// first, then the semantic list(s).
pub fn rrf_merge_weighted(lists: &[(&[(u32, f64)], f64)], k: f64) -> Vec<(u32, f64)> {
    let cap: usize = lists.iter().map(|(l, _)| l.len()).sum();
    let mut order: Vec<u32> = Vec::with_capacity(cap);
    let mut scores: HashMap<u32, f64> = HashMap::with_capacity(cap);
    for (list, _) in lists {
        for &(doc, _) in list.iter() {
            scores.entry(doc).or_insert_with(|| {
                order.push(doc);
                0.0
            });
        }
    }
    for (list, weight) in lists {
        for (i, &(doc, _)) in list.iter().enumerate() {
            *scores.get_mut(&doc).unwrap() += weight / (k + i as f64 + 1.0);
        }
    }
    let mut merged: Vec<(u32, f64)> = order.into_iter().map(|d| (d, scores[&d])).collect();
    merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    merged
}

/// Two-list convenience wrapper (BM25 weight 1, semantic weight
/// `semantic_weight`) — the original web `_rrfMerge` signature.
pub fn rrf_merge(
    bm25: &[(u32, f64)],
    semantic: &[(u32, f64)],
    k: f64,
    semantic_weight: f64,
) -> Vec<(u32, f64)> {
    rrf_merge_weighted(&[(bm25, 1.0), (semantic, semantic_weight)], k)
}

/// The confirmed web defaults (§6): single semantic list, k=60. Historically
/// small and large shared one weight (2.0); this anchors on `RRF_WEIGHT_SMALL`
/// as the representative value since both are still equal by default.
pub fn rrf_merge_default(bm25: &[(u32, f64)], semantic: &[(u32, f64)]) -> Vec<(u32, f64)> {
    rrf_merge(bm25, semantic, RRF_K, RRF_WEIGHT_SMALL)
}

/// Hybrid fusion for the CLI: BM25 (`RRF_WEIGHT_BM25`) plus **each** semantic
/// list at its own model's weight (`ModelSize::rrf_weight`), RRF `k = RRF_K`.
/// `semantics` and `sizes` are index-aligned (both in `ModelSel::sizes()`
/// order — see `SemanticEngine::rank`). One semantic list reproduces
/// `rrf_merge_default`; two lists (small, large) are the `max` ensemble.
/// Keeping each semantic list at the same weight it has in single mode means
/// small vs large is decided purely by their relative ranks — a hit both
/// models place 2nd–3rd outscores one a single model places 1st.
pub fn rrf_merge_hybrid(
    bm25: &[(u32, f64)],
    semantics: &[Vec<(u32, f64)>],
    sizes: &[ModelSize],
) -> Vec<(u32, f64)> {
    let mut lists: Vec<(&[(u32, f64)], f64)> = Vec::with_capacity(1 + semantics.len());
    lists.push((bm25, RRF_WEIGHT_BM25));
    for (s, size) in semantics.iter().zip(sizes) {
        lists.push((s.as_slice(), size.rrf_weight()));
    }
    rrf_merge_weighted(&lists, RRF_K)
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

    #[test]
    fn hybrid_single_list_matches_default() {
        // rrf_merge_hybrid with one semantic list == rrf_merge_default.
        let bm25 = vec![(0, 9.0), (1, 5.0), (2, 1.0)];
        let semantic = vec![(1, 0.9), (3, 0.8)];
        let a = rrf_merge_default(&bm25, &semantic);
        let b = rrf_merge_hybrid(&bm25, std::slice::from_ref(&semantic), &[ModelSize::Small]);
        assert_eq!(a, b);
    }

    #[test]
    fn zero_weight_list_contributes_nothing() {
        // The custom mode can zero out a model. A weight-0 list must not move
        // any ranking — identical to omitting it.
        let bm25 = vec![(0, 5.0), (1, 3.0)];
        let small = vec![(1, 0.9), (2, 0.8)];
        let large = vec![(9, 1.0)];
        let with_zero = rrf_merge_weighted(&[(&bm25, 1.0), (&small, 2.0), (&large, 0.0)], 60.0);
        let without = rrf_merge_weighted(&[(&bm25, 1.0), (&small, 2.0)], 60.0);
        // Every doc present in both must have an equal score; the zero-weight
        // doc (9) gets 0 and sorts last.
        for (id, score) in &without {
            let got = with_zero.iter().find(|(d, _)| d == id).unwrap().1;
            assert!((got - score).abs() < 1e-12, "doc {id}");
        }
        assert!((with_zero.iter().find(|(d, _)| *d == 9).unwrap().1).abs() < 1e-12);
    }

    #[test]
    fn max_ensemble_rewards_agreement_across_models() {
        // The `max` payoff: doc 5 is only 2nd in each model but is placed by
        // BOTH; doc 1 is 1st in small but far down in large. Agreement wins.
        let bm25: Vec<(u32, f64)> = vec![];
        let mut large = vec![(9, 1.0), (5, 0.9)]; // 5 is 2nd
        for d in 10..30 {
            large.push((d, 0.5)); // push doc 1 far down in large
        }
        large.push((1, 0.1));
        let small = vec![(1, 1.0), (5, 0.9)]; // 1 is 1st, 5 is 2nd
        let merged = rrf_merge_hybrid(
            &bm25,
            &[small, large],
            &[ModelSize::Small, ModelSize::Large],
        );
        let rank_of = |doc: u32| merged.iter().position(|(d, _)| *d == doc).unwrap();
        assert!(
            rank_of(5) < rank_of(1),
            "doc placed 2nd by both models should outrank one placed 1st by a single model"
        );
    }
}
