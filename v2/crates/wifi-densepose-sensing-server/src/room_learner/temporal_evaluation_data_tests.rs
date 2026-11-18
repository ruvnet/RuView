//! Synthetic metadata fixtures only. None creates a runtime/fit/Complete permit.
use super::*;
use serde_json::{json, Value};
fn fixture_sha(n: u64) -> String {
    format!("{n:064x}")
}
fn claims() -> DiagnosticClaimsData {
    DiagnosticClaimsData {
        independence_verified: false,
        physically_validated: false,
        qualification_granted: false,
        adoption_authorized: false,
        measurement_clock_available: false,
    }
}
fn identity() -> TemporalEvaluationIdentityData {
    TemporalEvaluationIdentityData {
        owner_scope: "owner-a".into(),
        installation_id: "installation-a".into(),
        context_digest: fixture_sha(1),
        room_digest: fixture_sha(2),
        run: "run-a".into(),
        original_deadline: OriginalDeadlineData {
            id: "recovery-a".into(),
            created_at_unix_ms: 1_800_000_000_000,
            expires_at_unix_ms: 1_800_086_400_000,
        },
        artifact_kind: MODEL_KIND.into(),
        artifact_sha256: fixture_sha(3),
        verified_fit_record_sha256: fixture_sha(4),
        prefix_schema: PREFIX_SCHEMA.into(),
        metadata_kind: METADATA_KIND.into(),
        continuation_policy_sha256: fixture_sha(5),
        input_policy_sha256: fixture_sha(6),
        execution_policy_sha256: fixture_sha(7),
        schedule_sha256: fixture_sha(8),
        configured_fit_sha256: fixture_sha(9),
        source_ids: vec![15, 16, 17],
    }
}
fn token(id: &TemporalEvaluationIdentityData, revision: u16) -> CommitmentData {
    CommitmentData {
        generation: 0,
        revision,
        run: id.run.clone(),
        id: id.original_deadline.id.clone(),
        commitment_sha256: fixture_sha(u64::from(revision) + 100),
    }
}
fn train() -> TemporalTrainProofData {
    let id = identity();
    let cut = "1800000900000000000".to_owned();
    let body = TemporalTrainProofBodyData {
        accepted_train_commitment: token(&id, 78),
        source_floors: (0..3).map(|i| FrozenSourceFloorData {
            source_id: id.source_ids[i],
            sequence: 10_000,
            receiver_ns: "1800000899999000000".into(),
        }).collect(),
        identity: id,
        training_data_sha256: fixture_sha(20),
        training_evidence_sha256: fixture_sha(21),
        training_identities_sha256: fixture_sha(22),
        train_seal_sha256s: (0..7).map(|i| fixture_sha(200 + i)).collect(),
        train_seal_chain_sha256: fixture_sha(23),
        train_windows: 64,
        windows_per_condition: [16; 4],
        declared_episodes_per_condition: [1, 2, 2, 2],
        freeze_prepare_receiver_ns: cut,
        claims: claims(),
    };
    TemporalTrainProofData {
        schema: TRAIN_SCHEMA.into(),
        sha256: data_sha(TRAIN_SCHEMA, &body).unwrap(),
        body,
    }
}
fn held(t: &TemporalTrainProofData) -> TemporalHeldOutEvidenceData {
    let body = TemporalHeldOutBodyData {
        identity: t.body.identity.clone(),
        training_proof_sha256: t.sha256.clone(),
        held_out_data_sha256: fixture_sha(30),
        held_out_evidence_sha256: fixture_sha(31),
        held_out_identities_sha256: fixture_sha(32),
        complete_rows_sha256: fixture_sha(33),
        complete_evidence_sha256: fixture_sha(34),
        complete_identities_sha256: fixture_sha(35),
        complete_ledger_sha256: fixture_sha(36),
        held_out_seal_sha256s: (0..8).map(|i| fixture_sha(300 + i)).collect(),
        held_out_seal_chain_sha256: fixture_sha(37),
        complete_record_sha256: fixture_sha(38),
        complete_commitment: token(&t.body.identity, 160),
        complete_prepare_receiver_ns: "1800001300000000000".into(),
        complete_persisted_receiver_ns: "1800001300000000000".into(),
        final_source_fences: (0..3).map(|i| {
            let id = t.body.identity.source_ids[i];
            let ns = 1_800_001_299_999_000_000u64;
            FinalSourceFenceData {
                source_id: id,
                sequence: 20_000,
                receiver_ns: ns.to_string(),
                native_frame_id: ((u128::from(ns) << 40) | (u128::from(id) << 32) | 20_000)
                    .to_string(),
            }
        }).collect(),
        held_out_windows: 64,
        windows_per_condition: [16; 4],
        declared_episodes_per_condition: [2; 4],
        claims: claims(),
    };
    TemporalHeldOutEvidenceData {
        schema: HELD_OUT_SCHEMA.into(),
        sha256: data_sha(HELD_OUT_SCHEMA, &body).unwrap(),
        body,
    }
}
fn rates(sensitivity: Option<f32>, specificity: Option<f32>) -> Value {
    json!({"evaluated_cohort_coverage":1.0,"unconditional_accuracy":1.0,"scored_accuracy":1.0,"unconditional_sensitivity":sensitivity,"scored_sensitivity":sensitivity,"unconditional_specificity":specificity,"scored_specificity":specificity})
}
fn confusion(empty: usize, occupied: usize) -> Value {
    json!({"true_empty":empty,"false_occupied":0,"false_empty":0,"true_occupied":occupied,"abstained_empty":0,"abstained_occupied":0,"missing_empty":0,"missing_occupied":0})
}
fn report() -> EvaluationReport {
    let labels = ["Empty", "OccupiedStatic", "OccupiedMoving", "AdjacentOnly"];
    let per:Vec<Value>=labels.into_iter().map(|label|{let occupied=label.starts_with("Occupied");json!({"condition":label,"samples":16,"correct":16,"abstained":0,"scored":16,"missing":0,"binary_cross_entropy":0.1,"binary_cross_entropy_samples":16,"declared_trial_count":2,"declared_session_count":2,"confusion":confusion(if occupied{0}else{16},if occupied{16}else{0}),"rates":rates(occupied.then_some(1.0),(!occupied).then_some(1.0))})}).collect();
    serde_json::from_value(json!({"samples":64,"correct":64,"abstained":0,"binary_cross_entropy":0.1,"binary_cross_entropy_samples":64,"scored":64,"missing":0,"declared_trial_count":8,"declared_session_count":8,"confusion":confusion(32,32),"rates":rates(Some(1.0),Some(1.0)),"per_condition":per,"independence_verified":false})).unwrap()
}
fn evaluation(
    t: &TemporalTrainProofData,
    h: &TemporalHeldOutEvidenceData,
) -> TemporalEvaluationReceiptData {
    let body = TemporalEvaluationBodyData {
        identity: t.body.identity.clone(),
        training_proof_sha256: t.sha256.clone(),
        held_out_evidence_sha256: h.sha256.clone(),
        complete_record_sha256: h.body.complete_record_sha256.clone(),
        training_data_sha256: t.body.training_data_sha256.clone(),
        held_out_data_sha256: h.body.held_out_data_sha256.clone(),
        role: DataRole::HeldOut,
        window_unit: "native_receiver_2s_window".into(),
        evaluated_at_receiver_ns: "1800001301000000000".into(),
        evaluated_at_unix_ms: 1_800_001_301_000,
        provenance: vec![CaptureProvenance::LiveReceiverObserved],
        clock_claims: vec![ClockKind::ReceiverOnly],
        train_lineage_checked: true,
        report: report(),
        claims: claims(),
    };
    TemporalEvaluationReceiptData {
        schema: EVALUATION_SCHEMA.into(),
        sha256: data_sha(EVALUATION_SCHEMA, &body).unwrap(),
        body,
    }
}
fn rehash_train(t: &mut TemporalTrainProofData) {
    t.sha256 = data_sha(TRAIN_SCHEMA, &t.body).unwrap();
}
fn rehash_held(h: &mut TemporalHeldOutEvidenceData) {
    h.sha256 = data_sha(HELD_OUT_SCHEMA, &h.body).unwrap();
}
fn rehash_eval(e: &mut TemporalEvaluationReceiptData) {
    e.sha256 = data_sha(EVALUATION_SCHEMA, &e.body).unwrap();
}
#[test]
fn three_canonical_data_records_roundtrip_without_authority() {
    let t = train();
    let h = held(&t);
    let e = evaluation(&t, &h);
    let tb = serde_json::to_vec(&t).unwrap();
    let hb = serde_json::to_vec(&h).unwrap();
    let eb = serde_json::to_vec(&e).unwrap();
    assert_eq!(TemporalTrainProofData::decode_data(&tb).unwrap(), t);
    assert_eq!(TemporalHeldOutEvidenceData::decode_data(&hb).unwrap(), h);
    assert_eq!(TemporalEvaluationReceiptData::decode_data(&eb).unwrap(), e);
    e.validate_data_links(&t, &h).unwrap();
    assert!(!e.body.claims.adoption_authorized);
    assert!(!e.body.claims.qualification_granted);
}
#[test]
fn unknown_inner_identity_and_mutable_descriptor_fields_are_rejected() {
    let t = train();
    let h = held(&t);
    let e = evaluation(&t, &h);
    for key in ["model_sha256", "model_version", "adoption_authorized"] {
        let mut value = serde_json::to_value(&e).unwrap();
        value["body"]["report"][key] = json!("untrusted-inner-identity");
        assert!(
            TemporalEvaluationReceiptData::decode_data(&serde_json::to_vec(&value).unwrap())
                .is_err()
        );
    }
    let mut value = serde_json::to_value(&t).unwrap();
    value["body"]["identity"]["original_deadline"]["saved_at_ms"] = json!(1_800_000_500_000u64);
    assert!(TemporalTrainProofData::decode_data(&serde_json::to_vec(&value).unwrap()).is_err());
}
#[test]
fn canonical_encoding_size_digest_and_domain_near_misses_fail() {
    let t = train();
    let bytes = serde_json::to_vec(&t).unwrap();
    assert!(TemporalTrainProofData::decode_data(&[bytes.clone(), vec![b' ']].concat()).is_err());
    assert!(TemporalTrainProofData::decode_data(&vec![b' '; MAX_METADATA_BYTES + 1]).is_err());
    let mut bad = t.clone();
    bad.sha256 = fixture_sha(999);
    assert!(bad.validate_data().is_err());
    bad = t.clone();
    bad.schema = HELD_OUT_SCHEMA.into();
    bad.sha256 = data_sha(&bad.schema, &bad.body).unwrap();
    assert!(bad.validate_data().is_err());
}
#[test]
fn rehashed_positive_claims_roles_fake_cohort_counts_and_wrong_kind_fail() {
    let t = train();
    let h = held(&t);
    let e = evaluation(&t, &h);
    for i in 0..5 {
        let mut bad = t.clone();
        match i {
            0 => bad.body.claims.adoption_authorized = true,
            1 => bad.body.claims.qualification_granted = true,
            2 => bad.body.claims.independence_verified = true,
            3 => bad.body.claims.physically_validated = true,
            _ => bad.body.claims.measurement_clock_available = true,
        };
        rehash_train(&mut bad);
        assert!(bad.validate_data().is_err());
    }
    let mut bad = t.clone();
    bad.body.windows_per_condition = [4; 4];
    rehash_train(&mut bad);
    assert!(bad.validate_data().is_err());
    bad = t.clone();
    bad.body.identity.artifact_kind = "ruview.room-model.v1".into();
    rehash_train(&mut bad);
    assert!(bad.validate_data().is_err());
    let mut badh = h.clone();
    badh.body.declared_episodes_per_condition = [16; 4];
    rehash_held(&mut badh);
    assert!(badh.validate_data().is_err());
    let mut bade = e.clone();
    bade.body.role = DataRole::Inference;
    rehash_eval(&mut bade);
    assert!(bade.validate_data().is_err());
    bade = e.clone();
    bade.body.clock_claims = vec![ClockKind::SourceMeasurement];
    rehash_eval(&mut bade);
    assert!(bade.validate_data().is_err());
    bade = e.clone();
    bade.body.report.declared_trial_count = 64;
    rehash_eval(&mut bade);
    assert!(bade.validate_data().is_err());
}
#[test]
fn deadline_clock_floor_and_canonical_integer_checks_do_not_rebase() {
    let t = train();
    let h = held(&t);
    let e = evaluation(&t, &h);
    for cut in [
        "01800000900000000000",
        "+1800000900000000000",
        "18446744073709551616",
        "1800086400000000000",
        "1799999999999999999",
    ] {
        let mut bad = t.clone();
        bad.body.freeze_prepare_receiver_ns = cut.into();
        rehash_train(&mut bad);
        assert!(bad.validate_data().is_err(), "{cut}");
    }
    let mut bad = t.clone();
    bad.body.identity.original_deadline.expires_at_unix_ms += 1;
    rehash_train(&mut bad);
    assert!(bad.validate_data().is_err());
    bad = t.clone();
    bad.body.source_floors[0].receiver_ns = "1800000900000000001".into();
    rehash_train(&mut bad);
    assert!(bad.validate_data().is_err());
    let mut bade = e.clone();
    bade.body.evaluated_at_unix_ms += 1;
    rehash_eval(&mut bade);
    assert!(bade.validate_data().is_err());
    let mut badh = h.clone();
    badh.body.complete_prepare_receiver_ns = "1800000800000000000".into();
    rehash_held(&mut badh);
    assert!(badh.validate_data_links(&t).is_err());
}
#[test]
fn rehashed_cross_record_owner_model_policy_dataset_and_revision_mismatches_fail() {
    let t = train();
    let h = held(&t);
    let e = evaluation(&t, &h);
    for i in 0..5 {
        let mut bad = h.clone();
        match i {
            0 => bad.body.identity.owner_scope = "owner-b".into(),
            1 => bad.body.identity.artifact_sha256 = fixture_sha(999),
            2 => bad.body.identity.continuation_policy_sha256 = fixture_sha(998),
            3 => bad.body.training_proof_sha256 = fixture_sha(997),
            _ => bad.body.complete_commitment.revision = 1,
        };
        rehash_held(&mut bad);
        assert!(bad.validate_data_links(&t).is_err());
    }
    let mut bad = e.clone();
    bad.body.held_out_data_sha256 = fixture_sha(999);
    rehash_eval(&mut bad);
    assert!(bad.validate_data_links(&t, &h).is_err());
    bad = e.clone();
    bad.body.complete_record_sha256 = fixture_sha(998);
    rehash_eval(&mut bad);
    assert!(bad.validate_data_links(&t, &h).is_err());
}
#[test]
fn train_data_identity_does_not_follow_heldout_progress_or_evaluation_time() {
    let t = train();
    let original = serde_json::to_vec(&t).unwrap();
    let mut h = held(&t);
    let mut e = evaluation(&t, &h);
    h.body.complete_commitment.revision += 1;
    h.body.complete_record_sha256 = fixture_sha(777);
    rehash_held(&mut h);
    e.body.held_out_evidence_sha256 = h.sha256.clone();
    e.body.complete_record_sha256 = h.body.complete_record_sha256.clone();
    e.body.evaluated_at_unix_ms += 1;
    e.body.evaluated_at_receiver_ns = "1800001301001000000".into();
    rehash_eval(&mut e);
    e.validate_data_links(&t, &h).unwrap();
    assert_eq!(original, serde_json::to_vec(&t).unwrap());
}
#[test]
fn bounded_max_width_metadata_fixture_fits_without_relaxing_reader_limit() {
    let mut t = train();
    let id = &mut t.body.identity;
    let escaped = "\"".repeat(256);
    id.owner_scope = escaped.clone();
    id.installation_id = escaped.clone();
    id.run = escaped.clone();
    id.original_deadline.id = escaped.clone();
    id.source_ids = vec![253, 254, 255];
    id.original_deadline.expires_at_unix_ms = u64::MAX / 1_000_000 + 1;
    id.original_deadline.created_at_unix_ms = id.original_deadline.expires_at_unix_ms - TTL_MS;
    t.body.freeze_prepare_receiver_ns = (u64::MAX - 1_000_000_000_000).to_string();
    t.body.accepted_train_commitment.run = escaped.clone();
    t.body.accepted_train_commitment.id = escaped;
    t.body.accepted_train_commitment.generation = u64::MAX;
    t.body.accepted_train_commitment.revision = 430;
    for (floor, id) in t
        .body
        .source_floors
        .iter_mut()
        .zip(t.body.identity.source_ids.iter().copied())
    {
        floor.source_id = id;
        floor.sequence = u32::MAX as u64 - 100_000;
        floor.receiver_ns = (u64::MAX - 1_001_000_000_000).to_string();
    }
    rehash_train(&mut t);
    let mut h = held(&t);
    h.body.complete_commitment.generation = u64::MAX;
    h.body.complete_commitment.revision = 432;
    h.body.complete_prepare_receiver_ns = (u64::MAX - 1_000_000_000).to_string();
    h.body.complete_persisted_receiver_ns = h.body.complete_prepare_receiver_ns.clone();
    for floor in &mut h.body.final_source_fences {
        let ns = u64::MAX - 1_000_000_001;
        floor.sequence = u32::MAX as u64;
        floor.receiver_ns = ns.to_string();
        floor.native_frame_id = ((u128::from(ns) << 40)
            | (u128::from(floor.source_id) << 32)
            | u128::from(floor.sequence))
        .to_string();
    }
    rehash_held(&mut h);
    let mut e = evaluation(&t, &h);
    e.body.evaluated_at_receiver_ns = (u64::MAX - 1).to_string();
    e.body.evaluated_at_unix_ms = (u64::MAX - 1) / 1_000_000;
    for condition in &mut e.body.report.per_condition {
        condition.binary_cross_entropy = Some(1.234_567_8e-30);
        condition.declared_session_count = 16;
    }
    e.body.report.binary_cross_entropy = Some(1.234_567_8e-30);
    e.body.report.declared_session_count = 64;
    rehash_eval(&mut e);
    e.validate_data_links(&t, &h).unwrap();
    let sizes = [
        serde_json::to_vec(&t).unwrap().len(),
        serde_json::to_vec(&h).unwrap().len(),
        serde_json::to_vec(&e).unwrap().len(),
    ];
    assert!(sizes.iter().all(|n| *n < MAX_METADATA_BYTES));
    eprintln!("SYNTHETIC bounded max-width DATA bytes={sizes:?}; not a raw128 recovery-size or performance proof");
}

