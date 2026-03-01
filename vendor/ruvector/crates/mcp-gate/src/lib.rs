//! mcp-gate: MCP (Model Context Protocol) server for the Anytime-Valid Coherence Gate
//!
//! This crate provides an MCP server that enables AI agents to request permissions
//! from the coherence gate. It implements the Model Context Protocol for
//! stdio-based communication with tool orchestrators.
//!
//! # MCP Tools
//!
//! The server exposes three main tools:
//!
//! - **permit_action**: Request permission for an action. Returns a PermitToken
//!   for permitted actions, escalation info for deferred actions, or denial details.
//!
//! - **get_receipt**: Retrieve a witness receipt by sequence number for audit purposes.
//!   Each decision generates a cryptographically signed receipt.
//!
//! - **replay_decision**: Deterministically replay a past decision for audit and
//!   verification. Optionally verifies the hash chain integrity.
//!
//! # Example Usage
//!
//! ```no_run
//! use mcp_gate::McpGateServer;
//!
//! #[tokio::main]
//! async fn main() {
//!     let server = McpGateServer::new();
//!     server.run_stdio().await.expect("Server failed");
//! }
//! ```
//!
//! # Protocol
//!
//! The server uses JSON-RPC 2.0 over stdio. Example request:
//!
//! ```json
//! {
//!   "jsonrpc": "2.0",
//!   "id": 1,
//!   "method": "tools/call",
//!   "params": {
//!     "name": "permit_action",
//!     "arguments": {
//!       "action_id": "cfg-push-7a3f",
//!       "action_type": "config_change",
//!       "target": {
//!         "device": "router-west-03",
//!         "path": "/network/interfaces/eth0"
//!       }
//!     }
//!   }
//! }
//! ```

pub mod server;
pub mod tools;
pub mod types;

// Re-export main types
pub use server::{McpGateConfig, McpGateServer, ServerCapabilities, ServerInfo};
pub use tools::{McpError, McpGateTools};
pub use types::*;

// Re-export types from cognitum-gate-tilezero for convenience
pub use cognitum_gate_tilezero::{
    ActionContext, ActionMetadata, ActionTarget, EscalationInfo, GateDecision, GateThresholds,
    PermitToken, TileZero, WitnessReceipt,
};
