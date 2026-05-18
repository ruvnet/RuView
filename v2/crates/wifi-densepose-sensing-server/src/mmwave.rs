//! ADR-121: HLK-LD2402 24 GHz mmWave radar reader.
//!
//! Auxiliary range/vitals modality, attached over a CP2102 USB-UART
//! bridge. The module ships factory firmware that emits ASCII
//! `distance:<cm>\r\n` lines @ 115200 baud, ~6 Hz, in Normal Mode.
//!
//! This reader runs in a dedicated thread (blocking serial I/O is
//! awkward inside tokio) and pushes the latest reading + monotonic
//! timestamp into a global `OnceLock<Mutex<…>>` that the broadcast
//! tick task reads.
//!
//! Cold-start tolerance: if the port cannot be opened, the thread
//! logs once and exits cleanly — the server keeps running with WiFi
//! sensing only. No panics, no retries (operator can hot-plug; if
//! they want auto-reconnect we can add it later).

use std::io::Read;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::vital_signs::{VitalSignDetector, VitalSigns};

/// HLK-LD2402 Normal-Mode cadence: factory firmware emits ~6 distance
/// lines/sec. We pick 6.0 Hz as the detector's nominal sample_rate so
/// the breathing band (0.1–0.5 Hz) sits comfortably inside Nyquist.
const MMWAVE_SAMPLE_RATE_HZ: f64 = 6.0;

/// Latest mmWave reading + when it landed.
#[derive(Debug, Clone, Copy)]
pub struct MmwaveReading {
    pub distance_cm: u32,
    pub at: Instant,
}

static LATEST: OnceLock<Mutex<Option<MmwaveReading>>> = OnceLock::new();
static VITALS: OnceLock<Mutex<Option<VitalSigns>>> = OnceLock::new();
static DETECTOR: OnceLock<Mutex<VitalSignDetector>> = OnceLock::new();

fn latest() -> &'static Mutex<Option<MmwaveReading>> {
    LATEST.get_or_init(|| Mutex::new(None))
}

fn vitals_slot() -> &'static Mutex<Option<VitalSigns>> {
    VITALS.get_or_init(|| Mutex::new(None))
}

fn detector_slot() -> &'static Mutex<VitalSignDetector> {
    DETECTOR.get_or_init(|| Mutex::new(VitalSignDetector::new(MMWAVE_SAMPLE_RATE_HZ)))
}

/// Returns the most recent reading if it landed within `staleness`.
pub fn current(staleness: Duration) -> Option<MmwaveReading> {
    let g = latest().lock().unwrap();
    let r = (*g)?;
    if r.at.elapsed() <= staleness { Some(r) } else { None }
}

/// Returns the latest mmWave-derived VitalSigns. Breathing is
/// computed from a 30-s buffer of distance samples (chest movement
/// modulates range by 5–10 mm — visible as flicker between adjacent
/// cm bins). Heart rate at 6 Hz / cm precision is essentially below
/// the noise floor; we surface it but expect very low confidence.
///
/// Returns `None` if no recent mmWave reading exists within
/// `staleness` (so the UI can show "—" when the radar is unplugged).
pub fn current_vitals(staleness: Duration) -> Option<VitalSigns> {
    // Gate on data freshness: if no recent distance reading, vitals
    // are stale — return None rather than the last cached estimate.
    current(staleness)?;
    vitals_slot().lock().unwrap().clone()
}

/// Buffer fill stats for UI ("12/180 samples").
pub fn buffer_status() -> (usize, usize) {
    let det = detector_slot().lock().unwrap();
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

    let mut buf = Vec::with_capacity(256);
    let mut tmp = [0u8; 128];
    loop {
        match serial.read(&mut tmp) {
            Ok(0) => continue,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::TimedOut { continue; }
                tracing::warn!("ADR-121 mmWave reader: read error: {e}");
                return;
            }
        }
        // Drain complete lines.
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let raw_line: Vec<u8> = buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&raw_line).trim().to_string();
            if let Some(cm) = parse_distance(&line) {
                *latest().lock().unwrap() = Some(MmwaveReading {
                    distance_cm: cm,
                    at: Instant::now(),
                });
                // Feed the same value into a VitalSignDetector tuned for
                // mmWave's 6 Hz cadence so we can publish a
                // breathing-rate estimate from chest-induced cm
                // flicker. Phase is empty (radar gives no phase here)
                // — extract_heartbeat falls back to amplitude residual
                // which is mostly noise at cm precision, but we
                // surface it anyway for transparency.
                let cm_f = cm as f64;
                let vs = detector_slot()
                    .lock()
                    .unwrap()
                    .process_frame(&[cm_f], &[]);
                *vitals_slot().lock().unwrap() = Some(vs);
            } else if !line.is_empty() {
                tracing::trace!("mmwave non-distance line: {line:?}");
            }
        }
        // Guard against runaway buffer if module emits non-newline garbage.
        if buf.len() > 1024 { buf.clear(); }
    }
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
}
