# RuVector Postgres v2 - SQL Schema Specification

## Overview

This document defines the complete SQL schema for RuVector Postgres v2, including types, operators, functions, tables, and views. The schema is designed for **100% pgvector compatibility** while providing extended functionality.

---

## 1. Data Types

### 1.1 Core Vector Types (pgvector Compatible)

```sql
-- Dense vector type (pgvector compatible)
CREATE TYPE vector;

-- Input/output functions
CREATE FUNCTION vector_in(cstring, oid, integer) RETURNS vector
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_out(vector) RETURNS cstring
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_typmod_in(cstring[]) RETURNS integer
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_typmod_out(integer) RETURNS cstring
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_recv(internal, oid, integer) RETURNS vector
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_send(vector) RETURNS bytea
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE TYPE vector (
    INPUT     = vector_in,
    OUTPUT    = vector_out,
    TYPMOD_IN = vector_typmod_in,
    TYPMOD_OUT = vector_typmod_out,
    RECEIVE   = vector_recv,
    SEND      = vector_send,
    STORAGE   = external,
    INTERNALLENGTH = VARIABLE,
    ALIGNMENT = double
);
```

### 1.2 Extended Vector Types

```sql
-- Half-precision vector (float16)
CREATE TYPE halfvec (
    INPUT     = halfvec_in,
    OUTPUT    = halfvec_out,
    TYPMOD_IN = halfvec_typmod_in,
    TYPMOD_OUT = halfvec_typmod_out,
    RECEIVE   = halfvec_recv,
    SEND      = halfvec_send,
    STORAGE   = external,
    INTERNALLENGTH = VARIABLE,
    ALIGNMENT = double
);

-- Sparse vector (coordinate format)
CREATE TYPE sparsevec (
    INPUT     = sparsevec_in,
    OUTPUT    = sparsevec_out,
    TYPMOD_IN = sparsevec_typmod_in,
    TYPMOD_OUT = sparsevec_typmod_out,
    RECEIVE   = sparsevec_recv,
    SEND      = sparsevec_send,
    STORAGE   = external,
    INTERNALLENGTH = VARIABLE,
    ALIGNMENT = double
);

-- Binary vector (bit-packed)
CREATE TYPE binaryvec (
    INPUT     = binaryvec_in,
    OUTPUT    = binaryvec_out,
    TYPMOD_IN = binaryvec_typmod_in,
    TYPMOD_OUT = binaryvec_typmod_out,
    RECEIVE   = binaryvec_recv,
    SEND      = binaryvec_send,
    STORAGE   = plain,
    INTERNALLENGTH = VARIABLE,
    ALIGNMENT = int4
);
```

---

## 2. Operators (pgvector Compatible)

### 2.1 Distance Operators

```sql
-- L2 (Euclidean) distance
CREATE OPERATOR <-> (
    LEFTARG   = vector,
    RIGHTARG  = vector,
    FUNCTION  = vector_l2_distance,
    COMMUTATOR = <->
);

-- Cosine distance (1 - cosine_similarity)
CREATE OPERATOR <=> (
    LEFTARG   = vector,
    RIGHTARG  = vector,
    FUNCTION  = vector_cosine_distance,
    COMMUTATOR = <=>
);

-- Negative inner product (for MAX inner product via MIN)
CREATE OPERATOR <#> (
    LEFTARG   = vector,
    RIGHTARG  = vector,
    FUNCTION  = vector_negative_inner_product,
    COMMUTATOR = <#>
);

-- L1 (Manhattan) distance (RuVector extension)
CREATE OPERATOR <+> (
    LEFTARG   = vector,
    RIGHTARG  = vector,
    FUNCTION  = vector_l1_distance,
    COMMUTATOR = <+>
);
```

### 2.2 Operator Implementation Functions

```sql
-- Distance calculation functions
CREATE FUNCTION vector_l2_distance(vector, vector) RETURNS float8
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_cosine_distance(vector, vector) RETURNS float8
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_negative_inner_product(vector, vector) RETURNS float8
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_l1_distance(vector, vector) RETURNS float8
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

-- Similarity functions (convenience)
CREATE FUNCTION cosine_similarity(vector, vector) RETURNS float8
    AS 'MODULE_PATHNAME', 'vector_cosine_similarity'
    LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION inner_product(vector, vector) RETURNS float8
    AS 'MODULE_PATHNAME', 'vector_inner_product'
    LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION l2_distance(vector, vector) RETURNS float8
    AS 'MODULE_PATHNAME', 'vector_l2_distance'
    LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;
```

### 2.3 Vector Arithmetic

