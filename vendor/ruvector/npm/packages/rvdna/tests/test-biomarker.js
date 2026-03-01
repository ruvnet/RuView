'use strict';

const {
  biomarkerReferences, zScore, classifyBiomarker,
  computeRiskScores, encodeProfileVector, generateSyntheticPopulation,
  SNPS, INTERACTIONS, CAT_ORDER,
} = require('../src/biomarker');

const {
  RingBuffer, StreamProcessor, generateReadings, defaultStreamConfig,
  Z_SCORE_THRESHOLD,
} = require('../src/stream');

// ── Test harness ─────────────────────────────────────────────────────────────

let passed = 0, failed = 0, benchResults = [];

function assert(cond, msg) {
  if (!cond) throw new Error(`Assertion failed: ${msg}`);
}

function assertClose(a, b, eps, msg) {
  if (Math.abs(a - b) > eps) throw new Error(`${msg}: ${a} != ${b} (eps=${eps})`);
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
  // Warmup
  for (let i = 0; i < Math.min(iterations, 1000); i++) fn();
  const start = performance.now();
  for (let i = 0; i < iterations; i++) fn();
  const elapsed = performance.now() - start;
  const perOp = (elapsed / iterations * 1000).toFixed(2);
  benchResults.push({ name, perOp: `${perOp} us`, total: `${elapsed.toFixed(1)} ms`, iterations });
  process.stdout.write(`  BENCH ${name}: ${perOp} us/op (${iterations} iters, ${elapsed.toFixed(1)} ms)\n`);
}

// ── Helpers ──────────────────────────────────────────────────────────────────

function fullHomRef() {
  const gts = new Map();
  for (const snp of SNPS) gts.set(snp.rsid, snp.homRef);
  return gts;
}

function reading(ts, id, val, lo, hi) {
  return { timestampMs: ts, biomarkerId: id, value: val, referenceLow: lo, referenceHigh: hi, isAnomaly: false, zScore: 0 };
}

function glucose(ts, val) { return reading(ts, 'glucose', val, 70, 100); }

// ═════════════════════════════════════════════════════════════════════════════
// Biomarker Reference Tests
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- Biomarker References ---\n');

test('biomarker_references_count', () => {
  assert(biomarkerReferences().length === 13, `expected 13, got ${biomarkerReferences().length}`);
});

test('z_score_midpoint_is_zero', () => {
  const ref = biomarkerReferences()[0]; // Total Cholesterol
  const mid = (ref.normalLow + ref.normalHigh) / 2;
  assertClose(zScore(mid, ref), 0, 1e-10, 'midpoint z-score');
});

test('z_score_high_bound_is_one', () => {
  const ref = biomarkerReferences()[0];
  assertClose(zScore(ref.normalHigh, ref), 1.0, 1e-10, 'high-bound z-score');
});

// ═════════════════════════════════════════════════════════════════════════════
// Classification Tests
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- Classification ---\n');

test('classify_normal', () => {
  const ref = biomarkerReferences()[0]; // 125-200
  assert(classifyBiomarker(150, ref) === 'Normal', 'expected Normal');
});

test('classify_high', () => {
  const ref = biomarkerReferences()[0]; // normalHigh=200, criticalHigh=300
  assert(classifyBiomarker(250, ref) === 'High', 'expected High');
});

test('classify_critical_high', () => {
  const ref = biomarkerReferences()[0]; // criticalHigh=300
  assert(classifyBiomarker(350, ref) === 'CriticalHigh', 'expected CriticalHigh');
});

test('classify_low', () => {
  const ref = biomarkerReferences()[0]; // normalLow=125, criticalLow=100
  assert(classifyBiomarker(110, ref) === 'Low', 'expected Low');
});

test('classify_critical_low', () => {
  const ref = biomarkerReferences()[0]; // criticalLow=100
  assert(classifyBiomarker(90, ref) === 'CriticalLow', 'expected CriticalLow');
});

// ═════════════════════════════════════════════════════════════════════════════
// Risk Scoring Tests
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- Risk Scoring ---\n');

test('all_hom_ref_low_risk', () => {
  const gts = fullHomRef();
  const profile = computeRiskScores(gts);
  assert(profile.globalRiskScore < 0.15, `hom-ref should be low risk, got ${profile.globalRiskScore}`);
});

test('high_cancer_risk', () => {
  const gts = fullHomRef();
  gts.set('rs80357906', 'DI');
  gts.set('rs1042522', 'GG');
  gts.set('rs11571833', 'TT');
  const profile = computeRiskScores(gts);
  const cancer = profile.categoryScores['Cancer Risk'];
  assert(cancer.score > 0.3, `should have elevated cancer risk, got ${cancer.score}`);
});

