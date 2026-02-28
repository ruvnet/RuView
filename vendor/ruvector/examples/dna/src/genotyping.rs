//! Native 23andMe genotyping analysis
//!
//! Parses 23andMe raw data exports (v4/v5 tab-separated format) and runs
//! a 7-stage genomic analysis pipeline entirely in Rust — no Python bridge.
//!
//! Based on: <https://github.com/ericporres/rvdna-bridge>

use crate::error::{DnaError, Result};
use crate::health::{self, ApoeResult, HealthVariantResult, MthfrResult, PainProfile};
use crate::pharma::{self, DrugRecommendation, MetabolizerPhenotype};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as FmtWrite;
use std::io::{BufRead, BufReader};
use std::time::Instant;

// ═══════════════════════════════════════════════════════════════════════
// Core Types
// ═══════════════════════════════════════════════════════════════════════

/// Reference genome build
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GenomeBuild {
    /// GRCh37 / hg19 (23andMe v4, most v5)
    GRCh37,
    /// GRCh38 / hg38
    GRCh38,
    /// Could not determine from file header
    Unknown,
}

/// Confidence level for a pharmacogenomic call
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallConfidence {
    /// Panel lacks required markers entirely
    Unsupported,
    /// <50% of defining variants genotyped
    Weak,
    /// >=50% genotyped but missing phase/CNV information
    Moderate,
    /// All defining variants genotyped (still no phase resolution)
    Strong,
}

/// A single SNP from a 23andMe genotyping file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snp {
    /// rsid identifier (e.g., "rs429358")
    pub rsid: String,
    /// Chromosome (e.g., "1", "22", "X", "MT")
    pub chromosome: String,
    /// Position on chromosome
    pub position: u64,
    /// Normalized genotype call (uppercase, alleles sorted for SNPs)
    pub genotype: String,
}

/// Parsed 23andMe genotype data
#[derive(Debug, Clone)]
pub struct GenotypeData {
    /// SNPs indexed by rsid
    pub snps: HashMap<String, Snp>,
    /// Total markers in file (including no-calls)
    pub total_markers: usize,
    /// Number of no-call markers ("--")
    pub no_calls: usize,
    /// Per-chromosome marker counts (sorted)
    pub chr_counts: BTreeMap<String, usize>,
    /// Detected reference genome build
    pub build: GenomeBuild,
}

impl GenotypeData {
    /// Number of called (non-"--") markers
    pub fn called(&self) -> usize {
        self.total_markers - self.no_calls
    }

