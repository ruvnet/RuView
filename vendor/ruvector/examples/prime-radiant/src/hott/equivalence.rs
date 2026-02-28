//! Type equivalences and the Univalence Axiom
//!
//! An equivalence A ~ B is a function f : A -> B with a two-sided inverse.
//! The Univalence Axiom states that (A ~ B) ~ (A = B), meaning
//! equivalent types can be identified.
//!
//! This is the central axiom of HoTT that distinguishes it from
//! ordinary type theory.

use std::sync::Arc;
use super::{Term, Type, Path, TypeError};

/// A function with homotopy data
pub type HomotopyFn = Arc<dyn Fn(&Term) -> Term + Send + Sync>;

/// Half-adjoint equivalence between types A and B
///
/// This is the "good" notion of equivalence in HoTT that
/// provides both computational and logical properties.
#[derive(Clone)]
pub struct Equivalence {
    /// Domain type A
    pub domain: Type,
    /// Codomain type B
    pub codomain: Type,
    /// Forward function f : A -> B
    pub forward: HomotopyFn,
    /// Backward function g : B -> A
    pub backward: HomotopyFn,
    /// Right homotopy: (x : A) -> g(f(x)) = x
    pub section: HomotopyFn,
    /// Left homotopy: (y : B) -> f(g(y)) = y
    pub retraction: HomotopyFn,
    /// Coherence: for all x, ap f (section x) = retraction (f x)
    pub coherence: Option<HomotopyFn>,
}

impl Equivalence {
    /// Create a new equivalence
    pub fn new(
        domain: Type,
        codomain: Type,
        forward: impl Fn(&Term) -> Term + Send + Sync + 'static,
        backward: impl Fn(&Term) -> Term + Send + Sync + 'static,
        section: impl Fn(&Term) -> Term + Send + Sync + 'static,
        retraction: impl Fn(&Term) -> Term + Send + Sync + 'static,
    ) -> Self {
        Equivalence {
            domain,
            codomain,
            forward: Arc::new(forward),
            backward: Arc::new(backward),
            section: Arc::new(section),
            retraction: Arc::new(retraction),
            coherence: None,
        }
    }

    /// Add coherence data for half-adjoint equivalence
    pub fn with_coherence(
        mut self,
        coherence: impl Fn(&Term) -> Term + Send + Sync + 'static,
    ) -> Self {
        self.coherence = Some(Arc::new(coherence));
        self
    }

    /// Apply the forward function
    pub fn apply(&self, term: &Term) -> Term {
        (self.forward)(term)
    }

    /// Apply the backward function (inverse)
    pub fn unapply(&self, term: &Term) -> Term {
        (self.backward)(term)
    }

    /// Get the section proof for a term
    pub fn section_at(&self, term: &Term) -> Term {
        (self.section)(term)
    }

    /// Get the retraction proof for a term
    pub fn retraction_at(&self, term: &Term) -> Term {
        (self.retraction)(term)
    }

    /// Compose two equivalences: A ~ B and B ~ C gives A ~ C
    pub fn compose(&self, other: &Equivalence) -> Result<Equivalence, TypeError> {
        // Check that types match
        if !self.codomain.structural_eq(&other.domain) {
            return Err(TypeError::TypeMismatch {
                expected: format!("{:?}", self.codomain),
                found: format!("{:?}", other.domain),
            });
        }

        let f1 = Arc::clone(&self.forward);
        let f1_section = Arc::clone(&self.forward);
        let f2 = Arc::clone(&other.forward);
        let g1 = Arc::clone(&self.backward);
        let g2 = Arc::clone(&other.backward);
        let g2_retract = Arc::clone(&other.backward);
        let s1 = Arc::clone(&self.section);
        let s2 = Arc::clone(&other.section);
        let r1 = Arc::clone(&self.retraction);
        let r2 = Arc::clone(&other.retraction);

        Ok(Equivalence {
            domain: self.domain.clone(),
            codomain: other.codomain.clone(),
            forward: Arc::new(move |x| f2(&f1(x))),
            backward: Arc::new(move |x| g1(&g2(x))),
            section: Arc::new(move |x| {
                // g1(g2(f2(f1(x)))) = x
                // Use s1 and s2 together
                let inner = s2(&f1_section(x));
                let _outer = s1(x);
                // In full implementation, would compose these paths
                inner
            }),
            retraction: Arc::new(move |y| {
                // f2(f1(g1(g2(y)))) = y
                let inner = r1(&g2_retract(y));
                let _outer = r2(y);
                inner
            }),
            coherence: None,
        })
    }

