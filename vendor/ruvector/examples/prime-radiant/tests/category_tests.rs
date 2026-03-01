//! Comprehensive tests for Category Theory Module
//!
//! This test suite verifies category-theoretic properties including:
//! - Category laws (identity, associativity)
//! - Functor preservation
//! - Topos subobject classifier
//! - Higher category coherence

use prime_radiant_category::{
    Category, Morphism, Object, SetCategory, VectorCategory,
    Functor, EmbeddingFunctor, ForgetfulFunctor,
    NaturalTransformation,
    Topos, SubobjectClassifier,
    TwoCategory, TwoMorphism, CoherenceResult,
    ObjectId, MorphismId, CategoryError,
    verify_pentagon, verify_triangle,
};
use proptest::prelude::*;
use approx::assert_relative_eq;
use std::collections::HashMap;

// =============================================================================
// CATEGORY LAW TESTS
// =============================================================================

mod category_law_tests {
    use super::*;

    /// Test left identity: id_B . f = f
    #[test]
    fn test_left_identity_law() {
        let mut cat = SetCategory::new();

        let a = cat.add_object("A");
        let b = cat.add_object("B");

        let f = cat.add_morphism(a, b, "f").unwrap();
        let id_b = cat.identity(b).unwrap();

        // Compose id_B . f
        let composed = cat.compose(id_b, f).unwrap();

        // Should equal f (same source and target)
        let f_data = cat.get_morphism(f).unwrap();
        let composed_data = cat.get_morphism(composed).unwrap();

        assert_eq!(f_data.source, composed_data.source);
        assert_eq!(f_data.target, composed_data.target);
    }

    /// Test right identity: f . id_A = f
    #[test]
    fn test_right_identity_law() {
        let mut cat = SetCategory::new();

        let a = cat.add_object("A");
        let b = cat.add_object("B");

        let f = cat.add_morphism(a, b, "f").unwrap();
        let id_a = cat.identity(a).unwrap();

        // Compose f . id_A
        let composed = cat.compose(f, id_a).unwrap();

        let f_data = cat.get_morphism(f).unwrap();
        let composed_data = cat.get_morphism(composed).unwrap();

        assert_eq!(f_data.source, composed_data.source);
        assert_eq!(f_data.target, composed_data.target);
    }

    /// Test associativity: (h . g) . f = h . (g . f)
    #[test]
    fn test_associativity_law() {
        let mut cat = SetCategory::new();

        let a = cat.add_object("A");
        let b = cat.add_object("B");
        let c = cat.add_object("C");
        let d = cat.add_object("D");

        let f = cat.add_morphism(a, b, "f").unwrap();
        let g = cat.add_morphism(b, c, "g").unwrap();
        let h = cat.add_morphism(c, d, "h").unwrap();

        // Left association: (h . g) . f
        let hg = cat.compose(h, g).unwrap();
        let left = cat.compose(hg, f).unwrap();

        // Right association: h . (g . f)
        let gf = cat.compose(g, f).unwrap();
        let right = cat.compose(h, gf).unwrap();

        // Both should have same source and target
        let left_data = cat.get_morphism(left).unwrap();
        let right_data = cat.get_morphism(right).unwrap();

        assert_eq!(left_data.source, right_data.source);
        assert_eq!(left_data.target, right_data.target);
    }

    /// Test category law verification
    #[test]
    fn test_verify_laws() {
        let mut cat = SetCategory::new();

        // Create a small category
        let a = cat.add_object("A");
        let b = cat.add_object("B");
        cat.add_morphism(a, b, "f").unwrap();
        cat.identity(a).unwrap();
        cat.identity(b).unwrap();

        // Category should verify laws
        assert!(cat.verify_laws());
    }

    /// Test composition with incompatible morphisms
    #[test]
    fn test_incompatible_composition() {
        let mut cat = SetCategory::new();

        let a = cat.add_object("A");
        let b = cat.add_object("B");
        let c = cat.add_object("C");
        let d = cat.add_object("D");

        let f = cat.add_morphism(a, b, "f").unwrap();  // A -> B
        let g = cat.add_morphism(c, d, "g").unwrap();  // C -> D

        // Cannot compose g . f since target(f) = B != C = source(g)
        let result = cat.compose(g, f);
        assert!(result.is_err());
        assert!(matches!(result, Err(CategoryError::NotComposable(_, _))));
    }
}

