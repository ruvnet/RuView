/// Temporal hysteresis tracker for stable gating decisions.
/// An edge only flips after the new decision is consistent for `tau` consecutive steps.
#[derive(Debug, Clone)]
pub struct HysteresisTracker {
    prev_mask: Option<Vec<bool>>,
    counts: Vec<usize>,
    tau: usize,
    step: usize,
}

impl HysteresisTracker {
    pub fn new(tau: usize) -> Self {
        Self {
            prev_mask: None,
            counts: Vec::new(),
            tau,
            step: 0,
        }
    }

    /// Apply hysteresis to a raw gating mask, returning the stabilised mask.
    pub fn apply(&mut self, raw: &[bool]) -> Vec<bool> {
        self.step += 1;
        let stable = match &self.prev_mask {
            None => {
                self.counts = vec![0; raw.len()];
                self.prev_mask = Some(raw.to_vec());
                return raw.to_vec();
            }
            Some(p) => p.clone(),
        };
        if self.counts.len() != raw.len() {
            self.counts = vec![0; raw.len()];
            self.prev_mask = Some(raw.to_vec());
            return raw.to_vec();
        }
        let mut result = stable.clone();
        for i in 0..raw.len() {
            if raw[i] != stable[i] {
                self.counts[i] += 1;
                if self.counts[i] >= self.tau {
                    result[i] = raw[i];
                    self.counts[i] = 0;
                }
            } else {
                self.counts[i] = 0;
            }
        }
        self.prev_mask = Some(result.clone());
        result
    }

    pub fn step(&self) -> usize {
        self.step
    }
    pub fn current_mask(&self) -> Option<&[bool]> {
        self.prev_mask.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_step_passthrough() {
        let mut t = HysteresisTracker::new(3);
        assert_eq!(t.apply(&[true, false, true]), vec![true, false, true]);
    }

    #[test]
    fn test_no_flip_before_tau() {
        let mut t = HysteresisTracker::new(3);
        let init = vec![true, true, false];
        t.apply(&init);
        let changed = vec![false, true, true];
        assert_eq!(t.apply(&changed), init);
        assert_eq!(t.apply(&changed), init);
    }

    #[test]
    fn test_flip_at_tau() {
        let mut t = HysteresisTracker::new(2);
        t.apply(&[true, false]);
        let c = vec![false, true];
        t.apply(&c);
        assert_eq!(t.apply(&c), c);
    }

    #[test]
    fn test_counter_reset_on_agreement() {
        let mut t = HysteresisTracker::new(3);
        t.apply(&[true]);
        t.apply(&[false]); // count=1
        t.apply(&[true]); // reset
        t.apply(&[false]); // count=1
        assert_eq!(t.apply(&[false]), vec![true]); // count=2 < 3
    }
}
