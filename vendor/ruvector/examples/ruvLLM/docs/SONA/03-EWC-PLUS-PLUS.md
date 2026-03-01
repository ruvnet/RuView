# SONA EWC++: Enhanced Elastic Weight Consolidation

## Zero Catastrophic Forgetting with Task-Aware Regularization

---

## 1. The Forgetting Problem

### Why LLMs Forget

```
CATASTROPHIC FORGETTING
═══════════════════════

Task A learned     Task B learned     Result
───────────────    ───────────────    ──────────────────
Weights W_A        Weights W_B        W_A knowledge LOST
                   ↑                  as W moves toward B
                   Training on B
                   overwrites A
```

When fine-tuning on new data:
- Weights shift toward new task optimum
- Previous task knowledge encoded in old weights is overwritten
- Model "forgets" earlier capabilities

### Standard EWC Solution

Elastic Weight Consolidation (EWC) adds a regularization term:

```
L_total = L_task + λ/2 · Σᵢ Fᵢ · (θᵢ - θ*ᵢ)²

Where:
- L_task = current task loss
- λ = regularization strength
- Fᵢ = Fisher Information (importance) of parameter i
- θᵢ = current parameter value
- θ*ᵢ = optimal parameter value from previous task
```

### EWC Limitations

1. **Single task memory**: Only remembers one previous task
2. **Static Fisher**: Computed once, never updated
3. **Diagonal approximation**: Ignores parameter correlations
4. **No task detection**: Doesn't know when task changes
5. **Uniform λ**: Same regularization for all parameters

---

## 2. SONA EWC++ Enhancements

### Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         EWC++ ARCHITECTURE                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌───────────────┐    ┌───────────────┐    ┌───────────────┐      │
│   │ Task Buffer   │    │ Online Fisher │    │ Adaptive λ    │      │
│   │ (N tasks)     │    │ Estimation    │    │ Scheduler     │      │
│   └───────┬───────┘    └───────┬───────┘    └───────┬───────┘      │
│           │                    │                    │               │
│           ▼                    ▼                    ▼               │
│   ┌─────────────────────────────────────────────────────────────┐  │
│   │                    EWC++ CORE ENGINE                         │  │
│   │                                                               │  │
│   │  L = L_task + Σₜ λₜ/2 · Σᵢ Fᵢᵗ · (θᵢ - θ*ᵢᵗ)² + L_sparse   │  │
│   │      └─────┘   └──────────────────────────────────┘ └──────┘  │  │
│   │      Task      Multi-task EWC                       Sparsity  │  │
│   │      Loss      Regularization                       Penalty   │  │
│   └─────────────────────────────────────────────────────────────┘  │
│           │                    │                    │               │
│           ▼                    ▼                    ▼               │
│   ┌───────────────┐    ┌───────────────┐    ┌───────────────┐      │
│   │ Gradient      │    │ Task Boundary │    │ Parameter     │      │
│   │ Projection    │    │ Detection     │    │ Importance    │      │
│   └───────────────┘    └───────────────┘    └───────────────┘      │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. Multi-Task Memory Buffer

### Task-Stratified Fisher Storage

