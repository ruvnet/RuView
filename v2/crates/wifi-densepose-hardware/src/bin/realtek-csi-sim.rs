//! Deterministic Realtek RTL8721Dx (AmebaDplus) 1x1 CSI simulator (ADR-323).

use clap::Parser;
use std::{
    fs::File,
    io::{self, Write},
    net::{SocketAddr, UdpSocket},
    path::PathBuf,
    thread,
    time::Duration,
};
use wifi_densepose_hardware::realtek_csi::{
    simulator::{RealtekCsiSimulator, SimulatorConfig},
    CsiFrame, CsiMode,
};

#[derive(Debug, Parser)]
#[command(
    name = "realtek-csi-sim",
    about = "Emit synthetic ADR-323 Realtek RTL8721Dx CSI frames"
)]
struct Args {
    #[arg(long, default_value_t = 100)]
    frames: u32,
    #[arg(long, default_value = "0x5241433152544c31", value_parser=parse_u64)]
    seed: u64,
    #[arg(long, default_value_t = 6)]
    channel: u8,
    #[arg(long)]
    bandwidth_40mhz: bool,
    #[arg(long, default_value_t = 52)]
    subcarriers: u16,
    #[arg(long, default_value_t = 16)]
    tone_bits: u8,
    #[arg(long, default_value_t = 1)]
    decimation: u8,
    #[arg(long, default_value_t = 64)]
    interval_ms: u64,
    #[arg(long)]
    udp: Option<SocketAddr>,
    /// Replay: little-endian u32 length followed by one ADR-323 envelope.
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    realtime: bool,
}

fn parse_u64(v: &str) -> Result<u64, String> {
    if let Some(h) = v.strip_prefix("0x").or_else(|| v.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).map_err(|e| e.to_string())
    } else {
        v.parse()
            .map_err(|e: std::num::ParseIntError| e.to_string())
    }
}

fn emit(
    frame: CsiFrame,
    socket: Option<&UdpSocket>,
    destination: Option<SocketAddr>,
    output: &mut Option<File>,
) -> Result<usize, Box<dyn std::error::Error>> {
    let wire = frame.to_bytes()?;
    if let (Some(s), Some(d)) = (socket, destination) {
        if s.send_to(&wire, d)? != wire.len() {
            return Err(io::Error::new(io::ErrorKind::WriteZero, "partial UDP datagram").into());
        }
    }
    if let Some(f) = output {
        f.write_all(&(wire.len() as u32).to_le_bytes())?;
        f.write_all(&wire)?;
    }
    Ok(wire.len())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = Args::parse();
    if a.udp.is_none() && a.output.is_none() {
        return Err("select at least one sink with --udp or --output".into());
    }
    let cfg = SimulatorConfig {
        seed: a.seed,
        csi_mode: CsiMode::Passive,
        channel: a.channel,
        bandwidth: if a.bandwidth_40mhz { 1 } else { 0 },
        num_sub_carrier: a.subcarriers,
        num_bit_per_tone: a.tone_bits,
        decimation: a.decimation,
        frame_period_us: (a.interval_ms * 1000) as u32,
        ..Default::default()
    };
    let mut sim = RealtekCsiSimulator::new(cfg)?;
    let socket = a.udp.map(|_| UdpSocket::bind("0.0.0.0:0")).transpose()?;
    let mut output = a.output.as_ref().map(File::create).transpose()?;
    let mut bytes = 0usize;
    for _ in 0..a.frames {
        bytes += emit(sim.next_frame(), socket.as_ref(), a.udp, &mut output)?;
        if a.realtime {
            thread::sleep(Duration::from_millis(a.interval_ms));
        }
    }
    eprintln!(
        "emitted {} synthetic Realtek RTL8721Dx CSI frames ({} bytes, channel={}, subcarriers={}, tone_bits={}, seed={:#x})",
        a.frames, bytes, a.channel, a.subcarriers, a.tone_bits, a.seed
    );
    Ok(())
}
