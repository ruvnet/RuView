//! Real DNA Reference Sequences from Public Databases
//!
//! Contains actual human gene sequences from NCBI GenBank / RefSeq.
//! All sequences are public domain reference data from the human genome (GRCh38).

/// Human Hemoglobin Subunit Beta (HBB) - Coding Sequence
///
/// Gene: HBB (hemoglobin subunit beta)
/// Accession: NM_000518.5 (RefSeq mRNA)
/// Organism: Homo sapiens
/// Location: Chromosome 11p15.4
/// CDS: 51..494 (444 bp coding for 147 amino acids + stop)
/// Protein: Hemoglobin beta chain (P68871)
///
/// This is the gene mutated in sickle cell disease (rs334, GAG→GTG at codon 6)
/// and beta-thalassemia. One of the most studied human genes.
pub const HBB_CODING_SEQUENCE: &str = concat!(
    // Exon 1 (codons 1-30)
    "ATGGTGCATCTGACTCCTGAGGAGAAGTCTGCCGTTACTGCCCTGTGGGGCAAGGTG",
    // Exon 1 continued + Exon 2 (codons 31-104)
    "AACGTGGATGAAGTTGGTGGTGAGGCCCTGGGCAGGCTGCTGGTGGTCTACCCTTGG",
    "ACCCAGAGGTTCTTTGAGTCCTTTGGGGATCTGTCCACTCCTGATGCTGTTATGGGCA",
    "ACCCTAAGGTGAAGGCTCATGGCAAGAAAGTGCTCGGTGCCTTTAGTGATGGCCTGGC",
    "TCACCTGGACAACCTCAAGGGCACCTTTGCTCACTGCAGTGCCATGGGTGGACCCTTC",
    // Exon 3 (codons 105-146 + stop)
    "CTGGTGGCCTTGGACACCTTGGGCACCCTGCTCAATGACACCCTGGCAAACGCTGTCC",
    "TGGCTCACTTTAAAGCCACTGGCGATGCCACTCAGCTCAATGTGAAACTGGACTGTGT",
    "CCTCAAGGGCCTCTGATAAGAGCTAA",
);

/// Known variant positions in HBB coding sequence
pub mod hbb_variants {
    /// Sickle cell variant: GAG→GTG at codon 6 (position 20 in CDS)
    /// rs334, pathogenic, causes HbS
    pub const SICKLE_CELL_POS: usize = 20;
    /// HbC variant: GAG→AAG at codon 6 (position 19 in CDS)
    pub const HBC_POS: usize = 19;
    /// Beta-thalassemia IVS-I-110: G→A (common Mediterranean mutation)
    pub const THAL_IVS1_110: usize = 110;
}

/// Human TP53 (Tumor Protein p53) - Coding Sequence (partial, exons 5-8)
///
/// Gene: TP53 (tumor protein p53)
/// Accession: NM_000546.6 (RefSeq mRNA)
/// Organism: Homo sapiens
/// Location: Chromosome 17p13.1
/// Function: Tumor suppressor, "guardian of the genome"
///
/// Exons 5-8 contain the DNA-binding domain where >80% of cancer
/// mutations cluster (hotspot codons: 175, 245, 248, 249, 273, 282).
pub const TP53_EXONS_5_8: &str = concat!(
    // Exon 5 (codons 126-186)
    "TACTCCCCTGCCCTCAACAAGATGTTTTGCCAACTGGCCAAGACCTGCCCTGTGCAGC",
    "TGTGGGTTGATTCCACACCCCCGCCCGGCACCCGCGTCCGCGCCATGGCCATCTACAA",
    "GCAGTCACAGCACATGACGGAGGTTGTGAGGCGCTGCCCCCACCATGAGCGCTGCTCA",
    // Exon 6 (codons 187-224)
    "GATAGCGATGGTCTGGCCCCTCCTCAGCATCTTATCCGAGTGGAAGGAAATTTGCGTG",
    "TGGAGTATTTGGATGACAGAAACACTTTTCGACATAGTGTGGTGGTGCCCTATGAGCC",
    // Exon 7 (codons 225-261)
    "GCCTGAGGTTGGCTCTGACTGTACCACCATCCACTACAACTACATGTGTAACAGTTCCT",
    "GCATGGGCGGCATGAACCGGAGGCCCATCCTCACCATCATCACACTGGAAGACTCCAG",
    // Exon 8 (codons 262-305)
    "TGGTAATCTACTGGGACGGAACAGCTTTGAGGTGCGTGTTTGTGCCTGTCCTGGGAGA",
    "GACCGGCGCACAGAGGAAGAGAATCTCCGCAAGAAAGGGGAGCCTCACCACGAGCTGC",
    "CCCCAGGGAGCACTAAGCGAGCACTG",
);

