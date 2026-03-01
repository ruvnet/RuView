# ADR-014: Health Biomarker Analysis Engine

**Status:** Accepted | **Date:** 2026-02-22 | **Authors:** RuVector Genomics Architecture Team
**Parents:** ADR-001 (Vision), ADR-003 (HNSW Index), ADR-004 (Attention), ADR-009 (Variant Calling), ADR-011 (Performance Targets), ADR-013 (RVDNA Format)

## Context

The rvDNA crate already implements 17 clinically-relevant health SNPs across 4 categories (Cancer Risk, Cardiovascular, Neurological, Metabolism) in `health.rs`, with dedicated analysis functions for APOE genotyping, MTHFR compound status, and COMT/OPRM1 pain profiling. The genotyping pipeline (`genotyping.rs`) provides end-to-end 23andMe analysis with 7-stage processing.

However, the current health variant analysis has several limitations:

| Limitation | Impact | Module |
|-----------|--------|--------|
| No polygenic risk scoring | Individual SNP effects miss gene-gene interactions | `health.rs` |
| No longitudinal tracking | Cannot monitor biomarker changes over time | None |
| No streaming data ingestion | Real-time health monitoring impossible | None |
| No vector-indexed biomarker search | Cannot correlate across populations | None |
| No composite health scoring | No unified risk quantification | `health.rs` |
| No RVDNA biomarker section | Health data not persisted in AI-native format | `rvdna.rs` |

The health biomarker domain requires three capabilities beyond SNP lookup: (1) composite risk scoring that aggregates across gene networks, (2) streaming ingestion for real-time monitoring, and (3) HNSW-indexed population-scale similarity search for correlating individual profiles against reference cohorts.

## Decision: Health Biomarker Analysis Engine

We introduce a biomarker analysis engine (`biomarker.rs`) that extends the existing `health.rs` SNP analysis with:

1. **Composite Biomarker Profiles** — Aggregate individual SNP results into category-level and global risk scores with configurable weighting
2. **Streaming Data Simulation** — Simulated real-time biomarker data streams with configurable noise, drift, and anomaly injection for testing temporal analysis
3. **HNSW-Indexed Profile Search** — Store biomarker profiles as dense vectors in HNSW index for population-scale similarity search
4. **Temporal Biomarker Tracking** — Time-series analysis with trend detection, moving averages, and anomaly detection
5. **Real Example Data** — Curated biomarker datasets based on clinically validated reference ranges

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                   Health Biomarker Engine                        │
├──────────────┬──────────────┬───────────────┬───────────────────┤
│  Composite   │  Streaming   │  HNSW-Indexed │   Temporal        │
│  Risk Score  │  Simulator   │  Population   │   Tracker         │
│              │              │  Search       │                   │
├──────────────┤              │               │                   │
│ Gene Network │  Noise Model │  Profile Vec  │  Moving Average   │
│ Interaction  │  Drift Model │  Quantization │  Trend Detection  │
│ Weights      │  Anomalies   │  Similarity   │  Anomaly Detect   │
└──────┬───────┴──────┬───────┴───────┬───────┴───────┬───────────┘
       │              │               │               │
┌──────┴──────┐ ┌─────┴─────┐  ┌─────┴──────┐  ┌────┴────────┐
│ health.rs   │ │ tokio     │  │ ruvector   │  │ biomarker   │
│ 17 SNPs     │ │ streams   │  │ -core HNSW │  │ time series │
│ APOE/MTHFR  │ │ channels  │  │ VectorDB   │  │ ring buffer │
└─────────────┘ └───────────┘  └────────────┘  └─────────────┘
```

### Component Specifications

#### 1. Composite Biomarker Profile

```rust
pub struct BiomarkerProfile {
    pub subject_id: String,
    pub timestamp: i64,
    pub snp_results: Vec<HealthVariantResult>,
    pub category_scores: HashMap<String, CategoryScore>,
    pub global_risk_score: f64,
    pub profile_vector: Vec<f32>,      // Dense vector for HNSW indexing
}

