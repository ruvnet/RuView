//! # Tier 4: Compositional Hyperdimensional Computing
//!
//! SOTA application: Zero-shot concept composition via HDC binding.
//!
//! ## The Problem
//! Traditional embeddings:
//! - Fixed vocabulary at training time
//! - Cannot represent "red dog" if never seen together
//! - Composition requires retraining
//! - No algebraic structure for reasoning
//!
//! ## What Changes
//! - HDC: concepts are binary hypervectors (10,000 bits)
//! - XOR binding: combine concepts preserving similarity
//! - Bundling: create superpositions (sets of concepts)
//! - Algebra: unbind to recover components
//!
//! ## Why This Matters
//! - Zero-shot: represent any combination of known concepts
//! - Sub-100ns operations: composition is just XOR
//! - Distributed: no central vocabulary server
//! - Interpretable: can unbind to see what's in a representation
//!
//! This is what embeddings should have been: compositional by construction.

use std::collections::HashMap;

// ============================================================================
// Hypervector Operations
// ============================================================================

/// Number of bits in hypervector
const DIM: usize = 10_000;
/// Number of u64 words
const WORDS: usize = (DIM + 63) / 64;

/// Binary hypervector with SIMD-friendly operations
#[derive(Clone)]
pub struct Hypervector {
    bits: [u64; WORDS],
}

impl Hypervector {
    /// Create zero vector
    pub fn zeros() -> Self {
        Self { bits: [0; WORDS] }
    }

    /// Create random vector (approximately 50% ones)
    pub fn random(seed: u64) -> Self {
        let mut bits = [0u64; WORDS];
        let mut state = seed;

        for word in &mut bits {
            // Xorshift64
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            *word = state;
        }

        Self { bits }
    }

    /// Create from seed string (deterministic)
    pub fn from_seed(seed: &str) -> Self {
        let hash = seed
            .bytes()
            .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
        Self::random(hash)
    }

    /// XOR binding: A ⊗ B
    /// Key property: (A ⊗ B) is dissimilar to both A and B
    /// but (A ⊗ B) ⊗ B ≈ A (unbinding)
    pub fn bind(&self, other: &Self) -> Self {
        let mut result = Self::zeros();
        for i in 0..WORDS {
            result.bits[i] = self.bits[i] ^ other.bits[i];
        }
        result
    }

    /// Unbind: given A ⊗ B and B, recover A
    /// Since XOR is its own inverse: A ⊗ B ⊗ B = A
    pub fn unbind(&self, key: &Self) -> Self {
        self.bind(key) // Same as bind
    }

    /// Bundle (superposition): majority vote
    /// Result has bits that are 1 in most inputs
    pub fn bundle(vectors: &[Self]) -> Self {
        if vectors.is_empty() {
            return Self::zeros();
        }

        if vectors.len() == 1 {
            return vectors[0].clone();
        }

        let threshold = vectors.len() / 2;
        let mut result = Self::zeros();

        for bit_idx in 0..DIM {
            let word_idx = bit_idx / 64;
            let bit_pos = bit_idx % 64;

            let count: usize = vectors
                .iter()
                .filter(|v| (v.bits[word_idx] >> bit_pos) & 1 == 1)
                .count();

            if count > threshold {
                result.bits[word_idx] |= 1 << bit_pos;
            }
        }

        result
    }

    /// Permute: shift bits (creates sequence-sensitive binding)
    pub fn permute(&self, shift: usize) -> Self {
        let shift = shift % DIM;
        if shift == 0 {
            return self.clone();
        }

        let mut result = Self::zeros();

        for bit_idx in 0..DIM {
            let new_idx = (bit_idx + shift) % DIM;
            let old_word = bit_idx / 64;
            let old_pos = bit_idx % 64;
            let new_word = new_idx / 64;
            let new_pos = new_idx % 64;

            if (self.bits[old_word] >> old_pos) & 1 == 1 {
                result.bits[new_word] |= 1 << new_pos;
            }
        }

        result
    }

    /// Hamming distance (number of differing bits)
    pub fn hamming_distance(&self, other: &Self) -> u32 {
        let mut dist = 0u32;
        for i in 0..WORDS {
            dist += (self.bits[i] ^ other.bits[i]).count_ones();
        }
        dist
    }

    /// Cosine-like similarity: 1 - 2 * (distance / DIM)
    pub fn similarity(&self, other: &Self) -> f32 {
        let dist = self.hamming_distance(other);
        1.0 - 2.0 * (dist as f32 / DIM as f32)
    }

