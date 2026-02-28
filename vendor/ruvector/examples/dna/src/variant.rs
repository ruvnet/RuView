//! Variant calling module for DNA analysis
//!
//! Provides SNP and indel calling from pileup data.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Pileup column representing reads aligned at a single position
#[derive(Debug, Clone)]
pub struct PileupColumn {
    /// Observed bases from aligned reads
    pub bases: Vec<u8>,
    /// Quality scores for each base
    pub qualities: Vec<u8>,
    /// Genomic position
    pub position: u64,
    /// Chromosome number
    pub chromosome: u8,
}

/// Genotype classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Genotype {
    /// Homozygous reference (0/0)
    HomRef,
    /// Heterozygous (0/1)
    Het,
    /// Homozygous alternate (1/1)
    HomAlt,
}

/// Variant filter status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterStatus {
    /// Passed all filters
    Pass,
    /// Failed quality filter
    LowQuality,
    /// Failed depth filter
    LowDepth,
}

/// Called variant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantCall {
    /// Chromosome number
    pub chromosome: u8,
    /// Genomic position
    pub position: u64,
    /// Reference allele
    pub ref_allele: u8,
    /// Alternate allele
    pub alt_allele: u8,
    /// Variant quality (Phred-scaled)
    pub quality: f64,
    /// Genotype call
    pub genotype: Genotype,
    /// Total read depth
    pub depth: usize,
    /// Alternate allele depth
    pub allele_depth: usize,
    /// Filter status
    pub filter_status: FilterStatus,
}

/// Variant caller configuration
#[derive(Debug, Clone)]
pub struct VariantCallerConfig {
    /// Minimum base quality to consider
    pub min_quality: u8,
    /// Minimum read depth
    pub min_depth: usize,
    /// Minimum alternate allele frequency for heterozygous call
    pub het_threshold: f64,
    /// Minimum alternate allele frequency for homozygous alt call
    pub hom_alt_threshold: f64,
}

impl Default for VariantCallerConfig {
    fn default() -> Self {
        Self {
            min_quality: 20,
            min_depth: 5,
            het_threshold: 0.2,
            hom_alt_threshold: 0.8,
        }
    }
}

/// Variant caller that processes pileup data to call SNPs
pub struct VariantCaller {
    config: VariantCallerConfig,
}

impl VariantCaller {
    /// Create a new variant caller with the given configuration
    pub fn new(config: VariantCallerConfig) -> Self {
        Self { config }
    }

    /// Call a SNP at a single pileup position
    ///
    /// Returns `Some(VariantCall)` if a variant is detected, `None` if all reads
    /// match the reference or depth is insufficient.
    pub fn call_snp(&self, pileup: &PileupColumn, reference_base: u8) -> Option<VariantCall> {
        let ref_base = reference_base.to_ascii_uppercase();

        // Count alleles (only high-quality bases)
        let mut allele_counts: HashMap<u8, usize> = HashMap::new();
        for (i, &base) in pileup.bases.iter().enumerate() {
            let qual = pileup.qualities.get(i).copied().unwrap_or(0);
            if qual >= self.config.min_quality {
                *allele_counts.entry(base.to_ascii_uppercase()).or_insert(0) += 1;
            }
        }

        let total_depth: usize = allele_counts.values().sum();
        if total_depth < self.config.min_depth {
            return None;
        }

        // Find the most common non-reference allele
        let mut best_alt: Option<(u8, usize)> = None;
        for (&allele, &count) in &allele_counts {
            if allele != ref_base {
                if best_alt.map_or(true, |(_, best_count)| count > best_count) {
                    best_alt = Some((allele, count));
                }
            }
        }

        let (alt_allele, alt_count) = best_alt?;
        let alt_freq = alt_count as f64 / total_depth as f64;

        if alt_freq < self.config.het_threshold {
            return None;
        }

        let genotype = if alt_freq >= self.config.hom_alt_threshold {
            Genotype::HomAlt
        } else {
            Genotype::Het
        };

        // Phred-scaled quality estimate
        let quality = -10.0 * (1.0 - alt_freq).max(1e-10).log10() * (alt_count as f64);

        Some(VariantCall {
            chromosome: pileup.chromosome,
            position: pileup.position,
            ref_allele: ref_base,
            alt_allele,
            quality,
            genotype,
            depth: total_depth,
            allele_depth: alt_count,
            filter_status: FilterStatus::Pass,
        })
    }

