//! `wifi-densepose-mtk-bridge` — MediaTek MT7981 vendor CSI → ADR-267 MTC1.
//!
//! Live:    `wifi-densepose-mtk-bridge --listen 0.0.0.0:8888 --csidump <router-ip>:8888 --listen-allow <router-ip> --node lr`
//! Dump:    `wifi-densepose-mtk-bridge --replay csi.json --captured-on <model>/<firmware> --node lr`
//! Replay:  `wifi-densepose-mtk-bridge --replay session.mtkcap --captured-on <model>/<firmware> --node lr`

use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::net::UdpSocket;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use wifi_densepose_hardware::mediatek_csi::ChipsetProfile;

use wifi_densepose_mtk_bridge::assemble::{AssemblerConfig, FrameAssembler, GroupBy};
use wifi_densepose_mtk_bridge::capture::{read_capture, CaptureHeader, CaptureWriter};
use wifi_densepose_mtk_bridge::dump_file::parse_dump_with_stats;
use wifi_densepose_mtk_bridge::listen_allow::{SourceAllowlist, SourceFilter};
use wifi_densepose_mtk_bridge::provenance::{
    authorize_replay, merge_capture_header, resolve_path_provenance, ReplayAuthorization,
};
use wifi_densepose_mtk_bridge::record::{device_id_from_node, MtkRecord};
use wifi_densepose_mtk_bridge::sink::{FrameSink, UdpFrameSink, VecFrameSink};
use wifi_densepose_mtk_bridge::udp_in::{decode_datagram, REGISTER_PAYLOAD};
use wifi_densepose_mtk_bridge::BridgeError;

#[derive(Parser, Debug)]
#[command(
    name = "wifi-densepose-mtk-bridge",
    about = "Bridge MediaTek MT7981 (mt76) vendor CSI into ADR-267 MTC1 frames"
)]
struct Args {
    /// Bind here for MtkCSIdump datagrams. Mutually exclusive with --replay.
    #[arg(long)]
    listen: Option<String>,

    /// Register with a running `CSIdump` server at this address so it starts
    /// sending to us (it only forwards to clients that sent "register").
    #[arg(long)]
    csidump: Option<String>,

    /// Source networks allowed to feed --listen, as IP or IP/prefix, repeatable
    /// and comma-separated. Required with --listen: without it the bridge
    /// refuses to listen rather than trusting whatever reaches the socket.
    /// Loopback is not implicit; name 127.0.0.1 to accept a local sender.
    #[arg(long = "listen-allow", value_name = "CIDR")]
    listen_allow: Vec<String>,

    /// Replay an mt76-vendor JSON dump or a .mtkcap capture instead of listening.
    #[arg(long)]
    replay: Option<PathBuf>,

    /// Write every decoded record to this capture file for later --replay.
    #[arg(long)]
    record: Option<PathBuf>,

    /// Sensing server MTC1 ingest.
    #[arg(long, default_value = "127.0.0.1:5005")]
    sink: String,

    /// Node name; hashed into a stable device id. Two receivers must differ.
    #[arg(long, default_value = "mtk-node")]
    node: String,

    /// Explicit 64-bit device id in hex, overriding --node.
    #[arg(long)]
    device_id: Option<String>,

    /// Centre frequency of the monitored channel.
    #[arg(long, default_value_t = 5_210_000)]
    center_freq_khz: u32,

    /// Fallback transmit-chain count for a dump replay whose `chain_info` never
    /// sets the last-chain bit. Normally unused: dimensions come from the chains
    /// each run actually contains.
    #[arg(long, default_value_t = 2)]
    tx_chains: u8,

    /// Fallback receive-chain count, as `--tx-chains`. Ignored on the UDP path,
    /// which emits one 1x1 frame per datagram.
    #[arg(long, default_value_t = 2)]
    rx_chains: u8,

    /// Assert that this host's clock is disciplined against the other
    /// receivers. Sets TIME_SYNCHRONIZED. Off unless you really have it.
    #[arg(long)]
    time_sync: bool,

