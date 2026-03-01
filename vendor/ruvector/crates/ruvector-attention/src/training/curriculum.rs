//! Curriculum learning for attention training
//!
//! Provides schedulers for progressive training difficulty.

/// Decay type for temperature/parameter annealing
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum DecayType {
    #[default]
    Linear,
    Exponential,
    Cosine,
    Step,
}

/// Curriculum learning stage
#[derive(Clone, Debug)]
pub struct CurriculumStage {
    pub name: String,
    pub difficulty: f32,       // 0.0 = easy, 1.0 = hard
    pub duration: usize,       // Steps in this stage
    pub temperature: f32,      // Softmax temperature
    pub negative_count: usize, // Number of negatives
}

impl CurriculumStage {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            difficulty: 0.5,
            duration: 1000,
            temperature: 1.0,
            negative_count: 10,
        }
    }

    pub fn difficulty(mut self, d: f32) -> Self {
        self.difficulty = d.clamp(0.0, 1.0);
        self
    }

    pub fn duration(mut self, d: usize) -> Self {
        self.duration = d;
        self
    }

    pub fn temperature(mut self, t: f32) -> Self {
        self.temperature = t.max(0.01);
        self
    }

    pub fn negative_count(mut self, n: usize) -> Self {
        self.negative_count = n.max(1);
        self
    }
}

/// Curriculum scheduler for progressive training
pub struct CurriculumScheduler {
    stages: Vec<CurriculumStage>,
    current_stage: usize,
    steps_in_stage: usize,
    total_steps: usize,
}

impl CurriculumScheduler {
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            current_stage: 0,
            steps_in_stage: 0,
            total_steps: 0,
        }
    }

    /// Add a stage to the curriculum
    pub fn add_stage(mut self, stage: CurriculumStage) -> Self {
        self.stages.push(stage);
        self
    }

    /// Build a default easy-to-hard curriculum
    pub fn default_curriculum(total_steps: usize) -> Self {
        let stage_duration = total_steps / 4;

        Self::new()
            .add_stage(
                CurriculumStage::new("warm_up")
                    .difficulty(0.1)
                    .duration(stage_duration)
                    .temperature(2.0)
                    .negative_count(5),
            )
            .add_stage(
                CurriculumStage::new("easy")
                    .difficulty(0.3)
                    .duration(stage_duration)
                    .temperature(1.0)
                    .negative_count(10),
            )
            .add_stage(
                CurriculumStage::new("medium")
                    .difficulty(0.6)
                    .duration(stage_duration)
                    .temperature(0.5)
                    .negative_count(20),
            )
            .add_stage(
                CurriculumStage::new("hard")
                    .difficulty(1.0)
                    .duration(stage_duration)
                    .temperature(0.1)
                    .negative_count(50),
            )
    }

    /// Get current stage
    pub fn current_stage(&self) -> Option<&CurriculumStage> {
        self.stages.get(self.current_stage)
    }

    /// Advance one step and return current stage
    pub fn step(&mut self) -> Option<&CurriculumStage> {
        if self.stages.is_empty() {
            return None;
        }

        self.steps_in_stage += 1;
        self.total_steps += 1;

        // Check if we should advance to next stage
        if let Some(stage) = self.stages.get(self.current_stage) {
            if self.steps_in_stage >= stage.duration && self.current_stage < self.stages.len() - 1 {
                self.current_stage += 1;
                self.steps_in_stage = 0;
            }
        }

        self.current_stage()
    }

    /// Get current difficulty (0.0 to 1.0)
    pub fn difficulty(&self) -> f32 {
        self.current_stage().map(|s| s.difficulty).unwrap_or(1.0)
    }

    /// Get current temperature
    pub fn temperature(&self) -> f32 {
        self.current_stage().map(|s| s.temperature).unwrap_or(1.0)
    }

    /// Get current negative count
    pub fn negative_count(&self) -> usize {
        self.current_stage().map(|s| s.negative_count).unwrap_or(10)
    }

    /// Check if training is complete
    pub fn is_complete(&self) -> bool {
        if self.stages.is_empty() {
            return true;
        }
        self.current_stage >= self.stages.len() - 1
            && self.steps_in_stage >= self.stages.last().map(|s| s.duration).unwrap_or(0)
    }

    /// Get progress (0.0 to 1.0)
    pub fn progress(&self) -> f32 {
        let total_duration: usize = self.stages.iter().map(|s| s.duration).sum();
        if total_duration == 0 {
            return 1.0;
        }
        self.total_steps as f32 / total_duration as f32
    }

    /// Reset curriculum
    pub fn reset(&mut self) {
        self.current_stage = 0;
        self.steps_in_stage = 0;
        self.total_steps = 0;
    }
}

