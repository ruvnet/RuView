//! Transport and Path Induction in HoTT
//!
//! Transport is the fundamental operation that moves terms along paths.
//! Given P : A -> Type, p : a = b, and x : P(a), transport gives us
//! transport_P(p, x) : P(b).
//!
//! Path induction (J-eliminator) is the elimination principle for
//! identity types, expressing that to prove something about all paths,
//! it suffices to prove it for reflexivity.

use super::{Term, Type, Path, PathOps, TypeError};

/// Transport a term along a path in a type family
///
/// Given:
/// - family: P : A -> Type (a type family over A)
/// - path: p : a = b (a path in A)
/// - term: x : P(a) (a term in the fiber over a)
///
/// Returns: transport_P(p, x) : P(b)
///
/// # Example
///
/// ```rust,ignore
/// // Transport along identity gives identity
/// let refl_a = Path::refl(a);
/// let result = transport(&Type::Nat, &refl_a, &x);
/// // result is definitionally equal to x
/// ```
pub fn transport(family: &Type, path: &Path, term: &Term) -> Term {
    // If the path is reflexivity, transport is identity
    if path.is_refl() {
        return term.clone();
    }

    // Otherwise, construct the transport term
    Term::Transport {
        family: Box::new(type_to_family_term(family)),
        path: Box::new(path.proof().clone()),
        term: Box::new(term.clone()),
    }
}

/// Transport with explicit proof term
pub fn transport_term(family_term: &Term, path_proof: &Term, term: &Term) -> Term {
    Term::Transport {
        family: Box::new(family_term.clone()),
        path: Box::new(path_proof.clone()),
        term: Box::new(term.clone()),
    }
}

/// Path induction (J-eliminator)
///
/// The fundamental elimination principle for identity types.
/// To prove C(a, b, p) for all a, b : A and p : a = b,
/// it suffices to prove C(a, a, refl_a) for all a.
///
/// # Arguments
///
/// * `motive` - C : (a b : A) -> (a = b) -> Type
/// * `base_case` - c : (a : A) -> C(a, a, refl_a)
/// * `path` - The path to eliminate
///
/// # Returns
///
/// A term of type C(path.source, path.target, path)
pub fn path_induction<F>(
    path: &Path,
    base_case: F,
) -> Term
where
    F: Fn(&Term) -> Term,
{
    // If the path is reflexivity, apply the base case directly
    if path.is_refl() {
        return base_case(path.source());
    }

    // Otherwise, construct the J term
    Term::J {
        motive: Box::new(Term::Var("C".to_string())), // placeholder
        base_case: Box::new(Term::Lambda {
            var: "a".to_string(),
            body: Box::new(base_case(&Term::Var("a".to_string()))),
        }),
        left: Box::new(path.source().clone()),
        right: Box::new(path.target().clone()),
        path: Box::new(path.proof().clone()),
    }
}

/// Full J eliminator with explicit motive
pub fn j_elim(
    motive: Term,
    base_case: Term,
    left: Term,
    right: Term,
    path: Term,
) -> Term {
    Term::J {
        motive: Box::new(motive),
        base_case: Box::new(base_case),
        left: Box::new(left),
        right: Box::new(right),
        path: Box::new(path),
    }
}

/// Apply a function to a path (ap)
///
/// Given f : A -> B and p : a = b, produces ap_f(p) : f(a) = f(b)
///
/// This is the functoriality of functions with respect to paths.
pub fn ap(func: &Term, path: &Path) -> Path {
    use super::PathOps;
    path.ap(func)
}

/// Apply a function to a path, returning just the proof term
pub fn ap_term(func: &Term, path_proof: &Term) -> Term {
    Term::Ap {
        func: Box::new(func.clone()),
        path: Box::new(path_proof.clone()),
    }
}

/// Dependent application (apd)
///
/// Given f : (a : A) -> P(a) and p : a = b,
/// produces apd_f(p) : transport_P(p, f(a)) = f(b)
///
/// This is the dependent version of ap.
pub fn apd(func: &Term, path: &Path) -> Term {
    // If path is reflexivity, apd is reflexivity
    if path.is_refl() {
        let fa = Term::App {
            func: Box::new(func.clone()),
            arg: Box::new(path.source().clone()),
        };
        return Term::Refl(Box::new(fa));
    }

    Term::Apd {
        func: Box::new(func.clone()),
        path: Box::new(path.proof().clone()),
    }
}

/// Transport laws and properties
pub struct TransportLaws;

impl TransportLaws {
    /// transport_P(refl, x) = x (computation rule)
    pub fn transport_refl(x: &Term) -> Term {
        x.clone()
    }

    /// transport_P(p . q, x) = transport_P(q, transport_P(p, x))
    pub fn transport_compose(
        family: &Type,
        p: &Path,
        q: &Path,
        x: &Term,
    ) -> Option<(Term, Term)> {
        use super::PathOps;

        let pq = p.compose(q)?;

        let left = transport(family, &pq, x);

        let transported_p = transport(family, p, x);
        let right = transport(family, q, &transported_p);

        Some((left, right))
    }

    /// transport_P(p^(-1), transport_P(p, x)) = x
    pub fn transport_inverse_left(
        family: &Type,
        path: &Path,
        x: &Term,
    ) -> (Term, Term) {
        use super::PathOps;

        let transported = transport(family, path, x);
        let back = transport(family, &path.inverse(), &transported);

        (back, x.clone())
    }

    /// transport_P(p, transport_P(p^(-1), y)) = y
    pub fn transport_inverse_right(
        family: &Type,
        path: &Path,
        y: &Term,
    ) -> (Term, Term) {
        use super::PathOps;

        let transported = transport(family, &path.inverse(), y);
        let back = transport(family, path, &transported);

        (back, y.clone())
    }
}

