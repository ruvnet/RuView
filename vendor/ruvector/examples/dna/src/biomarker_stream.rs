//! Streaming biomarker data simulator with ring buffer and anomaly detection.
//!
//! Generates synthetic biomarker readings (glucose, cholesterol, HDL, LDL,
//! triglycerides, CRP) with configurable noise, drift, and anomaly injection.
//! Provides a [`StreamProcessor`] with rolling statistics, z-score anomaly
//! detection, and linear regression trend analysis over a [`RingBuffer`].

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rand_distr::Normal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for simulated biomarker streams.
#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub base_interval_ms: u64,
    pub noise_amplitude: f64,
    pub drift_rate: f64,
    pub anomaly_probability: f64,
    pub anomaly_magnitude: f64,
    pub num_biomarkers: usize,
    pub window_size: usize,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            base_interval_ms: 1000,
            noise_amplitude: 0.02,
            drift_rate: 0.0,
            anomaly_probability: 0.02,
            anomaly_magnitude: 2.5,
            num_biomarkers: 6,
            window_size: 100,
        }
    }
}

/// A single timestamped biomarker data point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiomarkerReading {
    pub timestamp_ms: u64,
    pub biomarker_id: String,
    pub value: f64,
    pub reference_low: f64,
    pub reference_high: f64,
    pub is_anomaly: bool,
    pub z_score: f64,
}

/// Fixed-capacity circular buffer backed by a flat `Vec<T>`.
///
/// Eliminates the `Option<T>` wrapper used in naive implementations,
/// halving per-slot memory for primitive types like `f64` (8 bytes vs 16).
pub struct RingBuffer<T> {
    buffer: Vec<T>,
    head: usize,
    len: usize,
    capacity: usize,
}

impl<T: Clone + Default> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "RingBuffer capacity must be > 0");
        Self {
            buffer: vec![T::default(); capacity],
            head: 0,
            len: 0,
            capacity,
        }
    }

    pub fn push(&mut self, item: T) {
        self.buffer[self.head] = item;
        self.head = (self.head + 1) % self.capacity;
        if self.len < self.capacity {
            self.len += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let start = if self.len < self.capacity {
            0
        } else {
            self.head
        };
        let (cap, len) = (self.capacity, self.len);
        (0..len).map(move |i| &self.buffer[(start + i) % cap])
    }

    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_full(&self) -> bool {
        self.len == self.capacity
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }
}

// ── Biomarker definitions ───────────────────────────────────────────────────

struct BiomarkerDef {
    id: &'static str,
    low: f64,
    high: f64,
}

const BIOMARKER_DEFS: &[BiomarkerDef] = &[
    BiomarkerDef {
        id: "glucose",
        low: 70.0,
        high: 100.0,
    },
    BiomarkerDef {
        id: "cholesterol_total",
        low: 150.0,
        high: 200.0,
    },
    BiomarkerDef {
        id: "hdl",
        low: 40.0,
        high: 60.0,
    },
    BiomarkerDef {
        id: "ldl",
        low: 70.0,
        high: 130.0,
    },
    BiomarkerDef {
        id: "triglycerides",
        low: 50.0,
        high: 150.0,
    },
    BiomarkerDef {
        id: "crp",
        low: 0.1,
        high: 3.0,
    },
];

// ── Batch generation ────────────────────────────────────────────────────────

