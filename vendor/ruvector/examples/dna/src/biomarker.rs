//! Composite health biomarker analysis engine -- combines SNP genotype data
//! with clinical biomarker reference ranges to produce composite risk scores,
//! 64-dim profile vectors (for HNSW indexing), and synthetic populations.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::health::{analyze_mthfr, analyze_pain};

/// Clinical reference range for a single biomarker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiomarkerReference {
    pub name: &'static str,
    pub unit: &'static str,
    pub normal_low: f64,
    pub normal_high: f64,
    pub critical_low: Option<f64>,
    pub critical_high: Option<f64>,
    pub category: &'static str,
}

/// Classification of a biomarker value relative to its reference range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BiomarkerClassification {
    CriticalLow,
    Low,
    Normal,
    High,
    CriticalHigh,
}

static REFERENCES: &[BiomarkerReference] = &[
    BiomarkerReference {
        name: "Total Cholesterol",
        unit: "mg/dL",
        normal_low: 125.0,
        normal_high: 200.0,
        critical_low: Some(100.0),
        critical_high: Some(300.0),
        category: "Lipid",
    },
    BiomarkerReference {
        name: "LDL",
        unit: "mg/dL",
        normal_low: 50.0,
        normal_high: 100.0,
        critical_low: Some(25.0),
        critical_high: Some(190.0),
        category: "Lipid",
    },
    BiomarkerReference {
        name: "HDL",
        unit: "mg/dL",
        normal_low: 40.0,
        normal_high: 90.0,
        critical_low: Some(20.0),
        critical_high: None,
        category: "Lipid",
    },
    BiomarkerReference {
        name: "Triglycerides",
        unit: "mg/dL",
        normal_low: 35.0,
        normal_high: 150.0,
        critical_low: Some(20.0),
        critical_high: Some(500.0),
        category: "Lipid",
    },
    BiomarkerReference {
        name: "Fasting Glucose",
        unit: "mg/dL",
        normal_low: 70.0,
        normal_high: 100.0,
        critical_low: Some(50.0),
        critical_high: Some(250.0),
        category: "Metabolic",
    },
    BiomarkerReference {
        name: "HbA1c",
        unit: "%",
        normal_low: 4.0,
        normal_high: 5.7,
        critical_low: None,
        critical_high: Some(9.0),
        category: "Metabolic",
    },
    BiomarkerReference {
        name: "Homocysteine",
        unit: "umol/L",
        normal_low: 5.0,
        normal_high: 15.0,
        critical_low: None,
        critical_high: Some(30.0),
        category: "Metabolic",
    },
    BiomarkerReference {
        name: "Vitamin D",
        unit: "ng/mL",
        normal_low: 30.0,
        normal_high: 80.0,
        critical_low: Some(10.0),
        critical_high: Some(150.0),
        category: "Nutritional",
    },
    BiomarkerReference {
        name: "CRP",
        unit: "mg/L",
        normal_low: 0.0,
        normal_high: 3.0,
        critical_low: None,
        critical_high: Some(10.0),
        category: "Inflammatory",
    },
    BiomarkerReference {
        name: "TSH",
        unit: "mIU/L",
        normal_low: 0.4,
        normal_high: 4.0,
        critical_low: Some(0.1),
        critical_high: Some(10.0),
        category: "Thyroid",
    },
    BiomarkerReference {
        name: "Ferritin",
        unit: "ng/mL",
        normal_low: 20.0,
        normal_high: 250.0,
        critical_low: Some(10.0),
        critical_high: Some(1000.0),
        category: "Iron",
    },
    BiomarkerReference {
        name: "Vitamin B12",
        unit: "pg/mL",
        normal_low: 200.0,
        normal_high: 900.0,
        critical_low: Some(150.0),
        critical_high: None,
        category: "Nutritional",
    },
    BiomarkerReference {
        name: "Lp(a)",
        unit: "nmol/L",
        normal_low: 0.0,
        normal_high: 75.0,
        critical_low: None,
        critical_high: Some(200.0),
        category: "Lipid",
    },
];

/// Return the static biomarker reference table.
pub fn biomarker_references() -> &'static [BiomarkerReference] {
    REFERENCES
}