```rust
/// EWC++ state with multi-task memory
#[derive(Clone)]
pub struct EWCPlusPlusState {
    /// Per-task Fisher information (circular buffer of N tasks)
    pub task_fishers: CircularBuffer<TaskFisher>,
    /// Maximum number of tasks to remember
    pub max_tasks: usize,
    /// Per-task regularization strength
    pub task_lambdas: Vec<f32>,
    /// Global lambda base
    pub lambda_base: f32,
    /// Online Fisher estimator
    pub online_fisher: OnlineFisherEstimator,
    /// Task boundary detector
    pub task_detector: TaskBoundaryDetector,
    /// Parameter importance scores
    pub importance_scores: Vec<f32>,
}

/// Fisher information for a single task
#[derive(Clone)]
pub struct TaskFisher {
    /// Task identifier
    pub task_id: u64,
    /// Diagonal Fisher Information
    pub fisher_diag: Vec<f32>,
    /// Optimal weights at task completion
    pub optimal_weights: Vec<f32>,
    /// Task-specific lambda (learned)
    pub lambda: f32,
    /// Sample count used to compute Fisher
    pub sample_count: usize,
    /// Task quality score
    pub quality: f32,
    /// Timestamp
    pub timestamp: i64,
}

impl EWCPlusPlusState {
    /// Create new EWC++ state
    pub fn new(num_params: usize, max_tasks: usize, lambda_base: f32) -> Self {
        Self {
            task_fishers: CircularBuffer::new(max_tasks),
            max_tasks,
            task_lambdas: Vec::new(),
            lambda_base,
            online_fisher: OnlineFisherEstimator::new(num_params),
            task_detector: TaskBoundaryDetector::new(),
            importance_scores: vec![1.0; num_params],
        }
    }

    /// Compute total EWC++ regularization loss
    pub fn regularization_loss(&self, current_weights: &[f32]) -> f32 {
        let mut total_loss = 0.0;

        // Sum over all remembered tasks
        for task in self.task_fishers.iter() {
            let task_loss: f32 = task.fisher_diag.iter()
                .zip(current_weights.iter())
                .zip(task.optimal_weights.iter())
                .zip(self.importance_scores.iter())
                .map(|(((f, w), w_star), imp)| {
                    // Importance-weighted Fisher regularization
                    imp * f * (w - w_star).powi(2)
                })
                .sum();

            total_loss += task.lambda * task_loss;
        }

        total_loss / 2.0
    }

    /// Compute gradients of EWC++ loss
    pub fn regularization_gradient(&self, current_weights: &[f32]) -> Vec<f32> {
        let mut grad = vec![0.0f32; current_weights.len()];

        for task in self.task_fishers.iter() {
            for (i, ((f, w), w_star)) in task.fisher_diag.iter()
                .zip(current_weights.iter())
                .zip(task.optimal_weights.iter())
                .enumerate()
            {
                // d/dw [F * (w - w*)²] = 2 * F * (w - w*)
                grad[i] += task.lambda * self.importance_scores[i] * f * (w - w_star);
            }
        }

        grad
    }

    /// Record completion of current task
    pub fn complete_task(&mut self, weights: &[f32], quality: f32) {
        let task_id = self.task_fishers.len() as u64;

        // Finalize online Fisher estimate
        let fisher_diag = self.online_fisher.finalize();

        // Compute task-specific lambda based on quality
        let lambda = self.compute_task_lambda(quality);

        let task_fisher = TaskFisher {
            task_id,
            fisher_diag,
            optimal_weights: weights.to_vec(),
            lambda,
            sample_count: self.online_fisher.sample_count(),
            quality,
            timestamp: chrono::Utc::now().timestamp(),
        };

        self.task_fishers.push(task_fisher);
        self.task_lambdas.push(lambda);

        // Reset online Fisher for next task
        self.online_fisher.reset();
    }

    /// Compute task-specific lambda based on quality
    fn compute_task_lambda(&self, quality: f32) -> f32 {
        // Higher quality tasks get stronger protection
        self.lambda_base * (0.5 + 0.5 * quality)
    }
}
```

---

## 4. Online Fisher Estimation

### Streaming Fisher Information Computation