    /// Create the inverse equivalence: A ~ B gives B ~ A
    pub fn inverse(&self) -> Equivalence {
        Equivalence {
            domain: self.codomain.clone(),
            codomain: self.domain.clone(),
            forward: Arc::clone(&self.backward),
            backward: Arc::clone(&self.forward),
            section: Arc::clone(&self.retraction),
            retraction: Arc::clone(&self.section),
            coherence: None,
        }
    }

    /// Identity equivalence: A ~ A
    pub fn identity(ty: Type) -> Equivalence {
        Equivalence {
            domain: ty.clone(),
            codomain: ty,
            forward: Arc::new(|x| x.clone()),
            backward: Arc::new(|x| x.clone()),
            section: Arc::new(|x| Term::Refl(Box::new(x.clone()))),
            retraction: Arc::new(|x| Term::Refl(Box::new(x.clone()))),
            coherence: Some(Arc::new(|x| Term::Refl(Box::new(x.clone())))),
        }
    }
}

impl std::fmt::Debug for Equivalence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Equivalence")
            .field("domain", &self.domain)
            .field("codomain", &self.codomain)
            .finish()
    }
}

/// Simple isomorphism (without homotopy data)
/// Used for computational purposes when coherence isn't needed
#[derive(Clone)]
pub struct Isomorphism {
    pub domain: Type,
    pub codomain: Type,
    pub forward: HomotopyFn,
    pub backward: HomotopyFn,
}

impl Isomorphism {
    pub fn new(
        domain: Type,
        codomain: Type,
        forward: impl Fn(&Term) -> Term + Send + Sync + 'static,
        backward: impl Fn(&Term) -> Term + Send + Sync + 'static,
    ) -> Self {
        Isomorphism {
            domain,
            codomain,
            forward: Arc::new(forward),
            backward: Arc::new(backward),
        }
    }

    /// Convert to full equivalence (need to provide homotopy witnesses)
    pub fn to_equivalence(
        self,
        section: impl Fn(&Term) -> Term + Send + Sync + 'static,
        retraction: impl Fn(&Term) -> Term + Send + Sync + 'static,
    ) -> Equivalence {
        Equivalence {
            domain: self.domain,
            codomain: self.codomain,
            forward: self.forward,
            backward: self.backward,
            section: Arc::new(section),
            retraction: Arc::new(retraction),
            coherence: None,
        }
    }
}

impl std::fmt::Debug for Isomorphism {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Isomorphism")
            .field("domain", &self.domain)
            .field("codomain", &self.codomain)
            .finish()
    }
}

/// The Univalence Axiom
///
/// ua : (A ~ B) -> (A = B)
///
/// This function computes a path between types from an equivalence.
/// The inverse is:
///
/// ua^(-1) : (A = B) -> (A ~ B)
///
/// given by transport along the path.
pub fn univalence(equiv: Equivalence) -> Path {
    // Create a path in the universe of types
    // The proof term witnesses the univalence axiom
    let proof = Term::Pair {
        fst: Box::new(Term::Lambda {
            var: "x".to_string(),
            body: Box::new((equiv.forward)(&Term::Var("x".to_string()))),
        }),
        snd: Box::new(Term::Pair {
            fst: Box::new(Term::Lambda {
                var: "y".to_string(),
                body: Box::new((equiv.backward)(&Term::Var("y".to_string()))),
            }),
            snd: Box::new(Term::Pair {
                fst: Box::new(Term::Lambda {
                    var: "x".to_string(),
                    body: Box::new((equiv.section)(&Term::Var("x".to_string()))),
                }),
                snd: Box::new(Term::Lambda {
                    var: "y".to_string(),
                    body: Box::new((equiv.retraction)(&Term::Var("y".to_string()))),
                }),
            }),
        }),
    };

    // Source and target are type terms
    let source = type_to_term(&equiv.domain);
    let target = type_to_term(&equiv.codomain);

    Path::with_type(
        source,
        target,
        proof,
        Type::Universe(std::cmp::max(
            equiv.domain.universe_level(),
            equiv.codomain.universe_level(),
        )),
    )
}

