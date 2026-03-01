use serde::{Deserialize, Serialize};

/// Configuration for the min-cut gating attention operator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinCutConfig {
    pub lambda: f32,
    pub tau: usize,
    pub eps: f32,
    pub seed: u64,
    pub witness_enabled: bool,
}

impl Default for MinCutConfig {
    fn default() -> Self {
        Self {
            lambda: 0.5,
            tau: 2,
            eps: 0.01,
            seed: 42,
            witness_enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let c = MinCutConfig::default();
        assert!((c.lambda - 0.5).abs() < f32::EPSILON);
        assert_eq!(c.tau, 2);
        assert!((c.eps - 0.01).abs() < f32::EPSILON);
        assert_eq!(c.seed, 42);
        assert!(c.witness_enabled);
    }

    #[test]
    fn test_serde_roundtrip() {
        let c = MinCutConfig {
            lambda: 0.3,
            tau: 5,
            eps: 0.001,
            seed: 99,
            witness_enabled: false,
        };
        let json = serde_json::to_string(&c).unwrap();
        let r: MinCutConfig = serde_json::from_str(&json).unwrap();
        assert!((r.lambda - 0.3).abs() < f32::EPSILON);
        assert_eq!(r.tau, 5);
        assert!(!r.witness_enabled);
    }
}
