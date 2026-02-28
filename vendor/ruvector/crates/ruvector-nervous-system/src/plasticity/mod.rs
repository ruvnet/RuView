//! Synaptic plasticity mechanisms
//!
//! This module implements biological learning rules:
//! - BTSP: Behavioral Timescale Synaptic Plasticity for one-shot learning
//! - EWC: Elastic Weight Consolidation for continual learning
//! - E-prop: Eligibility Propagation for online learning in spiking networks
//! - Future: STDP, homeostatic plasticity, metaplasticity

pub mod btsp;
pub mod consolidate;
pub mod eprop;
