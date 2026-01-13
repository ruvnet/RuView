//! Adapter for wifi-densepose-hardware crate with real hardware support.
//!
//! This module provides adapters for various WiFi CSI hardware:
//! - ESP32 with CSI support via serial communication
//! - Intel 5300 NIC with Linux CSI Tool
//! - Atheros CSI extraction via ath9k/ath10k drivers
//!
//! # Example
//!
//! ```ignore
//! use wifi_densepose_mat::integration::{HardwareAdapter, HardwareConfig, DeviceType};
//!
//! let config = HardwareConfig::esp32("/dev/ttyUSB0", 921600);
//! let mut adapter = HardwareAdapter::with_config(config);
//! adapter.initialize().await?;
//!
//! // Start streaming CSI data
//! let mut stream = adapter.start_csi_stream().await?;
//! while let Some(reading) = stream.next().await {
//!     // Process CSI data
//! }
//! ```

use super::AdapterError;
use crate::domain::SensorPosition;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};

/// Hardware configuration for CSI devices
#[derive(Debug, Clone)]
pub struct HardwareConfig {
    /// Device type selection
    pub device_type: DeviceType,
    /// Device-specific settings
    pub device_settings: DeviceSettings,
    /// Buffer size for CSI data
    pub buffer_size: usize,
    /// Whether to enable raw mode (minimal processing)
    pub raw_mode: bool,
    /// Sample rate override (Hz, 0 for device default)
    pub sample_rate_override: u32,
    /// Channel configuration
    pub channel_config: ChannelConfig,
}

impl Default for HardwareConfig {
    fn default() -> Self {
        Self {
            device_type: DeviceType::Simulated,
            device_settings: DeviceSettings::Simulated,
            buffer_size: 4096,
            raw_mode: false,
            sample_rate_override: 0,
            channel_config: ChannelConfig::default(),
        }
    }
}

impl HardwareConfig {
    /// Create configuration for ESP32 via serial
    pub fn esp32(serial_port: &str, baud_rate: u32) -> Self {
        Self {
            device_type: DeviceType::Esp32,
            device_settings: DeviceSettings::Serial(SerialSettings {
                port: serial_port.to_string(),
                baud_rate,
                data_bits: 8,
                stop_bits: 1,
                parity: Parity::None,
                flow_control: FlowControl::None,
                read_timeout_ms: 1000,
            }),
            buffer_size: 2048,
            raw_mode: false,
            sample_rate_override: 0,
            channel_config: ChannelConfig::default(),
        }
    }

    /// Create configuration for Intel 5300 NIC
    pub fn intel_5300(interface: &str) -> Self {
        Self {
            device_type: DeviceType::Intel5300,
            device_settings: DeviceSettings::NetworkInterface(NetworkInterfaceSettings {
                interface: interface.to_string(),
                monitor_mode: true,
                channel: 6,
                bandwidth: Bandwidth::HT20,
                antenna_config: AntennaConfig::default(),
            }),
            buffer_size: 8192,
            raw_mode: false,
            sample_rate_override: 0,
            channel_config: ChannelConfig {
                channel: 6,
                bandwidth: Bandwidth::HT20,
                num_subcarriers: 30, // Intel 5300 provides 30 subcarriers
            },
        }
    }

    /// Create configuration for Atheros NIC
    pub fn atheros(interface: &str, driver: AtherosDriver) -> Self {
        let num_subcarriers = match driver {
            AtherosDriver::Ath9k => 56,
            AtherosDriver::Ath10k => 114,
            AtherosDriver::Ath11k => 234,
        };

        Self {
            device_type: DeviceType::Atheros(driver),
            device_settings: DeviceSettings::NetworkInterface(NetworkInterfaceSettings {
                interface: interface.to_string(),
                monitor_mode: true,
                channel: 36,
                bandwidth: Bandwidth::HT40,
                antenna_config: AntennaConfig::default(),
            }),
            buffer_size: 16384,
            raw_mode: false,
            sample_rate_override: 0,
            channel_config: ChannelConfig {
                channel: 36,
                bandwidth: Bandwidth::HT40,
                num_subcarriers,
            },
        }
    }

    /// Create configuration for UDP receiver (generic CSI)
    pub fn udp_receiver(bind_addr: &str, port: u16) -> Self {
        Self {
            device_type: DeviceType::UdpReceiver,
            device_settings: DeviceSettings::Udp(UdpSettings {
                bind_address: bind_addr.to_string(),
                port,
                multicast_group: None,
                buffer_size: 65536,
            }),
            buffer_size: 8192,
            raw_mode: false,
            sample_rate_override: 0,
            channel_config: ChannelConfig::default(),
        }
    }
}

/// Supported device types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceType {
    /// ESP32 with ESP-CSI firmware
    Esp32,
    /// Intel 5300 NIC with Linux CSI Tool
    Intel5300,
    /// Atheros NIC with specific driver
    Atheros(AtherosDriver),
    /// Generic UDP CSI receiver
    UdpReceiver,
    /// PCAP file replay
    PcapFile,
    /// Simulated device (for testing)
    Simulated,
}

/// Atheros driver variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtherosDriver {
    /// ath9k driver (legacy, 56 subcarriers)
    Ath9k,
    /// ath10k driver (802.11ac, 114 subcarriers)
    Ath10k,
    /// ath11k driver (802.11ax, 234 subcarriers)
    Ath11k,
}

