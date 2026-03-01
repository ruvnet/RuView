//! Semantic analysis and type checking for Cypher queries
//!
//! Validates the semantic correctness of parsed Cypher queries including:
//! - Variable scope checking
//! - Type compatibility validation
//! - Aggregation context verification
//! - Pattern validity

use super::ast::*;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SemanticError {
    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),

    #[error("Variable already defined: {0}")]
    VariableAlreadyDefined(String),

    #[error("Type mismatch: expected {expected}, found {found}")]
    TypeMismatch { expected: String, found: String },

    #[error("Aggregation not allowed in {0}")]
    InvalidAggregation(String),

    #[error("Cannot mix aggregated and non-aggregated expressions")]
    MixedAggregation,

    #[error("Invalid pattern: {0}")]
    InvalidPattern(String),

    #[error("Invalid hyperedge: {0}")]
    InvalidHyperedge(String),

    #[error("Property access on non-object type")]
    InvalidPropertyAccess,

    #[error(
        "Invalid number of arguments for function {function}: expected {expected}, found {found}"
    )]
    InvalidArgumentCount {
        function: String,
        expected: usize,
        found: usize,
    },
}

type SemanticResult<T> = Result<T, SemanticError>;

/// Semantic analyzer for Cypher queries
pub struct SemanticAnalyzer {
    scope_stack: Vec<Scope>,
    in_aggregation: bool,
}

#[derive(Debug, Clone)]
struct Scope {
    variables: HashMap<String, ValueType>,
}

/// Type system for Cypher values
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueType {
    Integer,
    Float,
    String,
    Boolean,
    Null,
    Node,
    Relationship,
    Path,
    List(Box<ValueType>),
    Map,
    Any,
}

impl ValueType {
    /// Check if this type is compatible with another type
    pub fn is_compatible_with(&self, other: &ValueType) -> bool {
        match (self, other) {
            (ValueType::Any, _) | (_, ValueType::Any) => true,
            (ValueType::Null, _) | (_, ValueType::Null) => true,
            (ValueType::Integer, ValueType::Float) | (ValueType::Float, ValueType::Integer) => true,
            (ValueType::List(a), ValueType::List(b)) => a.is_compatible_with(b),
            _ => self == other,
        }
    }

    /// Check if this is a numeric type
    pub fn is_numeric(&self) -> bool {
        matches!(self, ValueType::Integer | ValueType::Float | ValueType::Any)
    }

    /// Check if this is a graph element (node, relationship, path)
    pub fn is_graph_element(&self) -> bool {
        matches!(
            self,
            ValueType::Node | ValueType::Relationship | ValueType::Path | ValueType::Any
        )
    }
}

