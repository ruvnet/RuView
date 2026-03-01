//! Integration tests for k-mer indexing module
//!
//! These tests use real VectorDB instances to validate k-mer encoding,
//! indexing, and similarity search functionality.

use ::rvdna::kmer::{canonical_kmer, KmerEncoder, KmerIndex, MinHashSketch};
use tempfile::TempDir;

/// Helper to create a test directory that will be automatically cleaned up
fn create_test_db() -> TempDir {
    TempDir::new().expect("Failed to create temp directory")
}

#[test]
fn test_kmer_encoding_basic() {
    let encoder = KmerEncoder::new(4).expect("Failed to create encoder");
    let sequence = b"ACGTACGT";

    let vector = encoder
        .encode_sequence(sequence)
        .expect("Failed to encode sequence");

    // Verify vector has correct dimensions
    assert_eq!(
        vector.len(),
        encoder.dimensions(),
        "Vector dimensions should match encoder dimensions"
    );

    // Verify L2 normalization
    let magnitude: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (magnitude - 1.0).abs() < 1e-5,
        "Vector should be L2 normalized, got magnitude: {}",
        magnitude
    );

    // Verify non-zero elements exist (sequence has k-mers)
    let non_zero_count = vector.iter().filter(|&&x| x != 0.0).count();
    assert!(non_zero_count > 0, "Vector should have non-zero elements");
}

#[test]
fn test_kmer_encoding_deterministic() {
    let encoder = KmerEncoder::new(11).expect("Failed to create encoder");
    let sequence = b"ACGTACGTACGTACGTACGT";

    let vector1 = encoder
        .encode_sequence(sequence)
        .expect("Failed to encode sequence first time");
    let vector2 = encoder
        .encode_sequence(sequence)
        .expect("Failed to encode sequence second time");

    // Verify same sequence produces identical vectors
    assert_eq!(
        vector1.len(),
        vector2.len(),
        "Vectors should have same length"
    );

    for (i, (&v1, &v2)) in vector1.iter().zip(vector2.iter()).enumerate() {
        assert!(
            (v1 - v2).abs() < 1e-6,
            "Vector element {} should be identical: {} vs {}",
            i,
            v1,
            v2
        );
    }
}

#[test]
fn test_kmer_complement_symmetry() {
    let kmer1 = b"ACGT";
    let kmer2 = b"ACGT"; // reverse complement is ACGT (palindrome)

    let canon1 = canonical_kmer(kmer1);
    let canon2 = canonical_kmer(kmer2);

    assert_eq!(canon1, canon2, "Canonical k-mers should be equal");

    // Test with non-palindrome
    let kmer3 = b"AAAA";
    let kmer4 = b"TTTT"; // reverse complement of AAAA

    let canon3 = canonical_kmer(kmer3);
    let canon4 = canonical_kmer(kmer4);

    assert_eq!(
        canon3, canon4,
        "Canonical k-mer should be same for sequence and revcomp"
    );
}

#[test]
fn test_kmer_index_insert_and_search() {
    let _temp_dir = create_test_db();

    // Create index with k=11
    let encoder = KmerEncoder::new(11).expect("Failed to create encoder");
    let index = KmerIndex::new(11, encoder.dimensions()).expect("Failed to create index");

    // Insert 3 sequences
    let seq1 = b"ACGTACGTACGTACGTACGT";
    let seq2 = b"ACGTACGTACGTACGTACGG"; // Similar to seq1
    let seq3 = b"TTTTTTTTTTTTTTTTTTTT"; // Very different

    index
        .index_sequence("seq1", seq1)
        .expect("Failed to index seq1");
    index
        .index_sequence("seq2", seq2)
        .expect("Failed to index seq2");
    index
        .index_sequence("seq3", seq3)
        .expect("Failed to index seq3");

    // Search for similar sequences to seq1
    let results = index.search_similar(seq1, 3).expect("Failed to search");

    assert!(results.len() > 0, "Should find at least one result");

    // First result should be seq1 itself (exact match)
    assert_eq!(results[0].id, "seq1", "First result should be exact match");
    assert!(
        results[0].distance < 0.01,
        "Exact match should have very low distance: {}",
        results[0].distance
    );

    // seq2 should be closer than seq3
    let seq2_idx = results.iter().position(|r| r.id == "seq2");
    let seq3_idx = results.iter().position(|r| r.id == "seq3");

    if let (Some(idx2), Some(idx3)) = (seq2_idx, seq3_idx) {
        assert!(
            idx2 < idx3,
            "Similar sequence should rank higher than different sequence"
        );
    }
}

