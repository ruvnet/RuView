//! Causal Atlas Dashboard — ADR-040 Phase 2 Capstone Example
//!
//! Builds a single RVF file containing:
//!   - Vector knowledge base (100 targets, 15 windows each)
//!   - KERNEL_SEG (HermitOS x86_64)
//!   - EBPF_SEG (distance computation)
//!   - DASHBOARD_SEG (embedded web dashboard)
//!   - 17-entry witness chain
//!   - Ed25519 attestation
//!
//! Then starts an HTTP server at localhost:8080 serving the embedded dashboard
//! with domain-specific API endpoints and WebSocket live streaming.
//!
//! Run: cargo run --example causal_atlas_dashboard

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use std::sync::Arc;
use tokio::sync::Mutex;

use rvf_runtime::{
    MetadataEntry, MetadataValue, RvfOptions, RvfStore,
};
use rvf_runtime::options::DistanceMetric;
use rvf_types::kernel::{KernelArch, KernelType, KernelHeader, KERNEL_MAGIC};
use rvf_types::ebpf::{EbpfAttachType, EbpfHeader, EbpfProgramType, EBPF_MAGIC};
use rvf_types::dashboard::{DashboardHeader, DASHBOARD_MAGIC};
use rvf_types::{SegmentHeader, SegmentType};
use rvf_crypto::{
    sign_segment, verify_segment,
    create_witness_chain, verify_witness_chain, shake256_256, WitnessEntry,
};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// LCG helpers (same as causal_atlas_sealed.rs)
// ---------------------------------------------------------------------------

fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed.wrapping_add(1);
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

// ---------------------------------------------------------------------------
// Synthetic dashboard bundle builder
// ---------------------------------------------------------------------------