// =============================================================================
// VECTOR CATEGORY TESTS
// =============================================================================

mod vector_category_tests {
    use super::*;

    /// Test VectorCategory creation
    #[test]
    fn test_vector_category_creation() {
        let cat = VectorCategory::new(768);
        assert!(cat.verify_laws());
    }

    /// Test linear map morphisms
    #[test]
    fn test_linear_morphisms() {
        let mut cat = VectorCategory::new(3);

        let v1 = cat.add_object("V1");
        let v2 = cat.add_object("V2");

        // Add a linear map
        let matrix = vec![
            1.0, 0.0, 0.0,
            0.0, 1.0, 0.0,
            0.0, 0.0, 1.0,
        ]; // Identity matrix

        let f = cat.add_linear_morphism(v1, v2, matrix).unwrap();

        // Identity composition should work
        let id_v1 = cat.identity(v1).unwrap();
        let composed = cat.compose(f, id_v1).unwrap();

        assert!(cat.get_morphism(composed).is_some());
    }

    /// Test linear map application
    #[test]
    fn test_apply_linear_map() {
        let mut cat = VectorCategory::new(2);

        let v1 = cat.add_object("V1");
        let v2 = cat.add_object("V2");

        // Rotation by 90 degrees
        let matrix = vec![
            0.0, -1.0,
            1.0, 0.0,
        ];

        let f = cat.add_linear_morphism(v1, v2, matrix).unwrap();

        // Apply to vector [1, 0]
        let input = vec![1.0, 0.0];
        let output = cat.apply_morphism(f, &input).unwrap();

        assert_relative_eq!(output[0], 0.0, epsilon = 1e-10);
        assert_relative_eq!(output[1], 1.0, epsilon = 1e-10);
    }

    /// Test composition preserves linearity
    #[test]
    fn test_composition_preserves_linearity() {
        let mut cat = VectorCategory::new(2);

        let a = cat.add_object("A");
        let b = cat.add_object("B");
        let c = cat.add_object("C");

        // Scale by 2
        let scale = vec![2.0, 0.0, 0.0, 2.0];
        let f = cat.add_linear_morphism(a, b, scale).unwrap();

        // Scale by 3
        let scale2 = vec![3.0, 0.0, 0.0, 3.0];
        let g = cat.add_linear_morphism(b, c, scale2).unwrap();

        // Composition should scale by 6
        let composed = cat.compose(g, f).unwrap();

        let input = vec![1.0, 1.0];
        let output = cat.apply_morphism(composed, &input).unwrap();

        assert_relative_eq!(output[0], 6.0, epsilon = 1e-10);
        assert_relative_eq!(output[1], 6.0, epsilon = 1e-10);
    }
}

// =============================================================================
// FUNCTOR TESTS
// =============================================================================

mod functor_tests {
    use super::*;

    /// Test functor preserves identity: F(id_A) = id_{F(A)}
    #[test]
    fn test_functor_preserves_identity() {
        let mut source_cat = SetCategory::new();
        let mut target_cat = VectorCategory::new(3);

        let a = source_cat.add_object("A");
        let id_a = source_cat.identity(a).unwrap();

        let functor = EmbeddingFunctor::new(3);

        // Map the identity
        let fa = functor.map_object(a, &mut target_cat).unwrap();
        let f_id_a = functor.map_morphism(id_a, &source_cat, &mut target_cat).unwrap();

        // F(id_A) should equal id_{F(A)}
        let id_fa = target_cat.identity(fa).unwrap();

        let f_id_data = target_cat.get_morphism(f_id_a).unwrap();
        let id_fa_data = target_cat.get_morphism(id_fa).unwrap();

        assert_eq!(f_id_data.source, id_fa_data.source);
        assert_eq!(f_id_data.target, id_fa_data.target);
    }

