//! Criterion benchmarks for DNA Analyzer
//!
//! Comprehensive performance benchmarks covering:
//! - K-mer encoding and HNSW indexing
//! - Sequence alignment
//! - Variant calling
//! - Protein translation
//! - Full pipeline integration

use ::rvdna::prelude::*;
use ::rvdna::types::KmerIndex as TypesKmerIndex;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Generate random DNA sequence of specified length
fn random_dna(len: usize, seed: u64) -> DnaSequence {
    let mut rng = StdRng::seed_from_u64(seed);
    let bases = [Nucleotide::A, Nucleotide::C, Nucleotide::G, Nucleotide::T];
    let sequence: Vec<Nucleotide> = (0..len).map(|_| bases[rng.gen_range(0..4)]).collect();
    DnaSequence::new(sequence)
}

/// Generate multiple random sequences
fn random_sequences(count: usize, len: usize, seed: u64) -> Vec<DnaSequence> {
    (0..count)
        .map(|i| random_dna(len, seed + i as u64))
        .collect()
}

// ============================================================================
// K-mer Benchmarks
// ============================================================================

fn kmer_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("kmer");

    group.bench_function("encode_1kb", |b| {
        let seq = random_dna(1_000, 42);
        b.iter(|| black_box(seq.to_kmer_vector(11, 512).unwrap()));
    });

    group.bench_function("encode_10kb", |b| {
        let seq = random_dna(10_000, 42);
        b.iter(|| black_box(seq.to_kmer_vector(11, 512).unwrap()));
    });

    group.bench_function("encode_100kb", |b| {
        let seq = random_dna(100_000, 42);
        b.iter(|| black_box(seq.to_kmer_vector(11, 512).unwrap()));
    });

    // HNSW index insertion
    group.bench_function("index_insert_100", |b| {
        let sequences = random_sequences(100, 100, 42);
        b.iter(|| {
            let temp = tempfile::TempDir::new().unwrap();
            let index =
                TypesKmerIndex::new(11, 512, temp.path().join("idx").to_str().unwrap()).unwrap();
            for (i, seq) in sequences.iter().enumerate() {
                let vec = seq.to_kmer_vector(11, 512).unwrap();
                index
                    .db()
                    .insert(ruvector_core::VectorEntry {
                        id: Some(format!("seq{}", i)),
                        vector: vec,
                        metadata: None,
                    })
                    .unwrap();
            }
            black_box(index)
        });
    });

    // HNSW search
    group.bench_function("search_top10", |b| {
        let sequences = random_sequences(100, 100, 42);
        let temp = tempfile::TempDir::new().unwrap();
        let index =
            TypesKmerIndex::new(11, 512, temp.path().join("idx").to_str().unwrap()).unwrap();

        for (i, seq) in sequences.iter().enumerate() {
            let vec = seq.to_kmer_vector(11, 512).unwrap();
            index
                .db()
                .insert(ruvector_core::VectorEntry {
                    id: Some(format!("seq{}", i)),
                    vector: vec,
                    metadata: None,
                })
                .unwrap();
        }

        let query = random_dna(100, 999);
        let query_vec = query.to_kmer_vector(11, 512).unwrap();

        b.iter(|| {
            black_box(
                index
                    .db()
                    .search(ruvector_core::SearchQuery {
                        vector: query_vec.clone(),
                        k: 10,
                        filter: None,
                        ef_search: None,
                    })
                    .unwrap(),
            )
        });
    });

    group.finish();
}

// ============================================================================
// Alignment Benchmarks
// ============================================================================

fn alignment_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("alignment");

    group.bench_function("one_hot_encoding_1kb", |b| {
        let seq = random_dna(1_000, 42);
        b.iter(|| black_box(seq.encode_one_hot()));
    });

    group.bench_function("attention_align_100bp", |b| {
        let query = random_dna(100, 42);
        let reference = random_dna(1_000, 43);
        b.iter(|| black_box(query.align_with_attention(&reference).unwrap()));
    });

    group.bench_function("smith_waterman_100bp", |b| {
        let query = random_dna(100, 42);
        let reference = random_dna(500, 43);
        let aligner = SmithWaterman::new(AlignmentConfig::default());
        b.iter(|| black_box(aligner.align(&query, &reference).unwrap()));
    });

    group.finish();
}

