# RuVector Postgres v2 - Phase 3: Graph Engine & Cypher

## Overview

Phase 3 adds property graph capabilities with Cypher query support, enabling users to model relationships between vectors and perform graph traversals alongside vector similarity search.

---

## Objectives

### Primary Goals
1. Property graph storage in PostgreSQL
2. Cypher query execution via `ruvector_cypher()`
3. Relational bridge views for SQL-graph mixing
4. Vector-enriched graph queries

### Success Criteria
- Full Cypher read query support
- Graph nodes can reference vectors
- SQL joins with graph data
- < 10ms overhead for simple traversals

---

## Graph-SQL Join Keys and Identity System

### Minimum Viable Bridge

When exposing `ruvector_nodes` and `ruvector_edges` as views, users need clear join keys to mix Cypher output with their relational tables.

```
+------------------------------------------------------------------+
|              GRAPH-SQL IDENTITY SYSTEM                            |
+------------------------------------------------------------------+

DESIGN PRINCIPLES:
  • "SQL first" - users join Cypher output to their tables easily
  • Stable identifiers that survive graph mutations
  • No need to learn a new identity system

IDENTITY MAPPING:

  Graph Node ID (BIGINT):
    • Stable, auto-generated within RuVector
    • Stored in catalog table: ruvector.node_catalog
    • Maps to user-provided external_id

  External ID (TEXT):
    • User-provided identifier (e.g., "user_123", "doc_abc")
    • Unique within node_type
    • Used for joins with user tables

  Vector Reference (TID or FK):
    • Optional link to user table with vector column
    • Enables vector operations on graph nodes

+------------------------------------------------------------------+
```

### Node Catalog Table

```sql
-- Central catalog for node identity mapping
CREATE TABLE ruvector.node_catalog (
    -- Internal stable ID (primary key)
    node_id         BIGSERIAL PRIMARY KEY,

    -- Collection (graph) this node belongs to
    collection_id   INTEGER NOT NULL REFERENCES ruvector.collections(id),

    -- User-provided external identifier
    external_id     TEXT NOT NULL,

    -- Node type/label (e.g., 'User', 'Document', 'Product')
    node_type       TEXT NOT NULL,

    -- Reference to user table (for vector access)
    source_table    TEXT,           -- e.g., 'public.documents'
    source_pk       TEXT,           -- e.g., 'id'
    source_pk_value TEXT,           -- e.g., '12345'

    -- Metadata
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Uniqueness constraint
    CONSTRAINT uq_node_external UNIQUE (collection_id, node_type, external_id)
);

CREATE INDEX idx_node_catalog_external
    ON ruvector.node_catalog(collection_id, external_id);
CREATE INDEX idx_node_catalog_source
    ON ruvector.node_catalog(source_table, source_pk_value);
```

### Edge Catalog Table

```sql
-- Edge storage with relational metadata
CREATE TABLE ruvector.edge_catalog (
    -- Internal edge ID
    edge_id         BIGSERIAL PRIMARY KEY,

    -- Collection
    collection_id   INTEGER NOT NULL REFERENCES ruvector.collections(id),

    -- Source and target nodes (foreign keys)
    source_node_id  BIGINT NOT NULL REFERENCES ruvector.node_catalog(node_id),
    target_node_id  BIGINT NOT NULL REFERENCES ruvector.node_catalog(node_id),

    -- Edge type/label (e.g., 'FOLLOWS', 'PURCHASED', 'SIMILAR_TO')
    edge_type       TEXT NOT NULL,

    -- Edge weight (for weighted graph algorithms)
    weight          REAL DEFAULT 1.0,

    -- User-provided properties
    properties      JSONB DEFAULT '{}'::jsonb,

    -- Metadata
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_edge_source ON ruvector.edge_catalog(source_node_id);
CREATE INDEX idx_edge_target ON ruvector.edge_catalog(target_node_id);
CREATE INDEX idx_edge_type ON ruvector.edge_catalog(collection_id, edge_type);
```

### Relational Views for SQL Access

```sql
-- View for SQL users to query nodes
CREATE VIEW ruvector.nodes AS
SELECT
    nc.node_id,
    nc.external_id,
    nc.node_type AS label,
    c.name AS collection,
    nc.source_table,
    nc.source_pk_value,
    n.properties,
    nc.created_at
FROM ruvector.node_catalog nc
JOIN ruvector.collections c ON nc.collection_id = c.id
LEFT JOIN ruvector._node_properties n ON nc.node_id = n.node_id;

-- View for SQL users to query edges
CREATE VIEW ruvector.edges AS
SELECT
    ec.edge_id,
    ec.edge_type AS type,
    src.external_id AS source_external_id,
    src.node_type AS source_label,
    tgt.external_id AS target_external_id,
    tgt.node_type AS target_label,
    ec.weight,
    ec.properties,
    c.name AS collection
FROM ruvector.edge_catalog ec
JOIN ruvector.node_catalog src ON ec.source_node_id = src.node_id
JOIN ruvector.node_catalog tgt ON ec.target_node_id = tgt.node_id
JOIN ruvector.collections c ON ec.collection_id = c.id;
```

### Joining Cypher Results to SQL Tables

```sql
-- Example: Join Cypher results with user table
WITH graph_results AS (
    SELECT * FROM ruvector_cypher(
        'social',
        'MATCH (u:User)-[:FOLLOWS*1..3]->(friend:User)
         WHERE u.external_id = $user_id
         RETURN friend.external_id AS friend_id
         LIMIT 100',
        jsonb_build_object('user_id', 'user_123')
    )
)
SELECT
    u.id,
    u.name,
    u.email,
    u.profile_vector <-> query_vector AS similarity
FROM graph_results gr
JOIN users u ON u.user_id = gr.friend_id  -- Join on external_id
CROSS JOIN (SELECT '[1,2,3,...]'::vector AS query_vector) q
ORDER BY similarity
LIMIT 10;
```