#[test]
fn test_kmer_index_batch_insert() {
    let _temp_dir = create_test_db();

    let encoder = KmerEncoder::new(11).expect("Failed to create encoder");
    let index = KmerIndex::new(11, encoder.dimensions()).expect("Failed to create index");

    // Generate 100 random sequences
    let mut sequences = Vec::new();
    for i in 0..100 {
        let seq = generate_random_sequence(50, i as u64);
        sequences.push((format!("seq_{}", i), seq));
    }

    // Convert to reference slices for batch insert
    let batch: Vec<(&str, &[u8])> = sequences
        .iter()
        .map(|(id, seq)| (id.as_str(), seq.as_slice()))
        .collect();

    // Batch insert
    index
        .index_batch(batch)
        .expect("Failed to batch insert sequences");

    // Verify we can search and get results
    let query = b"ACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGT";
    let results = index.search_similar(query, 10).expect("Failed to search");

    assert!(results.len() > 0, "Should find results after batch insert");
}

#[test]
fn test_kmer_similar_sequences_score_higher() {
    let _temp_dir = create_test_db();

    let encoder = KmerEncoder::new(11).expect("Failed to create encoder");
    let index = KmerIndex::new(11, encoder.dimensions()).expect("Failed to create index");

    // Create two similar sequences (90% identical)
    let base_seq = b"ACGTACGTACGTACGTACGTACGTACGTACGTACGTACGT"; // 40 bases
    let similar_seq = b"ACGTACGTACGTACGTACGTACGTACGTACGTACGTACGG"; // 1 base different
    let random_seq = generate_random_sequence(40, 12345);

    index
        .index_sequence("base", base_seq)
        .expect("Failed to index base");
    index
        .index_sequence("similar", similar_seq)
        .expect("Failed to index similar");
    index
        .index_sequence("random", &random_seq)
        .expect("Failed to index random");

    // Search with base sequence
    let results = index
        .search_similar(base_seq, 10)
        .expect("Failed to search");

    assert!(results.len() > 0, "Should find at least one result");

    // Find positions in results
    let base_pos = results.iter().position(|r| r.id == "base");
    let similar_pos = results.iter().position(|r| r.id == "similar");

    // Base and similar should definitely be in top results
    assert!(
        base_pos.is_some(),
        "Base sequence (exact match) should be found in results"
    );
    assert!(
        similar_pos.is_some(),
        "Similar sequence should be found in results"
    );

    // Base should be first (exact match has distance 0)
    assert_eq!(
        base_pos.unwrap(),
        0,
        "Base sequence should be the top result (exact match)"
    );

    // Similar sequence should be in top 3
    assert!(
        similar_pos.unwrap() < 3,
        "Similar sequence should rank in top 3, was at position {}",
        similar_pos.unwrap()
    );
}

#[test]
fn test_kmer_different_k_values() {
    // Test k=11
    let encoder11 = KmerEncoder::new(11).expect("Failed to create k=11 encoder");
    let seq = b"ACGTACGTACGTACGTACGTACGTACGT";
    let vec11 = encoder11
        .encode_sequence(seq)
        .expect("Failed to encode with k=11");
    assert_eq!(vec11.len(), encoder11.dimensions());

    // Test k=21
    let encoder21 = KmerEncoder::new(21).expect("Failed to create k=21 encoder");
    let seq_long = b"ACGTACGTACGTACGTACGTACGTACGTACGTACGTACGT";
    let vec21 = encoder21
        .encode_sequence(seq_long)
        .expect("Failed to encode with k=21");
    assert_eq!(vec21.len(), encoder21.dimensions());

    // Test k=31
    let encoder31 = KmerEncoder::new(31).expect("Failed to create k=31 encoder");
    let seq_longer = b"ACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGT";
    let vec31 = encoder31
        .encode_sequence(seq_longer)
        .expect("Failed to encode with k=31");
    assert_eq!(vec31.len(), encoder31.dimensions());

    // All should be normalized
    for (vec, k) in &[(vec11, 11), (vec21, 21), (vec31, 31)] {
        let magnitude: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (magnitude - 1.0).abs() < 1e-5,
            "k={} vector should be normalized",
            k
        );
    }
}

