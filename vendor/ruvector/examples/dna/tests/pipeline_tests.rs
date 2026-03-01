//! End-to-End Integration Tests for DNA Analysis Pipeline
//!
//! Real data, real computation, real assertions. No mocks, no stubs.
//! Tests the complete DNA analysis workflow from nucleotide encoding
//! through variant calling, protein translation, epigenetics, and pharmacogenomics.

use ::rvdna::*;

// ============================================================================
// NUCLEOTIDE & SEQUENCE TESTS
// ============================================================================

#[test]
fn test_nucleotide_encoding() {
    assert_eq!(Nucleotide::A.to_u8(), 0);
    assert_eq!(Nucleotide::C.to_u8(), 1);
    assert_eq!(Nucleotide::G.to_u8(), 2);
    assert_eq!(Nucleotide::T.to_u8(), 3);
    assert_eq!(Nucleotide::N.to_u8(), 4);

    assert_eq!(Nucleotide::from_u8(0).unwrap(), Nucleotide::A);
    assert_eq!(Nucleotide::from_u8(1).unwrap(), Nucleotide::C);
    assert_eq!(Nucleotide::from_u8(2).unwrap(), Nucleotide::G);
    assert_eq!(Nucleotide::from_u8(3).unwrap(), Nucleotide::T);
    assert_eq!(Nucleotide::from_u8(4).unwrap(), Nucleotide::N);
}

#[test]
fn test_dna_sequence_reverse_complement() {
    let seq1 = DnaSequence::from_str("ACGT").unwrap();
    let rc1 = seq1.reverse_complement();
    assert_eq!(rc1.to_string(), "ACGT");

    let seq2 = DnaSequence::from_str("AACG").unwrap();
    let rc2 = seq2.reverse_complement();
    assert_eq!(rc2.to_string(), "CGTT");

    let seq3 = DnaSequence::from_str("ATGCATGC").unwrap();
    let rc3 = seq3.reverse_complement();
    assert_eq!(rc3.to_string(), "GCATGCAT");
}

// ============================================================================
// VARIANT CALLING TESTS
// ============================================================================

#[test]
fn test_variant_calling_homozygous_snp() {
    let caller = VariantCaller::new(VariantCallerConfig::default());

    let pileup = PileupColumn {
        bases: vec![b'G'; 15],
        qualities: vec![40; 15],
        position: 1000,
        chromosome: 1,
    };

    let call = caller.call_snp(&pileup, b'A').expect("Should call variant");
    assert_eq!(call.genotype, Genotype::HomAlt);
    assert_eq!(call.alt_allele, b'G');
    assert_eq!(call.ref_allele, b'A');
    assert!(call.quality > 20.0);
}

#[test]
fn test_variant_calling_heterozygous_snp() {
    let caller = VariantCaller::new(VariantCallerConfig::default());

    let mut bases = vec![b'A'; 10];
    bases.extend(vec![b'G'; 10]);

    let pileup = PileupColumn {
        bases,
        qualities: vec![40; 20],
        position: 2000,
        chromosome: 1,
    };

    let call = caller.call_snp(&pileup, b'A').expect("Should call variant");
    assert_eq!(call.genotype, Genotype::Het);
    assert_eq!(call.alt_allele, b'G');
    assert!(call.quality > 20.0);
}

#[test]
fn test_variant_calling_no_variant() {
    let caller = VariantCaller::new(VariantCallerConfig::default());

    let pileup = PileupColumn {
        bases: vec![b'A'; 20],
        qualities: vec![40; 20],
        position: 3000,
        chromosome: 1,
    };

    let call = caller.call_snp(&pileup, b'A');
    if let Some(c) = call {
        assert_eq!(c.ref_allele, b'A');
        assert!((c.allele_depth as f32 / c.depth as f32) < 0.2);
    }
}

