# Cypher Query Language Parser for RuVector

A complete Cypher-compatible query language parser implementation for the RuVector graph database, built using the nom parser combinator library.

## Overview

This module provides a full-featured Cypher query parser that converts Cypher query text into an Abstract Syntax Tree (AST) suitable for execution. It includes:

- **Lexical Analysis** (`lexer.rs`): Tokenizes Cypher query strings
- **Syntax Parsing** (`parser.rs`): Recursive descent parser using nom
- **AST Definitions** (`ast.rs`): Complete type system for Cypher queries
- **Semantic Analysis** (`semantic.rs`): Type checking and validation
- **Query Optimization** (`optimizer.rs`): Query plan optimization

## Supported Cypher Features

### Pattern Matching
```cypher
MATCH (n:Person)
MATCH (a:Person)-[r:KNOWS]->(b:Person)
OPTIONAL MATCH (n)-[r]->()
```

### Hyperedges (N-ary Relationships)
```cypher
-- Transaction involving multiple parties
MATCH (person)-[r:TRANSACTION]->(acc1:Account, acc2:Account, merchant:Merchant)
WHERE r.amount > 1000
RETURN person, r, acc1, acc2, merchant
```

### Filtering
```cypher
WHERE n.age > 30 AND n.name = 'Alice'
WHERE n.age >= 18 OR n.verified = true
```

### Projections and Aggregations
```cypher
RETURN n.name, n.age
RETURN COUNT(n), AVG(n.age), MAX(n.salary), COLLECT(n.name)
RETURN DISTINCT n.department
```

### Mutations
```cypher
CREATE (n:Person {name: 'Bob', age: 30})
MERGE (n:Person {email: 'alice@example.com'})
  ON CREATE SET n.created = timestamp()
  ON MATCH SET n.accessed = timestamp()
DELETE n
DETACH DELETE n
SET n.age = 31, n.updated = timestamp()
```

### Query Chaining
```cypher
MATCH (n:Person)
WITH n, n.age AS age
WHERE age > 30
RETURN n.name, age
ORDER BY age DESC
LIMIT 10
```

### Path Patterns
```cypher
MATCH p = (a:Person)-[*1..5]->(b:Person)
RETURN p
```

### Advanced Expressions
```cypher
CASE
  WHEN n.age < 18 THEN 'minor'
  WHEN n.age < 65 THEN 'adult'
  ELSE 'senior'
END
```

## Architecture

### 1. Lexer (`lexer.rs`)

The lexer converts raw text into a stream of tokens:

```rust
use ruvector_graph::cypher::lexer::tokenize;

let tokens = tokenize("MATCH (n:Person) RETURN n")?;
// Returns: [MATCH, (, Identifier("n"), :, Identifier("Person"), ), RETURN, Identifier("n")]
```

**Features:**
- Full Cypher keyword support
- String literals (single and double quoted)
- Numeric literals (integers and floats with scientific notation)
- Operators and delimiters
- Position tracking for error reporting

### 2. Parser (`parser.rs`)

Recursive descent parser using nom combinators:

```rust
use ruvector_graph::cypher::parse_cypher;

let query = "MATCH (n:Person) WHERE n.age > 30 RETURN n.name";
let ast = parse_cypher(query)?;
```

**Features:**
- Error recovery and detailed error messages
- Support for all Cypher clauses
- Hyperedge pattern recognition
- Operator precedence handling
- Property map parsing

### 3. AST (`ast.rs`)

Complete Abstract Syntax Tree representation:

```rust
pub struct Query {
    pub statements: Vec<Statement>,
}

pub enum Statement {
    Match(MatchClause),
    Create(CreateClause),
    Merge(MergeClause),
    Delete(DeleteClause),
    Set(SetClause),
    Return(ReturnClause),
    With(WithClause),
}

// Hyperedge support for N-ary relationships
pub struct HyperedgePattern {
    pub variable: Option<String>,
    pub rel_type: String,
    pub properties: Option<PropertyMap>,
    pub from: Box<NodePattern>,
    pub to: Vec<NodePattern>,  // Multiple targets
    pub arity: usize,           // N-ary degree
}
```

