'use strict';

const fs = require('fs');
const path = require('path');

// Import from index.js (the package entry point) to test the full re-export chain
const rvdna = require('../index.js');

// ── Test harness ─────────────────────────────────────────────────────────────

let passed = 0, failed = 0, benchResults = [];

function assert(cond, msg) {
  if (!cond) throw new Error(`Assertion failed: ${msg}`);
}

function assertClose(a, b, eps, msg) {
  if (Math.abs(a - b) > eps) throw new Error(`${msg}: ${a} != ${b} (eps=${eps})`);
}

function assertGt(a, b, msg) {
  if (!(a > b)) throw new Error(`${msg}: expected ${a} > ${b}`);
}

function assertLt(a, b, msg) {
  if (!(a < b)) throw new Error(`${msg}: expected ${a} < ${b}`);
}

function test(name, fn) {
  try {
    fn();
    passed++;
    process.stdout.write(`  PASS  ${name}\n`);
  } catch (e) {
    failed++;
    process.stdout.write(`  FAIL  ${name}: ${e.message}\n`);
  }
}

function bench(name, fn, iterations) {
  for (let i = 0; i < Math.min(iterations, 1000); i++) fn();
  const start = performance.now();
  for (let i = 0; i < iterations; i++) fn();
  const elapsed = performance.now() - start;
  const perOp = (elapsed / iterations * 1000).toFixed(2);
  benchResults.push({ name, perOp: `${perOp} us`, total: `${elapsed.toFixed(1)} ms`, iterations });
  process.stdout.write(`  BENCH ${name}: ${perOp} us/op (${iterations} iters, ${elapsed.toFixed(1)} ms)\n`);
}

// ── Fixture loading ──────────────────────────────────────────────────────────

const FIXTURES = path.join(__dirname, 'fixtures');

function loadFixture(name) {
  return fs.readFileSync(path.join(FIXTURES, name), 'utf8');
}

function parseFixtureToGenotypes(name) {
  const text = loadFixture(name);
  const data = rvdna.parse23andMe(text);
  const gts = new Map();
  for (const [rsid, snp] of data.snps) gts.set(rsid, snp.genotype);
  return { data, gts };
}

// ═════════════════════════════════════════════════════════════════════════════
// SECTION 1: End-to-End Pipeline (parse 23andMe → biomarker scoring → stream)
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- End-to-End Pipeline ---\n');

test('e2e_high_risk_cardio_pipeline', () => {
  const { data, gts } = parseFixtureToGenotypes('sample-high-risk-cardio.23andme.txt');

  // Stage 1: 23andMe parsing
  assert(data.totalMarkers === 29, `expected 29 markers, got ${data.totalMarkers}`);
  assert(data.build === 'GRCh37', `expected GRCh37, got ${data.build}`);
  assert(data.noCalls === 0, 'no no-calls expected');

  // Stage 2: Genotyping analysis
  const analysis = rvdna.analyze23andMe(loadFixture('sample-high-risk-cardio.23andme.txt'));
  assert(analysis.cyp2d6.phenotype !== undefined, 'CYP2D6 phenotype should be defined');
  assert(analysis.cyp2c19.phenotype !== undefined, 'CYP2C19 phenotype should be defined');

  // Stage 3: Biomarker risk scoring
  const profile = rvdna.computeRiskScores(gts);
  assert(profile.profileVector.length === 64, 'profile vector should be 64-dim');
  assert(profile.globalRiskScore >= 0 && profile.globalRiskScore <= 1, 'risk in [0,1]');

  // High-risk cardiac: MTHFR 677TT + LPA het + SLCO1B1 het → elevated metabolism + cardiovascular
  const metab = profile.categoryScores['Metabolism'];
  assertGt(metab.score, 0.3, 'MTHFR 677TT should elevate metabolism risk');
  assertGt(metab.confidence, 0.5, 'metabolism confidence should be substantial');

  const cardio = profile.categoryScores['Cardiovascular'];
  assert(cardio.contributingVariants.includes('rs10455872'), 'LPA variant should contribute');
  assert(cardio.contributingVariants.includes('rs4363657'), 'SLCO1B1 variant should contribute');
  assert(cardio.contributingVariants.includes('rs3798220'), 'LPA rs3798220 should contribute');

  // Stage 4: Feed synthetic biomarker readings through streaming processor
  const cfg = rvdna.defaultStreamConfig();
  const processor = new rvdna.StreamProcessor(cfg);
  const readings = rvdna.generateReadings(cfg, 50, 42);
  for (const r of readings) processor.processReading(r);
  const summary = processor.summary();
  assert(summary.totalReadings > 0, 'should have processed readings');
  assert(summary.anomalyRate >= 0, 'anomaly rate should be valid');
});