/// Compute a z-score for a value relative to a reference range.
pub fn z_score(value: f64, reference: &BiomarkerReference) -> f64 {
    let mid = (reference.normal_low + reference.normal_high) / 2.0;
    let half_range = (reference.normal_high - reference.normal_low) / 2.0;
    if half_range == 0.0 {
        return 0.0;
    }
    (value - mid) / half_range
}

/// Classify a biomarker value against its reference range.
pub fn classify_biomarker(value: f64, reference: &BiomarkerReference) -> BiomarkerClassification {
    if let Some(cl) = reference.critical_low {
        if value < cl {
            return BiomarkerClassification::CriticalLow;
        }
    }
    if value < reference.normal_low {
        return BiomarkerClassification::Low;
    }
    if let Some(ch) = reference.critical_high {
        if value > ch {
            return BiomarkerClassification::CriticalHigh;
        }
    }
    if value > reference.normal_high {
        return BiomarkerClassification::High;
    }
    BiomarkerClassification::Normal
}

/// Risk score for a single clinical category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryScore {
    pub category: String,
    pub score: f64,
    pub confidence: f64,
    pub contributing_variants: Vec<String>,
}

/// Full biomarker + genotype risk profile for one subject.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiomarkerProfile {
    pub subject_id: String,
    pub timestamp: i64,
    pub category_scores: HashMap<String, CategoryScore>,
    pub global_risk_score: f64,
    pub profile_vector: Vec<f32>,
    pub biomarker_values: HashMap<String, f64>,
}

/// Unified SNP descriptor — eliminates parallel-array fragility.
struct SnpDef {
    rsid: &'static str,
    category: &'static str,
    w_ref: f64,
    w_het: f64,
    w_alt: f64,
    hom_ref: &'static str,
    het: &'static str,
    hom_alt: &'static str,
    maf: f64, // minor allele frequency
}

