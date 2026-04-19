//! Serial CSI reader — parse ESP32 CSI data from /dev/ttyACM0 and /dev/ttyUSB0.
//!
//! ESP32 firmware outputs lines like:
//!   I (56994) csi_collector: CSI cb #2900: len=256 rssi=-32 ch=5
//!
//! This module reads those lines, extracts RSSI, and tracks signal changes
//! to detect motion and presence.

use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct CsiReading {
    pub port: String,
    pub rssi: i32,
    pub len: u32,
    pub channel: u8,
    pub callback_num: u64,
    pub timestamp_ms: i64,
}

#[derive(Clone, Debug)]
pub struct CsiState {
    /// Latest readings from each port
    pub readings: Vec<CsiReading>,
    /// RSSI history for motion detection (last 20 values per port)
    pub rssi_history: Vec<Vec<i32>>,
    /// Motion score (0.0 = still, 1.0 = strong motion)
    pub motion_score: f32,
    /// Estimated presence distance (from RSSI)
    pub presence_distance_m: f32,
    /// Total frames received
    pub total_frames: u64,
}

impl Default for CsiState {
    fn default() -> Self {
        Self {
            readings: Vec::new(),
            rssi_history: Vec::new(),
            motion_score: 0.0,
            presence_distance_m: 3.0,
            total_frames: 0,
        }
    }
}

/// Start reading CSI from serial ports in background threads.
/// Returns shared state that updates as frames arrive.
pub fn start_serial_readers(ports: &[&str]) -> Arc<Mutex<CsiState>> {
    let state = Arc::new(Mutex::new(CsiState::default()));

    for (idx, port) in ports.iter().enumerate() {
        let port_path = port.to_string();
        let st = state.clone();

        std::thread::spawn(move || {
            loop {
                if let Ok(file) = std::fs::File::open(&port_path) {
                    let reader = BufReader::new(file);
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            if let Some(reading) = parse_csi_line(&line, &port_path) {
                                update_state(&st, idx, reading);
                            }
                        }
                    }
                }
                // Retry if port disconnects
                std::thread::sleep(std::time::Duration::from_secs(2));
                eprintln!("  CSI: reconnecting {port_path}...");
            }
        });

        eprintln!("  CSI: reading {port}");
    }

    state
}

fn parse_csi_line(line: &str, port: &str) -> Option<CsiReading> {
    // Parse: I (56994) csi_collector: CSI cb #2900: len=256 rssi=-32 ch=5
    if !line.contains("csi_collector") || !line.contains("CSI cb") {
        return None;
    }

    let rssi = line.split("rssi=").nth(1)?
        .split_whitespace().next()?
        .parse::<i32>().ok()?;

    let len = line.split("len=").nth(1)?
        .split_whitespace().next()?
        .parse::<u32>().ok()?;

    let channel = line.split("ch=").nth(1)?
        .split_whitespace().next()
        .unwrap_or("0")
        .parse::<u8>().unwrap_or(0);

    let cb_num = line.split('#').nth(1)?
        .split(':').next()?
        .parse::<u64>().ok()?;

    Some(CsiReading {
        port: port.to_string(),
        rssi,
        len,
        channel,
        callback_num: cb_num,
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
    })
}

fn update_state(state: &Arc<Mutex<CsiState>>, port_idx: usize, reading: CsiReading) {
    let mut st = state.lock().unwrap();

    // Ensure vectors are big enough
    while st.readings.len() <= port_idx {
        st.readings.push(reading.clone());
        st.rssi_history.push(Vec::new());
    }

    st.readings[port_idx] = reading.clone();
    st.total_frames += 1;

    // Track RSSI history
    let hist = &mut st.rssi_history[port_idx];
    hist.push(reading.rssi);
    if hist.len() > 20 { hist.remove(0); }

    // Motion detection: RSSI variance over last 20 readings
    if hist.len() >= 5 {
        let mean: f32 = hist.iter().map(|&r| r as f32).sum::<f32>() / hist.len() as f32;
        let variance: f32 = hist.iter().map(|&r| (r as f32 - mean).powi(2)).sum::<f32>() / hist.len() as f32;
        // High variance = motion (someone moving changes signal reflections)
        st.motion_score = (variance / 50.0).min(1.0);  // normalize: variance of 50 = full motion
    }

    // Estimate presence distance from RSSI (path loss model)
    // Free space: RSSI = -10 * n * log10(d) + A
    // n ≈ 2.5 for indoor, A ≈ -30 (1m reference)
    let avg_rssi: f32 = st.readings.iter().map(|r| r.rssi as f32).sum::<f32>()
        / st.readings.len().max(1) as f32;
    let d = 10.0f32.powf((-30.0 - avg_rssi) / (10.0 * 2.5));
    st.presence_distance_m = d.clamp(0.3, 10.0);
}

/// Convert CSI state to occupancy influence on the point cloud.
/// Returns (motion_score, presence_distance, total_frames).
pub fn get_csi_influence(state: &Arc<Mutex<CsiState>>) -> (f32, f32, u64) {
    let st = state.lock().unwrap();
    (st.motion_score, st.presence_distance_m, st.total_frames)
}