**Key Types:**
- `Pattern`: Node, Relationship, Path, and Hyperedge patterns
- `Expression`: Full expression tree with operators and functions
- `AggregationFunction`: COUNT, SUM, AVG, MIN, MAX, COLLECT
- `BinaryOperator`: Arithmetic, comparison, logical, string operations

### 4. Semantic Analyzer (`semantic.rs`)

Type checking and validation:

```rust
use ruvector_graph::cypher::semantic::SemanticAnalyzer;

let mut analyzer = SemanticAnalyzer::new();
analyzer.analyze_query(&ast)?;
```

**Checks:**
- Variable scope and lifetime
- Type compatibility
- Aggregation context validation
- Hyperedge validity (minimum 2 target nodes)
- Pattern correctness

### 5. Query Optimizer (`optimizer.rs`)

Query plan optimization:

```rust
use ruvector_graph::cypher::optimizer::QueryOptimizer;

let optimizer = QueryOptimizer::new();
let plan = optimizer.optimize(query);

println!("Optimizations: {:?}", plan.optimizations_applied);
println!("Estimated cost: {}", plan.estimated_cost);
```

**Optimizations:**
- **Constant Folding**: Evaluate constant expressions at parse time
- **Predicate Pushdown**: Move filters closer to data access
- **Join Reordering**: Minimize intermediate result sizes
- **Selectivity Estimation**: Optimize pattern matching order

## Usage Examples

### Basic Query Parsing

```rust
use ruvector_graph::cypher::{parse_cypher, Query};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let query = r#"
        MATCH (person:Person)-[knows:KNOWS]->(friend:Person)
        WHERE person.age > 25 AND friend.city = 'NYC'
        RETURN person.name, friend.name, knows.since
        ORDER BY knows.since DESC
        LIMIT 10
    "#;

    let ast = parse_cypher(query)?;

    println!("Parsed {} statements", ast.statements.len());
    println!("Read-only query: {}", ast.is_read_only());

    Ok(())
}
```

### Hyperedge Queries

```rust
use ruvector_graph::cypher::parse_cypher;

// Parse a hyperedge pattern (N-ary relationship)
let query = r#"
    MATCH (buyer:Person)-[txn:PURCHASE]->(
        product:Product,
        seller:Person,
        warehouse:Location
    )
    WHERE txn.amount > 100
    RETURN buyer, product, seller, warehouse, txn.timestamp
"#;

let ast = parse_cypher(query)?;
assert!(ast.has_hyperedges());
```

### Semantic Analysis

```rust
use ruvector_graph::cypher::{parse_cypher, semantic::SemanticAnalyzer};

let query = "MATCH (n:Person) RETURN COUNT(n), AVG(n.age)";
let ast = parse_cypher(query)?;

let mut analyzer = SemanticAnalyzer::new();
match analyzer.analyze_query(&ast) {
    Ok(()) => println!("Query is semantically valid"),
    Err(e) => eprintln!("Semantic error: {}", e),
}
```

### Query Optimization

```rust
use ruvector_graph::cypher::{parse_cypher, optimizer::QueryOptimizer};

let query = r#"
    MATCH (a:Person), (b:Person)
    WHERE a.age > 30 AND b.name = 'Alice' AND 2 + 2 = 4
    RETURN a, b
"#;

let ast = parse_cypher(query)?;
let optimizer = QueryOptimizer::new();
let plan = optimizer.optimize(ast);

println!("Applied optimizations: {:?}", plan.optimizations_applied);
println!("Estimated execution cost: {:.2}", plan.estimated_cost);
```

## Hyperedge Support

Traditional graph databases represent relationships as binary edges (one source, one target). RuVector's Cypher parser supports **hyperedges** - relationships connecting multiple nodes simultaneously.

### Why Hyperedges?

