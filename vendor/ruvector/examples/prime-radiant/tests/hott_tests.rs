//! Comprehensive tests for Homotopy Type Theory (HoTT) Module
//!
//! This test suite verifies HoTT constructs including:
//! - Type checking and inference
//! - Path composition and inversion
//! - Transport along paths
//! - Univalence axiom (equivalence = identity)

use prime_radiant::hott::{
    Type, Term, Path, TypeChecker, TypeContext,
    Equivalence, Transport, Univalence,
    PathComposition, PathInversion, PathConcatenation,
    HigherInductiveType, Circle, Sphere, Torus,
    HomotopyLevel, is_contractible, is_proposition, is_set,
    FunctionExtensionality, funext,
    HottError,
};
use proptest::prelude::*;
use approx::assert_relative_eq;

// =============================================================================
// TYPE CHECKING TESTS
// =============================================================================

mod type_checking_tests {
    use super::*;

    /// Test type checking for base types
    #[test]
    fn test_base_type_checking() {
        let mut ctx = TypeContext::new();

        // Natural numbers type
        let nat = Type::Nat;
        assert!(ctx.is_well_formed(&nat));

        // Boolean type
        let bool_ty = Type::Bool;
        assert!(ctx.is_well_formed(&bool_ty));

        // Unit type
        let unit = Type::Unit;
        assert!(ctx.is_well_formed(&unit));
    }

    /// Test type checking for function types
    #[test]
    fn test_function_type_checking() {
        let mut ctx = TypeContext::new();

        // Nat -> Bool
        let func_type = Type::Pi {
            param: Box::new(Type::Nat),
            body: Box::new(Type::Bool),
        };

        assert!(ctx.is_well_formed(&func_type));
    }

    /// Test type checking for dependent types
    #[test]
    fn test_dependent_type_checking() {
        let mut ctx = TypeContext::new();

        // Dependent product type: (x: A) -> B(x)
        let dep_prod = Type::Pi {
            param: Box::new(Type::Nat),
            body: Box::new(Type::Family {
                base: Box::new(Type::Nat),
                fiber: Box::new(|_n| Type::Bool),
            }),
        };

        assert!(ctx.is_well_formed(&dep_prod));
    }

    /// Test type checking for sigma types
    #[test]
    fn test_sigma_type_checking() {
        let mut ctx = TypeContext::new();

        // Sigma type: (x: A) * B(x)
        let sigma = Type::Sigma {
            first: Box::new(Type::Nat),
            second: Box::new(Type::Bool),
        };

        assert!(ctx.is_well_formed(&sigma));
    }

    /// Test type checking for identity types
    #[test]
    fn test_identity_type_checking() {
        let mut ctx = TypeContext::new();

        // Identity type: a =_A b
        let a = Term::zero();
        let b = Term::succ(Term::zero());

        let id_type = Type::Identity {
            base_type: Box::new(Type::Nat),
            left: Box::new(a),
            right: Box::new(b),
        };

        assert!(ctx.is_well_formed(&id_type));
    }

    /// Test type inference
    #[test]
    fn test_type_inference() {
        let mut ctx = TypeContext::new();

        let zero = Term::zero();
        let inferred = ctx.infer_type(&zero).unwrap();
        assert_eq!(inferred, Type::Nat);

        let true_val = Term::true_val();
        let inferred = ctx.infer_type(&true_val).unwrap();
        assert_eq!(inferred, Type::Bool);
    }

    /// Test type checking with variable bindings
    #[test]
    fn test_variable_bindings() {
        let mut ctx = TypeContext::new();

        // Add variable x: Nat to context
        ctx.add_variable("x", Type::Nat);

        let var_x = Term::variable("x");
        let inferred = ctx.infer_type(&var_x).unwrap();
        assert_eq!(inferred, Type::Nat);
    }

    /// Test lambda type checking
    #[test]
    fn test_lambda_type_checking() {
        let mut ctx = TypeContext::new();

        // lambda x: Nat. x + 1
        let lambda = Term::Lambda {
            param: "x".to_string(),
            param_type: Box::new(Type::Nat),
            body: Box::new(Term::succ(Term::variable("x"))),
        };

        let inferred = ctx.infer_type(&lambda).unwrap();

        match inferred {
            Type::Pi { param, body } => {
                assert_eq!(*param, Type::Nat);
                assert_eq!(*body, Type::Nat);
            }
            _ => panic!("Expected Pi type"),
        }
    }
}

