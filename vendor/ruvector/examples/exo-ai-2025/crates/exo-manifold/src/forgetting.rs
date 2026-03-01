//! Simplified forgetting module

use crate::network::LearnedManifold;
use exo_core::{Pattern, Result};
use parking_lot::RwLock;
use std::sync::Arc;

pub struct StrategicForgetting {
    _network: Arc<RwLock<LearnedManifold>>,
}

impl StrategicForgetting {
    pub fn new(network: Arc<RwLock<LearnedManifold>>) -> Self {
        Self { _network: network }
    }

    pub fn forget(
        &self,
        patterns: &Arc<RwLock<Vec<Pattern>>>,
        salience_threshold: f32,
        _decay_rate: f32,
    ) -> Result<usize> {
        let mut patterns = patterns.write();
        let initial_len = patterns.len();

        // Remove patterns below salience threshold
        patterns.retain(|p| p.salience >= salience_threshold);

        Ok(initial_len - patterns.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exo_core::{Metadata, PatternId, SubstrateTime};

    #[test]
    fn test_forgetting() {
        let network = Arc::new(RwLock::new(LearnedManifold::new(64, 128, 3)));
        let forgetter = StrategicForgetting::new(network);

        let patterns = Arc::new(RwLock::new(vec![
            Pattern {
                id: PatternId::new(),
                embedding: vec![1.0; 64],
                metadata: Metadata::default(),
                timestamp: SubstrateTime::now(),
                antecedents: vec![],
                salience: 0.9,
            },
            Pattern {
                id: PatternId::new(),
                embedding: vec![0.5; 64],
                metadata: Metadata::default(),
                timestamp: SubstrateTime::now(),
                antecedents: vec![],
                salience: 0.3,
            },
        ]));

        let forgotten = forgetter.forget(&patterns, 0.5, 0.1).unwrap();
        assert_eq!(forgotten, 1);
        assert_eq!(patterns.read().len(), 1);
    }
}
