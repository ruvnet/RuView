//! Type definitions for HoTT
//!
//! Types in HoTT are interpreted as spaces (homotopy types):
//! - Unit type: contractible space (one point)
//! - Empty type: empty space (no points)
//! - Sum types: disjoint union of spaces
//! - Product types: cartesian product of spaces
//! - Pi-types: space of sections of a fibration
//! - Sigma-types: total space of a fibration
//! - Identity types: path space

use std::fmt;
use std::sync::Arc;
use super::{Level, Term};

/// Type error variants
#[derive(Debug, Clone, PartialEq)]
pub enum TypeError {
    /// Variable not found in context
    UnboundVariable(String),
    /// Type mismatch during checking
    TypeMismatch { expected: String, found: String },
    /// Not a function type (for application)
    NotAFunction(String),
    /// Not a pair type (for projections)
    NotAPair(String),
    /// Invalid path composition (endpoints don't match)
    PathMismatch { left_target: String, right_source: String },
    /// Universe level violation
    UniverseLevel { expected: Level, found: Level },
    /// Cannot infer type
    CannotInfer(String),
    /// Invalid transport (family doesn't depend on type)
    InvalidTransport(String),
    /// J-eliminator applied incorrectly
    InvalidPathInduction(String),
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeError::UnboundVariable(v) => write!(f, "Unbound variable: {}", v),
            TypeError::TypeMismatch { expected, found } => {
                write!(f, "Type mismatch: expected {}, found {}", expected, found)
            }
            TypeError::NotAFunction(t) => write!(f, "Not a function type: {}", t),
            TypeError::NotAPair(t) => write!(f, "Not a pair type: {}", t),
            TypeError::PathMismatch { left_target, right_source } => {
                write!(f, "Path composition mismatch: {} != {}", left_target, right_source)
            }
            TypeError::UniverseLevel { expected, found } => {
                write!(f, "Universe level error: expected U_{}, found U_{}", expected, found)
            }
            TypeError::CannotInfer(t) => write!(f, "Cannot infer type: {}", t),
            TypeError::InvalidTransport(msg) => write!(f, "Invalid transport: {}", msg),
            TypeError::InvalidPathInduction(msg) => write!(f, "Invalid path induction: {}", msg),
        }
    }
}

impl std::error::Error for TypeError {}

/// Universe type with predicative hierarchy
#[derive(Debug, Clone, PartialEq)]
pub struct Universe {
    /// Universe level (Type_0, Type_1, etc.)
    pub level: Level,
}

impl Universe {
    pub fn new(level: Level) -> Self {
        Universe { level }
    }

    /// Get the universe containing this universe
    pub fn lift(&self) -> Self {
        Universe { level: self.level + 1 }
    }

    /// Check if self can be contained in other
    pub fn fits_in(&self, other: &Universe) -> bool {
        self.level < other.level
    }
}

/// Dependent function type family
/// For Pi(A, B), B is a function from terms of A to types
pub type TypeFamily = Arc<dyn Fn(&Term) -> Type + Send + Sync>;

/// Types in HoTT (spaces in the homotopical interpretation)
#[derive(Clone)]
pub enum Type {
    /// Unit type (contractible space with one point)
    Unit,

    /// Empty type (empty space, no inhabitants)
    Empty,

    /// Boolean type (discrete space with two points)
    Bool,

    /// Natural numbers (discrete countable space)
    Nat,

    /// Universe of types at a given level
    Universe(Level),

    /// Dependent function type (Pi-type)
    /// Pi(A, B) where B : A -> Type
    /// Represents the space of sections of the fibration B over A
    Pi {
        domain: Box<Type>,
        codomain: TypeFamily,
        /// Variable name for pretty printing
        var_name: String,
    },

    /// Dependent pair type (Sigma-type)
    /// Sigma(A, B) where B : A -> Type
    /// Represents the total space of the fibration B over A
    Sigma {
        base: Box<Type>,
        fiber: TypeFamily,
        /// Variable name for pretty printing
        var_name: String,
    },

