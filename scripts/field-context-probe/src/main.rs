//! Synthetic reliability probe using actual pinned RuField and WorldGraph.
//! No activity model, physical capture, confidence calibration or person track.
use std::collections::{BTreeMap, BTreeSet};
use rufield_core::{FieldEvent, FieldTensor, FieldAxis, Modality, Observation, PrivacyClass,
    ProvenanceRef, SensorDescriptor, FusionEngine, InferenceQuery, FieldInference};
use rufield_fusion::{RuFieldFusion, RuleSet};
use rufield_provenance::{sha256_hex, Signer, verify_event, TrustVerifier, TrustPolicy, TrustedKeyRegistry};
use wifi_densepose_worldgraph::{WorldGraph, WorldNode, WorldId, WorldEdge, SemanticProvenance};
use serde_json::{json, Value};

const NOW: u64 = 1_800_000_000_000_000_000; // Authored simulation clock, not capture metadata.
const MAX_AGE: u64 = 2_000_000_000;
const MAX_EVENTS: usize = 32;
const RULES: &str = r#"
[rule.synthetic_presence]
inputs = ["wifi_csi"]
method = "weighted_bayes"
feature = "presence"
threshold = 0.5
privacy_max = "P2"
"#;

fn fixture(id: &str, timestamp: u64) -> FieldEvent {
    let tensor = FieldTensor::new(timestamp, Modality::WifiCsi,
        vec![FieldAxis::Frequency], vec![3], vec![1., 2., 3.],
        0.9, 0.01, Some("synthetic_room_cal_v1".into()), PrivacyClass::P2).unwrap();
    let mut observation = Observation::occupancy(0.9, PrivacyClass::P2);
    observation.features.insert("presence".into(), 0.9);
    let mut event = FieldEvent::new(id, timestamp, SensorDescriptor {
        modality: "wifi_csi".into(), vendor: "SYNTHETIC_FIXTURE".into(),
        device_id: "synthetic_sensor".into(), placement: "authored_fixture".into(),
        coordinate_frame: None, position_m: None, orientation_xyzw: None,
        clock_domain: "synthetic_clock".into(),
    }, tensor, observation, ProvenanceRef {
        raw_hash: sha256_hex(format!("SYNTHETIC fixture {id} {timestamp}").as_bytes()),
        firmware_hash: sha256_hex(b"SYNTHETIC fixture generator"),
        model_id: "authored_feature_not_trained_model".into(),
        calibration_id: "synthetic_room_cal_v1".into(), synthetic: true,
        signature_hex: None, signer_pubkey_hex: None,
    });
    // Public deterministic test key; never a production credential.
    Signer::from_seed(&[17; 32]).sign_event(&mut event).unwrap();
    event
}

