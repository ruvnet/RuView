//! ADR-121 + ADR-122: HLK-LD2402 24 GHz mmWave radar reader.
//!
//! Auxiliary range + per-range-gate vitals modality, attached over a
//! CP2102 USB-UART bridge.
//!
//! Two operating modes are supported on the wire:
//!
//! * **Normal Mode** (factory default) — emits ASCII
//!   `distance:<cm>\r\n` lines @ 115200 baud, ~6 Hz. Implemented by
//!   `parse_distance` as a fallback when Engineering Mode setup fails
//!   or no enable-config ACK is received.
//!
//! * **Engineering Mode** (ADR-122) — after the host issues
//!   `enable-config → set-mode(0x04) → disable-config` the module
//!   emits binary frames at the same ~6 Hz cadence:
//!
//!   ```text
//!   F4 F3 F2 F1   data header
//!   LL LL         u16 LE length of payload (typically 131)
//!   01            frame type = engineering
//!   DD DD         u16 LE distance (cm)
//!   00 00 00 00 00 00 00 00       8 reserved bytes
//!   <15 × u32 LE> 15 motion-gate energies (gate 0 = 0–0.7 m, etc.)
//!   <15 × u32 LE> 15 micromotion-gate energies (same range bins)
//!   F8 F7 F6 F5   data footer
//!   ```
//!
//!   The micromotion gate at the target's range is the input we feed
//!   into the vital-sign detector for heart-rate extraction — at 6 Hz
//!   the energy time-series carries cardiac modulation that the
//!   integer-cm distance reading cannot resolve.
//!
//! This reader runs in a dedicated thread (blocking serial I/O is
//! awkward inside tokio) and pushes the latest distance, the per-gate
//! energy snapshot, and the computed VitalSigns into global
//! `OnceLock<Mutex<…>>` slots that the broadcast tick task reads.
//!
//! Cold-start tolerance: if the port cannot be opened, the thread
//! logs once and exits cleanly — the server keeps running with WiFi
//! sensing only. If config commands fail, we fall back to the ASCII
//! Normal Mode parser so distance keeps working.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::vital_signs::{VitalSignDetector, VitalSigns};

/// HLK-LD2402 Normal-Mode cadence: factory firmware emits ~6 distance
/// lines/sec. We pick 6.0 Hz as the detector's nominal sample_rate so
/// the breathing band (0.1–0.5 Hz) sits comfortably inside Nyquist.
const MMWAVE_SAMPLE_RATE_HZ: f64 = 6.0;

/// Gate count emitted in each Engineering Mode frame (each for
/// motion + micromotion). The module's firmware reports 15 gates of
/// 0.7 m each, covering 0–10.5 m.
const GATE_COUNT: usize = 15;

/// Bytes per gate energy (u32 LE).
const GATE_BYTES: usize = 4;

/// Engineering frame fixed-payload prelude: type(1) + distance(2) + 8 reserved.
const ENG_PRELUDE_BYTES: usize = 1 + 2 + 8;

/// Expected payload length for an engineering frame:
/// prelude(11) + 2 * 15 gates * 4 bytes = 131.
const ENG_PAYLOAD_LEN: u16 = (ENG_PRELUDE_BYTES + 2 * GATE_COUNT * GATE_BYTES) as u16;

/// Engineering data frame markers.
const DATA_HEAD: [u8; 4] = [0xF4, 0xF3, 0xF2, 0xF1];
const DATA_FOOT: [u8; 4] = [0xF8, 0xF7, 0xF6, 0xF5];

/// Engineering frame type byte (observed on LD2402 firmware; ESPHome
/// docs mention 0x84 but the actual device emits 0x01).
const FRAME_TYPE_ENGINEERING: u8 = 0x01;

/// How many recent micromotion-energy samples to keep per gate. At
/// 6 Hz sampling, 30 s = 180 samples — enough for the FFT bin in the
/// heartbeat band (0.8–2.0 Hz) to resolve cleanly.
const GATE_HISTORY_LEN: usize = 180;

/// Latest mmWave reading + when it landed.
#[derive(Debug, Clone, Copy)]
pub struct MmwaveReading {
    pub distance_cm: u32,
    pub at: Instant,
}

