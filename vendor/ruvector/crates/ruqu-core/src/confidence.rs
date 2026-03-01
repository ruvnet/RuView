//! Confidence bounds, statistical tests, and convergence utilities for
//! quantum measurement analysis.
//!
//! This module provides tools for reasoning about the statistical quality of
//! shot-based quantum simulation results, including confidence intervals for
//! binomial proportions, expectation values, shot budget estimation, distribution
//! distance metrics, goodness-of-fit tests, and convergence monitoring.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A confidence interval around a point estimate.
#[derive(Debug, Clone)]
pub struct ConfidenceInterval {
    /// Lower bound of the interval.
    pub lower: f64,
    /// Upper bound of the interval.
    pub upper: f64,
    /// Point estimate (e.g., sample proportion).
    pub point_estimate: f64,
    /// Confidence level, e.g., 0.95 for a 95 % interval.
    pub confidence_level: f64,
    /// Human-readable label for the method used.
    pub method: &'static str,
}

/// Result of a chi-squared goodness-of-fit test.
#[derive(Debug, Clone)]
pub struct ChiSquaredResult {
    /// The chi-squared statistic.
    pub statistic: f64,
    /// Degrees of freedom (number of categories minus one).
    pub degrees_of_freedom: usize,
    /// Approximate p-value.
    pub p_value: f64,
    /// Whether the result is significant at the 0.05 level.
    pub significant: bool,
}

/// Tracks a running sequence of estimates and detects convergence.
pub struct ConvergenceMonitor {
    estimates: Vec<f64>,
    window_size: usize,
}

// ---------------------------------------------------------------------------
// Helpers: inverse normal CDF (z-score)
// ---------------------------------------------------------------------------

/// Approximate the z-score (inverse standard-normal CDF) for a given two-sided
/// confidence level using the rational approximation of Abramowitz & Stegun
/// (formula 26.2.23).
///
/// For confidence level `c`, we compute the upper quantile at
/// `p = (1 + c) / 2` and return the corresponding z-value.
///
/// # Panics
///
/// Panics if `confidence` is not in the open interval (0, 1).
pub fn z_score(confidence: f64) -> f64 {
    assert!(
        confidence > 0.0 && confidence < 1.0,
        "confidence must be in (0, 1)"
    );

    let p = (1.0 + confidence) / 2.0; // upper tail probability
                                      // 1 - p is the tail area; for p close to 1 this is small and positive.
    let tail = 1.0 - p;

    // Rational approximation: for tail area `q`, set t = sqrt(-2 ln q).
    let t = (-2.0_f64 * tail.ln()).sqrt();

    // Coefficients (Abramowitz & Stegun 26.2.23)
    let c0 = 2.515517;
    let c1 = 0.802853;
    let c2 = 0.010328;
    let d1 = 1.432788;
    let d2 = 0.189269;
    let d3 = 0.001308;

    t - (c0 + c1 * t + c2 * t * t) / (1.0 + d1 * t + d2 * t * t + d3 * t * t * t)
}

// ---------------------------------------------------------------------------
// Wilson score interval
// ---------------------------------------------------------------------------

/// Compute the Wilson score confidence interval for a binomial proportion.
///
/// The Wilson interval is centred near the MLE but accounts for the discrete
/// nature of the binomial and never produces bounds outside [0, 1].
///
/// # Arguments
///
/// * `successes` -- number of successes observed.
/// * `trials`    -- total number of trials (must be > 0).
/// * `confidence` -- desired confidence level in (0, 1).
pub fn wilson_interval(successes: usize, trials: usize, confidence: f64) -> ConfidenceInterval {
    assert!(trials > 0, "trials must be > 0");
    assert!(
        confidence > 0.0 && confidence < 1.0,
        "confidence must be in (0, 1)"
    );

    let n = trials as f64;
    let p_hat = successes as f64 / n;
    let z = z_score(confidence);
    let z2 = z * z;

    let denom = 1.0 + z2 / n;
    let centre = (p_hat + z2 / (2.0 * n)) / denom;
    let half_width = z * (p_hat * (1.0 - p_hat) / n + z2 / (4.0 * n * n)).sqrt() / denom;

    let lower = (centre - half_width).max(0.0);
    let upper = (centre + half_width).min(1.0);

    ConfidenceInterval {
        lower,
        upper,
        point_estimate: p_hat,
        confidence_level: confidence,
        method: "wilson",
    }
}

