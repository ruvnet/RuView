//! Membership filter for shared HNSW index traversal.
//!
//! Include mode (default): vector visible iff `filter.contains(id)`.
//! Exclude mode: vector visible iff `!filter.contains(id)`.
//!
//! Empty filter in include mode = empty view (fail-safe).
//!
//! HNSW traversal integration:
//! - Excluded nodes MAY be pushed onto exploration heap (routing waypoints)
//! - Excluded nodes MUST NOT be pushed onto result heap
//! - Excluded nodes DO NOT decrement `ef_remaining`

use rvf_types::membership::{FilterMode, MembershipHeader, MEMBERSHIP_MAGIC};
use rvf_types::{ErrorCode, RvfError};

/// Membership filter backed by a dense bitmap.
pub struct MembershipFilter {
    /// Include or exclude mode.
    mode: FilterMode,
    /// Dense bit vector: one bit per vector ID.
    bitmap: Vec<u64>,
    /// Total vector count (capacity of the filter).
    vector_count: u64,
    /// Number of set bits (members).
    member_count: u64,
    /// Generation counter for optimistic concurrency.
    generation_id: u32,
}

impl MembershipFilter {
    /// Create a new include-mode filter with given capacity. All bits start clear.
    pub fn new_include(vector_count: u64) -> Self {
        let words = vector_count.div_ceil(64) as usize;
        Self {
            mode: FilterMode::Include,
            bitmap: vec![0u64; words],
            vector_count,
            member_count: 0,
            generation_id: 0,
        }
    }

    /// Create a new exclude-mode filter with given capacity. All bits start clear.
    pub fn new_exclude(vector_count: u64) -> Self {
        let words = vector_count.div_ceil(64) as usize;
        Self {
            mode: FilterMode::Exclude,
            bitmap: vec![0u64; words],
            vector_count,
            member_count: 0,
            generation_id: 0,
        }
    }

    /// Add a vector ID to the filter.
    pub fn add(&mut self, vector_id: u64) {
        if vector_id >= self.vector_count {
            return;
        }
        let word = (vector_id / 64) as usize;
        let bit = vector_id % 64;
        if word < self.bitmap.len() {
            let mask = 1u64 << bit;
            if self.bitmap[word] & mask == 0 {
                self.bitmap[word] |= mask;
                self.member_count += 1;
            }
        }
    }

    /// Remove a vector ID from the filter.
    pub fn remove(&mut self, vector_id: u64) {
        if vector_id >= self.vector_count {
            return;
        }
        let word = (vector_id / 64) as usize;
        let bit = vector_id % 64;
        if word < self.bitmap.len() {
            let mask = 1u64 << bit;
            if self.bitmap[word] & mask != 0 {
                self.bitmap[word] &= !mask;
                self.member_count -= 1;
            }
        }
    }

    /// Check if a vector ID is in the filter bitmap.
    fn bitmap_contains(&self, vector_id: u64) -> bool {
        if vector_id >= self.vector_count {
            return false;
        }
        let word = (vector_id / 64) as usize;
        let bit = vector_id % 64;
        if word < self.bitmap.len() {
            self.bitmap[word] & (1u64 << bit) != 0
        } else {
            false
        }
    }

    /// Check if a vector ID is visible through this filter.
    ///
    /// In Include mode: visible iff the bit is set.
    /// In Exclude mode: visible iff the bit is NOT set.
    pub fn contains(&self, vector_id: u64) -> bool {
        match self.mode {
            FilterMode::Include => self.bitmap_contains(vector_id),
            FilterMode::Exclude => !self.bitmap_contains(vector_id),
        }
    }

    /// Number of set bits (members in the bitmap).
    pub fn member_count(&self) -> u64 {
        self.member_count
    }

    /// Total vector capacity.
    pub fn vector_count(&self) -> u64 {
        self.vector_count
    }

    /// Filter mode.
    pub fn mode(&self) -> FilterMode {
        self.mode
    }

    /// Generation ID.
    pub fn generation_id(&self) -> u32 {
        self.generation_id
    }

    /// Increment generation ID.
    pub fn bump_generation(&mut self) {
        self.generation_id += 1;
    }

