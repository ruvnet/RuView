//! Cross-domain evaluation metrics (MERIDIAN Phase 6).
//!
//! MPJPE, domain gap ratio, and adaptation speedup for measuring how well a
//! WiFi-DensePose model generalizes across environments and hardware.

use std::collections::HashMap;

/// Aggregated cross-domain evaluation metrics.
#[derive(Debug, Clone)]
pub struct CrossDomainMetrics {
    /// In-domain (source) MPJPE (mm).
    pub in_domain_mpjpe: f32,
    /// Cross-domain (unseen environment) MPJPE (mm).
    pub cross_domain_mpjpe: f32,
    /// MPJPE after few-shot adaptation (mm).
    pub few_shot_mpjpe: f32,
    /// MPJPE across different WiFi hardware (mm).
    pub cross_hardware_mpjpe: f32,
    /// cross-domain / in-domain MPJPE. Target: < 1.5.
    pub domain_gap_ratio: f32,
    /// Labelled-sample savings vs training from scratch.
    pub adaptation_speedup: f32,
}

/// Evaluates pose estimation across multiple domains.
///
/// Domain 0 = in-domain (source); other IDs = cross-domain.
///
/// ```rust
/// use wifi_densepose_train::eval::{CrossDomainEvaluator, mpjpe};
/// let ev = CrossDomainEvaluator::new(17);
/// let preds = vec![(vec![0.0_f32; 51], vec![0.0_f32; 51])];
/// let m = ev.evaluate(&preds, &[0]);
/// assert!(m.in_domain_mpjpe >= 0.0);
/// ```
pub struct CrossDomainEvaluator {
    n_joints: usize,
}

impl CrossDomainEvaluator {
    /// Create evaluator for `n_joints` body joints (e.g. 17 for COCO).
    pub fn new(n_joints: usize) -> Self { Self { n_joints } }

    /// Evaluate predictions grouped by domain. Each pair is (predicted, gt)
    /// with `n_joints * 3` floats. `domain_labels` must match length.
    pub fn evaluate(&self, predictions: &[(Vec<f32>, Vec<f32>)], domain_labels: &[u32]) -> CrossDomainMetrics {
        assert_eq!(predictions.len(), domain_labels.len(), "length mismatch");
        let mut by_dom: HashMap<u32, Vec<f32>> = HashMap::new();
        for (i, (p, g)) in predictions.iter().enumerate() {
            by_dom.entry(domain_labels[i]).or_default().push(mpjpe(p, g, self.n_joints));
        }
        let in_dom = mean_of(by_dom.get(&0));
        let cross_errs: Vec<f32> = by_dom.iter().filter(|(&d, _)| d != 0).flat_map(|(_, e)| e.iter().copied()).collect();
        let cross_dom = if cross_errs.is_empty() { 0.0 } else { cross_errs.iter().sum::<f32>() / cross_errs.len() as f32 };
        let few_shot = if by_dom.contains_key(&2) { mean_of(by_dom.get(&2)) } else { (in_dom + cross_dom) / 2.0 };
        let cross_hw = if by_dom.contains_key(&3) { mean_of(by_dom.get(&3)) } else { cross_dom };
        let gap = if in_dom > 1e-10 { cross_dom / in_dom } else if cross_dom > 1e-10 { f32::INFINITY } else { 1.0 };
        let speedup = if few_shot > 1e-10 { cross_dom / few_shot } else { 1.0 };
        CrossDomainMetrics { in_domain_mpjpe: in_dom, cross_domain_mpjpe: cross_dom, few_shot_mpjpe: few_shot,
            cross_hardware_mpjpe: cross_hw, domain_gap_ratio: gap, adaptation_speedup: speedup }
    }
}