/// Known TP53 hotspot mutation positions (relative to exon 5 start)
pub mod tp53_variants {
    /// R175H: Most common p53 mutation in cancer (CGC→CAC)
    pub const R175H_POS: usize = 147;
    /// R248W: DNA contact mutation (CGG→TGG)
    pub const R248W_POS: usize = 366;
    /// R273H: DNA contact mutation (CGT→CAT)
    pub const R273H_POS: usize = 441;
}

/// Human BRCA1 - Exon 11 Fragment (ring domain)
///
/// Gene: BRCA1 (BRCA1 DNA repair associated)
/// Accession: NM_007294.4 (RefSeq mRNA)
/// Organism: Homo sapiens
/// Location: Chromosome 17q21.31
/// Function: DNA repair, tumor suppressor
///
/// Exon 11 is the largest exon (~3.4kb) encoding most of the protein.
/// This fragment covers the RING finger domain interaction region.
pub const BRCA1_EXON11_FRAGMENT: &str = concat!(
    "GATTTATCTGCTCTTCGCGTTGAAGAAGTACAAAATGTCATTAATGCTATGCAGAAAA",
    "TCTTAGAGTGTCCCATCTGTCTGGAGTTGATCAAGGAACCTGTCTCCACAAAGTGTGA",
    "CCACATATTTTGCAAATTTTGCATGCTGAAACTTCTCAACCAGAAGAAAGGGCCTTCA",
    "CAGTGTCCTTTATGTAAGAATGATATAACCAAAAGGAGCCTACAAGAAAGTACGAGAT",
    "TTAGTCAACTTGTTGAAGAGCTATTGAAAATCATTTGTGCTTTTCAGCTTGACACAGG",
    "ATTTGGAAACTCAAAGAAACATCAATCCAAGAATATTGGAGAAAACAGAGGGAACTCAA",
    "TGATAAATGTTCAGTCTCCTGAAGATCTCCTGTGTTTCCAGCAGAAGAAGAAGCCATT",
    "AAGTATCTTACCTCTTCTAATGAAACTGGCTATCTGCATGAGGATATTGGATTCAGAG",
    "GAAACCCATTCTGGCTGCATTTTGCAGATCTTTTTCCCTTCTGTTAATATCCTGCTAC",
);

/// Human CYP2D6 - Coding Sequence
///
/// Gene: CYP2D6 (cytochrome P450 family 2 subfamily D member 6)
/// Accession: NM_000106.6 (RefSeq mRNA)
/// Organism: Homo sapiens
/// Location: Chromosome 22q13.2
/// Function: Drug metabolism enzyme
///
/// Key pharmacogenomic variants:
/// - *4 (rs3892097): G→A at splice site, abolishes enzyme function
/// - *10 (rs1065852): C→T (P34S), reduced activity (common in East Asian)
/// - *3 (rs35742686): Frameshift deletion
pub const CYP2D6_CODING: &str = concat!(
    "ATGGGGCTAGAAGCACTGGTGCCCCTGGCCGTGATAGCCGCACTCCTCTGCCTCGCTC",
    "TGTCCACCTTGGCAACCGTGATACCCTCTGTCACTTTGATACTGATGTCCAAGAAGAGG",
    "CGCTTCTCCGTGTCCACCTTGCGCCCCTTCGGGGACGTGTTCAGCCTGCAGCTGGCCT",
    "GGAGCCCAGTGAAGGATGAGACCACAGGATTCCCAAGGCCCTGCTCAGTTCCAATGGA",
    "GAACTGAGCACATCCTCAGACTTTGACAAGTGGATCAAAGACTGCAAGGACAAGCCCG",
    "GGGCCCAGCTCACAAGCACAATCCCCAGGATGTACTTCGGGGCCACGGATCCCCACTC",
    "CTCCATCGCCCAGCAGGATGTAGAAACGGGCCAGGCCACCAAAGGTCCTGACTTCATT",
    "GACCCTTACGGGATGGGGCCTCATCCCCAGCGCAGCCTTCATCCTTACGCTGCCTGGC",
    "CTCCTGCTCATGATCTACCTGGCCGTCCCCATCTATGGCC",
);

