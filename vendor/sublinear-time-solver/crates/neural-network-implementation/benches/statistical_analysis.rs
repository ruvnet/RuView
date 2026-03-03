//! Statistical analysis benchmark for System A vs System B comparison
//!
//! This benchmark performs rigorous statistical tests to validate the
//! performance differences between systems, including effect size calculations.

use criterion::{criterion_group, criterion_main, Criterion};
use std::time::Duration;
use temporal_neural_net::prelude::*;
use nalgebra::{DMatrix, DVector};

/// Number of samples for statistical analysis
const STATISTICAL_SAMPLES: usize = 10000;

/// Statistical test result
#[derive(Debug, Clone)]
struct StatisticalTestResult {
    test_name: String,
    test_statistic: f64,
    p_value: f64,
    confidence_interval: (f64, f64),
    effect_size: f64,
    interpretation: String,
    significant: bool,
}

/// Effect size classification
#[derive(Debug, Clone)]
enum EffectSize {
    Negligible,  // < 0.2
    Small,       // 0.2 - 0.5
    Medium,      // 0.5 - 0.8
    Large,       // > 0.8
}

/// Comprehensive statistical analysis context
struct StatisticalAnalysisContext {
    system_a: SystemA,
    system_b: SystemB,
    test_inputs: Vec<DMatrix<f64>>,
}

impl StatisticalAnalysisContext {
    /// Create new statistical analysis context
    fn new() -> Result<Self> {
        let config_a = Config::default();
        let mut config_b = config_a.clone();
        config_b.system = crate::config::SystemConfig::TemporalSolver(
            crate::config::TemporalSolverConfig::default()
        );

        let system_a = SystemA::new(&config_a.model)?;
        let system_b = SystemB::new(&config_b.model)?;

        // Generate test inputs
        let test_inputs = Self::generate_test_inputs();

        Ok(Self {
            system_a,
            system_b,
            test_inputs,
        })
    }

    /// Generate test inputs for statistical analysis
    fn generate_test_inputs() -> Vec<DMatrix<f64>> {
        use rand::prelude::*;
        let mut rng = StdRng::seed_from_u64(12345);

        (0..STATISTICAL_SAMPLES)
            .map(|_| {
                DMatrix::from_fn(64, 4, |_, _| {
                    rng.gen_range(-1.0..1.0)
                })
            })
            .collect()
    }

    /// Collect latency samples for both systems
    fn collect_latency_samples(&mut self) -> Result<(Vec<f64>, Vec<f64>)> {
        use std::time::Instant;

        let mut system_a_latencies = Vec::new();
        let mut system_b_latencies = Vec::new();

        println!("Collecting {} latency samples for statistical analysis...", STATISTICAL_SAMPLES);

        for (i, input) in self.test_inputs.iter().enumerate() {
            // Measure System A
            let start_a = Instant::now();
            let _ = self.system_a.forward(input)?;
            let latency_a = start_a.elapsed().as_micros() as f64; // microseconds
            system_a_latencies.push(latency_a);

            // Measure System B
            let start_b = Instant::now();
            let _ = self.system_b.forward(input)?;
            let latency_b = start_b.elapsed().as_micros() as f64; // microseconds
            system_b_latencies.push(latency_b);

            if i % 1000 == 0 {
                println!("Progress: {}/{}", i, STATISTICAL_SAMPLES);
            }
        }

        Ok((system_a_latencies, system_b_latencies))
    }