test('interaction_comt_oprm1', () => {
  const gts = fullHomRef();
  gts.set('rs4680', 'AA');
  gts.set('rs1799971', 'GG');
  const withInteraction = computeRiskScores(gts);
  const neuroInter = withInteraction.categoryScores['Neurological'].score;

  const gts2 = fullHomRef();
  gts2.set('rs4680', 'AA');
  const withoutFull = computeRiskScores(gts2);
  const neuroSingle = withoutFull.categoryScores['Neurological'].score;

  assert(neuroInter > neuroSingle, `interaction should amplify risk: ${neuroInter} > ${neuroSingle}`);
});

test('interaction_brca1_tp53', () => {
  const gts = fullHomRef();
  gts.set('rs80357906', 'DI');
  gts.set('rs1042522', 'GG');
  const profile = computeRiskScores(gts);
  const cancer = profile.categoryScores['Cancer Risk'];
  assert(cancer.contributingVariants.includes('rs80357906'), 'missing rs80357906');
  assert(cancer.contributingVariants.includes('rs1042522'), 'missing rs1042522');
});

// ═════════════════════════════════════════════════════════════════════════════
// Profile Vector Tests
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- Profile Vectors ---\n');

test('vector_dimension_is_64', () => {
  const gts = fullHomRef();
  const profile = computeRiskScores(gts);
  assert(profile.profileVector.length === 64, `expected 64, got ${profile.profileVector.length}`);
});

test('vector_is_l2_normalized', () => {
  const gts = fullHomRef();
  gts.set('rs4680', 'AG');
  gts.set('rs1799971', 'AG');
  const profile = computeRiskScores(gts);
  let norm = 0;
  for (let i = 0; i < 64; i++) norm += profile.profileVector[i] ** 2;
  norm = Math.sqrt(norm);
  assertClose(norm, 1.0, 1e-4, 'L2 norm');
});

test('vector_deterministic', () => {
  const gts = fullHomRef();
  gts.set('rs1801133', 'AG');
  const a = computeRiskScores(gts);
  const b = computeRiskScores(gts);
  for (let i = 0; i < 64; i++) {
    assertClose(a.profileVector[i], b.profileVector[i], 1e-10, `dim ${i}`);
  }
});

// ═════════════════════════════════════════════════════════════════════════════
// Population Generation Tests
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- Population Generation ---\n');

test('population_correct_count', () => {
  const pop = generateSyntheticPopulation(50, 42);
  assert(pop.length === 50, `expected 50, got ${pop.length}`);
  for (const p of pop) {
    assert(p.profileVector.length === 64, `expected 64-dim vector`);
    assert(Object.keys(p.biomarkerValues).length > 0, 'should have biomarker values');
    assert(p.globalRiskScore >= 0 && p.globalRiskScore <= 1, 'risk in [0,1]');
  }
});

test('population_deterministic', () => {
  const a = generateSyntheticPopulation(10, 99);
  const b = generateSyntheticPopulation(10, 99);
  for (let i = 0; i < 10; i++) {
    assert(a[i].subjectId === b[i].subjectId, 'subject IDs must match');
    assertClose(a[i].globalRiskScore, b[i].globalRiskScore, 1e-10, `risk score ${i}`);
  }
});

test('mthfr_elevates_homocysteine', () => {
  const pop = generateSyntheticPopulation(200, 7);
  const high = [], low = [];
  for (const p of pop) {
    const hcy = p.biomarkerValues['Homocysteine'] || 0;
    const metaScore = p.categoryScores['Metabolism'] ? p.categoryScores['Metabolism'].score : 0;
    if (metaScore > 0.3) high.push(hcy); else low.push(hcy);
  }
  if (high.length > 0 && low.length > 0) {
    const avgHigh = high.reduce((a, b) => a + b, 0) / high.length;
    const avgLow = low.reduce((a, b) => a + b, 0) / low.length;
    assert(avgHigh > avgLow, `MTHFR should elevate homocysteine: high=${avgHigh}, low=${avgLow}`);
  }
});

// ═════════════════════════════════════════════════════════════════════════════
// RingBuffer Tests
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- RingBuffer ---\n');