    /// Test functor preserves composition: F(g . f) = F(g) . F(f)
    #[test]
    fn test_functor_preserves_composition() {
        let mut source = SetCategory::new();
        let mut target = VectorCategory::new(2);

        let a = source.add_object("A");
        let b = source.add_object("B");
        let c = source.add_object("C");

        let f = source.add_morphism(a, b, "f").unwrap();
        let g = source.add_morphism(b, c, "g").unwrap();
        let gf = source.compose(g, f).unwrap();

        let functor = EmbeddingFunctor::new(2);

        // F(g . f)
        let f_gf = functor.map_morphism(gf, &source, &mut target).unwrap();

        // F(g) . F(f)
        let ff = functor.map_morphism(f, &source, &mut target).unwrap();
        let fg = functor.map_morphism(g, &source, &mut target).unwrap();
        let fg_ff = target.compose(fg, ff).unwrap();

        // Should have same source and target
        let f_gf_data = target.get_morphism(f_gf).unwrap();
        let fg_ff_data = target.get_morphism(fg_ff).unwrap();

        assert_eq!(f_gf_data.source, fg_ff_data.source);
        assert_eq!(f_gf_data.target, fg_ff_data.target);
    }

    /// Test forgetful functor
    #[test]
    fn test_forgetful_functor() {
        let mut vec_cat = VectorCategory::new(3);
        let mut set_cat = SetCategory::new();

        let v = vec_cat.add_object("V");

        let forgetful = ForgetfulFunctor::new();
        let forgotten = forgetful.map_object(v, &mut set_cat).unwrap();

        // Forgetful functor should create corresponding set object
        assert!(set_cat.get_object(forgotten).is_some());
    }

    /// Test embedding functor with different dimensions
    #[test]
    fn test_embedding_dimensions() {
        let mut source = SetCategory::new();
        let mut target2 = VectorCategory::new(2);
        let mut target10 = VectorCategory::new(10);

        let a = source.add_object("A");

        let embed2 = EmbeddingFunctor::new(2);
        let embed10 = EmbeddingFunctor::new(10);

        let fa2 = embed2.map_object(a, &mut target2).unwrap();
        let fa10 = embed10.map_object(a, &mut target10).unwrap();

        assert!(target2.get_object(fa2).is_some());
        assert!(target10.get_object(fa10).is_some());
    }
}

// =============================================================================
// NATURAL TRANSFORMATION TESTS
// =============================================================================

mod natural_transformation_tests {
    use super::*;

    /// Test naturality condition: eta_B . F(f) = G(f) . eta_A
    #[test]
    fn test_naturality_condition() {
        let mut source = SetCategory::new();
        let mut target = VectorCategory::new(3);

        let a = source.add_object("A");
        let b = source.add_object("B");
        let f = source.add_morphism(a, b, "f").unwrap();

        let functor_f = EmbeddingFunctor::new(3);
        let functor_g = EmbeddingFunctor::new(3);

        // Create natural transformation eta: F -> G
        let eta = NaturalTransformation::new(&functor_f, &functor_g);

        // Verify naturality
        let is_natural = eta.verify_naturality(&source, &mut target, f).unwrap();
        assert!(is_natural);
    }

    /// Test identity natural transformation
    #[test]
    fn test_identity_transformation() {
        let mut cat = VectorCategory::new(2);

        let a = cat.add_object("A");
        let functor = EmbeddingFunctor::new(2);

        let id_nat = NaturalTransformation::identity(&functor);

        // Component at A should be identity
        let component = id_nat.component(a, &mut cat).unwrap();
        let id_a = cat.identity(a).unwrap();

        let comp_data = cat.get_morphism(component).unwrap();
        let id_data = cat.get_morphism(id_a).unwrap();

        assert_eq!(comp_data.source, id_data.source);
        assert_eq!(comp_data.target, id_data.target);
    }

    /// Test vertical composition of natural transformations
    #[test]
    fn test_vertical_composition() {
        let functor_f = EmbeddingFunctor::new(2);
        let functor_g = EmbeddingFunctor::new(2);
        let functor_h = EmbeddingFunctor::new(2);

        let eta: NaturalTransformation<_, _> = NaturalTransformation::new(&functor_f, &functor_g);
        let mu: NaturalTransformation<_, _> = NaturalTransformation::new(&functor_g, &functor_h);

        // Vertical composition mu . eta : F -> H
        let composed = eta.compose_vertical(&mu).unwrap();

        assert_eq!(composed.source_functor_id(), functor_f.id());
        assert_eq!(composed.target_functor_id(), functor_h.id());
    }
}

// =============================================================================
// TOPOS TESTS
// =============================================================================

mod topos_tests {
    use super::*;