    /// Perform paired t-test
    fn paired_t_test(&self, sample_a: &[f64], sample_b: &[f64]) -> StatisticalTestResult {
        let n = sample_a.len() as f64;

        // Calculate differences
        let differences: Vec<f64> = sample_a.iter()
            .zip(sample_b.iter())
            .map(|(a, b)| a - b)
            .collect();

        // Calculate mean difference
        let mean_diff = differences.iter().sum::<f64>() / n;

        // Calculate standard deviation of differences
        let variance = differences.iter()
            .map(|d| (d - mean_diff).powi(2))
            .sum::<f64>() / (n - 1.0);
        let std_dev = variance.sqrt();

        // Calculate t-statistic
        let t_statistic = mean_diff / (std_dev / n.sqrt());

        // Degrees of freedom
        let df = n - 1.0;

        // Calculate p-value (simplified - using t-distribution approximation)
        let p_value = self.t_distribution_p_value(t_statistic, df);

        // Calculate confidence interval (95%)
        let t_critical = 1.96; // Approximate for large samples
        let margin_error = t_critical * (std_dev / n.sqrt());
        let confidence_interval = (mean_diff - margin_error, mean_diff + margin_error);

        // Calculate effect size (Cohen's d for paired samples)
        let effect_size = mean_diff / std_dev;

        // Interpret results
        let significant = p_value < 0.05;
        let interpretation = format!(
            "Mean difference: {:.3}μs, 95% CI: ({:.3}, {:.3}), Cohen's d: {:.3}",
            mean_diff, confidence_interval.0, confidence_interval.1, effect_size
        );

        StatisticalTestResult {
            test_name: "Paired t-test".to_string(),
            test_statistic: t_statistic,
            p_value,
            confidence_interval,
            effect_size,
            interpretation,
            significant,
        }
    }

    /// Perform Mann-Whitney U test (Wilcoxon rank-sum test)
    fn mann_whitney_u_test(&self, sample_a: &[f64], sample_b: &[f64]) -> StatisticalTestResult {
        let n1 = sample_a.len();
        let n2 = sample_b.len();

        // Combine and rank all observations
        let mut combined: Vec<(f64, usize)> = Vec::new();
        for (i, &val) in sample_a.iter().enumerate() {
            combined.push((val, 0)); // 0 for group A
        }
        for (i, &val) in sample_b.iter().enumerate() {
            combined.push((val, 1)); // 1 for group B
        }

        // Sort by value
        combined.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        // Assign ranks (handling ties with average ranks)
        let mut ranks = vec![0.0; combined.len()];
        let mut i = 0;
        while i < combined.len() {
            let mut j = i;
            while j < combined.len() && combined[j].0 == combined[i].0 {
                j += 1;
            }
            let avg_rank = (i + j + 1) as f64 / 2.0;
            for k in i..j {
                ranks[k] = avg_rank;
            }
            i = j;
        }

        // Calculate rank sums
        let mut rank_sum_a = 0.0;
        let mut rank_sum_b = 0.0;
        for (i, (_, group)) in combined.iter().enumerate() {
            if *group == 0 {
                rank_sum_a += ranks[i];
            } else {
                rank_sum_b += ranks[i];
            }
        }

        // Calculate U statistics
        let u1 = rank_sum_a - (n1 * (n1 + 1)) as f64 / 2.0;
        let u2 = rank_sum_b - (n2 * (n2 + 1)) as f64 / 2.0;
        let u_statistic = u1.min(u2);

        // Calculate z-score for normal approximation
        let mean_u = (n1 * n2) as f64 / 2.0;
        let std_u = ((n1 * n2 * (n1 + n2 + 1)) as f64 / 12.0).sqrt();
        let z_score = (u_statistic - mean_u) / std_u;

        // Calculate p-value (two-tailed)
        let p_value = 2.0 * (1.0 - self.standard_normal_cdf(z_score.abs()));

        // Effect size (rank-biserial correlation)
        let effect_size = 1.0 - (2.0 * u_statistic) / (n1 * n2) as f64;

        let significant = p_value < 0.05;
        let interpretation = format!(
            "U statistic: {:.1}, Z-score: {:.3}, Effect size (r): {:.3}",
            u_statistic, z_score, effect_size
        );

        StatisticalTestResult {
            test_name: "Mann-Whitney U test".to_string(),
            test_statistic: u_statistic,
            p_value,
            confidence_interval: (0.0, 0.0), // Not typically calculated for U test
            effect_size,
            interpretation,
            significant,
        }
    }

