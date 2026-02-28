//! # Tier 2: Adaptive Simulation and Digital Twins
//!
//! Industrial systems, cities, logistics.
//!
//! ## What Changes
//! - Simulation runs continuously at low fidelity
//! - High fidelity kicks in during "bullet time"
//! - Learning improves predictive accuracy
//!
//! ## Why This Matters
//! - Prediction becomes proactive
//! - Simulation is always warm, never cold-started
//! - Costs scale with relevance, not size
//!
//! This is underexplored and powerful.

use std::collections::{HashMap, VecDeque};

/// A digital twin component
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ComponentId(pub String);

/// Fidelity level of simulation
#[derive(Clone, Debug, PartialEq)]
pub enum FidelityLevel {
    /// Coarse-grained, fast, low accuracy
    Low { time_step_ms: u64, accuracy: f32 },
    /// Moderate detail
    Medium { time_step_ms: u64, accuracy: f32 },
    /// Full physics simulation
    High { time_step_ms: u64, accuracy: f32 },
    /// Maximum fidelity for critical moments
    BulletTime { time_step_ms: u64, accuracy: f32 },
}

impl FidelityLevel {
    pub fn compute_cost(&self) -> f32 {
        match self {
            FidelityLevel::Low { .. } => 1.0,
            FidelityLevel::Medium { .. } => 10.0,
            FidelityLevel::High { .. } => 100.0,
            FidelityLevel::BulletTime { .. } => 1000.0,
        }
    }

    pub fn time_step_ms(&self) -> u64 {
        match self {
            FidelityLevel::Low { time_step_ms, .. } => *time_step_ms,
            FidelityLevel::Medium { time_step_ms, .. } => *time_step_ms,
            FidelityLevel::High { time_step_ms, .. } => *time_step_ms,
            FidelityLevel::BulletTime { time_step_ms, .. } => *time_step_ms,
        }
    }
}

/// State of a simulated component
#[derive(Clone, Debug)]
pub struct ComponentState {
    pub id: ComponentId,
    pub position: (f32, f32, f32),
    pub velocity: (f32, f32, f32),
    pub properties: HashMap<String, f32>,
    pub predicted_trajectory: Vec<(f32, f32, f32)>,
}

/// Prediction from the simulation
#[derive(Clone, Debug)]
pub struct Prediction {
    pub component: ComponentId,
    pub timestamp: u64,
    pub predicted_value: f32,
    pub confidence: f32,
    pub horizon_ms: u64,
}

/// Actual measurement from the real system
#[derive(Clone, Debug)]
pub struct Measurement {
    pub component: ComponentId,
    pub timestamp: u64,
    pub actual_value: f32,
    pub sensor_id: String,
}

/// Predictive error for learning
#[derive(Clone, Debug)]
pub struct PredictionError {
    pub component: ComponentId,
    pub timestamp: u64,
    pub predicted: f32,
    pub actual: f32,
    pub error: f32,
    pub fidelity_at_prediction: FidelityLevel,
}

/// Adaptive fidelity controller
pub struct FidelityController {
    pub current_fidelity: FidelityLevel,
    pub urgency_threshold_high: f32,
    pub urgency_threshold_low: f32,
    pub bullet_time_until: u64,
    pub error_history: VecDeque<f32>,
}

impl FidelityController {
    pub fn new() -> Self {
        Self {
            current_fidelity: FidelityLevel::Low {
                time_step_ms: 100,
                accuracy: 0.7,
            },
            urgency_threshold_high: 0.8,
            urgency_threshold_low: 0.3,
            bullet_time_until: 0,
            error_history: VecDeque::new(),
        }
    }

    /// Decide fidelity based on system state
    pub fn decide(&mut self, urgency: f32, timestamp: u64) -> FidelityLevel {
        // Bullet time takes priority
        if timestamp < self.bullet_time_until {
            return FidelityLevel::BulletTime {
                time_step_ms: 1,
                accuracy: 0.99,
            };
        }

        // Adapt based on urgency
        if urgency > self.urgency_threshold_high {
            self.current_fidelity = FidelityLevel::High {
                time_step_ms: 10,
                accuracy: 0.95,
            };
        } else if urgency > 0.5 {
            self.current_fidelity = FidelityLevel::Medium {
                time_step_ms: 50,
                accuracy: 0.85,
            };
        } else if urgency < self.urgency_threshold_low {
            self.current_fidelity = FidelityLevel::Low {
                time_step_ms: 100,
                accuracy: 0.7,
            };
        }

        self.current_fidelity.clone()
    }

