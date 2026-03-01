//! Protein translation and amino acid analysis module
//!
//! Provides DNA to protein translation using the standard genetic code,
//! and amino acid property calculations.

use serde::{Deserialize, Serialize};

/// Amino acid representation with full names
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AminoAcid {
    /// Alanine
    Ala,
    /// Arginine
    Arg,
    /// Asparagine
    Asn,
    /// Aspartic acid
    Asp,
    /// Cysteine
    Cys,
    /// Glutamic acid
    Glu,
    /// Glutamine
    Gln,
    /// Glycine
    Gly,
    /// Histidine
    His,
    /// Isoleucine
    Ile,
    /// Leucine
    Leu,
    /// Lysine
    Lys,
    /// Methionine (start codon)
    Met,
    /// Phenylalanine
    Phe,
    /// Proline
    Pro,
    /// Serine
    Ser,
    /// Threonine
    Thr,
    /// Tryptophan
    Trp,
    /// Tyrosine
    Tyr,
    /// Valine
    Val,
    /// Stop codon
    Stop,
}

impl AminoAcid {
    /// Get single-letter code
    pub fn to_char(&self) -> char {
        match self {
            AminoAcid::Ala => 'A',
            AminoAcid::Arg => 'R',
            AminoAcid::Asn => 'N',
            AminoAcid::Asp => 'D',
            AminoAcid::Cys => 'C',
            AminoAcid::Glu => 'E',
            AminoAcid::Gln => 'Q',
            AminoAcid::Gly => 'G',
            AminoAcid::His => 'H',
            AminoAcid::Ile => 'I',
            AminoAcid::Leu => 'L',
            AminoAcid::Lys => 'K',
            AminoAcid::Met => 'M',
            AminoAcid::Phe => 'F',
            AminoAcid::Pro => 'P',
            AminoAcid::Ser => 'S',
            AminoAcid::Thr => 'T',
            AminoAcid::Trp => 'W',
            AminoAcid::Tyr => 'Y',
            AminoAcid::Val => 'V',
            AminoAcid::Stop => '*',
        }
    }

    /// Get Kyte-Doolittle hydrophobicity value
    pub fn hydrophobicity(&self) -> f32 {
        match self {
            AminoAcid::Ile => 4.5,
            AminoAcid::Val => 4.2,
            AminoAcid::Leu => 3.8,
            AminoAcid::Phe => 2.8,
            AminoAcid::Cys => 2.5,
            AminoAcid::Met => 1.9,
            AminoAcid::Ala => 1.8,
            AminoAcid::Gly => -0.4,
            AminoAcid::Thr => -0.7,
            AminoAcid::Ser => -0.8,
            AminoAcid::Trp => -0.9,
            AminoAcid::Tyr => -1.3,
            AminoAcid::Pro => -1.6,
            AminoAcid::His => -3.2,
            AminoAcid::Glu => -3.5,
            AminoAcid::Gln => -3.5,
            AminoAcid::Asp => -3.5,
            AminoAcid::Asn => -3.5,
            AminoAcid::Lys => -3.9,
            AminoAcid::Arg => -4.5,
            AminoAcid::Stop => 0.0,
        }
    }

    /// Get average molecular weight in Daltons (monoisotopic)
    pub fn molecular_weight(&self) -> f64 {
        match self {
            AminoAcid::Ala => 71.03711,
            AminoAcid::Arg => 156.10111,
            AminoAcid::Asn => 114.04293,
            AminoAcid::Asp => 115.02694,
            AminoAcid::Cys => 103.00919,
            AminoAcid::Glu => 129.04259,
            AminoAcid::Gln => 128.05858,
            AminoAcid::Gly => 57.02146,
            AminoAcid::His => 137.05891,
            AminoAcid::Ile => 113.08406,
            AminoAcid::Leu => 113.08406,
            AminoAcid::Lys => 128.09496,
            AminoAcid::Met => 131.04049,
            AminoAcid::Phe => 147.06841,
            AminoAcid::Pro => 97.05276,
            AminoAcid::Ser => 87.03203,
            AminoAcid::Thr => 101.04768,
            AminoAcid::Trp => 186.07931,
            AminoAcid::Tyr => 163.06333,
            AminoAcid::Val => 99.06841,
            AminoAcid::Stop => 0.0,
        }
    }