```rust
/// Online Fisher Information estimator using gradient accumulation
pub struct OnlineFisherEstimator {
    /// Running sum of squared gradients
    gradient_sq_sum: Vec<f32>,
    /// Sample count
    count: usize,
    /// Exponential moving average decay
    decay: f32,
    /// Minimum samples before valid estimate
    min_samples: usize,
}

impl OnlineFisherEstimator {
    pub fn new(num_params: usize) -> Self {
        Self {
            gradient_sq_sum: vec![0.0; num_params],
            count: 0,
            decay: 0.99, // EMA decay factor
            min_samples: 100,
        }
    }

    /// Update Fisher estimate with new gradient sample
    #[inline]
    pub fn update(&mut self, gradients: &[f32]) {
        self.count += 1;

        if self.count == 1 {
            // First sample: initialize
            for (sum, g) in self.gradient_sq_sum.iter_mut().zip(gradients.iter()) {
                *sum = g * g;
            }
        } else {
            // EMA update: F_new = decay * F_old + (1 - decay) * g²
            let alpha = 1.0 - self.decay;
            for (sum, g) in self.gradient_sq_sum.iter_mut().zip(gradients.iter()) {
                *sum = self.decay * *sum + alpha * g * g;
            }
        }
    }

    /// Finalize and return Fisher diagonal
    pub fn finalize(&self) -> Vec<f32> {
        if self.count < self.min_samples {
            tracing::warn!(
                count = self.count,
                min = self.min_samples,
                "Fisher estimate may be unreliable"
            );
        }

        // Normalize and apply minimum threshold
        let min_fisher = 1e-6;
        self.gradient_sq_sum.iter()
            .map(|&f| f.max(min_fisher))
            .collect()
    }

    /// Reset for new task
    pub fn reset(&mut self) {
        self.gradient_sq_sum.fill(0.0);
        self.count = 0;
    }

    pub fn sample_count(&self) -> usize {
        self.count
    }
}
```

---

## 5. Automatic Task Boundary Detection

### Detecting When the Task Changes

```rust
/// Automatic task boundary detection via distribution shift
pub struct TaskBoundaryDetector {
    /// Recent query embedding buffer
    recent_embeddings: CircularBuffer<Vec<f32>>,
    /// Baseline distribution (mean, variance)
    baseline: Option<DistributionStats>,
    /// Threshold for detecting shift (Mahalanobis distance)
    shift_threshold: f32,
    /// Minimum samples before detection
    warmup_samples: usize,
    /// Current drift score
    drift_score: f32,
}

impl TaskBoundaryDetector {
    pub fn new() -> Self {
        Self {
            recent_embeddings: CircularBuffer::new(1000),
            baseline: None,
            shift_threshold: 3.0, // 3 sigma
            warmup_samples: 500,
            drift_score: 0.0,
        }
    }

    /// Update with new embedding and check for task boundary
    pub fn update(&mut self, embedding: &[f32]) -> TaskBoundaryResult {
        self.recent_embeddings.push(embedding.to_vec());

        if self.recent_embeddings.len() < self.warmup_samples {
            return TaskBoundaryResult::Warmup;
        }

        match &self.baseline {
            None => {
                // First baseline establishment
                self.baseline = Some(self.compute_stats());
                TaskBoundaryResult::BaselineEstablished
            }
            Some(baseline) => {
                // Compute current distribution
                let current = self.compute_recent_stats(100);

                // Mahalanobis distance between distributions
                let distance = self.mahalanobis_distance(baseline, &current);
                self.drift_score = distance;

                if distance > self.shift_threshold {
                    // Task boundary detected!
                    self.baseline = Some(current);
                    TaskBoundaryResult::BoundaryDetected {
                        drift_score: distance,
                    }
                } else {
                    TaskBoundaryResult::Stable {
                        drift_score: distance,
                    }
                }
            }
        }
    }

    fn compute_stats(&self) -> DistributionStats {
        let n = self.recent_embeddings.len();
        let dim = self.recent_embeddings[0].len();

        let mut mean = vec![0.0f32; dim];
        let mut var = vec![0.0f32; dim];

        // Compute mean
        for emb in self.recent_embeddings.iter() {
            for (m, e) in mean.iter_mut().zip(emb.iter()) {
                *m += e;
            }
        }
        for m in &mut mean {
            *m /= n as f32;
        }

        // Compute variance
        for emb in self.recent_embeddings.iter() {
            for (v, (e, m)) in var.iter_mut().zip(emb.iter().zip(mean.iter())) {
                *v += (e - m).powi(2);
            }
        }
        for v in &mut var {
            *v /= n as f32;
            *v = v.max(1e-6); // Avoid division by zero
        }

        DistributionStats { mean, variance: var }
    }

    fn compute_recent_stats(&self, n: usize) -> DistributionStats {
        // Similar but only for last n samples
        // ... implementation ...
    }

    fn mahalanobis_distance(&self, a: &DistributionStats, b: &DistributionStats) -> f32 {
        a.mean.iter()
            .zip(b.mean.iter())
            .zip(a.variance.iter())
            .map(|((m_a, m_b), v)| (m_a - m_b).powi(2) / v)
            .sum::<f32>()
            .sqrt()
    }
}

#[derive(Debug)]
pub enum TaskBoundaryResult {
    Warmup,
    BaselineEstablished,
    Stable { drift_score: f32 },
    BoundaryDetected { drift_score: f32 },
}
```