    /// Identity type (path type)
    /// Id(A, a, b) represents the space of paths from a to b in A
    /// This is the central type of HoTT
    Id(Box<Type>, Box<Term>, Box<Term>),

    /// Coproduct/Sum type
    Coprod(Box<Type>, Box<Type>),

    /// Non-dependent function type (special case of Pi)
    Arrow(Box<Type>, Box<Type>),

    /// Non-dependent product type (special case of Sigma)
    Product(Box<Type>, Box<Type>),

    /// Type variable (for polymorphism)
    Var(String),

    /// Circle type (S^1) - fundamental example in HoTT
    /// Has a base point and a loop
    Circle,

    /// Interval type I with endpoints 0 and 1
    Interval,

    /// Truncation type ||A||_n
    /// n-truncation of a type (sets are 0-truncated, props are (-1)-truncated)
    Truncation {
        inner: Box<Type>,
        level: i32, // -1 for prop, 0 for set, 1 for groupoid, etc.
    },
}

impl Type {
    /// Create a non-dependent function type A -> B
    pub fn arrow(domain: Type, codomain: Type) -> Self {
        Type::Arrow(Box::new(domain), Box::new(codomain))
    }

    /// Create a non-dependent product type A x B
    pub fn product(left: Type, right: Type) -> Self {
        Type::Product(Box::new(left), Box::new(right))
    }

    /// Create a dependent function type (x : A) -> B(x)
    pub fn pi<F>(var_name: &str, domain: Type, codomain: F) -> Self
    where
        F: Fn(&Term) -> Type + Send + Sync + 'static,
    {
        Type::Pi {
            domain: Box::new(domain),
            codomain: Arc::new(codomain),
            var_name: var_name.to_string(),
        }
    }

    /// Create a dependent pair type (x : A) * B(x)
    pub fn sigma<F>(var_name: &str, base: Type, fiber: F) -> Self
    where
        F: Fn(&Term) -> Type + Send + Sync + 'static,
    {
        Type::Sigma {
            base: Box::new(base),
            fiber: Arc::new(fiber),
            var_name: var_name.to_string(),
        }
    }

    /// Create an identity type a =_A b
    pub fn id(ty: Type, left: Term, right: Term) -> Self {
        Type::Id(Box::new(ty), Box::new(left), Box::new(right))
    }

    /// Get the universe level of this type
    pub fn universe_level(&self) -> Level {
        match self {
            Type::Unit | Type::Empty | Type::Bool | Type::Nat | Type::Circle | Type::Interval => 0,
            Type::Universe(n) => n + 1,
            Type::Pi { domain, .. } | Type::Arrow(domain, _) => {
                // Level is max of domain and codomain levels
                // For simplicity, we compute based on domain
                domain.universe_level()
            }
            Type::Sigma { base, .. } | Type::Product(base, _) => base.universe_level(),
            Type::Id(ty, _, _) => ty.universe_level(),
            Type::Coprod(left, right) => std::cmp::max(left.universe_level(), right.universe_level()),
            Type::Var(_) => 0, // Variables are at level 0 by default
            Type::Truncation { inner, .. } => inner.universe_level(),
        }
    }

    /// Check if this type is a proposition (all proofs are equal)
    pub fn is_prop(&self) -> bool {
        matches!(self, Type::Truncation { level: -1, .. }) || matches!(self, Type::Empty)
    }

    /// Check if this type is a set (all identity proofs are equal)
    pub fn is_set(&self) -> bool {
        matches!(self,
            Type::Nat | Type::Bool | Type::Unit |
            Type::Truncation { level: 0, .. }
        )
    }

