//! Structural wire examples only: fake digest strings do not grant authority.
use super::*;
fn h(c: char) -> String {
    std::iter::repeat(c).take(64).collect()
}
fn guard() -> MutationGuard {
    MutationGuard {
        runtime_epoch: "synthetic-wire-epoch".into(),
        generation: 0,
        workflow_revision: 0,
        context_digest: None,
        phase: None,
    }
}
fn model() -> TemporalModelIdentityWire {
    TemporalModelIdentityWire {
        model_kind: WholeTemporalKind::ManagedTemporalV3,
        model_sha256: h('a'),
        verified_fit_record_sha256: h('b'),
    }
}
fn complete() -> TemporalCompleteRefsWire {
    TemporalCompleteRefsWire {
        model: model(),
        training_proof_sha256: h('c'),
        complete_record_sha256: h('d'),
        complete_commitment_sha256: h('e'),
    }
}
fn eval() -> TemporalEvaluationRefsWire {
    TemporalEvaluationRefsWire {
        complete: complete(),
        held_out_evidence_sha256: h('f'),
        evaluation_receipt_sha256: h('1'),
    }
}
fn profile() -> TemporalProfileWire {
    let conditions = [
        RoomLabel::Empty,
        RoomLabel::OccupiedStatic,
        RoomLabel::OccupiedMoving,
        RoomLabel::AdjacentOnly,
    ];
    let order = [0, 1, 1, 2, 2, 3, 3, 0, 0, 1, 1, 2, 2, 3, 3];
    TemporalProfileWire {
        profile_id: "native-2s-three-source-56bin-option-a-engineering-v1".into(),
        window_unit: "native_receiver_2s_window".into(),
        native_window_ms: 2000,
        train_windows: 64,
        held_out_windows: 64,
        train_episodes: 7,
        held_out_episodes: 8,
        windows_per_condition: [16; 4],
        train_episodes_per_condition: [1, 2, 2, 2],
        held_out_episodes_per_condition: [2; 4],
        episodes: order
            .iter()
            .enumerate()
            .map(|(i, c)| TemporalEpisodeWire {
                index: i as u8,
                role: if i < 7 {
                    DataRole::Train
                } else {
                    DataRole::HeldOut
                },
                label: conditions[*c],
                target_ms: if i == 0 { 600000 } else { 30000 },
                lead_in_ms: if i < 7 { 5000 } else { 15000 },
                native_windows: if i == 0 { 16 } else { 8 },
            })
            .collect(),
    }
}
#[test]
fn public_identity_kind_is_required_and_legacy_numeric_version_is_not_an_alias() {
    let mut value = serde_json::to_value(model()).unwrap();
    assert_eq!(
        value["model_kind"],
        "ruview.room-learner.managed-temporal-input.v3"
    );
    value.as_object_mut().unwrap().remove("model_kind");
    assert!(serde_json::from_value::<TemporalModelIdentityWire>(value.clone()).is_err());
    value["model_kind"] = serde_json::json!("unknown");
    assert!(serde_json::from_value::<TemporalModelIdentityWire>(value.clone()).is_err());
    value["model_kind"] = serde_json::json!("ruview.room-learner.managed-temporal-input.v3");
    value["model_version"] = serde_json::json!(1);
    assert!(serde_json::from_value::<TemporalModelIdentityWire>(value).is_err());
}
#[test]
fn command_inputs_reject_rows_reports_and_verified_booleans() {
    let command = TemporalEvaluateRequestV2 {
            record_comparison_evidence: false,
        schema: EVALUATE_REQUEST_SCHEMA.into(),
        expected: guard(),
        capture_generation_id: h('0'),
        refs: complete(),
    };
    let original = serde_json::to_value(command).unwrap();
    for key in ["rows", "report", "verified", "complete_witness"] {
        let mut value = original.clone();
        value[key] = serde_json::json!(true);
        assert!(
            serde_json::from_value::<TemporalEvaluateRequestV2>(value).is_err(),
            "{key}"
        );
    }
}
#[test]
fn export_canonical_structural_wire_examples_without_runtime_authority() {
    let selected = TemporalSelectedIdentityWire {
        selection_sha256: h('2'),
        selection_revision: 1,
        model_kind: WholeTemporalKind::ManagedTemporalV3,
        model_sha256: h('a'),
        verified_fit_record_sha256: h('b'),
        training_proof_sha256: h('c'),
        held_out_evidence_sha256: h('f'),
        complete_record_sha256: h('d'),
        evaluation_receipt_sha256: h('1'),
        qualification_sha256: h('3'),
        policy_sha256: super::super::qualification::hash(
            &TemporalQualificationPolicyWire::current(),
        )
        .unwrap(),
        context_digest: h('4'),
        room_digest: h('5'),
        source_node_ids: vec![15, 16, 17],
        selected_at_unix_ms: 1_000_200,
        evaluated_at_unix_ms: 1_000_100,
        authority_scope: "local_operator_binary_presence".into(),
    };
    let policy = TemporalQualificationPolicyWire::current();
    let criteria = [
        RoomLabel::Empty,
        RoomLabel::OccupiedStatic,
        RoomLabel::OccupiedMoving,
        RoomLabel::AdjacentOnly,
    ]
    .iter()
    .map(|label| {
        let positive = matches!(label, RoomLabel::OccupiedStatic | RoomLabel::OccupiedMoving);
        TemporalConditionCriterionWire {
            condition: *label,
            train_windows: 16,
            held_out_windows: 16,
            scored_windows: 16,
            abstained_windows: 0,
            missing_windows: 0,
            declared_train_episodes: if *label == RoomLabel::Empty { 1 } else { 2 },
            declared_held_out_episodes: 2,
            evaluated_window_coverage: 1.,
            unconditional_sensitivity: positive.then_some(1.),
            false_positive_rate: (!positive).then_some(0.),
        }
    })
    .collect();
    let qualification = TemporalQualificationSummaryWire {
        schema: QUALIFICATION_SCHEMA.into(),
        receipt_sha256: h('3'),
        model_kind: WholeTemporalKind::ManagedTemporalV3,
        model_sha256: h('a'),
        verified_fit_record_sha256: h('b'),
        training_lineage_basis: "managed_temporal_training_proof_v1".into(),
        training_proof_sha256: h('c'),
        held_out_evidence_sha256: h('f'),
        complete_record_sha256: h('d'),
        evaluation_receipt_sha256: h('1'),
        context_digest: h('4'),
        room_digest: h('5'),
        evaluated_at_unix_ms: 1_000_100,
        policy_sha256: super::super::qualification::hash(&policy).unwrap(),
        policy,
        label_provenance: "temporal-training-proof-v1+sealed-native-heldout-v1".into(),
        criteria,
        authority_scope: "local_operator_binary_presence".into(),
        window_unit: "native_receiver_2s_window".into(),
        train_windows: 64,
        held_out_windows: 64,
        train_episodes: 7,
        held_out_episodes: 8,
        independence_verified: false,
        physically_validated: false,
        measurement_clock_available: false,
    };
    let idle = TemporalLearnerStatusWire {
        schema: STATUS_SCHEMA.into(),
        enabled: true,
        available_model_kinds: vec![WholeTemporalKind::ManagedTemporalV3],
        temporal_profile: profile(),
        workflow_kind: WorkflowKindWire::Idle,
        mutation_guard: guard(),
        context_digest: None,
        room_digest: None,
        source_node_ids: vec![],
        recovery: None,
        capture: None,
        candidate: None,
        training_proof_sha256: None,
        complete: None,
        evaluation: None,
        assessment: None,
        model_selection: None,
        job_state: "idle".into(),
        job_operation: None,
        error_code: None,
        error: None,
        legacy_status: None,
    };
    let selection = TemporalSelectionStatusWire {
        schema: SELECTION_STATUS_SCHEMA.into(),
        revision: 1,
        selected: Some(selected.clone()),
        previous_available: false,
        qualification: Some(qualification.clone()),
        qualification_error: None,
        integrity_error: None,
        current: TemporalCurrentEvidenceWire::Unavailable {
            identity: Some(selected.clone()),
            reason: "collecting_full_horizon".into(),
            boot_epoch: Some("synthetic-boot".into()),
            receiver_epoch: Some("synthetic-receiver".into()),
        },
    };
    let matching = TemporalPairedProgressWire {
        schema: PAIRED_PROGRESS_SCHEMA.into(),
        success: true,
        installation_id: "synthetic-installation".into(),
        runtime_epoch: "synthetic-boot".into(),
        observed_at_unix_ms: 1_001_000,
        expires_at_unix_ms: 1_011_000,
        room: Some(super::super::lifecycle::PublishedProgressRoom {
            binding_digest: h('5'),
            revision: 1,
        }),
        context_match: true,
        stage: "full_complete".into(),
        state: "ready".into(),
        model_kind: Some(WholeTemporalKind::ManagedTemporalV3),
        profile: Some(profile()),
        capture: Some(TemporalPairedCaptureProgressWire {
            capture_generation_id: h('0'),
            schedule_sha256: h('6'),
            stage: "full_complete".into(),
            current_episode: None,
            role: None,
            label: None,
            target_ms: None,
            lead_in_remaining_ms: 0,
            observed_ms: 0,
            retained_windows: 128,
            durable_windows: 128,
            sealed_train_episodes: 7,
            sealed_held_out_episodes: 8,
            durability_pending: false,
            awaiting_ground_truth: false,
            original_deadline: TemporalOriginalDeadlineWire {
                created_at_unix_ms: 1_000_000,
                expires_at_unix_ms: 87_400_000,
            },
        }),
        candidate: Some(TemporalCandidateIdentityWire {
            model_kind: WholeTemporalKind::ManagedTemporalV3,
            model_sha256: h('a'),
            verified_fit_record_sha256: h('b'),
            context_digest: h('4'),
            room_digest: h('5'),
            persisted: true,
        }),
        evaluation: None,
        qualification: Some(qualification.clone()),
        selection: Some(selection.clone()),
        sources: Some(TemporalPairedSourcesWire {
            required: 3,
            ready: 3,
        }),
        save_status: "not_reported".into(),
        blocker: None,
    };
    let mismatch = TemporalPairedProgressWire {
        schema: PAIRED_PROGRESS_SCHEMA.into(),
        success: true,
        installation_id: "synthetic-installation".into(),
        runtime_epoch: "synthetic-boot".into(),
        observed_at_unix_ms: 1_001_000,
        expires_at_unix_ms: 1_011_000,
        room: None,
        context_match: false,
        stage: "blocked".into(),
        state: "idle".into(),
        model_kind: None,
        profile: None,
        capture: None,
        candidate: None,
        evaluation: None,
        qualification: None,
        selection: None,
        sources: None,
        save_status: "not_reported".into(),
        blocker: Some("context_mismatch".into()),
    };
    println!("C4C_CANONICAL_PROGRESS_EXAMPLES={}",serde_json::to_string(&serde_json::json!({"fixture_kind":"SYNTHETIC_STRUCTURAL_SERIALIZATION_ONLY_NOT_AUTHORITY","matching_complete":matching,"context_mismatch_redacted":mismatch,"selected_temporal_idle":{"schema":SELECTION_SCHEMA,"success":true,"progress_schema":PAIRED_PROGRESS_SCHEMA,"model_selection":selection}})).unwrap());
    let examples = serde_json::json!({"fixture_kind":"SYNTHETIC_STRUCTURAL_SERIALIZATION_ONLY_NOT_AUTHORITY","idle_response":{"success":true,"status":idle},"candidate_identity":TemporalCandidateIdentityWire{model_kind:WholeTemporalKind::ManagedTemporalV3,model_sha256:h('a'),verified_fit_record_sha256:h('b'),context_digest:h('4'),room_digest:h('5'),persisted:false},"selected_identity":selected,"qualification_summary":qualification,"configure_request":TemporalConfigureRequestV2{schema:CONFIGURE_REQUEST_SCHEMA.into(),model_kind:WholeTemporalKind::ManagedTemporalV3,expected:guard(),binding_digest:h('4'),revision:1,source_node_ids:vec![15,16,17],new_training:true,recovery_consent:true},"evaluate_request":TemporalEvaluateRequestV2{record_comparison_evidence:false,schema:EVALUATE_REQUEST_SCHEMA.into(),expected:guard(),capture_generation_id:h('0'),refs:complete()},"qualify_request":TemporalQualifyRequestV2{schema:QUALIFY_REQUEST_SCHEMA.into(),expected:guard(),capture_generation_id:h('0'),refs:eval()}});
    println!(
        "C4C_CANONICAL_PUBLIC_WIRE_EXAMPLES={}",
        serde_json::to_string(&examples).unwrap()
    );
}
