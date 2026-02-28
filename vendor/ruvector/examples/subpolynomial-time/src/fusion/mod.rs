//! Vector-Graph Fusion Module
//!
//! Unified retrieval substrate combining vector similarity and graph relations
//! with minimum-cut brittleness detection for robust knowledge retrieval.

mod fusion_graph;
mod optimizer;
mod structural_monitor;

pub use fusion_graph::{
    EdgeOrigin, FusionConfig, FusionEdge, FusionGraph, FusionNode, FusionResult, RelationType,
};
pub use optimizer::{
    LearningGate, MaintenancePlan, MaintenanceTask, OptimizationResult, Optimizer, OptimizerAction,
};
pub use structural_monitor::{
    BrittlenessSignal, MonitorConfig as StructuralMonitorConfig, MonitorState, StructuralMonitor,
    Trigger, TriggerType,
};
