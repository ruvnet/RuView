//! Contribution Curve for Early Adopter Rewards
//!
//! Implements an exponential decay curve that rewards early network participants
//! with higher multipliers that decay as the network grows.
//!
//! ```text
//! Multiplier
//! 10x |*
//!     | *
//!  8x |  *
//!     |   *
//!  6x |    *
//!     |     *
//!  4x |      *
//!     |       **
//!  2x |         ***
//!     |            *****
//!  1x |                 ****************************
//!     +--+--+--+--+--+--+--+--+--+--+--+--+--+--+---> Network Compute (M hours)
//!     0  1  2  3  4  5  6  7  8  9  10
//! ```

use wasm_bindgen::prelude::*;

/// Contribution curve calculator for early adopter rewards
///
/// The multiplier follows an exponential decay formula:
/// ```text
/// multiplier = 1 + (MAX_BONUS - 1) * e^(-network_compute / DECAY_CONSTANT)
/// ```
///
/// This ensures:
/// - Genesis contributors (0 compute) get MAX_BONUS (10x)
/// - At DECAY_CONSTANT compute hours, bonus is ~37% remaining (~4.3x)
/// - At very high compute, approaches baseline (1x)
/// - Never goes below 1x
pub struct ContributionCurve;

impl ContributionCurve {
    /// Maximum multiplier for genesis contributors
    pub const MAX_BONUS: f32 = 10.0;

    /// Decay constant in CPU-hours (half-life of bonus decay)
    pub const DECAY_CONSTANT: f64 = 1_000_000.0;

    /// Calculate current multiplier based on total network compute
    ///
    /// # Arguments
    /// * `network_compute_hours` - Total CPU-hours contributed to the network
    ///
    /// # Returns
    /// A multiplier between 1.0 (baseline) and MAX_BONUS (genesis)
    ///
    /// # Example
    /// ```
    /// use ruvector_economy_wasm::ContributionCurve;
    ///
    /// // Genesis: 10x multiplier
    /// assert!((ContributionCurve::current_multiplier(0.0) - 10.0).abs() < 0.01);
    ///
    /// // At 1M hours: ~4.3x multiplier
    /// let mult = ContributionCurve::current_multiplier(1_000_000.0);
    /// assert!(mult > 4.0 && mult < 4.5);
    ///
    /// // At 10M hours: ~1.0x multiplier
    /// let mult = ContributionCurve::current_multiplier(10_000_000.0);
    /// assert!(mult < 1.1);
    /// ```
    pub fn current_multiplier(network_compute_hours: f64) -> f32 {
        let decay = (-network_compute_hours / Self::DECAY_CONSTANT).exp();
        1.0 + (Self::MAX_BONUS - 1.0) * decay as f32
    }

    /// Calculate reward with multiplier applied
    ///
    /// # Arguments
    /// * `base_reward` - Base reward amount before multiplier
    /// * `network_compute_hours` - Total network compute for multiplier calculation
    ///
    /// # Returns
    /// The reward amount with multiplier applied
    pub fn calculate_reward(base_reward: u64, network_compute_hours: f64) -> u64 {
        let multiplier = Self::current_multiplier(network_compute_hours);
        (base_reward as f32 * multiplier) as u64
    }

    /// Get multiplier tier information for UI display
    ///
    /// Returns a vector of (compute_hours, multiplier) tuples representing
    /// key milestones in the contribution curve.
    pub fn get_tiers() -> Vec<(f64, f32)> {
        vec![
            (0.0, 10.0),
            (100_000.0, 9.1),
            (500_000.0, 6.1),
            (1_000_000.0, 4.3),
            (2_000_000.0, 2.6),
            (5_000_000.0, 1.4),
            (10_000_000.0, 1.0),
        ]
    }

