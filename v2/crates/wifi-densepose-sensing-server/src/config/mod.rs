//! Server runtime and operational configuration.

pub mod multistatic_guard;
pub mod runtime_config;

pub use multistatic_guard::{multistatic_guard_config_from, multistatic_guard_config_from_env};
pub use runtime_config::{load_runtime_config, save_runtime_config, RuntimeConfig};