### Cypher Return Format

```sql
-- ruvector_cypher returns setof records with stable IDs
CREATE FUNCTION ruvector_cypher(
    p_collection TEXT,
    p_query TEXT,
    p_params JSONB DEFAULT '{}'::jsonb
) RETURNS TABLE (
    -- Node columns (when RETURN includes nodes)
    node_id         BIGINT,
    external_id     TEXT,
    label           TEXT,
    properties      JSONB,

    -- Edge columns (when RETURN includes relationships)
    edge_id         BIGINT,
    edge_type       TEXT,
    source_id       BIGINT,
    target_id       BIGINT,
    weight          REAL,

    -- Path columns (when RETURN includes paths)
    path_length     INTEGER,
    path_nodes      BIGINT[],
    path_edges      BIGINT[]
) AS 'MODULE_PATHNAME', 'ruvector_cypher' LANGUAGE C;
```

### Join Acceleration

```sql
-- Materialized metadata for fast joins
CREATE MATERIALIZED VIEW ruvector.node_external_ids AS
SELECT
    node_id,
    external_id,
    node_type,
    collection_id
FROM ruvector.node_catalog;

CREATE UNIQUE INDEX idx_node_ext_lookup
    ON ruvector.node_external_ids(collection_id, external_id);

-- Refresh periodically
CREATE FUNCTION ruvector.refresh_join_cache()
RETURNS void AS $$
    REFRESH MATERIALIZED VIEW CONCURRENTLY ruvector.node_external_ids;
$$ LANGUAGE SQL;
```

---

## Architecture

### Graph Stack

```
+------------------------------------------------------------------+
|                     User Queries                                  |
|  SQL:    SELECT * FROM items ORDER BY embedding <-> $q           |
|  Cypher: MATCH (a)-[:LIKES]->(b) WHERE a.vector <=> $q < 0.5     |
+------------------------------------------------------------------+
              |                           |
              v                           v
+---------------------------+  +---------------------------+
|     SQL Query Path        |  |    Cypher Query Path      |
|                           |  |                           |
| Parse -> Plan -> Execute  |  | Parse -> Plan -> Execute  |
|                           |  |                           |
| Uses: PostgreSQL Executor |  | Uses: RuVector Cypher     |
+---------------------------+  +---------------------------+
              |                           |
              +-------------+-------------+
                            |
                            v
+------------------------------------------------------------------+
|                    Graph Storage Layer                            |
|  - ruvector.nodes (PostgreSQL table)                             |
|  - ruvector.edges (PostgreSQL table)                             |
|  - ruvector.hyperedges (PostgreSQL table)                        |
|  - Property indexes (GIN on JSONB)                               |
+------------------------------------------------------------------+
              |
              v
+------------------------------------------------------------------+
|                    Vector Integration                             |
|  - node.vector_ref -> user table (TID)                           |
|  - Cypher can use vector operators                               |
|  - GNN training from graph structure                             |
+------------------------------------------------------------------+
```

### Data Model

```
                    +------------------+
                    |     Graph        |
                    | (id, name, ...)  |
                    +--------+---------+
                             |
                             | 1:N
                             v
+------------------+  +------+------+  +------------------+
|      Node        |  |    Edge     |  |   Hyperedge      |
| - id             |  | - id        |  | - id             |
| - external_id    |  | - source_id |  | - node_ids[]     |
| - node_type      |  | - target_id |  | - weights[]      |
| - properties     |  | - edge_type |  | - properties     |
| - vector_ref     |  | - weight    |  +------------------+
+------------------+  | - properties|
        |             +-------------+
        |
        | References
        v
+------------------+
|   User Table     |
| - id             |
| - embedding      |  <-- vector column
| - metadata       |
+------------------+
```

---

## Deliverables

### 1. Graph Storage Schema

```sql
-- Graph metadata
CREATE TABLE ruvector.graphs (
    id              SERIAL PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    description     TEXT,
    node_count      BIGINT NOT NULL DEFAULT 0,
    edge_count      BIGINT NOT NULL DEFAULT 0,
    hyperedge_count BIGINT NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    config          JSONB NOT NULL DEFAULT '{}'::jsonb
);

-- Graph nodes
CREATE TABLE ruvector.nodes (
    id              BIGSERIAL PRIMARY KEY,
    graph_id        INTEGER NOT NULL REFERENCES ruvector.graphs(id) ON DELETE CASCADE,
    external_id     TEXT,
    node_type       TEXT NOT NULL DEFAULT 'default',
    vector_ref      TID,
    collection_id   INTEGER REFERENCES ruvector.collections(id),
    properties      JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(graph_id, external_id)
);

CREATE INDEX idx_nodes_graph_type ON ruvector.nodes(graph_id, node_type);
CREATE INDEX idx_nodes_properties ON ruvector.nodes USING gin(properties);
CREATE INDEX idx_nodes_vector_ref ON ruvector.nodes(collection_id, vector_ref)
    WHERE vector_ref IS NOT NULL;

-- Graph edges
CREATE TABLE ruvector.edges (
    id              BIGSERIAL PRIMARY KEY,
    graph_id        INTEGER NOT NULL REFERENCES ruvector.graphs(id) ON DELETE CASCADE,
    source_id       BIGINT NOT NULL REFERENCES ruvector.nodes(id) ON DELETE CASCADE,
    target_id       BIGINT NOT NULL REFERENCES ruvector.nodes(id) ON DELETE CASCADE,
    edge_type       TEXT NOT NULL DEFAULT 'default',
    weight          REAL NOT NULL DEFAULT 1.0,
    properties      JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CHECK (source_id <> target_id)
);

CREATE INDEX idx_edges_source ON ruvector.edges(graph_id, source_id);
CREATE INDEX idx_edges_target ON ruvector.edges(graph_id, target_id);
CREATE INDEX idx_edges_type ON ruvector.edges(graph_id, edge_type);
CREATE INDEX idx_edges_properties ON ruvector.edges USING gin(properties);

-- Hyperedges (connect multiple nodes)
CREATE TABLE ruvector.hyperedges (
    id              BIGSERIAL PRIMARY KEY,
    graph_id        INTEGER NOT NULL REFERENCES ruvector.graphs(id) ON DELETE CASCADE,
    hyperedge_type  TEXT NOT NULL DEFAULT 'default',
    node_ids        BIGINT[] NOT NULL,
    weights         REAL[],
    properties      JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CHECK (array_length(node_ids, 1) >= 2)
);

CREATE INDEX idx_hyperedges_graph ON ruvector.hyperedges(graph_id);
CREATE INDEX idx_hyperedges_nodes ON ruvector.hyperedges USING gin(node_ids);
CREATE INDEX idx_hyperedges_type ON ruvector.hyperedges(graph_id, hyperedge_type);
```

