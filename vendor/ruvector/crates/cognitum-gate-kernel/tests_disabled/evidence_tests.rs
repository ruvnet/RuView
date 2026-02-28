//! Comprehensive tests for E-value accumulator
//!
//! Tests cover:
//! - E-value bounds (E[e] <= 1 under null)
//! - Overflow/underflow protection
//! - Update rules (Product, Average, ExponentialMoving, Maximum)
//! - Stopping rules

use cognitum_gate_kernel::evidence::{
    EValueAccumulator, EValueError, StoppingDecision, StoppingRule, UpdateRule,
    E_VALUE_MAX, E_VALUE_MIN,
};

#[cfg(test)]
mod basic_operations {
    use super::*;

    #[test]
    fn test_accumulator_creation() {
        let acc = EValueAccumulator::new();
        assert_eq!(acc.current_value(), 1.0);
        assert_eq!(acc.observation_count(), 0);
    }

    #[test]
    fn test_observe_updates_count() {
        let mut acc = EValueAccumulator::new();
        acc.observe(0.5);
        assert_eq!(acc.observation_count(), 1);
        acc.observe(0.7);
        assert_eq!(acc.observation_count(), 2);
    }

    #[test]
    fn test_reset() {
        let mut acc = EValueAccumulator::new();
        acc.observe(0.5);
        acc.reset();
        assert_eq!(acc.current_value(), 1.0);
        assert_eq!(acc.observation_count(), 0);
    }
}

#[cfg(test)]
mod update_rules {
    use super::*;

    #[test]
    fn test_product_rule() {
        let mut acc = EValueAccumulator::with_rule(UpdateRule::Product);
        acc.observe_evalue(2.0);
        assert!((acc.current_value() - 2.0).abs() < 0.001);
        acc.observe_evalue(3.0);
        assert!((acc.current_value() - 6.0).abs() < 0.001);
    }

    #[test]
    fn test_average_rule() {
        let mut acc = EValueAccumulator::with_rule(UpdateRule::Average);
        acc.observe_evalue(2.0);
        acc.observe_evalue(4.0);
        assert!((acc.current_value() - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_exponential_moving() {
        let mut acc = EValueAccumulator::with_rule(UpdateRule::ExponentialMoving { lambda: 0.5 });
        acc.observe_evalue(2.0);
        acc.observe_evalue(4.0);
        assert!((acc.current_value() - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_maximum_rule() {
        let mut acc = EValueAccumulator::with_rule(UpdateRule::Maximum);
        acc.observe_evalue(2.0);
        acc.observe_evalue(5.0);
        acc.observe_evalue(3.0);
        assert_eq!(acc.current_value(), 5.0);
    }
}

#[cfg(test)]
mod bounds_and_overflow {
    use super::*;

    #[test]
    fn test_e_value_clamping_high() {
        let mut acc = EValueAccumulator::with_rule(UpdateRule::Product);
        acc.observe_evalue(1e20);
        assert!(acc.current_value() <= E_VALUE_MAX);
    }

    #[test]
    fn test_e_value_clamping_low() {
        let mut acc = EValueAccumulator::with_rule(UpdateRule::Product);
        acc.observe_evalue(1e-20);
        assert!(acc.current_value() >= E_VALUE_MIN);
    }

    #[test]
    fn test_product_overflow_protection() {
        let mut acc = EValueAccumulator::with_rule(UpdateRule::Product);
        for _ in 0..100 {
            acc.observe_evalue(100.0);
        }
        assert!(acc.current_value() <= E_VALUE_MAX);
        assert!(acc.current_value().is_finite());
    }
}

#[cfg(test)]
mod likelihood_ratio {
    use super::*;

    #[test]
    fn test_valid_likelihood_ratio() {
        let result = EValueAccumulator::from_likelihood_ratio(0.9, 0.1);
        assert!(result.is_ok());
        assert!((result.unwrap() - 9.0).abs() < 0.001);
    }

    #[test]
    fn test_zero_denominator() {
        let result = EValueAccumulator::from_likelihood_ratio(0.5, 0.0);
        assert_eq!(result, Err(EValueError::DivisionByZero));
    }

    #[test]
    fn test_nan_input() {
        let result = EValueAccumulator::from_likelihood_ratio(f64::NAN, 0.5);
        assert_eq!(result, Err(EValueError::InvalidInput));
    }
}

#[cfg(test)]
mod mixture_evalue {
    use super::*;

    #[test]
    fn test_uniform_mixture() {
        let components = [2.0, 4.0, 6.0];
        let weights = [1.0, 1.0, 1.0];
        let result = EValueAccumulator::mixture(&components, &weights);
        assert!(result.is_ok());
        assert!((result.unwrap() - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_empty_mixture() {
        let result = EValueAccumulator::mixture(&[], &[]);
        assert_eq!(result, Err(EValueError::InvalidInput));
    }
}

#[cfg(test)]
mod stopping_rules {
    use super::*;

    #[test]
    fn test_continue_decision() {
        let rule = StoppingRule::new(100.0);
        let acc = EValueAccumulator::new();
        assert_eq!(rule.check(&acc), StoppingDecision::Continue);
    }

    #[test]
    fn test_accept_decision() {
        let rule = StoppingRule::new(100.0);
        let mut acc = EValueAccumulator::with_rule(UpdateRule::Product);
        for _ in 0..10 {
            acc.observe_evalue(2.0);
        }
        assert!(acc.current_value() > 100.0);
        assert_eq!(rule.check(&acc), StoppingDecision::Accept);
    }

    #[test]
    fn test_reject_decision() {
        let rule = StoppingRule::with_accept(100.0, 0.01);
        let mut acc = EValueAccumulator::with_rule(UpdateRule::Product);
        for _ in 0..10 {
            acc.observe_evalue(0.1);
        }
        assert!(acc.current_value() < 0.01);
        assert_eq!(rule.check(&acc), StoppingDecision::Reject);
    }

    #[test]
    fn test_confidence_calculation() {
        let rule = StoppingRule::default();
        let mut acc = EValueAccumulator::new();
        assert_eq!(rule.confidence(&acc), 0.0);
        acc.observe_evalue(2.0);
        assert!((rule.confidence(&acc) - 0.5).abs() < 0.001);
    }
}

#[cfg(test)]
mod combine_evalues {
    use super::*;

    #[test]
    fn test_combine_basic() {
        let combined = EValueAccumulator::combine(2.0, 3.0);
        assert_eq!(combined, 6.0);
    }

    #[test]
    fn test_combine_overflow_clamped() {
        let combined = EValueAccumulator::combine(1e10, 1e10);
        assert!(combined <= E_VALUE_MAX);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_e_value_always_positive(score in 0.0f64..1.0) {
            let acc = EValueAccumulator::new();
            let e = acc.compute_e_value(score);
            assert!(e > 0.0);
        }

        #[test]
        fn prop_e_value_bounded(score in 0.0f64..1.0) {
            let acc = EValueAccumulator::new();
            let e = acc.compute_e_value(score);
            assert!(e >= E_VALUE_MIN);
            assert!(e <= E_VALUE_MAX);
        }

        #[test]
        fn prop_maximum_never_decreases(observations in proptest::collection::vec(0.1f64..10.0, 1..20)) {
            let mut acc = EValueAccumulator::with_rule(UpdateRule::Maximum);
            let mut max_seen = 0.0f64;

            for o in observations {
                acc.observe_evalue(o);
                let current = acc.current_value();
                assert!(current >= max_seen);
                max_seen = current;
            }
        }
    }
}
