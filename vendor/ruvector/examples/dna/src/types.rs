//! Core types for DNA analysis

use crate::error::{DnaError, Result};
use ruvector_core::{
    types::{DbOptions, DistanceMetric, HnswConfig},
    VectorDB,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// DNA nucleotide base
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Nucleotide {
    /// Adenine
    A,
    /// Cytosine
    C,
    /// Guanine
    G,
    /// Thymine
    T,
    /// Unknown/ambiguous base
    N,
}

impl Nucleotide {
    /// Get complement base (Watson-Crick pairing)
    pub fn complement(&self) -> Self {
        match self {
            Nucleotide::A => Nucleotide::T,
            Nucleotide::T => Nucleotide::A,
            Nucleotide::C => Nucleotide::G,
            Nucleotide::G => Nucleotide::C,
            Nucleotide::N => Nucleotide::N,
        }
    }

    /// Convert to u8 encoding (0-4)
    pub fn to_u8(&self) -> u8 {
        match self {
            Nucleotide::A => 0,
            Nucleotide::C => 1,
            Nucleotide::G => 2,
            Nucleotide::T => 3,
            Nucleotide::N => 4,
        }
    }

    /// Create from u8 encoding
    pub fn from_u8(val: u8) -> Result<Self> {
        match val {
            0 => Ok(Nucleotide::A),
            1 => Ok(Nucleotide::C),
            2 => Ok(Nucleotide::G),
            3 => Ok(Nucleotide::T),
            4 => Ok(Nucleotide::N),
            _ => Err(DnaError::InvalidSequence(format!(
                "Invalid nucleotide encoding: {}",
                val
            ))),
        }
    }
}

impl fmt::Display for Nucleotide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Nucleotide::A => 'A',
                Nucleotide::C => 'C',
                Nucleotide::G => 'G',
                Nucleotide::T => 'T',
                Nucleotide::N => 'N',
            }
        )
    }
}

/// DNA sequence
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnaSequence {
    bases: Vec<Nucleotide>,
}

impl DnaSequence {
    /// Create new DNA sequence from nucleotides
    pub fn new(bases: Vec<Nucleotide>) -> Self {
        Self { bases }
    }

    /// Create from string (ACGTN)
    pub fn from_str(s: &str) -> Result<Self> {
        let bases: Result<Vec<_>> = s
            .chars()
            .map(|c| match c.to_ascii_uppercase() {
                'A' => Ok(Nucleotide::A),
                'C' => Ok(Nucleotide::C),
                'G' => Ok(Nucleotide::G),
                'T' => Ok(Nucleotide::T),
                'N' => Ok(Nucleotide::N),
                _ => Err(DnaError::InvalidSequence(format!(
                    "Invalid character: {}",
                    c
                ))),
            })
            .collect();

        let bases = bases?;
        if bases.is_empty() {
            return Err(DnaError::EmptySequence);
        }
        Ok(Self { bases })
    }

    /// Get complement sequence
    pub fn complement(&self) -> Self {
        Self {
            bases: self.bases.iter().map(|b| b.complement()).collect(),
        }
    }

    /// Get reverse complement
    pub fn reverse_complement(&self) -> Self {
        Self {
            bases: self.bases.iter().rev().map(|b| b.complement()).collect(),
        }
    }

    /// Convert to k-mer frequency vector for indexing
    ///
    /// Uses rolling polynomial hash: O(1) per k-mer instead of O(k).
    pub fn to_kmer_vector(&self, k: usize, dims: usize) -> Result<Vec<f32>> {
        if k == 0 || k > 15 {
            return Err(DnaError::InvalidKmerSize(k));
        }
        if self.bases.len() < k {
            return Err(DnaError::InvalidSequence(
                "Sequence shorter than k-mer size".to_string(),
            ));
        }

        let mut vector = vec![0.0f32; dims];

        // Precompute 5^k for rolling hash removal of leading nucleotide
        let base: u64 = 5;
        let pow_k = base.pow(k as u32 - 1);

        // Compute initial hash for first k-mer
        let mut hash = self.bases[..k].iter().fold(0u64, |acc, &b| {
            acc.wrapping_mul(5).wrapping_add(b.to_u8() as u64)
        });
        vector[(hash as usize) % dims] += 1.0;

        // Rolling hash: remove leading nucleotide, add trailing
        for i in 1..=(self.bases.len() - k) {
            let old = self.bases[i - 1].to_u8() as u64;
            let new = self.bases[i + k - 1].to_u8() as u64;
            hash = hash
                .wrapping_sub(old.wrapping_mul(pow_k))
                .wrapping_mul(5)
                .wrapping_add(new);
            vector[(hash as usize) % dims] += 1.0;
        }

        // Normalize to unit vector
        let magnitude: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            let inv = 1.0 / magnitude;
            for v in &mut vector {
                *v *= inv;
            }
        }