test('e2e_low_risk_baseline_pipeline', () => {
  const { data, gts } = parseFixtureToGenotypes('sample-low-risk-baseline.23andme.txt');

  // Parse
  assert(data.totalMarkers === 29, `expected 29 markers`);
  assert(data.build === 'GRCh38', `expected GRCh38, got ${data.build}`);

  // Score
  const profile = rvdna.computeRiskScores(gts);
  assertLt(profile.globalRiskScore, 0.15, 'all-ref should be very low risk');

  // All categories should be near-zero
  for (const [cat, cs] of Object.entries(profile.categoryScores)) {
    assertLt(cs.score, 0.05, `${cat} should be near-zero for all-ref`);
  }

  // APOE should be e3/e3
  const apoe = rvdna.determineApoe(gts);
  assert(apoe.genotype.includes('e3/e3'), `expected e3/e3, got ${apoe.genotype}`);
});

// ═════════════════════════════════════════════════════════════════════════════
// SECTION 2: Clinical Scenario Tests
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- Clinical Scenarios ---\n');

test('scenario_apoe_e4e4_brca1_carrier', () => {
  const { gts } = parseFixtureToGenotypes('sample-multi-risk.23andme.txt');
  const profile = rvdna.computeRiskScores(gts);

  // APOE e4/e4 → high neurological risk
  const neuro = profile.categoryScores['Neurological'];
  assertGt(neuro.score, 0.5, `APOE e4/e4 + COMT Met/Met should push neuro >0.5, got ${neuro.score}`);
  assert(neuro.contributingVariants.includes('rs429358'), 'APOE should contribute');
  assert(neuro.contributingVariants.includes('rs4680'), 'COMT should contribute');

  // BRCA1 carrier + TP53 variant → elevated cancer risk with interaction
  const cancer = profile.categoryScores['Cancer Risk'];
  assertGt(cancer.score, 0.4, `BRCA1 carrier + TP53 should push cancer >0.4, got ${cancer.score}`);
  assert(cancer.contributingVariants.includes('rs80357906'), 'BRCA1 should contribute');
  assert(cancer.contributingVariants.includes('rs1042522'), 'TP53 should contribute');

  // Cardiovascular should be elevated from SLCO1B1 + LPA
  const cardio = profile.categoryScores['Cardiovascular'];
  assertGt(cardio.score, 0.3, `SLCO1B1 + LPA should push cardio >0.3, got ${cardio.score}`);

  // NQO1 null (TT) should contribute to cancer
  assert(cancer.contributingVariants.includes('rs1800566'), 'NQO1 should contribute');

  // Global risk should be substantial
  assertGt(profile.globalRiskScore, 0.4, `multi-risk global should be >0.4, got ${profile.globalRiskScore}`);

  // APOE determination
  const apoe = rvdna.determineApoe(gts);
  assert(apoe.genotype.includes('e4/e4'), `expected e4/e4, got ${apoe.genotype}`);
});

test('scenario_pcsk9_protective', () => {
  const { gts } = parseFixtureToGenotypes('sample-pcsk9-protective.23andme.txt');
  const profile = rvdna.computeRiskScores(gts);

  // PCSK9 R46L het (rs11591147 GT) → negative cardiovascular weight (protective)
  const cardio = profile.categoryScores['Cardiovascular'];
  // With only PCSK9 protective allele and no risk alleles, cardio score should be very low
  assertLt(cardio.score, 0.05, `PCSK9 protective should keep cardio very low, got ${cardio.score}`);

  // APOE e2/e3 protective
  const apoe = rvdna.determineApoe(gts);
  assert(apoe.genotype.includes('e2/e3'), `expected e2/e3, got ${apoe.genotype}`);
});

test('scenario_mthfr_compound_heterozygote', () => {
  const { gts } = parseFixtureToGenotypes('sample-high-risk-cardio.23andme.txt');
  // This file has rs1801133=AA (677TT hom) + rs1801131=GT (1298AC het) → compound score 3

  const profile = rvdna.computeRiskScores(gts);
  const metab = profile.categoryScores['Metabolism'];

  // MTHFR compound should push metabolism risk up
  assertGt(metab.score, 0.3, `MTHFR compound should elevate metabolism, got ${metab.score}`);
  assert(metab.contributingVariants.includes('rs1801133'), 'rs1801133 (C677T) should contribute');
  assert(metab.contributingVariants.includes('rs1801131'), 'rs1801131 (A1298C) should contribute');

  // MTHFR interaction with MTHFR should amplify
  // The interaction rs1801133×rs1801131 has modifier 1.3
});