/// Mean Per Joint Position Error: average Euclidean distance across `n_joints`.
///
/// `pred` and `gt` are flat `[n_joints * 3]` (x, y, z per joint).
pub fn mpjpe(pred: &[f32], gt: &[f32], n_joints: usize) -> f32 {
    if n_joints == 0 { return 0.0; }
    let total: f32 = (0..n_joints).map(|j| {
        let b = j * 3;
        let d = |off| pred.get(b + off).copied().unwrap_or(0.0) - gt.get(b + off).copied().unwrap_or(0.0);
        (d(0).powi(2) + d(1).powi(2) + d(2).powi(2)).sqrt()
    }).sum();
    total / n_joints as f32
}

fn mean_of(v: Option<&Vec<f32>>) -> f32 {
    match v { Some(e) if !e.is_empty() => e.iter().sum::<f32>() / e.len() as f32, _ => 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mpjpe_known_value() {
        assert!((mpjpe(&[0.0, 0.0, 0.0], &[3.0, 4.0, 0.0], 1) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn mpjpe_two_joints() {
        // Joint 0: dist=5, Joint 1: dist=0 -> mean=2.5
        assert!((mpjpe(&[0.0,0.0,0.0, 1.0,1.0,1.0], &[3.0,4.0,0.0, 1.0,1.0,1.0], 2) - 2.5).abs() < 1e-6);
    }

    #[test]
    fn mpjpe_zero_when_identical() {
        let c = vec![1.5, 2.3, 0.7, 4.1, 5.9, 3.2];
        assert!(mpjpe(&c, &c, 2).abs() < 1e-10);
    }

    #[test]
    fn mpjpe_zero_joints() { assert_eq!(mpjpe(&[], &[], 0), 0.0); }

    #[test]
    fn domain_gap_ratio_computed() {
        let ev = CrossDomainEvaluator::new(1);
        let preds = vec![
            (vec![0.0,0.0,0.0], vec![1.0,0.0,0.0]), // dom 0, err=1
            (vec![0.0,0.0,0.0], vec![2.0,0.0,0.0]), // dom 1, err=2
        ];
        let m = ev.evaluate(&preds, &[0, 1]);
        assert!((m.in_domain_mpjpe - 1.0).abs() < 1e-6);
        assert!((m.cross_domain_mpjpe - 2.0).abs() < 1e-6);
        assert!((m.domain_gap_ratio - 2.0).abs() < 1e-6);
    }

    #[test]
    fn evaluate_groups_by_domain() {
        let ev = CrossDomainEvaluator::new(1);
        let preds = vec![
            (vec![0.0,0.0,0.0], vec![1.0,0.0,0.0]),
            (vec![0.0,0.0,0.0], vec![3.0,0.0,0.0]),
            (vec![0.0,0.0,0.0], vec![5.0,0.0,0.0]),
        ];
        let m = ev.evaluate(&preds, &[0, 0, 1]);
        assert!((m.in_domain_mpjpe - 2.0).abs() < 1e-6);
        assert!((m.cross_domain_mpjpe - 5.0).abs() < 1e-6);
    }

    #[test]
    fn domain_gap_perfect() {
        let ev = CrossDomainEvaluator::new(1);
        let preds = vec![(vec![1.0,2.0,3.0], vec![1.0,2.0,3.0]), (vec![4.0,5.0,6.0], vec![4.0,5.0,6.0])];
        assert!((ev.evaluate(&preds, &[0, 1]).domain_gap_ratio - 1.0).abs() < 1e-6);
    }

    #[test]
    fn evaluate_multiple_cross_domains() {
        let ev = CrossDomainEvaluator::new(1);
        let preds = vec![
            (vec![0.0,0.0,0.0], vec![1.0,0.0,0.0]),
            (vec![0.0,0.0,0.0], vec![4.0,0.0,0.0]),
            (vec![0.0,0.0,0.0], vec![6.0,0.0,0.0]),
        ];
        let m = ev.evaluate(&preds, &[0, 1, 3]);
        assert!((m.in_domain_mpjpe - 1.0).abs() < 1e-6);
        assert!((m.cross_domain_mpjpe - 5.0).abs() < 1e-6);
        assert!((m.cross_hardware_mpjpe - 6.0).abs() < 1e-6);
    }
}
