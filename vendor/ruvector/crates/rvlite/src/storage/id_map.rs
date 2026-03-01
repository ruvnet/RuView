//! Direct mapping between RVF vector IDs and SQL primary keys.
//!
//! In rvlite the mapping is identity: RVF u64 IDs are the same as SQL
//! primary keys. This zero-cost design avoids an extra lookup table and
//! keeps memory usage minimal.
//!
//! The [`IdMapping`] trait exists for future extensibility -- if a
//! non-identity mapping is ever needed (e.g. hashed IDs, composite keys),
//! a new implementation can be swapped in without changing call sites.

/// Trait for converting between RVF vector IDs and SQL primary keys.
///
/// Implementors define how the two ID spaces relate to each other.
/// The default implementation ([`DirectIdMap`]) uses identity mapping.
pub trait IdMapping {
    /// Convert a SQL primary key to an RVF vector ID.
    fn to_rvf_id(&self, sql_pk: u64) -> u64;

    /// Convert an RVF vector ID back to a SQL primary key.
    fn to_sql_pk(&self, rvf_id: u64) -> u64;

    /// Validate that every RVF ID in the slice has a corresponding SQL PK
    /// in the other slice, and vice versa. Both slices must contain the
    /// same set of values (possibly in different order) for the mapping
    /// to be considered valid.
    fn validate_mapping(&self, rvf_ids: &[u64], sql_pks: &[u64]) -> bool;
}

/// Zero-cost identity mapping where RVF u64 IDs equal SQL primary keys.
///
/// This is the default and recommended mapping for rvlite. Because
/// both ID spaces use `u64`, no conversion is needed and the mapping
/// functions compile down to no-ops.
///
/// # Example
///
/// ```
/// # use rvlite::storage::id_map::{DirectIdMap, IdMapping};
/// let map = DirectIdMap;
/// assert_eq!(map.to_rvf_id(42), 42);
/// assert_eq!(map.to_sql_pk(42), 42);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct DirectIdMap;

impl DirectIdMap {
    /// Create a new direct (identity) ID map.
    pub fn new() -> Self {
        Self
    }

    /// Convert a SQL primary key to an RVF vector ID (identity).
    ///
    /// This is a free function alternative to the trait method, useful when
    /// you know the concrete type and want to avoid dynamic dispatch.
    #[inline(always)]
    pub fn to_rvf_id(sql_pk: u64) -> u64 {
        sql_pk
    }

    /// Convert an RVF vector ID to a SQL primary key (identity).
    #[inline(always)]
    pub fn to_sql_pk(rvf_id: u64) -> u64 {
        rvf_id
    }

    /// Validate that the two slices contain the same set of IDs.
    ///
    /// Under identity mapping, `rvf_ids` and `sql_pks` must be equal
    /// as sets (same elements, possibly different order).
    pub fn validate_mapping(rvf_ids: &[u64], sql_pks: &[u64]) -> bool {
        if rvf_ids.len() != sql_pks.len() {
            return false;
        }
        let mut rvf_sorted: Vec<u64> = rvf_ids.to_vec();
        let mut sql_sorted: Vec<u64> = sql_pks.to_vec();
        rvf_sorted.sort_unstable();
        sql_sorted.sort_unstable();
        rvf_sorted == sql_sorted
    }
}

impl IdMapping for DirectIdMap {
    #[inline(always)]
    fn to_rvf_id(&self, sql_pk: u64) -> u64 {
        sql_pk
    }

    #[inline(always)]
    fn to_sql_pk(&self, rvf_id: u64) -> u64 {
        rvf_id
    }

    fn validate_mapping(&self, rvf_ids: &[u64], sql_pks: &[u64]) -> bool {
        DirectIdMap::validate_mapping(rvf_ids, sql_pks)
    }
}

/// An offset-based ID mapping where SQL PKs start from a different base.
///
/// Useful when the SQL table uses auto-increment starting at 1 but
/// the RVF store is zero-indexed (or vice versa).
///
/// `rvf_id = sql_pk + offset`
#[derive(Debug, Clone, Copy)]
pub struct OffsetIdMap {
    /// Offset added to SQL PK to produce the RVF ID.
    /// Can be negative via wrapping arithmetic on u64.
    offset: i64,
}

impl OffsetIdMap {
    /// Create an offset mapping.
    ///
    /// `offset` is added to SQL PKs to produce RVF IDs.
    /// Use a negative offset if RVF IDs are smaller than SQL PKs.
    pub fn new(offset: i64) -> Self {
        Self { offset }
    }
}

impl IdMapping for OffsetIdMap {
    #[inline]
    fn to_rvf_id(&self, sql_pk: u64) -> u64 {
        (sql_pk as i64).wrapping_add(self.offset) as u64
    }

    #[inline]
    fn to_sql_pk(&self, rvf_id: u64) -> u64 {
        (rvf_id as i64).wrapping_sub(self.offset) as u64
    }