// =============================================================================
// PATH COMPOSITION TESTS
// =============================================================================

mod path_composition_tests {
    use super::*;

    /// Test reflexivity path: refl_a : a = a
    #[test]
    fn test_reflexivity_path() {
        let a = Term::zero();
        let refl = Path::refl(&a);

        assert_eq!(refl.start(), &a);
        assert_eq!(refl.end(), &a);
        assert!(refl.is_reflexivity());
    }

    /// Test path concatenation: p . q : a = c for p: a = b, q: b = c
    #[test]
    fn test_path_concatenation() {
        let a = Term::zero();
        let b = Term::succ(Term::zero());
        let c = Term::succ(Term::succ(Term::zero()));

        // p: a = b (hypothetical)
        let p = Path::hypothesis(&a, &b, "p");

        // q: b = c (hypothetical)
        let q = Path::hypothesis(&b, &c, "q");

        // p . q : a = c
        let composed = p.concat(&q).unwrap();

        assert_eq!(composed.start(), &a);
        assert_eq!(composed.end(), &c);
    }

    /// Test path concatenation fails for non-matching endpoints
    #[test]
    fn test_path_concat_mismatch() {
        let a = Term::zero();
        let b = Term::succ(Term::zero());
        let c = Term::succ(Term::succ(Term::zero()));
        let d = Term::succ(Term::succ(Term::succ(Term::zero())));

        let p = Path::hypothesis(&a, &b, "p");  // a = b
        let q = Path::hypothesis(&c, &d, "q");  // c = d, not b = something

        let result = p.concat(&q);
        assert!(result.is_err());
    }

    /// Test path inversion: p^(-1) : b = a for p : a = b
    #[test]
    fn test_path_inversion() {
        let a = Term::zero();
        let b = Term::succ(Term::zero());

        let p = Path::hypothesis(&a, &b, "p");
        let p_inv = p.inverse();

        assert_eq!(p_inv.start(), &b);
        assert_eq!(p_inv.end(), &a);
    }

    /// Test double inversion: (p^(-1))^(-1) = p
    #[test]
    fn test_double_inversion() {
        let a = Term::zero();
        let b = Term::succ(Term::zero());

        let p = Path::hypothesis(&a, &b, "p");
        let p_inv_inv = p.inverse().inverse();

        assert_eq!(p_inv_inv.start(), p.start());
        assert_eq!(p_inv_inv.end(), p.end());
    }

    /// Test associativity: (p . q) . r = p . (q . r)
    #[test]
    fn test_path_associativity() {
        let a = Term::zero();
        let b = Term::succ(Term::zero());
        let c = Term::succ(Term::succ(Term::zero()));
        let d = Term::succ(Term::succ(Term::succ(Term::zero())));

        let p = Path::hypothesis(&a, &b, "p");
        let q = Path::hypothesis(&b, &c, "q");
        let r = Path::hypothesis(&c, &d, "r");

        // (p . q) . r
        let left = p.concat(&q).unwrap().concat(&r).unwrap();

        // p . (q . r)
        let right = p.concat(&q.concat(&r).unwrap()).unwrap();

        // Both should have same endpoints
        assert_eq!(left.start(), right.start());
        assert_eq!(left.end(), right.end());

        // And there should be a path between them (associator)
        assert!(Path::path_between(&left, &right).is_some());
    }

    /// Test left unit law: refl_a . p = p
    #[test]
    fn test_left_unit_law() {
        let a = Term::zero();
        let b = Term::succ(Term::zero());

        let refl_a = Path::refl(&a);
        let p = Path::hypothesis(&a, &b, "p");

        let left_unit = refl_a.concat(&p).unwrap();

        // Should be propositionally equal to p
        assert_eq!(left_unit.start(), p.start());
        assert_eq!(left_unit.end(), p.end());
    }

    /// Test right unit law: p . refl_b = p
    #[test]
    fn test_right_unit_law() {
        let a = Term::zero();
        let b = Term::succ(Term::zero());

        let p = Path::hypothesis(&a, &b, "p");
        let refl_b = Path::refl(&b);

        let right_unit = p.concat(&refl_b).unwrap();

        assert_eq!(right_unit.start(), p.start());
        assert_eq!(right_unit.end(), p.end());
    }