test('scenario_comt_oprm1_pain_interaction', () => {
  // Use controlled genotypes that don't saturate the category at 1.0
  const gts = new Map();
  for (const snp of rvdna.SNPS) gts.set(snp.rsid, snp.homRef);
  gts.set('rs4680', 'AA');    // COMT Met/Met
  gts.set('rs1799971', 'GG'); // OPRM1 Asp/Asp
  const profile = rvdna.computeRiskScores(gts);
  const neuro = profile.categoryScores['Neurological'];

  // Without OPRM1 variant → no interaction modifier
  const gts2 = new Map(gts);
  gts2.set('rs1799971', 'AA'); // reference
  const profile2 = rvdna.computeRiskScores(gts2);
  const neuro2 = profile2.categoryScores['Neurological'];

  assertGt(neuro.score, neuro2.score, 'COMT×OPRM1 interaction should amplify neurological risk');
});

test('scenario_drd2_comt_interaction', () => {
  // Use controlled genotypes that don't saturate the category at 1.0
  const gts = new Map();
  for (const snp of rvdna.SNPS) gts.set(snp.rsid, snp.homRef);
  gts.set('rs1800497', 'AA'); // DRD2 A1/A1
  gts.set('rs4680', 'AA');    // COMT Met/Met
  const profile = rvdna.computeRiskScores(gts);

  // Without DRD2 variant → no DRD2×COMT interaction
  const gts2 = new Map(gts);
  gts2.set('rs1800497', 'GG'); // reference
  const profile2 = rvdna.computeRiskScores(gts2);

  assertGt(
    profile.categoryScores['Neurological'].score,
    profile2.categoryScores['Neurological'].score,
    'DRD2×COMT interaction should amplify'
  );
});

// ═════════════════════════════════════════════════════════════════════════════
// SECTION 3: Cross-Validation (JS matches Rust expectations)
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- Cross-Validation (JS ↔ Rust parity) ---\n');

test('parity_reference_count_matches_rust', () => {
  assert(rvdna.BIOMARKER_REFERENCES.length === 13, 'should have 13 references (matches Rust)');
  assert(rvdna.SNPS.length === 20, 'should have 20 SNPs (matches Rust)');
  assert(rvdna.INTERACTIONS.length === 6, 'should have 6 interactions (matches Rust)');
  assert(rvdna.CAT_ORDER.length === 4, 'should have 4 categories (matches Rust)');
});

test('parity_snp_table_exact_match', () => {
  // Verify first and last SNP match Rust exactly
  const first = rvdna.SNPS[0];
  assert(first.rsid === 'rs429358', 'first SNP rsid');
  assertClose(first.wHet, 0.4, 1e-10, 'first SNP wHet');
  assertClose(first.wAlt, 0.9, 1e-10, 'first SNP wAlt');
  assert(first.homRef === 'TT', 'first SNP homRef');
  assert(first.category === 'Neurological', 'first SNP category');

  const last = rvdna.SNPS[19];
  assert(last.rsid === 'rs11591147', 'last SNP rsid');
  assertClose(last.wHet, -0.30, 1e-10, 'PCSK9 wHet (negative = protective)');
  assertClose(last.wAlt, -0.55, 1e-10, 'PCSK9 wAlt (negative = protective)');
});

test('parity_interaction_table_exact_match', () => {
  const i0 = rvdna.INTERACTIONS[0];
  assert(i0.rsidA === 'rs4680' && i0.rsidB === 'rs1799971', 'first interaction pair');
  assertClose(i0.modifier, 1.4, 1e-10, 'COMT×OPRM1 modifier');

  const i3 = rvdna.INTERACTIONS[3];
  assert(i3.rsidA === 'rs80357906' && i3.rsidB === 'rs1042522', 'BRCA1×TP53 pair');
  assertClose(i3.modifier, 1.5, 1e-10, 'BRCA1×TP53 modifier');
});