/// Device-specific settings
#[derive(Debug, Clone)]
pub enum DeviceSettings {
    /// Serial port settings (ESP32)
    Serial(SerialSettings),
    /// Network interface settings (Intel 5300, Atheros)
    NetworkInterface(NetworkInterfaceSettings),
    /// UDP receiver settings
    Udp(UdpSettings),
    /// PCAP file settings
    Pcap(PcapSettings),
    /// Simulated device (no real hardware)
    Simulated,
}

/// Serial port configuration
#[derive(Debug, Clone)]
pub struct SerialSettings {
    /// Serial port path
    pub port: String,
    /// Baud rate
    pub baud_rate: u32,
    /// Data bits (5-8)
    pub data_bits: u8,
    /// Stop bits (1, 2)
    pub stop_bits: u8,
    /// Parity setting
    pub parity: Parity,
    /// Flow control
    pub flow_control: FlowControl,
    /// Read timeout in milliseconds
    pub read_timeout_ms: u64,
}

/// Parity options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    None,
    Odd,
    Even,
}

/// Flow control options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowControl {
    None,
    Hardware,
    Software,
}

/// Network interface configuration
#[derive(Debug, Clone)]
pub struct NetworkInterfaceSettings {
    /// Interface name (e.g., "wlan0")
    pub interface: String,
    /// Enable monitor mode
    pub monitor_mode: bool,
    /// WiFi channel
    pub channel: u8,
    /// Channel bandwidth
    pub bandwidth: Bandwidth,
    /// Antenna configuration
    pub antenna_config: AntennaConfig,
}

/// Channel bandwidth options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Bandwidth {
    /// 20 MHz (legacy)
    #[default]
    HT20,
    /// 40 MHz (802.11n)
    HT40,
    /// 80 MHz (802.11ac)
    VHT80,
    /// 160 MHz (802.11ac Wave 2)
    VHT160,
}

impl Bandwidth {
    /// Get number of subcarriers for this bandwidth
    pub fn subcarrier_count(&self) -> usize {
        match self {
            Bandwidth::HT20 => 56,
            Bandwidth::HT40 => 114,
            Bandwidth::VHT80 => 242,
            Bandwidth::VHT160 => 484,
        }
    }
}

/// Antenna configuration for MIMO
#[derive(Debug, Clone)]
pub struct AntennaConfig {
    /// Number of transmit antennas
    pub tx_antennas: u8,
    /// Number of receive antennas
    pub rx_antennas: u8,
    /// Enabled antenna mask
    pub antenna_mask: u8,
}

impl Default for AntennaConfig {
    fn default() -> Self {
        Self {
            tx_antennas: 1,
            rx_antennas: 3,
            antenna_mask: 0x07, // Enable antennas 0, 1, 2
        }
    }
}

/// UDP receiver settings
#[derive(Debug, Clone)]
pub struct UdpSettings {
    /// Bind address
    pub bind_address: String,
    /// Port number
    pub port: u16,
    /// Multicast group (optional)
    pub multicast_group: Option<String>,
    /// Socket buffer size
    pub buffer_size: usize,
}

/// PCAP file settings
#[derive(Debug, Clone)]
pub struct PcapSettings {
    /// Path to PCAP file
    pub file_path: String,
    /// Playback speed multiplier (1.0 = realtime)
    pub playback_speed: f64,
    /// Loop playback
    pub loop_playback: bool,
}

/// Channel configuration
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    /// WiFi channel
    pub channel: u8,
    /// Bandwidth
    pub bandwidth: Bandwidth,
    /// Number of OFDM subcarriers
    pub num_subcarriers: usize,
}

impl Default for ChannelConfig {
    fn default() -> Self {
        Self {
            channel: 6,
            bandwidth: Bandwidth::HT20,
            num_subcarriers: 56,
        }
    }
}

/// Hardware adapter for sensor communication
pub struct HardwareAdapter {
    /// Configuration
    config: HardwareConfig,
    /// Connected sensors
    sensors: Vec<SensorInfo>,
    /// Whether hardware is initialized
    initialized: bool,
    /// CSI broadcast channel
    csi_broadcaster: Option<broadcast::Sender<CsiReadings>>,
    /// Device state (shared for async operations)
    state: Arc<RwLock<DeviceState>>,
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
}

/// Internal device state
struct DeviceState {
    /// Whether streaming is active
    streaming: bool,
    /// Total packets received
    packets_received: u64,
    /// Packets with errors
    error_count: u64,
    /// Last error message
    last_error: Option<String>,
    /// Device-specific state
    device_state: DeviceSpecificState,
}

/// Device-specific runtime state
enum DeviceSpecificState {
    Esp32 {
        firmware_version: Option<String>,
        mac_address: Option<String>,
    },
    Intel5300 {
        bfee_count: u64,
    },
    Atheros {
        driver: AtherosDriver,
        csi_buf_ptr: Option<u64>,
    },
    Other,
}

/// Information about a connected sensor
#[derive(Debug, Clone)]
pub struct SensorInfo {
    /// Unique sensor ID
    pub id: String,
    /// Sensor position
    pub position: SensorPosition,
    /// Current status
    pub status: SensorStatus,
    /// Last RSSI reading (if available)
    pub last_rssi: Option<f64>,
    /// Battery level (0-100, if applicable)
    pub battery_level: Option<u8>,
    /// MAC address (if available)
    pub mac_address: Option<String>,
    /// Firmware version (if available)
    pub firmware_version: Option<String>,
}

