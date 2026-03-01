//! Learning rate scheduling for Graph Neural Networks
//!
//! Provides various learning rate scheduling strategies to prevent catastrophic
//! forgetting and optimize training dynamics in continual learning scenarios.

use std::f32::consts::PI;

/// Learning rate scheduling strategies
#[derive(Debug, Clone)]
pub enum SchedulerType {
    /// Constant learning rate throughout training
    Constant,

    /// Step decay: multiply learning rate by gamma every step_size epochs
    /// Formula: lr = base_lr * gamma^(epoch / step_size)
    StepDecay { step_size: usize, gamma: f32 },

    /// Exponential decay: multiply learning rate by gamma each epoch
    /// Formula: lr = base_lr * gamma^epoch
    Exponential { gamma: f32 },

    /// Cosine annealing with warm restarts
    /// Formula: lr = eta_min + 0.5 * (base_lr - eta_min) * (1 + cos(pi * (epoch % t_max) / t_max))
    CosineAnnealing { t_max: usize, eta_min: f32 },

    /// Warmup phase followed by linear decay
    /// Linearly increases lr from 0 to base_lr over warmup_steps,
    /// then linearly decreases to 0 over remaining steps
    WarmupLinear {
        warmup_steps: usize,
        total_steps: usize,
    },

    /// Reduce learning rate when a metric plateaus
    /// Useful for online learning scenarios
    ReduceOnPlateau {
        factor: f32,
        patience: usize,
        min_lr: f32,
    },
}

/// Learning rate scheduler for GNN training
///
/// Implements various scheduling strategies to control learning rate
/// during training, helping prevent catastrophic forgetting and
/// improve convergence.
#[derive(Debug, Clone)]
pub struct LearningRateScheduler {
    scheduler_type: SchedulerType,
    base_lr: f32,
    current_lr: f32,
    step_count: usize,
    best_metric: f32,
    patience_counter: usize,
}

impl LearningRateScheduler {
    /// Creates a new learning rate scheduler
    ///
    /// # Arguments
    /// * `scheduler_type` - The scheduling strategy to use
    /// * `base_lr` - The initial/base learning rate
    ///
    /// # Example
    /// ```
    /// use ruvector_gnn::scheduler::{LearningRateScheduler, SchedulerType};
    ///
    /// let scheduler = LearningRateScheduler::new(
    ///     SchedulerType::StepDecay { step_size: 10, gamma: 0.9 },
    ///     0.001
    /// );
    /// ```
    pub fn new(scheduler_type: SchedulerType, base_lr: f32) -> Self {
        Self {
            scheduler_type,
            base_lr,
            current_lr: base_lr,
            step_count: 0,
            best_metric: f32::INFINITY,
            patience_counter: 0,
        }
    }

    /// Advances the scheduler by one step and returns the new learning rate
    ///
    /// For most schedulers, this should be called once per epoch.
    /// For ReduceOnPlateau, use `step_with_metric` instead.
    ///
    /// # Returns
    /// The updated learning rate
    pub fn step(&mut self) -> f32 {
        self.step_count += 1;
        self.current_lr = self.calculate_lr();
        self.current_lr
    }

    /// Advances the scheduler with a metric value (for ReduceOnPlateau)
    ///
    /// # Arguments
    /// * `metric` - The metric value to monitor (e.g., validation loss)
    ///
    /// # Returns
    /// The updated learning rate
    pub fn step_with_metric(&mut self, metric: f32) -> f32 {
        self.step_count += 1;

        match &self.scheduler_type {
            SchedulerType::ReduceOnPlateau {
                factor,
                patience,
                min_lr,
            } => {
                // Check if metric improved
                if metric < self.best_metric - 1e-8 {
                    self.best_metric = metric;
                    self.patience_counter = 0;
                } else {
                    self.patience_counter += 1;

                    // Reduce learning rate if patience exceeded
                    if self.patience_counter >= *patience {
                        self.current_lr = (self.current_lr * factor).max(*min_lr);
                        self.patience_counter = 0;
                    }
                }
            }
            _ => {
                // For non-plateau schedulers, just use step()
                self.current_lr = self.calculate_lr();
            }
        }

        self.current_lr
    }