/// Latest per-gate energy snapshot.
#[derive(Debug, Clone)]
pub struct GateSnapshot {
    /// Motion gate energies (raw u32 from frame).
    pub motion: [u32; GATE_COUNT],
    /// Micromotion gate energies (raw u32 from frame).
    pub micro: [u32; GATE_COUNT],
    /// Index of the dominant gate (highest motion energy) — used as
    /// the target-gate hint for HR extraction.
    pub target_gate: usize,
    pub at: Instant,
}

static LATEST: OnceLock<Mutex<Option<MmwaveReading>>> = OnceLock::new();
static GATES: OnceLock<Mutex<Option<GateSnapshot>>> = OnceLock::new();
static VITALS: OnceLock<Mutex<Option<VitalSigns>>> = OnceLock::new();
static BR_DETECTOR: OnceLock<Mutex<VitalSignDetector>> = OnceLock::new();
static HR_HISTORY: OnceLock<Mutex<Vec<VecDeque<f64>>>> = OnceLock::new();

fn latest() -> &'static Mutex<Option<MmwaveReading>> {
    LATEST.get_or_init(|| Mutex::new(None))
}

fn gates_slot() -> &'static Mutex<Option<GateSnapshot>> {
    GATES.get_or_init(|| Mutex::new(None))
}

fn vitals_slot() -> &'static Mutex<Option<VitalSigns>> {
    VITALS.get_or_init(|| Mutex::new(None))
}

fn br_detector() -> &'static Mutex<VitalSignDetector> {
    BR_DETECTOR.get_or_init(|| Mutex::new(VitalSignDetector::new(MMWAVE_SAMPLE_RATE_HZ)))
}

fn hr_history() -> &'static Mutex<Vec<VecDeque<f64>>> {
    HR_HISTORY.get_or_init(|| {
        Mutex::new(
            (0..GATE_COUNT)
                .map(|_| VecDeque::with_capacity(GATE_HISTORY_LEN))
                .collect(),
        )
    })
}

/// Returns the most recent distance reading if it landed within `staleness`.
pub fn current(staleness: Duration) -> Option<MmwaveReading> {
    let g = latest().lock().unwrap();
    let r = (*g)?;
    if r.at.elapsed() <= staleness { Some(r) } else { None }
}

/// Returns the latest per-gate energy snapshot, gated on freshness.
pub fn current_gates(staleness: Duration) -> Option<GateSnapshot> {
    let g = gates_slot().lock().unwrap();
    let s = (*g).clone()?;
    if s.at.elapsed() <= staleness { Some(s) } else { None }
}

/// Returns the latest mmWave-derived VitalSigns. Breathing is computed
/// from the distance time-series; heart rate (when present) is
/// computed from the micromotion-gate energy time-series at the
/// detected target gate.
///
/// Returns `None` if the most recent distance reading is older than
/// `staleness`.
pub fn current_vitals(staleness: Duration) -> Option<VitalSigns> {
    current(staleness)?;
    vitals_slot().lock().unwrap().clone()
}

/// Buffer fill stats for UI ("breathing: 120/180 samples").
pub fn buffer_status() -> (usize, usize) {
    let det = br_detector().lock().unwrap();
    let (br_samples, br_cap, _hr_samples, _hr_cap) = det.buffer_status();
    (br_samples, br_cap)
}

/// Spawn the blocking serial reader thread. Returns immediately.
/// `port` example: `/dev/cu.usbserial-1140` (macOS) or `/dev/ttyUSB0`
/// (Linux). `baud` should be 115200 for HLK-LD2402 default firmware.
pub fn spawn_reader(port: String, baud: u32) {
    std::thread::Builder::new()
        .name("mmwave-reader".into())
        .spawn(move || run(port, baud))
        .expect("failed to spawn mmwave-reader thread");
}

// ── Command frame builders (host → module) ──────────────────────────

const CMD_HEAD: [u8; 4] = [0xFD, 0xFC, 0xFB, 0xFA];
const CMD_FOOT: [u8; 4] = [0x04, 0x03, 0x02, 0x01];

fn build_cmd(cmd: u16, data: &[u8]) -> Vec<u8> {
    let body_len: u16 = (2 + data.len()) as u16;
    let mut out = Vec::with_capacity(4 + 2 + 2 + data.len() + 4);
    out.extend_from_slice(&CMD_HEAD);
    out.extend_from_slice(&body_len.to_le_bytes());
    out.extend_from_slice(&cmd.to_le_bytes());
    out.extend_from_slice(data);
    out.extend_from_slice(&CMD_FOOT);
    out
}

