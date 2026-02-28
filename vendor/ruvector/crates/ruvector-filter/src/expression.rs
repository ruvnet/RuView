use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Filter expression for querying vectors by payload
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FilterExpression {
    // Comparison operators
    Eq {
        field: String,
        value: Value,
    },
    Ne {
        field: String,
        value: Value,
    },
    Gt {
        field: String,
        value: Value,
    },
    Gte {
        field: String,
        value: Value,
    },
    Lt {
        field: String,
        value: Value,
    },
    Lte {
        field: String,
        value: Value,
    },

    // Range
    Range {
        field: String,
        gte: Option<Value>,
        lte: Option<Value>,
    },

    // Array operations
    In {
        field: String,
        values: Vec<Value>,
    },

    // Text matching
    Match {
        field: String,
        text: String,
    },

    // Geo operations (basic)
    GeoRadius {
        field: String,
        lat: f64,
        lon: f64,
        radius_m: f64,
    },
    GeoBoundingBox {
        field: String,
        top_left: (f64, f64),
        bottom_right: (f64, f64),
    },

    // Logical operators
    And(Vec<FilterExpression>),
    Or(Vec<FilterExpression>),
    Not(Box<FilterExpression>),

    // Existence check
    Exists {
        field: String,
    },
    IsNull {
        field: String,
    },
}

impl FilterExpression {
    /// Create an equality filter
    pub fn eq(field: impl Into<String>, value: Value) -> Self {
        Self::Eq {
            field: field.into(),
            value,
        }
    }

    /// Create a not-equal filter
    pub fn ne(field: impl Into<String>, value: Value) -> Self {
        Self::Ne {
            field: field.into(),
            value,
        }
    }

    /// Create a greater-than filter
    pub fn gt(field: impl Into<String>, value: Value) -> Self {
        Self::Gt {
            field: field.into(),
            value,
        }
    }

    /// Create a greater-than-or-equal filter
    pub fn gte(field: impl Into<String>, value: Value) -> Self {
        Self::Gte {
            field: field.into(),
            value,
        }
    }

    /// Create a less-than filter
    pub fn lt(field: impl Into<String>, value: Value) -> Self {
        Self::Lt {
            field: field.into(),
            value,
        }
    }

    /// Create a less-than-or-equal filter
    pub fn lte(field: impl Into<String>, value: Value) -> Self {
        Self::Lte {
            field: field.into(),
            value,
        }
    }

    /// Create a range filter
    pub fn range(field: impl Into<String>, gte: Option<Value>, lte: Option<Value>) -> Self {
        Self::Range {
            field: field.into(),
            gte,
            lte,
        }
    }

    /// Create an IN filter
    pub fn in_values(field: impl Into<String>, values: Vec<Value>) -> Self {
        Self::In {
            field: field.into(),
            values,
        }
    }

    /// Create a text match filter
    pub fn match_text(field: impl Into<String>, text: impl Into<String>) -> Self {
        Self::Match {
            field: field.into(),
            text: text.into(),
        }
    }

    /// Create a geo radius filter
    pub fn geo_radius(field: impl Into<String>, lat: f64, lon: f64, radius_m: f64) -> Self {
        Self::GeoRadius {
            field: field.into(),
            lat,
            lon,
            radius_m,
        }
    }

    /// Create a geo bounding box filter
    pub fn geo_bounding_box(
        field: impl Into<String>,
        top_left: (f64, f64),
        bottom_right: (f64, f64),
    ) -> Self {
        Self::GeoBoundingBox {
            field: field.into(),
            top_left,
            bottom_right,
        }
    }

    /// Create an AND filter
    pub fn and(filters: Vec<FilterExpression>) -> Self {
        Self::And(filters)
    }

    /// Create an OR filter
    pub fn or(filters: Vec<FilterExpression>) -> Self {
        Self::Or(filters)
    }

    /// Create a NOT filter
    pub fn not(filter: FilterExpression) -> Self {
        Self::Not(Box::new(filter))
    }

    /// Create an EXISTS filter
    pub fn exists(field: impl Into<String>) -> Self {
        Self::Exists {
            field: field.into(),
        }
    }

    /// Create an IS NULL filter
    pub fn is_null(field: impl Into<String>) -> Self {
        Self::IsNull {
            field: field.into(),
        }
    }

    /// Get all field names referenced in this expression
    pub fn get_fields(&self) -> Vec<String> {
        let mut fields = Vec::new();
        self.collect_fields(&mut fields);
        fields.sort();
        fields.dedup();
        fields
    }

    fn collect_fields(&self, fields: &mut Vec<String>) {
        match self {
            Self::Eq { field, .. }
            | Self::Ne { field, .. }
            | Self::Gt { field, .. }
            | Self::Gte { field, .. }
            | Self::Lt { field, .. }
            | Self::Lte { field, .. }
            | Self::Range { field, .. }
            | Self::In { field, .. }
            | Self::Match { field, .. }
            | Self::GeoRadius { field, .. }
            | Self::GeoBoundingBox { field, .. }
            | Self::Exists { field }
            | Self::IsNull { field } => {
                fields.push(field.clone());
            }
            Self::And(exprs) | Self::Or(exprs) => {
                for expr in exprs {
                    expr.collect_fields(fields);
                }
            }
            Self::Not(expr) => {
                expr.collect_fields(fields);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_filter_builders() {
        let filter = FilterExpression::eq("status", json!("active"));
        assert!(matches!(filter, FilterExpression::Eq { .. }));

        let filter = FilterExpression::and(vec![
            FilterExpression::eq("status", json!("active")),
            FilterExpression::gte("age", json!(18)),
        ]);
        assert!(matches!(filter, FilterExpression::And(_)));
    }

    #[test]
    fn test_get_fields() {
        let filter = FilterExpression::and(vec![
            FilterExpression::eq("status", json!("active")),
            FilterExpression::or(vec![
                FilterExpression::gte("age", json!(18)),
                FilterExpression::lt("score", json!(100)),
            ]),
        ]);

        let fields = filter.get_fields();
        assert_eq!(fields, vec!["age", "score", "status"]);
    }

    #[test]
    fn test_serialization() {
        let filter = FilterExpression::eq("status", json!("active"));
        let json = serde_json::to_string(&filter).unwrap();
        let deserialized: FilterExpression = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, FilterExpression::Eq { .. }));
    }
}