// ---------------------------------------------------------------------------
// Clopper-Pearson exact interval
// ---------------------------------------------------------------------------

/// Compute the Clopper-Pearson (exact) confidence interval for a binomial
/// proportion via bisection on the binomial CDF.
///
/// This interval is conservative -- it guarantees at least the nominal coverage
/// probability, but may be wider than necessary.
///
/// # Arguments
///
/// * `successes` -- number of successes observed.
/// * `trials`    -- total number of trials (must be > 0).
/// * `confidence` -- desired confidence level in (0, 1).
pub fn clopper_pearson(successes: usize, trials: usize, confidence: f64) -> ConfidenceInterval {
    assert!(trials > 0, "trials must be > 0");
    assert!(
        confidence > 0.0 && confidence < 1.0,
        "confidence must be in (0, 1)"
    );

    let alpha = 1.0 - confidence;
    let n = trials;
    let k = successes;
    let p_hat = k as f64 / n as f64;

    // Lower bound: find p such that P(X >= k | n, p) = alpha/2,
    // equivalently P(X <= k-1 | n, p) = 1 - alpha/2.
    let lower = if k == 0 {
        0.0
    } else {
        bisect_binomial_cdf(n, k - 1, 1.0 - alpha / 2.0)
    };

    // Upper bound: find p such that P(X <= k | n, p) = alpha/2.
    let upper = if k == n {
        1.0
    } else {
        bisect_binomial_cdf(n, k, alpha / 2.0)
    };

    ConfidenceInterval {
        lower,
        upper,
        point_estimate: p_hat,
        confidence_level: confidence,
        method: "clopper-pearson",
    }
}

/// Use bisection to find `p` such that `binomial_cdf(n, k, p) = target`.
///
/// `binomial_cdf(n, k, p)` = sum_{i=0}^{k} C(n,i) p^i (1-p)^{n-i}.
fn bisect_binomial_cdf(n: usize, k: usize, target: f64) -> f64 {
    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;

    for _ in 0..200 {
        let mid = (lo + hi) / 2.0;
        let cdf = binomial_cdf(n, k, mid);
        if cdf < target {
            // CDF is too small; increasing p increases CDF, so move lo up.
            // Actually: increasing p *decreases* P(X <= k) when k < n.
            // Let's think carefully:
            //   P(X <= k | p) is monotonically *decreasing* in p for k < n.
            //   So if cdf < target we need to *decrease* p.
            hi = mid;
        } else {
            lo = mid;
        }

        if (hi - lo) < 1e-15 {
            break;
        }
    }
    (lo + hi) / 2.0
}

/// Evaluate the binomial CDF: P(X <= k) where X ~ Bin(n, p).
///
/// Uses a log-space computation to avoid overflow for large n.
fn binomial_cdf(n: usize, k: usize, p: f64) -> f64 {
    if p <= 0.0 {
        return 1.0;
    }
    if p >= 1.0 {
        return if k >= n { 1.0 } else { 0.0 };
    }
    if k >= n {
        return 1.0;
    }

    // Use the regularised incomplete beta function identity:
    //   P(X <= k | n, p) = I_{1-p}(n - k, k + 1)
    // We compute the CDF directly via summation in log-space for moderate n.
    // For very large n this could be slow, but quantum shot counts are typically
    // at most millions, and this is called from bisection which only needs
    // ~200 evaluations.
    let mut cdf = 0.0_f64;
    // log_binom accumulates log(C(n, i)) incrementally.
    let ln_p = p.ln();
    let ln_1mp = (1.0 - p).ln();

    // Start with i = 0: C(n,0) * p^0 * (1-p)^n
    let mut log_binom = 0.0_f64; // log C(n, 0) = 0
    cdf += (log_binom + ln_1mp * n as f64).exp();

    for i in 1..=k {
        // log C(n, i) = log C(n, i-1) + log(n - i + 1) - log(i)
        log_binom += ((n - i + 1) as f64).ln() - (i as f64).ln();
        let log_term = log_binom + ln_p * i as f64 + ln_1mp * (n - i) as f64;
        cdf += log_term.exp();
    }

    cdf.min(1.0).max(0.0)
}

// ---------------------------------------------------------------------------
// Expectation value confidence interval
// ---------------------------------------------------------------------------

