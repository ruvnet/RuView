# Feature 18: Adversarial Robustness Layer (ARL)

## Overview

### Problem Statement
GNN attention mechanisms are vulnerable to adversarial attacks where malicious actors craft query perturbations to manipulate retrieval results, extract sensitive information, or cause denial of service. Traditional GNNs lack built-in defenses against query poisoning, membership inference attacks, and adversarial examples. Production systems need robust security mechanisms to detect and resist these attacks.

### Proposed Solution
The Adversarial Robustness Layer (ARL) implements a multi-layered defense system that detects anomalous queries, applies defensive projections to sanitize inputs, and logs attacks for analysis. The system uses anomaly detection, input validation, certified defenses, and adaptive hardening to protect against both known and unknown attack vectors.

### Expected Benefits
- **Attack Detection**: 90-95% detection rate for known attack patterns
- **Robustness**: 60-80% reduction in attack success rate
- **Zero-Day Defense**: Detect novel attacks via anomaly detection
- **Auditability**: Complete attack logging and forensics
- **Minimal False Positives**: <5% false positive rate on benign queries
- **Performance**: <10% latency overhead for defense mechanisms

### Novelty Claim
**Unique Contribution**: First GNN attention system with integrated multi-layered adversarial defense including certified robustness guarantees, anomaly detection, defensive distillation, and attack attribution. Unlike post-hoc defenses or adversarial training alone, ARL provides defense-in-depth with formal security guarantees.

**Differentiators**:
1. Multi-layered defense architecture (detection, projection, verification)
2. Certified robustness bounds via randomized smoothing
3. Adaptive defense that learns from attack patterns
4. Attack attribution and forensics
5. Minimal performance impact on benign queries

## Technical Design

### Architecture Diagram

```
                    Input Query (q)
                         |
         +---------------+--------------+
         |                              |
    Fast Path                      Suspicious?
    (benign)                             |
         |                               v
         |                    ┌──────────────────────┐
         |                    │  Anomaly Detection   │
         |                    │  - Statistical       │
         |                    │  - ML-based          │
         |                    │  - Pattern matching  │
         |                    └──────┬───────────────┘
         |                           |
         |                    Anomaly Score > θ?
         |                           |
         |                     +-----+-----+
         |                     |           |
         |                    Yes         No
         |                     |           |
         |                     v           |
         |            ┌─────────────────┐  |
         |            │ Defense Layer   │  |
         |            │                 │  |
         |            │ 1. Input        │  |
         |            │    Validation   │  |
         |            │                 │  |
         |            │ 2. Defensive    │  |
         |            │    Projection   │  |
         |            │                 │  |
         |            │ 3. Certified    │  |
         |            │    Smoothing    │  |
         |            │                 │  |
         |            │ 4. Sanitization │  |
         |            └────┬────────────┘  |
         |                 |               |
         |                 v               |
         |          Sanitized Query        |
         |                 |               |
         +--------+--------+---------------+
                  |
                  v
         ┌────────────────────┐
         │  Verification      │
         │  - Range check     │
         │  - Norm check      │
         │  - Semantics check │
         └────────┬───────────┘
                  |
            Valid? |
                  |
         +--------+--------+
         |                 |
        Yes               No
         |                 |
         v                 v
    Proceed          Reject + Log
         |                 |
         v                 v
    GNN Attention    ┌──────────────┐
         |           │ Attack Logger│
         |           │ - Timestamp  │
         |           │ - Pattern    │
         |           │ - Attribution│
         |           └──────────────┘
         v
    Results
         |
         v
    ┌──────────────────────┐
    │ Post-processing      │
    │ - Output validation  │
    │ - Information hiding │
    │ - Rate limiting      │
    └──────────────────────┘


Defense Layers Detail:

┌─────────────────────────────────────────┐
│         Anomaly Detection                │
│                                          │
│  ┌────────────────┐  ┌───────────────┐ │
│  │ Statistical    │  │ ML-based      │ │
│  │ - Norm > θ     │  │ - Autoencoder │ │
│  │ - Sparsity     │  │ - One-class   │ │
│  │ - Entropy      │  │   SVM         │ │
│  └────────────────┘  └───────────────┘ │
│           |                  |          │
│           +--------+---------+          │
│                    |                    │
│              Anomaly Score              │
│                    |                    │
│         High > θ_high -> Reject         │
│         Med > θ_med -> Defend           │
│         Low < θ_med -> Pass             │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│      Defensive Projection                │
│                                          │
│  Original Query (q)                      │
│         |                                │
│         v                                │
│  ┌──────────────┐                       │
│  │ Project to   │                       │
│  │ Safe Subspace│                       │
│  │              │                       │
│  │ q' = P(q)    │                       │
│  │              │                       │
│  │ where P      │                       │
│  │ removes      │                       │
│  │ adversarial  │                       │
│  │ components   │                       │
│  └──────┬───────┘                       │
│         |                                │
│         v                                │
│  Sanitized Query                         │
└─────────────────────────────────────────┘

┌─────────────────────────────────────────┐
│    Certified Robustness                  │
│    (Randomized Smoothing)                │
│                                          │
│  Sanitized Query (q')                    │
│         |                                │
│         v                                │
│  Sample N perturbations                  │
│  q'_i = q' + σ·ε_i, ε_i ~ N(0, I)      │
│         |                                │
│         v                                │
│  Run GNN on all samples                  │
│  results_i = GNN(q'_i)                  │
│         |                                │
│         v                                │
│  Majority vote / Average                 │
│         |                                │
│         v                                │
│  Certified Result                        │
│  (provably robust to ||δ|| < R)         │
└─────────────────────────────────────────┘
```

