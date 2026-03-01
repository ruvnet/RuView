//! Path types and operations in HoTT
//!
//! Paths are the fundamental concept in HoTT, representing:
//! - Equality proofs in the logical interpretation
//! - Continuous paths in the topological interpretation
//! - Morphisms in the categorical interpretation
//!
//! Key operations:
//! - Reflexivity: every point has a trivial path to itself
//! - Symmetry: paths can be reversed
//! - Transitivity: paths can be composed
//! - Functoriality: functions preserve paths (ap)
//! - Transport: paths allow moving between fibers

use std::fmt;
use super::{Term, Type};

/// A path between two terms in a type
///
/// In HoTT, a path p : a = b represents:
/// - A proof that a equals b
/// - A continuous path from a to b in the space
/// - A morphism from a to b in the groupoid
#[derive(Clone)]
pub struct Path {
    /// Source point of the path
    source: Term,
    /// Target point of the path
    target: Term,
    /// The proof term (witness of equality)
    proof: Term,
    /// The type this path lives in (optional, for type checking)
    ambient_type: Option<Type>,
}

impl Path {
    /// Create a new path from source to target with given proof
    pub fn new(source: Term, target: Term, proof: Term) -> Self {
        Path {
            source,
            target,
            proof,
            ambient_type: None,
        }
    }

    /// Create a path with explicit ambient type
    pub fn with_type(source: Term, target: Term, proof: Term, ty: Type) -> Self {
        Path {
            source,
            target,
            proof,
            ambient_type: Some(ty),
        }
    }

    /// Create reflexivity path: refl_a : a = a
    pub fn refl(point: Term) -> Self {
        Path {
            source: point.clone(),
            target: point.clone(),
            proof: Term::Refl(Box::new(point)),
            ambient_type: None,
        }
    }

    /// Get the source of the path
    pub fn source(&self) -> &Term {
        &self.source
    }

    /// Get the target of the path
    pub fn target(&self) -> &Term {
        &self.target
    }

    /// Get the proof term
    pub fn proof(&self) -> &Term {
        &self.proof
    }

    /// Get the ambient type
    pub fn ambient_type(&self) -> Option<&Type> {
        self.ambient_type.as_ref()
    }

    /// Get the identity type this path inhabits
    pub fn path_type(&self) -> Option<Type> {
        self.ambient_type.as_ref().map(|ty| {
            Type::Id(Box::new(ty.clone()), Box::new(self.source.clone()), Box::new(self.target.clone()))
        })
    }
}

/// Operations on paths (groupoid structure)
pub trait PathOps: Sized {
    /// Compose two paths (transitivity): p . q : a = c when p : a = b and q : b = c
    fn compose(&self, other: &Self) -> Option<Self>;

    /// Invert a path (symmetry): p^(-1) : b = a when p : a = b
    fn inverse(&self) -> Self;

    /// Check if path endpoints match for composition
    fn composable(&self, other: &Self) -> bool;

    /// Check if this is a reflexivity path
    fn is_refl(&self) -> bool;

    /// Apply a function to a path (functoriality)
    fn ap(&self, func: &Term) -> Self;

    /// Whiskering: compose path with reflexivity on left
    fn whisker_left(&self, point: &Term) -> Self;

    /// Whiskering: compose path with reflexivity on right
    fn whisker_right(&self, point: &Term) -> Self;
}

impl PathOps for Path {
    fn compose(&self, other: &Path) -> Option<Path> {
        // Check that endpoints match
        if !self.target.structural_eq(&other.source) {
            return None;
        }

        Some(Path {
            source: self.source.clone(),
            target: other.target.clone(),
            proof: Term::PathCompose {
                left: Box::new(self.proof.clone()),
                right: Box::new(other.proof.clone()),
            },
            ambient_type: self.ambient_type.clone(),
        })
    }

    fn inverse(&self) -> Path {
        Path {
            source: self.target.clone(),
            target: self.source.clone(),
            proof: Term::PathInverse(Box::new(self.proof.clone())),
            ambient_type: self.ambient_type.clone(),
        }
    }

    fn composable(&self, other: &Path) -> bool {
        self.target.structural_eq(&other.source)
    }

    fn is_refl(&self) -> bool {
        self.source.structural_eq(&self.target) &&
        matches!(&self.proof, Term::Refl(_))
    }

    fn ap(&self, func: &Term) -> Path {
        Path {
            source: Term::App {
                func: Box::new(func.clone()),
                arg: Box::new(self.source.clone()),
            },
            target: Term::App {
                func: Box::new(func.clone()),
                arg: Box::new(self.target.clone()),
            },
            proof: Term::Ap {
                func: Box::new(func.clone()),
                path: Box::new(self.proof.clone()),
            },
            ambient_type: None, // Type changes under function application
        }
    }

