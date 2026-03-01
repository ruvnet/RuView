-- SPARQL PR#66 Comprehensive Test Suite
-- Tests all 14 SPARQL/RDF functions added in the PR

\echo '========================================='
\echo 'RuVector SPARQL/RDF Test Suite - PR #66'
\echo '========================================='
\echo ''

-- Verify extension is loaded
SELECT ruvector_version() AS version;
\echo ''

\echo '========================================='
\echo 'Test 1: Create RDF Triple Store'
\echo '========================================='
SELECT ruvector_create_rdf_store('test_knowledge_graph') AS store_created;
\echo ''

\echo '========================================='
\echo 'Test 2: Insert Individual Triples'
\echo '========================================='
-- Insert person type
SELECT ruvector_insert_triple(
    'test_knowledge_graph',
    '<http://example.org/person/alice>',
    '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>',
    '<http://example.org/Person>'
) AS alice_type_id;

-- Insert person name
SELECT ruvector_insert_triple(
    'test_knowledge_graph',
    '<http://example.org/person/alice>',
    '<http://xmlns.com/foaf/0.1/name>',
    '"Alice Smith"'
) AS alice_name_id;

-- Insert another person
SELECT ruvector_insert_triple(
    'test_knowledge_graph',
    '<http://example.org/person/bob>',
    '<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>',
    '<http://example.org/Person>'
) AS bob_type_id;

SELECT ruvector_insert_triple(
    'test_knowledge_graph',
    '<http://example.org/person/bob>',
    '<http://xmlns.com/foaf/0.1/name>',
    '"Bob Jones"'
) AS bob_name_id;

-- Insert friendship relation
SELECT ruvector_insert_triple(
    'test_knowledge_graph',
    '<http://example.org/person/alice>',
    '<http://xmlns.com/foaf/0.1/knows>',
    '<http://example.org/person/bob>'
) AS friendship_id;
\echo ''