        Ok(vector)
    }

    /// Get length
    pub fn len(&self) -> usize {
        self.bases.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.bases.is_empty()
    }

    /// Get a nucleotide at a specific index
    pub fn get(&self, index: usize) -> Option<Nucleotide> {
        self.bases.get(index).copied()
    }

    /// Get bases
    pub fn bases(&self) -> &[Nucleotide] {
        &self.bases
    }

    /// Encode as one-hot vectors (4 floats per nucleotide: A, C, G, T)
    pub fn encode_one_hot(&self) -> Vec<f32> {
        let mut result = vec![0.0f32; self.bases.len() * 4];
        for (i, base) in self.bases.iter().enumerate() {
            let offset = i * 4;
            match base {
                Nucleotide::A => result[offset] = 1.0,
                Nucleotide::C => result[offset + 1] = 1.0,
                Nucleotide::G => result[offset + 2] = 1.0,
                Nucleotide::T => result[offset + 3] = 1.0,
                Nucleotide::N => {} // all zeros for N
            }
        }
        result
    }

    /// Translate DNA sequence to protein using standard genetic code
    pub fn translate(&self) -> Result<ProteinSequence> {
        if self.bases.len() < 3 {
            return Err(DnaError::InvalidSequence(
                "Sequence too short for translation".to_string(),
            ));
        }

        let mut residues = Vec::new();
        for chunk in self.bases.chunks(3) {
            if chunk.len() < 3 {
                break;
            }
            let codon = (chunk[0], chunk[1], chunk[2]);
            let aa = match codon {
                (Nucleotide::A, Nucleotide::T, Nucleotide::G) => ProteinResidue::M, // Met (start)
                (Nucleotide::T, Nucleotide::G, Nucleotide::G) => ProteinResidue::W, // Trp
                (Nucleotide::T, Nucleotide::T, Nucleotide::T)
                | (Nucleotide::T, Nucleotide::T, Nucleotide::C) => ProteinResidue::F, // Phe
                (Nucleotide::T, Nucleotide::T, Nucleotide::A)
                | (Nucleotide::T, Nucleotide::T, Nucleotide::G)
                | (Nucleotide::C, Nucleotide::T, _) => ProteinResidue::L, // Leu
                (Nucleotide::A, Nucleotide::T, Nucleotide::T)
                | (Nucleotide::A, Nucleotide::T, Nucleotide::C)
                | (Nucleotide::A, Nucleotide::T, Nucleotide::A) => ProteinResidue::I, // Ile
                (Nucleotide::G, Nucleotide::T, _) => ProteinResidue::V,             // Val
                (Nucleotide::T, Nucleotide::C, _)
                | (Nucleotide::A, Nucleotide::G, Nucleotide::T)
                | (Nucleotide::A, Nucleotide::G, Nucleotide::C) => ProteinResidue::S, // Ser
                (Nucleotide::C, Nucleotide::C, _) => ProteinResidue::P,             // Pro
                (Nucleotide::A, Nucleotide::C, _) => ProteinResidue::T,             // Thr
                (Nucleotide::G, Nucleotide::C, _) => ProteinResidue::A,             // Ala
                (Nucleotide::T, Nucleotide::A, Nucleotide::T)
                | (Nucleotide::T, Nucleotide::A, Nucleotide::C) => ProteinResidue::Y, // Tyr
                (Nucleotide::C, Nucleotide::A, Nucleotide::T)
                | (Nucleotide::C, Nucleotide::A, Nucleotide::C) => ProteinResidue::H, // His
                (Nucleotide::C, Nucleotide::A, Nucleotide::A)
                | (Nucleotide::C, Nucleotide::A, Nucleotide::G) => ProteinResidue::Q, // Gln
                (Nucleotide::A, Nucleotide::A, Nucleotide::T)
                | (Nucleotide::A, Nucleotide::A, Nucleotide::C) => ProteinResidue::N, // Asn
                (Nucleotide::A, Nucleotide::A, Nucleotide::A)
                | (Nucleotide::A, Nucleotide::A, Nucleotide::G) => ProteinResidue::K, // Lys
                (Nucleotide::G, Nucleotide::A, Nucleotide::T)
                | (Nucleotide::G, Nucleotide::A, Nucleotide::C) => ProteinResidue::D, // Asp
                (Nucleotide::G, Nucleotide::A, Nucleotide::A)
                | (Nucleotide::G, Nucleotide::A, Nucleotide::G) => ProteinResidue::E, // Glu
                (Nucleotide::T, Nucleotide::G, Nucleotide::T)
                | (Nucleotide::T, Nucleotide::G, Nucleotide::C) => ProteinResidue::C, // Cys
                (Nucleotide::C, Nucleotide::G, _)
                | (Nucleotide::A, Nucleotide::G, Nucleotide::A)
                | (Nucleotide::A, Nucleotide::G, Nucleotide::G) => ProteinResidue::R, // Arg
                (Nucleotide::G, Nucleotide::G, _) => ProteinResidue::G,             // Gly
                // Stop codons
                (Nucleotide::T, Nucleotide::A, Nucleotide::A)
                | (Nucleotide::T, Nucleotide::A, Nucleotide::G)
                | (Nucleotide::T, Nucleotide::G, Nucleotide::A) => break,
                _ => ProteinResidue::X, // Unknown
            };
            residues.push(aa);
        }

        Ok(ProteinSequence::new(residues))
    }

    /// Simple attention-based alignment against a reference sequence
    ///
    /// Uses dot-product attention between one-hot encodings to find
    /// the best alignment position.
    pub fn align_with_attention(&self, reference: &DnaSequence) -> Result<AlignmentResult> {
        if self.is_empty() || reference.is_empty() {
            return Err(DnaError::AlignmentError(
                "Cannot align empty sequences".to_string(),
            ));
        }

        let query_len = self.len();
        let ref_len = reference.len();

        // Compute dot-product attention scores at each offset
        let mut best_score = i32::MIN;
        let mut best_offset = 0;

        for offset in 0..ref_len.saturating_sub(query_len / 2) {
            let mut score: i32 = 0;
            let overlap = query_len.min(ref_len - offset);

            for i in 0..overlap {
                if self.bases[i] == reference.bases[offset + i] {
                    score += 2; // match
                } else {
                    score -= 1; // mismatch
                }
            }

            if score > best_score {
                best_score = score;
                best_offset = offset;
            }
        }

        // Build CIGAR string
        let overlap = query_len.min(ref_len.saturating_sub(best_offset));
        let mut cigar = Vec::new();
        let mut match_run = 0;

        for i in 0..overlap {
            if self.bases[i] == reference.bases[best_offset + i] {
                match_run += 1;
            } else {
                if match_run > 0 {
                    cigar.push(CigarOp::M(match_run));
                    match_run = 0;
                }
                cigar.push(CigarOp::M(1)); // mismatch also represented as M
            }
        }
        if match_run > 0 {
            cigar.push(CigarOp::M(match_run));
        }

        Ok(AlignmentResult {
            score: best_score,
            cigar,
            mapped_position: GenomicPosition {
                chromosome: 1,
                position: best_offset as u64,
                reference_allele: reference
                    .bases
                    .get(best_offset)
                    .copied()
                    .unwrap_or(Nucleotide::N),
                alternate_allele: None,
            },
            mapping_quality: QualityScore::new(
                ((best_score.max(0) as f64 / overlap.max(1) as f64) * 60.0).min(60.0) as u8,
            )
            .unwrap_or(QualityScore(0)),
        })
    }
}