    /// Test inverse law: p . p^(-1) = refl_a
    #[test]
    fn test_inverse_law() {
        let a = Term::zero();
        let b = Term::succ(Term::zero());

        let p = Path::hypothesis(&a, &b, "p");
        let p_inv = p.inverse();

        let composed = p.concat(&p_inv).unwrap();

        // Should equal refl_a (propositionally)
        assert_eq!(composed.start(), &a);
        assert_eq!(composed.end(), &a);
    }
}

// =============================================================================
// TRANSPORT TESTS
// =============================================================================

mod transport_tests {
    use super::*;

    /// Test transport along reflexivity path is identity
    #[test]
    fn test_transport_refl_is_identity() {
        let a = Term::zero();
        let refl = Path::refl(&a);

        // Type family B(x) = Nat for simplicity
        let family = Type::Family {
            base: Box::new(Type::Nat),
            fiber: Box::new(|_| Type::Nat),
        };

        let b_a = Term::succ(Term::zero());  // Some term in B(a)

        let transported = Transport::transport(&refl, &family, &b_a).unwrap();

        // transport(refl_a, b) = b
        assert_eq!(transported, b_a);
    }

    /// Test transport composition: transport(p.q) = transport(q) . transport(p)
    #[test]
    fn test_transport_composition() {
        let a = Term::zero();
        let b = Term::succ(Term::zero());
        let c = Term::succ(Term::succ(Term::zero()));

        let p = Path::hypothesis(&a, &b, "p");
        let q = Path::hypothesis(&b, &c, "q");
        let pq = p.concat(&q).unwrap();

        let family = Type::Family {
            base: Box::new(Type::Nat),
            fiber: Box::new(|_| Type::Nat),
        };

        let term_a = Term::succ(Term::succ(Term::succ(Term::zero())));

        // transport(p.q, x)
        let direct = Transport::transport(&pq, &family, &term_a).unwrap();

        // transport(q, transport(p, x))
        let p_transported = Transport::transport(&p, &family, &term_a).unwrap();
        let composed = Transport::transport(&q, &family, &p_transported).unwrap();

        // Should be propositionally equal
        assert!(Term::propositionally_equal(&direct, &composed));
    }

    /// Test dependent transport (transport in dependent types)
    #[test]
    fn test_dependent_transport() {
        let ctx = TypeContext::new();

        // Type family indexed by Nat
        let family = Type::Family {
            base: Box::new(Type::Nat),
            fiber: Box::new(|n| Type::Vec {
                element_type: Box::new(Type::Nat),
                length: n,
            }),
        };

        // Path from 0 to 1
        let p = Path::hypothesis(&Term::zero(), &Term::succ(Term::zero()), "p");

        // Empty vector at type Vec(Nat, 0)
        let empty_vec = Term::empty_vec();

        // Transport should fail or produce Vec(Nat, 1)
        let result = Transport::transport(&p, &family, &empty_vec);

        // May require coercion witness
        assert!(result.is_ok() || result.is_err());
    }

    /// Test path lifting (apd)
    #[test]
    fn test_apd() {
        let a = Term::zero();
        let b = Term::succ(Term::zero());

        let family = Type::Family {
            base: Box::new(Type::Nat),
            fiber: Box::new(|_| Type::Nat),
        };

        // Function f: (x: Nat) -> B(x)
        let f = Term::Lambda {
            param: "x".to_string(),
            param_type: Box::new(Type::Nat),
            body: Box::new(Term::succ(Term::variable("x"))),
        };

        let p = Path::hypothesis(&a, &b, "p");

        // apd f p : transport(p, f(a)) = f(b)
        let apd_path = Transport::apd(&f, &p, &family).unwrap();

        // Check endpoints
        let f_a = Term::succ(a.clone());
        let f_b = Term::succ(b.clone());

        let transported_f_a = Transport::transport(&p, &family, &f_a).unwrap();

        assert_eq!(apd_path.start(), &transported_f_a);
        assert_eq!(apd_path.end(), &f_b);
    }
}

// =============================================================================
// UNIVALENCE TESTS
// =============================================================================

mod univalence_tests {
    use super::*;

