//! # ruvector-economy-wasm
//!
//! A CRDT-based autonomous credit economy for distributed compute networks.
//! Designed for WASM execution with P2P consistency guarantees.
//!
//! ## Features
//!
//! - **CRDT Ledger**: G-Counter and PN-Counter for P2P-safe credit tracking
//! - **Contribution Curve**: 10x early adopter multiplier decaying to 1x baseline
//! - **Stake/Slash Mechanics**: Participation requirements with slashing for bad actors
//! - **Reputation Scoring**: Multi-factor reputation based on accuracy, uptime, and stake
//! - **Merkle Verification**: State root for quick ledger verification
//!
//! ## Quick Start (JavaScript)
//!
//! ```javascript
//! import { CreditLedger, ReputationScore, contribution_multiplier } from '@ruvector/economy-wasm';
//!
//! // Create a new ledger for a node
//! const ledger = new CreditLedger("node-123");
//!
//! // Earn credits
//! ledger.credit(100, "task:abc");
//! console.log(`Balance: ${ledger.balance()}`);
//!
//! // Check multiplier for early adopters
//! const mult = contribution_multiplier(50000.0);  // 50K network compute hours
//! console.log(`Multiplier: ${mult}x`);  // ~8.5x
//!
//! // Track reputation
//! const rep = new ReputationScore(0.95, 0.98, 1000);
//! console.log(`Composite score: ${rep.composite_score()}`);
//! ```
//!
//! ## Architecture
//!
//! ```text
//! +------------------------+
//! |     CreditLedger       |  <-- CRDT-based P2P-safe ledger
//! |  +------------------+  |
//! |  | G-Counter: Earned|  |  <-- Monotonically increasing
//! |  | PN-Counter: Spent|  |  <-- Can handle disputes
//! |  | Stake: Locked    |  |  <-- Participation requirement
//! |  | State Root       |  |  <-- Merkle root for verification
//! |  +------------------+  |
//! +------------------------+
//!           |
//!           v
//! +------------------------+
//! |  ContributionCurve     |  <-- Exponential decay: 10x -> 1x
//! +------------------------+
//!           |
//!           v
//! +------------------------+
//! |   ReputationScore      |  <-- accuracy * uptime * stake_weight
//! +------------------------+
//! ```

use wasm_bindgen::prelude::*;

pub mod curve;
pub mod ledger;
pub mod reputation;
pub mod stake;

pub use curve::{contribution_multiplier, ContributionCurve};
pub use ledger::CreditLedger;
pub use reputation::ReputationScore;
pub use stake::{SlashReason, StakeManager};

/// Initialize panic hook for better error messages in console
#[wasm_bindgen(start)]
pub fn init_panic_hook() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Get the current version of the economy module
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(version(), "0.1.0");
    }
}
