# Test Fixtures

This directory contains sample datasets and expected results for testing the RuVector graph database.

## Datasets

### movie_database.json
A small movie database inspired by Neo4j's example dataset:
- 3 actors (Keanu Reeves, Carrie-Anne Moss, Laurence Fishburne)
- 1 movie (The Matrix)
- 3 ACTED_IN relationships with role properties

### social_network.json
A simple social network for testing graph algorithms:
- 5 people
- 6 KNOWS relationships forming a small network

## Expected Results

### expected_results.json
Contains test cases with:
- Query text (Cypher)
- Which dataset to use
- Expected query results

Use these to validate that query execution returns correct results.

## Usage in Tests

```rust
use std::fs;
use serde_json::Value;

#[test]
fn test_with_fixture() {
    let fixture = fs::read_to_string("tests/fixtures/movie_database.json").unwrap();
    let data: Value = serde_json::from_str(&fixture).unwrap();

    // Load data into graph
    // Execute queries
    // Validate against expected results
}
```

## Adding New Fixtures

When adding new fixtures:
1. Follow the JSON schema used in existing files
2. Add corresponding expected results
3. Document the dataset purpose
4. Keep datasets small and focused on specific test scenarios