    /// Activate bullet time for a duration
    pub fn activate_bullet_time(&mut self, duration_ms: u64, current_time: u64) {
        self.bullet_time_until = current_time + duration_ms;
        println!("  [BULLET TIME] Activated for {}ms", duration_ms);
    }

    /// Track prediction error for adaptive learning
    pub fn record_error(&mut self, error: f32) {
        self.error_history.push_back(error.abs());
        if self.error_history.len() > 100 {
            self.error_history.pop_front();
        }
    }

    /// Get average recent error
    pub fn average_error(&self) -> f32 {
        if self.error_history.is_empty() {
            return 0.0;
        }
        self.error_history.iter().sum::<f32>() / self.error_history.len() as f32
    }
}

/// Predictive model that learns from errors
pub struct PredictiveModel {
    pub weights: HashMap<String, f32>,
    pub learning_rate: f32,
    pub bias: f32,
    pub predictions_made: u64,
    pub cumulative_error: f32,
}

impl PredictiveModel {
    pub fn new() -> Self {
        Self {
            weights: HashMap::new(),
            learning_rate: 0.01,
            bias: 0.0,
            predictions_made: 0,
            cumulative_error: 0.0,
        }
    }

    /// Make prediction based on current state
    pub fn predict(&mut self, state: &ComponentState, horizon_ms: u64) -> Prediction {
        self.predictions_made += 1;

        // Simple linear extrapolation + learned bias
        let (x, y, z) = state.velocity;
        let dt = horizon_ms as f32 / 1000.0;

        let predicted = state.position.0 + x * dt + self.bias;

        Prediction {
            component: state.id.clone(),
            timestamp: 0, // Will be set by caller
            predicted_value: predicted,
            confidence: 0.8 - (self.average_error() * 0.5).min(0.5),
            horizon_ms,
        }
    }

    /// Learn from prediction error
    pub fn learn(&mut self, error: &PredictionError) {
        // Simple gradient descent
        self.bias -= self.learning_rate * error.error;
        self.cumulative_error += error.error.abs();
    }

    pub fn average_error(&self) -> f32 {
        if self.predictions_made == 0 {
            return 0.0;
        }
        self.cumulative_error / self.predictions_made as f32
    }
}

/// Digital twin simulation system
pub struct DigitalTwin {
    pub name: String,
    pub components: HashMap<ComponentId, ComponentState>,
    pub fidelity: FidelityController,
    pub model: PredictiveModel,
    pub predictions: VecDeque<Prediction>,
    pub simulation_time: u64,
    pub real_time: u64,
    pub total_compute_cost: f32,
}