/// Thin conservative bridge policy; these guards are NOT native WorldGraph features.
/// Only fresh signed synthetic root events enter this demonstration. Derived
/// observations cannot feed back into the root evidence stream.
struct Bridge {
    fusion: RuFieldFusion,
    world: WorldGraph,
    roots: BTreeMap<String, WorldId>,
    hashes: BTreeSet<String>,
    published: BTreeSet<String>,
}
impl Bridge {
    fn new() -> Self {
        Self { fusion: RuFieldFusion::with_rules(RuleSet::from_toml(RULES).unwrap()),
            world: WorldGraph::new(Default::default()), roots: BTreeMap::new(),
            hashes: BTreeSet::new(), published: BTreeSet::new() }
    }
    fn ingest(&mut self, event: FieldEvent, ancestry: &[String], now: u64) -> Result<(), String> {
        if !event.provenance.synthetic { return Err("synthetic_probe_only".into()); }
        if !ancestry.is_empty() { return Err("derived_or_circular_evidence_excluded".into()); }
        if event.timestamp_ns > now || now.saturating_sub(event.timestamp_ns) > MAX_AGE {
            return Err("stale_or_future_bridge_gate".into());
        }
        if self.roots.len() >= MAX_EVENTS { return Err("bounded_capacity".into()); }
        if self.roots.contains_key(&event.event_id) || self.hashes.contains(&event.provenance.raw_hash) {
            return Err("duplicate_evidence_bridge_gate".into());
        }
        verify_event(&event).map_err(|e| format!("signature: {e}"))?;
        let id = event.event_id.clone();
        let hash = event.provenance.raw_hash.clone();
        let at = i64::try_from(event.timestamp_ns / 1_000_000).map_err(|_| "timestamp_overflow")?;
        self.fusion.ingest_at(event, now).map_err(|e| e.to_string())?;
        let node = self.world.upsert_node(WorldNode::Event { id: WorldId::UNASSIGNED,
            event_type: format!("SYNTHETIC root {id}"), at_unix_ms: at, located_in: None });
        self.roots.insert(id, node);
        self.hashes.insert(hash);
        Ok(())
    }
    fn inference(&self) -> FieldInference {
        self.fusion.infer(&InferenceQuery::all()).unwrap().into_iter().next().unwrap()
    }
    fn publish(&mut self, inference: &FieldInference, now: u64) -> Result<WorldId, String> {
        if inference.expires_ns <= now || inference.produced_ns > now {
            return Err("expired_inference_bridge_gate".into());
        }
        if !inference.confidence.is_finite() || !(0.0..=1.0).contains(&inference.confidence) {
            return Err("invalid_heuristic_score".into());
        }
        let sources: BTreeSet<_> = inference.supporting_events.iter().cloned().collect();
        if sources.is_empty() || sources.len() != inference.supporting_events.len() ||
            sources.iter().any(|id| !self.roots.contains_key(id)) {
            return Err("missing_or_repeated_root_ancestry".into());
        }
        let fingerprint = format!("{}:{:?}:{:?}", inference.model_id, sources, inference.produced_ns);
        if self.published.contains(&fingerprint) { return Err("duplicate_belief_bridge_gate".into()); }
        let nodes: Vec<_> = sources.iter().map(|id| self.roots[id]).collect();
        let handles: Vec<_> = sources.into_iter().collect();
        let state = self.world.add_semantic_state(
            "SYNTHETIC fixture presence; heuristic score, not calibrated probability".into(),
            inference.confidence, (inference.produced_ns / 1_000_000) as i64,
            SemanticProvenance { evidence: handles, model_version: inference.model_id.clone(),
                calibration_version: inference.calibration_id.clone().unwrap_or_default(),
                privacy_decision: "SYNTHETIC P2 local probe; no external release".into() }, &nodes);
        self.published.insert(fingerprint);
        Ok(state)
    }
}

fn probe() -> Value {
    let event = fixture("root-1", NOW);
    let mut native = TrustVerifier::simulation();
    native.verify_and_record_at(&event, NOW).unwrap();
    let native_duplicate = native.verify_and_record_at(&event, NOW).is_err();
    let old_event = fixture("old-root", NOW - MAX_AGE - 1);
    let mut simulation = TrustVerifier::simulation();
    let native_simulation_accepts_old = simulation.verify_and_record_at(&old_event, NOW).is_ok();
    let mut production = TrustVerifier::new(TrustPolicy::production(), TrustedKeyRegistry::new());
    let native_production_rejects_synthetic = production.verify_and_record_at(&event, NOW).is_err();

    let mut bridge = Bridge::new();
    bridge.ingest(event.clone(), &[], NOW).unwrap();
    let before = bridge.world.node_count();
    let duplicate_blocked = bridge.ingest(event, &[], NOW).is_err();
    let stale_blocked = bridge.ingest(old_event, &[], NOW).is_err();
    let cycle_blocked = bridge.ingest(fixture("derived-1", NOW + 1), &["derived-1".into()], NOW + 1).is_err();
    assert_eq!(before, bridge.world.node_count());
    let inference = bridge.inference();
    let belief = bridge.publish(&inference, NOW).unwrap();
    let before_rejections = bridge.world.to_json().unwrap();
    let repeated_belief = bridge.publish(&inference, NOW).is_err();
    let expired = bridge.publish(&inference, inference.expires_ns).is_err();
    let mut missing = inference.clone();
    missing.supporting_events = vec!["unknown-root".into()];
    let missing_blocked = bridge.publish(&missing, NOW).is_err();
    assert_eq!(before_rejections, bridge.world.to_json().unwrap());
    let ancestry = bridge.world.neighbors(belief);
    assert_eq!(ancestry.len(), 1);
    assert_eq!(ancestry[0].0, bridge.roots["root-1"]);
    assert!(matches!(&ancestry[0].1, WorldEdge::DerivedFrom { evidence } if evidence == "root-1"));
    let wire = bridge.world.to_json().unwrap();
    let restored = WorldGraph::from_json(&wire).unwrap();
    assert_eq!(restored.node_count(), bridge.world.node_count());
    assert_eq!(restored.edge_count(), bridge.world.edge_count());
    let restored_ancestry = restored.neighbors(belief);
    assert_eq!(restored_ancestry[0].0, bridge.roots["root-1"]);
    assert!(matches!(&restored_ancestry[0].1, WorldEdge::DerivedFrom { evidence } if evidence == "root-1"));

    // Characterize native WorldGraph's boundary honestly in an isolated graph.
    let mut unrestricted = WorldGraph::new(Default::default());
    let a = unrestricted.upsert_node(WorldNode::Event { id: WorldId::UNASSIGNED,
        event_type: "SYNTHETIC cycle boundary fixture".into(), at_unix_ms: 0, located_in: None });
    let native_cycle_allowed = unrestricted.add_edge(a, a, WorldEdge::DerivedFrom { evidence: "fixture".into() }).is_ok();
    json!({"evidence":"SYNTHETIC", "purpose":"integrity and reliability, not activity accuracy",
        "ruFieldRevision":"b74bc035e4a3d4023665aa5479b41e938facd113",
        "worldGraphRevision":"9b1c79c836cdacfb7b44f058c593157bac4c1dab",
        "native": {"duplicateRejected":native_duplicate,
            "simulationSkipsWallClockFreshness":native_simulation_accepts_old,
            "productionRejectsSynthetic":native_production_rejects_synthetic,
            "worldGraphAllowsCycles":native_cycle_allowed},
        "bridge": {"duplicateRejected":duplicate_blocked,"staleRejected":stale_blocked,
            "derivedCircularRejected":cycle_blocked,"duplicateBeliefRejected":repeated_belief,
            "expiredBeliefRejected":expired,"missingAncestryRejected":missing_blocked,
            "rejectionsDoNotMutateWorldGraph":true,"ancestryEdges":ancestry.len(),
            "worldGraphRoundTrip":true,"nodes":restored.node_count(),"edges":restored.edge_count()},
        "heuristicScore":inference.confidence,"confidenceIsCalibrated":false,
        "rawDataIncluded":false,"promotionAuthorized":false,
        "limitations":["No real hardware or ESP-Fi timestamps/labels used",
            "Bridge freshness/ancestry guards are additional policy, not native WorldGraph guarantees",
            "Simulation trust is not production sensor enrollment",
            "No model training or predictive accuracy uplift established"]})
}