\echo '========================================='
\echo 'Test 3: Bulk Load N-Triples'
\echo '========================================='
SELECT ruvector_load_ntriples('test_knowledge_graph', '
    <http://example.org/person/charlie> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> .
    <http://example.org/person/charlie> <http://xmlns.com/foaf/0.1/name> "Charlie Davis" .
    <http://example.org/person/charlie> <http://xmlns.com/foaf/0.1/knows> <http://example.org/person/alice> .
    <http://example.org/person/alice> <http://example.org/age> "30" .
    <http://example.org/person/bob> <http://example.org/age> "25" .
') AS triples_loaded;
\echo ''

\echo '========================================='
\echo 'Test 4: RDF Store Statistics'
\echo '========================================='
SELECT ruvector_rdf_stats('test_knowledge_graph') AS store_stats;
\echo ''

\echo '========================================='
\echo 'Test 5: Query Triples by Pattern'
\echo '========================================='
\echo 'Query: Get all triples about Alice'
SELECT ruvector_query_triples(
    'test_knowledge_graph',
    '<http://example.org/person/alice>',
    NULL,
    NULL
) AS alice_triples;
\echo ''

\echo 'Query: Get all name predicates'
SELECT ruvector_query_triples(
    'test_knowledge_graph',
    NULL,
    '<http://xmlns.com/foaf/0.1/name>',
    NULL
) AS all_names;
\echo ''

\echo '========================================='
\echo 'Test 6: SPARQL SELECT Queries'
\echo '========================================='
\echo 'Query: Select all persons with their names'
SELECT ruvector_sparql('test_knowledge_graph', '
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    PREFIX ex: <http://example.org/>
    SELECT ?person ?name
    WHERE {
        ?person a ex:Person .
        ?person foaf:name ?name .
    }
    ORDER BY ?name
', 'json') AS select_persons;
\echo ''

\echo 'Query: Find who Alice knows'
SELECT ruvector_sparql('test_knowledge_graph', '
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    SELECT ?friend ?friendName
    WHERE {
        <http://example.org/person/alice> foaf:knows ?friend .
        ?friend foaf:name ?friendName .
    }
', 'json') AS alice_friends;
\echo ''

\echo 'Query: Get all triples (LIMIT 10)'
SELECT ruvector_sparql('test_knowledge_graph', '
    SELECT ?s ?p ?o
    WHERE {
        ?s ?p ?o .
    }
    LIMIT 10
', 'json') AS all_triples;
\echo ''

\echo '========================================='
\echo 'Test 7: SPARQL ASK Queries'
\echo '========================================='
\echo 'Query: Does Alice exist?'
SELECT ruvector_sparql('test_knowledge_graph', '
    ASK { <http://example.org/person/alice> ?p ?o }
', 'json') AS alice_exists;
\echo ''

\echo 'Query: Does Alice know Bob?'
SELECT ruvector_sparql('test_knowledge_graph', '
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    ASK {
        <http://example.org/person/alice> foaf:knows <http://example.org/person/bob>
    }
', 'json') AS alice_knows_bob;
\echo ''

\echo 'Query: Does Bob know Alice? (should be false)'
SELECT ruvector_sparql('test_knowledge_graph', '
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    ASK {
        <http://example.org/person/bob> foaf:knows <http://example.org/person/alice>
    }
', 'json') AS bob_knows_alice;
\echo ''

\echo '========================================='
\echo 'Test 8: SPARQL JSON Results'
\echo '========================================='
SELECT ruvector_sparql_json('test_knowledge_graph', '
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    SELECT ?name
    WHERE {
        ?person foaf:name ?name .
    }
') AS json_result;
\echo ''

\echo '========================================='
\echo 'Test 9: SPARQL UPDATE Operations'
\echo '========================================='
SELECT ruvector_sparql_update('test_knowledge_graph', '
    INSERT DATA {
        <http://example.org/person/diana> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.org/Person> .
        <http://example.org/person/diana> <http://xmlns.com/foaf/0.1/name> "Diana Prince" .
    }
') AS update_result;
\echo ''

\echo 'Verify Diana was added:'
SELECT ruvector_sparql('test_knowledge_graph', '
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    SELECT ?name
    WHERE {
        <http://example.org/person/diana> foaf:name ?name .
    }
', 'json') AS diana_name;
\echo ''

\echo '========================================='
\echo 'Test 10: SPARQL with Different Formats'
\echo '========================================='
\echo 'Format: CSV'
SELECT ruvector_sparql('test_knowledge_graph', '
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    SELECT ?name WHERE { ?person foaf:name ?name } LIMIT 3
', 'csv') AS csv_format;
\echo ''

\echo 'Format: TSV'
SELECT ruvector_sparql('test_knowledge_graph', '
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    SELECT ?name WHERE { ?person foaf:name ?name } LIMIT 3
', 'tsv') AS tsv_format;
\echo ''

\echo '========================================='
\echo 'Test 11: Complex SPARQL Query with FILTER'
\echo '========================================='
SELECT ruvector_sparql('test_knowledge_graph', '
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>
    PREFIX ex: <http://example.org/>
    SELECT ?person ?name
    WHERE {
        ?person a ex:Person .
        ?person foaf:name ?name .
        FILTER(REGEX(?name, "^[AB]", "i"))
    }
', 'json') AS filtered_names;
\echo ''

\echo '========================================='
\echo 'Test 12: DBpedia-style Knowledge Graph'
\echo '========================================='
SELECT ruvector_create_rdf_store('dbpedia_scientists') AS dbpedia_created;

SELECT ruvector_load_ntriples('dbpedia_scientists', '
    <http://dbpedia.org/resource/Albert_Einstein> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbpedia.org/ontology/Scientist> .
    <http://dbpedia.org/resource/Albert_Einstein> <http://xmlns.com/foaf/0.1/name> "Albert Einstein" .
    <http://dbpedia.org/resource/Albert_Einstein> <http://dbpedia.org/ontology/birthPlace> <http://dbpedia.org/resource/Ulm> .
    <http://dbpedia.org/resource/Albert_Einstein> <http://dbpedia.org/ontology/field> <http://dbpedia.org/resource/Physics> .
    <http://dbpedia.org/resource/Marie_Curie> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbpedia.org/ontology/Scientist> .
    <http://dbpedia.org/resource/Marie_Curie> <http://xmlns.com/foaf/0.1/name> "Marie Curie" .
    <http://dbpedia.org/resource/Marie_Curie> <http://dbpedia.org/ontology/field> <http://dbpedia.org/resource/Physics> .
') AS dbpedia_loaded;

\echo 'Query: Find all physicists'
SELECT ruvector_sparql('dbpedia_scientists', '
    PREFIX dbo: <http://dbpedia.org/ontology/>
    PREFIX dbr: <http://dbpedia.org/resource/>
    PREFIX foaf: <http://xmlns.com/foaf/0.1/>

    SELECT ?name
    WHERE {
        ?person a dbo:Scientist .
        ?person dbo:field dbr:Physics .
        ?person foaf:name ?name .
    }
', 'json') AS physicists;
\echo ''

\echo 'Query: Check if Einstein was a scientist'
SELECT ruvector_sparql('dbpedia_scientists', '
    PREFIX dbo: <http://dbpedia.org/ontology/>
    PREFIX dbr: <http://dbpedia.org/resource/>

    ASK { dbr:Albert_Einstein a dbo:Scientist }
', 'json') AS einstein_is_scientist;
\echo ''

\echo '========================================='
\echo 'Test 13: List All RDF Stores'
\echo '========================================='
SELECT ruvector_list_rdf_stores() AS all_stores;
\echo ''

\echo '========================================='
\echo 'Test 14: Store Management Operations'
\echo '========================================='
\echo 'Get final statistics:'
SELECT ruvector_rdf_stats('test_knowledge_graph') AS final_stats;
\echo ''

\echo 'Clear test store:'
SELECT ruvector_clear_rdf_store('test_knowledge_graph') AS cleared;
SELECT ruvector_rdf_stats('test_knowledge_graph') AS stats_after_clear;
\echo ''

\echo 'Delete stores:'
SELECT ruvector_delete_rdf_store('test_knowledge_graph') AS test_deleted;
SELECT ruvector_delete_rdf_store('dbpedia_scientists') AS dbpedia_deleted;
\echo ''

\echo 'Verify stores deleted:'
SELECT ruvector_list_rdf_stores() AS remaining_stores;
\echo ''

\echo '========================================='
\echo 'All SPARQL/RDF Tests Completed!'
\echo '========================================='
