//! Wire-contract tests: golden lines match the ADR-382 examples (copied
//! verbatim from WeftOS ADR-107 §7), conversions mirror their sources, and
//! invalid values never serialise.

use ruview_spatial_evidence::{
    free_space_amplitude, gaussian_to_record, map_to_records, parse_line, to_jsonl, to_line,
    EvidenceError, GaussianExport, LinkMeasurement, Motion, ProofTag, Provenance, RecordBody,
    RecordMeta,
};
use ruview_unified::gaussian::{GaussianMap, MotionState, Provenance as UProv, RfGaussian};
use serde_json::Value;

/// ADR-382 / ADR-107 §7 example lines for the two RF types, verbatim.
const ADR_RF_LINK: &str = r#"{"schema":"spatial.evidence.v1","type":"rf_link_observation","t_ns":1759500001300000000,"frame":"room_enu","region":"region/urth/meso/test-room","source_id":"node-2","uncertainty_m":1.0,"provenance":{"receipt":"csi:node1-node2:000881","producer":"ruview-adapter@0.1","proof":"MEASURED"},"tx":[6.20,3.50,1.10],"rx":[0.20,0.20,1.10],"freq_hz":2437000000.0,"excess_loss_db":4.2}"#;
const ADR_RF_GAUSSIAN: &str = r#"{"schema":"spatial.evidence.v1","type":"rf_gaussian","t_ns":1759500002000000000,"frame":"room_enu","region":"region/urth/meso/test-room","source_id":"ruview-unified","uncertainty_m":0.5,"provenance":{"receipt":"gauss:7f3a","producer":"ruview-unified@0.3","proof":"CODE"},"position":[3.2,0.0,1.2],"scale":[0.05,1.6,1.2],"orientation":[1,0,0,0],"occupancy":0.7,"confidence":0.8,"motion":"static","role":"absorber"}"#;

/// The 2026-10-03 ADR-107 amendment's mounted-sensor lines, verbatim. This
/// crate does not mirror them, so they must fail as unknown types.
const ADR_TOF_DEPTH: &str = r#"{"schema":"spatial.evidence.v1","type":"tof_depth","t_ns":1759500004000000000,"frame":"room_enu","region":"region/urth/meso/test-room","source_id":"sen0628-1","uncertainty_m":0.02,"provenance":{"receipt":"sen0628-1:000042","producer":"sen0628-tof@0.1.0","proof":"MEASURED"},"position":[4.80,0.05,1.60],"yaw_deg":90.0,"pitch_deg":-35.0,"fov_deg":[60.0,60.0],"grid":[4,4],"range_mm":[0,0,0,0,3290,3105,3110,3302,2512,1190,1185,2530,2005,1072,1066,2011],"valid":[false,false,false,false,true,true,true,true,true,true,true,true,true,true,true,true]}"#;
const ADR_RADAR_RANGE: &str = r#"{"schema":"spatial.evidence.v1","type":"radar_range","t_ns":1759500004100000000,"frame":"room_enu","region":"region/urth/meso/test-room","source_id":"rd03e-1","uncertainty_m":0.3,"provenance":{"receipt":"rd03e-1:000007","producer":"rd-03e@0.1.0","proof":"MEASURED"},"position":[0.10,1.83,1.00],"yaw_deg":0.0,"fov_deg":[90.0,60.0],"range_m":2.45,"targets":1}"#;
const ADR_RADAR_TRACK_SENSOR: &str = r#"{"schema":"spatial.evidence.v1","type":"radar_track_point","t_ns":1759500001350000000,"frame":"room_enu","region":"region/urth/meso/test-room","source_id":"ld2450-1","uncertainty_m":0.3,"provenance":{"receipt":"ld2450-1:000124","producer":"ld2450-radar@0.1.0","proof":"MEASURED"},"track":1,"position":[2.60,1.30,0.0],"sensor":{"position":[0.50,0.50,1.80],"yaw_deg":26.0,"pitch_deg":-10.0,"elev_half_deg":35.0,"az_half_deg":60.0}}"#;

const REGION: &str = "region/urth/meso/test-room";