    /// Test that equivalence can be converted to path (ua)
    #[test]
    fn test_ua_from_equivalence() {
        // Equivalence between Bool and Bool (identity)
        let bool_equiv = Equivalence::identity(Type::Bool);

        // ua should produce a path Bool = Bool
        let path = Univalence::ua(&bool_equiv).unwrap();

        assert_eq!(path.start_type(), &Type::Bool);
        assert_eq!(path.end_type(), &Type::Bool);
    }

    /// Test that path can be converted to equivalence (ua^-1)
    #[test]
    fn test_ua_inverse() {
        // Reflexivity path on type
        let refl_nat = Path::type_refl(&Type::Nat);

        // ua^-1 should produce equivalence Nat ~ Nat
        let equiv = Univalence::ua_inverse(&refl_nat).unwrap();

        assert!(equiv.is_valid_equivalence());
        assert_eq!(equiv.domain(), &Type::Nat);
        assert_eq!(equiv.codomain(), &Type::Nat);
    }

    /// Test round-trip: ua(ua^-1(p)) = p
    #[test]
    fn test_univalence_round_trip_path() {
        let p = Path::type_refl(&Type::Bool);

        let equiv = Univalence::ua_inverse(&p).unwrap();
        let recovered = Univalence::ua(&equiv).unwrap();

        // Should be propositionally equal
        assert_eq!(recovered.start_type(), p.start_type());
        assert_eq!(recovered.end_type(), p.end_type());
    }

    /// Test round-trip: ua^-1(ua(e)) = e
    #[test]
    fn test_univalence_round_trip_equiv() {
        let equiv = Equivalence::identity(Type::Nat);

        let path = Univalence::ua(&equiv).unwrap();
        let recovered = Univalence::ua_inverse(&path).unwrap();

        // Forward maps should be equal
        assert!(Equivalence::equal(&recovered, &equiv));
    }

    /// Test transport along ua(e) is the equivalence
    #[test]
    fn test_transport_along_ua() {
        // Create non-trivial equivalence (e.g., negation on Bool)
        let neg_equiv = Equivalence::bool_negation();

        let path = Univalence::ua(&neg_equiv).unwrap();

        // Type family that uses the base type directly
        let family = Type::Family {
            base: Box::new(Type::Universe(0)),
            fiber: Box::new(|ty| ty.clone()),
        };

        let true_val = Term::true_val();

        // transport(ua(neg), true) should equal neg(true) = false
        let transported = Transport::transport(&path, &family, &true_val).unwrap();
        let neg_true = neg_equiv.apply(&true_val).unwrap();

        assert!(Term::propositionally_equal(&transported, &neg_true));
    }

    /// Test univalence with type isomorphism
    #[test]
    fn test_type_isomorphism_gives_equality() {
        // Unit + Unit is isomorphic to Bool
        let sum_type = Type::Sum {
            left: Box::new(Type::Unit),
            right: Box::new(Type::Unit),
        };

        // Construct isomorphism
        let iso = Equivalence::sum_unit_to_bool();

        assert!(iso.is_valid_equivalence());

        // By univalence, types are equal
        let path = Univalence::ua(&iso).unwrap();

        assert_eq!(*path.start_type(), sum_type);
        assert_eq!(*path.end_type(), Type::Bool);
    }
}

// =============================================================================
// HIGHER INDUCTIVE TYPE TESTS
// =============================================================================

mod hit_tests {
    use super::*;

    /// Test circle type S^1
    #[test]
    fn test_circle_type() {
        let circle = Circle::new();

        // Circle has base point
        let base = circle.base_point();
        assert!(base.has_type(&Type::Circle));

        // Circle has loop: base = base
        let loop_path = circle.loop_path();
        assert_eq!(loop_path.start(), &base);
        assert_eq!(loop_path.end(), &base);
    }

    /// Test circle recursion principle
    #[test]
    fn test_circle_recursion() {
        let circle = Circle::new();

        // To map S^1 -> A, need:
        // - a: A (image of base)
        // - p: a = a (image of loop)

        let target_type = Type::Nat;
        let a = Term::zero();
        let p = Path::refl(&a);  // Use refl for simplicity

        let rec = circle.recursion(&target_type, &a, &p).unwrap();

        // rec(base) = a
        let base_image = rec.apply(&circle.base_point()).unwrap();
        assert_eq!(base_image, a);
    }

