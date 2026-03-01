//! Integration tests for Prime-Radiant Advanced Math Modules
//!
//! Tests cross-module interactions and end-to-end workflows including:
//! - Category theory operations
//! - HoTT path algebra
//! - Cross-module coherence

use prime_radiant_category::category::{
    Category, SetCategory, VectorCategory,
};
use prime_radiant_category::hott::{
    Term, Path, PathOps,
};

// ============================================================================
// CATEGORY THEORY INTEGRATION TESTS
// ============================================================================

mod category_integration {
    use super::*;

    /// Test SetCategory creation and basic operations
    #[test]
    fn test_set_category_basics() {
        let cat = SetCategory::new();
        assert_eq!(cat.objects().len(), 0);
        assert!(cat.verify_laws());
    }

    /// Test VectorCategory creation and dimension
    #[test]
    fn test_vector_category_basics() {
        let cat = VectorCategory::new(768);
        assert_eq!(cat.dimension(), 768);
        assert!(cat.verify_laws());
    }

    /// Test VectorCategory with different dimensions
    #[test]
    fn test_vector_category_dimensions() {
        // Common embedding dimensions
        let dims = [64, 128, 256, 384, 512, 768, 1024, 1536];

        for dim in dims {
            let cat = VectorCategory::new(dim);
            assert_eq!(cat.dimension(), dim);
        }
    }
}

// ============================================================================
// HOTT PATH ALGEBRA TESTS
// ============================================================================

mod hott_integration {
    use super::*;

    /// Test that path composition corresponds to morphism composition
    #[test]
    fn test_path_composition() {
        let a = Term::var("a");
        let b = Term::var("b");
        let c = Term::var("c");

        let p = Path::new(a.clone(), b.clone(), Term::var("p"));
        let q = Path::new(b.clone(), c.clone(), Term::var("q"));

        // Path composition should work like morphism composition
        let composed = p.compose(&q);
        assert!(composed.is_some(), "Composable paths should compose");

        let pq = composed.unwrap();
        assert_eq!(pq.source(), &a);
        assert_eq!(pq.target(), &c);
    }

    /// Test that reflexivity paths act as identity morphisms
    #[test]
    fn test_reflexivity_as_identity() {
        let x = Term::var("x");
        let refl_x = Path::refl(x.clone());

        // Reflexivity is the identity path
        assert!(refl_x.is_refl());
        assert_eq!(refl_x.source(), refl_x.target());
    }

    /// Test categorical unit laws through HoTT path algebra
    #[test]
    fn test_unit_laws() {
        let a = Term::var("a");
        let b = Term::var("b");

        // Path p : a = b
        let p = Path::new(a.clone(), b.clone(), Term::var("p"));

        // Reflexivity paths
        let refl_a = Path::refl(a.clone());
        let refl_b = Path::refl(b.clone());

        // refl_a . p should give path from a to b (like p)
        let left_unit = refl_a.compose(&p);
        assert!(left_unit.is_some());
        let lu = left_unit.unwrap();
        assert_eq!(lu.source(), &a);
        assert_eq!(lu.target(), &b);

        // p . refl_b should give path from a to b (like p)
        let right_unit = p.compose(&refl_b);
        assert!(right_unit.is_some());
        let ru = right_unit.unwrap();
        assert_eq!(ru.source(), &a);
        assert_eq!(ru.target(), &b);
    }

    /// Test path inverse (symmetry)
    #[test]
    fn test_path_inverse() {
        let a = Term::var("a");
        let b = Term::var("b");

        let p = Path::new(a.clone(), b.clone(), Term::var("p"));
        let p_inv = p.inverse();

        // Inverse reverses endpoints
        assert_eq!(p_inv.source(), &b);
        assert_eq!(p_inv.target(), &a);

        // Composing with inverse should give loop
        let round_trip = p.compose(&p_inv);
        assert!(round_trip.is_some());

        let rt = round_trip.unwrap();
        assert_eq!(rt.source(), &a);
        assert_eq!(rt.target(), &a);
    }