```sql
-- Addition
CREATE OPERATOR + (
    LEFTARG   = vector,
    RIGHTARG  = vector,
    FUNCTION  = vector_add,
    COMMUTATOR = +
);

-- Subtraction
CREATE OPERATOR - (
    LEFTARG   = vector,
    RIGHTARG  = vector,
    FUNCTION  = vector_sub
);

-- Scalar multiplication
CREATE OPERATOR * (
    LEFTARG   = vector,
    RIGHTARG  = float8,
    FUNCTION  = vector_mul_scalar
);

CREATE OPERATOR * (
    LEFTARG   = float8,
    RIGHTARG  = vector,
    FUNCTION  = scalar_mul_vector,
    COMMUTATOR = *
);

-- Comparison (for DISTINCT, GROUP BY)
CREATE OPERATOR = (
    LEFTARG   = vector,
    RIGHTARG  = vector,
    FUNCTION  = vector_eq,
    COMMUTATOR = =,
    NEGATOR   = <>,
    HASHES,
    MERGES
);

CREATE OPERATOR <> (
    LEFTARG   = vector,
    RIGHTARG  = vector,
    FUNCTION  = vector_ne,
    COMMUTATOR = <>,
    NEGATOR   = =
);
```

---

## 3. Operator Classes

### 3.1 HNSW Operator Classes

```sql
-- L2 distance for HNSW
CREATE OPERATOR CLASS vector_l2_ops
    DEFAULT FOR TYPE vector USING hnsw AS
    OPERATOR 1 <-> (vector, vector) FOR ORDER BY float_ops,
    FUNCTION 1 vector_l2_distance(vector, vector);

-- Cosine distance for HNSW
CREATE OPERATOR CLASS vector_cosine_ops
    FOR TYPE vector USING hnsw AS
    OPERATOR 1 <=> (vector, vector) FOR ORDER BY float_ops,
    FUNCTION 1 vector_cosine_distance(vector, vector);

-- Inner product for HNSW
CREATE OPERATOR CLASS vector_ip_ops
    FOR TYPE vector USING hnsw AS
    OPERATOR 1 <#> (vector, vector) FOR ORDER BY float_ops,
    FUNCTION 1 vector_negative_inner_product(vector, vector);
```

### 3.2 IVFFlat Operator Classes

```sql
-- L2 distance for IVFFlat
CREATE OPERATOR CLASS vector_l2_ops
    DEFAULT FOR TYPE vector USING ivfflat AS
    OPERATOR 1 <-> (vector, vector) FOR ORDER BY float_ops,
    FUNCTION 1 vector_l2_distance(vector, vector);

-- Cosine distance for IVFFlat
CREATE OPERATOR CLASS vector_cosine_ops
    FOR TYPE vector USING ivfflat AS
    OPERATOR 1 <=> (vector, vector) FOR ORDER BY float_ops,
    FUNCTION 1 vector_cosine_distance(vector, vector);

-- Inner product for IVFFlat
CREATE OPERATOR CLASS vector_ip_ops
    FOR TYPE vector USING ivfflat AS
    OPERATOR 1 <#> (vector, vector) FOR ORDER BY float_ops,
    FUNCTION 1 vector_negative_inner_product(vector, vector);
```

---

## 4. Index Access Methods

### 4.1 HNSW Access Method

```sql
CREATE ACCESS METHOD hnsw TYPE INDEX HANDLER hnsw_handler;

CREATE FUNCTION hnsw_handler(internal) RETURNS index_am_handler
    AS 'MODULE_PATHNAME' LANGUAGE C;

COMMENT ON ACCESS METHOD hnsw IS
    'HNSW index for approximate nearest neighbor search';
```

### 4.2 IVFFlat Access Method

```sql
CREATE ACCESS METHOD ivfflat TYPE INDEX HANDLER ivfflat_handler;

CREATE FUNCTION ivfflat_handler(internal) RETURNS index_am_handler
    AS 'MODULE_PATHNAME' LANGUAGE C;

COMMENT ON ACCESS METHOD ivfflat IS
    'IVF-Flat index for approximate nearest neighbor search';
```

---

## 5. Core Extension Tables

### 5.1 Collection Metadata

```sql
-- Stores metadata about vector collections
CREATE TABLE ruvector.collections (
    id              SERIAL PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    table_schema    TEXT NOT NULL,
    table_name      TEXT NOT NULL,
    column_name     TEXT NOT NULL,
    dimensions      INTEGER NOT NULL CHECK (dimensions > 0 AND dimensions <= 16000),
    distance_metric TEXT NOT NULL DEFAULT 'l2'
                    CHECK (distance_metric IN ('l2', 'cosine', 'ip', 'l1')),
    index_type      TEXT CHECK (index_type IN ('hnsw', 'ivfflat', 'flat')),
    index_oid       OID,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    config          JSONB NOT NULL DEFAULT '{}'::jsonb,

    UNIQUE(table_schema, table_name, column_name)
);

CREATE INDEX idx_collections_table
    ON ruvector.collections(table_schema, table_name);
```

### 5.2 Index Statistics

```sql
-- Stores per-index statistics
CREATE TABLE ruvector.index_stats (
    index_oid       OID PRIMARY KEY,
    collection_id   INTEGER REFERENCES ruvector.collections(id) ON DELETE CASCADE,
    vector_count    BIGINT NOT NULL DEFAULT 0,
    index_size_bytes BIGINT NOT NULL DEFAULT 0,
    last_build      TIMESTAMPTZ,
    last_vacuum     TIMESTAMPTZ,
    last_sample     TIMESTAMPTZ,
    build_time_ms   BIGINT,
    avg_search_ms   REAL,
    recall_estimate REAL,
    stats_json      JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_index_stats_collection
    ON ruvector.index_stats(collection_id);
```

