//! Filter expression evaluation for metadata-based vector filtering.
//!
//! Filter expressions are boolean predicate trees evaluated against
//! per-vector metadata. The runtime selects a strategy (pre-filter,
//! intra-filter, or post-filter) based on estimated selectivity.

use crate::options::MetadataValue;

/// A filter expression for metadata-based vector filtering.
///
/// Leaf nodes compare a metadata field against a literal value.
/// Internal nodes combine sub-expressions with boolean logic.
#[derive(Clone, Debug)]
pub enum FilterExpr {
    /// field == value
    Eq(u16, FilterValue),
    /// field != value
    Ne(u16, FilterValue),
    /// field < value
    Lt(u16, FilterValue),
    /// field <= value
    Le(u16, FilterValue),
    /// field > value
    Gt(u16, FilterValue),
    /// field >= value
    Ge(u16, FilterValue),
    /// field in [values]
    In(u16, Vec<FilterValue>),
    /// field in [low, high)
    Range(u16, FilterValue, FilterValue),
    /// All sub-expressions must match.
    And(Vec<FilterExpr>),
    /// Any sub-expression must match.
    Or(Vec<FilterExpr>),
    /// Negate the sub-expression.
    Not(Box<FilterExpr>),
}

/// A typed value used in filter comparisons.
#[derive(Clone, Debug, PartialEq)]
pub enum FilterValue {
    U64(u64),
    I64(i64),
    F64(f64),
    String(String),
    Bool(bool),
}