    /// Detect insertions/deletions from pileup data
    ///
    /// Looks for gaps (represented as b'-') in the pileup bases that indicate
    /// indels relative to the reference.
    pub fn call_indel(
        &self,
        pileup: &PileupColumn,
        reference_base: u8,
        next_ref_bases: &[u8],
    ) -> Option<VariantCall> {
        let ref_base = reference_base.to_ascii_uppercase();
        let mut del_count = 0usize;
        let mut ins_count = 0usize;

        for (i, &base) in pileup.bases.iter().enumerate() {
            let qual = pileup.qualities.get(i).copied().unwrap_or(0);
            if qual < self.config.min_quality {
                continue;
            }
            if base == b'-' || base == b'*' {
                del_count += 1;
            } else if base == b'+' {
                ins_count += 1;
            }
        }

        let total = pileup.bases.len();
        if total < self.config.min_depth {
            return None;
        }

        // Check for deletion
        if del_count > 0 {
            let del_freq = del_count as f64 / total as f64;
            if del_freq >= self.config.het_threshold {
                let genotype = if del_freq >= self.config.hom_alt_threshold {
                    Genotype::HomAlt
                } else {
                    Genotype::Het
                };
                let quality = -10.0 * (1.0 - del_freq).max(1e-10).log10() * (del_count as f64);
                return Some(VariantCall {
                    chromosome: pileup.chromosome,
                    position: pileup.position,
                    ref_allele: ref_base,
                    alt_allele: b'-',
                    quality,
                    genotype,
                    depth: total,
                    allele_depth: del_count,
                    filter_status: FilterStatus::Pass,
                });
            }
        }

        // Check for insertion
        if ins_count > 0 {
            let ins_freq = ins_count as f64 / total as f64;
            if ins_freq >= self.config.het_threshold {
                let genotype = if ins_freq >= self.config.hom_alt_threshold {
                    Genotype::HomAlt
                } else {
                    Genotype::Het
                };
                let quality = -10.0 * (1.0 - ins_freq).max(1e-10).log10() * (ins_count as f64);
                return Some(VariantCall {
                    chromosome: pileup.chromosome,
                    position: pileup.position,
                    ref_allele: ref_base,
                    alt_allele: b'+',
                    quality,
                    genotype,
                    depth: total,
                    allele_depth: ins_count,
                    filter_status: FilterStatus::Pass,
                });
            }
        }

        None
    }

    /// Apply quality and depth filters to a list of variant calls
    pub fn filter_variants(&self, calls: &mut [VariantCall]) {
        for call in calls.iter_mut() {
            if call.quality < self.config.min_quality as f64 {
                call.filter_status = FilterStatus::LowQuality;
            } else if call.depth < self.config.min_depth {
                call.filter_status = FilterStatus::LowDepth;
            }
        }
    }

    /// Generate VCF-formatted output for variant calls
    pub fn to_vcf(&self, calls: &[VariantCall], sample_name: &str) -> String {
        let mut vcf = String::new();
        vcf.push_str("##fileformat=VCFv4.3\n");
        vcf.push_str(&format!("##source=RuVectorDNA\n"));
        vcf.push_str("#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\t");
        vcf.push_str(sample_name);
        vcf.push('\n');

        for call in calls {
            let filter = match call.filter_status {
                FilterStatus::Pass => "PASS",
                FilterStatus::LowQuality => "LowQual",
                FilterStatus::LowDepth => "LowDepth",
            };
            let gt = match call.genotype {
                Genotype::HomRef => "0/0",
                Genotype::Het => "0/1",
                Genotype::HomAlt => "1/1",
            };
            vcf.push_str(&format!(
                "chr{}\t{}\t.\t{}\t{}\t{:.1}\t{}\tDP={};AF={:.3}\tGT:DP:AD\t{}:{}:{}\n",
                call.chromosome,
                call.position,
                call.ref_allele as char,
                call.alt_allele as char,
                call.quality,
                filter,
                call.depth,
                call.allele_depth as f64 / call.depth as f64,
                gt,
                call.depth,
                call.allele_depth,
            ));
        }

        vcf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variant_caller_creation() {
        let config = VariantCallerConfig::default();
        let _caller = VariantCaller::new(config);
    }

    #[test]
    fn test_snp_calling() {
        let caller = VariantCaller::new(VariantCallerConfig::default());
        let pileup = PileupColumn {
            bases: vec![b'G'; 15],
            qualities: vec![40; 15],
            position: 1000,
            chromosome: 1,
        };

        let call = caller.call_snp(&pileup, b'A');
        assert!(call.is_some());
        let call = call.unwrap();
        assert_eq!(call.genotype, Genotype::HomAlt);
    }
}