#[test]
fn test_variant_quality_filtering() {
    let mut config = VariantCallerConfig::default();
    config.min_quality = 30;
    config.min_depth = 10;
    let caller = VariantCaller::new(config);

    let mut calls = vec![
        VariantCall {
            chromosome: 1,
            position: 1000,
            ref_allele: b'A',
            alt_allele: b'G',
            quality: 35.0,
            genotype: Genotype::Het,
            depth: 20,
            allele_depth: 10,
            filter_status: FilterStatus::Pass,
        },
        VariantCall {
            chromosome: 1,
            position: 2000,
            ref_allele: b'C',
            alt_allele: b'T',
            quality: 25.0,
            genotype: Genotype::Het,
            depth: 20,
            allele_depth: 10,
            filter_status: FilterStatus::Pass,
        },
        VariantCall {
            chromosome: 1,
            position: 3000,
            ref_allele: b'G',
            alt_allele: b'A',
            quality: 40.0,
            genotype: Genotype::Het,
            depth: 5,
            allele_depth: 2,
            filter_status: FilterStatus::Pass,
        },
    ];

    caller.filter_variants(&mut calls);
    assert_eq!(calls[0].filter_status, FilterStatus::Pass);
    assert_eq!(calls[1].filter_status, FilterStatus::LowQuality);
    assert_eq!(calls[2].filter_status, FilterStatus::LowDepth);
}

// ============================================================================
// PROTEIN TRANSLATION TESTS
// ============================================================================

#[test]
fn test_protein_translation() {
    use ::rvdna::protein::{translate_dna, AminoAcid};
    let proteins = translate_dna(b"ATGGCAGGT");
    assert_eq!(proteins.len(), 3);
    assert_eq!(proteins[0], AminoAcid::Met);
    assert_eq!(proteins[1], AminoAcid::Ala);
    assert_eq!(proteins[2], AminoAcid::Gly);
}

#[test]
fn test_protein_translation_stop_codon() {
    use ::rvdna::protein::{translate_dna, AminoAcid};
    let p1 = translate_dna(b"ATGGCATAA");
    assert_eq!(p1.len(), 2);
    assert_eq!(p1[0], AminoAcid::Met);

    let p2 = translate_dna(b"ATGGCATAG");
    assert_eq!(p2.len(), 2);

    let p3 = translate_dna(b"ATGGCATGA");
    assert_eq!(p3.len(), 2);
}

#[test]
fn test_amino_acid_hydrophobicity() {
    use ::rvdna::protein::AminoAcid;
    assert_eq!(AminoAcid::Ile.hydrophobicity(), 4.5);
    assert_eq!(AminoAcid::Arg.hydrophobicity(), -4.5);
    assert_eq!(AminoAcid::Val.hydrophobicity(), 4.2);
    assert_eq!(AminoAcid::Lys.hydrophobicity(), -3.9);
    assert_eq!(AminoAcid::Gly.hydrophobicity(), -0.4);
}

// ============================================================================
// EPIGENETICS TESTS
// ============================================================================

#[test]
fn test_methylation_profile_creation() {
    let positions = vec![(1, 1000), (1, 2000), (2, 3000), (2, 4000)];
    let betas = vec![0.1, 0.5, 0.8, 0.3];
    let profile = MethylationProfile::from_beta_values(positions, betas);
    assert_eq!(profile.sites.len(), 4);
    let mean = profile.mean_methylation();
    assert!((mean - 0.425).abs() < 0.001);
}

#[test]
fn test_horvath_clock_prediction() {
    let clock = HorvathClock::default_clock();
    let positions: Vec<(u8, u64)> = (0..700).map(|i| (1, i * 1000)).collect();
    let betas: Vec<f32> = (0..700)
        .map(|i| {
            if i < 100 {
                0.3
            } else if i < 200 {
                0.7
            } else {
                0.5
            }
        })
        .collect();
    let profile = MethylationProfile::from_beta_values(positions, betas);
    let predicted_age = clock.predict_age(&profile);
    assert!(predicted_age > 0.0);
    assert!(predicted_age < 150.0);
}

// ============================================================================
// PHARMACOGENOMICS TESTS
// ============================================================================

#[test]
fn test_pharma_star_allele_calling() {
    assert_eq!(call_star_allele(&[]), StarAllele::Star1);
    assert_eq!(
        call_star_allele(&[(42130692, b'G', b'A')]),
        StarAllele::Star4
    );
    assert_eq!(
        call_star_allele(&[(42126611, b'T', b'-')]),
        StarAllele::Star5
    );
}