    /// Calculate bootstrap confidence interval for difference in means
    fn bootstrap_confidence_interval(&self, sample_a: &[f64], sample_b: &[f64], n_bootstrap: usize) -> (f64, f64) {
        use rand::prelude::*;
        let mut rng = StdRng::seed_from_u64(42);

        let mut bootstrap_diffs = Vec::new();

        for _ in 0..n_bootstrap {
            // Bootstrap resample both groups
            let bootstrap_a: Vec<f64> = (0..sample_a.len())
                .map(|_| sample_a[rng.gen_range(0..sample_a.len())])
                .collect();
            let bootstrap_b: Vec<f64> = (0..sample_b.len())
                .map(|_| sample_b[rng.gen_range(0..sample_b.len())])
                .collect();

            // Calculate means
            let mean_a = bootstrap_a.iter().sum::<f64>() / bootstrap_a.len() as f64;
            let mean_b = bootstrap_b.iter().sum::<f64>() / bootstrap_b.len() as f64;

            bootstrap_diffs.push(mean_a - mean_b);
        }

        bootstrap_diffs.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // 95% confidence interval
        let lower_idx = ((n_bootstrap as f64) * 0.025) as usize;
        let upper_idx = ((n_bootstrap as f64) * 0.975) as usize;

        (bootstrap_diffs[lower_idx], bootstrap_diffs[upper_idx])
    }

    /// Calculate various effect size measures
    fn calculate_effect_sizes(&self, sample_a: &[f64], sample_b: &[f64]) -> Vec<(String, f64, EffectSize)> {
        let mean_a = sample_a.iter().sum::<f64>() / sample_a.len() as f64;
        let mean_b = sample_b.iter().sum::<f64>() / sample_b.len() as f64;

        let var_a = sample_a.iter()
            .map(|x| (x - mean_a).powi(2))
            .sum::<f64>() / (sample_a.len() - 1) as f64;
        let var_b = sample_b.iter()
            .map(|x| (x - mean_b).powi(2))
            .sum::<f64>() / (sample_b.len() - 1) as f64;

        let pooled_std = ((var_a + var_b) / 2.0).sqrt();

        let mut effect_sizes = Vec::new();

        // Cohen's d
        let cohens_d = (mean_a - mean_b) / pooled_std;
        effect_sizes.push(("Cohen's d".to_string(), cohens_d, self.classify_effect_size(cohens_d.abs())));

        // Glass's Δ (using sample_a as control)
        let glass_delta = (mean_a - mean_b) / var_a.sqrt();
        effect_sizes.push(("Glass's Δ".to_string(), glass_delta, self.classify_effect_size(glass_delta.abs())));

        // Hedge's g (bias-corrected Cohen's d)
        let n = sample_a.len() + sample_b.len();
        let correction = 1.0 - 3.0 / (4.0 * n as f64 - 9.0);
        let hedges_g = cohens_d * correction;
        effect_sizes.push(("Hedge's g".to_string(), hedges_g, self.classify_effect_size(hedges_g.abs())));

        // Common Language Effect Size (probability of superiority)
        let cles = self.calculate_cles(sample_a, sample_b);
        effect_sizes.push(("CLES".to_string(), cles, self.classify_effect_size((cles - 0.5).abs() * 2.0)));

        effect_sizes
    }

    /// Calculate Common Language Effect Size
    fn calculate_cles(&self, sample_a: &[f64], sample_b: &[f64]) -> f64 {
        let mut count = 0;
        let mut total = 0;

        for &a in sample_a {
            for &b in sample_b {
                total += 1;
                if a > b {
                    count += 1;
                }
            }
        }

        count as f64 / total as f64
    }

    /// Classify effect size magnitude
    fn classify_effect_size(&self, effect_size: f64) -> EffectSize {
        if effect_size < 0.2 {
            EffectSize::Negligible
        } else if effect_size < 0.5 {
            EffectSize::Small
        } else if effect_size < 0.8 {
            EffectSize::Medium
        } else {
            EffectSize::Large
        }
    }

    /// Simplified t-distribution p-value calculation
    fn t_distribution_p_value(&self, t: f64, df: f64) -> f64 {
        // Simplified approximation - in practice, use a proper statistical library
        let z = t / (1.0 + t.powi(2) / (4.0 * df)).sqrt();
        2.0 * (1.0 - self.standard_normal_cdf(z.abs()))
    }

