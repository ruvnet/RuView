use crate::error::{FilterError, Result};
use crate::expression::FilterExpression;
use crate::index::{PayloadIndex, PayloadIndexManager};
use ordered_float::OrderedFloat;
use serde_json::Value;
use std::collections::HashSet;

/// Evaluates filter expressions against payload indices
pub struct FilterEvaluator<'a> {
    indices: &'a PayloadIndexManager,
}

impl<'a> FilterEvaluator<'a> {
    /// Create a new filter evaluator
    pub fn new(indices: &'a PayloadIndexManager) -> Self {
        Self { indices }
    }

    /// Evaluate a filter expression and return matching vector IDs
    pub fn evaluate(&self, filter: &FilterExpression) -> Result<HashSet<String>> {
        match filter {
            FilterExpression::Eq { field, value } => self.evaluate_eq(field, value),
            FilterExpression::Ne { field, value } => self.evaluate_ne(field, value),
            FilterExpression::Gt { field, value } => self.evaluate_gt(field, value),
            FilterExpression::Gte { field, value } => self.evaluate_gte(field, value),
            FilterExpression::Lt { field, value } => self.evaluate_lt(field, value),
            FilterExpression::Lte { field, value } => self.evaluate_lte(field, value),
            FilterExpression::Range { field, gte, lte } => {
                self.evaluate_range(field, gte.as_ref(), lte.as_ref())
            }
            FilterExpression::In { field, values } => self.evaluate_in(field, values),
            FilterExpression::Match { field, text } => self.evaluate_match(field, text),
            FilterExpression::GeoRadius {
                field,
                lat,
                lon,
                radius_m,
            } => self.evaluate_geo_radius(field, *lat, *lon, *radius_m),
            FilterExpression::GeoBoundingBox {
                field,
                top_left,
                bottom_right,
            } => self.evaluate_geo_bbox(field, *top_left, *bottom_right),
            FilterExpression::And(filters) => self.evaluate_and(filters),
            FilterExpression::Or(filters) => self.evaluate_or(filters),
            FilterExpression::Not(filter) => self.evaluate_not(filter),
            FilterExpression::Exists { field } => self.evaluate_exists(field),
            FilterExpression::IsNull { field } => self.evaluate_is_null(field),
        }
    }

    /// Check if a payload matches a filter expression
    pub fn matches(&self, payload: &Value, filter: &FilterExpression) -> bool {
        match filter {
            FilterExpression::Eq { field, value } => {
                Self::get_field_value(payload, field).map_or(false, |v| v == value)
            }
            FilterExpression::Ne { field, value } => {
                Self::get_field_value(payload, field).map_or(true, |v| v != value)
            }
            FilterExpression::Gt { field, value } => Self::get_field_value(payload, field)
                .map_or(false, |v| {
                    Self::compare_values(v, value) == Some(std::cmp::Ordering::Greater)
                }),
            FilterExpression::Gte { field, value } => {
                Self::get_field_value(payload, field).map_or(false, |v| {
                    matches!(
                        Self::compare_values(v, value),
                        Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
                    )
                })
            }
            FilterExpression::Lt { field, value } => Self::get_field_value(payload, field)
                .map_or(false, |v| {
                    Self::compare_values(v, value) == Some(std::cmp::Ordering::Less)
                }),
            FilterExpression::Lte { field, value } => {
                Self::get_field_value(payload, field).map_or(false, |v| {
                    matches!(
                        Self::compare_values(v, value),
                        Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
                    )
                })
            }
            FilterExpression::Range { field, gte, lte } => {
                if let Some(v) = Self::get_field_value(payload, field) {
                    let gte_match = gte.as_ref().map_or(true, |gte_val| {
                        matches!(
                            Self::compare_values(v, gte_val),
                            Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
                        )
                    });
                    let lte_match = lte.as_ref().map_or(true, |lte_val| {
                        matches!(
                            Self::compare_values(v, lte_val),
                            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
                        )
                    });
                    gte_match && lte_match
                } else {
                    false
                }
            }
            FilterExpression::In { field, values } => {
                Self::get_field_value(payload, field).map_or(false, |v| values.contains(v))
            }
            FilterExpression::Match { field, text } => Self::get_field_value(payload, field)
                .and_then(|v| v.as_str())
                .map_or(false, |s| s.to_lowercase().contains(&text.to_lowercase())),
            FilterExpression::And(filters) => filters.iter().all(|f| self.matches(payload, f)),
            FilterExpression::Or(filters) => filters.iter().any(|f| self.matches(payload, f)),
            FilterExpression::Not(filter) => !self.matches(payload, filter),
            FilterExpression::Exists { field } => Self::get_field_value(payload, field).is_some(),
            FilterExpression::IsNull { field } => {
                Self::get_field_value(payload, field).map_or(true, |v| v.is_null())
            }
            _ => false, // Geo operations not supported in direct matching
        }
    }