    /// Test topos subobject classifier existence
    #[test]
    fn test_subobject_classifier_exists() {
        let topos = Topos::set_topos();

        let classifier = topos.subobject_classifier();
        assert!(classifier.is_some());

        let omega = classifier.unwrap();
        assert!(topos.is_valid_classifier(&omega));
    }

    /// Test truth morphism: true: 1 -> Omega
    #[test]
    fn test_truth_morphism() {
        let mut topos = Topos::set_topos();

        let terminal = topos.terminal_object().unwrap();
        let omega = topos.subobject_classifier().unwrap();

        let true_morphism = topos.truth_morphism().unwrap();
        let true_data = topos.get_morphism(true_morphism).unwrap();

        assert_eq!(true_data.source, terminal.id());
        assert_eq!(true_data.target, omega.id());
    }

    /// Test characteristic morphism construction
    #[test]
    fn test_characteristic_morphism() {
        let mut topos = Topos::set_topos();

        let a = topos.add_object("A");
        let b = topos.add_object("B");
        let mono = topos.add_monomorphism(a, b).unwrap();

        // Should produce characteristic morphism B -> Omega
        let chi = topos.characteristic_morphism(mono).unwrap();
        let omega = topos.subobject_classifier().unwrap();

        let chi_data = topos.get_morphism(chi).unwrap();
        assert_eq!(chi_data.source, b);
        assert_eq!(chi_data.target, omega.id());
    }

    /// Test pullback existence in topos
    #[test]
    fn test_pullback_exists() {
        let mut topos = Topos::set_topos();

        let a = topos.add_object("A");
        let b = topos.add_object("B");
        let c = topos.add_object("C");

        let f = topos.add_morphism(a, c, "f").unwrap();
        let g = topos.add_morphism(b, c, "g").unwrap();

        // Pullback should exist in a topos
        let pullback = topos.pullback(f, g).unwrap();

        assert!(pullback.is_valid());
        assert!(pullback.is_universal(&topos));
    }

    /// Test exponential object existence
    #[test]
    fn test_exponential_exists() {
        let mut topos = Topos::set_topos();

        let a = topos.add_object("A");
        let b = topos.add_object("B");

        // Exponential B^A should exist
        let exp = topos.exponential(a, b).unwrap();

        assert!(exp.is_valid());

        // Evaluation morphism should exist
        let eval = topos.evaluation_morphism(a, b).unwrap();
        let eval_data = topos.get_morphism(eval).unwrap();

        // eval: B^A x A -> B
        let product = topos.product(exp.id(), a).unwrap();
        assert_eq!(eval_data.source, product.id());
        assert_eq!(eval_data.target, b);
    }

    /// Test power object
    #[test]
    fn test_power_object() {
        let mut topos = Topos::set_topos();

        let a = topos.add_object("A");
        let omega = topos.subobject_classifier().unwrap();

        // Power object P(A) = Omega^A
        let power_a = topos.exponential(a, omega.id()).unwrap();

        assert!(power_a.is_valid());
    }
}

// =============================================================================
// HIGHER CATEGORY TESTS
// =============================================================================

mod higher_category_tests {
    use super::*;

    /// Test 2-category structure
    #[test]
    fn test_two_category_structure() {
        let mut two_cat = TwoCategory::new();

        // Add objects (0-cells)
        let a = two_cat.add_object("A");
        let b = two_cat.add_object("B");

        // Add 1-morphisms
        let f = two_cat.add_1_morphism(a, b, "f").unwrap();
        let g = two_cat.add_1_morphism(a, b, "g").unwrap();

        // Add 2-morphism alpha: f => g
        let alpha = two_cat.add_2_morphism(f, g, "alpha").unwrap();

        assert!(two_cat.get_2_morphism(alpha).is_some());
    }

    /// Test horizontal composition of 2-morphisms
    #[test]
    fn test_horizontal_composition() {
        let mut two_cat = TwoCategory::new();

        let a = two_cat.add_object("A");
        let b = two_cat.add_object("B");
        let c = two_cat.add_object("C");

        let f = two_cat.add_1_morphism(a, b, "f").unwrap();
        let g = two_cat.add_1_morphism(a, b, "g").unwrap();
        let h = two_cat.add_1_morphism(b, c, "h").unwrap();
        let k = two_cat.add_1_morphism(b, c, "k").unwrap();

        let alpha = two_cat.add_2_morphism(f, g, "alpha").unwrap();
        let beta = two_cat.add_2_morphism(h, k, "beta").unwrap();

        // Horizontal composition: beta * alpha : h.f => k.g
        let composed = two_cat.horizontal_compose(beta, alpha).unwrap();

        assert!(two_cat.get_2_morphism(composed).is_some());
    }

