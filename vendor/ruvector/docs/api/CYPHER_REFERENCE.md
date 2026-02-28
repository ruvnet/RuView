# Cypher Query Language Reference

RuVector implements a Neo4j-compatible Cypher query language with extensions for hyperedges.

## Quick Examples

```cypher
-- Create nodes
CREATE (p:Person {name: 'Alice', age: 30})

-- Create relationships
CREATE (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person {name: 'Bob'})

-- Pattern matching
MATCH (p:Person)-[:KNOWS]->(friend)
WHERE p.name = 'Alice'
RETURN friend.name

-- Hyperedges (N-ary relationships)
CREATE (a:Person)-[:ATTENDED]->(meeting:Meeting, room:Room, company:Company)
```

## Supported Clauses

### MATCH

Find patterns in the graph.

```cypher
-- Simple node
MATCH (n:Person)
RETURN n

-- With relationship
MATCH (a:Person)-[:KNOWS]->(b:Person)
RETURN a.name, b.name

-- Variable-length paths
MATCH (a)-[*1..3]->(b)
RETURN a, b

-- Optional matching
OPTIONAL MATCH (p:Person)-[:OWNS]->(c:Car)
RETURN p.name, c.model
```

### CREATE

Create new nodes and relationships.

```cypher
-- Single node
CREATE (n:Person {name: 'Alice'})

-- Multiple labels
CREATE (n:Person:Employee {name: 'Bob'})

-- With relationship
CREATE (a:Person {name: 'Alice'})-[:FRIEND]->(b:Person {name: 'Bob'})
```

### MERGE

Create if not exists, match if exists.

```cypher
MERGE (p:Person {email: 'alice@example.com'})
ON CREATE SET p.created = timestamp()
ON MATCH SET p.lastSeen = timestamp()
```

### SET

Update properties.

```cypher
MATCH (p:Person {name: 'Alice'})
SET p.age = 31, p.updated = timestamp()
```

### DELETE

Remove nodes and relationships.

```cypher
-- Delete node (must have no relationships)
MATCH (p:Person {name: 'Temp'})
DELETE p

-- Delete node and all relationships
MATCH (p:Person {name: 'Temp'})
DETACH DELETE p
```

### RETURN

Project results.

```cypher
MATCH (p:Person)
RETURN p.name AS name, p.age
ORDER BY p.age DESC
SKIP 10
LIMIT 5
```

### WITH

Intermediate projection (query chaining).

```cypher
MATCH (p:Person)
WITH p.name AS name, COUNT(*) AS count
WHERE count > 1
RETURN name, count
```

### WHERE

Filter results.

```cypher
MATCH (p:Person)
WHERE p.age > 21 AND p.city = 'NYC'
RETURN p

-- Pattern predicates
WHERE (p)-[:KNOWS]->(:Expert)
```

## Hyperedges (N-ary Relationships)

RuVector extends Cypher with hyperedge support for N-ary relationships:

```cypher
-- Create hyperedge (3+ nodes)
CREATE (author:Person)-[:WROTE]->(paper:Paper, journal:Journal, year:Year)

-- Match hyperedge
MATCH (a:Person)-[r:ATTENDED]->(meeting, room, company)
RETURN a.name, meeting.topic, room.number
```

## Expressions

### Operators

| Type | Operators |
|------|-----------|
| Arithmetic | `+`, `-`, `*`, `/`, `%`, `^` |
| Comparison | `=`, `<>`, `<`, `>`, `<=`, `>=` |
| Logical | `AND`, `OR`, `NOT`, `XOR` |
| String | `STARTS WITH`, `ENDS WITH`, `CONTAINS` |
| Null | `IS NULL`, `IS NOT NULL` |
| List | `IN`, `[]` (indexing) |

### Functions

```cypher
-- String
RETURN toUpper(name), toLower(name), trim(name), substring(name, 0, 5)

-- Numeric
RETURN abs(x), ceil(x), floor(x), round(x), sqrt(x)

-- Collections
RETURN size(list), head(list), tail(list), range(1, 10)

-- Type
RETURN type(r), labels(n), keys(n)
```

### Aggregations

```cypher
MATCH (p:Person)
RETURN
  COUNT(*) AS total,
  AVG(p.age) AS avgAge,
  MIN(p.age) AS minAge,
  MAX(p.age) AS maxAge,
  SUM(p.salary) AS totalSalary,
  COLLECT(p.name) AS names
```

### CASE Expressions

```cypher
MATCH (p:Person)
RETURN p.name,
  CASE
    WHEN p.age < 18 THEN 'minor'
    WHEN p.age < 65 THEN 'adult'
    ELSE 'senior'
  END AS category
```

## Data Types

| Type | Example |
|------|---------|
| Integer | `42`, `-17` |
| Float | `3.14`, `-2.5e10` |
| String | `'hello'`, `"world"` |
| Boolean | `true`, `false` |
| Null | `null` |
| List | `[1, 2, 3]`, `['a', 'b']` |
| Map | `{name: 'Alice', age: 30}` |

## Path Variables

```cypher
-- Assign path to variable
MATCH p = (a:Person)-[:KNOWS*]->(b:Person)
RETURN p, length(p)

-- Path functions
RETURN nodes(p), relationships(p), length(p)
```

## Best Practices

1. **Use labels** - Always specify node labels for faster lookups
2. **Index properties** - Create indexes on frequently queried properties
3. **Limit results** - Use `LIMIT` to avoid large result sets
4. **Parameterize** - Use parameters for values to enable query caching

## See Also

- [Getting Started Guide](../guide/GETTING_STARTED.md)
- [Node.js API](./NODEJS_API.md)
- [Rust API](./RUST_API.md)
- [GNN Architecture](../gnn-layer-implementation.md)
