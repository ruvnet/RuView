# ADR-015: npm/WASM Health Biomarker Engine

**Status:** Accepted | **Date:** 2026-02-22 | **Authors:** RuVector Genomics Architecture Team
**Parents:** ADR-001 (Vision), ADR-008 (WASM Edge), ADR-011 (Performance Targets), ADR-014 (Health Biomarker Analysis)

## Context

ADR-014 delivered the Rust biomarker analysis engine (`biomarker.rs`, `biomarker_stream.rs`) with composite risk scoring across 20 SNPs, 6 gene-gene interactions, 64-dim L2-normalized profile vectors, and a streaming processor with RingBuffer, CUSUM changepoint detection, and Welford online statistics. ADR-008 established WASM as the delivery mechanism for browser-side genomic computation.

The `@ruvector/rvdna` npm package (v0.2.0) already exposes 2-bit encoding, protein translation, cosine similarity, and 23andMe genotyping via pure-JS fallbacks and optional NAPI-RS native bindings. However, it lacks the biomarker engine entirely:

| Gap | Impact | Severity |
|-----|--------|----------|
| No biomarker risk scoring in JS | Browser/Node users cannot compute composite health risk | Critical |
| No streaming processor in JS | Real-time biomarker dashboards impossible without native | Critical |
| No profile vector encoding | Population similarity search unavailable in JS | High |
| No TypeScript types for biomarker API | Developer experience degraded | Medium |
| No benchmarks for JS path | Cannot validate performance parity claims | Medium |

The decision is whether to (a) require WASM/native for all biomarker features, (b) provide a pure-JS implementation that mirrors the Rust engine exactly, or (c) a hybrid approach.

## Decision: Pure-JS Biomarker Engine with WASM Acceleration Path

We implement a **complete pure-JS biomarker engine** in `@ruvector/rvdna` v0.3.0 that mirrors the Rust `biomarker.rs` and `biomarker_stream.rs` exactly, with a future WASM acceleration path for compute-intensive operations.

### Rationale

1. **Zero-dependency accessibility** — Any Node.js or browser environment can run biomarker analysis without compiling Rust or loading WASM
2. **Exact algorithmic parity** — Same 20 SNPs, same 6 interactions, same 64-dim vector layout, same CUSUM parameters, same Welford statistics
3. **Progressive enhancement** — Pure JS works everywhere; WASM (future) accelerates hot paths (vector encoding, population generation)
4. **Test oracle** — JS implementation serves as a cross-language verification oracle against the Rust engine

### Architecture

```
@ruvector/rvdna v0.3.0
├── index.js                 # Entry point, re-exports all modules
├── index.d.ts               # Full TypeScript definitions
├── src/
│   ├── biomarker.js         # Risk scoring engine (mirrors biomarker.rs)
│   └── stream.js            # Streaming processor (mirrors biomarker_stream.rs)
└── tests/
    └── test-biomarker.js    # Comprehensive test suite + benchmarks
```

### Module 1: Biomarker Risk Scoring (`src/biomarker.js`)

**Data Tables (exact mirror of Rust):**

| Table | Entries | Fields |
|-------|---------|--------|
| `BIOMARKER_REFERENCES` | 13 | name, unit, normalLow, normalHigh, criticalLow, criticalHigh, category |
| `SNPS` | 20 | rsid, category, wRef, wHet, wAlt, homRef, het, homAlt, maf |
| `INTERACTIONS` | 6 | rsidA, rsidB, modifier, category |
| `CAT_ORDER` | 4 | Cancer Risk, Cardiovascular, Neurological, Metabolism |

**Functions:**

| Function | Input | Output | Mirrors |
|----------|-------|--------|---------|
| `biomarkerReferences()` | — | `BiomarkerReference[]` | `biomarker_references()` |
| `zScore(value, ref)` | number, BiomarkerReference | number | `z_score()` |
| `classifyBiomarker(value, ref)` | number, BiomarkerReference | enum string | `classify_biomarker()` |
| `computeRiskScores(genotypes)` | `Map<rsid,genotype>` | `BiomarkerProfile` | `compute_risk_scores()` |
| `encodeProfileVector(profile)` | BiomarkerProfile | `Float32Array(64)` | `encode_profile_vector()` |
| `generateSyntheticPopulation(count, seed)` | number, number | `BiomarkerProfile[]` | `generate_synthetic_population()` |

