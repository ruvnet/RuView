# SPARQL Query Examples for RuVector-Postgres

**Project**: RuVector-Postgres SPARQL Extension
**Date**: December 2025

---

## Table of Contents

1. [Basic Queries](#basic-queries)
2. [Filtering and Constraints](#filtering-and-constraints)
3. [Optional Patterns](#optional-patterns)
4. [Property Paths](#property-paths)
5. [Aggregation](#aggregation)
6. [Update Operations](#update-operations)
7. [Named Graphs](#named-graphs)
8. [Hybrid Queries (SPARQL + Vector)](#hybrid-queries-sparql--vector)
9. [Advanced Patterns](#advanced-patterns)

---

## Basic Queries

### Example 1: Simple SELECT

Find all people and their names:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name
WHERE {
  ?person foaf:name ?name .
}
```

### Example 2: Multiple Patterns

Find people with both name and email:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name ?email
WHERE {
  ?person foaf:name ?name .
  ?person foaf:email ?email .
}
```

### Example 3: ASK Query

Check if a specific person exists:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

ASK {
  ?person foaf:name "Alice" .
}
```

### Example 4: CONSTRUCT Query

Build a new graph with simplified structure:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX ex: <http://example.org/>

CONSTRUCT {
  ?person ex:hasName ?name .
  ?person ex:contactEmail ?email .
}
WHERE {
  ?person foaf:name ?name .
  ?person foaf:email ?email .
}
```

### Example 5: DESCRIBE Query

Get all information about a resource:

```sparql
DESCRIBE <http://example.org/person/alice>
```

---

## Filtering and Constraints

### Example 6: Numeric Comparison

Find people aged 18 or older:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name ?age
WHERE {
  ?person foaf:name ?name .
  ?person foaf:age ?age .
  FILTER(?age >= 18)
}
```

### Example 7: String Matching

Find people with email addresses at example.com:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name ?email
WHERE {
  ?person foaf:name ?name .
  ?person foaf:email ?email .
  FILTER(CONTAINS(?email, "@example.com"))
}
```

### Example 8: Regex Pattern Matching

Find people whose names start with 'A':

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name
WHERE {
  ?person foaf:name ?name .
  FILTER(REGEX(?name, "^A", "i"))
}
```

### Example 9: Multiple Conditions

Find adults between 18 and 65:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name ?age
WHERE {
  ?person foaf:name ?name .
  ?person foaf:age ?age .
  FILTER(?age >= 18 && ?age < 65)
}
```

### Example 10: Logical OR

Find people with either phone or email:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name ?contact
WHERE {
  ?person foaf:name ?name .
  {
    ?person foaf:phone ?contact .
  }
  UNION
  {
    ?person foaf:email ?contact .
  }
}
```

---

## Optional Patterns

### Example 11: Simple OPTIONAL

Find all people, including email if available:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name ?email
WHERE {
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:email ?email }
}
```

### Example 12: Multiple OPTIONAL

Find people with optional contact information:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name ?email ?phone ?homepage
WHERE {
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:email ?email }
  OPTIONAL { ?person foaf:phone ?phone }
  OPTIONAL { ?person foaf:homepage ?homepage }
}
```

### Example 13: OPTIONAL with FILTER

Find people with optional business emails:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name ?businessEmail
WHERE {
  ?person foaf:name ?name .
  OPTIONAL {
    ?person foaf:email ?businessEmail .
    FILTER(!CONTAINS(?businessEmail, "@gmail.com"))
  }
}
```

### Example 14: Nested OPTIONAL

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name ?friendName ?friendEmail
WHERE {
  ?person foaf:name ?name .
  OPTIONAL {
    ?person foaf:knows ?friend .
    ?friend foaf:name ?friendName .
    OPTIONAL { ?friend foaf:email ?friendEmail }
  }
}
```

---

## Property Paths

### Example 15: Transitive Closure

Find all people someone knows (directly or indirectly):

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name ?friendName
WHERE {
  <http://example.org/alice> foaf:name ?name .
  <http://example.org/alice> foaf:knows+ ?friend .
  ?friend foaf:name ?friendName .
}
```

### Example 16: Path Sequence

Find grandchildren:

```sparql
PREFIX ex: <http://example.org/>

SELECT ?person ?grandchild
WHERE {
  ?person ex:hasChild / ex:hasChild ?grandchild .
}
```

### Example 17: Alternative Paths

Find either name or label:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

SELECT ?person ?label
WHERE {
  ?person (foaf:name | rdfs:label) ?label .
}
```

### Example 18: Inverse Path

Find all children of a person:

```sparql
PREFIX ex: <http://example.org/>

SELECT ?child
WHERE {
  <http://example.org/alice> ^ex:hasChild ?child .
}
```

### Example 19: Zero or More

Find all connected people (including self):

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?connected
WHERE {
  <http://example.org/alice> foaf:knows* ?connected .
}
```

### Example 20: Negated Property

Find relationships that aren't "knows":

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?x ?y
WHERE {
  ?x !foaf:knows ?y .
}
```

---

## Aggregation

### Example 21: COUNT

Count employees per company:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?company (COUNT(?employee) AS ?employeeCount)
WHERE {
  ?employee foaf:workplaceHomepage ?company .
}
GROUP BY ?company
```

### Example 22: AVG

Average salary by department:

```sparql
PREFIX ex: <http://example.org/>

SELECT ?dept (AVG(?salary) AS ?avgSalary)
WHERE {
  ?employee ex:department ?dept .
  ?employee ex:salary ?salary .
}
GROUP BY ?dept
```

### Example 23: MIN and MAX

Salary range by department:

```sparql
PREFIX ex: <http://example.org/>

SELECT ?dept (MIN(?salary) AS ?minSalary) (MAX(?salary) AS ?maxSalary)
WHERE {
  ?employee ex:department ?dept .
  ?employee ex:salary ?salary .
}
GROUP BY ?dept
```

### Example 24: GROUP_CONCAT

Concatenate skills per person:

```sparql
PREFIX ex: <http://example.org/>

SELECT ?person (GROUP_CONCAT(?skill; SEPARATOR=", ") AS ?skills)
WHERE {
  ?person ex:hasSkill ?skill .
}
GROUP BY ?person
```

### Example 25: HAVING

Find departments with more than 10 employees:

```sparql
PREFIX ex: <http://example.org/>

SELECT ?dept (COUNT(?employee) AS ?count)
WHERE {
  ?employee ex:department ?dept .
}
GROUP BY ?dept
HAVING (COUNT(?employee) > 10)
```

### Example 26: Multiple Aggregates

Comprehensive statistics per department:

```sparql
PREFIX ex: <http://example.org/>

SELECT ?dept
       (COUNT(?employee) AS ?empCount)
       (AVG(?salary) AS ?avgSalary)
       (MIN(?salary) AS ?minSalary)
       (MAX(?salary) AS ?maxSalary)
       (SUM(?salary) AS ?totalSalary)
WHERE {
  ?employee ex:department ?dept .
  ?employee ex:salary ?salary .
}
GROUP BY ?dept
ORDER BY DESC(?avgSalary)
```

---

## Update Operations

### Example 27: INSERT DATA

Add new triples:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

INSERT DATA {
  <http://example.org/alice> foaf:name "Alice" .
  <http://example.org/alice> foaf:age 30 .
  <http://example.org/alice> foaf:email "alice@example.com" .
}
```

### Example 28: DELETE DATA

Remove specific triples:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

DELETE DATA {
  <http://example.org/alice> foaf:email "old@example.com" .
}
```

### Example 29: DELETE/INSERT

Update based on pattern:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

DELETE { ?person foaf:age ?oldAge }
INSERT { ?person foaf:age ?newAge }
WHERE {
  ?person foaf:name "Alice" .
  ?person foaf:age ?oldAge .
  BIND(?oldAge + 1 AS ?newAge)
}
```

### Example 30: DELETE WHERE

Remove triples matching pattern:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

DELETE WHERE {
  ?person foaf:email ?email .
  FILTER(CONTAINS(?email, "@oldcompany.com"))
}
```

### Example 31: LOAD

Load RDF data from URL:

```sparql
LOAD <http://example.org/data.ttl> INTO GRAPH <http://example.org/graph1>
```

### Example 32: CLEAR

Clear all triples from a graph:

```sparql
CLEAR GRAPH <http://example.org/graph1>
```

### Example 33: CREATE and DROP

Manage graphs:

```sparql
CREATE GRAPH <http://example.org/newgraph>

-- later...

DROP GRAPH <http://example.org/oldgraph>
```

---

## Named Graphs

### Example 34: Query Specific Graph

Query data from a specific named graph:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name
FROM <http://example.org/graph1>
WHERE {
  ?person foaf:name ?name .
}
```

### Example 35: GRAPH Keyword

Query with graph variable:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name ?graph
WHERE {
  GRAPH ?graph {
    ?person foaf:name ?name .
  }
}
```

### Example 36: Query Multiple Graphs

Query data from multiple graphs:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name
FROM <http://example.org/graph1>
FROM <http://example.org/graph2>
WHERE {
  ?person foaf:name ?name .
}
```

### Example 37: Insert into Named Graph

Add triples to specific graph:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

INSERT DATA {
  GRAPH <http://example.org/graph1> {
    <http://example.org/bob> foaf:name "Bob" .
  }
}
```

---

## Hybrid Queries (SPARQL + Vector)

### Example 38: Semantic Search with Knowledge Graph

Find people similar to a query embedding:

```sql
-- Using RuVector-Postgres hybrid function
SELECT * FROM ruvector_sparql_vector_search(
  'SELECT ?person ?name ?bio
   WHERE {
     ?person foaf:name ?name .
     ?person ex:bio ?bio .
     ?person ex:embedding ?embedding .
   }',
  'http://example.org/embedding',
  '[0.15, 0.25, 0.35, ...]'::ruvector,  -- query vector
  0.8,  -- similarity threshold
  10    -- top K results
);
```

### Example 39: Combine Graph Traversal and Vector Similarity

Find friends of friends who are similar:

```sql
WITH friends_of_friends AS (
  SELECT DISTINCT o.subject AS person
  FROM ruvector_rdf_triples t1
  JOIN ruvector_rdf_triples t2 ON t1.object = t2.subject
  WHERE t1.subject = 'http://example.org/alice'
    AND t1.predicate = 'http://xmlns.com/foaf/0.1/knows'
    AND t2.predicate = 'http://xmlns.com/foaf/0.1/knows'
)
SELECT
  f.person,
  r.object AS name,
  e.embedding <=> $1::ruvector AS similarity
FROM friends_of_friends f
JOIN ruvector_rdf_triples r
  ON f.person = r.subject
  AND r.predicate = 'http://xmlns.com/foaf/0.1/name'
JOIN person_embeddings e
  ON f.person = e.person_iri
WHERE e.embedding <=> $1::ruvector < 0.5
ORDER BY similarity
LIMIT 10;
```

### Example 40: Hybrid Ranking

Combine SPARQL pattern matching with vector similarity:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX ex: <http://example.org/>

SELECT ?person ?name ?skills
       (ex:vectorSimilarity(?embedding, ?queryVector) AS ?similarity)
WHERE {
  ?person foaf:name ?name .
  ?person ex:skills ?skills .
  ?person ex:embedding ?embedding .

  # Pattern constraints
  FILTER(CONTAINS(?skills, "Python"))
  FILTER(ex:vectorSimilarity(?embedding, ?queryVector) > 0.7)
}
ORDER BY DESC(?similarity)
LIMIT 20
```

### Example 41: Multi-Modal Search

Search using both text and semantic embeddings:

```sql
-- Combine full-text search with vector similarity
SELECT
  t.subject AS document,
  t_title.object AS title,
  ts_rank(to_tsvector('english', t_content.object), plainto_tsquery('machine learning')) AS text_score,
  e.embedding <=> $1::ruvector AS vector_score,
  0.4 * ts_rank(to_tsvector('english', t_content.object), plainto_tsquery('machine learning'))
    + 0.6 * (1.0 - (e.embedding <=> $1::ruvector)) AS combined_score
FROM ruvector_rdf_triples t
JOIN ruvector_rdf_triples t_title
  ON t.subject = t_title.subject
  AND t_title.predicate = 'http://purl.org/dc/terms/title'
JOIN ruvector_rdf_triples t_content
  ON t.subject = t_content.subject
  AND t_content.predicate = 'http://purl.org/dc/terms/content'
JOIN document_embeddings e
  ON t.subject = e.doc_iri
WHERE to_tsvector('english', t_content.object) @@ plainto_tsquery('machine learning')
  AND e.embedding <=> $1::ruvector < 0.8
ORDER BY combined_score DESC
LIMIT 50;
```

---

## Advanced Patterns

### Example 42: Subquery

Find companies with above-average salaries:

```sparql
PREFIX ex: <http://example.org/>

SELECT ?company ?avgSalary
WHERE {
  {
    SELECT ?company (AVG(?salary) AS ?avgSalary)
    WHERE {
      ?employee ex:worksAt ?company .
      ?employee ex:salary ?salary .
    }
    GROUP BY ?company
  }

  {
    SELECT (AVG(?salary) AS ?overallAvg)
    WHERE {
      ?employee ex:salary ?salary .
    }
  }

  FILTER(?avgSalary > ?overallAvg)
}
ORDER BY DESC(?avgSalary)
```

### Example 43: VALUES

Query specific entities:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name ?age
WHERE {
  VALUES ?person {
    <http://example.org/alice>
    <http://example.org/bob>
    <http://example.org/charlie>
  }
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:age ?age }
}
```

### Example 44: BIND

Compute new values:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?fullName ?birthYear
WHERE {
  ?person foaf:givenName ?first .
  ?person foaf:familyName ?last .
  ?person foaf:age ?age .

  BIND(CONCAT(?first, " ", ?last) AS ?fullName)
  BIND(year(now()) - ?age AS ?birthYear)
}
```

### Example 45: NOT EXISTS

Find people without email:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?person ?name
WHERE {
  ?person foaf:name ?name .
  FILTER NOT EXISTS { ?person foaf:email ?email }
}
```

### Example 46: MINUS

Set difference - people who don't work at any company:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX ex: <http://example.org/>

SELECT ?person ?name
WHERE {
  ?person a foaf:Person .
  ?person foaf:name ?name .

  MINUS {
    ?person ex:worksAt ?company .
  }
}
```

### Example 47: Complex Property Path

Find all organizational hierarchies:

```sparql
PREFIX org: <http://www.w3.org/ns/org#>

SELECT ?person ?manager ?level
WHERE {
  ?person a foaf:Person .

  # Find manager at any level
  ?person (^org:reportsTo)* ?manager .

  # Calculate reporting level
  BIND(1 AS ?level)
}
```

### Example 48: Conditional Logic

Categorize people by age:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name ?age ?category
WHERE {
  ?person foaf:name ?name .
  ?person foaf:age ?age .

  BIND(
    IF(?age < 18, "minor",
      IF(?age < 65, "adult", "senior")
    ) AS ?category
  )
}
```

### Example 49: String Manipulation

Extract username and domain from email:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name ?username ?domain
WHERE {
  ?person foaf:name ?name .
  ?person foaf:email ?email .

  BIND(STRBEFORE(?email, "@") AS ?username)
  BIND(STRAFTER(?email, "@") AS ?domain)
}
```

### Example 50: Date/Time Operations

Find recent activities:

```sparql
PREFIX ex: <http://example.org/>

SELECT ?person ?activity ?date
WHERE {
  ?person ex:activity ?activity .
  ?activity ex:date ?date .

  # Activities in last 30 days
  FILTER(?date > (now() - "P30D"^^xsd:duration))
}
ORDER BY DESC(?date)
```

---

## Performance Tips

### Use Specific Predicates

**Good:**
```sparql
?person foaf:name ?name .
```

**Avoid:**
```sparql
?person ?p ?name .
FILTER(?p = foaf:name)
```

### Order Patterns by Selectivity

**Good (most selective first):**
```sparql
?person foaf:email "alice@example.com" .  # Very selective
?person foaf:name ?name .                  # Less selective
?person foaf:knows ?friend .               # Least selective
```

### Use LIMIT

Always use LIMIT when exploring:
```sparql
SELECT ?s ?p ?o
WHERE { ?s ?p ?o }
LIMIT 100
```

### Avoid Cartesian Products

**Bad:**
```sparql
?person1 foaf:name ?name1 .
?person2 foaf:name ?name2 .
```

**Good:**
```sparql
?person1 foaf:name ?name1 .
?person1 foaf:knows ?person2 .
?person2 foaf:name ?name2 .
```

### Use OPTIONAL Wisely

OPTIONAL can be expensive. Use only when necessary.

---

## Next Steps

1. Review the [SPARQL Specification](./SPARQL_SPECIFICATION.md) for complete syntax details
2. Check the [Implementation Guide](./IMPLEMENTATION_GUIDE.md) for architecture
3. Try examples in your PostgreSQL environment
4. Adapt queries for your specific use case

---

## References

- [W3C SPARQL 1.1 Query Language](https://www.w3.org/TR/sparql11-query/)
- [W3C SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/)
- [Apache Jena Tutorials](https://jena.apache.org/tutorials/sparql.html)
- [RuVector PostgreSQL Extension](../../crates/ruvector-postgres/README.md)