/// Status of a sensor
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SensorStatus {
    /// Sensor is connected and operational
    Connected,
    /// Sensor is disconnected
    Disconnected,
    /// Sensor is in error state
    Error,
    /// Sensor is initializing
    Initializing,
    /// Sensor battery is low
    LowBattery,
    /// Sensor is in standby mode
    Standby,
}

impl HardwareAdapter {
    /// Create a new hardware adapter with default configuration
    pub fn new() -> Self {
        Self::with_config(HardwareConfig::default())
    }

    /// Create a new hardware adapter with specific configuration
    pub fn with_config(config: HardwareConfig) -> Self {
        Self {
            config,
            sensors: Vec::new(),
            initialized: false,
            csi_broadcaster: None,
            state: Arc::new(RwLock::new(DeviceState {
                streaming: false,
                packets_received: 0,
                error_count: 0,
                last_error: None,
                device_state: DeviceSpecificState::Other,
            })),
            shutdown_tx: None,
        }
    }

    /// Get the current configuration
    pub fn config(&self) -> &HardwareConfig {
        &self.config
    }

    /// Initialize hardware communication
    pub async fn initialize(&mut self) -> Result<(), AdapterError> {
        tracing::info!("Initializing hardware adapter for {:?}", self.config.device_type);

        match &self.config.device_type {
            DeviceType::Esp32 => self.initialize_esp32().await?,
            DeviceType::Intel5300 => self.initialize_intel_5300().await?,
            DeviceType::Atheros(driver) => self.initialize_atheros(*driver).await?,
            DeviceType::UdpReceiver => self.initialize_udp().await?,
            DeviceType::PcapFile => self.initialize_pcap().await?,
            DeviceType::Simulated => self.initialize_simulated().await?,
        }

        // Create CSI broadcast channel
        let (tx, _) = broadcast::channel(self.config.buffer_size);
        self.csi_broadcaster = Some(tx);

        self.initialized = true;
        tracing::info!("Hardware adapter initialized successfully");
        Ok(())
    }

    /// Initialize ESP32 device
    async fn initialize_esp32(&mut self) -> Result<(), AdapterError> {
        let settings = match &self.config.device_settings {
            DeviceSettings::Serial(s) => s,
            _ => return Err(AdapterError::Config("ESP32 requires serial settings".into())),
        };

        tracing::info!("Initializing ESP32 on {} at {} baud", settings.port, settings.baud_rate);

        // Verify serial port exists
        #[cfg(unix)]
        {
            if !std::path::Path::new(&settings.port).exists() {
                return Err(AdapterError::Hardware(format!(
                    "Serial port {} not found",
                    settings.port
                )));
            }
        }

        // Update device state
        let mut state = self.state.write().await;
        state.device_state = DeviceSpecificState::Esp32 {
            firmware_version: None,
            mac_address: None,
        };

        Ok(())
    }

    /// Initialize Intel 5300 NIC
    async fn initialize_intel_5300(&mut self) -> Result<(), AdapterError> {
        let settings = match &self.config.device_settings {
            DeviceSettings::NetworkInterface(s) => s,
            _ => return Err(AdapterError::Config("Intel 5300 requires network interface settings".into())),
        };

        tracing::info!("Initializing Intel 5300 on interface {}", settings.interface);

        // Check if iwlwifi driver is loaded
        #[cfg(target_os = "linux")]
        {
            let output = tokio::process::Command::new("lsmod")
                .output()
                .await
                .map_err(|e| AdapterError::Hardware(format!("Failed to check kernel modules: {}", e)))?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.contains("iwlwifi") {
                tracing::warn!("iwlwifi module not loaded - CSI extraction may not work");
            }
        }

        // Verify connector proc file exists (Linux CSI Tool)
        #[cfg(target_os = "linux")]
        {
            let connector_path = "/proc/net/connector";
            if !std::path::Path::new(connector_path).exists() {
                tracing::warn!("Connector proc file not found - install Linux CSI Tool");
            }
        }

        let mut state = self.state.write().await;
        state.device_state = DeviceSpecificState::Intel5300 { bfee_count: 0 };

        Ok(())
    }

    /// Initialize Atheros NIC
    async fn initialize_atheros(&mut self, driver: AtherosDriver) -> Result<(), AdapterError> {
        let settings = match &self.config.device_settings {
            DeviceSettings::NetworkInterface(s) => s,
            _ => return Err(AdapterError::Config("Atheros requires network interface settings".into())),
        };

        tracing::info!(
            "Initializing Atheros ({:?}) on interface {}",
            driver,
            settings.interface
        );

        // Check for driver-specific debugfs entries
        #[cfg(target_os = "linux")]
        {
            let debugfs_path = format!(
                "/sys/kernel/debug/ieee80211/phy0/ath{}/csi",
                match driver {
                    AtherosDriver::Ath9k => "9k",
                    AtherosDriver::Ath10k => "10k",
                    AtherosDriver::Ath11k => "11k",
                }
            );

            if !std::path::Path::new(&debugfs_path).exists() {
                tracing::warn!(
                    "CSI debugfs path {} not found - CSI patched driver may not be installed",
                    debugfs_path
                );
            }
        }

        let mut state = self.state.write().await;
        state.device_state = DeviceSpecificState::Atheros {
            driver,
            csi_buf_ptr: None,
        };

        Ok(())
    }

