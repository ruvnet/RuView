//! Rust Program Synthesis Domain
//!
//! Generates tasks that require synthesizing Rust programs from specifications.
//! Task types include:
//!
//! - **Transform**: Apply a function to data (map, filter, fold)
//! - **DataStructure**: Implement a data structure with specific operations
//! - **Algorithm**: Implement a named algorithm (sorting, searching, graph)
//! - **TypeLevel**: Express constraints via Rust's type system
//! - **Concurrency**: Safe concurrent data access patterns
//!
//! Solutions are evaluated on correctness (do test cases pass?),
//! efficiency (complexity class), and elegance (idiomatic Rust patterns).

use crate::domain::{Domain, DomainEmbedding, DomainId, Evaluation, Solution, Task};
use rand::Rng;
use serde::{Deserialize, Serialize};

/// Embedding dimension for Rust synthesis domain.
const EMBEDDING_DIM: usize = 64;

/// Categories of Rust synthesis tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RustTaskCategory {
    /// Transform data: map, filter, fold, scan.
    Transform,
    /// Implement a data structure with trait impls.
    DataStructure,
    /// Implement a named algorithm.
    Algorithm,
    /// Type-level programming: generics, trait bounds, associated types.
    TypeLevel,
    /// Concurrent programming: Arc, Mutex, channels, atomics.
    Concurrency,
}

/// Specification for a Rust synthesis task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustTaskSpec {
    /// Task category.
    pub category: RustTaskCategory,
    /// Function signature that must be implemented.
    pub signature: String,
    /// Natural language description of the required behavior.
    pub description: String,
    /// Test cases as (input_json, expected_output_json) pairs.
    pub test_cases: Vec<(String, String)>,
    /// Required traits the solution must implement.
    pub required_traits: Vec<String>,
    /// Banned patterns (e.g., "unsafe", "unwrap").
    pub banned_patterns: Vec<String>,
    /// Expected complexity class (e.g., "O(n log n)").
    pub expected_complexity: Option<String>,
}

/// Rust program synthesis domain.
pub struct RustSynthesisDomain {
    id: DomainId,
}

impl RustSynthesisDomain {
    /// Create a new Rust synthesis domain.
    pub fn new() -> Self {
        Self {
            id: DomainId("rust_synthesis".to_string()),
        }
    }

    /// Generate a transform task at the given difficulty.
    fn gen_transform(&self, difficulty: f32, rng: &mut impl Rng) -> RustTaskSpec {
        let (signature, description, tests, complexity) = if difficulty < 0.3 {
            // Easy: simple map
            let ops = ["double", "negate", "abs", "square"];
            let op = ops[rng.gen_range(0..ops.len())];
            (
                format!("fn {}(values: &[i64]) -> Vec<i64>", op),
                format!("Apply {} to each element in the slice.", op),
                match op {
                    "double" => vec![
                        ("[1, 2, 3]".into(), "[2, 4, 6]".into()),
                        ("[-1, 0, 5]".into(), "[-2, 0, 10]".into()),
                    ],
                    "negate" => vec![
                        ("[1, -2, 3]".into(), "[-1, 2, -3]".into()),
                        ("[0]".into(), "[0]".into()),
                    ],
                    "abs" => vec![
                        ("[-1, 2, -3]".into(), "[1, 2, 3]".into()),
                        ("[0, -0]".into(), "[0, 0]".into()),
                    ],
                    _ => vec![
                        ("[2, 3, 4]".into(), "[4, 9, 16]".into()),
                        ("[0, -1]".into(), "[0, 1]".into()),
                    ],
                },
                "O(n)",
            )
        } else if difficulty < 0.7 {
            // Medium: filter + fold combos
            (
                "fn sum_positives(values: &[i64]) -> i64".into(),
                "Sum all positive values in the slice.".into(),
                vec![
                    ("[1, -2, 3, -4, 5]".into(), "9".into()),
                    ("[-1, -2, -3]".into(), "0".into()),
                    ("[]".into(), "0".into()),
                ],
                "O(n)",
            )
        } else {
            // Hard: sliding window / scan
            (
                "fn max_subarray_sum(values: &[i64]) -> i64".into(),
                "Find the maximum sum contiguous subarray (Kadane's algorithm).".into(),
                vec![
                    ("[-2, 1, -3, 4, -1, 2, 1, -5, 4]".into(), "6".into()),
                    ("[-1, -2, -3]".into(), "-1".into()),
                    ("[5]".into(), "5".into()),
                ],
                "O(n)",
            )
        };

        RustTaskSpec {
            category: RustTaskCategory::Transform,
            signature,
            description,
            test_cases: tests,
            required_traits: Vec::new(),
            banned_patterns: vec!["unsafe".into()],
            expected_complexity: Some(complexity.into()),
        }
    }