### 2. Cypher Parser

```rust
// src/graph/cypher/parser.rs

use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while1},
    character::complete::{alphanumeric1, char, multispace0, multispace1},
    combinator::{map, opt, recognize},
    multi::{many0, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, separated_pair, tuple},
    IResult,
};

/// Cypher AST node types
#[derive(Debug, Clone)]
pub enum CypherStatement {
    Match(MatchClause),
    Return(ReturnClause),
    Create(CreateClause),
    Delete(DeleteClause),
    Set(SetClause),
    Query(CypherQuery),
}

#[derive(Debug, Clone)]
pub struct CypherQuery {
    pub match_clause: Option<MatchClause>,
    pub where_clause: Option<WhereClause>,
    pub return_clause: Option<ReturnClause>,
    pub order_by: Option<OrderByClause>,
    pub limit: Option<usize>,
    pub skip: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct MatchClause {
    pub patterns: Vec<Pattern>,
    pub optional: bool,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Node(NodePattern),
    Path(PathPattern),
}

#[derive(Debug, Clone)]
pub struct NodePattern {
    pub variable: Option<String>,
    pub labels: Vec<String>,
    pub properties: Option<MapLiteral>,
}

#[derive(Debug, Clone)]
pub struct PathPattern {
    pub elements: Vec<PathElement>,
}

#[derive(Debug, Clone)]
pub enum PathElement {
    Node(NodePattern),
    Relationship(RelationshipPattern),
}

#[derive(Debug, Clone)]
pub struct RelationshipPattern {
    pub variable: Option<String>,
    pub types: Vec<String>,
    pub direction: Direction,
    pub properties: Option<MapLiteral>,
    pub length: Option<RelationshipLength>,
}

#[derive(Debug, Clone, Copy)]
pub enum Direction {
    Left,      // <-
    Right,     // ->
    Both,      // --
    Undirected, // --
}

#[derive(Debug, Clone)]
pub struct RelationshipLength {
    pub min: Option<usize>,
    pub max: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct WhereClause {
    pub expression: Expression,
}

#[derive(Debug, Clone)]
pub enum Expression {
    Literal(Literal),
    Property(PropertyAccess),
    Parameter(String),
    FunctionCall(FunctionCall),
    BinaryOp(Box<BinaryOp>),
    UnaryOp(Box<UnaryOp>),
    List(Vec<Expression>),
    Map(MapLiteral),
}

#[derive(Debug, Clone)]
pub struct BinaryOp {
    pub op: BinaryOperator,
    pub left: Expression,
    pub right: Expression,
}

#[derive(Debug, Clone, Copy)]
pub enum BinaryOperator {
    Eq, Ne, Lt, Gt, Le, Ge,
    And, Or, Xor,
    Add, Sub, Mul, Div, Mod,
    Contains, StartsWith, EndsWith,
    In,
    VectorDistance,  // Custom: <-> for vectors
    VectorSimilarity, // Custom: <=> for vectors
}

#[derive(Debug, Clone)]
pub struct PropertyAccess {
    pub variable: String,
    pub properties: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Literal {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

#[derive(Debug, Clone)]
pub struct MapLiteral {
    pub entries: Vec<(String, Expression)>,
}

#[derive(Debug, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub args: Vec<Expression>,
    pub distinct: bool,
}

#[derive(Debug, Clone)]
pub struct ReturnClause {
    pub items: Vec<ReturnItem>,
    pub distinct: bool,
}

#[derive(Debug, Clone)]
pub struct ReturnItem {
    pub expression: Expression,
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OrderByClause {
    pub items: Vec<OrderByItem>,
}

#[derive(Debug, Clone)]
pub struct OrderByItem {
    pub expression: Expression,
    pub ascending: bool,
}

/// Parse a Cypher query
pub fn parse_cypher(input: &str) -> Result<CypherQuery, ParseError> {
    match cypher_query(input) {
        Ok((_, query)) => Ok(query),
        Err(e) => Err(ParseError::SyntaxError(format!("{:?}", e))),
    }
}

fn cypher_query(input: &str) -> IResult<&str, CypherQuery> {
    let (input, _) = multispace0(input)?;

    let (input, match_clause) = opt(match_clause)(input)?;
    let (input, _) = multispace0(input)?;

    let (input, where_clause) = opt(where_clause)(input)?;
    let (input, _) = multispace0(input)?;

    let (input, return_clause) = opt(return_clause)(input)?;
    let (input, _) = multispace0(input)?;

    let (input, order_by) = opt(order_by_clause)(input)?;
    let (input, _) = multispace0(input)?;

    let (input, limit) = opt(limit_clause)(input)?;
    let (input, _) = multispace0(input)?;

    let (input, skip) = opt(skip_clause)(input)?;

    Ok((input, CypherQuery {
        match_clause,
        where_clause,
        return_clause,
        order_by,
        limit,
        skip,
    }))
}

fn match_clause(input: &str) -> IResult<&str, MatchClause> {
    let (input, optional) = opt(preceded(
        tuple((tag_no_case("OPTIONAL"), multispace1)),
        tag_no_case("")
    ))(input)?;

    let (input, _) = tag_no_case("MATCH")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, patterns) = separated_list1(
        tuple((multispace0, char(','), multispace0)),
        pattern
    )(input)?;

    Ok((input, MatchClause {
        patterns,
        optional: optional.is_some(),
    }))
}

fn pattern(input: &str) -> IResult<&str, Pattern> {
    alt((
        map(path_pattern, Pattern::Path),
        map(node_pattern, Pattern::Node),
    ))(input)
}

fn node_pattern(input: &str) -> IResult<&str, NodePattern> {
    let (input, _) = char('(')(input)?;
    let (input, _) = multispace0(input)?;

    let (input, variable) = opt(identifier)(input)?;
    let (input, labels) = many0(preceded(char(':'), identifier))(input)?;
    let (input, _) = multispace0(input)?;
    let (input, properties) = opt(map_literal)(input)?;

    let (input, _) = multispace0(input)?;
    let (input, _) = char(')')(input)?;

    Ok((input, NodePattern {
        variable,
        labels,
        properties,
    }))
}

fn path_pattern(input: &str) -> IResult<&str, PathPattern> {
    let (input, first_node) = node_pattern(input)?;
    let (input, rest) = many0(pair(relationship_pattern, node_pattern))(input)?;

    let mut elements = vec![PathElement::Node(first_node)];
    for (rel, node) in rest {
        elements.push(PathElement::Relationship(rel));
        elements.push(PathElement::Node(node));
    }

    Ok((input, PathPattern { elements }))
}

fn relationship_pattern(input: &str) -> IResult<&str, RelationshipPattern> {
    // Handle different arrow types: -[r:TYPE]->  <-[r:TYPE]-  -[r:TYPE]-
    let (input, left_arrow) = opt(char('<'))(input)?;
    let (input, _) = char('-')(input)?;

    let (input, details) = opt(delimited(
        char('['),
        relationship_details,
        char(']')
    ))(input)?;

    let (input, _) = char('-')(input)?;
    let (input, right_arrow) = opt(char('>'))(input)?;

    let direction = match (left_arrow.is_some(), right_arrow.is_some()) {
        (true, false) => Direction::Left,
        (false, true) => Direction::Right,
        (true, true) => Direction::Both,
        (false, false) => Direction::Undirected,
    };

    let (variable, types, properties, length) = details.unwrap_or((None, vec![], None, None));

    Ok((input, RelationshipPattern {
        variable,
        types,
        direction,
        properties,
        length,
    }))
}

fn relationship_details(
    input: &str
) -> IResult<&str, (Option<String>, Vec<String>, Option<MapLiteral>, Option<RelationshipLength>)> {
    let (input, _) = multispace0(input)?;
    let (input, variable) = opt(identifier)(input)?;
    let (input, types) = many0(preceded(char(':'), identifier))(input)?;
    let (input, length) = opt(relationship_length)(input)?;
    let (input, _) = multispace0(input)?;
    let (input, properties) = opt(map_literal)(input)?;
    let (input, _) = multispace0(input)?;

    Ok((input, (variable, types, properties, length)))
}

fn where_clause(input: &str) -> IResult<&str, WhereClause> {
    let (input, _) = tag_no_case("WHERE")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, expression) = expression(input)?;

    Ok((input, WhereClause { expression }))
}

fn return_clause(input: &str) -> IResult<&str, ReturnClause> {
    let (input, _) = tag_no_case("RETURN")(input)?;
    let (input, _) = multispace1(input)?;

    let (input, distinct) = opt(preceded(
        tag_no_case("DISTINCT"),
        multispace1
    ))(input)?;

    let (input, items) = separated_list1(
        tuple((multispace0, char(','), multispace0)),
        return_item
    )(input)?;

    Ok((input, ReturnClause {
        items,
        distinct: distinct.is_some(),
    }))
}

fn return_item(input: &str) -> IResult<&str, ReturnItem> {
    let (input, expression) = expression(input)?;
    let (input, alias) = opt(preceded(
        tuple((multispace1, tag_no_case("AS"), multispace1)),
        identifier
    ))(input)?;

    Ok((input, ReturnItem { expression, alias }))
}

fn identifier(input: &str) -> IResult<&str, String> {
    map(
        recognize(pair(
            alt((alphanumeric1, tag("_"))),
            many0(alt((alphanumeric1, tag("_"))))
        )),
        |s: &str| s.to_string()
    )(input)
}

fn expression(input: &str) -> IResult<&str, Expression> {
    // Simplified expression parser
    or_expression(input)
}

fn or_expression(input: &str) -> IResult<&str, Expression> {
    let (input, left) = and_expression(input)?;
    let (input, rest) = many0(preceded(
        tuple((multispace0, tag_no_case("OR"), multispace0)),
        and_expression
    ))(input)?;

    let result = rest.into_iter().fold(left, |acc, right| {
        Expression::BinaryOp(Box::new(BinaryOp {
            op: BinaryOperator::Or,
            left: acc,
            right,
        }))
    });

    Ok((input, result))
}

fn and_expression(input: &str) -> IResult<&str, Expression> {
    let (input, left) = comparison_expression(input)?;
    let (input, rest) = many0(preceded(
        tuple((multispace0, tag_no_case("AND"), multispace0)),
        comparison_expression
    ))(input)?;

    let result = rest.into_iter().fold(left, |acc, right| {
        Expression::BinaryOp(Box::new(BinaryOp {
            op: BinaryOperator::And,
            left: acc,
            right,
        }))
    });

    Ok((input, result))
}

fn comparison_expression(input: &str) -> IResult<&str, Expression> {
    let (input, left) = primary_expression(input)?;
    let (input, _) = multispace0(input)?;

    let (input, op_right) = opt(pair(
        comparison_operator,
        preceded(multispace0, primary_expression)
    ))(input)?;

    match op_right {
        Some((op, right)) => {
            Ok((input, Expression::BinaryOp(Box::new(BinaryOp {
                op,
                left,
                right,
            }))))
        }
        None => Ok((input, left)),
    }
}

fn comparison_operator(input: &str) -> IResult<&str, BinaryOperator> {
    alt((
        map(tag("<->"), |_| BinaryOperator::VectorDistance),
        map(tag("<=>"), |_| BinaryOperator::VectorSimilarity),
        map(tag("<="), |_| BinaryOperator::Le),
        map(tag(">="), |_| BinaryOperator::Ge),
        map(tag("<>"), |_| BinaryOperator::Ne),
        map(tag("!="), |_| BinaryOperator::Ne),
        map(char('<'), |_| BinaryOperator::Lt),
        map(char('>'), |_| BinaryOperator::Gt),
        map(char('='), |_| BinaryOperator::Eq),
    ))(input)
}

fn primary_expression(input: &str) -> IResult<&str, Expression> {
    alt((
        map(literal, Expression::Literal),
        map(parameter, Expression::Parameter),
        map(property_access, Expression::Property),
        map(function_call, Expression::FunctionCall),
    ))(input)
}

// ... Additional parser functions ...
```

