//! Life Candidate Scoring Pipeline (L0-L2) using RVF
//!
//! Demonstrates spectral disequilibrium analysis from ADR-040:
//!   L0 Ingest:              Synthetic JWST NIRSpec spectra with absorption features
//!   L1 Feature Extraction:  Molecule identification + co-occurrence edges
//!   L2 Disequilibrium:      Score imbalance, repeatability, penalties
//!
//! Output: Ranked life candidate list with uncertainty and follow-up
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG, WITNESS_SEG
//!
//! Run: cargo run --example life_candidate

use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore,
};
use rvf_runtime::filter::FilterValue;
use rvf_runtime::options::DistanceMetric;
use rvf_crypto::{create_witness_chain, verify_witness_chain, shake256_256, WitnessEntry};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// LCG helpers
// ---------------------------------------------------------------------------

fn lcg_next(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *state
}

fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed.wrapping_add(1);
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

fn lcg_f64(state: &mut u64) -> f64 {
    lcg_next(state);
    (*state >> 11) as f64 / ((1u64 << 53) as f64)
}

// ---------------------------------------------------------------------------
// ADR-040 domain types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Molecule {
    name: &'static str,
    wavelength_um: f64,
    width_um: f64,
}

const MOLECULES: &[Molecule] = &[
    Molecule { name: "H2O",  wavelength_um: 1.4,  width_um: 0.15 },
    Molecule { name: "CO2",  wavelength_um: 2.0,  width_um: 0.2  },
    Molecule { name: "CH4",  wavelength_um: 2.3,  width_um: 0.1  },
    Molecule { name: "O3",   wavelength_um: 0.6,  width_um: 0.08 },
    Molecule { name: "NH3",  wavelength_um: 3.0,  width_um: 0.12 },
];

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Spectrum {
    target_id: u64,
    target_name: String,
    wavelengths: Vec<f64>,
    flux: Vec<f64>,
    detected_molecules: Vec<String>,
}