fn enable_engineering_mode(serial: &mut dyn serialport::SerialPort) -> std::io::Result<()> {
    // 1) enable config: cmd 0x00FF, data = 01 00 (protocol-version request)
    serial.write_all(&build_cmd(0x00FF, &[0x01, 0x00]))?;
    std::thread::sleep(Duration::from_millis(200));
    let _ = drain(serial);
    // 2) set work mode = engineering (0x04). Payload is 2 reserved bytes
    //    followed by the u32 mode value.
    let mut mode_data = [0u8; 6];
    mode_data[2..].copy_from_slice(&0x0000_0004u32.to_le_bytes());
    serial.write_all(&build_cmd(0x0012, &mode_data))?;
    std::thread::sleep(Duration::from_millis(300));
    let _ = drain(serial);
    // 3) disable config — data flow resumes (now binary).
    serial.write_all(&build_cmd(0x00FE, &[]))?;
    std::thread::sleep(Duration::from_millis(200));
    let _ = drain(serial);
    Ok(())
}

fn drain(serial: &mut dyn serialport::SerialPort) -> std::io::Result<()> {
    let mut buf = [0u8; 256];
    for _ in 0..4 {
        match serial.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => continue,
        }
    }
    Ok(())
}

fn run(port: String, baud: u32) {
    let mut serial = match serialport::new(&port, baud)
        .timeout(Duration::from_millis(500))
        .open()
    {
        Ok(s) => {
            tracing::info!("ADR-121 mmWave reader: opened {port} @ {baud}");
            s
        }
        Err(e) => {
            tracing::warn!("ADR-121 mmWave reader: cannot open {port} @ {baud}: {e}");
            return;
        }
    };

    // Try to switch into Engineering Mode. If it fails the loop below
    // still parses ASCII `distance:NNN\r\n` Normal-Mode lines, so the
    // distance pill keeps working even when commands can't get through.
    let engineering_mode = match enable_engineering_mode(serial.as_mut()) {
        Ok(()) => {
            tracing::info!(
                "ADR-122 mmWave: Engineering Mode enabled (per-gate energies @ {} Hz)",
                MMWAVE_SAMPLE_RATE_HZ
            );
            true
        }
        Err(e) => {
            tracing::warn!(
                "ADR-122 mmWave: engineering-mode setup failed ({e}); falling back to ASCII Normal Mode"
            );
            false
        }
    };

    let mut buf: Vec<u8> = Vec::with_capacity(2048);
    let mut tmp = [0u8; 1024];
    loop {
        match serial.read(&mut tmp) {
            Ok(0) => continue,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::TimedOut {
                    continue;
                }
                tracing::warn!("ADR-121 mmWave reader: read error: {e}");
                return;
            }
        }

        if engineering_mode {
            // Binary frames only. The engineering payload contains
            // arbitrary bytes (including 0x0A), so we must NOT run the
            // ASCII line-drain or it will chop partial frames in half
            // mid-buffer. Lesson learned: at 6 Hz × 141 bytes the
            // tail of every read is usually a partial frame, and the
            // ASCII drain destroyed ~80 % of them in our first cut.
            consume_binary_frames(&mut buf);
        } else {
            // Normal Mode fallback: ASCII `distance:NNN\r\n` lines.
            while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                let raw_line: Vec<u8> = buf.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&raw_line).trim().to_string();
                if let Some(cm) = parse_distance(&line) {
                    ingest_distance(cm);
                }
            }
        }

        if buf.len() > 8192 {
            // Runaway buffer guard.
            buf.clear();
        }
    }
}

