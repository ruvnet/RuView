# SPARQL PostgreSQL Implementation Guide

**Project**: RuVector-Postgres SPARQL Extension
**Date**: December 2025
**Status**: Research Phase

---

## Overview

This document outlines the implementation strategy for adding SPARQL query capabilities to RuVector-Postgres, enabling semantic graph queries alongside existing vector search operations.

---

## Architecture Overview

### Components

```
┌─────────────────────────────────────────────────────────────┐
│                    SPARQL Interface                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Query Parser │  │ Query Algebra│  │ SQL Generator│      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│                  RDF Triple Store Layer                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │ Triple Store │  │   Indexes    │  │ Named Graphs │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│                     PostgreSQL Layer                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   Tables     │  │   Indexes    │  │  Functions   │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Data Model

### Triple Store Schema

```sql
-- Main triple store table
CREATE TABLE ruvector_rdf_triples (
    id BIGSERIAL PRIMARY KEY,

    -- Subject
    subject TEXT NOT NULL,
    subject_type VARCHAR(10) NOT NULL CHECK (subject_type IN ('iri', 'bnode')),

    -- Predicate (always IRI)
    predicate TEXT NOT NULL,

    -- Object
    object TEXT NOT NULL,
    object_type VARCHAR(10) NOT NULL CHECK (object_type IN ('iri', 'literal', 'bnode')),
    object_datatype TEXT,
    object_language VARCHAR(20),

    -- Named graph (NULL = default graph)
    graph TEXT,

    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    modified_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for all access patterns
CREATE INDEX idx_rdf_spo ON ruvector_rdf_triples(subject, predicate, object);
CREATE INDEX idx_rdf_pos ON ruvector_rdf_triples(predicate, object, subject);
CREATE INDEX idx_rdf_osp ON ruvector_rdf_triples(object, subject, predicate);
CREATE INDEX idx_rdf_graph ON ruvector_rdf_triples(graph) WHERE graph IS NOT NULL;
CREATE INDEX idx_rdf_predicate ON ruvector_rdf_triples(predicate);

-- Full-text search on literals
CREATE INDEX idx_rdf_object_text ON ruvector_rdf_triples
    USING GIN(to_tsvector('english', object))
    WHERE object_type = 'literal';

-- Namespace prefix mapping
CREATE TABLE ruvector_rdf_namespaces (
    prefix VARCHAR(50) PRIMARY KEY,
    namespace TEXT NOT NULL UNIQUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Named graph metadata
CREATE TABLE ruvector_rdf_graphs (
    graph_iri TEXT PRIMARY KEY,
    label TEXT,
    description TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    modified_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### Custom Types

```sql
-- RDF term type
CREATE TYPE ruvector_rdf_term AS (
    value TEXT,
    term_type VARCHAR(10),  -- 'iri', 'literal', 'bnode'
    datatype TEXT,
    language VARCHAR(20)
);

-- SPARQL result binding
CREATE TYPE ruvector_sparql_binding AS (
    variable TEXT,
    term ruvector_rdf_term
);
```

---

## Phase 2: Core Functions

### Basic RDF Operations

```sql
-- Add a triple
CREATE FUNCTION ruvector_rdf_add_triple(
    subject TEXT,
    subject_type VARCHAR(10),
    predicate TEXT,
    object TEXT,
    object_type VARCHAR(10),
    object_datatype TEXT DEFAULT NULL,
    object_language VARCHAR(20) DEFAULT NULL,
    graph TEXT DEFAULT NULL
) RETURNS BIGINT;

-- Delete triples matching pattern
CREATE FUNCTION ruvector_rdf_delete_triple(
    subject TEXT DEFAULT NULL,
    predicate TEXT DEFAULT NULL,
    object TEXT DEFAULT NULL,
    graph TEXT DEFAULT NULL
) RETURNS INTEGER;

-- Check if triple exists
CREATE FUNCTION ruvector_rdf_has_triple(
    subject TEXT,
    predicate TEXT,
    object TEXT,
    graph TEXT DEFAULT NULL
) RETURNS BOOLEAN;

-- Get all triples for subject
CREATE FUNCTION ruvector_rdf_get_triples(
    subject TEXT,
    graph TEXT DEFAULT NULL
) RETURNS TABLE (
    predicate TEXT,
    object TEXT,
    object_type VARCHAR(10),
    object_datatype TEXT,
    object_language VARCHAR(20)
);
```

### Namespace Management

```sql
-- Register namespace prefix
CREATE FUNCTION ruvector_rdf_register_prefix(
    prefix VARCHAR(50),
    namespace TEXT
) RETURNS VOID;

-- Resolve prefixed name to IRI
CREATE FUNCTION ruvector_rdf_expand_prefix(
    prefixed_name TEXT
) RETURNS TEXT;

-- Shorten IRI to prefixed name
CREATE FUNCTION ruvector_rdf_compact_iri(
    iri TEXT
) RETURNS TEXT;
```

---

## Phase 3: SPARQL Query Engine

### Query Execution

```sql
-- Execute SPARQL SELECT query
CREATE FUNCTION ruvector_sparql_query(
    query TEXT,
    parameters JSONB DEFAULT NULL
) RETURNS TABLE (
    bindings JSONB
);

-- Execute SPARQL ASK query
CREATE FUNCTION ruvector_sparql_ask(
    query TEXT,
    parameters JSONB DEFAULT NULL
) RETURNS BOOLEAN;

-- Execute SPARQL CONSTRUCT query
CREATE FUNCTION ruvector_sparql_construct(
    query TEXT,
    parameters JSONB DEFAULT NULL
) RETURNS TABLE (
    subject TEXT,
    predicate TEXT,
    object TEXT,
    object_type VARCHAR(10)
);

-- Execute SPARQL DESCRIBE query
CREATE FUNCTION ruvector_sparql_describe(
    resource TEXT,
    graph TEXT DEFAULT NULL
) RETURNS TABLE (
    predicate TEXT,
    object TEXT,
    object_type VARCHAR(10)
);
```

### Update Operations

```sql
-- Execute SPARQL UPDATE
CREATE FUNCTION ruvector_sparql_update(
    update_query TEXT
) RETURNS INTEGER;

-- Bulk insert from N-Triples/Turtle
CREATE FUNCTION ruvector_rdf_load(
    data TEXT,
    format VARCHAR(20),  -- 'ntriples', 'turtle', 'rdfxml'
    graph TEXT DEFAULT NULL
) RETURNS INTEGER;
```

---

## Phase 4: Query Translation

### SPARQL to SQL Translation Strategy

#### 1. Basic Graph Pattern (BGP)

**SPARQL:**
```sparql
?person foaf:name ?name .
?person foaf:age ?age .
```

**SQL:**
```sql
SELECT
    t1.subject AS person,
    t1.object AS name,
    t2.object AS age
FROM ruvector_rdf_triples t1
JOIN ruvector_rdf_triples t2
    ON t1.subject = t2.subject
WHERE t1.predicate = 'http://xmlns.com/foaf/0.1/name'
  AND t2.predicate = 'http://xmlns.com/foaf/0.1/age'
  AND t1.object_type = 'literal'
  AND t2.object_type = 'literal';
```

#### 2. OPTIONAL Pattern

**SPARQL:**
```sparql
?person foaf:name ?name .
OPTIONAL { ?person foaf:email ?email }
```

**SQL:**
```sql
SELECT
    t1.subject AS person,
    t1.object AS name,
    t2.object AS email
FROM ruvector_rdf_triples t1
LEFT JOIN ruvector_rdf_triples t2
    ON t1.subject = t2.subject
    AND t2.predicate = 'http://xmlns.com/foaf/0.1/email'
WHERE t1.predicate = 'http://xmlns.com/foaf/0.1/name';
```

#### 3. UNION Pattern

**SPARQL:**
```sparql
{ ?x foaf:name ?name }
UNION
{ ?x rdfs:label ?name }
```

**SQL:**
```sql
SELECT subject AS x, object AS name
FROM ruvector_rdf_triples
WHERE predicate = 'http://xmlns.com/foaf/0.1/name'

UNION ALL

SELECT subject AS x, object AS name
FROM ruvector_rdf_triples
WHERE predicate = 'http://www.w3.org/2000/01/rdf-schema#label';
```

#### 4. FILTER with Comparison

**SPARQL:**
```sparql
?person foaf:age ?age .
FILTER(?age >= 18 && ?age < 65)
```

**SQL:**
```sql
SELECT
    subject AS person,
    object AS age
FROM ruvector_rdf_triples
WHERE predicate = 'http://xmlns.com/foaf/0.1/age'
  AND object_type = 'literal'
  AND object_datatype = 'http://www.w3.org/2001/XMLSchema#integer'
  AND CAST(object AS INTEGER) >= 18
  AND CAST(object AS INTEGER) < 65;
```

#### 5. Property Path (Transitive)

**SPARQL:**
```sparql
?person foaf:knows+ ?friend .
```

**SQL (with CTE):**
```sql
WITH RECURSIVE transitive AS (
    -- Base case: direct connections
    SELECT subject, object
    FROM ruvector_rdf_triples
    WHERE predicate = 'http://xmlns.com/foaf/0.1/knows'

    UNION

    -- Recursive case: follow chains
    SELECT t.subject, r.object
    FROM ruvector_rdf_triples t
    JOIN transitive r ON t.object = r.subject
    WHERE t.predicate = 'http://xmlns.com/foaf/0.1/knows'
)
SELECT subject AS person, object AS friend
FROM transitive;
```

#### 6. Aggregation with GROUP BY

**SPARQL:**
```sparql
SELECT ?company (COUNT(?employee) AS ?count) (AVG(?salary) AS ?avg)
WHERE {
  ?employee foaf:workplaceHomepage ?company .
  ?employee ex:salary ?salary .
}
GROUP BY ?company
HAVING (COUNT(?employee) >= 10)
```

**SQL:**
```sql
SELECT
    t1.object AS company,
    COUNT(*) AS count,
    AVG(CAST(t2.object AS NUMERIC)) AS avg
FROM ruvector_rdf_triples t1
JOIN ruvector_rdf_triples t2
    ON t1.subject = t2.subject
WHERE t1.predicate = 'http://xmlns.com/foaf/0.1/workplaceHomepage'
  AND t2.predicate = 'http://example.org/salary'
  AND t2.object_type = 'literal'
GROUP BY t1.object
HAVING COUNT(*) >= 10;
```

---

## Phase 5: Optimization

### Query Optimization Strategies

#### 1. Statistics Collection

```sql
-- Predicate statistics
CREATE TABLE ruvector_rdf_stats (
    predicate TEXT PRIMARY KEY,
    triple_count BIGINT,
    distinct_subjects BIGINT,
    distinct_objects BIGINT,
    avg_object_length NUMERIC,
    last_updated TIMESTAMP
);

-- Update statistics
CREATE FUNCTION ruvector_rdf_update_stats() RETURNS VOID AS $$
BEGIN
    DELETE FROM ruvector_rdf_stats;

    INSERT INTO ruvector_rdf_stats
    SELECT
        predicate,
        COUNT(*) as triple_count,
        COUNT(DISTINCT subject) as distinct_subjects,
        COUNT(DISTINCT object) as distinct_objects,
        AVG(LENGTH(object)) as avg_object_length,
        CURRENT_TIMESTAMP
    FROM ruvector_rdf_triples
    GROUP BY predicate;
END;
$$ LANGUAGE plpgsql;
```

#### 2. Join Ordering

Use statistics to order joins by selectivity:
1. Most selective (fewest results) first
2. Predicates with fewer distinct values
3. Literal objects before IRI objects

#### 3. Materialized Property Paths

```sql
-- Materialize common transitive closures
CREATE MATERIALIZED VIEW ruvector_rdf_knows_closure AS
WITH RECURSIVE transitive AS (
    SELECT subject, object, 1 as depth
    FROM ruvector_rdf_triples
    WHERE predicate = 'http://xmlns.com/foaf/0.1/knows'

    UNION

    SELECT t.subject, r.object, r.depth + 1
    FROM ruvector_rdf_triples t
    JOIN transitive r ON t.object = r.subject
    WHERE t.predicate = 'http://xmlns.com/foaf/0.1/knows'
      AND r.depth < 10  -- Limit depth
)
SELECT * FROM transitive;

CREATE INDEX idx_knows_closure_so ON ruvector_rdf_knows_closure(subject, object);
```

#### 4. Cached Queries

```sql
-- Query cache
CREATE TABLE ruvector_sparql_cache (
    query_hash TEXT PRIMARY KEY,
    query TEXT,
    plan JSONB,
    result JSONB,
    created_at TIMESTAMP,
    hit_count INTEGER DEFAULT 0,
    avg_exec_time INTERVAL
);
```

---

## Phase 6: Integration with RuVector

### Hybrid Queries (SPARQL + Vector Search)

```sql
-- Function to combine SPARQL with vector similarity
CREATE FUNCTION ruvector_sparql_vector_search(
    sparql_query TEXT,
    embedding_predicate TEXT,
    query_vector ruvector,
    similarity_threshold FLOAT,
    top_k INTEGER
) RETURNS TABLE (
    subject TEXT,
    bindings JSONB,
    similarity FLOAT
);
```

**Example Usage:**

```sql
-- Find similar people based on semantic description
SELECT * FROM ruvector_sparql_vector_search(
    'SELECT ?person ?name ?interests
     WHERE {
       ?person foaf:name ?name .
       ?person ex:interests ?interests .
       ?person ex:embedding ?embedding .
     }',
    'http://example.org/embedding',
    '[0.15, 0.25, ...]'::ruvector,
    0.8,
    10
);
```

### Knowledge Graph + Vector Embeddings

```sql
-- Store both RDF triples and embeddings
INSERT INTO ruvector_rdf_triples (subject, predicate, object, object_type)
VALUES
    ('http://example.org/alice', 'http://xmlns.com/foaf/0.1/name', 'Alice', 'literal'),
    ('http://example.org/alice', 'http://xmlns.com/foaf/0.1/age', '30', 'literal');

-- Add vector embedding using RuVector
CREATE TABLE person_embeddings (
    person_iri TEXT PRIMARY KEY,
    embedding ruvector(384)
);

INSERT INTO person_embeddings VALUES
    ('http://example.org/alice', '[0.1, 0.2, ...]'::ruvector);

-- Query combining both
SELECT
    r.subject AS person,
    r.object AS name,
    v.embedding <=> $1::ruvector AS similarity
FROM ruvector_rdf_triples r
JOIN person_embeddings v ON r.subject = v.person_iri
WHERE r.predicate = 'http://xmlns.com/foaf/0.1/name'
  AND v.embedding <=> $1::ruvector < 0.5
ORDER BY similarity
LIMIT 10;
```

---

## Phase 7: Advanced Features

### 1. SPARQL Federation

Support for SERVICE keyword to query remote endpoints:

```sql
CREATE FUNCTION ruvector_sparql_federated_query(
    query TEXT,
    remote_endpoints JSONB
) RETURNS TABLE (bindings JSONB);
```

### 2. Full-Text Search Integration

```sql
-- SPARQL query with full-text search
CREATE FUNCTION ruvector_sparql_text_search(
    search_term TEXT,
    language TEXT DEFAULT 'english'
) RETURNS TABLE (
    subject TEXT,
    predicate TEXT,
    object TEXT,
    rank FLOAT
);
```

### 3. GeoSPARQL Support

```sql
-- Spatial predicates
CREATE FUNCTION ruvector_geo_within(
    point1 GEOMETRY,
    point2 GEOMETRY,
    distance_meters FLOAT
) RETURNS BOOLEAN;
```

### 4. Reasoning and Inference

```sql
-- Simple RDFS entailment
CREATE FUNCTION ruvector_rdf_infer_rdfs() RETURNS INTEGER;

-- Materialize inferred triples
CREATE TABLE ruvector_rdf_inferred (
    LIKE ruvector_rdf_triples INCLUDING ALL,
    inference_rule TEXT
);
```

---

## Implementation Roadmap

### Phase 1: Foundation (Weeks 1-2)
- [ ] Design and implement triple store schema
- [ ] Create basic RDF manipulation functions
- [ ] Implement namespace management
- [ ] Build indexes for all access patterns

### Phase 2: Parser (Weeks 3-4)
- [ ] SPARQL 1.1 query parser (using Rust crate like `sparql-grammar`)
- [ ] Parse PREFIX declarations
- [ ] Parse SELECT, ASK, CONSTRUCT, DESCRIBE queries
- [ ] Parse WHERE clauses with BGP, OPTIONAL, UNION, FILTER

### Phase 3: Algebra (Week 5)
- [ ] Translate parsed queries to SPARQL algebra
- [ ] Implement BGP, Join, LeftJoin, Union, Filter operators
- [ ] Handle property paths
- [ ] Support subqueries

### Phase 4: SQL Generation (Weeks 6-7)
- [ ] Translate algebra to PostgreSQL SQL
- [ ] Optimize join ordering using statistics
- [ ] Generate CTEs for property paths
- [ ] Handle aggregates and solution modifiers

### Phase 5: Query Execution (Week 8)
- [ ] Execute generated SQL
- [ ] Format results as JSON/XML/CSV/TSV
- [ ] Implement result streaming for large datasets
- [ ] Add query timeout and resource limits

### Phase 6: Update Operations (Week 9)
- [ ] Implement INSERT DATA, DELETE DATA
- [ ] Implement DELETE/INSERT with WHERE
- [ ] Implement LOAD, CLEAR, CREATE, DROP
- [ ] Transaction support for updates

### Phase 7: Optimization (Week 10)
- [ ] Query result caching
- [ ] Statistics-based query planning
- [ ] Materialized property path views
- [ ] Prepared statement support

### Phase 8: RuVector Integration (Week 11)
- [ ] Hybrid SPARQL + vector similarity queries
- [ ] Semantic search with knowledge graphs
- [ ] Vector embeddings in RDF
- [ ] Combined ranking (semantic + vector)

### Phase 9: Testing & Documentation (Week 12)
- [ ] Unit tests for all components
- [ ] Integration tests with W3C SPARQL test suite
- [ ] Performance benchmarks
- [ ] User documentation and examples

---

## Testing Strategy

### Unit Tests

```sql
-- Test basic triple insertion
DO $$
DECLARE
    triple_id BIGINT;
BEGIN
    triple_id := ruvector_rdf_add_triple(
        'http://example.org/alice',
        'iri',
        'http://xmlns.com/foaf/0.1/name',
        'Alice',
        'literal'
    );

    ASSERT triple_id IS NOT NULL, 'Triple insertion failed';
END $$;
```

### W3C Test Suite

Implement tests from:
- SPARQL 1.1 Query Test Cases
- SPARQL 1.1 Update Test Cases
- Property Path Test Cases

### Performance Benchmarks

```sql
-- Benchmark query execution time
CREATE FUNCTION benchmark_sparql_query(
    query TEXT,
    iterations INTEGER DEFAULT 100
) RETURNS TABLE (
    avg_time INTERVAL,
    min_time INTERVAL,
    max_time INTERVAL,
    stddev_time INTERVAL
);
```

---

## Documentation Structure

```
docs/research/sparql/
├── SPARQL_SPECIFICATION.md          # Complete SPARQL 1.1 spec
├── IMPLEMENTATION_GUIDE.md          # This document
├── API_REFERENCE.md                 # SQL function reference
├── EXAMPLES.md                      # Usage examples
├── PERFORMANCE_TUNING.md            # Optimization guide
└── MIGRATION_GUIDE.md               # Migration from other triple stores
```

---

## Performance Targets

| Operation | Target | Notes |
|-----------|--------|-------|
| Simple BGP (3 patterns) | < 10ms | With proper indexes |
| Complex query (joins + filters) | < 100ms | 1M triples |
| Property path (depth 5) | < 500ms | 1M triples |
| Aggregate query | < 200ms | GROUP BY over 100K groups |
| INSERT DATA (1000 triples) | < 100ms | Bulk insert |
| DELETE/INSERT (pattern) | < 500ms | Affects 10K triples |

---

## Security Considerations

1. **SQL Injection Prevention**: Parameterized queries only
2. **Resource Limits**: Query timeout, memory limits
3. **Access Control**: Row-level security on triple store
4. **Audit Logging**: Log all UPDATE operations
5. **Rate Limiting**: Prevent DoS via complex queries

---

## Dependencies

### Rust Crates

- `sparql-parser` or `oxigraph` - SPARQL parsing
- `pgrx` - PostgreSQL extension framework
- `serde_json` - JSON serialization
- `regex` - FILTER regex support

### PostgreSQL Extensions

- `plpgsql` - Procedural language
- `pg_trgm` - Trigram text search
- `btree_gin` / `btree_gist` - Advanced indexing

---

## Future Enhancements

1. **SPARQL 1.2 Support**: When specification is finalized
2. **SHACL Validation**: Shape constraint language
3. **GraphQL Interface**: Map GraphQL to SPARQL
4. **Streaming Updates**: Real-time triple stream processing
5. **Distributed Queries**: Federate across multiple databases
6. **Machine Learning**: Train embeddings from knowledge graph

---

## References

- [SPARQL Specification Document](./SPARQL_SPECIFICATION.md)
- [RuVector PostgreSQL Extension](../../crates/ruvector-postgres/README.md)
- [W3C SPARQL 1.1 Test Suite](https://www.w3.org/2009/sparql/docs/tests/)
- [Apache Jena Documentation](https://jena.apache.org/documentation/query/)
- [Oxigraph Implementation](https://github.com/oxigraph/oxigraph)

---

**Status**: Research Complete - Ready for Implementation

**Next Steps**:
1. Review implementation guide with team
2. Create GitHub issues for each phase
3. Set up development environment
4. Begin Phase 1 implementation