    /// Test vertical composition of 2-morphisms
    #[test]
    fn test_vertical_composition() {
        let mut two_cat = TwoCategory::new();

        let a = two_cat.add_object("A");
        let b = two_cat.add_object("B");

        let f = two_cat.add_1_morphism(a, b, "f").unwrap();
        let g = two_cat.add_1_morphism(a, b, "g").unwrap();
        let h = two_cat.add_1_morphism(a, b, "h").unwrap();

        let alpha = two_cat.add_2_morphism(f, g, "alpha").unwrap();
        let beta = two_cat.add_2_morphism(g, h, "beta").unwrap();

        // Vertical composition: beta . alpha : f => h
        let composed = two_cat.vertical_compose(beta, alpha).unwrap();

        let composed_data = two_cat.get_2_morphism(composed).unwrap();
        assert_eq!(composed_data.source_1_morphism, f);
        assert_eq!(composed_data.target_1_morphism, h);
    }

    /// Test interchange law: (delta . gamma) * (beta . alpha) = (delta * beta) . (gamma * alpha)
    #[test]
    fn test_interchange_law() {
        let mut two_cat = TwoCategory::new();

        let a = two_cat.add_object("A");
        let b = two_cat.add_object("B");
        let c = two_cat.add_object("C");

        // Setup for interchange law test
        let f = two_cat.add_1_morphism(a, b, "f").unwrap();
        let g = two_cat.add_1_morphism(a, b, "g").unwrap();
        let h = two_cat.add_1_morphism(a, b, "h").unwrap();

        let p = two_cat.add_1_morphism(b, c, "p").unwrap();
        let q = two_cat.add_1_morphism(b, c, "q").unwrap();
        let r = two_cat.add_1_morphism(b, c, "r").unwrap();

        let alpha = two_cat.add_2_morphism(f, g, "alpha").unwrap();
        let beta = two_cat.add_2_morphism(g, h, "beta").unwrap();
        let gamma = two_cat.add_2_morphism(p, q, "gamma").unwrap();
        let delta = two_cat.add_2_morphism(q, r, "delta").unwrap();

        // Left side: (delta . gamma) * (beta . alpha)
        let delta_gamma = two_cat.vertical_compose(delta, gamma).unwrap();
        let beta_alpha = two_cat.vertical_compose(beta, alpha).unwrap();
        let left = two_cat.horizontal_compose(delta_gamma, beta_alpha).unwrap();

        // Right side: (delta * beta) . (gamma * alpha)
        let delta_beta = two_cat.horizontal_compose(delta, beta).unwrap();
        let gamma_alpha = two_cat.horizontal_compose(gamma, alpha).unwrap();
        let right = two_cat.vertical_compose(delta_beta, gamma_alpha).unwrap();

        // Both should represent the same 2-morphism
        let left_data = two_cat.get_2_morphism(left).unwrap();
        let right_data = two_cat.get_2_morphism(right).unwrap();

        assert_eq!(left_data.source_1_morphism, right_data.source_1_morphism);
        assert_eq!(left_data.target_1_morphism, right_data.target_1_morphism);
    }
}

// =============================================================================
// COHERENCE VERIFICATION TESTS
// =============================================================================

mod coherence_tests {
    use super::*;

    /// Test pentagon identity for associator
    #[test]
    fn test_pentagon_identity() {
        let mut cat = VectorCategory::new(2);

        let a = cat.add_object("A");
        let b = cat.add_object("B");
        let c = cat.add_object("C");
        let d = cat.add_object("D");

        let result = verify_pentagon(&cat, a, b, c, d);

        match result {
            CoherenceResult::Satisfied => (),
            CoherenceResult::Violated(msg) => panic!("Pentagon failed: {}", msg),
            CoherenceResult::NotApplicable => (), // May not apply for this category
        }
    }

