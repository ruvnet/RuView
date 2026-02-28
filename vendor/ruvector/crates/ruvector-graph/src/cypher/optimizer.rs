//! Query optimizer for Cypher queries
//!
//! Optimizes query execution plans through:
//! - Predicate pushdown (filter as early as possible)
//! - Join reordering (minimize intermediate results)
//! - Index utilization
//! - Constant folding
//! - Dead code elimination

use super::ast::*;
use std::collections::HashSet;

/// Query optimization plan
#[derive(Debug, Clone)]
pub struct OptimizationPlan {
    pub optimized_query: Query,
    pub optimizations_applied: Vec<OptimizationType>,
    pub estimated_cost: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationType {
    PredicatePushdown,
    JoinReordering,
    ConstantFolding,
    IndexHint,
    EarlyFiltering,
    PatternSimplification,
    DeadCodeElimination,
}

pub struct QueryOptimizer {
    enable_predicate_pushdown: bool,
    enable_join_reordering: bool,
    enable_constant_folding: bool,
}

impl QueryOptimizer {
    pub fn new() -> Self {
        Self {
            enable_predicate_pushdown: true,
            enable_join_reordering: true,
            enable_constant_folding: true,
        }
    }

    /// Optimize a query and return an execution plan
    pub fn optimize(&self, query: Query) -> OptimizationPlan {
        let mut optimized = query;
        let mut optimizations = Vec::new();

        // Apply optimizations in order
        if self.enable_constant_folding {
            if let Some(q) = self.apply_constant_folding(optimized.clone()) {
                optimized = q;
                optimizations.push(OptimizationType::ConstantFolding);
            }
        }

        if self.enable_predicate_pushdown {
            if let Some(q) = self.apply_predicate_pushdown(optimized.clone()) {
                optimized = q;
                optimizations.push(OptimizationType::PredicatePushdown);
            }
        }

        if self.enable_join_reordering {
            if let Some(q) = self.apply_join_reordering(optimized.clone()) {
                optimized = q;
                optimizations.push(OptimizationType::JoinReordering);
            }
        }

        let cost = self.estimate_cost(&optimized);

        OptimizationPlan {
            optimized_query: optimized,
            optimizations_applied: optimizations,
            estimated_cost: cost,
        }
    }

    /// Apply constant folding to simplify expressions
    fn apply_constant_folding(&self, mut query: Query) -> Option<Query> {
        let mut changed = false;

        for statement in &mut query.statements {
            if self.fold_statement(statement) {
                changed = true;
            }
        }

        if changed {
            Some(query)
        } else {
            None
        }
    }

    fn fold_statement(&self, statement: &mut Statement) -> bool {
        match statement {
            Statement::Match(clause) => {
                let mut changed = false;
                if let Some(where_clause) = &mut clause.where_clause {
                    if let Some(folded) = self.fold_expression(&where_clause.condition) {
                        where_clause.condition = folded;
                        changed = true;
                    }
                }
                changed
            }
            Statement::Return(clause) => {
                let mut changed = false;
                for item in &mut clause.items {
                    if let Some(folded) = self.fold_expression(&item.expression) {
                        item.expression = folded;
                        changed = true;
                    }
                }
                changed
            }
            _ => false,
        }
    }

    fn fold_expression(&self, expr: &Expression) -> Option<Expression> {
        match expr {
            Expression::BinaryOp { left, op, right } => {
                // Fold operands first
                let left = self
                    .fold_expression(left)
                    .unwrap_or_else(|| (**left).clone());
                let right = self
                    .fold_expression(right)
                    .unwrap_or_else(|| (**right).clone());

                // Try to evaluate constant expressions
                if left.is_constant() && right.is_constant() {
                    return self.evaluate_constant_binary_op(&left, *op, &right);
                }

                // Return simplified expression
                Some(Expression::BinaryOp {
                    left: Box::new(left),
                    op: *op,
                    right: Box::new(right),
                })
            }
            Expression::UnaryOp { op, operand } => {
                let operand = self
                    .fold_expression(operand)
                    .unwrap_or_else(|| (**operand).clone());

                if operand.is_constant() {
                    return self.evaluate_constant_unary_op(*op, &operand);
                }

                Some(Expression::UnaryOp {
                    op: *op,
                    operand: Box::new(operand),
                })
            }
            _ => None,
        }
    }

