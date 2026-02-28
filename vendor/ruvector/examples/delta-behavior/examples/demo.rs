//! # Delta-Behavior Demo
//!
//! This example demonstrates the core concepts of delta-behavior:
//! - Coherence as a measure of system stability
//! - Transitions that are gated by coherence bounds
//! - Enforcement that blocks destabilizing operations
//! - Attractor guidance toward stable states
//!
//! Run with: `cargo run --example demo`

use delta_behavior::{
    Coherence, CoherenceBounds, DeltaConfig, DeltaEnforcer, DeltaSystem,
    EnforcementResult,
};

fn main() {
    println!("=== Delta-Behavior Demo ===\n");

    demo_coherence();
    demo_enforcement();
    demo_delta_system();
    demo_attractor_guidance();

    println!("\n=== Demo Complete ===");
}

/// Demonstrate coherence measurement and bounds.
fn demo_coherence() {
    println!("--- 1. Coherence Basics ---\n");

    // Create coherence values
    let high = Coherence::new(0.9).unwrap();
    let medium = Coherence::new(0.5).unwrap();
    let low = Coherence::new(0.2).unwrap();

    println!("High coherence:   {:.2}", high.value());
    println!("Medium coherence: {:.2}", medium.value());
    println!("Low coherence:    {:.2}", low.value());

    // Check bounds
    let bounds = CoherenceBounds::default();
    println!("\nDefault bounds:");
    println!("  Minimum:  {:.2}", bounds.min_coherence.value());
    println!("  Throttle: {:.2}", bounds.throttle_threshold.value());
    println!("  Target:   {:.2}", bounds.target_coherence.value());

    // Check against bounds
    println!("\nCoherence above minimum?");
    println!("  High:   {}", high.value() >= bounds.min_coherence.value());
    println!("  Medium: {}", medium.value() >= bounds.min_coherence.value());
    println!("  Low:    {}", low.value() >= bounds.min_coherence.value());
    println!();
}

/// Demonstrate enforcement of coherence bounds.
fn demo_enforcement() {
    println!("--- 2. Enforcement ---\n");

    let config = DeltaConfig::default();
    let mut enforcer = DeltaEnforcer::new(config);

    // Try various transitions
    let test_cases = [
        (0.8, 0.75, "small drop"),
        (0.8, 0.65, "medium drop"),
        (0.8, 0.45, "large drop"),
        (0.8, 0.25, "destabilizing drop"),
        (0.4, 0.35, "below throttle"),
    ];

    println!("Testing transitions (current -> predicted):\n");

    for (current, predicted, description) in test_cases {
        let current_c = Coherence::new(current).unwrap();
        let predicted_c = Coherence::new(predicted).unwrap();

        let result = enforcer.check(current_c, predicted_c);

        let status = match &result {
            EnforcementResult::Allowed => "ALLOWED",
            EnforcementResult::Blocked(_) => "BLOCKED",
            EnforcementResult::Throttled(_) => "THROTTLED",
        };

        println!(
            "  {:.2} -> {:.2} ({:20}): {}",
            current, predicted, description, status
        );

        // Tick to regenerate energy
        enforcer.tick();
    }
    println!();
}

/// Demonstrate a system implementing DeltaSystem.
fn demo_delta_system() {
    println!("--- 3. Delta System ---\n");

    let mut system = SimpleSystem::new();

    println!("Initial state: {:.2}, coherence: {:.3}", system.state(), system.coherence().value());
    println!("In attractor: {}\n", system.in_attractor());

    // Apply a series of transitions
    let transitions = [0.5, 0.5, 1.0, 2.0, 5.0, 10.0];

    for delta in transitions {
        let predicted = system.predict_coherence(&delta);
        println!("Attempting delta={:.1}:", delta);
        println!("  Predicted coherence: {:.3}", predicted.value());

        match system.step(&delta) {
            Ok(()) => {
                println!("  Result: SUCCESS");
                println!("  New state: {:.2}, coherence: {:.3}", system.state(), system.coherence().value());
            }
            Err(e) => {
                println!("  Result: BLOCKED - {}", e);
            }
        }
        println!();
    }
}

/// Demonstrate attractor guidance.
fn demo_attractor_guidance() {
    println!("--- 4. Attractor Guidance ---\n");

    use delta_behavior::attractor::GuidanceForce;

    // Current position far from attractor at origin
    let position = [5.0, 3.0];
    let attractor = [0.0, 0.0];

    let force = GuidanceForce::toward(&position, &attractor, 1.0);

    println!("Position: ({:.1}, {:.1})", position[0], position[1]);
    println!("Attractor: ({:.1}, {:.1})", attractor[0], attractor[1]);
    println!("Guidance force:");
    println!("  Direction: ({:.3}, {:.3})", force.direction[0], force.direction[1]);
    println!("  Magnitude: {:.3}", force.magnitude);

    // Simulate movement toward attractor
    println!("\nSimulating movement toward attractor:");
    let mut pos = position;
    for step in 0..5 {
        let f = GuidanceForce::toward(&pos, &attractor, 0.3);
        pos[0] += f.direction[0] * f.magnitude;
        pos[1] += f.direction[1] * f.magnitude;

        let dist = (pos[0].powi(2) + pos[1].powi(2)).sqrt();
        println!(
            "  Step {}: ({:.2}, {:.2}), distance to attractor: {:.2}",
            step + 1, pos[0], pos[1], dist
        );
    }
    println!();
}

// ============================================================================
// Simple System Implementation
// ============================================================================

/// A simple system demonstrating delta-behavior.
struct SimpleSystem {
    state: f64,
    coherence: Coherence,
}

impl SimpleSystem {
    fn new() -> Self {
        Self {
            state: 0.0,
            coherence: Coherence::maximum(),
        }
    }
}

impl DeltaSystem for SimpleSystem {
    type State = f64;
    type Transition = f64;
    type Error = &'static str;

    fn coherence(&self) -> Coherence {
        self.coherence
    }

    fn step(&mut self, delta: &f64) -> Result<(), Self::Error> {
        // Predict the outcome
        let predicted = self.predict_coherence(delta);

        // Enforce coherence bound
        if predicted.value() < 0.3 {
            return Err("Would violate minimum coherence bound");
        }

        // Check for excessive drop
        if self.coherence.value() - predicted.value() > 0.15 {
            return Err("Transition too destabilizing");
        }

        // Apply the transition
        self.state += delta;
        self.coherence = predicted;

        Ok(())
    }

    fn predict_coherence(&self, delta: &f64) -> Coherence {
        // Larger deltas cause more coherence loss
        // Models uncertainty accumulation
        let impact = delta.abs() * 0.1;
        Coherence::clamped(self.coherence.value() - impact)
    }

    fn state(&self) -> &f64 {
        &self.state
    }

    fn in_attractor(&self) -> bool {
        // System is "attracted" to the origin
        self.state.abs() < 0.5
    }
}
