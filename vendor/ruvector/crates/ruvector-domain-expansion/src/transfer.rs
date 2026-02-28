//! Cross-Domain Transfer Engine with Meta Thompson Sampling
//!
//! Transfer happens through priors, not raw memories.
//! Ship compact priors and verified kernels between domains.
//!
//! ## Two-Layer Learning Architecture
//!
//! **Policy learning layer**: Chooses strategies, budgets, and tool paths
//! using uncertainty-aware selection (Thompson Sampling with Beta priors).
//!
//! **Operator layer**: Executes deterministic kernels and graders,
//! logs witnesses, and commits state through gates.
//!
//! ## Meta Thompson Sampling
//!
//! After each cycle, compute posterior summary per bucket and arm.
//! Store as TransferPrior. When a new domain starts, initialize its
//! buckets with these priors instead of uniform, enabling faster adaptation.
//!
//! ## Cross-Domain Transfer Protocol
//!
//! A delta is promotable only if it improves Domain 2 without regressing
//! Domain 1, or improves Domain 1 without regressing Domain 2.
//! That is generalization.

use crate::domain::DomainId;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Beta distribution parameters for Thompson Sampling.
/// Represents uncertainty about an arm's reward probability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetaParams {
    /// Success count + prior (alpha).
    pub alpha: f32,
    /// Failure count + prior (beta).
    pub beta: f32,
}

impl BetaParams {
    /// Uniform (uninformative) prior: Beta(1, 1).
    pub fn uniform() -> Self {
        Self {
            alpha: 1.0,
            beta: 1.0,
        }
    }

    /// Create from observed successes and failures.
    pub fn from_observations(successes: f32, failures: f32) -> Self {
        Self {
            alpha: successes + 1.0,
            beta: failures + 1.0,
        }
    }

    /// Mean of the Beta distribution: E[X] = alpha / (alpha + beta).
    pub fn mean(&self) -> f32 {
        self.alpha / (self.alpha + self.beta)
    }

    /// Variance: measures uncertainty. Lower = more confident.
    pub fn variance(&self) -> f32 {
        let total = self.alpha + self.beta;
        (self.alpha * self.beta) / (total * total * (total + 1.0))
    }

    /// Sample from the Beta distribution using the Kumaraswamy approximation.
    /// Fast, no special functions needed, good enough for Thompson Sampling.
    pub fn sample(&self, rng: &mut impl Rng) -> f32 {
        // Use inverse CDF of Beta via simple approximation
        let u: f32 = rng.gen_range(0.001..0.999);
        // Kumaraswamy approximation: x = (1 - (1 - u^(1/b))^(1/a))
        // Better approximation using ratio of gammas via the normal approach
        let x = Self::beta_inv_approx(u, self.alpha, self.beta);
        x.clamp(0.0, 1.0)
    }

    /// Approximate inverse CDF of Beta distribution.
    fn beta_inv_approx(p: f32, a: f32, b: f32) -> f32 {
        // Use normal approximation for Beta when a,b are not too small
        if a > 1.0 && b > 1.0 {
            let mean = a / (a + b);
            let var = (a * b) / ((a + b) * (a + b) * (a + b + 1.0));
            let std = var.sqrt();
            // Inverse normal approximation (Abramowitz & Stegun)
            let t = if p < 0.5 {
                (-2.0 * (p).ln()).sqrt()
            } else {
                (-2.0 * (1.0 - p).ln()).sqrt()
            };
            let x = if p < 0.5 {
                mean - std * t
            } else {
                mean + std * t
            };
            x.clamp(0.001, 0.999)
        } else {
            // Fallback: simple power approximation
            p.powf(1.0 / a) * (1.0 - (1.0 - p).powf(1.0 / b)) + p.powf(1.0 / a) * 0.5
        }
    }

    /// Update with an observation (Bayesian posterior update).
    pub fn update(&mut self, reward: f32) {
        self.alpha += reward;
        self.beta += 1.0 - reward;
    }

    /// Merge two Beta distributions (approximate: sum parameters).
    pub fn merge(&self, other: &BetaParams) -> BetaParams {
        BetaParams {
            alpha: self.alpha + other.alpha - 1.0, // subtract uniform prior
            beta: self.beta + other.beta - 1.0,
        }
    }
}

/// A context bucket groups similar problem instances for targeted learning.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextBucket {
    /// Difficulty tier: "easy", "medium", "hard".
    pub difficulty_tier: String,
    /// Problem category within the domain.
    pub category: String,
}

/// An arm in the multi-armed bandit: a strategy choice.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArmId(pub String);

