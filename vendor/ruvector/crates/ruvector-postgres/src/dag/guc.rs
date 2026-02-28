//! GUC (Grand Unified Configuration) variables for DAG learning

use pgrx::prelude::*;

// GUC variables
static mut NEURAL_DAG_ENABLED: bool = false;
static mut DAG_LEARNING_RATE: f64 = 0.01;
static mut DAG_ATTENTION_MECHANISM: Option<String> = None;
static mut DAG_MICRO_LORA_RANK: i32 = 2;
static mut DAG_EWC_LAMBDA: f64 = 5000.0;
static mut DAG_PATTERN_CLUSTERS: i32 = 100;

/// Initialize GUC variables
pub fn init_guc() {
    // Register GUC variables with PostgreSQL
    // This would use pgrx GUC macros
}

/// Check if neural DAG is enabled
pub fn is_enabled() -> bool {
    unsafe { NEURAL_DAG_ENABLED }
}

/// Get current learning rate
pub fn get_learning_rate() -> f64 {
    unsafe { DAG_LEARNING_RATE }
}

/// Get attention mechanism name
pub fn get_attention_mechanism() -> Option<&'static str> {
    unsafe { DAG_ATTENTION_MECHANISM.as_deref() }
}

/// Get MicroLoRA rank
pub fn get_micro_lora_rank() -> i32 {
    unsafe { DAG_MICRO_LORA_RANK }
}

/// Get EWC lambda
pub fn get_ewc_lambda() -> f64 {
    unsafe { DAG_EWC_LAMBDA }
}

/// Get pattern cluster count
pub fn get_pattern_clusters() -> i32 {
    unsafe { DAG_PATTERN_CLUSTERS }
}