    /// Get pKa values for Henderson-Hasselbalch isoelectric point calculation
    /// Returns (pKa_amino, pKa_carboxyl, pKa_sidechain or None)
    pub fn pka_sidechain(&self) -> Option<f64> {
        match self {
            AminoAcid::Asp => Some(3.65),
            AminoAcid::Glu => Some(4.25),
            AminoAcid::His => Some(6.00),
            AminoAcid::Cys => Some(8.18),
            AminoAcid::Tyr => Some(10.07),
            AminoAcid::Lys => Some(10.53),
            AminoAcid::Arg => Some(12.48),
            _ => None,
        }
    }
}

/// Calculate total molecular weight of a protein in Daltons
///
/// Accounts for water loss from peptide bond formation.
pub fn molecular_weight(protein: &[AminoAcid]) -> f64 {
    if protein.is_empty() {
        return 0.0;
    }
    // Sum residue weights + water (18.01056 Da) - water for each peptide bond
    let residue_sum: f64 = protein.iter().map(|aa| aa.molecular_weight()).sum();
    // N-term H (1.00794) + C-term OH (17.00274) + residues - H2O per bond
    residue_sum + 18.01056 - (protein.len().saturating_sub(1) as f64 * 0.0) // Already accounted in residue weights
}

/// Estimate isoelectric point (pI) using the bisection method
///
/// pI is the pH at which the net charge of the protein is zero.
/// Uses Henderson-Hasselbalch equation with standard pKa values.
pub fn isoelectric_point(protein: &[AminoAcid]) -> f64 {
    if protein.is_empty() {
        return 7.0;
    }

    const PKA_NH2: f64 = 9.69; // N-terminal amino group
    const PKA_COOH: f64 = 2.34; // C-terminal carboxyl group

    let charge_at_ph = |ph: f64| -> f64 {
        // N-terminal positive charge
        let mut charge = 1.0 / (1.0 + 10_f64.powf(ph - PKA_NH2));
        // C-terminal negative charge
        charge -= 1.0 / (1.0 + 10_f64.powf(PKA_COOH - ph));

        for aa in protein {
            if let Some(pka) = aa.pka_sidechain() {
                match aa {
                    // Positively charged at low pH: His, Lys, Arg
                    AminoAcid::His | AminoAcid::Lys | AminoAcid::Arg => {
                        charge += 1.0 / (1.0 + 10_f64.powf(ph - pka));
                    }
                    // Negatively charged at high pH: Asp, Glu, Cys, Tyr
                    _ => {
                        charge -= 1.0 / (1.0 + 10_f64.powf(pka - ph));
                    }
                }
            }
        }
        charge
    };

    // Bisection method to find pH where charge = 0
    let mut low = 0.0_f64;
    let mut high = 14.0_f64;

    for _ in 0..100 {
        let mid = (low + high) / 2.0;
        let charge = charge_at_ph(mid);
        if charge > 0.0 {
            low = mid;
        } else {
            high = mid;
        }
    }

    (low + high) / 2.0
}