test('ring_buffer_push_iter_len', () => {
  const rb = new RingBuffer(4);
  for (const v of [10, 20, 30]) rb.push(v);
  const arr = rb.toArray();
  assert(arr.length === 3 && arr[0] === 10 && arr[1] === 20 && arr[2] === 30, 'push/iter');
  assert(rb.length === 3, 'length');
  assert(!rb.isFull(), 'not full');
});

test('ring_buffer_overflow_keeps_newest', () => {
  const rb = new RingBuffer(3);
  for (let v = 1; v <= 4; v++) rb.push(v);
  assert(rb.isFull(), 'should be full');
  const arr = rb.toArray();
  assert(arr[0] === 2 && arr[1] === 3 && arr[2] === 4, `got [${arr}]`);
});

test('ring_buffer_capacity_one', () => {
  const rb = new RingBuffer(1);
  rb.push(42); rb.push(99);
  const arr = rb.toArray();
  assert(arr.length === 1 && arr[0] === 99, `got [${arr}]`);
});

test('ring_buffer_clear_resets', () => {
  const rb = new RingBuffer(3);
  rb.push(1); rb.push(2); rb.clear();
  assert(rb.length === 0, 'length after clear');
  assert(!rb.isFull(), 'not full after clear');
  assert(rb.toArray().length === 0, 'empty after clear');
});

// ═════════════════════════════════════════════════════════════════════════════
// Stream Processor Tests
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- Stream Processor ---\n');

test('processor_computes_stats', () => {
  const cfg = { ...defaultStreamConfig(), windowSize: 10 };
  const p = new StreamProcessor(cfg);
  const readings = generateReadings(cfg, 20, 55);
  for (const r of readings) p.processReading(r);
  const s = p.getStats('glucose');
  assert(s !== null, 'should have glucose stats');
  assert(s.count > 0 && s.mean > 0 && s.min <= s.max, 'valid stats');
});

test('processor_summary_totals', () => {
  const cfg = defaultStreamConfig();
  const p = new StreamProcessor(cfg);
  const readings = generateReadings(cfg, 30, 77);
  for (const r of readings) p.processReading(r);
  const s = p.summary();
  assert(s.totalReadings === 30 * cfg.numBiomarkers, `expected ${30 * cfg.numBiomarkers}, got ${s.totalReadings}`);
  assert(s.anomalyRate >= 0 && s.anomalyRate <= 1, 'anomaly rate in [0,1]');
});

test('processor_throughput_positive', () => {
  const cfg = defaultStreamConfig();
  const p = new StreamProcessor(cfg);
  const readings = generateReadings(cfg, 100, 88);
  for (const r of readings) p.processReading(r);
  const s = p.summary();
  assert(s.throughputReadingsPerSec > 0, 'throughput should be positive');
});

// ═════════════════════════════════════════════════════════════════════════════
// Anomaly Detection Tests
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- Anomaly Detection ---\n');

test('detects_z_score_anomaly', () => {
  const p = new StreamProcessor({ ...defaultStreamConfig(), windowSize: 20 });
  for (let i = 0; i < 20; i++) p.processReading(glucose(i * 1000, 85));
  const r = p.processReading(glucose(20000, 300));
  assert(r.isAnomaly, 'should detect anomaly');
  assert(Math.abs(r.zScore) > Z_SCORE_THRESHOLD, `z-score ${r.zScore} should exceed threshold`);
});

test('detects_out_of_range_anomaly', () => {
  const p = new StreamProcessor({ ...defaultStreamConfig(), windowSize: 5 });
  for (const [i, v] of [80, 82, 78, 84, 81].entries()) {
    p.processReading(glucose(i * 1000, v));
  }
  // 140 >> ref_high(100) + 20%*range(30)=106
  const r = p.processReading(glucose(5000, 140));
  assert(r.isAnomaly, 'should detect out-of-range anomaly');
});

test('zero_anomaly_for_constant_stream', () => {
  const p = new StreamProcessor({ ...defaultStreamConfig(), windowSize: 50 });
  for (let i = 0; i < 10; i++) p.processReading(reading(i * 1000, 'crp', 1.5, 0.1, 3));
  const s = p.getStats('crp');
  assert(Math.abs(s.anomalyRate) < 1e-9, `expected zero anomaly rate, got ${s.anomalyRate}`);
});

// ═════════════════════════════════════════════════════════════════════════════
// Trend Detection Tests
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- Trend Detection ---\n');

test('positive_trend_for_increasing', () => {
  const p = new StreamProcessor({ ...defaultStreamConfig(), windowSize: 20 });
  let r;
  for (let i = 0; i < 20; i++) r = p.processReading(glucose(i * 1000, 70 + i));
  assert(r.currentTrend > 0, `expected positive trend, got ${r.currentTrend}`);
});