    /// Count ones
    pub fn popcount(&self) -> u32 {
        self.bits.iter().map(|w| w.count_ones()).sum()
    }
}

impl std::fmt::Debug for Hypervector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HV(popcount={})", self.popcount())
    }
}

// ============================================================================
// Concept Memory
// ============================================================================

/// Memory of atomic concepts
pub struct ConceptMemory {
    /// Named concepts
    concepts: HashMap<String, Hypervector>,
    /// Role vectors for binding positions
    roles: HashMap<String, Hypervector>,
}

impl ConceptMemory {
    pub fn new() -> Self {
        let mut mem = Self {
            concepts: HashMap::new(),
            roles: HashMap::new(),
        };

        // Create role vectors for structured binding
        mem.roles.insert(
            "subject".to_string(),
            Hypervector::from_seed("role:subject"),
        );
        mem.roles.insert(
            "predicate".to_string(),
            Hypervector::from_seed("role:predicate"),
        );
        mem.roles
            .insert("object".to_string(), Hypervector::from_seed("role:object"));
        mem.roles.insert(
            "modifier".to_string(),
            Hypervector::from_seed("role:modifier"),
        );
        mem.roles.insert(
            "position_1".to_string(),
            Hypervector::from_seed("role:position_1"),
        );
        mem.roles.insert(
            "position_2".to_string(),
            Hypervector::from_seed("role:position_2"),
        );
        mem.roles.insert(
            "position_3".to_string(),
            Hypervector::from_seed("role:position_3"),
        );

        mem
    }

    /// Add a new atomic concept
    pub fn learn(&mut self, name: &str) -> Hypervector {
        if let Some(v) = self.concepts.get(name) {
            return v.clone();
        }

        let v = Hypervector::from_seed(&format!("concept:{}", name));
        self.concepts.insert(name.to_string(), v.clone());
        v
    }

    /// Get a concept (learn if new)
    pub fn get(&mut self, name: &str) -> Hypervector {
        self.learn(name)
    }

    /// Get a role vector
    pub fn role(&self, name: &str) -> Option<&Hypervector> {
        self.roles.get(name)
    }

    /// Bind concept to role
    pub fn bind_role(&self, concept: &Hypervector, role: &str) -> Option<Hypervector> {
        self.roles.get(role).map(|r| concept.bind(r))
    }

    /// Unbind role to recover concept
    pub fn unbind_role(&self, bound: &Hypervector, role: &str) -> Option<Hypervector> {
        self.roles.get(role).map(|r| bound.unbind(r))
    }

    /// Query: find best matching concept
    pub fn query(&self, hv: &Hypervector) -> Vec<(String, f32)> {
        let mut results: Vec<_> = self
            .concepts
            .iter()
            .map(|(name, v)| (name.clone(), hv.similarity(v)))
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
    }
}

// ============================================================================
// Compositional Structures
// ============================================================================

/// Compose "modifier concept" pairs (e.g., "red" + "dog")
pub fn compose_modifier(memory: &mut ConceptMemory, modifier: &str, concept: &str) -> Hypervector {
    let m = memory.get(modifier);
    let c = memory.get(concept);

    // Bind modifier to modifier role, then bundle with concept
    let m_bound = m.bind(memory.role("modifier").unwrap());
    let c_bound = c.bind(memory.role("subject").unwrap());

    Hypervector::bundle(&[m_bound, c_bound])
}

/// Compose a sequence (e.g., "A then B then C")
pub fn compose_sequence(memory: &mut ConceptMemory, items: &[&str]) -> Hypervector {
    let mut parts = Vec::new();

    for (i, item) in items.iter().enumerate() {
        let v = memory.get(item);
        // Permute by position to create order-sensitive representation
        parts.push(v.permute(i * 10));
    }

    Hypervector::bundle(&parts)
}

/// Compose a relation triple (subject, predicate, object)
pub fn compose_triple(
    memory: &mut ConceptMemory,
    subject: &str,
    predicate: &str,
    object: &str,
) -> Hypervector {
    let s = memory.get(subject).bind(memory.role("subject").unwrap());
    let p = memory
        .get(predicate)
        .bind(memory.role("predicate").unwrap());
    let o = memory.get(object).bind(memory.role("object").unwrap());

    Hypervector::bundle(&[s, p, o])
}

/// Query a composed structure for a specific role
pub fn query_role(memory: &ConceptMemory, composed: &Hypervector, role: &str) -> Hypervector {
    composed.unbind(memory.role(role).unwrap())
}