### 3. Cypher Executor

```rust
// src/graph/cypher/executor.rs

use super::parser::*;
use std::collections::HashMap;

/// Execute a Cypher query
pub struct CypherExecutor {
    graph_id: i32,
    params: HashMap<String, serde_json::Value>,
}

impl CypherExecutor {
    pub fn new(graph_id: i32) -> Self {
        Self {
            graph_id,
            params: HashMap::new(),
        }
    }

    pub fn with_params(mut self, params: HashMap<String, serde_json::Value>) -> Self {
        self.params = params;
        self
    }

    /// Execute query and return results
    pub fn execute(&self, query: &CypherQuery) -> Result<Vec<serde_json::Value>, ExecutionError> {
        // Build execution plan
        let plan = self.plan(query)?;

        // Execute plan
        let mut context = ExecutionContext::new(self.graph_id, &self.params);
        let results = plan.execute(&mut context)?;

        // Format results according to RETURN clause
        self.format_results(results, query)
    }

    fn plan(&self, query: &CypherQuery) -> Result<ExecutionPlan, ExecutionError> {
        let mut plan = ExecutionPlan::new();

        // Add MATCH operations
        if let Some(ref match_clause) = query.match_clause {
            for pattern in &match_clause.patterns {
                plan.add_operation(self.plan_pattern(pattern)?);
            }
        }

        // Add WHERE filter
        if let Some(ref where_clause) = query.where_clause {
            plan.add_filter(where_clause.clone());
        }

        // Add ORDER BY
        if let Some(ref order_by) = query.order_by {
            plan.add_order_by(order_by.clone());
        }

        // Add LIMIT/SKIP
        if let Some(limit) = query.limit {
            plan.set_limit(limit);
        }
        if let Some(skip) = query.skip {
            plan.set_skip(skip);
        }

        Ok(plan)
    }

    fn plan_pattern(&self, pattern: &Pattern) -> Result<PatternOperation, ExecutionError> {
        match pattern {
            Pattern::Node(node) => {
                Ok(PatternOperation::ScanNodes {
                    variable: node.variable.clone(),
                    labels: node.labels.clone(),
                    properties: node.properties.clone(),
                })
            }
            Pattern::Path(path) => {
                self.plan_path_pattern(path)
            }
        }
    }

    fn plan_path_pattern(&self, path: &PathPattern) -> Result<PatternOperation, ExecutionError> {
        let mut operations = Vec::new();

        for (i, element) in path.elements.iter().enumerate() {
            match element {
                PathElement::Node(node) if i == 0 => {
                    operations.push(PatternOperation::ScanNodes {
                        variable: node.variable.clone(),
                        labels: node.labels.clone(),
                        properties: node.properties.clone(),
                    });
                }
                PathElement::Relationship(rel) => {
                    let next_node = match path.elements.get(i + 1) {
                        Some(PathElement::Node(n)) => n.clone(),
                        _ => return Err(ExecutionError::InvalidPattern),
                    };

                    operations.push(PatternOperation::Traverse {
                        rel_variable: rel.variable.clone(),
                        rel_types: rel.types.clone(),
                        direction: rel.direction,
                        target_variable: next_node.variable.clone(),
                        target_labels: next_node.labels.clone(),
                    });
                }
                _ => {}
            }
        }

        Ok(PatternOperation::PathMatch { operations })
    }

    fn format_results(
        &self,
        results: Vec<ResultRow>,
        query: &CypherQuery,
    ) -> Result<Vec<serde_json::Value>, ExecutionError> {
        let return_clause = query.return_clause.as_ref()
            .ok_or(ExecutionError::NoReturnClause)?;

        results.into_iter()
            .map(|row| {
                let mut obj = serde_json::Map::new();

                for item in &return_clause.items {
                    let key = item.alias.clone()
                        .unwrap_or_else(|| format_expression(&item.expression));
                    let value = evaluate_expression(&item.expression, &row)?;
                    obj.insert(key, value);
                }

                Ok(serde_json::Value::Object(obj))
            })
            .collect()
    }
}

/// Execution plan
struct ExecutionPlan {
    operations: Vec<PatternOperation>,
    filter: Option<WhereClause>,
    order_by: Option<OrderByClause>,
    limit: Option<usize>,
    skip: Option<usize>,
}

impl ExecutionPlan {
    fn new() -> Self {
        Self {
            operations: Vec::new(),
            filter: None,
            order_by: None,
            limit: None,
            skip: None,
        }
    }

    fn add_operation(&mut self, op: PatternOperation) {
        self.operations.push(op);
    }

    fn add_filter(&mut self, filter: WhereClause) {
        self.filter = Some(filter);
    }

    fn add_order_by(&mut self, order_by: OrderByClause) {
        self.order_by = Some(order_by);
    }

    fn set_limit(&mut self, limit: usize) {
        self.limit = Some(limit);
    }

    fn set_skip(&mut self, skip: usize) {
        self.skip = Some(skip);
    }

    fn execute(&self, context: &mut ExecutionContext) -> Result<Vec<ResultRow>, ExecutionError> {
        let mut results = Vec::new();

        // Execute pattern operations
        for op in &self.operations {
            let op_results = op.execute(context)?;
            results = if results.is_empty() {
                op_results
            } else {
                // Cross-product or join based on shared variables
                join_results(results, op_results, context)
            };
        }

        // Apply filter
        if let Some(ref filter) = self.filter {
            results = results.into_iter()
                .filter(|row| evaluate_predicate(&filter.expression, row).unwrap_or(false))
                .collect();
        }

        // Apply ORDER BY
        if let Some(ref order_by) = self.order_by {
            sort_results(&mut results, order_by)?;
        }

        // Apply SKIP
        if let Some(skip) = self.skip {
            results = results.into_iter().skip(skip).collect();
        }

        // Apply LIMIT
        if let Some(limit) = self.limit {
            results = results.into_iter().take(limit).collect();
        }

        Ok(results)
    }
}

enum PatternOperation {
    ScanNodes {
        variable: Option<String>,
        labels: Vec<String>,
        properties: Option<MapLiteral>,
    },
    Traverse {
        rel_variable: Option<String>,
        rel_types: Vec<String>,
        direction: Direction,
        target_variable: Option<String>,
        target_labels: Vec<String>,
    },
    PathMatch {
        operations: Vec<PatternOperation>,
    },
}

impl PatternOperation {
    fn execute(&self, context: &mut ExecutionContext) -> Result<Vec<ResultRow>, ExecutionError> {
        match self {
            PatternOperation::ScanNodes { variable, labels, properties } => {
                scan_nodes(context, variable, labels, properties)
            }
            PatternOperation::Traverse { rel_variable, rel_types, direction, target_variable, target_labels } => {
                traverse_edges(context, rel_variable, rel_types, *direction, target_variable, target_labels)
            }
            PatternOperation::PathMatch { operations } => {
                let mut results = Vec::new();
                for op in operations {
                    let op_results = op.execute(context)?;
                    results = if results.is_empty() {
                        op_results
                    } else {
                        extend_paths(results, op_results)?
                    };
                }
                Ok(results)
            }
        }
    }
}

fn scan_nodes(
    context: &ExecutionContext,
    variable: &Option<String>,
    labels: &[String],
    properties: &Option<MapLiteral>,
) -> Result<Vec<ResultRow>, ExecutionError> {
    Spi::connect(|client| {
        let mut query = format!(
            "SELECT id, external_id, node_type, properties, vector_ref, collection_id
             FROM ruvector.nodes WHERE graph_id = $1"
        );
        let mut params: Vec<_> = vec![context.graph_id.into()];

        // Filter by labels (node_type)
        if !labels.is_empty() {
            query.push_str(&format!(
                " AND node_type IN ({})",
                labels.iter()
                    .enumerate()
                    .map(|(i, _)| format!("${}", i + 2))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
            for label in labels {
                params.push(label.clone().into());
            }
        }

        // Filter by properties
        if let Some(props) = properties {
            for (key, value) in &props.entries {
                let idx = params.len() + 1;
                query.push_str(&format!(
                    " AND properties->>'{}' = ${}",
                    key, idx
                ));
                params.push(literal_to_param(value));
            }
        }

        let results = client.select(&query, None, &params)?;

        results.map(|row| {
            let mut result_row = ResultRow::new();

            if let Some(var) = variable {
                result_row.set(var, serde_json::json!({
                    "id": row.get::<i64>(1)?,
                    "external_id": row.get::<Option<String>>(2)?,
                    "labels": vec![row.get::<String>(3)?],
                    "properties": row.get::<pgrx::JsonB>(4)?.0,
                    "_vector_ref": row.get::<Option<String>>(5)?,
                    "_collection_id": row.get::<Option<i32>>(6)?,
                }));
            }

            Ok(result_row)
        }).collect()
    })
}

fn traverse_edges(
    context: &ExecutionContext,
    rel_variable: &Option<String>,
    rel_types: &[String],
    direction: Direction,
    target_variable: &Option<String>,
    target_labels: &[String],
) -> Result<Vec<ResultRow>, ExecutionError> {
    // Implementation would query ruvector.edges and join with nodes
    // Based on direction and type constraints
    todo!("Implement edge traversal")
}
```