static SNPS: &[SnpDef] = &[
    SnpDef {
        rsid: "rs429358",
        category: "Neurological",
        w_ref: 0.0,
        w_het: 0.4,
        w_alt: 0.9,
        hom_ref: "TT",
        het: "CT",
        hom_alt: "CC",
        maf: 0.14,
    },
    SnpDef {
        rsid: "rs7412",
        category: "Neurological",
        w_ref: 0.0,
        w_het: -0.15,
        w_alt: -0.3,
        hom_ref: "CC",
        het: "CT",
        hom_alt: "TT",
        maf: 0.08,
    },
    SnpDef {
        rsid: "rs1042522",
        category: "Cancer Risk",
        w_ref: 0.0,
        w_het: 0.25,
        w_alt: 0.5,
        hom_ref: "CC",
        het: "CG",
        hom_alt: "GG",
        maf: 0.40,
    },
    SnpDef {
        rsid: "rs80357906",
        category: "Cancer Risk",
        w_ref: 0.0,
        w_het: 0.7,
        w_alt: 0.95,
        hom_ref: "DD",
        het: "DI",
        hom_alt: "II",
        maf: 0.003,
    },
    SnpDef {
        rsid: "rs28897696",
        category: "Cancer Risk",
        w_ref: 0.0,
        w_het: 0.3,
        w_alt: 0.6,
        hom_ref: "GG",
        het: "AG",
        hom_alt: "AA",
        maf: 0.005,
    },
    SnpDef {
        rsid: "rs11571833",
        category: "Cancer Risk",
        w_ref: 0.0,
        w_het: 0.20,
        w_alt: 0.5,
        hom_ref: "AA",
        het: "AT",
        hom_alt: "TT",
        maf: 0.01,
    },
    SnpDef {
        rsid: "rs1801133",
        category: "Metabolism",
        w_ref: 0.0,
        w_het: 0.35,
        w_alt: 0.7,
        hom_ref: "GG",
        het: "AG",
        hom_alt: "AA",
        maf: 0.32,
    },
    SnpDef {
        rsid: "rs1801131",
        category: "Metabolism",
        w_ref: 0.0,
        w_het: 0.10,
        w_alt: 0.25,
        hom_ref: "TT",
        het: "GT",
        hom_alt: "GG",
        maf: 0.30,
    },
    SnpDef {
        rsid: "rs4680",
        category: "Neurological",
        w_ref: 0.0,
        w_het: 0.2,
        w_alt: 0.45,
        hom_ref: "GG",
        het: "AG",
        hom_alt: "AA",
        maf: 0.50,
    },
    SnpDef {
        rsid: "rs1799971",
        category: "Neurological",
        w_ref: 0.0,
        w_het: 0.2,
        w_alt: 0.4,
        hom_ref: "AA",
        het: "AG",
        hom_alt: "GG",
        maf: 0.15,
    },
    SnpDef {
        rsid: "rs762551",
        category: "Metabolism",
        w_ref: 0.0,
        w_het: 0.15,
        w_alt: 0.35,
        hom_ref: "AA",
        het: "AC",
        hom_alt: "CC",
        maf: 0.37,
    },
    SnpDef {
        rsid: "rs4988235",
        category: "Metabolism",
        w_ref: 0.0,
        w_het: 0.05,
        w_alt: 0.15,
        hom_ref: "AA",
        het: "AG",
        hom_alt: "GG",
        maf: 0.24,
    },
    SnpDef {
        rsid: "rs53576",
        category: "Neurological",
        w_ref: 0.0,
        w_het: 0.1,
        w_alt: 0.25,
        hom_ref: "GG",
        het: "AG",
        hom_alt: "AA",
        maf: 0.35,
    },
    SnpDef {
        rsid: "rs6311",
        category: "Neurological",
        w_ref: 0.0,
        w_het: 0.15,
        w_alt: 0.3,
        hom_ref: "CC",
        het: "CT",
        hom_alt: "TT",
        maf: 0.45,
    },
    SnpDef {
        rsid: "rs1800497",
        category: "Neurological",
        w_ref: 0.0,
        w_het: 0.25,
        w_alt: 0.5,
        hom_ref: "GG",
        het: "AG",
        hom_alt: "AA",
        maf: 0.20,
    },
    SnpDef {
        rsid: "rs4363657",
        category: "Cardiovascular",
        w_ref: 0.0,
        w_het: 0.35,
        w_alt: 0.7,
        hom_ref: "TT",
        het: "CT",
        hom_alt: "CC",
        maf: 0.15,
    },
    SnpDef {
        rsid: "rs1800566",
        category: "Cancer Risk",
        w_ref: 0.0,
        w_het: 0.15,
        w_alt: 0.30,
        hom_ref: "CC",
        het: "CT",
        hom_alt: "TT",
        maf: 0.22,
    },
    // LPA — Lp(a) cardiovascular risk (2024 meta-analysis: OR 1.6-1.75/allele CHD)
    SnpDef {
        rsid: "rs10455872",
        category: "Cardiovascular",
        w_ref: 0.0,
        w_het: 0.40,
        w_alt: 0.75,
        hom_ref: "AA",
        het: "AG",
        hom_alt: "GG",
        maf: 0.07,
    },
    SnpDef {
        rsid: "rs3798220",
        category: "Cardiovascular",
        w_ref: 0.0,
        w_het: 0.35,
        w_alt: 0.65,
        hom_ref: "TT",
        het: "CT",
        hom_alt: "CC",
        maf: 0.02,
    },
    // PCSK9 R46L — protective loss-of-function (NEJM 2006: OR 0.77 CHD, 0.40 MI)
    SnpDef {
        rsid: "rs11591147",
        category: "Cardiovascular",
        w_ref: 0.0,
        w_het: -0.30,
        w_alt: -0.55,
        hom_ref: "GG",
        het: "GT",
        hom_alt: "TT",
        maf: 0.024,
    },
];

/// Number of SNPs with one-hot encoding in profile vector (first 17 for 64-dim SIMD alignment).
/// Additional SNPs (LPA) contribute to risk scores but use summary dims in the vector.
const NUM_ONEHOT_SNPS: usize = 17;
const NUM_SNPS: usize = 20;

fn genotype_code(snp: &SnpDef, gt: &str) -> u8 {
    if gt == snp.hom_ref {
        0
    } else if gt.len() == 2 && gt.as_bytes()[0] != gt.as_bytes()[1] {
        1
    } else {
        2
    }
}

fn snp_weight(snp: &SnpDef, code: u8) -> f64 {
    match code {
        0 => snp.w_ref,
        1 => snp.w_het,
        _ => snp.w_alt,
    }
}

struct Interaction {
    rsid_a: &'static str,
    rsid_b: &'static str,
    modifier: f64,
    category: &'static str,
}