    fn whisker_left(&self, point: &Term) -> Path {
        let refl_path = Path::refl(point.clone());
        // This should always succeed since refl composes with anything
        refl_path.compose(self).unwrap_or_else(|| self.clone())
    }

    fn whisker_right(&self, point: &Term) -> Path {
        let refl_path = Path::refl(point.clone());
        self.compose(&refl_path).unwrap_or_else(|| self.clone())
    }
}

/// Higher paths (paths between paths)
/// These represent homotopies in the topological interpretation
#[derive(Clone)]
pub struct Path2 {
    /// Source path
    pub source: Path,
    /// Target path
    pub target: Path,
    /// The 2-dimensional proof term
    pub proof: Term,
}

impl Path2 {
    /// Create a 2-path from source to target
    pub fn new(source: Path, target: Path, proof: Term) -> Self {
        Path2 { source, target, proof }
    }

    /// Reflexivity 2-path (trivial homotopy)
    pub fn refl(path: Path) -> Self {
        Path2 {
            source: path.clone(),
            target: path.clone(),
            proof: Term::Refl(Box::new(path.proof)),
        }
    }

    /// Vertical composition of 2-paths
    pub fn vcompose(&self, other: &Path2) -> Option<Path2> {
        if !path_eq(&self.target, &other.source) {
            return None;
        }

        Some(Path2 {
            source: self.source.clone(),
            target: other.target.clone(),
            proof: Term::PathCompose {
                left: Box::new(self.proof.clone()),
                right: Box::new(other.proof.clone()),
            },
        })
    }

    /// Horizontal composition of 2-paths
    pub fn hcompose(&self, other: &Path2) -> Option<Path2> {
        // Requires compatible 1-paths
        let new_source = self.source.compose(&other.source)?;
        let new_target = self.target.compose(&other.target)?;

        Some(Path2 {
            source: new_source,
            target: new_target,
            proof: Term::Pair {
                fst: Box::new(self.proof.clone()),
                snd: Box::new(other.proof.clone()),
            },
        })
    }

    /// Inverse 2-path
    pub fn inverse(&self) -> Path2 {
        Path2 {
            source: self.target.clone(),
            target: self.source.clone(),
            proof: Term::PathInverse(Box::new(self.proof.clone())),
        }
    }
}

/// Check if two paths are equal (as 1-cells)
fn path_eq(p: &Path, q: &Path) -> bool {
    p.source.structural_eq(&q.source) &&
    p.target.structural_eq(&q.target) &&
    p.proof.structural_eq(&q.proof)
}

/// Path algebra laws (as paths between paths)
pub struct PathLaws;

impl PathLaws {
    /// Left unit: refl . p = p
    pub fn left_unit(p: &Path) -> Path2 {
        let refl_source = Path::refl(p.source.clone());
        let composed = refl_source.compose(p).unwrap();
        Path2::new(composed, p.clone(), Term::Refl(Box::new(p.proof.clone())))
    }

    /// Right unit: p . refl = p
    pub fn right_unit(p: &Path) -> Path2 {
        let refl_target = Path::refl(p.target.clone());
        let composed = p.compose(&refl_target).unwrap();
        Path2::new(composed, p.clone(), Term::Refl(Box::new(p.proof.clone())))
    }

    /// Left inverse: p^(-1) . p = refl
    pub fn left_inverse(p: &Path) -> Path2 {
        let inv = p.inverse();
        let composed = inv.compose(p).unwrap();
        let refl = Path::refl(p.target.clone());
        Path2::new(composed, refl, Term::Refl(Box::new(p.proof.clone())))
    }

    /// Right inverse: p . p^(-1) = refl
    pub fn right_inverse(p: &Path) -> Path2 {
        let inv = p.inverse();
        let composed = p.compose(&inv).unwrap();
        let refl = Path::refl(p.source.clone());
        Path2::new(composed, refl, Term::Refl(Box::new(p.proof.clone())))
    }

    /// Associativity: (p . q) . r = p . (q . r)
    pub fn assoc(p: &Path, q: &Path, r: &Path) -> Option<Path2> {
        let pq = p.compose(q)?;
        let qr = q.compose(r)?;
        let left = pq.compose(r)?;
        let right = p.compose(&qr)?;

        Some(Path2::new(left, right, Term::Refl(Box::new(p.proof.clone()))))
    }