// ============================================================================
// Variant Calling Benchmarks
// ============================================================================

fn variant_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("variant");

    group.bench_function("snp_calling_single", |b| {
        let caller = VariantCaller::new(VariantCallerConfig::default());
        let pileup = PileupColumn {
            bases: vec![b'A', b'A', b'G', b'G', b'G', b'G', b'G', b'G', b'G', b'G'],
            qualities: vec![35; 10],
            position: 12345,
            chromosome: 1,
        };

        b.iter(|| black_box(caller.call_snp(&pileup, b'A')));
    });

    group.bench_function("snp_calling_1000_positions", |b| {
        let caller = VariantCaller::new(VariantCallerConfig::default());
        let mut rng = StdRng::seed_from_u64(42);

        let pileups: Vec<(PileupColumn, u8)> = (0..1000)
            .map(|i| {
                let bases: Vec<u8> = (0..20)
                    .map(|_| [b'A', b'C', b'G', b'T'][rng.gen_range(0..4)])
                    .collect();
                let quals: Vec<u8> = (0..20).map(|_| rng.gen_range(20..41)).collect();
                let ref_base = [b'A', b'C', b'G', b'T'][i % 4];
                (
                    PileupColumn {
                        bases,
                        qualities: quals,
                        position: i as u64,
                        chromosome: 1,
                    },
                    ref_base,
                )
            })
            .collect();

        b.iter(|| {
            let mut count = 0;
            for (pileup, ref_base) in &pileups {
                if caller.call_snp(pileup, *ref_base).is_some() {
                    count += 1;
                }
            }
            black_box(count)
        });
    });

    group.finish();
}

// ============================================================================
// Protein Analysis Benchmarks
// ============================================================================

fn protein_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("protein");

    group.bench_function("translate_1kb", |b| {
        let seq = random_dna(1_002, 42);
        b.iter(|| black_box(seq.translate().unwrap()));
    });

    group.bench_function("contact_graph_100residues", |b| {
        let protein = create_random_protein(100, 42);
        b.iter(|| black_box(protein.build_contact_graph(8.0).unwrap()));
    });

    group.bench_function("contact_prediction_100residues", |b| {
        let protein = create_random_protein(100, 42);
        let graph = protein.build_contact_graph(8.0).unwrap();
        b.iter(|| black_box(protein.predict_contacts(&graph).unwrap()));
    });

    group.finish();
}

// ============================================================================
// RVDNA Format Benchmarks
// ============================================================================

fn rvdna_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("rvdna");

    group.bench_function("encode_2bit_1kb", |b| {
        let seq = random_dna(1_000, 42);
        b.iter(|| black_box(rvdna::encode_2bit(seq.bases())));
    });

    group.bench_function("encode_2bit_100kb", |b| {
        let seq = random_dna(100_000, 42);
        b.iter(|| black_box(rvdna::encode_2bit(seq.bases())));
    });

    group.bench_function("fasta_to_rvdna_1kb", |b| {
        let seq_str: String = random_dna(1_000, 42)
            .bases()
            .iter()
            .map(|n| match n {
                Nucleotide::A => 'A',
                Nucleotide::C => 'C',
                Nucleotide::G => 'G',
                Nucleotide::T => 'T',
                _ => 'N',
            })
            .collect();
        b.iter(|| black_box(rvdna::fasta_to_rvdna(&seq_str, 11, 256, 1000).unwrap()));
    });

    group.finish();
}

// ============================================================================
// Epigenomics Benchmarks
// ============================================================================