impl fmt::Display for DnaSequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for base in &self.bases {
            write!(f, "{}", base)?;
        }
        Ok(())
    }
}

/// Genomic position with variant information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenomicPosition {
    /// Chromosome number (1-22, X=23, Y=24, M=25)
    pub chromosome: u8,
    /// Position on chromosome (0-based)
    pub position: u64,
    /// Reference allele
    pub reference_allele: Nucleotide,
    /// Alternate allele (if variant)
    pub alternate_allele: Option<Nucleotide>,
}

/// Quality score (Phred scale)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct QualityScore(u8);

impl QualityScore {
    /// Create new quality score (0-93, Phred+33)
    pub fn new(score: u8) -> Result<Self> {
        if score > 93 {
            return Err(DnaError::InvalidQuality(score));
        }
        Ok(Self(score))
    }

    /// Get raw score
    pub fn value(&self) -> u8 {
        self.0
    }

    /// Convert to probability of error
    pub fn to_error_probability(&self) -> f64 {
        10_f64.powf(-(self.0 as f64) / 10.0)
    }
}

/// Variant type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Variant {
    /// Single nucleotide polymorphism
    Snp {
        position: GenomicPosition,
        quality: QualityScore,
    },
    /// Insertion
    Insertion {
        position: GenomicPosition,
        inserted_bases: DnaSequence,
        quality: QualityScore,
    },
    /// Deletion
    Deletion {
        position: GenomicPosition,
        deleted_length: usize,
        quality: QualityScore,
    },
    /// Structural variant (large rearrangement)
    StructuralVariant {
        chromosome: u8,
        start: u64,
        end: u64,
        variant_type: String,
        quality: QualityScore,
    },
}

