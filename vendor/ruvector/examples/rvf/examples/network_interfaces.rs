//! Network Interface Embeddings — Network OS Integration
//!
//! Category: **Vertical Domain / Network Operations**
//!
//! Demonstrates RVF as a telemetry and configuration store for network
//! operating systems with multi-chassis, multi-interface topologies:
//!
//! 1. Interface state embeddings: encode interface counters, status, and
//!    configuration into fixed-dimensional vectors for anomaly detection
//! 2. Multi-chassis topology: store per-switch interface data with metadata
//!    (hostname, interface name, speed, VLAN, BGP ASN)
//! 3. Anomaly detection: query for interfaces with unusual counter patterns
//! 4. Configuration drift: derive snapshots and compare epochs
//! 5. Witness chain: audit trail for config changes and state transitions
//! 6. Filtered queries: find interfaces by chassis, speed, or VLAN
//!
//! This pattern applies to any network OS (EOS-style, NX-OS-style, JunOS-style,
//! SONiC, DENT, OpenSwitch) — RVF stores the interface telemetry as vectors
//! alongside structured metadata for fast similarity search.
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG, WITNESS_SEG
//!
//! Run: `cargo run --example network_interfaces`

use rvf_crypto::{create_witness_chain, shake256_256, verify_witness_chain, WitnessEntry};
use rvf_runtime::options::DistanceMetric;
use rvf_runtime::filter::FilterValue;
use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore,
};
use rvf_types::DerivationType;
use tempfile::TempDir;

/// Encode interface counters into a fixed-dimensional embedding vector.
///
/// Each interface's operational state is captured as a 64-dim vector:
///   [0..8]   = normalized counter rates (rx_bytes, tx_bytes, rx_pkts, tx_pkts,
///              rx_errors, tx_errors, rx_drops, tx_drops)
///   [8..16]  = counter deltas (rate of change)
///   [16..24] = utilization metrics (link util, buffer util, queue depth, CRC errors, etc.)
///   [24..32] = protocol state (BGP state, OSPF cost, STP port state, LACP rate, etc.)
///   [32..64] = reserved / derived features
fn encode_interface(counters: &InterfaceCounters, seed: u64) -> Vec<f32> {
    let dim = 64;
    let mut v = vec![0.0f32; dim];

    // Normalized counter rates (bytes/sec scaled to [0, 1] range)
    let max_rate = 100_000_000_000.0f64; // 100 Gbps
    v[0] = (counters.rx_bytes_per_sec as f64 / max_rate) as f32;
    v[1] = (counters.tx_bytes_per_sec as f64 / max_rate) as f32;
    v[2] = (counters.rx_pkts_per_sec as f64 / 150_000_000.0) as f32; // 150 Mpps max
    v[3] = (counters.tx_pkts_per_sec as f64 / 150_000_000.0) as f32;
    v[4] = counters.rx_error_rate;
    v[5] = counters.tx_error_rate;
    v[6] = counters.rx_drop_rate;
    v[7] = counters.tx_drop_rate;

    // Counter deltas (rate of change from last sample)
    v[8] = counters.rx_bytes_delta;
    v[9] = counters.tx_bytes_delta;
    v[10] = counters.rx_pkts_delta;
    v[11] = counters.tx_pkts_delta;
    v[12] = counters.error_delta;
    v[13] = counters.drop_delta;
    v[14] = counters.crc_error_rate;
    v[15] = counters.fcs_error_rate;

    // Utilization metrics
    v[16] = counters.link_utilization;
    v[17] = counters.buffer_utilization;
    v[18] = (counters.queue_depth as f32) / 65535.0;
    v[19] = counters.jitter_ms / 100.0;
    v[20] = counters.latency_us / 10000.0;

    // Protocol state encoding
    v[24] = match counters.oper_status {
        OperStatus::Up => 1.0,
        OperStatus::Down => 0.0,
        OperStatus::Dormant => 0.5,
        OperStatus::NotPresent => -1.0,
    };
    v[25] = counters.bgp_state_value;
    v[26] = counters.ospf_cost / 65535.0;
    v[27] = counters.stp_port_state;

    // Fill remaining dimensions with deterministic features
    let mut x = seed.wrapping_add(1);
    for slot in v.iter_mut().skip(32) {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *slot = ((x >> 33) as f32) / (u32::MAX as f32) * 0.1; // small noise
    }

    v
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
enum OperStatus {
    Up,
    Down,
    Dormant,
    NotPresent,
}

impl OperStatus {
    fn as_str(&self) -> &'static str {
        match self {
            OperStatus::Up => "up",
            OperStatus::Down => "down",
            OperStatus::Dormant => "dormant",
            OperStatus::NotPresent => "notPresent",
        }
    }
}

