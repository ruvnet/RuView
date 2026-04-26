//! Alerting module for emergency notifications.

mod generator;
mod dispatcher;
mod triage_service;

pub use generator::AlertGenerator;
pub use dispatcher::{AlertDispatcher, AlertConfig};
pub use triage_service::{TriageService, PriorityCalculator};