    /// Generate a data structure task.
    fn gen_data_structure(&self, difficulty: f32, _rng: &mut impl Rng) -> RustTaskSpec {
        if difficulty < 0.4 {
            RustTaskSpec {
                category: RustTaskCategory::DataStructure,
                signature: "struct Stack<T>".into(),
                description: "Implement a generic stack with push, pop, peek, is_empty, len."
                    .into(),
                test_cases: vec![
                    ("push(1); push(2); pop()".into(), "Some(2)".into()),
                    ("is_empty()".into(), "true".into()),
                    ("push(1); len()".into(), "1".into()),
                ],
                required_traits: vec!["Default".into()],
                banned_patterns: vec!["unsafe".into()],
                expected_complexity: Some("O(1) per operation".into()),
            }
        } else if difficulty < 0.7 {
            RustTaskSpec {
                category: RustTaskCategory::DataStructure,
                signature: "struct MinHeap<T: Ord>".into(),
                description: "Implement a binary min-heap with insert, extract_min, peek_min."
                    .into(),
                test_cases: vec![
                    (
                        "insert(3); insert(1); insert(2); extract_min()".into(),
                        "Some(1)".into(),
                    ),
                    ("peek_min() on empty".into(), "None".into()),
                ],
                required_traits: vec!["Default".into()],
                banned_patterns: vec!["unsafe".into(), "BinaryHeap".into()],
                expected_complexity: Some("O(log n) insert/extract".into()),
            }
        } else {
            RustTaskSpec {
                category: RustTaskCategory::DataStructure,
                signature: "struct LRUCache<K: Hash + Eq, V>".into(),
                description: "Implement an LRU cache with get, put, and capacity eviction.".into(),
                test_cases: vec![
                    (
                        "cap=2; put(1,'a'); put(2,'b'); get(1); put(3,'c'); get(2)".into(),
                        "None".into(),
                    ),
                    (
                        "cap=1; put(1,'a'); put(2,'b'); get(1)".into(),
                        "None".into(),
                    ),
                ],
                required_traits: Vec::new(),
                banned_patterns: vec!["unsafe".into()],
                expected_complexity: Some("O(1) get/put".into()),
            }
        }
    }

    /// Generate an algorithm task.
    fn gen_algorithm(&self, difficulty: f32, _rng: &mut impl Rng) -> RustTaskSpec {
        if difficulty < 0.4 {
            RustTaskSpec {
                category: RustTaskCategory::Algorithm,
                signature: "fn binary_search(sorted: &[i64], target: i64) -> Option<usize>".into(),
                description: "Implement binary search on a sorted slice.".into(),
                test_cases: vec![
                    ("[1,3,5,7,9], 5".into(), "Some(2)".into()),
                    ("[1,3,5,7,9], 4".into(), "None".into()),
                    ("[], 1".into(), "None".into()),
                ],
                required_traits: Vec::new(),
                banned_patterns: vec!["unsafe".into()],
                expected_complexity: Some("O(log n)".into()),
            }
        } else if difficulty < 0.7 {
            RustTaskSpec {
                category: RustTaskCategory::Algorithm,
                signature: "fn merge_sort(values: &mut [i64])".into(),
                description: "Implement stable merge sort in-place.".into(),
                test_cases: vec![
                    ("[3,1,4,1,5,9,2,6]".into(), "[1,1,2,3,4,5,6,9]".into()),
                    ("[1]".into(), "[1]".into()),
                    ("[]".into(), "[]".into()),
                ],
                required_traits: Vec::new(),
                banned_patterns: vec!["unsafe".into(), ".sort".into()],
                expected_complexity: Some("O(n log n)".into()),
            }
        } else {
            RustTaskSpec {
                category: RustTaskCategory::Algorithm,
                signature: "fn shortest_path(adj: &[Vec<(usize, u64)>], src: usize, dst: usize) -> Option<u64>".into(),
                description: "Implement Dijkstra's shortest path on a weighted directed graph.".into(),
                test_cases: vec![
                    ("3 nodes, 0->1:2, 1->2:3, 0->2:10; src=0, dst=2".into(), "Some(5)".into()),
                    ("2 nodes, no edges; src=0, dst=1".into(), "None".into()),
                ],
                required_traits: Vec::new(),
                banned_patterns: vec!["unsafe".into()],
                expected_complexity: Some("O((V + E) log V)".into()),
            }
        }
    }