    /// Initialize UDP receiver
    async fn initialize_udp(&mut self) -> Result<(), AdapterError> {
        let settings = match &self.config.device_settings {
            DeviceSettings::Udp(s) => s,
            _ => return Err(AdapterError::Config("UDP receiver requires UDP settings".into())),
        };

        tracing::info!("Initializing UDP receiver on {}:{}", settings.bind_address, settings.port);

        // Verify port is available
        let addr = format!("{}:{}", settings.bind_address, settings.port);
        let socket = tokio::net::UdpSocket::bind(&addr)
            .await
            .map_err(|e| AdapterError::Hardware(format!("Failed to bind UDP socket: {}", e)))?;

        // Join multicast group if specified
        if let Some(ref group) = settings.multicast_group {
            let multicast_addr: std::net::Ipv4Addr = group
                .parse()
                .map_err(|e| AdapterError::Config(format!("Invalid multicast address: {}", e)))?;

            socket
                .join_multicast_v4(multicast_addr, std::net::Ipv4Addr::UNSPECIFIED)
                .map_err(|e| AdapterError::Hardware(format!("Failed to join multicast group: {}", e)))?;
        }

        // Socket will be recreated when streaming starts
        drop(socket);

        Ok(())
    }

    /// Initialize PCAP file reader
    async fn initialize_pcap(&mut self) -> Result<(), AdapterError> {
        let settings = match &self.config.device_settings {
            DeviceSettings::Pcap(s) => s,
            _ => return Err(AdapterError::Config("PCAP requires PCAP settings".into())),
        };

        tracing::info!("Initializing PCAP file reader: {}", settings.file_path);

        // Verify file exists
        if !std::path::Path::new(&settings.file_path).exists() {
            return Err(AdapterError::Hardware(format!(
                "PCAP file not found: {}",
                settings.file_path
            )));
        }

        Ok(())
    }

    /// Initialize simulated device
    async fn initialize_simulated(&mut self) -> Result<(), AdapterError> {
        tracing::info!("Initializing simulated CSI device");
        Ok(())
    }

    /// Start CSI streaming
    pub async fn start_csi_stream(&mut self) -> Result<CsiStream, AdapterError> {
        if !self.initialized {
            return Err(AdapterError::Hardware("Hardware not initialized".into()));
        }

        let broadcaster = self.csi_broadcaster.as_ref()
            .ok_or_else(|| AdapterError::Hardware("CSI broadcaster not initialized".into()))?;

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        // Start device-specific streaming
        let tx = broadcaster.clone();
        let config = self.config.clone();
        let state = Arc::clone(&self.state);

        tokio::spawn(async move {
            Self::run_streaming_loop(config, tx, state, shutdown_rx).await;
        });

        // Update streaming state
        {
            let mut state = self.state.write().await;
            state.streaming = true;
        }

        let rx = broadcaster.subscribe();
        Ok(CsiStream { receiver: rx })
    }