### Core Data Structures

```rust
/// Configuration for Adversarial Robustness Layer
#[derive(Debug, Clone)]
pub struct ARLConfig {
    /// Enable anomaly detection
    pub enable_anomaly_detection: bool,

    /// Anomaly detection threshold (0.0 - 1.0)
    pub anomaly_threshold: f32,

    /// High threshold for immediate rejection
    pub high_anomaly_threshold: f32,

    /// Enable defensive projection
    pub enable_defensive_projection: bool,

    /// Enable certified robustness (expensive)
    pub enable_certified_robustness: bool,

    /// Number of samples for randomized smoothing
    pub smoothing_samples: usize,

    /// Noise level for randomized smoothing
    pub smoothing_sigma: f32,

    /// Enable attack logging
    pub enable_logging: bool,

    /// Enable rate limiting
    pub enable_rate_limiting: bool,

    /// Maximum queries per second per user
    pub max_qps_per_user: usize,

    /// Adaptive defense (learn from attacks)
    pub adaptive: bool,
}

/// Anomaly detector trait
pub trait AnomalyDetector: Send + Sync {
    /// Compute anomaly score (0.0 = normal, 1.0 = highly anomalous)
    fn score(&self, query: &[f32]) -> f32;

    /// Update detector with new data (online learning)
    fn update(&mut self, query: &[f32], is_anomaly: bool);

    /// Get detector type
    fn detector_type(&self) -> DetectorType;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetectorType {
    Statistical,
    Autoencoder,
    OneClassSVM,
    IsolationForest,
    Ensemble,
}

/// Statistical anomaly detector
#[derive(Debug)]
pub struct StatisticalDetector {
    /// Expected mean vector
    mean: Array1<f32>,

    /// Expected covariance matrix
    covariance: Array2<f32>,

    /// Mahalanobis distance threshold
    threshold: f32,

    /// Running statistics for online updates
    running_mean: Array1<f32>,
    running_var: Array1<f32>,
    n_samples: usize,
}

impl AnomalyDetector for StatisticalDetector {
    fn score(&self, query: &[f32]) -> f32 {
        // Compute Mahalanobis distance
        let q = Array1::from_vec(query.to_vec());
        let diff = &q - &self.mean;

        // M^2 = (x - μ)^T Σ^(-1) (x - μ)
        let inv_cov = self.covariance.inv().unwrap_or_else(|_| Array2::eye(q.len()));
        let mahalanobis = diff.dot(&inv_cov.dot(&diff)).sqrt();

        // Normalize to 0-1 range
        (mahalanobis / self.threshold).min(1.0)
    }

    fn update(&mut self, query: &[f32], _is_anomaly: bool) {
        // Update running statistics
        let q = Array1::from_vec(query.to_vec());
        self.n_samples += 1;
        let n = self.n_samples as f32;

        // Update mean: μ_n = μ_{n-1} + (x_n - μ_{n-1}) / n
        let delta = &q - &self.running_mean;
        self.running_mean = &self.running_mean + &(&delta / n);

        // Update variance
        let delta2 = &q - &self.running_mean;
        self.running_var = &self.running_var + &(&delta * &delta2);
    }

    fn detector_type(&self) -> DetectorType {
        DetectorType::Statistical
    }
}

/// Autoencoder-based anomaly detector
#[derive(Debug)]
pub struct AutoencoderDetector {
    /// Encoder network
    encoder: Vec<DenseLayer>,

    /// Decoder network
    decoder: Vec<DenseLayer>,

    /// Latent dimension
    latent_dim: usize,

    /// Reconstruction error threshold
    threshold: f32,

    /// Optimizer for online learning
    optimizer: Option<AdamOptimizer>,
}

impl AnomalyDetector for AutoencoderDetector {
    fn score(&self, query: &[f32]) -> f32 {
        // Forward through encoder-decoder
        let input = Array1::from_vec(query.to_vec());
        let mut hidden = input.clone();

        // Encode
        for layer in &self.encoder {
            hidden = layer.forward(&hidden);
            hidden = relu(&hidden);
        }

        // Decode
        for layer in &self.decoder {
            hidden = layer.forward(&hidden);
            hidden = relu(&hidden);
        }

        let reconstruction = hidden;

        // Compute reconstruction error
        let error = (&input - &reconstruction).mapv(|x| x * x).sum().sqrt();

        // Normalize
        (error / self.threshold).min(1.0)
    }

    fn update(&mut self, query: &[f32], is_anomaly: bool) {
        if is_anomaly {
            return; // Don't train on anomalies
        }

        if let Some(ref mut opt) = self.optimizer {
            // Train autoencoder on normal data
            let input = Array1::from_vec(query.to_vec());
            let loss = self.compute_reconstruction_loss(&input);
            let grads = self.compute_gradients(&input);
            self.apply_gradients(grads, opt);
        }
    }

    fn detector_type(&self) -> DetectorType {
        DetectorType::Autoencoder
    }
}

/// Ensemble anomaly detector
#[derive(Debug)]
pub struct EnsembleDetector {
    /// Component detectors
    detectors: Vec<Box<dyn AnomalyDetector>>,

    /// Detector weights (learned)
    weights: Vec<f32>,

    /// Aggregation strategy
    strategy: AggregationStrategy,
}

#[derive(Debug, Clone, Copy)]
pub enum AggregationStrategy {
    /// Average of scores
    Average,

    /// Maximum score (most pessimistic)
    Maximum,

    /// Weighted average
    Weighted,

    /// Majority voting
    MajorityVote,
}

impl AnomalyDetector for EnsembleDetector {
    fn score(&self, query: &[f32]) -> f32 {
        let scores: Vec<f32> = self.detectors.iter()
            .map(|d| d.score(query))
            .collect();

        match self.strategy {
            AggregationStrategy::Average => {
                scores.iter().sum::<f32>() / scores.len() as f32
            },
            AggregationStrategy::Maximum => {
                scores.iter().copied().fold(0.0, f32::max)
            },
            AggregationStrategy::Weighted => {
                scores.iter().zip(&self.weights)
                    .map(|(s, w)| s * w)
                    .sum()
            },
            AggregationStrategy::MajorityVote => {
                let threshold = 0.5;
                let votes = scores.iter().filter(|&&s| s > threshold).count();
                votes as f32 / scores.len() as f32
            }
        }
    }

    fn update(&mut self, query: &[f32], is_anomaly: bool) {
        for detector in &mut self.detectors {
            detector.update(query, is_anomaly);
        }
    }

    fn detector_type(&self) -> DetectorType {
        DetectorType::Ensemble
    }
}

/// Defensive projection to sanitize queries
#[derive(Debug)]
pub struct DefensiveProjection {
    /// Projection matrix to safe subspace
    projection_matrix: Array2<f32>,

    /// Safe subspace dimension
    safe_dim: usize,

    /// Original dimension
    original_dim: usize,

    /// Clip values to range
    clip_range: Option<(f32, f32)>,
}

impl DefensiveProjection {
    /// Project query to safe subspace
    fn project(&self, query: &[f32]) -> Vec<f32> {
        let q = Array1::from_vec(query.to_vec());

        // Project to safe subspace
        let projected = self.projection_matrix.dot(&q);

        // Reconstruct in original space
        let reconstructed = self.projection_matrix.t().dot(&projected);

        // Clip if necessary
        let mut result = reconstructed.to_vec();
        if let Some((min, max)) = self.clip_range {
            for val in &mut result {
                *val = val.max(min).min(max);
            }
        }

        result
    }

    /// Compute projection matrix via PCA on normal queries
    fn fit(&mut self, normal_queries: &[Vec<f32>]) {
        // Compute covariance matrix
        let n = normal_queries.len();
        let d = normal_queries[0].len();

        let mut data_matrix = Array2::zeros((n, d));
        for (i, query) in normal_queries.iter().enumerate() {
            for (j, &val) in query.iter().enumerate() {
                data_matrix[[i, j]] = val;
            }
        }

        // Center data
        let mean = data_matrix.mean_axis(Axis(0)).unwrap();
        let centered = &data_matrix - &mean.insert_axis(Axis(0));

        // Compute covariance
        let cov = centered.t().dot(&centered) / (n - 1) as f32;

        // Eigen decomposition
        let (eigenvalues, eigenvectors) = cov.eig().unwrap();

        // Select top-k eigenvectors
        let mut indexed_eigenvalues: Vec<(usize, f32)> = eigenvalues
            .iter()
            .enumerate()
            .map(|(i, &val)| (i, val))
            .collect();
        indexed_eigenvalues.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let top_k_indices: Vec<usize> = indexed_eigenvalues
            .iter()
            .take(self.safe_dim)
            .map(|&(i, _)| i)
            .collect();

        // Construct projection matrix
        let mut projection = Array2::zeros((self.safe_dim, d));
        for (i, &idx) in top_k_indices.iter().enumerate() {
            projection.row_mut(i).assign(&eigenvectors.column(idx));
        }

        self.projection_matrix = projection;
    }
}

/// Certified robustness via randomized smoothing
#[derive(Debug)]
pub struct CertifiedSmoothing {
    /// Number of samples for Monte Carlo
    num_samples: usize,

    /// Gaussian noise standard deviation
    sigma: f32,

    /// Confidence level (e.g., 0.95)
    confidence: f32,

    /// Random number generator
    rng: StdRng,
}

impl CertifiedSmoothing {
    /// Smooth GNN prediction with certified robustness
    fn smooth_prediction(
        &mut self,
        query: &[f32],
        gnn: &mut dyn AttentionLayer,
        k: usize
    ) -> (Vec<usize>, Vec<f32>, f32) {

        let mut vote_counts: HashMap<usize, usize> = HashMap::new();

        // Sample perturbations
        for _ in 0..self.num_samples {
            // Add Gaussian noise
            let mut perturbed = query.to_vec();
            for val in &mut perturbed {
                let noise: f32 = self.rng.sample(StandardNormal);
                *val += self.sigma * noise;
            }

            // Run GNN on perturbed query
            let (indices, _) = gnn.forward(&perturbed, k).unwrap();

            // Count votes for each index
            for idx in indices {
                *vote_counts.entry(idx).or_insert(0) += 1;
            }
        }

        // Select top-k by vote count
        let mut sorted_votes: Vec<(usize, usize)> = vote_counts.into_iter().collect();
        sorted_votes.sort_by(|a, b| b.1.cmp(&a.1));
        sorted_votes.truncate(k);

        let top_indices: Vec<usize> = sorted_votes.iter().map(|&(idx, _)| idx).collect();
        let vote_scores: Vec<f32> = sorted_votes.iter()
            .map(|&(_, count)| count as f32 / self.num_samples as f32)
            .collect();

        // Compute certified radius
        let max_votes = sorted_votes[0].1;
        let p_max = max_votes as f32 / self.num_samples as f32;
        let certified_radius = self.sigma * (2.0 * p_max - 1.0).sqrt();

        (top_indices, vote_scores, certified_radius)
    }
}

/// Attack pattern tracker
#[derive(Debug, Clone)]
pub struct AttackPattern {
    /// Attack type
    pub attack_type: AttackType,

    /// Timestamp
    pub timestamp: std::time::SystemTime,

    /// Query that triggered detection
    pub query_hash: u64,

    /// Anomaly score
    pub anomaly_score: f32,

    /// Source information (IP, user ID, etc.)
    pub source: SourceInfo,

    /// Attack characteristics
    pub characteristics: AttackCharacteristics,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttackType {
    /// Query perturbation to manipulate results
    QueryPoisoning,

    /// Trying to infer if data point is in training set
    MembershipInference,

    /// Extracting model parameters
    ModelExtraction,

    /// Denial of service via expensive queries
    DoS,

    /// Unknown/novel attack
    Unknown,
}

#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub user_id: Option<String>,
    pub ip_address: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AttackCharacteristics {
    /// Query norm
    pub query_norm: f32,

    /// Query sparsity
    pub sparsity: f32,

    /// Similarity to known attacks
    pub attack_similarity: f32,

    /// Rate of queries
    pub query_rate: f32,
}

/// Attack logger
#[derive(Debug)]
pub struct AttackLogger {
    /// Recent attacks
    attacks: Vec<AttackPattern>,

    /// Maximum log size
    max_size: usize,

    /// Attack statistics
    stats: AttackStats,

    /// Alert thresholds
    alert_threshold: AlertConfig,
}

#[derive(Debug, Default)]
pub struct AttackStats {
    pub total_attacks: usize,
    pub attacks_by_type: HashMap<AttackType, usize>,
    pub attacks_by_source: HashMap<String, usize>,
    pub false_positives: usize,
}

#[derive(Debug, Clone)]
pub struct AlertConfig {
    /// Alert if >N attacks in time window
    pub attack_count_threshold: usize,
    pub time_window_secs: u64,

    /// Alert if attack rate > threshold
    pub attack_rate_threshold: f32,
}

/// Main Adversarial Robustness Layer
pub struct AdversarialRobustnessLayer {
    /// Configuration
    config: ARLConfig,

    /// Anomaly detectors
    detectors: EnsembleDetector,

    /// Defensive projection
    projection: DefensiveProjection,

    /// Certified smoothing
    smoothing: Option<CertifiedSmoothing>,

    /// Attack logger
    logger: Arc<RwLock<AttackLogger>>,

    /// Rate limiter
    rate_limiter: Arc<RwLock<RateLimiter>>,

    /// Metrics
    metrics: Arc<RwLock<ARLMetrics>>,

    /// Underlying GNN attention
    attention: Box<dyn AttentionLayer>,
}

#[derive(Debug, Default)]
pub struct ARLMetrics {
    pub total_queries: usize,
    pub anomalous_queries: usize,
    pub rejected_queries: usize,
    pub sanitized_queries: usize,
    pub certified_queries: usize,
    pub false_positives: usize,
    pub avg_anomaly_score: f32,
    pub avg_defense_latency_ms: f32,
}

#[derive(Debug)]
pub struct RateLimiter {
    /// Query counts per user
    user_counts: HashMap<String, VecDeque<std::time::Instant>>,

    /// Time window for rate limiting
    window_secs: u64,

    /// Maximum queries per window
    max_queries: usize,
}
```

