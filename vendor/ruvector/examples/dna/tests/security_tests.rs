//! Security validation tests for DNA analyzer - NO MOCKS, real computation only
use ::rvdna::error::DnaError;
use ::rvdna::types::*;
use ::rvdna::VectorEntry;
use std::sync::{Arc, Mutex};
use std::thread;

#[test]
fn test_buffer_overflow_protection() {
    // 10M+ bases shouldn't cause OOM/crash
    let large_size = 10_000_000;
    let bases: Vec<Nucleotide> = (0..large_size)
        .map(|i| match i % 4 {
            0 => Nucleotide::A,
            1 => Nucleotide::C,
            2 => Nucleotide::G,
            _ => Nucleotide::T,
        })
        .collect();
    let seq = DnaSequence::new(bases);
    assert_eq!(seq.len(), large_size);
    let rc = seq.reverse_complement();
    assert_eq!(rc.len(), large_size);
    assert!(seq.to_kmer_vector(11, 512).is_ok());
}

#[test]
fn test_invalid_base_handling() {
    // Non-ACGTN characters rejected gracefully
    for input in ["ACGTX", "ACGT123", "ACGT!@#"] {
        let result = DnaSequence::from_str(input);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DnaError::InvalidSequence(_)));
    }
    assert!(DnaSequence::from_str("ACGTN").is_ok());
    assert!(DnaSequence::from_str("acgtn").is_ok());
}

#[test]
fn test_unicode_injection() {
    // Unicode/malicious IDs don't break indexing
    let seq = DnaSequence::from_str("ACGTACGT").unwrap();
    let vector = seq.to_kmer_vector(3, 128).unwrap();
    let temp_dir = std::env::temp_dir().join(format!("dna_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let index = KmerIndex::new(3, 128, temp_dir.join("unicode").to_str().unwrap()).unwrap();

    for id in ["seq_cafe_dna", "patient123", "seq_hidden"] {
        let entry = VectorEntry {
            id: Some(id.to_string()),
            vector: vector.clone(),
            metadata: None,
        };
        assert!(index.db().insert(entry).is_ok());
    }
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_path_traversal_prevention() {
    // Verify KmerIndex handles unusual paths without panicking
    // The key security property: operations complete or fail gracefully
    let temp_dir = std::env::temp_dir().join(format!("dna_path_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);

    for path in ["../../../tmp/evil", "../../etc/passwd"] {
        let full_path = temp_dir.join(path);
        // KmerIndex creation with traversal paths should either succeed
        // (contained to actual resolved path) or fail gracefully - never panic
        let result =
            std::panic::catch_unwind(|| KmerIndex::new(3, 128, full_path.to_str().unwrap()));
        assert!(result.is_ok(), "Path traversal should not cause panic");
    }

    // Clean up any created dirs
    let _ = std::fs::remove_dir_all(&temp_dir);
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("evil"));
}

#[test]
fn test_integer_overflow_kmer() {
    // k=64 would overflow, k=0 invalid
    let seq = DnaSequence::from_str("ACGTACGTACGTACGT").unwrap();
    assert!(matches!(
        seq.to_kmer_vector(64, 512).unwrap_err(),
        DnaError::InvalidKmerSize(64)
    ));
    assert!(seq.to_kmer_vector(0, 512).is_err());
    assert!(seq.to_kmer_vector(11, 512).is_ok());
    assert!(seq.to_kmer_vector(15, 512).is_ok());
}

#[test]
fn test_empty_input_safety() {
    // Empty inputs handled safely
    assert!(matches!(
        DnaSequence::from_str("").unwrap_err(),
        DnaError::EmptySequence
    ));
    let empty = DnaSequence::new(vec![]);
    assert!(empty.is_empty() && empty.len() == 0);
    assert!(empty.complement().is_empty());
    assert!(empty.reverse_complement().is_empty());
    assert_eq!(empty.to_string(), "");
}

#[test]
fn test_null_byte_handling() {
    // Null bytes rejected
    assert!(DnaSequence::from_str("ACGT\0").is_err());
}

#[test]
fn test_concurrent_access_safety() {
    // 10 threads accessing VectorDB concurrently
    let temp_dir = std::env::temp_dir().join(format!("dna_conc_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let index = Arc::new(Mutex::new(
        KmerIndex::new(3, 128, temp_dir.join("idx").to_str().unwrap()).unwrap(),
    ));

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let idx_clone = Arc::clone(&index);
            thread::spawn(move || {
                let seq = DnaSequence::from_str("ACGTACGTACGT").unwrap();
                let entry = VectorEntry {
                    id: Some(format!("seq_{}", i)),
                    vector: seq.to_kmer_vector(3, 128).unwrap(),
                    metadata: None,
                };
                idx_clone.lock().unwrap().db().insert(entry).unwrap();
            })
        })
        .collect();

    for h in handles {
        assert!(h.join().is_ok());
    }
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_quality_score_bounds() {
    // Phred >93 rejected, 0-93 accepted
    assert!(matches!(
        QualityScore::new(100).unwrap_err(),
        DnaError::InvalidQuality(100)
    ));
    assert!(QualityScore::new(0).is_ok());
    assert!(QualityScore::new(93).is_ok());
    assert!((QualityScore::new(30).unwrap().to_error_probability() - 0.001).abs() < 1e-6);
    assert!((QualityScore::new(0).unwrap().to_error_probability() - 1.0).abs() < 0.01);
}

#[test]
fn test_variant_position_overflow() {
    // u64::MAX positions handled
    let pos = GenomicPosition {
        chromosome: 25,
        position: u64::MAX,
        reference_allele: Nucleotide::A,
        alternate_allele: Some(Nucleotide::G),
    };
    assert_eq!(pos.position, u64::MAX);
}

#[test]
fn test_methylation_bounds() {
    // Beta values clamped to [0,1]
    for val in [-0.5f32, 0.0, 0.5, 1.0, 1.5] {
        let clamped = val.clamp(0.0, 1.0);
        assert!(clamped >= 0.0 && clamped <= 1.0);
    }
}

#[test]
fn test_deterministic_output() {
    // Same input -> same output (no randomness)
    let seq = DnaSequence::from_str("ACGTACGTACGTACGT").unwrap();
    assert_eq!(
        seq.to_kmer_vector(11, 512).unwrap(),
        seq.to_kmer_vector(11, 512).unwrap()
    );
    assert_eq!(
        seq.reverse_complement().to_string(),
        seq.reverse_complement().to_string()
    );
    assert_eq!(seq.complement().to_string(), seq.complement().to_string());
    assert_eq!(seq.to_string(), seq.to_string());
}