impl DigitalTwin {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            components: HashMap::new(),
            fidelity: FidelityController::new(),
            model: PredictiveModel::new(),
            predictions: VecDeque::new(),
            simulation_time: 0,
            real_time: 0,
            total_compute_cost: 0.0,
        }
    }

    /// Add component to simulation
    pub fn add_component(
        &mut self,
        id: &str,
        position: (f32, f32, f32),
        velocity: (f32, f32, f32),
    ) {
        self.components.insert(
            ComponentId(id.to_string()),
            ComponentState {
                id: ComponentId(id.to_string()),
                position,
                velocity,
                properties: HashMap::new(),
                predicted_trajectory: Vec::new(),
            },
        );
    }

    /// Compute urgency based on system state
    pub fn compute_urgency(&self) -> f32 {
        let mut max_urgency = 0.0f32;

        for state in self.components.values() {
            // High velocity = high urgency
            let speed =
                (state.velocity.0.powi(2) + state.velocity.1.powi(2) + state.velocity.2.powi(2))
                    .sqrt();

            max_urgency = max_urgency.max(speed / 100.0); // Normalize

            // Check for collision risk
            for other in self.components.values() {
                if state.id != other.id {
                    let dist = ((state.position.0 - other.position.0).powi(2)
                        + (state.position.1 - other.position.1).powi(2))
                    .sqrt();

                    if dist < 10.0 {
                        max_urgency = max_urgency.max(1.0 - dist / 10.0);
                    }
                }
            }
        }

        max_urgency.min(1.0)
    }

    /// Step simulation forward
    pub fn step(&mut self, real_dt_ms: u64) {
        self.real_time += real_dt_ms;

        // Compute urgency and decide fidelity
        let urgency = self.compute_urgency();
        let fidelity = self.fidelity.decide(urgency, self.real_time);

        let sim_dt = fidelity.time_step_ms();
        let cost = fidelity.compute_cost();
        self.total_compute_cost += cost * (real_dt_ms as f32 / sim_dt as f32);

        // Update simulation
        self.simulation_time += sim_dt;

        for state in self.components.values_mut() {
            let dt = sim_dt as f32 / 1000.0;
            state.position.0 += state.velocity.0 * dt;
            state.position.1 += state.velocity.1 * dt;
            state.position.2 += state.velocity.2 * dt;

            // Generate prediction
            let prediction = self.model.predict(state, 1000);
            state.predicted_trajectory.push((
                prediction.predicted_value,
                state.position.1,
                state.position.2,
            ));

            // Keep trajectory bounded
            if state.predicted_trajectory.len() > 100 {
                state.predicted_trajectory.remove(0);
            }
        }

        // Check for bullet time triggers
        if urgency > 0.9 {
            self.fidelity.activate_bullet_time(100, self.real_time);
        }
    }

    /// Receive real measurement and learn
    pub fn receive_measurement(&mut self, measurement: Measurement) {
        // Find matching prediction
        if let Some(prediction) = self.predictions.iter().find(|p| {
            p.component == measurement.component
                && (measurement.timestamp as i64 - p.timestamp as i64).abs() < 100
        }) {
            let error = PredictionError {
                component: measurement.component.clone(),
                timestamp: measurement.timestamp,
                predicted: prediction.predicted_value,
                actual: measurement.actual_value,
                error: prediction.predicted_value - measurement.actual_value,
                fidelity_at_prediction: self.fidelity.current_fidelity.clone(),
            };

            // Learn from error
            self.model.learn(&error);
            self.fidelity.record_error(error.error);

            // Update component state with actual
            if let Some(state) = self.components.get_mut(&measurement.component) {
                state.position.0 = measurement.actual_value;
            }
        }
    }

    /// Get simulation efficiency
    pub fn efficiency_ratio(&self) -> f32 {
        // Compare actual compute to always-high-fidelity
        let always_high_cost = self.real_time as f32 * 100.0;
        if self.total_compute_cost > 0.0 {
            always_high_cost / self.total_compute_cost
        } else {
            1.0
        }
    }
}