struct InterfaceCounters {
    rx_bytes_per_sec: u64,
    tx_bytes_per_sec: u64,
    rx_pkts_per_sec: u64,
    tx_pkts_per_sec: u64,
    rx_error_rate: f32,
    tx_error_rate: f32,
    rx_drop_rate: f32,
    tx_drop_rate: f32,
    rx_bytes_delta: f32,
    tx_bytes_delta: f32,
    rx_pkts_delta: f32,
    tx_pkts_delta: f32,
    error_delta: f32,
    drop_delta: f32,
    crc_error_rate: f32,
    fcs_error_rate: f32,
    link_utilization: f32,
    buffer_utilization: f32,
    queue_depth: u16,
    jitter_ms: f32,
    latency_us: f32,
    oper_status: OperStatus,
    bgp_state_value: f32,
    ospf_cost: f32,
    stp_port_state: f32,
}

struct NetworkInterface {
    id: u64,
    hostname: &'static str,
    name: &'static str,
    speed_gbps: u32,
    vlan: u16,
    mtu: u16,
    asn: u32,
    counters: InterfaceCounters,
}

/// Generate a network topology with multiple chassis and interfaces.
fn generate_topology() -> Vec<NetworkInterface> {
    let chassis = [
        ("spine-01", 65001u32),
        ("spine-02", 65001),
        ("leaf-01", 65101),
        ("leaf-02", 65102),
        ("leaf-03", 65103),
        ("border-01", 65200),
    ];

    let interface_templates = [
        ("Ethernet1/1", 100, 1, 9216),
        ("Ethernet1/2", 100, 1, 9216),
        ("Ethernet2/1", 25, 100, 9000),
        ("Ethernet2/2", 25, 100, 9000),
        ("Ethernet3/1", 10, 200, 1500),
        ("Ethernet3/2", 10, 200, 1500),
        ("Ethernet4/1", 400, 1, 9216),
        ("Management1", 1, 999, 1500),
        ("Loopback0", 0, 0, 65535),
        ("Vlan100", 0, 100, 9000),
    ];

    let mut interfaces = Vec::new();
    let mut next_id = 1u64;
    let mut seed = 42u64;

    for (chassis_idx, (hostname, asn)) in chassis.iter().enumerate() {
        for (intf_idx, (name, speed, vlan, mtu)) in interface_templates.iter().enumerate() {
            seed = seed.wrapping_mul(31).wrapping_add(chassis_idx as u64 * 100 + intf_idx as u64);

            // Generate realistic counters based on interface type
            let is_uplink = *speed >= 100;
            let is_mgmt = *name == "Management1";
            let is_loopback = name.starts_with("Loopback");

            let base_rate = if is_loopback {
                0
            } else if is_mgmt {
                1_000_000
            } else if is_uplink {
                (seed % 80_000_000_000) + 1_000_000_000
            } else {
                (seed % 5_000_000_000) + 100_000_000
            };

            // Inject anomalies on specific interfaces
            let is_anomaly = chassis_idx == 2 && intf_idx == 4; // leaf-01 Ethernet3/1
            let error_rate = if is_anomaly { 0.15 } else { (seed % 100) as f32 / 100000.0 };
            let drop_rate = if is_anomaly { 0.08 } else { (seed % 50) as f32 / 100000.0 };

            let oper = if is_anomaly {
                OperStatus::Dormant
            } else if is_loopback || is_uplink || is_mgmt {
                OperStatus::Up
            } else if seed.is_multiple_of(20) {
                OperStatus::Down
            } else {
                OperStatus::Up
            };

            interfaces.push(NetworkInterface {
                id: next_id,
                hostname,
                name,
                speed_gbps: *speed,
                vlan: *vlan,
                mtu: *mtu,
                asn: *asn,
                counters: InterfaceCounters {
                    rx_bytes_per_sec: base_rate,
                    tx_bytes_per_sec: base_rate * 8 / 10, // slight asymmetry
                    rx_pkts_per_sec: base_rate / 800,
                    tx_pkts_per_sec: base_rate / 900,
                    rx_error_rate: error_rate,
                    tx_error_rate: error_rate * 0.3,
                    rx_drop_rate: drop_rate,
                    tx_drop_rate: drop_rate * 0.2,
                    rx_bytes_delta: ((seed % 200) as f32 - 100.0) / 1000.0,
                    tx_bytes_delta: ((seed % 180) as f32 - 90.0) / 1000.0,
                    rx_pkts_delta: ((seed % 150) as f32 - 75.0) / 1000.0,
                    tx_pkts_delta: ((seed % 120) as f32 - 60.0) / 1000.0,
                    error_delta: if is_anomaly { 0.5 } else { 0.0 },
                    drop_delta: if is_anomaly { 0.3 } else { 0.0 },
                    crc_error_rate: if is_anomaly { 0.02 } else { 0.0 },
                    fcs_error_rate: if is_anomaly { 0.01 } else { 0.0 },
                    link_utilization: (base_rate as f32) / ((*speed as f64 * 1e9) as f32).max(1.0),
                    buffer_utilization: ((seed % 60) as f32) / 100.0,
                    queue_depth: (seed % 1000) as u16,
                    jitter_ms: ((seed % 50) as f32) / 10.0,
                    latency_us: ((seed % 500) as f32) + 10.0,
                    oper_status: oper,
                    bgp_state_value: if is_loopback { 1.0 } else { 0.8 },
                    ospf_cost: if is_loopback { 1.0 } else { (*speed as f32).recip() * 1000.0 },
                    stp_port_state: if is_uplink { 1.0 } else { 0.5 },
                },
            });
            next_id += 1;
        }
    }

    interfaces
}