/// Transfer prior: compact posterior summary from a source domain.
/// This is what gets shipped between domains — not raw trajectories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferPrior {
    /// Source domain that generated this prior.
    pub source_domain: DomainId,
    /// Per-bucket, per-arm Beta parameters (posterior summaries).
    pub bucket_priors: HashMap<ContextBucket, HashMap<ArmId, BetaParams>>,
    /// Cost EMA (exponential moving average) priors per bucket.
    pub cost_ema_priors: HashMap<ContextBucket, f32>,
    /// Number of cycles this prior was trained on.
    pub training_cycles: u64,
    /// Witness hash: proof of how this prior was derived.
    pub witness_hash: String,
}

impl TransferPrior {
    /// Create an empty (uniform) prior for a domain.
    pub fn uniform(source_domain: DomainId) -> Self {
        Self {
            source_domain,
            bucket_priors: HashMap::new(),
            cost_ema_priors: HashMap::new(),
            training_cycles: 0,
            witness_hash: String::new(),
        }
    }

    /// Get the prior for a specific bucket and arm, defaulting to uniform.
    pub fn get_prior(&self, bucket: &ContextBucket, arm: &ArmId) -> BetaParams {
        self.bucket_priors
            .get(bucket)
            .and_then(|arms| arms.get(arm))
            .cloned()
            .unwrap_or_else(BetaParams::uniform)
    }

    /// Update the posterior for a bucket/arm with a new observation.
    pub fn update_posterior(&mut self, bucket: ContextBucket, arm: ArmId, reward: f32) {
        let arms = self.bucket_priors.entry(bucket.clone()).or_default();
        let params = arms.entry(arm).or_insert_with(BetaParams::uniform);
        params.update(reward);
        self.training_cycles += 1;
    }

    /// Update cost EMA for a bucket.
    pub fn update_cost_ema(&mut self, bucket: ContextBucket, cost: f32, decay: f32) {
        let entry = self.cost_ema_priors.entry(bucket).or_insert(cost);
        *entry = decay * (*entry) + (1.0 - decay) * cost;
    }

    /// Extract a compact summary suitable for shipping to another domain.
    pub fn extract_summary(&self) -> TransferPrior {
        // Only ship buckets with sufficient evidence (>10 observations)
        let filtered: HashMap<ContextBucket, HashMap<ArmId, BetaParams>> = self
            .bucket_priors
            .iter()
            .filter_map(|(bucket, arms)| {
                let significant_arms: HashMap<ArmId, BetaParams> = arms
                    .iter()
                    .filter(|(_, params)| (params.alpha + params.beta) > 12.0)
                    .map(|(arm, params)| (arm.clone(), params.clone()))
                    .collect();
                if significant_arms.is_empty() {
                    None
                } else {
                    Some((bucket.clone(), significant_arms))
                }
            })
            .collect();

        TransferPrior {
            source_domain: self.source_domain.clone(),
            bucket_priors: filtered,
            cost_ema_priors: self.cost_ema_priors.clone(),
            training_cycles: self.training_cycles,
            witness_hash: self.witness_hash.clone(),
        }
    }
}

/// Meta Thompson Sampling engine that manages priors across domains.
pub struct MetaThompsonEngine {
    /// Active priors per domain.
    domain_priors: HashMap<DomainId, TransferPrior>,
    /// Available arms (strategies) shared across domains.
    arms: Vec<ArmId>,
    /// Difficulty tiers for bucketing.
    difficulty_tiers: Vec<String>,
}

impl MetaThompsonEngine {
    /// Create a new engine with the given strategy arms.
    pub fn new(arms: Vec<String>) -> Self {
        Self {
            domain_priors: HashMap::new(),
            arms: arms.into_iter().map(ArmId).collect(),
            difficulty_tiers: vec!["easy".into(), "medium".into(), "hard".into()],
        }
    }

    /// Initialize a domain with uniform priors.
    pub fn init_domain_uniform(&mut self, domain_id: DomainId) {
        self.domain_priors
            .insert(domain_id.clone(), TransferPrior::uniform(domain_id));
    }

    /// Initialize a domain using transfer priors from a source domain.
    /// This is the key mechanism: Meta-TS seeds new domains with learned priors.
    pub fn init_domain_with_transfer(
        &mut self,
        target_domain: DomainId,
        source_prior: &TransferPrior,
    ) {
        let mut prior = TransferPrior::uniform(target_domain.clone());

        // Copy bucket priors from source, scaling by confidence
        for (bucket, arms) in &source_prior.bucket_priors {
            for (arm, params) in arms {
                // Dampen the prior: don't fully trust cross-domain evidence.
                // Use sqrt scaling: reduces confidence while preserving mean.
                let dampened = BetaParams {
                    alpha: 1.0 + (params.alpha - 1.0).sqrt(),
                    beta: 1.0 + (params.beta - 1.0).sqrt(),
                };
                prior
                    .bucket_priors
                    .entry(bucket.clone())
                    .or_default()
                    .insert(arm.clone(), dampened);
            }
        }

        // Transfer cost EMAs with dampening
        for (bucket, &cost) in &source_prior.cost_ema_priors {
            prior.cost_ema_priors.insert(bucket.clone(), cost * 1.5); // pessimistic transfer
        }

        prior.witness_hash = format!("transfer_from_{}", source_prior.source_domain);
        self.domain_priors.insert(target_domain, prior);
    }

