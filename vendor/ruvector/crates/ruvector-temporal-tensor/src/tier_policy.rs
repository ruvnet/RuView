//! Tier policy for access-pattern-driven bit-width selection.
//!
//! Score = `access_count * 1024 / (now_ts - last_access_ts + 1)`
//!
//! | Tier | Condition | Bits |
//! |------|-----------|------|
//! | Hot  | score >= hot_min_score | 8 |
//! | Warm | score >= warm_min_score | warm_bits (7 or 5) |
//! | Cold | otherwise | 3 |

#[derive(Clone, Copy, Debug)]
pub struct TierPolicy {
    pub hot_min_score: u32,
    pub warm_min_score: u32,
    pub warm_bits: u8,
    /// Drift tolerance as Q8 fixed-point. 26 means ~10.2% (26/256).
    pub drift_pct_q8: u32,
    pub group_len: u32,
}

impl Default for TierPolicy {
    fn default() -> Self {
        Self {
            hot_min_score: 512,
            warm_min_score: 64,
            warm_bits: 7,
            drift_pct_q8: 26,
            group_len: 64,
        }
    }
}

impl TierPolicy {
    /// Select bit width based on access pattern.
    pub fn select_bits(&self, access_count: u32, last_access_ts: u32, now_ts: u32) -> u8 {
        let age = now_ts.wrapping_sub(last_access_ts).wrapping_add(1);
        let score = access_count.saturating_mul(1024).wrapping_div(age);

        if score >= self.hot_min_score {
            8
        } else if score >= self.warm_min_score {
            self.warm_bits
        } else {
            3
        }
    }

    /// Compute the drift factor as 1.0 + drift_pct_q8/256.
    pub fn drift_factor(&self) -> f32 {
        1.0 + (self.drift_pct_q8 as f32) / 256.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy() {
        let p = TierPolicy::default();
        assert_eq!(p.hot_min_score, 512);
        assert_eq!(p.warm_min_score, 64);
        assert_eq!(p.warm_bits, 7);
        assert_eq!(p.drift_pct_q8, 26);
        assert_eq!(p.group_len, 64);
    }

    #[test]
    fn test_tier_selection_hot() {
        let p = TierPolicy::default();
        // 100 accesses, age=10 -> score = 100*1024/10 = 10240 >= 512
        assert_eq!(p.select_bits(100, 0, 9), 8);
    }

    #[test]
    fn test_tier_selection_warm() {
        let p = TierPolicy::default();
        // 10 accesses, age=100 -> score = 10*1024/100 = 102 >= 64, < 512
        assert_eq!(p.select_bits(10, 0, 99), 7);
    }

    #[test]
    fn test_tier_selection_cold() {
        let p = TierPolicy::default();
        // 1 access, age=1000 -> score = 1024/1000 = 1 < 64
        assert_eq!(p.select_bits(1, 0, 999), 3);
    }

    #[test]
    fn test_drift_factor() {
        let p = TierPolicy::default();
        let df = p.drift_factor();
        assert!((df - 1.1015625).abs() < 1e-6);
    }

    #[test]
    fn test_warm_bits_5() {
        let p = TierPolicy {
            warm_bits: 5,
            ..Default::default()
        };
        assert_eq!(p.select_bits(10, 0, 99), 5);
    }
}