    /// Gets the current learning rate without advancing the scheduler
    pub fn get_lr(&self) -> f32 {
        self.current_lr
    }

    /// Resets the scheduler to its initial state
    pub fn reset(&mut self) {
        self.current_lr = self.base_lr;
        self.step_count = 0;
        self.best_metric = f32::INFINITY;
        self.patience_counter = 0;
    }

    /// Calculates the learning rate based on the current step and scheduler type
    fn calculate_lr(&self) -> f32 {
        match &self.scheduler_type {
            SchedulerType::Constant => self.base_lr,

            SchedulerType::StepDecay { step_size, gamma } => {
                let decay_factor = (*gamma).powi((self.step_count / step_size) as i32);
                self.base_lr * decay_factor
            }

            SchedulerType::Exponential { gamma } => {
                let decay_factor = (*gamma).powi(self.step_count as i32);
                self.base_lr * decay_factor
            }

            SchedulerType::CosineAnnealing { t_max, eta_min } => {
                let cycle_step = self.step_count % t_max;
                let cos_term = (PI * cycle_step as f32 / *t_max as f32).cos();
                eta_min + 0.5 * (self.base_lr - eta_min) * (1.0 + cos_term)
            }

            SchedulerType::WarmupLinear {
                warmup_steps,
                total_steps,
            } => {
                if self.step_count < *warmup_steps {
                    // Warmup phase: linear increase
                    self.base_lr * (self.step_count as f32 / *warmup_steps as f32)
                } else if self.step_count < *total_steps {
                    // Decay phase: linear decrease
                    let remaining_steps = *total_steps - self.step_count;
                    let total_decay_steps = *total_steps - *warmup_steps;
                    self.base_lr * (remaining_steps as f32 / total_decay_steps as f32)
                } else {
                    // After total_steps, keep at 0
                    0.0
                }
            }

            SchedulerType::ReduceOnPlateau { .. } => {
                // For plateau scheduler, lr is updated in step_with_metric
                self.current_lr
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-6;

    fn assert_close(a: f32, b: f32, msg: &str) {
        assert!((a - b).abs() < EPSILON, "{}: {} != {}", msg, a, b);
    }

    #[test]
    fn test_constant_scheduler() {
        let mut scheduler = LearningRateScheduler::new(SchedulerType::Constant, 0.01);

        assert_close(scheduler.get_lr(), 0.01, "Initial LR");

        for i in 1..=10 {
            let lr = scheduler.step();
            assert_close(lr, 0.01, &format!("Step {} LR", i));
        }
    }

    #[test]
    fn test_step_decay() {
        let mut scheduler = LearningRateScheduler::new(
            SchedulerType::StepDecay {
                step_size: 5,
                gamma: 0.5,
            },
            0.1,
        );

        assert_close(scheduler.get_lr(), 0.1, "Initial LR");

        // Steps 1-4: no decay
        for i in 1..=4 {
            let lr = scheduler.step();
            assert_close(lr, 0.1, &format!("Step {} LR", i));
        }

        // Step 5: first decay (0.1 * 0.5)
        let lr = scheduler.step();
        assert_close(lr, 0.05, "Step 5 LR (first decay)");

        // Steps 6-9: maintain decayed rate
        for i in 6..=9 {
            let lr = scheduler.step();
            assert_close(lr, 0.05, &format!("Step {} LR", i));
        }

        // Step 10: second decay (0.1 * 0.5^2)
        let lr = scheduler.step();
        assert_close(lr, 0.025, "Step 10 LR (second decay)");
    }

    #[test]
    fn test_exponential_decay() {
        let mut scheduler =
            LearningRateScheduler::new(SchedulerType::Exponential { gamma: 0.9 }, 0.1);

        assert_close(scheduler.get_lr(), 0.1, "Initial LR");

        let expected_lrs = vec![
            0.1 * 0.9,   // Step 1
            0.1 * 0.81,  // Step 2 (0.9^2)
            0.1 * 0.729, // Step 3 (0.9^3)
        ];

        for (i, expected) in expected_lrs.iter().enumerate() {
            let lr = scheduler.step();
            assert_close(lr, *expected, &format!("Step {} LR", i + 1));
        }
    }

    #[test]
    fn test_cosine_annealing() {
        let mut scheduler = LearningRateScheduler::new(
            SchedulerType::CosineAnnealing {
                t_max: 10,
                eta_min: 0.0,
            },
            1.0,
        );

        assert_close(scheduler.get_lr(), 1.0, "Initial LR");

        // Cosine annealing formula: lr = eta_min + 0.5 * (base_lr - eta_min) * (1 + cos(pi * cycle_step / t_max))
        // cycle_step = step_count % t_max
        // At step 5: cycle_step = 5, cos(pi * 5/10) = cos(pi/2) = 0, lr = 0 + 0.5 * 1 * (1 + 0) = 0.5
        // At step 10: cycle_step = 0 (wrapped), cos(0) = 1, lr = 0 + 0.5 * 1 * (1 + 1) = 1.0 (restart)

        for _ in 1..=5 {
            scheduler.step();
        }
        assert_close(scheduler.get_lr(), 0.5, "Mid-cycle LR (step 5)");

        // At step 9: cycle_step = 9, cos(pi * 9/10) ≈ -0.951, lr ≈ 0.025
        for _ in 6..=9 {
            scheduler.step();
        }
        let lr_step9 = scheduler.get_lr();
        assert!(
            lr_step9 < 0.1,
            "Near end of cycle LR (step 9) should be small: {}",
            lr_step9
        );

        // At step 10: warm restart (cycle_step = 0), LR goes back to base
        scheduler.step();
        assert_close(
            scheduler.get_lr(),
            1.0,
            "Restart at step 10 (cycle_step = 0)",
        );

        // Continue new cycle
        scheduler.step();
        assert!(
            scheduler.get_lr() < 1.0,
            "Step 11 should be less than base LR"
        );
    }

    #[test]
    fn test_warmup_linear() {
        let mut scheduler = LearningRateScheduler::new(
            SchedulerType::WarmupLinear {
                warmup_steps: 5,
                total_steps: 10,
            },
            1.0,
        );

        assert_close(scheduler.get_lr(), 1.0, "Initial LR");

        // Warmup phase: linear increase
        scheduler.step();
        assert_close(scheduler.get_lr(), 0.2, "Step 1 (warmup)");

        scheduler.step();
        assert_close(scheduler.get_lr(), 0.4, "Step 2 (warmup)");

        scheduler.step();
        assert_close(scheduler.get_lr(), 0.6, "Step 3 (warmup)");

        scheduler.step();
        assert_close(scheduler.get_lr(), 0.8, "Step 4 (warmup)");

        scheduler.step();
        assert_close(scheduler.get_lr(), 1.0, "Step 5 (warmup end)");

        // Decay phase: linear decrease
        scheduler.step();
        assert_close(scheduler.get_lr(), 0.8, "Step 6 (decay)");

        scheduler.step();
        assert_close(scheduler.get_lr(), 0.6, "Step 7 (decay)");

        scheduler.step();
        assert_close(scheduler.get_lr(), 0.4, "Step 8 (decay)");

        scheduler.step();
        assert_close(scheduler.get_lr(), 0.2, "Step 9 (decay)");

        scheduler.step();
        assert_close(scheduler.get_lr(), 0.0, "Step 10 (decay end)");

        // After total_steps
        scheduler.step();
        assert_close(scheduler.get_lr(), 0.0, "Step 11 (after total)");
    }

    #[test]
    fn test_reduce_on_plateau() {
        let mut scheduler = LearningRateScheduler::new(
            SchedulerType::ReduceOnPlateau {
                factor: 0.5,
                patience: 3,
                min_lr: 0.0001,
            },
            0.01,
        );

        assert_close(scheduler.get_lr(), 0.01, "Initial LR");

        // Improving metrics: no reduction (sets best_metric, resets patience)
        scheduler.step_with_metric(1.0);
        assert_close(
            scheduler.get_lr(),
            0.01,
            "Step 1 (first metric, sets baseline)",
        );

        scheduler.step_with_metric(0.9);
        assert_close(scheduler.get_lr(), 0.01, "Step 2 (improving)");

        // Plateau: metric not improving (patience counter: 1, 2, 3)
        scheduler.step_with_metric(0.91);
        assert_close(scheduler.get_lr(), 0.01, "Step 3 (plateau 1)");

        scheduler.step_with_metric(0.92);
        assert_close(scheduler.get_lr(), 0.01, "Step 4 (plateau 2)");

        // patience=3 means after 3 non-improvements, reduce LR
        // Step 5 is the 3rd non-improvement, so LR gets reduced
        scheduler.step_with_metric(0.93);
        assert_close(
            scheduler.get_lr(),
            0.005,
            "Step 5 (patience exceeded, reduced)",
        );

        // Counter is reset after reduction, so we need 3 more non-improvements
        scheduler.step_with_metric(0.94); // plateau 1 after reset
        assert_close(scheduler.get_lr(), 0.005, "Step 6 (plateau 1 after reset)");

        scheduler.step_with_metric(0.95); // plateau 2
        assert_close(scheduler.get_lr(), 0.005, "Step 7 (plateau 2)");

        scheduler.step_with_metric(0.96); // plateau 3 - triggers reduction
        assert_close(scheduler.get_lr(), 0.0025, "Step 8 (reduced again)");

        // Test min_lr floor
        for _ in 0..20 {
            scheduler.step_with_metric(1.0);
        }
        assert!(
            scheduler.get_lr() >= 0.0001,
            "LR should not go below min_lr"
        );
    }

    #[test]
    fn test_scheduler_reset() {
        let mut scheduler =
            LearningRateScheduler::new(SchedulerType::Exponential { gamma: 0.9 }, 0.1);

        // Run for several steps
        for _ in 0..5 {
            scheduler.step();
        }
        assert!(scheduler.get_lr() < 0.1, "LR should have decayed");

        // Reset and verify
        scheduler.reset();
        assert_close(scheduler.get_lr(), 0.1, "Reset LR");
        assert_eq!(scheduler.step_count, 0, "Reset step count");
    }

    #[test]
    fn test_scheduler_cloning() {
        let scheduler1 = LearningRateScheduler::new(
            SchedulerType::StepDecay {
                step_size: 10,
                gamma: 0.5,
            },
            0.01,
        );

        let mut scheduler2 = scheduler1.clone();

        // Advance clone
        scheduler2.step();

        // Original should be unchanged
        assert_close(scheduler1.get_lr(), 0.01, "Original LR");
        assert_close(scheduler2.get_lr(), 0.01, "Clone LR after step");
    }

    #[test]
    fn test_multiple_scheduler_types() {
        let schedulers = vec![
            (SchedulerType::Constant, 0.01),
            (
                SchedulerType::StepDecay {
                    step_size: 5,
                    gamma: 0.9,
                },
                0.01,
            ),
            (SchedulerType::Exponential { gamma: 0.95 }, 0.01),
            (
                SchedulerType::CosineAnnealing {
                    t_max: 10,
                    eta_min: 0.001,
                },
                0.01,
            ),
            (
                SchedulerType::WarmupLinear {
                    warmup_steps: 5,
                    total_steps: 20,
                },
                0.01,
            ),
            (
                SchedulerType::ReduceOnPlateau {
                    factor: 0.5,
                    patience: 5,
                    min_lr: 0.0001,
                },
                0.01,
            ),
        ];

        for (sched_type, base_lr) in schedulers {
            let mut scheduler = LearningRateScheduler::new(sched_type, base_lr);

            // All schedulers should start at base_lr
            assert_close(scheduler.get_lr(), base_lr, "Initial LR for scheduler type");

            // All schedulers should be able to step
            let _ = scheduler.step();
            assert!(scheduler.get_lr() >= 0.0, "LR should be non-negative");
        }
    }

    #[test]
    fn test_edge_cases() {
        // Zero learning rate
        let mut scheduler = LearningRateScheduler::new(SchedulerType::Constant, 0.0);
        assert_close(scheduler.get_lr(), 0.0, "Zero LR");
        scheduler.step();
        assert_close(scheduler.get_lr(), 0.0, "Zero LR after step");

        // Very small gamma
        let mut scheduler =
            LearningRateScheduler::new(SchedulerType::Exponential { gamma: 0.1 }, 1.0);
        for _ in 0..10 {
            scheduler.step();
        }
        assert!(scheduler.get_lr() > 0.0, "LR should remain positive");
        assert!(scheduler.get_lr() < 1e-8, "LR should be very small");
    }
}