    /// Extract structural features from a Rust solution for embedding.
    fn extract_features(&self, solution: &Solution) -> Vec<f32> {
        let code = &solution.content;
        let mut features = vec![0.0f32; EMBEDDING_DIM];

        // Feature 0-7: Control flow complexity
        features[0] = code.matches("if ").count() as f32 / 10.0;
        features[1] = code.matches("for ").count() as f32 / 5.0;
        features[2] = code.matches("while ").count() as f32 / 5.0;
        features[3] = code.matches("match ").count() as f32 / 5.0;
        features[4] = code.matches("loop ").count() as f32 / 3.0;
        features[5] = code.matches("return ").count() as f32 / 5.0;
        features[6] = code.matches("break").count() as f32 / 3.0;
        features[7] = code.matches("continue").count() as f32 / 3.0;

        // Feature 8-15: Type system usage
        features[8] = code.matches("impl ").count() as f32 / 5.0;
        features[9] = code.matches("trait ").count() as f32 / 3.0;
        features[10] = code.matches("struct ").count() as f32 / 3.0;
        features[11] = code.matches("enum ").count() as f32 / 3.0;
        features[12] = code.matches("where ").count() as f32 / 3.0;
        features[13] = code.matches("dyn ").count() as f32 / 3.0;
        features[14] = code.matches("Box<").count() as f32 / 3.0;
        features[15] = code.matches("Rc<").count() as f32 / 3.0;

        // Feature 16-23: Functional patterns
        features[16] = code.matches(".map(").count() as f32 / 5.0;
        features[17] = code.matches(".filter(").count() as f32 / 5.0;
        features[18] = code.matches(".fold(").count() as f32 / 3.0;
        features[19] = code.matches(".collect()").count() as f32 / 3.0;
        features[20] = code.matches(".iter()").count() as f32 / 5.0;
        features[21] = code.matches("|").count() as f32 / 10.0; // closures
        features[22] = code.matches("Some(").count() as f32 / 5.0;
        features[23] = code.matches("None").count() as f32 / 5.0;

        // Feature 24-31: Memory/ownership patterns
        features[24] = code.matches("&mut ").count() as f32 / 5.0;
        features[25] = code.matches("&self").count() as f32 / 5.0;
        features[26] = code.matches("mut ").count() as f32 / 10.0;
        features[27] = code.matches(".clone()").count() as f32 / 5.0;
        features[28] = code.matches("Vec<").count() as f32 / 5.0;
        features[29] = code.matches("HashMap").count() as f32 / 3.0;
        features[30] = code.matches("String").count() as f32 / 5.0;
        features[31] = code.matches("Result<").count() as f32 / 3.0;

        // Feature 32-39: Concurrency patterns
        features[32] = code.matches("Arc<").count() as f32 / 3.0;
        features[33] = code.matches("Mutex<").count() as f32 / 3.0;
        features[34] = code.matches("RwLock").count() as f32 / 3.0;
        features[35] = code.matches("async ").count() as f32 / 3.0;
        features[36] = code.matches("await").count() as f32 / 5.0;
        features[37] = code.matches("spawn").count() as f32 / 3.0;
        features[38] = code.matches("channel").count() as f32 / 3.0;
        features[39] = code.matches("Atomic").count() as f32 / 3.0;

        // Feature 40-47: Code structure metrics
        let lines: Vec<&str> = code.lines().collect();
        features[40] = (lines.len() as f32) / 100.0;
        features[41] = lines.iter().filter(|l| l.trim().is_empty()).count() as f32
            / (lines.len().max(1) as f32);
        features[42] = code.matches("fn ").count() as f32 / 10.0;
        features[43] = code.matches("pub ").count() as f32 / 10.0;
        features[44] = code.matches("mod ").count() as f32 / 5.0;
        features[45] = code.matches("use ").count() as f32 / 10.0;
        features[46] = code.matches("#[").count() as f32 / 5.0; // attributes
        features[47] = code.matches("///").count() as f32 / 10.0; // doc comments

        // Feature 48-55: Error handling patterns
        features[48] = code.matches("unwrap()").count() as f32 / 5.0;
        features[49] = code.matches("expect(").count() as f32 / 5.0;
        features[50] = code.matches("?;").count() as f32 / 5.0; // error propagation
        features[51] = code.matches("Err(").count() as f32 / 5.0;
        features[52] = code.matches("Ok(").count() as f32 / 5.0;
        features[53] = code.matches("panic!").count() as f32 / 3.0;
        features[54] = code.matches("assert").count() as f32 / 5.0;
        features[55] = code.matches("debug_assert").count() as f32 / 3.0;

        // Feature 56-63: Algorithm indicators
        features[56] = code.matches("sort").count() as f32 / 3.0;
        features[57] = code.matches("binary_search").count() as f32 / 2.0;
        features[58] = code.matches("push").count() as f32 / 5.0;
        features[59] = code.matches("pop").count() as f32 / 5.0;
        features[60] = code.matches("swap").count() as f32 / 5.0;
        features[61] = code.matches("len()").count() as f32 / 5.0;
        features[62] = code.matches("is_empty").count() as f32 / 3.0;
        features[63] = code.matches("contains").count() as f32 / 3.0;

        // Normalize to unit length
        let norm: f32 = features.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-10 {
            for f in &mut features {
                *f /= norm;
            }
        }

        features
    }

