# SPARQL 1.1 Specification for PostgreSQL Implementation

**Version**: SPARQL 1.1 (W3C Recommendation March 2013)
**Target**: PostgreSQL Query Engine Implementation
**Research Date**: December 2025
**Project**: RuVector-Postgres

---

## Table of Contents

1. [Introduction](#introduction)
2. [Core SPARQL Components](#core-sparql-components)
3. [SPARQL Syntax](#sparql-syntax)
4. [SPARQL Operations](#sparql-operations)
5. [SPARQL Update](#sparql-update)
6. [Built-in Functions](#built-in-functions)
7. [SPARQL Algebra](#sparql-algebra)
8. [Query Result Formats](#query-result-formats)
9. [Implementation Considerations](#implementation-considerations)
10. [References](#references)

---

## Introduction

SPARQL (SPARQL Protocol and RDF Query Language) is a W3C standard query language for querying and manipulating RDF (Resource Description Framework) data. RDF is a directed, labeled graph data format representing information as triples (subject, predicate, object).

### SPARQL 1.1 Enhancements

SPARQL 1.1 adds significant features over SPARQL 1.0:
- **Subqueries**: Nested SELECT queries
- **Value assignment**: BIND and VALUES clauses
- **Property paths**: Regular expression-like path matching
- **Aggregates**: COUNT, SUM, AVG, MIN, MAX, GROUP_CONCAT, SAMPLE
- **Negation**: NOT EXISTS and MINUS operators
- **Service federation**: Querying remote SPARQL endpoints
- **Update operations**: INSERT, DELETE, LOAD, CLEAR, etc.

---

## Core SPARQL Components

### 1. RDF Triple Model

The foundation of SPARQL is the RDF triple:

```
<subject> <predicate> <object>
```

**Components:**
- **Subject**: IRI or blank node
- **Predicate**: IRI only
- **Object**: IRI, blank node, or literal

**Example:**
```turtle
<http://example.org/person/Alice> <http://xmlns.com/foaf/0.1/knows> <http://example.org/person/Bob>
```

### 2. Graph Patterns

Graph patterns are the building blocks of SPARQL queries:

#### Basic Graph Pattern (BGP)

A set of triple patterns that must all match:

```sparql
?person foaf:name ?name .
?person foaf:age ?age .
```

#### Group Graph Pattern

Multiple patterns enclosed in braces:

```sparql
{
  ?person foaf:name ?name .
  ?person foaf:age ?age .
  FILTER(?age >= 18)
}
```

#### Optional Graph Pattern

Extends solutions with additional patterns if they match:

```sparql
?person foaf:name ?name .
OPTIONAL { ?person foaf:email ?email }
```

**Semantics**: LEFT JOIN - keeps all solutions from the first pattern whether or not the OPTIONAL pattern matches.

#### Union Graph Pattern

Alternatives - tries multiple patterns:

```sparql
{
  { ?person foaf:name ?name }
  UNION
  { ?person rdfs:label ?name }
}
```

#### Filter Pattern

Constrains solutions with boolean expressions:

```sparql
?person foaf:age ?age .
FILTER(?age >= 21 && ?age < 65)
```

### 3. Query Forms

SPARQL defines four query forms:

#### SELECT Query

Returns variable bindings as a table:

```sparql
SELECT ?name ?age
WHERE {
  ?person foaf:name ?name .
  ?person foaf:age ?age .
}
```

**Returns**: Solution sequence (table of variable bindings)

#### CONSTRUCT Query

Builds an RDF graph using a template:

```sparql
CONSTRUCT {
  ?person ex:hasName ?name .
  ?person ex:hasAge ?age .
}
WHERE {
  ?person foaf:name ?name .
  ?person foaf:age ?age .
}
```

**Returns**: RDF graph

**Shorthand**:
```sparql
CONSTRUCT WHERE {
  ?s ?p ?o .
}
```

#### ASK Query

Returns boolean indicating if pattern matches:

```sparql
ASK {
  ?person foaf:name "Alice" .
}
```

**Returns**: `true` or `false`

#### DESCRIBE Query

Returns RDF description of resources:

```sparql
DESCRIBE <http://example.org/person/Alice>
```

**Returns**: Implementation-specific RDF graph describing the resource

### 4. Solution Modifiers

Modifiers that transform query results:

#### ORDER BY

Sorts results by one or more expressions:

```sparql
SELECT ?name ?age
WHERE { ?person foaf:name ?name ; foaf:age ?age }
ORDER BY DESC(?age) ?name
```

**Options**: `ASC(?expr)` (ascending, default), `DESC(?expr)` (descending)

#### DISTINCT

Removes duplicate solutions:

```sparql
SELECT DISTINCT ?name
WHERE { ?person foaf:name ?name }
```

#### REDUCED

Allows (but doesn't require) duplicate elimination:

```sparql
SELECT REDUCED ?name
WHERE { ?person foaf:name ?name }
```

#### LIMIT

Restricts result count:

```sparql
SELECT ?name
WHERE { ?person foaf:name ?name }
LIMIT 10
```

#### OFFSET

Skips initial solutions:

```sparql
SELECT ?name
WHERE { ?person foaf:name ?name }
OFFSET 20
LIMIT 10
```

#### GROUP BY

Groups solutions for aggregation:

```sparql
SELECT ?company (COUNT(?employee) AS ?empCount)
WHERE {
  ?employee foaf:workplaceHomepage ?company .
}
GROUP BY ?company
```

#### HAVING

Filters grouped results:

```sparql
SELECT ?company (COUNT(?employee) AS ?empCount)
WHERE {
  ?employee foaf:workplaceHomepage ?company .
}
GROUP BY ?company
HAVING (COUNT(?employee) >= 10)
```

---

## SPARQL Syntax

### PREFIX Declarations

Associate prefix labels with IRIs:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

SELECT ?name
WHERE {
  ?person foaf:name ?name .
}
```

### BASE Declaration

Define base IRI for relative IRIs:

```sparql
BASE <http://example.org/>

SELECT ?name
WHERE {
  <person/Alice> foaf:name ?name .
}
```

### Variable Syntax

Variables start with `?` or `$`:

```sparql
?name
$age
```

**Note**: `?var` and `$var` refer to the same variable.

### URI/IRI Syntax

Three ways to specify IRIs:

1. **Full IRI**: `<http://example.org/resource>`
2. **Prefixed name**: `prefix:localPart` (e.g., `foaf:name`)
3. **Relative IRI**: `<resource>` (resolved against BASE)

### Literal Syntax

#### String Literals

```sparql
"simple string"
'another string'
"""multi-line
string"""
'''another
multi-line'''
```

#### Numeric Literals

```sparql
42                    # xsd:integer
3.14                  # xsd:decimal
1.5e6                 # xsd:double
```

#### Boolean Literals

```sparql
true
false
```

#### Language-Tagged Literals

```sparql
"chat"@en
"chat"@fr
```

#### Typed Literals

```sparql
"42"^^xsd:integer
"2025-12-09"^^xsd:date
"P1Y2M"^^xsd:duration
```

### Blank Node Syntax

#### Labeled Blank Nodes

```sparql
_:label
_:alice
```

#### Anonymous Blank Nodes

```sparql
[]                          # Empty blank node
[ foaf:name "Alice" ]       # Blank node with properties
```

#### Blank Node Property Lists

```sparql
[
  foaf:name "Alice" ;
  foaf:age 30 ;
  foaf:knows [ foaf:name "Bob" ]
]
```

### Triple Pattern Abbreviations

#### Semicolon (;) - Shared Subject

```sparql
?person foaf:name "Alice" ;
        foaf:age 30 ;
        foaf:knows ?friend .

# Equivalent to:
?person foaf:name "Alice" .
?person foaf:age 30 .
?person foaf:knows ?friend .
```

#### Comma (,) - Shared Subject-Predicate

```sparql
?person foaf:knows ?bob, ?charlie, ?diana .

# Equivalent to:
?person foaf:knows ?bob .
?person foaf:knows ?charlie .
?person foaf:knows ?diana .
```

#### rdf:type Shorthand (a)

```sparql
?person a foaf:Person .

# Equivalent to:
?person rdf:type foaf:Person .
```

### Collections (RDF Lists)

```sparql
?list rdf:rest*/rdf:first ?item .

# Or using collection syntax:
?x foaf:knows ( :Alice :Bob :Charlie ) .
```

---

## SPARQL Operations

### 1. Pattern Matching

#### Basic Triple Patterns

```sparql
SELECT ?subject ?object
WHERE {
  ?subject foaf:knows ?object .
}
```

#### Multiple Patterns (Conjunction)

```sparql
SELECT ?name ?email
WHERE {
  ?person foaf:name ?name .
  ?person foaf:email ?email .
}
```

### 2. FILTER Expressions

Apply constraints to solutions:

```sparql
SELECT ?name ?age
WHERE {
  ?person foaf:name ?name .
  ?person foaf:age ?age .
  FILTER(?age >= 18 && ?age < 65)
}
```

#### FILTER Operators

**Logical:**
- `&&` (AND)
- `||` (OR)
- `!` (NOT)

**Comparison:**
- `=` (equals)
- `!=` (not equals)
- `<` (less than)
- `>` (greater than)
- `<=` (less than or equal)
- `>=` (greater than or equal)

**Arithmetic:**
- `+` (addition)
- `-` (subtraction)
- `*` (multiplication)
- `/` (division)

**Other:**
- `IN` (set membership)
- `NOT IN` (set non-membership)

### 3. OPTIONAL Patterns

Left join - augments solutions if pattern matches:

```sparql
SELECT ?name ?email
WHERE {
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:email ?email }
}
```

**Multiple OPTIONAL blocks:**

```sparql
SELECT ?name ?email ?phone
WHERE {
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:email ?email }
  OPTIONAL { ?person foaf:phone ?phone }
}
```

**OPTIONAL with FILTER:**

```sparql
SELECT ?name ?email
WHERE {
  ?person foaf:name ?name .
  OPTIONAL {
    ?person foaf:email ?email .
    FILTER(CONTAINS(?email, "@example.com"))
  }
}
```

### 4. UNION Patterns

Disjunction - tries alternative patterns:

```sparql
SELECT ?name
WHERE {
  {
    ?person foaf:name ?name .
  }
  UNION
  {
    ?person rdfs:label ?name .
  }
}
```

### 5. Property Paths

Regular expression-like patterns over properties:

#### Operators

| Operator | Syntax | Description |
|----------|--------|-------------|
| Sequence | `elt1 / elt2` | Follow elt1, then elt2 |
| Alternative | `elt1 \| elt2` | Try elt1 or elt2 |
| Inverse | `^elt` | Reverse direction (object to subject) |
| Zero or more | `elt*` | Zero or more occurrences |
| One or more | `elt+` | One or more occurrences |
| Zero or one | `elt?` | Optional occurrence |
| Negation | `!elt` | Not this property |
| Negated set | `!(elt1\|elt2)` | None of these properties |

#### Examples

**Transitive closure:**
```sparql
SELECT ?ancestor
WHERE {
  ?person foaf:knows+ ?ancestor .
}
```

**Path sequence:**
```sparql
SELECT ?grandchild
WHERE {
  ?person ex:hasChild / ex:hasChild ?grandchild .
}
```

**Inverse path:**
```sparql
SELECT ?child
WHERE {
  ?parent ^ex:hasChild ?child .
}
```

**Alternative paths:**
```sparql
SELECT ?name
WHERE {
  ?person (foaf:name | rdfs:label) ?name .
}
```

**Negated property set:**
```sparql
SELECT ?x ?y
WHERE {
  ?x !rdf:type ?y .
}
```

### 6. Subqueries

Nested SELECT queries:

```sparql
SELECT ?name ?avgAge
WHERE {
  {
    SELECT ?company (AVG(?age) AS ?avgAge)
    WHERE {
      ?employee foaf:workplaceHomepage ?company .
      ?employee foaf:age ?age .
    }
    GROUP BY ?company
  }
  ?company rdfs:label ?name .
}
```

### 7. Negation

#### NOT EXISTS

Pattern must not match:

```sparql
SELECT ?person
WHERE {
  ?person a foaf:Person .
  FILTER NOT EXISTS { ?person foaf:email ?email }
}
```

#### MINUS

Set difference operation:

```sparql
SELECT ?person
WHERE {
  ?person a foaf:Person .
  MINUS { ?person foaf:email ?email }
}
```

**Difference**: `NOT EXISTS` is a filter, `MINUS` is a set operation.

### 8. VALUES

Inject inline data:

```sparql
SELECT ?name ?age
WHERE {
  VALUES (?person ?age) {
    (<http://example.org/alice> 30)
    (<http://example.org/bob> 25)
  }
  ?person foaf:name ?name .
}
```

**Single variable:**
```sparql
VALUES ?x { :a :b :c }
```

**Multiple variables:**
```sparql
VALUES (?x ?y) {
  (:a 1)
  (:b 2)
  UNDEF
}
```

### 9. BIND

Assign values to variables:

```sparql
SELECT ?name ?fullName
WHERE {
  ?person foaf:givenName ?first .
  ?person foaf:familyName ?last .
  BIND(CONCAT(?first, " ", ?last) AS ?fullName)
}
```

### 10. Aggregates

#### Aggregate Functions

- `COUNT(?var)` or `COUNT(*)` - Count solutions
- `SUM(?expr)` - Sum numeric values
- `AVG(?expr)` - Average numeric values
- `MIN(?expr)` - Minimum value
- `MAX(?expr)` - Maximum value
- `GROUP_CONCAT(?expr)` - Concatenate strings
- `SAMPLE(?expr)` - Arbitrary value

#### GROUP BY Example

```sparql
SELECT ?company (COUNT(?employee) AS ?count) (AVG(?salary) AS ?avgSalary)
WHERE {
  ?employee foaf:workplaceHomepage ?company .
  ?employee ex:salary ?salary .
}
GROUP BY ?company
```

#### HAVING Example

```sparql
SELECT ?company (AVG(?salary) AS ?avgSalary)
WHERE {
  ?employee foaf:workplaceHomepage ?company .
  ?employee ex:salary ?salary .
}
GROUP BY ?company
HAVING (AVG(?salary) > 50000)
```

### 11. Named Graphs

Query specific graphs:

```sparql
SELECT ?name
FROM <http://example.org/graph1>
WHERE {
  ?person foaf:name ?name .
}
```

**GRAPH keyword:**
```sparql
SELECT ?name ?graph
WHERE {
  GRAPH ?graph {
    ?person foaf:name ?name .
  }
}
```

**Multiple graphs:**
```sparql
SELECT ?name
FROM <http://example.org/graph1>
FROM <http://example.org/graph2>
WHERE {
  ?person foaf:name ?name .
}
```

---

## SPARQL Update

SPARQL 1.1 Update provides operations for modifying RDF graphs.

### 1. INSERT DATA

Add ground triples (no variables):

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

INSERT DATA {
  <http://example.org/alice> foaf:name "Alice" .
  <http://example.org/alice> foaf:age 30 .
}
```

**With named graph:**
```sparql
INSERT DATA {
  GRAPH <http://example.org/graph1> {
    <http://example.org/alice> foaf:name "Alice" .
  }
}
```

**Behavior**:
- Creates graph if it doesn't exist (SHOULD)
- Blank nodes are "fresh" (distinct from existing nodes)
- No effect if triples already exist

### 2. DELETE DATA

Remove specific triples:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

DELETE DATA {
  <http://example.org/alice> foaf:age 30 .
}
```

**With named graph:**
```sparql
DELETE DATA {
  GRAPH <http://example.org/graph1> {
    <http://example.org/alice> foaf:age 30 .
  }
}
```

**Behavior**:
- No error if triples don't exist
- No variables or blank nodes allowed

### 3. DELETE/INSERT

Pattern-based updates using WHERE clause:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

DELETE { ?person foaf:age ?age }
INSERT { ?person foaf:age 31 }
WHERE {
  ?person foaf:name "Alice" .
  ?person foaf:age ?age .
  FILTER(?age = 30)
}
```

**Order**: DELETE executes before INSERT

**Only DELETE:**
```sparql
DELETE { ?person foaf:email ?email }
WHERE {
  ?person foaf:email ?email .
  FILTER(CONTAINS(?email, "@oldcompany.com"))
}
```

**Only INSERT:**
```sparql
INSERT {
  ?person foaf:age 0 .
}
WHERE {
  ?person a foaf:Person .
  FILTER NOT EXISTS { ?person foaf:age ?age }
}
```

### 4. DELETE WHERE

Shorthand for DELETE...WHERE without INSERT:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

DELETE WHERE {
  ?person foaf:email ?email .
  FILTER(CONTAINS(?email, "@spam.com"))
}
```

### 5. LOAD

Load RDF document from IRI into graph:

```sparql
LOAD <http://example.org/data.ttl>
```

**Into named graph:**
```sparql
LOAD <http://example.org/data.ttl> INTO GRAPH <http://example.org/graph1>
```

**Silent mode (no error):**
```sparql
LOAD SILENT <http://example.org/data.ttl>
```

### 6. CLEAR

Remove all triples from graph:

```sparql
CLEAR GRAPH <http://example.org/graph1>
```

**Options:**
- `CLEAR DEFAULT` - Clear default graph
- `CLEAR NAMED` - Clear all named graphs
- `CLEAR ALL` - Clear all graphs
- `CLEAR SILENT GRAPH <uri>` - No error if graph doesn't exist

### 7. CREATE

Create empty graph:

```sparql
CREATE GRAPH <http://example.org/graph1>
```

**Silent mode:**
```sparql
CREATE SILENT GRAPH <http://example.org/graph1>
```

### 8. DROP

Remove graph entirely:

```sparql
DROP GRAPH <http://example.org/graph1>
```

**Options:**
- `DROP DEFAULT` - Equivalent to CLEAR DEFAULT
- `DROP NAMED` - Drop all named graphs
- `DROP ALL` - Drop all graphs
- `DROP SILENT GRAPH <uri>` - No error if graph doesn't exist

### 9. COPY

Copy graph content to another graph:

```sparql
COPY GRAPH <http://example.org/source> TO GRAPH <http://example.org/dest>
```

**Behavior**:
- Destination graph cleared first
- Source unchanged

### 10. MOVE

Move graph content to another graph:

```sparql
MOVE GRAPH <http://example.org/source> TO GRAPH <http://example.org/dest>
```

**Behavior**:
- Destination graph cleared first
- Source graph cleared after copy

### 11. ADD

Add graph content to another graph:

```sparql
ADD GRAPH <http://example.org/source> TO GRAPH <http://example.org/dest>
```

**Behavior**:
- Destination graph augmented
- Source unchanged

### 12. WITH Clause

Default graph for update operations:

```sparql
WITH <http://example.org/graph1>
DELETE { ?person foaf:age ?age }
INSERT { ?person foaf:age 31 }
WHERE {
  ?person foaf:name "Alice" .
  ?person foaf:age ?age .
}
```

### 13. USING and USING NAMED

Specify graphs for WHERE clause:

```sparql
DELETE { GRAPH <http://example.org/dest> { ?s ?p ?o } }
USING <http://example.org/source>
WHERE {
  ?s ?p ?o .
  FILTER(?p = foaf:age)
}
```

---

## Built-in Functions

### 1. Logical and Conditional

#### bound(?var)

Test if variable is bound:

```sparql
SELECT ?name ?email
WHERE {
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:email ?email }
  FILTER(bound(?email))
}
```

#### IF(?cond, ?then, ?else)

Conditional expression:

```sparql
SELECT ?name (IF(?age >= 18, "adult", "minor") AS ?status)
WHERE {
  ?person foaf:name ?name .
  ?person foaf:age ?age .
}
```

#### COALESCE(?expr1, ?expr2, ...)

Return first non-error value:

```sparql
SELECT ?name (COALESCE(?email, ?phone, "no contact") AS ?contact)
WHERE {
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:email ?email }
  OPTIONAL { ?person foaf:phone ?phone }
}
```

#### EXISTS { pattern }

Test if pattern matches:

```sparql
SELECT ?person
WHERE {
  ?person a foaf:Person .
  FILTER EXISTS { ?person foaf:email ?email }
}
```

#### NOT EXISTS { pattern }

Test if pattern doesn't match:

```sparql
SELECT ?person
WHERE {
  ?person a foaf:Person .
  FILTER NOT EXISTS { ?person foaf:email ?email }
}
```

#### IN and NOT IN

Set membership:

```sparql
SELECT ?name
WHERE {
  ?person foaf:name ?name .
  ?person foaf:age ?age .
  FILTER(?age IN (20, 25, 30, 35))
}
```

### 2. RDF Term Functions

#### isIRI(?x) / isURI(?x)

Test if value is IRI:

```sparql
FILTER(isIRI(?resource))
```

#### isBlank(?x)

Test if value is blank node:

```sparql
FILTER(isBlank(?node))
```

#### isLiteral(?x)

Test if value is literal:

```sparql
FILTER(isLiteral(?value))
```

#### isNumeric(?x)

Test if value is numeric:

```sparql
FILTER(isNumeric(?value))
```

#### str(?x)

Convert to string:

```sparql
SELECT (str(?uri) AS ?string)
WHERE { ?s ?p ?uri }
```

#### lang(?x)

Extract language tag:

```sparql
SELECT ?name (lang(?name) AS ?language)
WHERE {
  ?person foaf:name ?name .
  FILTER(lang(?name) = "en")
}
```

#### datatype(?x)

Get datatype IRI:

```sparql
SELECT ?value (datatype(?value) AS ?type)
WHERE { ?s ?p ?value }
```

#### IRI(?x) / URI(?x)

Construct IRI from string:

```sparql
BIND(IRI(CONCAT("http://example.org/", ?id)) AS ?resource)
```

#### BNODE() / BNODE(?label)

Create blank node:

```sparql
BIND(BNODE() AS ?newNode)
```

#### STRDT(?string, ?datatype)

Create typed literal:

```sparql
BIND(STRDT("42", xsd:integer) AS ?number)
```

#### STRLANG(?string, ?language)

Create language-tagged literal:

```sparql
BIND(STRLANG("hello", "en") AS ?greeting)
```

#### UUID()

Generate random UUID:

```sparql
BIND(UUID() AS ?id)
```

#### STRUUID()

Generate string UUID:

```sparql
BIND(STRUUID() AS ?idString)
```

### 3. String Functions

#### STRLEN(?string)

String length:

```sparql
SELECT ?name (STRLEN(?name) AS ?length)
WHERE { ?person foaf:name ?name }
```

#### SUBSTR(?string, ?start, ?length)

Extract substring:

```sparql
SELECT (SUBSTR(?name, 1, 3) AS ?initials)
WHERE { ?person foaf:name ?name }
```

**Note**: Start position is 1-based

#### UCASE(?string)

Convert to uppercase:

```sparql
SELECT (UCASE(?name) AS ?upper)
WHERE { ?person foaf:name ?name }
```

#### LCASE(?string)

Convert to lowercase:

```sparql
SELECT (LCASE(?name) AS ?lower)
WHERE { ?person foaf:name ?name }
```

#### STRSTARTS(?string, ?prefix)

Test if string starts with prefix:

```sparql
FILTER(STRSTARTS(?email, "admin@"))
```

#### STRENDS(?string, ?suffix)

Test if string ends with suffix:

```sparql
FILTER(STRENDS(?email, "@example.com"))
```

#### CONTAINS(?string, ?substring)

Test if string contains substring:

```sparql
FILTER(CONTAINS(?description, "important"))
```

#### STRBEFORE(?string, ?search)

Extract text before substring:

```sparql
SELECT (STRBEFORE(?email, "@") AS ?username)
WHERE { ?person foaf:email ?email }
```

#### STRAFTER(?string, ?search)

Extract text after substring:

```sparql
SELECT (STRAFTER(?email, "@") AS ?domain)
WHERE { ?person foaf:email ?email }
```

#### ENCODE_FOR_URI(?string)

Percent-encode for URI:

```sparql
SELECT (ENCODE_FOR_URI(?name) AS ?encoded)
WHERE { ?person foaf:name ?name }
```

#### CONCAT(?string1, ?string2, ...)

Concatenate strings:

```sparql
SELECT (CONCAT(?first, " ", ?last) AS ?fullName)
WHERE {
  ?person foaf:givenName ?first .
  ?person foaf:familyName ?last .
}
```

#### langMatches(?tag, ?range)

Match language tags:

```sparql
FILTER(langMatches(lang(?label), "en"))
```

**Language ranges:**
- `"en"` - Exact match
- `"*"` - Any language
- `"en-US"` - Specific locale

#### REGEX(?string, ?pattern) / REGEX(?string, ?pattern, ?flags)

Regular expression matching:

```sparql
FILTER(REGEX(?email, "^[a-z]+@example\\.com$", "i"))
```

**Flags:**
- `"i"` - Case insensitive
- `"s"` - Dot matches newline
- `"m"` - Multi-line mode
- `"x"` - Ignore whitespace

#### REPLACE(?string, ?pattern, ?replacement) / REPLACE(?string, ?pattern, ?replacement, ?flags)

String replacement:

```sparql
SELECT (REPLACE(?phone, "[^0-9]", "") AS ?digitsOnly)
WHERE { ?person foaf:phone ?phone }
```

### 4. Numeric Functions

#### abs(?number)

Absolute value:

```sparql
SELECT (abs(?diff) AS ?absDiff)
WHERE { BIND(?a - ?b AS ?diff) }
```

#### round(?number)

Round to nearest integer:

```sparql
SELECT (round(?value) AS ?rounded)
WHERE { ?item ex:price ?value }
```

#### ceil(?number)

Ceiling function:

```sparql
SELECT (ceil(?value) AS ?ceiling)
WHERE { ?item ex:price ?value }
```

#### floor(?number)

Floor function:

```sparql
SELECT (floor(?value) AS ?floor)
WHERE { ?item ex:price ?value }
```

#### RAND()

Random number [0, 1):

```sparql
SELECT ?item
WHERE {
  ?item a ex:Product .
  FILTER(RAND() < 0.1)
}
```

### 5. Date/Time Functions

#### now()

Current timestamp:

```sparql
BIND(now() AS ?currentTime)
```

#### year(?datetime)

Extract year:

```sparql
SELECT (year(?date) AS ?year)
WHERE { ?event ex:date ?date }
```

#### month(?datetime)

Extract month (1-12):

```sparql
SELECT (month(?date) AS ?month)
WHERE { ?event ex:date ?date }
```

#### day(?datetime)

Extract day (1-31):

```sparql
SELECT (day(?date) AS ?day)
WHERE { ?event ex:date ?date }
```

#### hours(?datetime)

Extract hours (0-23):

```sparql
SELECT (hours(?timestamp) AS ?hour)
WHERE { ?event ex:timestamp ?timestamp }
```

#### minutes(?datetime)

Extract minutes (0-59):

```sparql
SELECT (minutes(?timestamp) AS ?minute)
WHERE { ?event ex:timestamp ?timestamp }
```

#### seconds(?datetime)

Extract seconds (0-59.999...):

```sparql
SELECT (seconds(?timestamp) AS ?second)
WHERE { ?event ex:timestamp ?timestamp }
```

#### timezone(?datetime)

Extract timezone:

```sparql
SELECT (timezone(?timestamp) AS ?tz)
WHERE { ?event ex:timestamp ?timestamp }
```

#### tz(?datetime)

Timezone abbreviation:

```sparql
SELECT (tz(?timestamp) AS ?tzAbbr)
WHERE { ?event ex:timestamp ?timestamp }
```

### 6. Hash Functions

#### MD5(?string)

MD5 hash (lowercase hex):

```sparql
SELECT (MD5(?email) AS ?hash)
WHERE { ?person foaf:email ?email }
```

#### SHA1(?string)

SHA-1 hash:

```sparql
SELECT (SHA1(?password) AS ?hash)
WHERE { ?user ex:password ?password }
```

#### SHA256(?string)

SHA-256 hash:

```sparql
SELECT (SHA256(?data) AS ?hash)
WHERE { ?item ex:data ?data }
```

#### SHA384(?string)

SHA-384 hash:

```sparql
SELECT (SHA384(?data) AS ?hash)
WHERE { ?item ex:data ?data }
```

#### SHA512(?string)

SHA-512 hash:

```sparql
SELECT (SHA512(?data) AS ?hash)
WHERE { ?item ex:data ?data }
```

### 7. Aggregate Functions

#### COUNT(?var) / COUNT(*)

Count solutions:

```sparql
SELECT ?company (COUNT(?employee) AS ?count)
WHERE {
  ?employee foaf:workplaceHomepage ?company .
}
GROUP BY ?company
```

**DISTINCT modifier:**
```sparql
SELECT (COUNT(DISTINCT ?type) AS ?typeCount)
WHERE { ?s rdf:type ?type }
```

#### SUM(?expr)

Sum numeric values:

```sparql
SELECT ?department (SUM(?salary) AS ?totalSalary)
WHERE {
  ?employee ex:department ?department .
  ?employee ex:salary ?salary .
}
GROUP BY ?department
```

#### AVG(?expr)

Average numeric values:

```sparql
SELECT ?department (AVG(?salary) AS ?avgSalary)
WHERE {
  ?employee ex:department ?department .
  ?employee ex:salary ?salary .
}
GROUP BY ?department
```

#### MIN(?expr)

Minimum value:

```sparql
SELECT ?department (MIN(?salary) AS ?minSalary)
WHERE {
  ?employee ex:department ?department .
  ?employee ex:salary ?salary .
}
GROUP BY ?department
```

#### MAX(?expr)

Maximum value:

```sparql
SELECT ?department (MAX(?salary) AS ?maxSalary)
WHERE {
  ?employee ex:department ?department .
  ?employee ex:salary ?salary .
}
GROUP BY ?department
```

#### GROUP_CONCAT(?expr) / GROUP_CONCAT(?expr; SEPARATOR = ?sep)

Concatenate values:

```sparql
SELECT ?person (GROUP_CONCAT(?skill; SEPARATOR = ", ") AS ?skills)
WHERE {
  ?person ex:hasSkill ?skill .
}
GROUP BY ?person
```

#### SAMPLE(?expr)

Arbitrary value from group:

```sparql
SELECT ?company (SAMPLE(?employee) AS ?anyEmployee)
WHERE {
  ?employee foaf:workplaceHomepage ?company .
}
GROUP BY ?company
```

---

## SPARQL Algebra

The SPARQL algebra defines formal semantics for query evaluation.

### Core Algebraic Operators

#### 1. Basic Graph Pattern (BGP)

A set of triple patterns:

```
BGP(tp1, tp2, ..., tpn)
```

#### 2. Join (⋈)

Combines two graph patterns:

```
P1 Join P2
```

**Definition**: Solutions that match both P1 and P2 with compatible bindings.

#### 3. LeftJoin (⟕)

Left outer join (OPTIONAL):

```
LeftJoin(P1, P2, expr)
```

**Definition**: All solutions from P1, augmented with P2 solutions where compatible and expr is true.

**Formal**:
```
LeftJoin(Ω1, Ω2, expr) =
  { merge(μ1, μ2) | μ1 ∈ Ω1, μ2 ∈ Ω2, compatible(μ1, μ2), expr(merge(μ1, μ2)) = true }
  ∪ { μ1 | μ1 ∈ Ω1, ∀μ2 ∈ Ω2: ¬compatible(μ1, μ2) }
  ∪ { μ1 | μ1 ∈ Ω1, ∃μ2 ∈ Ω2: compatible(μ1, μ2), expr(merge(μ1, μ2)) = false }
```

#### 4. Filter (σ)

Selection operation:

```
Filter(expr, P)
```

**Definition**: Solutions from P where expr evaluates to true.

#### 5. Union (∪)

Disjunction:

```
Union(P1, P2)
```

**Definition**: Solutions from P1 or P2 (bag union).

#### 6. Minus

Set difference:

```
Minus(P1, P2)
```

**Definition**: Solutions from P1 that don't join with any solution from P2.

#### 7. Graph

Named graph pattern:

```
Graph(g, P)
```

**Definition**: Evaluate P against graph g.

#### 8. Extend (Bind)

Add variable binding:

```
Extend(P, ?var, expr)
```

**Definition**: Add binding ?var = expr to each solution in P.

#### 9. Project (π)

Project variables:

```
Project(P, vars)
```

**Definition**: Keep only specified variables from P.

#### 10. Distinct

Remove duplicates:

```
Distinct(P)
```

#### 11. Reduced

Allow duplicate removal:

```
Reduced(P)
```

#### 12. OrderBy

Sort solutions:

```
OrderBy(P, conditions)
```

#### 13. Slice

Limit and offset:

```
Slice(P, start, length)
```

#### 14. ToList

Convert to solution sequence:

```
ToList(P)
```

### Algebraic Properties

**Important**: Unlike relational algebra, SPARQL's LeftJoin is **NOT distributive over Union**:

```
LeftJoin(P1, Union(P2, P3), expr) ≠ Union(LeftJoin(P1, P2, expr), LeftJoin(P1, P3, expr))
```

This limits algebraic optimization opportunities.

### Query Translation

A SPARQL query is translated to algebra in this order:

1. **Parse** query text to syntax tree
2. **Translate** patterns to algebra
3. **Apply** solution modifiers (GROUP BY, ORDER BY, etc.)
4. **Apply** projection (SELECT variables)
5. **Apply** slice (LIMIT/OFFSET)

**Example**:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>

SELECT ?name ?email
WHERE {
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:email ?email }
}
ORDER BY ?name
LIMIT 10
```

**Translates to**:

```
Slice(
  OrderBy(
    Project(
      LeftJoin(
        BGP(?person foaf:name ?name),
        BGP(?person foaf:email ?email),
        true
      ),
      {?name, ?email}
    ),
    [ASC(?name)]
  ),
  0,
  10
)
```

---

## Query Result Formats

SPARQL supports multiple serialization formats for query results.

### 1. JSON Format

For SELECT and ASK queries.

#### SELECT Results

```json
{
  "head": {
    "vars": ["name", "email"]
  },
  "results": {
    "bindings": [
      {
        "name": {
          "type": "literal",
          "value": "Alice"
        },
        "email": {
          "type": "literal",
          "value": "alice@example.com"
        }
      },
      {
        "name": {
          "type": "literal",
          "value": "Bob"
        }
      }
    ]
  }
}
```

#### RDF Term Types

**IRI:**
```json
{
  "type": "uri",
  "value": "http://example.org/alice"
}
```

**Literal:**
```json
{
  "type": "literal",
  "value": "Alice"
}
```

**Language-tagged literal:**
```json
{
  "type": "literal",
  "value": "Alice",
  "xml:lang": "en"
}
```

**Typed literal:**
```json
{
  "type": "literal",
  "value": "42",
  "datatype": "http://www.w3.org/2001/XMLSchema#integer"
}
```

**Blank node:**
```json
{
  "type": "bnode",
  "value": "b0"
}
```

#### ASK Results

```json
{
  "head": {},
  "boolean": true
}
```

### 2. XML Format

#### SELECT Results

```xml
<?xml version="1.0"?>
<sparql xmlns="http://www.w3.org/2005/sparql-results#">
  <head>
    <variable name="name"/>
    <variable name="email"/>
  </head>
  <results>
    <result>
      <binding name="name">
        <literal>Alice</literal>
      </binding>
      <binding name="email">
        <literal>alice@example.com</literal>
      </binding>
    </result>
    <result>
      <binding name="name">
        <literal>Bob</literal>
      </binding>
    </result>
  </results>
</sparql>
```

#### RDF Term Elements

**IRI:**
```xml
<uri>http://example.org/alice</uri>
```

**Literal:**
```xml
<literal>Alice</literal>
```

**Language-tagged:**
```xml
<literal xml:lang="en">Alice</literal>
```

**Typed literal:**
```xml
<literal datatype="http://www.w3.org/2001/XMLSchema#integer">42</literal>
```

**Blank node:**
```xml
<bnode>b0</bnode>
```

#### ASK Results

```xml
<?xml version="1.0"?>
<sparql xmlns="http://www.w3.org/2005/sparql-results#">
  <head/>
  <boolean>true</boolean>
</sparql>
```

### 3. CSV Format

Simplified format without type information:

```csv
name,email
Alice,alice@example.com
Bob,
```

**Characteristics:**
- **Lossy**: No type information (IRI vs literal vs blank node)
- **Simple**: Easy to consume in applications
- **Header row**: Variable names
- **Empty cells**: Unbound variables

### 4. TSV Format

Tab-separated with type encoding:

```tsv
?name	?email
"Alice"	"alice@example.com"
"Bob"
```

**RDF Term Encoding:**
- **IRI**: `<http://example.org/resource>`
- **Literal**: `"value"`
- **Language-tagged**: `"value"@en`
- **Typed literal**: `"value"^^<datatype>`
- **Blank node**: `_:label`

**Characteristics:**
- **Lossless**: Preserves all type information
- **SPARQL/Turtle syntax**: Uses standard RDF term syntax
- **Simple parsing**: Split on tabs

---

## Implementation Considerations

### For PostgreSQL Integration

#### 1. Data Model Mapping

**RDF Triples → PostgreSQL Tables:**

```sql
-- Triple store table
CREATE TABLE rdf_triples (
  id BIGSERIAL PRIMARY KEY,
  subject TEXT NOT NULL,
  subject_type VARCHAR(10) NOT NULL,  -- 'iri', 'bnode'
  predicate TEXT NOT NULL,
  object TEXT NOT NULL,
  object_type VARCHAR(10) NOT NULL,   -- 'iri', 'literal', 'bnode'
  object_datatype TEXT,
  object_language VARCHAR(10),
  graph TEXT
);

-- Indexes for query performance
CREATE INDEX idx_triples_spo ON rdf_triples(subject, predicate, object);
CREATE INDEX idx_triples_pos ON rdf_triples(predicate, object, subject);
CREATE INDEX idx_triples_osp ON rdf_triples(object, subject, predicate);
CREATE INDEX idx_triples_graph ON rdf_triples(graph);
```

#### 2. Query Translation

**SPARQL → SQL Translation:**

SPARQL BGP:
```sparql
?person foaf:name ?name .
?person foaf:age ?age .
```

SQL Translation:
```sql
SELECT t1.subject AS person, t1.object AS name, t2.object AS age
FROM rdf_triples t1
JOIN rdf_triples t2 ON t1.subject = t2.subject
WHERE t1.predicate = 'http://xmlns.com/foaf/0.1/name'
  AND t2.predicate = 'http://xmlns.com/foaf/0.1/age';
```

#### 3. OPTIONAL → LEFT JOIN

SPARQL:
```sparql
?person foaf:name ?name .
OPTIONAL { ?person foaf:email ?email }
```

SQL:
```sql
SELECT t1.subject AS person, t1.object AS name, t2.object AS email
FROM rdf_triples t1
LEFT JOIN rdf_triples t2 ON t1.subject = t2.subject
  AND t2.predicate = 'http://xmlns.com/foaf/0.1/email'
WHERE t1.predicate = 'http://xmlns.com/foaf/0.1/name';
```

#### 4. UNION → UNION ALL

SPARQL:
```sparql
{ ?person foaf:name ?name }
UNION
{ ?person rdfs:label ?name }
```

SQL:
```sql
SELECT subject AS person, object AS name
FROM rdf_triples
WHERE predicate = 'http://xmlns.com/foaf/0.1/name'
UNION ALL
SELECT subject AS person, object AS name
FROM rdf_triples
WHERE predicate = 'http://www.w3.org/2000/01/rdf-schema#label';
```

#### 5. FILTER → WHERE

SPARQL:
```sparql
?person foaf:age ?age .
FILTER(?age >= 18)
```

SQL:
```sql
SELECT subject AS person, object AS age
FROM rdf_triples
WHERE predicate = 'http://xmlns.com/foaf/0.1/age'
  AND object_type = 'literal'
  AND object_datatype = 'http://www.w3.org/2001/XMLSchema#integer'
  AND CAST(object AS INTEGER) >= 18;
```

#### 6. Property Paths

Property paths require recursive queries:

SPARQL:
```sparql
?person foaf:knows+ ?ancestor .
```

SQL (PostgreSQL CTE):
```sql
WITH RECURSIVE transitive AS (
  -- Base case
  SELECT subject, object
  FROM rdf_triples
  WHERE predicate = 'http://xmlns.com/foaf/0.1/knows'

  UNION

  -- Recursive case
  SELECT t.subject, r.object
  FROM rdf_triples t
  JOIN transitive r ON t.object = r.subject
  WHERE t.predicate = 'http://xmlns.com/foaf/0.1/knows'
)
SELECT * FROM transitive;
```

#### 7. Aggregates

SPARQL aggregates map to SQL aggregates:

SPARQL:
```sparql
SELECT ?company (COUNT(?employee) AS ?count)
WHERE { ?employee foaf:workplaceHomepage ?company }
GROUP BY ?company
```

SQL:
```sql
SELECT object AS company, COUNT(*) AS count
FROM rdf_triples
WHERE predicate = 'http://xmlns.com/foaf/0.1/workplaceHomepage'
GROUP BY object;
```

#### 8. Optimization Strategies

**Statistics-based query planning:**
- Collect statistics on predicate frequencies
- Estimate selectivity of triple patterns
- Order joins by selectivity

**Materialized views:**
- Pre-compute common property paths
- Cache frequently accessed subgraphs

**Indexes:**
- SPO, POS, OSP indexes (covering all access patterns)
- Partial indexes for specific predicates
- GiST/GIN indexes for full-text search

**Caching:**
- Query result cache
- Parsed query cache
- Compiled SQL cache

#### 9. PostgreSQL Extensions

Leverage existing PostgreSQL features:
- **JSONB**: Store complex objects
- **Full-text search**: Text matching in literals
- **GiST indexes**: Spatial/hierarchical data
- **CTEs**: Recursive queries for property paths
- **Window functions**: Advanced analytics
- **Parallel query**: Scale to large datasets

#### 10. Integration with RuVector

Combine SPARQL with vector operations:

```sparql
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX rv: <http://ruvector.org/functions/>

SELECT ?name ?similarity
WHERE {
  ?person foaf:name ?name .
  ?person rv:embedding ?embedding .

  # Use RuVector distance function
  BIND(rv:cosine_similarity(?embedding, $query_vector) AS ?similarity)
  FILTER(?similarity > 0.8)
}
ORDER BY DESC(?similarity)
LIMIT 10
```

Implementation:
```sql
SELECT t1.object AS name,
       ruvector_cosine_similarity(
         t2.object::ruvector,
         $1::ruvector
       ) AS similarity
FROM rdf_triples t1
JOIN rdf_triples t2 ON t1.subject = t2.subject
WHERE t1.predicate = 'http://xmlns.com/foaf/0.1/name'
  AND t2.predicate = 'http://ruvector.org/properties/embedding'
  AND ruvector_cosine_similarity(t2.object::ruvector, $1::ruvector) > 0.8
ORDER BY similarity DESC
LIMIT 10;
```

### Performance Considerations

1. **Index Strategy**: SPO, POS, OSP covering all join orders
2. **Query Optimization**: Statistics-based join reordering
3. **Caching**: Parsed queries and compiled SQL
4. **Parallelization**: Leverage PostgreSQL parallel query
5. **Partitioning**: By graph, predicate, or subject
6. **Connection Pooling**: Reuse database connections
7. **Prepared Statements**: Reduce parsing overhead

### Standards Compliance

Implement according to:
- SPARQL 1.1 Query Language (W3C Recommendation)
- SPARQL 1.1 Update (W3C Recommendation)
- SPARQL 1.1 Protocol (HTTP bindings)
- SPARQL 1.1 Results JSON/XML/CSV/TSV Formats

Consider SPARQL 1.2 draft features:
- Enhanced property paths
- New functions
- Improved federation

---

## References

### W3C Specifications

1. [SPARQL 1.1 Query Language](https://www.w3.org/TR/sparql11-query/) - W3C Recommendation, March 2013
2. [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/) - W3C Recommendation, March 2013
3. [SPARQL 1.1 Protocol](https://www.w3.org/TR/sparql11-protocol/) - W3C Recommendation, March 2013
4. [SPARQL 1.1 Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) - W3C Recommendation, March 2013
5. [SPARQL 1.1 Results CSV/TSV Formats](https://www.w3.org/TR/sparql11-results-csv-tsv/) - W3C Recommendation, March 2013
6. [SPARQL 1.1 Property Paths](https://www.w3.org/TR/sparql11-property-paths/) - W3C Recommendation, March 2013
7. [SPARQL Query Language for RDF (1.0)](https://www.w3.org/TR/rdf-sparql-query/) - W3C Recommendation, January 2008

### Draft Specifications

8. [SPARQL 1.2 Overview](https://w3c.github.io/sparql-concepts/spec/) - W3C Working Draft
9. [SPARQL 1.2 Query Language](https://w3c.github.io/sparql-query/spec/) - W3C Working Draft

### Formal Semantics

10. [SPARQL Algebra](https://www.w3.org/2001/sw/DataAccess/rq23/rq24-algebra.html) - W3C Draft
11. [ARQ's SPARQL Algebra](https://www.w3.org/2011/09/SparqlAlgebra/ARQalgebra) - W3C Community

### Academic Resources

12. [Apache Jena SPARQL Tutorials](https://jena.apache.org/tutorials/sparql_basic_patterns.html)
13. [TU Dresden SPARQL Algebra Lectures](https://iccl.inf.tu-dresden.de/w/images/e/ee/FSWT-L16-SPARQL-Algebra.pdf)
14. [GraphDB SPARQL Functions Reference](https://graphdb.ontotext.com/documentation/10.2/sparql-functions-reference.html)

### Implementation Guides

15. [Virtuoso SPARQL Examples](https://vos.openlinksw.com/owiki/wiki/VOS/VirtTipsAndTricksSPARQL11Update)
16. [EasyRdf Property Paths](https://www.easyrdf.org/docs/property-paths)

---

## Appendix: Grammar Summary

### Query Structure

```ebnf
Query ::= Prologue ( SelectQuery | ConstructQuery | DescribeQuery | AskQuery ) ValuesClause
Prologue ::= ( BaseDecl | PrefixDecl )*
BaseDecl ::= 'BASE' IRIREF
PrefixDecl ::= 'PREFIX' PNAME_NS IRIREF

SelectQuery ::= SelectClause DatasetClause* WhereClause SolutionModifier
ConstructQuery ::= 'CONSTRUCT' ( ConstructTemplate DatasetClause* WhereClause | DatasetClause* 'WHERE' '{' TriplesTemplate? '}' ) SolutionModifier
DescribeQuery ::= 'DESCRIBE' ( VarOrIri+ | '*' ) DatasetClause* WhereClause? SolutionModifier
AskQuery ::= 'ASK' DatasetClause* WhereClause SolutionModifier

DatasetClause ::= 'FROM' ( DefaultGraphClause | NamedGraphClause )
WhereClause ::= 'WHERE'? GroupGraphPattern
SolutionModifier ::= GroupClause? HavingClause? OrderClause? LimitOffsetClauses?

GroupClause ::= 'GROUP' 'BY' GroupCondition+
HavingClause ::= 'HAVING' HavingCondition+
OrderClause ::= 'ORDER' 'BY' OrderCondition+
LimitOffsetClauses ::= LimitClause OffsetClause? | OffsetClause LimitClause?
LimitClause ::= 'LIMIT' INTEGER
OffsetClause ::= 'OFFSET' INTEGER
```

### Graph Patterns

```ebnf
GroupGraphPattern ::= '{' ( SubSelect | GroupGraphPatternSub ) '}'
GroupGraphPatternSub ::= TriplesBlock? ( GraphPatternNotTriples '.'? TriplesBlock? )*

GraphPatternNotTriples ::= GroupOrUnionGraphPattern | OptionalGraphPattern | MinusGraphPattern | GraphGraphPattern | ServiceGraphPattern | Filter | Bind | InlineData

OptionalGraphPattern ::= 'OPTIONAL' GroupGraphPattern
GraphGraphPattern ::= 'GRAPH' VarOrIri GroupGraphPattern
ServiceGraphPattern ::= 'SERVICE' 'SILENT'? VarOrIri GroupGraphPattern
Bind ::= 'BIND' '(' Expression 'AS' Var ')'
InlineData ::= 'VALUES' DataBlock
MinusGraphPattern ::= 'MINUS' GroupGraphPattern
GroupOrUnionGraphPattern ::= GroupGraphPattern ( 'UNION' GroupGraphPattern )*
Filter ::= 'FILTER' Constraint
```

### Triple Patterns

```ebnf
TriplesBlock ::= TriplesSameSubjectPath ( '.' TriplesBlock? )?
TriplesSameSubjectPath ::= VarOrTerm PropertyListPathNotEmpty | TriplesNodePath PropertyListPath
PropertyListPath ::= PropertyListPathNotEmpty?
PropertyListPathNotEmpty ::= ( VerbPath | VerbSimple ) ObjectListPath ( ';' ( ( VerbPath | VerbSimple ) ObjectList )? )*
VerbPath ::= Path
VerbSimple ::= Var
ObjectListPath ::= ObjectPath ( ',' ObjectPath )*
ObjectPath ::= GraphNodePath
Path ::= PathAlternative
PathAlternative ::= PathSequence ( '|' PathSequence )*
PathSequence ::= PathEltOrInverse ( '/' PathEltOrInverse )*
PathElt ::= PathPrimary PathMod?
PathEltOrInverse ::= PathElt | '^' PathElt
PathMod ::= '?' | '*' | '+'
PathPrimary ::= iri | 'a' | '!' PathNegatedPropertySet | '(' Path ')'
PathNegatedPropertySet ::= PathOneInPropertySet | '(' ( PathOneInPropertySet ( '|' PathOneInPropertySet )* )? ')'
PathOneInPropertySet ::= iri | 'a' | '^' ( iri | 'a' )
```

### Update Operations

```ebnf
Update ::= Prologue ( Update1 ( ';' Update )? )?
Update1 ::= Load | Clear | Drop | Add | Move | Copy | Create | InsertData | DeleteData | DeleteWhere | Modify

Load ::= 'LOAD' 'SILENT'? iri ( 'INTO' GraphRef )?
Clear ::= 'CLEAR' 'SILENT'? GraphRefAll
Drop ::= 'DROP' 'SILENT'? GraphRefAll
Create ::= 'CREATE' 'SILENT'? GraphRef
Add ::= 'ADD' 'SILENT'? GraphOrDefault 'TO' GraphOrDefault
Move ::= 'MOVE' 'SILENT'? GraphOrDefault 'TO' GraphOrDefault
Copy ::= 'COPY' 'SILENT'? GraphOrDefault 'TO' GraphOrDefault

InsertData ::= 'INSERT DATA' QuadData
DeleteData ::= 'DELETE DATA' QuadData
DeleteWhere ::= 'DELETE WHERE' QuadPattern
Modify ::= ( 'WITH' iri )? ( DeleteClause InsertClause? | InsertClause ) UsingClause* 'WHERE' GroupGraphPattern

DeleteClause ::= 'DELETE' QuadPattern
InsertClause ::= 'INSERT' QuadPattern
UsingClause ::= 'USING' ( iri | 'NAMED' iri )

GraphRefAll ::= GraphRef | 'DEFAULT' | 'NAMED' | 'ALL'
GraphRef ::= 'GRAPH' iri
```

---

**End of Specification Document**

This document provides comprehensive coverage of SPARQL 1.1 for implementing a query engine in PostgreSQL. For complete formal definitions and edge cases, refer to the official W3C specifications linked in the References section.
