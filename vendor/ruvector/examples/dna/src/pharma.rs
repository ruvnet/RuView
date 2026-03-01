//! Pharmacogenomics module
//!
//! Provides CYP enzyme star allele calling and metabolizer phenotype
//! prediction for pharmacogenomic analysis.

use serde::{Deserialize, Serialize};

/// CYP2D6 star allele classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StarAllele {
    /// *1 - Normal function (wild-type)
    Star1,
    /// *2 - Normal function
    Star2,
    /// *3 - No function (frameshift)
    Star3,
    /// *4 - No function (splicing defect)
    Star4,
    /// *5 - No function (gene deletion)
    Star5,
    /// *6 - No function (frameshift)
    Star6,
    /// *10 - Decreased function
    Star10,
    /// *17 - Decreased function
    Star17,
    /// *41 - Decreased function
    Star41,
    /// Unknown allele
    Unknown,
}

impl StarAllele {
    /// Get the activity score for this allele
    pub fn activity_score(&self) -> f64 {
        match self {
            StarAllele::Star1 | StarAllele::Star2 => 1.0,
            StarAllele::Star10 | StarAllele::Star17 | StarAllele::Star41 => 0.5,
            StarAllele::Star3 | StarAllele::Star4 | StarAllele::Star5 | StarAllele::Star6 => 0.0,
            StarAllele::Unknown => 0.5,
        }
    }
}

/// Drug metabolizer phenotype
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetabolizerPhenotype {
    /// Ultra-rapid metabolizer (activity score > 2.0)
    UltraRapid,
    /// Normal metabolizer (1.0 <= activity score <= 2.0)
    Normal,
    /// Intermediate metabolizer (0.5 <= activity score < 1.0)
    Intermediate,
    /// Poor metabolizer (activity score < 0.5)
    Poor,
}

/// Pharmacogenomic variant for a specific gene
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PharmaVariant {
    /// Gene name (e.g., "CYP2D6")
    pub gene: String,
    /// Genomic position
    pub position: u64,
    /// Reference allele
    pub ref_allele: u8,
    /// Alternate allele
    pub alt_allele: u8,
    /// Clinical significance
    pub significance: String,
}

/// CYP2C19 star allele classification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cyp2c19Allele {
    /// *1 - Normal function (wild-type)
    Star1,
    /// *2 - No function (rs4244285, c.681G>A, splicing defect)
    Star2,
    /// *3 - No function (rs4986893, c.636G>A, premature stop)
    Star3,
    /// *17 - Increased function (rs12248560, c.-806C>T)
    Star17,
    /// Unknown allele
    Unknown,
}

impl Cyp2c19Allele {
    /// Get the activity score for this allele (CPIC guidelines)
    pub fn activity_score(&self) -> f64 {
        match self {
            Cyp2c19Allele::Star1 => 1.0,
            Cyp2c19Allele::Star17 => 1.5, // Increased function
            Cyp2c19Allele::Star2 | Cyp2c19Allele::Star3 => 0.0,
            Cyp2c19Allele::Unknown => 0.5,
        }
    }
}

/// Call CYP2C19 star allele from observed variants
pub fn call_cyp2c19_allele(variants: &[(u64, u8, u8)]) -> Cyp2c19Allele {
    for &(pos, ref_allele, alt_allele) in variants {
        match (pos, ref_allele, alt_allele) {
            // *2: G>A at rs4244285 (c.681G>A, splicing defect)
            (96541616, b'G', b'A') => return Cyp2c19Allele::Star2,
            // *3: G>A at rs4986893 (c.636G>A, premature stop codon)
            (96540410, b'G', b'A') => return Cyp2c19Allele::Star3,
            // *17: C>T at rs12248560 (c.-806C>T, increased expression)
            (96522463, b'C', b'T') => return Cyp2c19Allele::Star17,
            _ => {}
        }
    }
    Cyp2c19Allele::Star1
}

/// Predict CYP2C19 metabolizer phenotype from diplotype
pub fn predict_cyp2c19_phenotype(
    allele1: &Cyp2c19Allele,
    allele2: &Cyp2c19Allele,
) -> MetabolizerPhenotype {
    let total_activity = allele1.activity_score() + allele2.activity_score();
    if total_activity > 2.0 {
        MetabolizerPhenotype::UltraRapid
    } else if total_activity >= 1.0 {
        MetabolizerPhenotype::Normal
    } else if total_activity >= 0.5 {
        MetabolizerPhenotype::Intermediate
    } else {
        MetabolizerPhenotype::Poor
    }
}

/// Call CYP2D6 star allele from observed variants
///
/// Uses a simplified lookup table based on key defining variants.
pub fn call_star_allele(variants: &[(u64, u8, u8)]) -> StarAllele {
    for &(pos, ref_allele, alt_allele) in variants {
        match (pos, ref_allele, alt_allele) {
            // *4: G>A at intron 3/exon 4 boundary (rs3892097)
            (42130692, b'G', b'A') => return StarAllele::Star4,
            // *5: whole gene deletion
            (42126611, b'T', b'-') => return StarAllele::Star5,
            // *3: frameshift (A deletion at rs35742686)
            (42127941, b'A', b'-') => return StarAllele::Star3,
            // *6: T deletion at rs5030655
            (42127803, b'T', b'-') => return StarAllele::Star6,
            // *10: C>T at rs1065852
            (42126938, b'C', b'T') => return StarAllele::Star10,
            _ => {}
        }
    }

    StarAllele::Star1 // Wild-type
}