    fn validate_mapping(&self, rvf_ids: &[u64], sql_pks: &[u64]) -> bool {
        if rvf_ids.len() != sql_pks.len() {
            return false;
        }
        let mut expected: Vec<u64> = sql_pks.iter().map(|&pk| self.to_rvf_id(pk)).collect();
        let mut actual: Vec<u64> = rvf_ids.to_vec();
        expected.sort_unstable();
        actual.sort_unstable();
        expected == actual
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- DirectIdMap tests ----

    #[test]
    fn direct_to_rvf_id_is_identity() {
        assert_eq!(DirectIdMap::to_rvf_id(0), 0);
        assert_eq!(DirectIdMap::to_rvf_id(42), 42);
        assert_eq!(DirectIdMap::to_rvf_id(u64::MAX), u64::MAX);
    }

    #[test]
    fn direct_to_sql_pk_is_identity() {
        assert_eq!(DirectIdMap::to_sql_pk(0), 0);
        assert_eq!(DirectIdMap::to_sql_pk(42), 42);
        assert_eq!(DirectIdMap::to_sql_pk(u64::MAX), u64::MAX);
    }

    #[test]
    fn direct_roundtrip() {
        for id in [0, 1, 100, u64::MAX / 2, u64::MAX] {
            assert_eq!(DirectIdMap::to_sql_pk(DirectIdMap::to_rvf_id(id)), id);
            assert_eq!(DirectIdMap::to_rvf_id(DirectIdMap::to_sql_pk(id)), id);
        }
    }

    #[test]
    fn direct_validate_same_elements() {
        let rvf = vec![1, 2, 3];
        let sql = vec![3, 1, 2];
        assert!(DirectIdMap::validate_mapping(&rvf, &sql));
    }

    #[test]
    fn direct_validate_empty() {
        assert!(DirectIdMap::validate_mapping(&[], &[]));
    }

    #[test]
    fn direct_validate_different_length_fails() {
        let rvf = vec![1, 2, 3];
        let sql = vec![1, 2];
        assert!(!DirectIdMap::validate_mapping(&rvf, &sql));
    }

    #[test]
    fn direct_validate_different_elements_fails() {
        let rvf = vec![1, 2, 3];
        let sql = vec![1, 2, 4];
        assert!(!DirectIdMap::validate_mapping(&rvf, &sql));
    }

    #[test]
    fn direct_validate_duplicates_match() {
        let rvf = vec![1, 1, 2];
        let sql = vec![1, 2, 1];
        assert!(DirectIdMap::validate_mapping(&rvf, &sql));
    }

    #[test]
    fn direct_validate_duplicates_mismatch() {
        let rvf = vec![1, 1, 2];
        let sql = vec![1, 2, 2];
        assert!(!DirectIdMap::validate_mapping(&rvf, &sql));
    }

    // ---- IdMapping trait via DirectIdMap ----

    #[test]
    fn trait_direct_to_rvf_id() {
        let map = DirectIdMap;
        assert_eq!(IdMapping::to_rvf_id(&map, 99), 99);
    }

    #[test]
    fn trait_direct_to_sql_pk() {
        let map = DirectIdMap;
        assert_eq!(IdMapping::to_sql_pk(&map, 99), 99);
    }

    #[test]
    fn trait_direct_validate() {
        let map = DirectIdMap;
        assert!(IdMapping::validate_mapping(&map, &[1, 2], &[2, 1]));
        assert!(!IdMapping::validate_mapping(&map, &[1, 2], &[2, 3]));
    }

    // ---- OffsetIdMap tests ----

    #[test]
    fn offset_positive() {
        let map = OffsetIdMap::new(10);
        assert_eq!(map.to_rvf_id(0), 10);
        assert_eq!(map.to_rvf_id(5), 15);
        assert_eq!(map.to_sql_pk(10), 0);
        assert_eq!(map.to_sql_pk(15), 5);
    }

    #[test]
    fn offset_negative() {
        let map = OffsetIdMap::new(-1);
        // SQL PK 1 -> RVF ID 0
        assert_eq!(map.to_rvf_id(1), 0);
        assert_eq!(map.to_sql_pk(0), 1);
    }

    #[test]
    fn offset_zero_is_identity() {
        let map = OffsetIdMap::new(0);
        for id in [0, 1, 42, 1000] {
            assert_eq!(map.to_rvf_id(id), id);
            assert_eq!(map.to_sql_pk(id), id);
        }
    }

    #[test]
    fn offset_roundtrip() {
        let map = OffsetIdMap::new(7);
        for pk in [0, 1, 100, 999] {
            assert_eq!(map.to_sql_pk(map.to_rvf_id(pk)), pk);
        }
    }

    #[test]
    fn offset_validate() {
        let map = OffsetIdMap::new(10);
        // SQL PKs [0, 1, 2] -> RVF IDs [10, 11, 12]
        assert!(map.validate_mapping(&[12, 10, 11], &[2, 0, 1]));
        assert!(!map.validate_mapping(&[10, 11, 12], &[0, 1, 3]));
    }

    // ---- Dynamic dispatch ----

    #[test]
    fn trait_object_works() {
        let direct: Box<dyn IdMapping> = Box::new(DirectIdMap);
        assert_eq!(direct.to_rvf_id(5), 5);

        let offset: Box<dyn IdMapping> = Box::new(OffsetIdMap::new(100));
        assert_eq!(offset.to_rvf_id(5), 105);
    }

    // ---- Default impl ----

    #[test]
    fn direct_default() {
        let map: DirectIdMap = Default::default();
        assert_eq!(map.to_rvf_id(7), 7);
    }
}
