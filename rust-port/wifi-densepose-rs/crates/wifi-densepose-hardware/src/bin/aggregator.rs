//! UDP aggregator CLI for receiving ESP32 CSI frames (ADR-018).
//!
//! Listens for ADR-018 binary CSI frames on a UDP socket, parses each
//! packet, and prints a one-line summary to stdout.
//!
//! Usage:
//!   cargo run -p wifi-densepose-hardware --bin aggregator -- --bind 0.0.0.0:5005

use std::net::UdpSocket;
use std::process;

use clap::Parser;
use wifi_densepose_hardware::Esp32CsiParser;

/// UDP aggregator for ESP32 CSI nodes (ADR-018).
#[derive(Parser)]
#[command(name = "aggregator", about = "Receive and display live CSI frames from ESP32 nodes")]
struct Cli {
    /// Address:port to bind the UDP listener to.
    #[arg(long, default_value = "0.0.0.0:5005")]
    bind: String,

    /// Print raw hex dump alongside parsed output.
    #[arg(long, short)]
    verbose: bool,
}

fn main() {
    let cli = Cli::parse();

    let socket = match UdpSocket::bind(&cli.bind) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: cannot bind to {}: {}", cli.bind, e);
            process::exit(1);
        }
    };

    eprintln!("Listening on {}...", cli.bind);

    let mut buf = [0u8; 2048];

    loop {
        let (n, src) = match socket.recv_from(&mut buf) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("recv error: {}", e);
                continue;
            }
        };

        if cli.verbose {
            eprintln!("  [{} bytes from {}]", n, src);
        }

        match Esp32CsiParser::parse_frame(&buf[..n]) {
            Ok((frame, _consumed)) => {
                let mean_amp = frame.mean_amplitude();
                println!(
                    "[node:{} seq:{}] sc={} rssi={} amp={:.1}",
                    frame.metadata.node_id,
                    frame.metadata.sequence,
                    frame.subcarrier_count(),
                    frame.metadata.rssi_dbm,
                    mean_amp,
                );
            }
            Err(e) => {
                if cli.verbose {
                    eprintln!("  parse error: {}", e);
                }
            }
        }
    }
}