/// Build a minimal synthetic HTML/JS dashboard that demonstrates the
/// DASHBOARD_SEG mechanism. The raw HTML bytes are embedded directly so
/// the rvf-server `serve_index` handler can serve them at `/`.
///
/// The dashboard JS targets the domain API endpoints already implemented
/// in `rvf-server::http` (e.g. `/api/candidates/planet`, `/api/status`).
fn build_synthetic_dashboard() -> Vec<u8> {
    let html = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>RVF Causal Atlas Dashboard</title>
<style>
* { margin: 0; padding: 0; box-sizing: border-box; }
body { font-family: system-ui, sans-serif; background: #1a1a2e; color: #e0e0e0; }
nav { background: #16213e; padding: 12px 24px; display: flex; gap: 16px; align-items: center; }
nav a { color: #4fc3f7; text-decoration: none; padding: 6px 12px; border-radius: 4px; }
nav a:hover, nav a.active { background: #0f3460; }
h1 { font-size: 18px; color: #4fc3f7; margin-right: auto; }
main { padding: 24px; }
.card { background: #16213e; border-radius: 8px; padding: 16px; margin-bottom: 16px; }
.card h2 { color: #4fc3f7; margin-bottom: 8px; font-size: 16px; }
.grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
table { width: 100%; border-collapse: collapse; }
th, td { padding: 8px 12px; text-align: left; border-bottom: 1px solid #333; }
th { color: #4fc3f7; }
.gauge { height: 20px; background: #0f3460; border-radius: 10px; overflow: hidden; margin: 4px 0; }
.gauge-fill { height: 100%; border-radius: 10px; transition: width 0.3s; }
.status-ok { color: #4caf50; }
.ws-indicator { width: 10px; height: 10px; border-radius: 50%; display: inline-block; }
.ws-connected { background: #4caf50; }
.ws-disconnected { background: #f44336; }
#log { max-height: 300px; overflow-y: auto; font-family: monospace; font-size: 12px; }
#log div { padding: 2px 0; border-bottom: 1px solid #222; }
</style>
</head>
<body>
<nav>
  <h1>Causal Atlas</h1>
  <a href="#/atlas">Atlas</a>
  <a href="#/coherence">Coherence</a>
  <a href="#/planets">Planets</a>
  <a href="#/life">Life</a>
  <a href="#/status">Status</a>
  <span class="ws-indicator ws-disconnected" id="ws-dot"></span>
</nav>
<main id="content">Loading...</main>
<script>
var content = document.getElementById('content');
var wsDot = document.getElementById('ws-dot');

// WebSocket
var ws;
function connectWS() {
  var proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  ws = new WebSocket(proto + '//' + location.host + '/ws/live');
  ws.onopen = function() { wsDot.className = 'ws-indicator ws-connected'; };
  ws.onclose = function() { wsDot.className = 'ws-indicator ws-disconnected'; setTimeout(connectWS, 3000); };
  ws.onmessage = function(e) {
    try {
      var evt = JSON.parse(e.data);
      var log = document.getElementById('log');
      if (log) {
        var div = document.createElement('div');
        div.textContent = evt.timestamp + ' [' + evt.event_type + '] ' + JSON.stringify(evt.data);
        log.appendChild(div);
        log.scrollTop = log.scrollHeight;
      }
    } catch(ex) {}
  };
}
connectWS();

// API helper
function api(path) {
  return fetch(path).then(function(r) { return r.json(); });
}

// Views — each view fetches from rvf-server domain API endpoints
var views = {
  atlas: function() {
    return api('/api/atlas/query?event_id=evt_001').then(function(data) {
      return '<div class="card"><h2>Atlas Explorer</h2>' +
        '<p>Event: ' + data.event_id + '</p>' +
        '<p>Parents: ' + data.parents.join(', ') + '</p>' +
        '<p>Children: ' + data.children.join(', ') + '</p>' +
        '<p>Weight: ' + data.weight + '</p>' +
        '<p><em>Full 3D view requires Vite build. Run: cd examples/rvf/dashboard && npm run build</em></p></div>';
    });
  },
  coherence: function() {
    return api('/api/coherence').then(function(data) {
      var rows = data.values.map(function(row) {
        return row.map(function(v) { return v.toFixed(2); }).join(' ');
      }).join('\n');
      return '<div class="card"><h2>Coherence Heatmap</h2>' +
        '<p>Grid: ' + data.grid_size[0] + 'x' + data.grid_size[1] +
        ' | Range: ' + data.min.toFixed(2) + ' - ' + data.max.toFixed(2) + '</p>' +
        '<pre style="font-size:10px;line-height:1.2;overflow:auto;max-height:300px">' + rows + '</pre></div>';
    });
  },
  planets: function() {
    return api('/api/candidates/planet').then(function(data) {
      var rows = data.candidates.map(function(p) {
        return '<tr><td>' + p.id + '</td><td>' + p.score.toFixed(2) +
          '</td><td>' + p.period_days.toFixed(1) + 'd</td><td>' +
          p.radius_earth.toFixed(2) + ' R\u2295</td><td>' +
          p.stellar_type + '</td><td>' + p.status + '</td></tr>';
      }).join('');
      return '<div class="card"><h2>Planet Candidates (' + data.total + ')</h2>' +
        '<p>Confirmed: ' + data.confirmed + ' | Mean Score: ' + data.mean_score.toFixed(3) + '</p>' +
        '<table><tr><th>ID</th><th>Score</th><th>Period</th><th>Radius</th><th>Star</th><th>Status</th></tr>' +
        rows + '</table></div>';
    });
  },
  life: function() {
    return api('/api/candidates/life').then(function(data) {
      var rows = data.candidates.map(function(l) {
        return '<tr><td>' + l.id + '</td><td>' + l.life_score.toFixed(2) +
          '</td><td>' + (l.h2o_detected ? 'Yes' : 'No') +
          '</td><td>' + l.o2_ppm + ' ppm</td><td>' +
          l.biosig_confidence.toFixed(2) + '</td><td>' +
          l.habitability_index.toFixed(2) + '</td></tr>';
      }).join('');
      return '<div class="card"><h2>Life Candidates (' + data.total + ')</h2>' +
        '<p>High Confidence: ' + data.high_confidence +
        ' | Mean Score: ' + data.mean_life_score.toFixed(3) + '</p>' +
        '<table><tr><th>ID</th><th>Life Score</th><th>H2O</th><th>O2</th>' +
        '<th>Biosig Conf</th><th>Habitability</th></tr>' +
        rows + '</table></div>';
    });
  },
  status: function() {
    return Promise.all([api('/api/status'), api('/api/memory/tiers')]).then(function(results) {
      var st = results[0];
      var mem = results[1];
      var tierRows = mem.tiers.map(function(t) {
        return '<div style="margin:4px 0"><span style="display:inline-block;width:160px">' +
          t.name + ' (' + t.label + ')</span>' +
          '<div class="gauge"><div class="gauge-fill" style="width:' +
          (t.utilization * 100).toFixed(0) + '%;background:#4fc3f7"></div></div>' +
          '<span style="font-size:11px;color:#888">' +
          t.used_mb.toFixed(1) + ' / ' + t.capacity_mb + ' MB</span></div>';
      }).join('');
      var features = st.features.join(', ');
      return '<div class="grid">' +
        '<div class="card"><h2>System Status</h2>' +
        '<p>Status: <span class="status-ok">' + st.status + '</span></p>' +
        '<p>Vectors: ' + st.store.total_vectors + '</p>' +
        '<p>Segments: ' + st.store.total_segments + '</p>' +
        '<p>File Size: ' + st.store.file_size + ' bytes</p>' +
        '<p>API: v' + st.api_version + '</p>' +
        '<p>Features: ' + features + '</p>' +
        '<h3 style="margin-top:12px">Memory Tiers</h3>' + tierRows +
        '</div>' +
        '<div class="card"><h2>Live Events</h2><div id="log"></div></div>' +
        '</div>';
    });
  }
};

// Router
function navigate() {
  var hash = location.hash.replace('#/', '') || 'atlas';
  var links = document.querySelectorAll('nav a');
  for (var i = 0; i < links.length; i++) {
    links[i].classList.toggle('active', links[i].getAttribute('href') === '#/' + hash);
  }
  var viewFn = views[hash];
  if (viewFn) {
    viewFn().then(function(html) { content.innerHTML = html; });
  } else {
    content.innerHTML = '<div class="card"><h2>Not Found</h2><p>View not found.</p></div>';
  }
}
window.addEventListener('hashchange', navigate);
navigate();
</script>
</body>
</html>"##;

    html.as_bytes().to_vec()
}

// ---------------------------------------------------------------------------
// Main (async for HTTP server)
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    println!("=== Causal Atlas Dashboard (ADR-040 Phase 2) ===\n");

    let dim = 128;
    let num_targets = 100;
    let windows_per_target = 15;
    let total_windows = num_targets * windows_per_target;

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("causal_atlas_dashboard.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");

    // ==== 1. Vector knowledge base ====
    println!("--- 1. Vector Knowledge Base ({} Targets) ---", num_targets);

    let domains = ["transit", "flare", "rotation", "eclipse", "variability"];
    let scales = ["2h", "12h", "3d", "27d"];

    let mut all_vectors: Vec<Vec<f32>> = Vec::with_capacity(total_windows);
    let mut all_ids: Vec<u64> = Vec::with_capacity(total_windows);
    let mut all_metadata: Vec<MetadataEntry> = Vec::with_capacity(total_windows * 4);

    for target in 0..num_targets {
        for win in 0..windows_per_target {
            let global_id = (target * windows_per_target + win) as u64;
            let vec = random_vector(dim, global_id * 31 + target as u64);
            all_vectors.push(vec);
            all_ids.push(global_id);

            let domain_idx = target % domains.len();
            let scale_idx = win % scales.len();
            let epoch = 1_600_000_000u64 + win as u64 * 7200 + target as u64 * 86400;

            all_metadata.push(MetadataEntry {
                field_id: 0,
                value: MetadataValue::String(domains[domain_idx].to_string()),
            });
            all_metadata.push(MetadataEntry {
                field_id: 1,
                value: MetadataValue::String(scales[scale_idx].to_string()),
            });
            all_metadata.push(MetadataEntry {
                field_id: 2,
                value: MetadataValue::U64(target as u64),
            });
            all_metadata.push(MetadataEntry {
                field_id: 3,
                value: MetadataValue::U64(epoch),
            });
        }
    }

    let vec_refs: Vec<&[f32]> = all_vectors.iter().map(|v| v.as_slice()).collect();
    let ingest = store
        .ingest_batch(&vec_refs, &all_ids, Some(&all_metadata))
        .expect("ingest failed");
    println!("  Vectors: {} ({} dims)", ingest.accepted, dim);

    // ==== 2. Kernel image ====
    println!("\n--- 2. Runtime (Kernel Image) ---");
    let mut kernel_image = Vec::with_capacity(16384);
    kernel_image.extend_from_slice(&[0x7F, b'E', b'L', b'F']);
    kernel_image.extend_from_slice(b"RVF-CAUSAL-ATLAS-DASHBOARD-v2.0");
    for i in 35..16384u32 {
        kernel_image.push((i.wrapping_mul(0xCAFE) >> 8) as u8);
    }
    let kernel_seg_id = store
        .embed_kernel(
            KernelArch::X86_64 as u8,
            KernelType::Hermit as u8,
            0x00F8,
            &kernel_image,
            8080,
            Some("rvf.mode=dashboard rvf.atlas=true"),
        )
        .expect("failed to embed kernel");
    println!("  Kernel embedded: segment ID {}", kernel_seg_id);

    // ==== 3. eBPF accelerator ====
    println!("\n--- 3. Accelerator (eBPF Program) ---");
    let num_insns = 128;
    let mut ebpf_bytecode = Vec::with_capacity(num_insns * 8);
    for i in 0..num_insns {
        let insn: u64 = 0xB700_0000_0000_0000 | ((i as u64) << 16);
        ebpf_bytecode.extend_from_slice(&insn.to_le_bytes());
    }
    let ebpf_seg_id = store
        .embed_ebpf(
            EbpfProgramType::XdpDistance as u8,
            EbpfAttachType::XdpIngress as u8,
            dim as u16,
            &ebpf_bytecode,
            None,
        )
        .expect("failed to embed eBPF");
    println!("  eBPF embedded: segment ID {}", ebpf_seg_id);

    // ==== 4. Dashboard bundle (DASHBOARD_SEG) ====
    println!("\n--- 4. Dashboard (DASHBOARD_SEG) ---");
    let bundle_data = build_synthetic_dashboard();
    let dashboard_seg_id = store
        .embed_dashboard(0, &bundle_data, "index.html")
        .expect("failed to embed dashboard");
    println!("  Dashboard embedded: segment ID {}", dashboard_seg_id);
    println!("  Bundle size: {} bytes", bundle_data.len());
    println!("  Entry point: index.html");

    // Verify round-trip
    let (dh_bytes, db_bytes) = store
        .extract_dashboard()
        .expect("extract_dashboard failed")
        .expect("no dashboard found");
    let dh_arr: [u8; 64] = dh_bytes.try_into().unwrap();
    let dh = DashboardHeader::from_bytes(&dh_arr).expect("invalid dashboard header");
    assert_eq!(dh.dashboard_magic, DASHBOARD_MAGIC);
    assert_eq!(dh.ui_framework, 0);
    assert_eq!(db_bytes, bundle_data);
    println!(
        "  Verified: magic={:#010X}, framework=threejs",
        dh.dashboard_magic
    );

    // ==== 5. Witness chain ====
    println!("\n--- 5. Witness Chain ---");
    let chain_steps: Vec<(&str, u8)> = vec![
        ("genesis", 0x01),
        ("target_catalog_load", 0x08),
        ("light_curve_ingest", 0x08),
        ("windowing", 0x02),
        ("feature_extraction", 0x02),
        ("embedding_generation", 0x02),
        ("causal_edge_build", 0x02),
        ("coherence_field_compute", 0x02),
        ("boundary_tracking", 0x02),
        ("planet_detection_p0", 0x02),
        ("planet_detection_p1", 0x02),
        ("planet_detection_p2", 0x02),
        ("life_scoring_l0_l2", 0x02),
        ("kernel_embed", 0x02),
        ("ebpf_embed", 0x02),
        ("dashboard_embed", 0x02),
        ("atlas_seal", 0x01),
    ];

    let entries: Vec<WitnessEntry> = chain_steps
        .iter()
        .enumerate()
        .map(|(i, (step, wtype))| {
            let action_data =
                format!("causal_atlas_dashboard:{}:step_{}", step, i);
            WitnessEntry {
                prev_hash: [0u8; 32],
                action_hash: shake256_256(action_data.as_bytes()),
                timestamp_ns: 1_700_000_000_000_000_000 + i as u64 * 1_000_000_000,
                witness_type: *wtype,
            }
        })
        .collect();

    let chain_bytes = create_witness_chain(&entries);
    let verified_chain =
        verify_witness_chain(&chain_bytes).expect("chain verification failed");
    println!("  Chain entries: {}", verified_chain.len());
    println!("  Integrity: VALID");

    // ==== 6. Ed25519 attestation ====
    println!("\n--- 6. Attestation (Ed25519) ---");
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    let mut header = SegmentHeader::new(SegmentType::Vec as u8, 1);
    header.timestamp_ns = 1_700_000_000_000_000_000;
    header.payload_length = 4096;
    let attestation_payload =
        b"Sealed Causal Atlas Dashboard: ADR-040 Phase 2";
    let footer = sign_segment(&header, attestation_payload, &signing_key);
    let sig_valid =
        verify_segment(&header, attestation_payload, &footer, &verifying_key);
    println!(
        "  Signer:    {}...",
        hex_string(&verifying_key.to_bytes()[..16])
    );
    println!("  Signature: VALID ({})", sig_valid);

    // ==== 7. Component verification ====
    println!("\n--- 7. Component Verification ---");

    // Verify kernel
    let (kh_bytes, ki_bytes) = store
        .extract_kernel()
        .expect("extract_kernel failed")
        .expect("no kernel found");
    let kh_arr: [u8; 128] = kh_bytes.try_into().unwrap();
    let kh = KernelHeader::from_bytes(&kh_arr).expect("invalid kernel header");
    assert_eq!(kh.kernel_magic, KERNEL_MAGIC);
    assert_eq!(kh.arch, KernelArch::X86_64 as u8);
    assert_eq!(kh.api_port, 8080);
    assert!(ki_bytes.starts_with(&kernel_image));
    println!(
        "  Kernel:    VALID (magic={:#010X}, arch=x86_64, port=8080)",
        kh.kernel_magic
    );

    // Verify eBPF
    let (eh_bytes, ep_bytes) = store
        .extract_ebpf()
        .expect("extract_ebpf failed")
        .expect("no eBPF found");
    let eh_arr: [u8; 64] = eh_bytes.try_into().unwrap();
    let eh = EbpfHeader::from_bytes(&eh_arr).expect("invalid eBPF header");
    assert_eq!(eh.ebpf_magic, EBPF_MAGIC);
    assert_eq!(eh.program_type, EbpfProgramType::XdpDistance as u8);
    assert_eq!(eh.max_dimension, dim as u16);
    assert_eq!(&ep_bytes[..ebpf_bytecode.len()], ebpf_bytecode.as_slice());
    println!(
        "  eBPF:      VALID (magic={:#010X}, type=XDP, dim={})",
        eh.ebpf_magic, eh.max_dimension
    );

    // Verify witness chain
    let re_verified =
        verify_witness_chain(&chain_bytes).expect("re-verify failed");
    assert_eq!(re_verified.len(), chain_steps.len());
    println!("  Witness:   VALID ({} entries)", re_verified.len());

    // Verify signature
    assert!(sig_valid);
    println!("  Signature: VALID (Ed25519)");

    // ==== 8. Manifest ====
    println!("\n--- 8. Sealed Atlas Dashboard Manifest ---");
    let final_status = store.status();

    println!("  +-----------------------------------------------------------+");
    println!("  |       CAUSAL ATLAS DASHBOARD v2.0 (ADR-040 Phase 2)       |");
    println!("  +-----------------------------------------------------------+");
    println!("  | Component          | Details                               |");
    println!("  |--------------------|---------------------------------------|");
    println!(
        "  | Knowledge Base     | {} vectors x {} dims              |",
        final_status.total_vectors, dim
    );
    println!(
        "  | Targets            | {} synthetic targets                |",
        num_targets
    );
    println!(
        "  | Runtime            | HermitOS x86_64 ({} KB)            |",
        kernel_image.len() / 1024
    );
    println!(
        "  | Accelerator        | XDP eBPF ({} insns)                |",
        num_insns
    );
    println!(
        "  | Dashboard          | {} bytes (embedded HTML/JS)     |",
        bundle_data.len()
    );
    println!(
        "  | Trust Chain        | {} witness entries                  |",
        chain_steps.len()
    );
    println!("  | Attestation        | Ed25519 signature                    |");
    println!(
        "  | Total Segments     | {}                                  |",
        final_status.total_segments
    );
    println!(
        "  | File Size          | {} bytes                          |",
        final_status.file_size
    );
    println!("  +-----------------------------------------------------------+");

    // ==== 9. Start HTTP server ====
    println!("\n--- 9. Starting HTTP Server ---");
    println!("  Dashboard: http://localhost:8080");
    println!("  API:       http://localhost:8080/api/status");
    println!("  WebSocket: ws://localhost:8080/ws/live");
    println!("  Press Ctrl+C to stop.\n");

    let shared_store = Arc::new(Mutex::new(store));

    // Create event channel for WebSocket
    let (event_tx, _rx) = rvf_server::ws::event_channel();

    // Resolve dashboard dist directory (Vite build with Three.js)
    let dashboard_dist = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dashboard")
        .join("dist");
    let static_dir = if dashboard_dist.join("index.html").exists() {
        println!("  Using Vite build: {}", dashboard_dist.display());
        Some(dashboard_dist)
    } else {
        println!("  Using embedded DASHBOARD_SEG (run `cd examples/rvf/dashboard && npm run build` for Three.js)");
        None
    };

    // Build router with static file serving for Three.js assets
    let app = rvf_server::http::router_with_static(shared_store, event_tx.clone(), static_dir);

    // Spawn a task to send periodic demo events
    let tx_clone = event_tx.clone();
    tokio::spawn(async move {
        let mut counter = 0u64;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            counter += 1;
            let event = rvf_server::ws::LiveEvent {
                event_type: match counter % 3 {
                    0 => "boundary_alert".to_string(),
                    1 => "candidate_new".to_string(),
                    _ => "coherence_update".to_string(),
                },
                timestamp: format!(
                    "2024-01-15T10:{:02}:00Z",
                    counter % 60
                ),
                data: serde_json::json!({
                    "counter": counter,
                    "message": format!("Demo event #{}", counter)
                }),
            };
            let _ = tx_clone.send(event);
        }
    });

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("failed to bind to port 8080");

    axum::serve(listener, app).await.expect("server error");
}