    fn evaluate_eq(&self, field: &str, value: &Value) -> Result<HashSet<String>> {
        let index = self
            .indices
            .get_index(field)
            .ok_or_else(|| FilterError::IndexNotFound(field.to_string()))?;

        match index {
            PayloadIndex::Integer(map) => {
                if let Some(num) = value.as_i64() {
                    Ok(map.get(&num).cloned().unwrap_or_default())
                } else {
                    Ok(HashSet::new())
                }
            }
            PayloadIndex::Float(map) => {
                if let Some(num) = value.as_f64() {
                    Ok(map.get(&OrderedFloat(num)).cloned().unwrap_or_default())
                } else {
                    Ok(HashSet::new())
                }
            }
            PayloadIndex::Keyword(map) => {
                if let Some(s) = value.as_str() {
                    Ok(map.get(s).cloned().unwrap_or_default())
                } else {
                    Ok(HashSet::new())
                }
            }
            PayloadIndex::Bool(map) => {
                if let Some(b) = value.as_bool() {
                    Ok(map.get(&b).cloned().unwrap_or_default())
                } else {
                    Ok(HashSet::new())
                }
            }
            _ => Err(FilterError::InvalidIndexType(field.to_string())),
        }
    }

    fn evaluate_ne(&self, field: &str, value: &Value) -> Result<HashSet<String>> {
        let eq_results = self.evaluate_eq(field, value)?;
        let all_ids = self.get_all_ids_for_field(field)?;
        Ok(all_ids.difference(&eq_results).cloned().collect())
    }

    fn evaluate_gt(&self, field: &str, value: &Value) -> Result<HashSet<String>> {
        let index = self
            .indices
            .get_index(field)
            .ok_or_else(|| FilterError::IndexNotFound(field.to_string()))?;

        match index {
            PayloadIndex::Integer(map) => {
                if let Some(num) = value.as_i64() {
                    Ok(map
                        .range((num + 1)..)
                        .flat_map(|(_, ids)| ids)
                        .cloned()
                        .collect())
                } else {
                    Ok(HashSet::new())
                }
            }
            PayloadIndex::Float(map) => {
                if let Some(num) = value.as_f64() {
                    let threshold = OrderedFloat(num);
                    Ok(map
                        .range(threshold..)
                        .filter(|(k, _)| **k > threshold)
                        .flat_map(|(_, ids)| ids)
                        .cloned()
                        .collect())
                } else {
                    Ok(HashSet::new())
                }
            }
            _ => Err(FilterError::InvalidIndexType(field.to_string())),
        }
    }

    fn evaluate_gte(&self, field: &str, value: &Value) -> Result<HashSet<String>> {
        let index = self
            .indices
            .get_index(field)
            .ok_or_else(|| FilterError::IndexNotFound(field.to_string()))?;

        match index {
            PayloadIndex::Integer(map) => {
                if let Some(num) = value.as_i64() {
                    Ok(map.range(num..).flat_map(|(_, ids)| ids).cloned().collect())
                } else {
                    Ok(HashSet::new())
                }
            }
            PayloadIndex::Float(map) => {
                if let Some(num) = value.as_f64() {
                    Ok(map
                        .range(OrderedFloat(num)..)
                        .flat_map(|(_, ids)| ids)
                        .cloned()
                        .collect())
                } else {
                    Ok(HashSet::new())
                }
            }
            _ => Err(FilterError::InvalidIndexType(field.to_string())),
        }
    }

    fn evaluate_lt(&self, field: &str, value: &Value) -> Result<HashSet<String>> {
        let index = self
            .indices
            .get_index(field)
            .ok_or_else(|| FilterError::IndexNotFound(field.to_string()))?;

        match index {
            PayloadIndex::Integer(map) => {
                if let Some(num) = value.as_i64() {
                    Ok(map.range(..num).flat_map(|(_, ids)| ids).cloned().collect())
                } else {
                    Ok(HashSet::new())
                }
            }
            PayloadIndex::Float(map) => {
                if let Some(num) = value.as_f64() {
                    Ok(map
                        .range(..OrderedFloat(num))
                        .flat_map(|(_, ids)| ids)
                        .cloned()
                        .collect())
                } else {
                    Ok(HashSet::new())
                }
            }
            _ => Err(FilterError::InvalidIndexType(field.to_string())),
        }
    }