    /// Standard normal CDF approximation
    fn standard_normal_cdf(&self, x: f64) -> f64 {
        // Abramowitz and Stegun approximation
        let a1 =  0.254829592;
        let a2 = -0.284496736;
        let a3 =  1.421413741;
        let a4 = -1.453152027;
        let a5 =  1.061405429;
        let p  =  0.3275911;

        let sign = if x < 0.0 { -1.0 } else { 1.0 };
        let x = x.abs();

        let t = 1.0 / (1.0 + p * x);
        let y = 1.0 - (((((a5 * t + a4) * t) + a3) * t + a2) * t + a1) * t * (-x * x / 2.0).exp();

        0.5 * (1.0 + sign * y)
    }

    /// Perform power analysis
    fn power_analysis(&self, sample_a: &[f64], sample_b: &[f64], alpha: f64) -> f64 {
        let mean_a = sample_a.iter().sum::<f64>() / sample_a.len() as f64;
        let mean_b = sample_b.iter().sum::<f64>() / sample_b.len() as f64;

        let var_a = sample_a.iter()
            .map(|x| (x - mean_a).powi(2))
            .sum::<f64>() / (sample_a.len() - 1) as f64;
        let var_b = sample_b.iter()
            .map(|x| (x - mean_b).powi(2))
            .sum::<f64>() / (sample_b.len() - 1) as f64;

        let pooled_var = (var_a + var_b) / 2.0;
        let effect_size = (mean_a - mean_b).abs() / pooled_var.sqrt();

        let n = sample_a.len().min(sample_b.len()) as f64;
        let delta = effect_size * (n / 2.0).sqrt();

        // Critical value for two-tailed test
        let z_alpha = self.inverse_normal_cdf(1.0 - alpha / 2.0);
        let z_beta = delta - z_alpha;

        self.standard_normal_cdf(z_beta)
    }

    /// Inverse normal CDF (simplified)
    fn inverse_normal_cdf(&self, p: f64) -> f64 {
        // Simplified approximation - Beasley-Springer-Moro algorithm
        let a = vec![
            -3.969683028665376e+01,
             2.209460984245205e+02,
            -2.759285104469687e+02,
             1.383577518672690e+02,
            -3.066479806614716e+01,
             2.506628277459239e+00,
        ];

        let b = vec![
            -5.447609879822406e+01,
             1.615858368580409e+02,
            -1.556989798598866e+02,
             6.680131188771972e+01,
            -1.328068155288572e+01,
        ];

        let c = vec![
            -7.784894002430293e-03,
            -3.223964580411365e-01,
            -2.400758277161838e+00,
            -2.549732539343734e+00,
             4.374664141464968e+00,
             2.938163982698783e+00,
        ];

        let d = vec![
             7.784695709041462e-03,
             3.224671290700398e-01,
             2.445134137142996e+00,
             3.754408661907416e+00,
        ];

        let p_low = 0.02425;
        let p_high = 1.0 - p_low;

        if p < p_low {
            let q = (-2.0 * p.ln()).sqrt();
            return (((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5]) /
                   ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0);
        } else if p <= p_high {
            let q = p - 0.5;
            let r = q * q;
            return (((((a[0] * r + a[1]) * r + a[2]) * r + a[3]) * r + a[4]) * r + a[5]) * q /
                   (((((b[0] * r + b[1]) * r + b[2]) * r + b[3]) * r + b[4]) * r + 1.0);
        } else {
            let q = (-2.0 * (1.0 - p).ln()).sqrt();
            return -(((((c[0] * q + c[1]) * q + c[2]) * q + c[3]) * q + c[4]) * q + c[5]) /
                    ((((d[0] * q + d[1]) * q + d[2]) * q + d[3]) * q + 1.0);
        }
    }