    /// Score a Rust solution based on pattern matching heuristics.
    fn score_solution(&self, spec: &RustTaskSpec, solution: &Solution) -> Evaluation {
        let code = &solution.content;
        let mut correctness = 0.0f32;
        let mut efficiency = 0.5f32;
        let mut elegance = 0.5f32;
        let mut notes = Vec::new();

        // Check for banned patterns
        let mut banned_found = false;
        for pattern in &spec.banned_patterns {
            if code.contains(pattern.as_str()) {
                notes.push(format!("Banned pattern found: {}", pattern));
                banned_found = true;
            }
        }

        if banned_found {
            elegance *= 0.5;
        }

        // Check that the solution contains the expected signature
        let sig_name = spec
            .signature
            .split('(')
            .next()
            .unwrap_or("")
            .split_whitespace()
            .last()
            .unwrap_or("");

        if code.contains(sig_name) {
            correctness += 0.3;
        } else {
            notes.push(format!("Missing expected identifier: {}", sig_name));
        }

        // Check for fn definition
        if code.contains("fn ") {
            correctness += 0.2;
        }

        // Check for test case coverage hints
        let test_coverage = spec
            .test_cases
            .iter()
            .filter(|(input, _)| {
                // Heuristic: solution likely handles the input pattern
                let key_tokens: Vec<&str> = input.split(|c: char| !c.is_alphanumeric()).collect();
                key_tokens.iter().any(|t| !t.is_empty() && code.contains(t))
            })
            .count() as f32
            / spec.test_cases.len().max(1) as f32;
        correctness += test_coverage * 0.5;
        correctness = correctness.clamp(0.0, 1.0);

        // Efficiency: penalize obviously quadratic patterns
        let nested_loops = code.matches("for ").count() > 1 && code.matches("for ").count() > 2;
        if nested_loops {
            if let Some(ref expected) = spec.expected_complexity {
                if expected.contains("O(n)") || expected.contains("O(log") {
                    efficiency *= 0.5;
                    notes.push("Possible O(n^2) when O(n) or O(log n) expected".into());
                }
            }
        }

        // Elegance: favor idiomatic Rust
        let iterator_usage = code.matches(".iter()").count()
            + code.matches(".map(").count()
            + code.matches(".filter(").count()
            + code.matches(".fold(").count();
        if iterator_usage > 0 {
            elegance += 0.2;
        }

        // Penalize excessive unwrap
        let unwrap_count = code.matches("unwrap()").count();
        if unwrap_count > 3 {
            elegance -= 0.2;
            notes.push("Excessive unwrap() usage".into());
        }

        // Proper error handling bonus
        if code.contains("Result<") || code.contains("?;") {
            elegance += 0.1;
        }

        elegance = elegance.clamp(0.0, 1.0);

        // Constraint results
        let constraint_results = spec
            .banned_patterns
            .iter()
            .map(|p| !code.contains(p.as_str()))
            .collect();

        let score = 0.6 * correctness + 0.25 * efficiency + 0.15 * elegance;

        Evaluation {
            score: score.clamp(0.0, 1.0),
            correctness,
            efficiency,
            elegance,
            constraint_results,
            notes,
        }
    }
}

impl Default for RustSynthesisDomain {
    fn default() -> Self {
        Self::new()
    }
}

impl Domain for RustSynthesisDomain {
    fn id(&self) -> &DomainId {
        &self.id
    }

    fn name(&self) -> &str {
        "Rust Program Synthesis"
    }