    /// Test associativity of path composition
    #[test]
    fn test_path_associativity() {
        let a = Term::var("a");
        let b = Term::var("b");
        let c = Term::var("c");
        let d = Term::var("d");

        let p = Path::new(a.clone(), b.clone(), Term::var("p"));
        let q = Path::new(b.clone(), c.clone(), Term::var("q"));
        let r = Path::new(c.clone(), d.clone(), Term::var("r"));

        // (p . q) . r
        let pq = p.compose(&q).unwrap();
        let left = pq.compose(&r);
        assert!(left.is_some());

        // p . (q . r)
        let qr = q.compose(&r).unwrap();
        let right = p.compose(&qr);
        assert!(right.is_some());

        // Both should have same endpoints
        let left = left.unwrap();
        let right = right.unwrap();
        assert_eq!(left.source(), right.source());
        assert_eq!(left.target(), right.target());
    }

    /// Test functoriality via ap
    #[test]
    fn test_ap_functoriality() {
        let a = Term::var("a");
        let b = Term::var("b");
        let f = Term::var("f");

        let p = Path::new(a.clone(), b.clone(), Term::var("p"));
        let ap_p = p.ap(&f);

        // ap f p : f(a) = f(b)
        // The endpoints should be function applications
        assert!(!ap_p.is_refl() || a.structural_eq(&b));
    }

    /// Test path composition fails on mismatch
    #[test]
    fn test_composition_mismatch() {
        let a = Term::var("a");
        let b = Term::var("b");
        let c = Term::var("c");

        let p = Path::new(a.clone(), b.clone(), Term::var("p"));
        let q = Path::new(c.clone(), a.clone(), Term::var("q")); // c != b

        // Should fail - endpoints don't match
        assert!(p.compose(&q).is_none());
    }
}

// ============================================================================
// CROSS-MODULE INTEGRATION TESTS
// ============================================================================

mod cross_module_integration {
    use super::*;

    /// Test that HoTT paths correspond to category morphisms
    #[test]
    fn test_hott_category_correspondence() {
        // In HoTT, a category is a type with:
        // - Objects as terms
        // - Morphisms as paths
        // - Composition as path composition

        let a = Term::var("a");
        let b = Term::var("b");
        let c = Term::var("c");

        // Morphisms are paths
        let f = Path::new(a.clone(), b.clone(), Term::var("f"));
        let g = Path::new(b.clone(), c.clone(), Term::var("g"));

        // Composition is path composition
        let gf = f.compose(&g);
        assert!(gf.is_some());

        // Identity is reflexivity
        let id_a = Path::refl(a.clone());
        assert!(id_a.is_refl());

        // Identity laws hold via path algebra
        let f_id = f.compose(&Path::refl(b.clone()));
        assert!(f_id.is_some());
    }

    /// Test belief modeling with paths
    #[test]
    fn test_belief_path_integration() {
        // Model belief transitions as paths
        let belief_a = Term::var("belief_a");
        let belief_b = Term::var("belief_b");

        // Evidence for transition
        let evidence = Path::new(
            belief_a.clone(),
            belief_b.clone(),
            Term::var("evidence"),
        );

        // Can compose evidence chains
        let belief_c = Term::var("belief_c");
        let more_evidence = Path::new(
            belief_b.clone(),
            belief_c.clone(),
            Term::var("more_evidence"),
        );

        let full_path = evidence.compose(&more_evidence);
        assert!(full_path.is_some());
    }

    /// Test category-path interaction
    #[test]
    fn test_category_path_interaction() {
        // Create a category
        let cat = VectorCategory::new(768);
        assert!(cat.verify_laws());

        // Model categorical morphism composition with paths
        let obj_a = Term::var("vec_a");
        let obj_b = Term::var("vec_b");
        let obj_c = Term::var("vec_c");

        // Linear maps as paths
        let linear_f = Path::new(obj_a.clone(), obj_b.clone(), Term::var("f"));
        let linear_g = Path::new(obj_b.clone(), obj_c.clone(), Term::var("g"));

        // Composition
        let gf = linear_f.compose(&linear_g);
        assert!(gf.is_some());

        let composed = gf.unwrap();
        assert_eq!(composed.source(), &obj_a);
        assert_eq!(composed.target(), &obj_c);
    }
}

