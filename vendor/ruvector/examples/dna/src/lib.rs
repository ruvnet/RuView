//! # rvDNA — AI-Native Genomic Analysis
//!
//! Fast, accurate genomic analysis in pure Rust with WASM support.
//! Includes the `.rvdna` binary file format for storing pre-computed
//! AI features alongside raw DNA sequences.
//!
//! - **K-mer HNSW Indexing**: Sequence similarity search via vector embeddings
//! - **Smith-Waterman Alignment**: Local alignment with CIGAR and mapping quality
//! - **Bayesian Variant Calling**: SNP/indel detection with Phred quality scores
//! - **Protein Translation**: DNA-to-protein with GNN contact graph prediction
//! - **Epigenomics**: Methylation profiling and Horvath biological age clock
//! - **Pharmacogenomics**: CYP enzyme star allele calling and drug recommendations
//! - **Pipeline Orchestration**: DAG-based multi-stage execution
//! - **RVDNA Format**: AI-native binary file format with pre-computed tensors

#![warn(missing_docs)]
#![allow(clippy::all)]

pub mod alignment;
pub mod biomarker;
pub mod biomarker_stream;
pub mod epigenomics;
pub mod error;
pub mod genotyping;
pub mod health;
pub mod kmer;
pub mod kmer_pagerank;
pub mod pharma;
pub mod pipeline;
pub mod protein;
pub mod real_data;
pub mod rvdna;
pub mod types;
pub mod variant;

pub use alignment::{AlignmentConfig, SmithWaterman};
pub use epigenomics::{
    CancerSignalDetector, CancerSignalResult, CpGSite, HorvathClock, MethylationProfile,
};
pub use error::{DnaError, Result};
pub use pharma::{
    call_cyp2c19_allele, call_star_allele, get_recommendations, predict_cyp2c19_phenotype,
    predict_phenotype, Cyp2c19Allele, DrugRecommendation, MetabolizerPhenotype, PharmaVariant,
    StarAllele,
};
pub use protein::{isoelectric_point, molecular_weight, translate_dna, AminoAcid};
pub use rvdna::{
    decode_2bit, encode_2bit, fasta_to_rvdna, Codec, KmerVectorBlock, RvdnaHeader, RvdnaReader,
    RvdnaStats, RvdnaWriter, SparseAttention, VariantTensor,
};
pub use types::{
    AlignmentResult, AnalysisConfig, CigarOp, ContactGraph, DnaSequence, GenomicPosition,
    KmerIndex, Nucleotide, ProteinResidue, ProteinSequence, QualityScore, Variant,
};
pub use variant::{
    FilterStatus, Genotype, PileupColumn, VariantCall, VariantCaller, VariantCallerConfig,
};

pub use ruvector_core::{
    types::{DbOptions, DistanceMetric, HnswConfig, SearchQuery, SearchResult, VectorEntry},
    VectorDB,
};

pub use biomarker::{BiomarkerClassification, BiomarkerProfile, BiomarkerReference, CategoryScore};
pub use biomarker_stream::{
    BiomarkerReading, RingBuffer, StreamConfig, StreamProcessor, StreamStats,
};
pub use genotyping::{
    CallConfidence, CypDiplotype, GenomeBuild, GenotypeAnalysis, GenotypeData, Snp,
};
pub use health::{ApoeResult, HealthVariantResult, MthfrResult, PainProfile};
pub use kmer_pagerank::{KmerGraphRanker, SequenceRank};

/// Prelude module for common imports
pub mod prelude {
    pub use crate::alignment::*;
    pub use crate::epigenomics::*;
    pub use crate::error::{DnaError, Result};
    pub use crate::kmer::*;
    pub use crate::pharma::*;
    pub use crate::protein::*;
    pub use crate::types::*;
    pub use crate::variant::*;
}
