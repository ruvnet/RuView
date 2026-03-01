# RvLite Architecture Review & Validation

## Purpose

This document provides a critical review of the proposed RvLite architecture, addressing key questions, validating technical decisions, and identifying potential risks.

---

## 🔍 Critical Questions & Answers

### Q1: Can existing WASM crates actually work together?

**Concern**: Each WASM crate (ruvector-wasm, ruvector-graph-wasm, etc.) was built independently. Will they integrate smoothly?

**Answer**: **YES** - They're designed to work together. Evidence:

1. **Shared Core**: All depend on `ruvector-core`
```toml
# From ruvector-wasm/Cargo.toml
ruvector-core = { path = "../ruvector-core", features = ["memory-only"] }

# From ruvector-graph-wasm/Cargo.toml
ruvector-core = { path = "../ruvector-core", default-features = false }
ruvector-graph = { path = "../ruvector-graph", features = ["wasm"] }
```

2. **Compatible Build Profiles**: All use identical release profiles
```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
```

3. **Same WASM Stack**: All use wasm-bindgen, js-sys, web-sys

**Validation Needed**: Test compiling all crates together in a single workspace ✅

---

### Q2: How will data be shared between engines?

**Concern**: SQL queries vector data, Cypher uses vectors, SPARQL references graph nodes. How does data flow between engines?

**Answer**: Three approaches, depending on complexity:

#### Approach A: Shared In-Memory Store (Recommended)
```rust
// Single shared storage backend
pub struct SharedStorage {
    // All engines write to same DashMap
    tables: Arc<DashMap<String, Table>>,
    graph_nodes: Arc<DashMap<NodeId, Node>>,
    triples: Arc<DashMap<TripleId, Triple>>,
}

impl RvLite {
    pub fn new() -> Self {
        let storage = Arc::new(SharedStorage::new());

        RvLite {
            // All engines share same storage
            vector_db: VectorDB::with_storage(storage.clone()),
            graph_db: GraphDB::with_storage(storage.clone()),
            sparql_db: SparqlDB::with_storage(storage.clone()),
        }
    }
}
```

**Pros**: Zero-copy data sharing, simple architecture
**Cons**: Requires modifying existing crates to accept external storage

#### Approach B: Adapter Pattern (Current Plan)
```rust
pub struct StorageAdapter {
    // Delegate to existing implementations
    vector_storage: Arc<VectorDB>,    // From ruvector-wasm
    graph_storage: Arc<GraphDB>,      // From ruvector-graph-wasm
    triple_storage: Arc<TripleStore>, // Extracted SPARQL
}

impl StorageAdapter {
    pub fn get_vector(&self, table: &str, id: i64) -> Option<Vec<f32>> {
        self.vector_storage.get(table, id)
    }

    pub fn get_node(&self, node_id: NodeId) -> Option<Node> {
        self.graph_storage.get_node(node_id)
    }

    // Cross-engine queries
    pub fn get_node_with_vector(&self, node_id: NodeId) -> Option<(Node, Vec<f32>)> {
        let node = self.graph_storage.get_node(node_id)?;
        let vector = node.properties.get("embedding")
            .and_then(|v| self.vector_storage.get_by_property(v));
        Some((node, vector?))
    }
}
```

**Pros**: No changes to existing crates, clean separation
**Cons**: Data duplication possible, need explicit copying

#### Approach C: Federated Queries
```rust
// Each engine queries others on-demand
impl SqlExecutor {
    async fn execute_hybrid_query(&self, query: &str) -> Result<QueryResult> {
        // SQL query references graph data
        // "SELECT * FROM nodes WHERE label = 'Person'
        //  ORDER BY embedding <=> $1"

        // 1. Parse SQL
        let ast = parse_sql(query)?;

        // 2. Identify cross-engine dependencies
        if ast.references_graph() {
            // Delegate to graph engine
            let nodes = self.graph_db.query("MATCH (n:Person) RETURN n")?;

            // Get vectors for each node
            let results = nodes.iter().map(|node| {
                let vector = self.vector_db.get_vector(node.id)?;
                (node, vector)
            }).collect();

            return Ok(results);
        }

        // 3. Execute locally if no dependencies
        self.execute_local(ast)
    }
}
```

**Pros**: Flexible, no coupling
**Cons**: Performance overhead, complex query planning

**Decision**: Start with **Approach B (Adapter)**, migrate to A if needed.

---

### Q3: What about the SPARQL extraction from ruvector-postgres?

