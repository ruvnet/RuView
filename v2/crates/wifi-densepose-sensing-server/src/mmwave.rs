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

/// Latest mmWave reading + when it landed.
#[derive(Debug, Clone, Copy)]
pub struct MmwaveReading {
    pub distance_cm: u32,
    pub at: Instant,
}

static LATEST: OnceLock<Mutex<Option<MmwaveReading>>> = OnceLock::new();

fn latest() -> &'static Mutex<Option<MmwaveReading>> {
    LATEST.get_or_init(|| Mutex::new(None))
}

/// Returns the most recent reading if it landed within `staleness`.
pub fn current(staleness: Duration) -> Option<MmwaveReading> {
    let g = latest().lock().unwrap();
    let r = (*g)?;
    if r.at.elapsed() <= staleness { Some(r) } else { None }
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