/// Compute a confidence interval for the expectation value <Z> of a given
/// qubit from shot counts.
///
/// For qubit `q`, the Z expectation value is `P(0) - P(1)` where P(0) is the
/// fraction of shots where qubit `q` measured `false` and P(1) where it
/// measured `true`.
///
/// The standard error is computed from the multinomial variance:
///   Var(<Z>) = (1 - <Z>^2) / n
///   SE       = sqrt(Var(<Z>) / n)  ... but more precisely, each shot produces
///   a value +1 or -1 so Var = 1 - mean^2, and SE = sqrt(Var / n).
///
/// The returned interval is `<Z> +/- z * SE`.
pub fn expectation_confidence(
    counts: &HashMap<Vec<bool>, usize>,
    qubit: u32,
    confidence: f64,
) -> ConfidenceInterval {
    assert!(
        confidence > 0.0 && confidence < 1.0,
        "confidence must be in (0, 1)"
    );

    let mut n_zero: usize = 0;
    let mut n_one: usize = 0;

    for (bits, &count) in counts {
        if let Some(&b) = bits.get(qubit as usize) {
            if b {
                n_one += count;
            } else {
                n_zero += count;
            }
        }
    }

    let total = (n_zero + n_one) as f64;
    assert!(total > 0.0, "no shots found for the given qubit");

    let p0 = n_zero as f64 / total;
    let p1 = n_one as f64 / total;
    let exp_z = p0 - p1; // <Z>

    // Each shot yields +1 (qubit=0) or -1 (qubit=1).
    // Variance of a single shot = E[X^2] - E[X]^2 = 1 - exp_z^2.
    let var_single = 1.0 - exp_z * exp_z;
    let se = (var_single / total).sqrt();

    let z = z_score(confidence);
    let lower = (exp_z - z * se).max(-1.0);
    let upper = (exp_z + z * se).min(1.0);

    ConfidenceInterval {
        lower,
        upper,
        point_estimate: exp_z,
        confidence_level: confidence,
        method: "expectation-z-se",
    }
}

// ---------------------------------------------------------------------------
// Shot budget calculator
// ---------------------------------------------------------------------------

/// Compute the minimum number of shots required so that the additive error of
/// an empirical probability is at most `epsilon` with probability at least
/// `1 - delta`, using the Hoeffding bound.
///
/// Formula: N >= ln(2 / delta) / (2 * epsilon^2)
///
/// # Panics
///
/// Panics if `epsilon` or `delta` is not in (0, 1).
pub fn required_shots(epsilon: f64, delta: f64) -> usize {
    assert!(epsilon > 0.0 && epsilon < 1.0, "epsilon must be in (0, 1)");
    assert!(delta > 0.0 && delta < 1.0, "delta must be in (0, 1)");

    let n = (2.0_f64 / delta).ln() / (2.0 * epsilon * epsilon);
    n.ceil() as usize
}

// ---------------------------------------------------------------------------
// Total variation distance
// ---------------------------------------------------------------------------

/// Compute the total variation distance between two empirical distributions
/// given as shot-count histograms.
///
/// TVD = 0.5 * sum_i |p_i - q_i| over all bitstrings present in either
/// distribution.
pub fn total_variation_distance(
    p: &HashMap<Vec<bool>, usize>,
    q: &HashMap<Vec<bool>, usize>,
) -> f64 {
    let total_p: f64 = p.values().sum::<usize>() as f64;
    let total_q: f64 = q.values().sum::<usize>() as f64;

    if total_p == 0.0 && total_q == 0.0 {
        return 0.0;
    }

    // Collect all keys from both distributions.
    let mut all_keys: Vec<&Vec<bool>> = Vec::new();
    for key in p.keys() {
        all_keys.push(key);
    }
    for key in q.keys() {
        if !p.contains_key(key) {
            all_keys.push(key);
        }
    }

    let mut tvd = 0.0_f64;
    for key in &all_keys {
        let pi = if total_p > 0.0 {
            *p.get(*key).unwrap_or(&0) as f64 / total_p
        } else {
            0.0
        };
        let qi = if total_q > 0.0 {
            *q.get(*key).unwrap_or(&0) as f64 / total_q
        } else {
            0.0
        };
        tvd += (pi - qi).abs();
    }

    0.5 * tvd
}