/// Predict metabolizer phenotype from diplotype (two alleles)
pub fn predict_phenotype(allele1: &StarAllele, allele2: &StarAllele) -> MetabolizerPhenotype {
    let total_activity = allele1.activity_score() + allele2.activity_score();

    if total_activity > 2.0 {
        MetabolizerPhenotype::UltraRapid
    } else if total_activity >= 1.0 {
        MetabolizerPhenotype::Normal
    } else if total_activity >= 0.5 {
        MetabolizerPhenotype::Intermediate
    } else {
        MetabolizerPhenotype::Poor
    }
}

/// Drug recommendation based on metabolizer phenotype
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrugRecommendation {
    /// Drug name
    pub drug: String,
    /// Gene involved
    pub gene: String,
    /// Recommendation text
    pub recommendation: String,
    /// Dosing adjustment factor (1.0 = standard dose)
    pub dose_factor: f64,
}

/// Get drug recommendations for a given phenotype
pub fn get_recommendations(
    gene: &str,
    phenotype: &MetabolizerPhenotype,
) -> Vec<DrugRecommendation> {
    match (gene, phenotype) {
        ("CYP2D6", MetabolizerPhenotype::Poor) => vec![
            DrugRecommendation {
                drug: "Codeine".to_string(),
                gene: gene.to_string(),
                recommendation:
                    "AVOID codeine; no conversion to morphine. Use alternative analgesic."
                        .to_string(),
                dose_factor: 0.0,
            },
            DrugRecommendation {
                drug: "Tramadol".to_string(),
                gene: gene.to_string(),
                recommendation: "AVOID tramadol; reduced efficacy. Use alternative analgesic."
                    .to_string(),
                dose_factor: 0.0,
            },
            DrugRecommendation {
                drug: "Tamoxifen".to_string(),
                gene: gene.to_string(),
                recommendation: "Consider alternative endocrine therapy (aromatase inhibitor)."
                    .to_string(),
                dose_factor: 0.0,
            },
            DrugRecommendation {
                drug: "Ondansetron".to_string(),
                gene: gene.to_string(),
                recommendation: "Use standard dose; may have increased exposure.".to_string(),
                dose_factor: 0.75,
            },
        ],
        ("CYP2D6", MetabolizerPhenotype::UltraRapid) => vec![
            DrugRecommendation {
                drug: "Codeine".to_string(),
                gene: gene.to_string(),
                recommendation:
                    "AVOID codeine; risk of fatal toxicity from ultra-rapid morphine conversion."
                        .to_string(),
                dose_factor: 0.0,
            },
            DrugRecommendation {
                drug: "Tramadol".to_string(),
                gene: gene.to_string(),
                recommendation: "AVOID tramadol; risk of respiratory depression.".to_string(),
                dose_factor: 0.0,
            },
        ],
        ("CYP2D6", MetabolizerPhenotype::Intermediate) => vec![
            DrugRecommendation {
                drug: "Codeine".to_string(),
                gene: gene.to_string(),
                recommendation: "Use lower dose or alternative analgesic.".to_string(),
                dose_factor: 0.5,
            },
            DrugRecommendation {
                drug: "Tamoxifen".to_string(),
                gene: gene.to_string(),
                recommendation: "Consider higher dose or alternative therapy.".to_string(),
                dose_factor: 0.75,
            },
        ],
        ("CYP2C19", MetabolizerPhenotype::Poor) => vec![
            DrugRecommendation {
                drug: "Clopidogrel (Plavix)".to_string(),
                gene: gene.to_string(),
                recommendation: "AVOID clopidogrel; use prasugrel or ticagrelor instead."
                    .to_string(),
                dose_factor: 0.0,
            },
            DrugRecommendation {
                drug: "Voriconazole".to_string(),
                gene: gene.to_string(),
                recommendation: "Reduce dose by 50%; monitor for toxicity.".to_string(),
                dose_factor: 0.5,
            },
            DrugRecommendation {
                drug: "PPIs (omeprazole)".to_string(),
                gene: gene.to_string(),
                recommendation: "Reduce dose; slower clearance increases exposure.".to_string(),
                dose_factor: 0.5,
            },
            DrugRecommendation {
                drug: "Escitalopram".to_string(),
                gene: gene.to_string(),
                recommendation: "Consider 50% dose reduction.".to_string(),
                dose_factor: 0.5,
            },
        ],
        ("CYP2C19", MetabolizerPhenotype::UltraRapid) => vec![
            DrugRecommendation {
                drug: "Clopidogrel (Plavix)".to_string(),
                gene: gene.to_string(),
                recommendation: "Standard dosing (enhanced activation is beneficial).".to_string(),
                dose_factor: 1.0,
            },
            DrugRecommendation {
                drug: "Omeprazole".to_string(),
                gene: gene.to_string(),
                recommendation: "Increase dose; rapid clearance reduces efficacy.".to_string(),
                dose_factor: 2.0,
            },
            DrugRecommendation {
                drug: "Voriconazole".to_string(),
                gene: gene.to_string(),
                recommendation: "Use alternative antifungal.".to_string(),
                dose_factor: 0.0,
            },
        ],
        ("CYP2C19", MetabolizerPhenotype::Intermediate) => vec![
            DrugRecommendation {
                drug: "Clopidogrel (Plavix)".to_string(),
                gene: gene.to_string(),
                recommendation: "Consider alternative antiplatelet or increased dose.".to_string(),
                dose_factor: 1.5,
            },
            DrugRecommendation {
                drug: "PPIs (omeprazole)".to_string(),
                gene: gene.to_string(),
                recommendation:
                    "Standard dose likely adequate; may have slightly increased exposure."
                        .to_string(),
                dose_factor: 1.0,
            },
            DrugRecommendation {
                drug: "Escitalopram".to_string(),
                gene: gene.to_string(),
                recommendation: "Use standard dose; monitor response.".to_string(),
                dose_factor: 1.0,
            },
        ],
        _ => vec![DrugRecommendation {
            drug: "Standard".to_string(),
            gene: gene.to_string(),
            recommendation: "Use standard dosing".to_string(),
            dose_factor: 1.0,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_star_allele_calling() {
        // Wild-type
        assert_eq!(call_star_allele(&[]), StarAllele::Star1);

        // *4 variant
        let star4 = call_star_allele(&[(42130692, b'G', b'A')]);
        assert_eq!(star4, StarAllele::Star4);
        assert_eq!(star4.activity_score(), 0.0);

        // *10 variant (decreased function)
        let star10 = call_star_allele(&[(42126938, b'C', b'T')]);
        assert_eq!(star10, StarAllele::Star10);
        assert_eq!(star10.activity_score(), 0.5);
    }

    #[test]
    fn test_phenotype_prediction() {
        assert_eq!(
            predict_phenotype(&StarAllele::Star1, &StarAllele::Star1),
            MetabolizerPhenotype::Normal
        );
        assert_eq!(
            predict_phenotype(&StarAllele::Star1, &StarAllele::Star4),
            MetabolizerPhenotype::Normal
        );
        assert_eq!(
            predict_phenotype(&StarAllele::Star4, &StarAllele::Star10),
            MetabolizerPhenotype::Intermediate
        );
        assert_eq!(
            predict_phenotype(&StarAllele::Star4, &StarAllele::Star4),
            MetabolizerPhenotype::Poor
        );
    }

    #[test]
    fn test_drug_recommendations() {
        let recs = get_recommendations("CYP2D6", &MetabolizerPhenotype::Poor);
        assert!(recs.len() >= 1);
        assert_eq!(recs[0].dose_factor, 0.0);

        let recs_normal = get_recommendations("CYP2D6", &MetabolizerPhenotype::Normal);
        assert_eq!(recs_normal[0].dose_factor, 1.0);
    }

    #[test]
    fn test_cyp2c19_allele_calling() {
        assert_eq!(call_cyp2c19_allele(&[]), Cyp2c19Allele::Star1);

        let star2 = call_cyp2c19_allele(&[(96541616, b'G', b'A')]);
        assert_eq!(star2, Cyp2c19Allele::Star2);
        assert_eq!(star2.activity_score(), 0.0);

        let star17 = call_cyp2c19_allele(&[(96522463, b'C', b'T')]);
        assert_eq!(star17, Cyp2c19Allele::Star17);
        assert_eq!(star17.activity_score(), 1.5);
    }

    #[test]
    fn test_cyp2c19_phenotype() {
        assert_eq!(
            predict_cyp2c19_phenotype(&Cyp2c19Allele::Star17, &Cyp2c19Allele::Star17),
            MetabolizerPhenotype::UltraRapid
        );
        assert_eq!(
            predict_cyp2c19_phenotype(&Cyp2c19Allele::Star2, &Cyp2c19Allele::Star2),
            MetabolizerPhenotype::Poor
        );
        assert_eq!(
            predict_cyp2c19_phenotype(&Cyp2c19Allele::Star1, &Cyp2c19Allele::Star2),
            MetabolizerPhenotype::Normal
        );
    }

    #[test]
    fn test_cyp2c19_drug_recommendations() {
        let recs = get_recommendations("CYP2C19", &MetabolizerPhenotype::Poor);
        assert!(recs.len() >= 1);
        assert_eq!(recs[0].drug, "Clopidogrel (Plavix)");
        assert_eq!(recs[0].dose_factor, 0.0);

        let recs_ultra = get_recommendations("CYP2C19", &MetabolizerPhenotype::UltraRapid);
        assert!(recs_ultra.len() >= 2);
    }
}