    /// Select an arm for a given domain and context using Thompson Sampling.
    pub fn select_arm(
        &self,
        domain_id: &DomainId,
        bucket: &ContextBucket,
        rng: &mut impl Rng,
    ) -> Option<ArmId> {
        let prior = self.domain_priors.get(domain_id)?;

        let mut best_arm = None;
        let mut best_sample = f32::NEG_INFINITY;

        for arm in &self.arms {
            let params = prior.get_prior(bucket, arm);
            let sample = params.sample(rng);
            if sample > best_sample {
                best_sample = sample;
                best_arm = Some(arm.clone());
            }
        }

        best_arm
    }

    /// Record the outcome of using an arm in a domain.
    pub fn record_outcome(
        &mut self,
        domain_id: &DomainId,
        bucket: ContextBucket,
        arm: ArmId,
        reward: f32,
        cost: f32,
    ) {
        if let Some(prior) = self.domain_priors.get_mut(domain_id) {
            prior.update_posterior(bucket.clone(), arm, reward);
            prior.update_cost_ema(bucket, cost, 0.9);
        }
    }

    /// Extract transfer prior from a domain (for shipping to another domain).
    pub fn extract_prior(&self, domain_id: &DomainId) -> Option<TransferPrior> {
        self.domain_priors
            .get(domain_id)
            .map(|p| p.extract_summary())
    }

    /// Get all domain IDs currently tracked.
    pub fn domain_ids(&self) -> Vec<&DomainId> {
        self.domain_priors.keys().collect()
    }

    /// Check if posterior variance is high (triggers speculative dual-path).
    pub fn is_uncertain(
        &self,
        domain_id: &DomainId,
        bucket: &ContextBucket,
        threshold: f32,
    ) -> bool {
        let prior = match self.domain_priors.get(domain_id) {
            Some(p) => p,
            None => return true, // No data = maximum uncertainty
        };

        // Check if top two arms are within delta of each other
        let mut samples: Vec<(f32, &ArmId)> = self
            .arms
            .iter()
            .map(|arm| {
                let params = prior.get_prior(bucket, arm);
                (params.mean(), arm)
            })
            .collect();
        samples.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        if samples.len() < 2 {
            return true;
        }

        let gap = samples[0].0 - samples[1].0;
        gap < threshold
    }
}

/// Speculative dual-path execution for high-uncertainty decisions.
/// When the top two arms are within delta, run both and pick the winner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualPathResult {
    /// Primary arm and its outcome.
    pub primary: (ArmId, f32),
    /// Secondary arm and its outcome.
    pub secondary: (ArmId, f32),
    /// Which arm won.
    pub winner: ArmId,
    /// The loser becomes a counterexample for that context.
    pub counterexample: ArmId,
}

/// Cross-domain transfer verification result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferVerification {
    /// Source domain.
    pub source: DomainId,
    /// Target domain.
    pub target: DomainId,
    /// Did transfer improve the target domain?
    pub improved_target: bool,
    /// Did transfer regress the source domain?
    pub regressed_source: bool,
    /// Is this delta promotable? (improved target AND not regressed source).
    pub promotable: bool,
    /// Acceleration factor: ratio of convergence speeds.
    pub acceleration_factor: f32,
    /// Source score before/after.
    pub source_scores: (f32, f32),
    /// Target score before/after.
    pub target_scores: (f32, f32),
}