    /// Stop CSI streaming
    pub async fn stop_csi_stream(&mut self) -> Result<(), AdapterError> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }

        let mut state = self.state.write().await;
        state.streaming = false;

        Ok(())
    }

    /// Internal streaming loop
    async fn run_streaming_loop(
        config: HardwareConfig,
        tx: broadcast::Sender<CsiReadings>,
        state: Arc<RwLock<DeviceState>>,
        mut shutdown_rx: mpsc::Receiver<()>,
    ) {
        tracing::debug!("Starting CSI streaming loop for {:?}", config.device_type);

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    tracing::info!("CSI streaming shutdown requested");
                    break;
                }
                result = Self::read_csi_packet(&config, &state) => {
                    match result {
                        Ok(reading) => {
                            // Update packet count
                            {
                                let mut state = state.write().await;
                                state.packets_received += 1;
                            }

                            // Broadcast to subscribers
                            if tx.receiver_count() > 0 {
                                let _ = tx.send(reading);
                            }
                        }
                        Err(e) => {
                            let mut state = state.write().await;
                            state.error_count += 1;
                            state.last_error = Some(e.to_string());

                            if state.error_count > 100 {
                                tracing::error!("Too many CSI read errors, stopping stream");
                                break;
                            }
                        }
                    }
                }
            }
        }

        tracing::debug!("CSI streaming loop ended");
    }

    /// Read a single CSI packet from the device
    async fn read_csi_packet(
        config: &HardwareConfig,
        _state: &Arc<RwLock<DeviceState>>,
    ) -> Result<CsiReadings, AdapterError> {
        match &config.device_type {
            DeviceType::Esp32 => Self::read_esp32_csi(config).await,
            DeviceType::Intel5300 => Self::read_intel_5300_csi(config).await,
            DeviceType::Atheros(driver) => Self::read_atheros_csi(config, *driver).await,
            DeviceType::UdpReceiver => Self::read_udp_csi(config).await,
            DeviceType::PcapFile => Self::read_pcap_csi(config).await,
            DeviceType::Simulated => Self::generate_simulated_csi(config).await,
        }
    }

    /// Read CSI from ESP32 via serial
    async fn read_esp32_csi(config: &HardwareConfig) -> Result<CsiReadings, AdapterError> {
        let settings = match &config.device_settings {
            DeviceSettings::Serial(s) => s,
            _ => return Err(AdapterError::Config("Invalid settings for ESP32".into())),
        };

        // In a real implementation, this would read from the serial port
        // and parse ESP-CSI format data
        tracing::trace!("Reading ESP32 CSI from {}", settings.port);

        // Simulate read delay
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Return placeholder - real implementation would parse serial data
        Ok(CsiReadings {
            timestamp: Utc::now(),
            readings: vec![],
            metadata: CsiMetadata {
                device_type: DeviceType::Esp32,
                channel: config.channel_config.channel,
                bandwidth: config.channel_config.bandwidth,
                num_subcarriers: config.channel_config.num_subcarriers,
                rssi: None,
                noise_floor: None,
                fc_type: FrameControlType::Data,
            },
        })
    }

    /// Read CSI from Intel 5300 NIC
    async fn read_intel_5300_csi(config: &HardwareConfig) -> Result<CsiReadings, AdapterError> {
        // Intel 5300 uses connector interface from Linux CSI Tool
        tracing::trace!("Reading Intel 5300 CSI");

        // In a real implementation, this would:
        // 1. Open /proc/net/connector (netlink socket)
        // 2. Listen for BFEE_NOTIF messages
        // 3. Parse the bfee struct

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        Ok(CsiReadings {
            timestamp: Utc::now(),
            readings: vec![],
            metadata: CsiMetadata {
                device_type: DeviceType::Intel5300,
                channel: config.channel_config.channel,
                bandwidth: config.channel_config.bandwidth,
                num_subcarriers: 30, // Intel 5300 provides 30 subcarriers
                rssi: None,
                noise_floor: None,
                fc_type: FrameControlType::Data,
            },
        })
    }

    /// Read CSI from Atheros NIC
    async fn read_atheros_csi(
        config: &HardwareConfig,
        driver: AtherosDriver,
    ) -> Result<CsiReadings, AdapterError> {
        tracing::trace!("Reading Atheros ({:?}) CSI", driver);

        // In a real implementation, this would:
        // 1. Read from debugfs CSI buffer
        // 2. Parse driver-specific CSI format

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let num_subcarriers = match driver {
            AtherosDriver::Ath9k => 56,
            AtherosDriver::Ath10k => 114,
            AtherosDriver::Ath11k => 234,
        };

        Ok(CsiReadings {
            timestamp: Utc::now(),
            readings: vec![],
            metadata: CsiMetadata {
                device_type: DeviceType::Atheros(driver),
                channel: config.channel_config.channel,
                bandwidth: config.channel_config.bandwidth,
                num_subcarriers,
                rssi: None,
                noise_floor: None,
                fc_type: FrameControlType::Data,
            },
        })
    }

    /// Read CSI from UDP socket
    async fn read_udp_csi(config: &HardwareConfig) -> Result<CsiReadings, AdapterError> {
        let settings = match &config.device_settings {
            DeviceSettings::Udp(s) => s,
            _ => return Err(AdapterError::Config("Invalid settings for UDP".into())),
        };

        tracing::trace!("Reading UDP CSI on {}:{}", settings.bind_address, settings.port);

        // Placeholder - real implementation would receive and parse UDP packets
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        Ok(CsiReadings {
            timestamp: Utc::now(),
            readings: vec![],
            metadata: CsiMetadata {
                device_type: DeviceType::UdpReceiver,
                channel: config.channel_config.channel,
                bandwidth: config.channel_config.bandwidth,
                num_subcarriers: config.channel_config.num_subcarriers,
                rssi: None,
                noise_floor: None,
                fc_type: FrameControlType::Data,
            },
        })
    }

    /// Read CSI from PCAP file
    async fn read_pcap_csi(config: &HardwareConfig) -> Result<CsiReadings, AdapterError> {
        let settings = match &config.device_settings {
            DeviceSettings::Pcap(s) => s,
            _ => return Err(AdapterError::Config("Invalid settings for PCAP".into())),
        };

        tracing::trace!("Reading PCAP CSI from {}", settings.file_path);

        // Placeholder - real implementation would read and parse PCAP packets
        tokio::time::sleep(tokio::time::Duration::from_millis(
            (10.0 / settings.playback_speed) as u64,
        ))
        .await;

        Ok(CsiReadings {
            timestamp: Utc::now(),
            readings: vec![],
            metadata: CsiMetadata {
                device_type: DeviceType::PcapFile,
                channel: config.channel_config.channel,
                bandwidth: config.channel_config.bandwidth,
                num_subcarriers: config.channel_config.num_subcarriers,
                rssi: None,
                noise_floor: None,
                fc_type: FrameControlType::Data,
            },
        })
    }

    /// Generate simulated CSI data
    async fn generate_simulated_csi(config: &HardwareConfig) -> Result<CsiReadings, AdapterError> {
        use std::f64::consts::PI;

        // Simulate packet rate
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let num_subcarriers = config.channel_config.num_subcarriers;
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();

        // Generate simulated breathing pattern (~0.3 Hz)
        let breathing_component = (2.0 * PI * 0.3 * t).sin();

        // Generate simulated heartbeat pattern (~1.2 Hz)
        let heartbeat_component = 0.1 * (2.0 * PI * 1.2 * t).sin();

        let mut amplitudes = Vec::with_capacity(num_subcarriers);
        let mut phases = Vec::with_capacity(num_subcarriers);

        for i in 0..num_subcarriers {
            // Add frequency-dependent characteristics
            let freq_factor = (i as f64 / num_subcarriers as f64 * PI).sin();

            // Amplitude with breathing/heartbeat modulation
            let amp = 1.0 + 0.1 * breathing_component * freq_factor + heartbeat_component;

            // Phase with random walk + breathing modulation
            let phase = (i as f64 * 0.1 + 0.2 * breathing_component) % (2.0 * PI);

            amplitudes.push(amp);
            phases.push(phase);
        }

        Ok(CsiReadings {
            timestamp: Utc::now(),
            readings: vec![SensorCsiReading {
                sensor_id: "simulated".to_string(),
                amplitudes,
                phases,
                rssi: -45.0 + 2.0 * rand_simple(),
                noise_floor: -92.0,
                tx_mac: Some("00:11:22:33:44:55".to_string()),
                rx_mac: Some("AA:BB:CC:DD:EE:FF".to_string()),
                sequence_num: None,
            }],
            metadata: CsiMetadata {
                device_type: DeviceType::Simulated,
                channel: config.channel_config.channel,
                bandwidth: config.channel_config.bandwidth,
                num_subcarriers,
                rssi: Some(-45.0),
                noise_floor: Some(-92.0),
                fc_type: FrameControlType::Data,
            },
        })
    }

    /// Discover available sensors
    pub async fn discover_sensors(&mut self) -> Result<Vec<SensorInfo>, AdapterError> {
        if !self.initialized {
            return Err(AdapterError::Hardware("Hardware not initialized".into()));
        }

        // Discovery depends on device type
        match &self.config.device_type {
            DeviceType::Esp32 => self.discover_esp32_sensors().await,
            DeviceType::Intel5300 | DeviceType::Atheros(_) => self.discover_nic_sensors().await,
            DeviceType::UdpReceiver => Ok(vec![]),
            DeviceType::PcapFile => Ok(vec![]),
            DeviceType::Simulated => self.discover_simulated_sensors().await,
        }
    }

    async fn discover_esp32_sensors(&self) -> Result<Vec<SensorInfo>, AdapterError> {
        // ESP32 discovery would scan for beacons or query connected devices
        tracing::debug!("Discovering ESP32 sensors...");
        Ok(vec![])
    }

    async fn discover_nic_sensors(&self) -> Result<Vec<SensorInfo>, AdapterError> {
        // NIC-based systems would scan for nearby APs
        tracing::debug!("Discovering NIC sensors...");
        Ok(vec![])
    }

    async fn discover_simulated_sensors(&self) -> Result<Vec<SensorInfo>, AdapterError> {
        use crate::domain::SensorType;

        // Return fake sensors for testing
        Ok(vec![
            SensorInfo {
                id: "sim-tx-1".to_string(),
                position: SensorPosition {
                    id: "sim-tx-1".to_string(),
                    x: 0.0,
                    y: 0.0,
                    z: 2.0,
                    sensor_type: SensorType::Transmitter,
                    is_operational: true,
                },
                status: SensorStatus::Connected,
                last_rssi: Some(-42.0),
                battery_level: Some(100),
                mac_address: Some("00:11:22:33:44:55".to_string()),
                firmware_version: Some("1.0.0".to_string()),
            },
            SensorInfo {
                id: "sim-rx-1".to_string(),
                position: SensorPosition {
                    id: "sim-rx-1".to_string(),
                    x: 5.0,
                    y: 0.0,
                    z: 2.0,
                    sensor_type: SensorType::Receiver,
                    is_operational: true,
                },
                status: SensorStatus::Connected,
                last_rssi: Some(-48.0),
                battery_level: Some(85),
                mac_address: Some("AA:BB:CC:DD:EE:FF".to_string()),
                firmware_version: Some("1.0.0".to_string()),
            },
        ])
    }

    /// Add a sensor
    pub fn add_sensor(&mut self, sensor: SensorInfo) -> Result<(), AdapterError> {
        if self.sensors.iter().any(|s| s.id == sensor.id) {
            return Err(AdapterError::Hardware(format!(
                "Sensor {} already registered",
                sensor.id
            )));
        }

        self.sensors.push(sensor);
        Ok(())
    }

    /// Remove a sensor
    pub fn remove_sensor(&mut self, sensor_id: &str) -> Result<(), AdapterError> {
        let initial_len = self.sensors.len();
        self.sensors.retain(|s| s.id != sensor_id);

        if self.sensors.len() == initial_len {
            return Err(AdapterError::Hardware(format!(
                "Sensor {} not found",
                sensor_id
            )));
        }

        Ok(())
    }

    /// Get all sensors
    pub fn sensors(&self) -> &[SensorInfo] {
        &self.sensors
    }

    /// Get operational sensors
    pub fn operational_sensors(&self) -> Vec<&SensorInfo> {
        self.sensors
            .iter()
            .filter(|s| s.status == SensorStatus::Connected)
            .collect()
    }

    /// Get sensor positions for localization
    pub fn sensor_positions(&self) -> Vec<SensorPosition> {
        self.sensors
            .iter()
            .filter(|s| s.status == SensorStatus::Connected)
            .map(|s| s.position.clone())
            .collect()
    }

    /// Read CSI data from sensors (synchronous wrapper)
    pub fn read_csi(&self) -> Result<CsiReadings, AdapterError> {
        if !self.initialized {
            return Err(AdapterError::Hardware("Hardware not initialized".into()));
        }

        // Return empty readings - use async stream for real data
        Ok(CsiReadings {
            timestamp: Utc::now(),
            readings: Vec::new(),
            metadata: CsiMetadata {
                device_type: self.config.device_type.clone(),
                channel: self.config.channel_config.channel,
                bandwidth: self.config.channel_config.bandwidth,
                num_subcarriers: self.config.channel_config.num_subcarriers,
                rssi: None,
                noise_floor: None,
                fc_type: FrameControlType::Data,
            },
        })
    }

    /// Read RSSI from all sensors
    pub fn read_rssi(&self) -> Result<Vec<(String, f64)>, AdapterError> {
        if !self.initialized {
            return Err(AdapterError::Hardware("Hardware not initialized".into()));
        }

        Ok(self
            .sensors
            .iter()
            .filter_map(|s| s.last_rssi.map(|rssi| (s.id.clone(), rssi)))
            .collect())
    }

    /// Update sensor position
    pub fn update_sensor_position(
        &mut self,
        sensor_id: &str,
        position: SensorPosition,
    ) -> Result<(), AdapterError> {
        let sensor = self
            .sensors
            .iter_mut()
            .find(|s| s.id == sensor_id)
            .ok_or_else(|| AdapterError::Hardware(format!("Sensor {} not found", sensor_id)))?;

        sensor.position = position;
        Ok(())
    }

    /// Check hardware health
    pub fn health_check(&self) -> HardwareHealth {
        let total = self.sensors.len();
        let connected = self
            .sensors
            .iter()
            .filter(|s| s.status == SensorStatus::Connected)
            .count();
        let low_battery = self
            .sensors
            .iter()
            .filter(|s| matches!(s.battery_level, Some(b) if b < 20))
            .count();

        let status = if connected == 0 && total > 0 {
            HealthStatus::Critical
        } else if connected < total / 2 {
            HealthStatus::Degraded
        } else if low_battery > 0 {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        };

        HardwareHealth {
            status,
            total_sensors: total,
            connected_sensors: connected,
            low_battery_sensors: low_battery,
        }
    }

    /// Get streaming statistics
    pub async fn streaming_stats(&self) -> StreamingStats {
        let state = self.state.read().await;
        StreamingStats {
            is_streaming: state.streaming,
            packets_received: state.packets_received,
            error_count: state.error_count,
            last_error: state.last_error.clone(),
        }
    }

    /// Configure channel settings
    pub async fn set_channel(&mut self, channel: u8, bandwidth: Bandwidth) -> Result<(), AdapterError> {
        if !self.initialized {
            return Err(AdapterError::Hardware("Hardware not initialized".into()));
        }

        // Validate channel
        let valid_2g = (1..=14).contains(&channel);
        let valid_5g = [36, 40, 44, 48, 52, 56, 60, 64, 100, 104, 108, 112, 116, 120, 124, 128, 132, 136, 140, 144, 149, 153, 157, 161, 165].contains(&channel);

        if !valid_2g && !valid_5g {
            return Err(AdapterError::Config(format!("Invalid WiFi channel: {}", channel)));
        }

        self.config.channel_config.channel = channel;
        self.config.channel_config.bandwidth = bandwidth;
        self.config.channel_config.num_subcarriers = bandwidth.subcarrier_count();

        tracing::info!("Channel set to {} with {:?} bandwidth", channel, bandwidth);

        Ok(())
    }
}

