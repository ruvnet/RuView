//! Abstract Syntax Tree definitions for Cypher query language
//!
//! Represents the parsed structure of Cypher queries including:
//! - Pattern matching (MATCH, OPTIONAL MATCH)
//! - Filtering (WHERE)
//! - Projections (RETURN, WITH)
//! - Mutations (CREATE, MERGE, DELETE, SET)
//! - Aggregations and ordering
//! - Hyperedge support for N-ary relationships

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level query representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Query {
    pub statements: Vec<Statement>,
}

/// Individual query statement
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Statement {
    Match(MatchClause),
    Create(CreateClause),
    Merge(MergeClause),
    Delete(DeleteClause),
    Set(SetClause),
    Remove(RemoveClause),
    Return(ReturnClause),
    With(WithClause),
}

/// MATCH clause for pattern matching
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchClause {
    pub optional: bool,
    pub patterns: Vec<Pattern>,
    pub where_clause: Option<WhereClause>,
}

/// Pattern matching expressions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Pattern {
    /// Simple node pattern: (n:Label {props})
    Node(NodePattern),
    /// Relationship pattern: (a)-[r:TYPE]->(b)
    Relationship(RelationshipPattern),
    /// Path pattern: p = (a)-[*1..5]->(b)
    Path(PathPattern),
    /// Hyperedge pattern for N-ary relationships: (a)-[r:TYPE]->(b,c,d)
    Hyperedge(HyperedgePattern),
}

/// Node pattern: (variable:Label {property: value})
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodePattern {
    pub variable: Option<String>,
    pub labels: Vec<String>,
    pub properties: Option<PropertyMap>,
}

/// Relationship pattern: [variable:Type {properties}]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationshipPattern {
    pub variable: Option<String>,
    pub rel_type: Option<String>,
    pub properties: Option<PropertyMap>,
    pub direction: Direction,
    pub range: Option<RelationshipRange>,
    /// Source node pattern
    pub from: Box<NodePattern>,
    /// Target - can be a NodePattern or another Pattern for chained relationships
    /// For simple relationships like (a)-[r]->(b), this is just the node
    /// For chained patterns like (a)-[r]->(b)<-[s]-(c), the target is nested
    pub to: Box<Pattern>,
}

/// Hyperedge pattern for N-ary relationships
/// Example: (person)-[r:TRANSACTION]->(account1, account2, merchant)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HyperedgePattern {
    pub variable: Option<String>,
    pub rel_type: String,
    pub properties: Option<PropertyMap>,
    pub from: Box<NodePattern>,
    pub to: Vec<NodePattern>, // Multiple target nodes for N-ary relationships
    pub arity: usize,         // Number of participating nodes (including source)
}

/// Relationship direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Outgoing,   // ->
    Incoming,   // <-
    Undirected, // -
}

/// Relationship range for path queries: [*min..max]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipRange {
    pub min: Option<usize>,
    pub max: Option<usize>,
}

/// Path pattern: p = (a)-[*]->(b)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathPattern {
    pub variable: String,
    pub pattern: Box<Pattern>,
}

/// Property map: {key: value, ...}
pub type PropertyMap = HashMap<String, Expression>;

/// WHERE clause for filtering
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhereClause {
    pub condition: Expression,
}

/// CREATE clause for creating nodes and relationships
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateClause {
    pub patterns: Vec<Pattern>,
}

/// MERGE clause for create-or-match
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MergeClause {
    pub pattern: Pattern,
    pub on_create: Option<SetClause>,
    pub on_match: Option<SetClause>,
}

/// DELETE clause
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeleteClause {
    pub detach: bool,
    pub expressions: Vec<Expression>,
}