static INTERACTIONS: &[Interaction] = &[
    Interaction {
        rsid_a: "rs4680",
        rsid_b: "rs1799971",
        modifier: 1.4,
        category: "Neurological",
    },
    Interaction {
        rsid_a: "rs1801133",
        rsid_b: "rs1801131",
        modifier: 1.3,
        category: "Metabolism",
    },
    Interaction {
        rsid_a: "rs429358",
        rsid_b: "rs1042522",
        modifier: 1.2,
        category: "Cancer Risk",
    },
    Interaction {
        rsid_a: "rs80357906",
        rsid_b: "rs1042522",
        modifier: 1.5,
        category: "Cancer Risk",
    },
    Interaction {
        rsid_a: "rs1801131",
        rsid_b: "rs4680",
        modifier: 1.25,
        category: "Neurological",
    }, // A1298C×COMT depression (geneticlifehacks)
    Interaction {
        rsid_a: "rs1800497",
        rsid_b: "rs4680",
        modifier: 1.2,
        category: "Neurological",
    }, // DRD2×COMT working memory (geneticlifehacks)
];

fn snp_idx(rsid: &str) -> Option<usize> {
    SNPS.iter().position(|s| s.rsid == rsid)
}

fn is_non_ref(gts: &HashMap<String, String>, rsid: &str) -> bool {
    match (gts.get(rsid), snp_idx(rsid)) {
        (Some(g), Some(idx)) => g != SNPS[idx].hom_ref,
        _ => false,
    }
}

fn interaction_mod(gts: &HashMap<String, String>, ix: &Interaction) -> f64 {
    if is_non_ref(gts, ix.rsid_a) && is_non_ref(gts, ix.rsid_b) {
        ix.modifier
    } else {
        1.0
    }
}

struct CategoryMeta {
    name: &'static str,
    max_possible: f64,
    expected_count: usize,
}

static CAT_ORDER: &[&str] = &[
    "Cancer Risk",
    "Cardiovascular",
    "Neurological",
    "Metabolism",
];

fn category_meta() -> &'static [CategoryMeta] {
    use std::sync::LazyLock;
    static META: LazyLock<Vec<CategoryMeta>> = LazyLock::new(|| {
        CAT_ORDER
            .iter()
            .map(|&cat| {
                let (mp, ec) = SNPS
                    .iter()
                    .filter(|s| s.category == cat)
                    .fold((0.0, 0usize), |(s, n), snp| (s + snp.w_alt.max(0.0), n + 1));
                CategoryMeta {
                    name: cat,
                    max_possible: mp.max(1.0),
                    expected_count: ec,
                }
            })
            .collect()
    });
    &META
}

/// Compute composite risk scores from genotype data.
pub fn compute_risk_scores(genotypes: &HashMap<String, String>) -> BiomarkerProfile {
    let meta = category_meta();
    let mut cat_scores: HashMap<&str, (f64, Vec<String>, usize)> = HashMap::with_capacity(4);

    for snp in SNPS {
        if let Some(gt) = genotypes.get(snp.rsid) {
            let code = genotype_code(snp, gt);
            let w = snp_weight(snp, code);
            let entry = cat_scores
                .entry(snp.category)
                .or_insert_with(|| (0.0, Vec::new(), 0));
            entry.0 += w;
            entry.2 += 1;
            if code > 0 {
                entry.1.push(snp.rsid.to_string());
            }
        }
    }

    for inter in INTERACTIONS {
        let m = interaction_mod(genotypes, inter);
        if m > 1.0 {
            if let Some(entry) = cat_scores.get_mut(inter.category) {
                entry.0 *= m;
            }
        }
    }

    let mut category_scores = HashMap::with_capacity(meta.len());
    for cm in meta {
        let (raw, variants, count) = cat_scores.remove(cm.name).unwrap_or((0.0, Vec::new(), 0));
        let score = (raw / cm.max_possible).clamp(0.0, 1.0);
        let confidence = if count > 0 {
            (count as f64 / cm.expected_count.max(1) as f64).min(1.0)
        } else {
            0.0
        };
        let cat = cm.name.to_string();
        category_scores.insert(
            cat.clone(),
            CategoryScore {
                category: cat,
                score,
                confidence,
                contributing_variants: variants,
            },
        );
    }

    let (ws, cs) = category_scores.values().fold((0.0, 0.0), |(ws, cs), c| {
        (ws + c.score * c.confidence, cs + c.confidence)
    });
    let global = if cs > 0.0 { ws / cs } else { 0.0 };

    let mut profile = BiomarkerProfile {
        subject_id: String::new(),
        timestamp: 0,
        category_scores,
        global_risk_score: global,
        profile_vector: Vec::new(),
        biomarker_values: HashMap::new(),
    };
    profile.profile_vector = encode_profile_vector_with_genotypes(&profile, genotypes);
    profile
}