### 4. SQL Functions

```sql
-- Execute Cypher query
CREATE FUNCTION ruvector_cypher(
    p_graph_name TEXT,
    p_query TEXT,
    p_params JSONB DEFAULT '{}'::jsonb
) RETURNS SETOF JSONB AS 'MODULE_PATHNAME', 'ruvector_cypher' LANGUAGE C;

-- Create graph
CREATE FUNCTION ruvector_graph_create(
    p_name TEXT,
    p_description TEXT DEFAULT NULL
) RETURNS INTEGER AS $$
DECLARE
    v_id INTEGER;
BEGIN
    INSERT INTO ruvector.graphs (name, description)
    VALUES (p_name, p_description)
    RETURNING id INTO v_id;
    RETURN v_id;
END;
$$ LANGUAGE plpgsql;

-- Delete graph
CREATE FUNCTION ruvector_graph_delete(p_name TEXT) RETURNS BOOLEAN AS $$
BEGIN
    DELETE FROM ruvector.graphs WHERE name = p_name;
    RETURN FOUND;
END;
$$ LANGUAGE plpgsql;

-- Add node
CREATE FUNCTION ruvector_node_add(
    p_graph_name TEXT,
    p_external_id TEXT,
    p_node_type TEXT DEFAULT 'default',
    p_properties JSONB DEFAULT '{}'::jsonb,
    p_vector_table TEXT DEFAULT NULL,
    p_vector_column TEXT DEFAULT NULL,
    p_vector_id TEXT DEFAULT NULL
) RETURNS BIGINT AS $$
DECLARE
    v_graph_id INTEGER;
    v_node_id BIGINT;
    v_vector_ref TID;
    v_collection_id INTEGER;
BEGIN
    SELECT id INTO v_graph_id FROM ruvector.graphs WHERE name = p_graph_name;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Graph not found: %', p_graph_name;
    END IF;

    -- Get vector reference if specified
    IF p_vector_table IS NOT NULL AND p_vector_id IS NOT NULL THEN
        EXECUTE format(
            'SELECT ctid FROM %I WHERE id = $1',
            p_vector_table
        ) INTO v_vector_ref USING p_vector_id;

        SELECT id INTO v_collection_id
        FROM ruvector.collections
        WHERE table_name = p_vector_table AND column_name = COALESCE(p_vector_column, 'embedding');
    END IF;

    INSERT INTO ruvector.nodes (graph_id, external_id, node_type, properties, vector_ref, collection_id)
    VALUES (v_graph_id, p_external_id, p_node_type, p_properties, v_vector_ref, v_collection_id)
    RETURNING id INTO v_node_id;

    UPDATE ruvector.graphs SET node_count = node_count + 1, updated_at = NOW()
    WHERE id = v_graph_id;

    RETURN v_node_id;
END;
$$ LANGUAGE plpgsql;

-- Add edge
CREATE FUNCTION ruvector_edge_add(
    p_graph_name TEXT,
    p_source_external_id TEXT,
    p_target_external_id TEXT,
    p_edge_type TEXT DEFAULT 'default',
    p_weight REAL DEFAULT 1.0,
    p_properties JSONB DEFAULT '{}'::jsonb
) RETURNS BIGINT AS $$
DECLARE
    v_graph_id INTEGER;
    v_source_id BIGINT;
    v_target_id BIGINT;
    v_edge_id BIGINT;
BEGIN
    SELECT id INTO v_graph_id FROM ruvector.graphs WHERE name = p_graph_name;

    SELECT id INTO v_source_id FROM ruvector.nodes
    WHERE graph_id = v_graph_id AND external_id = p_source_external_id;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Source node not found: %', p_source_external_id;
    END IF;

    SELECT id INTO v_target_id FROM ruvector.nodes
    WHERE graph_id = v_graph_id AND external_id = p_target_external_id;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Target node not found: %', p_target_external_id;
    END IF;

    INSERT INTO ruvector.edges (graph_id, source_id, target_id, edge_type, weight, properties)
    VALUES (v_graph_id, v_source_id, v_target_id, p_edge_type, p_weight, p_properties)
    RETURNING id INTO v_edge_id;

    UPDATE ruvector.graphs SET edge_count = edge_count + 1, updated_at = NOW()
    WHERE id = v_graph_id;

    RETURN v_edge_id;
END;
$$ LANGUAGE plpgsql;
```

