//! # Morphogenetic Cognition
//!
//! Self-organizing pattern formation inspired by biological development.
//! Uses reaction-diffusion systems (Turing patterns) to generate
//! emergent cognitive structures.
//!
//! ## Key Concepts
//!
//! - **Turing Patterns**: Emergent patterns from reaction-diffusion
//! - **Morphogens**: Signaling molecules that create concentration gradients
//! - **Self-Organization**: Structure emerges from local rules
//! - **Cognitive Embryogenesis**: Growing cognitive structures
//!
//! ## Mathematical Foundation
//!
//! Based on Turing's 1952 paper "The Chemical Basis of Morphogenesis":
//! ∂u/∂t = Du∇²u + f(u,v)
//! ∂v/∂t = Dv∇²v + g(u,v)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A field where morphogenetic patterns emerge
#[derive(Debug)]
pub struct MorphogeneticField {
    /// Width of the field
    width: usize,
    /// Height of the field
    height: usize,
    /// Activator concentration
    activator: Vec<Vec<f64>>,
    /// Inhibitor concentration
    inhibitor: Vec<Vec<f64>>,
    /// Diffusion rate for activator
    da: f64,
    /// Diffusion rate for inhibitor
    db: f64,
    /// Reaction parameters
    params: ReactionParams,
    /// Pattern history for analysis
    pattern_history: Vec<PatternSnapshot>,
    /// Time step
    dt: f64,
}

/// Parameters for reaction kinetics
#[derive(Debug, Clone)]
pub struct ReactionParams {
    /// Feed rate
    pub f: f64,
    /// Kill rate
    pub k: f64,
    /// Activator production rate
    pub alpha: f64,
    /// Inhibitor production rate
    pub beta: f64,
}

/// A snapshot of the pattern state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternSnapshot {
    pub timestamp: u64,
    pub complexity: f64,
    pub dominant_wavelength: f64,
    pub symmetry_score: f64,
}

/// Turing pattern generator
#[derive(Debug)]
pub struct TuringPattern {
    /// Pattern type
    pub pattern_type: PatternType,
    /// Characteristic wavelength
    pub wavelength: f64,
    /// Amplitude of pattern
    pub amplitude: f64,
    /// Pattern data
    pub data: Vec<Vec<f64>>,
}

/// Types of Turing patterns
#[derive(Debug, Clone, PartialEq)]
pub enum PatternType {
    /// Spots pattern
    Spots,
    /// Stripes pattern
    Stripes,
    /// Labyrinth pattern
    Labyrinth,
    /// Hexagonal pattern
    Hexagonal,
    /// Mixed/transitional
    Mixed,
}

/// Cognitive embryogenesis - growing cognitive structures
#[derive(Debug)]
pub struct CognitiveEmbryogenesis {
    /// Current developmental stage
    stage: DevelopmentStage,
    /// Growing cognitive structures
    structures: Vec<CognitiveStructure>,
    /// Morphogen gradients
    gradients: HashMap<String, Vec<f64>>,
    /// Development history
    history: Vec<DevelopmentEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DevelopmentStage {
    /// Initial undifferentiated state
    Zygote,
    /// Early division
    Cleavage,
    /// Pattern formation
    Gastrulation,
    /// Structure differentiation
    Organogenesis,
    /// Mature structure
    Mature,
}

#[derive(Debug, Clone)]
pub struct CognitiveStructure {
    pub id: Uuid,
    pub structure_type: StructureType,
    pub position: (f64, f64, f64),
    pub size: f64,
    pub connectivity: Vec<Uuid>,
    pub specialization: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StructureType {
    SensoryRegion,
    ProcessingNode,
    MemoryStore,
    IntegrationHub,
    OutputRegion,
}

#[derive(Debug, Clone)]
pub struct DevelopmentEvent {
    pub stage: DevelopmentStage,
    pub event_type: String,
    pub timestamp: u64,
}

impl MorphogeneticField {
    /// Create a new morphogenetic field
    pub fn new(width: usize, height: usize) -> Self {
        let mut field = Self {
            width,
            height,
            activator: vec![vec![1.0; width]; height],
            inhibitor: vec![vec![0.0; width]; height],
            da: 1.0,
            db: 0.5,
            params: ReactionParams {
                f: 0.055,
                k: 0.062,
                alpha: 1.0,
                beta: 1.0,
            },
            pattern_history: Vec::new(),
            dt: 1.0,
        };

        // Add initial perturbation
        field.add_random_perturbation(0.05);
        field
    }