    fn evaluate_constant_binary_op(
        &self,
        left: &Expression,
        op: BinaryOperator,
        right: &Expression,
    ) -> Option<Expression> {
        match (left, op, right) {
            // Arithmetic operations
            (Expression::Integer(a), BinaryOperator::Add, Expression::Integer(b)) => {
                Some(Expression::Integer(a + b))
            }
            (Expression::Integer(a), BinaryOperator::Subtract, Expression::Integer(b)) => {
                Some(Expression::Integer(a - b))
            }
            (Expression::Integer(a), BinaryOperator::Multiply, Expression::Integer(b)) => {
                Some(Expression::Integer(a * b))
            }
            (Expression::Integer(a), BinaryOperator::Divide, Expression::Integer(b)) if *b != 0 => {
                Some(Expression::Integer(a / b))
            }
            (Expression::Integer(a), BinaryOperator::Modulo, Expression::Integer(b)) if *b != 0 => {
                Some(Expression::Integer(a % b))
            }
            (Expression::Float(a), BinaryOperator::Add, Expression::Float(b)) => {
                Some(Expression::Float(a + b))
            }
            (Expression::Float(a), BinaryOperator::Subtract, Expression::Float(b)) => {
                Some(Expression::Float(a - b))
            }
            (Expression::Float(a), BinaryOperator::Multiply, Expression::Float(b)) => {
                Some(Expression::Float(a * b))
            }
            (Expression::Float(a), BinaryOperator::Divide, Expression::Float(b)) if *b != 0.0 => {
                Some(Expression::Float(a / b))
            }
            // Comparison operations for integers
            (Expression::Integer(a), BinaryOperator::Equal, Expression::Integer(b)) => {
                Some(Expression::Boolean(a == b))
            }
            (Expression::Integer(a), BinaryOperator::NotEqual, Expression::Integer(b)) => {
                Some(Expression::Boolean(a != b))
            }
            (Expression::Integer(a), BinaryOperator::LessThan, Expression::Integer(b)) => {
                Some(Expression::Boolean(a < b))
            }
            (Expression::Integer(a), BinaryOperator::LessThanOrEqual, Expression::Integer(b)) => {
                Some(Expression::Boolean(a <= b))
            }
            (Expression::Integer(a), BinaryOperator::GreaterThan, Expression::Integer(b)) => {
                Some(Expression::Boolean(a > b))
            }
            (
                Expression::Integer(a),
                BinaryOperator::GreaterThanOrEqual,
                Expression::Integer(b),
            ) => Some(Expression::Boolean(a >= b)),
            // Comparison operations for floats
            (Expression::Float(a), BinaryOperator::Equal, Expression::Float(b)) => {
                Some(Expression::Boolean((a - b).abs() < f64::EPSILON))
            }
            (Expression::Float(a), BinaryOperator::NotEqual, Expression::Float(b)) => {
                Some(Expression::Boolean((a - b).abs() >= f64::EPSILON))
            }
            (Expression::Float(a), BinaryOperator::LessThan, Expression::Float(b)) => {
                Some(Expression::Boolean(a < b))
            }
            (Expression::Float(a), BinaryOperator::LessThanOrEqual, Expression::Float(b)) => {
                Some(Expression::Boolean(a <= b))
            }
            (Expression::Float(a), BinaryOperator::GreaterThan, Expression::Float(b)) => {
                Some(Expression::Boolean(a > b))
            }
            (Expression::Float(a), BinaryOperator::GreaterThanOrEqual, Expression::Float(b)) => {
                Some(Expression::Boolean(a >= b))
            }
            // String comparison
            (Expression::String(a), BinaryOperator::Equal, Expression::String(b)) => {
                Some(Expression::Boolean(a == b))
            }
            (Expression::String(a), BinaryOperator::NotEqual, Expression::String(b)) => {
                Some(Expression::Boolean(a != b))
            }
            // Boolean operations
            (Expression::Boolean(a), BinaryOperator::And, Expression::Boolean(b)) => {
                Some(Expression::Boolean(*a && *b))
            }
            (Expression::Boolean(a), BinaryOperator::Or, Expression::Boolean(b)) => {
                Some(Expression::Boolean(*a || *b))
            }
            (Expression::Boolean(a), BinaryOperator::Equal, Expression::Boolean(b)) => {
                Some(Expression::Boolean(a == b))
            }
            (Expression::Boolean(a), BinaryOperator::NotEqual, Expression::Boolean(b)) => {
                Some(Expression::Boolean(a != b))
            }
            _ => None,
        }
    }