#[test]
fn final_complete_source_fences_are_exact_bounded_native_metadata_not_a_live_permit() {
    let t = train();
    let h = held(&t);
    h.validate_data_links(&t).unwrap();
    for case in 0..9 {
        let mut bad = h.clone();
        match case {
            0 => bad.body.final_source_fences[0].source_id = 99,
            1 => bad.body.final_source_fences.swap(0, 1),
            2 => bad.body.final_source_fences[0].sequence = u32::MAX as u64 + 1,
            3 => bad.body.final_source_fences[0].receiver_ns = "01800001299999000000".into(),
            4 => bad.body.final_source_fences[0].native_frame_id = "01".into(),
            5 => {
                bad.body.final_source_fences[0].native_frame_id =
                    "340282366920938463463374607431768211456".into()
            }
            6 => bad.body.final_source_fences[0].native_frame_id = "999".into(),
            7 => bad.body.complete_persisted_receiver_ns = "1800001299000000000".into(),
            _ => bad.body.complete_persisted_receiver_ns = "1800001300000000001".into(),
        }
        rehash_held(&mut bad);
        assert!(bad.validate_data().is_err(), "case{case}");
    }
    let mut lower = h.clone();
    let floor = &mut lower.body.final_source_fences[0];
    floor.sequence = t.body.source_floors[0].sequence - 1;
    let ns = decimal(&floor.receiver_ns).unwrap();
    floor.native_frame_id =
        ((u128::from(ns) << 40) | (u128::from(floor.source_id) << 32) | u128::from(floor.sequence))
            .to_string();
    rehash_held(&mut lower);
    assert!(lower.validate_data_links(&t).is_err());
    let mut value = serde_json::to_value(&h).unwrap();
    value["body"]["final_source_fences"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(
        TemporalHeldOutEvidenceData::decode_data(&serde_json::to_vec(&value).unwrap()).is_err()
    );
    let mut value = serde_json::to_value(&h).unwrap();
    value["body"]["final_source_fences"][0]["sequence"] = json!("20000");
    assert!(
        TemporalHeldOutEvidenceData::decode_data(&serde_json::to_vec(&value).unwrap()).is_err()
    );
    assert!(!h.body.claims.adoption_authorized);
}


fn operator_records() -> (TemporalTrainProofData, TemporalHeldOutEvidenceData, TemporalEvaluationReceiptData) {
    let mut tb = train().body;
    tb.train_seal_sha256s.truncate(4);
    tb.declared_episodes_per_condition = [1; 4];
    let t = TemporalTrainProofData::from_body_data(tb).unwrap();
    let mut hb = held(&t).body;
    hb.held_out_seal_sha256s.truncate(4);
    hb.held_out_windows = 16;
    hb.windows_per_condition = [4; 4];
    hb.declared_episodes_per_condition = [1; 4];
    let h = TemporalHeldOutEvidenceData::from_body_data(hb).unwrap();
    let mut eb = evaluation(&t, &h).body;
    eb.role = DataRole::Validation;
    let mut report = serde_json::to_value(&eb.report).unwrap();
    for key in ["samples", "correct", "scored", "binary_cross_entropy_samples"] { report[key] = json!(16); }
    for key in ["declared_trial_count", "declared_session_count"] { report[key] = json!(4); }
    report["confusion"] = confusion(8, 8);
    for c in report["per_condition"].as_array_mut().unwrap() {
        for key in ["samples", "correct", "scored", "binary_cross_entropy_samples"] { c[key] = json!(4); }
        for key in ["declared_trial_count", "declared_session_count"] { c[key] = json!(1); }
        let occupied = c["condition"].as_str().unwrap().starts_with("Occupied");
        c["confusion"] = confusion(if occupied { 0 } else { 4 }, if occupied { 4 } else { 0 });
    }
    eb.report = serde_json::from_value(report).unwrap();
    let e = TemporalEvaluationReceiptData::from_body_data(eb).unwrap();
    (t, h, e)
}

#[test]
fn operator_eight_step_validation_metadata_roundtrips_without_heldout_claims() {
    let (t, h, e) = operator_records();
    assert_eq!(t.schema, OPERATOR_TRAIN_SCHEMA);
    assert_eq!(h.schema, OPERATOR_CHECK_SCHEMA);
    assert_eq!(e.schema, OPERATOR_EVALUATION_SCHEMA);
    assert_eq!(e.body.role, DataRole::Validation);
    assert_eq!(e.body.report.samples, 16);
    assert!(!e.body.report.independence_verified);
    e.validate_data_links(&t, &h).unwrap();
    assert_eq!(TemporalTrainProofData::decode_data(&t.encode_data().unwrap()).unwrap(), t);
    assert_eq!(TemporalHeldOutEvidenceData::decode_data(&h.encode_data().unwrap()).unwrap(), h);
    assert_eq!(TemporalEvaluationReceiptData::decode_data(&e.encode_data().unwrap()).unwrap(), e);
}

#[test]
fn operator_and_legacy_metadata_cannot_be_relabelled_or_cross_linked_after_rehash() {
    let (t, h, e) = operator_records();
    let mut old_name = t.clone(); old_name.schema = TRAIN_SCHEMA.into(); old_name.sha256 = data_sha(&old_name.schema, &old_name.body).unwrap();
    assert!(old_name.validate_data().is_err());
    let mut invented = train(); invented.schema = OPERATOR_TRAIN_SCHEMA.into(); invented.sha256 = data_sha(&invented.schema, &invented.body).unwrap();
    assert!(invented.validate_data().is_err());
    let mut wrong_role = e.clone(); wrong_role.body.role = DataRole::HeldOut; wrong_role.sha256 = data_sha(&wrong_role.schema, &wrong_role.body).unwrap();
    assert!(wrong_role.validate_data().is_err());
    let mut wrong_count = h.clone(); wrong_count.body.held_out_windows = 64; wrong_count.sha256 = data_sha(&wrong_count.schema, &wrong_count.body).unwrap();
    assert!(wrong_count.validate_data().is_err());
    let legacy = train();
    let mut crossed = h.clone(); crossed.body.training_proof_sha256 = legacy.sha256.clone(); crossed.sha256 = data_sha(&crossed.schema, &crossed.body).unwrap();
    assert!(crossed.validate_data_links(&legacy).is_err());
}