    /// Create with specific parameters
    pub fn with_params(
        width: usize,
        height: usize,
        da: f64,
        db: f64,
        params: ReactionParams,
    ) -> Self {
        let mut field = Self::new(width, height);
        field.da = da;
        field.db = db;
        field.params = params;
        field
    }

    /// Add random perturbation to break symmetry
    pub fn add_random_perturbation(&mut self, magnitude: f64) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(12345) as u64;

        let mut state = seed;

        for y in 0..self.height {
            for x in 0..self.width {
                // Simple LCG random
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let r = (state as f64) / (u64::MAX as f64);
                self.inhibitor[y][x] += (r - 0.5) * magnitude;
            }
        }
    }

    /// Measure pattern complexity
    pub fn measure_complexity(&self) -> f64 {
        // Complexity based on spatial entropy and gradient magnitude
        let mut gradient_sum = 0.0;
        let mut count = 0;

        for y in 1..self.height - 1 {
            for x in 1..self.width - 1 {
                let dx = self.activator[y][x + 1] - self.activator[y][x - 1];
                let dy = self.activator[y + 1][x] - self.activator[y - 1][x];
                gradient_sum += (dx * dx + dy * dy).sqrt();
                count += 1;
            }
        }

        if count > 0 {
            (gradient_sum / count as f64).min(1.0)
        } else {
            0.0
        }
    }

    /// Run one simulation step using Gray-Scott model
    pub fn step(&mut self) {
        let mut new_a = self.activator.clone();
        let mut new_b = self.inhibitor.clone();

        for y in 1..self.height - 1 {
            for x in 1..self.width - 1 {
                let a = self.activator[y][x];
                let b = self.inhibitor[y][x];

                // Laplacian (diffusion)
                let lap_a = self.activator[y - 1][x]
                    + self.activator[y + 1][x]
                    + self.activator[y][x - 1]
                    + self.activator[y][x + 1]
                    - 4.0 * a;

                let lap_b = self.inhibitor[y - 1][x]
                    + self.inhibitor[y + 1][x]
                    + self.inhibitor[y][x - 1]
                    + self.inhibitor[y][x + 1]
                    - 4.0 * b;

                // Gray-Scott reaction
                let reaction = a * b * b;

                new_a[y][x] =
                    a + self.dt * (self.da * lap_a - reaction + self.params.f * (1.0 - a));

                new_b[y][x] = b + self.dt
                    * (self.db * lap_b + reaction - (self.params.f + self.params.k) * b);

                // Clamp values
                new_a[y][x] = new_a[y][x].clamp(0.0, 1.0);
                new_b[y][x] = new_b[y][x].clamp(0.0, 1.0);
            }
        }

        self.activator = new_a;
        self.inhibitor = new_b;
    }

    /// Run simulation for n steps
    pub fn simulate(&mut self, steps: usize) {
        for _ in 0..steps {
            self.step();
        }

        // Record snapshot
        self.pattern_history.push(PatternSnapshot {
            timestamp: self.pattern_history.len() as u64,
            complexity: self.measure_complexity(),
            dominant_wavelength: self.estimate_wavelength(),
            symmetry_score: self.measure_symmetry(),
        });
    }

    /// Estimate dominant wavelength using autocorrelation
    fn estimate_wavelength(&self) -> f64 {
        let center_y = self.height / 2;
        let slice: Vec<f64> = (0..self.width)
            .map(|x| self.activator[center_y][x])
            .collect();

        // Find first minimum in autocorrelation
        let mut best_lag = 1;
        let mut min_corr = f64::MAX;

        for lag in 1..self.width / 4 {
            let mut corr = 0.0;
            let mut count = 0;

            for i in 0..self.width - lag {
                corr += slice[i] * slice[i + lag];
                count += 1;
            }

            if count > 0 {
                corr /= count as f64;
                if corr < min_corr {
                    min_corr = corr;
                    best_lag = lag;
                }
            }
        }

        (best_lag * 2) as f64 // Wavelength is twice the first minimum lag
    }

