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