    /// Serialize the bitmap to bytes (just the raw bitmap words).
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.bitmap.len() * 8);
        for &word in &self.bitmap {
            buf.extend_from_slice(&word.to_le_bytes());
        }
        buf
    }

    /// Deserialize a MembershipFilter from bitmap bytes and a header.
    pub fn deserialize(data: &[u8], header: &MembershipHeader) -> Result<Self, RvfError> {
        let mode = FilterMode::try_from(header.filter_mode)
            .map_err(|_| RvfError::Code(ErrorCode::MembershipInvalid))?;

        let word_count = header.vector_count.div_ceil(64) as usize;
        let expected_bytes = word_count * 8;
        if data.len() < expected_bytes {
            return Err(RvfError::Code(ErrorCode::MembershipInvalid));
        }

        let mut bitmap = Vec::with_capacity(word_count);
        for i in 0..word_count {
            let offset = i * 8;
            let word = u64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            bitmap.push(word);
        }

        // Recount set bits
        let member_count: u64 = bitmap.iter().map(|w| w.count_ones() as u64).sum();

        Ok(Self {
            mode,
            bitmap,
            vector_count: header.vector_count,
            member_count,
            generation_id: header.generation_id,
        })
    }

    /// Build a MembershipHeader for this filter.
    pub fn to_header(&self) -> MembershipHeader {
        let bitmap_bytes = self.serialize();
        let filter_hash = crate::store::simple_shake256_256(&bitmap_bytes);

        MembershipHeader {
            magic: MEMBERSHIP_MAGIC,
            version: 1,
            filter_type: rvf_types::membership::FilterType::Bitmap as u8,
            filter_mode: self.mode as u8,
            vector_count: self.vector_count,
            member_count: self.member_count,
            filter_offset: 96, // right after header
            filter_size: bitmap_bytes.len() as u32,
            generation_id: self.generation_id,
            filter_hash,
            bloom_offset: 0,
            bloom_size: 0,
            _reserved: 0,
            _reserved2: [0u8; 8],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn include_mode_empty_is_empty_view() {
        let filter = MembershipFilter::new_include(100);
        for i in 0..100 {
            assert!(!filter.contains(i));
        }
    }

    #[test]
    fn include_mode_add_and_check() {
        let mut filter = MembershipFilter::new_include(100);
        filter.add(10);
        filter.add(50);
        filter.add(99);

        assert!(filter.contains(10));
        assert!(filter.contains(50));
        assert!(filter.contains(99));
        assert!(!filter.contains(0));
        assert!(!filter.contains(11));
        assert_eq!(filter.member_count(), 3);
    }

    #[test]
    fn exclude_mode() {
        let mut filter = MembershipFilter::new_exclude(100);
        // In exclude mode, all are visible when bitmap is empty
        assert!(filter.contains(0));
        assert!(filter.contains(50));

        // Add to bitmap means "exclude this vector"
        filter.add(50);
        assert!(!filter.contains(50));
        assert!(filter.contains(0));
        assert!(filter.contains(99));
    }

    #[test]
    fn add_remove() {
        let mut filter = MembershipFilter::new_include(64);
        filter.add(10);
        assert_eq!(filter.member_count(), 1);
        assert!(filter.contains(10));

        filter.remove(10);
        assert_eq!(filter.member_count(), 0);
        assert!(!filter.contains(10));
    }

    #[test]
    fn add_out_of_bounds_ignored() {
        let mut filter = MembershipFilter::new_include(10);
        filter.add(100); // beyond vector_count
        assert_eq!(filter.member_count(), 0);
    }

    #[test]
    fn double_add_no_double_count() {
        let mut filter = MembershipFilter::new_include(64);
        filter.add(5);
        filter.add(5);
        assert_eq!(filter.member_count(), 1);
    }

    #[test]
    fn serialize_deserialize_round_trip() {
        let mut filter = MembershipFilter::new_include(200);
        filter.add(0);
        filter.add(63);
        filter.add(64);
        filter.add(127);
        filter.add(199);

        let header = filter.to_header();
        let bitmap_data = filter.serialize();

        let filter2 = MembershipFilter::deserialize(&bitmap_data, &header).unwrap();
        assert_eq!(filter2.vector_count(), 200);
        assert_eq!(filter2.member_count(), 5);
        assert!(filter2.contains(0));
        assert!(filter2.contains(63));
        assert!(filter2.contains(64));
        assert!(filter2.contains(127));
        assert!(filter2.contains(199));
        assert!(!filter2.contains(1));
        assert!(!filter2.contains(100));
    }

    #[test]
    fn generation_bump() {
        let mut filter = MembershipFilter::new_include(10);
        assert_eq!(filter.generation_id(), 0);
        filter.bump_generation();
        assert_eq!(filter.generation_id(), 1);
    }

    #[test]
    fn bitmap_word_boundary() {
        // Test vectors near 64-bit word boundaries
        let mut filter = MembershipFilter::new_include(130);
        filter.add(63);
        filter.add(64);
        filter.add(128);

        assert!(filter.contains(63));
        assert!(filter.contains(64));
        assert!(filter.contains(128));
        assert!(!filter.contains(62));
        assert!(!filter.contains(65));
        assert!(!filter.contains(129));
    }
}
