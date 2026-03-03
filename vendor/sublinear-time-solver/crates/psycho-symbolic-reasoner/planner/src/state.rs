use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(target_arch = "wasm32")]
fn wasm_compatible_timestamp() -> u64 {
    // For WASM, use a simple counter or js Date
    use js_sys::Date;
    Date::now() as u64 / 1000
}

#[cfg(not(target_arch = "wasm32"))]
fn wasm_compatible_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum StateValue {
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<StateValue>),
    Object(HashMap<String, StateValue>),
}

impl StateValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            StateValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<i64> {
        match self {
            StateValue::Integer(i) => Some(*i),
            StateValue::Float(f) => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            StateValue::Float(f) => Some(*f),
            StateValue::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&String> {
        match self {
            StateValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&Vec<StateValue>> {
        match self {
            StateValue::List(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&HashMap<String, StateValue>> {
        match self {
            StateValue::Object(o) => Some(o),
            _ => None,
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            StateValue::Boolean(b) => *b,
            StateValue::Integer(i) => *i != 0,
            StateValue::Float(f) => *f != 0.0,
            StateValue::String(s) => !s.is_empty(),
            StateValue::List(l) => !l.is_empty(),
            StateValue::Object(o) => !o.is_empty(),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            StateValue::Boolean(_) => "boolean",
            StateValue::Integer(_) => "integer",
            StateValue::Float(_) => "float",
            StateValue::String(_) => "string",
            StateValue::List(_) => "list",
            StateValue::Object(_) => "object",
        }
    }
}

impl From<bool> for StateValue {
    fn from(value: bool) -> Self {
        StateValue::Boolean(value)
    }
}

impl From<i64> for StateValue {
    fn from(value: i64) -> Self {
        StateValue::Integer(value)
    }
}

impl From<f64> for StateValue {
    fn from(value: f64) -> Self {
        StateValue::Float(value)
    }
}

impl From<String> for StateValue {
    fn from(value: String) -> Self {
        StateValue::String(value)
    }
}

impl From<&str> for StateValue {
    fn from(value: &str) -> Self {
        StateValue::String(value.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldState {
    states: IndexMap<String, StateValue>,
    timestamp: u64,
    version: u32,
}

impl WorldState {
    pub fn new() -> Self {
        Self {
            states: IndexMap::new(),
            timestamp: wasm_compatible_timestamp(),
            version: 0,
        }
    }

    pub fn set_state(&mut self, key: &str, value: StateValue) {
        self.states.insert(key.to_string(), value);
        self.version += 1;
        self.update_timestamp();
    }

    pub fn get_state(&self, key: &str) -> Option<&StateValue> {
        self.states.get(key)
    }

    pub fn remove_state(&mut self, key: &str) -> Option<StateValue> {
        self.version += 1;
        self.update_timestamp();
        self.states.remove(key)
    }

    pub fn has_state(&self, key: &str) -> bool {
        self.states.contains_key(key)
    }

    pub fn get_all_states(&self) -> &IndexMap<String, StateValue> {
        &self.states
    }

    pub fn clear(&mut self) {
        self.states.clear();
        self.version += 1;
        self.update_timestamp();
    }

    pub fn merge(&mut self, other: &WorldState) {
        for (key, value) in &other.states {
            self.states.insert(key.clone(), value.clone());
        }
        self.version += 1;
        self.update_timestamp();
    }

    pub fn diff(&self, other: &WorldState) -> Vec<StateDifference> {
        let mut differences = Vec::new();

        // Check for changes and additions
        for (key, value) in &other.states {
            match self.states.get(key) {
                Some(existing_value) => {
                    if existing_value != value {
                        differences.push(StateDifference {
                            key: key.clone(),
                            change_type: ChangeType::Modified,
                            old_value: Some(existing_value.clone()),
                            new_value: Some(value.clone()),
                        });
                    }
                }
                None => {
                    differences.push(StateDifference {
                        key: key.clone(),
                        change_type: ChangeType::Added,
                        old_value: None,
                        new_value: Some(value.clone()),
                    });
                }
            }
        }

        // Check for removals
        for (key, value) in &self.states {
            if !other.states.contains_key(key) {
                differences.push(StateDifference {
                    key: key.clone(),
                    change_type: ChangeType::Removed,
                    old_value: Some(value.clone()),
                    new_value: None,
                });
            }
        }

        differences
    }

    pub fn satisfies_condition(&self, key: &str, expected_value: &StateValue) -> bool {
        match self.get_state(key) {
            Some(actual_value) => actual_value == expected_value,
            None => false,
        }
    }

    pub fn satisfies_conditions(&self, conditions: &[(String, StateValue)]) -> bool {
        conditions
            .iter()
            .all(|(key, value)| self.satisfies_condition(key, value))
    }

    pub fn distance_to(&self, target: &WorldState) -> f64 {
        let mut distance = 0.0;
        let mut compared_keys = std::collections::HashSet::new();

        // Compare existing states
        for (key, target_value) in &target.states {
            compared_keys.insert(key.clone());
            match self.states.get(key) {
                Some(current_value) => {
                    if current_value != target_value {
                        distance += self.value_distance(current_value, target_value);
                    }
                }
                None => {
                    distance += 1.0; // Missing state
                }
            }
        }

        // Add distance for extra states in current
        for key in self.states.keys() {
            if !compared_keys.contains(key) {
                distance += 0.5; // Penalty for extra state
            }
        }

        distance
    }

    fn value_distance(&self, a: &StateValue, b: &StateValue) -> f64 {
        match (a, b) {
            (StateValue::Boolean(a), StateValue::Boolean(b)) => {
                if a == b { 0.0 } else { 1.0 }
            }
            (StateValue::Integer(a), StateValue::Integer(b)) => {
                ((*a - *b).abs() as f64).min(10.0) / 10.0
            }
            (StateValue::Float(a), StateValue::Float(b)) => {
                ((a - b).abs()).min(10.0) / 10.0
            }
            (StateValue::String(a), StateValue::String(b)) => {
                if a == b { 0.0 } else { 1.0 }
            }
            (StateValue::List(a), StateValue::List(b)) => {
                let len_diff = (a.len() as i32 - b.len() as i32).abs() as f64;
                let content_diff = a.iter().zip(b.iter())
                    .map(|(av, bv)| self.value_distance(av, bv))
                    .sum::<f64>();
                (len_diff + content_diff) / (a.len().max(b.len()).max(1) as f64)
            }
            _ => 1.0, // Different types
        }
    }

    pub fn get_timestamp(&self) -> u64 {
        self.timestamp
    }

    pub fn get_version(&self) -> u32 {
        self.version
    }

    fn update_timestamp(&mut self) {
        self.timestamp = wasm_compatible_timestamp();
    }

    pub fn to_compact_string(&self) -> String {
        let mut parts = Vec::new();
        for (key, value) in &self.states {
            let value_str = match value {
                StateValue::Boolean(b) => b.to_string(),
                StateValue::Integer(i) => i.to_string(),
                StateValue::Float(f) => f.to_string(),
                StateValue::String(s) => format!("\"{}\"", s),
                StateValue::List(_) => "[...]".to_string(),
                StateValue::Object(_) => "{...}".to_string(),
            };
            parts.push(format!("{}:{}", key, value_str));
        }
        format!("{{{}}}", parts.join(","))
    }

    pub fn validate(&self) -> Result<(), StateValidationError> {
        // Check for empty keys
        for key in self.states.keys() {
            if key.is_empty() {
                return Err(StateValidationError::EmptyKey);
            }
        }

        // Check for circular references in objects (simplified check)
        for (key, value) in &self.states {
            if let StateValue::Object(obj) = value {
                if obj.contains_key(key) {
                    return Err(StateValidationError::CircularReference(key.clone()));
                }
            }
        }

        Ok(())
    }
}

impl Default for WorldState {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for WorldState {
    fn eq(&self, other: &Self) -> bool {
        self.states == other.states
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDifference {
    pub key: String,
    pub change_type: ChangeType,
    pub old_value: Option<StateValue>,
    pub new_value: Option<StateValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Added,
    Modified,
    Removed,
}

#[derive(Debug, thiserror::Error)]
pub enum StateValidationError {
    #[error("Empty state key is not allowed")]
    EmptyKey,
    #[error("Circular reference detected in state: {0}")]
    CircularReference(String),
    #[error("Invalid state value: {0}")]
    InvalidValue(String),
}

// State query functionality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateQuery {
    pub conditions: Vec<StateCondition>,
    pub operator: LogicalOperator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateCondition {
    pub key: String,
    pub operator: ComparisonOperator,
    pub value: StateValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogicalOperator {
    And,
    Or,
    Not,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    Contains,
    StartsWith,
    EndsWith,
}

impl StateQuery {
    pub fn new() -> Self {
        Self {
            conditions: Vec::new(),
            operator: LogicalOperator::And,
        }
    }

    pub fn add_condition(mut self, condition: StateCondition) -> Self {
        self.conditions.push(condition);
        self
    }

    pub fn with_operator(mut self, operator: LogicalOperator) -> Self {
        self.operator = operator;
        self
    }

    pub fn evaluate(&self, state: &WorldState) -> bool {
        if self.conditions.is_empty() {
            return true;
        }

        let results: Vec<bool> = self.conditions
            .iter()
            .map(|condition| condition.evaluate(state))
            .collect();

        match self.operator {
            LogicalOperator::And => results.iter().all(|&x| x),
            LogicalOperator::Or => results.iter().any(|&x| x),
            LogicalOperator::Not => !results.iter().all(|&x| x),
        }
    }
}

impl StateCondition {
    pub fn new(key: &str, operator: ComparisonOperator, value: StateValue) -> Self {
        Self {
            key: key.to_string(),
            operator,
            value,
        }
    }

    pub fn evaluate(&self, state: &WorldState) -> bool {
        match state.get_state(&self.key) {
            Some(actual_value) => self.compare_values(actual_value, &self.value),
            None => false,
        }
    }

    fn compare_values(&self, actual: &StateValue, expected: &StateValue) -> bool {
        match self.operator {
            ComparisonOperator::Equal => actual == expected,
            ComparisonOperator::NotEqual => actual != expected,
            ComparisonOperator::GreaterThan => {
                self.numeric_comparison(actual, expected, |a, b| a > b)
            }
            ComparisonOperator::LessThan => {
                self.numeric_comparison(actual, expected, |a, b| a < b)
            }
            ComparisonOperator::GreaterThanOrEqual => {
                self.numeric_comparison(actual, expected, |a, b| a >= b)
            }
            ComparisonOperator::LessThanOrEqual => {
                self.numeric_comparison(actual, expected, |a, b| a <= b)
            }
            ComparisonOperator::Contains => {
                self.string_operation(actual, expected, |a, b| a.contains(b))
            }
            ComparisonOperator::StartsWith => {
                self.string_operation(actual, expected, |a, b| a.starts_with(b))
            }
            ComparisonOperator::EndsWith => {
                self.string_operation(actual, expected, |a, b| a.ends_with(b))
            }
        }
    }

    fn numeric_comparison<F>(&self, actual: &StateValue, expected: &StateValue, op: F) -> bool
    where
        F: Fn(f64, f64) -> bool,
    {
        match (actual.as_float(), expected.as_float()) {
            (Some(a), Some(b)) => op(a, b),
            _ => false,
        }
    }

    fn string_operation<F>(&self, actual: &StateValue, expected: &StateValue, op: F) -> bool
    where
        F: Fn(&str, &str) -> bool,
    {
        match (actual.as_string(), expected.as_string()) {
            (Some(a), Some(b)) => op(a, b),
            _ => false,
        }
    }
}

impl Default for StateQuery {
    fn default() -> Self {
        Self::new()
    }
}

// State manipulation utilities
pub struct StateBuilder {
    state: WorldState,
}

impl StateBuilder {
    pub fn new() -> Self {
        Self {
            state: WorldState::new(),
        }
    }

    pub fn with_bool(mut self, key: &str, value: bool) -> Self {
        self.state.set_state(key, StateValue::Boolean(value));
        self
    }

    pub fn with_int(mut self, key: &str, value: i64) -> Self {
        self.state.set_state(key, StateValue::Integer(value));
        self
    }

    pub fn with_float(mut self, key: &str, value: f64) -> Self {
        self.state.set_state(key, StateValue::Float(value));
        self
    }

    pub fn with_string(mut self, key: &str, value: &str) -> Self {
        self.state.set_state(key, StateValue::String(value.to_string()));
        self
    }

    pub fn with_state(mut self, key: &str, value: StateValue) -> Self {
        self.state.set_state(key, value);
        self
    }

    pub fn build(self) -> WorldState {
        self.state
    }
}

impl Default for StateBuilder {
    fn default() -> Self {
        Self::new()
    }
}