### 5. Relational Bridge Views

```sql
-- Unified node view with vector data
CREATE VIEW ruvector.nodes_view AS
SELECT
    n.id,
    n.graph_id,
    g.name AS graph_name,
    n.external_id,
    n.node_type AS label,
    n.properties,
    n.created_at,
    c.table_schema || '.' || c.table_name AS vector_table,
    c.column_name AS vector_column,
    n.vector_ref
FROM ruvector.nodes n
JOIN ruvector.graphs g ON n.graph_id = g.id
LEFT JOIN ruvector.collections c ON n.collection_id = c.id;

-- Edge view with full details
CREATE VIEW ruvector.edges_view AS
SELECT
    e.id,
    e.graph_id,
    g.name AS graph_name,
    e.source_id,
    src.external_id AS source_external_id,
    src.node_type AS source_label,
    e.target_id,
    tgt.external_id AS target_external_id,
    tgt.node_type AS target_label,
    e.edge_type,
    e.weight,
    e.properties,
    e.created_at
FROM ruvector.edges e
JOIN ruvector.graphs g ON e.graph_id = g.id
JOIN ruvector.nodes src ON e.source_id = src.id
JOIN ruvector.nodes tgt ON e.target_id = tgt.id;

-- Adjacency list view for SQL-based traversals
CREATE VIEW ruvector.adjacency_list AS
SELECT
    g.name AS graph_name,
    src.external_id AS source,
    e.edge_type,
    tgt.external_id AS target,
    e.weight
FROM ruvector.edges e
JOIN ruvector.graphs g ON e.graph_id = g.id
JOIN ruvector.nodes src ON e.source_id = src.id
JOIN ruvector.nodes tgt ON e.target_id = tgt.id;

-- Function to get neighbors in SQL
CREATE FUNCTION ruvector_neighbors(
    p_graph_name TEXT,
    p_node_external_id TEXT,
    p_edge_types TEXT[] DEFAULT NULL,
    p_direction TEXT DEFAULT 'both'  -- 'in', 'out', 'both'
) RETURNS TABLE (
    neighbor_id BIGINT,
    neighbor_external_id TEXT,
    neighbor_label TEXT,
    edge_type TEXT,
    edge_weight REAL,
    direction TEXT
) AS $$
BEGIN
    RETURN QUERY
    WITH graph AS (
        SELECT id FROM ruvector.graphs WHERE name = p_graph_name
    ),
    source_node AS (
        SELECT n.id FROM ruvector.nodes n, graph g
        WHERE n.graph_id = g.id AND n.external_id = p_node_external_id
    )
    SELECT
        n.id,
        n.external_id,
        n.node_type,
        e.edge_type,
        e.weight,
        CASE
            WHEN e.source_id = s.id THEN 'out'
            ELSE 'in'
        END
    FROM ruvector.edges e
    JOIN source_node s ON (e.source_id = s.id OR e.target_id = s.id)
    JOIN ruvector.nodes n ON (
        CASE
            WHEN e.source_id = s.id THEN e.target_id = n.id
            ELSE e.source_id = n.id
        END
    )
    WHERE (p_edge_types IS NULL OR e.edge_type = ANY(p_edge_types))
      AND (
          p_direction = 'both'
          OR (p_direction = 'out' AND e.source_id = s.id)
          OR (p_direction = 'in' AND e.target_id = s.id)
      );
END;
$$ LANGUAGE plpgsql;
```