fn main() {
    println!("=== Tier 2: Adaptive Simulation and Digital Twins ===\n");

    let mut twin = DigitalTwin::new("Industrial System");

    // Add components
    twin.add_component("conveyor_1", (0.0, 0.0, 0.0), (10.0, 0.0, 0.0));
    twin.add_component("robot_arm", (50.0, 10.0, 0.0), (0.0, 5.0, 0.0));
    twin.add_component("package_a", (0.0, 0.0, 1.0), (15.0, 0.0, 0.0));

    println!(
        "Digital twin initialized with {} components",
        twin.components.len()
    );

    // Simulate normal operation (low fidelity, low cost)
    println!("\nNormal operation (low fidelity)...");
    for i in 0..100 {
        twin.step(10);

        if i % 20 == 0 {
            let urgency = twin.compute_urgency();
            println!(
                "  t={}: urgency={:.2}, fidelity={:?}",
                twin.simulation_time, urgency, twin.fidelity.current_fidelity
            );
        }
    }

    println!("\n  Compute cost so far: {:.1}", twin.total_compute_cost);
    println!(
        "  Efficiency vs always-high: {:.1}x",
        twin.efficiency_ratio()
    );

    // Create collision scenario (triggers high fidelity)
    println!("\nCreating collision scenario...");
    if let Some(pkg) = twin.components.get_mut(&ComponentId("package_a".into())) {
        pkg.velocity = (50.0, 0.0, 0.0); // Fast moving
    }
    if let Some(robot) = twin.components.get_mut(&ComponentId("robot_arm".into())) {
        robot.position = (55.0, 5.0, 0.0); // In path
    }

    for i in 0..20 {
        twin.step(10);

        let urgency = twin.compute_urgency();
        if i % 5 == 0 || urgency > 0.5 {
            println!(
                "  t={}: urgency={:.2}, fidelity={:?}",
                twin.simulation_time, urgency, twin.fidelity.current_fidelity
            );
        }
    }

    // Simulate receiving real measurements
    println!("\nReceiving real measurements (learning)...");
    for i in 0..10 {
        let measurement = Measurement {
            component: ComponentId("conveyor_1".into()),
            timestamp: twin.real_time,
            actual_value: 100.0 + i as f32 * 10.0 + (i as f32 * 0.1).sin() * 2.0,
            sensor_id: "sensor_1".to_string(),
        };

        // First make a prediction
        if let Some(state) = twin.components.get(&ComponentId("conveyor_1".into())) {
            let prediction = twin.model.predict(state, 100);
            twin.predictions.push_back(Prediction {
                timestamp: twin.real_time,
                ..prediction
            });
        }

        twin.receive_measurement(measurement);
        twin.step(100);
    }

    println!("  Model average error: {:.3}", twin.model.average_error());
    println!("  Predictions made: {}", twin.model.predictions_made);

    // Summary
    println!("\n=== Final Statistics ===");
    println!("  Real time simulated: {}ms", twin.real_time);
    println!("  Simulation time: {}ms", twin.simulation_time);
    println!("  Total compute cost: {:.1}", twin.total_compute_cost);
    println!("  Efficiency ratio: {:.1}x", twin.efficiency_ratio());
    println!("  Current fidelity: {:?}", twin.fidelity.current_fidelity);

    println!("\n=== Key Benefits ===");
    println!("- Simulation always warm, never cold-started");
    println!("- Costs scale with relevance, not system size");
    println!("- Bullet time for critical moments");
    println!("- Continuous learning improves predictions");
    println!("- Proactive prediction instead of reactive analysis");
    println!("\nThis is underexplored and powerful.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fidelity_controller() {
        let mut controller = FidelityController::new();

        // Low urgency = low fidelity
        let fidelity = controller.decide(0.1, 0);
        assert!(matches!(fidelity, FidelityLevel::Low { .. }));

        // High urgency = high fidelity
        let fidelity = controller.decide(0.9, 1);
        assert!(matches!(fidelity, FidelityLevel::High { .. }));
    }

    #[test]
    fn test_bullet_time() {
        let mut controller = FidelityController::new();

        controller.activate_bullet_time(100, 0);
        let fidelity = controller.decide(0.1, 50); // Still in bullet time
        assert!(matches!(fidelity, FidelityLevel::BulletTime { .. }));

        let fidelity = controller.decide(0.1, 150); // After bullet time
        assert!(!matches!(fidelity, FidelityLevel::BulletTime { .. }));
    }

    #[test]
    fn test_predictive_model_learning() {
        let mut model = PredictiveModel::new();

        // Make predictions and learn from errors
        for _ in 0..10 {
            let error = PredictionError {
                component: ComponentId("test".into()),
                timestamp: 0,
                predicted: 1.0,
                actual: 0.9,
                error: 0.1,
                fidelity_at_prediction: FidelityLevel::Low {
                    time_step_ms: 100,
                    accuracy: 0.7,
                },
            };
            model.learn(&error);
        }

        // Bias should have adjusted
        assert!(model.bias != 0.0);
    }

    #[test]
    fn test_digital_twin_efficiency() {
        let mut twin = DigitalTwin::new("test");
        twin.add_component("a", (0.0, 0.0, 0.0), (1.0, 0.0, 0.0));

        // Low urgency operation should be efficient
        for _ in 0..100 {
            twin.step(10);
        }

        assert!(twin.efficiency_ratio() > 5.0); // Should be much more efficient
    }
}