    /// Get the tier name based on network compute level
    pub fn get_tier_name(network_compute_hours: f64) -> &'static str {
        if network_compute_hours < 100_000.0 {
            "Genesis"
        } else if network_compute_hours < 500_000.0 {
            "Pioneer"
        } else if network_compute_hours < 1_000_000.0 {
            "Early Adopter"
        } else if network_compute_hours < 5_000_000.0 {
            "Established"
        } else {
            "Baseline"
        }
    }

    /// Calculate time remaining until next tier
    ///
    /// # Arguments
    /// * `current_compute` - Current network compute hours
    /// * `hourly_growth` - Estimated hourly compute growth rate
    ///
    /// # Returns
    /// Hours until next tier boundary, or None if at baseline
    pub fn hours_until_next_tier(current_compute: f64, hourly_growth: f64) -> Option<f64> {
        if hourly_growth <= 0.0 {
            return None;
        }

        let tiers = Self::get_tiers();
        for (threshold, _) in &tiers {
            if current_compute < *threshold {
                return Some((*threshold - current_compute) / hourly_growth);
            }
        }

        None // Already at baseline
    }
}

/// Calculate contribution multiplier (WASM export)
///
/// Returns the reward multiplier based on total network compute hours.
/// Early adopters get up to 10x rewards, decaying to 1x as network grows.
#[wasm_bindgen]
pub fn contribution_multiplier(network_compute_hours: f64) -> f32 {
    ContributionCurve::current_multiplier(network_compute_hours)
}

/// Calculate reward with multiplier (WASM export)
#[wasm_bindgen]
pub fn calculate_reward(base_reward: u64, network_compute_hours: f64) -> u64 {
    ContributionCurve::calculate_reward(base_reward, network_compute_hours)
}

/// Get tier name based on compute level (WASM export)
#[wasm_bindgen]
pub fn get_tier_name(network_compute_hours: f64) -> String {
    ContributionCurve::get_tier_name(network_compute_hours).to_string()
}

/// Get tier information as JSON (WASM export)
#[wasm_bindgen]
pub fn get_tiers_json() -> String {
    let tiers = ContributionCurve::get_tiers();
    let tier_objs: Vec<_> = tiers
        .iter()
        .map(|(hours, mult)| format!(r#"{{"hours":{},"multiplier":{:.1}}}"#, hours, mult))
        .collect();

    format!("[{}]", tier_objs.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_multiplier() {
        let mult = ContributionCurve::current_multiplier(0.0);
        assert!(
            (mult - 10.0).abs() < 0.01,
            "Genesis should give 10x, got {}",
            mult
        );
    }

    #[test]
    fn test_decay_constant_multiplier() {
        // At decay constant, e^(-1) ~= 0.368
        // So multiplier = 1 + 9 * 0.368 = 4.31
        let mult = ContributionCurve::current_multiplier(1_000_000.0);
        assert!(
            mult > 4.0 && mult < 4.5,
            "At decay constant should be ~4.3x, got {}",
            mult
        );
    }

    #[test]
    fn test_high_compute_baseline() {
        let mult = ContributionCurve::current_multiplier(10_000_000.0);
        assert!(mult < 1.1, "High compute should approach 1x, got {}", mult);
    }

    #[test]
    fn test_multiplier_never_below_one() {
        let mult = ContributionCurve::current_multiplier(100_000_000.0);
        assert!(
            mult >= 1.0,
            "Multiplier should never go below 1, got {}",
            mult
        );
    }

    #[test]
    fn test_calculate_reward() {
        let base = 100;
        let reward = ContributionCurve::calculate_reward(base, 0.0);
        assert_eq!(
            reward, 1000,
            "Genesis 100 base should give 1000, got {}",
            reward
        );
    }

    #[test]
    fn test_tier_names() {
        assert_eq!(ContributionCurve::get_tier_name(0.0), "Genesis");
        assert_eq!(ContributionCurve::get_tier_name(100_000.0), "Pioneer");
        assert_eq!(ContributionCurve::get_tier_name(500_000.0), "Early Adopter");
        assert_eq!(ContributionCurve::get_tier_name(1_000_000.0), "Established");
        assert_eq!(ContributionCurve::get_tier_name(10_000_000.0), "Baseline");
    }

    #[test]
    fn test_wasm_export_functions() {
        assert!((contribution_multiplier(0.0) - 10.0).abs() < 0.01);
        assert_eq!(calculate_reward(100, 0.0), 1000);
        assert_eq!(get_tier_name(0.0), "Genesis");
        assert!(get_tiers_json().contains("Genesis") == false); // JSON format
        assert!(get_tiers_json().starts_with("["));
    }
}