---

## 6. Tiered Storage Tables (Phase 2)

### 6.1 Tier Configuration

```sql
-- Tier policy configuration per collection
CREATE TABLE ruvector.tier_policies (
    id              SERIAL PRIMARY KEY,
    collection_id   INTEGER NOT NULL REFERENCES ruvector.collections(id) ON DELETE CASCADE,
    tier_name       TEXT NOT NULL CHECK (tier_name IN ('hot', 'warm', 'cool', 'cold')),
    threshold_hours INTEGER NOT NULL DEFAULT 24,
    compression     TEXT NOT NULL DEFAULT 'none'
                    CHECK (compression IN ('none', 'sq8', 'pq16', 'pq32', 'pq64')),
    storage_path    TEXT,  -- NULL = default storage
    enabled         BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(collection_id, tier_name)
);

-- Default tier policies
INSERT INTO ruvector.tier_policies (collection_id, tier_name, threshold_hours, compression)
SELECT id, 'hot', 0, 'none' FROM ruvector.collections
UNION ALL
SELECT id, 'warm', 24, 'sq8' FROM ruvector.collections
UNION ALL
SELECT id, 'cool', 168, 'pq16' FROM ruvector.collections
UNION ALL
SELECT id, 'cold', 720, 'pq32' FROM ruvector.collections;
```

### 6.2 Access Tracking

```sql
-- Per-vector access counters (partitioned by collection)
CREATE TABLE ruvector.access_counters (
    collection_id   INTEGER NOT NULL,
    vector_tid      TID NOT NULL,  -- Tuple ID in source table
    access_count    INTEGER NOT NULL DEFAULT 0,
    last_access     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    current_tier    TEXT NOT NULL DEFAULT 'hot',

    PRIMARY KEY (collection_id, vector_tid)
) PARTITION BY LIST (collection_id);

-- Create partitions dynamically via trigger on collections insert
CREATE OR REPLACE FUNCTION ruvector.create_access_partition()
RETURNS TRIGGER AS $$
BEGIN
    EXECUTE format(
        'CREATE TABLE IF NOT EXISTS ruvector.access_counters_%s
         PARTITION OF ruvector.access_counters
         FOR VALUES IN (%s)',
        NEW.id, NEW.id
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_create_access_partition
    AFTER INSERT ON ruvector.collections
    FOR EACH ROW EXECUTE FUNCTION ruvector.create_access_partition();
```

### 6.3 Tier Statistics

```sql
-- Aggregated tier statistics
CREATE TABLE ruvector.tier_stats (
    id              SERIAL PRIMARY KEY,
    collection_id   INTEGER NOT NULL REFERENCES ruvector.collections(id) ON DELETE CASCADE,
    tier_name       TEXT NOT NULL,
    vector_count    BIGINT NOT NULL DEFAULT 0,
    size_bytes      BIGINT NOT NULL DEFAULT 0,
    avg_access_rate REAL NOT NULL DEFAULT 0,
    last_compaction TIMESTAMPTZ,
    compaction_count INTEGER NOT NULL DEFAULT 0,
    snapshot_time   TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(collection_id, tier_name, snapshot_time)
);

CREATE INDEX idx_tier_stats_collection_time
    ON ruvector.tier_stats(collection_id, snapshot_time DESC);
```

---

## 7. Graph Schema (Phase 3)

### 7.1 Graph Storage

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

-- Graph nodes (can reference vectors)
CREATE TABLE ruvector.nodes (
    id              BIGSERIAL PRIMARY KEY,
    graph_id        INTEGER NOT NULL REFERENCES ruvector.graphs(id) ON DELETE CASCADE,
    external_id     TEXT,  -- Application-provided ID
    node_type       TEXT NOT NULL DEFAULT 'default',
    vector_ref      TID,   -- Reference to vector in user table
    collection_id   INTEGER REFERENCES ruvector.collections(id),
    properties      JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(graph_id, external_id)
);

CREATE INDEX idx_nodes_graph_type ON ruvector.nodes(graph_id, node_type);
CREATE INDEX idx_nodes_properties ON ruvector.nodes USING gin(properties);

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

    -- No self-loops by default
    CHECK (source_id <> target_id)
);

CREATE INDEX idx_edges_source ON ruvector.edges(graph_id, source_id);
CREATE INDEX idx_edges_target ON ruvector.edges(graph_id, target_id);
CREATE INDEX idx_edges_type ON ruvector.edges(graph_id, edge_type);

-- Hyperedges (connect multiple nodes)
CREATE TABLE ruvector.hyperedges (
    id              BIGSERIAL PRIMARY KEY,
    graph_id        INTEGER NOT NULL REFERENCES ruvector.graphs(id) ON DELETE CASCADE,
    hyperedge_type  TEXT NOT NULL DEFAULT 'default',
    node_ids        BIGINT[] NOT NULL,
    weights         REAL[],  -- Optional per-node weights
    properties      JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CHECK (array_length(node_ids, 1) >= 2)
);