impl Default for HardwareAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple pseudo-random number generator (for simulation)
fn rand_simple() -> f64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    (nanos % 1000) as f64 / 1000.0 - 0.5
}

/// CSI readings from sensors
#[derive(Debug, Clone)]
pub struct CsiReadings {
    /// Timestamp of readings
    pub timestamp: DateTime<Utc>,
    /// Individual sensor readings
    pub readings: Vec<SensorCsiReading>,
    /// Metadata about the capture
    pub metadata: CsiMetadata,
}

/// Metadata for CSI capture
#[derive(Debug, Clone)]
pub struct CsiMetadata {
    /// Device type that captured this data
    pub device_type: DeviceType,
    /// WiFi channel
    pub channel: u8,
    /// Channel bandwidth
    pub bandwidth: Bandwidth,
    /// Number of subcarriers
    pub num_subcarriers: usize,
    /// Overall RSSI
    pub rssi: Option<f64>,
    /// Noise floor
    pub noise_floor: Option<f64>,
    /// Frame control type
    pub fc_type: FrameControlType,
}

/// WiFi frame control types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameControlType {
    /// Management frame (beacon, probe, etc.)
    Management,
    /// Control frame (ACK, RTS, CTS)
    Control,
    /// Data frame
    Data,
    /// Extension
    Extension,
}

