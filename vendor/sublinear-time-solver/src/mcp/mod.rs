//! MCP (Model Context Protocol) tool integration for sublinear solver
//!
//! Provides MCP server endpoints for solver and scheduler operations.
//! Created by rUv - https://github.com/ruvnet

#[cfg(feature = "cli")]
pub mod scheduler_tool;

#[cfg(feature = "cli")]
pub use scheduler_tool::SchedulerTool;