    fn evaluate_lte(&self, field: &str, value: &Value) -> Result<HashSet<String>> {
        let index = self
            .indices
            .get_index(field)
            .ok_or_else(|| FilterError::IndexNotFound(field.to_string()))?;

        match index {
            PayloadIndex::Integer(map) => {
                if let Some(num) = value.as_i64() {
                    Ok(map
                        .range(..=num)
                        .flat_map(|(_, ids)| ids)
                        .cloned()
                        .collect())
                } else {
                    Ok(HashSet::new())
                }
            }
            PayloadIndex::Float(map) => {
                if let Some(num) = value.as_f64() {
                    Ok(map
                        .range(..=OrderedFloat(num))
                        .flat_map(|(_, ids)| ids)
                        .cloned()
                        .collect())
                } else {
                    Ok(HashSet::new())
                }
            }
            _ => Err(FilterError::InvalidIndexType(field.to_string())),
        }
    }

    fn evaluate_range(
        &self,
        field: &str,
        gte: Option<&Value>,
        lte: Option<&Value>,
    ) -> Result<HashSet<String>> {
        let mut result = self.get_all_ids_for_field(field)?;

        if let Some(gte_val) = gte {
            let gte_results = self.evaluate_gte(field, gte_val)?;
            result = result.intersection(&gte_results).cloned().collect();
        }

        if let Some(lte_val) = lte {
            let lte_results = self.evaluate_lte(field, lte_val)?;
            result = result.intersection(&lte_results).cloned().collect();
        }

        Ok(result)
    }

    fn evaluate_in(&self, field: &str, values: &[Value]) -> Result<HashSet<String>> {
        let mut result = HashSet::new();
        for value in values {
            let ids = self.evaluate_eq(field, value)?;
            result.extend(ids);
        }
        Ok(result)
    }

    fn evaluate_match(&self, field: &str, text: &str) -> Result<HashSet<String>> {
        let index = self
            .indices
            .get_index(field)
            .ok_or_else(|| FilterError::IndexNotFound(field.to_string()))?;

        match index {
            PayloadIndex::Text(map) => {
                let words: Vec<_> = text.split_whitespace().map(|w| w.to_lowercase()).collect();
                let mut result = HashSet::new();
                for word in words {
                    if let Some(ids) = map.get(&word) {
                        result.extend(ids.iter().cloned());
                    }
                }
                Ok(result)
            }
            _ => Err(FilterError::InvalidIndexType(field.to_string())),
        }
    }

    fn evaluate_geo_radius(
        &self,
        field: &str,
        lat: f64,
        lon: f64,
        radius_m: f64,
    ) -> Result<HashSet<String>> {
        let index = self
            .indices
            .get_index(field)
            .ok_or_else(|| FilterError::IndexNotFound(field.to_string()))?;

        match index {
            PayloadIndex::Geo(points) => {
                let mut result = HashSet::new();
                for (id, point_lat, point_lon) in points {
                    let distance = haversine_distance(lat, lon, *point_lat, *point_lon);
                    if distance <= radius_m {
                        result.insert(id.clone());
                    }
                }
                Ok(result)
            }
            _ => Err(FilterError::InvalidIndexType(field.to_string())),
        }
    }

    fn evaluate_geo_bbox(
        &self,
        field: &str,
        top_left: (f64, f64),
        bottom_right: (f64, f64),
    ) -> Result<HashSet<String>> {
        let index = self
            .indices
            .get_index(field)
            .ok_or_else(|| FilterError::IndexNotFound(field.to_string()))?;

        match index {
            PayloadIndex::Geo(points) => {
                let mut result = HashSet::new();
                let (north, west) = top_left;
                let (south, east) = bottom_right;

                for (id, lat, lon) in points {
                    if *lat <= north && *lat >= south && *lon >= west && *lon <= east {
                        result.insert(id.clone());
                    }
                }
                Ok(result)
            }
            _ => Err(FilterError::InvalidIndexType(field.to_string())),
        }
    }

    fn evaluate_and(&self, filters: &[FilterExpression]) -> Result<HashSet<String>> {
        if filters.is_empty() {
            return Ok(HashSet::new());
        }

        let mut result = self.evaluate(&filters[0])?;
        for filter in &filters[1..] {
            let next = self.evaluate(filter)?;
            result = result.intersection(&next).cloned().collect();
            if result.is_empty() {
                break;
            }
        }
        Ok(result)
    }

    fn evaluate_or(&self, filters: &[FilterExpression]) -> Result<HashSet<String>> {
        let mut result = HashSet::new();
        for filter in filters {
            let next = self.evaluate(filter)?;
            result.extend(next);
        }
        Ok(result)
    }

    fn evaluate_not(&self, filter: &FilterExpression) -> Result<HashSet<String>> {
        let filter_results = self.evaluate(filter)?;
        let fields = filter.get_fields();
        let mut all_ids = HashSet::new();

        for field in fields {
            all_ids.extend(self.get_all_ids_for_field(&field)?);
        }

        Ok(all_ids.difference(&filter_results).cloned().collect())
    }