/// Translate a DNA sequence to a vector of amino acids using the standard genetic code.
///
/// Translation proceeds in triplets (codons) from the start of the sequence.
/// Stop codons (TAA, TAG, TGA) terminate translation.
/// Incomplete codons at the end are ignored.
pub fn translate_dna(dna: &[u8]) -> Vec<AminoAcid> {
    let mut proteins = Vec::new();

    for chunk in dna.chunks(3) {
        if chunk.len() < 3 {
            break;
        }

        let codon = [
            chunk[0].to_ascii_uppercase(),
            chunk[1].to_ascii_uppercase(),
            chunk[2].to_ascii_uppercase(),
        ];

        let aa = match &codon {
            b"ATG" => AminoAcid::Met,
            b"TGG" => AminoAcid::Trp,
            b"TTT" | b"TTC" => AminoAcid::Phe,
            b"TTA" | b"TTG" | b"CTT" | b"CTC" | b"CTA" | b"CTG" => AminoAcid::Leu,
            b"ATT" | b"ATC" | b"ATA" => AminoAcid::Ile,
            b"GTT" | b"GTC" | b"GTA" | b"GTG" => AminoAcid::Val,
            b"TCT" | b"TCC" | b"TCA" | b"TCG" | b"AGT" | b"AGC" => AminoAcid::Ser,
            b"CCT" | b"CCC" | b"CCA" | b"CCG" => AminoAcid::Pro,
            b"ACT" | b"ACC" | b"ACA" | b"ACG" => AminoAcid::Thr,
            b"GCT" | b"GCC" | b"GCA" | b"GCG" => AminoAcid::Ala,
            b"TAT" | b"TAC" => AminoAcid::Tyr,
            b"CAT" | b"CAC" => AminoAcid::His,
            b"CAA" | b"CAG" => AminoAcid::Gln,
            b"AAT" | b"AAC" => AminoAcid::Asn,
            b"AAA" | b"AAG" => AminoAcid::Lys,
            b"GAT" | b"GAC" => AminoAcid::Asp,
            b"GAA" | b"GAG" => AminoAcid::Glu,
            b"TGT" | b"TGC" => AminoAcid::Cys,
            b"CGT" | b"CGC" | b"CGA" | b"CGG" | b"AGA" | b"AGG" => AminoAcid::Arg,
            b"GGT" | b"GGC" | b"GGA" | b"GGG" => AminoAcid::Gly,
            b"TAA" | b"TAG" | b"TGA" => break, // Stop codons
            _ => continue,                     // Unknown codon, skip
        };

        proteins.push(aa);
    }

    proteins
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_basic() {
        let dna = b"ATGGCAGGT";
        let result = translate_dna(dna);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], AminoAcid::Met);
        assert_eq!(result[1], AminoAcid::Ala);
        assert_eq!(result[2], AminoAcid::Gly);
    }

    #[test]
    fn test_translate_stop_codon() {
        let dna = b"ATGGCATAA"; // Met-Ala-Stop
        let result = translate_dna(dna);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_hydrophobicity() {
        assert_eq!(AminoAcid::Ile.hydrophobicity(), 4.5);
        assert_eq!(AminoAcid::Arg.hydrophobicity(), -4.5);
    }

    #[test]
    fn test_molecular_weight() {
        let protein = vec![AminoAcid::Met, AminoAcid::Ala, AminoAcid::Gly];
        let mw = molecular_weight(&protein);
        // Met (131.04) + Ala (71.04) + Gly (57.02) + H2O (18.01) = ~277.11
        assert!(mw > 270.0 && mw < 290.0, "MW should be ~277: got {}", mw);
    }

    #[test]
    fn test_isoelectric_point() {
        // Hemoglobin beta N-terminus MVHLTPEEK has pI around 6.7
        let hbb_start = translate_dna(b"ATGGTGCATCTGACTCCTGAGGAGAAG");
        let pi = isoelectric_point(&hbb_start);
        assert!(pi > 4.0 && pi < 10.0, "pI should be reasonable: got {}", pi);

        // Lysine-rich peptide should have high pI
        let basic = vec![
            AminoAcid::Lys,
            AminoAcid::Lys,
            AminoAcid::Lys,
            AminoAcid::Arg,
        ];
        let pi_basic = isoelectric_point(&basic);
        assert!(
            pi_basic > 9.0,
            "Basic peptide pI should be >9: got {}",
            pi_basic
        );

        // Aspartate-rich peptide should have low pI
        let acidic = vec![
            AminoAcid::Asp,
            AminoAcid::Asp,
            AminoAcid::Glu,
            AminoAcid::Glu,
        ];
        let pi_acidic = isoelectric_point(&acidic);
        assert!(
            pi_acidic < 5.0,
            "Acidic peptide pI should be <5: got {}",
            pi_acidic
        );
    }
}