CREATE INDEX idx_hyperedges_graph ON ruvector.hyperedges(graph_id);
CREATE INDEX idx_hyperedges_nodes ON ruvector.hyperedges USING gin(node_ids);
```

### 7.2 Graph Views (Relational Bridge)

```sql
-- Unified node view with vector data
CREATE VIEW ruvector.nodes_view AS
SELECT
    n.id,
    n.graph_id,
    g.name AS graph_name,
    n.external_id,
    n.node_type,
    n.properties,
    n.created_at,
    c.table_schema,
    c.table_name,
    c.column_name,
    n.vector_ref
FROM ruvector.nodes n
JOIN ruvector.graphs g ON n.graph_id = g.id
LEFT JOIN ruvector.collections c ON n.collection_id = c.id;

-- Edge view with source/target details
CREATE VIEW ruvector.edges_view AS
SELECT
    e.id,
    e.graph_id,
    g.name AS graph_name,
    e.source_id,
    src.external_id AS source_external_id,
    src.node_type AS source_type,
    e.target_id,
    tgt.external_id AS target_external_id,
    tgt.node_type AS target_type,
    e.edge_type,
    e.weight,
    e.properties,
    e.created_at
FROM ruvector.edges e
JOIN ruvector.graphs g ON e.graph_id = g.id
JOIN ruvector.nodes src ON e.source_id = src.id
JOIN ruvector.nodes tgt ON e.target_id = tgt.id;

-- Hyperedge view with expanded nodes
CREATE VIEW ruvector.hyperedges_view AS
SELECT
    h.id,
    h.graph_id,
    g.name AS graph_name,
    h.hyperedge_type,
    h.node_ids,
    h.weights,
    h.properties,
    h.created_at,
    array_agg(n.external_id) AS node_external_ids
FROM ruvector.hyperedges h
JOIN ruvector.graphs g ON h.graph_id = g.id
LEFT JOIN LATERAL unnest(h.node_ids) AS node_id ON true
LEFT JOIN ruvector.nodes n ON n.id = node_id
GROUP BY h.id, g.name;
```

---

## 8. Integrity Control Schema (Phase 4)

### 8.1 Integrity State

```sql
-- Current integrity state per collection
CREATE TABLE ruvector.integrity_state (
    collection_id   INTEGER PRIMARY KEY REFERENCES ruvector.collections(id) ON DELETE CASCADE,
    state           TEXT NOT NULL DEFAULT 'normal'
                    CHECK (state IN ('normal', 'stress', 'critical')),
    lambda_cut      REAL NOT NULL DEFAULT 1.0,
    witness_edges   JSONB NOT NULL DEFAULT '[]'::jsonb,
    last_sample     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    sample_count    INTEGER NOT NULL DEFAULT 0,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Integrity state transitions history
CREATE TABLE ruvector.integrity_events (
    id              BIGSERIAL PRIMARY KEY,
    collection_id   INTEGER NOT NULL REFERENCES ruvector.collections(id) ON DELETE CASCADE,
    event_type      TEXT NOT NULL CHECK (event_type IN (
        'state_change', 'sample', 'policy_update', 'manual_override', 'recovery'
    )),
    previous_state  TEXT,
    new_state       TEXT,
    lambda_cut      REAL,
    witness_edges   JSONB,
    metadata        JSONB NOT NULL DEFAULT '{}'::jsonb,
    signature       BYTEA,  -- Ed25519 signature for audit
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_integrity_events_collection_time
    ON ruvector.integrity_events(collection_id, created_at DESC);
CREATE INDEX idx_integrity_events_type
    ON ruvector.integrity_events(event_type, created_at DESC);
```

### 8.2 Integrity Policies

```sql
-- Integrity policies per collection
CREATE TABLE ruvector.integrity_policies (
    id              SERIAL PRIMARY KEY,
    collection_id   INTEGER NOT NULL REFERENCES ruvector.collections(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    description     TEXT,

    -- Thresholds
    threshold_high  REAL NOT NULL DEFAULT 0.8,  -- Above = normal
    threshold_low   REAL NOT NULL DEFAULT 0.3,  -- Below = critical

    -- Actions by state
    normal_actions  JSONB NOT NULL DEFAULT '{
        "allow_bulk_insert": true,
        "allow_index_rewire": true,
        "allow_compression": true,
        "allow_replication": true
    }'::jsonb,

    stress_actions  JSONB NOT NULL DEFAULT '{
        "allow_bulk_insert": false,
        "allow_index_rewire": true,
        "allow_compression": true,
        "allow_replication": true,
        "throttle_inserts_pct": 50
    }'::jsonb,

    critical_actions JSONB NOT NULL DEFAULT '{
        "allow_bulk_insert": false,
        "allow_index_rewire": false,
        "allow_compression": false,
        "allow_replication": true,
        "throttle_inserts_pct": 90
    }'::jsonb,

    -- Sampling configuration
    sample_interval_secs INTEGER NOT NULL DEFAULT 60,
    sample_size     INTEGER NOT NULL DEFAULT 1000,

    enabled         BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(collection_id, name)
);
```

### 8.3 Contracted Graph Schema

```sql
-- Contracted operational graph for mincut computation
-- CRITICAL: Never compute mincut on full similarity graph!
CREATE TABLE ruvector.contracted_graph (
    collection_id   INTEGER NOT NULL REFERENCES ruvector.collections(id) ON DELETE CASCADE,
    node_type       TEXT NOT NULL CHECK (node_type IN (
        'partition', 'centroid', 'shard', 'maintenance_dep'
    )),
    node_id         BIGINT NOT NULL,
    node_data       JSONB NOT NULL DEFAULT '{}'::jsonb,

    PRIMARY KEY (collection_id, node_type, node_id)
);

