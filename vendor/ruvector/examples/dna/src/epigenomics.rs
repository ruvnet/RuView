//! Epigenomics analysis module
//!
//! Provides methylation profiling and epigenetic age prediction
//! using the Horvath clock model.

use serde::{Deserialize, Serialize};

/// A CpG site with methylation data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpGSite {
    /// Chromosome number
    pub chromosome: u8,
    /// Genomic position
    pub position: u64,
    /// Methylation level (beta value, 0.0 to 1.0)
    pub methylation_level: f32,
}

/// Methylation profile containing CpG site measurements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethylationProfile {
    /// CpG sites with measured methylation levels
    pub sites: Vec<CpGSite>,
}

impl MethylationProfile {
    /// Create a methylation profile from position and beta value arrays
    pub fn from_beta_values(positions: Vec<(u8, u64)>, betas: Vec<f32>) -> Self {
        let sites = positions
            .into_iter()
            .zip(betas.into_iter())
            .map(|((chr, pos), beta)| CpGSite {
                chromosome: chr,
                position: pos,
                methylation_level: beta.clamp(0.0, 1.0),
            })
            .collect();

        Self { sites }
    }

    /// Calculate mean methylation across all sites
    pub fn mean_methylation(&self) -> f32 {
        if self.sites.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.sites.iter().map(|s| s.methylation_level).sum();
        sum / self.sites.len() as f32
    }

    /// Calculate methylation entropy (Shannon entropy of beta values)
    ///
    /// High entropy indicates heterogeneous methylation (potential tumor heterogeneity)
    pub fn methylation_entropy(&self) -> f64 {
        if self.sites.is_empty() {
            return 0.0;
        }

        // Bin methylation into 10 bins [0, 0.1), [0.1, 0.2), ..., [0.9, 1.0]
        let mut bins = [0u32; 10];
        for site in &self.sites {
            let bin = ((site.methylation_level * 10.0) as usize).min(9);
            bins[bin] += 1;
        }

        let n = self.sites.len() as f64;
        let mut entropy = 0.0;
        for &count in &bins {
            if count > 0 {
                let p = count as f64 / n;
                entropy -= p * p.ln();
            }
        }

        entropy
    }

    /// Calculate extreme methylation ratio
    ///
    /// Fraction of sites with beta < 0.1 (hypomethylated) or > 0.9 (hypermethylated).
    /// High ratio indicates global methylation disruption (cancer hallmark).
    pub fn extreme_methylation_ratio(&self) -> f32 {
        if self.sites.is_empty() {
            return 0.0;
        }
        let extreme_count = self
            .sites
            .iter()
            .filter(|s| s.methylation_level < 0.1 || s.methylation_level > 0.9)
            .count();
        extreme_count as f32 / self.sites.len() as f32
    }
}

/// Horvath epigenetic clock for biological age prediction
///
/// Uses a simplified linear model based on CpG site methylation levels
/// to predict biological age.
pub struct HorvathClock {
    /// Intercept term
    intercept: f64,
    /// Coefficient per CpG site bin
    coefficients: Vec<f64>,
    /// Number of bins to partition sites into
    num_bins: usize,
}

impl HorvathClock {
    /// Create the default Horvath clock model
    ///
    /// Uses a simplified model with binned methylation values.
    /// Real implementation would use 353 specific CpG sites.
    pub fn default_clock() -> Self {
        Self {
            intercept: 30.0,
            coefficients: vec![
                -15.0, // Low methylation bin (young)
                10.0,  // High methylation bin (age-associated)
                0.5,   // Neutral bin
            ],
            num_bins: 3,
        }
    }

    /// Predict biological age from a methylation profile
    pub fn predict_age(&self, profile: &MethylationProfile) -> f64 {
        if profile.sites.is_empty() {
            return self.intercept;
        }

        // Partition sites into bins and compute mean methylation per bin
        let bin_size = profile.sites.len() / self.num_bins.max(1);
        let mut age = self.intercept;

        for (bin_idx, coefficient) in self.coefficients.iter().enumerate() {
            let start = bin_idx * bin_size;
            let end = ((bin_idx + 1) * bin_size).min(profile.sites.len());

            if start >= profile.sites.len() {
                break;
            }

            let bin_sites = &profile.sites[start..end];
            if !bin_sites.is_empty() {
                let mean_meth: f64 = bin_sites
                    .iter()
                    .map(|s| s.methylation_level as f64)
                    .sum::<f64>()
                    / bin_sites.len() as f64;

                age += coefficient * mean_meth;
            }
        }

        age.max(0.0)
    }

    /// Calculate age acceleration (difference between biological and chronological age)
    ///
    /// Positive values indicate accelerated aging (associated with mortality risk).
    /// Negative values indicate decelerated aging.
    pub fn age_acceleration(predicted_age: f64, chronological_age: f64) -> f64 {
        predicted_age - chronological_age
    }
}