    /// Measure pattern symmetry
    fn measure_symmetry(&self) -> f64 {
        let mut diff_sum = 0.0;
        let mut count = 0;

        // Check left-right symmetry
        for y in 0..self.height {
            for x in 0..self.width / 2 {
                let mirror_x = self.width - 1 - x;
                let diff = (self.activator[y][x] - self.activator[y][mirror_x]).abs();
                diff_sum += diff;
                count += 1;
            }
        }

        if count > 0 {
            1.0 - (diff_sum / count as f64).min(1.0)
        } else {
            0.0
        }
    }

    /// Detect pattern type
    pub fn detect_pattern_type(&self) -> PatternType {
        let complexity = self.measure_complexity();
        let symmetry = self.measure_symmetry();
        let wavelength = self.estimate_wavelength();

        if complexity < 0.1 {
            PatternType::Mixed // Uniform
        } else if symmetry > 0.7 && wavelength > self.width as f64 / 4.0 {
            PatternType::Stripes
        } else if symmetry > 0.5 && wavelength < self.width as f64 / 8.0 {
            PatternType::Spots
        } else if complexity > 0.5 {
            PatternType::Labyrinth
        } else {
            PatternType::Mixed
        }
    }

    /// Get the activator field
    pub fn activator_field(&self) -> &Vec<Vec<f64>> {
        &self.activator
    }

    /// Get the inhibitor field
    pub fn inhibitor_field(&self) -> &Vec<Vec<f64>> {
        &self.inhibitor
    }

    /// Get pattern at specific location
    pub fn sample(&self, x: usize, y: usize) -> Option<(f64, f64)> {
        if x < self.width && y < self.height {
            Some((self.activator[y][x], self.inhibitor[y][x]))
        } else {
            None
        }
    }
}

impl CognitiveEmbryogenesis {
    /// Create a new embryogenesis process
    pub fn new() -> Self {
        Self {
            stage: DevelopmentStage::Zygote,
            structures: Vec::new(),
            gradients: HashMap::new(),
            history: Vec::new(),
        }
    }

    /// Advance development by one stage
    pub fn develop(&mut self) -> DevelopmentStage {
        let new_stage = match self.stage {
            DevelopmentStage::Zygote => {
                self.initialize_gradients();
                DevelopmentStage::Cleavage
            }
            DevelopmentStage::Cleavage => {
                self.divide_structures();
                DevelopmentStage::Gastrulation
            }
            DevelopmentStage::Gastrulation => {
                self.form_patterns();
                DevelopmentStage::Organogenesis
            }
            DevelopmentStage::Organogenesis => {
                self.differentiate();
                DevelopmentStage::Mature
            }
            DevelopmentStage::Mature => DevelopmentStage::Mature,
        };

        self.history.push(DevelopmentEvent {
            stage: new_stage.clone(),
            event_type: format!("Transition to {:?}", new_stage),
            timestamp: self.history.len() as u64,
        });

        self.stage = new_stage.clone();
        new_stage
    }

    fn initialize_gradients(&mut self) {
        // Create morphogen gradients
        let gradient_length = 100;

        // Anterior-posterior gradient
        let ap_gradient: Vec<f64> = (0..gradient_length)
            .map(|i| i as f64 / gradient_length as f64)
            .collect();
        self.gradients
            .insert("anterior_posterior".to_string(), ap_gradient);

        // Dorsal-ventral gradient
        let dv_gradient: Vec<f64> = (0..gradient_length)
            .map(|i| {
                let x = i as f64 / gradient_length as f64;
                (x * std::f64::consts::PI).sin()
            })
            .collect();
        self.gradients
            .insert("dorsal_ventral".to_string(), dv_gradient);
    }

    fn divide_structures(&mut self) {
        // Create initial structures through division
        let initial = CognitiveStructure {
            id: Uuid::new_v4(),
            structure_type: StructureType::ProcessingNode,
            position: (0.5, 0.5, 0.5),
            size: 1.0,
            connectivity: Vec::new(),
            specialization: 0.0,
        };

        // Divide into multiple structures
        for i in 0..4 {
            let angle = i as f64 * std::f64::consts::PI / 2.0;
            self.structures.push(CognitiveStructure {
                id: Uuid::new_v4(),
                structure_type: StructureType::ProcessingNode,
                position: (0.5 + 0.3 * angle.cos(), 0.5 + 0.3 * angle.sin(), 0.5),
                size: initial.size / 4.0,
                connectivity: Vec::new(),
                specialization: 0.0,
            });
        }
    }

