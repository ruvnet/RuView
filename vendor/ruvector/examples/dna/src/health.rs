//! Health variant analysis for genotyping data
//!
//! Clinically significant variant interpretation for 17+ health-relevant
//! SNPs commonly found in 23andMe/genotyping panels. Covers APOE, BRCA1/2,
//! TP53, MTHFR, COMT, OPRM1, CYP1A2, and more.
//!
//! Based on: <https://github.com/ericporres/rvdna-bridge>

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of analyzing a single health variant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthVariantResult {
    /// rsid identifier
    pub rsid: String,
    /// Gene name
    pub gene: String,
    /// Variant common name
    pub name: String,
    /// Observed genotype
    pub genotype: String,
    /// Risk allele
    pub risk_allele: char,
    /// Human-readable interpretation
    pub interpretation: String,
    /// Clinical significance
    pub clinical_significance: String,
}

/// APOE genotype determination result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApoeResult {
    /// Full APOE genotype string (e.g., "e2/e3")
    pub genotype: String,
    /// rs429358 genotype
    pub rs429358: String,
    /// rs7412 genotype
    pub rs7412: String,
}

/// MTHFR compound status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MthfrResult {
    /// C677T genotype (rs1801133)
    pub c677t: String,
    /// A1298C genotype (rs1801131)
    pub a1298c: String,
    /// Compound risk score (0-4)
    pub score: u8,
    /// Clinical assessment text
    pub assessment: String,
}

/// Pain sensitivity profile (COMT + OPRM1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PainProfile {
    /// COMT genotype (rs4680)
    pub comt: String,
    /// OPRM1 genotype (rs1799971)
    pub oprm1: String,
    /// Combined pain score (0-4)
    pub score: u8,
    /// Sensitivity label
    pub label: String,
    /// COMT interpretation
    pub comt_note: String,
    /// OPRM1 interpretation
    pub oprm1_note: String,
}

// ── Internal definition type ──

struct VDef {
    rsid: &'static str,
    gene: &'static str,
    name: &'static str,
    risk_allele: char,
    // (genotype, description, significance)
    interps: &'static [(&'static str, &'static str, &'static str)],
}