    /// Test triangle identity for unitor
    #[test]
    fn test_triangle_identity() {
        let mut cat = VectorCategory::new(2);

        let a = cat.add_object("A");
        let b = cat.add_object("B");

        let result = verify_triangle(&cat, a, b);

        match result {
            CoherenceResult::Satisfied => (),
            CoherenceResult::Violated(msg) => panic!("Triangle failed: {}", msg),
            CoherenceResult::NotApplicable => (),
        }
    }

    /// Test Mac Lane's coherence theorem implications
    #[test]
    fn test_coherence_theorem() {
        // Any two parallel morphisms built from associators and unitors
        // in a monoidal category are equal

        let mut cat = VectorCategory::with_monoidal_structure(2);

        let a = cat.add_object("A");
        let b = cat.add_object("B");
        let c = cat.add_object("C");

        // Two different bracketings should give same result
        let ab = cat.tensor_product(a, b).unwrap();
        let bc = cat.tensor_product(b, c).unwrap();

        let ab_c = cat.tensor_product(ab, c).unwrap();
        let a_bc = cat.tensor_product(a, bc).unwrap();

        // The associator should provide canonical isomorphism
        let assoc = cat.associator(a, b, c).unwrap();

        let assoc_data = cat.get_morphism(assoc).unwrap();
        assert_eq!(assoc_data.source, ab_c);
        assert_eq!(assoc_data.target, a_bc);
    }
}

// =============================================================================
// PROPERTY-BASED TESTS
// =============================================================================

mod property_tests {
    use super::*;

    proptest! {
        /// Property: Identity is unique (any id satisfies identity laws is THE identity)
        #[test]
        fn prop_identity_unique(n in 1..10usize) {
            let mut cat = SetCategory::new();
            let objects: Vec<_> = (0..n).map(|i| cat.add_object(&format!("O{}", i))).collect();

            for &obj in &objects {
                let id1 = cat.identity(obj).unwrap();
                let id2 = cat.identity(obj).unwrap();

                // Both satisfy identity laws, so must be "equal"
                let id1_data = cat.get_morphism(id1).unwrap();
                let id2_data = cat.get_morphism(id2).unwrap();

                prop_assert_eq!(id1_data.source, id2_data.source);
                prop_assert_eq!(id1_data.target, id2_data.target);
            }
        }

        /// Property: Composition is closed
        #[test]
        fn prop_composition_closed(n in 2..5usize) {
            let mut cat = SetCategory::new();
            let objects: Vec<_> = (0..n).map(|i| cat.add_object(&format!("O{}", i))).collect();

            // Create chain of morphisms
            let mut morphisms = Vec::new();
            for i in 0..(n-1) {
                let m = cat.add_morphism(objects[i], objects[i+1], &format!("f{}", i)).unwrap();
                morphisms.push(m);
            }

            // Compose all
            let mut result = morphisms[0];
            for &m in &morphisms[1..] {
                result = cat.compose(m, result).unwrap();
            }

            // Result should still be a valid morphism
            prop_assert!(cat.get_morphism(result).is_some());
        }
    }
}

// =============================================================================
// EDGE CASE TESTS
// =============================================================================

mod edge_case_tests {
    use super::*;

    /// Test empty category
    #[test]
    fn test_empty_category() {
        let cat = SetCategory::new();
        assert!(cat.verify_laws()); // Empty category trivially satisfies laws
    }

    /// Test single-object category (monoid)
    #[test]
    fn test_monoid_category() {
        let mut cat = SetCategory::new();
        let a = cat.add_object("A");

        // Self-morphisms form a monoid
        let f = cat.add_morphism(a, a, "f").unwrap();
        let g = cat.add_morphism(a, a, "g").unwrap();

        // Should compose
        let fg = cat.compose(f, g).unwrap();
        let gf = cat.compose(g, f).unwrap();

        // Both compositions valid
        assert!(cat.get_morphism(fg).is_some());
        assert!(cat.get_morphism(gf).is_some());
    }

    /// Test morphism lookup for non-existent morphism
    #[test]
    fn test_nonexistent_morphism() {
        let cat = SetCategory::new();
        let fake_id = MorphismId::new();

        assert!(cat.get_morphism(fake_id).is_none());
    }

    /// Test object lookup for non-existent object
    #[test]
    fn test_nonexistent_object() {
        let cat = SetCategory::new();
        let fake_id = ObjectId::new();

        assert!(cat.get_object(fake_id).is_none());
    }
}
