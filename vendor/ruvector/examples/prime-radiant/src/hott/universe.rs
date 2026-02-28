//! Type Universe implementation for HoTT
//!
//! The type universe hierarchy Type_0 : Type_1 : Type_2 : ...
//! provides a foundation for type-theoretic reasoning.

use super::{Type, Term, TypeError};
use std::collections::HashMap;

/// A type universe at a specific level
#[derive(Debug, Clone)]
pub struct TypeUniverse {
    /// Universe level (0, 1, 2, ...)
    level: usize,
    /// Types defined in this universe
    types: HashMap<String, Type>,
    /// Type aliases
    aliases: HashMap<String, Type>,
}

impl TypeUniverse {
    /// Create a new universe at the given level
    pub fn new(level: usize) -> Self {
        Self {
            level,
            types: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    /// Get the universe level
    pub fn level(&self) -> usize {
        self.level
    }

    /// Get the type of this universe (lives in the next universe)
    pub fn universe_type(&self) -> Type {
        Type::Universe(self.level + 1)
    }

    /// Define a new type in this universe
    pub fn define_type(&mut self, name: impl Into<String>, ty: Type) -> Result<(), TypeError> {
        let name = name.into();

        // Check that the type lives in this universe
        let ty_level = ty.universe_level();
        if ty_level > self.level {
            return Err(TypeError::UniverseViolation {
                expected: self.level,
                found: ty_level,
            });
        }

        self.types.insert(name, ty);
        Ok(())
    }

    /// Get a type by name
    pub fn get_type(&self, name: &str) -> Option<&Type> {
        self.types.get(name).or_else(|| self.aliases.get(name))
    }

    /// Add a type alias
    pub fn add_alias(&mut self, alias: impl Into<String>, ty: Type) {
        self.aliases.insert(alias.into(), ty);
    }

    /// Check if a type lives in this universe
    pub fn contains(&self, ty: &Type) -> bool {
        ty.universe_level() <= self.level
    }

    /// Lift a type to the next universe
    pub fn lift(&self, ty: &Type) -> Type {
        // In HoTT, types can be lifted to higher universes
        ty.clone()
    }

    /// Get all defined types
    pub fn types(&self) -> impl Iterator<Item = (&String, &Type)> {
        self.types.iter()
    }

    /// Create the base universe (Type_0) with standard types
    pub fn base() -> Self {
        let mut universe = Self::new(0);

        // Define standard types
        universe.types.insert("Unit".to_string(), Type::Unit);
        universe.types.insert("Empty".to_string(), Type::Empty);
        universe.types.insert("Bool".to_string(), Type::Bool);
        universe.types.insert("Nat".to_string(), Type::Nat);

        universe
    }
}

/// A cumulative universe hierarchy
#[derive(Debug, Clone)]
pub struct UniverseHierarchy {
    /// Universes indexed by level
    universes: Vec<TypeUniverse>,
}

impl UniverseHierarchy {
    /// Create a new hierarchy with a maximum level
    pub fn new(max_level: usize) -> Self {
        let universes = (0..=max_level)
            .map(TypeUniverse::new)
            .collect();
        Self { universes }
    }

    /// Get a universe at a specific level
    pub fn universe(&self, level: usize) -> Option<&TypeUniverse> {
        self.universes.get(level)
    }

    /// Get a mutable universe at a specific level
    pub fn universe_mut(&mut self, level: usize) -> Option<&mut TypeUniverse> {
        self.universes.get_mut(level)
    }

    /// Find the smallest universe containing a type
    pub fn smallest_universe(&self, ty: &Type) -> usize {
        ty.universe_level()
    }

    /// Check cumulativity: Type_i : Type_{i+1}
    pub fn is_cumulative(&self) -> bool {
        // By construction, our hierarchy is cumulative
        true
    }
}

impl Default for UniverseHierarchy {
    fn default() -> Self {
        Self::new(10) // Default to 10 universe levels
    }
}

/// Universe polymorphism support
#[derive(Debug, Clone)]
pub struct UniversePolymorphic<T> {
    /// The polymorphic value
    value: T,
    /// Level constraints (lower bounds)
    constraints: Vec<usize>,
}

impl<T> UniversePolymorphic<T> {
    /// Create a new universe-polymorphic value
    pub fn new(value: T) -> Self {
        Self {
            value,
            constraints: Vec::new(),
        }
    }

    /// Add a level constraint
    pub fn with_constraint(mut self, level: usize) -> Self {
        self.constraints.push(level);
        self
    }

    /// Get the value
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Get the minimum required level
    pub fn min_level(&self) -> usize {
        self.constraints.iter().copied().max().unwrap_or(0)
    }

    /// Instantiate at a specific level
    pub fn instantiate(&self, level: usize) -> Option<&T> {
        if level >= self.min_level() {
            Some(&self.value)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_universe_creation() {
        let u = TypeUniverse::new(0);
        assert_eq!(u.level(), 0);
    }

    #[test]
    fn test_base_universe() {
        let base = TypeUniverse::base();
        assert!(base.get_type("Bool").is_some());
        assert!(base.get_type("Nat").is_some());
    }

    #[test]
    fn test_universe_contains() {
        let u0 = TypeUniverse::new(0);
        assert!(u0.contains(&Type::Bool));
        assert!(u0.contains(&Type::Nat));
    }

    #[test]
    fn test_hierarchy() {
        let hierarchy = UniverseHierarchy::new(3);
        assert!(hierarchy.universe(0).is_some());
        assert!(hierarchy.universe(3).is_some());
        assert!(hierarchy.universe(4).is_none());
    }
}