#[test]
fn test_minhash_sketch_basic() {
    let num_hashes = 100;
    let mut sketch = MinHashSketch::new(num_hashes);
    let sequence = b"ACGTACGTACGTACGTACGTACGTACGTACGT";

    let hashes = sketch
        .sketch(sequence, 11)
        .expect("Failed to sketch sequence");

    assert!(
        hashes.len() <= num_hashes,
        "Sketch should have at most {} hashes, got {}",
        num_hashes,
        hashes.len()
    );
    assert!(hashes.len() > 0, "Sketch should have at least one hash");

    // Verify hashes are sorted (implementation detail)
    for i in 1..hashes.len() {
        assert!(hashes[i] >= hashes[i - 1], "Hashes should be sorted");
    }
}

#[test]
fn test_minhash_jaccard_identical() {
    let mut sketch1 = MinHashSketch::new(100);
    let mut sketch2 = MinHashSketch::new(100);

    let sequence = b"ACGTACGTACGTACGTACGTACGTACGTACGT";

    sketch1
        .sketch(sequence, 11)
        .expect("Failed to sketch sequence 1");
    sketch2
        .sketch(sequence, 11)
        .expect("Failed to sketch sequence 2");

    let distance = sketch1.jaccard_distance(&sketch2);

    assert!(
        distance < 0.01,
        "Identical sequences should have distance close to 0, got {}",
        distance
    );
}

#[test]
fn test_minhash_jaccard_different() {
    let mut sketch1 = MinHashSketch::new(100);
    let mut sketch2 = MinHashSketch::new(100);

    let seq1 = b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    let seq2 = b"CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";

    sketch1
        .sketch(seq1, 11)
        .expect("Failed to sketch sequence 1");
    sketch2
        .sketch(seq2, 11)
        .expect("Failed to sketch sequence 2");

    let distance = sketch1.jaccard_distance(&sketch2);

    assert!(
        distance > 0.9,
        "Very different sequences should have distance close to 1, got {}",
        distance
    );
}

#[test]
fn test_kmer_index_empty_sequence() {
    let encoder = KmerEncoder::new(11).expect("Failed to create encoder");

    // Test empty sequence
    let empty_seq = b"";
    let result = encoder.encode_sequence(empty_seq);

    assert!(result.is_err(), "Empty sequence should return error");

    // Test sequence shorter than k
    let short_seq = b"ACGT"; // k=11 but only 4 bases
    let result = encoder.encode_sequence(short_seq);

    assert!(
        result.is_err(),
        "Sequence shorter than k should return error"
    );
}

#[test]
fn test_kmer_index_with_n_bases() {
    let encoder = KmerEncoder::new(11).expect("Failed to create encoder");

    // Sequence with N (unknown) bases
    let seq_with_n = b"ACGTACGTNNNACGTACGT";

    // Should still encode (N bases are handled in canonical_kmer)
    let result = encoder.encode_sequence(seq_with_n);

    assert!(
        result.is_ok(),
        "Sequence with N bases should encode successfully"
    );

    let vector = result.unwrap();
    assert_eq!(
        vector.len(),
        encoder.dimensions(),
        "Vector should have correct dimensions"
    );
}

// Helper function to generate random DNA sequences
fn generate_random_sequence(length: usize, seed: u64) -> Vec<u8> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let bases = [b'A', b'C', b'G', b'T'];
    let mut sequence = Vec::with_capacity(length);

    for i in 0..length {
        let mut hasher = DefaultHasher::new();
        seed.hash(&mut hasher);
        i.hash(&mut hasher);
        let hash = hasher.finish();
        let base_idx = (hash % 4) as usize;
        sequence.push(bases[base_idx]);
    }

    sequence
}