/// Cancer signal detector using methylation patterns
///
/// Combines methylation entropy and extreme methylation ratio
/// to produce a cancer risk score (0.0 to 1.0).
pub struct CancerSignalDetector {
    /// Entropy weight in the combined score
    entropy_weight: f64,
    /// Extreme ratio weight
    extreme_weight: f64,
    /// Threshold for elevated cancer risk
    risk_threshold: f64,
}

impl CancerSignalDetector {
    /// Create with default parameters
    pub fn new() -> Self {
        Self {
            entropy_weight: 0.4,
            extreme_weight: 0.6,
            risk_threshold: 0.3,
        }
    }

    /// Detect cancer signal from methylation profile
    ///
    /// Returns (risk_score, is_elevated) where risk_score is 0.0-1.0
    /// and is_elevated indicates whether the score exceeds the threshold.
    pub fn detect(&self, profile: &MethylationProfile) -> CancerSignalResult {
        if profile.sites.is_empty() {
            return CancerSignalResult {
                risk_score: 0.0,
                is_elevated: false,
                entropy: 0.0,
                extreme_ratio: 0.0,
            };
        }

        let entropy = profile.methylation_entropy();
        let extreme_ratio = profile.extreme_methylation_ratio() as f64;

        // Normalize entropy to 0-1 range (max entropy for 10 bins = ln(10) ≈ 2.302)
        let normalized_entropy = (entropy / 2.302).min(1.0);

        let risk_score = (self.entropy_weight * normalized_entropy
            + self.extreme_weight * extreme_ratio)
            .min(1.0);

        CancerSignalResult {
            risk_score,
            is_elevated: risk_score >= self.risk_threshold,
            entropy,
            extreme_ratio,
        }
    }
}

impl Default for CancerSignalDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Result from cancer signal detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancerSignalResult {
    /// Combined risk score (0.0 to 1.0)
    pub risk_score: f64,
    /// Whether the risk score exceeds the threshold
    pub is_elevated: bool,
    /// Raw methylation entropy
    pub entropy: f64,
    /// Fraction of extreme methylation sites
    pub extreme_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_methylation_profile() {
        let positions = vec![(1, 1000), (1, 2000)];
        let betas = vec![0.3, 0.7];
        let profile = MethylationProfile::from_beta_values(positions, betas);

        assert_eq!(profile.sites.len(), 2);
        assert!((profile.mean_methylation() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_horvath_clock() {
        let clock = HorvathClock::default_clock();
        let positions = vec![(1, 1000), (1, 2000), (1, 3000)];
        let betas = vec![0.5, 0.5, 0.5];
        let profile = MethylationProfile::from_beta_values(positions, betas);
        let age = clock.predict_age(&profile);
        assert!(age > 0.0);
    }

    #[test]
    fn test_age_acceleration() {
        let accel = HorvathClock::age_acceleration(55.0, 50.0);
        assert!((accel - 5.0).abs() < 0.001);

        let decel = HorvathClock::age_acceleration(40.0, 50.0);
        assert!((decel - (-10.0)).abs() < 0.001);
    }

    #[test]
    fn test_methylation_entropy() {
        // Uniform methylation = low entropy
        let positions: Vec<(u8, u64)> = (0..100).map(|i| (1u8, i as u64)).collect();
        let betas = vec![0.5; 100];
        let profile = MethylationProfile::from_beta_values(positions, betas);
        let entropy = profile.methylation_entropy();
        assert!(
            entropy < 0.1,
            "Uniform should have low entropy: {}",
            entropy
        );

        // Spread methylation = high entropy
        let positions2: Vec<(u8, u64)> = (0..100).map(|i| (1u8, i as u64)).collect();
        let betas2: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
        let profile2 = MethylationProfile::from_beta_values(positions2, betas2);
        let entropy2 = profile2.methylation_entropy();
        assert!(
            entropy2 > 1.0,
            "Spread should have high entropy: {}",
            entropy2
        );
    }

    #[test]
    fn test_cancer_signal_detector() {
        let detector = CancerSignalDetector::new();

        // Normal profile (moderate methylation)
        let positions: Vec<(u8, u64)> = (0..100).map(|i| (1u8, i as u64)).collect();
        let betas = vec![0.5; 100];
        let profile = MethylationProfile::from_beta_values(positions, betas);
        let result = detector.detect(&profile);
        assert!(!result.is_elevated, "Normal profile should not be elevated");
        assert!(result.risk_score < 0.3);

        // Cancerous profile (extreme methylation)
        let positions2: Vec<(u8, u64)> = (0..100).map(|i| (1u8, i as u64)).collect();
        let betas2: Vec<f32> = (0..100)
            .map(|i| if i % 2 == 0 { 0.95 } else { 0.05 })
            .collect();
        let profile2 = MethylationProfile::from_beta_values(positions2, betas2);
        let result2 = detector.detect(&profile2);
        assert!(result2.is_elevated, "Cancer profile should be elevated");
        assert!(result2.extreme_ratio > 0.8);
    }
}