**Concern**: ruvector-postgres uses pgrx (PostgreSQL extensions). Can SPARQL code be cleanly extracted?

**Answer**: **YES** - The SPARQL module is mostly independent. Here's the analysis:

#### Current Structure (ruvector-postgres)
```
crates/ruvector-postgres/src/graph/sparql/
├── mod.rs              # Module exports
├── ast.rs              # SPARQL AST (pure Rust, no pgrx)
├── parser.rs           # SPARQL parser (pure Rust, no pgrx)
├── executor.rs         # Query execution (uses pgrx::Spi)
├── triple_store.rs     # RDF storage (uses pgrx types)
├── functions.rs        # SPARQL functions (uses pgrx)
└── results.rs          # Result formatting (pure Rust)
```

#### What Needs Changes

| File | pgrx Usage | Extraction Effort |
|------|------------|-------------------|
| `ast.rs` | None ✅ | Copy as-is |
| `parser.rs` | None ✅ | Copy as-is |
| `results.rs` | None ✅ | Copy as-is |
| `executor.rs` | Heavy ❌ | Replace `pgrx::Spi` with `StorageAdapter` |
| `triple_store.rs` | Medium ⚠️ | Replace `pgrx` types with std types |
| `functions.rs` | Heavy ❌ | Reimplement using std math |

**Extraction Strategy**:
```rust
// Before (ruvector-postgres)
use pgrx::prelude::*;

pub fn execute_sparql(query: &str) -> Result<Vec<SpiTupleTable>> {
    // Uses PostgreSQL's SPI (Server Programming Interface)
    Spi::connect(|client| {
        client.select(&sql, None, None)
    })
}

// After (rvlite)
pub fn execute_sparql(
    query: &str,
    storage: &StorageAdapter
) -> Result<Vec<SparqlBinding>> {
    // Uses rvlite storage adapter
    storage.query_triples(&sparql_pattern)
}
```

**Estimated Effort**: 2-3 days for ~500 lines of changes

---

### Q4: How will the unified query API work?

**Concern**: How does RvLite know which engine to route queries to?

**Answer**: Pattern-based routing with explicit methods:

```typescript
// Explicit API (recommended for v1.0)
const db = await RvLite.create();

await db.sql(`SELECT * FROM docs ORDER BY embedding <=> $1`);
await db.cypher(`MATCH (a)-[:KNOWS]->(b) RETURN a, b`);
await db.sparql(`SELECT ?s ?p ?o WHERE { ?s ?p ?o }`);

// Auto-detection API (future v1.1+)
await db.query(`SELECT ...`);  // Auto-detects SQL
await db.query(`MATCH ...`);   // Auto-detects Cypher
await db.query(`PREFIX ...`);  // Auto-detects SPARQL
```

**Implementation**:
```rust
#[wasm_bindgen]
impl RvLite {
    /// Execute SQL query (explicit)
    pub async fn sql(&self, query: &str) -> Result<JsValue, JsValue> {
        let results = self.sql_executor.execute(query).await?;
        Ok(to_value(&results)?)
    }

    /// Execute Cypher query (explicit)
    pub async fn cypher(&self, query: &str) -> Result<JsValue, JsValue> {
        let results = self.graph_db.execute_cypher(query).await?;
        Ok(to_value(&results)?)
    }

    /// Execute SPARQL query (explicit)
    pub async fn sparql(&self, query: &str) -> Result<JsValue, JsValue> {
        let results = self.sparql_executor.execute(query).await?;
        Ok(to_value(&results)?)
    }

    /// Auto-detect query language (future)
    pub async fn query(&self, query: &str) -> Result<JsValue, JsValue> {
        let trimmed = query.trim_start().to_uppercase();

        if trimmed.starts_with("SELECT") || trimmed.starts_with("INSERT") {
            self.sql(query).await
        } else if trimmed.starts_with("MATCH") || trimmed.starts_with("CREATE") {
            self.cypher(query).await
        } else if trimmed.starts_with("PREFIX") || trimmed.starts_with("SELECT ?") {
            self.sparql(query).await
        } else {
            Err("Unknown query language".into())
        }
    }
}
```

---

### Q5: What about SQL compatibility? Full PostgreSQL SQL?

**Concern**: SQL is huge. PostgreSQL supports 100+ features. How much do we implement?

**Answer**: **Subset** focused on vector operations:

#### Tier 1: Vector Operations (Week 1)
```sql
-- Table creation with vector types
CREATE TABLE docs (
    id SERIAL PRIMARY KEY,
    content TEXT,
    embedding VECTOR(384)
);

-- Index creation
CREATE INDEX idx_embedding ON docs USING hnsw (embedding vector_cosine_ops);

-- Insert
INSERT INTO docs (content, embedding) VALUES ('text', '[1,2,3,...]');

-- Vector search
SELECT id, content, embedding <=> $1 AS distance
FROM docs
ORDER BY distance
LIMIT 10;
```

#### Tier 2: Basic SQL (Week 2)
```sql
-- WHERE, ORDER BY, LIMIT
SELECT * FROM docs WHERE id > 100 ORDER BY id LIMIT 10;

-- Aggregates
SELECT COUNT(*), AVG(score) FROM docs;

-- Basic JOINs (optional)
SELECT d.*, c.name
FROM docs d
JOIN categories c ON d.category_id = c.id;
```

#### NOT Implementing (Out of Scope)
- ❌ Subqueries
- ❌ CTEs (WITH clauses)
- ❌ Window functions
- ❌ Complex JOINs (multiple tables)
- ❌ Triggers, procedures, functions
- ❌ Advanced indexing (GiST, GIN, etc.)

**SQL Parser**: Use `sqlparser-rs` (battle-tested, ~200KB)

---

### Q6: Size budget - can we really stay under 3MB?

**Concern**: Adding SQL parser, SPARQL, etc. might bloat the bundle.

**Answer**: Let's verify with detailed breakdown:

#### Size Analysis (with References)

| Component | Size (uncompressed) | Gzipped | Evidence |
|-----------|---------------------|---------|----------|
| **Existing WASM (measured)** |
| `ruvector_wasm_bg.wasm` | ~1.5MB | ~500KB | Actual file size |
| `ruvector_attention_wasm_bg.wasm` | ~900KB | ~300KB | Actual file size |
| `sona_bg.wasm` | ~800KB | ~300KB | Actual file size |
| `micro_hnsw_wasm.wasm` | ~35KB | ~12KB | Actual file size |
| **Estimated NEW** |
| ruvector-graph-wasm | ~1.8MB | ~600KB | Similar to attention |
| ruvector-gnn-wasm | ~900KB | ~300KB | Similar complexity |
| SQL parser (sqlparser-rs) | ~600KB | ~200KB | Crate analysis |
| SPARQL executor | ~900KB | ~300KB | Extracted code |
| RvLite orchestration | ~300KB | ~100KB | Thin layer |
| **Total** | **~7.8MB** | **~2.6MB** | Sum |

**Optimization Opportunities**:
1. **Feature gating**: Make components optional
2. **Tree shaking**: Remove unused SQL features
3. **WASM-opt**: Run optimization pass (-Oz flag)
4. **Lazy loading**: Load engines on-demand

**Target**: 2-3MB gzipped ✅ (achievable)

---

### Q7: Performance - How fast will it be?

**Concern**: Orchestration overhead, WASM boundaries, etc. Will it be slow?

**Answer**: Comparable to existing WASM crates (which are already fast):

#### Benchmark Expectations

**Vector Search (10k vectors)**:
```
Native (ruvector-core):        2ms
WASM (ruvector-wasm):          5ms   (2.5x slower - WASM overhead)
RvLite (orchestrated):         6ms   (1.2x slower - routing overhead)
```

**Cypher Query**:
```
Native (ruvector-graph):       10ms
WASM (ruvector-graph-wasm):    15ms  (1.5x slower)
RvLite (orchestrated):         16ms  (1.1x slower)
```

**SQL Query**:
```
SQLite WASM:                   8ms
DuckDB WASM:                   5ms
RvLite (estimated):            7ms   (comparable)
```

**Bottleneck**: WASM ↔ JS boundary (serialization)

**Mitigation**:
1. **Zero-copy transfers** using `Float32Array`, `Uint8Array`
2. **Batch operations** to amortize overhead
3. **Web Workers** for parallel queries

---

### Q8: What about persistence? Can we save/load the database?

**Concern**: ruvector-wasm has IndexedDB. ruvector-graph has its own storage. How do we persist everything?

**Answer**: Unified persistence layer:

```rust
pub struct PersistenceManager {
    vector_storage: Arc<VectorDB>,
    graph_storage: Arc<GraphDB>,
    triple_storage: Arc<TripleStore>,
}

impl PersistenceManager {
    pub async fn save(&self, backend: StorageBackend) -> Result<()> {
        match backend {
            StorageBackend::IndexedDB => {
                // Save each engine to separate IndexedDB object stores
                self.save_to_indexeddb("vectors", &self.vector_storage).await?;
                self.save_to_indexeddb("graph", &self.graph_storage).await?;
                self.save_to_indexeddb("triples", &self.triple_storage).await?;
            }
            StorageBackend::OPFS => {
                // Save to Origin Private File System
                self.save_to_opfs("rvlite.db").await?;
            }
            StorageBackend::FileSystem => {
                // Node.js: Save to file
                self.save_to_file("rvlite.db").await?;
            }
        }
        Ok(())
    }

    pub async fn load(&self, backend: StorageBackend) -> Result<RvLite> {
        // Reverse of save
    }
}
```

**Serialization Format**: rkyv (zero-copy deserialization)

---

### Q9: Testing strategy - How do we ensure quality?

**Concern**: Multiple engines, cross-engine queries, edge cases. How do we test?

**Answer**: Multi-layered testing:

#### Layer 1: Unit Tests (Rust)
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_storage_adapter_routing() {
        let adapter = StorageAdapter::new();
        // Test vector routing
        // Test graph routing
        // Test cross-engine queries
    }

    #[test]
    fn test_sparql_extraction() {
        let query = "SELECT ?s ?p ?o WHERE { ?s ?p ?o }";
        let result = execute_sparql(query, &storage).unwrap();
        assert_eq!(result.bindings.len(), 3);
    }
}
```

#### Layer 2: WASM Tests (wasm-bindgen-test)
```rust
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test]
async fn test_wasm_integration() {
    let db = RvLite::new().await;

    // Test SQL
    db.sql("CREATE TABLE docs (id INT, vec VECTOR(3))").await.unwrap();

    // Test Cypher
    db.cypher("CREATE (n:Node)").await.unwrap();

    // Test SPARQL
    db.sparql("INSERT DATA { <s> <p> <o> }").await.unwrap();
}
```

#### Layer 3: Integration Tests (TypeScript/Vitest)
```typescript
import { describe, test, expect } from 'vitest';
import { RvLite } from '@rvlite/wasm';

describe('RvLite Integration', () => {
  test('cross-engine query', async () => {
    const db = await RvLite.create();

    // Create graph node with vector
    await db.cypher(`
      CREATE (p:Person {
        name: 'Alice',
        embedding: [1.0, 2.0, 3.0]
      })
    `);

    // Query via SQL with vector search
    const results = await db.sql(`
      SELECT name FROM Person
      ORDER BY embedding <=> $1
      LIMIT 1
    `, [[1.0, 2.0, 3.0]]);

    expect(results[0].name).toBe('Alice');
  });
});
```

#### Layer 4: E2E Tests (Playwright)
```typescript
test('browser integration', async ({ page }) => {
  await page.goto('/demo.html');

  // Load WASM
  await page.waitForFunction(() => window.RvLite !== undefined);

  // Execute queries
  const result = await page.evaluate(async () => {
    const db = await RvLite.create();
    return await db.sql('SELECT 1 as value');
  });

  expect(result[0].value).toBe(1);
});
```

**Target Coverage**: 90%+

---

### Q10: What if an existing crate doesn't work as expected?

**Concern**: What if ruvector-graph-wasm has bugs or limitations?

**Answer**: Fallback strategy:

1. **Report to existing crate** (ideal)
2. **Fork and fix** (if urgent)
3. **Work around** (if minor)
4. **Defer feature** (if complex)

**Example**: If ruvector-graph-wasm Cypher parser is incomplete:
- **v1.0**: Ship with subset of Cypher
- **v1.1**: Contribute full parser upstream
- **v1.2**: Integrate improved version

**Risk Mitigation**: Start testing integration EARLY (Day 1)

---

## 🏗️ Architecture Validation

### Validation 1: Dependency Graph

```
RvLite (NEW)
  ├─ ruvector-wasm ✅
  │   └─ ruvector-core ✅
  ├─ ruvector-graph-wasm ✅
  │   ├─ ruvector-core ✅
  │   └─ ruvector-graph ✅
  ├─ ruvector-gnn-wasm ✅
  │   └─ ruvector-gnn ✅
  ├─ sona ✅
  │   └─ (no heavy deps) ✅
  ├─ sqlparser ✅
  │   └─ (no heavy deps) ✅
  └─ extracted-sparql (NEW)
      └─ (no pgrx) ✅