### Key Algorithms

#### 1. Main Defense Pipeline

```rust
/// Forward pass with adversarial defense
async fn forward_with_defense(
    &mut self,
    query: &[f32],
    k: usize,
    source: &SourceInfo
) -> Result<(Vec<usize>, Vec<f32>), ARLError> {

    let start_time = Instant::now();

    // Step 1: Rate limiting check
    if self.config.enable_rate_limiting {
        let mut rate_limiter = self.rate_limiter.write().await;
        if !rate_limiter.check_rate_limit(source) {
            self.log_attack(AttackType::DoS, query, source, 1.0).await;
            return Err(ARLError::RateLimitExceeded);
        }
    }

    // Step 2: Anomaly detection
    let anomaly_score = if self.config.enable_anomaly_detection {
        self.detectors.score(query)
    } else {
        0.0
    };

    // Step 3: Decision based on anomaly score
    let sanitized_query = if anomaly_score > self.config.high_anomaly_threshold {
        // High anomaly: reject immediately
        self.log_attack(AttackType::Unknown, query, source, anomaly_score).await;
        return Err(ARLError::MaliciousQuery { score: anomaly_score });

    } else if anomaly_score > self.config.anomaly_threshold {
        // Medium anomaly: sanitize
        if self.config.enable_defensive_projection {
            self.projection.project(query)
        } else {
            query.to_vec()
        }
    } else {
        // Low anomaly: pass through
        query.to_vec()
    };

    // Step 4: Input validation
    self.validate_input(&sanitized_query)?;

    // Step 5: Run attention with defense
    let (indices, scores) = if self.config.enable_certified_robustness && anomaly_score > 0.3 {
        // Use certified robustness for suspicious queries
        let mut smoothing = self.smoothing.as_mut().unwrap();
        let (idx, sc, radius) = smoothing.smooth_prediction(
            &sanitized_query,
            self.attention.as_mut(),
            k
        );

        // Update metrics
        self.metrics.write().await.certified_queries += 1;

        (idx, sc)
    } else {
        // Normal attention
        self.attention.forward(&sanitized_query, k)?
    };

    // Step 6: Output validation
    self.validate_output(&indices, &scores)?;

    // Step 7: Update metrics
    let defense_latency = start_time.elapsed();
    self.update_metrics(anomaly_score, defense_latency).await;

    // Step 8: Online learning update
    if self.config.adaptive {
        // Assume benign if no alerts triggered
        self.detectors.update(&sanitized_query, false);
    }

    Ok((indices, scores))
}

/// Validate input query
fn validate_input(&self, query: &[f32]) -> Result<(), ARLError> {
    // Check dimension
    if query.len() != self.config.expected_dim {
        return Err(ARLError::InvalidDimension {
            expected: self.config.expected_dim,
            actual: query.len(),
        });
    }

    // Check for NaN/Inf
    if query.iter().any(|&x| !x.is_finite()) {
        return Err(ARLError::InvalidValues);
    }

    // Check norm
    let norm: f32 = query.iter().map(|&x| x * x).sum::<f32>().sqrt();
    if norm > self.config.max_norm {
        return Err(ARLError::NormTooLarge { norm });
    }

    Ok(())
}

/// Validate output
fn validate_output(&self, indices: &[usize], scores: &[f32]) -> Result<(), ARLError> {
    // Check for valid indices
    if indices.iter().any(|&idx| idx >= self.config.max_candidates) {
        return Err(ARLError::InvalidOutput);
    }

    // Check for valid scores
    if scores.iter().any(|&s| !s.is_finite() || s < 0.0) {
        return Err(ARLError::InvalidOutput);
    }

    Ok(())
}

/// Log detected attack
async fn log_attack(
    &self,
    attack_type: AttackType,
    query: &[f32],
    source: &SourceInfo,
    anomaly_score: f32
) {
    let pattern = AttackPattern {
        attack_type,
        timestamp: SystemTime::now(),
        query_hash: hash_query(query),
        anomaly_score,
        source: source.clone(),
        characteristics: AttackCharacteristics {
            query_norm: compute_norm(query),
            sparsity: compute_sparsity(query),
            attack_similarity: 0.0,  // TODO: compute
            query_rate: 0.0,  // TODO: compute
        },
    };

    let mut logger = self.logger.write().await;
    logger.log_attack(pattern);

    // Check alert thresholds
    if logger.should_alert() {
        self.send_alert(&logger.stats).await;
    }
}
```