/// Encode a profile into a 64-dim f32 vector (L2-normalized).
pub fn encode_profile_vector(profile: &BiomarkerProfile) -> Vec<f32> {
    encode_profile_vector_with_genotypes(profile, &HashMap::new())
}

fn encode_profile_vector_with_genotypes(
    profile: &BiomarkerProfile,
    genotypes: &HashMap<String, String>,
) -> Vec<f32> {
    let mut v = vec![0.0f32; 64];
    // Dims 0..50: one-hot genotype encoding (first 17 SNPs x 3 = 51 dims)
    for (i, snp) in SNPS.iter().take(NUM_ONEHOT_SNPS).enumerate() {
        let code = genotypes
            .get(snp.rsid)
            .map(|gt| genotype_code(snp, gt))
            .unwrap_or(0);
        v[i * 3 + code as usize] = 1.0;
    }
    // Dims 51..54: category scores
    for (j, cat) in CAT_ORDER.iter().enumerate() {
        v[51 + j] = profile
            .category_scores
            .get(*cat)
            .map(|c| c.score as f32)
            .unwrap_or(0.0);
    }
    v[55] = profile.global_risk_score as f32;
    // Dims 56..59: first 4 interaction modifiers
    for (j, inter) in INTERACTIONS.iter().take(4).enumerate() {
        let m = interaction_mod(genotypes, inter);
        v[56 + j] = if m > 1.0 { (m - 1.0) as f32 } else { 0.0 };
    }
    // Dims 60..63: derived clinical scores
    v[60] = analyze_mthfr(genotypes).score as f32 / 4.0;
    v[61] = analyze_pain(genotypes)
        .map(|p| p.score as f32 / 4.0)
        .unwrap_or(0.0);
    v[62] = genotypes
        .get("rs429358")
        .map(|g| genotype_code(&SNPS[0], g) as f32 / 2.0)
        .unwrap_or(0.0);
    // LPA composite: average of rs10455872 + rs3798220 genotype codes
    let lpa = SNPS
        .iter()
        .filter(|s| s.rsid == "rs10455872" || s.rsid == "rs3798220")
        .filter_map(|s| {
            genotypes
                .get(s.rsid)
                .map(|g| genotype_code(s, g) as f32 / 2.0)
        })
        .sum::<f32>()
        / 2.0;
    v[63] = lpa;

    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        v.iter_mut().for_each(|x| *x /= norm);
    }
    v
}

fn random_genotype(rng: &mut StdRng, snp: &SnpDef) -> String {
    let p = snp.maf;
    let q = 1.0 - p;
    let r: f64 = rng.gen();
    if r < q * q {
        snp.hom_ref
    } else if r < q * q + 2.0 * p * q {
        snp.het
    } else {
        snp.hom_alt
    }
    .to_string()
}

