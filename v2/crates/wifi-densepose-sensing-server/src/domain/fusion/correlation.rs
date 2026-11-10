use std::collections::VecDeque;
use ruvector_mincut::{DynamicMinCut, MinCutBuilder};

/// Estimate person count via DynamicMinCut on the subcarrier temporal correlation graph.
///
/// Builds a graph where:
/// - Nodes = active subcarriers (variance > noise floor)
/// - Edges = Pearson correlation between subcarrier time series
/// - Source = virtual node connected to highest-variance subcarrier
/// - Sink = virtual node connected to lowest-variance subcarrier
pub fn estimate_persons_from_correlation(frame_history: &VecDeque<Vec<f64>>) -> usize {
    let n_frames = frame_history.len();
    if n_frames < 10 {
        return 1;
    }

    let window: Vec<&Vec<f64>> = frame_history.iter().rev().take(20).collect();
    let n_sub = window[0].len().min(56);
    if n_sub < 4 {
        return 1;
    }
    let k = window.len() as f64;

    let mut means = vec![0.0f64; n_sub];
    let mut variances = vec![0.0f64; n_sub];
    for frame in &window {
        for sc in 0..n_sub.min(frame.len()) {
            means[sc] += frame[sc] / k;
        }
    }
    for frame in &window {
        for sc in 0..n_sub.min(frame.len()) {
            variances[sc] += (frame[sc] - means[sc]).powi(2) / k;
        }
    }

    let noise_floor = 1.0;
    let active: Vec<usize> = (0..n_sub)
        .filter(|&sc| variances[sc] > noise_floor)
        .collect();
    let m = active.len();
    if m < 3 {
        return if m == 0 { 0 } else { 1 };
    }

    let mut edges: Vec<(u64, u64, f64)> = Vec::new();
    let source = m as u64;
    let sink = (m + 1) as u64;

    let stds: Vec<f64> = active
        .iter()
        .map(|&sc| variances[sc].sqrt().max(1e-9))
        .collect();

    for i in 0..m {
        for j in (i + 1)..m {
            let mut cov = 0.0f64;
            for frame in &window {
                let si = active[i];
                let sj = active[j];
                if si < frame.len() && sj < frame.len() {
                    cov += (frame[si] - means[si]) * (frame[sj] - means[sj]) / k;
                }
            }
            let corr = (cov / (stds[i] * stds[j])).abs();
            if corr > 0.1 {
                let weight = corr * 10.0;
                edges.push((i as u64, j as u64, weight));
                edges.push((j as u64, i as u64, weight));
            }
        }
    }

    let (max_var_idx, _) = active
        .iter()
        .enumerate()
        .max_by(|(_, &a), (_, &b)| {
            variances[a]
                .partial_cmp(&variances[b])
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or((0, &0));
    let (min_var_idx, _) = active
        .iter()
        .enumerate()
        .min_by(|(_, &a), (_, &b)| {
            variances[a]
                .partial_cmp(&variances[b])
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or((0, &0));

    if max_var_idx == min_var_idx {
        return 1;
    }

    edges.push((source, max_var_idx as u64, 100.0));
    edges.push((min_var_idx as u64, sink, 100.0));

    let mc: DynamicMinCut = match MinCutBuilder::new()
        .exact()
        .with_edges(edges.clone())
        .build()
    {
        Ok(mc) => mc,
        Err(_) => return 1,
    };

    let cut_value = mc.min_cut_value();
    let total_edge_weight: f64 = edges
        .iter()
        .filter(|(s, t, _)| *s != source && *s != sink && *t != source && *t != sink)
        .map(|(_, _, w)| w)
        .sum::<f64>()
        / 2.0;

    if total_edge_weight < 1e-9 {
        return 1;
    }

    let cut_ratio = cut_value / total_edge_weight;
    if cut_ratio > 0.4 {
        1
    } else if cut_ratio > 0.15 {
        2
    } else {
        3
    }
}

/// Map a DynamicMinCut occupancy estimate onto a target score for hysteresis discretization.
pub fn corr_persons_to_score(corr_persons: usize) -> f64 {
    match corr_persons {
        0 => 0.20,
        1 => 0.40,
        2 => 0.74,
        _ => 0.96,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::classification::score_to_person_count;

    fn converge(corr_persons: usize) -> usize {
        let mut score = 0.0f64;
        let mut count = 1usize;
        for _ in 0..400 {
            let target = corr_persons_to_score(corr_persons);
            score = score * 0.92 + target * 0.08;
            count = score_to_person_count(score, count);
        }
        count
    }

    #[test]
    fn sustained_one_person_estimate_reports_one() {
        assert_eq!(converge(1), 1);
    }

    #[test]
    fn sustained_two_person_estimate_reports_two() {
        assert_eq!(converge(2), 2);
    }

    #[test]
    fn sustained_three_person_estimate_reports_three() {
        assert_eq!(converge(3), 3);
    }
}