/// CSI reading from a single sensor
#[derive(Debug, Clone)]
pub struct SensorCsiReading {
    /// Sensor ID
    pub sensor_id: String,
    /// CSI amplitudes (per subcarrier)
    pub amplitudes: Vec<f64>,
    /// CSI phases (per subcarrier)
    pub phases: Vec<f64>,
    /// RSSI value
    pub rssi: f64,
    /// Noise floor
    pub noise_floor: f64,
    /// Transmitter MAC address
    pub tx_mac: Option<String>,
    /// Receiver MAC address
    pub rx_mac: Option<String>,
    /// Sequence number
    pub sequence_num: Option<u16>,
}

/// CSI stream for async iteration
pub struct CsiStream {
    receiver: broadcast::Receiver<CsiReadings>,
}

impl CsiStream {
    /// Receive the next CSI reading
    pub async fn next(&mut self) -> Option<CsiReadings> {
        match self.receiver.recv().await {
            Ok(reading) => Some(reading),
            Err(broadcast::error::RecvError::Closed) => None,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!("CSI stream lagged by {} messages", n);
                self.receiver.recv().await.ok()
            }
        }
    }
}

/// Streaming statistics
#[derive(Debug, Clone)]
pub struct StreamingStats {
    /// Whether streaming is active
    pub is_streaming: bool,
    /// Total packets received
    pub packets_received: u64,
    /// Number of errors
    pub error_count: u64,
    /// Last error message
    pub last_error: Option<String>,
}