/// Locate and consume any complete binary engineering frames inside
/// `buf`. Anything between frames is kept (ASCII lines are processed
/// separately by the caller).
fn consume_binary_frames(buf: &mut Vec<u8>) {
    loop {
        let Some(start) = find_subseq(buf, &DATA_HEAD) else {
            // Trim head if buffer is huge and no header is in sight.
            if buf.len() > 4096 {
                let keep = std::cmp::min(8, buf.len());
                let new_buf = buf.split_off(buf.len() - keep);
                *buf = new_buf;
            }
            return;
        };

        // Need head(4) + len(2) + at least 1 byte of payload to know size.
        if buf.len() < start + 4 + 2 + 1 {
            // Wait for more data.
            // Drop the prefix in front of the header to avoid scanning it forever.
            if start > 0 {
                buf.drain(..start);
            }
            return;
        }

        let len_bytes = &buf[start + 4..start + 6];
        let payload_len = u16::from_le_bytes([len_bytes[0], len_bytes[1]]) as usize;
        let end = start + 4 + 2 + payload_len + 4; // head + len + payload + foot
        if buf.len() < end {
            if start > 0 {
                buf.drain(..start);
            }
            return; // wait for more
        }

        // Footer check; if missing, drop just the header and keep scanning.
        if buf[end - 4..end] != DATA_FOOT {
            buf.drain(..start + 4);
            continue;
        }

        let payload = &buf[start + 6..start + 6 + payload_len];
        parse_engineering_payload(payload);
        buf.drain(..end);
    }
}