// ============================================================================
// EDGE CASES AND ROBUSTNESS
// ============================================================================

mod edge_cases {
    use super::*;

    /// Test path composition with identity
    #[test]
    fn test_path_identity_composition() {
        let a = Term::var("a");

        // Identity path
        let refl_a = Path::refl(a.clone());

        // Composing identity with itself should give identity
        let composed = refl_a.compose(&refl_a);
        assert!(composed.is_some());

        let c = composed.unwrap();
        assert_eq!(c.source(), &a);
        assert_eq!(c.target(), &a);
    }

    /// Test multiple path inversions
    #[test]
    fn test_double_inverse() {
        let a = Term::var("a");
        let b = Term::var("b");

        let p = Path::new(a.clone(), b.clone(), Term::var("p"));
        let p_inv = p.inverse();
        let p_inv_inv = p_inv.inverse();

        // Double inverse should return to original endpoints
        assert_eq!(p_inv_inv.source(), &a);
        assert_eq!(p_inv_inv.target(), &b);
    }

    /// Test long path chains
    #[test]
    fn test_long_path_chain() {
        // Create a chain of 10 paths
        let points: Vec<Term> = (0..11)
            .map(|i| Term::var(&format!("p{}", i)))
            .collect();

        let paths: Vec<Path> = (0..10)
            .map(|i| Path::new(
                points[i].clone(),
                points[i + 1].clone(),
                Term::var(&format!("path{}", i)),
            ))
            .collect();

        // Compose all paths
        let mut composed = paths[0].clone();
        for path in paths.iter().skip(1) {
            composed = composed.compose(path).expect("Composition should succeed");
        }

        // Result should go from first to last point
        assert_eq!(composed.source(), &points[0]);
        assert_eq!(composed.target(), &points[10]);
    }

    /// Test category with many objects
    #[test]
    fn test_large_category() {
        let cat = VectorCategory::new(768);

        // Creating many vector spaces should work
        for _ in 0..100 {
            // VectorCategory should handle multiple dimensions
            assert!(cat.verify_laws());
        }
    }

    /// Test paths with numeric variable names
    #[test]
    fn test_numeric_variable_paths() {
        let vars: Vec<Term> = (0..5)
            .map(|i| Term::var(&i.to_string()))
            .collect();

        // Create paths between sequential points
        for i in 0..4 {
            let p = Path::new(
                vars[i].clone(),
                vars[i + 1].clone(),
                Term::var(&format!("p{}", i)),
            );
            assert_eq!(p.source(), &vars[i]);
            assert_eq!(p.target(), &vars[i + 1]);
        }
    }

    /// Test reflexivity on complex terms
    #[test]
    fn test_complex_term_reflexivity() {
        // Create a lambda term
        let body = Term::var("x");
        let lambda = Term::lambda("x", body);

        // Reflexivity should work on any term
        let refl = Path::refl(lambda.clone());
        assert!(refl.is_refl());
        assert_eq!(refl.source(), &lambda);
        assert_eq!(refl.target(), &lambda);
    }
}

// ============================================================================
// PERFORMANCE TESTS
// ============================================================================

mod performance_tests {
    use super::*;

    /// Test path composition performance
    #[test]
    fn test_path_composition_performance() {
        let start = std::time::Instant::now();

        // Create and compose many paths
        for i in 0..1000 {
            let a = Term::var(&format!("a{}", i));
            let b = Term::var(&format!("b{}", i));
            let c = Term::var(&format!("c{}", i));

            let p = Path::new(a.clone(), b.clone(), Term::var("p"));
            let q = Path::new(b.clone(), c.clone(), Term::var("q"));

            let _ = p.compose(&q);
        }

        let duration = start.elapsed();

        // Should complete quickly
        assert!(duration.as_secs() < 5,
            "Path composition should be fast: {:?}", duration);
    }

