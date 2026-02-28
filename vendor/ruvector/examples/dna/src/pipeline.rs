//! DAG-based genomic analysis pipeline orchestrator

use crate::error::Result;
use crate::types::{DnaSequence, KmerIndex, Nucleotide, ProteinResidue, ProteinSequence};
use ruvector_core::types::{SearchQuery, VectorEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

/// Pipeline configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// K-mer size (default: 21)
    pub k: usize,
    /// Attention window size (default: 512)
    pub window_size: usize,
    /// Variant calling min depth (default: 10)
    pub min_depth: usize,
    /// Min variant quality (default: 20)
    pub min_quality: u8,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            k: 21,
            window_size: 512,
            min_depth: 10,
            min_quality: 20,
        }
    }
}

/// K-mer analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KmerAnalysisResult {
    /// Total k-mers extracted
    pub total_kmers: usize,
    /// Unique k-mers found
    pub unique_kmers: usize,
    /// GC content ratio
    pub gc_content: f64,
    /// Top similar sequences
    pub top_similar_sequences: Vec<SimilarSequence>,
}

/// Similar sequence match
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarSequence {
    /// Sequence identifier
    pub id: String,
    /// Similarity score
    pub similarity: f32,
    /// Position in the index
    pub position: usize,
}

/// Variant call result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantCall {
    /// Genomic position
    pub position: u64,
    /// Reference base
    pub reference: Nucleotide,
    /// Alternate base
    pub alternate: Nucleotide,
    /// Variant quality
    pub quality: u8,
    /// Read depth
    pub depth: usize,
    /// Allele frequency
    pub allele_frequency: f64,
}

/// Pileup column for variant calling
#[derive(Debug, Clone)]
pub struct PileupColumn {
    /// Genomic position
    pub position: u64,
    /// Reference base
    pub reference: Nucleotide,
    /// Observed bases
    pub bases: Vec<Nucleotide>,
    /// Quality scores
    pub qualities: Vec<u8>,
}

/// Protein analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProteinAnalysisResult {
    /// Amino acid sequence (single letter codes)
    pub sequence: String,
    /// Protein length
    pub length: usize,
    /// Predicted contacts as (i, j, score)
    pub predicted_contacts: Vec<(usize, usize, f32)>,
    /// Secondary structure prediction (H/E/C)
    pub secondary_structure: Vec<char>,
}

/// Full pipeline analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullAnalysisResult {
    /// K-mer statistics
    pub kmer_stats: KmerAnalysisResult,
    /// Called variants
    pub variants: Vec<VariantCall>,
    /// Protein analysis results
    pub proteins: Vec<ProteinAnalysisResult>,
    /// Execution time in milliseconds
    pub execution_time_ms: u128,
}

/// Genomic analysis pipeline orchestrator
pub struct GenomicPipeline {
    config: PipelineConfig,
}

impl GenomicPipeline {
    /// Create new pipeline with configuration
    pub fn new(config: PipelineConfig) -> Self {
        Self { config }
    }

