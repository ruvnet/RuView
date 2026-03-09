//! rUv Neural ESP32 — Edge integration for neural sensor data acquisition and preprocessing.
//!
//! This crate provides lightweight processing that runs on ESP32 hardware for
//! real-time sensor data acquisition and preprocessing before sending to the
//! main RuVector backend.
//!
//! # Modules
//!
//! - [`adc`] — ADC interface for sensor data acquisition
//! - [`preprocessing`] — Lightweight edge preprocessing (IIR filters, downsampling)
//! - [`protocol`] — Communication protocol with the RuVector backend
//! - [`tdm`] — Time-Division Multiplexing for multi-sensor coordination
//! - [`power`] — Power management for battery operation
//! - [`aggregator`] — Multi-node data aggregation

pub mod adc;
pub mod aggregator;
pub mod power;
pub mod preprocessing;
pub mod protocol;
pub mod tdm;

pub use adc::{AdcChannel, AdcConfig, AdcReader, Attenuation};
pub use aggregator::NodeAggregator;
pub use power::{PowerConfig, PowerManager, PowerMode};
pub use preprocessing::{EdgePreprocessor, IirCoeffs};
pub use protocol::{ChannelData, NeuralDataPacket, PacketHeader};
pub use tdm::{SyncMethod, TdmNode, TdmScheduler};
