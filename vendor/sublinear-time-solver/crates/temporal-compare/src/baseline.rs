use crate::data::{Sample, to_class};

/// Naive baseline: next = last_in_window; class from that
pub struct Baseline;

impl Baseline {
    pub fn predict_reg(samples: &[Sample], window: usize) -> Vec<f32> {
        samples.iter().map(|s| s.x[window-1]).collect()
    }

    pub fn predict_cls(samples: &[Sample], window: usize) -> Vec<usize> {
        samples.iter().map(|s| {
            let yhat = s.x[window-1];
            to_class(yhat)
        }).collect()
    }
}