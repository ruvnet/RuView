//! Comprehensive tests for TileReport generation and serialization
//!
//! Tests cover:
//! - Report creation and initialization
//! - Serialization/deserialization roundtrips
//! - Checksum verification
//! - WitnessFragment operations

use cognitum_gate_kernel::report::{TileReport, TileStatus, WitnessFragment};
use cognitum_gate_kernel::shard::EdgeId;

#[cfg(test)]
mod tile_status {
    use super::*;

    #[test]
    fn test_status_values() {
        assert_eq!(TileStatus::Active as u8, 0);
        assert_eq!(TileStatus::Idle as u8, 1);
        assert_eq!(TileStatus::Recovery as u8, 2);
        assert_eq!(TileStatus::Error as u8, 3);
    }

    #[test]
    fn test_status_from_u8() {
        assert_eq!(TileStatus::from_u8(0), Some(TileStatus::Active));
        assert_eq!(TileStatus::from_u8(1), Some(TileStatus::Idle));
        assert_eq!(TileStatus::from_u8(255), None);
    }

    #[test]
    fn test_is_healthy() {
        assert!(TileStatus::Active.is_healthy());
        assert!(TileStatus::Idle.is_healthy());
        assert!(!TileStatus::Error.is_healthy());
    }
}

#[cfg(test)]
mod witness_fragment {
    use super::*;

    #[test]
    fn test_fragment_creation() {
        let frag = WitnessFragment::new(42);
        assert_eq!(frag.tile_id, 42);
        assert_eq!(frag.min_cut_value, 0);
    }

    #[test]
    fn test_is_fragile() {
        let mut frag = WitnessFragment::new(0);
        frag.min_cut_value = 5;
        assert!(frag.is_fragile(10));
        assert!(!frag.is_fragile(5));
    }

    #[test]
    fn test_fragment_hash_deterministic() {
        let frag = WitnessFragment::new(5);
        assert_eq!(frag.compute_hash(), frag.compute_hash());
    }

    #[test]
    fn test_fragment_hash_unique() {
        let frag1 = WitnessFragment::new(1);
        let frag2 = WitnessFragment::new(2);
        assert_ne!(frag1.compute_hash(), frag2.compute_hash());
    }
}

#[cfg(test)]
mod tile_report_creation {
    use super::*;

    #[test]
    fn test_new_report() {
        let report = TileReport::new(5);
        assert_eq!(report.tile_id, 5);
        assert_eq!(report.status, TileStatus::Active);
        assert!(report.is_healthy());
    }

    #[test]
    fn test_error_report() {
        let report = TileReport::error(10);
        assert_eq!(report.status, TileStatus::Error);
        assert!(!report.is_healthy());
    }

    #[test]
    fn test_idle_report() {
        let report = TileReport::idle(15);
        assert_eq!(report.status, TileStatus::Idle);
        assert!(report.is_healthy());
    }
}

#[cfg(test)]
mod report_health_checks {
    use super::*;

    #[test]
    fn test_needs_attention_boundary_moved() {
        let mut report = TileReport::new(0);
        assert!(!report.needs_attention());
        report.boundary_moved = true;
        assert!(report.needs_attention());
    }

    #[test]
    fn test_needs_attention_negative_coherence() {
        let mut report = TileReport::new(0);
        report.coherence = -100;
        assert!(report.needs_attention());
    }
}

#[cfg(test)]
mod coherence_conversion {
    use super::*;

    #[test]
    fn test_coherence_f32_values() {
        let mut report = TileReport::new(0);

        report.coherence = 0;
        assert!((report.coherence_f32() - 0.0).abs() < 0.001);

        report.coherence = 256;
        assert!((report.coherence_f32() - 1.0).abs() < 0.01);

        report.coherence = -128;
        assert!((report.coherence_f32() - (-0.5)).abs() < 0.01);
    }
}

#[cfg(test)]
mod serialization {
    use super::*;

    #[test]
    fn test_to_bytes_size() {
        let report = TileReport::new(0);
        let bytes = report.to_bytes();
        assert_eq!(bytes.len(), 64);
    }

    #[test]
    fn test_roundtrip_basic() {
        let report = TileReport::new(42);
        let bytes = report.to_bytes();
        let restored = TileReport::from_bytes(&bytes).unwrap();
        assert_eq!(report.tile_id, restored.tile_id);
        assert_eq!(report.status, restored.status);
    }

    #[test]
    fn test_roundtrip_with_data() {
        let mut report = TileReport::new(100);
        report.coherence = 512;
        report.e_value = 2.5;
        report.boundary_moved = true;
        report.suspicious_edges[0] = EdgeId(100);

        let bytes = report.to_bytes();
        let restored = TileReport::from_bytes(&bytes).unwrap();

        assert_eq!(restored.coherence, 512);
        assert!((restored.e_value - 2.5).abs() < 0.001);
        assert!(restored.boundary_moved);
        assert_eq!(restored.suspicious_edges[0], EdgeId(100));
    }
}

#[cfg(test)]
mod checksum {
    use super::*;

    #[test]
    fn test_checksum_deterministic() {
        let report = TileReport::new(42);
        assert_eq!(report.checksum(), report.checksum());
    }

    #[test]
    fn test_checksum_different_reports() {
        let r1 = TileReport::new(1);
        let r2 = TileReport::new(2);
        assert_ne!(r1.checksum(), r2.checksum());
    }

    #[test]
    fn test_verify_checksum() {
        let report = TileReport::new(42);
        let cs = report.checksum();
        assert!(report.verify_checksum(cs));
        assert!(!report.verify_checksum(0));
    }
}

#[cfg(test)]
mod report_size {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn test_report_fits_cache_line() {
        assert!(size_of::<TileReport>() <= 64);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_serialization_roundtrip(
            tile_id in 0u8..255,
            coherence in i16::MIN..i16::MAX,
            e_value in 0.0f32..100.0,
            boundary_moved: bool
        ) {
            let mut report = TileReport::new(tile_id);
            report.coherence = coherence;
            report.e_value = e_value;
            report.boundary_moved = boundary_moved;

            let bytes = report.to_bytes();
            let restored = TileReport::from_bytes(&bytes).unwrap();

            assert_eq!(report.tile_id, restored.tile_id);
            assert_eq!(report.coherence, restored.coherence);
            assert_eq!(report.boundary_moved, restored.boundary_moved);
        }

        #[test]
        fn prop_checksum_changes_with_data(a: i16, b: i16) {
            prop_assume!(a != b);
            let mut r1 = TileReport::new(0);
            let mut r2 = TileReport::new(0);
            r1.coherence = a;
            r2.coherence = b;
            assert_ne!(r1.checksum(), r2.checksum());
        }
    }
}