**Scoring Algorithm (identical to Rust):**
1. For each of 20 SNPs, look up genotype and compute weight (wRef/wHet/wAlt)
2. Aggregate weights per category (Cancer Risk, Cardiovascular, Neurological, Metabolism)
3. Apply 6 multiplicative interaction modifiers where both SNPs are non-reference
4. Normalize each category: `score = raw / maxPossible`, clamped to [0, 1]
5. Confidence = genotyped fraction per category
6. Global risk = weighted average: `sum(score * confidence) / sum(confidence)`

**Profile Vector Layout (64 dimensions, L2-normalized):**

| Dims | Content | Count |
|------|---------|-------|
| 0–50 | One-hot genotype encoding (17 SNPs x 3) | 51 |
| 51–54 | Category scores | 4 |
| 55 | Global risk score | 1 |
| 56–59 | First 4 interaction modifiers | 4 |
| 60 | MTHFR score / 4 | 1 |
| 61 | Pain score / 4 | 1 |
| 62 | APOE risk code / 2 | 1 |
| 63 | LPA composite | 1 |

**PRNG:** Mulberry32 (deterministic, no dependencies, matches seeded output for synthetic populations).

### Module 2: Streaming Biomarker Processor (`src/stream.js`)

**Data Structures:**

| Structure | Purpose | Mirrors |
|-----------|---------|---------|
| `RingBuffer` | Fixed-capacity circular buffer, no allocation after init | `RingBuffer<T>` |
| `StreamProcessor` | Per-biomarker rolling stats, anomaly detection, trend analysis | `StreamProcessor` |
| `StreamStats` | mean, variance, min, max, EMA, CUSUM, changepoint | `StreamStats` |

**Constants (identical to Rust):**

| Constant | Value | Purpose |
|----------|-------|---------|
| `EMA_ALPHA` | 0.1 | Exponential moving average smoothing |
| `Z_SCORE_THRESHOLD` | 2.5 | Anomaly detection threshold |
| `REF_OVERSHOOT` | 0.20 | Out-of-range tolerance (20% of range) |
| `CUSUM_THRESHOLD` | 4.0 | Changepoint detection sensitivity |
| `CUSUM_DRIFT` | 0.5 | CUSUM allowable drift |

**Statistics:**
- **Welford's online algorithm** for single-pass mean and sample standard deviation (2x fewer cache misses than two-pass)
- **Simple linear regression** for trend slope via least-squares
- **CUSUM** (Cumulative Sum) for changepoint detection with automatic reset

**Biomarker Definitions (6 streams):**

| ID | Reference Low | Reference High |
|----|--------------|---------------|
| glucose | 70 | 100 |
| cholesterol_total | 150 | 200 |
| hdl | 40 | 60 |
| ldl | 70 | 130 |
| triglycerides | 50 | 150 |
| crp | 0.1 | 3.0 |

### Performance Targets

| Operation | JS Target | Rust Baseline | Acceptable Ratio |
|-----------|-----------|---------------|------------------|
| `computeRiskScores` (20 SNPs) | <200 us | <50 us | 4x |
| `encodeProfileVector` (64-dim) | <300 us | <100 us | 3x |
| `StreamProcessor.processReading` | <50 us | <10 us | 5x |
| `generateSyntheticPopulation(1000)` | <100 ms | <20 ms | 5x |
| RingBuffer push+iter (100 items) | <20 us | <2 us | 10x |

**Benchmark methodology:** `performance.now()` with 1000-iteration warmup, 10000 measured iterations, report p50/p99.

### TypeScript Definitions

Full `.d.ts` types for every exported function, interface, and enum. Key types:

- `BiomarkerReference` — 13-field clinical reference range
- `BiomarkerClassification` — `'CriticalLow' | 'Low' | 'Normal' | 'High' | 'CriticalHigh'`
- `CategoryScore` — per-category risk with confidence and contributing variants
- `BiomarkerProfile` — complete risk profile with 64-dim vector
- `StreamConfig` — streaming processor configuration
- `BiomarkerReading` — timestamped biomarker data point
- `StreamStats` — rolling statistics with CUSUM state
- `ProcessingResult` — per-reading anomaly detection result
- `StreamSummary` — aggregate statistics across all biomarker streams