- **Multi-party Transactions**: Model transfers involving multiple accounts
- **Complex Events**: Represent events with multiple participants
- **N-way Relationships**: Natural representation of real-world scenarios

### Hyperedge Syntax

```cypher
-- Create a 3-way transaction
CREATE (alice:Person)-[t:TRANSFER {amount: 100}]->(
    bob:Person,
    carol:Person
)

-- Match complex patterns
MATCH (author:Person)-[collab:AUTHORED]->(
    paper:Paper,
    coauthor1:Person,
    coauthor2:Person
)
RETURN author, paper, coauthor1, coauthor2

-- Hyperedge with properties
MATCH (teacher)-[class:TEACHES {semester: 'Fall2024'}]->(
    student1, student2, student3, course:Course
)
WHERE course.level = 'Graduate'
RETURN teacher, course, student1, student2, student3
```

### Hyperedge AST

```rust
pub struct HyperedgePattern {
    pub variable: Option<String>,    // Optional variable binding
    pub rel_type: String,             // Relationship type (required)
    pub properties: Option<PropertyMap>, // Optional properties
    pub from: Box<NodePattern>,       // Source node
    pub to: Vec<NodePattern>,         // Multiple target nodes (>= 2)
    pub arity: usize,                 // Total nodes (source + targets)
}
```

## Error Handling

The parser provides detailed error messages with position information:

```rust
use ruvector_graph::cypher::parse_cypher;

match parse_cypher("MATCH (n:Person WHERE n.age > 30") {
    Ok(ast) => { /* ... */ },
    Err(e) => {
        eprintln!("Parse error: {}", e);
        // Output: "Unexpected token: expected ), found WHERE at line 1, column 17"
    }
}
```

## Performance

- **Lexer**: ~500ns per token on average
- **Parser**: ~50-200μs for typical queries
- **Optimization**: ~10-50μs for plan generation

Benchmarks available in `benches/cypher_parser.rs`:

```bash
cargo bench --package ruvector-graph --bench cypher_parser
```

## Testing

Comprehensive test coverage across all modules:

```bash
# Run all Cypher tests
cargo test --package ruvector-graph --lib cypher

# Run parser integration tests
cargo test --package ruvector-graph --test cypher_parser_integration

# Run specific test
cargo test --package ruvector-graph test_hyperedge_pattern
```

## Implementation Details

### Nom Parser Combinators

The parser uses [nom](https://github.com/Geal/nom), a Rust parser combinator library:

```rust
fn parse_node_pattern(input: &str) -> IResult<&str, NodePattern> {
    preceded(
        char('('),
        terminated(
            parse_node_content,
            char(')')
        )
    )(input)
}
```

**Benefits:**
- Zero-copy parsing
- Composable parsers
- Excellent error handling
- Type-safe combinators

### Type System

The semantic analyzer implements a simple type system:

```rust
pub enum ValueType {
    Integer, Float, String, Boolean, Null,
    Node, Relationship, Path,
    List(Box<ValueType>),
    Map,
    Any,
}
```

Type compatibility checks ensure query correctness before execution.

### Cost-Based Optimization

The optimizer estimates query cost based on:

1. **Pattern Selectivity**: More specific patterns are cheaper
2. **Index Availability**: Indexed properties reduce scan cost
3. **Cardinality Estimates**: Smaller intermediate results are better
4. **Operation Cost**: Aggregations, sorts, and joins have inherent costs

## Future Enhancements

- [ ] Subqueries (CALL {...})
- [ ] User-defined functions
- [ ] Graph projections
- [ ] Pattern comprehensions
- [ ] JIT compilation for hot paths
- [ ] Parallel query execution
- [ ] Advanced cost-based optimization
- [ ] Query result caching

## References

- [Cypher Query Language Reference](https://neo4j.com/docs/cypher-manual/current/)
- [openCypher](http://www.opencypher.org/) - Open specification
- [GQL Standard](https://www.gqlstandards.org/) - ISO graph query language

## License

MIT License - See LICENSE file for details