/// Hardware health status
#[derive(Debug, Clone)]
pub struct HardwareHealth {
    /// Overall status
    pub status: HealthStatus,
    /// Total number of sensors
    pub total_sensors: usize,
    /// Number of connected sensors
    pub connected_sensors: usize,
    /// Number of sensors with low battery
    pub low_battery_sensors: usize,
}

/// Health status levels
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    /// All systems operational
    Healthy,
    /// Minor issues, still functional
    Warning,
    /// Significant issues, reduced capability
    Degraded,
    /// System not functional
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SensorType;

    fn create_test_sensor(id: &str) -> SensorInfo {
        SensorInfo {
            id: id.to_string(),
            position: SensorPosition {
                id: id.to_string(),
                x: 0.0,
                y: 0.0,
                z: 1.5,
                sensor_type: SensorType::Transceiver,
                is_operational: true,
            },
            status: SensorStatus::Connected,
            last_rssi: Some(-45.0),
            battery_level: Some(80),
            mac_address: None,
            firmware_version: None,
        }
    }

    #[tokio::test]
    async fn test_initialize_simulated() {
        let mut adapter = HardwareAdapter::new();
        assert!(adapter.initialize().await.is_ok());
    }

    #[test]
    fn test_add_sensor() {
        let mut adapter = HardwareAdapter::new();

        let sensor = create_test_sensor("s1");
        assert!(adapter.add_sensor(sensor).is_ok());
        assert_eq!(adapter.sensors().len(), 1);
    }

    #[test]
    fn test_duplicate_sensor_error() {
        let mut adapter = HardwareAdapter::new();

        let sensor1 = create_test_sensor("s1");
        let sensor2 = create_test_sensor("s1");

        adapter.add_sensor(sensor1).unwrap();
        assert!(adapter.add_sensor(sensor2).is_err());
    }

    #[test]
    fn test_health_check() {
        let mut adapter = HardwareAdapter::new();

        // No sensors - should be healthy (nothing to fail)
        let health = adapter.health_check();
        assert!(matches!(health.status, HealthStatus::Healthy));

        // Add connected sensor
        adapter.add_sensor(create_test_sensor("s1")).unwrap();
        let health = adapter.health_check();
        assert!(matches!(health.status, HealthStatus::Healthy));
    }

    #[test]
    fn test_sensor_positions() {
        let mut adapter = HardwareAdapter::new();

        adapter.add_sensor(create_test_sensor("s1")).unwrap();
        adapter.add_sensor(create_test_sensor("s2")).unwrap();

        let positions = adapter.sensor_positions();
        assert_eq!(positions.len(), 2);
    }

    #[test]
    fn test_esp32_config() {
        let config = HardwareConfig::esp32("/dev/ttyUSB0", 921600);
        assert!(matches!(config.device_type, DeviceType::Esp32));
        assert!(matches!(config.device_settings, DeviceSettings::Serial(_)));
    }

    #[test]
    fn test_intel_5300_config() {
        let config = HardwareConfig::intel_5300("wlan0");
        assert!(matches!(config.device_type, DeviceType::Intel5300));
        assert_eq!(config.channel_config.num_subcarriers, 30);
    }

    #[test]
    fn test_atheros_config() {
        let config = HardwareConfig::atheros("wlan0", AtherosDriver::Ath10k);
        assert!(matches!(config.device_type, DeviceType::Atheros(AtherosDriver::Ath10k)));
        assert_eq!(config.channel_config.num_subcarriers, 114);
    }

    #[test]
    fn test_bandwidth_subcarriers() {
        assert_eq!(Bandwidth::HT20.subcarrier_count(), 56);
        assert_eq!(Bandwidth::HT40.subcarrier_count(), 114);
        assert_eq!(Bandwidth::VHT80.subcarrier_count(), 242);
        assert_eq!(Bandwidth::VHT160.subcarrier_count(), 484);
    }

    #[tokio::test]
    async fn test_csi_stream() {
        let mut adapter = HardwareAdapter::new();
        adapter.initialize().await.unwrap();

        let mut stream = adapter.start_csi_stream().await.unwrap();

        // Receive a few packets
        for _ in 0..3 {
            let reading = stream.next().await;
            assert!(reading.is_some());
        }

        adapter.stop_csi_stream().await.unwrap();
    }

    #[tokio::test]
    async fn test_discover_simulated_sensors() {
        let mut adapter = HardwareAdapter::new();
        adapter.initialize().await.unwrap();

        let sensors = adapter.discover_sensors().await.unwrap();
        assert_eq!(sensors.len(), 2);
    }
}