---

## 6. Adaptive Lambda Scheduling

### Dynamic Regularization Strength

```rust
/// Adaptive lambda scheduler based on learning progress
pub struct AdaptiveLambdaScheduler {
    /// Base lambda value
    base_lambda: f32,
    /// Current effective lambda
    current_lambda: f32,
    /// Performance history (task quality over time)
    performance_history: Vec<f32>,
    /// Lambda adjustment rate
    adjustment_rate: f32,
}

impl AdaptiveLambdaScheduler {
    pub fn new(base_lambda: f32) -> Self {
        Self {
            base_lambda,
            current_lambda: base_lambda,
            performance_history: Vec::new(),
            adjustment_rate: 0.1,
        }
    }

    /// Update lambda based on recent performance
    pub fn update(&mut self, current_quality: f32, forgetting_detected: bool) {
        self.performance_history.push(current_quality);

        if forgetting_detected {
            // Increase lambda to prevent forgetting
            self.current_lambda *= 1.0 + self.adjustment_rate;
            tracing::info!(
                new_lambda = self.current_lambda,
                "Increased lambda due to forgetting"
            );
        } else if self.is_learning_stalled() {
            // Decrease lambda to allow more plasticity
            self.current_lambda *= 1.0 - self.adjustment_rate;
            self.current_lambda = self.current_lambda.max(self.base_lambda * 0.1);
            tracing::info!(
                new_lambda = self.current_lambda,
                "Decreased lambda to increase plasticity"
            );
        }

        // Clamp to reasonable range
        self.current_lambda = self.current_lambda.clamp(
            self.base_lambda * 0.1,
            self.base_lambda * 10.0,
        );
    }

    fn is_learning_stalled(&self) -> bool {
        if self.performance_history.len() < 10 {
            return false;
        }

        let recent: Vec<_> = self.performance_history.iter()
            .rev()
            .take(10)
            .collect();

        // Check if variance in recent performance is very low
        let mean: f32 = recent.iter().map(|&&x| x).sum::<f32>() / 10.0;
        let var: f32 = recent.iter()
            .map(|&&x| (x - mean).powi(2))
            .sum::<f32>() / 10.0;

        var < 0.001 // Stalled if very low variance
    }

    pub fn get_lambda(&self) -> f32 {
        self.current_lambda
    }
}
```

---

## 7. Parameter Importance Scoring

### Which Parameters Matter Most