pub struct CategoryScore {
    pub category: String,
    pub score: f64,                     // 0.0 (low risk) to 1.0 (high risk)
    pub confidence: f64,                // Based on genotyped fraction
    pub contributing_variants: Vec<String>,
}
```

**Scoring Algorithm:**
- Each SNP contributes a risk weight based on its clinical significance and genotype
- Category scores aggregate SNP weights within gene-network boundaries
- Gene-gene interaction terms (e.g., COMT x OPRM1 for pain) apply multiplicative modifiers
- Global risk score uses weighted geometric mean across categories
- Profile vector is the concatenation of normalized category scores + individual SNP encodings (one-hot genotype)

**Weight Matrix (evidence-based):**

| Gene | Risk Weight (Hom Ref) | Risk Weight (Het) | Risk Weight (Hom Alt) | Category |
|------|----------------------|-------------------|----------------------|----------|
| APOE (rs429358) | 0.0 | 0.45 | 0.90 | Neurological |
| BRCA1 (rs80357906) | 0.0 | 0.70 | 0.95 | Cancer |
| MTHFR C677T | 0.0 | 0.30 | 0.65 | Metabolism |
| COMT Val158Met | 0.0 | 0.25 | 0.50 | Neurological |
| CYP1A2 | 0.0 | 0.15 | 0.35 | Metabolism |
| SLCO1B1 | 0.0 | 0.40 | 0.75 | Cardiovascular |

**Interaction Terms:**

| Interaction | Modifier | Rationale |
|------------|----------|-----------|
| COMT(AA) x OPRM1(GG) | 1.4x pain score | Synergistic pain sensitivity |
| MTHFR(677TT) x MTHFR(1298CC) | 1.3x metabolism score | Compound heterozygote |
| APOE(e4/e4) x TP53(variant) | 1.2x neurological score | Neurodegeneration + impaired DNA repair |
| BRCA1(carrier) x TP53(variant) | 1.5x cancer score | DNA repair pathway compromise |

#### 2. Streaming Biomarker Simulator

```rust
pub struct StreamConfig {
    pub base_interval_ms: u64,          // Interval between readings
    pub noise_amplitude: f64,           // Gaussian noise σ
    pub drift_rate: f64,                // Linear drift per reading
    pub anomaly_probability: f64,       // Probability of anomalous reading
    pub anomaly_magnitude: f64,         // Size of anomaly spike
    pub num_biomarkers: usize,          // Number of parallel streams
    pub window_size: usize,             // Sliding window for statistics
}

