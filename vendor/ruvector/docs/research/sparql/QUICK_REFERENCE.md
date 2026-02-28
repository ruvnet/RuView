# SPARQL Quick Reference

**One-page cheat sheet for SPARQL 1.1**

---

## Query Forms

```sparql
# SELECT - Return variable bindings
SELECT ?var1 ?var2 WHERE { ... }

# ASK - Return boolean
ASK WHERE { ... }

# CONSTRUCT - Build new graph
CONSTRUCT { ?s ?p ?o } WHERE { ... }

# DESCRIBE - Describe resources
DESCRIBE <http://example.org/resource>
```

---

## Basic Syntax

```sparql
# Prefixes
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

# Variables
?var  $var  # Both are equivalent

# URIs
<http://example.org/resource>
foaf:name  # Prefixed name

# Literals
"string"
"text"@en              # Language tag
"42"^^xsd:integer      # Typed literal
42  3.14  true         # Shorthand

# Blank nodes
_:label
[]
[ foaf:name "Alice" ]
```

---

## Triple Patterns

```sparql
# Basic pattern
?subject ?predicate ?object .

# Multiple patterns (AND)
?person foaf:name ?name .
?person foaf:age ?age .

# Shared subject (semicolon)
?person foaf:name ?name ;
        foaf:age ?age ;
        foaf:email ?email .

# Shared subject-predicate (comma)
?person foaf:knows ?alice, ?bob, ?charlie .

# rdf:type shorthand
?person a foaf:Person .  # Same as: ?person rdf:type foaf:Person
```

---

## Graph Patterns

```sparql
# OPTIONAL - Left join
?person foaf:name ?name .
OPTIONAL { ?person foaf:email ?email }

# UNION - Alternative patterns
{ ?x foaf:name ?name }
UNION
{ ?x rdfs:label ?name }

# FILTER - Constraints
?person foaf:age ?age .
FILTER(?age >= 18)

# BIND - Assign values
BIND(CONCAT(?first, " ", ?last) AS ?fullName)

# VALUES - Inline data
VALUES ?x { :alice :bob :charlie }

# Subquery
{
  SELECT ?company (AVG(?salary) AS ?avg)
  WHERE { ... }
  GROUP BY ?company
}
```

---

## Property Paths

```sparql
# Sequence
?x foaf:knows / foaf:name ?name

# Alternative
?x (foaf:name | rdfs:label) ?label

# Inverse
?child ^ex:hasChild ?parent

# Zero or more
?x foaf:knows* ?connected

# One or more
?x foaf:knows+ ?friend

# Zero or one
?x foaf:knows? ?maybeFriend

# Negation
?x !rdf:type ?y
```

---

## Filters

```sparql
# Comparison
FILTER(?age >= 18)
FILTER(?score > 0.5 && ?score < 1.0)

# String functions
FILTER(CONTAINS(?email, "@example.com"))
FILTER(STRSTARTS(?name, "A"))
FILTER(STRENDS(?url, ".com"))
FILTER(REGEX(?text, "pattern", "i"))

# Logical
FILTER(?age >= 18 && ?age < 65)
FILTER(?x = :alice || ?x = :bob)
FILTER(!bound(?optional))

# Functions
FILTER(bound(?var))           # Variable is bound
FILTER(isIRI(?x))             # Is IRI
FILTER(isLiteral(?x))         # Is literal
FILTER(lang(?x) = "en")       # Language tag
FILTER(datatype(?x) = xsd:integer)  # Datatype

# Set operations
FILTER(?x IN (:a, :b, :c))
FILTER(?x NOT IN (:d, :e))

# Existence
FILTER EXISTS { ?x foaf:knows ?y }
FILTER NOT EXISTS { ?x foaf:email ?email }
```

---

## Solution Modifiers

```sparql
# ORDER BY - Sort results
ORDER BY ?age                 # Ascending (default)
ORDER BY DESC(?age)           # Descending
ORDER BY ?name DESC(?age)     # Multiple criteria

# DISTINCT - Remove duplicates
SELECT DISTINCT ?name WHERE { ... }

# LIMIT - Maximum results
LIMIT 10

# OFFSET - Skip results
OFFSET 20
LIMIT 10

# GROUP BY - Group for aggregation
GROUP BY ?company

# HAVING - Filter groups
HAVING (COUNT(?emp) > 10)
```

---

## Aggregates

```sparql
# COUNT
SELECT (COUNT(?x) AS ?count) WHERE { ... }
SELECT (COUNT(DISTINCT ?x) AS ?count) WHERE { ... }

# SUM, AVG, MIN, MAX
SELECT (SUM(?value) AS ?sum) WHERE { ... }
SELECT (AVG(?value) AS ?avg) WHERE { ... }
SELECT (MIN(?value) AS ?min) WHERE { ... }
SELECT (MAX(?value) AS ?max) WHERE { ... }

# GROUP_CONCAT
SELECT (GROUP_CONCAT(?skill; SEPARATOR=", ") AS ?skills)
WHERE { ... }
GROUP BY ?person

# SAMPLE - Arbitrary value
SELECT ?company (SAMPLE(?employee) AS ?anyEmp)
WHERE { ... }
GROUP BY ?company
```