/// Generate `count` synthetic readings per active biomarker with noise, drift,
/// and stochastic anomaly spikes.
pub fn generate_readings(config: &StreamConfig, count: usize, seed: u64) -> Vec<BiomarkerReading> {
    let mut rng = StdRng::seed_from_u64(seed);
    let active = &BIOMARKER_DEFS[..config.num_biomarkers.min(BIOMARKER_DEFS.len())];
    let mut readings = Vec::with_capacity(count * active.len());
    // Pre-compute distributions per biomarker (avoids Normal::new in inner loop)
    let dists: Vec<_> = active
        .iter()
        .map(|def| {
            let range = def.high - def.low;
            let mid = (def.low + def.high) / 2.0;
            let sigma = (config.noise_amplitude * range).max(1e-12);
            let normal = Normal::new(0.0, sigma).unwrap();
            let spike = Normal::new(0.0, sigma * config.anomaly_magnitude).unwrap();
            (mid, range, normal, spike)
        })
        .collect();
    let mut ts: u64 = 0;

    for step in 0..count {
        for (j, def) in active.iter().enumerate() {
            let (mid, range, ref normal, ref spike) = dists[j];
            let drift = config.drift_rate * range * step as f64;
            let is_anom = rng.gen::<f64>() < config.anomaly_probability;
            let value = if is_anom {
                (mid + rng.sample::<f64, _>(spike) + drift).max(0.0)
            } else {
                (mid + rng.sample::<f64, _>(normal) + drift).max(0.0)
            };
            readings.push(BiomarkerReading {
                timestamp_ms: ts,
                biomarker_id: def.id.into(),
                value,
                reference_low: def.low,
                reference_high: def.high,
                is_anomaly: is_anom,
                z_score: 0.0,
            });
        }
        ts += config.base_interval_ms;
    }
    readings
}

// ── Statistics & results ────────────────────────────────────────────────────

/// Rolling statistics for a single biomarker stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStats {
    pub mean: f64,
    pub variance: f64,
    pub min: f64,
    pub max: f64,
    pub count: u64,
    pub anomaly_rate: f64,
    pub trend_slope: f64,
    pub ema: f64,
    pub cusum_pos: f64, // CUSUM positive direction
    pub cusum_neg: f64, // CUSUM negative direction
    pub changepoint_detected: bool,
}

impl Default for StreamStats {
    fn default() -> Self {
        Self {
            mean: 0.0,
            variance: 0.0,
            min: f64::MAX,
            max: f64::MIN,
            count: 0,
            anomaly_rate: 0.0,
            trend_slope: 0.0,
            ema: 0.0,
            cusum_pos: 0.0,
            cusum_neg: 0.0,
            changepoint_detected: false,
        }
    }
}

/// Result of processing a single reading.
pub struct ProcessingResult {
    pub accepted: bool,
    pub z_score: f64,
    pub is_anomaly: bool,
    pub current_trend: f64,
}

/// Aggregate summary across all biomarker streams.
pub struct StreamSummary {
    pub total_readings: u64,
    pub anomaly_count: u64,
    pub anomaly_rate: f64,
    pub biomarker_stats: HashMap<String, StreamStats>,
    pub throughput_readings_per_sec: f64,
}

// ── Stream processor ────────────────────────────────────────────────────────

const EMA_ALPHA: f64 = 0.1;
const Z_SCORE_THRESHOLD: f64 = 2.5;
const REF_OVERSHOOT: f64 = 0.20;
const CUSUM_THRESHOLD: f64 = 4.0; // Cumulative sum threshold for changepoint detection
const CUSUM_DRIFT: f64 = 0.5; // Allowable drift before CUSUM accumulates

/// Processes biomarker readings with per-stream ring buffers, z-score anomaly
/// detection, and trend analysis via simple linear regression.
pub struct StreamProcessor {
    config: StreamConfig,
    buffers: HashMap<String, RingBuffer<f64>>,
    stats: HashMap<String, StreamStats>,
    total_readings: u64,
    anomaly_count: u64,
    anom_per_bio: HashMap<String, u64>,
    start_ts: Option<u64>,
    last_ts: Option<u64>,
}

impl StreamProcessor {
    pub fn new(config: StreamConfig) -> Self {
        let cap = config.num_biomarkers;
        Self {
            config,
            buffers: HashMap::with_capacity(cap),
            stats: HashMap::with_capacity(cap),
            total_readings: 0,
            anomaly_count: 0,
            anom_per_bio: HashMap::with_capacity(cap),
            start_ts: None,
            last_ts: None,
        }
    }

