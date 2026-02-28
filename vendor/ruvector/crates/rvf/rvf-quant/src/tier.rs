//! Temperature tier assignment for vector blocks.

/// Access count above which a block is considered "hot" (Tier 0).
pub const HOT_THRESHOLD: u8 = 128;

/// Access count above which a block is considered "warm" (Tier 1).
/// Below this threshold, a block is "cold" (Tier 2).
pub const WARM_THRESHOLD: u8 = 16;

/// Temperature tier for a vector block.
///
/// Determines the quantization level and storage layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum TemperatureTier {
    /// Frequently accessed. Scalar quantized (int8), interleaved layout.
    Hot = 0,
    /// Moderately accessed. Product quantized, columnar layout.
    Warm = 1,
    /// Rarely accessed. Binary quantized, columnar + heavy compression.
    Cold = 2,
}

/// Assign a temperature tier based on the estimated access count.
///
/// Uses the thresholds from the RVF spec:
/// - `count > HOT_THRESHOLD`  -> Hot
/// - `count > WARM_THRESHOLD` -> Warm
/// - otherwise                -> Cold
pub fn assign_tier(access_count: u8) -> TemperatureTier {
    if access_count > HOT_THRESHOLD {
        TemperatureTier::Hot
    } else if access_count > WARM_THRESHOLD {
        TemperatureTier::Warm
    } else {
        TemperatureTier::Cold
    }
}

impl TemperatureTier {
    /// Returns the wire representation (0, 1, or 2).
    #[inline]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

impl TryFrom<u8> for TemperatureTier {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Hot),
            1 => Ok(Self::Warm),
            2 => Ok(Self::Cold),
            other => Err(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_assignment() {
        assert_eq!(assign_tier(255), TemperatureTier::Hot);
        assert_eq!(assign_tier(129), TemperatureTier::Hot);
        assert_eq!(assign_tier(128), TemperatureTier::Warm); // not >128
        assert_eq!(assign_tier(64), TemperatureTier::Warm);
        assert_eq!(assign_tier(17), TemperatureTier::Warm);
        assert_eq!(assign_tier(16), TemperatureTier::Cold); // not >16
        assert_eq!(assign_tier(1), TemperatureTier::Cold);
        assert_eq!(assign_tier(0), TemperatureTier::Cold);
    }

    #[test]
    fn round_trip() {
        for raw in 0..=2u8 {
            let t = TemperatureTier::try_from(raw).unwrap();
            assert_eq!(t.as_u8(), raw);
        }
    }

    #[test]
    fn invalid_tier() {
        assert_eq!(TemperatureTier::try_from(3), Err(3));
    }

    #[test]
    fn ordering() {
        assert!(TemperatureTier::Hot < TemperatureTier::Warm);
        assert!(TemperatureTier::Warm < TemperatureTier::Cold);
    }
}