// ---------------------------------------------------------------------------
// Chi-squared test
// ---------------------------------------------------------------------------

/// Perform a chi-squared goodness-of-fit test comparing an observed
/// distribution to an expected distribution.
///
/// The expected distribution is scaled to match the total number of observed
/// counts. The p-value is approximated using the Wilson-Hilferty cube-root
/// transformation of the chi-squared CDF.
///
/// # Panics
///
/// Panics if there are no categories or if the expected distribution has zero
/// total counts.
pub fn chi_squared_test(
    observed: &HashMap<Vec<bool>, usize>,
    expected: &HashMap<Vec<bool>, usize>,
) -> ChiSquaredResult {
    let total_observed: f64 = observed.values().sum::<usize>() as f64;
    let total_expected: f64 = expected.values().sum::<usize>() as f64;

    assert!(
        total_expected > 0.0,
        "expected distribution must have nonzero total"
    );

    // Collect all keys.
    let mut all_keys: Vec<&Vec<bool>> = Vec::new();
    for key in observed.keys() {
        all_keys.push(key);
    }
    for key in expected.keys() {
        if !observed.contains_key(key) {
            all_keys.push(key);
        }
    }

    let mut statistic = 0.0_f64;
    let mut num_categories = 0_usize;

    for key in &all_keys {
        let o = *observed.get(*key).unwrap_or(&0) as f64;
        // Scale expected counts to match observed total.
        let e_raw = *expected.get(*key).unwrap_or(&0) as f64;
        let e = e_raw * total_observed / total_expected;

        if e > 0.0 {
            statistic += (o - e) * (o - e) / e;
            num_categories += 1;
        }
    }

    let df = if num_categories > 1 {
        num_categories - 1
    } else {
        1
    };

    let p_value = chi_squared_survival(statistic, df);

    ChiSquaredResult {
        statistic,
        degrees_of_freedom: df,
        p_value,
        significant: p_value < 0.05,
    }
}

/// Approximate the survival function (1 - CDF) of the chi-squared distribution
/// using the Wilson-Hilferty normal approximation.
///
/// For chi-squared random variable X with k degrees of freedom:
///   (X/k)^{1/3} is approximately normal with mean 1 - 2/(9k)
///   and variance 2/(9k).
///
/// So P(X > x) approx P(Z > z) where
///   z = ((x/k)^{1/3} - (1 - 2/(9k))) / sqrt(2/(9k))
/// and P(Z > z) = 1 - Phi(z) = Phi(-z).
fn chi_squared_survival(x: f64, df: usize) -> f64 {
    if df == 0 {
        return if x > 0.0 { 0.0 } else { 1.0 };
    }

    if x <= 0.0 {
        return 1.0;
    }

    let k = df as f64;
    let term = 2.0 / (9.0 * k);
    let cube_root = (x / k).powf(1.0 / 3.0);
    let z = (cube_root - (1.0 - term)) / term.sqrt();

    // P(Z > z) = 1 - Phi(z) = Phi(-z)
    normal_cdf(-z)
}

/// Approximate the standard normal CDF using the Abramowitz & Stegun
/// approximation (formula 7.1.26).
fn normal_cdf(x: f64) -> f64 {
    // Use the error function relation: Phi(x) = 0.5 * (1 + erf(x / sqrt(2)))
    // We approximate erf via the Horner form of the A&S rational approximation.
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x_abs = x.abs();

    let t = 1.0 / (1.0 + 0.2316419 * x_abs);
    let d = 0.3989422804014327; // 1/sqrt(2*pi)
    let p = d * (-x_abs * x_abs / 2.0).exp();

    let poly = t
        * (0.319381530
            + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));

    if sign > 0.0 {
        1.0 - p * poly
    } else {
        p * poly
    }
}

// ---------------------------------------------------------------------------
// Convergence monitor
// ---------------------------------------------------------------------------

impl ConvergenceMonitor {
    /// Create a new monitor with the given window size.
    ///
    /// The monitor considers the sequence converged when the last
    /// `window_size` estimates all lie within `epsilon` of each other.
    pub fn new(window_size: usize) -> Self {
        assert!(window_size > 0, "window_size must be > 0");
        Self {
            estimates: Vec::new(),
            window_size,
        }
    }

    /// Record a new estimate.
    pub fn add_estimate(&mut self, value: f64) {
        self.estimates.push(value);
    }

