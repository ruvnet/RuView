-- Graph Operations Examples for ruvector-postgres
-- This file demonstrates the graph database capabilities

-- ============================================================================
-- Basic Graph Operations
-- ============================================================================

-- Create a new graph
SELECT ruvector_create_graph('social_network');

-- List all graphs
SELECT ruvector_list_graphs();

-- ============================================================================
-- Social Network Example
-- ============================================================================

-- Add users
SELECT ruvector_add_node(
    'social_network',
    ARRAY['Person'],
    jsonb_build_object('name', 'Alice', 'age', 30, 'city', 'New York')
) AS alice_id;

SELECT ruvector_add_node(
    'social_network',
    ARRAY['Person'],
    jsonb_build_object('name', 'Bob', 'age', 25, 'city', 'San Francisco')
) AS bob_id;

SELECT ruvector_add_node(
    'social_network',
    ARRAY['Person'],
    jsonb_build_object('name', 'Charlie', 'age', 35, 'city', 'Boston')
) AS charlie_id;

SELECT ruvector_add_node(
    'social_network',
    ARRAY['Person'],
    jsonb_build_object('name', 'Diana', 'age', 28, 'city', 'Seattle')
) AS diana_id;

-- Create friendships
SELECT ruvector_add_edge(
    'social_network',
    1, 2, -- Alice -> Bob
    'FRIENDS',
    jsonb_build_object('since', '2020-01-15', 'strength', 0.9)
);

SELECT ruvector_add_edge(
    'social_network',
    2, 3, -- Bob -> Charlie
    'FRIENDS',
    jsonb_build_object('since', '2019-06-20', 'strength', 0.8)
);

SELECT ruvector_add_edge(
    'social_network',
    1, 4, -- Alice -> Diana
    'FRIENDS',
    jsonb_build_object('since', '2021-03-10', 'strength', 0.7)
);

SELECT ruvector_add_edge(
    'social_network',
    3, 4, -- Charlie -> Diana
    'FRIENDS',
    jsonb_build_object('since', '2020-09-05', 'strength', 0.85)
);

-- Get graph statistics
SELECT ruvector_graph_stats('social_network');

-- Find nodes by label
SELECT ruvector_find_nodes_by_label('social_network', 'Person');

-- Get neighbors of Alice (node 1)
SELECT ruvector_get_neighbors('social_network', 1);

-- Find shortest path from Alice to Charlie
SELECT ruvector_shortest_path('social_network', 1, 3, 10);

-- Find weighted shortest path
SELECT ruvector_shortest_path_weighted('social_network', 1, 3, 'strength');

-- ============================================================================
-- Cypher Query Examples
-- ============================================================================

-- Create nodes with Cypher
SELECT ruvector_cypher(
    'social_network',
    'CREATE (n:Person {name: ''Eve'', age: 27, city: ''Austin''}) RETURN n',
    NULL
);

-- Match all persons
SELECT ruvector_cypher(
    'social_network',
    'MATCH (n:Person) RETURN n.name, n.age',
    NULL
);

-- Match with WHERE clause
SELECT ruvector_cypher(
    'social_network',
    'MATCH (n:Person) WHERE n.age > 28 RETURN n.name, n.age',
    NULL
);

-- Parameterized query
SELECT ruvector_cypher(
    'social_network',
    'MATCH (n:Person) WHERE n.name = $name RETURN n',
    jsonb_build_object('name', 'Alice')
);

-- Create relationship with Cypher
SELECT ruvector_cypher(
    'social_network',
    'CREATE (a:Person {name: ''Frank''})-[:KNOWS {since: 2022}]->(b:Person {name: ''Grace''}) RETURN a, b',
    NULL
);

-- ============================================================================
-- Knowledge Graph Example
-- ============================================================================

SELECT ruvector_create_graph('knowledge');

-- Add concepts
SELECT ruvector_cypher(
    'knowledge',
    'CREATE (ml:Concept {name: ''Machine Learning'', category: ''AI''})
     CREATE (nn:Concept {name: ''Neural Networks'', category: ''AI''})
     CREATE (dl:Concept {name: ''Deep Learning'', category: ''AI''})
     CREATE (cv:Concept {name: ''Computer Vision'', category: ''AI''})
     CREATE (nlp:Concept {name: ''Natural Language Processing'', category: ''AI''})
     RETURN ml, nn, dl, cv, nlp',
    NULL
);