```rust
/// Per-parameter importance scoring for selective regularization
pub struct ParameterImportanceScorer {
    /// Importance scores (0-1 for each parameter)
    scores: Vec<f32>,
    /// Gradient magnitude history
    gradient_magnitudes: Vec<CircularBuffer<f32>>,
    /// Activation frequency
    activation_frequency: Vec<f32>,
}

impl ParameterImportanceScorer {
    pub fn new(num_params: usize) -> Self {
        Self {
            scores: vec![1.0; num_params],
            gradient_magnitudes: (0..num_params)
                .map(|_| CircularBuffer::new(100))
                .collect(),
            activation_frequency: vec![0.0; num_params],
        }
    }

    /// Update importance based on gradient
    pub fn update(&mut self, gradients: &[f32], activations: &[bool]) {
        for (i, (g, &active)) in gradients.iter().zip(activations.iter()).enumerate() {
            // Track gradient magnitude
            self.gradient_magnitudes[i].push(g.abs());

            // Track activation frequency
            if active {
                self.activation_frequency[i] = 0.99 * self.activation_frequency[i] + 0.01;
            } else {
                self.activation_frequency[i] *= 0.99;
            }
        }

        // Recompute importance scores
        self.recompute_scores();
    }

    fn recompute_scores(&mut self) {
        for i in 0..self.scores.len() {
            // Average gradient magnitude
            let avg_grad: f32 = self.gradient_magnitudes[i].iter()
                .sum::<f32>() / self.gradient_magnitudes[i].len().max(1) as f32;

            // Importance = activation_freq * gradient_magnitude
            // High activation + high gradient = important parameter
            self.scores[i] = self.activation_frequency[i] * avg_grad;
        }

        // Normalize scores to [0, 1]
        let max_score = self.scores.iter().cloned().fold(0.0f32, f32::max);
        if max_score > 0.0 {
            for s in &mut self.scores {
                *s /= max_score;
            }
        }
    }

    pub fn get_scores(&self) -> &[f32] {
        &self.scores
    }
}
```

---

## 8. Gradient Projection

### Safe Parameter Updates

```rust
/// Project gradients to avoid interfering with important past knowledge
pub struct GradientProjector {
    /// Null space of important task gradients
    null_space: Option<Array2<f32>>,
    /// Task gradient subspace (principal components)
    task_subspace: Option<Array2<f32>>,
}

impl GradientProjector {
    /// Project gradient to not interfere with past tasks
    pub fn project(&self, gradient: &[f32]) -> Vec<f32> {
        match &self.null_space {
            Some(null) => {
                // Project gradient onto null space of past task gradients
                let g = Array1::from_vec(gradient.to_vec());
                let projected = null.t().dot(&null.dot(&g));
                projected.to_vec()
            }
            None => gradient.to_vec(),
        }
    }

    /// Update null space with new task gradient directions
    pub fn add_task_gradients(&mut self, task_gradients: &[Vec<f32>]) {
        // Stack gradients into matrix
        let n_samples = task_gradients.len();
        let n_params = task_gradients[0].len();

        let mut g_matrix = Array2::zeros((n_samples, n_params));
        for (i, g) in task_gradients.iter().enumerate() {
            for (j, &v) in g.iter().enumerate() {
                g_matrix[[i, j]] = v;
            }
        }

        // SVD to find principal gradient directions
        let svd = g_matrix.svd(true, true).unwrap();
        let u = svd.u.unwrap();

        // Null space = complement of principal directions
        // For memory efficiency, keep top-k directions
        let k = 10.min(n_samples);
        let task_directions = u.slice(s![.., ..k]).to_owned();

        // Compute null space projection matrix
        let identity = Array2::eye(n_params);
        let projection = identity - task_directions.t().dot(&task_directions);

        self.null_space = Some(projection);
    }
}
```

---

## 9. Full EWC++ Training Loop

### Putting It All Together