test('parity_z_score_matches_rust', () => {
  // z_score(mid, ref) should be 0.0 (Rust test_z_score_midpoint_is_zero)
  const ref = rvdna.BIOMARKER_REFERENCES[0]; // Total Cholesterol
  const mid = (ref.normalLow + ref.normalHigh) / 2;
  assertClose(rvdna.zScore(mid, ref), 0, 1e-10, 'midpoint z-score = 0');
  // z_score(normalHigh, ref) should be 1.0 (Rust test_z_score_high_bound_is_one)
  assertClose(rvdna.zScore(ref.normalHigh, ref), 1, 1e-10, 'high-bound z-score = 1');
});

test('parity_classification_matches_rust', () => {
  const ref = rvdna.BIOMARKER_REFERENCES[0]; // Total Cholesterol 125-200
  assert(rvdna.classifyBiomarker(150, ref) === 'Normal', 'Normal');
  assert(rvdna.classifyBiomarker(350, ref) === 'CriticalHigh', 'CriticalHigh (>300)');
  assert(rvdna.classifyBiomarker(110, ref) === 'Low', 'Low');
  assert(rvdna.classifyBiomarker(90, ref) === 'CriticalLow', 'CriticalLow (<100)');
});

test('parity_vector_layout_64dim_l2', () => {
  // Rust test_vector_dimension_is_64 and test_vector_is_l2_normalized
  const gts = new Map();
  for (const snp of rvdna.SNPS) gts.set(snp.rsid, snp.homRef);
  gts.set('rs4680', 'AG');
  gts.set('rs1799971', 'AG');
  const profile = rvdna.computeRiskScores(gts);
  assert(profile.profileVector.length === 64, '64 dims');
  let norm = 0;
  for (let i = 0; i < 64; i++) norm += profile.profileVector[i] ** 2;
  norm = Math.sqrt(norm);
  assertClose(norm, 1.0, 1e-4, 'L2 norm');
});

test('parity_hom_ref_low_risk_matches_rust', () => {
  // Rust test_risk_scores_all_hom_ref_low_risk: global < 0.15
  const gts = new Map();
  for (const snp of rvdna.SNPS) gts.set(snp.rsid, snp.homRef);
  const profile = rvdna.computeRiskScores(gts);
  assertLt(profile.globalRiskScore, 0.15, 'hom-ref should be <0.15');
});

test('parity_high_cancer_matches_rust', () => {
  // Rust test_risk_scores_high_cancer_risk: cancer > 0.3
  const gts = new Map();
  for (const snp of rvdna.SNPS) gts.set(snp.rsid, snp.homRef);
  gts.set('rs80357906', 'DI');
  gts.set('rs1042522', 'GG');
  gts.set('rs11571833', 'TT');
  const profile = rvdna.computeRiskScores(gts);
  assertGt(profile.categoryScores['Cancer Risk'].score, 0.3, 'cancer > 0.3');
});

// ═════════════════════════════════════════════════════════════════════════════
// SECTION 4: Population-Scale Correlation Tests
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- Population Correlations ---\n');

test('population_apoe_lowers_hdl', () => {
  // Mirrors Rust test_apoe_lowers_hdl_in_population
  const pop = rvdna.generateSyntheticPopulation(300, 88);
  const apoeHdl = [], refHdl = [];
  for (const p of pop) {
    const hdl = p.biomarkerValues['HDL'] || 0;
    const neuro = p.categoryScores['Neurological'] ? p.categoryScores['Neurological'].score : 0;
    if (neuro > 0.3) apoeHdl.push(hdl); else refHdl.push(hdl);
  }
  if (apoeHdl.length > 0 && refHdl.length > 0) {
    const avgApoe = apoeHdl.reduce((a, b) => a + b, 0) / apoeHdl.length;
    const avgRef = refHdl.reduce((a, b) => a + b, 0) / refHdl.length;
    assertLt(avgApoe, avgRef, 'APOE e4 should lower HDL');
  }
});

test('population_lpa_elevates_lpa_biomarker', () => {
  const pop = rvdna.generateSyntheticPopulation(300, 44);
  const lpaHigh = [], lpaLow = [];
  for (const p of pop) {
    const lpaVal = p.biomarkerValues['Lp(a)'] || 0;
    const cardio = p.categoryScores['Cardiovascular'] ? p.categoryScores['Cardiovascular'].score : 0;
    if (cardio > 0.2) lpaHigh.push(lpaVal); else lpaLow.push(lpaVal);
  }
  if (lpaHigh.length > 0 && lpaLow.length > 0) {
    const avgHigh = lpaHigh.reduce((a, b) => a + b, 0) / lpaHigh.length;
    const avgLow = lpaLow.reduce((a, b) => a + b, 0) / lpaLow.length;
    assertGt(avgHigh, avgLow, 'cardiovascular risk should correlate with elevated Lp(a)');
  }
});

