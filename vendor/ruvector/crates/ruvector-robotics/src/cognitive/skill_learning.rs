//! Skill acquisition via learning from demonstration.
//!
//! Robots can observe demonstrations (trajectories with timestamps),
//! generalise a skill by averaging, and progressively improve confidence
//! through execution feedback.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single demonstration of a skill (e.g., a recorded trajectory).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Demonstration {
    pub trajectory: Vec<[f64; 3]>,
    pub timestamps: Vec<i64>,
    pub metadata: String,
}

/// A learned skill derived from one or more demonstrations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub trajectory: Vec<[f64; 3]>,
    pub confidence: f64,
    pub execution_count: u64,
}

// ---------------------------------------------------------------------------
// Library
// ---------------------------------------------------------------------------

/// A library of learned skills keyed by name.
#[derive(Debug, Clone, Default)]
pub struct SkillLibrary {
    skills: HashMap<String, Skill>,
}

impl SkillLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    /// Learn a skill from one or more demonstrations by averaging their
    /// trajectories point-by-point. The resulting trajectory length equals
    /// the shortest demonstration.
    /// # Panics
    ///
    /// Returns early with a zero-confidence skill if `demos` is empty.
    pub fn learn_from_demonstration(&mut self, name: &str, demos: &[Demonstration]) -> Skill {
        if demos.is_empty() {
            let skill = Skill {
                name: name.to_string(),
                trajectory: Vec::new(),
                confidence: 0.0,
                execution_count: 0,
            };
            self.skills.insert(name.to_string(), skill.clone());
            return skill;
        }

        let min_len = demos.iter().map(|d| d.trajectory.len()).min().unwrap_or(0);

        let mut avg_traj: Vec<[f64; 3]> = Vec::with_capacity(min_len);
        let n = demos.len() as f64;

        for i in 0..min_len {
            let mut sum = [0.0_f64; 3];
            for demo in demos {
                sum[0] += demo.trajectory[i][0];
                sum[1] += demo.trajectory[i][1];
                sum[2] += demo.trajectory[i][2];
            }
            avg_traj.push([sum[0] / n, sum[1] / n, sum[2] / n]);
        }

        let confidence = 1.0 - (1.0 / (demos.len() as f64 + 1.0));

        let skill = Skill {
            name: name.to_string(),
            trajectory: avg_traj,
            confidence,
            execution_count: 0,
        };

        self.skills.insert(name.to_string(), skill.clone());
        skill
    }

    /// Execute a named skill, returning its trajectory and incrementing
    /// the execution count. Returns `None` if the skill is not found.
    pub fn execute_skill(&mut self, name: &str) -> Option<Vec<[f64; 3]>> {
        if let Some(skill) = self.skills.get_mut(name) {
            skill.execution_count += 1;
            Some(skill.trajectory.clone())
        } else {
            None
        }
    }

    /// Adjust a skill's confidence based on external feedback.
    /// Positive feedback increases confidence; negative decreases it.
    /// Confidence is clamped to [0.0, 1.0].
    pub fn improve_skill(&mut self, name: &str, feedback: f64) {
        if let Some(skill) = self.skills.get_mut(name) {
            skill.confidence = (skill.confidence + feedback).clamp(0.0, 1.0);
        }
    }

    /// Look up a skill by name.
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// Number of skills in the library.
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// Whether the library is empty.
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn demo(pts: Vec<[f64; 3]>) -> Demonstration {
        let n = pts.len();
        Demonstration {
            trajectory: pts,
            timestamps: (0..n as i64).collect(),
            metadata: String::new(),
        }
    }

    #[test]
    fn test_learn_single_demo() {
        let mut lib = SkillLibrary::new();
        let skill = lib.learn_from_demonstration("wave", &[demo(vec![[1.0, 2.0, 3.0]])]);
        assert_eq!(skill.trajectory, vec![[1.0, 2.0, 3.0]]);
        assert!(skill.confidence > 0.0);
        assert_eq!(lib.len(), 1);
    }

    #[test]
    fn test_learn_multiple_demos_averages() {
        let mut lib = SkillLibrary::new();
        let d1 = demo(vec![[0.0, 0.0, 0.0], [2.0, 2.0, 2.0]]);
        let d2 = demo(vec![[2.0, 2.0, 2.0], [4.0, 4.0, 4.0]]);
        let skill = lib.learn_from_demonstration("reach", &[d1, d2]);
        assert_eq!(skill.trajectory.len(), 2);
        assert!((skill.trajectory[0][0] - 1.0).abs() < 1e-9);
        assert!((skill.trajectory[1][0] - 3.0).abs() < 1e-9);
    }

    #[test]
    fn test_execute_increments_count() {
        let mut lib = SkillLibrary::new();
        lib.learn_from_demonstration("grab", &[demo(vec![[0.0, 0.0, 0.0]])]);
        let traj = lib.execute_skill("grab");
        assert!(traj.is_some());
        assert_eq!(lib.get("grab").unwrap().execution_count, 1);
        lib.execute_skill("grab");
        assert_eq!(lib.get("grab").unwrap().execution_count, 2);
    }

    #[test]
    fn test_execute_missing_skill() {
        let mut lib = SkillLibrary::new();
        assert!(lib.execute_skill("nonexistent").is_none());
    }

    #[test]
    fn test_improve_skill() {
        let mut lib = SkillLibrary::new();
        lib.learn_from_demonstration("push", &[demo(vec![[1.0, 0.0, 0.0]])]);
        let initial = lib.get("push").unwrap().confidence;
        lib.improve_skill("push", 0.1);
        assert!(lib.get("push").unwrap().confidence > initial);
    }

    #[test]
    fn test_improve_skill_clamp() {
        let mut lib = SkillLibrary::new();
        lib.learn_from_demonstration("pull", &[demo(vec![[0.0, 0.0, 0.0]])]);
        lib.improve_skill("pull", 10.0);
        assert!((lib.get("pull").unwrap().confidence - 1.0).abs() < 1e-9);
        lib.improve_skill("pull", -20.0);
        assert!((lib.get("pull").unwrap().confidence).abs() < 1e-9);
    }

    #[test]
    fn test_different_length_demos() {
        let mut lib = SkillLibrary::new();
        let d1 = demo(vec![[1.0, 1.0, 1.0], [2.0, 2.0, 2.0], [3.0, 3.0, 3.0]]);
        let d2 = demo(vec![[3.0, 3.0, 3.0], [4.0, 4.0, 4.0]]);
        let skill = lib.learn_from_demonstration("mixed", &[d1, d2]);
        // Uses min length = 2
        assert_eq!(skill.trajectory.len(), 2);
    }

    #[test]
    fn test_confidence_increases_with_more_demos() {
        let mut lib = SkillLibrary::new();
        let s1 = lib.learn_from_demonstration("s1", &[demo(vec![[0.0, 0.0, 0.0]])]);
        let s2 = lib.learn_from_demonstration(
            "s2",
            &[
                demo(vec![[0.0, 0.0, 0.0]]),
                demo(vec![[1.0, 1.0, 1.0]]),
                demo(vec![[2.0, 2.0, 2.0]]),
            ],
        );
        assert!(s2.confidence > s1.confidence);
    }
}