#### 2. Attack Pattern Classification

```rust
/// Classify attack type based on query characteristics
fn classify_attack(
    query: &[f32],
    anomaly_score: f32,
    characteristics: &AttackCharacteristics
) -> AttackType {

    // High query rate -> DoS
    if characteristics.query_rate > 100.0 {
        return AttackType::DoS;
    }

    // Very high norm -> Query poisoning
    if characteristics.query_norm > 10.0 {
        return AttackType::QueryPoisoning;
    }

    // High sparsity + targeted queries -> Membership inference
    if characteristics.sparsity > 0.9 && characteristics.attack_similarity > 0.7 {
        return AttackType::MembershipInference;
    }

    // Systematic probing -> Model extraction
    // (would need session-level analysis)

    AttackType::Unknown
}
```

#### 3. Adaptive Defense Learning

```rust
/// Update defense based on labeled attack/benign data
async fn adaptive_update(
    &mut self,
    query: &[f32],
    is_attack: bool,
    attack_type: Option<AttackType>
) {
    // Update anomaly detectors
    self.detectors.update(query, is_attack);

    // Update defensive projection if attack
    if is_attack {
        // Add to attack examples
        self.projection.add_attack_example(query);

        // Recompute safe subspace
        if self.projection.attack_examples.len() % 100 == 0 {
            self.projection.recompute_safe_subspace();
        }
    }

    // Update attack logger
    if let Some(atype) = attack_type {
        let mut logger = self.logger.write().await;
        logger.stats.attacks_by_type.entry(atype)
            .and_modify(|c| *c += 1)
            .or_insert(1);
    }
}
```