---

## Built-in Functions

### String Functions

```sparql
STRLEN(?str)                  # Length
SUBSTR(?str, 1, 5)            # Substring (1-indexed)
UCASE(?str)                   # Uppercase
LCASE(?str)                   # Lowercase
STRSTARTS(?str, "prefix")     # Starts with
STRENDS(?str, "suffix")       # Ends with
CONTAINS(?str, "substring")   # Contains
STRBEFORE(?email, "@")        # Before substring
STRAFTER(?email, "@")         # After substring
CONCAT(?str1, " ", ?str2)     # Concatenate
REPLACE(?str, "old", "new")   # Replace
ENCODE_FOR_URI(?str)          # URL encode
```

### Numeric Functions

```sparql
abs(?num)                     # Absolute value
round(?num)                   # Round
ceil(?num)                    # Ceiling
floor(?num)                   # Floor
RAND()                        # Random [0,1)
```

### Date/Time Functions

```sparql
now()                         # Current timestamp
year(?date)                   # Extract year
month(?date)                  # Extract month
day(?date)                    # Extract day
hours(?time)                  # Extract hours
minutes(?time)                # Extract minutes
seconds(?time)                # Extract seconds
```

### Hash Functions

```sparql
MD5(?str)                     # MD5 hash
SHA1(?str)                    # SHA1 hash
SHA256(?str)                  # SHA256 hash
SHA512(?str)                  # SHA512 hash
```

### RDF Term Functions

```sparql
str(?term)                    # Convert to string
lang(?literal)                # Language tag
datatype(?literal)            # Datatype IRI
IRI(?string)                  # Construct IRI
BNODE()                       # New blank node
STRDT("42", xsd:integer)      # Typed literal
STRLANG("hello", "en")        # Language-tagged literal
isIRI(?x)                     # Check if IRI
isBlank(?x)                   # Check if blank node
isLiteral(?x)                 # Check if literal
isNumeric(?x)                 # Check if numeric
bound(?var)                   # Check if bound
```

### Conditional Functions

```sparql
IF(?cond, ?then, ?else)       # Conditional
COALESCE(?a, ?b, ?c)          # First non-error value
```

---

## Update Operations

```sparql
# INSERT DATA - Add ground triples
INSERT DATA {
  :alice foaf:name "Alice" .
  :alice foaf:age 30 .
}

# DELETE DATA - Remove specific triples
DELETE DATA {
  :alice foaf:age 30 .
}

# DELETE/INSERT - Pattern-based update
DELETE { ?person foaf:age ?old }
INSERT { ?person foaf:age ?new }
WHERE {
  ?person foaf:name "Alice" .
  ?person foaf:age ?old .
  BIND(?old + 1 AS ?new)
}

# DELETE WHERE - Shorthand
DELETE WHERE {
  ?person foaf:email ?email .
  FILTER(CONTAINS(?email, "@oldcompany.com"))
}

# LOAD - Load RDF document
LOAD <http://example.org/data.ttl>
LOAD <http://example.org/data.ttl> INTO GRAPH <http://example.org/g1>

# CLEAR - Remove all triples
CLEAR GRAPH <http://example.org/g1>
CLEAR DEFAULT                 # Clear default graph
CLEAR NAMED                   # Clear all named graphs
CLEAR ALL                     # Clear everything

# CREATE - Create empty graph
CREATE GRAPH <http://example.org/g1>

# DROP - Remove graph
DROP GRAPH <http://example.org/g1>
DROP DEFAULT
DROP ALL

# COPY - Copy graph
COPY GRAPH <http://example.org/g1> TO GRAPH <http://example.org/g2>

# MOVE - Move graph
MOVE GRAPH <http://example.org/g1> TO GRAPH <http://example.org/g2>

# ADD - Add to graph
ADD GRAPH <http://example.org/g1> TO GRAPH <http://example.org/g2>
```

---

## Named Graphs

```sparql
# FROM - Query specific graph
SELECT ?s ?p ?o
FROM <http://example.org/graph1>
WHERE { ?s ?p ?o }

# GRAPH - Graph pattern
SELECT ?s ?p ?o ?g
WHERE {
  GRAPH ?g {
    ?s ?p ?o .
  }
}

# Insert into named graph
INSERT DATA {
  GRAPH <http://example.org/g1> {
    :alice foaf:name "Alice" .
  }
}
```

---

## Negation