pub struct BiomarkerReading {
    pub timestamp_ms: u64,
    pub biomarker_id: String,
    pub value: f64,
    pub reference_range: (f64, f64),
    pub is_anomaly: bool,
    pub z_score: f64,
}
```

**Simulation Model:**
- Base values drawn from clinically validated reference ranges (see Section 3)
- Gaussian noise with configurable σ (default: 2% of reference range)
- Linear drift models chronic condition progression
- Anomaly injection via Poisson process (default: p=0.02 per reading)
- Anomalies modeled as multiplicative spikes (default: 2.5x normal variation)

**Streaming Protocol:**
- Uses `tokio::sync::mpsc` channels for async data flow
- Ring buffer (capacity: 10,000 readings) for windowed statistics
- Moving average, exponential smoothing, and z-score computation in real-time
- Backpressure via bounded channels prevents memory exhaustion

#### 3. HNSW-Indexed Population Search

Biomarker profile vectors are stored in RuVector's HNSW index for population-scale similarity search:

```rust
pub struct PopulationIndex {
    pub db: VectorDB,
    pub profile_dim: usize,             // Vector dimension (typically 64)
    pub population_size: usize,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

**Vector Encoding:**
- 17 SNPs x 3 genotype one-hot = 51 dimensions
- 4 category scores = 4 dimensions
- 1 global risk score = 1 dimension
- 4 interaction terms = 4 dimensions
- MTHFR score (1) + Pain score (1) + APOE risk (1) + Caffeine metabolism (1) = 4 dimensions
- **Total: 64 dimensions** (power of 2 for SIMD alignment)

**Search Performance (from ADR-011):**
- p50 latency: <100 μs at 10k profiles
- p99 latency: <250 μs at 10k profiles
- Recall@10: >97%
- HNSW config: M=16, ef_construction=200, ef_search=50

#### 4. Reference Biomarker Data

Curated reference ranges from clinical literature (CDC, WHO, NCBI ClinVar):

| Biomarker | Unit | Low | Normal Low | Normal High | High | Critical |
|-----------|------|-----|------------|-------------|------|----------|
| Total Cholesterol | mg/dL | - | <200 | 200-239 | >=240 | >300 |
| LDL Cholesterol | mg/dL | - | <100 | 100-159 | >=160 | >190 |
| HDL Cholesterol | mg/dL | <40 | 40-59 | >=60 | - | - |
| Triglycerides | mg/dL | - | <150 | 150-199 | >=200 | >500 |
| Fasting Glucose | mg/dL | <70 | 70-99 | 100-125 | >=126 | >300 |
| HbA1c | % | <4.0 | 4.0-5.6 | 5.7-6.4 | >=6.5 | >10.0 |
| Homocysteine | μmol/L | - | <10 | 10-15 | >15 | >30 |
| Vitamin D (25-OH) | ng/mL | <20 | 20-29 | 30-100 | >100 | >150 |
| CRP (hs) | mg/L | - | <1.0 | 1.0-3.0 | >3.0 | >10.0 |
| TSH | mIU/L | <0.4 | 0.4-2.0 | 2.0-4.0 | >4.0 | >10.0 |
| Ferritin | ng/mL | <12 | 12-150 | 150-300 | >300 | >1000 |
| Vitamin B12 | pg/mL | <200 | 200-300 | 300-900 | >900 | - |

These values are used to:
1. Validate streaming simulator output
2. Calculate z-scores for anomaly detection
3. Generate realistic synthetic population data
4. Provide clinical context in biomarker reports

### Performance Targets

| Operation | Target | Mechanism |
|-----------|--------|-----------|
| Composite score (17 SNPs) | <50 μs | In-memory weight matrix multiply |
| Profile vector encoding | <100 μs | One-hot + normalize |
| Population similarity top-10 | <150 μs | HNSW search on 64-dim vectors |
| Stream processing (single reading) | <10 μs | Ring buffer + running stats |
| Anomaly detection | <5 μs | Z-score against moving window |
| Full biomarker report | <1 ms | Score + encode + search |
| Population index build (10k) | <500 ms | Batch HNSW insert |
| Streaming throughput | >100k readings/sec | Lock-free ring buffer |

### Integration Points

| Existing Module | Integration | Direction |
|----------------|-------------|-----------|
| `health.rs` | SNP results feed composite scorer | Input |
| `genotyping.rs` | 23andMe pipeline generates BiomarkerProfile | Input |
| `ruvector-core` | HNSW index stores profile vectors | Bidirectional |
| `rvdna.rs` | Profile vectors stored in metadata section | Output |
| `epigenomics.rs` | Methylation data enriches biomarker profile | Input |
| `pharma.rs` | CYP metabolizer status informs drug-related biomarkers | Input |

## Consequences

**Positive:**
- Unified risk scoring replaces per-SNP interpretation with actionable composite scores
- Streaming architecture enables real-time health monitoring use cases
- HNSW indexing enables population-scale "patients like me" queries in <150 μs
- Reference biomarker data provides clinical validation framework
- 64-dim profile vectors are SIMD-aligned for maximum throughput
- Ring buffer streaming achieves >100k readings/sec without allocation pressure

**Negative:**
- Composite scoring weights are simplified; clinical deployment requires validated coefficients from GWAS
- Streaming simulator generates synthetic data only; real clinical integration requires HL7/FHIR adapters
- Additional 64-dim vector per profile increases RVDNA file size by ~256 bytes per subject

**Neutral:**
- Risk scores are educational/research only; same disclaimer as existing `health.rs`
- Gene-gene interaction terms are limited to known pairs; extensible via configuration

## Options Considered

1. **Extend health.rs with scoring** — rejected: would grow file beyond 500-line limit; scoring + streaming + search are distinct bounded contexts
2. **Separate crate** — rejected: too much coupling to existing types; shared types across modules
3. **New module (biomarker.rs)** — selected: clean separation, imports from `health.rs`, integrates with `ruvector-core` HNSW, stays within the rvDNA crate boundary

## Implementation Strategy

**Phase 1 (This ADR):**
- `biomarker.rs`: Composite scoring engine with reference data
- `biomarker_stream.rs`: Streaming simulator with ring buffer and anomaly detection
- Integration tests with realistic 23andMe-derived profiles
- Benchmark suite validating performance targets

**Phase 2 (Future):**
- RVDNA Section 7: Biomarker profile storage in binary format
- Population index persistence (serialize HNSW graph to RVDNA)
- WASM export for browser-based biomarker dashboards
- HL7/FHIR streaming adapter for clinical integration

## Related Decisions

- **ADR-001**: Vision — health biomarker analysis is a key clinical application
- **ADR-003**: HNSW index — population search uses the same index infrastructure
- **ADR-009**: Variant calling — biomarker profiles integrate variant quality scores
- **ADR-011**: Performance targets — all biomarker operations must meet latency budgets
- **ADR-013**: RVDNA format — biomarker vectors stored in metadata section (Phase 1) or dedicated section (Phase 2)

## References

- [CPIC Guidelines](https://cpicpgx.org/) — Pharmacogenomics dosing guidelines
- [ClinVar](https://www.ncbi.nlm.nih.gov/clinvar/) — Clinical variant significance database
- [gnomAD](https://gnomad.broadinstitute.org/) — Population allele frequencies
- [Horvath Clock](https://doi.org/10.1186/gb-2013-14-10-r115) — Epigenetic age estimation
- [APOE Alzheimer's Meta-Analysis](https://doi.org/10.1001/jama.278.16.1349) — e4 odds ratios
- [MTHFR Clinical Review](https://doi.org/10.1007/s12035-019-1547-z) — Compound heterozygote effects