/// Computation rule for univalence (beta)
/// transport (ua e) = forward e
pub fn ua_beta(equiv: &Equivalence, term: &Term) -> Term {
    equiv.apply(term)
}

/// Uniqueness rule for univalence (eta)
/// ua (idtoeqv p) = p
pub fn ua_eta(path: &Path) -> Path {
    // The path is unchanged (this is a definitional equality in cubical type theory)
    path.clone()
}

/// Convert type equality (path) back to equivalence
/// This is the inverse of univalence: (A = B) -> (A ~ B)
pub fn path_to_equiv(path: &Path, source_type: Type, target_type: Type) -> Equivalence {
    let proof = path.proof().clone();

    // Transport gives the forward function
    let forward_proof = proof.clone();
    let backward_proof = Term::PathInverse(Box::new(proof.clone()));

    Equivalence::new(
        source_type.clone(),
        target_type.clone(),
        move |x| Term::Transport {
            family: Box::new(Term::Lambda {
                var: "X".to_string(),
                body: Box::new(Term::Var("X".to_string())),
            }),
            path: Box::new(forward_proof.clone()),
            term: Box::new(x.clone()),
        },
        move |y| Term::Transport {
            family: Box::new(Term::Lambda {
                var: "X".to_string(),
                body: Box::new(Term::Var("X".to_string())),
            }),
            path: Box::new(backward_proof.clone()),
            term: Box::new(y.clone()),
        },
        |x| Term::Refl(Box::new(x.clone())),
        |y| Term::Refl(Box::new(y.clone())),
    )
}

/// Convert a type to a term (for universe polymorphism)
fn type_to_term(ty: &Type) -> Term {
    match ty {
        Type::Unit => Term::Annot {
            term: Box::new(Term::Star),
            ty: Box::new(Type::Universe(0)),
        },
        Type::Empty => Term::Annot {
            term: Box::new(Term::Var("Empty".to_string())),
            ty: Box::new(Type::Universe(0)),
        },
        Type::Bool => Term::Annot {
            term: Box::new(Term::Var("Bool".to_string())),
            ty: Box::new(Type::Universe(0)),
        },
        Type::Nat => Term::Annot {
            term: Box::new(Term::Var("Nat".to_string())),
            ty: Box::new(Type::Universe(0)),
        },
        Type::Universe(n) => Term::Annot {
            term: Box::new(Term::Var(format!("Type_{}", n))),
            ty: Box::new(Type::Universe(n + 1)),
        },
        Type::Var(name) => Term::Var(name.clone()),
        _ => Term::Var(format!("{:?}", ty)),
    }
}

/// Standard equivalences

/// Bool ~ Bool via negation
pub fn bool_negation_equiv() -> Equivalence {
    Equivalence::new(
        Type::Bool,
        Type::Bool,
        |x| match x {
            Term::True => Term::False,
            Term::False => Term::True,
            _ => Term::If {
                cond: Box::new(x.clone()),
                then_branch: Box::new(Term::False),
                else_branch: Box::new(Term::True),
            },
        },
        |x| match x {
            Term::True => Term::False,
            Term::False => Term::True,
            _ => Term::If {
                cond: Box::new(x.clone()),
                then_branch: Box::new(Term::False),
                else_branch: Box::new(Term::True),
            },
        },
        |x| Term::Refl(Box::new(x.clone())),
        |x| Term::Refl(Box::new(x.clone())),
    )
}

/// A x B ~ B x A (product commutativity)
pub fn product_comm_equiv(a: Type, b: Type) -> Equivalence {
    Equivalence::new(
        Type::product(a.clone(), b.clone()),
        Type::product(b.clone(), a.clone()),
        |p| Term::Pair {
            fst: Box::new(Term::Snd(Box::new(p.clone()))),
            snd: Box::new(Term::Fst(Box::new(p.clone()))),
        },
        |p| Term::Pair {
            fst: Box::new(Term::Snd(Box::new(p.clone()))),
            snd: Box::new(Term::Fst(Box::new(p.clone()))),
        },
        |p| Term::Refl(Box::new(p.clone())),
        |p| Term::Refl(Box::new(p.clone())),
    )
}