    /// Build rsid -> genotype map for downstream analysis
    pub fn genotype_map(&self) -> HashMap<String, String> {
        self.snps
            .iter()
            .map(|(k, v)| (k.clone(), v.genotype.clone()))
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Genotype Normalization
// ═══════════════════════════════════════════════════════════════════════

/// Normalize a genotype string: uppercase, trim, sort allele pair.
///
/// "ag" → "AG", "TC" → "CT", "DI" → "DI", "id" → "DI"
pub fn normalize_genotype(gt: &str) -> String {
    let gt = gt.trim().to_uppercase();
    let b = gt.as_bytes();
    if b.len() == 2 && b[0] > b[1] {
        return format!("{}{}", b[1] as char, b[0] as char);
    }
    gt
}

// ═══════════════════════════════════════════════════════════════════════
// Stage 1: 23andMe Parser
// ═══════════════════════════════════════════════════════════════════════

/// Parse a 23andMe raw data file (v4/v5 tab-separated format).
///
/// Normalizes all genotype strings on load (uppercase, sorted alleles).
/// Detects genome build from header comments when present.
pub fn parse_23andme<R: std::io::Read>(reader: R) -> Result<GenotypeData> {
    let buf = BufReader::new(reader);
    let mut snps = HashMap::with_capacity(650_000);
    let mut chr_counts = BTreeMap::new();
    let mut total = 0usize;
    let mut no_calls = 0usize;
    let mut build = GenomeBuild::Unknown;

    for line_result in buf.lines() {
        let line = line_result.map_err(DnaError::IoError)?;
        if line.starts_with('#') {
            // Detect genome build from header
            let lower = line.to_lowercase();
            if lower.contains("build 37") || lower.contains("grch37") || lower.contains("hg19") {
                build = GenomeBuild::GRCh37;
            } else if lower.contains("build 38")
                || lower.contains("grch38")
                || lower.contains("hg38")
            {
                build = GenomeBuild::GRCh38;
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }

        let mut parts = line.splitn(4, '\t');
        let rsid = match parts.next() {
            Some(s) => s,
            None => continue,
        };
        let chrom = match parts.next() {
            Some(s) => s,
            None => continue,
        };
        let pos_str = match parts.next() {
            Some(s) => s,
            None => continue,
        };
        let genotype = match parts.next() {
            Some(s) => s,
            None => continue,
        };

        total += 1;
        if genotype == "--" {
            no_calls += 1;
            continue;
        }

        let pos: u64 = pos_str.parse().unwrap_or(0);
        let norm_gt = normalize_genotype(genotype);
        *chr_counts.entry(chrom.to_string()).or_insert(0) += 1;
        snps.insert(
            rsid.to_string(),
            Snp {
                rsid: rsid.to_string(),
                chromosome: chrom.to_string(),
                position: pos,
                genotype: norm_gt,
            },
        );
    }

    if total == 0 {
        return Err(DnaError::ParseError("No markers found in file".into()));
    }
    Ok(GenotypeData {
        snps,
        total_markers: total,
        no_calls,
        chr_counts,
        build,
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Stage 2: Panel Signature & Call Rate QC
// ═══════════════════════════════════════════════════════════════════════

/// QC metrics for a gene region
#[derive(Debug, Clone)]
pub struct RegionQc {
    pub name: String,
    pub snp_count: usize,
    pub het_count: usize,
    pub het_rate: f64,
    pub signature: Vec<f32>,
}

struct GeneRegion {
    name: &'static str,
    chromosome: &'static str,
    start: u64,
    end: u64,
}

/// GRCh37 coordinates for gene regions
static GENE_REGIONS_37: &[GeneRegion] = &[
    GeneRegion {
        name: "HBB",
        chromosome: "11",
        start: 5_225_464,
        end: 5_229_395,
    },
    GeneRegion {
        name: "TP53",
        chromosome: "17",
        start: 7_571_720,
        end: 7_590_868,
    },
    GeneRegion {
        name: "BRCA1",
        chromosome: "17",
        start: 41_196_312,
        end: 41_277_500,
    },
    GeneRegion {
        name: "CYP2D6",
        chromosome: "22",
        start: 42_522_500,
        end: 42_528_000,
    },
    GeneRegion {
        name: "INS",
        chromosome: "11",
        start: 2_159_779,
        end: 2_161_341,
    },
];

#[inline]
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn signature_vector(snps: &[&Snp], k: usize, dims: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; dims];
    let seq: Vec<u8> = snps.iter().flat_map(|s| s.genotype.bytes()).collect();
    if seq.len() < k {
        return v;
    }
    for w in seq.windows(k) {
        v[(fnv1a(w) as usize) % dims] += 1.0;
    }
    let mag: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag > 0.0 {
        let inv = 1.0 / mag;
        for x in &mut v {
            *x *= inv;
        }
    }
    v
}

fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let ma: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if ma == 0.0 || mb == 0.0 {
        0.0
    } else {
        dot / (ma * mb)
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Stage 4: Pharmacogenomics (rsid-based, with confidence)
// ═══════════════════════════════════════════════════════════════════════

struct CypRsidDef {
    rsid: &'static str,
    allele_name: &'static str,
    alt_base: char,
    is_deletion: bool,
    activity: f64,
    function: &'static str,
}

static CYP2D6_RSID_DEFS: &[CypRsidDef] = &[
    CypRsidDef {
        rsid: "rs3892097",
        allele_name: "*4",
        alt_base: 'T',
        is_deletion: false,
        activity: 0.0,
        function: "No function (splicing defect)",
    },
    CypRsidDef {
        rsid: "rs35742686",
        allele_name: "*3",
        alt_base: '-',
        is_deletion: true,
        activity: 0.0,
        function: "No function (frameshift)",
    },
    CypRsidDef {
        rsid: "rs5030655",
        allele_name: "*6",
        alt_base: '-',
        is_deletion: true,
        activity: 0.0,
        function: "No function (frameshift)",
    },
    CypRsidDef {
        rsid: "rs1065852",
        allele_name: "*10",
        alt_base: 'T',
        is_deletion: false,
        activity: 0.5,
        function: "Decreased function",
    },
    CypRsidDef {
        rsid: "rs28371725",
        allele_name: "*41",
        alt_base: 'T',
        is_deletion: false,
        activity: 0.5,
        function: "Decreased function",
    },
    CypRsidDef {
        rsid: "rs28371706",
        allele_name: "*17",
        alt_base: 'T',
        is_deletion: false,
        activity: 0.5,
        function: "Decreased function",
    },
];

static CYP2C19_RSID_DEFS: &[CypRsidDef] = &[
    CypRsidDef {
        rsid: "rs4244285",
        allele_name: "*2",
        alt_base: 'A',
        is_deletion: false,
        activity: 0.0,
        function: "No function (splicing defect)",
    },
    CypRsidDef {
        rsid: "rs4986893",
        allele_name: "*3",
        alt_base: 'A',
        is_deletion: false,
        activity: 0.0,
        function: "No function (premature stop)",
    },
    CypRsidDef {
        rsid: "rs12248560",
        allele_name: "*17",
        alt_base: 'T',
        is_deletion: false,
        activity: 1.5,
        function: "Increased function",
    },
];

/// CYP enzyme diplotype calling result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CypDiplotype {
    pub gene: String,
    pub allele1: String,
    pub allele2: String,
    pub activity: f64,
    pub phenotype: MetabolizerPhenotype,
    /// Call confidence (accounts for genotyped fraction, not phase/CNV)
    pub confidence: CallConfidence,
    /// Number of defining rsids present on the genotyping panel
    pub rsids_genotyped: usize,
    /// Number of defining rsids that matched a non-reference allele
    pub rsids_matched: usize,
    /// Total defining rsids for this gene
    pub rsids_total: usize,
    /// Human-readable notes (warnings, limitations)
    pub notes: Vec<String>,
    /// Per-rsid detail lines for the report
    pub details: Vec<String>,
}

fn call_cyp_diplotype(
    gene: &str,
    defs: &[CypRsidDef],
    gts: &HashMap<String, String>,
) -> CypDiplotype {
    let mut alleles: Vec<(&str, f64)> = Vec::new();
    let mut details = Vec::new();
    let mut notes = Vec::new();
    let mut genotyped = 0usize;
    let mut matched = 0usize; // non-reference calls

    for def in defs {
        if let Some(gt) = gts.get(def.rsid) {
            genotyped += 1;
            if def.is_deletion {
                match gt.as_str() {
                    "DD" => {
                        matched += 1;
                        alleles.push((def.allele_name, def.activity));
                        alleles.push((def.allele_name, def.activity));
                        details.push(format!(
                            "  {}: {} -> homozygous {} ({})",
                            def.rsid, gt, def.allele_name, def.function
                        ));
                    }
                    "DI" => {
                        matched += 1;
                        alleles.push((def.allele_name, def.activity));
                        details.push(format!(
                            "  {}: {} -> heterozygous {} ({})",
                            def.rsid, gt, def.allele_name, def.function
                        ));
                    }
                    _ => details.push(format!(
                        "  {}: {} -> reference (no {})",
                        def.rsid, gt, def.allele_name
                    )),
                }
            } else {
                let alt = def.alt_base;
                let hom = format!("{}{}", alt, alt);
                if *gt == hom {
                    matched += 1;
                    alleles.push((def.allele_name, def.activity));
                    alleles.push((def.allele_name, def.activity));
                    details.push(format!(
                        "  {}: {} -> homozygous {} ({})",
                        def.rsid, gt, def.allele_name, def.function
                    ));
                } else if gt.contains(alt) {
                    matched += 1;
                    alleles.push((def.allele_name, def.activity));
                    details.push(format!(
                        "  {}: {} -> heterozygous {} ({})",
                        def.rsid, gt, def.allele_name, def.function
                    ));
                } else {
                    details.push(format!(
                        "  {}: {} -> reference (no {})",
                        def.rsid, gt, def.allele_name
                    ));
                }
            }
        } else {
            details.push(format!("  {}: not genotyped", def.rsid));
        }
    }

    // Confidence rules:
    //   Strong:      2+ rsids matched (non-ref) AND genotyped >= 50%
    //   Moderate:    1+ rsid matched AND genotyped >= 2, OR genotyped >= 50% (all ref)
    //   Weak:        only 1 rsid genotyped (ref or not)
    //   Unsupported: 0 genotyped
    let confidence = if genotyped == 0 {
        CallConfidence::Unsupported
    } else if matched >= 2 && genotyped * 2 >= defs.len() {
        CallConfidence::Strong
    } else if (matched >= 1 && genotyped >= 2) || genotyped * 2 >= defs.len() {
        CallConfidence::Moderate
    } else {
        CallConfidence::Weak
    };

    // Populate notes
    if confidence == CallConfidence::Unsupported {
        notes.push("Panel lacks all defining variants for this gene.".into());
    }
    if confidence == CallConfidence::Weak {
        notes.push(format!(
            "Only {}/{} defining rsids genotyped; call unreliable.",
            genotyped,
            defs.len()
        ));
    }
    notes.push("No phase or CNV resolution from genotyping array.".into());

    while alleles.len() < 2 {
        alleles.push(("*1", 1.0));
    }
    let total = alleles[0].1 + alleles[1].1;
    let phenotype = if total > 2.0 {
        MetabolizerPhenotype::UltraRapid
    } else if total >= 1.0 {
        MetabolizerPhenotype::Normal
    } else if total >= 0.5 {
        MetabolizerPhenotype::Intermediate
    } else {
        MetabolizerPhenotype::Poor
    };

    CypDiplotype {
        gene: gene.into(),
        allele1: alleles[0].0.into(),
        allele2: alleles[1].0.into(),
        activity: total,
        phenotype,
        confidence,
        rsids_genotyped: genotyped,
        rsids_matched: matched,
        rsids_total: defs.len(),
        notes,
        details,
    }
}

/// Call CYP2D6 diplotype from 23andMe genotypes
pub fn call_cyp2d6(gts: &HashMap<String, String>) -> CypDiplotype {
    call_cyp_diplotype("CYP2D6", CYP2D6_RSID_DEFS, gts)
}

/// Call CYP2C19 diplotype from 23andMe genotypes
pub fn call_cyp2c19(gts: &HashMap<String, String>) -> CypDiplotype {
    call_cyp_diplotype("CYP2C19", CYP2C19_RSID_DEFS, gts)
}

// ═══════════════════════════════════════════════════════════════════════
// Full Analysis Pipeline
// ═══════════════════════════════════════════════════════════════════════

/// Complete 23andMe analysis result
#[derive(Debug, Clone)]
pub struct GenotypeAnalysis {
    pub data: GenotypeData,
    pub cyp2d6: CypDiplotype,
    pub cyp2c19: CypDiplotype,
    pub cyp2d6_recs: Vec<DrugRecommendation>,
    pub cyp2c19_recs: Vec<DrugRecommendation>,
    pub health_variants: Vec<HealthVariantResult>,
    pub apoe: ApoeResult,
    pub mthfr: MthfrResult,
    pub pain: Option<PainProfile>,
    pub region_qc: Vec<RegionQc>,
    pub similarities: Vec<(String, String, f32)>,
    pub homozygous: usize,
    pub heterozygous: usize,
    pub indels: usize,
    pub het_ratio: f64,
    pub elapsed_ms: u128,
}

/// Run the full 7-stage 23andMe analysis pipeline.
pub fn analyze<R: std::io::Read>(reader: R) -> Result<GenotypeAnalysis> {
    let start = Instant::now();

    // Stage 1: Parse (with normalization)
    let data = parse_23andme(reader)?;
    let gts = data.genotype_map();

    // Stage 2: Panel signature & call-rate QC
    let regions = GENE_REGIONS_37; // TODO: select by data.build
    let mut region_qc = Vec::new();
    for reg in regions {
        let mut rsnps: Vec<&Snp> = data
            .snps
            .values()
            .filter(|s| {
                s.chromosome == reg.chromosome && s.position >= reg.start && s.position <= reg.end
            })
            .collect();
        rsnps.sort_by_key(|s| s.position);
        let het = rsnps
            .iter()
            .filter(|s| {
                let b = s.genotype.as_bytes();
                b.len() == 2 && b[0] != b[1]
            })
            .count();
        let het_rate = if rsnps.is_empty() {
            0.0
        } else {
            het as f64 / rsnps.len() as f64
        };
        let sig = signature_vector(&rsnps, 11, 512);
        region_qc.push(RegionQc {
            name: reg.name.into(),
            snp_count: rsnps.len(),
            het_count: het,
            het_rate,
            signature: sig,
        });
    }
    let mut similarities = Vec::new();
    for i in 0..region_qc.len() {
        for j in (i + 1)..region_qc.len() {
            let sim = cosine_sim(&region_qc[i].signature, &region_qc[j].signature);
            similarities.push((region_qc[i].name.clone(), region_qc[j].name.clone(), sim));
        }
    }

    // Stage 3: Variant classification (on normalized genotypes)
    let (mut hom, mut het, mut indel) = (0usize, 0usize, 0usize);
    for snp in data.snps.values() {
        let b = snp.genotype.as_bytes();
        if b.len() == 2 {
            let is_nuc = |c: u8| matches!(c, b'A' | b'C' | b'G' | b'T');
            if is_nuc(b[0]) && is_nuc(b[1]) {
                if b[0] == b[1] {
                    hom += 1;
                } else {
                    het += 1;
                }
            } else {
                // D/I markers
                indel += 1;
            }
        }
    }
    let het_ratio = if data.called() > 0 {
        het as f64 / data.called() as f64 * 100.0
    } else {
        0.0
    };

    // Stage 4: Pharmacogenomics (with confidence)
    let cyp2d6 = call_cyp2d6(&gts);
    let cyp2c19 = call_cyp2c19(&gts);
    // Conservative: only emit drug recs when confidence >= Moderate
    let cyp2d6_recs = if cyp2d6.confidence as u8 >= CallConfidence::Moderate as u8 {
        pharma::get_recommendations("CYP2D6", &cyp2d6.phenotype)
    } else {
        vec![]
    };
    let cyp2c19_recs = if cyp2c19.confidence as u8 >= CallConfidence::Moderate as u8 {
        pharma::get_recommendations("CYP2C19", &cyp2c19.phenotype)
    } else {
        vec![]
    };

    // Stage 5: Health variants
    let health_variants = health::analyze_health_variants(&gts);
    let apoe = health::determine_apoe(&gts);

    // Stage 6: Compound analysis
    let mthfr = health::analyze_mthfr(&gts);
    let pain = health::analyze_pain(&gts);

    Ok(GenotypeAnalysis {
        data,
        cyp2d6,
        cyp2c19,
        cyp2d6_recs,
        cyp2c19_recs,
        health_variants,
        apoe,
        mthfr,
        pain,
        region_qc,
        similarities,
        homozygous: hom,
        heterozygous: het,
        indels: indel,
        het_ratio,
        elapsed_ms: start.elapsed().as_millis(),
    })
}

// ═══════════════════════════════════════════════════════════════════════
// Stage 7: Report
// ═══════════════════════════════════════════════════════════════════════

/// Format the analysis as a human-readable text report.
pub fn format_report(a: &GenotypeAnalysis) -> String {
    let mut r = String::with_capacity(8192);
    let sep = "=".repeat(80);
    let thin = "-".repeat(55);

    let _ = writeln!(r, "{}", sep);
    let _ = writeln!(
        r,
        "  rvDNA: 23andMe Genomic Analysis Pipeline (Native Rust)"
    );
    let _ = writeln!(
        r,
        "  https://github.com/ruvnet/ruvector/tree/main/examples/dna"
    );
    let _ = writeln!(r, "{}", sep);

    // Stage 1
    let _ = writeln!(r, "\n--- Stage 1: Loading 23andMe Genotype Data ---");
    let _ = writeln!(r, "  Total markers:  {:>10}", fmt_num(a.data.total_markers));
    let _ = writeln!(r, "  Called:          {:>10}", fmt_num(a.data.called()));
    let _ = writeln!(r, "  No-calls:       {:>10}", fmt_num(a.data.no_calls));
    let cr = a.data.called() as f64 / a.data.total_markers.max(1) as f64 * 100.0;
    let _ = writeln!(r, "  Call rate:       {:>9.1}%", cr);
    let _ = writeln!(r, "  Genome build:   {:?}", a.data.build);
    if a.data.build == GenomeBuild::Unknown {
        let _ = writeln!(
            r,
            "  WARNING: Build not detected. Coordinates assume GRCh37."
        );
    }
    let _ = writeln!(r, "\n  Chromosome distribution:");
    for c in (1..=22)
        .map(|i| i.to_string())
        .chain(["X", "Y", "MT"].iter().map(|s| s.to_string()))
    {
        if let Some(&n) = a.data.chr_counts.get(&c) {
            let _ = writeln!(
                r,
                "    Chr {:>2}: {:>6} {}",
                c,
                fmt_num(n),
                "|".repeat((n / 1500).min(40))
            );
        }
    }

    // Stage 2 (QC)
    let _ = writeln!(r, "\n--- Stage 2: Panel Signature & Call Rate QC ---");
    let _ = writeln!(
        r,
        "  NOTE: Signatures are genotype-panel fingerprints, not biological k-mers."
    );
    let _ = writeln!(
        r,
        "  {:8} {:>5} {:>5} {:>7}",
        "Region", "SNPs", "Het", "Het%"
    );
    for q in &a.region_qc {
        let _ = writeln!(
            r,
            "  {:8} {:>5} {:>5} {:>6.1}%",
            q.name,
            q.snp_count,
            q.het_count,
            q.het_rate * 100.0
        );
    }
    let _ = writeln!(r, "\n  Cross-region panel similarity (cosine):");
    for (g1, g2, sim) in &a.similarities {
        let _ = writeln!(r, "    {:8} vs {:8}: {:.4}", g1, g2, sim);
    }

    // Stage 3
    let _ = writeln!(r, "\n--- Stage 3: Variant Classification Summary ---");
    let _ = writeln!(r, "  Homozygous:    {:>8}", fmt_num(a.homozygous));
    let _ = writeln!(r, "  Heterozygous:  {:>8}", fmt_num(a.heterozygous));
    let _ = writeln!(
        r,
        "  Indels (D/I):  {:>8}  (panel-dependent; treat as optional)",
        fmt_num(a.indels)
    );
    let _ = writeln!(
        r,
        "  Het ratio:     {:>7.1}% (typical: 25-35%)",
        a.het_ratio
    );

    // Stage 4
    let _ = writeln!(r, "\n--- Stage 4: Pharmacogenomic Analysis ---");
    let _ = writeln!(
        r,
        "  NOTE: Diplotypes are approximate — 23andMe lacks phase and CNV data."
    );
    format_cyp(&mut r, &a.cyp2d6, &a.cyp2d6_recs, &thin);
    format_cyp(&mut r, &a.cyp2c19, &a.cyp2c19_recs, &thin);

    // Stage 5
    let _ = writeln!(r, "\n--- Stage 5: Health Variant Analysis ---");
    let _ = writeln!(r, "\n  -- APOE Genotype (Alzheimer's Risk) {}", thin);
    let _ = writeln!(
        r,
        "  rs429358: {}  rs7412: {}",
        a.apoe.rs429358, a.apoe.rs7412
    );
    let _ = writeln!(r, "  APOE Status: {}", a.apoe.genotype);
    for (cat, genes) in health::variant_categories() {
        let hits: Vec<_> = a
            .health_variants
            .iter()
            .filter(|v| genes.contains(&v.gene.as_str()))
            .collect();
        if hits.is_empty() {
            continue;
        }
        let _ = writeln!(r, "\n  -- {} {}", cat, thin);
        for v in hits {
            let _ = writeln!(r, "  {} ({} - {})", v.rsid, v.gene, v.name);
            let _ = writeln!(r, "    Genotype:       {}", v.genotype);
            let _ = writeln!(r, "    Interpretation: {}", v.interpretation);
            let _ = writeln!(r, "    Significance:   {}", v.clinical_significance);
        }
    }

    // Stage 6
    let _ = writeln!(r, "\n--- Stage 6: Compound Genotype Analysis ---");
    let _ = writeln!(r, "\n  -- MTHFR Compound Status {}", thin);
    let _ = writeln!(r, "  C677T (rs1801133):  {}", a.mthfr.c677t);
    let _ = writeln!(r, "  A1298C (rs1801131): {}", a.mthfr.a1298c);
    let _ = writeln!(r, "  Assessment: {}", a.mthfr.assessment);
    if let Some(ref p) = a.pain {
        let _ = writeln!(r, "\n  -- Pain Sensitivity Profile {}", thin);
        let _ = writeln!(r, "  COMT (rs4680):     {} -> {}", p.comt, p.comt_note);
        let _ = writeln!(r, "  OPRM1 (rs1799971): {} -> {}", p.oprm1, p.oprm1_note);
        let _ = writeln!(r, "  Combined sensitivity: {}", p.label);
        if p.score >= 2 {
            let _ = writeln!(
                r,
                "  Note: May need higher opioid doses or alternative pain management."
            );
        }
    }

    // Summary
    let _ = writeln!(r, "\n{}", sep);
    let _ = writeln!(r, "  PIPELINE SUMMARY");
    let _ = writeln!(r, "{}", sep);
    let _ = writeln!(r, "  Markers analyzed:     {}", fmt_num(a.data.called()));
    let _ = writeln!(
        r,
        "  Pharmacogenes:        CYP2D6 ({:?}, {:?}), CYP2C19 ({:?}, {:?})",
        a.cyp2d6.phenotype, a.cyp2d6.confidence, a.cyp2c19.phenotype, a.cyp2c19.confidence
    );
    let _ = writeln!(r, "  APOE status:          {}", a.apoe.genotype);
    let _ = writeln!(
        r,
        "  Health variants:      {} analyzed",
        a.health_variants.len()
    );
    let _ = writeln!(
        r,
        "  Drug recommendations: {} generated",
        a.cyp2d6_recs.len() + a.cyp2c19_recs.len()
    );
    let _ = writeln!(r, "  Total pipeline time:  {}ms", a.elapsed_ms);
    let _ = writeln!(r);
    let _ = writeln!(
        r,
        "  DISCLAIMER: This analysis is for RESEARCH/EDUCATIONAL purposes only."
    );
    let _ = writeln!(
        r,
        "  It is NOT a medical diagnosis. Consult a healthcare provider or genetic"
    );
    let _ = writeln!(
        r,
        "  counselor before making any medical decisions based on these results."
    );
    let _ = writeln!(r, "{}", sep);
    r
}

fn format_cyp(r: &mut String, d: &CypDiplotype, recs: &[DrugRecommendation], thin: &str) {
    let _ = writeln!(r, "\n  -- {} (Drug Metabolism Enzyme) {}", d.gene, thin);
    for line in &d.details {
        let _ = writeln!(r, "{}", line);
    }
    let _ = writeln!(r, "\n  Diplotype:   {}/{}", d.allele1, d.allele2);
    let _ = writeln!(r, "  Activity:    {:.1}", d.activity);
    let tentative =
        if d.confidence == CallConfidence::Weak || d.confidence == CallConfidence::Unsupported {
            " [TENTATIVE]"
        } else {
            ""
        };
    let _ = writeln!(r, "  Phenotype:   {:?}{}", d.phenotype, tentative);
    let _ = writeln!(
        r,
        "  Confidence:  {:?}  ({}/{} rsids genotyped, {} matched)",
        d.confidence, d.rsids_genotyped, d.rsids_total, d.rsids_matched
    );
    for note in &d.notes {
        let _ = writeln!(r, "  Note: {}", note);
    }
    if !recs.is_empty() {
        let _ = writeln!(r, "\n  Drug Recommendations (CPIC):");
        for rec in recs {
            let dose = if rec.dose_factor > 0.0 {
                format!("{:.0}%", rec.dose_factor * 100.0)
            } else {
                "AVOID".into()
            };
            let _ = writeln!(r, "    - {}: {}", rec.drug, rec.recommendation);
            let _ = writeln!(r, "      Dose adjustment: {}", dose);
        }
    } else if d.confidence == CallConfidence::Weak || d.confidence == CallConfidence::Unsupported {
        let _ = writeln!(r, "\n  Drug recommendations withheld (confidence too low).");
    }
}

fn fmt_num(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_23andme() -> &'static str {
        "# rsid\tchromosome\tposition\tgenotype\n\
         # build 37\n\
         rs429358\t19\t44908684\tTT\n\
         rs7412\t19\t44908822\tCT\n\
         rs4680\t22\t19951271\tAG\n\
         rs1799971\t6\t154360797\tAA\n\
         rs762551\t15\t75041917\tAA\n\
         rs1801133\t1\t11856378\tAG\n\
         rs1801131\t1\t11854476\tTT\n\
         rs3892097\t22\t42130692\tCC\n\
         rs4244285\t10\t96541616\tGG\n\
         rs12345678\t1\t100000\t--\n"
    }

    /// Same data but with reversed/mixed-case genotypes
    fn sample_23andme_messy() -> &'static str {
        "# rsid\tchromosome\tposition\tgenotype\n\
         rs429358\t19\t44908684\ttt\n\
         rs7412\t19\t44908822\tTC\n\
         rs4680\t22\t19951271\tga\n\
         rs1799971\t6\t154360797\tAA\n\
         rs762551\t15\t75041917\tAa\n\
         rs1801133\t1\t11856378\tGA\n\
         rs1801131\t1\t11854476\tTT\n\
         rs3892097\t22\t42130692\tCC\n\
         rs4244285\t10\t96541616\tGG\n\
         rs12345678\t1\t100000\t--\n"
    }

    #[test]
    fn test_normalize_genotype() {
        assert_eq!(normalize_genotype("ag"), "AG");
        assert_eq!(normalize_genotype("TC"), "CT");
        assert_eq!(normalize_genotype("GA"), "AG");
        assert_eq!(normalize_genotype("AA"), "AA");
        assert_eq!(normalize_genotype("ID"), "DI");
        assert_eq!(normalize_genotype("DI"), "DI");
        assert_eq!(normalize_genotype(" ct "), "CT");
    }

    #[test]
    fn test_parse_detects_build() {
        let data = parse_23andme(sample_23andme().as_bytes()).unwrap();
        assert_eq!(data.build, GenomeBuild::GRCh37);
    }

    #[test]
    fn test_parse_counts() {
        let data = parse_23andme(sample_23andme().as_bytes()).unwrap();
        assert_eq!(data.total_markers, 10);
        assert_eq!(data.no_calls, 1);
        assert_eq!(data.called(), 9);
    }

    #[test]
    fn test_full_pipeline() {
        let a = analyze(sample_23andme().as_bytes()).unwrap();
        assert!(a.apoe.genotype.contains("e2/e3"));
        assert_eq!(a.cyp2d6.allele1, "*1");
        assert_eq!(a.cyp2d6.phenotype, MetabolizerPhenotype::Normal);
    }

    #[test]
    fn test_cyp2d6_poor() {
        let mut gts = HashMap::new();
        gts.insert("rs3892097".into(), "TT".into());
        let d = call_cyp2d6(&gts);
        assert_eq!(d.allele1, "*4");
        assert_eq!(d.allele2, "*4");
        assert_eq!(d.phenotype, MetabolizerPhenotype::Poor);
    }

    #[test]
    fn test_cyp_confidence_levels() {
        // No markers → Unsupported
        let d = call_cyp2d6(&HashMap::new());
        assert_eq!(d.confidence, CallConfidence::Unsupported);
        assert_eq!(d.rsids_genotyped, 0);
        assert_eq!(d.rsids_matched, 0);
        assert_eq!(d.rsids_total, 6);

        // 1 of 6 markers (reference) → Weak
        let mut gts = HashMap::new();
        gts.insert("rs3892097".into(), "CC".into());
        let d = call_cyp2d6(&gts);
        assert_eq!(d.confidence, CallConfidence::Weak);
        assert_eq!(d.rsids_genotyped, 1);
        assert_eq!(d.rsids_matched, 0);

        // 1 non-ref match only → still Weak (only 1 genotyped)
        let mut gts_one = HashMap::new();
        gts_one.insert("rs3892097".into(), "TT".into());
        let d = call_cyp2d6(&gts_one);
        assert_eq!(d.confidence, CallConfidence::Weak);
        assert_eq!(d.rsids_matched, 1);

        // 1 non-ref + 2 genotyped total → Moderate
        gts_one.insert("rs1065852".into(), "CC".into());
        let d = call_cyp2d6(&gts_one);
        assert_eq!(d.confidence, CallConfidence::Moderate);
        assert_eq!(d.rsids_genotyped, 2);
        assert_eq!(d.rsids_matched, 1);

        // 3 of 6 genotyped (all ref) → Moderate (>=50%)
        gts.insert("rs1065852".into(), "CC".into());
        gts.insert("rs28371725".into(), "CC".into());
        let d = call_cyp2d6(&gts);
        assert_eq!(d.confidence, CallConfidence::Moderate);

        // 2 non-ref matches + >=50% genotyped → Strong
        let mut gts_strong = HashMap::new();
        gts_strong.insert("rs3892097".into(), "TT".into()); // *4 hom
        gts_strong.insert("rs1065852".into(), "CT".into()); // *10 het
        gts_strong.insert("rs28371725".into(), "CC".into()); // ref
        let d = call_cyp2d6(&gts_strong);
        assert_eq!(d.confidence, CallConfidence::Strong);
        assert_eq!(d.rsids_matched, 2);

        // All 6 genotyped, all ref → Moderate (no matches despite coverage)
        let mut gts_all_ref = HashMap::new();
        for rsid in [
            "rs3892097",
            "rs35742686",
            "rs5030655",
            "rs1065852",
            "rs28371725",
            "rs28371706",
        ] {
            gts_all_ref.insert(rsid.into(), "CC".into());
        }
        let d = call_cyp2d6(&gts_all_ref);
        assert_eq!(d.confidence, CallConfidence::Moderate);
        assert_eq!(d.rsids_genotyped, 6);
        assert_eq!(d.rsids_matched, 0);
    }

    /// Acceptance test per user spec: CYP confidence gating
    #[test]
    fn test_cyp_confidence_gating_acceptance() {
        // Only one CYP2D6 rsid present with ref genotype → Weak, no CPIC recs
        let mut gts = HashMap::new();
        gts.insert("rs3892097".into(), "CC".into());
        let d = call_cyp2d6(&gts);
        assert_eq!(d.confidence, CallConfidence::Weak);
        // Recs should be empty when gated at Moderate
        let recs = if d.confidence as u8 >= CallConfidence::Moderate as u8 {
            pharma::get_recommendations("CYP2D6", &d.phenotype)
        } else {
            vec![]
        };
        assert!(recs.is_empty());

        // rs3892097 TT (hom *4) + one more genotyped → Moderate, Poor reported
        let mut gts2 = HashMap::new();
        gts2.insert("rs3892097".into(), "TT".into());
        gts2.insert("rs1065852".into(), "CC".into());
        let d = call_cyp2d6(&gts2);
        assert!(d.confidence as u8 >= CallConfidence::Moderate as u8);
        assert_eq!(d.phenotype, MetabolizerPhenotype::Poor);
        let recs = pharma::get_recommendations("CYP2D6", &d.phenotype);
        assert!(!recs.is_empty());
    }

    /// Acceptance test: reversed/mixed-case genotypes produce identical results
    #[test]
    fn test_normalization_acceptance() {
        let clean = analyze(sample_23andme().as_bytes()).unwrap();
        let messy = analyze(sample_23andme_messy().as_bytes()).unwrap();

        // APOE must match
        assert_eq!(clean.apoe.genotype, messy.apoe.genotype);
        // CYP diplotypes must match
        assert_eq!(clean.cyp2d6.allele1, messy.cyp2d6.allele1);
        assert_eq!(clean.cyp2d6.phenotype, messy.cyp2d6.phenotype);
        assert_eq!(clean.cyp2c19.phenotype, messy.cyp2c19.phenotype);
        // Health variant count and genotypes must match
        assert_eq!(clean.health_variants.len(), messy.health_variants.len());
        for (c, m) in clean
            .health_variants
            .iter()
            .zip(messy.health_variants.iter())
        {
            assert_eq!(c.rsid, m.rsid);
            assert_eq!(c.genotype, m.genotype);
            assert_eq!(c.clinical_significance, m.clinical_significance);
        }
        // MTHFR must match
        assert_eq!(clean.mthfr.assessment, messy.mthfr.assessment);
    }

    #[test]
    fn test_report_generation() {
        let a = analyze(sample_23andme().as_bytes()).unwrap();
        let report = format_report(&a);
        assert!(report.contains("Stage 1"));
        assert!(report.contains("Panel Signature"));
        assert!(report.contains("Confidence"));
        assert!(report.contains("PIPELINE SUMMARY"));
    }

    #[test]
    fn test_fmt_num() {
        assert_eq!(fmt_num(596007), "596,007");
        assert_eq!(fmt_num(0), "0");
        assert_eq!(fmt_num(1000), "1,000");
    }
}