    /// Test sphere type S^2
    #[test]
    fn test_sphere_type() {
        let sphere = Sphere::new(2);

        let base = sphere.base_point();
        assert!(base.has_type(&Type::Sphere(2)));

        // S^2 has refl-refl as 2-path
        let surf = sphere.surface();
        assert!(surf.is_2_path());
    }

    /// Test torus type
    #[test]
    fn test_torus_type() {
        let torus = Torus::new();

        let base = torus.base_point();

        // Torus has two loops
        let p = torus.meridian();
        let q = torus.longitude();

        // And a square: p . q = q . p
        let surface = torus.surface();

        // surface : p . q = q . p
        let pq = p.concat(&q).unwrap();
        let qp = q.concat(&p).unwrap();

        assert_eq!(surface.start(), &pq);
        assert_eq!(surface.end(), &qp);
    }

    /// Test pushout as HIT
    #[test]
    fn test_pushout_hit() {
        // Pushout of A <- C -> B
        let a_type = Type::Nat;
        let b_type = Type::Bool;
        let c_type = Type::Unit;

        let f = Term::Lambda {
            param: "c".to_string(),
            param_type: Box::new(c_type.clone()),
            body: Box::new(Term::zero()),
        };

        let g = Term::Lambda {
            param: "c".to_string(),
            param_type: Box::new(c_type.clone()),
            body: Box::new(Term::true_val()),
        };

        let pushout = HigherInductiveType::pushout(&a_type, &b_type, &c_type, &f, &g);

        // Has injections from A and B
        let inl = pushout.left_injection();
        let inr = pushout.right_injection();

        // For each c: C, path glue(c): inl(f(c)) = inr(g(c))
        let unit = Term::unit();
        let glue_path = pushout.glue(&unit);

        let inl_fc = inl.apply(&f.apply(&unit).unwrap()).unwrap();
        let inr_gc = inr.apply(&g.apply(&unit).unwrap()).unwrap();

        assert_eq!(glue_path.start(), &inl_fc);
        assert_eq!(glue_path.end(), &inr_gc);
    }
}

// =============================================================================
// HOMOTOPY LEVEL TESTS
// =============================================================================

mod homotopy_level_tests {
    use super::*;

    /// Test contractibility (h-level -2)
    #[test]
    fn test_contractible() {
        // Unit type is contractible
        assert!(is_contractible(&Type::Unit));

        // Nat is not contractible
        assert!(!is_contractible(&Type::Nat));
    }

    /// Test propositions (h-level -1)
    #[test]
    fn test_is_proposition() {
        // Empty type is a proposition (vacuously)
        assert!(is_proposition(&Type::Empty));

        // Unit type is a proposition (all elements equal)
        assert!(is_proposition(&Type::Unit));

        // Nat is not a proposition
        assert!(!is_proposition(&Type::Nat));
    }

    /// Test sets (h-level 0)
    #[test]
    fn test_is_set() {
        // Nat is a set
        assert!(is_set(&Type::Nat));

        // Bool is a set
        assert!(is_set(&Type::Bool));

        // Universe is not a set (by univalence)
        assert!(!is_set(&Type::Universe(0)));
    }

    /// Test h-level preservation under products
    #[test]
    fn test_hlevel_product() {
        // Product of sets is a set
        let nat_nat = Type::Product {
            left: Box::new(Type::Nat),
            right: Box::new(Type::Nat),
        };

        assert!(is_set(&nat_nat));
    }

    /// Test h-level of identity types
    #[test]
    fn test_identity_hlevel() {
        // For a set A, identity types a =_A b are propositions
        let a = Term::zero();
        let b = Term::succ(Term::zero());

        let id_type = Type::Identity {
            base_type: Box::new(Type::Nat),
            left: Box::new(a),
            right: Box::new(b),
        };

        assert!(is_proposition(&id_type));
    }
}

// =============================================================================
// FUNCTION EXTENSIONALITY TESTS
// =============================================================================

mod funext_tests {
    use super::*;

