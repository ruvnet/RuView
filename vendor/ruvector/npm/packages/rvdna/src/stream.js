'use strict';

// ── Constants (identical to biomarker_stream.rs) ─────────────────────────────

const EMA_ALPHA = 0.1;
const Z_SCORE_THRESHOLD = 2.5;
const REF_OVERSHOOT = 0.20;
const CUSUM_THRESHOLD = 4.0;
const CUSUM_DRIFT = 0.5;

// ── Biomarker definitions ────────────────────────────────────────────────────

const BIOMARKER_DEFS = Object.freeze([
  { id: 'glucose',          low: 70,  high: 100 },
  { id: 'cholesterol_total', low: 150, high: 200 },
  { id: 'hdl',              low: 40,  high: 60 },
  { id: 'ldl',              low: 70,  high: 130 },
  { id: 'triglycerides',    low: 50,  high: 150 },
  { id: 'crp',              low: 0.1, high: 3.0 },
]);

// ── RingBuffer ───────────────────────────────────────────────────────────────

class RingBuffer {
  constructor(capacity) {
    if (capacity <= 0) throw new Error('RingBuffer capacity must be > 0');
    this._buffer = new Float64Array(capacity);
    this._head = 0;
    this._len = 0;
    this._capacity = capacity;
  }

  push(item) {
    this._buffer[this._head] = item;
    this._head = (this._head + 1) % this._capacity;
    if (this._len < this._capacity) this._len++;
  }

  /** Push item and return evicted value (NaN if buffer wasn't full). */
  pushPop(item) {
    const wasFull = this._len === this._capacity;
    const evicted = wasFull ? this._buffer[this._head] : NaN;
    this._buffer[this._head] = item;
    this._head = (this._head + 1) % this._capacity;
    if (!wasFull) this._len++;
    return evicted;
  }

  /** Iterate in insertion order (oldest to newest). */
  *[Symbol.iterator]() {
    const start = this._len < this._capacity ? 0 : this._head;
    for (let i = 0; i < this._len; i++) {
      yield this._buffer[(start + i) % this._capacity];
    }
  }

  /** Return values as a plain array (oldest to newest). */
  toArray() {
    const arr = new Array(this._len);
    const start = this._len < this._capacity ? 0 : this._head;
    for (let i = 0; i < this._len; i++) {
      arr[i] = this._buffer[(start + i) % this._capacity];
    }
    return arr;
  }

  get length() { return this._len; }
  get capacity() { return this._capacity; }
  isFull() { return this._len === this._capacity; }

  clear() {
    this._head = 0;
    this._len = 0;
  }
}

// ── Welford's online mean+std (single-pass, mirrors Rust) ────────────────────

function windowMeanStd(buf) {
  const n = buf.length;
  if (n === 0) return [0, 0];
  let mean = 0, m2 = 0, k = 0;
  for (const x of buf) {
    k++;
    const delta = x - mean;
    mean += delta / k;
    m2 += delta * (x - mean);
  }
  if (n < 2) return [mean, 0];
  return [mean, Math.sqrt(m2 / (n - 1))];
}

// ── Trend slope via simple linear regression (mirrors Rust) ──────────────────

function computeTrendSlope(buf) {
  const n = buf.length;
  if (n < 2) return 0;
  const nf = n;
  const xm = (nf - 1) / 2;
  let ys = 0, xys = 0, xxs = 0, i = 0;
  for (const y of buf) {
    ys += y;
    xys += i * y;
    xxs += i * i;
    i++;
  }
  const ssXy = xys - nf * xm * (ys / nf);
  const ssXx = xxs - nf * xm * xm;
  return Math.abs(ssXx) < 1e-12 ? 0 : ssXy / ssXx;
}

// ── StreamConfig ─────────────────────────────────────────────────────────────

function defaultStreamConfig() {
  return {
    baseIntervalMs: 1000,
    noiseAmplitude: 0.02,
    driftRate: 0.0,
    anomalyProbability: 0.02,
    anomalyMagnitude: 2.5,
    numBiomarkers: 6,
    windowSize: 100,
  };
}

// ── Mulberry32 PRNG ──────────────────────────────────────────────────────────

function mulberry32(seed) {
  let t = (seed + 0x6D2B79F5) | 0;
  return function () {
    t = (t + 0x6D2B79F5) | 0;
    let z = t ^ (t >>> 15);
    z = Math.imul(z | 1, z);
    z ^= z + Math.imul(z ^ (z >>> 7), z | 61);
    return ((z ^ (z >>> 14)) >>> 0) / 4294967296;
  };
}

// Box-Muller for normal distribution
function normalSample(rng, mean, stddev) {
  const u1 = rng();
  const u2 = rng();
  return mean + stddev * Math.sqrt(-2 * Math.log(u1 || 1e-12)) * Math.cos(2 * Math.PI * u2);
}

// ── Batch generation (mirrors generate_readings in Rust) ─────────────────────

function generateReadings(config, count, seed) {
  const rng = mulberry32(seed);
  const active = BIOMARKER_DEFS.slice(0, Math.min(config.numBiomarkers, BIOMARKER_DEFS.length));
  const readings = [];

  // Pre-compute distributions per biomarker
  const dists = active.map(def => {
    const range = def.high - def.low;
    const mid = (def.low + def.high) / 2;
    const sigma = Math.max(config.noiseAmplitude * range, 1e-12);
    return { mid, range, sigma };
  });

  let ts = 0;
  for (let step = 0; step < count; step++) {
    for (let j = 0; j < active.length; j++) {
      const def = active[j];
      const { mid, range, sigma } = dists[j];
      const drift = config.driftRate * range * step;
      const isAnomaly = rng() < config.anomalyProbability;
      const effectiveSigma = isAnomaly ? sigma * config.anomalyMagnitude : sigma;
      const value = Math.max(normalSample(rng, mid + drift, effectiveSigma), 0);
      readings.push({
        timestampMs: ts,
        biomarkerId: def.id,
        value,
        referenceLow: def.low,
        referenceHigh: def.high,
        isAnomaly,
        zScore: 0,
      });
    }
    ts += config.baseIntervalMs;
  }
  return readings;
}