    fn evaluate_constant_unary_op(
        &self,
        op: UnaryOperator,
        operand: &Expression,
    ) -> Option<Expression> {
        match (op, operand) {
            (UnaryOperator::Not, Expression::Boolean(b)) => Some(Expression::Boolean(!b)),
            (UnaryOperator::Minus, Expression::Integer(n)) => Some(Expression::Integer(-n)),
            (UnaryOperator::Minus, Expression::Float(n)) => Some(Expression::Float(-n)),
            _ => None,
        }
    }

    /// Apply predicate pushdown optimization
    /// Move WHERE clauses as close to data access as possible
    fn apply_predicate_pushdown(&self, query: Query) -> Option<Query> {
        // In a real implementation, this would analyze the query graph
        // and push predicates down to the earliest possible point
        // For now, we'll do a simple transformation

        // This is a placeholder - real implementation would be more complex
        None
    }

    /// Reorder joins to minimize intermediate result sizes
    fn apply_join_reordering(&self, query: Query) -> Option<Query> {
        // Analyze pattern complexity and reorder based on selectivity
        // Patterns with more constraints should be evaluated first

        let mut optimized = query.clone();
        let mut changed = false;

        for statement in &mut optimized.statements {
            if let Statement::Match(clause) = statement {
                let mut patterns = clause.patterns.clone();

                // Sort patterns by estimated selectivity (more selective first)
                patterns.sort_by_key(|p| {
                    let selectivity = self.estimate_pattern_selectivity(p);
                    // Use negative to sort in descending order (most selective first)
                    -(selectivity * 1000.0) as i64
                });

                if patterns != clause.patterns {
                    clause.patterns = patterns;
                    changed = true;
                }
            }
        }

        if changed {
            Some(optimized)
        } else {
            None
        }
    }

    /// Estimate the selectivity of a pattern (0.0 = least selective, 1.0 = most selective)
    fn estimate_pattern_selectivity(&self, pattern: &Pattern) -> f64 {
        match pattern {
            Pattern::Node(node) => {
                let mut selectivity = 0.3; // Base selectivity for node

                // More labels = more selective
                selectivity += node.labels.len() as f64 * 0.1;

                // Properties = more selective
                if let Some(props) = &node.properties {
                    selectivity += props.len() as f64 * 0.15;
                }

                selectivity.min(1.0)
            }
            Pattern::Relationship(rel) => {
                let mut selectivity = 0.2; // Base selectivity for relationship

                // Specific type = more selective
                if rel.rel_type.is_some() {
                    selectivity += 0.2;
                }

                // Properties = more selective
                if let Some(props) = &rel.properties {
                    selectivity += props.len() as f64 * 0.15;
                }

                // Add selectivity from connected nodes
                selectivity +=
                    self.estimate_pattern_selectivity(&Pattern::Node(*rel.from.clone())) * 0.3;
                // rel.to is now a Pattern (can be NodePattern or chained RelationshipPattern)
                selectivity += self.estimate_pattern_selectivity(&*rel.to) * 0.3;

                selectivity.min(1.0)
            }
            Pattern::Hyperedge(hyperedge) => {
                let mut selectivity = 0.5; // Hyperedges are typically more selective

                // More nodes involved = more selective
                selectivity += hyperedge.arity as f64 * 0.1;

                if let Some(props) = &hyperedge.properties {
                    selectivity += props.len() as f64 * 0.15;
                }

                selectivity.min(1.0)
            }
            Pattern::Path(_) => 0.1, // Paths are typically less selective
        }
    }

    /// Estimate the cost of executing a query
    fn estimate_cost(&self, query: &Query) -> f64 {
        let mut cost = 0.0;

        for statement in &query.statements {
            cost += self.estimate_statement_cost(statement);
        }

        cost
    }

    fn estimate_statement_cost(&self, statement: &Statement) -> f64 {
        match statement {
            Statement::Match(clause) => {
                let mut cost = 0.0;

                for pattern in &clause.patterns {
                    cost += self.estimate_pattern_cost(pattern);
                }

                // WHERE clause adds filtering cost
                if clause.where_clause.is_some() {
                    cost *= 1.2;
                }

                cost
            }
            Statement::Create(clause) => {
                // Create operations are expensive
                clause.patterns.len() as f64 * 50.0
            }
            Statement::Merge(clause) => {
                // Merge is more expensive than match or create alone
                self.estimate_pattern_cost(&clause.pattern) * 2.0
            }
            Statement::Delete(_) => 30.0,
            Statement::Set(_) => 20.0,
            Statement::Remove(clause) => clause.items.len() as f64 * 15.0,
            Statement::Return(clause) => {
                let mut cost = 10.0;

                // Aggregations are expensive
                for item in &clause.items {
                    if item.expression.has_aggregation() {
                        cost += 50.0;
                    }
                }

                // Sorting adds cost
                if clause.order_by.is_some() {
                    cost += 100.0;
                }

                cost
            }
            Statement::With(_) => 15.0,
        }
    }