    /// Generate comprehensive statistical report
    fn generate_statistical_report(
        &self,
        sample_a: &[f64],
        sample_b: &[f64],
        tests: &[StatisticalTestResult],
        effect_sizes: &[(String, f64, EffectSize)],
        bootstrap_ci: (f64, f64),
        power: f64,
    ) -> String {
        let mut report = String::new();
        report.push_str("# Statistical Analysis Report: System A vs System B\n\n");

        // Sample statistics
        let mean_a = sample_a.iter().sum::<f64>() / sample_a.len() as f64;
        let mean_b = sample_b.iter().sum::<f64>() / sample_b.len() as f64;
        let std_a = (sample_a.iter().map(|x| (x - mean_a).powi(2)).sum::<f64>() / (sample_a.len() - 1) as f64).sqrt();
        let std_b = (sample_b.iter().map(|x| (x - mean_b).powi(2)).sum::<f64>() / (sample_b.len() - 1) as f64).sqrt();

        report.push_str("## Descriptive Statistics\n\n");
        report.push_str("| System | N | Mean (μs) | Std Dev (μs) | Min (μs) | Max (μs) |\n");
        report.push_str("|--------|---|-----------|--------------|----------|----------|\n");
        report.push_str(&format!("| System A | {} | {:.3} | {:.3} | {:.3} | {:.3} |\n",
            sample_a.len(), mean_a, std_a,
            sample_a.iter().fold(f64::INFINITY, |acc, &x| acc.min(x)),
            sample_a.iter().fold(f64::NEG_INFINITY, |acc, &x| acc.max(x))
        ));
        report.push_str(&format!("| System B | {} | {:.3} | {:.3} | {:.3} | {:.3} |\n\n",
            sample_b.len(), mean_b, std_b,
            sample_b.iter().fold(f64::INFINITY, |acc, &x| acc.min(x)),
            sample_b.iter().fold(f64::NEG_INFINITY, |acc, &x| acc.max(x))
        ));

        // Statistical tests
        report.push_str("## Statistical Tests\n\n");
        for test in tests {
            report.push_str(&format!("### {}\n\n", test.test_name));
            report.push_str("| Metric | Value |\n|--------|-------|\n");
            report.push_str(&format!("| Test Statistic | {:.4} |\n", test.test_statistic));
            report.push_str(&format!("| p-value | {:.6} |\n", test.p_value));
            report.push_str(&format!("| Significant (α=0.05) | {} |\n", if test.significant { "Yes ✅" } else { "No ❌" }));
            if test.confidence_interval.0 != 0.0 || test.confidence_interval.1 != 0.0 {
                report.push_str(&format!("| 95% CI | ({:.3}, {:.3}) |\n", test.confidence_interval.0, test.confidence_interval.1));
            }
            report.push_str(&format!("| Interpretation | {} |\n\n", test.interpretation));
        }

        // Bootstrap confidence interval
        report.push_str("## Bootstrap Analysis\n\n");
        report.push_str(&format!("**Bootstrap 95% Confidence Interval for Difference in Means:**\n"));
        report.push_str(&format!("({:.3}, {:.3}) μs\n\n", bootstrap_ci.0, bootstrap_ci.1));

        // Effect sizes
        report.push_str("## Effect Size Analysis\n\n");
        report.push_str("| Measure | Value | Magnitude | Interpretation |\n");
        report.push_str("|---------|-------|-----------|----------------|\n");
        for (name, value, magnitude) in effect_sizes {
            let magnitude_str = match magnitude {
                EffectSize::Negligible => "Negligible",
                EffectSize::Small => "Small",
                EffectSize::Medium => "Medium",
                EffectSize::Large => "Large",
            };
            let interpretation = match name.as_str() {
                "Cohen's d" => "Standardized mean difference",
                "Glass's Δ" => "Mean difference in control group SD units",
                "Hedge's g" => "Bias-corrected Cohen's d",
                "CLES" => "Probability that System A > System B",
                _ => "Effect size measure",
            };
            report.push_str(&format!("| {} | {:.4} | {} | {} |\n", name, value, magnitude_str, interpretation));
        }

        // Power analysis
        report.push_str(&format!("\n## Power Analysis\n\n"));
        report.push_str(&format!("**Statistical Power:** {:.3} ({:.1}%)\n\n", power, power * 100.0));

        if power < 0.8 {
            report.push_str("⚠️  **Warning:** Statistical power is below the conventional threshold of 0.8. Consider increasing sample size for more reliable results.\n\n");
        } else {
            report.push_str("✅ **Good:** Statistical power exceeds the conventional threshold of 0.8.\n\n");
        }

        // Summary and conclusions
        report.push_str("## Summary and Conclusions\n\n");

        let significant_tests = tests.iter().filter(|t| t.significant).count();
        let total_tests = tests.len();

        report.push_str(&format!("**Statistical Significance:** {}/{} tests show significant differences (p < 0.05)\n\n", significant_tests, total_tests));

        let largest_effect = effect_sizes.iter()
            .filter(|(name, _, _)| name == "Cohen's d")
            .map(|(_, value, _)| value.abs())
            .next()
            .unwrap_or(0.0);

        if significant_tests > 0 && largest_effect > 0.5 {
            report.push_str("🎉 **Conclusion:** The statistical analysis provides strong evidence for a significant and meaningful performance difference between System A and System B. The effect size is moderate to large, indicating practical significance beyond statistical significance.\n\n");
        } else if significant_tests > 0 {
            report.push_str("📊 **Conclusion:** There is statistical evidence for a difference between systems, but the effect size suggests the practical impact may be limited.\n\n");
        } else {
            report.push_str("📈 **Conclusion:** No statistically significant differences were detected between the systems at the α = 0.05 level.\n\n");
        }

        // Recommendations
        report.push_str("## Recommendations\n\n");

        if power < 0.8 {
            let recommended_n = ((1.96 + 0.84).powi(2) * (std_a.powi(2) + std_b.powi(2)) / (mean_a - mean_b).powi(2)).ceil() as usize;
            report.push_str(&format!("1. **Sample Size:** Consider increasing sample size to approximately {} per group for 80% power.\n", recommended_n));
        }

        if largest_effect > 0.8 {
            report.push_str("2. **Effect Size:** The large effect size suggests this is a practically meaningful difference worth investigating further.\n");
        }

        if significant_tests == total_tests {
            report.push_str("3. **Consistency:** All statistical tests agree on significance, providing strong evidence for the observed difference.\n");
        }

        report.push_str(&format!("\n---\n*Generated from {} samples per system using rigorous statistical methods.*", sample_a.len()));

        report
    }
}