fn meta(
    t_ns: u64,
    source: &str,
    unc: f64,
    receipt: &str,
    producer: &str,
    proof: ProofTag,
) -> RecordMeta {
    RecordMeta {
        t_ns,
        region: REGION.into(),
        source_id: source.into(),
        uncertainty_m: unc,
        provenance: Provenance {
            receipt: receipt.into(),
            producer: producer.into(),
            proof,
        },
    }
}

/// Parse JSON with every number as f64, so `1` and `1.0` compare equal.
fn json(s: &str) -> Value {
    fn norm(v: Value) -> Value {
        match v {
            Value::Number(n) => Value::from(n.as_f64().expect("finite")),
            Value::Array(a) => Value::Array(a.into_iter().map(norm).collect()),
            Value::Object(o) => Value::Object(o.into_iter().map(|(k, v)| (k, norm(v))).collect()),
            other => other,
        }
    }
    norm(serde_json::from_str(s).expect("json"))
}

fn gaussian(device: &str, synthetic: bool) -> RfGaussian {
    RfGaussian::new(
        [3.2, 0.0, 1.2],
        [0.05, 1.6, 1.2],
        [1.0, 0.0, 0.0, 0.0],
        0.7,
        0.8,
        1_759_500_002_000_000_000,
        300.0,
        UProv {
            device_id: device.into(),
            model_version: 3,
            synthetic,
        },
    )
    .expect("valid gaussian")
}

fn export(proof: ProofTag) -> GaussianExport {
    GaussianExport {
        region: REGION.into(),
        producer: "ruview-unified@0.3".into(),
        proof,
        reproducer: Some("test-repro".into()),
    }
}

#[test]
fn export_defaults_to_code() {
    let ctx = GaussianExport::new(REGION, "ruview-unified@0.3");
    assert_eq!(ctx.proof, ProofTag::Code);
    let rec = gaussian_to_record(&gaussian("esp32-node-1", false), 0, &ctx).unwrap();
    assert_eq!(rec.provenance.proof, ProofTag::Code);
}

#[test]
fn measured_requires_a_non_empty_reproducer() {
    let g = gaussian("esp32-node-1", false);
    let mut ctx = GaussianExport::new(REGION, "p@0.1");
    ctx.proof = ProofTag::Measured;
    for bad in [None, Some(String::new()), Some("  ".to_string())] {
        ctx.reproducer = bad;
        assert_eq!(
            gaussian_to_record(&g, 0, &ctx),
            Err(EvidenceError::MissingReproducer)
        );
    }
    let ok = gaussian_to_record(&g, 0, &GaussianExport::new(REGION, "p@0.1").measured("run-42"))
        .unwrap();
    assert_eq!(ok.provenance.proof, ProofTag::Measured);
}

#[test]
fn synthetic_gaussian_needs_no_reproducer() {
    let mut ctx = GaussianExport::new(REGION, "p@0.1");
    ctx.proof = ProofTag::Measured;
    let rec = gaussian_to_record(&gaussian("sim", true), 0, &ctx).unwrap();
    assert_eq!(rec.provenance.proof, ProofTag::Synthetic);
}

#[test]
fn adr_examples_parse_and_reserialise_to_the_same_shape() {
    for line in [ADR_RF_LINK, ADR_RF_GAUSSIAN] {
        let rec = parse_line(line).expect("ADR example parses");
        assert_eq!(json(&to_line(&rec).unwrap()), json(line));
    }
}

#[test]
fn golden_rf_link_line_matches_adr_example() {
    let m = LinkMeasurement {
        tx: [6.20, 3.50, 1.10],
        rx: [0.20, 0.20, 1.10],
        freq_hz: 2_437_000_000.0,
        measured_amplitude: 1.0,
        free_space_amplitude: 10f64.powf(4.2 / 20.0),
    };
    let rec = m
        .to_record(meta(
            1_759_500_001_300_000_000,
            "node-2",
            1.0,
            "csi:node1-node2:000881",
            "ruview-adapter@0.1",
            ProofTag::Measured,
        ))
        .unwrap();
    let mut got = json(&to_line(&rec).unwrap());
    // Float round-off in 20·log10(10^(4.2/20)): compare that field numerically.
    let loss = got["excess_loss_db"].as_f64().unwrap();
    assert!((loss - 4.2).abs() < 1e-12);
    got["excess_loss_db"] = json("4.2");
    assert_eq!(got, json(ADR_RF_LINK));
}