/// CIGAR operation for alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CigarOp {
    /// Match/mismatch
    M(usize),
    /// Insertion to reference
    I(usize),
    /// Deletion from reference
    D(usize),
    /// Soft clipping (clipped sequence present in SEQ)
    S(usize),
    /// Hard clipping (clipped sequence NOT present in SEQ)
    H(usize),
}

/// Alignment result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentResult {
    /// Alignment score
    pub score: i32,
    /// CIGAR string
    pub cigar: Vec<CigarOp>,
    /// Mapped position
    pub mapped_position: GenomicPosition,
    /// Mapping quality
    pub mapping_quality: QualityScore,
}

/// Protein residue (amino acid)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProteinResidue {
    A,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    K,
    L,
    M,
    N,
    P,
    Q,
    R,
    S,
    T,
    V,
    W,
    Y,
    /// Stop codon or unknown
    X,
}

impl ProteinResidue {
    /// Get single-letter code
    pub fn to_char(&self) -> char {
        match self {
            ProteinResidue::A => 'A',
            ProteinResidue::C => 'C',
            ProteinResidue::D => 'D',
            ProteinResidue::E => 'E',
            ProteinResidue::F => 'F',
            ProteinResidue::G => 'G',
            ProteinResidue::H => 'H',
            ProteinResidue::I => 'I',
            ProteinResidue::K => 'K',
            ProteinResidue::L => 'L',
            ProteinResidue::M => 'M',
            ProteinResidue::N => 'N',
            ProteinResidue::P => 'P',
            ProteinResidue::Q => 'Q',
            ProteinResidue::R => 'R',
            ProteinResidue::S => 'S',
            ProteinResidue::T => 'T',
            ProteinResidue::V => 'V',
            ProteinResidue::W => 'W',
            ProteinResidue::Y => 'Y',
            ProteinResidue::X => 'X',
        }
    }
}

/// Protein sequence
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProteinSequence {
    residues: Vec<ProteinResidue>,
}

impl ProteinSequence {
    /// Create new protein sequence
    pub fn new(residues: Vec<ProteinResidue>) -> Self {
        Self { residues }
    }

    /// Get residues
    pub fn residues(&self) -> &[ProteinResidue] {
        &self.residues
    }

    /// Get length
    pub fn len(&self) -> usize {
        self.residues.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.residues.is_empty()
    }

    /// Build a simplified contact graph based on sequence distance
    ///
    /// Residues within `distance_threshold` positions of each other
    /// are considered potential contacts (simplified from 3D distance).
    pub fn build_contact_graph(&self, distance_threshold: f32) -> Result<ContactGraph> {
        if self.residues.is_empty() {
            return Err(DnaError::InvalidSequence(
                "Cannot build contact graph for empty protein".to_string(),
            ));
        }

        let n = self.residues.len();
        let threshold = distance_threshold as usize;
        let mut edges = Vec::new();

        for i in 0..n {
            for j in (i + 4)..n {
                // Simplified: sequence separation as proxy for spatial distance
                // In real structure prediction, this would use 3D coordinates
                let seq_dist = j - i;
                if seq_dist <= threshold {
                    // Closer in sequence = higher contact probability
                    let contact_prob = 1.0 / (1.0 + (seq_dist as f32 - 4.0) / threshold as f32);
                    edges.push((i, j, contact_prob));
                }
            }
        }

        Ok(ContactGraph {
            num_residues: n,
            distance_threshold,
            edges,
        })
    }