    fn estimate_pattern_cost(&self, pattern: &Pattern) -> f64 {
        match pattern {
            Pattern::Node(node) => {
                let mut cost = 100.0;

                // Labels reduce cost (more selective)
                cost /= (1.0 + node.labels.len() as f64 * 0.5);

                // Properties reduce cost
                if let Some(props) = &node.properties {
                    cost /= (1.0 + props.len() as f64 * 0.3);
                }

                cost
            }
            Pattern::Relationship(rel) => {
                let mut cost = 200.0; // Relationships are more expensive

                // Specific type reduces cost
                if rel.rel_type.is_some() {
                    cost *= 0.7;
                }

                // Variable length paths are very expensive
                if let Some(range) = &rel.range {
                    let max = range.max.unwrap_or(10);
                    cost *= max as f64;
                }

                cost
            }
            Pattern::Hyperedge(hyperedge) => {
                // Hyperedges are more expensive due to N-ary nature
                150.0 * hyperedge.arity as f64
            }
            Pattern::Path(_) => 300.0, // Paths can be expensive
        }
    }

    /// Get variables used in an expression
    fn get_variables_in_expression(&self, expr: &Expression) -> HashSet<String> {
        let mut vars = HashSet::new();
        self.collect_variables(expr, &mut vars);
        vars
    }

    fn collect_variables(&self, expr: &Expression, vars: &mut HashSet<String>) {
        match expr {
            Expression::Variable(name) => {
                vars.insert(name.clone());
            }
            Expression::Property { object, .. } => {
                self.collect_variables(object, vars);
            }
            Expression::BinaryOp { left, right, .. } => {
                self.collect_variables(left, vars);
                self.collect_variables(right, vars);
            }
            Expression::UnaryOp { operand, .. } => {
                self.collect_variables(operand, vars);
            }
            Expression::FunctionCall { args, .. } => {
                for arg in args {
                    self.collect_variables(arg, vars);
                }
            }
            Expression::Aggregation { expression, .. } => {
                self.collect_variables(expression, vars);
            }
            Expression::List(items) => {
                for item in items {
                    self.collect_variables(item, vars);
                }
            }
            Expression::Case {
                expression,
                alternatives,
                default,
            } => {
                if let Some(expr) = expression {
                    self.collect_variables(expr, vars);
                }
                for (cond, result) in alternatives {
                    self.collect_variables(cond, vars);
                    self.collect_variables(result, vars);
                }
                if let Some(default_expr) = default {
                    self.collect_variables(default_expr, vars);
                }
            }
            _ => {}
        }
    }
}

impl Default for QueryOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cypher::parser::parse_cypher;

    #[test]
    fn test_constant_folding() {
        let query = parse_cypher("MATCH (n) WHERE 2 + 3 = 5 RETURN n").unwrap();
        let optimizer = QueryOptimizer::new();
        let plan = optimizer.optimize(query);

        assert!(plan
            .optimizations_applied
            .contains(&OptimizationType::ConstantFolding));
    }

    #[test]
    fn test_cost_estimation() {
        let query = parse_cypher("MATCH (n:Person {age: 30}) RETURN n").unwrap();
        let optimizer = QueryOptimizer::new();
        let cost = optimizer.estimate_cost(&query);

        assert!(cost > 0.0);
    }

    #[test]
    fn test_pattern_selectivity() {
        let optimizer = QueryOptimizer::new();

        let node_with_label = Pattern::Node(NodePattern {
            variable: Some("n".to_string()),
            labels: vec!["Person".to_string()],
            properties: None,
        });

        let node_without_label = Pattern::Node(NodePattern {
            variable: Some("n".to_string()),
            labels: vec![],
            properties: None,
        });

        let sel_with = optimizer.estimate_pattern_selectivity(&node_with_label);
        let sel_without = optimizer.estimate_pattern_selectivity(&node_without_label);

        assert!(sel_with > sel_without);
    }
}
