//! Core business domain entities and algorithms.
//!
//! Separated strictly from networking, transport, and framework code
//! in compliance with ELMAS.md.

pub mod classification;
pub mod csi_grid;
pub mod fps;
pub mod fusion;
pub mod node;
pub mod pose_attachment;
pub mod rolling_p95;
pub mod smoothing;
pub mod types;
pub mod vitals;

pub use pose_attachment::{
    apply_temporal_smoothing, attach_field_positions,
};

pub use classification::{
    classification_from_room, classify_vitals, debounce_room_classification_state,
    score_to_person_count, NODE_STALE_AFTER_MS, ROOM_DEBOUNCE_DURATION_SECS,
};
pub use csi_grid::{
    CalibratedPresenceEvidence, CalibrationGridBinding, CalibrationGridEvidence,
    CalibrationModelReceipt, CsiGridKey, DetectorDiagnostics, RawGridObservation,
};
pub use node::{
    build_node_features, default_position_node_ids, evict_stale_node_states,
    is_plausible_rssi, node_inference_for, node_is_fresh, node_positions_by_active_id,
    note_default_position_nodes, update_edge_node_classification,
    update_node_person_count, NodeState, CALIBRATION_GRID_EVIDENCE_CAPACITY,
    CALIBRATION_GRID_EVIDENCE_WINDOW, CALIBRATION_GRID_MAX_GAP_S,
    CALIBRATION_GRID_MIN_RATE_HZ, CALIBRATION_GRID_MIN_SPAN_S,
    COHERENCE_LOW_THRESHOLD, COHERENCE_WINDOW, DEFAULT_NODE_POSITION,
    MAX_BONE_CHANGE_RATIO, MAX_PLAUSIBLE_RSSI_DBM, NOVELTY_HISTORY_CAPACITY,
    NOVELTY_SKETCH_VERSION, NOVELTY_VECTOR_DIM, TEMPORAL_EMA_ALPHA_DEFAULT,
    TEMPORAL_EMA_ALPHA_LOW_COHERENCE,
};
pub use rolling_p95::RollingP95;
pub use types::{
    BoundingBox, ClassificationInfo, FeatureInfo, MediatekNodeDiagnostics, NodeInfo,
    NodeSyncSnapshot, PerNodeFeatureInfo, PersonDetection, PoseKeypoint, SensingUpdate,
    SignalField,
};
pub use vitals::{
    edge_vitals_message_for_publication, vitals_for_publication,
    VITAL_PUBLICATION_MIN_CONFIDENCE, VITAL_PUBLICATION_MIN_SIGNAL_QUALITY,
};