#[test]
fn test_pharma_metabolizer_phenotype() {
    assert_eq!(
        predict_phenotype(&StarAllele::Star1, &StarAllele::Star1),
        MetabolizerPhenotype::Normal
    );
    assert_eq!(
        predict_phenotype(&StarAllele::Star1, &StarAllele::Star4),
        MetabolizerPhenotype::Normal
    );
    assert_eq!(
        predict_phenotype(&StarAllele::Star4, &StarAllele::Star4),
        MetabolizerPhenotype::Poor
    );
}

// ============================================================================
// ALIGNMENT TESTS
// ============================================================================

#[test]
fn test_smith_waterman_alignment() {
    let aligner = SmithWaterman::new(AlignmentConfig::default());
    let query = DnaSequence::from_str("ACGT").unwrap();
    let reference = DnaSequence::from_str("ACGT").unwrap();
    let result = aligner.align(&query, &reference).unwrap();
    assert_eq!(result.score, 8); // 4 matches * 2 points each
}

#[test]
fn test_attention_alignment() {
    let query = DnaSequence::from_str("ATCGATCG").unwrap();
    let reference = DnaSequence::from_str("TTTTATCGATCGTTTT").unwrap();
    let alignment = query.align_with_attention(&reference).unwrap();
    assert!(alignment.score > 0);
}

// ============================================================================
// FULL PIPELINE INTEGRATION
// ============================================================================

#[test]
fn test_pipeline_config_defaults() {
    let config = AnalysisConfig::default();
    assert_eq!(config.kmer_size, 11);
    assert_eq!(config.vector_dims, 512);
    assert_eq!(config.min_quality, 20);
    assert!(config.parameters.is_empty());
}

#[test]
fn test_full_pipeline_runs() {
    // 1. Create and manipulate DNA
    let dna_seq = DnaSequence::from_str("ATGCGATCGATCGATCGATCGTAGCTAGCTAGC").unwrap();
    let rev_comp = dna_seq.reverse_complement();
    assert_eq!(rev_comp.len(), dna_seq.len());

    // 2. K-mer vector
    let kmer_vec = dna_seq.to_kmer_vector(11, 512).unwrap();
    assert_eq!(kmer_vec.len(), 512);

    // 3. Variant calling
    let caller = VariantCaller::new(VariantCallerConfig::default());
    let pileup = PileupColumn {
        bases: vec![b'A', b'A', b'G', b'G', b'G', b'G', b'G', b'G', b'G', b'G'],
        qualities: vec![40; 10],
        position: 1000,
        chromosome: 1,
    };
    assert!(caller.call_snp(&pileup, b'A').is_some());

    // 4. Protein translation
    let proteins = translate_dna(b"ATGGCAGGTAAACCC");
    assert!(!proteins.is_empty());

    // 5. Methylation + Horvath
    let profile = MethylationProfile::from_beta_values(
        vec![(1, 1000), (1, 2000), (1, 3000)],
        vec![0.3, 0.5, 0.7],
    );
    let age = HorvathClock::default_clock().predict_age(&profile);
    assert!(age > 0.0);

    // 6. Pharmacogenomics
    let allele = call_star_allele(&[(42130692, b'G', b'A')]);
    assert_eq!(allele, StarAllele::Star4);
    let phenotype = predict_phenotype(&allele, &StarAllele::Star1);
    assert_eq!(phenotype, MetabolizerPhenotype::Normal);

    // 7. Alignment
    let alignment = dna_seq.align_with_attention(&rev_comp).unwrap();
    assert!(alignment.score > 0);

    // 8. Protein contact graph
    let protein = ProteinSequence::new(vec![
        ProteinResidue::A,
        ProteinResidue::V,
        ProteinResidue::L,
        ProteinResidue::I,
        ProteinResidue::F,
        ProteinResidue::G,
        ProteinResidue::K,
        ProteinResidue::D,
        ProteinResidue::E,
        ProteinResidue::R,
        ProteinResidue::M,
        ProteinResidue::N,
    ]);
    let graph = protein.build_contact_graph(8.0).unwrap();
    let contacts = protein.predict_contacts(&graph).unwrap();
    assert!(!contacts.is_empty());
}