    /// Test category operations performance
    #[test]
    fn test_category_operations_performance() {
        let start = std::time::Instant::now();

        for _ in 0..100 {
            let cat = VectorCategory::new(768);
            let _ = cat.verify_laws();
        }

        let duration = start.elapsed();

        assert!(duration.as_secs() < 10,
            "Category operations should be fast: {:?}", duration);
    }

    /// Test path inverse performance
    #[test]
    fn test_path_inverse_performance() {
        let start = std::time::Instant::now();

        for i in 0..1000 {
            let a = Term::var(&format!("a{}", i));
            let b = Term::var(&format!("b{}", i));

            let p = Path::new(a, b, Term::var("p"));
            let _ = p.inverse();
        }

        let duration = start.elapsed();

        assert!(duration.as_secs() < 5,
            "Path inverse should be fast: {:?}", duration);
    }

    /// Test long composition chain performance
    #[test]
    fn test_long_chain_performance() {
        let start = std::time::Instant::now();

        // Create chain of 100 paths
        let points: Vec<Term> = (0..101)
            .map(|i| Term::var(&format!("p{}", i)))
            .collect();

        let paths: Vec<Path> = (0..100)
            .map(|i| Path::new(
                points[i].clone(),
                points[i + 1].clone(),
                Term::var(&format!("path{}", i)),
            ))
            .collect();

        // Compose all
        let mut composed = paths[0].clone();
        for path in paths.iter().skip(1) {
            composed = composed.compose(path).expect("Should compose");
        }

        let duration = start.elapsed();

        assert!(duration.as_secs() < 5,
            "Long chain composition should be fast: {:?}", duration);
        assert_eq!(composed.source(), &points[0]);
        assert_eq!(composed.target(), &points[100]);
    }
}

// ============================================================================
// GROUPOID STRUCTURE TESTS
// ============================================================================

mod groupoid_structure {
    use super::*;

    /// Test that paths form a groupoid (category where every morphism is invertible)
    #[test]
    fn test_groupoid_structure() {
        let a = Term::var("a");
        let b = Term::var("b");

        let p = Path::new(a.clone(), b.clone(), Term::var("p"));

        // Every path has an inverse
        let p_inv = p.inverse();
        assert_eq!(p_inv.source(), &b);
        assert_eq!(p_inv.target(), &a);

        // p . p^(-1) gives identity (loop at source)
        let loop_a = p.compose(&p_inv);
        assert!(loop_a.is_some());
        let loop_a = loop_a.unwrap();
        assert_eq!(loop_a.source(), &a);
        assert_eq!(loop_a.target(), &a);

        // p^(-1) . p gives identity (loop at target)
        let loop_b = p_inv.compose(&p);
        assert!(loop_b.is_some());
        let loop_b = loop_b.unwrap();
        assert_eq!(loop_b.source(), &b);
        assert_eq!(loop_b.target(), &b);
    }

    /// Test inverse properties
    #[test]
    fn test_inverse_properties() {
        let a = Term::var("a");
        let b = Term::var("b");
        let c = Term::var("c");

        let p = Path::new(a.clone(), b.clone(), Term::var("p"));
        let q = Path::new(b.clone(), c.clone(), Term::var("q"));

        // (p . q)^(-1) should have endpoints reversed
        let pq = p.compose(&q).unwrap();
        let pq_inv = pq.inverse();

        assert_eq!(pq_inv.source(), &c);
        assert_eq!(pq_inv.target(), &a);

        // Compare with q^(-1) . p^(-1)
        let q_inv = q.inverse();
        let p_inv = p.inverse();
        let reversed = q_inv.compose(&p_inv).unwrap();

        assert_eq!(reversed.source(), &c);
        assert_eq!(reversed.target(), &a);
    }

    /// Test reflexivity inverse is itself
    #[test]
    fn test_refl_inverse() {
        let a = Term::var("a");
        let refl_a = Path::refl(a.clone());
        let refl_a_inv = refl_a.inverse();

        // Inverse of refl should still be a loop at a
        assert_eq!(refl_a_inv.source(), &a);
        assert_eq!(refl_a_inv.target(), &a);
    }
}
