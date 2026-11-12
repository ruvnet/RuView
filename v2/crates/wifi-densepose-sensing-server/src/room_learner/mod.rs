//! Account-bound multi-source room calibration and learned inference.
pub mod observation;
pub mod capture;
pub mod model;
pub mod training;
pub mod artifact;
pub mod lifecycle;
pub mod native;
pub mod recovery_store;

pub mod qualification;
pub mod selection;

pub mod assessment;

pub(crate) mod managed_training;

pub(crate) mod managed_capture_fences;

/// Experimental source-only temporal acquisition; not wired to v1 runtime.
pub mod temporal_acquisition;

/// Experimental policy-bound adapter; no runtime or adoption wiring.
pub mod temporal_model_input;

/// Default-inactive continuous temporal input diagnostic; no runtime wiring.
pub mod temporal_live_driver;

/// Inactive engineering schedule contract; no managed capture authority.
pub mod managed_schedule_contract;

/// Whole-model diagnostic proof/evaluation DATA; decoded bytes are not authority.
pub(crate) mod temporal_evaluation;

/// Versioned public whole-kind DTOs; no decoder grants producer authority.
pub mod temporal_wire;

pub(crate) mod temporal_qualification;