-- Create relationships between concepts
WITH ids AS (
    SELECT generate_series(1, 5) AS id
)
SELECT
    CASE
        WHEN i.id = 1 THEN ruvector_add_edge('knowledge', 1, 2, 'INCLUDES', '{"strength": 0.9}'::jsonb)
        WHEN i.id = 2 THEN ruvector_add_edge('knowledge', 2, 3, 'SPECIALIZES_IN', '{"strength": 0.95}'::jsonb)
        WHEN i.id = 3 THEN ruvector_add_edge('knowledge', 3, 4, 'APPLIES_TO', '{"strength": 0.85}'::jsonb)
        WHEN i.id = 4 THEN ruvector_add_edge('knowledge', 3, 5, 'APPLIES_TO', '{"strength": 0.9}'::jsonb)
    END AS edge_id
FROM ids i
WHERE i.id <= 4;

-- Find path from Machine Learning to Computer Vision
SELECT ruvector_shortest_path('knowledge', 1, 4, 10);

-- ============================================================================
-- Recommendation System Example
-- ============================================================================

SELECT ruvector_create_graph('recommendations');

-- Add users and movies
SELECT ruvector_cypher(
    'recommendations',
    'CREATE (u1:User {name: ''Alice'', preference: ''SciFi''})
     CREATE (u2:User {name: ''Bob'', preference: ''Action''})
     CREATE (u3:User {name: ''Charlie'', preference: ''SciFi''})
     CREATE (m1:Movie {title: ''Inception'', genre: ''SciFi''})
     CREATE (m2:Movie {title: ''Interstellar'', genre: ''SciFi''})
     CREATE (m3:Movie {title: ''The Matrix'', genre: ''SciFi''})
     CREATE (m4:Movie {title: ''Die Hard'', genre: ''Action''})
     RETURN u1, u2, u3, m1, m2, m3, m4',
    NULL
);

-- Create watch history
SELECT ruvector_add_edge('recommendations', 1, 4, 'WATCHED', '{"rating": 5, "timestamp": "2024-01-15"}'::jsonb);
SELECT ruvector_add_edge('recommendations', 1, 5, 'WATCHED', '{"rating": 4, "timestamp": "2024-01-20"}'::jsonb);
SELECT ruvector_add_edge('recommendations', 2, 7, 'WATCHED', '{"rating": 5, "timestamp": "2024-01-18"}'::jsonb);
SELECT ruvector_add_edge('recommendations', 3, 4, 'WATCHED', '{"rating": 5, "timestamp": "2024-01-22"}'::jsonb);
SELECT ruvector_add_edge('recommendations', 3, 6, 'WATCHED', '{"rating": 4, "timestamp": "2024-01-25"}'::jsonb);

-- Get statistics
SELECT ruvector_graph_stats('recommendations');

-- ============================================================================
-- Organizational Hierarchy Example
-- ============================================================================

SELECT ruvector_create_graph('org_chart');

-- Create organizational structure
SELECT ruvector_cypher(
    'org_chart',
    'CREATE (ceo:Employee {name: ''Jane Doe'', title: ''CEO'', level: 1})
     CREATE (cto:Employee {name: ''John Smith'', title: ''CTO'', level: 2})
     CREATE (cfo:Employee {name: ''Emily Brown'', title: ''CFO'', level: 2})
     CREATE (dev1:Employee {name: ''Alex Johnson'', title: ''Senior Dev'', level: 3})
     CREATE (dev2:Employee {name: ''Sarah Wilson'', title: ''Senior Dev'', level: 3})
     CREATE (acc1:Employee {name: ''Michael Davis'', title: ''Accountant'', level: 3})
     RETURN ceo, cto, cfo, dev1, dev2, acc1',
    NULL
);

-- Create reporting structure
SELECT ruvector_add_edge('org_chart', 2, 1, 'REPORTS_TO', '{}'::jsonb);
SELECT ruvector_add_edge('org_chart', 3, 1, 'REPORTS_TO', '{}'::jsonb);
SELECT ruvector_add_edge('org_chart', 4, 2, 'REPORTS_TO', '{}'::jsonb);
SELECT ruvector_add_edge('org_chart', 5, 2, 'REPORTS_TO', '{}'::jsonb);
SELECT ruvector_add_edge('org_chart', 6, 3, 'REPORTS_TO', '{}'::jsonb);