CREATE TABLE ruvector.contracted_edges (
    id              BIGSERIAL PRIMARY KEY,
    collection_id   INTEGER NOT NULL REFERENCES ruvector.collections(id) ON DELETE CASCADE,
    source_type     TEXT NOT NULL,
    source_id       BIGINT NOT NULL,
    target_type     TEXT NOT NULL,
    target_id       BIGINT NOT NULL,
    edge_type       TEXT NOT NULL CHECK (edge_type IN (
        'partition_link', 'routing_link', 'dependency', 'replication'
    )),
    capacity        REAL NOT NULL DEFAULT 1.0,  -- For max-flow computation
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    FOREIGN KEY (collection_id, source_type, source_id)
        REFERENCES ruvector.contracted_graph(collection_id, node_type, node_id) ON DELETE CASCADE,
    FOREIGN KEY (collection_id, target_type, target_id)
        REFERENCES ruvector.contracted_graph(collection_id, node_type, node_id) ON DELETE CASCADE
);

CREATE INDEX idx_contracted_edges_source
    ON ruvector.contracted_edges(collection_id, source_type, source_id);
CREATE INDEX idx_contracted_edges_target
    ON ruvector.contracted_edges(collection_id, target_type, target_id);
```

---

## 9. GNN & Learning Tables

### 9.1 Training Data

```sql
-- Captured training interactions
CREATE TABLE ruvector.training_interactions (
    id              BIGSERIAL PRIMARY KEY,
    collection_id   INTEGER NOT NULL REFERENCES ruvector.collections(id) ON DELETE CASCADE,
    query_vector    vector,  -- Query that was performed
    result_ids      TEXT[] NOT NULL,  -- TIDs of returned results
    selected_id     TEXT,    -- TID user actually selected (if known)
    feedback_score  REAL,    -- Explicit feedback (-1 to 1)
    latency_ms      INTEGER,
    metadata        JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_training_collection_time
    ON ruvector.training_interactions(collection_id, created_at DESC);

-- Partitioned by time for efficient cleanup
-- In production, add monthly partitioning
```

### 9.2 GNN Models

```sql
-- Trained GNN model registry
CREATE TABLE ruvector.gnn_models (
    id              SERIAL PRIMARY KEY,
    collection_id   INTEGER NOT NULL REFERENCES ruvector.collections(id) ON DELETE CASCADE,
    model_name      TEXT NOT NULL,
    model_type      TEXT NOT NULL CHECK (model_type IN ('gcn', 'graphsage', 'gat', 'custom')),
    version         INTEGER NOT NULL DEFAULT 1,
    is_active       BOOLEAN NOT NULL DEFAULT false,

    -- Model binary (serialized weights)
    model_data      BYTEA,

    -- Training metadata
    training_samples INTEGER,
    training_epochs  INTEGER,
    validation_loss  REAL,
    validation_recall REAL,

    -- Configuration
    config          JSONB NOT NULL DEFAULT '{}'::jsonb,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(collection_id, model_name, version)
);

CREATE INDEX idx_gnn_models_active
    ON ruvector.gnn_models(collection_id, is_active) WHERE is_active;
```

---

## 10. Core Functions

### 10.1 Collection Management

```sql
-- Register a new vector collection
CREATE FUNCTION ruvector_register_collection(
    p_name TEXT,
    p_table_schema TEXT,
    p_table_name TEXT,
    p_column_name TEXT,
    p_dimensions INTEGER,
    p_distance_metric TEXT DEFAULT 'l2'
) RETURNS INTEGER AS $$
DECLARE
    v_id INTEGER;
BEGIN
    INSERT INTO ruvector.collections
        (name, table_schema, table_name, column_name, dimensions, distance_metric)
    VALUES
        (p_name, p_table_schema, p_table_name, p_column_name, p_dimensions, p_distance_metric)
    RETURNING id INTO v_id;

    -- Initialize integrity state
    INSERT INTO ruvector.integrity_state (collection_id) VALUES (v_id);

    -- Create default integrity policy
    INSERT INTO ruvector.integrity_policies (collection_id, name, description)
    VALUES (v_id, 'default', 'Default integrity policy');

    RETURN v_id;
END;
$$ LANGUAGE plpgsql;

-- Unregister a collection
CREATE FUNCTION ruvector_unregister_collection(p_name TEXT) RETURNS BOOLEAN AS $$
BEGIN
    DELETE FROM ruvector.collections WHERE name = p_name;
    RETURN FOUND;
END;
$$ LANGUAGE plpgsql;

-- List all collections
CREATE FUNCTION ruvector_list_collections()
RETURNS TABLE (
    id INTEGER,
    name TEXT,
    table_ref TEXT,
    dimensions INTEGER,
    distance_metric TEXT,
    index_type TEXT,
    vector_count BIGINT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        c.id,
        c.name,
        c.table_schema || '.' || c.table_name || '.' || c.column_name,
        c.dimensions,
        c.distance_metric,
        c.index_type,
        COALESCE(s.vector_count, 0)
    FROM ruvector.collections c
    LEFT JOIN ruvector.index_stats s ON c.index_oid = s.index_oid;
END;
$$ LANGUAGE plpgsql;
```

### 10.2 Index Management

```sql
-- Create HNSW index with custom parameters
CREATE FUNCTION ruvector_index_create(
    p_table_name TEXT,
    p_column_name TEXT,
    p_index_type TEXT DEFAULT 'hnsw',
    p_distance TEXT DEFAULT 'l2',
    p_m INTEGER DEFAULT 16,
    p_ef_construction INTEGER DEFAULT 64
) RETURNS TEXT AS $$
DECLARE
    v_index_name TEXT;
    v_ops_class TEXT;
BEGIN
    v_index_name := format('idx_%s_%s_%s',
        replace(p_table_name, '.', '_'),
        p_column_name,
        p_index_type);

    v_ops_class := CASE p_distance
        WHEN 'l2' THEN 'vector_l2_ops'
        WHEN 'cosine' THEN 'vector_cosine_ops'
        WHEN 'ip' THEN 'vector_ip_ops'
        ELSE 'vector_l2_ops'
    END;

    IF p_index_type = 'hnsw' THEN
        EXECUTE format(
            'CREATE INDEX %I ON %s USING hnsw (%I %s) WITH (m = %s, ef_construction = %s)',
            v_index_name, p_table_name, p_column_name, v_ops_class, p_m, p_ef_construction
        );
    ELSIF p_index_type = 'ivfflat' THEN
        EXECUTE format(
            'CREATE INDEX %I ON %s USING ivfflat (%I %s) WITH (lists = 100)',
            v_index_name, p_table_name, p_column_name, v_ops_class
        );
    END IF;

    RETURN v_index_name;
END;
$$ LANGUAGE plpgsql;

-- Get index statistics
CREATE FUNCTION ruvector_index_stats(p_index_name TEXT)
RETURNS JSONB AS 'MODULE_PATHNAME', 'ruvector_index_stats' LANGUAGE C;

-- Perform index maintenance
CREATE FUNCTION ruvector_index_maintenance(p_index_name TEXT)
RETURNS TEXT AS 'MODULE_PATHNAME' LANGUAGE C;
```

### 10.3 Vector Search

```sql
-- Extended search with options
CREATE FUNCTION ruvector_search(
    p_collection_name TEXT,
    p_query vector,
    p_k INTEGER DEFAULT 10,
    p_ef_search INTEGER DEFAULT NULL,
    p_use_gnn BOOLEAN DEFAULT FALSE,
    p_filter JSONB DEFAULT NULL
) RETURNS TABLE (
    id TEXT,
    distance REAL,
    score REAL
) AS 'MODULE_PATHNAME', 'ruvector_search' LANGUAGE C;

-- Batch search
CREATE FUNCTION ruvector_search_batch(
    p_collection_name TEXT,
    p_queries vector[],
    p_k INTEGER DEFAULT 10
) RETURNS TABLE (
    query_idx INTEGER,
    id TEXT,
    distance REAL
) AS 'MODULE_PATHNAME', 'ruvector_search_batch' LANGUAGE C;
```

### 10.4 Tiered Storage Functions

```sql
-- Configure tier thresholds
CREATE FUNCTION ruvector_set_tiers(
    p_collection_name TEXT,
    p_hot_hours INTEGER DEFAULT 24,
    p_warm_hours INTEGER DEFAULT 168,
    p_cool_hours INTEGER DEFAULT 720
) RETURNS BOOLEAN AS $$
DECLARE
    v_collection_id INTEGER;
BEGIN
    SELECT id INTO v_collection_id FROM ruvector.collections WHERE name = p_collection_name;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Collection not found: %', p_collection_name;
    END IF;

    UPDATE ruvector.tier_policies
    SET threshold_hours = p_hot_hours WHERE collection_id = v_collection_id AND tier_name = 'warm';

    UPDATE ruvector.tier_policies
    SET threshold_hours = p_warm_hours WHERE collection_id = v_collection_id AND tier_name = 'cool';

    UPDATE ruvector.tier_policies
    SET threshold_hours = p_cool_hours WHERE collection_id = v_collection_id AND tier_name = 'cold';

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

-- Trigger manual compaction
CREATE FUNCTION ruvector_compact(p_collection_name TEXT)
RETURNS JSONB AS 'MODULE_PATHNAME', 'ruvector_compact' LANGUAGE C;

-- Get tier report
CREATE FUNCTION ruvector_tier_report(p_collection_name TEXT)
RETURNS TABLE (
    tier_name TEXT,
    vector_count BIGINT,
    size_mb REAL,
    compression TEXT,
    avg_age_hours REAL
) AS 'MODULE_PATHNAME', 'ruvector_tier_report' LANGUAGE C;
```

### 10.5 Graph & Cypher Functions

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

-- Add node
CREATE FUNCTION ruvector_node_add(
    p_graph_name TEXT,
    p_external_id TEXT,
    p_node_type TEXT DEFAULT 'default',
    p_properties JSONB DEFAULT '{}'::jsonb
) RETURNS BIGINT AS $$
DECLARE
    v_graph_id INTEGER;
    v_node_id BIGINT;
BEGIN
    SELECT id INTO v_graph_id FROM ruvector.graphs WHERE name = p_graph_name;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Graph not found: %', p_graph_name;
    END IF;

    INSERT INTO ruvector.nodes (graph_id, external_id, node_type, properties)
    VALUES (v_graph_id, p_external_id, p_node_type, p_properties)
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

    SELECT id INTO v_target_id FROM ruvector.nodes
    WHERE graph_id = v_graph_id AND external_id = p_target_external_id;

    INSERT INTO ruvector.edges (graph_id, source_id, target_id, edge_type, weight, properties)
    VALUES (v_graph_id, v_source_id, v_target_id, p_edge_type, p_weight, p_properties)
    RETURNING id INTO v_edge_id;

    UPDATE ruvector.graphs SET edge_count = edge_count + 1, updated_at = NOW()
    WHERE id = v_graph_id;

    RETURN v_edge_id;
END;
$$ LANGUAGE plpgsql;
```

### 10.6 Integrity Control Functions

```sql
-- Sample integrity state
CREATE FUNCTION ruvector_integrity_sample(p_collection_name TEXT)
RETURNS JSONB AS 'MODULE_PATHNAME', 'ruvector_integrity_sample' LANGUAGE C;

-- Set integrity policy
CREATE FUNCTION ruvector_integrity_policy_set(
    p_collection_name TEXT,
    p_policy_name TEXT,
    p_config JSONB
) RETURNS BOOLEAN AS $$
DECLARE
    v_collection_id INTEGER;
BEGIN
    SELECT id INTO v_collection_id FROM ruvector.collections WHERE name = p_collection_name;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Collection not found: %', p_collection_name;
    END IF;

    -- Update or insert policy
    INSERT INTO ruvector.integrity_policies (collection_id, name,
        threshold_high, threshold_low,
        normal_actions, stress_actions, critical_actions,
        sample_interval_secs, sample_size)
    VALUES (
        v_collection_id,
        p_policy_name,
        COALESCE((p_config->>'threshold_high')::REAL, 0.8),
        COALESCE((p_config->>'threshold_low')::REAL, 0.3),
        COALESCE(p_config->'normal_actions', '{}'::jsonb),
        COALESCE(p_config->'stress_actions', '{}'::jsonb),
        COALESCE(p_config->'critical_actions', '{}'::jsonb),
        COALESCE((p_config->>'sample_interval_secs')::INTEGER, 60),
        COALESCE((p_config->>'sample_size')::INTEGER, 1000)
    )
    ON CONFLICT (collection_id, name) DO UPDATE SET
        threshold_high = EXCLUDED.threshold_high,
        threshold_low = EXCLUDED.threshold_low,
        normal_actions = EXCLUDED.normal_actions,
        stress_actions = EXCLUDED.stress_actions,
        critical_actions = EXCLUDED.critical_actions,
        sample_interval_secs = EXCLUDED.sample_interval_secs,
        sample_size = EXCLUDED.sample_size,
        updated_at = NOW();

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

-- Check integrity gate
CREATE FUNCTION ruvector_integrity_gate(
    p_collection_name TEXT,
    p_operation TEXT
) RETURNS JSONB AS 'MODULE_PATHNAME', 'ruvector_integrity_gate' LANGUAGE C;

-- Get current integrity state
CREATE FUNCTION ruvector_integrity_status(p_collection_name TEXT)
RETURNS JSONB AS $$
DECLARE
    v_result JSONB;
BEGIN
    SELECT jsonb_build_object(
        'collection', p_collection_name,
        'state', s.state,
        'lambda_cut', s.lambda_cut,
        'witness_edges', s.witness_edges,
        'last_sample', s.last_sample,
        'sample_count', s.sample_count
    ) INTO v_result
    FROM ruvector.collections c
    JOIN ruvector.integrity_state s ON c.id = s.collection_id
    WHERE c.name = p_collection_name;

    RETURN v_result;
END;
$$ LANGUAGE plpgsql;
```

### 10.7 GNN Functions

```sql
-- Record interaction for training
CREATE FUNCTION ruvector_record_interaction(
    p_collection_name TEXT,
    p_query vector,
    p_result_ids TEXT[],
    p_selected_id TEXT DEFAULT NULL,
    p_feedback_score REAL DEFAULT NULL
) RETURNS BIGINT AS $$
DECLARE
    v_collection_id INTEGER;
    v_id BIGINT;
BEGIN
    SELECT id INTO v_collection_id FROM ruvector.collections WHERE name = p_collection_name;

    INSERT INTO ruvector.training_interactions
        (collection_id, query_vector, result_ids, selected_id, feedback_score)
    VALUES
        (v_collection_id, p_query, p_result_ids, p_selected_id, p_feedback_score)
    RETURNING id INTO v_id;

    RETURN v_id;
END;
$$ LANGUAGE plpgsql;

-- Trigger GNN training
CREATE FUNCTION ruvector_gnn_train(
    p_collection_name TEXT,
    p_model_type TEXT DEFAULT 'graphsage',
    p_epochs INTEGER DEFAULT 100,
    p_config JSONB DEFAULT '{}'::jsonb
) RETURNS JSONB AS 'MODULE_PATHNAME', 'ruvector_gnn_train' LANGUAGE C;

-- Get active GNN model
CREATE FUNCTION ruvector_gnn_model_active(p_collection_name TEXT)
RETURNS JSONB AS $$
DECLARE
    v_result JSONB;
BEGIN
    SELECT jsonb_build_object(
        'model_name', m.model_name,
        'model_type', m.model_type,
        'version', m.version,
        'training_samples', m.training_samples,
        'validation_recall', m.validation_recall,
        'created_at', m.created_at
    ) INTO v_result
    FROM ruvector.collections c
    JOIN ruvector.gnn_models m ON c.id = m.collection_id
    WHERE c.name = p_collection_name AND m.is_active;

    RETURN COALESCE(v_result, '{}'::jsonb);
END;
$$ LANGUAGE plpgsql;
```

---

## 11. Utility Functions

```sql
-- Extension version
CREATE FUNCTION ruvector_version() RETURNS TEXT
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE PARALLEL SAFE;

-- SIMD capability info
CREATE FUNCTION ruvector_simd_info() RETURNS TEXT
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE PARALLEL SAFE;

-- Memory statistics
CREATE FUNCTION ruvector_memory_stats() RETURNS JSONB
    AS 'MODULE_PATHNAME' LANGUAGE C;

-- Vector utilities
CREATE FUNCTION vector_dims(vector) RETURNS INTEGER
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_norm(vector) RETURNS REAL
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

CREATE FUNCTION vector_normalize(vector) RETURNS vector
    AS 'MODULE_PATHNAME' LANGUAGE C IMMUTABLE STRICT PARALLEL SAFE;

-- Aggregates
CREATE AGGREGATE avg(vector) (
    SFUNC = vector_accum,
    STYPE = internal,
    FINALFUNC = vector_avg_final,
    COMBINEFUNC = vector_combine,
    PARALLEL = SAFE
);

CREATE AGGREGATE sum(vector) (
    SFUNC = vector_add,
    STYPE = vector,
    COMBINEFUNC = vector_add,
    PARALLEL = SAFE
);
```

---

## 12. Schema Initialization

```sql
-- Create extension schema
CREATE SCHEMA IF NOT EXISTS ruvector;

-- Set search path
ALTER EXTENSION ruvector SET SCHEMA ruvector;

-- Grant usage
GRANT USAGE ON SCHEMA ruvector TO PUBLIC;
GRANT SELECT ON ALL TABLES IN SCHEMA ruvector TO PUBLIC;

-- Create roles
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'ruvector_admin') THEN
        CREATE ROLE ruvector_admin;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'ruvector_operator') THEN
        CREATE ROLE ruvector_operator;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'ruvector_user') THEN
        CREATE ROLE ruvector_user;
    END IF;
END
$$;

-- Grant permissions
GRANT ALL ON SCHEMA ruvector TO ruvector_admin;
GRANT ALL ON ALL TABLES IN SCHEMA ruvector TO ruvector_admin;
GRANT ALL ON ALL SEQUENCES IN SCHEMA ruvector TO ruvector_admin;

GRANT SELECT, INSERT, UPDATE, DELETE ON ruvector.collections TO ruvector_operator;
GRANT SELECT, INSERT, UPDATE, DELETE ON ruvector.index_stats TO ruvector_operator;
GRANT SELECT, INSERT, UPDATE, DELETE ON ruvector.tier_policies TO ruvector_operator;
GRANT SELECT ON ALL TABLES IN SCHEMA ruvector TO ruvector_user;
```

---

## Testing Requirements

### Type Tests
- Vector input/output roundtrip
- Dimension validation
- Operator correctness

### Index Tests
- HNSW build and search
- IVFFlat build and search
- Recall at various ef_search

### Function Tests
- All functions callable
- Error handling
- Permission checks

---

## Dependencies

- PostgreSQL 14+ (for operator class features)
- pgrx 0.11+ (Rust bindings)
- serde (JSON serialization)
- ed25519-dalek (signature verification)