fn main() {
    println!("=== Network Interface Embeddings ===\n");

    let dim = 64;
    let tmp = TempDir::new().expect("temp dir");

    // ────────────────────────────────────────────────
    // Phase 1: Build network topology
    // ────────────────────────────────────────────────
    println!("--- Phase 1: Network Topology ---");
    let topology = generate_topology();
    println!("  Chassis count:    6 (2 spine, 3 leaf, 1 border)");
    println!("  Interfaces/host:  10");
    println!("  Total interfaces: {}", topology.len());
    println!("  Embedding dim:    {} (counter rates, deltas, utilization, protocol state)", dim);
    println!();

    // ────────────────────────────────────────────────
    // Phase 2: Ingest interface telemetry into RVF
    // ────────────────────────────────────────────────
    println!("--- Phase 2: Ingest Telemetry → RVF ---");
    let store_path = tmp.path().join("network_telemetry.rvf");
    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };
    let mut store = RvfStore::create(&store_path, options).expect("create store");

    // Metadata field layout:
    //   field_id 0: hostname (String)
    //   field_id 1: interface_name (String)
    //   field_id 2: speed_gbps (U64)
    //   field_id 3: vlan (U64)
    //   field_id 4: asn (U64)
    //   field_id 5: oper_status (String)
    //   field_id 6: mtu (U64)

    let batch_size = 20;
    for chunk in topology.chunks(batch_size) {
        let vecs: Vec<Vec<f32>> = chunk
            .iter()
            .enumerate()
            .map(|(i, intf)| encode_interface(&intf.counters, intf.id * 7 + i as u64))
            .collect();
        let refs: Vec<&[f32]> = vecs.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = chunk.iter().map(|intf| intf.id).collect();

        let mut metadata = Vec::new();
        for intf in chunk {
            metadata.push(MetadataEntry {
                field_id: 0,
                value: MetadataValue::String(intf.hostname.to_string()),
            });
            metadata.push(MetadataEntry {
                field_id: 1,
                value: MetadataValue::String(intf.name.to_string()),
            });
            metadata.push(MetadataEntry {
                field_id: 2,
                value: MetadataValue::U64(intf.speed_gbps as u64),
            });
            metadata.push(MetadataEntry {
                field_id: 3,
                value: MetadataValue::U64(intf.vlan as u64),
            });
            metadata.push(MetadataEntry {
                field_id: 4,
                value: MetadataValue::U64(intf.asn as u64),
            });
            metadata.push(MetadataEntry {
                field_id: 5,
                value: MetadataValue::String(intf.counters.oper_status.as_str().to_string()),
            });
            metadata.push(MetadataEntry {
                field_id: 6,
                value: MetadataValue::U64(intf.mtu as u64),
            });
        }

        store
            .ingest_batch(&refs, &ids, Some(&metadata))
            .expect("ingest batch");
    }

    let status = store.status();
    println!("  Ingested {} interface embeddings", status.total_vectors);
    println!("  Segments: {}", status.total_segments);
    println!("  File size: {} bytes ({:.1} KB)", status.file_size, status.file_size as f64 / 1024.0);
    println!();

    // ────────────────────────────────────────────────
    // Phase 3: Anomaly detection — find similar-to-anomaly interfaces
    // ────────────────────────────────────────────────
    println!("--- Phase 3: Anomaly Detection ---");

    // Create an anomalous interface pattern (high error/drop rates)
    let anomaly_pattern = InterfaceCounters {
        rx_bytes_per_sec: 1_000_000_000,
        tx_bytes_per_sec: 800_000_000,
        rx_pkts_per_sec: 1_250_000,
        tx_pkts_per_sec: 890_000,
        rx_error_rate: 0.20,
        tx_error_rate: 0.06,
        rx_drop_rate: 0.10,
        tx_drop_rate: 0.02,
        rx_bytes_delta: 0.05,
        tx_bytes_delta: 0.03,
        rx_pkts_delta: 0.04,
        tx_pkts_delta: 0.02,
        error_delta: 0.8,
        drop_delta: 0.5,
        crc_error_rate: 0.05,
        fcs_error_rate: 0.03,
        link_utilization: 0.1,
        buffer_utilization: 0.8,
        queue_depth: 900,
        jitter_ms: 25.0,
        latency_us: 800.0,
        oper_status: OperStatus::Dormant,
        bgp_state_value: 0.3,
        ospf_cost: 100.0,
        stp_port_state: 0.0,
    };
    let anomaly_query = encode_interface(&anomaly_pattern, 9999);
    let anomaly_results = store
        .query(&anomaly_query, 10, &QueryOptions::default())
        .expect("anomaly query");

    println!("  Query: find interfaces most similar to anomalous pattern");
    println!("  Pattern: high error/drop rates, dormant status, high jitter");
    println!("  Top-10 matches:");
    for (i, r) in anomaly_results.iter().enumerate() {
        let intf = &topology[(r.id - 1) as usize];
        println!(
            "    #{:2}: id={:3} {:12} {:14} speed={:>3}G vlan={:>4} status={:<10} dist={:.4}",
            i + 1,
            r.id,
            intf.hostname,
            intf.name,
            intf.speed_gbps,
            intf.vlan,
            intf.counters.oper_status.as_str(),
            r.distance,
        );
    }
    println!();

    // ────────────────────────────────────────────────
    // Phase 4: Filtered queries — per-chassis, per-speed, per-VLAN
    // ────────────────────────────────────────────────
    println!("--- Phase 4: Filtered Queries ---");

    // Find all 100G uplinks on spine-01
    let spine_query = encode_interface(
        &InterfaceCounters {
            rx_bytes_per_sec: 50_000_000_000,
            tx_bytes_per_sec: 40_000_000_000,
            rx_pkts_per_sec: 62_500_000,
            tx_pkts_per_sec: 44_000_000,
            rx_error_rate: 0.0,
            tx_error_rate: 0.0,
            rx_drop_rate: 0.0,
            tx_drop_rate: 0.0,
            rx_bytes_delta: 0.0,
            tx_bytes_delta: 0.0,
            rx_pkts_delta: 0.0,
            tx_pkts_delta: 0.0,
            error_delta: 0.0,
            drop_delta: 0.0,
            crc_error_rate: 0.0,
            fcs_error_rate: 0.0,
            link_utilization: 0.5,
            buffer_utilization: 0.3,
            queue_depth: 100,
            jitter_ms: 1.0,
            latency_us: 50.0,
            oper_status: OperStatus::Up,
            bgp_state_value: 1.0,
            ospf_cost: 10.0,
            stp_port_state: 1.0,
        },
        1234,
    );

    let spine_opts = QueryOptions {
        filter: Some(FilterExpr::And(vec![
            FilterExpr::Eq(0, FilterValue::String("spine-01".to_string())),
            FilterExpr::Eq(2, FilterValue::U64(100)),
        ])),
        ..Default::default()
    };
    let spine_results = store.query(&spine_query, 5, &spine_opts).expect("spine query");
    println!("  Filter: hostname='spine-01' AND speed=100G");
    println!("  Results: {} interfaces", spine_results.len());
    for r in &spine_results {
        let intf = &topology[(r.id - 1) as usize];
        println!(
            "    id={:3} {} {} (dist={:.4})",
            r.id, intf.hostname, intf.name, r.distance,
        );
    }
    println!();

    // Find all VLAN 200 interfaces across all chassis
    let vlan_opts = QueryOptions {
        filter: Some(FilterExpr::Eq(3, FilterValue::U64(200))),
        ..Default::default()
    };
    let vlan_results = store.query(&spine_query, 20, &vlan_opts).expect("vlan query");
    println!("  Filter: vlan=200 (across all chassis)");
    println!("  Results: {} interfaces", vlan_results.len());
    for r in &vlan_results {
        let intf = &topology[(r.id - 1) as usize];
        println!(
            "    id={:3} {:12} {:14} speed={:>3}G ASN={}",
            r.id, intf.hostname, intf.name, intf.speed_gbps, intf.asn,
        );
    }
    println!();

    // ────────────────────────────────────────────────
    // Phase 5: Configuration drift detection (derive snapshot)
    // ────────────────────────────────────────────────
    println!("--- Phase 5: Configuration Drift ---");
    let snapshot_path = tmp.path().join("telemetry_epoch2.rvf");
    let snapshot = store
        .derive(&snapshot_path, DerivationType::Snapshot, None)
        .expect("derive snapshot");

    println!("  Derived epoch-2 snapshot:");
    println!("    Parent:   network_telemetry.rvf");
    println!("    Child:    telemetry_epoch2.rvf");
    println!("    Depth:    {}", snapshot.lineage_depth());
    println!(
        "    Parent ID matches: {}",
        snapshot.parent_id() == store.file_id()
    );

    // Query the snapshot
    let snap_results = snapshot
        .query(&anomaly_query, 3, &QueryOptions::default())
        .expect("snapshot query");
    println!("  Snapshot anomaly query (top-3):");
    for (i, r) in snap_results.iter().enumerate() {
        let intf = &topology[(r.id - 1) as usize];
        println!(
            "    #{}: {} {} (dist={:.4})",
            i + 1,
            intf.hostname,
            intf.name,
            r.distance,
        );
    }
    println!();

    // ────────────────────────────────────────────────
    // Phase 6: Witness chain for network events
    // ────────────────────────────────────────────────
    println!("--- Phase 6: Network Event Audit Trail ---");

    let ts = 1_700_000_000_000_000_000u64;
    let witness_entries = vec![
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(
                format!(
                    "telemetry_ingest:chassis=6,interfaces={},dim={}",
                    topology.len(),
                    dim
                )
                .as_bytes(),
            ),
            timestamp_ns: ts,
            witness_type: 0x08, // DATA_PROVENANCE
        },
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(
                b"anomaly_detected:host=leaf-01,intf=Ethernet3/1,type=high_error_rate",
            ),
            timestamp_ns: ts + 1_000_000,
            witness_type: 0x02, // COMPUTATION
        },
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(
                b"config_change:host=leaf-01,intf=Ethernet3/1,action=shutdown",
            ),
            timestamp_ns: ts + 2_000_000,
            witness_type: 0x01, // PROVENANCE
        },
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(
                format!(
                    "snapshot_derived:parent_depth=0,child_depth=1,vectors={}",
                    topology.len()
                )
                .as_bytes(),
            ),
            timestamp_ns: ts + 3_000_000,
            witness_type: 0x09, // DERIVATION
        },
    ];

    let chain = create_witness_chain(&witness_entries);
    let verified = verify_witness_chain(&chain).expect("verify chain");
    println!("  Events recorded: {}", verified.len());
    for (i, e) in verified.iter().enumerate() {
        let label = match e.witness_type {
            0x01 => "PROVENANCE",
            0x02 => "COMPUTATION",
            0x08 => "DATA_PROVENANCE",
            0x09 => "DERIVATION",
            _ => "UNKNOWN",
        };
        println!(
            "    #{}: type=0x{:02X} ({}) hash={}",
            i + 1,
            e.witness_type,
            label,
            e.action_hash
                .iter()
                .take(8)
                .map(|b| format!("{:02x}", b))
                .collect::<String>(),
        );
    }
    println!();

    // ────────────────────────────────────────────────
    // Summary
    // ────────────────────────────────────────────────
    println!("=== Summary ===\n");
    println!("  Interfaces ingested:  {}", topology.len());
    println!("  Embedding dimensions: {} (counters + deltas + utilization + protocol)", dim);
    println!("  Chassis covered:      6 (2 spine, 3 leaf, 1 border)");
    println!("  Anomaly detection:    vector similarity search (L2 distance)");
    println!("  Filtered queries:     hostname, speed, VLAN, ASN metadata");
    println!("  Drift detection:      epoch snapshots via derive()");
    println!("  Audit trail:          {} witness entries, tamper-evident", verified.len());
    println!();
    println!("  Key insight: RVF turns network telemetry into a searchable,");
    println!("  portable, auditable vector store — anomaly detection and config");
    println!("  drift analysis via embedding similarity instead of threshold rules.");
    println!();
    println!("=== Done ===");
}