impl TransferVerification {
    /// Verify a transfer delta against the generalization rule:
    /// promotable iff it improves Domain 2 without regressing Domain 1.
    pub fn verify(
        source: DomainId,
        target: DomainId,
        source_before: f32,
        source_after: f32,
        target_before: f32,
        target_after: f32,
        target_baseline_cycles: u64,
        target_transfer_cycles: u64,
    ) -> Self {
        let improved_target = target_after > target_before;
        let regressed_source = source_after < source_before - 0.01; // small tolerance

        let promotable = improved_target && !regressed_source;

        // Acceleration = baseline_cycles / transfer_cycles (higher = better transfer)
        let acceleration_factor = if target_transfer_cycles > 0 {
            target_baseline_cycles as f32 / target_transfer_cycles as f32
        } else {
            1.0
        };

        Self {
            source,
            target,
            improved_target,
            regressed_source,
            promotable,
            acceleration_factor,
            source_scores: (source_before, source_after),
            target_scores: (target_before, target_after),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_beta_params_uniform() {
        let p = BetaParams::uniform();
        assert_eq!(p.alpha, 1.0);
        assert_eq!(p.beta, 1.0);
        assert!((p.mean() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_beta_params_update() {
        let mut p = BetaParams::uniform();
        p.update(1.0); // success
        assert_eq!(p.alpha, 2.0);
        assert_eq!(p.beta, 1.0);
        assert!(p.mean() > 0.5);
    }

    #[test]
    fn test_beta_params_sample_in_range() {
        let p = BetaParams::from_observations(10.0, 5.0);
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let s = p.sample(&mut rng);
            assert!(s >= 0.0 && s <= 1.0, "Sample {} out of [0,1]", s);
        }
    }

    #[test]
    fn test_transfer_prior_round_trip() {
        let domain = DomainId("test".into());
        let mut prior = TransferPrior::uniform(domain);

        let bucket = ContextBucket {
            difficulty_tier: "easy".into(),
            category: "transform".into(),
        };
        let arm = ArmId("strategy_a".into());

        for _ in 0..20 {
            prior.update_posterior(bucket.clone(), arm.clone(), 0.8);
        }

        let summary = prior.extract_summary();
        assert!(!summary.bucket_priors.is_empty());

        let retrieved = summary.get_prior(&bucket, &arm);
        assert!(retrieved.mean() > 0.5);
    }

    #[test]
    fn test_meta_thompson_engine() {
        let mut engine = MetaThompsonEngine::new(vec![
            "strategy_a".into(),
            "strategy_b".into(),
            "strategy_c".into(),
        ]);

        let domain1 = DomainId("rust_synthesis".into());
        engine.init_domain_uniform(domain1.clone());

        let bucket = ContextBucket {
            difficulty_tier: "medium".into(),
            category: "algorithm".into(),
        };

        let mut rng = rand::thread_rng();

        // Record some outcomes
        for _ in 0..50 {
            let arm = engine.select_arm(&domain1, &bucket, &mut rng).unwrap();
            let reward = if arm.0 == "strategy_a" { 0.9 } else { 0.3 };
            engine.record_outcome(&domain1, bucket.clone(), arm, reward, 1.0);
        }

        // Extract prior and transfer to domain2
        let prior = engine.extract_prior(&domain1).unwrap();
        let domain2 = DomainId("planning".into());
        engine.init_domain_with_transfer(domain2.clone(), &prior);

        // Domain2 should now have informative priors
        let d2_prior = engine.domain_priors.get(&domain2).unwrap();
        let a_params = d2_prior.get_prior(&bucket, &ArmId("strategy_a".into()));
        assert!(
            a_params.mean() > 0.5,
            "Transferred prior should favor strategy_a"
        );
    }

    #[test]
    fn test_transfer_verification() {
        let v = TransferVerification::verify(
            DomainId("d1".into()),
            DomainId("d2".into()),
            0.8,  // source before
            0.79, // source after (slight decrease, within tolerance)
            0.3,  // target before
            0.7,  // target after (big improvement)
            100,  // baseline cycles
            40,   // transfer cycles
        );

        assert!(v.improved_target);
        assert!(!v.regressed_source); // within tolerance
        assert!(v.promotable);
        assert!((v.acceleration_factor - 2.5).abs() < 1e-4);
    }

    #[test]
    fn test_transfer_not_promotable_on_regression() {
        let v = TransferVerification::verify(
            DomainId("d1".into()),
            DomainId("d2".into()),
            0.8, // source before
            0.5, // source after (regression!)
            0.3, // target before
            0.7, // target after
            100,
            40,
        );

        assert!(v.improved_target);
        assert!(v.regressed_source);
        assert!(!v.promotable);
    }

    #[test]
    fn test_uncertainty_detection() {
        let mut engine = MetaThompsonEngine::new(vec!["a".into(), "b".into()]);

        let domain = DomainId("test".into());
        engine.init_domain_uniform(domain.clone());

        let bucket = ContextBucket {
            difficulty_tier: "easy".into(),
            category: "test".into(),
        };

        // With uniform priors, should be uncertain
        assert!(engine.is_uncertain(&domain, &bucket, 0.1));

        // After many observations favoring one arm, should be certain
        for _ in 0..100 {
            engine.record_outcome(&domain, bucket.clone(), ArmId("a".into()), 0.95, 1.0);
            engine.record_outcome(&domain, bucket.clone(), ArmId("b".into()), 0.1, 1.0);
        }

        assert!(!engine.is_uncertain(&domain, &bucket, 0.1));
    }
}