#[test]
fn golden_rf_gaussian_line_matches_adr_example_except_role() {
    let rec = gaussian_to_record(
        &gaussian("ruview-unified", false),
        0,
        &export(ProofTag::Code),
    )
    .unwrap();
    let line = to_line(&rec).unwrap();
    assert_eq!(
        line,
        r#"{"schema":"spatial.evidence.v1","t_ns":1759500002000000000,"frame":"room_enu","region":"region/urth/meso/test-room","source_id":"ruview-unified","uncertainty_m":0.45788569702133275,"provenance":{"receipt":"gauss:ruview-unified:1759500002000000000:0","producer":"ruview-unified@0.3","proof":"CODE"},"type":"rf_gaussian","position":[3.2,0.0,1.2],"scale":[0.05,1.6,1.2],"orientation":[1.0,0.0,0.0,0.0],"occupancy":0.7,"confidence":0.8,"motion":"static"}"#
    );
    // Same field set as the ADR example once the export-only `role` and
    // the per-record receipt/uncertainty are aligned.
    let mut got = json(&line);
    let want = json(ADR_RF_GAUSSIAN);
    got["role"] = want["role"].clone();
    got["provenance"]["receipt"] = want["provenance"]["receipt"].clone();
    got["uncertainty_m"] = want["uncertainty_m"].clone();
    assert_eq!(got, want);
}

#[test]
fn synthetic_gaussian_cannot_be_raised_to_measured() {
    let rec = gaussian_to_record(&gaussian("sim", true), 0, &export(ProofTag::Measured)).unwrap();
    assert_eq!(rec.provenance.proof, ProofTag::Synthetic);
    let rec =
        gaussian_to_record(&gaussian("esp32-node-1", false), 0, &export(ProofTag::Code)).unwrap();
    assert_eq!(rec.provenance.proof, ProofTag::Code);
}

#[test]
fn device_id_is_sanitised_into_a_valid_source_id() {
    let rec = gaussian_to_record(
        &gaussian("node 1 (kitchen)", false),
        4,
        &export(ProofTag::Code),
    )
    .unwrap();
    assert_eq!(rec.source_id, "node-1--kitchen-");
    let rec = gaussian_to_record(&gaussian("", false), 0, &export(ProofTag::Code)).unwrap();
    assert_eq!(rec.source_id, "ruview-unified");
    assert!(to_line(&rec).is_ok());
}

#[test]
fn map_export_mirrors_every_gaussian_and_motion_class() {
    let mut map = GaussianMap::new(1.0);
    let mut moving = gaussian("node-5", false);
    moving.position = [1.0, 1.0, 1.0];
    moving.scale = [0.3, 0.3, 0.9];
    moving.motion = MotionState::Fast;
    map.insert(gaussian("node-1", false));
    map.insert(moving);
    let recs = map_to_records(&map, &export(ProofTag::Code)).unwrap();
    assert_eq!(recs.len(), map.len());
    for (rec, g) in recs.iter().zip(map.gaussians()) {
        let RecordBody::RfGaussian(body) = &rec.body else {
            panic!("wrong type")
        };
        assert_eq!(body.position, g.position);
        assert_eq!(body.scale, g.scale);
        assert_eq!(body.orientation, g.orientation);
        assert_eq!(rec.t_ns, g.timestamp_ns);
    }
    assert!(recs
        .iter()
        .any(|r| matches!(&r.body, RecordBody::RfGaussian(b) if b.motion == Motion::Fast)));
    // Deterministic, newline-terminated JSONL that parses back line by line.
    let doc = to_jsonl(&recs).unwrap();
    assert_eq!(
        doc,
        to_jsonl(&map_to_records(&map, &export(ProofTag::Code)).unwrap()).unwrap()
    );
    // serde_json's default float parse may differ by one ulp, so compare
    // the uncertainty numerically and everything else exactly.
    let back: Vec<_> = doc.lines().map(|l| parse_line(l).unwrap()).collect();
    assert_eq!(back.len(), recs.len());
    for (b, r) in back.iter().zip(&recs) {
        assert!((b.uncertainty_m - r.uncertainty_m).abs() < 1e-12);
        assert_eq!(
            (&b.body, &b.provenance, b.t_ns),
            (&r.body, &r.provenance, r.t_ns)
        );
    }
}

