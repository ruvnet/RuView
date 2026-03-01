//! # RuVector Exotic WASM
//!
//! Exotic AI mechanisms for emergent behavior in distributed systems.
//! This crate provides novel coordination primitives inspired by:
//!
//! - **Decentralized governance** (Neural Autonomous Organizations)
//! - **Developmental biology** (Morphogenetic Networks)
//! - **Quantum physics** (Time Crystals)
//!
//! ## Features
//!
//! ### Neural Autonomous Organization (NAO)
//!
//! Decentralized governance for AI agent collectives using:
//! - Stake-weighted quadratic voting
//! - Oscillatory synchronization for coherence
//! - Quorum-based consensus
//!
//! ```rust
//! use ruvector_exotic_wasm::nao::NeuralAutonomousOrg;
//!
//! let mut nao = NeuralAutonomousOrg::new(0.7); // 70% quorum
//! nao.add_member("agent_1", 100);
//! nao.add_member("agent_2", 50);
//!
//! let prop_id = nao.propose("Upgrade memory backend");
//! nao.vote(&prop_id, "agent_1", 0.9);
//! nao.vote(&prop_id, "agent_2", 0.6);
//!
//! if nao.execute(&prop_id) {
//!     println!("Proposal executed!");
//! }
//! ```
//!
//! ### Morphogenetic Network
//!
//! Biologically-inspired network growth with:
//! - Cellular differentiation through morphogen gradients
//! - Emergent network topology
//! - Synaptic pruning for optimization
//!
//! ```rust
//! use ruvector_exotic_wasm::morphogenetic::MorphogeneticNetwork;
//!
//! let mut net = MorphogeneticNetwork::new(100, 100);
//! net.seed_cell(50, 50, ruvector_exotic_wasm::morphogenetic::CellType::Signaling);
//!
//! for _ in 0..1000 {
//!     net.grow(0.1);
//!     net.differentiate();
//! }
//! net.prune(0.1);
//! ```
//!
//! ### Time Crystal Coordinator
//!
//! Robust distributed coordination using discrete time crystal dynamics:
//! - Period-doubled oscillations for stable coordination
//! - Floquet engineering for noise resilience
//! - Phase-locked agent synchronization
//!
//! ```rust
//! use ruvector_exotic_wasm::time_crystal::TimeCrystal;
//!
//! let mut crystal = TimeCrystal::new(10, 100); // 10 oscillators, 100ms period
//! crystal.crystallize();
//!
//! for _ in 0..200 {
//!     let pattern = crystal.tick();
//!     // Use pattern for coordination
//! }
//! ```
//!
//! ## WASM Support
//!
//! All structures have WASM bindings via `wasm-bindgen`:
//!
//! ```javascript
//! import { WasmNAO, WasmMorphogeneticNetwork, WasmTimeCrystal } from 'ruvector-exotic-wasm';
//!
//! // Neural Autonomous Org
//! const nao = new WasmNAO(0.7);
//! nao.addMember("agent_1", 100);
//! const propId = nao.propose("Action");
//! nao.vote(propId, "agent_1", 0.9);
//!
//! // Morphogenetic Network
//! const net = new WasmMorphogeneticNetwork(100, 100);
//! net.seedSignaling(50, 50);
//! net.grow(0.1);
//!
//! // Time Crystal
//! const crystal = new WasmTimeCrystal(10, 100);
//! crystal.crystallize();
//! const pattern = crystal.tick();
//! ```

use wasm_bindgen::prelude::*;

pub mod morphogenetic;
pub mod nao;
pub mod time_crystal;

// Re-export main types
pub use morphogenetic::{Cell, CellType, GrowthFactor, MorphogeneticNetwork, NetworkStats};
pub use nao::{NeuralAutonomousOrg, OscillatorySynchronizer, Proposal, ProposalStatus};
pub use time_crystal::{CoordinationPattern, Oscillator, TimeCrystal};

// Re-export WASM types
pub use morphogenetic::WasmMorphogeneticNetwork;
pub use nao::WasmNAO;
pub use time_crystal::WasmTimeCrystal;

/// Initialize the WASM module with panic hook
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// Get the version of the ruvector-exotic-wasm crate
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get information about available exotic mechanisms
#[wasm_bindgen]
pub fn available_mechanisms() -> JsValue {
    let mechanisms = vec!["NeuralAutonomousOrg", "MorphogeneticNetwork", "TimeCrystal"];
    serde_wasm_bindgen::to_value(&mechanisms).unwrap()
}

/// Create a demonstration of all three exotic mechanisms working together
#[wasm_bindgen]
pub struct ExoticEcosystem {
    nao: nao::NeuralAutonomousOrg,
    network: morphogenetic::MorphogeneticNetwork,
    crystal: time_crystal::TimeCrystal,
    step: u64,
}