### API Design

```rust
/// Public API for Adversarial Robustness Layer
pub trait ARLLayer {
    /// Create new ARL
    fn new(
        config: ARLConfig,
        attention: Box<dyn AttentionLayer>
    ) -> Self;

    /// Forward with defense
    async fn forward(
        &mut self,
        query: &[f32],
        k: usize,
        source: &SourceInfo
    ) -> Result<(Vec<usize>, Vec<f32>), ARLError>;

    /// Report attack (for supervised learning)
    async fn report_attack(
        &mut self,
        query: &[f32],
        attack_type: AttackType,
        source: &SourceInfo
    );

    /// Report false positive
    async fn report_false_positive(&mut self, query: &[f32]);

    /// Get attack statistics
    async fn get_attack_stats(&self) -> AttackStats;

    /// Get defense metrics
    async fn get_metrics(&self) -> ARLMetrics;

    /// Export attack logs
    async fn export_logs(&self, path: &str) -> Result<(), ARLError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ARLError {
    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Malicious query detected (score: {score})")]
    MaliciousQuery { score: f32 },

    #[error("Invalid dimension: expected {expected}, got {actual}")]
    InvalidDimension { expected: usize, actual: usize },

    #[error("Invalid values in query")]
    InvalidValues,

    #[error("Query norm too large: {norm}")]
    NormTooLarge { norm: f32 },

    #[error("Invalid output")]
    InvalidOutput,

    #[error("Attention error: {0}")]
    AttentionError(String),
}
```

