//! Unit tests for exo-core traits and types

use exo_core::*;

#[cfg(test)]
mod substrate_backend_tests {
    use super::*;

    #[test]
    fn test_pattern_construction() {
        // Test Pattern type construction with valid data
        let pattern = Pattern {
            id: PatternId::new(),
            embedding: vec![0.1, 0.2, 0.3, 0.4],
            metadata: Metadata::default(),
            timestamp: SubstrateTime(1000),
            antecedents: vec![],
            salience: 0.5,
        };
        assert_eq!(pattern.embedding.len(), 4);
    }

    #[test]
    fn test_pattern_with_antecedents() {
        // Test Pattern with causal antecedents
        let parent_id = PatternId::new();
        let pattern = Pattern {
            id: PatternId::new(),
            embedding: vec![0.1, 0.2, 0.3],
            metadata: Metadata::default(),
            timestamp: SubstrateTime::now(),
            antecedents: vec![parent_id],
            salience: 0.8,
        };
        assert_eq!(pattern.antecedents.len(), 1);
    }

    #[test]
    fn test_topological_query_persistent_homology() {
        // Test PersistentHomology variant construction
        let query = TopologicalQuery::PersistentHomology {
            dimension: 1,
            epsilon_range: (0.0, 1.0),
        };
        match query {
            TopologicalQuery::PersistentHomology { dimension, .. } => {
                assert_eq!(dimension, 1);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_topological_query_betti_numbers() {
        // Test BettiNumbers variant
        let query = TopologicalQuery::BettiNumbers { max_dimension: 3 };
        match query {
            TopologicalQuery::BettiNumbers { max_dimension } => {
                assert_eq!(max_dimension, 3);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_topological_query_sheaf_consistency() {
        // Test SheafConsistency variant
        let sections = vec![SectionId::new(), SectionId::new()];
        let query = TopologicalQuery::SheafConsistency {
            local_sections: sections.clone(),
        };
        match query {
            TopologicalQuery::SheafConsistency { local_sections } => {
                assert_eq!(local_sections.len(), 2);
            }
            _ => panic!("Wrong variant"),
        }
    }
}

#[cfg(test)]
mod temporal_context_tests {
    use super::*;

    #[test]
    fn test_substrate_time_ordering() {
        // Test SubstrateTime comparison
        let t1 = SubstrateTime(1000);
        let t2 = SubstrateTime(2000);
        assert!(t1 < t2);
    }

    #[test]
    fn test_substrate_time_now() {
        // Test current time generation
        let now = SubstrateTime::now();
        std::thread::sleep(std::time::Duration::from_nanos(100));
        let later = SubstrateTime::now();
        assert!(later >= now);
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_error_display() {
        // Test error Display implementation
        let err = Error::PatternNotFound(PatternId::new());
        let display = format!("{}", err);
        assert!(display.contains("Pattern not found"));
    }
}

#[cfg(test)]
mod filter_tests {
    use super::*;

    #[test]
    fn test_filter_construction() {
        // Test Filter type construction
        let filter = Filter {
            conditions: vec![FilterCondition {
                field: "category".to_string(),
                operator: FilterOperator::Equal,
                value: MetadataValue::String("test".to_string()),
            }],
        };
        assert_eq!(filter.conditions.len(), 1);
    }
}