#[test]
fn friis_reference_and_excess_loss() {
    let (tx, rx, f) = ([0.4, 0.4, 0.9], [6.2, 3.46, 0.91], 2.437e9);
    let d = ((5.8f64).powi(2) + 3.06f64.powi(2) + 0.01f64.powi(2)).sqrt();
    let want = 299_792_458.0 / f / (4.0 * std::f64::consts::PI * d);
    let free = free_space_amplitude(tx, rx, f).unwrap();
    assert!((free - want).abs() / want < 1e-9);
    assert_eq!(free_space_amplitude(tx, tx, f), None);
    assert_eq!(free_space_amplitude(tx, rx, f64::NAN), None);

    let at_free = LinkMeasurement::against_friis(tx, rx, f, free).unwrap();
    assert!(at_free.excess_loss_db().unwrap().abs() < 1e-12);
    let halved = LinkMeasurement::against_friis(tx, rx, f, free / 2.0).unwrap();
    assert!((halved.excess_loss_db().unwrap() - 6.020_599_913).abs() < 1e-6);
    let stronger = LinkMeasurement::against_friis(tx, rx, f, free * 2.0).unwrap();
    assert!(stronger.excess_loss_db().unwrap() < 0.0);
}

#[test]
fn bad_link_amplitudes_and_out_of_range_loss_are_rejected() {
    let base = LinkMeasurement {
        tx: [0.0; 3],
        rx: [1.0, 0.0, 0.0],
        freq_hz: 2.437e9,
        measured_amplitude: 1.0,
        free_space_amplitude: 1.0,
    };
    let m = |measured: f64, free: f64| LinkMeasurement {
        measured_amplitude: measured,
        free_space_amplitude: free,
        ..base.clone()
    };
    let mk = || meta(1, "node-1", 1.0, "r-1", "p@0.1", ProofTag::Code);
    assert_eq!(
        m(0.0, 1.0).excess_loss_db(),
        Err(EvidenceError::OutOfRange("measured_amplitude"))
    );
    assert_eq!(
        m(f64::NAN, 1.0).excess_loss_db(),
        Err(EvidenceError::NonFinite("measured_amplitude"))
    );
    assert_eq!(
        m(1.0, -1.0).excess_loss_db(),
        Err(EvidenceError::OutOfRange("free_space_amplitude"))
    );
    // 1e-11 below free space is 220 dB: outside the v1 range [-60, 200].
    assert_eq!(
        m(1e-11, 1.0).to_record(mk()),
        Err(EvidenceError::OutOfRange("excess_loss_db"))
    );
    let far = LinkMeasurement {
        tx: [0.0, 0.0, 2000.0],
        ..base.clone()
    };
    assert_eq!(far.to_record(mk()), Err(EvidenceError::OutOfRange("tx")));
    let low = LinkMeasurement {
        freq_hz: 1e6,
        ..base
    };
    assert_eq!(
        low.to_record(mk()),
        Err(EvidenceError::OutOfRange("freq_hz"))
    );
}