fn native_once(event: &FieldEvent) {
    verify_event(event).unwrap();
    let mut fusion = RuFieldFusion::with_rules(RuleSet::from_toml(RULES).unwrap());
    fusion.ingest_at(event.clone(), NOW).unwrap();
    std::hint::black_box(fusion.infer(&InferenceQuery::all()).unwrap());
}
fn bridge_once(event: &FieldEvent) {
    let mut bridge = Bridge::new();
    bridge.ingest(event.clone(), &[], NOW).unwrap();
    let inference = bridge.inference();
    std::hint::black_box(bridge.publish(&inference, NOW).unwrap());
}
fn quantile(values: &[f64], q: f64) -> f64 {
    let mut ordered = values.to_vec();
    ordered.sort_by(f64::total_cmp);
    ordered[((ordered.len() - 1) as f64 * q).round() as usize]
}
fn benchmark() -> Value {
    let event = fixture("benchmark-root", NOW);
    for _ in 0..100 { native_once(&event); bridge_once(&event); }
    let mut native = Vec::with_capacity(1000);
    let mut bridge = Vec::with_capacity(1000);
    for i in 0..1000 {
        let mut measure = |with_bridge: bool| {
            let start = std::time::Instant::now();
            if with_bridge { bridge_once(&event); } else { native_once(&event); }
            let elapsed = start.elapsed().as_secs_f64() * 1000.;
            if with_bridge { bridge.push(elapsed); } else { native.push(elapsed); }
        };
        if i % 2 == 0 { measure(false); measure(true); } else { measure(true); measure(false); }
    }
    let differences: Vec<_> = bridge.iter().zip(&native).map(|(b,n)| b-n).collect();
    json!({"evidence":"MEASURED software timing on SYNTHETIC fixtures",
        "iterations":1000,"warmup":100,"profile":"release required for reported comparison",
        "scope":"Fresh isolated engine per call: signature verify, fixture clone, rule parsing, ingestion and inference; bridge adds guards, WorldGraph creation and ancestry publication. Excludes signing, sensing, training and persistent-stream costs.",
        "native_ms":{"p50":quantile(&native,0.5),"p95":quantile(&native,0.95)},
        "bridge_ms":{"p50":quantile(&bridge,0.5),"p95":quantile(&bridge,0.95)},
        "paired_extra_ms":{"p50":quantile(&differences,0.5),"p95":quantile(&differences,0.95)},
        "extra_p95_target_ms":0.1,"extra_p95_target_met":quantile(&differences,0.95)<0.1})
}
// Strict wrapper deliberately differs from the adapter's truncated-tail recovery.
fn inspect_text(text: &str) -> Value {
    let hash = sha256_hex(text.as_bytes());
    let lines: Vec<_> = text.lines().filter(|line| !line.trim().is_empty()).collect();
    if lines.len() > 10_000 { return json!({"accepted":false,"reason":"line_limit","sha256":hash}); }
    let mut previous_timestamp = None;
    let mut channel_width = None;
    for (index, line) in lines.iter().enumerate() {
        if serde_json::from_str::<Value>(line).is_err() {
            return json!({"accepted":false,"reason":"strict_json_parse","nonempty_line":index+1,"sha256":hash,
                "native_adapter_accepts":rufield_adapters::CsiReplayAdapter::from_jsonl(text).is_ok(),
                "calibration_performed":false,"graph_mutated":false});
        }
    }
    for line in &lines {
        let value: Value = serde_json::from_str(line).unwrap();
        let Some(timestamp) = value.get("timestamp").and_then(Value::as_f64) else {
            return json!({"accepted":false,"reason":"strict_schema","sha256":hash});
        };
        let Some(amplitudes) = value.get("subcarriers").and_then(Value::as_array) else {
            return json!({"accepted":false,"reason":"strict_schema","sha256":hash});
        };
        let nanos = timestamp * 1e9;
        if !timestamp.is_finite() || timestamp < 0. || !nanos.is_finite() || nanos >= u64::MAX as f64
            || amplitudes.is_empty() || amplitudes.len() > 2048
            || amplitudes.iter().any(|v| v.as_f64().map_or(true, |x| !x.is_finite() || x < 0. || x > f32::MAX as f64)) {
            return json!({"accepted":false,"reason":"strict_numeric_range","sha256":hash});
        }
        let timestamp_ns = nanos as u64;
        if previous_timestamp.is_some_and(|previous| timestamp_ns <= previous)
            || channel_width.is_some_and(|width| width != amplitudes.len()) {
            return json!({"accepted":false,"reason":"strict_recording_order_or_width","sha256":hash});
        }
        previous_timestamp = Some(timestamp_ns);
        channel_width = Some(amplitudes.len());
    }
    match rufield_adapters::CsiReplayAdapter::from_jsonl(text) {
        Ok(adapter) => {
            let frames = adapter.frames();
            if frames.len() != lines.len() {
                return json!({"accepted":false,"reason":"native_adapter_dropped_line","sha256":hash,
                    "nonempty_lines":lines.len(),"parsed_frames":frames.len(),
                    "calibration_performed":false,"graph_mutated":false});
            }
            let span = frames.last().unwrap().timestamp_ns.saturating_sub(frames[0].timestamp_ns);
            json!({"accepted":true,"sha256":hash,"frames":frames.len(),"timestamp_span_seconds":span as f64 / 1e9,
                "calibration_performed":false,"graph_mutated":false,"labels_inferred":false,
                "evidence":"Format-only replay parsing using actual pinned CsiReplayAdapter"})
        },
        Err(_) => json!({"accepted":false,"reason":"native_adapter_validation","sha256":hash,
            "calibration_performed":false,"graph_mutated":false})
    }
}
fn inspect_path(path: &str) -> Result<Value, Box<dyn std::error::Error>> {
    use std::io::Read;
    const MAX_BYTES: u64 = 1_048_576;
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_BYTES { return Err("regular file <= 1 MiB required".into()); }
    let mut text = String::new();
    std::fs::File::open(path)?.take(MAX_BYTES+1).read_to_string(&mut text)?;
    if text.len() as u64 > MAX_BYTES { return Err("file exceeds limit".into()); }
    Ok(inspect_text(&text))
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() == 2 && args[0] == "--inspect-jsonl" {
        match inspect_path(&args[1]) {
            Ok(report) => { println!("{}", serde_json::to_string_pretty(&report).unwrap());
                if report["accepted"] != true { std::process::exit(2); } },
            Err(_) => { eprintln!("Replay rejected: invalid, unreadable, oversized or non-UTF8 regular file"); std::process::exit(2); }
        }
        return;
    }
    let mut report = probe();
    if args == ["--benchmark"] { report["benchmark"] = benchmark(); }
    else if !args.is_empty() { eprintln!("Usage: ruview-field-context-probe [--benchmark | --inspect-jsonl PATH]"); std::process::exit(2); }
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_replay_rejects_truncated_tail_without_calibrating() {
        let valid = r#"{"timestamp":1.0,"subcarriers":[1.0,2.0]}
{"timestamp":1.1,"subcarriers":[2.0,3.0]}"#;
        let report = inspect_text(valid);
        assert_eq!(report["accepted"], true);
        assert_eq!(report["frames"], 2);
        assert_eq!(report["calibration_performed"], false);
        let truncated = format!("{valid}\n{{");
        let report = inspect_text(&truncated);
        assert_eq!(report["accepted"], false);
        assert_eq!(report["native_adapter_accepts"], true);
        assert_eq!(report["graph_mutated"], false);
        for tail in ["{}", r#"{"timestamp":"wrong-type","subcarriers":[1.0]}"#] {
            let report = inspect_text(&format!("{valid}\n{tail}"));
            assert_eq!(report["accepted"], false);
            assert_eq!(report["reason"], "strict_schema");
        }
        for bad in [
            r#"{"timestamp":1e300,"subcarriers":[1.0]}"#,
            r#"{"timestamp":1.0,"subcarriers":[1e300]}"#,
            r#"{"timestamp":1.0,"subcarriers":[-1.0]}"#,
            r#"{"timestamp":1.0,"subcarriers":[]}"#,
        ] { assert_eq!(inspect_text(bad)["accepted"], false); }
        for tail in [r#"{"timestamp":1.0,"subcarriers":[1.0,2.0]}"#,
                     r#"{"timestamp":1.2,"subcarriers":[1.0]}"#] {
            assert_eq!(inspect_text(&format!("{valid}\n{tail}"))["reason"], "strict_recording_order_or_width");
        }
        assert_eq!(inspect_text("")["accepted"], false);
    }
    #[test]
    fn actual_libraries_and_bridge_contract() {
        let result = probe();
        for key in ["duplicateRejected", "staleRejected", "derivedCircularRejected",
            "duplicateBeliefRejected", "expiredBeliefRejected", "missingAncestryRejected",
            "rejectionsDoNotMutateWorldGraph", "worldGraphRoundTrip"] {
            assert_eq!(result["bridge"][key], true, "{key}");
        }
        assert_eq!(result["native"]["duplicateRejected"], true);
        assert_eq!(result["native"]["simulationSkipsWallClockFreshness"], true);
        assert_eq!(result["native"]["productionRejectsSynthetic"], true);
        assert_eq!(result["native"]["worldGraphAllowsCycles"], true);
    }
    #[test]
    fn synthetic_replay_is_deterministic() { assert_eq!(probe(), probe()); }
    #[test]
    fn tampering_and_future_evidence_cannot_change_graph() {
        let mut bridge = Bridge::new();
        let mut event = fixture("tamper", NOW);
        event.tensor.values[0] = 999.;
        assert!(bridge.ingest(event, &[], NOW).is_err());
        assert!(bridge.ingest(fixture("future", NOW + 1), &[], NOW).is_err());
        assert_eq!(bridge.world.node_count(), 0);
    }
    #[test]
    fn capacity_and_invalid_score_fail_without_graph_mutation() {
        let mut bridge = Bridge::new();
        for i in 0..MAX_EVENTS {
            bridge.ingest(fixture(&format!("capacity-{i}"), NOW+i as u64), &[], NOW+i as u64).unwrap();
        }
        let before = bridge.world.to_json().unwrap();
        assert!(bridge.ingest(fixture("overflow", NOW+100), &[], NOW+100).is_err());
        let mut inference = bridge.inference();
        inference.confidence = f32::NAN;
        assert!(bridge.publish(&inference, NOW+100).is_err());
        assert_eq!(before, bridge.world.to_json().unwrap());
    }
    #[test]
    fn repeated_raw_hash_under_new_id_is_not_independent_evidence() {
        let mut bridge = Bridge::new();
        let event = fixture("a", NOW);
        let hash = event.provenance.raw_hash.clone();
        bridge.ingest(event, &[], NOW).unwrap();
        let mut correlated = fixture("b", NOW + 1);
        correlated.provenance.raw_hash = hash;
        Signer::from_seed(&[17; 32]).sign_event(&mut correlated).unwrap();
        assert!(bridge.ingest(correlated, &[], NOW + 1).is_err());
        assert_eq!(bridge.world.node_count(), 1);
    }
}