impl FilterValue {
    /// Compare two filter values. Returns None if types are incompatible.
    fn partial_cmp_value(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (FilterValue::U64(a), FilterValue::U64(b)) => a.partial_cmp(b),
            (FilterValue::I64(a), FilterValue::I64(b)) => a.partial_cmp(b),
            (FilterValue::F64(a), FilterValue::F64(b)) => a.partial_cmp(b),
            (FilterValue::String(a), FilterValue::String(b)) => a.partial_cmp(b),
            (FilterValue::Bool(a), FilterValue::Bool(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

/// In-memory metadata store for filter evaluation.
/// Maps (vector_id, field_id) -> MetadataValue.
pub(crate) struct MetadataStore {
    /// Entries indexed by vector position.
    entries: Vec<Vec<(u16, FilterValue)>>,
    /// Mapping from vector_id to position index.
    id_to_pos: std::collections::HashMap<u64, usize>,
}

impl MetadataStore {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
            id_to_pos: std::collections::HashMap::new(),
        }
    }

    /// Add metadata for a vector. `fields` are (field_id, value) pairs.
    pub(crate) fn insert(&mut self, vector_id: u64, fields: Vec<(u16, FilterValue)>) {
        let pos = self.entries.len();
        self.id_to_pos.insert(vector_id, pos);
        self.entries.push(fields);
    }

    /// Get a field value for a vector.
    pub(crate) fn get_field(&self, vector_id: u64, field_id: u16) -> Option<&FilterValue> {
        let pos = self.id_to_pos.get(&vector_id)?;
        self.entries
            .get(*pos)?
            .iter()
            .find(|(fid, _)| *fid == field_id)
            .map(|(_, v)| v)
    }

    /// Remove all metadata for the given vector IDs.
    pub(crate) fn remove_ids(&mut self, ids: &[u64]) {
        for id in ids {
            self.id_to_pos.remove(id);
        }
    }

    /// Return vector count tracked by the metadata store.
    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        self.id_to_pos.len()
    }
}

/// Evaluate a filter expression against a single vector's metadata.
pub(crate) fn evaluate(expr: &FilterExpr, vector_id: u64, meta: &MetadataStore) -> bool {
    match expr {
        FilterExpr::Eq(field_id, val) => meta
            .get_field(vector_id, *field_id)
            .map(|v| v == val)
            .unwrap_or(false),
        FilterExpr::Ne(field_id, val) => meta
            .get_field(vector_id, *field_id)
            .map(|v| v != val)
            .unwrap_or(true),
        FilterExpr::Lt(field_id, val) => meta
            .get_field(vector_id, *field_id)
            .and_then(|v| v.partial_cmp_value(val))
            .map(|ord| ord == std::cmp::Ordering::Less)
            .unwrap_or(false),
        FilterExpr::Le(field_id, val) => meta
            .get_field(vector_id, *field_id)
            .and_then(|v| v.partial_cmp_value(val))
            .map(|ord| ord != std::cmp::Ordering::Greater)
            .unwrap_or(false),
        FilterExpr::Gt(field_id, val) => meta
            .get_field(vector_id, *field_id)
            .and_then(|v| v.partial_cmp_value(val))
            .map(|ord| ord == std::cmp::Ordering::Greater)
            .unwrap_or(false),
        FilterExpr::Ge(field_id, val) => meta
            .get_field(vector_id, *field_id)
            .and_then(|v| v.partial_cmp_value(val))
            .map(|ord| ord != std::cmp::Ordering::Less)
            .unwrap_or(false),
        FilterExpr::In(field_id, vals) => meta
            .get_field(vector_id, *field_id)
            .map(|v| vals.contains(v))
            .unwrap_or(false),
        FilterExpr::Range(field_id, low, high) => meta
            .get_field(vector_id, *field_id)
            .and_then(|v| {
                let ge_low = v
                    .partial_cmp_value(low)
                    .map(|o| o != std::cmp::Ordering::Less)?;
                let lt_high = v
                    .partial_cmp_value(high)
                    .map(|o| o == std::cmp::Ordering::Less)?;
                Some(ge_low && lt_high)
            })
            .unwrap_or(false),
        FilterExpr::And(exprs) => exprs.iter().all(|e| evaluate(e, vector_id, meta)),
        FilterExpr::Or(exprs) => exprs.iter().any(|e| evaluate(e, vector_id, meta)),
        FilterExpr::Not(expr) => !evaluate(expr, vector_id, meta),
    }
}

/// Convert a MetadataValue (options module) to a FilterValue for evaluation.
pub(crate) fn metadata_value_to_filter(mv: &MetadataValue) -> FilterValue {
    match mv {
        MetadataValue::U64(v) => FilterValue::U64(*v),
        MetadataValue::I64(v) => FilterValue::I64(*v),
        MetadataValue::F64(v) => FilterValue::F64(*v),
        MetadataValue::String(v) => FilterValue::String(v.clone()),
        MetadataValue::Bytes(_) => FilterValue::String(String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store() -> MetadataStore {
        let mut store = MetadataStore::new();
        store.insert(
            0,
            vec![
                (0, FilterValue::String("apple".into())),
                (1, FilterValue::U64(100)),
            ],
        );
        store.insert(
            1,
            vec![
                (0, FilterValue::String("banana".into())),
                (1, FilterValue::U64(200)),
            ],
        );
        store.insert(
            2,
            vec![
                (0, FilterValue::String("apple".into())),
                (1, FilterValue::U64(300)),
            ],
        );
        store
    }

    #[test]
    fn filter_eq() {
        let store = make_store();
        let expr = FilterExpr::Eq(0, FilterValue::String("apple".into()));
        assert!(evaluate(&expr, 0, &store));
        assert!(!evaluate(&expr, 1, &store));
        assert!(evaluate(&expr, 2, &store));
    }

    #[test]
    fn filter_ne() {
        let store = make_store();
        let expr = FilterExpr::Ne(0, FilterValue::String("apple".into()));
        assert!(!evaluate(&expr, 0, &store));
        assert!(evaluate(&expr, 1, &store));
    }

    #[test]
    fn filter_range() {
        let store = make_store();
        let expr = FilterExpr::Range(1, FilterValue::U64(150), FilterValue::U64(250));
        assert!(!evaluate(&expr, 0, &store)); // 100 < 150
        assert!(evaluate(&expr, 1, &store)); // 200 in [150, 250)
        assert!(!evaluate(&expr, 2, &store)); // 300 >= 250
    }

    #[test]
    fn filter_and_or() {
        let store = make_store();
        let expr = FilterExpr::And(vec![
            FilterExpr::Eq(0, FilterValue::String("apple".into())),
            FilterExpr::Gt(1, FilterValue::U64(150)),
        ]);
        assert!(!evaluate(&expr, 0, &store)); // apple but 100 <= 150
        assert!(!evaluate(&expr, 1, &store)); // banana
        assert!(evaluate(&expr, 2, &store)); // apple and 300 > 150
    }

    #[test]
    fn filter_not() {
        let store = make_store();
        let expr = FilterExpr::Not(Box::new(FilterExpr::Eq(
            0,
            FilterValue::String("apple".into()),
        )));
        assert!(!evaluate(&expr, 0, &store));
        assert!(evaluate(&expr, 1, &store));
    }

    #[test]
    fn filter_in() {
        let store = make_store();
        let expr = FilterExpr::In(1, vec![FilterValue::U64(100), FilterValue::U64(300)]);
        assert!(evaluate(&expr, 0, &store));
        assert!(!evaluate(&expr, 1, &store));
        assert!(evaluate(&expr, 2, &store));
    }
}