    /// Check whether the last `window_size` estimates have converged: i.e.,
    /// the maximum minus the minimum within the window is less than `epsilon`.
    pub fn has_converged(&self, epsilon: f64) -> bool {
        if self.estimates.len() < self.window_size {
            return false;
        }

        let window = &self.estimates[self.estimates.len() - self.window_size..];
        let min = window.iter().copied().fold(f64::INFINITY, f64::min);
        let max = window.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        (max - min) < epsilon
    }

    /// Return the most recent estimate, or `None` if no estimates have been
    /// added.
    pub fn current_estimate(&self) -> Option<f64> {
        self.estimates.last().copied()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // z_score
    // -----------------------------------------------------------------------

    #[test]
    fn z_score_95() {
        let z = z_score(0.95);
        assert!(
            (z - 1.96).abs() < 0.01,
            "z_score(0.95) = {z}, expected ~1.96"
        );
    }

    #[test]
    fn z_score_99() {
        let z = z_score(0.99);
        assert!(
            (z - 2.576).abs() < 0.02,
            "z_score(0.99) = {z}, expected ~2.576"
        );
    }

    #[test]
    fn z_score_90() {
        let z = z_score(0.90);
        assert!(
            (z - 1.645).abs() < 0.01,
            "z_score(0.90) = {z}, expected ~1.645"
        );
    }

    // -----------------------------------------------------------------------
    // Wilson interval
    // -----------------------------------------------------------------------

    #[test]
    fn wilson_contains_true_proportion() {
        // 50 successes out of 100 trials, true p = 0.5
        let ci = wilson_interval(50, 100, 0.95);
        assert!(
            ci.lower < 0.5 && ci.upper > 0.5,
            "Wilson CI should contain 0.5: {ci:?}"
        );
        assert_eq!(ci.method, "wilson");
        assert!((ci.point_estimate - 0.5).abs() < 1e-12);
    }

    #[test]
    fn wilson_asymmetric() {
        // 1 success out of 100 -- the interval should still be reasonable.
        let ci = wilson_interval(1, 100, 0.95);
        assert!(ci.lower >= 0.0);
        assert!(ci.upper <= 1.0);
        assert!(ci.lower < 0.01);
        assert!(ci.upper > 0.01);
    }

    #[test]
    fn wilson_zero_successes() {
        let ci = wilson_interval(0, 100, 0.95);
        assert_eq!(ci.lower, 0.0);
        assert!(ci.upper > 0.0);
        assert!((ci.point_estimate - 0.0).abs() < 1e-12);
    }

    // -----------------------------------------------------------------------
    // Clopper-Pearson
    // -----------------------------------------------------------------------

    #[test]
    fn clopper_pearson_contains_true_proportion() {
        let ci = clopper_pearson(50, 100, 0.95);
        assert!(
            ci.lower < 0.5 && ci.upper > 0.5,
            "Clopper-Pearson CI should contain 0.5: {ci:?}"
        );
        assert_eq!(ci.method, "clopper-pearson");
    }

    #[test]
    fn clopper_pearson_is_conservative() {
        // Clopper-Pearson should be wider than Wilson for the same data.
        let cp = clopper_pearson(50, 100, 0.95);
        let w = wilson_interval(50, 100, 0.95);

        let cp_width = cp.upper - cp.lower;
        let w_width = w.upper - w.lower;

        assert!(
            cp_width >= w_width - 1e-10,
            "Clopper-Pearson width ({cp_width}) should be >= Wilson width ({w_width})"
        );
    }

    #[test]
    fn clopper_pearson_edge_zero() {
        let ci = clopper_pearson(0, 100, 0.95);
        assert_eq!(ci.lower, 0.0);
        assert!(ci.upper > 0.0);
    }

    #[test]
    fn clopper_pearson_edge_all() {
        let ci = clopper_pearson(100, 100, 0.95);
        assert_eq!(ci.upper, 1.0);
        assert!(ci.lower < 1.0);
    }

    // -----------------------------------------------------------------------
    // Expectation value confidence
    // -----------------------------------------------------------------------

    #[test]
    fn expectation_all_zero() {
        // All shots measure |0>: <Z> = 1.0
        let mut counts = HashMap::new();
        counts.insert(vec![false], 1000);
        let ci = expectation_confidence(&counts, 0, 0.95);
        assert!((ci.point_estimate - 1.0).abs() < 1e-12);
        assert!(ci.lower <= 1.0);
        assert!(ci.upper >= 1.0 - 1e-6);
    }

    #[test]
    fn expectation_all_one() {
        // All shots measure |1>: <Z> = -1.0
        let mut counts = HashMap::new();
        counts.insert(vec![true], 1000);
        let ci = expectation_confidence(&counts, 0, 0.95);
        assert!((ci.point_estimate - (-1.0)).abs() < 1e-12);
    }

    #[test]
    fn expectation_balanced() {
        // Equal |0> and |1>: <Z> = 0.0
        let mut counts = HashMap::new();
        counts.insert(vec![false], 500);
        counts.insert(vec![true], 500);
        let ci = expectation_confidence(&counts, 0, 0.95);
        assert!(
            ci.point_estimate.abs() < 1e-12,
            "expected 0.0, got {}",
            ci.point_estimate
        );
        assert!(ci.lower < 0.0);
        assert!(ci.upper > 0.0);
    }

    #[test]
    fn expectation_multi_qubit() {
        // Two-qubit system: qubit 0 always |0>, qubit 1 always |1>
        let mut counts = HashMap::new();
        counts.insert(vec![false, true], 1000);
        let ci0 = expectation_confidence(&counts, 0, 0.95);
        let ci1 = expectation_confidence(&counts, 1, 0.95);
        assert!((ci0.point_estimate - 1.0).abs() < 1e-12);
        assert!((ci1.point_estimate - (-1.0)).abs() < 1e-12);
    }

    // -----------------------------------------------------------------------
    // Required shots
    // -----------------------------------------------------------------------

    #[test]
    fn required_shots_standard() {
        let n = required_shots(0.01, 0.05);
        // ln(2/0.05) / (2 * 0.01^2) = ln(40) / 0.0002 = 3.6889 / 0.0002 = 18444.7
        assert!(
            (n as i64 - 18445).abs() <= 1,
            "required_shots(0.01, 0.05) = {n}, expected ~18445"
        );
    }

    #[test]
    fn required_shots_loose() {
        let n = required_shots(0.1, 0.1);
        // ln(20) / 0.02 = 2.9957 / 0.02 = 149.79 -> 150
        assert!(n >= 149 && n <= 151, "expected ~150, got {n}");
    }

    // -----------------------------------------------------------------------
    // Total variation distance
    // -----------------------------------------------------------------------

    #[test]
    fn tvd_identical() {
        let mut p = HashMap::new();
        p.insert(vec![false, false], 250);
        p.insert(vec![false, true], 250);
        p.insert(vec![true, false], 250);
        p.insert(vec![true, true], 250);

        let tvd = total_variation_distance(&p, &p);
        assert!(
            tvd.abs() < 1e-12,
            "TVD of identical distributions should be 0, got {tvd}"
        );
    }

    #[test]
    fn tvd_completely_different() {
        let mut p = HashMap::new();
        p.insert(vec![false], 1000);

        let mut q = HashMap::new();
        q.insert(vec![true], 1000);

        let tvd = total_variation_distance(&p, &q);
        assert!(
            (tvd - 1.0).abs() < 1e-12,
            "TVD of completely different distributions should be 1.0, got {tvd}"
        );
    }

    #[test]
    fn tvd_partial_overlap() {
        let mut p = HashMap::new();
        p.insert(vec![false], 600);
        p.insert(vec![true], 400);

        let mut q = HashMap::new();
        q.insert(vec![false], 400);
        q.insert(vec![true], 600);

        let tvd = total_variation_distance(&p, &q);
        // |0.6 - 0.4| + |0.4 - 0.6| = 0.4, times 0.5 = 0.2
        assert!((tvd - 0.2).abs() < 1e-12, "expected 0.2, got {tvd}");
    }

    #[test]
    fn tvd_empty() {
        let p: HashMap<Vec<bool>, usize> = HashMap::new();
        let q: HashMap<Vec<bool>, usize> = HashMap::new();
        let tvd = total_variation_distance(&p, &q);
        assert!(tvd.abs() < 1e-12);
    }

    // -----------------------------------------------------------------------
    // Chi-squared test
    // -----------------------------------------------------------------------

    #[test]
    fn chi_squared_matching() {
        // Observed matches expected perfectly.
        let mut obs = HashMap::new();
        obs.insert(vec![false, false], 250);
        obs.insert(vec![false, true], 250);
        obs.insert(vec![true, false], 250);
        obs.insert(vec![true, true], 250);

        let result = chi_squared_test(&obs, &obs);
        assert!(
            result.statistic < 1e-12,
            "statistic should be ~0 for identical distributions, got {}",
            result.statistic
        );
        assert!(
            result.p_value > 0.05,
            "p-value should be high for matching distributions, got {}",
            result.p_value
        );
        assert!(!result.significant);
    }

    #[test]
    fn chi_squared_very_different() {
        let mut obs = HashMap::new();
        obs.insert(vec![false], 1000);
        obs.insert(vec![true], 0);

        let mut exp = HashMap::new();
        exp.insert(vec![false], 500);
        exp.insert(vec![true], 500);

        let result = chi_squared_test(&obs, &exp);
        assert!(result.statistic > 100.0, "statistic should be large");
        assert!(
            result.p_value < 0.05,
            "p-value should be small: {}",
            result.p_value
        );
        assert!(result.significant);
    }

    #[test]
    fn chi_squared_degrees_of_freedom() {
        let mut obs = HashMap::new();
        obs.insert(vec![false, false], 100);
        obs.insert(vec![false, true], 100);
        obs.insert(vec![true, false], 100);
        obs.insert(vec![true, true], 100);

        let result = chi_squared_test(&obs, &obs);
        assert_eq!(result.degrees_of_freedom, 3);
    }

    // -----------------------------------------------------------------------
    // Convergence monitor
    // -----------------------------------------------------------------------

    #[test]
    fn convergence_detects_stable() {
        let mut monitor = ConvergenceMonitor::new(5);
        // Add a sequence that stabilises.
        for &v in &[
            0.5, 0.52, 0.49, 0.501, 0.499, 0.5001, 0.4999, 0.5002, 0.4998, 0.5001,
        ] {
            monitor.add_estimate(v);
        }
        assert!(
            monitor.has_converged(0.01),
            "should have converged: last 5 values are within 0.01"
        );
    }

    #[test]
    fn convergence_rejects_unstable() {
        let mut monitor = ConvergenceMonitor::new(5);
        for &v in &[0.1, 0.9, 0.1, 0.9, 0.1, 0.9, 0.1, 0.9, 0.1, 0.9] {
            monitor.add_estimate(v);
        }
        assert!(
            !monitor.has_converged(0.01),
            "should NOT have converged: values oscillate widely"
        );
    }

    #[test]
    fn convergence_insufficient_data() {
        let mut monitor = ConvergenceMonitor::new(10);
        monitor.add_estimate(1.0);
        monitor.add_estimate(1.0);
        assert!(
            !monitor.has_converged(0.1),
            "not enough data for window_size=10"
        );
    }

    #[test]
    fn convergence_current_estimate() {
        let mut monitor = ConvergenceMonitor::new(3);
        assert_eq!(monitor.current_estimate(), None);
        monitor.add_estimate(42.0);
        assert_eq!(monitor.current_estimate(), Some(42.0));
        monitor.add_estimate(43.0);
        assert_eq!(monitor.current_estimate(), Some(43.0));
    }

    // -----------------------------------------------------------------------
    // Binomial CDF helper
    // -----------------------------------------------------------------------

    #[test]
    fn binomial_cdf_edge_cases() {
        // P(X <= 10 | 10, 0.5) should be 1.0
        let c = binomial_cdf(10, 10, 0.5);
        assert!((c - 1.0).abs() < 1e-12);

        // P(X <= 0 | 10, 0.5) = (0.5)^10 ~ 0.000977
        let c = binomial_cdf(10, 0, 0.5);
        assert!((c - 0.0009765625).abs() < 1e-8);
    }

    // -----------------------------------------------------------------------
    // Normal CDF helper
    // -----------------------------------------------------------------------

    #[test]
    fn normal_cdf_values() {
        // Phi(0) = 0.5
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-6);

        // Phi(1.96) ~ 0.975
        assert!((normal_cdf(1.96) - 0.975).abs() < 0.002);

        // Phi(-1.96) ~ 0.025
        assert!((normal_cdf(-1.96) - 0.025).abs() < 0.002);
    }
}