    /// Check structural equality (not definitional equality)
    pub fn structural_eq(&self, other: &Type) -> bool {
        match (self, other) {
            (Type::Unit, Type::Unit) => true,
            (Type::Empty, Type::Empty) => true,
            (Type::Bool, Type::Bool) => true,
            (Type::Nat, Type::Nat) => true,
            (Type::Circle, Type::Circle) => true,
            (Type::Interval, Type::Interval) => true,
            (Type::Universe(a), Type::Universe(b)) => a == b,
            (Type::Arrow(a1, b1), Type::Arrow(a2, b2)) => {
                a1.structural_eq(a2) && b1.structural_eq(b2)
            }
            (Type::Product(a1, b1), Type::Product(a2, b2)) => {
                a1.structural_eq(a2) && b1.structural_eq(b2)
            }
            (Type::Coprod(a1, b1), Type::Coprod(a2, b2)) => {
                a1.structural_eq(a2) && b1.structural_eq(b2)
            }
            (Type::Var(a), Type::Var(b)) => a == b,
            (Type::Id(t1, a1, b1), Type::Id(t2, a2, b2)) => {
                t1.structural_eq(t2) && a1.structural_eq(a2) && b1.structural_eq(b2)
            }
            (Type::Truncation { inner: i1, level: l1 }, Type::Truncation { inner: i2, level: l2 }) => {
                l1 == l2 && i1.structural_eq(i2)
            }
            // Pi and Sigma require more careful comparison
            (Type::Pi { domain: d1, var_name: v1, .. }, Type::Pi { domain: d2, var_name: v2, .. }) => {
                d1.structural_eq(d2) && v1 == v2
            }
            (Type::Sigma { base: b1, var_name: v1, .. }, Type::Sigma { base: b2, var_name: v2, .. }) => {
                b1.structural_eq(b2) && v1 == v2
            }
            _ => false,
        }
    }
}

impl fmt::Debug for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Unit => write!(f, "Unit"),
            Type::Empty => write!(f, "Empty"),
            Type::Bool => write!(f, "Bool"),
            Type::Nat => write!(f, "Nat"),
            Type::Circle => write!(f, "S1"),
            Type::Interval => write!(f, "I"),
            Type::Universe(n) => write!(f, "Type_{}", n),
            Type::Arrow(a, b) => write!(f, "({:?} -> {:?})", a, b),
            Type::Product(a, b) => write!(f, "({:?} x {:?})", a, b),
            Type::Coprod(a, b) => write!(f, "({:?} + {:?})", a, b),
            Type::Var(name) => write!(f, "{}", name),
            Type::Pi { domain, var_name, .. } => {
                write!(f, "(({} : {:?}) -> ...)", var_name, domain)
            }
            Type::Sigma { base, var_name, .. } => {
                write!(f, "(({} : {:?}) * ...)", var_name, base)
            }
            Type::Id(ty, a, b) => write!(f, "({:?} =_{:?} {:?})", a, ty, b),
            Type::Truncation { inner, level } => {
                write!(f, "||{:?}||_{}", inner, level)
            }
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_universe_levels() {
        let u0 = Universe::new(0);
        let u1 = Universe::new(1);

        assert!(u0.fits_in(&u1));
        assert!(!u1.fits_in(&u0));
        assert!(!u0.fits_in(&u0));

        let u0_lifted = u0.lift();
        assert_eq!(u0_lifted.level, 1);
    }

    #[test]
    fn test_type_structural_eq() {
        assert!(Type::Nat.structural_eq(&Type::Nat));
        assert!(!Type::Nat.structural_eq(&Type::Bool));

        let arrow1 = Type::arrow(Type::Nat, Type::Bool);
        let arrow2 = Type::arrow(Type::Nat, Type::Bool);
        assert!(arrow1.structural_eq(&arrow2));
    }

    #[test]
    fn test_pi_type_creation() {
        let pi = Type::pi("x", Type::Nat, |_| Type::Bool);
        assert!(matches!(pi, Type::Pi { .. }));
    }
}