/// Generate a deterministic synthetic population of biomarker profiles.
pub fn generate_synthetic_population(count: usize, seed: u64) -> Vec<BiomarkerProfile> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut pop = Vec::with_capacity(count);

    for i in 0..count {
        let mut genotypes = HashMap::with_capacity(NUM_SNPS);
        for snp in SNPS {
            genotypes.insert(snp.rsid.to_string(), random_genotype(&mut rng, snp));
        }

        let mut profile = compute_risk_scores(&genotypes);
        profile.subject_id = format!("SYN-{:06}", i);
        profile.timestamp = 1700000000 + i as i64;

        let mthfr_score = analyze_mthfr(&genotypes).score;
        let apoe_code = genotypes
            .get("rs429358")
            .map(|g| genotype_code(&SNPS[0], g))
            .unwrap_or(0);
        let nqo1_idx = SNPS.iter().position(|s| s.rsid == "rs1800566").unwrap();
        let nqo1_code = genotypes
            .get("rs1800566")
            .map(|g| genotype_code(&SNPS[nqo1_idx], g))
            .unwrap_or(0);
        let lpa_risk: u8 = SNPS
            .iter()
            .filter(|s| s.rsid == "rs10455872" || s.rsid == "rs3798220")
            .filter_map(|s| genotypes.get(s.rsid).map(|g| genotype_code(s, g)))
            .sum();
        let pcsk9_idx = SNPS.iter().position(|s| s.rsid == "rs11591147").unwrap();
        let pcsk9_code = genotypes
            .get("rs11591147")
            .map(|g| genotype_code(&SNPS[pcsk9_idx], g))
            .unwrap_or(0);
        profile.biomarker_values.reserve(REFERENCES.len());

        for bref in REFERENCES {
            let mid = (bref.normal_low + bref.normal_high) / 2.0;
            let sd = (bref.normal_high - bref.normal_low) / 4.0;
            let mut val = mid + rng.gen_range(-1.5..1.5) * sd;
            // Gene→biomarker correlations from SOTA clinical evidence (additive)
            let nm = bref.name;
            if nm == "Homocysteine" && mthfr_score >= 2 {
                val += sd * (mthfr_score as f64 - 1.0);
            }
            if (nm == "Total Cholesterol" || nm == "LDL") && apoe_code > 0 {
                val += sd * 0.5 * apoe_code as f64;
            }
            if nm == "HDL" && apoe_code > 0 {
                val -= sd * 0.3 * apoe_code as f64;
            }
            if nm == "Triglycerides" && apoe_code > 0 {
                val += sd * 0.4 * apoe_code as f64;
            }
            if nm == "Vitamin B12" && mthfr_score >= 2 {
                val -= sd * 0.4;
            }
            if nm == "CRP" && nqo1_code == 2 {
                val += sd * 0.3;
            }
            if nm == "Lp(a)" && lpa_risk > 0 {
                val += sd * 1.5 * lpa_risk as f64;
            }
            if (nm == "LDL" || nm == "Total Cholesterol") && pcsk9_code > 0 {
                val -= sd * 0.6 * pcsk9_code as f64;
            }
            val = val.max(bref.critical_low.unwrap_or(0.0)).max(0.0);
            if let Some(ch) = bref.critical_high {
                val = val.min(ch * 1.2);
            }
            profile
                .biomarker_values
                .insert(bref.name.to_string(), (val * 10.0).round() / 10.0);
        }
        pop.push(profile);
    }
    pop
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_hom_ref() -> HashMap<String, String> {
        SNPS.iter()
            .map(|s| (s.rsid.to_string(), s.hom_ref.to_string()))
            .collect()
    }

    #[test]
    fn test_z_score_midpoint_is_zero() {
        let r = &REFERENCES[0]; // Total Cholesterol
        let mid = (r.normal_low + r.normal_high) / 2.0;
        assert!((z_score(mid, r)).abs() < 1e-10);
    }

    #[test]
    fn test_z_score_high_bound_is_one() {
        let r = &REFERENCES[0];
        assert!((z_score(r.normal_high, r) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_classify_normal() {
        let r = &REFERENCES[0]; // Total Cholesterol 125-200
        assert_eq!(
            classify_biomarker(150.0, r),
            BiomarkerClassification::Normal
        );
    }

    #[test]
    fn test_classify_critical_high() {
        let r = &REFERENCES[0]; // critical_high = 300
        assert_eq!(
            classify_biomarker(350.0, r),
            BiomarkerClassification::CriticalHigh
        );
    }

    #[test]
    fn test_classify_low() {
        let r = &REFERENCES[0]; // normal_low = 125, critical_low = 100
        assert_eq!(classify_biomarker(110.0, r), BiomarkerClassification::Low);
    }

    #[test]
    fn test_classify_critical_low() {
        let r = &REFERENCES[0]; // critical_low = 100
        assert_eq!(
            classify_biomarker(90.0, r),
            BiomarkerClassification::CriticalLow
        );
    }

    #[test]
    fn test_risk_scores_all_hom_ref_low_risk() {
        let gts = full_hom_ref();
        let profile = compute_risk_scores(&gts);
        assert!(
            profile.global_risk_score < 0.15,
            "hom-ref should be low risk, got {}",
            profile.global_risk_score
        );
    }

    #[test]
    fn test_risk_scores_high_cancer_risk() {
        let mut gts = full_hom_ref();
        gts.insert("rs80357906".into(), "DI".into());
        gts.insert("rs1042522".into(), "GG".into());
        gts.insert("rs11571833".into(), "TT".into());
        let profile = compute_risk_scores(&gts);
        let cancer = profile.category_scores.get("Cancer Risk").unwrap();
        assert!(
            cancer.score > 0.3,
            "should have elevated cancer risk, got {}",
            cancer.score
        );
    }

    #[test]
    fn test_vector_dimension_is_64() {
        let gts = full_hom_ref();
        let profile = compute_risk_scores(&gts);
        assert_eq!(profile.profile_vector.len(), 64);
    }

    #[test]
    fn test_vector_is_l2_normalized() {
        let mut gts = full_hom_ref();
        gts.insert("rs4680".into(), "AG".into());
        gts.insert("rs1799971".into(), "AG".into());
        let profile = compute_risk_scores(&gts);
        let norm: f32 = profile
            .profile_vector
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-4,
            "vector should be L2-normalized, got norm={}",
            norm
        );
    }

    #[test]
    fn test_interaction_comt_oprm1() {
        let mut gts = full_hom_ref();
        gts.insert("rs4680".into(), "AA".into());
        gts.insert("rs1799971".into(), "GG".into());
        let with_interaction = compute_risk_scores(&gts);
        let neuro_inter = with_interaction
            .category_scores
            .get("Neurological")
            .unwrap()
            .score;

        // Without full interaction (only one variant)
        let mut gts2 = full_hom_ref();
        gts2.insert("rs4680".into(), "AA".into());
        let without_full = compute_risk_scores(&gts2);
        let neuro_single = without_full
            .category_scores
            .get("Neurological")
            .unwrap()
            .score;

        assert!(
            neuro_inter > neuro_single,
            "interaction should amplify risk"
        );
    }

    #[test]
    fn test_interaction_brca1_tp53() {
        let mut gts = full_hom_ref();
        gts.insert("rs80357906".into(), "DI".into());
        gts.insert("rs1042522".into(), "GG".into());
        let profile = compute_risk_scores(&gts);
        let cancer = profile.category_scores.get("Cancer Risk").unwrap();
        assert!(cancer
            .contributing_variants
            .contains(&"rs80357906".to_string()));
        assert!(cancer
            .contributing_variants
            .contains(&"rs1042522".to_string()));
    }

    #[test]
    fn test_population_generation() {
        let pop = generate_synthetic_population(50, 42);
        assert_eq!(pop.len(), 50);
        for p in &pop {
            assert_eq!(p.profile_vector.len(), 64);
            assert!(!p.biomarker_values.is_empty());
            assert!(p.global_risk_score >= 0.0 && p.global_risk_score <= 1.0);
        }
    }

    #[test]
    fn test_population_deterministic() {
        let a = generate_synthetic_population(10, 99);
        let b = generate_synthetic_population(10, 99);
        for (pa, pb) in a.iter().zip(b.iter()) {
            assert_eq!(pa.subject_id, pb.subject_id);
            assert!((pa.global_risk_score - pb.global_risk_score).abs() < 1e-10);
        }
    }

    #[test]
    fn test_mthfr_elevates_homocysteine() {
        let pop = generate_synthetic_population(200, 7);
        let (mut mthfr_high, mut mthfr_low) = (Vec::new(), Vec::new());
        for p in &pop {
            let hcy = p
                .biomarker_values
                .get("Homocysteine")
                .copied()
                .unwrap_or(0.0);
            let mthfr_score = p
                .category_scores
                .get("Metabolism")
                .map(|c| c.score)
                .unwrap_or(0.0);
            if mthfr_score > 0.3 {
                mthfr_high.push(hcy);
            } else {
                mthfr_low.push(hcy);
            }
        }
        if !mthfr_high.is_empty() && !mthfr_low.is_empty() {
            let avg_high: f64 = mthfr_high.iter().sum::<f64>() / mthfr_high.len() as f64;
            let avg_low: f64 = mthfr_low.iter().sum::<f64>() / mthfr_low.len() as f64;
            assert!(
                avg_high > avg_low,
                "MTHFR variants should elevate homocysteine: high={}, low={}",
                avg_high,
                avg_low
            );
        }
    }

    #[test]
    fn test_biomarker_references_count() {
        assert_eq!(biomarker_references().len(), 13);
    }
}