/// Main statistical analysis benchmark
fn bench_statistical_analysis(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        let mut context = StatisticalAnalysisContext::new()
            .expect("Failed to create statistical analysis context");

        println!("Running comprehensive statistical analysis...");

        // Collect latency samples
        let (system_a_latencies, system_b_latencies) = context.collect_latency_samples()
            .expect("Failed to collect samples");

        // Perform statistical tests
        let mut tests = Vec::new();

        // Paired t-test
        let t_test = context.paired_t_test(&system_a_latencies, &system_b_latencies);
        tests.push(t_test);

        // Mann-Whitney U test
        let u_test = context.mann_whitney_u_test(&system_a_latencies, &system_b_latencies);
        tests.push(u_test);

        // Bootstrap confidence interval
        let bootstrap_ci = context.bootstrap_confidence_interval(&system_a_latencies, &system_b_latencies, 10000);

        // Effect size calculations
        let effect_sizes = context.calculate_effect_sizes(&system_a_latencies, &system_b_latencies);

        // Power analysis
        let power = context.power_analysis(&system_a_latencies, &system_b_latencies, 0.05);

        // Generate comprehensive report
        let report = context.generate_statistical_report(
            &system_a_latencies,
            &system_b_latencies,
            &tests,
            &effect_sizes,
            bootstrap_ci,
            power,
        );

        std::fs::write("statistical_analysis_report.md", report)
            .expect("Failed to save statistical analysis report");

        println!("✅ Statistical analysis completed!");
        println!("📊 Report saved to: statistical_analysis_report.md");
    });
}

criterion_group!(
    name = statistical_benches;
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(Duration::from_secs(180)) // 3 minutes for comprehensive analysis
        .warm_up_time(Duration::from_secs(30));
    targets = bench_statistical_analysis
);
criterion_main!(statistical_benches);