static HEALTH_VARIANTS: &[VDef] = &[
    // ── APOE (Alzheimer's) ──
    VDef {
        rsid: "rs429358",
        gene: "APOE",
        name: "APOE e4 determinant",
        risk_allele: 'C',
        interps: &[
            (
                "TT",
                "APOE e3/e3 or e2/e3 (depends on rs7412)",
                "Protective/Normal",
            ),
            (
                "CT",
                "One e4 allele present",
                "Increased Alzheimer's risk (~3x)",
            ),
            (
                "CC",
                "Two e4 alleles present",
                "Significantly increased Alzheimer's risk (~12x)",
            ),
        ],
    },
    VDef {
        rsid: "rs7412",
        gene: "APOE",
        name: "APOE e2 determinant",
        risk_allele: 'T',
        interps: &[
            ("CC", "No e2 allele", "Normal"),
            (
                "CT",
                "One e2 allele present",
                "Protective - reduced Alzheimer's risk",
            ),
            ("TT", "Two e2 alleles (e2/e2)", "Protective; monitor lipids"),
        ],
    },
    // ── TP53 (cancer) ──
    VDef {
        rsid: "rs1042522",
        gene: "TP53",
        name: "p53 Pro72Arg (R72P)",
        risk_allele: 'G',
        interps: &[
            (
                "CC",
                "Pro/Pro homozygous",
                "Normal apoptosis; slightly increased cancer survival",
            ),
            (
                "CG",
                "Pro/Arg heterozygous",
                "Mixed - Arg allele has stronger apoptotic activity",
            ),
            (
                "GG",
                "Arg/Arg homozygous",
                "Stronger apoptotic response; variable cancer risk",
            ),
        ],
    },
    // ── BRCA1 ──
    VDef {
        rsid: "rs80357906",
        gene: "BRCA1",
        name: "BRCA1 5382insC (Ashkenazi founder)",
        risk_allele: 'I',
        interps: &[
            (
                "DD",
                "No insertion detected",
                "Normal - no BRCA1 5382insC mutation",
            ),
            (
                "DI",
                "Heterozygous carrier",
                "INCREASED breast/ovarian cancer risk - genetic counseling recommended",
            ),
            (
                "II",
                "Homozygous insertion",
                "HIGH breast/ovarian cancer risk - urgent genetic counseling",
            ),
        ],
    },
    VDef {
        rsid: "rs28897696",
        gene: "BRCA1",
        name: "BRCA1 missense variant",
        risk_allele: 'A',
        interps: &[
            ("GG", "Reference genotype", "Normal"),
            (
                "AG",
                "Heterozygous",
                "Variant of uncertain significance - consult genetic counselor",
            ),
            ("AA", "Homozygous variant", "Consult genetic counselor"),
        ],
    },
    // ── BRCA2 ──
    VDef {
        rsid: "rs11571833",
        gene: "BRCA2",
        name: "BRCA2 K3326X",
        risk_allele: 'T',
        interps: &[
            ("AA", "Reference genotype", "Normal"),
            (
                "AT",
                "Heterozygous",
                "Modestly increased cancer risk (OR ~1.3)",
            ),
            (
                "TT",
                "Homozygous variant",
                "Increased cancer risk - genetic counseling recommended",
            ),
        ],
    },
    // ── MTHFR (folate metabolism) ──
    VDef {
        rsid: "rs1801133",
        gene: "MTHFR",
        name: "C677T",
        risk_allele: 'A',
        interps: &[
            (
                "GG",
                "CC genotype (normal)",
                "Normal MTHFR enzyme activity (100%)",
            ),
            (
                "AG",
                "CT heterozygous",
                "Reduced enzyme activity (~65%). Consider methylfolate.",
            ),
            (
                "AA",
                "TT homozygous",
                "Significantly reduced activity (~30%). Methylfolate recommended.",
            ),
        ],
    },
    VDef {
        rsid: "rs1801131",
        gene: "MTHFR",
        name: "A1298C",
        risk_allele: 'T',
        interps: &[
            ("GG", "CC homozygous variant", "Reduced enzyme activity"),
            ("GT", "AC heterozygous", "Mildly reduced enzyme activity"),
            (
                "TT",
                "AA reference",
                "Normal MTHFR activity at this position",
            ),
        ],
    },
    // ── COMT (dopamine/pain) ──
    VDef {
        rsid: "rs4680",
        gene: "COMT",
        name: "Val158Met",
        risk_allele: 'A',
        interps: &[
            (
                "GG",
                "Val/Val",
                "Higher COMT activity, lower dopamine. Better stress resilience.",
            ),
            (
                "AG",
                "Val/Met heterozygous",
                "Intermediate COMT activity. Balanced dopamine.",
            ),
            (
                "AA",
                "Met/Met",
                "Lower COMT activity, higher dopamine. Higher pain sensitivity.",
            ),
        ],
    },
    // ── OPRM1 (opioid receptor) ──
    VDef {
        rsid: "rs1799971",
        gene: "OPRM1",
        name: "A118G (Asn40Asp)",
        risk_allele: 'G',
        interps: &[
            ("AA", "Asn/Asn", "Normal opioid sensitivity"),
            (
                "AG",
                "Asn/Asp heterozygous",
                "Reduced opioid sensitivity; may need higher doses.",
            ),
            ("GG", "Asp/Asp", "Significantly reduced opioid sensitivity."),
        ],
    },
    // ── CYP1A2 (caffeine) ──
    VDef {
        rsid: "rs762551",
        gene: "CYP1A2",
        name: "Caffeine metabolism",
        risk_allele: 'C',
        interps: &[
            (
                "AA",
                "Fast metabolizer",
                "Rapid caffeine clearance. Coffee may REDUCE heart disease risk.",
            ),
            (
                "AC",
                "Intermediate",
                "Moderate caffeine clearance. Moderate coffee intake recommended.",
            ),
            (
                "CC",
                "Slow metabolizer",
                "Slow caffeine clearance. Excess coffee may INCREASE heart risk.",
            ),
        ],
    },
    // ── Lactose ──
    VDef {
        rsid: "rs4988235",
        gene: "MCM6/LCT",
        name: "Lactase persistence (European)",
        risk_allele: 'G',
        interps: &[
            (
                "AA",
                "Lactase persistent",
                "Likely lactose TOLERANT into adulthood",
            ),
            (
                "AG",
                "Heterozygous",
                "Likely lactose tolerant (persistence is dominant)",
            ),
            (
                "GG",
                "Lactase non-persistent",
                "Likely lactose INTOLERANT in adulthood",
            ),
        ],
    },
    // ── OXTR (oxytocin receptor) ──
    VDef {
        rsid: "rs53576",
        gene: "OXTR",
        name: "Oxytocin receptor",
        risk_allele: 'A',
        interps: &[
            (
                "GG",
                "GG genotype",
                "Higher empathy scores; better social cognition.",
            ),
            (
                "AG",
                "AG heterozygous",
                "Intermediate empathy and social cognition.",
            ),
            (
                "AA",
                "AA genotype",
                "May have lower empathy; potentially more resilient to social stress.",
            ),
        ],
    },
    // ── HTR2A (serotonin) ──
    VDef {
        rsid: "rs6311",
        gene: "HTR2A",
        name: "Serotonin 2A receptor (-1438G/A)",
        risk_allele: 'T',
        interps: &[
            ("CC", "GG genotype", "Normal serotonin receptor expression"),
            (
                "CT",
                "GA heterozygous",
                "Slightly altered serotonin signaling",
            ),
            (
                "TT",
                "AA genotype",
                "Altered serotonin receptor density; may affect SSRI response",
            ),
        ],
    },
    // ── ANKK1/DRD2 (dopamine) ──
    VDef {
        rsid: "rs1800497",
        gene: "ANKK1/DRD2",
        name: "Taq1A (dopamine receptor)",
        risk_allele: 'A',
        interps: &[
            ("GG", "A2/A2", "Normal dopamine receptor density"),
            (
                "AG",
                "A1/A2 heterozygous",
                "Reduced D2 receptor density (~30% less). Reward-seeking.",
            ),
            (
                "AA",
                "A1/A1",
                "Significantly reduced D2 receptor density. Higher addiction risk.",
            ),
        ],
    },
    // ── SLCO1B1 (statin metabolism) ──
    VDef {
        rsid: "rs4363657",
        gene: "SLCO1B1",
        name: "Statin transporter",
        risk_allele: 'C',
        interps: &[
            (
                "TT",
                "Reference",
                "Normal statin metabolism. Standard dosing.",
            ),
            (
                "CT",
                "Heterozygous",
                "Increased statin myopathy risk (~4.5x). Consider lower dose.",
            ),
            (
                "CC",
                "Homozygous variant",
                "High statin myopathy risk (~17x). Use lowest effective dose.",
            ),
        ],
    },
    // ── NQO1 (oxidative stress) ──
    VDef {
        rsid: "rs1800566",
        gene: "NQO1",
        name: "Pro187Ser (oxidative stress)",
        risk_allele: 'T',
        interps: &[
            ("CC", "Pro/Pro (reference)", "Normal NQO1 enzyme activity"),
            (
                "CT",
                "Pro/Ser heterozygous",
                "Reduced NQO1 activity (~3x lower). Impaired detox.",
            ),
            (
                "TT",
                "Ser/Ser",
                "No NQO1 activity. Significantly impaired quinone detoxification.",
            ),
        ],
    },
];