/// SET clause for updating properties
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetClause {
    pub items: Vec<SetItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SetItem {
    Property {
        variable: String,
        property: String,
        value: Expression,
    },
    Variable {
        variable: String,
        value: Expression,
    },
    Labels {
        variable: String,
        labels: Vec<String>,
    },
}

/// REMOVE clause for removing properties or labels
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoveClause {
    pub items: Vec<RemoveItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RemoveItem {
    /// Remove a property: REMOVE n.property
    Property { variable: String, property: String },
    /// Remove labels: REMOVE n:Label1:Label2
    Labels {
        variable: String,
        labels: Vec<String>,
    },
}

/// RETURN clause for projection
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReturnClause {
    pub distinct: bool,
    pub items: Vec<ReturnItem>,
    pub order_by: Option<OrderBy>,
    pub skip: Option<Expression>,
    pub limit: Option<Expression>,
}

/// WITH clause for chaining queries
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WithClause {
    pub distinct: bool,
    pub items: Vec<ReturnItem>,
    pub where_clause: Option<WhereClause>,
    pub order_by: Option<OrderBy>,
    pub skip: Option<Expression>,
    pub limit: Option<Expression>,
}

/// Return item: expression AS alias
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReturnItem {
    pub expression: Expression,
    pub alias: Option<String>,
}

/// ORDER BY clause
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBy {
    pub items: Vec<OrderByItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderByItem {
    pub expression: Expression,
    pub ascending: bool,
}

/// Expression tree
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expression {
    // Literals
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Null,

    // Variables and properties
    Variable(String),
    Property {
        object: Box<Expression>,
        property: String,
    },

    // Collections
    List(Vec<Expression>),
    Map(HashMap<String, Expression>),

    // Operators
    BinaryOp {
        left: Box<Expression>,
        op: BinaryOperator,
        right: Box<Expression>,
    },
    UnaryOp {
        op: UnaryOperator,
        operand: Box<Expression>,
    },

    // Functions and aggregations
    FunctionCall {
        name: String,
        args: Vec<Expression>,
    },
    Aggregation {
        function: AggregationFunction,
        expression: Box<Expression>,
        distinct: bool,
    },

    // Pattern predicates
    PatternPredicate(Box<Pattern>),

    // Case expressions
    Case {
        expression: Option<Box<Expression>>,
        alternatives: Vec<(Expression, Expression)>,
        default: Option<Box<Expression>>,
    },
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOperator {
    // Arithmetic
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Power,

    // Comparison
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,

    // Logical
    And,
    Or,
    Xor,

    // String
    Contains,
    StartsWith,
    EndsWith,
    Matches, // Regex

    // Collection
    In,

    // Null checking
    Is,
    IsNot,
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOperator {
    Not,
    Minus,
    Plus,
    IsNull,
    IsNotNull,
}

/// Aggregation functions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AggregationFunction {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    Collect,
    StdDev,
    StdDevP,
    Percentile,
}

impl Query {
    pub fn new(statements: Vec<Statement>) -> Self {
        Self { statements }
    }

    /// Check if query contains only read operations
    pub fn is_read_only(&self) -> bool {
        self.statements.iter().all(|stmt| {
            matches!(
                stmt,
                Statement::Match(_) | Statement::Return(_) | Statement::With(_)
            )
        })
    }

    /// Check if query contains hyperedges
    pub fn has_hyperedges(&self) -> bool {
        self.statements.iter().any(|stmt| match stmt {
            Statement::Match(m) => m
                .patterns
                .iter()
                .any(|p| matches!(p, Pattern::Hyperedge(_))),
            Statement::Create(c) => c
                .patterns
                .iter()
                .any(|p| matches!(p, Pattern::Hyperedge(_))),
            Statement::Merge(m) => matches!(&m.pattern, Pattern::Hyperedge(_)),
            _ => false,
        })
    }
}

impl Pattern {
    /// Get the arity of the pattern (number of nodes involved)
    pub fn arity(&self) -> usize {
        match self {
            Pattern::Node(_) => 1,
            Pattern::Relationship(_) => 2,
            Pattern::Path(_) => 2, // Simplified, could be variable
            Pattern::Hyperedge(h) => h.arity,
        }
    }
}

impl Expression {
    /// Check if expression is constant (no variables)
    pub fn is_constant(&self) -> bool {
        match self {
            Expression::Integer(_)
            | Expression::Float(_)
            | Expression::String(_)
            | Expression::Boolean(_)
            | Expression::Null => true,
            Expression::List(items) => items.iter().all(|e| e.is_constant()),
            Expression::Map(map) => map.values().all(|e| e.is_constant()),
            Expression::BinaryOp { left, right, .. } => left.is_constant() && right.is_constant(),
            Expression::UnaryOp { operand, .. } => operand.is_constant(),
            _ => false,
        }
    }

    /// Check if expression contains aggregation
    pub fn has_aggregation(&self) -> bool {
        match self {
            Expression::Aggregation { .. } => true,
            Expression::BinaryOp { left, right, .. } => {
                left.has_aggregation() || right.has_aggregation()
            }
            Expression::UnaryOp { operand, .. } => operand.has_aggregation(),
            Expression::FunctionCall { args, .. } => args.iter().any(|e| e.has_aggregation()),
            Expression::List(items) => items.iter().any(|e| e.has_aggregation()),
            Expression::Property { object, .. } => object.has_aggregation(),
            _ => false,
        }
    }
}