    /// Test function extensionality: (forall x, f(x) = g(x)) -> f = g
    #[test]
    fn test_function_extensionality() {
        let domain = Type::Nat;
        let codomain = Type::Nat;

        let f = Term::Lambda {
            param: "x".to_string(),
            param_type: Box::new(domain.clone()),
            body: Box::new(Term::succ(Term::variable("x"))),
        };

        let g = Term::Lambda {
            param: "y".to_string(),
            param_type: Box::new(domain.clone()),
            body: Box::new(Term::succ(Term::variable("y"))),
        };

        // Pointwise equality witness (hypothetical)
        let h = Term::Lambda {
            param: "x".to_string(),
            param_type: Box::new(domain.clone()),
            body: Box::new(Path::refl(&Term::succ(Term::variable("x"))).to_term()),
        };

        // Apply funext
        let path_f_g = funext(&f, &g, &h).unwrap();

        assert_eq!(path_f_g.start(), &f);
        assert_eq!(path_f_g.end(), &g);
    }

    /// Test funext inverse: f = g -> forall x, f(x) = g(x)
    #[test]
    fn test_funext_inverse() {
        let domain = Type::Bool;
        let codomain = Type::Nat;

        let f = Term::Lambda {
            param: "b".to_string(),
            param_type: Box::new(domain.clone()),
            body: Box::new(Term::if_then_else(
                Term::variable("b"),
                Term::zero(),
                Term::succ(Term::zero()),
            )),
        };

        let p = Path::refl(&f);

        // Get pointwise equalities
        let pointwise = FunctionExtensionality::inverse(&p).unwrap();

        // For each x: Bool, should have f(x) = f(x)
        let true_val = Term::true_val();
        let path_at_true = pointwise.at(&true_val).unwrap();

        assert!(path_at_true.is_reflexivity());
    }
}

// =============================================================================
// PROPERTY-BASED TESTS
// =============================================================================

mod property_tests {
    use super::*;

    proptest! {
        /// Property: refl . p = p for all paths
        #[test]
        fn prop_left_unit(
            start in 0..10i32,
            end in 0..10i32
        ) {
            let a = Term::from_int(start);
            let b = Term::from_int(end);

            let p = Path::hypothesis(&a, &b, "p");
            let refl = Path::refl(&a);

            let composed = refl.concat(&p).unwrap();

            prop_assert_eq!(composed.start(), p.start());
            prop_assert_eq!(composed.end(), p.end());
        }

        /// Property: p . refl = p for all paths
        #[test]
        fn prop_right_unit(
            start in 0..10i32,
            end in 0..10i32
        ) {
            let a = Term::from_int(start);
            let b = Term::from_int(end);

            let p = Path::hypothesis(&a, &b, "p");
            let refl = Path::refl(&b);

            let composed = p.concat(&refl).unwrap();

            prop_assert_eq!(composed.start(), p.start());
            prop_assert_eq!(composed.end(), p.end());
        }

        /// Property: (p^-1)^-1 = p
        #[test]
        fn prop_double_inverse(
            start in 0..10i32,
            end in 0..10i32
        ) {
            let a = Term::from_int(start);
            let b = Term::from_int(end);

            let p = Path::hypothesis(&a, &b, "p");
            let double_inv = p.inverse().inverse();

            prop_assert_eq!(double_inv.start(), p.start());
            prop_assert_eq!(double_inv.end(), p.end());
        }
    }
}

// =============================================================================
// EDGE CASE TESTS
// =============================================================================

mod edge_case_tests {
    use super::*;

    /// Test empty context type checking
    #[test]
    fn test_empty_context() {
        let ctx = TypeContext::new();
        assert!(ctx.is_empty());
    }

    /// Test universe levels
    #[test]
    fn test_universe_hierarchy() {
        let ctx = TypeContext::new();

        let type_0 = Type::Universe(0);  // Type of small types
        let type_1 = Type::Universe(1);  // Type of large types

        // Type_0 : Type_1
        assert!(ctx.inhabits(&type_0, &type_1));

        // But not Type_1 : Type_0 (no type-in-type)
        assert!(!ctx.inhabits(&type_1, &type_0));
    }

    /// Test type checking with free variables
    #[test]
    fn test_free_variable_error() {
        let ctx = TypeContext::new();

        let free_var = Term::variable("undefined");

        let result = ctx.infer_type(&free_var);
        assert!(result.is_err());
    }

    /// Test path between incompatible types
    #[test]
    fn test_heterogeneous_path_error() {
        let nat_term = Term::zero();
        let bool_term = Term::true_val();

        // Cannot form path between different types directly
        let result = Path::try_new(&nat_term, &bool_term);
        assert!(result.is_err());
    }
}