/// Analyze health variants from a genotype map (rsid -> genotype string).
pub fn analyze_health_variants(genotypes: &HashMap<String, String>) -> Vec<HealthVariantResult> {
    let mut results = Vec::new();

    for def in HEALTH_VARIANTS {
        if let Some(gt) = genotypes.get(def.rsid) {
            let (desc, sig) = def
                .interps
                .iter()
                .find(|(g, _, _)| *g == gt.as_str())
                .map(|(_, d, s)| (d.to_string(), s.to_string()))
                .unwrap_or_else(|| {
                    (
                        format!("Genotype {} - not in standard table", gt),
                        "Consult genetic counselor".to_string(),
                    )
                });

            results.push(HealthVariantResult {
                rsid: def.rsid.to_string(),
                gene: def.gene.to_string(),
                name: def.name.to_string(),
                genotype: gt.clone(),
                risk_allele: def.risk_allele,
                interpretation: desc,
                clinical_significance: sig,
            });
        }
    }

    results
}

/// Determine APOE genotype from rs429358 + rs7412 combination.
pub fn determine_apoe(genotypes: &HashMap<String, String>) -> ApoeResult {
    let gt1 = genotypes.get("rs429358").cloned().unwrap_or_default();
    let gt2 = genotypes.get("rs7412").cloned().unwrap_or_default();

    if gt1.is_empty() || gt2.is_empty() {
        return ApoeResult {
            genotype: "Unable to determine (missing data)".into(),
            rs429358: gt1,
            rs7412: gt2,
        };
    }

    // e4 alleles = count of 'C' at rs429358
    let e4 = gt1.chars().filter(|&c| c == 'C').count();
    // e2 alleles = count of 'T' at rs7412
    let e2 = gt2.chars().filter(|&c| c == 'T').count();

    let genotype = match (e4, e2) {
        (0, 0) => "e3/e3 (most common, baseline risk)".into(),
        (0, 1) => "e2/e3 (PROTECTIVE - reduced Alzheimer's risk)".into(),
        (0, 2) => "e2/e2 (protective; monitor for type III hyperlipoproteinemia)".into(),
        (1, 0) => "e3/e4 (increased Alzheimer's risk ~3x)".into(),
        (1, 1) => "e2/e4 (mixed - e2 partially offsets e4 risk)".into(),
        (2, _) => "e4/e4 (significantly increased Alzheimer's risk ~12x)".into(),
        _ => format!("Unusual combination: rs429358={}, rs7412={}", gt1, gt2),
    };

    ApoeResult {
        genotype,
        rs429358: gt1,
        rs7412: gt2,
    }
}