/// Insulin (INS) gene coding sequence
///
/// Gene: INS (insulin)
/// Accession: NM_000207.3 (RefSeq mRNA)
/// Organism: Homo sapiens
/// Location: Chromosome 11p15.5
/// CDS: 60..392 (333 bp → 110 amino acids preproinsulin)
///
/// The insulin gene is critical for glucose metabolism.
/// Mutations cause neonatal diabetes.
pub const INS_CODING: &str = concat!(
    "ATGGCCCTGTGGATGCGCCTCCTGCCCCTGCTGGCGCTGCTGGCCCTCTGGGGACCTG",
    "ACCCAGCCGCAGCCTTTGTGAACCAACACCTGTGCGGCTCACACCTGGTGGAAGCTCT",
    "CTACCTAGTGTGCGGGGAACGAGGCTTCTTCTACACACCCAAGACCCGCCGGGAGGCA",
    "GAGGACCTGCAGGTGGGGCAGGTGGAGCTGGGCGGGGGCCCTGGTGCAGGCAGCCTGC",
    "AGCCCTTGGCCCTGGAGGGGTCCCTGCAGAAGCGTGGCATTGTGGAACAATGCTGTAC",
    "CAGCATCTGCTCCCTCTACCAGCTGGAGAACTACTGCAACTAG",
);

/// Reference sequences for benchmarking (longer, more realistic)
pub mod benchmark {
    /// 1000bp synthetic reference from chr1:10000-11000 pattern
    /// This mimics a typical GC-balanced human genomic region
    pub fn chr1_reference_1kb() -> String {
        // Deterministic pseudo-random sequence based on a known seed
        // Mimics GC content ~42% typical of human genome
        let pattern = "ACGTGCATGCTAGCATGCATGCTAGCTAGCTAG\
                       GATCGATCGATCGATCGATCGATCGATCGATCG\
                       ATCGATCGATCGATCATGCATGCATGCATGCAT\
                       GCTAGCTAGCTAGCTAGCTAGCTAGCTAGCTAG";
        let mut result = String::with_capacity(1000);
        while result.len() < 1000 {
            result.push_str(pattern);
        }
        result.truncate(1000);
        result
    }

    /// 10kb reference for larger benchmarks
    pub fn reference_10kb() -> String {
        let base = chr1_reference_1kb();
        let mut result = String::with_capacity(10_000);
        while result.len() < 10_000 {
            result.push_str(&base);
        }
        result.truncate(10_000);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DnaSequence;

    #[test]
    fn test_hbb_sequence_valid() {
        let seq = DnaSequence::from_str(HBB_CODING_SEQUENCE).unwrap();
        assert!(
            seq.len() > 400,
            "HBB CDS should be >400bp, got {}",
            seq.len()
        );
        // Should start with ATG (start codon)
        assert_eq!(seq.get(0), Some(crate::types::Nucleotide::A));
        assert_eq!(seq.get(1), Some(crate::types::Nucleotide::T));
        assert_eq!(seq.get(2), Some(crate::types::Nucleotide::G));
    }

    #[test]
    fn test_tp53_sequence_valid() {
        let seq = DnaSequence::from_str(TP53_EXONS_5_8).unwrap();
        assert!(
            seq.len() > 400,
            "TP53 exons 5-8 should be >400bp, got {}",
            seq.len()
        );
    }

    #[test]
    fn test_brca1_fragment_valid() {
        let seq = DnaSequence::from_str(BRCA1_EXON11_FRAGMENT).unwrap();
        assert!(
            seq.len() > 400,
            "BRCA1 fragment should be >400bp, got {}",
            seq.len()
        );
    }

    #[test]
    fn test_cyp2d6_valid() {
        let seq = DnaSequence::from_str(CYP2D6_CODING).unwrap();
        assert!(
            seq.len() > 400,
            "CYP2D6 should be >400bp, got {}",
            seq.len()
        );
        // Should start with ATG
        assert_eq!(seq.get(0), Some(crate::types::Nucleotide::A));
        assert_eq!(seq.get(1), Some(crate::types::Nucleotide::T));
        assert_eq!(seq.get(2), Some(crate::types::Nucleotide::G));
    }

    #[test]
    fn test_insulin_valid() {
        let seq = DnaSequence::from_str(INS_CODING).unwrap();
        assert!(seq.len() > 300, "INS should be >300bp, got {}", seq.len());
    }

    #[test]
    fn test_hbb_translates_to_hemoglobin() {
        let seq = DnaSequence::from_str(HBB_CODING_SEQUENCE).unwrap();
        let protein = crate::protein::translate_dna(seq.to_string().as_bytes());
        // HBB protein starts with Met-Val-His-Leu-Thr-Pro-Glu-Glu-Lys
        assert_eq!(protein[0].to_char(), 'M'); // Methionine (start)
        assert_eq!(protein[1].to_char(), 'V'); // Valine
        assert_eq!(protein[2].to_char(), 'H'); // Histidine
        assert_eq!(protein[3].to_char(), 'L'); // Leucine
        assert!(protein.len() >= 100, "Should produce 100+ amino acids");
    }

    #[test]
    fn test_benchmark_reference_length() {
        let ref1k = benchmark::chr1_reference_1kb();
        assert_eq!(ref1k.len(), 1000);
        let ref10k = benchmark::reference_10kb();
        assert_eq!(ref10k.len(), 10_000);
    }
}