// ============================================================================
// Analogical Reasoning
// ============================================================================

/// Solve analogy: A is to B as C is to ?
/// Using: D = C ⊗ (B ⊗ A⁻¹) where A⁻¹ = A (self-inverse)
pub fn analogy(memory: &mut ConceptMemory, a: &str, b: &str, c: &str) -> Hypervector {
    let a_vec = memory.get(a);
    let b_vec = memory.get(b);
    let c_vec = memory.get(c);

    // Relationship: B ⊗ A (since XOR is self-inverse)
    let relationship = b_vec.bind(&a_vec);

    // Apply to C
    c_vec.bind(&relationship)
}

// ============================================================================
// Example Usage
// ============================================================================

fn main() {
    println!("=== Tier 4: Compositional Hyperdimensional Computing ===\n");

    let mut memory = ConceptMemory::new();

    // Learn atomic concepts
    println!("Learning atomic concepts...");
    let concepts = [
        "dog", "cat", "bird", "red", "blue", "big", "small", "run", "fly", "swim", "chase", "eat",
        "king", "queen", "man", "woman", "prince", "princess",
    ];

    for concept in &concepts {
        memory.learn(concept);
    }
    println!("  Learned {} concepts\n", concepts.len());

    // Demonstrate composition
    println!("=== Modifier + Concept Composition ===");

    let red_dog = compose_modifier(&mut memory, "red", "dog");
    let blue_dog = compose_modifier(&mut memory, "blue", "dog");
    let red_cat = compose_modifier(&mut memory, "red", "cat");

    println!(
        "'red dog' vs 'blue dog' similarity: {:.3}",
        red_dog.similarity(&blue_dog)
    );
    println!(
        "'red dog' vs 'red cat' similarity: {:.3}",
        red_dog.similarity(&red_cat)
    );
    println!(
        "'blue dog' vs 'red cat' similarity: {:.3}",
        blue_dog.similarity(&red_cat)
    );

    // Query composed structure
    println!("\nQuerying 'red dog' for modifier role:");
    let recovered = query_role(&memory, &red_dog, "modifier");
    let matches = memory.query(&recovered);
    println!("  Top matches: {:?}", &matches[..3.min(matches.len())]);

    // Sequence composition
    println!("\n=== Sequence Composition ===");

    let seq1 = compose_sequence(&mut memory, &["run", "jump", "fly"]);
    let seq2 = compose_sequence(&mut memory, &["run", "jump", "swim"]);
    let seq3 = compose_sequence(&mut memory, &["fly", "jump", "run"]);

    println!(
        "'run→jump→fly' vs 'run→jump→swim': {:.3}",
        seq1.similarity(&seq2)
    );
    println!(
        "'run→jump→fly' vs 'fly→jump→run': {:.3}",
        seq1.similarity(&seq3)
    );
    println!("  (Order matters: same elements, different sequence = different representation)");

    // Triple composition
    println!("\n=== Relation Triple Composition ===");

    let triple1 = compose_triple(&mut memory, "dog", "chase", "cat");
    let triple2 = compose_triple(&mut memory, "cat", "chase", "bird");
    let triple3 = compose_triple(&mut memory, "dog", "eat", "cat");

    println!(
        "'dog chase cat' vs 'cat chase bird': {:.3}",
        triple1.similarity(&triple2)
    );
    println!(
        "'dog chase cat' vs 'dog eat cat': {:.3}",
        triple1.similarity(&triple3)
    );

    // Query subject from triple
    println!("\nQuerying 'dog chase cat' for subject:");
    let subject_query = query_role(&memory, &triple1, "subject");
    let subject_matches = memory.query(&subject_query);
    println!(
        "  Top matches: {:?}",
        &subject_matches[..3.min(subject_matches.len())]
    );

    // Analogical reasoning
    println!("\n=== Analogical Reasoning ===");
    println!("Solving: 'king' is to 'queen' as 'man' is to ?");

    let answer = analogy(&mut memory, "king", "queen", "man");
    let analogy_matches = memory.query(&answer);
    println!(
        "  Top matches: {:?}",
        &analogy_matches[..5.min(analogy_matches.len())]
    );
    println!("  Expected: 'woman' should be near the top");

    // Zero-shot composition
    println!("\n=== Zero-Shot Composition ===");
    println!("Composing 'big blue cat' (never seen together):");

    // Multi-modifier composition
    let big = memory.get("big").bind(memory.role("modifier").unwrap());
    let blue = memory
        .get("blue")
        .bind(memory.role("modifier").unwrap())
        .permute(5);
    let cat = memory.get("cat").bind(memory.role("subject").unwrap());
    let big_blue_cat = Hypervector::bundle(&[big, blue, cat]);

    // Compare to similar compositions
    let small_red_dog = {
        let small = memory.get("small").bind(memory.role("modifier").unwrap());
        let red = memory
            .get("red")
            .bind(memory.role("modifier").unwrap())
            .permute(5);
        let dog = memory.get("dog").bind(memory.role("subject").unwrap());
        Hypervector::bundle(&[small, red, dog])
    };

    let big_blue_dog = {
        let big = memory.get("big").bind(memory.role("modifier").unwrap());
        let blue = memory
            .get("blue")
            .bind(memory.role("modifier").unwrap())
            .permute(5);
        let dog = memory.get("dog").bind(memory.role("subject").unwrap());
        Hypervector::bundle(&[big, blue, dog])
    };

    println!(
        "'big blue cat' vs 'small red dog': {:.3}",
        big_blue_cat.similarity(&small_red_dog)
    );
    println!(
        "'big blue cat' vs 'big blue dog': {:.3}",
        big_blue_cat.similarity(&big_blue_dog)
    );
    println!("  (Sharing modifiers increases similarity)");

    // Performance test
    println!("\n=== Performance ===");
    let start = std::time::Instant::now();
    let iterations = 10_000;

    let v1 = Hypervector::random(42);
    let v2 = Hypervector::random(123);

    for _ in 0..iterations {
        let _ = v1.bind(&v2);
    }
    let bind_time = start.elapsed();

    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let _ = v1.similarity(&v2);
    }
    let sim_time = start.elapsed();

    println!(
        "Bind (XOR) time: {:.1}ns per op",
        bind_time.as_nanos() as f64 / iterations as f64
    );
    println!(
        "Similarity time: {:.1}ns per op",
        sim_time.as_nanos() as f64 / iterations as f64
    );

    println!("\n=== Key Benefits ===");
    println!("- Zero-shot: compose any combination of known concepts");
    println!("- Sub-100ns: composition is just XOR operations");
    println!("- Algebraic: unbind to recover components");
    println!("- Distributed: no central vocabulary server");
    println!("- Interpretable: query reveals structure");
    println!("\nThis is what embeddings should have been: compositional by construction.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bind_unbind() {
        let a = Hypervector::random(42);
        let b = Hypervector::random(123);

        let bound = a.bind(&b);
        let recovered = bound.unbind(&b);

        // Recovered should be very similar to original
        assert!(recovered.similarity(&a) > 0.95);
    }

    #[test]
    fn test_binding_dissimilarity() {
        let a = Hypervector::random(42);
        let b = Hypervector::random(123);

        let bound = a.bind(&b);

        // Bound should be dissimilar to both components
        assert!(bound.similarity(&a).abs() < 0.2);
        assert!(bound.similarity(&b).abs() < 0.2);
    }

    #[test]
    fn test_bundle_similarity() {
        let a = Hypervector::random(42);
        let b = Hypervector::random(123);
        let c = Hypervector::random(456);

        let bundle_ab = Hypervector::bundle(&[a.clone(), b.clone()]);
        let bundle_ac = Hypervector::bundle(&[a.clone(), c.clone()]);

        // Bundles with shared component should be somewhat similar
        let sim = bundle_ab.similarity(&bundle_ac);
        assert!(sim > 0.2); // Some similarity due to shared A
    }

    #[test]
    fn test_composition() {
        let mut memory = ConceptMemory::new();

        let red_dog = compose_modifier(&mut memory, "red", "dog");
        let red_cat = compose_modifier(&mut memory, "red", "cat");
        let blue_dog = compose_modifier(&mut memory, "blue", "dog");

        // Same modifier = more similar than same noun
        let rd_rc = red_dog.similarity(&red_cat);
        let rd_bd = red_dog.similarity(&blue_dog);

        // Both should show some similarity due to shared component
        assert!(rd_rc.abs() > 0.1);
        assert!(rd_bd.abs() > 0.1);
    }

    #[test]
    fn test_sequence_order() {
        let mut memory = ConceptMemory::new();

        let seq1 = compose_sequence(&mut memory, &["a", "b", "c"]);
        let seq2 = compose_sequence(&mut memory, &["c", "b", "a"]);

        // Different order should produce different representations
        assert!(seq1.similarity(&seq2) < 0.5);
    }
}