/// Convert a type to a term representing a type family
fn type_to_family_term(ty: &Type) -> Term {
    // For constant families, the term is just a type annotation
    Term::Annot {
        term: Box::new(Term::Var("_".to_string())),
        ty: Box::new(ty.clone()),
    }
}

/// Based path induction (with fixed endpoint)
///
/// A variant of J where we fix one endpoint and vary the other.
/// This is equivalent to J but sometimes more convenient.
pub fn based_path_induction<F>(
    base_point: &Term,
    motive: impl Fn(&Term, &Path) -> Type,
    base_case: &Term,
    endpoint: &Term,
    path: &Path,
) -> Term {
    // If path is reflexivity, return base case
    if path.is_refl() {
        return base_case.clone();
    }

    // Otherwise, use J
    Term::J {
        motive: Box::new(Term::Lambda {
            var: "b".to_string(),
            body: Box::new(Term::Lambda {
                var: "p".to_string(),
                body: Box::new(Term::Var("_motive_".to_string())), // placeholder
            }),
        }),
        base_case: Box::new(base_case.clone()),
        left: Box::new(base_point.clone()),
        right: Box::new(endpoint.clone()),
        path: Box::new(path.proof().clone()),
    }
}

/// Contractibility: a type A is contractible if there exists a : A
/// such that for all x : A, a = x.
pub struct Contraction {
    /// The center of contraction
    pub center: Term,
    /// For each point, a path to the center
    pub contraction: Box<dyn Fn(&Term) -> Path + Send + Sync>,
}

impl std::fmt::Debug for Contraction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Contraction")
            .field("center", &self.center)
            .finish()
    }
}

impl Contraction {
    /// Create a new contraction
    pub fn new<F>(center: Term, contraction: F) -> Self
    where
        F: Fn(&Term) -> Path + Send + Sync + 'static,
    {
        Contraction {
            center,
            contraction: Box::new(contraction),
        }
    }

    /// Get the contraction path for a point
    pub fn contract(&self, point: &Term) -> Path {
        (self.contraction)(point)
    }
}

/// The unit type is contractible
pub fn unit_contraction() -> Contraction {
    Contraction::new(
        Term::Star,
        |_x| Path::refl(Term::Star),
    )
}

/// Singleton types are contractible
pub fn singleton_contraction(a: Term) -> Contraction {
    let center = a.clone();
    Contraction::new(
        Term::Pair {
            fst: Box::new(a.clone()),
            snd: Box::new(Term::Refl(Box::new(a.clone()))),
        },
        move |p| {
            // For (x, p) : Sigma(A, a = x), contract to (a, refl)
            Path::new(
                p.clone(),
                Term::Pair {
                    fst: Box::new(center.clone()),
                    snd: Box::new(Term::Refl(Box::new(center.clone()))),
                },
                Term::Var("singleton_contraction".to_string()),
            )
        },
    )
}

/// Fiber of a function at a point
#[derive(Clone)]
pub struct Fiber {
    /// The function f : A -> B
    pub func: Term,
    /// The point y : B
    pub point: Term,
    /// The fiber type: Sigma(A, f(x) = y)
    pub fiber_type: Type,
}

impl Fiber {
    /// Create a fiber
    pub fn new(func: Term, point: Term, domain: Type, codomain: Type) -> Self {
        let func_clone = func.clone();
        let point_clone = point.clone();
        let fiber_type = Type::sigma(
            "x",
            domain,
            move |x| Type::Id(
                Box::new(codomain.clone()),
                Box::new(Term::App {
                    func: Box::new(func_clone.clone()),
                    arg: Box::new(x.clone()),
                }),
                Box::new(point_clone.clone()),
            ),
        );

        Fiber {
            func,
            point,
            fiber_type,
        }
    }
}

/// Equivalence via contractible fibers
/// A function is an equivalence iff all its fibers are contractible
pub fn is_equiv_via_fibers(func: &Term, _domain: &Type, _codomain: &Type) -> bool {
    // In a full implementation, we would check that all fibers are contractible
    // For now, return a placeholder
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_refl() {
        let x = Term::nat(42);
        let refl = Path::refl(Term::Var("a".to_string()));

        let result = transport(&Type::Nat, &refl, &x);

        // Transport along refl should return the original term
        assert!(result.structural_eq(&x));
    }

    #[test]
    fn test_ap_refl() {
        let a = Term::Var("a".to_string());
        let f = Term::Var("f".to_string());
        let refl = Path::refl(a.clone());

        let ap_refl = ap(&f, &refl);

        // ap f refl should be a reflexivity path at f(a)
        // The source and target should be f(a)
        assert!(ap_refl.source().structural_eq(ap_refl.target()));
    }

    #[test]
    fn test_j_on_refl() {
        let a = Term::Var("a".to_string());
        let refl = Path::refl(a.clone());

        let result = path_induction(&refl, |x| {
            // Base case: identity function
            x.clone()
        });

        // J on refl should return the base case applied to a
        assert!(result.structural_eq(&a));
    }

    #[test]
    fn test_unit_contractible() {
        let contr = unit_contraction();

        // Center should be star
        assert!(matches!(contr.center, Term::Star));

        // Contraction of star should be refl
        let path = contr.contract(&Term::Star);
        assert!(path.is_refl());
    }

    #[test]
    fn test_apd_on_refl() {
        let a = Term::Var("a".to_string());
        let f = Term::Var("f".to_string());
        let refl = Path::refl(a.clone());

        let result = apd(&f, &refl);

        // apd f refl should be refl(f(a))
        assert!(matches!(result, Term::Refl(_)));
    }
}