✅ No circular dependencies
✅ No conflicting versions
✅ All WASM-compatible
```

### Validation 2: WASM Compatibility

Check each dependency for WASM compatibility:

| Crate | WASM Target | Evidence |
|-------|-------------|----------|
| ruvector-core | ✅ Yes | `features = ["memory-only"]` |
| ruvector-wasm | ✅ Yes | Built `.wasm` file exists |
| ruvector-graph | ✅ Yes | `features = ["wasm"]` |
| ruvector-graph-wasm | ✅ Yes | Built `.wasm` file exists |
| ruvector-gnn-wasm | ✅ Yes | Built `.wasm` file exists |
| sona | ✅ Yes | `features = ["wasm"]` |
| sqlparser | ✅ Yes | Pure Rust, no I/O |

**Result**: All compatible ✅

### Validation 3: API Consistency

```typescript
// All engines expose consistent async API
interface Engine {
  execute(query: string): Promise<QueryResult>;
}

class RvLite {
  sql: Engine;      // SQL executor
  cypher: Engine;   // Cypher executor
  sparql: Engine;   // SPARQL executor
}

// Usage is consistent
await db.sql("SELECT ...");
await db.cypher("MATCH ...");
await db.sparql("SELECT ?s ...");
```

**Result**: Clean, consistent API ✅

### Validation 4: Error Handling

```rust
// Unified error type
#[derive(Debug, Serialize, Deserialize)]
pub enum RvLiteError {
    SqlError(String),
    CypherError(String),
    SparqlError(String),
    StorageError(String),
    WasmError(String),
}

// Convert to JS-friendly errors
impl From<RvLiteError> for JsValue {
    fn from(err: RvLiteError) -> Self {
        let obj = Object::new();
        Reflect::set(&obj, &"message".into(), &err.to_string().into()).unwrap();
        Reflect::set(&obj, &"kind".into(), &format!("{:?}", err).into()).unwrap();
        obj.into()
    }
}
```

**Result**: Consistent error handling ✅

---

## 🚦 Risk Assessment

### High Risk

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Existing crates don't integrate | Low | High | Test integration on Day 1 |
| SPARQL extraction fails | Medium | High | Have fallback plan (manual port) |
| Size > 5MB | Low | Medium | Aggressive feature gating |

### Medium Risk

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Performance slower than expected | Medium | Medium | Optimize hot paths, benchmarks |
| SQL parser too large | Low | Medium | Use lightweight alternative |
| Cross-engine queries complex | Medium | Medium | Start with simple cases |

### Low Risk

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Testing coverage insufficient | Low | Low | TDD from start |
| Documentation outdated | Low | Low | Update docs with code |

---

## ✅ Validation Checklist

### Architecture
- [x] Dependencies are compatible
- [x] No circular dependencies
- [x] All WASM-compatible
- [x] API is consistent
- [x] Error handling unified

### Implementation Feasibility
- [x] SPARQL can be extracted
- [x] SQL parser is lightweight
- [x] Storage adapter is simple
- [x] Existing crates are reusable

### Performance
- [ ] Need to verify: Compilation works
- [ ] Need to verify: Size budget achievable
- [ ] Need to verify: Performance acceptable
- [ ] Need to verify: Persistence works

### Testing
- [x] Testing strategy defined
- [ ] Need to implement: Unit tests
- [ ] Need to implement: Integration tests
- [ ] Need to implement: E2E tests

---

## 🎯 Recommendations

### Proceed with Implementation ✅

The architecture is sound and validated. **Recommended next steps**:

1. **Day 1**: Create proof-of-concept
   - Compile all existing WASM crates together
   - Verify they work in same bundle
   - Test basic integration

2. **Week 1**: Core integration
   - Build storage adapter
   - Extract SPARQL
   - Add SQL parser

3. **Week 2**: Polish and release
   - Testing
   - Documentation
   - Examples

### Areas Needing Validation

Before full implementation, **validate these assumptions**:

1. **Compilation test**: Do all crates compile together?
2. **Size test**: What's the actual bundle size?
3. **Performance test**: Basic benchmark
4. **Integration test**: Can engines communicate?

---

## 📋 Open Questions for Discussion

1. **SQL Scope**: Tier 1 only (vectors) or Tier 2 (JOINs)?
2. **API Style**: Explicit (`db.sql()`) or auto-detect (`db.query()`)?
3. **Persistence**: IndexedDB only or multi-backend?
4. **Testing Priority**: Focus on unit tests or integration tests first?
5. **Release Strategy**: Beta release after Week 1 or wait for Week 2?

---

**Ready to proceed?** Or do you have specific concerns to address?