    pub fn process_reading(&mut self, reading: &BiomarkerReading) -> ProcessingResult {
        let id = &reading.biomarker_id;
        if self.start_ts.is_none() {
            self.start_ts = Some(reading.timestamp_ms);
        }
        self.last_ts = Some(reading.timestamp_ms);

        let buf = self
            .buffers
            .entry(id.clone())
            .or_insert_with(|| RingBuffer::new(self.config.window_size));
        buf.push(reading.value);
        self.total_readings += 1;

        let (wmean, wstd) = window_mean_std(buf);
        let z = if wstd > 1e-12 {
            (reading.value - wmean) / wstd
        } else {
            0.0
        };

        let rng = reading.reference_high - reading.reference_low;
        let overshoot = REF_OVERSHOOT * rng;
        let oor = reading.value < (reading.reference_low - overshoot)
            || reading.value > (reading.reference_high + overshoot);
        let is_anom = z.abs() > Z_SCORE_THRESHOLD || oor;

        if is_anom {
            self.anomaly_count += 1;
            *self.anom_per_bio.entry(id.clone()).or_insert(0) += 1;
        }

        let slope = compute_trend_slope(buf);
        let bio_anom = *self.anom_per_bio.get(id).unwrap_or(&0);
        let st = self.stats.entry(id.clone()).or_default();
        st.count += 1;
        st.mean = wmean;
        st.variance = wstd * wstd;
        st.trend_slope = slope;
        st.anomaly_rate = bio_anom as f64 / st.count as f64;
        if reading.value < st.min {
            st.min = reading.value;
        }
        if reading.value > st.max {
            st.max = reading.value;
        }
        st.ema = if st.count == 1 {
            reading.value
        } else {
            EMA_ALPHA * reading.value + (1.0 - EMA_ALPHA) * st.ema
        };
        // CUSUM changepoint detection: accumulate deviations from the mean
        if wstd > 1e-12 {
            let norm_dev = (reading.value - wmean) / wstd;
            st.cusum_pos = (st.cusum_pos + norm_dev - CUSUM_DRIFT).max(0.0);
            st.cusum_neg = (st.cusum_neg - norm_dev - CUSUM_DRIFT).max(0.0);
            st.changepoint_detected =
                st.cusum_pos > CUSUM_THRESHOLD || st.cusum_neg > CUSUM_THRESHOLD;
            if st.changepoint_detected {
                st.cusum_pos = 0.0;
                st.cusum_neg = 0.0;
            }
        }

        ProcessingResult {
            accepted: true,
            z_score: z,
            is_anomaly: is_anom,
            current_trend: slope,
        }
    }

    pub fn get_stats(&self, biomarker_id: &str) -> Option<&StreamStats> {
        self.stats.get(biomarker_id)
    }