    fn generate_tasks(&self, count: usize, difficulty: f32) -> Vec<Task> {
        let mut rng = rand::thread_rng();
        let difficulty = difficulty.clamp(0.0, 1.0);

        (0..count)
            .map(|i| {
                let category_roll: f32 = rng.gen();
                let spec = if category_roll < 0.4 {
                    self.gen_transform(difficulty, &mut rng)
                } else if category_roll < 0.7 {
                    self.gen_data_structure(difficulty, &mut rng)
                } else {
                    self.gen_algorithm(difficulty, &mut rng)
                };

                Task {
                    id: format!("rust_synth_{}_d{:.0}", i, difficulty * 100.0),
                    domain_id: self.id.clone(),
                    difficulty,
                    spec: serde_json::to_value(&spec).unwrap_or_default(),
                    constraints: spec.banned_patterns.clone(),
                }
            })
            .collect()
    }

    fn evaluate(&self, task: &Task, solution: &Solution) -> Evaluation {
        let spec: RustTaskSpec = match serde_json::from_value(task.spec.clone()) {
            Ok(s) => s,
            Err(e) => return Evaluation::zero(vec![format!("Invalid task spec: {}", e)]),
        };
        self.score_solution(&spec, solution)
    }

    fn embed(&self, solution: &Solution) -> DomainEmbedding {
        let features = self.extract_features(solution);
        DomainEmbedding::new(features, self.id.clone())
    }

    fn embedding_dim(&self) -> usize {
        EMBEDDING_DIM
    }

    fn reference_solution(&self, task: &Task) -> Option<Solution> {
        let spec: RustTaskSpec = serde_json::from_value(task.spec.clone()).ok()?;

        let content = match spec.category {
            RustTaskCategory::Transform => {
                if spec.signature.contains("sum_positives") {
                    "fn sum_positives(values: &[i64]) -> i64 {\n    values.iter().filter(|&&x| x > 0).sum()\n}".to_string()
                } else if spec.signature.contains("max_subarray_sum") {
                    "fn max_subarray_sum(values: &[i64]) -> i64 {\n    let mut max_so_far = values[0];\n    let mut max_ending = values[0];\n    for &v in &values[1..] {\n        max_ending = v.max(max_ending + v);\n        max_so_far = max_so_far.max(max_ending);\n    }\n    max_so_far\n}".to_string()
                } else {
                    format!(
                        "{} {{\n    values.iter().map(|&x| x /* TODO */).collect()\n}}",
                        spec.signature
                    )
                }
            }
            _ => return None,
        };

        Some(Solution {
            task_id: task.id.clone(),
            content,
            data: serde_json::Value::Null,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_tasks() {
        let domain = RustSynthesisDomain::new();
        let tasks = domain.generate_tasks(5, 0.5);
        assert_eq!(tasks.len(), 5);
        for task in &tasks {
            assert_eq!(task.domain_id, domain.id);
            assert!((task.difficulty - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn test_evaluate_good_solution() {
        let domain = RustSynthesisDomain::new();
        let tasks = domain.generate_tasks(1, 0.0);
        let task = &tasks[0];

        let solution = Solution {
            task_id: task.id.clone(),
            content: "fn double(values: &[i64]) -> Vec<i64> {\n    values.iter().map(|&x| x * 2).collect()\n}".to_string(),
            data: serde_json::Value::Null,
        };

        let eval = domain.evaluate(task, &solution);
        assert!(eval.score > 0.0);
    }

    #[test]
    fn test_embed_produces_correct_dim() {
        let domain = RustSynthesisDomain::new();
        let solution = Solution {
            task_id: "test".into(),
            content: "fn foo() { let x = 1; }".into(),
            data: serde_json::Value::Null,
        };
        let embedding = domain.embed(&solution);
        assert_eq!(embedding.dim, EMBEDDING_DIM);
        assert_eq!(embedding.vector.len(), EMBEDDING_DIM);
    }

    #[test]
    fn test_embedding_normalized() {
        let domain = RustSynthesisDomain::new();
        let solution = Solution {
            task_id: "test".into(),
            content: "fn foo() { for i in 0..10 { if i > 5 { println!(\"{}\", i); } } }".into(),
            data: serde_json::Value::Null,
        };
        let embedding = domain.embed(&solution);
        let norm: f32 = embedding.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_difficulty_range() {
        let domain = RustSynthesisDomain::new();
        // Easy tasks
        let easy = domain.generate_tasks(3, 0.1);
        for t in &easy {
            let spec: RustTaskSpec = serde_json::from_value(t.spec.clone()).unwrap();
            assert!(!spec.signature.is_empty());
        }
        // Hard tasks
        let hard = domain.generate_tasks(3, 0.9);
        for t in &hard {
            let spec: RustTaskSpec = serde_json::from_value(t.spec.clone()).unwrap();
            assert!(!spec.signature.is_empty());
        }
    }
}
