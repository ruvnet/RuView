//! Side-by-side comparison utilities for attention masks.

use serde::{Deserialize, Serialize};

/// Result of comparing two attention masks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub jaccard: f64,
    pub edge_flips: usize,
    pub baseline_edges: usize,
    pub gated_edges: usize,
    pub sparsity_ratio: f64,
}

/// Jaccard similarity: `|A & B| / |A | B|`. Returns `1.0` for two empty masks.
pub fn jaccard_similarity(mask_a: &[bool], mask_b: &[bool]) -> f64 {
    let n = mask_a.len().min(mask_b.len());
    let (mut inter, mut union) = (0usize, 0usize);
    for i in 0..n {
        if mask_a[i] || mask_b[i] {
            union += 1;
        }
        if mask_a[i] && mask_b[i] {
            inter += 1;
        }
    }
    union += count_true_tail(mask_a, n) + count_true_tail(mask_b, n);
    if union == 0 {
        1.0
    } else {
        inter as f64 / union as f64
    }
}

/// Counts positions where the two masks disagree.
pub fn edge_flip_count(mask_a: &[bool], mask_b: &[bool]) -> usize {
    let n = mask_a.len().min(mask_b.len());
    let mut flips = (0..n).filter(|&i| mask_a[i] != mask_b[i]).count();
    flips += count_true_tail(mask_a, n) + count_true_tail(mask_b, n);
    flips
}

/// Full comparison of two attention masks.
pub fn compare_attention_masks(baseline: &[bool], gated: &[bool]) -> ComparisonResult {
    let baseline_edges = baseline.iter().filter(|&&v| v).count();
    let gated_edges = gated.iter().filter(|&&v| v).count();
    let total = baseline.len().max(gated.len());
    let bl_sp = if total > 0 {
        1.0 - baseline_edges as f64 / total as f64
    } else {
        1.0
    };
    let gt_sp = if total > 0 {
        1.0 - gated_edges as f64 / total as f64
    } else {
        1.0
    };
    ComparisonResult {
        jaccard: jaccard_similarity(baseline, gated),
        edge_flips: edge_flip_count(baseline, gated),
        baseline_edges,
        gated_edges,
        sparsity_ratio: if bl_sp > f64::EPSILON {
            gt_sp / bl_sp
        } else {
            gt_sp
        },
    }
}

fn count_true_tail(mask: &[bool], from: usize) -> usize {
    if mask.len() > from {
        mask[from..].iter().filter(|&&v| v).count()
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jaccard_cases() {
        let m = vec![true, false, true, true];
        assert!((jaccard_similarity(&m, &m) - 1.0).abs() < 1e-10);
        assert!(jaccard_similarity(&[true, false], &[false, true]).abs() < 1e-10);
        assert_eq!(jaccard_similarity(&[], &[]), 1.0);
        // partial: intersection=1, union=3
        let (a, b) = (
            vec![true, true, false, false],
            vec![true, false, true, false],
        );
        assert!((jaccard_similarity(&a, &b) - 1.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn edge_flip_cases() {
        assert_eq!(edge_flip_count(&[true, false], &[true, false]), 0);
        assert_eq!(
            edge_flip_count(&[true, false, true], &[false, true, false]),
            3
        );
        assert_eq!(
            edge_flip_count(&[true, false], &[true, false, true, true]),
            2
        );
    }

    #[test]
    fn compare_masks() {
        let bl = vec![true, true, false, false, true];
        let gt = vec![true, false, false, true, true];
        let r = compare_attention_masks(&bl, &gt);
        assert_eq!(r.baseline_edges, 3);
        assert_eq!(r.gated_edges, 3);
        assert_eq!(r.edge_flips, 2);
        assert!((r.jaccard - 0.5).abs() < 1e-10);
    }
}