// ── StreamProcessor ──────────────────────────────────────────────────────────

class StreamProcessor {
  constructor(config) {
    this._config = config || defaultStreamConfig();
    this._buffers = new Map();
    this._stats = new Map();
    this._totalReadings = 0;
    this._anomalyCount = 0;
    this._anomPerBio = new Map();
    this._welford = new Map();
    this._startTs = null;
    this._lastTs = null;
  }

  _initBiomarker(id) {
    this._buffers.set(id, new RingBuffer(this._config.windowSize));
    this._stats.set(id, {
      mean: 0, variance: 0, min: Infinity, max: -Infinity,
      count: 0, anomalyRate: 0, trendSlope: 0, ema: 0,
      cusumPos: 0, cusumNeg: 0, changepointDetected: false,
    });
    // Incremental Welford state for windowed mean/variance (O(1) per reading)
    this._welford.set(id, { n: 0, mean: 0, m2: 0 });
  }

  processReading(reading) {
    const id = reading.biomarkerId;
    if (this._startTs === null) this._startTs = reading.timestampMs;
    this._lastTs = reading.timestampMs;

    if (!this._buffers.has(id)) this._initBiomarker(id);

    const buf = this._buffers.get(id);
    const evicted = buf.pushPop(reading.value);
    this._totalReadings++;

    // Incremental windowed Welford: O(1) add + O(1) remove
    const w = this._welford.get(id);
    const val = reading.value;
    if (Number.isNaN(evicted)) {
      // Buffer wasn't full — just add
      w.n++;
      const d1 = val - w.mean;
      w.mean += d1 / w.n;
      w.m2 += d1 * (val - w.mean);
    } else {
      // Buffer full — remove evicted, add new (n stays the same)
      const oldMean = w.mean;
      w.mean += (val - evicted) / w.n;
      w.m2 += (val - evicted) * ((val - w.mean) + (evicted - oldMean));
      if (w.m2 < 0) w.m2 = 0; // numerical guard
    }
    const wmean = w.mean;
    const wstd = w.n > 1 ? Math.sqrt(w.m2 / (w.n - 1)) : 0;

    const z = wstd > 1e-12 ? (val - wmean) / wstd : 0;

    const rng = reading.referenceHigh - reading.referenceLow;
    const overshoot = REF_OVERSHOOT * rng;
    const oor = val < (reading.referenceLow - overshoot) ||
                val > (reading.referenceHigh + overshoot);
    const isAnomaly = Math.abs(z) > Z_SCORE_THRESHOLD || oor;

    if (isAnomaly) {
      this._anomalyCount++;
      this._anomPerBio.set(id, (this._anomPerBio.get(id) || 0) + 1);
    }

    const slope = computeTrendSlope(buf);
    const bioAnom = this._anomPerBio.get(id) || 0;

    const st = this._stats.get(id);
    st.count++;
    st.mean = wmean;
    st.variance = wstd * wstd;
    st.trendSlope = slope;
    st.anomalyRate = bioAnom / st.count;
    if (val < st.min) st.min = val;
    if (val > st.max) st.max = val;
    st.ema = st.count === 1
      ? val
      : EMA_ALPHA * val + (1 - EMA_ALPHA) * st.ema;

    // CUSUM changepoint detection
    if (wstd > 1e-12) {
      const normDev = (val - wmean) / wstd;
      st.cusumPos = Math.max(st.cusumPos + normDev - CUSUM_DRIFT, 0);
      st.cusumNeg = Math.max(st.cusumNeg - normDev - CUSUM_DRIFT, 0);
      st.changepointDetected = st.cusumPos > CUSUM_THRESHOLD || st.cusumNeg > CUSUM_THRESHOLD;
      if (st.changepointDetected) { st.cusumPos = 0; st.cusumNeg = 0; }
    }

    return { accepted: true, zScore: z, isAnomaly, currentTrend: slope };
  }

  getStats(biomarkerId) {
    return this._stats.get(biomarkerId) || null;
  }

  summary() {
    const elapsed = (this._startTs !== null && this._lastTs !== null && this._lastTs > this._startTs)
      ? this._lastTs - this._startTs : 1;
    const ar = this._totalReadings > 0 ? this._anomalyCount / this._totalReadings : 0;
    const statsObj = {};
    for (const [k, v] of this._stats) statsObj[k] = { ...v };
    return {
      totalReadings: this._totalReadings,
      anomalyCount: this._anomalyCount,
      anomalyRate: ar,
      biomarkerStats: statsObj,
      throughputReadingsPerSec: this._totalReadings / (elapsed / 1000),
    };
  }
}

module.exports = {
  RingBuffer,
  StreamProcessor,
  BIOMARKER_DEFS,
  EMA_ALPHA,
  Z_SCORE_THRESHOLD,
  REF_OVERSHOOT,
  CUSUM_THRESHOLD,
  CUSUM_DRIFT,
  defaultStreamConfig,
  generateReadings,
};
