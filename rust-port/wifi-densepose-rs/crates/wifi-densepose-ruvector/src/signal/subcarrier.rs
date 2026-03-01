//! Subcarrier partitioning via graph min-cut (ruvector-mincut).
//!
//! Uses [`MinCutBuilder`] to partition subcarriers into two groups —
//! **sensitive** (high body-motion correlation) and **insensitive** (dominated
//! by static multipath or noise) — based on pairwise sensitivity similarity.
//!
//! The edge weight between subcarriers `i` and `j` is the inverse absolute
//! difference of their sensitivity scores; highly similar subcarriers have a
//! heavy edge, making the min-cut prefer to separate dissimilar ones.
//!
//! A virtual source (node `n`) and sink (node `n+1`) are added to make the
//! graph connected and enable the min-cut to naturally bifurcate the
//! subcarrier set. The cut edges that cross from the source-side to the
//! sink-side identify the two partitions.

use ruvector_mincut::{DynamicMinCut, MinCutBuilder};

/// Partition `sensitivity` scores into (sensitive_indices, insensitive_indices)
/// using graph min-cut. The group with higher mean sensitivity is "sensitive".
///
/// # Arguments
///
/// - `sensitivity`: per-subcarrier sensitivity score, one value per subcarrier.
///   Higher values indicate stronger body-motion correlation.
///
/// # Returns
///
/// A tuple `(sensitive, insensitive)` where each element is a `Vec<usize>` of
/// subcarrier indices belonging to that partition. Together they cover all
/// indices `0..sensitivity.len()`.
///
/// # Notes
///
/// When `sensitivity` is empty or all edges would be below threshold the
/// function falls back to a simple midpoint split.
pub fn mincut_subcarrier_partition(sensitivity: &[f32]) -> (Vec<usize>, Vec<usize>) {
    let n = sensitivity.len();
    if n == 0 {
        return (Vec::new(), Vec::new());
    }
    if n == 1 {
        return (vec![0], Vec::new());
    }

    // Build edges as a flow network:
    // - Nodes 0..n-1 are subcarrier nodes
    // - Node n is the virtual source (connected to high-sensitivity nodes)
    // - Node n+1 is the virtual sink (connected to low-sensitivity nodes)
    let source = n as u64;
    let sink = (n + 1) as u64;

    let mean_sens: f32 = sensitivity.iter().sum::<f32>() / n as f32;

    let mut edges: Vec<(u64, u64, f64)> = Vec::new();

    // Source connects to subcarriers with above-average sensitivity.
    // Sink connects to subcarriers with below-average sensitivity.
    for i in 0..n {
        let cap = (sensitivity[i] as f64).abs() + 1e-6;
        if sensitivity[i] >= mean_sens {
            edges.push((source, i as u64, cap));
        } else {
            edges.push((i as u64, sink, cap));
        }
    }

    // Subcarrier-to-subcarrier edges weighted by inverse sensitivity difference.
    let threshold = 0.1_f64;
    for i in 0..n {
        for j in (i + 1)..n {
            let diff = (sensitivity[i] - sensitivity[j]).abs() as f64;
            let weight = if diff > 1e-9 { 1.0 / diff } else { 1e6_f64 };
            if weight > threshold {
                edges.push((i as u64, j as u64, weight));
                edges.push((j as u64, i as u64, weight));
            }
        }
    }

    let mc: DynamicMinCut = match MinCutBuilder::new().exact().with_edges(edges).build() {
        Ok(mc) => mc,
        Err(_) => {
            // Fallback: midpoint split on builder error.
            let mid = n / 2;
            return ((0..mid).collect(), (mid..n).collect());
        }
    };

    // Use cut_edges to identify which side each node belongs to.
    // Nodes reachable from source in the residual graph are "source-side",
    // the rest are "sink-side".
    let cut = mc.cut_edges();

    // Collect nodes that appear on the source side of a cut edge (u nodes).
    let mut source_side: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut sink_side: std::collections::HashSet<u64> = std::collections::HashSet::new();

    for edge in &cut {
        // Cut edge goes from source-side node to sink-side node.
        if edge.source != source && edge.source != sink {
            source_side.insert(edge.source);
        }
        if edge.target != source && edge.target != sink {
            sink_side.insert(edge.target);
        }
    }

    // Any subcarrier not explicitly classified goes to whichever side is smaller.
    let mut side_a: Vec<usize> = source_side.iter().map(|&x| x as usize).collect();
    let mut side_b: Vec<usize> = sink_side.iter().map(|&x| x as usize).collect();

    // Assign unclassified nodes.
    for i in 0..n {
        if !source_side.contains(&(i as u64)) && !sink_side.contains(&(i as u64)) {
            if side_a.len() <= side_b.len() {
                side_a.push(i);
            } else {
                side_b.push(i);
            }
        }
    }

    // If one side is empty (no cut edges), fall back to midpoint split.
    if side_a.is_empty() || side_b.is_empty() {
        let mid = n / 2;
        side_a = (0..mid).collect();
        side_b = (mid..n).collect();
    }

    // The group with higher mean sensitivity becomes the "sensitive" group.
    let mean_of = |indices: &[usize]| -> f32 {
        if indices.is_empty() {
            return 0.0;
        }
        indices.iter().map(|&i| sensitivity[i]).sum::<f32>() / indices.len() as f32
    };

    if mean_of(&side_a) >= mean_of(&side_b) {
        (side_a, side_b)
    } else {
        (side_b, side_a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_covers_all_indices() {
        let sensitivity: Vec<f32> = (0..10).map(|i| i as f32 * 0.1).collect();
        let (sensitive, insensitive) = mincut_subcarrier_partition(&sensitivity);

        // Both groups must be non-empty for a non-trivial input.
        assert!(!sensitive.is_empty(), "sensitive group must not be empty");
        assert!(!insensitive.is_empty(), "insensitive group must not be empty");

        // Together they must cover every index exactly once.
        let mut all_indices: Vec<usize> = sensitive.iter().chain(insensitive.iter()).cloned().collect();
        all_indices.sort_unstable();
        let expected: Vec<usize> = (0..10).collect();
        assert_eq!(all_indices, expected, "partition must cover all 10 indices");
    }

    #[test]
    fn partition_empty_input() {
        let (s, i) = mincut_subcarrier_partition(&[]);
        assert!(s.is_empty());
        assert!(i.is_empty());
    }

    #[test]
    fn partition_single_element() {
        let (s, i) = mincut_subcarrier_partition(&[0.5]);
        assert_eq!(s, vec![0]);
        assert!(i.is_empty());
    }
}