## Integration Points

### Affected Crates/Modules

1. **`ruvector-gnn-core/src/attention/`**
   - Wrap all attention layers with ARL

2. **`ruvector-gnn-node/`**
   - Expose defense configuration in Node.js API

### New Modules to Create

```
ruvector-gnn-core/src/security/
├── mod.rs
├── arl/
│   ├── mod.rs
│   ├── config.rs
│   ├── detector/
│   │   ├── mod.rs
│   │   ├── statistical.rs
│   │   ├── autoencoder.rs
│   │   └── ensemble.rs
│   ├── defense/
│   │   ├── mod.rs
│   │   ├── projection.rs
│   │   ├── smoothing.rs
│   │   └── validation.rs
│   ├── logger.rs
│   ├── rate_limit.rs
│   └── metrics.rs
└── attacks/
    ├── mod.rs
    ├── patterns.rs
    └── attribution.rs
```

## Implementation Phases

### Phase 1: Core Defense (3 weeks)
- Statistical anomaly detector
- Input/output validation
- Attack logging
- Basic metrics

### Phase 2: Advanced Detection (2 weeks)
- Autoencoder detector
- Ensemble detector
- Defensive projection
- Rate limiting

### Phase 3: Certified Robustness (2 weeks)
- Randomized smoothing
- Robustness certification
- Performance optimization

### Phase 4: Adaptive Learning (1 week)
- Online detector updates
- Attack pattern learning
- Alert system

## Success Metrics

| Metric | Target |
|--------|--------|
| Attack Detection Rate | >90% |
| False Positive Rate | <5% |
| Certified Robustness Radius | >0.1 |
| Defense Latency Overhead | <10% |
| Zero-Day Detection | >70% |

## Risks and Mitigations

1. **Risk: High False Positive Rate**
   - Mitigation: Ensemble detectors, adaptive thresholds

2. **Risk: Certified Robustness Too Expensive**
   - Mitigation: Only for suspicious queries, optimize sampling

3. **Risk: Adaptive Attacks**
   - Mitigation: Continuous learning, diverse defense layers

4. **Risk: Privacy Concerns with Logging**
   - Mitigation: Hash queries, anonymize source info
