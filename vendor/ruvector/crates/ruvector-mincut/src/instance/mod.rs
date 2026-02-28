//! Instance module for bounded-range minimum cut
//!
//! This module provides the core abstractions for maintaining minimum proper cuts
//! over dynamic graphs with bounded cut values.

pub mod bounded;
pub mod stub;
pub mod traits;
pub mod witness;

pub use bounded::BoundedInstance;
pub use stub::StubInstance;
pub use traits::{InstanceResult, ProperCutInstance};
pub use witness::{ImplicitWitness, Witness, WitnessHandle};

#[cfg(test)]
mod tests {
    use super::*;
    use roaring::RoaringBitmap;

    #[test]
    fn test_module_exports() {
        let witness = WitnessHandle::new(0, RoaringBitmap::from_iter([0, 1]), 2);
        assert_eq!(witness.seed(), 0);

        let result = InstanceResult::ValueInRange {
            value: 2,
            witness: witness.clone(),
        };
        assert!(result.is_in_range());
    }

    #[test]
    fn test_witness_trait_object() {
        let witness = WitnessHandle::new(5, RoaringBitmap::from_iter([5, 6, 7]), 4);
        let trait_obj: &dyn Witness = &witness;

        assert_eq!(trait_obj.seed(), 5);
        assert_eq!(trait_obj.cardinality(), 3);
        assert!(trait_obj.contains(5));
        assert!(!trait_obj.contains(10));
    }
}