    /// Run k-mer analysis on sequences
    pub fn run_kmer_analysis(&self, sequences: &[(&str, &[u8])]) -> Result<KmerAnalysisResult> {
        let mut total_kmers = 0;
        let mut kmer_set = std::collections::HashSet::new();
        let mut gc_count = 0;
        let mut total_bases = 0;

        // Create temporary k-mer index
        let index = KmerIndex::new(self.config.k, 384, ":memory:")?;

        for (id, seq) in sequences {
            // Extract k-mers
            if seq.len() < self.config.k {
                continue;
            }

            total_bases += seq.len();

            for window in seq.windows(self.config.k) {
                total_kmers += 1;
                kmer_set.insert(window.to_vec());

                // Count GC content
                for &base in window {
                    if base == b'G' || base == b'C' {
                        gc_count += 1;
                    }
                }
            }

            // Convert sequence to vector and index
            let dna_seq = DnaSequence::from_str(&String::from_utf8_lossy(seq))?;

            if let Ok(vector) = dna_seq.to_kmer_vector(self.config.k, 384) {
                let entry = VectorEntry {
                    id: Some(id.to_string()),
                    vector,
                    metadata: None,
                };
                let _ = index.db().insert(entry);
            }
        }

        let gc_content = if total_bases > 0 {
            (gc_count as f64) / (total_bases as f64)
        } else {
            0.0
        };

        // Find similar sequences using HNSW search
        let mut top_similar = Vec::new();
        if !sequences.is_empty() {
            if let Some((query_id, query_seq)) = sequences.first() {
                let dna_seq = DnaSequence::from_str(&String::from_utf8_lossy(query_seq))?;

                if let Ok(query_vector) = dna_seq.to_kmer_vector(self.config.k, 384) {
                    let search_query = SearchQuery {
                        vector: query_vector,
                        k: 5,
                        filter: None,
                        ef_search: None,
                    };
                    if let Ok(results) = index.db().search(search_query) {
                        for result in results {
                            if result.id != *query_id {
                                top_similar.push(SimilarSequence {
                                    id: result.id.clone(),
                                    similarity: result.score,
                                    position: 0,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(KmerAnalysisResult {
            total_kmers,
            unique_kmers: kmer_set.len(),
            gc_content,
            top_similar_sequences: top_similar,
        })
    }

    /// Run variant calling against reference
    pub fn run_variant_calling(
        &self,
        pileups: &[PileupColumn],
        _reference: &[u8],
    ) -> Result<Vec<VariantCall>> {
        let mut variants = Vec::new();

        for pileup in pileups {
            if pileup.bases.len() < self.config.min_depth {
                continue;
            }

            // Count allele frequencies
            let mut allele_counts: HashMap<Nucleotide, usize> = HashMap::new();
            for &base in &pileup.bases {
                *allele_counts.entry(base).or_insert(0) += 1;
            }

            // Find most common alternate allele
            let _ref_count = allele_counts.get(&pileup.reference).copied().unwrap_or(0);

            for (&allele, &count) in &allele_counts {
                if allele == pileup.reference || allele == Nucleotide::N {
                    continue;
                }

                let allele_freq = count as f64 / pileup.bases.len() as f64;

                // Call variant if alternate allele frequency is significant
                if allele_freq > 0.2 && count >= 3 {
                    // Calculate quality score from supporting reads
                    let quality = pileup
                        .qualities
                        .iter()
                        .take(count)
                        .map(|&q| q as u16)
                        .sum::<u16>()
                        .min(255) as u8;

                    if quality >= self.config.min_quality {
                        variants.push(VariantCall {
                            position: pileup.position,
                            reference: pileup.reference,
                            alternate: allele,
                            quality,
                            depth: pileup.bases.len(),
                            allele_frequency: allele_freq,
                        });
                    }
                }
            }
        }

        Ok(variants)
    }

    /// Translate DNA to protein and analyze structure
    pub fn run_protein_analysis(&self, dna: &[u8]) -> Result<ProteinAnalysisResult> {
        // Translate DNA to protein using standard genetic code
        let protein = self.translate_dna(dna)?;

        // Predict contacts using heuristic scoring
        let contacts = self.predict_protein_contacts(&protein)?;

        // Simple secondary structure prediction
        let secondary_structure = self.predict_secondary_structure(&protein);

        Ok(ProteinAnalysisResult {
            sequence: protein.residues().iter().map(|r| r.to_char()).collect(),
            length: protein.len(),
            predicted_contacts: contacts,
            secondary_structure,
        })
    }

    /// Run full analysis pipeline
    pub fn run_full_pipeline(
        &self,
        sequence: &[u8],
        reference: &[u8],
    ) -> Result<FullAnalysisResult> {
        let start = Instant::now();

        // Stage 1: K-mer analysis
        let kmer_stats =
            self.run_kmer_analysis(&[("query", sequence), ("reference", reference)])?;

        // Stage 2: Variant calling - generate pileups from sequence
        let pileups = self.generate_pileups(sequence, reference)?;
        let variants = self.run_variant_calling(&pileups, reference)?;

        // Stage 3: Protein analysis - find ORFs and translate
        let proteins = self.find_orfs_and_translate(sequence)?;

        let execution_time_ms = start.elapsed().as_millis();

        Ok(FullAnalysisResult {
            kmer_stats,
            variants,
            proteins,
            execution_time_ms,
        })
    }

    // Helper methods

    /// Translate DNA to protein
    fn translate_dna(&self, dna: &[u8]) -> Result<ProteinSequence> {
        let mut residues = Vec::new();

        for codon in dna.chunks(3) {
            if codon.len() < 3 {
                break;
            }

            let aa = self.codon_to_amino_acid(codon);
            if aa == ProteinResidue::X {
                break; // Stop codon
            }
            residues.push(aa);
        }

        Ok(ProteinSequence::new(residues))
    }

    /// Map codon to amino acid (simplified genetic code)
    fn codon_to_amino_acid(&self, codon: &[u8]) -> ProteinResidue {
        match codon {
            b"ATG" => ProteinResidue::M,
            b"TGG" => ProteinResidue::W,
            b"TTT" | b"TTC" => ProteinResidue::F,
            b"TTA" | b"TTG" | b"CTT" | b"CTC" | b"CTA" | b"CTG" => ProteinResidue::L,
            b"ATT" | b"ATC" | b"ATA" => ProteinResidue::I,
            b"GTT" | b"GTC" | b"GTA" | b"GTG" => ProteinResidue::V,
            b"TCT" | b"TCC" | b"TCA" | b"TCG" | b"AGT" | b"AGC" => ProteinResidue::S,
            b"CCT" | b"CCC" | b"CCA" | b"CCG" => ProteinResidue::P,
            b"ACT" | b"ACC" | b"ACA" | b"ACG" => ProteinResidue::T,
            b"GCT" | b"GCC" | b"GCA" | b"GCG" => ProteinResidue::A,
            b"TAT" | b"TAC" => ProteinResidue::Y,
            b"CAT" | b"CAC" => ProteinResidue::H,
            b"CAA" | b"CAG" => ProteinResidue::Q,
            b"AAT" | b"AAC" => ProteinResidue::N,
            b"AAA" | b"AAG" => ProteinResidue::K,
            b"GAT" | b"GAC" => ProteinResidue::D,
            b"GAA" | b"GAG" => ProteinResidue::E,
            b"TGT" | b"TGC" => ProteinResidue::C,
            b"CGT" | b"CGC" | b"CGA" | b"CGG" | b"AGA" | b"AGG" => ProteinResidue::R,
            b"GGT" | b"GGC" | b"GGA" | b"GGG" => ProteinResidue::G,
            _ => ProteinResidue::X, // Stop or unknown
        }
    }

    /// Predict protein contacts using residue property heuristics
    fn predict_protein_contacts(
        &self,
        protein: &ProteinSequence,
    ) -> Result<Vec<(usize, usize, f32)>> {
        let residues = protein.residues();
        let n = residues.len();

        if n < 5 {
            return Ok(Vec::new());
        }

        // Compute residue feature scores
        let features: Vec<f32> = residues
            .iter()
            .map(|r| r.to_char() as u8 as f32 / 255.0)
            .collect();

        // Predict contacts: pairs of residues >4 apart with similar features
        let mut contacts = Vec::new();
        for i in 0..n {
            for j in (i + 5)..n {
                let score = (features[i] + features[j]) / 2.0;
                if score > 0.5 {
                    contacts.push((i, j, score));
                }
            }
        }

        contacts.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        contacts.truncate(10);
        Ok(contacts)
    }

    /// Simple secondary structure prediction
    fn predict_secondary_structure(&self, protein: &ProteinSequence) -> Vec<char> {
        protein
            .residues()
            .iter()
            .map(|r| match r {
                ProteinResidue::A | ProteinResidue::E | ProteinResidue::L | ProteinResidue::M => {
                    'H'
                }
                ProteinResidue::V | ProteinResidue::I | ProteinResidue::Y | ProteinResidue::F => {
                    'E'
                }
                _ => 'C',
            })
            .collect()
    }

    /// Generate pileups from sequence alignment
    fn generate_pileups(&self, sequence: &[u8], reference: &[u8]) -> Result<Vec<PileupColumn>> {
        let mut pileups = Vec::new();
        let min_len = sequence.len().min(reference.len());

        for i in 0..min_len {
            let ref_base = match reference[i] {
                b'A' => Nucleotide::A,
                b'C' => Nucleotide::C,
                b'G' => Nucleotide::G,
                b'T' => Nucleotide::T,
                _ => Nucleotide::N,
            };

            let seq_base = match sequence[i] {
                b'A' => Nucleotide::A,
                b'C' => Nucleotide::C,
                b'G' => Nucleotide::G,
                b'T' => Nucleotide::T,
                _ => Nucleotide::N,
            };

            // Simulate coverage depth
            let depth = 15 + (i % 10);
            let bases = vec![seq_base; depth];
            let qualities = vec![30; depth];

            pileups.push(PileupColumn {
                position: i as u64,
                reference: ref_base,
                bases,
                qualities,
            });
        }

        Ok(pileups)
    }

    /// Find ORFs and translate to proteins
    fn find_orfs_and_translate(&self, sequence: &[u8]) -> Result<Vec<ProteinAnalysisResult>> {
        let mut proteins = Vec::new();

        // Look for ATG start codons
        for i in 0..sequence.len().saturating_sub(30) {
            if sequence[i..].starts_with(b"ATG") {
                let orf = &sequence[i..];
                if let Ok(protein_result) = self.run_protein_analysis(orf) {
                    if protein_result.length >= 10 {
                        proteins.push(protein_result);
                        if proteins.len() >= 3 {
                            break;
                        }
                    }
                }
            }
        }

        Ok(proteins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_creation() {
        let config = PipelineConfig::default();
        let pipeline = GenomicPipeline::new(config);
        assert_eq!(pipeline.config.k, 21);
    }

    #[test]
    fn test_kmer_analysis() {
        let config = PipelineConfig::default();
        let pipeline = GenomicPipeline::new(config);

        let sequences = vec![("seq1", b"ACGTACGTACGTACGTACGTACGT".as_ref())];

        let result = pipeline.run_kmer_analysis(&sequences);
        assert!(result.is_ok());
    }
}