fn find_subseq(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Parse the payload (between length field and footer) of an
/// engineering data frame and feed it into the global state.
fn parse_engineering_payload(payload: &[u8]) {
    if payload.len() < ENG_PRELUDE_BYTES {
        return;
    }
    if payload[0] != FRAME_TYPE_ENGINEERING {
        return;
    }
    if payload.len() < ENG_PAYLOAD_LEN as usize {
        return;
    }
    let distance_cm = u16::from_le_bytes([payload[1], payload[2]]) as u32;

    // Gates start after the 11-byte prelude.
    let mut motion = [0u32; GATE_COUNT];
    let mut micro = [0u32; GATE_COUNT];
    let motion_start = ENG_PRELUDE_BYTES;
    let micro_start = motion_start + GATE_COUNT * GATE_BYTES;
    for (i, slot) in motion.iter_mut().enumerate() {
        let off = motion_start + i * GATE_BYTES;
        *slot = u32::from_le_bytes([
            payload[off], payload[off + 1], payload[off + 2], payload[off + 3],
        ]);
    }
    for (i, slot) in micro.iter_mut().enumerate() {
        let off = micro_start + i * GATE_BYTES;
        *slot = u32::from_le_bytes([
            payload[off], payload[off + 1], payload[off + 2], payload[off + 3],
        ]);
    }

    // Target gate selection — three layered heuristics, in priority:
    //
    //  1. The gate that brackets the reported `distance_cm` value
    //     (each gate is 0.7 m wide). This is the gate where the body
    //     is physically located, and where chest-induced micromotion
    //     should be most pronounced.
    //
    //  2. If that gate's micromotion energy is too low (e.g. the
    //     distance reading is stale or the radar guessed wrong), fall
    //     back to the gate with the highest micromotion energy among
    //     the mid-range gates 1..GATE_COUNT-2 (gate 0 is dominated by
    //     near-field clutter; the last gate is usually empty).
    //
    //  3. Final fallback: gate 1, which is the most common torso
    //     distance for a seated operator.
    let dist_gate = ((distance_cm as f64) / 70.0).floor() as usize;
    let target_gate = if dist_gate < GATE_COUNT && micro[dist_gate] > 0 {
        dist_gate
    } else {
        // Pick the micro-peak from mid-range gates only.
        let lo = 1usize;
        let hi = GATE_COUNT.saturating_sub(1).max(lo + 1);
        micro[lo..hi]
            .iter()
            .enumerate()
            .max_by_key(|(_, &e)| e)
            .map(|(i, _)| i + lo)
            .unwrap_or(1)
    };

    let now = Instant::now();
    *gates_slot().lock().unwrap() = Some(GateSnapshot {
        motion,
        micro,
        target_gate,
        at: now,
    });
    *latest().lock().unwrap() = Some(MmwaveReading {
        distance_cm,
        at: now,
    });

    // Push each gate's micromotion energy into its rolling history. We
    // keep all gates rather than only the current target — the target
    // can drift between samples and using a fixed gate over the FFT
    // window avoids discontinuity in the time-series.
    {
        let mut hist = hr_history().lock().unwrap();
        for (i, &e) in micro.iter().enumerate() {
            let q = &mut hist[i];
            // log-compress to suppress dynamic range and keep cardiac
            // ripple visible against breathing baseline.
            let v = if e > 0 { (e as f64).ln() } else { 0.0 };
            q.push_back(v);
            while q.len() > GATE_HISTORY_LEN {
                q.pop_front();
            }
        }
    }

    // ── breathing: distance time-series via existing detector ──────
    let cm_f = distance_cm as f64;
    let mut vs = br_detector().lock().unwrap().process_frame(&[cm_f], &[]);

    // ── heart rate: per-gate micromotion FFT in HR band ────────────
    let (hr_bpm, hr_conf) = compute_heart_rate(target_gate);
    vs.heart_rate_bpm = hr_bpm;
    vs.heartbeat_confidence = hr_conf;

    *vitals_slot().lock().unwrap() = Some(vs);
}

/// Run a bandpass + FFT-peak search on the target gate's micromotion
/// energy history, returning (BPM, confidence) in the 40–180 BPM range
/// (0.667–3.0 Hz). Returns (None, 0.0) until the buffer is full.
///
/// We reuse the same bandpass+FFT code path used by the WiFi-CSI
/// detector by spinning up a one-shot VitalSignDetector and feeding it
/// the recent history — that gives us identical confidence semantics
/// and avoids re-implementing peak detection.
fn compute_heart_rate(target_gate: usize) -> (Option<f64>, f64) {
    use crate::vital_signs::bandpass_filter;

    let hist = hr_history().lock().unwrap();
    let q = match hist.get(target_gate) {
        Some(q) => q,
        None => return (None, 0.0),
    };
    if q.len() < GATE_HISTORY_LEN / 2 {
        return (None, 0.0); // buffer still warming up (≥90 samples)
    }
    let samples: Vec<f64> = q.iter().copied().collect();
    drop(hist);

    // Heart rate band: 0.8–2.0 Hz = 48–120 BPM at adult resting.
    let filtered = bandpass_filter(&samples, 0.8, 2.0, MMWAVE_SAMPLE_RATE_HZ);
    let (peak_freq, peak_mag, band_mean) = fft_peak_in_band(&filtered, 0.8, 2.0, MMWAVE_SAMPLE_RATE_HZ);

    if peak_freq <= 0.0 || band_mean <= f64::EPSILON {
        return (None, 0.0);
    }
    let bpm = peak_freq * 60.0;
    let ratio = peak_mag / band_mean;
    // Same confidence shape as VitalSignDetector (threshold ~ 1.5):
    // ratio≥1.5 → high confidence, 1.0..1.5 → low.
    let confidence = if ratio >= 1.5 {
        ((ratio - 1.0) / 2.0).clamp(0.0, 1.0)
    } else {
        ((ratio - 1.0) / 0.5 * 0.5).clamp(0.0, 0.5)
    };
    (Some(bpm), confidence)
}

/// Naive radix-2 FFT magnitude peak search inside `[min_hz, max_hz]`.
/// Returns (peak_freq_hz, peak_mag, band_mean). Used only by the HR
/// path; the breathing path goes through the shared detector.
fn fft_peak_in_band(signal: &[f64], min_hz: f64, max_hz: f64, sample_rate: f64) -> (f64, f64, f64) {
    let n = signal.len().next_power_of_two().max(64);
    let mut re = vec![0.0f64; n];
    let mut im = vec![0.0f64; n];
    re[..signal.len()].copy_from_slice(signal);
    // Hann window so leakage doesn't kill the peak-to-mean ratio.
    for i in 0..signal.len() {
        let w = 0.5 - 0.5 * ((2.0 * std::f64::consts::PI * i as f64) / (signal.len() as f64 - 1.0)).cos();
        re[i] *= w;
    }
    fft_inplace(&mut re, &mut im);
    let half = n / 2;
    let bin_hz = sample_rate / n as f64;
    let min_bin = (min_hz / bin_hz).floor() as usize;
    let max_bin = ((max_hz / bin_hz).ceil() as usize).min(half - 1);
    if max_bin <= min_bin {
        return (0.0, 0.0, 0.0);
    }
    let mut peak_mag = 0.0;
    let mut peak_bin = min_bin;
    let mut band_sum = 0.0;
    let mut band_count = 0usize;
    for bin in min_bin..=max_bin {
        let m = (re[bin] * re[bin] + im[bin] * im[bin]).sqrt();
        band_sum += m;
        band_count += 1;
        if m > peak_mag {
            peak_mag = m;
            peak_bin = bin;
        }
    }
    let band_mean = if band_count > 0 { band_sum / band_count as f64 } else { 0.0 };
    // Parabolic interpolation for sub-bin frequency.
    let freq = if peak_bin > 0 && peak_bin + 1 < half {
        let am = (re[peak_bin - 1] * re[peak_bin - 1] + im[peak_bin - 1] * im[peak_bin - 1]).sqrt();
        let ap = (re[peak_bin + 1] * re[peak_bin + 1] + im[peak_bin + 1] * im[peak_bin + 1]).sqrt();
        let denom = am - 2.0 * peak_mag + ap;
        let shift = if denom.abs() > f64::EPSILON { 0.5 * (am - ap) / denom } else { 0.0 };
        (peak_bin as f64 + shift) * bin_hz
    } else {
        peak_bin as f64 * bin_hz
    };
    (freq, peak_mag, band_mean)
}

fn fft_inplace(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    debug_assert!(n.is_power_of_two() && im.len() == n);
    // Bit-reverse permutation
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    // Cooley–Tukey
    let mut size = 2usize;
    while size <= n {
        let half = size / 2;
        let table_step = std::f64::consts::PI / half as f64;
        for chunk in (0..n).step_by(size) {
            for k in 0..half {
                let angle = -table_step * k as f64;
                let (wr, wi) = (angle.cos(), angle.sin());
                let tr = re[chunk + k + half] * wr - im[chunk + k + half] * wi;
                let ti = re[chunk + k + half] * wi + im[chunk + k + half] * wr;
                re[chunk + k + half] = re[chunk + k] - tr;
                im[chunk + k + half] = im[chunk + k] - ti;
                re[chunk + k] += tr;
                im[chunk + k] += ti;
            }
        }
        size *= 2;
    }
}

fn ingest_distance(cm: u32) {
    let now = Instant::now();
    *latest().lock().unwrap() = Some(MmwaveReading {
        distance_cm: cm,
        at: now,
    });
    // Breathing-only update on ASCII fallback (no per-gate data).
    let cm_f = cm as f64;
    let vs = br_detector().lock().unwrap().process_frame(&[cm_f], &[]);
    *vitals_slot().lock().unwrap() = Some(vs);
}

/// Parse `distance:<digits>` (HLK-LD2402 Normal Mode line format).
pub fn parse_distance(line: &str) -> Option<u32> {
    let lower = line.trim().to_ascii_lowercase();
    let rest = lower.strip_prefix("distance:")?;
    rest.parse::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_distance_lines() {
        assert_eq!(parse_distance("distance:228"), Some(228));
        assert_eq!(parse_distance("Distance:0"), Some(0));
        assert_eq!(parse_distance(" distance:42 "), Some(42));
        assert_eq!(parse_distance("OFF"), None);
        assert_eq!(parse_distance(""), None);
        assert_eq!(parse_distance("distance:abc"), None);
    }

    #[test]
    fn engineering_frame_round_trip() {
        // Build a synthetic engineering frame and feed it through the
        // payload parser. Expected: distance + per-gate energies stored.
        let mut payload = Vec::new();
        payload.push(FRAME_TYPE_ENGINEERING);
        payload.extend_from_slice(&151u16.to_le_bytes());
        payload.extend_from_slice(&[0; 8]);
        for i in 0..GATE_COUNT {
            payload.extend_from_slice(&((1000 + i as u32) * 10).to_le_bytes());
        }
        for i in 0..GATE_COUNT {
            payload.extend_from_slice(&((500 + i as u32) * 5).to_le_bytes());
        }
        assert_eq!(payload.len(), ENG_PAYLOAD_LEN as usize);
        parse_engineering_payload(&payload);
        let snap = gates_slot().lock().unwrap().clone().expect("snap recorded");
        assert_eq!(snap.motion[0], 10000);
        assert_eq!(snap.micro[14], (500 + 14) * 5);
        let dist = latest().lock().unwrap().expect("dist recorded");
        assert_eq!(dist.distance_cm, 151);
    }

    #[test]
    fn build_cmd_layout() {
        // Enable-config command per probe:
        // FD FC FB FA 04 00 FF 00 01 00 04 03 02 01
        let bytes = build_cmd(0x00FF, &[0x01, 0x00]);
        assert_eq!(
            bytes,
            vec![
                0xFD, 0xFC, 0xFB, 0xFA, 0x04, 0x00, 0xFF, 0x00, 0x01, 0x00, 0x04, 0x03, 0x02, 0x01
            ]
        );
    }
}