```sparql
# NOT EXISTS - Filter negation
FILTER NOT EXISTS {
  ?person foaf:email ?email
}

# MINUS - Set difference
{
  ?person a foaf:Person .
}
MINUS {
  ?person foaf:email ?email .
}
```

---

## Common Patterns

### Find all triples

```sparql
SELECT ?s ?p ?o WHERE { ?s ?p ?o } LIMIT 100
```

### Count triples

```sparql
SELECT (COUNT(*) AS ?count) WHERE { ?s ?p ?o }
```

### List all predicates

```sparql
SELECT DISTINCT ?predicate WHERE { ?s ?predicate ?o }
```

### List all types

```sparql
SELECT DISTINCT ?type WHERE { ?s a ?type }
```

### Full-text search (implementation-specific)

```sparql
?document dc:content ?content .
FILTER(CONTAINS(LCASE(?content), "search term"))
```

### Pagination

```sparql
SELECT ?x WHERE { ... }
ORDER BY ?x
LIMIT 20 OFFSET 40  # Page 3, 20 per page
```

### Date range

```sparql
?event ex:date ?date .
FILTER(?date >= "2025-01-01"^^xsd:date && ?date < "2026-01-01"^^xsd:date)
```

### Optional chain

```sparql
?person foaf:knows ?friend .
OPTIONAL {
  ?friend foaf:knows ?friendOfFriend .
  OPTIONAL {
    ?friendOfFriend foaf:name ?name .
  }
}
```

---

## Performance Tips

1. **Be specific**: Use exact predicates instead of `?p`
2. **Order matters**: Put most selective patterns first
3. **Use LIMIT**: Always limit results when exploring
4. **Avoid cartesian products**: Connect patterns with shared variables
5. **Index-friendly**: Query by subject or predicate when possible
6. **OPTIONAL is expensive**: Use sparingly
7. **Property paths**: Simple paths (/, ^) are faster than complex ones (+, *)

---

## Common XSD Datatypes

```sparql
xsd:string                    # String (default for plain literals)
xsd:integer                   # Integer
xsd:decimal                   # Decimal number
xsd:double                    # Double-precision float
xsd:boolean                   # Boolean (true/false)
xsd:date                      # Date (YYYY-MM-DD)
xsd:dateTime                  # Date and time
xsd:time                      # Time
xsd:duration                  # Duration (P1Y2M3DT4H5M6S)
```

---

## Result Formats

- **JSON**: `application/sparql-results+json`
- **XML**: `application/sparql-results+xml`
- **CSV**: `text/csv`
- **TSV**: `text/tab-separated-values`

---

## Error Handling

```sparql
# Use COALESCE for defaults
SELECT ?name (COALESCE(?email, "no-email") AS ?contact)
WHERE {
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:email ?email }
}

# Use IF for conditional logic
SELECT ?name (IF(bound(?email), ?email, "N/A") AS ?contact)
WHERE {
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:email ?email }
}

# Silent operations (UPDATE)
LOAD SILENT <http://example.org/data.ttl>
DROP SILENT GRAPH <http://example.org/g1>
```

---

## RuVector Integration Examples

### Vector similarity in SPARQL

```sql
SELECT
  r.object AS name,
  ruvector_cosine_similarity(e.embedding, $1) AS similarity
FROM ruvector_rdf_triples r
JOIN person_embeddings e ON r.subject = e.person_iri
WHERE r.predicate = 'http://xmlns.com/foaf/0.1/name'
  AND ruvector_cosine_similarity(e.embedding, $1) > 0.8
ORDER BY similarity DESC
LIMIT 10;
```

### Hybrid knowledge graph + vector search

```sql
-- SPARQL pattern matching + vector ranking
WITH sparql_results AS (
  SELECT t1.subject AS person, t1.object AS name
  FROM ruvector_rdf_triples t1
  JOIN ruvector_rdf_triples t2 ON t1.subject = t2.subject
  WHERE t1.predicate = 'http://xmlns.com/foaf/0.1/name'
    AND t2.predicate = 'http://example.org/interests'
    AND t2.object = 'machine learning'
)
SELECT
  s.person,
  s.name,
  e.embedding <=> $1::ruvector AS distance
FROM sparql_results s
JOIN person_embeddings e ON s.person = e.person_iri
ORDER BY distance
LIMIT 20;
```

---

## Resources

- **W3C SPARQL 1.1**: https://www.w3.org/TR/sparql11-query/
- **Full Specification**: [SPARQL_SPECIFICATION.md](./SPARQL_SPECIFICATION.md)
- **Examples**: [EXAMPLES.md](./EXAMPLES.md)
- **Implementation Guide**: [IMPLEMENTATION_GUIDE.md](./IMPLEMENTATION_GUIDE.md)

---

**Print this page for quick reference during development!**