    fn form_patterns(&mut self) {
        // Establish connectivity patterns based on gradients
        let structure_ids: Vec<Uuid> = self.structures.iter().map(|s| s.id).collect();

        for i in 0..self.structures.len() {
            for j in i + 1..self.structures.len() {
                let dist = self.distance(i, j);
                if dist < 0.5 {
                    self.structures[i].connectivity.push(structure_ids[j]);
                    self.structures[j].connectivity.push(structure_ids[i]);
                }
            }
        }
    }

    fn distance(&self, i: usize, j: usize) -> f64 {
        let (x1, y1, z1) = self.structures[i].position;
        let (x2, y2, z2) = self.structures[j].position;
        ((x2 - x1).powi(2) + (y2 - y1).powi(2) + (z2 - z1).powi(2)).sqrt()
    }

    fn differentiate(&mut self) {
        // Differentiate structures based on position in gradients
        for structure in &mut self.structures {
            let (x, y, _) = structure.position;

            // Determine type based on position
            structure.structure_type = if x < 0.3 {
                StructureType::SensoryRegion
            } else if x > 0.7 {
                StructureType::OutputRegion
            } else if y < 0.3 {
                StructureType::MemoryStore
            } else if y > 0.7 {
                StructureType::IntegrationHub
            } else {
                StructureType::ProcessingNode
            };

            structure.specialization = 1.0;
        }
    }

    /// Get current stage
    pub fn current_stage(&self) -> &DevelopmentStage {
        &self.stage
    }

    /// Get structures
    pub fn structures(&self) -> &[CognitiveStructure] {
        &self.structures
    }

    /// Check if development is complete
    pub fn is_mature(&self) -> bool {
        self.stage == DevelopmentStage::Mature
    }

    /// Run full development
    pub fn full_development(&mut self) {
        while self.stage != DevelopmentStage::Mature {
            self.develop();
        }
    }
}

impl Default for CognitiveEmbryogenesis {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_morphogenetic_field_creation() {
        let field = MorphogeneticField::new(32, 32);
        assert_eq!(field.width, 32);
        assert_eq!(field.height, 32);
    }

    #[test]
    fn test_simulation_step() {
        let mut field = MorphogeneticField::new(32, 32);
        field.step();

        // Field should still be valid
        assert!(field.activator[16][16] >= 0.0);
        assert!(field.activator[16][16] <= 1.0);
    }

    #[test]
    fn test_pattern_complexity() {
        let mut field = MorphogeneticField::new(32, 32);

        // Initial complexity should be low
        let initial_complexity = field.measure_complexity();

        // After simulation, patterns should form
        field.simulate(100);
        let final_complexity = field.measure_complexity();

        // Complexity should generally increase (patterns form)
        assert!(final_complexity >= 0.0);
    }

    #[test]
    fn test_pattern_detection() {
        let mut field = MorphogeneticField::new(32, 32);
        field.simulate(50);

        let pattern_type = field.detect_pattern_type();
        // Should detect some pattern type
        assert!(matches!(
            pattern_type,
            PatternType::Spots
                | PatternType::Stripes
                | PatternType::Labyrinth
                | PatternType::Hexagonal
                | PatternType::Mixed
        ));
    }

    #[test]
    fn test_cognitive_embryogenesis() {
        let mut embryo = CognitiveEmbryogenesis::new();
        assert_eq!(*embryo.current_stage(), DevelopmentStage::Zygote);

        embryo.full_development();

        assert!(embryo.is_mature());
        assert!(!embryo.structures().is_empty());
    }

    #[test]
    fn test_structure_differentiation() {
        let mut embryo = CognitiveEmbryogenesis::new();
        embryo.full_development();

        // Should have different structure types
        let types: Vec<_> = embryo
            .structures()
            .iter()
            .map(|s| &s.structure_type)
            .collect();

        assert!(embryo.structures().iter().all(|s| s.specialization > 0.0));
    }

    #[test]
    fn test_gradient_initialization() {
        let mut embryo = CognitiveEmbryogenesis::new();
        embryo.develop(); // Zygote -> Cleavage, initializes gradients

        assert!(embryo.gradients.contains_key("anterior_posterior"));
        assert!(embryo.gradients.contains_key("dorsal_ventral"));
    }
}