### Test Coverage

| Category | Tests | Coverage |
|----------|-------|----------|
| Biomarker references | 2 | Count, z-score math |
| Classification | 5 | All 5 classification levels |
| Risk scoring | 4 | All-ref low risk, elevated cancer, interaction amplification, BRCA1+TP53 |
| Profile vectors | 3 | 64-dim, L2-normalized, deterministic |
| Population generation | 3 | Correct count, deterministic, MTHFR-homocysteine correlation |
| RingBuffer | 4 | Push/iter, overflow, capacity-1, clear |
| Stream processor | 3 | Stats computation, summary totals, throughput |
| Anomaly detection | 3 | Z-score anomaly, out-of-range, zero anomaly for constant |
| Trend detection | 3 | Positive, negative, exact slope |
| Z-score / EMA | 2 | Near-mean small z, EMA convergence |
| Benchmarks | 5 | All performance targets |

**Total: 37 tests + 5 benchmarks**

### WASM Acceleration Path (Future — Phase 2)

When `@ruvector/rvdna-wasm` is available:

```js
// Automatic acceleration — same API, WASM hot path
const { computeRiskScores } = require('@ruvector/rvdna');
// Internally checks: nativeModule?.computeRiskScores ?? jsFallback
```

**WASM candidates (>10x speedup potential):**
- `encodeProfileVector` — SIMD dot products for L2 normalization
- `generateSyntheticPopulation` — bulk PRNG + matrix operations
- `StreamProcessor.processReading` — vectorized Welford accumulation

### Versioning

- `@ruvector/rvdna` bumps from `0.2.0` to `0.3.0` (new public API surface)
- `files` array in `package.json` updated to include `src/` directory
- Keywords expanded: `biomarker`, `health`, `risk-score`, `streaming`, `anomaly-detection`
- No breaking changes to existing v0.2.0 API

## Consequences

**Positive:**
- Full biomarker engine available in any JS runtime without native compilation
- Algorithmic parity with Rust ensures cross-language consistency
- Pure JS means zero WASM load time for initial render in browser dashboards
- Comprehensive test suite provides regression safety net
- TypeScript types enable IDE autocompletion and compile-time checking
- Benchmarks establish baseline for future WASM optimization

**Negative:**
- JS is 3-10x slower than Rust for numerical computation
- Synthetic population generation uses Mulberry32 PRNG (not cryptographically identical to Rust's StdRng)
- MTHFR/pain analysis simplified in JS (no cross-module dependency on health.rs internals)

**Neutral:**
- Same clinical disclaimers apply: research/educational use only
- Gene-gene interaction weights unchanged from ADR-014

## Options Considered

1. **WASM-only** — rejected: forces async init, 2MB+ download, excludes lightweight Node.js scripts
2. **Pure JS only, no WASM path** — rejected: leaves performance on the table for browser dashboards
3. **Pure JS with WASM acceleration path** — selected: immediate availability + future optimization
4. **Thin wrapper over native module** — rejected: native bindings unavailable on most platforms

## Related Decisions

- **ADR-008**: WASM Edge Genomics — establishes WASM as browser delivery mechanism
- **ADR-011**: Performance Targets — JS targets derived as acceptable multiples of Rust baselines
- **ADR-014**: Health Biomarker Analysis — Rust engine this ADR mirrors in JavaScript

## References

- [Mulberry32 PRNG](https://gist.github.com/tommyettinger/46a874533244883189143505d203312c) — 32-bit deterministic PRNG
- [Welford's Online Algorithm](https://en.wikipedia.org/wiki/Algorithms_for_calculating_variance#Welford%27s_online_algorithm) — Numerically stable variance
- [CUSUM](https://en.wikipedia.org/wiki/CUSUM) — Cumulative sum control chart for changepoint detection
- [CPIC Guidelines](https://cpicpgx.org/) — Pharmacogenomics evidence base