---

## Usage Examples

### Basic Cypher Queries

```sql
-- Create a graph
SELECT ruvector_graph_create('social', 'Social network graph');

-- Add nodes
SELECT ruvector_node_add('social', 'alice', 'Person', '{"name": "Alice", "age": 30}');
SELECT ruvector_node_add('social', 'bob', 'Person', '{"name": "Bob", "age": 25}');
SELECT ruvector_node_add('social', 'charlie', 'Person', '{"name": "Charlie", "age": 35}');

-- Add edges
SELECT ruvector_edge_add('social', 'alice', 'bob', 'KNOWS', 1.0);
SELECT ruvector_edge_add('social', 'bob', 'charlie', 'KNOWS', 1.0);
SELECT ruvector_edge_add('social', 'alice', 'charlie', 'FOLLOWS', 0.5);

-- Query with Cypher
SELECT * FROM ruvector_cypher('social', '
    MATCH (a:Person)-[:KNOWS]->(b:Person)
    WHERE a.age > 25
    RETURN a.name AS person, b.name AS knows
');

-- Path queries
SELECT * FROM ruvector_cypher('social', '
    MATCH path = (a:Person)-[:KNOWS*1..3]->(b:Person)
    WHERE a.name = "Alice"
    RETURN a.name, b.name, length(path) AS distance
');
```