test('population_risk_score_distribution', () => {
  const pop = rvdna.generateSyntheticPopulation(1000, 123);
  const scores = pop.map(p => p.globalRiskScore);
  const min = Math.min(...scores);
  const max = Math.max(...scores);
  const mean = scores.reduce((a, b) => a + b, 0) / scores.length;

  // Should have good spread
  assertGt(max - min, 0.2, `risk score range should be >0.2, got ${max - min}`);
  // Mean should be moderate (not all near 0 or 1)
  assertGt(mean, 0.05, 'mean risk should be >0.05');
  assertLt(mean, 0.7, 'mean risk should be <0.7');
});

test('population_all_biomarkers_within_clinical_limits', () => {
  const pop = rvdna.generateSyntheticPopulation(500, 55);
  for (const p of pop) {
    for (const bref of rvdna.BIOMARKER_REFERENCES) {
      const val = p.biomarkerValues[bref.name];
      assert(val !== undefined, `missing ${bref.name} for ${p.subjectId}`);
      assert(val >= 0, `${bref.name} should be non-negative, got ${val}`);
      if (bref.criticalHigh !== null) {
        assertLt(val, bref.criticalHigh * 1.25, `${bref.name} should be < criticalHigh*1.25`);
      }
    }
  }
});

// ═════════════════════════════════════════════════════════════════════════════
// SECTION 5: Streaming with Real-Data Correlated Biomarkers
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- Streaming with Real Biomarkers ---\n');

test('stream_cusum_changepoint_on_shift', () => {
  // Mirror Rust test_cusum_changepoint_detection
  const cfg = { ...rvdna.defaultStreamConfig(), windowSize: 20 };
  const p = new rvdna.StreamProcessor(cfg);

  // Establish baseline at 85
  for (let i = 0; i < 30; i++) {
    p.processReading({
      timestampMs: i * 1000, biomarkerId: 'glucose', value: 85,
      referenceLow: 70, referenceHigh: 100, isAnomaly: false, zScore: 0,
    });
  }
  // Sustained shift to 120
  for (let i = 30; i < 50; i++) {
    p.processReading({
      timestampMs: i * 1000, biomarkerId: 'glucose', value: 120,
      referenceLow: 70, referenceHigh: 100, isAnomaly: false, zScore: 0,
    });
  }
  const stats = p.getStats('glucose');
  assertGt(stats.mean, 90, `mean should shift upward after changepoint: ${stats.mean}`);
});

test('stream_drift_detected_as_trend', () => {
  // Mirror Rust test_trend_detection
  const cfg = { ...rvdna.defaultStreamConfig(), windowSize: 50 };
  const p = new rvdna.StreamProcessor(cfg);

  // Strong upward drift
  for (let i = 0; i < 50; i++) {
    p.processReading({
      timestampMs: i * 1000, biomarkerId: 'glucose', value: 70 + i * 0.5,
      referenceLow: 70, referenceHigh: 100, isAnomaly: false, zScore: 0,
    });
  }
  assertGt(p.getStats('glucose').trendSlope, 0, 'should detect positive trend');
});

test('stream_population_biomarker_values_through_processor', () => {
  // Take synthetic population biomarker values and stream them
  const pop = rvdna.generateSyntheticPopulation(20, 77);
  const cfg = { ...rvdna.defaultStreamConfig(), windowSize: 20 };
  const p = new rvdna.StreamProcessor(cfg);

  for (let i = 0; i < pop.length; i++) {
    const homocysteine = pop[i].biomarkerValues['Homocysteine'];
    p.processReading({
      timestampMs: i * 1000, biomarkerId: 'homocysteine',
      value: homocysteine, referenceLow: 5, referenceHigh: 15,
      isAnomaly: false, zScore: 0,
    });
  }

  const stats = p.getStats('homocysteine');
  assert(stats !== null, 'should have homocysteine stats');
  assertGt(stats.count, 0, 'should have processed readings');
  assertGt(stats.mean, 0, 'mean should be positive');
});

// ═════════════════════════════════════════════════════════════════════════════
// SECTION 6: Package Re-export Verification
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- Package Re-exports ---\n');