test('negative_trend_for_decreasing', () => {
  const p = new StreamProcessor({ ...defaultStreamConfig(), windowSize: 20 });
  let r;
  for (let i = 0; i < 20; i++) r = p.processReading(reading(i * 1000, 'hdl', 60 - i * 0.5, 40, 60));
  assert(r.currentTrend < 0, `expected negative trend, got ${r.currentTrend}`);
});

test('exact_slope_for_linear_series', () => {
  const p = new StreamProcessor({ ...defaultStreamConfig(), windowSize: 10 });
  for (let i = 0; i < 10; i++) {
    p.processReading(reading(i * 1000, 'ldl', 100 + i * 3, 70, 130));
  }
  assertClose(p.getStats('ldl').trendSlope, 3.0, 1e-9, 'slope');
});

// ═════════════════════════════════════════════════════════════════════════════
// Z-score / EMA Tests
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- Z-Score / EMA ---\n');

test('z_score_small_for_near_mean', () => {
  const p = new StreamProcessor({ ...defaultStreamConfig(), windowSize: 10 });
  for (const [i, v] of [80, 82, 78, 84, 76, 86, 81, 79, 83].entries()) {
    p.processReading(glucose(i * 1000, v));
  }
  const mean = p.getStats('glucose').mean;
  const r = p.processReading(glucose(9000, mean));
  assert(Math.abs(r.zScore) < 1, `z-score for mean value should be small, got ${r.zScore}`);
});

test('ema_converges_to_constant', () => {
  const p = new StreamProcessor({ ...defaultStreamConfig(), windowSize: 50 });
  for (let i = 0; i < 50; i++) p.processReading(reading(i * 1000, 'crp', 2.0, 0.1, 3));
  assertClose(p.getStats('crp').ema, 2.0, 1e-6, 'EMA convergence');
});

// ═════════════════════════════════════════════════════════════════════════════
// Batch Generation Tests
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- Batch Generation ---\n');

test('generate_correct_count_and_ids', () => {
  const cfg = defaultStreamConfig();
  const readings = generateReadings(cfg, 50, 42);
  assert(readings.length === 50 * cfg.numBiomarkers, `expected ${50 * cfg.numBiomarkers}, got ${readings.length}`);
  const validIds = new Set(['glucose', 'cholesterol_total', 'hdl', 'ldl', 'triglycerides', 'crp']);
  for (const r of readings) assert(validIds.has(r.biomarkerId), `invalid id: ${r.biomarkerId}`);
});

test('generated_values_non_negative', () => {
  const readings = generateReadings(defaultStreamConfig(), 100, 999);
  for (const r of readings) assert(r.value >= 0, `negative value: ${r.value}`);
});

// ═════════════════════════════════════════════════════════════════════════════
// Benchmarks
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write('\n--- Benchmarks ---\n');

const benchGts = fullHomRef();
benchGts.set('rs4680', 'AG');
benchGts.set('rs1801133', 'AA');

bench('computeRiskScores (20 SNPs)', () => {
  computeRiskScores(benchGts);
}, 10000);

bench('encodeProfileVector (64-dim)', () => {
  const p = computeRiskScores(benchGts);
  encodeProfileVector(p);
}, 10000);

bench('StreamProcessor.processReading', () => {
  const p = new StreamProcessor({ ...defaultStreamConfig(), windowSize: 100 });
  const r = glucose(0, 85);
  for (let i = 0; i < 100; i++) p.processReading(r);
}, 1000);

bench('generateSyntheticPopulation(100)', () => {
  generateSyntheticPopulation(100, 42);
}, 100);

bench('RingBuffer push+iter (100 items)', () => {
  const rb = new RingBuffer(100);
  for (let i = 0; i < 100; i++) rb.push(i);
  let s = 0;
  for (const v of rb) s += v;
}, 10000);

// ═════════════════════════════════════════════════════════════════════════════
// Summary
// ═════════════════════════════════════════════════════════════════════════════

process.stdout.write(`\n${'='.repeat(60)}\n`);
process.stdout.write(`Results: ${passed} passed, ${failed} failed, ${passed + failed} total\n`);
if (benchResults.length > 0) {
  process.stdout.write('\nBenchmark Summary:\n');
  for (const b of benchResults) {
    process.stdout.write(`  ${b.name}: ${b.perOp}/op\n`);
  }
}
process.stdout.write(`${'='.repeat(60)}\n`);

process.exit(failed > 0 ? 1 : 0);