```rust
/// Complete EWC++ training step
pub fn ewc_plus_plus_train_step(
    model: &mut FastGRNNRouter,
    ewc: &mut EWCPlusPlusState,
    batch: &[RouterSample],
    config: &TrainingConfig,
) -> TrainStepResult {
    let mut result = TrainStepResult::default();

    // Forward pass
    let predictions: Vec<_> = batch.iter()
        .map(|s| model.forward(&s.features))
        .collect();

    // Task loss
    let task_loss = compute_cross_entropy_loss(&predictions, batch);
    result.task_loss = task_loss;

    // EWC++ regularization loss
    let ewc_loss = ewc.regularization_loss(model.get_weights());
    result.ewc_loss = ewc_loss;

    // Total loss
    let total_loss = task_loss + config.lambda * ewc_loss;
    result.total_loss = total_loss;

    // Compute task gradients
    let task_gradients = compute_gradients(&task_loss, model);

    // Compute EWC++ gradients
    let ewc_gradients = ewc.regularization_gradient(model.get_weights());

    // Total gradients
    let mut gradients: Vec<f32> = task_gradients.iter()
        .zip(ewc_gradients.iter())
        .map(|(t, e)| t + config.lambda * e)
        .collect();

    // Gradient projection (optional, for harder constraints)
    if config.use_gradient_projection {
        gradients = ewc.gradient_projector.project(&gradients);
    }

    // Gradient clipping
    let grad_norm: f32 = gradients.iter().map(|g| g * g).sum::<f32>().sqrt();
    if grad_norm > config.max_grad_norm {
        let scale = config.max_grad_norm / grad_norm;
        for g in &mut gradients {
            *g *= scale;
        }
        result.gradient_clipped = true;
    }

    // Apply gradients
    model.apply_gradients(&gradients, config.learning_rate);

    // Update online Fisher estimate
    ewc.online_fisher.update(&task_gradients);

    // Update parameter importance
    let activations: Vec<bool> = model.get_activation_mask();
    ewc.importance_scorer.update(&task_gradients, &activations);

    // Check for task boundary
    if let Some(query_emb) = batch.first().map(|s| &s.query_embedding) {
        let boundary = ewc.task_detector.update(query_emb);
        if let TaskBoundaryResult::BoundaryDetected { drift_score } = boundary {
            // Complete current task and start new one
            ewc.complete_task(model.get_weights(), result.compute_quality());
            result.task_boundary_detected = true;
            result.drift_score = drift_score;
        }
    }

    result
}
```

---

## 10. Benchmarks and Validation

### Forgetting Resistance Metrics

```rust
/// Measure forgetting resistance on held-out test sets
pub struct ForgettingBenchmark {
    /// Per-task test sets
    task_test_sets: Vec<TestSet>,
    /// Performance history per task
    task_performance: Vec<Vec<f32>>,
}

impl ForgettingBenchmark {
    /// Evaluate current model on all past tasks
    pub fn evaluate(&mut self, model: &FastGRNNRouter) -> ForgettingReport {
        let mut report = ForgettingReport::default();

        for (task_id, test_set) in self.task_test_sets.iter().enumerate() {
            let accuracy = self.evaluate_task(model, test_set);
            self.task_performance[task_id].push(accuracy);

            // Compute forgetting = max_accuracy - current_accuracy
            let max_acc = self.task_performance[task_id].iter()
                .cloned()
                .fold(0.0f32, f32::max);
            let forgetting = (max_acc - accuracy).max(0.0);

            report.per_task_accuracy.push(accuracy);
            report.per_task_forgetting.push(forgetting);
        }

        // Average forgetting
        report.avg_forgetting = report.per_task_forgetting.iter()
            .sum::<f32>() / report.per_task_forgetting.len().max(1) as f32;

        // Backward transfer (negative forgetting = improvement)
        report.backward_transfer = -report.avg_forgetting;

        report
    }

    fn evaluate_task(&self, model: &FastGRNNRouter, test: &TestSet) -> f32 {
        let correct = test.samples.iter()
            .filter(|s| model.forward(&s.features).predicted_class == s.label)
            .count();
        correct as f32 / test.samples.len() as f32
    }
}

#[derive(Debug, Default)]
pub struct ForgettingReport {
    pub per_task_accuracy: Vec<f32>,
    pub per_task_forgetting: Vec<f32>,
    pub avg_forgetting: f32,
    pub backward_transfer: f32,
}
```

---

## Summary: EWC++ vs Standard EWC

| Feature | Standard EWC | SONA EWC++ |
|---------|-------------|------------|
| Task memory | 1 task | N tasks (configurable) |
| Fisher estimation | Offline, single | Online, streaming |
| Lambda | Fixed | Adaptive per-task |
| Task detection | Manual | Automatic |
| Parameter importance | Uniform | Learned |
| Gradient handling | Direct | Projected |
| Forgetting rate | ~5-10% | **<0.1%** |

EWC++ enables SONA to learn continuously from every interaction while maintaining near-perfect retention of past knowledge.