    fn evaluate_exists(&self, field: &str) -> Result<HashSet<String>> {
        self.get_all_ids_for_field(field)
    }

    fn evaluate_is_null(&self, _field: &str) -> Result<HashSet<String>> {
        // This would require tracking null values separately
        // For now, return empty set
        Ok(HashSet::new())
    }

    fn get_all_ids_for_field(&self, field: &str) -> Result<HashSet<String>> {
        let index = self
            .indices
            .get_index(field)
            .ok_or_else(|| FilterError::IndexNotFound(field.to_string()))?;

        let ids = match index {
            PayloadIndex::Integer(map) => map.values().flatten().cloned().collect(),
            PayloadIndex::Float(map) => map.values().flatten().cloned().collect(),
            PayloadIndex::Keyword(map) => map.values().flatten().cloned().collect(),
            PayloadIndex::Bool(map) => map.values().flatten().cloned().collect(),
            PayloadIndex::Geo(points) => points.iter().map(|(id, _, _)| id.clone()).collect(),
            PayloadIndex::Text(map) => map.values().flatten().cloned().collect(),
        };

        Ok(ids)
    }

    fn get_field_value<'b>(payload: &'b Value, field: &str) -> Option<&'b Value> {
        payload.as_object()?.get(field)
    }

    fn compare_values(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
        match (a, b) {
            (Value::Number(a), Value::Number(b)) => {
                let a = a.as_f64()?;
                let b = b.as_f64()?;
                a.partial_cmp(&b)
            }
            (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }
}

/// Calculate haversine distance between two points in meters
fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_M: f64 = 6_371_000.0; // Earth's radius in meters

    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lon = (lon2 - lon1).to_radians();

    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    EARTH_RADIUS_M * c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::IndexType;
    use serde_json::json;

    #[test]
    fn test_eq_filter() {
        let mut manager = PayloadIndexManager::new();
        manager.create_index("status", IndexType::Keyword).unwrap();

        manager
            .index_payload("v1", &json!({"status": "active"}))
            .unwrap();
        manager
            .index_payload("v2", &json!({"status": "active"}))
            .unwrap();
        manager
            .index_payload("v3", &json!({"status": "inactive"}))
            .unwrap();

        let evaluator = FilterEvaluator::new(&manager);
        let filter = FilterExpression::eq("status", json!("active"));
        let results = evaluator.evaluate(&filter).unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.contains("v1"));
        assert!(results.contains("v2"));
    }

    #[test]
    fn test_range_filter() {
        let mut manager = PayloadIndexManager::new();
        manager.create_index("age", IndexType::Integer).unwrap();

        manager.index_payload("v1", &json!({"age": 25})).unwrap();
        manager.index_payload("v2", &json!({"age": 30})).unwrap();
        manager.index_payload("v3", &json!({"age": 35})).unwrap();

        let evaluator = FilterEvaluator::new(&manager);
        let filter = FilterExpression::range("age", Some(json!(25)), Some(json!(30)));
        let results = evaluator.evaluate(&filter).unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.contains("v1"));
        assert!(results.contains("v2"));
    }

    #[test]
    fn test_and_filter() {
        let mut manager = PayloadIndexManager::new();
        manager.create_index("age", IndexType::Integer).unwrap();
        manager.create_index("status", IndexType::Keyword).unwrap();

        manager
            .index_payload("v1", &json!({"age": 25, "status": "active"}))
            .unwrap();
        manager
            .index_payload("v2", &json!({"age": 30, "status": "active"}))
            .unwrap();
        manager
            .index_payload("v3", &json!({"age": 25, "status": "inactive"}))
            .unwrap();

        let evaluator = FilterEvaluator::new(&manager);
        let filter = FilterExpression::and(vec![
            FilterExpression::eq("age", json!(25)),
            FilterExpression::eq("status", json!("active")),
        ]);
        let results = evaluator.evaluate(&filter).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results.contains("v1"));
    }

    #[test]
    fn test_matches_payload() {
        let manager = PayloadIndexManager::new();
        let evaluator = FilterEvaluator::new(&manager);

        let payload = json!({
            "age": 25,
            "status": "active",
            "name": "Alice"
        });

        assert!(evaluator.matches(&payload, &FilterExpression::eq("age", json!(25))));
        assert!(evaluator.matches(&payload, &FilterExpression::eq("status", json!("active"))));
        assert!(!evaluator.matches(&payload, &FilterExpression::eq("age", json!(30))));
    }

    #[test]
    fn test_haversine_distance() {
        // New York to Los Angeles (approx 3935 km)
        let distance = haversine_distance(40.7128, -74.0060, 34.0522, -118.2437);
        assert!((distance - 3_935_000.0).abs() < 50_000.0); // Within 50km tolerance
    }
}
