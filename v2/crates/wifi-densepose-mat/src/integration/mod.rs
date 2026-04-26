//! Integration layer (Anti-Corruption Layer) for upstream crates.
//!
//! This module provides adapters to translate between:
//! - wifi-densepose-signal types and wifi-Mat domain types
//! - wifi-densepose-nn inference results and detection results
//! - wifi-densepose-hardware interfaces and sensor abstractions
//!
//! # Hardware Support
//!
//! The integration layer supports multiple WiFi CSI hardware platforms:
//!
//! - **ESP32**: Via serial communication using ESP-CSI firmware
//! - **Intel 5300 NIC**: Using Linux CSI Tool (iwlwifi driver)
//! - **Atheros NICs**: Using ath9k/ath10k/ath11k CSI patches
//! - **Nexmon**: For Broadcom chips with CSI firmware
//!
//! # Example Usage
//!
//! ```ignore
//! use wifi_densepose_mat::integration::{
//!     HardwareAdapter, HardwareConfig, AtherosDriver,
//!     csi_receiver::{UdpCsiReceiver, ReceiverConfig},
//! };
//!
//! // Configure for ESP32
//! let config = HardwareConfig::esp32("/dev/ttyUSB0", 921600);
//! let mut adapter = HardwareAdapter::with_config(config);
//! adapter.initialize().await?;
//!
//! // Or configure for Intel 5300
//! let config = HardwareConfig::intel_5300("wlan0");
//! let mut adapter = HardwareAdapter::with_config(config);
//!
//! // Or use UDP receiver for network streaming
//! let config = ReceiverConfig::udp("0.0.0.0", 5500);
//! let mut receiver = UdpCsiReceiver::new(config).await?;
//! ```

mod signal_adapter;
mod neural_adapter;
mod hardware_adapter;
pub mod csi_receiver;

pub use signal_adapter::SignalAdapter;
pub use neural_adapter::NeuralAdapter;
pub use hardware_adapter::{
    // Main adapter
    HardwareAdapter,
    // Configuration types
    HardwareConfig,
    DeviceType,
    DeviceSettings,
    AtherosDriver,
    ChannelConfig,
    Bandwidth,
    // Serial settings
    SerialSettings,
    Parity,
    FlowControl,
    // Network interface settings
    NetworkInterfaceSettings,
    AntennaConfig,
    // UDP settings
    UdpSettings,
    // PCAP settings
    PcapSettings,
    // Sensor types
    SensorInfo,
    SensorStatus,
    // CSI data types
    CsiReadings,
    CsiMetadata,
    SensorCsiReading,
    FrameControlType,
    CsiStream,
    // Health and stats
    HardwareHealth,
    HealthStatus,
    StreamingStats,
};

pub use csi_receiver::{
    // Receiver types
    UdpCsiReceiver,
    SerialCsiReceiver,
    PcapCsiReader,
    // Configuration
    ReceiverConfig,
    CsiSource,
    UdpSourceConfig,
    SerialSourceConfig,
    PcapSourceConfig,
    SerialParity,
    // Packet types
    CsiPacket,
    CsiPacketMetadata,
    CsiPacketFormat,
    // Parser
    CsiParser,
    // Stats
    ReceiverStats,
};

/// Configuration for integration layer
#[derive(Debug, Clone, Default)]
pub struct IntegrationConfig {
    /// Use GPU acceleration if available
    pub use_gpu: bool,
    /// Batch size for neural inference
    pub batch_size: usize,
    /// Enable signal preprocessing optimizations
    pub optimize_signal: bool,
    /// Hardware configuration
    pub hardware: Option<HardwareConfig>,
}

impl IntegrationConfig {
    /// Create configuration for real-time processing
    pub fn realtime() -> Self {
        Self {
            use_gpu: true,
            batch_size: 1,
            optimize_signal: true,
            hardware: None,
        }
    }

    /// Create configuration for batch processing
    pub fn batch(batch_size: usize) -> Self {
        Self {
            use_gpu: true,
            batch_size,
            optimize_signal: true,
            hardware: None,
        }
    }

    /// Create configuration with specific hardware
    pub fn with_hardware(hardware: HardwareConfig) -> Self {
        Self {
            use_gpu: true,
            batch_size: 1,
            optimize_signal: true,
            hardware: Some(hardware),
        }
    }
}

/// Error type for integration layer
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    /// Signal processing error
    #[error("Signal adapter error: {0}")]
    Signal(String),

    /// Neural network error
    #[error("Neural adapter error: {0}")]
    Neural(String),

    /// Hardware error
    #[error("Hardware adapter error: {0}")]
    Hardware(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Data format error
    #[error("Data format error: {0}")]
    DataFormat(String),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Timeout error
    #[error("Timeout error: {0}")]
    Timeout(String),
}

/// Prelude module for convenient imports
pub mod prelude {
    pub use super::{
        AdapterError,
        HardwareAdapter,
        HardwareConfig,
        DeviceType,
        AtherosDriver,
        Bandwidth,
        CsiReadings,
        CsiPacket,
        CsiPacketFormat,
        IntegrationConfig,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integration_config_defaults() {
        let config = IntegrationConfig::default();
        assert!(!config.use_gpu);
        assert_eq!(config.batch_size, 0);
        assert!(!config.optimize_signal);
        assert!(config.hardware.is_none());
    }

    #[test]
    fn test_integration_config_realtime() {
        let config = IntegrationConfig::realtime();
        assert!(config.use_gpu);
        assert_eq!(config.batch_size, 1);
        assert!(config.optimize_signal);
    }

    #[test]
    fn test_integration_config_batch() {
        let config = IntegrationConfig::batch(32);
        assert!(config.use_gpu);
        assert_eq!(config.batch_size, 32);
    }

    #[test]
    fn test_integration_config_with_hardware() {
        let hw_config = HardwareConfig::esp32("/dev/ttyUSB0", 921600);
        let config = IntegrationConfig::with_hardware(hw_config);
        assert!(config.hardware.is_some());
        assert!(matches!(
            config.hardware.as_ref().unwrap().device_type,
            DeviceType::Esp32
        ));
    }
}