    /// Predict contacts from a contact graph using residue properties
    ///
    /// Returns (residue_i, residue_j, confidence_score) tuples
    pub fn predict_contacts(&self, graph: &ContactGraph) -> Result<Vec<(usize, usize, f32)>> {
        let mut predictions: Vec<(usize, usize, f32)> = graph
            .edges
            .iter()
            .map(|&(i, j, base_score)| {
                // Boost score for hydrophobic-hydrophobic contacts (protein core)
                let boost = if i < self.residues.len() && j < self.residues.len() {
                    let ri = &self.residues[i];
                    let rj = &self.residues[j];
                    // Hydrophobic residues tend to be in protein core
                    let hydrophobic = |r: &ProteinResidue| {
                        matches!(
                            r,
                            ProteinResidue::A
                                | ProteinResidue::V
                                | ProteinResidue::L
                                | ProteinResidue::I
                                | ProteinResidue::F
                                | ProteinResidue::W
                                | ProteinResidue::M
                        )
                    };
                    if hydrophobic(ri) && hydrophobic(rj) {
                        1.5
                    } else {
                        1.0
                    }
                } else {
                    1.0
                };
                (i, j, (base_score * boost).min(1.0))
            })
            .collect();

        // Sort by confidence descending
        predictions.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

        Ok(predictions)
    }
}

/// Contact graph for protein structure analysis
#[derive(Debug, Clone)]
pub struct ContactGraph {
    /// Number of residues
    pub num_residues: usize,
    /// Distance threshold used
    pub distance_threshold: f32,
    /// Edges: (residue_i, residue_j, distance)
    pub edges: Vec<(usize, usize, f32)>,
}

/// K-mer index using RuVector HNSW
pub struct KmerIndex {
    db: VectorDB,
    k: usize,
    dims: usize,
}

impl KmerIndex {
    /// Create new k-mer index
    pub fn new(k: usize, dims: usize, storage_path: &str) -> Result<Self> {
        let options = DbOptions {
            dimensions: dims,
            distance_metric: DistanceMetric::Cosine,
            storage_path: storage_path.to_string(),
            hnsw_config: Some(HnswConfig {
                m: 16,
                ef_construction: 200,
                ef_search: 100,
                max_elements: 1_000_000,
            }),
            quantization: None,
        };

        let db = VectorDB::new(options)?;
        Ok(Self { db, k, dims })
    }

    /// Get underlying VectorDB
    pub fn db(&self) -> &VectorDB {
        &self.db
    }

    /// Get k-mer size
    pub fn k(&self) -> usize {
        self.k
    }

    /// Get dimensions
    pub fn dims(&self) -> usize {
        self.dims
    }
}

/// Analysis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    /// K-mer size for indexing
    pub kmer_size: usize,
    /// Vector dimensions
    pub vector_dims: usize,
    /// Minimum quality score for variants
    pub min_quality: u8,
    /// Alignment match score
    pub match_score: i32,
    /// Alignment mismatch penalty
    pub mismatch_penalty: i32,
    /// Alignment gap open penalty
    pub gap_open_penalty: i32,
    /// Alignment gap extend penalty
    pub gap_extend_penalty: i32,
    /// Additional pipeline parameters
    pub parameters: HashMap<String, serde_json::Value>,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            kmer_size: 11,
            vector_dims: 512,
            min_quality: 20,
            match_score: 2,
            mismatch_penalty: -1,
            gap_open_penalty: -3,
            gap_extend_penalty: -1,
            parameters: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nucleotide_complement() {
        assert_eq!(Nucleotide::A.complement(), Nucleotide::T);
        assert_eq!(Nucleotide::G.complement(), Nucleotide::C);
    }

    #[test]
    fn test_dna_sequence() {
        let seq = DnaSequence::from_str("ACGT").unwrap();
        assert_eq!(seq.len(), 4);
        assert_eq!(seq.to_string(), "ACGT");
    }

    #[test]
    fn test_reverse_complement() {
        let seq = DnaSequence::from_str("ACGT").unwrap();
        let rc = seq.reverse_complement();
        assert_eq!(rc.to_string(), "ACGT");
    }
}
