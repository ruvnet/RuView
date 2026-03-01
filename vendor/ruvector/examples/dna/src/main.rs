//! DNA Analyzer Demo - RuVector Genomic Analysis Pipeline
//!
//! Demonstrates SOTA genomic analysis using:
//! - Real human gene sequences (HBB, TP53, BRCA1, CYP2D6, INS)
//! - HNSW k-mer indexing for fast sequence search
//! - Attention-based sequence alignment
//! - Variant calling from pileup data
//! - Protein translation and contact prediction
//! - Epigenetic age prediction (Horvath clock)
//! - Pharmacogenomic star allele calling
//! - RVDNA AI-native file format with pre-computed tensors

use ::rvdna::prelude::*;
use ::rvdna::{
    alignment::{AlignmentConfig, SmithWaterman},
    epigenomics::{HorvathClock, MethylationProfile},
    genotyping, pharma,
    protein::translate_dna,
    real_data,
    rvdna::{
        self, Codec, KmerVectorBlock, RvdnaReader, RvdnaWriter, SparseAttention, VariantTensor,
    },
    variant::{PileupColumn, VariantCaller, VariantCallerConfig},
};
use rand::Rng;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

fn main() -> anyhow::Result<()> {
    // Check for 23andMe file argument
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        return run_23andme(&args[1]);
    }

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("RuVector DNA Analyzer - Genomic Analysis Pipeline");
    info!("================================================");
    info!("Using real human gene sequences from NCBI RefSeq");

    // -----------------------------------------------------------------------
    // Stage 1: Load real human gene sequences
    // -----------------------------------------------------------------------
    info!("\nStage 1: Loading real human gene sequences");
    let total_start = std::time::Instant::now();

    let hbb = DnaSequence::from_str(real_data::HBB_CODING_SEQUENCE)?;
    let tp53 = DnaSequence::from_str(real_data::TP53_EXONS_5_8)?;
    let brca1 = DnaSequence::from_str(real_data::BRCA1_EXON11_FRAGMENT)?;
    let cyp2d6 = DnaSequence::from_str(real_data::CYP2D6_CODING)?;
    let insulin = DnaSequence::from_str(real_data::INS_CODING)?;

    info!(
        "  HBB (hemoglobin beta):     {} bp  [chr11, sickle cell gene]",
        hbb.len()
    );
    info!(
        "  TP53 (tumor suppressor):   {} bp  [chr17, exons 5-8]",
        tp53.len()
    );
    info!(
        "  BRCA1 (DNA repair):        {} bp  [chr17, exon 11 fragment]",
        brca1.len()
    );
    info!(
        "  CYP2D6 (drug metabolism):  {} bp  [chr22, pharmacogenomic]",
        cyp2d6.len()
    );
    info!(
        "  INS (insulin):             {} bp  [chr11, preproinsulin]",
        insulin.len()
    );

    let gc_hbb = calculate_gc_content(&hbb);
    let gc_tp53 = calculate_gc_content(&tp53);
    info!("  HBB GC content: {:.1}%", gc_hbb * 100.0);
    info!("  TP53 GC content: {:.1}%", gc_tp53 * 100.0);

    // -----------------------------------------------------------------------
    // Stage 2: K-mer similarity search across gene panel
    // -----------------------------------------------------------------------
    info!("\nStage 2: K-mer similarity search across gene panel");
    let kmer_start = std::time::Instant::now();

    let hbb_vec = hbb.to_kmer_vector(11, 512)?;
    let tp53_vec = tp53.to_kmer_vector(11, 512)?;
    let brca1_vec = brca1.to_kmer_vector(11, 512)?;
    let cyp2d6_vec = cyp2d6.to_kmer_vector(11, 512)?;
    let ins_vec = insulin.to_kmer_vector(11, 512)?;

    let sim_hbb_tp53 = cosine_similarity(&hbb_vec, &tp53_vec);
    let sim_hbb_brca1 = cosine_similarity(&hbb_vec, &brca1_vec);
    let sim_tp53_brca1 = cosine_similarity(&tp53_vec, &brca1_vec);
    let sim_hbb_cyp2d6 = cosine_similarity(&hbb_vec, &cyp2d6_vec);

    info!("  K-mer similarity matrix (cosine, k=11, d=512):");
    info!("    HBB  vs TP53:  {:.4}", sim_hbb_tp53);
    info!("    HBB  vs BRCA1: {:.4}", sim_hbb_brca1);
    info!("    TP53 vs BRCA1: {:.4}", sim_tp53_brca1);
    info!("    HBB  vs CYP2D6:{:.4}", sim_hbb_cyp2d6);
    info!("  K-mer encoding time: {:?}", kmer_start.elapsed());

    // -----------------------------------------------------------------------
    // Stage 3: Align HBB query fragment against full HBB
    // -----------------------------------------------------------------------
    info!("\nStage 3: Smith-Waterman alignment on HBB");
    let align_start = std::time::Instant::now();

    // Extract a 50bp fragment from the middle of HBB (simulating a sequencing read)
    let hbb_str = hbb.to_string();
    let fragment_start = 100;
    let fragment_end = (fragment_start + 50).min(hbb_str.len());
    let query_fragment = DnaSequence::from_str(&hbb_str[fragment_start..fragment_end])?;

    let aligner = SmithWaterman::new(AlignmentConfig::default());
    let alignment = aligner.align(&query_fragment, &hbb)?;

    info!(
        "  Query: HBB[{}..{}] ({} bp read)",
        fragment_start,
        fragment_end,
        query_fragment.len()
    );
    info!("  Alignment score: {}", alignment.score);
    info!(
        "  Mapped position: {} (expected: {})",
        alignment.mapped_position.position, fragment_start
    );
    info!("  Mapping quality: {}", alignment.mapping_quality.value());
    info!("  CIGAR: {} ops", alignment.cigar.len());
    info!("  Alignment time: {:?}", align_start.elapsed());

    // -----------------------------------------------------------------------
    // Stage 4: Variant calling on HBB (sickle cell region)
    // -----------------------------------------------------------------------
    info!("\nStage 4: Variant calling on HBB (sickle cell detection)");
    let variant_start = std::time::Instant::now();

    let caller = VariantCaller::new(VariantCallerConfig::default());
    let hbb_bytes = hbb_str.as_bytes();
    let mut variant_count = 0;
    let mut rng = rand::thread_rng();

    // Simulate sequencing reads across HBB with a sickle cell mutation at position 20
    let sickle_pos = real_data::hbb_variants::SICKLE_CELL_POS;
    for i in 0..hbb_bytes.len().min(200) {
        let depth = rng.gen_range(20..51);
        let bases: Vec<u8> = (0..depth)
            .map(|_| {
                if i == sickle_pos && rng.gen::<f32>() < 0.5 {
                    b'T' // Simulate heterozygous sickle cell (A→T at codon 6)
                } else if rng.gen::<f32>() < 0.98 {
                    hbb_bytes[i]
                } else {
                    [b'A', b'C', b'G', b'T'][rng.gen_range(0..4)]
                }
            })
            .collect();
        let qualities: Vec<u8> = (0..depth).map(|_| rng.gen_range(25..41)).collect();

        let pileup = PileupColumn {
            bases,
            qualities,
            position: i as u64,
            chromosome: 11,
        };

        if let Some(call) = caller.call_snp(&pileup, hbb_bytes[i]) {
            variant_count += 1;
            if i == sickle_pos {
                info!(
                    "  ** Sickle cell variant at pos {}: ref={} alt={} depth={} qual={}",
                    i, call.ref_allele as char, call.alt_allele as char, call.depth, call.quality
                );
            }
        }
    }

    info!("  Positions analyzed: {}", hbb_bytes.len().min(200));
    info!("  Total variants detected: {}", variant_count);
    info!("  Variant calling time: {:?}", variant_start.elapsed());

    // -----------------------------------------------------------------------
    // Stage 5: Translate HBB → hemoglobin beta protein
    // -----------------------------------------------------------------------
    info!("\nStage 5: Protein translation - HBB to Hemoglobin Beta");
    let protein_start = std::time::Instant::now();

    let amino_acids = translate_dna(hbb_bytes);
    let protein_str: String = amino_acids.iter().map(|aa| aa.to_char()).collect();

    info!("  Protein length: {} amino acids", amino_acids.len());
    info!(
        "  First 20 aa: {}",
        if protein_str.len() > 20 {
            &protein_str[..20]
        } else {
            &protein_str
        }
    );
    info!("  Expected:     MVHLTPEEKSAVTALWGKVN (hemoglobin beta N-terminus)");

    // Build contact graph for the hemoglobin protein
    if amino_acids.len() >= 10 {
        let residues: Vec<ProteinResidue> = amino_acids
            .iter()
            .map(|aa| match aa.to_char() {
                'A' => ProteinResidue::A,
                'R' => ProteinResidue::R,
                'N' => ProteinResidue::N,
                'D' => ProteinResidue::D,
                'C' => ProteinResidue::C,
                'E' => ProteinResidue::E,
                'Q' => ProteinResidue::Q,
                'G' => ProteinResidue::G,
                'H' => ProteinResidue::H,
                'I' => ProteinResidue::I,
                'L' => ProteinResidue::L,
                'K' => ProteinResidue::K,
                'M' => ProteinResidue::M,
                'F' => ProteinResidue::F,
                'P' => ProteinResidue::P,
                'S' => ProteinResidue::S,
                'T' => ProteinResidue::T,
                'W' => ProteinResidue::W,
                'Y' => ProteinResidue::Y,
                'V' => ProteinResidue::V,
                _ => ProteinResidue::X,
            })
            .collect();
        let protein_seq = ProteinSequence::new(residues);
        let graph = protein_seq.build_contact_graph(8.0)?;
        let contacts = protein_seq.predict_contacts(&graph)?;

        info!("  Contact graph: {} edges", graph.edges.len());
        info!("  Top 3 predicted contacts:");
        for (i, (r1, r2, score)) in contacts.iter().take(3).enumerate() {
            info!(
                "    {}. Residues {} <-> {} (score: {:.3})",
                i + 1,
                r1,
                r2,
                score
            );
        }
    }
    info!("  Protein analysis time: {:?}", protein_start.elapsed());

    // -----------------------------------------------------------------------
    // Stage 6: Epigenetic age prediction
    // -----------------------------------------------------------------------
    info!("\nStage 6: Epigenetic age prediction (Horvath clock)");
    let epi_start = std::time::Instant::now();

    let positions: Vec<(u8, u64)> = (0..500).map(|i| (1, i * 1000)).collect();
    let betas: Vec<f32> = (0..500).map(|_| rng.gen_range(0.1..0.9)).collect();

    let profile = MethylationProfile::from_beta_values(positions, betas);
    let clock = HorvathClock::default_clock();
    let predicted_age = clock.predict_age(&profile);

    info!("  CpG sites analyzed: {}", profile.sites.len());
    info!("  Mean methylation: {:.3}", profile.mean_methylation());
    info!("  Predicted biological age: {:.1} years", predicted_age);
    info!("  Epigenomics time: {:?}", epi_start.elapsed());

    // -----------------------------------------------------------------------
    // Stage 7: Pharmacogenomics (CYP2D6 from real sequence)
    // -----------------------------------------------------------------------
    info!("\nStage 7: Pharmacogenomic analysis (CYP2D6)");

    let cyp2d6_variants = vec![(42130692, b'G', b'A')]; // *4 defining variant
    let allele1 = pharma::call_star_allele(&cyp2d6_variants);
    let allele2 = pharma::StarAllele::Star10; // *10: common in East Asian populations
    let phenotype = pharma::predict_phenotype(&allele1, &allele2);

    info!("  CYP2D6 sequence: {} bp analyzed", cyp2d6.len());
    info!(
        "  Allele 1: {:?} (activity: {:.1})",
        allele1,
        allele1.activity_score()
    );
    info!(
        "  Allele 2: {:?} (activity: {:.1})",
        allele2,
        allele2.activity_score()
    );
    info!("  Metabolizer phenotype: {:?}", phenotype);

    let recommendations = pharma::get_recommendations("CYP2D6", &phenotype);
    for rec in &recommendations {
        info!(
            "    - {}: {} (dose: {:.1}x)",
            rec.drug, rec.recommendation, rec.dose_factor
        );
    }

    // -----------------------------------------------------------------------
    // Stage 8: RVDNA AI-Native Format Demo
    // -----------------------------------------------------------------------
    info!("\nStage 8: RVDNA AI-Native File Format");
    let rvdna_start = std::time::Instant::now();

    // Convert HBB to RVDNA format with pre-computed k-mer vectors
    let rvdna_bytes = rvdna::fasta_to_rvdna(real_data::HBB_CODING_SEQUENCE, 11, 512, 500)?;

    info!("  FASTA → RVDNA conversion:");
    info!("    Input:  {} bases (ASCII, 1 byte/base)", hbb.len());
    info!("    Output: {} bytes (RVDNA binary)", rvdna_bytes.len());
    info!(
        "    Ratio:  {:.2}x compression (sequence section)",
        hbb.len() as f64 / rvdna_bytes.len() as f64
    );

    // Read back and validate
    let reader = RvdnaReader::from_bytes(rvdna_bytes)?;
    let restored = reader.read_sequence()?;
    assert_eq!(restored.to_string(), hbb.to_string(), "Lossless roundtrip");

    let kmer_blocks = reader.read_kmer_vectors()?;
    let stats = reader.stats();

    info!("  RVDNA file stats:");
    info!("    Format version: {}", reader.header.version);
    info!(
        "    Sequence section: {} bytes ({:.1} bits/base)",
        stats.section_sizes[0], stats.bits_per_base
    );
    info!(
        "    K-mer vectors: {} blocks pre-computed",
        kmer_blocks.len()
    );

    if !kmer_blocks.is_empty() {
        info!(
            "    Vector dims: {}, k={}",
            kmer_blocks[0].dimensions, kmer_blocks[0].k
        );
        // Demonstrate instant similarity search from pre-computed vectors
        let tp53_query = tp53.to_kmer_vector(11, 512)?;
        let sim = kmer_blocks[0].cosine_similarity(&tp53_query);
        info!(
            "    Instant HBB vs TP53 similarity: {:.4} (from pre-indexed)",
            sim
        );
    }

    info!("  RVDNA format time: {:?}", rvdna_start.elapsed());

    // Compare format sizes
    info!("\n  Format Comparison (HBB gene, {} bp):", hbb.len());
    info!("    FASTA (ASCII):    {} bytes (8 bits/base)", hbb.len());
    info!(
        "    RVDNA (2-bit):    {} bytes (seq section)",
        stats.section_sizes[0]
    );
    info!(
        "    RVDNA (total):    {} bytes (seq + k-mer vectors + metadata)",
        stats.total_size
    );
    info!("    Pre-computed:     k-mer vectors, ready for HNSW search");

    // -----------------------------------------------------------------------
    // Summary
    // -----------------------------------------------------------------------
    let total_time = total_start.elapsed();
    info!("\nPipeline Summary");
    info!("==================");
    info!("  Genes analyzed: 5 (HBB, TP53, BRCA1, CYP2D6, INS)");
    info!(
        "  Total bases: {} bp",
        hbb.len() + tp53.len() + brca1.len() + cyp2d6.len() + insulin.len()
    );
    info!(
        "  Variants called: {} (in HBB sickle cell region)",
        variant_count
    );
    info!("  Hemoglobin protein: {} amino acids", amino_acids.len());
    info!("  Predicted age: {:.1} years", predicted_age);
    info!("  CYP2D6 phenotype: {:?}", phenotype);
    info!(
        "  RVDNA format: {} bytes ({} sections)",
        stats.total_size,
        stats.section_sizes.iter().filter(|&&s| s > 0).count()
    );
    info!("  Total pipeline time: {:?}", total_time);

    info!("\nAnalysis complete!");

    Ok(())
}

/// Cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        0.0
    } else {
        dot / (mag_a * mag_b)
    }
}

/// Calculate GC content of DNA sequence
fn calculate_gc_content(sequence: &DnaSequence) -> f64 {
    let gc_count = sequence
        .bases()
        .iter()
        .filter(|&&b| b == Nucleotide::G || b == Nucleotide::C)
        .count();
    gc_count as f64 / sequence.len() as f64
}

/// Run 23andMe genotyping analysis pipeline
fn run_23andme(path: &str) -> anyhow::Result<()> {
    let file =
        std::fs::File::open(path).map_err(|e| anyhow::anyhow!("Cannot open {}: {}", path, e))?;
    let analysis =
        genotyping::analyze(file).map_err(|e| anyhow::anyhow!("Analysis failed: {}", e))?;
    print!("{}", genotyping::format_report(&analysis));
    Ok(())
}