    pub fn summary(&self) -> StreamSummary {
        let elapsed = match (self.start_ts, self.last_ts) {
            (Some(s), Some(e)) if e > s => (e - s) as f64,
            _ => 1.0,
        };
        let ar = if self.total_readings > 0 {
            self.anomaly_count as f64 / self.total_readings as f64
        } else {
            0.0
        };
        StreamSummary {
            total_readings: self.total_readings,
            anomaly_count: self.anomaly_count,
            anomaly_rate: ar,
            biomarker_stats: self.stats.clone(),
            throughput_readings_per_sec: self.total_readings as f64 / (elapsed / 1000.0),
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Single-pass mean and sample standard deviation using Welford's online algorithm.
/// Avoids iterating the buffer twice (sum then variance) — 2x fewer cache misses.
fn window_mean_std(buf: &RingBuffer<f64>) -> (f64, f64) {
    let n = buf.len();
    if n == 0 {
        return (0.0, 0.0);
    }
    let mut mean = 0.0;
    let mut m2 = 0.0;
    for (k, &x) in buf.iter().enumerate() {
        let k1 = (k + 1) as f64;
        let delta = x - mean;
        mean += delta / k1;
        m2 += delta * (x - mean);
    }
    if n < 2 {
        return (mean, 0.0);
    }
    (mean, (m2 / (n - 1) as f64).sqrt())
}

fn compute_trend_slope(buf: &RingBuffer<f64>) -> f64 {
    let n = buf.len();
    if n < 2 {
        return 0.0;
    }
    let nf = n as f64;
    let xm = (nf - 1.0) / 2.0;
    let (mut ys, mut xys, mut xxs) = (0.0, 0.0, 0.0);
    for (i, &y) in buf.iter().enumerate() {
        let x = i as f64;
        ys += y;
        xys += x * y;
        xxs += x * x;
    }
    let ss_xy = xys - nf * xm * (ys / nf);
    let ss_xx = xxs - nf * xm * xm;
    if ss_xx.abs() < 1e-12 {
        0.0
    } else {
        ss_xy / ss_xx
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(ts: u64, id: &str, val: f64, lo: f64, hi: f64) -> BiomarkerReading {
        BiomarkerReading {
            timestamp_ms: ts,
            biomarker_id: id.into(),
            value: val,
            reference_low: lo,
            reference_high: hi,
            is_anomaly: false,
            z_score: 0.0,
        }
    }

    fn glucose(ts: u64, val: f64) -> BiomarkerReading {
        reading(ts, "glucose", val, 70.0, 100.0)
    }

    // -- RingBuffer --

    #[test]
    fn ring_buffer_push_iter_len() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(4);
        for v in [10, 20, 30] {
            rb.push(v);
        }
        assert_eq!(rb.iter().copied().collect::<Vec<_>>(), vec![10, 20, 30]);
        assert_eq!(rb.len(), 3);
        assert!(!rb.is_full());
    }

    #[test]
    fn ring_buffer_overflow_keeps_newest() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(3);
        for v in 1..=4 {
            rb.push(v);
        }
        assert!(rb.is_full());
        assert_eq!(rb.iter().copied().collect::<Vec<_>>(), vec![2, 3, 4]);
    }

    #[test]
    fn ring_buffer_capacity_one() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(1);
        rb.push(42);
        rb.push(99);
        assert_eq!(rb.iter().copied().collect::<Vec<_>>(), vec![99]);
    }

    #[test]
    fn ring_buffer_clear_resets() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(3);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert_eq!(rb.len(), 0);
        assert!(!rb.is_full());
        assert_eq!(rb.iter().count(), 0);
    }

    // -- Batch generation --

    #[test]
    fn generate_correct_count_and_ids() {
        let cfg = StreamConfig::default();
        let readings = generate_readings(&cfg, 50, 42);
        assert_eq!(readings.len(), 50 * cfg.num_biomarkers);
        let valid: Vec<&str> = BIOMARKER_DEFS.iter().map(|d| d.id).collect();
        for r in &readings {
            assert!(valid.contains(&r.biomarker_id.as_str()));
        }
    }

    #[test]
    fn generated_reference_ranges_match_defs() {
        let readings = generate_readings(&StreamConfig::default(), 20, 123);
        for r in &readings {
            let d = BIOMARKER_DEFS
                .iter()
                .find(|d| d.id == r.biomarker_id)
                .unwrap();
            assert!((r.reference_low - d.low).abs() < 1e-9);
            assert!((r.reference_high - d.high).abs() < 1e-9);
        }
    }

    #[test]
    fn generated_values_non_negative() {
        for r in &generate_readings(&StreamConfig::default(), 100, 999) {
            assert!(r.value >= 0.0);
        }
    }

    // -- StreamProcessor --

    #[test]
    fn processor_computes_stats() {
        let cfg = StreamConfig {
            window_size: 10,
            ..Default::default()
        };
        let mut p = StreamProcessor::new(cfg.clone());
        for r in &generate_readings(&cfg, 20, 55) {
            p.process_reading(r);
        }
        let s = p.get_stats("glucose").unwrap();
        assert!(s.count > 0 && s.mean > 0.0 && s.min <= s.max);
    }

    #[test]
    fn processor_summary_totals() {
        let cfg = StreamConfig::default();
        let mut p = StreamProcessor::new(cfg.clone());
        for r in &generate_readings(&cfg, 30, 77) {
            p.process_reading(r);
        }
        let s = p.summary();
        assert_eq!(s.total_readings, 30 * cfg.num_biomarkers as u64);
        assert!((0.0..=1.0).contains(&s.anomaly_rate));
    }

