// Cypher AST (Abstract Syntax Tree) types

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Complete Cypher query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CypherQuery {
    pub clauses: Vec<Clause>,
}

impl CypherQuery {
    pub fn new() -> Self {
        Self {
            clauses: Vec::new(),
        }
    }

    pub fn with_clause(mut self, clause: Clause) -> Self {
        self.clauses.push(clause);
        self
    }
}

impl Default for CypherQuery {
    fn default() -> Self {
        Self::new()
    }
}

/// Query clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Clause {
    Match(MatchClause),
    Create(CreateClause),
    Return(ReturnClause),
    Where(WhereClause),
    Set(SetClause),
    Delete(DeleteClause),
    With(WithClause),
}

/// MATCH clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchClause {
    pub patterns: Vec<Pattern>,
    pub optional: bool,
}

impl MatchClause {
    pub fn new(patterns: Vec<Pattern>) -> Self {
        Self {
            patterns,
            optional: false,
        }
    }

    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
}

/// CREATE clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateClause {
    pub patterns: Vec<Pattern>,
}

impl CreateClause {
    pub fn new(patterns: Vec<Pattern>) -> Self {
        Self { patterns }
    }
}

/// RETURN clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnClause {
    pub items: Vec<ReturnItem>,
    pub distinct: bool,
    pub limit: Option<usize>,
    pub skip: Option<usize>,
}

impl ReturnClause {
    pub fn new(items: Vec<ReturnItem>) -> Self {
        Self {
            items,
            distinct: false,
            limit: None,
            skip: None,
        }
    }

    pub fn distinct(mut self) -> Self {
        self.distinct = true;
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn skip(mut self, skip: usize) -> Self {
        self.skip = Some(skip);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnItem {
    pub expression: Expression,
    pub alias: Option<String>,
}

impl ReturnItem {
    pub fn new(expression: Expression) -> Self {
        Self {
            expression,
            alias: None,
        }
    }

    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        self.alias = Some(alias.into());
        self
    }
}

/// WHERE clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhereClause {
    pub condition: Expression,
}

impl WhereClause {
    pub fn new(condition: Expression) -> Self {
        Self { condition }
    }
}

/// SET clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetClause {
    pub items: Vec<SetItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetItem {
    pub variable: String,
    pub property: String,
    pub value: Expression,
}

/// DELETE clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteClause {
    pub items: Vec<String>,
    pub detach: bool,
}

/// WITH clause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithClause {
    pub items: Vec<ReturnItem>,
}

/// Graph pattern (node)-[relationship]->(node)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub elements: Vec<PatternElement>,
}

impl Pattern {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
        }
    }

    pub fn with_element(mut self, element: PatternElement) -> Self {
        self.elements.push(element);
        self
    }
}

impl Default for Pattern {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternElement {
    Node(NodePattern),
    Relationship(RelationshipPattern),
}

/// Node pattern (n:Label {property: value})
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePattern {
    pub variable: Option<String>,
    pub labels: Vec<String>,
    pub properties: HashMap<String, Expression>,
}

impl NodePattern {
    pub fn new() -> Self {
        Self {
            variable: None,
            labels: Vec::new(),
            properties: HashMap::new(),
        }
    }

    pub fn with_variable(mut self, variable: impl Into<String>) -> Self {
        self.variable = Some(variable.into());
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }

    pub fn with_property(mut self, key: impl Into<String>, value: Expression) -> Self {
        self.properties.insert(key.into(), value);
        self
    }
}

impl Default for NodePattern {
    fn default() -> Self {
        Self::new()
    }
}

/// Relationship pattern -[r:TYPE {property: value}]->
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipPattern {
    pub variable: Option<String>,
    pub rel_type: Option<String>,
    pub properties: HashMap<String, Expression>,
    pub direction: Direction,
    pub min_hops: Option<usize>,
    pub max_hops: Option<usize>,
}

impl RelationshipPattern {
    pub fn new(direction: Direction) -> Self {
        Self {
            variable: None,
            rel_type: None,
            properties: HashMap::new(),
            direction,
            min_hops: None,
            max_hops: None,
        }
    }

    pub fn with_variable(mut self, variable: impl Into<String>) -> Self {
        self.variable = Some(variable.into());
        self
    }

    pub fn with_type(mut self, rel_type: impl Into<String>) -> Self {
        self.rel_type = Some(rel_type.into());
        self
    }

    pub fn with_property(mut self, key: impl Into<String>, value: Expression) -> Self {
        self.properties.insert(key.into(), value);
        self
    }

    pub fn with_hops(mut self, min: usize, max: usize) -> Self {
        self.min_hops = Some(min);
        self.max_hops = Some(max);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Outgoing, // ->
    Incoming, // <-
    Both,     // -
}

/// Expression in Cypher
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expression {
    Literal(JsonValue),
    Variable(String),
    Property(String, String), // variable.property
    Parameter(String),        // $param
    FunctionCall(String, Vec<Expression>),
    BinaryOp(Box<Expression>, BinaryOperator, Box<Expression>),
    UnaryOp(UnaryOperator, Box<Expression>),
}

impl Expression {
    pub fn literal(value: impl Into<JsonValue>) -> Self {
        Self::Literal(value.into())
    }

    pub fn variable(name: impl Into<String>) -> Self {
        Self::Variable(name.into())
    }

    pub fn property(var: impl Into<String>, prop: impl Into<String>) -> Self {
        Self::Property(var.into(), prop.into())
    }

    pub fn parameter(name: impl Into<String>) -> Self {
        Self::Parameter(name.into())
    }

    pub fn function(name: impl Into<String>, args: Vec<Expression>) -> Self {
        Self::FunctionCall(name.into(), args)
    }

    pub fn binary(left: Expression, op: BinaryOperator, right: Expression) -> Self {
        Self::BinaryOp(Box::new(left), op, Box::new(right))
    }

    pub fn unary(op: UnaryOperator, expr: Expression) -> Self {
        Self::UnaryOp(op, Box::new(expr))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOperator {
    Eq,         // =
    Neq,        // <>
    Lt,         // <
    Lte,        // <=
    Gt,         // >
    Gte,        // >=
    And,        // AND
    Or,         // OR
    Add,        // +
    Sub,        // -
    Mul,        // *
    Div,        // /
    Mod,        // %
    In,         // IN
    Contains,   // CONTAINS
    StartsWith, // STARTS WITH
    EndsWith,   // ENDS WITH
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOperator {
    Not,   // NOT
    Minus, // -
}
