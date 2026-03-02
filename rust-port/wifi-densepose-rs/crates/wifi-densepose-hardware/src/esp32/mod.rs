//! ESP32 hardware protocol modules.
//!
//! Implements sensing-first RF protocols for ESP32-S3 mesh nodes,
//! including TDM (Time-Division Multiplexed) sensing schedules
//! per ADR-029 (RuvSense) and ADR-031 (RuView).

pub mod tdm;

pub use tdm::{
    TdmSchedule, TdmCoordinator, TdmSlot, TdmSlotCompleted,
    SyncBeacon, TdmError,
};