/// A + B ~ B + A (coproduct commutativity)
pub fn coprod_comm_equiv(a: Type, b: Type) -> Equivalence {
    Equivalence::new(
        Type::Coprod(Box::new(a.clone()), Box::new(b.clone())),
        Type::Coprod(Box::new(b.clone()), Box::new(a.clone())),
        |x| Term::Case {
            scrutinee: Box::new(x.clone()),
            left_case: Box::new(Term::Lambda {
                var: "l".to_string(),
                body: Box::new(Term::Inr(Box::new(Term::Var("l".to_string())))),
            }),
            right_case: Box::new(Term::Lambda {
                var: "r".to_string(),
                body: Box::new(Term::Inl(Box::new(Term::Var("r".to_string())))),
            }),
        },
        |x| Term::Case {
            scrutinee: Box::new(x.clone()),
            left_case: Box::new(Term::Lambda {
                var: "l".to_string(),
                body: Box::new(Term::Inr(Box::new(Term::Var("l".to_string())))),
            }),
            right_case: Box::new(Term::Lambda {
                var: "r".to_string(),
                body: Box::new(Term::Inl(Box::new(Term::Var("r".to_string())))),
            }),
        },
        |x| Term::Refl(Box::new(x.clone())),
        |x| Term::Refl(Box::new(x.clone())),
    )
}

/// (A x B) -> C ~ A -> (B -> C) (currying)
pub fn curry_equiv(a: Type, b: Type, c: Type) -> Equivalence {
    Equivalence::new(
        Type::arrow(Type::product(a.clone(), b.clone()), c.clone()),
        Type::arrow(a.clone(), Type::arrow(b.clone(), c.clone())),
        |f| Term::Lambda {
            var: "a".to_string(),
            body: Box::new(Term::Lambda {
                var: "b".to_string(),
                body: Box::new(Term::App {
                    func: Box::new(f.clone()),
                    arg: Box::new(Term::Pair {
                        fst: Box::new(Term::Var("a".to_string())),
                        snd: Box::new(Term::Var("b".to_string())),
                    }),
                }),
            }),
        },
        |f| Term::Lambda {
            var: "p".to_string(),
            body: Box::new(Term::App {
                func: Box::new(Term::App {
                    func: Box::new(f.clone()),
                    arg: Box::new(Term::Fst(Box::new(Term::Var("p".to_string())))),
                }),
                arg: Box::new(Term::Snd(Box::new(Term::Var("p".to_string())))),
            }),
        },
        |f| Term::Refl(Box::new(f.clone())),
        |f| Term::Refl(Box::new(f.clone())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_equivalence() {
        let equiv = Equivalence::identity(Type::Nat);
        let x = Term::nat(42);

        assert!(equiv.apply(&x).structural_eq(&x));
        assert!(equiv.unapply(&x).structural_eq(&x));
    }

    #[test]
    fn test_bool_negation_is_involution() {
        let equiv = bool_negation_equiv();

        // not(not(true)) = true
        let t = Term::True;
        let result = equiv.apply(&equiv.apply(&t));
        assert!(result.structural_eq(&t));

        // not(not(false)) = false
        let f = Term::False;
        let result = equiv.apply(&equiv.apply(&f));
        assert!(result.structural_eq(&f));
    }

    #[test]
    fn test_equivalence_inverse() {
        let equiv = bool_negation_equiv();
        let inv = equiv.inverse();

        // Inverse should have swapped domain/codomain
        assert!(inv.domain.structural_eq(&equiv.codomain));
        assert!(inv.codomain.structural_eq(&equiv.domain));
    }

    #[test]
    fn test_univalence_creates_path() {
        let equiv = Equivalence::identity(Type::Bool);
        let path = univalence(equiv);

        // Path should go from Bool to Bool
        assert!(path.source().structural_eq(path.target()));
    }

    #[test]
    fn test_curry_uncurry() {
        let equiv = curry_equiv(Type::Nat, Type::Nat, Type::Bool);

        // The equivalence should exist
        assert!(equiv.domain.structural_eq(&Type::arrow(
            Type::product(Type::Nat, Type::Nat),
            Type::Bool
        )));
    }
}
