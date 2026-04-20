//! WiFi CSI receiver — ingests CSI frames from ESP32 nodes.
//!
//! ESP32 nodes send CSI data via UDP. This module receives the frames,
//! runs RF tomography, and produces OccupancyVolume for fusion.
//!
//! Protocol:
//!   ESP32 → serial → host (ruvzen) → UDP broadcast → this receiver
//!   Each packet: JSON with {mac, rssi, csi_data: [i8], timestamp_ms}

use crate::fusion::OccupancyVolume;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

/// Raw CSI frame from an ESP32 node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsiFrame {
    pub mac: String,
    pub rssi: i8,
    pub timestamp_ms: i64,
    pub channel: u8,
    pub bandwidth: u8,
    /// CSI subcarrier amplitudes (typically 52-114 values)
    pub csi_data: Vec<i8>,
    /// Optional: secondary stream (imaginary part)
    #[serde(default)]
    pub csi_imag: Vec<i8>,
}

/// CSI link — a pair of TX/RX nodes with accumulated frames.
#[derive(Debug)]
pub struct CsiLink {
    pub tx_mac: String,
    pub rx_mac: String,
    pub frames: VecDeque<CsiFrame>,
    pub attenuation: f64,  // current estimated attenuation
}

/// CSI receiver — listens on UDP and accumulates frames.
pub struct CsiReceiver {
    pub links: Arc<Mutex<Vec<CsiLink>>>,
    pub frame_count: Arc<Mutex<u64>>,
    bind_addr: String,
}

impl CsiReceiver {
    pub fn new(bind_addr: &str) -> Self {
        Self {
            links: Arc::new(Mutex::new(Vec::new())),
            frame_count: Arc::new(Mutex::new(0)),
            bind_addr: bind_addr.to_string(),
        }
    }

    /// Start receiving CSI frames in a background thread.
    pub fn start(&self) -> Result<()> {
        let socket = UdpSocket::bind(&self.bind_addr)?;
        socket.set_read_timeout(Some(std::time::Duration::from_secs(1)))?;
        eprintln!("  CSI receiver listening on {}", self.bind_addr);

        let links = self.links.clone();
        let count = self.frame_count.clone();

        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match socket.recv_from(&mut buf) {
                    Ok((n, _addr)) => {
                        if let Ok(frame) = serde_json::from_slice::<CsiFrame>(&buf[..n]) {
                            process_frame(&links, &count, frame);
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                    Err(_) => continue,
                }
            }
        });

        Ok(())
    }

    /// Get the current occupancy volume from accumulated CSI data.
    pub fn get_occupancy(&self) -> OccupancyVolume {
        let links = self.links.lock().unwrap();

        if links.is_empty() {
            return crate::fusion::demo_occupancy();
        }

        // Extract per-link attenuations for tomography
        let attenuations: Vec<f64> = links.iter().map(|l| l.attenuation).collect();
        let _n_links = attenuations.len();

        // Simple grid-based tomography (ISTA solver would go here)
        let nx = 8;
        let ny = 8;
        let nz = 4;
        let total = nx * ny * nz;
        let mut densities = vec![0.0f64; total];

        // For each link, distribute attenuation along the line between TX and RX
        // This is a simplified backprojection — real tomography uses ISTA L1 solver
        for (_i, atten) in attenuations.iter().enumerate() {
            // Distribute attenuation uniformly across voxels
            // (in production, use link geometry for proper ray tracing)
            let contribution = atten / total as f64;
            for d in &mut densities {
                *d += contribution;
            }
        }

        // Normalize
        let max = densities.iter().cloned().fold(0.0f64, f64::max);
        if max > 0.0 {
            for d in &mut densities { *d /= max; }
        }

        let occupied_count = densities.iter().filter(|&&d| d > 0.3).count();

        OccupancyVolume {
            densities,
            nx, ny, nz,
            bounds: [0.0, 0.0, 0.0, 5.0, 5.0, 3.0],
            occupied_count,
        }
    }

    pub fn frame_count(&self) -> u64 {
        *self.frame_count.lock().unwrap()
    }
}

fn process_frame(
    links: &Arc<Mutex<Vec<CsiLink>>>,
    count: &Arc<Mutex<u64>>,
    frame: CsiFrame,
) {
    // Calculate attenuation from RSSI + CSI amplitude
    let csi_power: f64 = frame.csi_data.iter()
        .map(|&v| (v as f64).powi(2))
        .sum::<f64>() / frame.csi_data.len().max(1) as f64;
    let attenuation = -(frame.rssi as f64) + csi_power.sqrt() * 0.1;

    let mut links = links.lock().unwrap();

    // Find or create link for this MAC
    let link = links.iter_mut().find(|l| l.tx_mac == frame.mac);
    if let Some(link) = link {
        link.attenuation = link.attenuation * 0.9 + attenuation * 0.1; // EMA
        link.frames.push_back(frame);
        if link.frames.len() > 100 { link.frames.pop_front(); }
    } else {
        let mut frames = VecDeque::new();
        frames.push_back(frame.clone());
        links.push(CsiLink {
            tx_mac: frame.mac,
            rx_mac: "receiver".to_string(),
            frames,
            attenuation,
        });
    }

    *count.lock().unwrap() += 1;
}

/// Send CSI frames via UDP (for testing — simulates ESP32 nodes).
pub fn send_test_frames(target: &str, count: usize) -> Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;

    for i in 0..count {
        let frame = CsiFrame {
            mac: format!("AA:BB:CC:DD:EE:{:02X}", i % 4),
            rssi: -40 - (i % 30) as i8,
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            channel: 6,
            bandwidth: 20,
            csi_data: (0..56).map(|j| ((i + j) % 128) as i8 - 64).collect(),
            csi_imag: Vec::new(),
        };

        let json = serde_json::to_vec(&frame)?;
        socket.send_to(&json, target)?;
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    Ok(())
}