#[derive(Debug, Clone)]
struct CoOccurrenceEdge {
    molecule_a: String,
    molecule_b: String,
    confidence: f64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct LifeScore {
    target_id: u64,
    target_name: String,
    disequilibrium: f64,
    repeatability: f64,
    contamination_penalty: f64,
    stellar_confound_penalty: f64,
    total_score: f64,
    uncertainty: f64,
    num_molecules: usize,
    follow_up: Vec<&'static str>,
}

// ---------------------------------------------------------------------------
// L0: Synthetic spectrum generation
// ---------------------------------------------------------------------------

fn generate_spectrum(target_id: u64, seed: u64) -> Spectrum {
    let mut rng = seed.wrapping_add(target_id * 6271);

    let target_names = [
        "TRAPPIST-1e", "TRAPPIST-1f", "K2-18b", "LHS-1140b",
        "Proxima-Cen-b", "TOI-700d", "Kepler-442b", "GJ-1002b",
        "Wolf-1061c", "Ross-128b", "GJ-273b", "Teegarden-b",
        "LP-890-9c", "TOI-1452b", "GJ-357d",
    ];
    let name = target_names[target_id as usize % target_names.len()].to_string();

    // Wavelength grid: 0.5 to 5.0 um in 200 bins
    let num_bins = 200;
    let wl_min = 0.5;
    let wl_max = 5.0;
    let mut wavelengths = Vec::with_capacity(num_bins);
    let mut flux = Vec::with_capacity(num_bins);

    for i in 0..num_bins {
        let wl = wl_min + (wl_max - wl_min) * (i as f64 / (num_bins - 1) as f64);
        wavelengths.push(wl);

        // Blackbody-ish continuum (simplified)
        let continuum = 1.0 / (1.0 + ((wl - 2.0) / 2.0).powi(2));
        let noise = (lcg_f64(&mut rng) - 0.5) * 0.02;
        flux.push(continuum + noise);
    }

    // Inject molecule absorption features
    let mut detected = Vec::new();
    for mol in MOLECULES {
        // Each target has a random chance of having each molecule
        let has_molecule = lcg_f64(&mut rng) > 0.35;
        if has_molecule {
            let depth = 0.05 + lcg_f64(&mut rng) * 0.3;
            for (i, &wl) in wavelengths.iter().enumerate() {
                let dist = (wl - mol.wavelength_um).abs();
                if dist < mol.width_um {
                    let gaussian = (-0.5 * (dist / (mol.width_um * 0.4)).powi(2)).exp();
                    flux[i] -= depth * gaussian;
                }
            }
            detected.push(mol.name.to_string());
        }
    }

    Spectrum {
        target_id,
        target_name: name,
        wavelengths,
        flux,
        detected_molecules: detected,
    }
}

// ---------------------------------------------------------------------------
// L1: Feature extraction + co-occurrence edges
// ---------------------------------------------------------------------------

fn extract_features(spectrum: &Spectrum) -> Vec<CoOccurrenceEdge> {
    let mut edges = Vec::new();
    let mols = &spectrum.detected_molecules;
    let mut rng: u64 = 0xBEEF + spectrum.target_id;

    for i in 0..mols.len() {
        for j in (i + 1)..mols.len() {
            let confidence = 0.5 + lcg_f64(&mut rng) * 0.5;
            edges.push(CoOccurrenceEdge {
                molecule_a: mols[i].clone(),
                molecule_b: mols[j].clone(),
                confidence,
            });
        }
    }
    edges
}

// ---------------------------------------------------------------------------
// L2: Disequilibrium scoring
// ---------------------------------------------------------------------------

// Equilibrium expectation: which molecule pairs would be expected together
fn equilibrium_expectation(a: &str, b: &str) -> f64 {
    match (a, b) {
        ("CO2", "H2O") | ("H2O", "CO2") => 0.8,  // common together
        ("O3", "CH4") | ("CH4", "O3") => 0.05,    // disequilibrium pair!
        ("H2O", "O3") | ("O3", "H2O") => 0.3,
        ("NH3", "CH4") | ("CH4", "NH3") => 0.4,
        ("CO2", "CH4") | ("CH4", "CO2") => 0.2,
        _ => 0.5,
    }
}

fn score_life_candidate(
    spectrum: &Spectrum,
    edges: &[CoOccurrenceEdge],
    observations: usize,
) -> LifeScore {
    let mut rng: u64 = 0xFACE + spectrum.target_id * 31;

    // Disequilibrium: how far are co-occurrences from equilibrium expectations?
    let mut disequilibrium = 0.0;
    if !edges.is_empty() {
        for edge in edges {
            let expected = equilibrium_expectation(&edge.molecule_a, &edge.molecule_b);
            let observed = edge.confidence;
            disequilibrium += (observed - expected).abs();
        }
        disequilibrium /= edges.len() as f64;
    }

    // Repeatability: more observations = more reliable
    let repeatability = 1.0 - (1.0 / (1.0 + observations as f64 * 0.3));

    // Contamination risk penalty: random small factor
    let contamination_penalty = lcg_f64(&mut rng) * 0.15;

    // Stellar activity confound penalty
    let stellar_confound_penalty = lcg_f64(&mut rng) * 0.1;

    // Total score
    let raw = disequilibrium * 0.4 + repeatability * 0.3
        - contamination_penalty * 0.15
        - stellar_confound_penalty * 0.15;
    let total_score = raw.max(0.0).min(1.0);

    // Uncertainty decreases with more molecules and observations
    let uncertainty = 0.5 / (1.0 + spectrum.detected_molecules.len() as f64 * 0.3 + observations as f64 * 0.1);

    // Follow-up recommendations
    let mut follow_up: Vec<&'static str> = Vec::new();
    if spectrum.detected_molecules.len() < 3 {
        follow_up.push("additional_nirspec_observations");
    }
    if disequilibrium > 0.3 {
        follow_up.push("high_resolution_spectroscopy");
    }
    if contamination_penalty > 0.1 {
        follow_up.push("contamination_check");
    }
    if follow_up.is_empty() {
        follow_up.push("routine_monitoring");
    }

    LifeScore {
        target_id: spectrum.target_id,
        target_name: spectrum.target_name.clone(),
        disequilibrium,
        repeatability,
        contamination_penalty,
        stellar_confound_penalty,
        total_score,
        uncertainty,
        num_molecules: spectrum.detected_molecules.len(),
        follow_up,
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    println!("=== Life Candidate Scoring Pipeline (L0-L2) ===\n");

    let dim = 64;
    let num_targets = 15;

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("life_candidate.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");

    // ====================================================================
    // L0: Ingest — generate synthetic JWST NIRSpec spectra
    // ====================================================================
    println!("--- L0. Ingest: Synthetic JWST NIRSpec Spectra ---");

    let spectra: Vec<Spectrum> = (0..num_targets)
        .map(|i| generate_spectrum(i, 42))
        .collect();

    // Store spectral windows as embeddings
    let mut all_vectors: Vec<Vec<f32>> = Vec::new();
    let mut all_ids: Vec<u64> = Vec::new();
    let mut all_metadata: Vec<MetadataEntry> = Vec::new();
    let mut global_id = 0u64;

    // Each spectrum gets split into wavelength bands
    let bands = ["vis", "nir-j", "nir-h", "nir-k", "mir"];
    let band_ranges: &[(f64, f64)] = &[
        (0.5, 0.9), (0.9, 1.4), (1.4, 1.8), (1.8, 2.5), (2.5, 5.0),
    ];

    for spectrum in &spectra {
        for (band_idx, band_name) in bands.iter().enumerate() {
            let (wl_lo, wl_hi) = band_ranges[band_idx];

            // Determine dominant molecule in this band
            let mut dominant_mol = "none";
            for mol in MOLECULES {
                if mol.wavelength_um >= wl_lo && mol.wavelength_um < wl_hi {
                    if spectrum.detected_molecules.contains(&mol.name.to_string()) {
                        dominant_mol = mol.name;
                        break;
                    }
                }
            }

            let vec = random_vector(dim, global_id * 13 + spectrum.target_id * 7);
            all_vectors.push(vec);
            all_ids.push(global_id);

            // Metadata: instrument (0), target_id (1), wavelength_band (2), molecule (3)
            all_metadata.push(MetadataEntry {
                field_id: 0,
                value: MetadataValue::String("jwst-nirspec".to_string()),
            });
            all_metadata.push(MetadataEntry {
                field_id: 1,
                value: MetadataValue::U64(spectrum.target_id),
            });
            all_metadata.push(MetadataEntry {
                field_id: 2,
                value: MetadataValue::String(band_name.to_string()),
            });
            all_metadata.push(MetadataEntry {
                field_id: 3,
                value: MetadataValue::String(dominant_mol.to_string()),
            });

            global_id += 1;
        }
    }

    let vec_refs: Vec<&[f32]> = all_vectors.iter().map(|v| v.as_slice()).collect();
    let ingest = store
        .ingest_batch(&vec_refs, &all_ids, Some(&all_metadata))
        .expect("ingest failed");

    println!("  Targets:     {}", num_targets);
    println!("  Bands:       {:?}", bands);
    println!("  Windows:     {} total", ingest.accepted);
    println!("  Embedding:   {} dims", dim);
    println!("  Instrument:  jwst-nirspec");

    println!("\n  Target spectra:");
    for s in spectra.iter().take(5) {
        println!(
            "    {} (id={}) molecules: [{}]",
            s.target_name,
            s.target_id,
            s.detected_molecules.join(", ")
        );
    }

    // ====================================================================
    // L1: Feature extraction — molecule identification + co-occurrence
    // ====================================================================
    println!("\n--- L1. Feature Extraction: Molecule Co-Occurrence ---");

    let all_edges: Vec<Vec<CoOccurrenceEdge>> = spectra.iter().map(|s| extract_features(s)).collect();

    let total_edges: usize = all_edges.iter().map(|e| e.len()).sum();
    println!("  Total co-occurrence edges: {}", total_edges);

    println!("\n  Molecule detection summary:");
    for mol in MOLECULES {
        let count = spectra
            .iter()
            .filter(|s| s.detected_molecules.contains(&mol.name.to_string()))
            .count();
        println!(
            "    {}: detected in {}/{} targets (lambda={:.1}um)",
            mol.name, count, num_targets, mol.wavelength_um
        );
    }

    println!("\n  Sample co-occurrence edges:");
    for (i, edges) in all_edges.iter().enumerate().take(3) {
        if !edges.is_empty() {
            println!("    {} (target {}):", spectra[i].target_name, i);
            for e in edges.iter().take(3) {
                println!(
                    "      {} <-> {} conf={:.4}",
                    e.molecule_a, e.molecule_b, e.confidence
                );
            }
        }
    }

    // ====================================================================
    // L2: Disequilibrium scoring
    // ====================================================================
    println!("\n--- L2. Disequilibrium Scoring ---");

    let mut scores: Vec<LifeScore> = Vec::new();
    for (i, spectrum) in spectra.iter().enumerate() {
        let num_obs = 3 + (i % 5); // simulate 3-7 observations
        let score = score_life_candidate(spectrum, &all_edges[i], num_obs);
        scores.push(score);
    }

    // Sort by total score descending
    scores.sort_by(|a, b| b.total_score.partial_cmp(&a.total_score).unwrap());

    println!("  Score components: disequilibrium(0.4), repeatability(0.3),");
    println!("                    -contamination(0.15), -stellar_confound(0.15)\n");

    println!(
        "    {:>14}  {:>5}  {:>6}  {:>5}  {:>5}  {:>7}  {:>5}",
        "Target", "Mols", "Diseq", "Rept", "Score", "Uncert", "Rank"
    );
    println!(
        "    {:->14}  {:->5}  {:->6}  {:->5}  {:->5}  {:->7}  {:->5}",
        "", "", "", "", "", "", ""
    );
    for (rank, s) in scores.iter().enumerate() {
        println!(
            "    {:>14}  {:>5}  {:>6.3}  {:>5.3}  {:>5.3}  {:>7.4}  {:>5}",
            s.target_name,
            s.num_molecules,
            s.disequilibrium,
            s.repeatability,
            s.total_score,
            s.uncertainty,
            rank + 1
        );
    }

    // ====================================================================
    // Filtered query: O3-bearing targets
    // ====================================================================
    println!("\n--- Filtered Query: O3-Bearing Windows ---");

    let query_vec = random_vector(dim, 101);
    let filter_o3 = FilterExpr::Eq(3, FilterValue::String("O3".to_string()));
    let opts_o3 = QueryOptions {
        filter: Some(filter_o3),
        ..Default::default()
    };
    let results_o3 = store
        .query(&query_vec, 10, &opts_o3)
        .expect("filtered query failed");
    println!("  O3-bearing windows: {}", results_o3.len());

    // ====================================================================
    // Witness chain: full provenance trace
    // ====================================================================
    println!("\n--- Witness Chain: Provenance Trace ---");

    let chain_steps = [
        ("genesis", 0x01u8),
        ("l0_spectrum_ingest", 0x08),
        ("l0_band_window", 0x02),
        ("l0_continuum_normalize", 0x02),
        ("l1_absorption_detect", 0x02),
        ("l1_molecule_identify", 0x02),
        ("l1_cooccurrence_build", 0x02),
        ("l2_equilibrium_compare", 0x02),
        ("l2_disequilibrium_score", 0x02),
        ("l2_repeatability_check", 0x02),
        ("l2_contamination_penalty", 0x02),
        ("l2_stellar_confound", 0x02),
        ("l2_final_rank", 0x02),
        ("provenance_seal", 0x01),
    ];

    let entries: Vec<WitnessEntry> = chain_steps
        .iter()
        .enumerate()
        .map(|(i, (step, wtype))| {
            let action_data = format!("life_candidate:{}:step_{}", step, i);
            WitnessEntry {
                prev_hash: [0u8; 32],
                action_hash: shake256_256(action_data.as_bytes()),
                timestamp_ns: 1_700_000_000_000_000_000 + i as u64 * 1_000_000_000,
                witness_type: *wtype,
            }
        })
        .collect();

    let chain_bytes = create_witness_chain(&entries);
    let verified = verify_witness_chain(&chain_bytes).expect("chain verification failed");

    println!("  Chain entries:  {}", verified.len());
    println!("  Chain size:     {} bytes", chain_bytes.len());
    println!("  Integrity:      VALID");

    println!("\n  Pipeline steps:");
    for (i, (step, _)) in chain_steps.iter().enumerate() {
        let wtype_name = match verified[i].witness_type {
            0x01 => "PROV",
            0x02 => "COMP",
            0x05 => "ATTS",
            0x08 => "DATA",
            _ => "????",
        };
        println!("    [{:>4}] {:>2} -> {}", wtype_name, i, step);
    }

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== Life Candidate Summary ===\n");
    println!("  Targets analyzed:    {}", num_targets);
    println!("  Spectral windows:    {}", ingest.accepted);
    println!("  Co-occurrence edges: {}", total_edges);
    println!("  Witness entries:     {}", verified.len());

    if let Some(best) = scores.first() {
        println!("\n  Top life candidate:");
        println!("    Target:        {}", best.target_name);
        println!("    Molecules:     {}", best.num_molecules);
        println!("    Disequilib.:   {:.4}", best.disequilibrium);
        println!("    Total score:   {:.4}", best.total_score);
        println!("    Uncertainty:   {:.4}", best.uncertainty);
        println!("    Follow-up:     {:?}", best.follow_up);
    }

    store.close().expect("failed to close store");
    println!("\nDone.");
}
