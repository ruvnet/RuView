//! Autonomous Economics for edge-net P2P Network
//!
//! This module provides economic mechanisms for the compute marketplace:
//!
//! ## Components
//!
//! - **AMM**: Automated Market Maker for compute pricing
//!   - x * y = k invariant
//!   - Dynamic fee based on utilization
//!   - Liquidity provision
//!
//! - **Reputation**: Bonding curves for trust and pricing
//!   - Reputation-weighted discounts
//!   - Superlinear task allocation priority
//!   - Stake requirements

pub mod amm;
pub mod reputation;

pub use amm::*;
pub use reputation::*;