-- Find all employees reporting to CTO (directly or indirectly)
SELECT ruvector_shortest_path('org_chart', 4, 1, 5);  -- Path from dev1 to CEO
SELECT ruvector_shortest_path('org_chart', 5, 1, 5);  -- Path from dev2 to CEO

-- ============================================================================
-- Transport Network Example
-- ============================================================================

SELECT ruvector_create_graph('transport');

-- Add cities as nodes
SELECT ruvector_add_node('transport', ARRAY['City'], '{"name": "New York", "population": 8336817}'::jsonb);
SELECT ruvector_add_node('transport', ARRAY['City'], '{"name": "Boston", "population": 692600}'::jsonb);
SELECT ruvector_add_node('transport', ARRAY['City'], '{"name": "Philadelphia", "population": 1584064}'::jsonb);
SELECT ruvector_add_node('transport', ARRAY['City'], '{"name": "Washington DC", "population": 705749}'::jsonb);

-- Add routes with distances
SELECT ruvector_add_edge('transport', 1, 2, 'ROUTE', '{"distance": 215, "mode": "train", "duration": 4.5}'::jsonb);
SELECT ruvector_add_edge('transport', 1, 3, 'ROUTE', '{"distance": 95, "mode": "train", "duration": 1.5}'::jsonb);
SELECT ruvector_add_edge('transport', 3, 4, 'ROUTE', '{"distance": 140, "mode": "train", "duration": 2.5}'::jsonb);
SELECT ruvector_add_edge('transport', 2, 3, 'ROUTE', '{"distance": 310, "mode": "train", "duration": 5.5}'::jsonb);

-- Find shortest route by distance
SELECT ruvector_shortest_path_weighted('transport', 2, 4, 'distance');

-- Find fastest route by duration
SELECT ruvector_shortest_path_weighted('transport', 2, 4, 'duration');

-- ============================================================================
-- Analytics Queries
-- ============================================================================

-- Get all graphs with their statistics
SELECT
    name,
    (ruvector_graph_stats(name)::jsonb)->>'node_count' AS nodes,
    (ruvector_graph_stats(name)::jsonb)->>'edge_count' AS edges
FROM (
    SELECT unnest(ruvector_list_graphs()) AS name
) graphs;

-- ============================================================================
-- Cleanup
-- ============================================================================

-- Delete specific graph
-- SELECT ruvector_delete_graph('social_network');

-- Delete all graphs
-- SELECT ruvector_delete_graph(name)
-- FROM unnest(ruvector_list_graphs()) AS name;

-- ============================================================================
-- Performance Testing
-- ============================================================================

-- Create a larger graph for performance testing
SELECT ruvector_create_graph('perf_test');

-- Generate random nodes
DO $$
DECLARE
    i INTEGER;
BEGIN
    FOR i IN 1..1000 LOOP
        PERFORM ruvector_add_node(
            'perf_test',
            ARRAY['Node'],
            jsonb_build_object('id', i, 'value', random() * 100)
        );
    END LOOP;
END $$;

-- Generate random edges
DO $$
DECLARE
    i INTEGER;
    source_id INTEGER;
    target_id INTEGER;
BEGIN
    FOR i IN 1..5000 LOOP
        source_id := 1 + floor(random() * 1000)::INTEGER;
        target_id := 1 + floor(random() * 1000)::INTEGER;
        IF source_id <> target_id THEN
            BEGIN
                PERFORM ruvector_add_edge(
                    'perf_test',
                    source_id,
                    target_id,
                    'CONNECTS',
                    jsonb_build_object('weight', random())
                );
            EXCEPTION WHEN OTHERS THEN
                -- Ignore errors (e.g., duplicate edges)
                NULL;
            END;
        END IF;
    END LOOP;
END $$;

-- Check performance stats
SELECT ruvector_graph_stats('perf_test');

-- Test path finding performance
\timing on
SELECT ruvector_shortest_path('perf_test', 1, 500, 20);
SELECT ruvector_shortest_path_weighted('perf_test', 1, 500, 'weight');
\timing off

-- Cleanup performance test
-- SELECT ruvector_delete_graph('perf_test');