/// Analyze MTHFR compound status from C677T + A1298C.
pub fn analyze_mthfr(genotypes: &HashMap<String, String>) -> MthfrResult {
    let c677t = genotypes.get("rs1801133").cloned().unwrap_or_default();
    let a1298c = genotypes.get("rs1801131").cloned().unwrap_or_default();

    if c677t.is_empty() || a1298c.is_empty() {
        return MthfrResult {
            c677t,
            a1298c,
            score: 0,
            assessment: "Incomplete MTHFR data".into(),
        };
    }

    let c_risk = match c677t.as_str() {
        "GG" => 0u8,
        "AG" => 1,
        "AA" => 2,
        _ => 0,
    };
    let a_risk = match a1298c.as_str() {
        "TT" => 0u8,
        "GT" => 1,
        "GG" => 2,
        _ => 0,
    };
    let score = c_risk + a_risk;

    let assessment = match score {
        0 => "Normal MTHFR function. No supplementation needed.",
        1 => "Mildly reduced MTHFR. Consider methylfolate if homocysteine elevated.",
        2 => "Moderately reduced MTHFR. Methylfolate (L-5-MTHF) recommended.",
        3 => "Significantly reduced MTHFR (compound heterozygote). Methylfolate strongly recommended.",
        _ => "Severely reduced MTHFR. Methylfolate essential. Regular homocysteine monitoring.",
    };

    MthfrResult {
        c677t,
        a1298c,
        score,
        assessment: assessment.into(),
    }
}