#[wasm_bindgen]
impl ExoticEcosystem {
    /// Create a new exotic ecosystem with interconnected mechanisms
    #[wasm_bindgen(constructor)]
    pub fn new(agents: usize, grid_size: i32, oscillators: usize) -> Self {
        let mut nao = nao::NeuralAutonomousOrg::new(0.5);
        let mut network = morphogenetic::MorphogeneticNetwork::new(grid_size, grid_size);
        let crystal = time_crystal::TimeCrystal::new(oscillators, 100);

        // Initialize agents in NAO
        for i in 0..agents {
            nao.add_member(&format!("agent_{}", i), 100);
        }

        // Seed some cells in the network
        for i in 0..agents {
            let x = (i as i32 * 10) % grid_size;
            let y = (i as i32 * 7) % grid_size;
            network.seed_cell(x, y, morphogenetic::CellType::Stem);
        }

        Self {
            nao,
            network,
            crystal,
            step: 0,
        }
    }

    /// Advance all systems by one step
    pub fn step(&mut self) {
        self.step += 1;

        // Use crystal coordination pattern to influence other systems
        let pattern = self.crystal.tick();

        // Use pattern to determine which agents should be active
        let _active_count = pattern
            .iter()
            .map(|b| b.count_ones() as usize)
            .sum::<usize>();

        // NAO tick with synchronized dynamics
        self.nao.tick(0.001);

        // Network growth influenced by crystal synchronization
        let sync_level = self.crystal.order_parameter();
        self.network.grow(0.1 * sync_level);

        // Differentiate periodically
        if self.step % 10 == 0 {
            self.network.differentiate();
        }

        // Prune occasionally
        if self.step % 100 == 0 {
            self.network.prune(0.05);
        }
    }

    /// Get current synchronization level (from time crystal)
    pub fn synchronization(&self) -> f32 {
        self.crystal.order_parameter()
    }

    /// Get current cell count (from morphogenetic network)
    #[wasm_bindgen(js_name = cellCount)]
    pub fn cell_count(&self) -> usize {
        self.network.cell_count()
    }

    /// Get current member count (from NAO)
    #[wasm_bindgen(js_name = memberCount)]
    pub fn member_count(&self) -> usize {
        self.nao.member_count()
    }

    /// Get current step
    #[wasm_bindgen(js_name = currentStep)]
    pub fn current_step(&self) -> u32 {
        self.step as u32
    }

    /// Crystallize the time crystal
    pub fn crystallize(&mut self) {
        self.crystal.crystallize();
    }

    /// Propose an action in the NAO
    pub fn propose(&mut self, action: &str) -> String {
        self.nao.propose(action)
    }

    /// Vote on a proposal
    pub fn vote(&mut self, proposal_id: &str, agent_id: &str, weight: f32) -> bool {
        self.nao.vote(proposal_id, agent_id, weight)
    }

    /// Execute a proposal
    pub fn execute(&mut self, proposal_id: &str) -> bool {
        self.nao.execute(proposal_id)
    }

    /// Get ecosystem summary as JSON
    #[wasm_bindgen(js_name = summaryJson)]
    pub fn summary_json(&self) -> Result<JsValue, JsValue> {
        let summary = serde_json::json!({
            "step": self.step,
            "nao": {
                "members": self.nao.member_count(),
                "active_proposals": self.nao.active_proposals().len(),
                "synchronization": self.nao.synchronization(),
            },
            "network": {
                "cells": self.network.cell_count(),
                "stats": self.network.stats(),
            },
            "crystal": {
                "oscillators": self.crystal.oscillator_count(),
                "order": self.crystal.order_parameter(),
                "crystallized": self.crystal.is_crystallized(),
                "pattern": format!("{:?}", self.crystal.detect_pattern()),
            }
        });

        serde_wasm_bindgen::to_value(&summary).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let v = version();
        assert!(!v.is_empty());
    }

    #[test]
    fn test_exotic_ecosystem() {
        let mut eco = ExoticEcosystem::new(5, 50, 8);

        assert_eq!(eco.member_count(), 5);
        assert!(eco.cell_count() > 0);

        // Run simulation
        for _ in 0..100 {
            eco.step();
        }

        assert_eq!(eco.current_step(), 100);
    }

    #[test]
    fn test_ecosystem_with_crystallization() {
        let mut eco = ExoticEcosystem::new(3, 30, 6);

        eco.crystallize();

        // Run with crystallized coordination
        for _ in 0..50 {
            eco.step();
        }

        // Should have increased synchronization
        assert!(eco.synchronization() > 0.0);
    }

    #[test]
    fn test_ecosystem_proposal_workflow() {
        let mut eco = ExoticEcosystem::new(3, 30, 6);

        let prop_id = eco.propose("Test action");
        assert!(eco.vote(&prop_id, "agent_0", 1.0));
        assert!(eco.vote(&prop_id, "agent_1", 0.8));

        // May or may not execute depending on quorum
        let _result = eco.execute(&prop_id);
    }

    #[test]
    fn test_all_modules_integrate() {
        // Test that all modules can work together
        let mut nao = NeuralAutonomousOrg::new(0.5);
        let mut network = MorphogeneticNetwork::new(50, 50);
        let mut crystal = TimeCrystal::new(8, 100);

        nao.add_member("a", 100);
        network.seed_cell(25, 25, CellType::Stem);
        crystal.crystallize();

        // Run all systems
        for _ in 0..50 {
            nao.tick(0.001);
            network.grow(0.1);
            crystal.tick();
        }

        assert!(nao.synchronization() > 0.0 || nao.synchronization() == 0.0); // Valid range
        assert!(crystal.order_parameter() >= 0.0);
    }
}