    /// ap preserves composition: ap f (p . q) = ap f p . ap f q
    pub fn ap_compose(f: &Term, p: &Path, q: &Path) -> Option<Path2> {
        let pq = p.compose(q)?;
        let left = pq.ap(f);

        let ap_p = p.ap(f);
        let ap_q = q.ap(f);
        let right = ap_p.compose(&ap_q)?;

        Some(Path2::new(left, right, Term::Refl(Box::new(f.clone()))))
    }

    /// ap preserves identity: ap f refl = refl
    pub fn ap_refl(f: &Term, a: &Term) -> Path2 {
        let refl_a = Path::refl(a.clone());
        let ap_refl = refl_a.ap(f);
        let fa = Term::App {
            func: Box::new(f.clone()),
            arg: Box::new(a.clone()),
        };
        let refl_fa = Path::refl(fa);

        Path2::new(ap_refl, refl_fa, Term::Refl(Box::new(f.clone())))
    }
}

/// Dependent path in a type family
/// For P : A -> Type, a dependent path over p : a = b is
/// a term of type transport P p (d a) = d b
#[derive(Clone)]
pub struct DepPath {
    /// The base path
    pub base: Path,
    /// The type family
    pub family: Term,
    /// Source point in the fiber over base.source
    pub source_fiber: Term,
    /// Target point in the fiber over base.target
    pub target_fiber: Term,
    /// The dependent path proof
    pub proof: Term,
}

impl DepPath {
    pub fn new(
        base: Path,
        family: Term,
        source_fiber: Term,
        target_fiber: Term,
        proof: Term,
    ) -> Self {
        DepPath {
            base,
            family,
            source_fiber,
            target_fiber,
            proof,
        }
    }

    /// Dependent reflexivity
    pub fn refl(point: Term, family: Term, fiber_point: Term) -> Self {
        DepPath {
            base: Path::refl(point),
            family,
            source_fiber: fiber_point.clone(),
            target_fiber: fiber_point.clone(),
            proof: Term::Refl(Box::new(fiber_point)),
        }
    }
}

impl fmt::Debug for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Path({:?} ={:?}= {:?})", self.source, self.proof, self.target)
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl fmt::Debug for Path2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Path2({:?} =[{:?}]=> {:?})", self.source, self.proof, self.target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_creation() {
        let a = Term::Var("a".to_string());
        let b = Term::Var("b".to_string());
        let p = Term::Var("p".to_string());

        let path = Path::new(a.clone(), b.clone(), p);
        assert_eq!(path.source(), &a);
        assert_eq!(path.target(), &b);
    }

    #[test]
    fn test_reflexivity() {
        let a = Term::Var("a".to_string());
        let refl = Path::refl(a.clone());

        assert!(refl.is_refl());
        assert_eq!(refl.source(), refl.target());
    }

    #[test]
    fn test_path_inverse() {
        let a = Term::Var("a".to_string());
        let b = Term::Var("b".to_string());
        let p = Path::new(a.clone(), b.clone(), Term::Var("p".to_string()));

        let inv = p.inverse();
        assert_eq!(inv.source(), &b);
        assert_eq!(inv.target(), &a);
    }

    #[test]
    fn test_path_composition() {
        let a = Term::Var("a".to_string());
        let b = Term::Var("b".to_string());
        let c = Term::Var("c".to_string());

        let p = Path::new(a.clone(), b.clone(), Term::Var("p".to_string()));
        let q = Path::new(b.clone(), c.clone(), Term::Var("q".to_string()));

        let composed = p.compose(&q);
        assert!(composed.is_some());

        let composed = composed.unwrap();
        assert_eq!(composed.source(), &a);
        assert_eq!(composed.target(), &c);
    }

    #[test]
    fn test_composition_fails_on_mismatch() {
        let a = Term::Var("a".to_string());
        let b = Term::Var("b".to_string());
        let c = Term::Var("c".to_string());

        let p = Path::new(a.clone(), b.clone(), Term::Var("p".to_string()));
        let q = Path::new(c.clone(), a.clone(), Term::Var("q".to_string()));

        assert!(p.compose(&q).is_none());
    }

    #[test]
    fn test_ap_functoriality() {
        let a = Term::Var("a".to_string());
        let b = Term::Var("b".to_string());
        let f = Term::Var("f".to_string());

        let p = Path::new(a.clone(), b.clone(), Term::Var("p".to_string()));
        let ap_p = p.ap(&f);

        // ap f p : f(a) = f(b)
        assert!(matches!(ap_p.source(), Term::App { .. }));
        assert!(matches!(ap_p.target(), Term::App { .. }));
    }
}
