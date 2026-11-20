//! Bridge MediaTek MT7981 (`mt76`) vendor CSI into RuView's ADR-267 MTC1 frames.
//!
//! Two inbound transports, both from real silicon:
//!
//! * [`udp_in`] — the MtkCSIdump UDP stream (`./CSIdump phy0-sta0 <rate> <port>`).
//! * [`dump_file`] — the `mt76-vendor dump csi <n> <file>` JSON capture.
//!
//! [`assemble`] folds the per-chain records either path yields into MTC1
//! [`CsiFrame`](wifi_densepose_hardware::mediatek_csi::CsiFrame)s, which
//! [`sink`] encodes and sends to the sensing server's UDP ingest.
//!
//! [`capture`] records decoded records so a hardware session can be replayed
//! later with no hardware present. [`listen_allow`] narrows the live path to
//! operator-named source networks.
//!
//! **Provenance.** Nothing in this crate ever sets `CALIBRATED`, and `SYNTHETIC`
//! is set only when the operator explicitly asks for it with `--synthetic` or
//! when the input declares itself synthetic. Frames off the live radio path are
//! labelled `mediatek:physical-unvalidated` by the sensing server.
//!
//! Because a file cannot prove it came from silicon, [`provenance`] makes
//! `--replay` refuse to run unless the operator either declares the input
//! synthetic or attests to the hardware it came off. See that module.

pub mod assemble;
pub mod capture;
pub mod dump_file;
pub mod listen_allow;
pub mod provenance;
pub mod record;
pub mod sink;
pub mod udp_in;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("datagram too short: needed {needed}, got {got}")]
    ShortDatagram { needed: usize, got: usize },
    #[error("datagram declares {declared} samples but carries {available}")]
    SampleCountMismatch { declared: usize, available: usize },
    #[error("antenna index {0} does not fit a chain index")]
    AntennaIndexOutOfRange(u32),
    #[error("sample {0} is not an exact integer")]
    NonIntegerSample(f64),
    #[error("sample {0} is outside i16 range")]
    SampleOutOfRange(f64),
    #[error("I has {i} samples but Q has {q}")]
    IqLengthMismatch { i: usize, q: usize },
    #[error("record carries no samples")]
    EmptyRecord,
    #[error("unknown bandwidth code {0}")]
    UnknownBandwidthCode(u8),
    #[error("{0} subcarriers matches no MT7981 bandwidth")]
    UnknownSubcarrierCount(usize),
    #[error("dump is not a JSON array")]
    DumpNotAnArray,
    #[error("dump record is not a JSON array")]
    DumpRecordNotAnArray,
    #[error("dump record has {got} fields, needs at least {needed}")]
    DumpRecordTooShort { needed: usize, got: usize },
    #[error("dump ta field is not a string")]
    DumpTaNotAString,
    #[error("dump ta {0:?} is not 12 hex digits")]
    DumpBadTa(String),
    #[error("dump field is not an integer")]
    DumpFieldNotAnInteger,
    #[error("dump field is not an array")]
    DumpFieldNotAnArray,
    #[error("dump contains no records")]
    EmptyDump,
    #[error("chains in one group report different subcarrier counts")]
    RaggedGroup,
    #[error("group is missing chain tx={tx} rx={rx}")]
    MissingChain { tx: u16, rx: u16 },
    #[error("record chain tx={tx} rx={rx} is outside the configured {tx_count}x{rx_count}")]
    ChainOutOfRange {
        tx: u16,
        rx: u16,
        tx_count: u8,
        rx_count: u8,
    },
    #[error(
        "input declares itself synthetic ({evidence}{}), so it can only be \
         replayed with --synthetic; an attestation cannot override that",
        match generator { Some(g) => format!(", generator: {g}"), None => String::new() }
    )]
    SyntheticInputNeedsSyntheticFlag {
        evidence: &'static str,
        generator: Option<String>,
    },
    #[error(
        "--replay needs a provenance decision: pass --synthetic for a fabricated \
         input, or --captured-on <model>/<firmware> to attest that it came off \
         real hardware"
    )]
    ReplayNeedsProvenance,
    #[error("--captured-on {0:?} must be <model>/<firmware>, both non-empty")]
    BadAttestation(String),
    #[error(
        "--listen needs --listen-allow <cidr>[,<cidr>]: the live path is the only \
         route to a physical-unvalidated label, so it will not accept datagrams \
         from unnamed sources. Use the router's subnet, e.g. \
         --listen-allow 192.168.1.0/24 (or 127.0.0.1 for a local sender)"
    )]
    ListenNeedsAllowlist,
    #[error("--listen-allow entry {0:?} is not an IP address or IP/prefix")]
    BadListenAllow(String),
    #[error("too many --listen-allow entries (limit {0})")]
    TooManyListenAllowEntries(usize),
    #[error("unknown capture format {0:?}")]
    UnknownCaptureFormat(String),
    #[error("unsupported capture version {0}")]
    UnsupportedCaptureVersion(u32),
    #[error("MTC1 encode rejected the frame: {0}")]
    Mtc1(wifi_densepose_hardware::mediatek_csi::CsiParseError),
    #[error("json: {0}")]
    Json(serde_json::Error),
    #[error("io: {0}")]
    Io(std::io::Error),
}
