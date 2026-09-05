//! # WiFi-DensePose Training Infrastructure
//!
//! This crate provides the complete training pipeline for the WiFi-DensePose pose
//! estimation model. It includes configuration management, dataset loading with
//! subcarrier interpolation, loss functions, evaluation metrics, and the training
//! loop orchestrator.
//!
//! ## Architecture
//!
//! ```text
//! TrainingConfig ──► Trainer ──► Model
//!       │               │
//!       │           DataLoader
//!       │               │
//!       │         CsiDataset (MmFiDataset | SyntheticCsiDataset)
//!       │               │
//!       │         subcarrier::interpolate_subcarriers
//!       │
//!       └──► losses / metrics
//! ```
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use wifi_densepose_train::config::TrainingConfig;
//! use wifi_densepose_train::dataset::{SyntheticCsiDataset, SyntheticConfig, CsiDataset};
//!
//! // Build config
//! let config = TrainingConfig::default();
//! config.validate().expect("config is valid");
//!
//! // Create a synthetic dataset (deterministic, fixed-seed)
//! let syn_cfg = SyntheticConfig::default();
//! let dataset = SyntheticCsiDataset::new(200, syn_cfg);
//!
//! // Load one sample
//! let sample = dataset.get(0).unwrap();
//! println!("amplitude shape: {:?}", sample.amplitude.shape());
//! ```

// Note: #![forbid(unsafe_code)] is intentionally absent because the `tch`
// dependency (PyTorch Rust bindings) internally requires unsafe code via FFI.
// All *this* crate's code is written without unsafe blocks.
#![warn(missing_docs)]

/// Metric-locked pose-accuracy harness (ADR-155 §Tier-1.2; needs ADR slot 173)
/// — selectable `PckNormalization` (torso / bbox-diagonal / absolute), `mpjpe`,
/// and a self-describing `PoseAccuracy` result so a reported PCK number always
/// carries the definition it was computed under.
pub mod accuracy;
pub mod config;
pub mod dataset;
pub mod domain;
pub mod error;
pub mod eval;
pub mod geometry;
pub mod mae;
/// Canonical pose-metric core (ADR-155 §Tier-1.1) — `pck_canonical` /
/// `oks_canonical`, available **without** the `tch-backend` feature so the
/// single metric definition is reachable from the workspace test gate.
pub mod metrics_core;
/// Model release sanity gates (ADR-298) — block degenerate and mislabeled
/// classifier artifacts (unreachable decision boundary, constant output,
/// degenerate class balance, missing baseline, metric-name provenance) before
/// release. Prevention only; withdraws nothing already published.
pub mod model_gates;
/// Public-benchmark split protocols and leakage guards (ADR-291 §2–3) —
/// deterministic cross-subject / cross-environment / cross-orientation
/// assignment plus the structural [`protocols::leakage::LeakageAudit`],
/// mean-pose baseline, and evidence-graded evaluation reports.
pub mod protocols;
pub mod rapid_adapt;
pub mod ruview_metrics;
/// Sensing evidence and claim gate (ADR-328) — validates provenance,
/// held-out-environment coverage, sample counts, pre-registered metric
/// thresholds, and claim class, then emits a deterministic JSON receipt.
pub mod sensing_claim_gate;
pub mod signal_features;
pub mod subcarrier;
pub mod virtual_aug;
pub mod wiflow_std;

// The following modules use `tch` (PyTorch Rust bindings) for GPU-accelerated
// training and are only compiled when the `tch-backend` feature is enabled.
// Without the feature the crate still provides the dataset / config / subcarrier
// APIs needed for data preprocessing and proof verification.
#[cfg(feature = "tch-backend")]
pub mod losses;
#[cfg(feature = "tch-backend")]
pub mod metrics;
#[cfg(feature = "tch-backend")]
pub mod model;
#[cfg(feature = "tch-backend")]
pub mod proof;

/// ADR-145 — ablation evaluation harness (feature matrix + privacy/latency metrics).
pub mod ablation;
/// Falsifiable occupancy/presence benchmark (real-CSI gate: provenance,
/// leak-free split, bootstrap-CI thresholds; refuses claims on synthetic/mock).
pub mod occupancy_bench;
#[cfg(feature = "tch-backend")]
pub mod trainer;

// Convenient re-exports at the crate root.
// Canonical metric (ADR-155 §Tier-1.1) — re-exported un-gated so the single
// source of truth is reachable with or without `tch-backend`.
pub use metrics_core::{
    canonical_torso_size, oks_canonical, pck_canonical, CANON_LEFT_HIP, CANON_RIGHT_HIP,
    COCO_KP_SIGMAS,
};
// ADR-155 §Tier-1.2 — metric-locked accuracy harness (selectable PCK
// normalization + MPJPE + self-describing result).
pub use accuracy::{
    accuracy_report, mpjpe as pck_mpjpe, pck_at, PckNormalization, PoseAccuracy, PoseFrame,
};
pub use config::TrainingConfig;
pub use dataset::{
    CsiDataset, CsiSample, DataLoader, MmFiDataset, SyntheticConfig, SyntheticCsiDataset,
};
// ADR-291 — Widar3.0 ingest, split protocols, and leakage guards.
pub use dataset::widar::{parse_bfee_bytes, BfeeParse, BfeeRecord, WidarDataset, WidarFileMeta};
pub use protocols::leakage::{
    EvaluationReport, EvidenceGrade, LeakageAudit, LeakageClaims, MeanPoseBaseline,
};
pub use protocols::{SampleMeta, SplitPlan, SplitProtocol, SplitSide};

// ADR-298 — model release sanity gates.
pub use model_gates::{
    check_baseline, check_class_balance, check_constant_output, check_metric_provenance,
    check_unreachable_boundary, evaluate_linear_head, GateError, GateFailure, GateOutcomeError,
    LabeledMetric, LinearHead, MetricKind, ModelGateReport, ProbeSet,
};

// ADR-328 — sensing evidence and claim receipts.
pub use sensing_claim_gate::{
    evaluate_claim_json_for_class, ClaimGateError, ClaimManifest, ClaimPolicy, GateDecision,
    GateReceipt, ReceiptEnvelope,
};

pub use error::{ConfigError, DatasetError, MaeError, ProtocolError, SubcarrierError, TrainError};
// TrainResult<T> is the generic Result alias from error.rs; the concrete
// TrainResult struct from trainer.rs is accessed via trainer::TrainResult.
pub use error::TrainResult as TrainResultAlias;
pub use subcarrier::{
    compute_interp_weights, interpolate_subcarriers, select_subcarriers_by_variance,
};

// ADR-152 §2.3 — UNSW MAE pretraining recipe re-exports.
pub use mae::{patchify, random_mask, unpatchify, MaePretrainConfig, MaskIndices, PatchGrid};

// ADR-152 §2.2 — WiFlow-STD (DY2434) spatio-temporal-decoupled pose model.
pub use wiflow_std::WiFlowStdConfig;
#[cfg(feature = "tch-backend")]
pub use wiflow_std::WiFlowStdModel;

// MERIDIAN (ADR-027) re-exports.
pub use domain::{AdversarialSchedule, DomainClassifier, DomainFactorizer, GradientReversalLayer};
pub use eval::CrossDomainEvaluator;
pub use geometry::{FilmLayer, FourierPositionalEncoding, GeometryEncoder, MeridianGeometryConfig};
pub use rapid_adapt::{AdaptError, AdaptationLoss, AdaptationResult, RapidAdaptation};
pub use virtual_aug::VirtualDomainAugmentor;

/// Crate version string.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