fn epigenomics_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("epigenomics");

    group.bench_function("cancer_signal_1000_sites", |b| {
        let positions: Vec<(u8, u64)> = (0..1000).map(|i| (1u8, i as u64)).collect();
        let betas: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0)).collect();
        let profile = rvdna::MethylationProfile::from_beta_values(positions, betas);
        let detector = rvdna::CancerSignalDetector::new();
        b.iter(|| black_box(detector.detect(&profile)));
    });

    group.bench_function("horvath_clock_1000_sites", |b| {
        let positions: Vec<(u8, u64)> = (0..1000).map(|i| (1u8, i as u64)).collect();
        let betas: Vec<f32> = (0..1000).map(|i| (i as f32 / 2000.0 + 0.25)).collect();
        let profile = rvdna::MethylationProfile::from_beta_values(positions, betas);
        let clock = rvdna::HorvathClock::default_clock();
        b.iter(|| black_box(clock.predict_age(&profile)));
    });

    group.finish();
}

// ============================================================================
// Protein Analysis Benchmarks (extended)
// ============================================================================

fn protein_extended_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("protein_analysis");

    group.bench_function("molecular_weight_300aa", |b| {
        let protein = rvdna::translate_dna(
            &random_dna(900, 42)
                .bases()
                .iter()
                .map(|n| match n {
                    Nucleotide::A => b'A',
                    Nucleotide::C => b'C',
                    Nucleotide::G => b'G',
                    Nucleotide::T => b'T',
                    _ => b'N',
                })
                .collect::<Vec<u8>>(),
        );
        b.iter(|| black_box(rvdna::molecular_weight(&protein)));
    });

    group.bench_function("isoelectric_point_300aa", |b| {
        let protein = rvdna::translate_dna(
            &random_dna(900, 42)
                .bases()
                .iter()
                .map(|n| match n {
                    Nucleotide::A => b'A',
                    Nucleotide::C => b'C',
                    Nucleotide::G => b'G',
                    Nucleotide::T => b'T',
                    _ => b'N',
                })
                .collect::<Vec<u8>>(),
        );
        b.iter(|| black_box(rvdna::isoelectric_point(&protein)));
    });

    group.finish();
}

// ============================================================================
// Full Pipeline Benchmarks
// ============================================================================

fn pipeline_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline");

    group.bench_function("full_pipeline_1kb", |b| {
        let reference = random_dna(1_000, 42);
        let reads = random_sequences(20, 150, 43);
        let caller = VariantCaller::new(VariantCallerConfig::default());

        b.iter(|| {
            // K-mer encoding
            let ref_vec = reference.to_kmer_vector(11, 512).unwrap();

            // Align reads
            let mut alignments = Vec::new();
            for read in &reads {
                if let Ok(alignment) = read.align_with_attention(&reference) {
                    alignments.push(alignment);
                }
            }

            // Call variants at a few positions
            let mut variants = Vec::new();
            let pileup = PileupColumn {
                bases: vec![b'A', b'G', b'G', b'G', b'A', b'G', b'G', b'A', b'G', b'G'],
                qualities: vec![35; 10],
                position: 0,
                chromosome: 1,
            };
            if let Some(v) = caller.call_snp(&pileup, b'A') {
                variants.push(v);
            }

            // Translate to protein
            let protein = reference.translate().unwrap();

            black_box((ref_vec, alignments, variants, protein))
        });
    });

    group.finish();
}

// ============================================================================
// Helpers
// ============================================================================

fn create_random_protein(len: usize, seed: u64) -> ProteinSequence {
    let mut rng = StdRng::seed_from_u64(seed);
    let residues = [
        ProteinResidue::A,
        ProteinResidue::C,
        ProteinResidue::D,
        ProteinResidue::E,
        ProteinResidue::F,
        ProteinResidue::G,
        ProteinResidue::H,
        ProteinResidue::I,
        ProteinResidue::K,
        ProteinResidue::L,
        ProteinResidue::M,
        ProteinResidue::N,
    ];

    let sequence: Vec<ProteinResidue> = (0..len)
        .map(|_| residues[rng.gen_range(0..residues.len())])
        .collect();

    ProteinSequence::new(sequence)
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    kmer_benchmarks,
    alignment_benchmarks,
    variant_benchmarks,
    protein_benchmarks,
    rvdna_benchmarks,
    epigenomics_benchmarks,
    protein_extended_benchmarks,
    pipeline_benchmarks
);

criterion_main!(benches);