#[test]
fn envelope_rules_match_v1() {
    let link = LinkMeasurement {
        tx: [0.0; 3],
        rx: [1.0, 0.0, 0.0],
        freq_hz: 2.437e9,
        measured_amplitude: 1.0,
        free_space_amplitude: 1.0,
    };
    let rec = |src: &str, unc: f64, receipt: &str| {
        link.to_record(meta(1, src, unc, receipt, "p@0.1", ProofTag::Code))
    };
    assert_eq!(
        rec("node 1", 1.0, "r"),
        Err(EvidenceError::BadText("source_id"))
    );
    assert_eq!(rec("", 1.0, "r"), Err(EvidenceError::BadText("source_id")));
    assert_eq!(
        rec(&"a".repeat(129), 1.0, "r"),
        Err(EvidenceError::BadText("source_id"))
    );
    assert_eq!(
        rec("n", 0.0, "r"),
        Err(EvidenceError::OutOfRange("uncertainty_m"))
    );
    assert_eq!(
        rec("n", 100.5, "r"),
        Err(EvidenceError::OutOfRange("uncertainty_m"))
    );
    assert_eq!(
        rec("n", f64::INFINITY, "r"),
        Err(EvidenceError::NonFinite("uncertainty_m"))
    );
    assert_eq!(
        rec("n", 1.0, "bad\nreceipt"),
        Err(EvidenceError::BadText("provenance.receipt"))
    );
}

#[test]
fn parse_line_rejects_wrong_version_type_overflow_and_length() {
    let v2 = ADR_RF_LINK.replace("spatial.evidence.v1", "spatial.evidence.v2");
    assert_eq!(
        parse_line(&v2),
        Err(EvidenceError::UnknownVersion("spatial.evidence.v2".into()))
    );
    let unschema = ADR_RF_LINK.replace(r#""schema":"spatial.evidence.v1","#, "");
    assert_eq!(
        parse_line(&unschema),
        Err(EvidenceError::UnknownVersion(String::new()))
    );
    let unknown = ADR_RF_LINK.replace("rf_link_observation", "rf_teleport");
    assert!(matches!(parse_line(&unknown), Err(EvidenceError::Parse(_))));
    let overflow = ADR_RF_LINK.replace("4.2}", "1e999}");
    assert!(matches!(
        parse_line(&overflow),
        Err(EvidenceError::Parse(_))
    ));
    let bad_quat = ADR_RF_GAUSSIAN.replace("[1,0,0,0]", "[0,0,0,0]");
    assert_eq!(
        parse_line(&bad_quat),
        Err(EvidenceError::OutOfRange("orientation"))
    );
    let bad_conf = ADR_RF_GAUSSIAN.replace("\"confidence\":0.8", "\"confidence\":1.5");
    assert_eq!(
        parse_line(&bad_conf),
        Err(EvidenceError::OutOfRange("confidence"))
    );
    let long = format!("{}{}", ADR_RF_LINK, " ".repeat(16 * 1024));
    assert!(matches!(
        parse_line(&long),
        Err(EvidenceError::LineTooLong(_))
    ));
}

#[test]
fn non_rf_types_are_unknown_to_this_emitter() {
    for line in [ADR_TOF_DEPTH, ADR_RADAR_RANGE, ADR_RADAR_TRACK_SENSOR] {
        let err = parse_line(line).expect_err("not mirrored here");
        let EvidenceError::Parse(msg) = err else {
            panic!("expected an unknown-type parse error, got {err:?}")
        };
        assert!(msg.contains("unknown variant"), "{msg}");
    }
}

#[test]
fn gaussian_orientation_follows_the_v1_yaw_convention() {
    // v1 yaw is counter-clockwise from room +x. A quaternion for +90° about
    // +z must therefore turn the Gaussian's local x axis onto room +y, and
    // reach the wire unchanged.
    let h = std::f64::consts::FRAC_PI_4;
    let mut g = gaussian("node-1", false);
    g.orientation = [h.cos(), 0.0, 0.0, h.sin()];
    let r = g.rotation();
    let local_x = [r[0][0], r[1][0], r[2][0]];
    for (got, want) in local_x.iter().zip([0.0, 1.0, 0.0]) {
        assert!((got - want).abs() < 1e-12, "{local_x:?}");
    }
    let rec = gaussian_to_record(&g, 0, &export(ProofTag::Code)).unwrap();
    let RecordBody::RfGaussian(body) = &rec.body else {
        panic!("wrong type")
    };
    assert_eq!(body.orientation, g.orientation);
}