impl Default for CurriculumScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Temperature annealing scheduler
pub struct TemperatureAnnealing {
    initial_temp: f32,
    final_temp: f32,
    total_steps: usize,
    current_step: usize,
    decay_type: DecayType,
    step_size: usize, // For step decay
}

impl TemperatureAnnealing {
    pub fn new(initial: f32, final_temp: f32, steps: usize) -> Self {
        Self {
            initial_temp: initial,
            final_temp: final_temp,
            total_steps: steps,
            current_step: 0,
            decay_type: DecayType::Linear,
            step_size: steps / 10,
        }
    }

    pub fn with_decay(mut self, decay: DecayType) -> Self {
        self.decay_type = decay;
        self
    }

    pub fn with_step_size(mut self, size: usize) -> Self {
        self.step_size = size;
        self
    }

    /// Get current temperature and advance
    pub fn step(&mut self) -> f32 {
        let temp = self.get_temp();
        self.current_step += 1;
        temp
    }

    /// Get current temperature without advancing
    pub fn get_temp(&self) -> f32 {
        if self.current_step >= self.total_steps {
            return self.final_temp;
        }

        let progress = self.current_step as f32 / self.total_steps as f32;
        let range = self.initial_temp - self.final_temp;

        match self.decay_type {
            DecayType::Linear => self.initial_temp - range * progress,
            DecayType::Exponential => {
                let decay_rate =
                    (self.final_temp / self.initial_temp).ln() / self.total_steps as f32;
                self.initial_temp * (decay_rate * self.current_step as f32).exp()
            }
            DecayType::Cosine => {
                self.final_temp + 0.5 * range * (1.0 + (std::f32::consts::PI * progress).cos())
            }
            DecayType::Step => {
                let num_steps = self.current_step / self.step_size.max(1);
                let step_decay =
                    range * num_steps as f32 / (self.total_steps / self.step_size.max(1)) as f32;
                (self.initial_temp - step_decay).max(self.final_temp)
            }
        }
    }

    /// Reset annealing
    pub fn reset(&mut self) {
        self.current_step = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_curriculum_stages() {
        let mut curriculum = CurriculumScheduler::new()
            .add_stage(CurriculumStage::new("easy").duration(10).difficulty(0.2))
            .add_stage(CurriculumStage::new("hard").duration(10).difficulty(0.8));

        assert_eq!(curriculum.current_stage().unwrap().name, "easy");
        assert!((curriculum.difficulty() - 0.2).abs() < 1e-5);

        // Progress through first stage
        for _ in 0..10 {
            curriculum.step();
        }

        assert_eq!(curriculum.current_stage().unwrap().name, "hard");
        assert!((curriculum.difficulty() - 0.8).abs() < 1e-5);
    }

    #[test]
    fn test_default_curriculum() {
        let mut curriculum = CurriculumScheduler::default_curriculum(400);

        assert_eq!(curriculum.stages.len(), 4);
        assert_eq!(curriculum.current_stage().unwrap().name, "warm_up");

        // Progress to end
        for _ in 0..400 {
            curriculum.step();
        }

        assert!(curriculum.is_complete());
    }

    #[test]
    fn test_temperature_linear() {
        let mut annealing = TemperatureAnnealing::new(1.0, 0.1, 100);

        let temp_start = annealing.step();
        assert!((temp_start - 1.0).abs() < 0.1);

        for _ in 0..99 {
            annealing.step();
        }

        let temp_end = annealing.get_temp();
        assert!((temp_end - 0.1).abs() < 0.1);
    }

    #[test]
    fn test_temperature_cosine() {
        let mut annealing = TemperatureAnnealing::new(1.0, 0.0, 100).with_decay(DecayType::Cosine);

        // Halfway should be approximately middle value
        for _ in 0..50 {
            annealing.step();
        }

        let temp_mid = annealing.get_temp();
        assert!(temp_mid > 0.4 && temp_mid < 0.6);
    }

    #[test]
    fn test_temperature_step() {
        let mut annealing = TemperatureAnnealing::new(1.0, 0.0, 100)
            .with_decay(DecayType::Step)
            .with_step_size(25);

        let temp_0 = annealing.get_temp();
        for _ in 0..25 {
            annealing.step();
        }
        let temp_25 = annealing.get_temp();

        // Should have dropped
        assert!(temp_25 < temp_0);
    }

    #[test]
    fn test_curriculum_progress() {
        let mut curriculum = CurriculumScheduler::new()
            .add_stage(CurriculumStage::new("stage1").duration(50))
            .add_stage(CurriculumStage::new("stage2").duration(50));

        assert!((curriculum.progress() - 0.0).abs() < 1e-5);

        for _ in 0..50 {
            curriculum.step();
        }

        assert!((curriculum.progress() - 0.5).abs() < 0.05);
    }
}