/// Analyze pain sensitivity profile from COMT + OPRM1.
pub fn analyze_pain(genotypes: &HashMap<String, String>) -> Option<PainProfile> {
    let comt = genotypes.get("rs4680")?;
    let oprm1 = genotypes.get("rs1799971")?;

    let mut score = 0u8;
    if comt == "AA" {
        score += 2;
    } else if comt == "AG" {
        score += 1;
    }
    if oprm1 == "GG" {
        score += 2;
    } else if oprm1 == "AG" {
        score += 1;
    }

    let label = match score {
        0 => "Low",
        1 => "Low-Moderate",
        2 => "Moderate",
        3 => "Moderate-High",
        _ => "High",
    };

    let comt_note = if comt.contains('A') {
        "Higher pain sensitivity"
    } else {
        "Lower pain sensitivity"
    };
    let oprm1_note = if oprm1.contains('G') {
        "Reduced opioid response"
    } else {
        "Normal opioid response"
    };

    Some(PainProfile {
        comt: comt.clone(),
        oprm1: oprm1.clone(),
        score,
        label: label.into(),
        comt_note: comt_note.into(),
        oprm1_note: oprm1_note.into(),
    })
}

/// Category groupings for health variant display
pub fn variant_categories() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("Cancer Risk", vec!["TP53", "BRCA1", "BRCA2", "NQO1"]),
        ("Cardiovascular", vec!["SLCO1B1"]),
        (
            "Neurological",
            vec!["APOE", "COMT", "OPRM1", "OXTR", "HTR2A", "ANKK1/DRD2"],
        ),
        ("Metabolism", vec!["MTHFR", "CYP1A2", "MCM6/LCT"]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_apoe_e3e3() {
        let gts = make_map(&[("rs429358", "TT"), ("rs7412", "CC")]);
        let r = determine_apoe(&gts);
        assert!(r.genotype.contains("e3/e3"));
    }

    #[test]
    fn test_apoe_e2e3() {
        let gts = make_map(&[("rs429358", "TT"), ("rs7412", "CT")]);
        let r = determine_apoe(&gts);
        assert!(r.genotype.contains("e2/e3"));
    }

    #[test]
    fn test_apoe_e4e4() {
        let gts = make_map(&[("rs429358", "CC"), ("rs7412", "CC")]);
        let r = determine_apoe(&gts);
        assert!(r.genotype.contains("e4/e4"));
    }

    #[test]
    fn test_mthfr_normal() {
        let gts = make_map(&[("rs1801133", "GG"), ("rs1801131", "TT")]);
        let r = analyze_mthfr(&gts);
        assert_eq!(r.score, 0);
        assert!(r.assessment.contains("Normal"));
    }

    #[test]
    fn test_mthfr_compound() {
        let gts = make_map(&[("rs1801133", "AG"), ("rs1801131", "GG")]);
        let r = analyze_mthfr(&gts);
        assert_eq!(r.score, 3);
        assert!(r.assessment.contains("compound"));
    }

    #[test]
    fn test_pain_low() {
        let gts = make_map(&[("rs4680", "GG"), ("rs1799971", "AA")]);
        let p = analyze_pain(&gts).unwrap();
        assert_eq!(p.score, 0);
        assert_eq!(p.label, "Low");
    }

    #[test]
    fn test_pain_high() {
        let gts = make_map(&[("rs4680", "AA"), ("rs1799971", "GG")]);
        let p = analyze_pain(&gts).unwrap();
        assert_eq!(p.score, 4);
        assert_eq!(p.label, "High");
    }

    #[test]
    fn test_health_variants_lookup() {
        let gts = make_map(&[("rs762551", "AA"), ("rs4680", "AG")]);
        let results = analyze_health_variants(&gts);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].gene, "COMT");
        assert_eq!(results[1].gene, "CYP1A2");
    }
}