    // -- Anomaly detection --

    #[test]
    fn detects_z_score_anomaly() {
        let mut p = StreamProcessor::new(StreamConfig {
            window_size: 20,
            ..Default::default()
        });
        for i in 0..20 {
            p.process_reading(&glucose(i * 1000, 85.0));
        }
        let r = p.process_reading(&glucose(20_000, 300.0));
        assert!(r.is_anomaly);
        assert!(r.z_score.abs() > Z_SCORE_THRESHOLD);
    }

    #[test]
    fn detects_out_of_range_anomaly() {
        let mut p = StreamProcessor::new(StreamConfig {
            window_size: 5,
            ..Default::default()
        });
        for (i, v) in [80.0, 82.0, 78.0, 84.0, 81.0].iter().enumerate() {
            p.process_reading(&glucose(i as u64 * 1000, *v));
        }
        // 140 >> ref_high(100) + 20%*range(30)=106
        assert!(p.process_reading(&glucose(5000, 140.0)).is_anomaly);
    }

    #[test]
    fn zero_anomaly_rate_for_constant_stream() {
        let mut p = StreamProcessor::new(StreamConfig {
            window_size: 50,
            ..Default::default()
        });
        for i in 0..10 {
            p.process_reading(&reading(i * 1000, "crp", 1.5, 0.1, 3.0));
        }
        assert!(p.get_stats("crp").unwrap().anomaly_rate.abs() < 1e-9);
    }

    // -- Trend detection --

    #[test]
    fn positive_trend_for_increasing() {
        let mut p = StreamProcessor::new(StreamConfig {
            window_size: 20,
            ..Default::default()
        });
        let mut r = ProcessingResult {
            accepted: true,
            z_score: 0.0,
            is_anomaly: false,
            current_trend: 0.0,
        };
        for i in 0..20 {
            r = p.process_reading(&glucose(i * 1000, 70.0 + i as f64));
        }
        assert!(r.current_trend > 0.0, "got {}", r.current_trend);
    }

    #[test]
    fn negative_trend_for_decreasing() {
        let mut p = StreamProcessor::new(StreamConfig {
            window_size: 20,
            ..Default::default()
        });
        let mut r = ProcessingResult {
            accepted: true,
            z_score: 0.0,
            is_anomaly: false,
            current_trend: 0.0,
        };
        for i in 0..20 {
            r = p.process_reading(&reading(i * 1000, "hdl", 60.0 - i as f64 * 0.5, 40.0, 60.0));
        }
        assert!(r.current_trend < 0.0, "got {}", r.current_trend);
    }

    #[test]
    fn exact_slope_for_linear_series() {
        let mut p = StreamProcessor::new(StreamConfig {
            window_size: 10,
            ..Default::default()
        });
        for i in 0..10 {
            p.process_reading(&reading(
                i * 1000,
                "ldl",
                100.0 + i as f64 * 3.0,
                70.0,
                130.0,
            ));
        }
        assert!((p.get_stats("ldl").unwrap().trend_slope - 3.0).abs() < 1e-9);
    }

    // -- Z-score --

    #[test]
    fn z_score_small_for_near_mean() {
        let mut p = StreamProcessor::new(StreamConfig {
            window_size: 10,
            ..Default::default()
        });
        for (i, v) in [80.0, 82.0, 78.0, 84.0, 76.0, 86.0, 81.0, 79.0, 83.0]
            .iter()
            .enumerate()
        {
            p.process_reading(&glucose(i as u64 * 1000, *v));
        }
        let mean = p.get_stats("glucose").unwrap().mean;
        assert!(p.process_reading(&glucose(9000, mean)).z_score.abs() < 1.0);
    }

    // -- EMA --

    #[test]
    fn ema_converges_to_constant() {
        let mut p = StreamProcessor::new(StreamConfig {
            window_size: 50,
            ..Default::default()
        });
        for i in 0..50 {
            p.process_reading(&reading(i * 1000, "crp", 2.0, 0.1, 3.0));
        }
        assert!((p.get_stats("crp").unwrap().ema - 2.0).abs() < 1e-6);
    }
}
