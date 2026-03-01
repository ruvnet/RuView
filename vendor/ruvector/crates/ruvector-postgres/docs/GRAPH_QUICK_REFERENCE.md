# Graph Operations Quick Reference

## Installation

```sql
CREATE EXTENSION ruvector_postgres;
```

## Graph Management

```sql
-- Create graph
SELECT ruvector_create_graph('my_graph');

-- List graphs
SELECT ruvector_list_graphs();

-- Get statistics
SELECT ruvector_graph_stats('my_graph');

-- Delete graph
SELECT ruvector_delete_graph('my_graph');
```

## Node Operations

```sql
-- Add node
SELECT ruvector_add_node(
    'graph_name',
    ARRAY['Label1', 'Label2'],
    '{"property": "value"}'::jsonb
) AS node_id;

-- Get node
SELECT ruvector_get_node('graph_name', 1);

-- Find by label
SELECT ruvector_find_nodes_by_label('graph_name', 'Person');
```

## Edge Operations

```sql
-- Add edge
SELECT ruvector_add_edge(
    'graph_name',
    1,  -- source_id
    2,  -- target_id
    'RELATIONSHIP_TYPE',
    '{"weight": 1.0}'::jsonb
) AS edge_id;

-- Get edge
SELECT ruvector_get_edge('graph_name', 1);

-- Get neighbors
SELECT ruvector_get_neighbors('graph_name', 1);
```

## Path Finding

```sql
-- Shortest path (unweighted)
SELECT ruvector_shortest_path(
    'graph_name',
    1,    -- start_id
    10,   -- end_id
    5     -- max_hops
);

-- Shortest path (weighted)
SELECT ruvector_shortest_path_weighted(
    'graph_name',
    1,    -- start_id
    10,   -- end_id
    'weight'  -- property for weights
);
```

## Cypher Queries

### CREATE

```sql
-- Create node
SELECT ruvector_cypher(
    'graph_name',
    'CREATE (n:Person {name: ''Alice'', age: 30}) RETURN n',
    NULL
);

-- Create relationship
SELECT ruvector_cypher(
    'graph_name',
    'CREATE (a:Person {name: ''Alice''})-[:KNOWS {since: 2020}]->(b:Person {name: ''Bob''}) RETURN a, b',
    NULL
);
```

### MATCH

```sql
-- Match all nodes
SELECT ruvector_cypher(
    'graph_name',
    'MATCH (n:Person) RETURN n',
    NULL
);

-- Match with WHERE
SELECT ruvector_cypher(
    'graph_name',
    'MATCH (n:Person) WHERE n.age > 25 RETURN n.name, n.age',
    NULL
);

-- Parameterized query
SELECT ruvector_cypher(
    'graph_name',
    'MATCH (n:Person) WHERE n.name = $name RETURN n',
    '{"name": "Alice"}'::jsonb
);
```

## Common Patterns

### Social Network

```sql
-- Setup
SELECT ruvector_create_graph('social');

-- Add users
SELECT ruvector_add_node('social', ARRAY['Person'],
    jsonb_build_object('name', 'Alice', 'age', 30));
SELECT ruvector_add_node('social', ARRAY['Person'],
    jsonb_build_object('name', 'Bob', 'age', 25));

-- Create friendship
SELECT ruvector_add_edge('social', 1, 2, 'FRIENDS',
    '{"since": "2020-01-15"}'::jsonb);

-- Find path
SELECT ruvector_shortest_path('social', 1, 2, 10);
```

### Knowledge Graph

```sql
-- Setup
SELECT ruvector_create_graph('knowledge');

-- Add concepts with Cypher
SELECT ruvector_cypher('knowledge',
    'CREATE (ml:Concept {name: ''Machine Learning''})
     CREATE (dl:Concept {name: ''Deep Learning''})
     CREATE (ml)-[:INCLUDES]->(dl)
     RETURN ml, dl',
    NULL
);

-- Query relationships
SELECT ruvector_cypher('knowledge',
    'MATCH (a:Concept)-[:INCLUDES]->(b:Concept)
     RETURN a.name, b.name',
    NULL
);
```

### Recommendation

```sql
-- Setup
SELECT ruvector_create_graph('recommendations');

-- Add users and items
SELECT ruvector_cypher('recommendations',
    'CREATE (u:User {name: ''Alice''})
     CREATE (m:Movie {title: ''Inception''})
     CREATE (u)-[:WATCHED {rating: 5}]->(m)
     RETURN u, m',
    NULL
);

-- Find similar users
SELECT ruvector_cypher('recommendations',
    'MATCH (u1:User)-[:WATCHED]->(m:Movie)<-[:WATCHED]-(u2:User)
     WHERE u1.name = ''Alice''
     RETURN u2.name',
    NULL
);
```

## Performance Tips

1. **Use labels for filtering**: Labels are indexed
2. **Limit hop count**: Specify reasonable max_hops
3. **Batch operations**: Use Cypher for multiple creates
4. **Property indexes**: Filter on indexed properties
5. **Parameterized queries**: Reuse query plans

## Return Value Formats

### Graph Stats
```json
{
    "name": "my_graph",
    "node_count": 100,
    "edge_count": 250,
    "labels": ["Person", "Movie"],
    "edge_types": ["KNOWS", "WATCHED"]
}
```

### Path Result
```json
{
    "nodes": [1, 3, 5, 10],
    "edges": [12, 45, 78],
    "length": 4,
    "cost": 2.5
}
```

### Node
```json
{
    "id": 1,
    "labels": ["Person"],
    "properties": {
        "name": "Alice",
        "age": 30
    }
}
```

### Edge
```json
{
    "id": 1,
    "source": 1,
    "target": 2,
    "edge_type": "KNOWS",
    "properties": {
        "since": "2020-01-15",
        "weight": 0.9
    }
}
```

## Error Handling

```sql
-- Check if graph exists before operations
DO $$
BEGIN
    IF 'my_graph' = ANY(ruvector_list_graphs()) THEN
        -- Perform operations
        RAISE NOTICE 'Graph exists';
    ELSE
        PERFORM ruvector_create_graph('my_graph');
    END IF;
END $$;

-- Handle missing nodes
DO $$
DECLARE
    result jsonb;
BEGIN
    result := ruvector_get_node('my_graph', 999);
    IF result IS NULL THEN
        RAISE NOTICE 'Node not found';
    END IF;
END $$;
```

## Best Practices

1. **Name graphs clearly**: Use descriptive names
2. **Use labels consistently**: Establish naming conventions
3. **Index frequently queried properties**: Plan for performance
4. **Batch similar operations**: Use Cypher for efficiency
5. **Clean up unused graphs**: Use delete_graph when done
6. **Monitor statistics**: Check graph_stats regularly
7. **Test queries**: Verify results before production
8. **Use parameters**: Prevent injection, enable caching

## Limitations

- **In-memory only**: No persistence across restarts
- **Single-node**: No distributed graph support
- **Simplified Cypher**: Basic patterns only
- **No transactions**: Operations are atomic but not grouped
- **No constraints**: No unique or foreign key constraints

## See Also

- [Full Documentation](README.md)
- [Implementation Details](GRAPH_IMPLEMENTATION.md)
- [SQL Examples](../sql/graph_examples.sql)
- [PostgreSQL Extension Docs](https://www.postgresql.org/docs/current/extend.html)
