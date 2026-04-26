//! Domain types for the BSSID Acquisition bounded context (ADR-022).

pub mod bssid;
pub mod frame;
pub mod registry;
pub mod result;

pub use bssid::{BandType, BssidId, BssidObservation, RadioType};
pub use frame::MultiApFrame;
pub use registry::{BssidEntry, BssidMeta, BssidRegistry, RunningStats};
pub use result::EnhancedSensingResult;