### Vector-Enriched Graph Queries

```sql
-- Create nodes linked to vectors
SELECT ruvector_node_add(
    'social',
    'alice',
    'Person',
    '{"name": "Alice"}',
    'user_embeddings',  -- table with vectors
    'embedding',         -- vector column
    '1'                  -- user ID
);

-- Query by vector similarity
SELECT * FROM ruvector_cypher('social', '
    MATCH (a:Person)
    WHERE a._vector <=> $query_vector < 0.5
    RETURN a.name, a._vector <=> $query_vector AS similarity
    ORDER BY similarity
    LIMIT 10
', '{"query_vector": [0.1, 0.2, 0.3, ...]}'::jsonb);
```

### SQL-Graph Mixing

```sql
-- Use SQL views with graph data
SELECT
    nv.external_id,
    nv.label,
    nv.properties->>'name' AS name,
    COUNT(DISTINCT ev.target_id) AS connection_count
FROM ruvector.nodes_view nv
LEFT JOIN ruvector.edges_view ev ON nv.id = ev.source_id
WHERE nv.graph_name = 'social'
GROUP BY nv.id, nv.external_id, nv.label, nv.properties
ORDER BY connection_count DESC;

-- Join graph data with user table
SELECT
    u.id,
    u.username,
    n.properties->>'score' AS graph_score,
    COUNT(e.id) AS edge_count
FROM users u
JOIN ruvector.nodes n ON n.external_id = u.id::text
LEFT JOIN ruvector.edges e ON e.source_id = n.id
WHERE n.graph_id = (SELECT id FROM ruvector.graphs WHERE name = 'social')
GROUP BY u.id, u.username, n.properties;
```

---

## Testing Requirements

### Unit Tests
- Cypher parser coverage
- Expression evaluation
- Pattern matching

### Integration Tests
- Graph CRUD operations
- Cypher execution
- Vector integration
- SQL view queries

### Performance Tests
- Large graph traversals
- Index effectiveness
- Memory usage

---

## Timeline

| Week | Deliverable |
|------|-------------|
| 9 | Graph storage schema |
| 10 | Cypher parser |
| 11 | Cypher executor |
| 12 | SQL functions and views |