    /// Declare the input fabricated. Flags every emitted frame SYNTHETIC, so the
    /// sensing server labels it `mediatek:simulated`.
    #[arg(long)]
    synthetic: bool,

    /// Attest that a replayed file came off real hardware, as
    /// `<model>/<firmware>` (e.g. `WN586X3/OpenWrt-24.10.8`). Required for
    /// --replay unless --synthetic is given, and refused for an input that
    /// declares itself synthetic.
    #[arg(long, value_name = "MODEL/FIRMWARE")]
    captured_on: Option<String>,

    /// Pace replay at this many frames per second. 0 sends as fast as possible.
    #[arg(long, default_value_t = 20.0)]
    replay_hz: f64,

    /// Stop after this many frames. 0 means run until interrupted.
    #[arg(long, default_value_t = 0)]
    max_frames: u64,

    /// Decode and assemble but send nothing.
    #[arg(long)]
    dry_run: bool,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("wifi-densepose-mtk-bridge: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), BridgeError> {
    let args = Args::parse();
    let device_id = match &args.device_id {
        Some(hex) => u64::from_str_radix(hex.trim_start_matches("0x"), 16).map_err(|_| {
            BridgeError::UnknownCaptureFormat(format!("--device-id {hex} is not hex"))
        })?,
        None => device_id_from_node(&args.node),
    };

    let mut sink: Box<dyn FrameSink> = if args.dry_run {
        Box::new(VecFrameSink::default())
    } else {
        Box::new(UdpFrameSink::connect(&args.sink)?)
    };

    eprintln!(
        "wifi-densepose-mtk-bridge: node={} device_id={:016x} sink={} chipset={}",
        args.node,
        device_id,
        if args.dry_run {
            "(dry-run)"
        } else {
            &args.sink
        },
        ChipsetProfile::Mt7981Mt7976.name()
    );

    match (&args.replay, &args.listen) {
        (Some(path), _) => replay(&args, device_id, path, sink.as_mut()),
        (None, Some(bind)) => listen(&args, device_id, bind, sink.as_mut()),
        (None, None) => Err(BridgeError::UnknownCaptureFormat(
            "need --listen <addr> or --replay <file>".to_string(),
        )),
    }
}

/// Load a replay source. An mt76-vendor dump starts with `[`; one of our
/// captures starts with the header object's `{`.
fn load_records(path: &PathBuf) -> Result<(String, bool, Vec<MtkRecord>), BridgeError> {
    let mut text = String::new();
    File::open(path)
        .map_err(BridgeError::Io)?
        .read_to_string(&mut text)
        .map_err(BridgeError::Io)?;
    match text.trim_start().chars().next() {
        Some('{') => {
            let (header, records) = read_capture(BufReader::new(text.as_bytes()))?;
            Ok((
                format!("capture/{}", header.transport),
                header.synthetic,
                records,
            ))
        }
        Some('[') => {
            let (records, stats) = parse_dump_with_stats(&text)?;
            if stats.arrays > 1 {
                eprintln!(
                    "note: {} top-level arrays — mt76-vendor appends with fopen(\"a+\"), so \
                     this file holds more than one dump",
                    stats.arrays
                );
            }
            if stats.duplicate_chain_records > 0 {
                eprintln!(
                    "WARNING: {} records repeat a (ta, ts, tx_idx, rx_idx) chain already seen. \
                     A capture loop that does not remove the dump file between batches replays \
                     every earlier dump, multiplying records downstream.",
                    stats.duplicate_chain_records
                );
            }
            Ok(("mt76-vendor dump".to_string(), false, records))
        }
        _ => Err(BridgeError::UnknownCaptureFormat(
            path.display().to_string(),
        )),
    }
}

fn replay(
    args: &Args,
    device_id: u64,
    path: &PathBuf,
    sink: &mut dyn FrameSink,
) -> Result<(), BridgeError> {
    // Decide provenance BEFORE any frame is built. A file cannot prove it came
    // off silicon, so the operator must say which it is.
    let declared = resolve_path_provenance(path)?;
    let (kind, header_synthetic, records) = load_records(path)?;
    let declared = merge_capture_header(declared, header_synthetic);
    let authorization = authorize_replay(&declared, args.synthetic, args.captured_on.as_deref())?;

    let synthetic = match &authorization {
        ReplayAuthorization::Synthetic { reason } => {
            eprintln!("PROVENANCE: SYNTHETIC — frames flagged SYNTHETIC ({reason})");
            true
        }
        ReplayAuthorization::Attested { captured_on } => {
            eprintln!(
                "PROVENANCE: PHYSICAL-UNVALIDATED — operator attests captured_on={captured_on}"
            );
            false
        }
    };
    if let Some(generator) = &declared.generator {
        eprintln!("PROVENANCE: input generator: {generator}");
    }

    // A capture taken off the UDP transport has no transmit index, so it has to
    // be regrouped the same way the live UDP path grouped it. Anything else
    // carries tx_idx and uses the packet rectangle.
    let group_by = if kind == "capture/udp" {
        // A UDP capture has no packet identity, exactly as the live stream did.
        GroupBy::PerRecord
    } else {
        // Dump records carry chain_info, so the driver's own last-chain marker
        // decides where a PPDU ends. --tx-chains/--rx-chains are only the
        // fallback for an input whose chain_info never sets BIT(15).
        GroupBy::LastChainMarker {
            fallback_tx: args.tx_chains,
            fallback_rx: args.rx_chains,
        }
    };
    let mut assembler = FrameAssembler::new(AssemblerConfig {
        device_id,
        chipset: ChipsetProfile::Mt7981Mt7976,
        center_freq_khz: args.center_freq_khz,
        time_synchronized: args.time_sync,
        synthetic,
        group_by,
    });
    eprintln!(
        "replaying {} records from {} ({kind})",
        records.len(),
        path.display()
    );

    let mut recorder = open_recorder(args, "replay", synthetic)?;
    let period = frame_period(args.replay_hz);
    let mut sent = 0u64;
    let mut logged_scalars = false;
    for record in records {
        if !logged_scalars {
            // Print the vendor scalars of the first record so a bring-up run
            // shows what the radio actually reported.
            logged_scalars = true;
            eprintln!("first record: {}", record.describe());
        }
        if let Some(w) = recorder.as_mut() {
            w.write_record(&record)?;
        }
        if let Some(frame) = assembler.push(record)? {
            sink.send(&frame)?;
            sent += 1;
            if args.max_frames != 0 && sent >= args.max_frames {
                break;
            }
            if let Some(p) = period {
                std::thread::sleep(p);
            }
        }
    }
    assembler.flush();
    if let Some(w) = recorder.as_mut() {
        w.flush()?;
    }
    report(&assembler, sent);
    Ok(())
}

fn listen(
    args: &Args,
    device_id: u64,
    bind: &str,
    sink: &mut dyn FrameSink,
) -> Result<(), BridgeError> {
    // Refuse before binding: an unnamed source must never reach the decoder,
    // because the live path is the only route to a physical-unvalidated label.
    let allow = SourceAllowlist::parse(&args.listen_allow)?;
    let mut filter = SourceFilter::new(allow);

    let socket = UdpSocket::bind(bind).map_err(BridgeError::Io)?;
    if let Some(server) = &args.csidump {
        // `motion_detector.cpp:73` only forwards to clients that registered.
        socket
            .send_to(REGISTER_PAYLOAD, server)
            .map_err(BridgeError::Io)?;
        eprintln!("registered with CSIdump server at {server}");
    }
    eprintln!(
        "listening for MtkCSIdump datagrams on {bind}, sources limited to {}",
        args.listen_allow.join(",")
    );

    // The UDP datagram has no transmit index, so chains are windowed per packet.
    let mut assembler = FrameAssembler::new(AssemblerConfig {
        device_id,
        chipset: ChipsetProfile::Mt7981Mt7976,
        center_freq_khz: args.center_freq_khz,
        time_synchronized: args.time_sync,
        // The live radio path is physical by construction. `--synthetic` is
        // still honoured so a fabricated sender can be exercised honestly.
        synthetic: args.synthetic,
        // One frame per datagram: the transport carries no packet identity and
        // CSIdump sends antenna-major, so chains cannot be correlated. See
        // `GroupBy::PerRecord`.
        group_by: GroupBy::PerRecord,
    });
    let mut recorder = open_recorder(args, "udp", args.synthetic)?;
    let mut buf = vec![0u8; 65_535];
    let mut sent = 0u64;
    let mut send_errors = 0u64;
    let mut announced = false;
    loop {
        let (n, from) = socket.recv_from(&mut buf).map_err(BridgeError::Io)?;
        if !filter.admit(from.ip()) {
            if filter.is_first_rejection() {
                eprintln!(
                    "rejected_source: {} is outside --listen-allow; dropping (further \
                     rejections counted silently)",
                    from.ip()
                );
            }
            continue;
        }
        let (_, record) = match decode_datagram(&buf[..n]) {
            Ok(v) => v,
            Err(err) => {
                eprintln!("dropping datagram: {err}");
                continue;
            }
        };
        // Announce on the first record that both passed the source filter and
        // decoded, so a rejected or malformed datagram never claims provenance.
        if !announced {
            announced = true;
            if args.synthetic {
                eprintln!(
                    "SYNTHETIC: source {}, device {device_id:016x} — frames flagged SYNTHETIC",
                    from.ip()
                );
            } else {
                eprintln!(
                    "PHYSICAL-UNVALIDATED: source {}, device {device_id:016x}",
                    from.ip()
                );
            }
            eprintln!("first record: {}", record.describe());
        }
        if let Some(w) = recorder.as_mut() {
            w.write_record(&record)?;
            w.flush()?;
        }
        match assembler.push(record) {
            Ok(Some(frame)) => {
                // A send failure must not end a live capture. The sensing server
                // may not be up yet, or may be restarting; ICMP port-unreachable
                // surfaces here as an error on a connected UDP socket. Count it,
                // say so once, and keep receiving — losing the rest of a hardware
                // session because the sink blinked is far worse.
                match sink.send(&frame) {
                    Ok(_) => sent += 1,
                    Err(err) => {
                        send_errors += 1;
                        if send_errors == 1 {
                            eprintln!(
                                "sink send failed ({err}); continuing to receive, further \
                                 failures counted silently"
                            );
                        }
                    }
                }
                if args.max_frames != 0 && sent >= args.max_frames {
                    break;
                }
            }
            Ok(None) => {}
            Err(err) => eprintln!("dropping record: {err}"),
        }
    }
    report(&assembler, sent);
    eprintln!(
        "rejected_source={} send_errors={}",
        filter.rejected_source(),
        send_errors
    );
    Ok(())
}

fn open_recorder(
    args: &Args,
    transport: &str,
    synthetic: bool,
) -> Result<Option<CaptureWriter<File>>, BridgeError> {
    match &args.record {
        Some(path) => {
            let file = File::create(path).map_err(BridgeError::Io)?;
            eprintln!("recording to {} (synthetic={synthetic})", path.display());
            // The header carries the provenance forward, so replaying this
            // capture later needs the same --synthetic decision.
            Ok(Some(CaptureWriter::new(
                file,
                CaptureHeader::new(transport, &args.node, synthetic),
            )))
        }
        None => Ok(None),
    }
}

fn frame_period(hz: f64) -> Option<Duration> {
    if hz > 0.0 {
        Some(Duration::from_secs_f64(1.0 / hz))
    } else {
        None
    }
}

fn report(assembler: &FrameAssembler, sent: u64) {
    let s = assembler.stats();
    let mut out = std::io::stderr();
    let _ = writeln!(
        out,
        "records_in={} frames_out={} sent={} incomplete_groups={} ragged_runs={} \
         pkt_sn_gaps={}",
        s.records_in,
        s.frames_out,
        sent,
        s.groups_dropped_incomplete,
        s.runs_dropped_ragged,
        s.pkt_sn_gaps
    );
}