test('index_exports_all_biomarker_apis', () => {
  const expectedFns = [
    'biomarkerReferences', 'zScore', 'classifyBiomarker',
    'computeRiskScores', 'encodeProfileVector', 'generateSyntheticPopulation',
  ];
  for (const fn of expectedFns) {
    assert(typeof rvdna[fn] === 'function', `missing export: ${fn}`);
  }
  const expectedConsts = ['BIOMARKER_REFERENCES', 'SNPS', 'INTERACTIONS', 'CAT_ORDER'];
  for (const c of expectedConsts) {
    assert(rvdna[c] !== undefined, `missing export: ${c}`);
  }
});

test('index_exports_all_stream_apis', () => {
  assert(typeof rvdna.RingBuffer === 'function', 'missing RingBuffer');
  assert(typeof rvdna.StreamProcessor === 'function', 'missing StreamProcessor');
  assert(typeof rvdna.generateReadings === 'function', 'missing generateReadings');
  assert(typeof rvdna.defaultStreamConfig === 'function', 'missing defaultStreamConfig');
  assert(rvdna.BIOMARKER_DEFS !== undefined, 'missing BIOMARKER_DEFS');
});

test('index_exports_v02_apis_unchanged', () => {
  const v02fns = [
    'encode2bit', 'decode2bit', 'translateDna', 'cosineSimilarity',
    'isNativeAvailable', 'normalizeGenotype', 'parse23andMe',
    'callCyp2d6', 'callCyp2c19', 'determineApoe', 'analyze23andMe',
  ];
  for (const fn of v02fns) {
    assert(typeof rvdna[fn] === 'function', `v0.2 API missing: ${fn}`);
  }
});

// ═════════════════════════════════════════════════════════════════════════════
// SECTION 7: Optimized Benchmarks (pre/post optimization comparison)
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- Optimized Benchmarks ---\n');

// Prepare benchmark genotypes from real fixture
const { gts: benchGts } = parseFixtureToGenotypes('sample-high-risk-cardio.23andme.txt');

bench('computeRiskScores (real 23andMe data, 20 SNPs)', () => {
  rvdna.computeRiskScores(benchGts);
}, 20000);

bench('encodeProfileVector (real profile)', () => {
  const p = rvdna.computeRiskScores(benchGts);
  rvdna.encodeProfileVector(p);
}, 20000);

bench('StreamProcessor.processReading (optimized incremental)', () => {
  const p = new rvdna.StreamProcessor({ ...rvdna.defaultStreamConfig(), windowSize: 100 });
  const r = { timestampMs: 0, biomarkerId: 'glucose', value: 85, referenceLow: 70, referenceHigh: 100, isAnomaly: false, zScore: 0 };
  for (let i = 0; i < 100; i++) {
    r.timestampMs = i * 1000;
    p.processReading(r);
  }
}, 2000);

bench('generateSyntheticPopulation(100) (optimized lookups)', () => {
  rvdna.generateSyntheticPopulation(100, 42);
}, 200);

bench('full pipeline: parse + score + stream (real data)', () => {
  const text = loadFixture('sample-high-risk-cardio.23andme.txt');
  const data = rvdna.parse23andMe(text);
  const gts = new Map();
  for (const [rsid, snp] of data.snps) gts.set(rsid, snp.genotype);
  const profile = rvdna.computeRiskScores(gts);
  const proc = new rvdna.StreamProcessor(rvdna.defaultStreamConfig());
  for (const bref of rvdna.BIOMARKER_REFERENCES) {
    const val = profile.biomarkerValues[bref.name] || ((bref.normalLow + bref.normalHigh) / 2);
    proc.processReading({
      timestampMs: 0, biomarkerId: bref.name, value: val,
      referenceLow: bref.normalLow, referenceHigh: bref.normalHigh,
      isAnomaly: false, zScore: 0,
    });
  }
}, 5000);

bench('population 1000 subjects', () => {
  rvdna.generateSyntheticPopulation(1000, 42);
}, 20);

// ═════════════════════════════════════════════════════════════════════════════
// Summary
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write(`\n${'='.repeat(70)}\n`);
process.stdout.write(`Results: ${passed} passed, ${failed} failed, ${passed + failed} total\n`);
if (benchResults.length > 0) {
  process.stdout.write('\nBenchmark Summary:\n');
  for (const b of benchResults) {
    process.stdout.write(`  ${b.name}: ${b.perOp}/op\n`);
  }
}
process.stdout.write(`${'='.repeat(70)}\n`);

process.exit(failed > 0 ? 1 : 0);