impl Scope {
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }

    fn define(&mut self, name: String, value_type: ValueType) -> SemanticResult<()> {
        if self.variables.contains_key(&name) {
            Err(SemanticError::VariableAlreadyDefined(name))
        } else {
            self.variables.insert(name, value_type);
            Ok(())
        }
    }

    fn get(&self, name: &str) -> Option<&ValueType> {
        self.variables.get(name)
    }
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            scope_stack: vec![Scope::new()],
            in_aggregation: false,
        }
    }

    fn current_scope(&self) -> &Scope {
        self.scope_stack.last().unwrap()
    }

    fn current_scope_mut(&mut self) -> &mut Scope {
        self.scope_stack.last_mut().unwrap()
    }

    fn push_scope(&mut self) {
        self.scope_stack.push(Scope::new());
    }

    fn pop_scope(&mut self) {
        self.scope_stack.pop();
    }

    fn lookup_variable(&self, name: &str) -> SemanticResult<&ValueType> {
        for scope in self.scope_stack.iter().rev() {
            if let Some(value_type) = scope.get(name) {
                return Ok(value_type);
            }
        }
        Err(SemanticError::UndefinedVariable(name.to_string()))
    }

    fn define_variable(&mut self, name: String, value_type: ValueType) -> SemanticResult<()> {
        self.current_scope_mut().define(name, value_type)
    }

    /// Analyze a complete query
    pub fn analyze_query(&mut self, query: &Query) -> SemanticResult<()> {
        for statement in &query.statements {
            self.analyze_statement(statement)?;
        }
        Ok(())
    }

    fn analyze_statement(&mut self, statement: &Statement) -> SemanticResult<()> {
        match statement {
            Statement::Match(clause) => self.analyze_match(clause),
            Statement::Create(clause) => self.analyze_create(clause),
            Statement::Merge(clause) => self.analyze_merge(clause),
            Statement::Delete(clause) => self.analyze_delete(clause),
            Statement::Set(clause) => self.analyze_set(clause),
            Statement::Remove(clause) => self.analyze_remove(clause),
            Statement::Return(clause) => self.analyze_return(clause),
            Statement::With(clause) => self.analyze_with(clause),
        }
    }

    fn analyze_remove(&mut self, clause: &RemoveClause) -> SemanticResult<()> {
        for item in &clause.items {
            match item {
                RemoveItem::Property { variable, .. } => {
                    // Verify variable is defined
                    self.lookup_variable(variable)?;
                }
                RemoveItem::Labels { variable, .. } => {
                    // Verify variable is defined
                    self.lookup_variable(variable)?;
                }
            }
        }
        Ok(())
    }

    fn analyze_match(&mut self, clause: &MatchClause) -> SemanticResult<()> {
        // Analyze patterns and define variables
        for pattern in &clause.patterns {
            self.analyze_pattern(pattern)?;
        }

        // Analyze WHERE clause
        if let Some(where_clause) = &clause.where_clause {
            let expr_type = self.analyze_expression(&where_clause.condition)?;
            if !expr_type.is_compatible_with(&ValueType::Boolean) {
                return Err(SemanticError::TypeMismatch {
                    expected: "Boolean".to_string(),
                    found: format!("{:?}", expr_type),
                });
            }
        }

        Ok(())
    }

    fn analyze_pattern(&mut self, pattern: &Pattern) -> SemanticResult<()> {
        match pattern {
            Pattern::Node(node) => self.analyze_node_pattern(node),
            Pattern::Relationship(rel) => self.analyze_relationship_pattern(rel),
            Pattern::Path(path) => self.analyze_path_pattern(path),
            Pattern::Hyperedge(hyperedge) => self.analyze_hyperedge_pattern(hyperedge),
        }
    }

    fn analyze_node_pattern(&mut self, node: &NodePattern) -> SemanticResult<()> {
        if let Some(variable) = &node.variable {
            self.define_variable(variable.clone(), ValueType::Node)?;
        }

        if let Some(properties) = &node.properties {
            for expr in properties.values() {
                self.analyze_expression(expr)?;
            }
        }

        Ok(())
    }

    fn analyze_relationship_pattern(&mut self, rel: &RelationshipPattern) -> SemanticResult<()> {
        self.analyze_node_pattern(&rel.from)?;
        // rel.to is now a Pattern (can be NodePattern or chained RelationshipPattern)
        self.analyze_pattern(&*rel.to)?;

        if let Some(variable) = &rel.variable {
            self.define_variable(variable.clone(), ValueType::Relationship)?;
        }

        if let Some(properties) = &rel.properties {
            for expr in properties.values() {
                self.analyze_expression(expr)?;
            }
        }

        // Validate range if present
        if let Some(range) = &rel.range {
            if let (Some(min), Some(max)) = (range.min, range.max) {
                if min > max {
                    return Err(SemanticError::InvalidPattern(
                        "Minimum range cannot be greater than maximum".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    fn analyze_path_pattern(&mut self, path: &PathPattern) -> SemanticResult<()> {
        self.define_variable(path.variable.clone(), ValueType::Path)?;
        self.analyze_pattern(&path.pattern)
    }

    fn analyze_hyperedge_pattern(&mut self, hyperedge: &HyperedgePattern) -> SemanticResult<()> {
        // Validate hyperedge has at least 2 target nodes
        if hyperedge.to.len() < 2 {
            return Err(SemanticError::InvalidHyperedge(
                "Hyperedge must have at least 2 target nodes".to_string(),
            ));
        }

        // Validate arity matches
        if hyperedge.arity != hyperedge.to.len() + 1 {
            return Err(SemanticError::InvalidHyperedge(
                "Hyperedge arity doesn't match number of participating nodes".to_string(),
            ));
        }

        self.analyze_node_pattern(&hyperedge.from)?;

        for target in &hyperedge.to {
            self.analyze_node_pattern(target)?;
        }

        if let Some(variable) = &hyperedge.variable {
            self.define_variable(variable.clone(), ValueType::Relationship)?;
        }

        if let Some(properties) = &hyperedge.properties {
            for expr in properties.values() {
                self.analyze_expression(expr)?;
            }
        }

        Ok(())
    }

    fn analyze_create(&mut self, clause: &CreateClause) -> SemanticResult<()> {
        for pattern in &clause.patterns {
            self.analyze_pattern(pattern)?;
        }
        Ok(())
    }

    fn analyze_merge(&mut self, clause: &MergeClause) -> SemanticResult<()> {
        self.analyze_pattern(&clause.pattern)?;

        if let Some(on_create) = &clause.on_create {
            self.analyze_set(on_create)?;
        }

        if let Some(on_match) = &clause.on_match {
            self.analyze_set(on_match)?;
        }

        Ok(())
    }

    fn analyze_delete(&mut self, clause: &DeleteClause) -> SemanticResult<()> {
        for expr in &clause.expressions {
            let expr_type = self.analyze_expression(expr)?;
            if !expr_type.is_graph_element() {
                return Err(SemanticError::TypeMismatch {
                    expected: "graph element (node, relationship, path)".to_string(),
                    found: format!("{:?}", expr_type),
                });
            }
        }
        Ok(())
    }

    fn analyze_set(&mut self, clause: &SetClause) -> SemanticResult<()> {
        for item in &clause.items {
            match item {
                SetItem::Property {
                    variable, value, ..
                } => {
                    self.lookup_variable(variable)?;
                    self.analyze_expression(value)?;
                }
                SetItem::Variable { variable, value } => {
                    self.lookup_variable(variable)?;
                    self.analyze_expression(value)?;
                }
                SetItem::Labels { variable, .. } => {
                    self.lookup_variable(variable)?;
                }
            }
        }
        Ok(())
    }

    fn analyze_return(&mut self, clause: &ReturnClause) -> SemanticResult<()> {
        self.analyze_return_items(&clause.items)?;

        if let Some(order_by) = &clause.order_by {
            for item in &order_by.items {
                self.analyze_expression(&item.expression)?;
            }
        }

        if let Some(skip) = &clause.skip {
            let skip_type = self.analyze_expression(skip)?;
            if !skip_type.is_compatible_with(&ValueType::Integer) {
                return Err(SemanticError::TypeMismatch {
                    expected: "Integer".to_string(),
                    found: format!("{:?}", skip_type),
                });
            }
        }

        if let Some(limit) = &clause.limit {
            let limit_type = self.analyze_expression(limit)?;
            if !limit_type.is_compatible_with(&ValueType::Integer) {
                return Err(SemanticError::TypeMismatch {
                    expected: "Integer".to_string(),
                    found: format!("{:?}", limit_type),
                });
            }
        }

        Ok(())
    }

    fn analyze_with(&mut self, clause: &WithClause) -> SemanticResult<()> {
        self.analyze_return_items(&clause.items)?;

        if let Some(where_clause) = &clause.where_clause {
            let expr_type = self.analyze_expression(&where_clause.condition)?;
            if !expr_type.is_compatible_with(&ValueType::Boolean) {
                return Err(SemanticError::TypeMismatch {
                    expected: "Boolean".to_string(),
                    found: format!("{:?}", expr_type),
                });
            }
        }

        Ok(())
    }

    fn analyze_return_items(&mut self, items: &[ReturnItem]) -> SemanticResult<()> {
        let mut has_aggregation = false;
        let mut has_non_aggregation = false;

        for item in items {
            let item_has_agg = item.expression.has_aggregation();
            has_aggregation |= item_has_agg;
            has_non_aggregation |= !item_has_agg && !item.expression.is_constant();
        }

        if has_aggregation && has_non_aggregation {
            return Err(SemanticError::MixedAggregation);
        }

        for item in items {
            self.analyze_expression(&item.expression)?;
        }

        Ok(())
    }

    fn analyze_expression(&mut self, expr: &Expression) -> SemanticResult<ValueType> {
        match expr {
            Expression::Integer(_) => Ok(ValueType::Integer),
            Expression::Float(_) => Ok(ValueType::Float),
            Expression::String(_) => Ok(ValueType::String),
            Expression::Boolean(_) => Ok(ValueType::Boolean),
            Expression::Null => Ok(ValueType::Null),

            Expression::Variable(name) => {
                self.lookup_variable(name)?;
                Ok(ValueType::Any)
            }

            Expression::Property { object, .. } => {
                let obj_type = self.analyze_expression(object)?;
                if !obj_type.is_graph_element()
                    && obj_type != ValueType::Map
                    && obj_type != ValueType::Any
                {
                    return Err(SemanticError::InvalidPropertyAccess);
                }
                Ok(ValueType::Any)
            }

            Expression::List(items) => {
                if items.is_empty() {
                    Ok(ValueType::List(Box::new(ValueType::Any)))
                } else {
                    let first_type = self.analyze_expression(&items[0])?;
                    for item in items.iter().skip(1) {
                        let item_type = self.analyze_expression(item)?;
                        if !item_type.is_compatible_with(&first_type) {
                            return Ok(ValueType::List(Box::new(ValueType::Any)));
                        }
                    }
                    Ok(ValueType::List(Box::new(first_type)))
                }
            }

            Expression::Map(map) => {
                for expr in map.values() {
                    self.analyze_expression(expr)?;
                }
                Ok(ValueType::Map)
            }

            Expression::BinaryOp { left, op, right } => {
                let left_type = self.analyze_expression(left)?;
                let right_type = self.analyze_expression(right)?;

                match op {
                    BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::Power => {
                        if !left_type.is_numeric() || !right_type.is_numeric() {
                            return Err(SemanticError::TypeMismatch {
                                expected: "numeric".to_string(),
                                found: format!("{:?} and {:?}", left_type, right_type),
                            });
                        }
                        if left_type == ValueType::Float || right_type == ValueType::Float {
                            Ok(ValueType::Float)
                        } else {
                            Ok(ValueType::Integer)
                        }
                    }
                    BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::LessThan
                    | BinaryOperator::LessThanOrEqual
                    | BinaryOperator::GreaterThan
                    | BinaryOperator::GreaterThanOrEqual => Ok(ValueType::Boolean),
                    BinaryOperator::And | BinaryOperator::Or | BinaryOperator::Xor => {
                        Ok(ValueType::Boolean)
                    }
                    _ => Ok(ValueType::Any),
                }
            }

            Expression::UnaryOp { op, operand } => {
                let operand_type = self.analyze_expression(operand)?;
                match op {
                    UnaryOperator::Not | UnaryOperator::IsNull | UnaryOperator::IsNotNull => {
                        Ok(ValueType::Boolean)
                    }
                    UnaryOperator::Minus | UnaryOperator::Plus => Ok(operand_type),
                }
            }

            Expression::FunctionCall { args, .. } => {
                for arg in args {
                    self.analyze_expression(arg)?;
                }
                Ok(ValueType::Any)
            }

            Expression::Aggregation { expression, .. } => {
                let old_in_agg = self.in_aggregation;
                self.in_aggregation = true;
                let result = self.analyze_expression(expression);
                self.in_aggregation = old_in_agg;
                result?;
                Ok(ValueType::Any)
            }

            Expression::PatternPredicate(pattern) => {
                self.analyze_pattern(pattern)?;
                Ok(ValueType::Boolean)
            }

            Expression::Case {
                expression,
                alternatives,
                default,
            } => {
                if let Some(expr) = expression {
                    self.analyze_expression(expr)?;
                }

                for (condition, result) in alternatives {
                    self.analyze_expression(condition)?;
                    self.analyze_expression(result)?;
                }

                if let Some(default_expr) = default {
                    self.analyze_expression(default_expr)?;
                }

                Ok(ValueType::Any)
            }
        }
    }
}

impl Default for SemanticAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cypher::parser::parse_cypher;

    #[test]
    fn test_analyze_simple_match() {
        let query = parse_cypher("MATCH (n:Person) RETURN n").unwrap();
        let mut analyzer = SemanticAnalyzer::new();
        assert!(analyzer.analyze_query(&query).is_ok());
    }

    #[test]
    fn test_undefined_variable() {
        let query = parse_cypher("MATCH (n:Person) RETURN m").unwrap();
        let mut analyzer = SemanticAnalyzer::new();
        assert!(matches!(
            analyzer.analyze_query(&query),
            Err(SemanticError::UndefinedVariable(_))
        ));
    }

    #[test]
    fn test_mixed_aggregation() {
        let query = parse_cypher("MATCH (n:Person) RETURN n.name, COUNT(n)").unwrap();
        let mut analyzer = SemanticAnalyzer::new();
        assert!(matches!(
            analyzer.analyze_query(&query),
            Err(SemanticError::MixedAggregation)
        ));
    }

    #[test]
    #[ignore = "Hyperedge syntax not yet implemented in parser"]
    fn test_hyperedge_validation() {
        let query = parse_cypher("MATCH (a)-[r:REL]->(b, c) RETURN a, r, b, c").unwrap();
        let mut analyzer = SemanticAnalyzer::new();
        assert!(analyzer.analyze_query(&query).is_ok());
    }
}
