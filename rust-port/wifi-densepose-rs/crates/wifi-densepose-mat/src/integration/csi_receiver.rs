//! CSI packet receivers for different input sources.
//!
//! This module provides receivers for:
//! - UDP packets (network streaming from remote sensors)
//! - Serial port (ESP32 and similar embedded devices)
//! - PCAP files (offline analysis and replay)
//!
//! # Example
//!
//! ```ignore
//! use wifi_densepose_mat::integration::csi_receiver::{
//!     UdpCsiReceiver, ReceiverConfig, CsiPacketFormat,
//! };
//!
//! let config = ReceiverConfig::udp("0.0.0.0", 5500);
//! let mut receiver = UdpCsiReceiver::new(config)?;
//!
//! while let Some(packet) = receiver.receive().await? {
//!     println!("Received CSI packet: {:?}", packet.metadata);
//! }
//! ```

use super::AdapterError;
use super::hardware_adapter::{
    Bandwidth, CsiMetadata, CsiReadings, DeviceType, FrameControlType, SensorCsiReading,
};
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// Configuration for CSI receivers
#[derive(Debug, Clone)]
pub struct ReceiverConfig {
    /// Input source type
    pub source: CsiSource,
    /// Expected packet format
    pub format: CsiPacketFormat,
    /// Buffer size for incoming data
    pub buffer_size: usize,
    /// Maximum packets to queue
    pub queue_size: usize,
    /// Timeout for receive operations (ms)
    pub timeout_ms: u64,
}

impl Default for ReceiverConfig {
    fn default() -> Self {
        Self {
            source: CsiSource::Udp(UdpSourceConfig::default()),
            format: CsiPacketFormat::Auto,
            buffer_size: 65536,
            queue_size: 1000,
            timeout_ms: 5000,
        }
    }
}

impl ReceiverConfig {
    /// Create UDP receiver configuration
    pub fn udp(bind_addr: &str, port: u16) -> Self {
        Self {
            source: CsiSource::Udp(UdpSourceConfig {
                bind_address: bind_addr.to_string(),
                port,
                multicast_group: None,
            }),
            ..Default::default()
        }
    }

    /// Create serial receiver configuration
    pub fn serial(port: &str, baud_rate: u32) -> Self {
        Self {
            source: CsiSource::Serial(SerialSourceConfig {
                port: port.to_string(),
                baud_rate,
                data_bits: 8,
                stop_bits: 1,
                parity: SerialParity::None,
            }),
            format: CsiPacketFormat::Esp32Csi,
            ..Default::default()
        }
    }

    /// Create PCAP file reader configuration
    pub fn pcap(file_path: &str) -> Self {
        Self {
            source: CsiSource::Pcap(PcapSourceConfig {
                file_path: file_path.to_string(),
                playback_speed: 1.0,
                loop_playback: false,
                start_offset: 0,
            }),
            format: CsiPacketFormat::Auto,
            ..Default::default()
        }
    }
}

/// CSI data source types
#[derive(Debug, Clone)]
pub enum CsiSource {
    /// UDP network source
    Udp(UdpSourceConfig),
    /// Serial port source
    Serial(SerialSourceConfig),
    /// PCAP file source
    Pcap(PcapSourceConfig),
}

/// UDP source configuration
#[derive(Debug, Clone)]
pub struct UdpSourceConfig {
    /// Address to bind
    pub bind_address: String,
    /// Port number
    pub port: u16,
    /// Multicast group to join (optional)
    pub multicast_group: Option<String>,
}

impl Default for UdpSourceConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 5500,
            multicast_group: None,
        }
    }
}

/// Serial source configuration
#[derive(Debug, Clone)]
pub struct SerialSourceConfig {
    /// Serial port path
    pub port: String,
    /// Baud rate
    pub baud_rate: u32,
    /// Data bits (5-8)
    pub data_bits: u8,
    /// Stop bits (1, 2)
    pub stop_bits: u8,
    /// Parity setting
    pub parity: SerialParity,
}

/// Serial parity options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialParity {
    None,
    Odd,
    Even,
}

/// PCAP source configuration
#[derive(Debug, Clone)]
pub struct PcapSourceConfig {
    /// Path to PCAP file
    pub file_path: String,
    /// Playback speed multiplier (1.0 = realtime)
    pub playback_speed: f64,
    /// Loop playback when reaching end
    pub loop_playback: bool,
    /// Start offset in bytes
    pub start_offset: u64,
}

/// Supported CSI packet formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CsiPacketFormat {
    /// Auto-detect format
    Auto,
    /// ESP32 CSI format (ESP-CSI firmware)
    Esp32Csi,
    /// Intel 5300 BFEE format (Linux CSI Tool)
    Intel5300Bfee,
    /// Atheros CSI format
    AtherosCsi,
    /// Nexmon CSI format (Broadcom)
    NexmonCsi,
    /// PicoScenes format
    PicoScenes,
    /// Generic JSON format
    JsonCsi,
    /// Raw binary format
    RawBinary,
}

/// Parsed CSI packet
#[derive(Debug, Clone)]
pub struct CsiPacket {
    /// Timestamp of packet
    pub timestamp: DateTime<Utc>,
    /// Source identifier
    pub source_id: String,
    /// CSI amplitude values per subcarrier
    pub amplitudes: Vec<f64>,
    /// CSI phase values per subcarrier
    pub phases: Vec<f64>,
    /// RSSI value
    pub rssi: i8,
    /// Noise floor
    pub noise_floor: i8,
    /// Packet metadata
    pub metadata: CsiPacketMetadata,
    /// Raw packet data (if preserved)
    pub raw_data: Option<Vec<u8>>,
}

/// Metadata for a CSI packet
#[derive(Debug, Clone)]
pub struct CsiPacketMetadata {
    /// Transmitter MAC address
    pub tx_mac: [u8; 6],
    /// Receiver MAC address
    pub rx_mac: [u8; 6],
    /// WiFi channel
    pub channel: u8,
    /// Channel bandwidth
    pub bandwidth: Bandwidth,
    /// Number of transmit streams (Ntx)
    pub ntx: u8,
    /// Number of receive streams (Nrx)
    pub nrx: u8,
    /// Sequence number
    pub sequence_num: u16,
    /// Frame control field
    pub frame_control: u16,
    /// Rate/MCS index
    pub rate: u8,
    /// Secondary channel offset
    pub secondary_channel: i8,
    /// Packet format
    pub format: CsiPacketFormat,
}

impl Default for CsiPacketMetadata {
    fn default() -> Self {
        Self {
            tx_mac: [0; 6],
            rx_mac: [0; 6],
            channel: 6,
            bandwidth: Bandwidth::HT20,
            ntx: 1,
            nrx: 3,
            sequence_num: 0,
            frame_control: 0,
            rate: 0,
            secondary_channel: 0,
            format: CsiPacketFormat::Auto,
        }
    }
}

/// UDP CSI receiver
pub struct UdpCsiReceiver {
    config: ReceiverConfig,
    socket: Option<tokio::net::UdpSocket>,
    buffer: Vec<u8>,
    parser: CsiParser,
    stats: ReceiverStats,
}

impl UdpCsiReceiver {
    /// Create a new UDP receiver
    pub async fn new(config: ReceiverConfig) -> Result<Self, AdapterError> {
        let udp_config = match &config.source {
            CsiSource::Udp(c) => c,
            _ => return Err(AdapterError::Config("Invalid config for UDP receiver".into())),
        };

        let addr = format!("{}:{}", udp_config.bind_address, udp_config.port);
        let socket = tokio::net::UdpSocket::bind(&addr)
            .await
            .map_err(|e| AdapterError::Hardware(format!("Failed to bind UDP socket: {}", e)))?;

        // Join multicast if specified
        if let Some(ref group) = udp_config.multicast_group {
            let multicast_addr: std::net::Ipv4Addr = group
                .parse()
                .map_err(|e| AdapterError::Config(format!("Invalid multicast address: {}", e)))?;

            socket
                .join_multicast_v4(multicast_addr, std::net::Ipv4Addr::UNSPECIFIED)
                .map_err(|e| AdapterError::Hardware(format!("Failed to join multicast: {}", e)))?;

            tracing::info!("Joined multicast group {}", group);
        }

        tracing::info!("UDP receiver bound to {}", addr);

        Ok(Self {
            buffer: vec![0u8; config.buffer_size],
            parser: CsiParser::new(config.format),
            stats: ReceiverStats::default(),
            config,
            socket: Some(socket),
        })
    }

    /// Receive next CSI packet
    pub async fn receive(&mut self) -> Result<Option<CsiPacket>, AdapterError> {
        let socket = self
            .socket
            .as_ref()
            .ok_or_else(|| AdapterError::Hardware("Socket not initialized".into()))?;

        let timeout = tokio::time::Duration::from_millis(self.config.timeout_ms);

        match tokio::time::timeout(timeout, socket.recv_from(&mut self.buffer)).await {
            Ok(Ok((len, addr))) => {
                self.stats.packets_received += 1;
                self.stats.bytes_received += len as u64;

                let data = &self.buffer[..len];

                match self.parser.parse(data) {
                    Ok(packet) => {
                        self.stats.packets_parsed += 1;
                        Ok(Some(packet))
                    }
                    Err(e) => {
                        self.stats.parse_errors += 1;
                        tracing::debug!("Failed to parse packet from {}: {}", addr, e);
                        Ok(None)
                    }
                }
            }
            Ok(Err(e)) => Err(AdapterError::Hardware(format!("Socket receive error: {}", e))),
            Err(_) => Ok(None), // Timeout
        }
    }

    /// Get receiver statistics
    pub fn stats(&self) -> &ReceiverStats {
        &self.stats
    }

    /// Close the receiver
    pub async fn close(&mut self) {
        self.socket = None;
    }
}

/// Serial CSI receiver
pub struct SerialCsiReceiver {
    config: ReceiverConfig,
    port_path: String,
    buffer: VecDeque<u8>,
    parser: CsiParser,
    stats: ReceiverStats,
    running: bool,
}

impl SerialCsiReceiver {
    /// Create a new serial receiver
    pub fn new(config: ReceiverConfig) -> Result<Self, AdapterError> {
        let serial_config = match &config.source {
            CsiSource::Serial(c) => c,
            _ => return Err(AdapterError::Config("Invalid config for serial receiver".into())),
        };

        // Verify port exists
        #[cfg(unix)]
        {
            if !Path::new(&serial_config.port).exists() {
                return Err(AdapterError::Hardware(format!(
                    "Serial port {} not found",
                    serial_config.port
                )));
            }
        }

        tracing::info!(
            "Serial receiver configured for {} at {} baud",
            serial_config.port,
            serial_config.baud_rate
        );

        Ok(Self {
            port_path: serial_config.port.clone(),
            buffer: VecDeque::with_capacity(config.buffer_size),
            parser: CsiParser::new(config.format),
            stats: ReceiverStats::default(),
            running: false,
            config,
        })
    }

    /// Start receiving (blocking, typically run in separate thread)
    pub fn start(&mut self) -> Result<(), AdapterError> {
        self.running = true;
        // In production, this would open the serial port using serialport crate
        // and start reading data
        Ok(())
    }

    /// Receive next CSI packet (non-blocking if data available)
    pub fn receive(&mut self) -> Result<Option<CsiPacket>, AdapterError> {
        if !self.running {
            return Err(AdapterError::Hardware("Receiver not started".into()));
        }

        // Try to parse a complete packet from buffer
        if let Some(packet_data) = self.extract_packet_from_buffer() {
            self.stats.packets_received += 1;

            match self.parser.parse(&packet_data) {
                Ok(packet) => {
                    self.stats.packets_parsed += 1;
                    return Ok(Some(packet));
                }
                Err(e) => {
                    self.stats.parse_errors += 1;
                    tracing::debug!("Failed to parse serial packet: {}", e);
                }
            }
        }

        Ok(None)
    }

    /// Extract a complete packet from the buffer
    fn extract_packet_from_buffer(&mut self) -> Option<Vec<u8>> {
        // Look for packet delimiter based on format
        match self.config.format {
            CsiPacketFormat::Esp32Csi => self.extract_esp32_packet(),
            CsiPacketFormat::JsonCsi => self.extract_json_packet(),
            _ => self.extract_newline_delimited(),
        }
    }

    /// Extract ESP32 CSI packet (CSV format with newline delimiter)
    fn extract_esp32_packet(&mut self) -> Option<Vec<u8>> {
        // ESP32 CSI uses newline-delimited CSV
        self.extract_newline_delimited()
    }

    /// Extract JSON packet
    fn extract_json_packet(&mut self) -> Option<Vec<u8>> {
        // Look for complete JSON object
        let mut depth = 0;
        let mut start = None;
        let mut end = None;

        for (i, &byte) in self.buffer.iter().enumerate() {
            if byte == b'{' {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            } else if byte == b'}' {
                depth -= 1;
                if depth == 0 && start.is_some() {
                    end = Some(i + 1);
                    break;
                }
            }
        }

        if let (Some(s), Some(e)) = (start, end) {
            let packet: Vec<u8> = self.buffer.drain(..e).skip(s).collect();
            return Some(packet);
        }

        None
    }

    /// Extract newline-delimited packet
    fn extract_newline_delimited(&mut self) -> Option<Vec<u8>> {
        if let Some(pos) = self.buffer.iter().position(|&b| b == b'\n') {
            let packet: Vec<u8> = self.buffer.drain(..=pos).collect();
            return Some(packet);
        }
        None
    }

    /// Add data to receive buffer (called from read thread)
    pub fn feed_data(&mut self, data: &[u8]) {
        self.buffer.extend(data);
        self.stats.bytes_received += data.len() as u64;
    }

    /// Stop receiving
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Get receiver statistics
    pub fn stats(&self) -> &ReceiverStats {
        &self.stats
    }
}

/// PCAP file CSI reader
pub struct PcapCsiReader {
    config: ReceiverConfig,
    file_path: String,
    parser: CsiParser,
    stats: ReceiverStats,
    packets: Vec<PcapPacket>,
    current_index: usize,
    start_time: Option<DateTime<Utc>>,
    playback_time: Option<DateTime<Utc>>,
}

/// Internal PCAP packet representation
struct PcapPacket {
    timestamp: DateTime<Utc>,
    data: Vec<u8>,
}

impl PcapCsiReader {
    /// Create a new PCAP reader
    pub fn new(config: ReceiverConfig) -> Result<Self, AdapterError> {
        let pcap_config = match &config.source {
            CsiSource::Pcap(c) => c,
            _ => return Err(AdapterError::Config("Invalid config for PCAP reader".into())),
        };

        if !Path::new(&pcap_config.file_path).exists() {
            return Err(AdapterError::Hardware(format!(
                "PCAP file not found: {}",
                pcap_config.file_path
            )));
        }

        tracing::info!("PCAP reader configured for {}", pcap_config.file_path);

        Ok(Self {
            file_path: pcap_config.file_path.clone(),
            parser: CsiParser::new(config.format),
            stats: ReceiverStats::default(),
            packets: Vec::new(),
            current_index: 0,
            start_time: None,
            playback_time: None,
            config,
        })
    }

    /// Load PCAP file into memory
    pub fn load(&mut self) -> Result<usize, AdapterError> {
        tracing::info!("Loading PCAP file: {}", self.file_path);

        let file = std::fs::File::open(&self.file_path)
            .map_err(|e| AdapterError::Hardware(format!("Failed to open PCAP file: {}", e)))?;

        let mut reader = BufReader::new(file);

        // Read PCAP global header
        let global_header = self.read_pcap_global_header(&mut reader)?;

        tracing::debug!(
            "PCAP file: magic={:08x}, version={}.{}, snaplen={}",
            global_header.magic,
            global_header.version_major,
            global_header.version_minor,
            global_header.snaplen
        );

        // Determine byte order from magic number
        let swapped = global_header.magic == 0xD4C3B2A1 || global_header.magic == 0x4D3CB2A1;

        // Read all packets
        self.packets.clear();
        let mut packet_count = 0;

        loop {
            match self.read_pcap_packet(&mut reader, swapped) {
                Ok(Some(packet)) => {
                    self.packets.push(packet);
                    packet_count += 1;
                }
                Ok(None) => break, // EOF
                Err(e) => {
                    tracing::warn!("Error reading packet {}: {}", packet_count, e);
                    break;
                }
            }
        }

        self.stats.packets_received = packet_count as u64;
        tracing::info!("Loaded {} packets from PCAP file", packet_count);

        Ok(packet_count)
    }

    /// Read PCAP global header
    fn read_pcap_global_header<R: Read>(
        &self,
        reader: &mut R,
    ) -> Result<PcapGlobalHeader, AdapterError> {
        let mut buf = [0u8; 24];
        reader
            .read_exact(&mut buf)
            .map_err(|e| AdapterError::Hardware(format!("Failed to read PCAP header: {}", e)))?;

        Ok(PcapGlobalHeader {
            magic: u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            version_major: u16::from_le_bytes([buf[4], buf[5]]),
            version_minor: u16::from_le_bytes([buf[6], buf[7]]),
            thiszone: i32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
            sigfigs: u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            snaplen: u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]),
            network: u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]),
        })
    }

    /// Read a single PCAP packet
    fn read_pcap_packet<R: Read>(
        &self,
        reader: &mut R,
        swapped: bool,
    ) -> Result<Option<PcapPacket>, AdapterError> {
        // Read packet header
        let mut header_buf = [0u8; 16];
        match reader.read_exact(&mut header_buf) {
            Ok(_) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => {
                return Err(AdapterError::Hardware(format!(
                    "Failed to read packet header: {}",
                    e
                )))
            }
        }

        let (ts_sec, ts_usec, incl_len, _orig_len) = if swapped {
            (
                u32::from_be_bytes([header_buf[0], header_buf[1], header_buf[2], header_buf[3]]),
                u32::from_be_bytes([header_buf[4], header_buf[5], header_buf[6], header_buf[7]]),
                u32::from_be_bytes([header_buf[8], header_buf[9], header_buf[10], header_buf[11]]),
                u32::from_be_bytes([
                    header_buf[12],
                    header_buf[13],
                    header_buf[14],
                    header_buf[15],
                ]),
            )
        } else {
            (
                u32::from_le_bytes([header_buf[0], header_buf[1], header_buf[2], header_buf[3]]),
                u32::from_le_bytes([header_buf[4], header_buf[5], header_buf[6], header_buf[7]]),
                u32::from_le_bytes([header_buf[8], header_buf[9], header_buf[10], header_buf[11]]),
                u32::from_le_bytes([
                    header_buf[12],
                    header_buf[13],
                    header_buf[14],
                    header_buf[15],
                ]),
            )
        };

        // Read packet data
        let mut data = vec![0u8; incl_len as usize];
        reader.read_exact(&mut data).map_err(|e| {
            AdapterError::Hardware(format!("Failed to read packet data: {}", e))
        })?;

        // Convert timestamp
        let timestamp = chrono::DateTime::from_timestamp(ts_sec as i64, ts_usec * 1000)
            .unwrap_or_else(Utc::now);

        Ok(Some(PcapPacket { timestamp, data }))
    }

    /// Read next CSI packet with timing
    pub async fn read_next(&mut self) -> Result<Option<CsiPacket>, AdapterError> {
        if self.current_index >= self.packets.len() {
            let pcap_config = match &self.config.source {
                CsiSource::Pcap(c) => c,
                _ => return Ok(None),
            };

            if pcap_config.loop_playback {
                self.current_index = 0;
                self.start_time = None;
                self.playback_time = None;
            } else {
                return Ok(None);
            }
        }

        let packet = &self.packets[self.current_index];

        // Initialize timing on first packet
        if self.start_time.is_none() {
            self.start_time = Some(packet.timestamp);
            self.playback_time = Some(Utc::now());
        }

        // Calculate delay for realtime playback
        let pcap_config = match &self.config.source {
            CsiSource::Pcap(c) => c,
            _ => return Ok(None),
        };

        if pcap_config.playback_speed > 0.0 {
            let packet_offset = packet.timestamp - self.start_time.unwrap();
            let real_offset = Utc::now() - self.playback_time.unwrap();
            let scaled_offset = packet_offset
                .num_milliseconds()
                .checked_div((pcap_config.playback_speed * 1000.0) as i64)
                .unwrap_or(0);

            let delay_ms = scaled_offset - real_offset.num_milliseconds();
            if delay_ms > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms as u64)).await;
            }
        }

        // Parse the packet
        let result = match self.parser.parse(&packet.data) {
            Ok(mut csi_packet) => {
                csi_packet.timestamp = packet.timestamp;
                self.stats.packets_parsed += 1;
                Ok(Some(csi_packet))
            }
            Err(e) => {
                self.stats.parse_errors += 1;
                tracing::debug!("Failed to parse PCAP packet: {}", e);
                Ok(None)
            }
        };

        self.current_index += 1;
        result
    }

    /// Reset playback to beginning
    pub fn reset(&mut self) {
        self.current_index = 0;
        self.start_time = None;
        self.playback_time = None;
    }

    /// Get current position
    pub fn position(&self) -> (usize, usize) {
        (self.current_index, self.packets.len())
    }

    /// Seek to specific packet index
    pub fn seek(&mut self, index: usize) -> Result<(), AdapterError> {
        if index >= self.packets.len() {
            return Err(AdapterError::Config(format!(
                "Seek index {} out of range (max {})",
                index,
                self.packets.len()
            )));
        }
        self.current_index = index;
        self.start_time = None;
        self.playback_time = None;
        Ok(())
    }

    /// Get receiver statistics
    pub fn stats(&self) -> &ReceiverStats {
        &self.stats
    }
}

/// PCAP global header structure
struct PcapGlobalHeader {
    magic: u32,
    version_major: u16,
    version_minor: u16,
    thiszone: i32,
    sigfigs: u32,
    snaplen: u32,
    network: u32,
}

/// CSI packet parser
pub struct CsiParser {
    format: CsiPacketFormat,
}

impl CsiParser {
    /// Create a new parser
    pub fn new(format: CsiPacketFormat) -> Self {
        Self { format }
    }

    /// Parse raw data into CSI packet
    pub fn parse(&self, data: &[u8]) -> Result<CsiPacket, AdapterError> {
        let format = if self.format == CsiPacketFormat::Auto {
            self.detect_format(data)
        } else {
            self.format
        };

        match format {
            CsiPacketFormat::Esp32Csi => self.parse_esp32(data),
            CsiPacketFormat::Intel5300Bfee => self.parse_intel_5300(data),
            CsiPacketFormat::AtherosCsi => self.parse_atheros(data),
            CsiPacketFormat::NexmonCsi => self.parse_nexmon(data),
            CsiPacketFormat::PicoScenes => self.parse_picoscenes(data),
            CsiPacketFormat::JsonCsi => self.parse_json(data),
            CsiPacketFormat::RawBinary => self.parse_raw_binary(data),
            CsiPacketFormat::Auto => Err(AdapterError::DataFormat("Unable to detect format".into())),
        }
    }

    /// Detect packet format from data
    fn detect_format(&self, data: &[u8]) -> CsiPacketFormat {
        // Check for JSON
        if data.first() == Some(&b'{') {
            return CsiPacketFormat::JsonCsi;
        }

        // Check for ESP32 CSV format (starts with "CSI_DATA,")
        if data.starts_with(b"CSI_DATA,") {
            return CsiPacketFormat::Esp32Csi;
        }

        // Check for Intel 5300 format (look for magic bytes)
        if data.len() >= 4 && data[0] == 0xBB && data[1] == 0x00 {
            return CsiPacketFormat::Intel5300Bfee;
        }

        // Check for PicoScenes format
        if data.len() >= 8 && data[0..4] == [0x50, 0x53, 0x43, 0x53] {
            // "PSCS"
            return CsiPacketFormat::PicoScenes;
        }

        // Default to raw binary
        CsiPacketFormat::RawBinary
    }

    /// Parse ESP32 CSI format
    fn parse_esp32(&self, data: &[u8]) -> Result<CsiPacket, AdapterError> {
        let line = std::str::from_utf8(data)
            .map_err(|e| AdapterError::DataFormat(format!("Invalid UTF-8: {}", e)))?
            .trim();

        // Format: CSI_DATA,mac,rssi,channel,len,data...
        let parts: Vec<&str> = line.split(',').collect();

        if parts.len() < 5 {
            return Err(AdapterError::DataFormat("Invalid ESP32 CSI format".into()));
        }

        let _prefix = parts[0]; // "CSI_DATA"
        let mac_str = parts[1];
        let rssi: i8 = parts[2]
            .parse()
            .map_err(|_| AdapterError::DataFormat("Invalid RSSI value".into()))?;
        let channel: u8 = parts[3]
            .parse()
            .map_err(|_| AdapterError::DataFormat("Invalid channel value".into()))?;
        let _len: usize = parts[4]
            .parse()
            .map_err(|_| AdapterError::DataFormat("Invalid length value".into()))?;

        // Parse MAC address
        let mut tx_mac = [0u8; 6];
        let mac_parts: Vec<&str> = mac_str.split(':').collect();
        if mac_parts.len() == 6 {
            for (i, part) in mac_parts.iter().enumerate() {
                tx_mac[i] = u8::from_str_radix(part, 16).unwrap_or(0);
            }
        }

        // Parse CSI data (remaining parts as comma-separated values)
        let mut amplitudes = Vec::new();
        let mut phases = Vec::new();

        for (i, part) in parts[5..].iter().enumerate() {
            if let Ok(val) = part.parse::<f64>() {
                // Alternate between amplitude and phase
                if i % 2 == 0 {
                    amplitudes.push(val);
                } else {
                    phases.push(val);
                }
            }
        }

        // Ensure phases vector matches amplitudes
        while phases.len() < amplitudes.len() {
            phases.push(0.0);
        }

        Ok(CsiPacket {
            timestamp: Utc::now(),
            source_id: mac_str.to_string(),
            amplitudes,
            phases,
            rssi,
            noise_floor: -92,
            metadata: CsiPacketMetadata {
                tx_mac,
                rx_mac: [0; 6],
                channel,
                bandwidth: Bandwidth::HT20,
                format: CsiPacketFormat::Esp32Csi,
                ..Default::default()
            },
            raw_data: Some(data.to_vec()),
        })
    }

    /// Parse Intel 5300 BFEE format
    fn parse_intel_5300(&self, data: &[u8]) -> Result<CsiPacket, AdapterError> {
        // Intel 5300 BFEE structure (from Linux CSI Tool)
        if data.len() < 25 {
            return Err(AdapterError::DataFormat("Intel 5300 packet too short".into()));
        }

        // Parse header
        let timestamp_low = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let bfee_count = u16::from_le_bytes([data[4], data[5]]);
        let _nrx = data[8];
        let ntx = data[9];
        let rssi_a = data[10] as i8;
        let rssi_b = data[11] as i8;
        let rssi_c = data[12] as i8;
        let noise = data[13] as i8;
        let agc = data[14];
        let perm = [data[15], data[16], data[17]];
        let rate = u16::from_le_bytes([data[18], data[19]]);

        // Average RSSI
        let rssi = ((rssi_a as i16 + rssi_b as i16 + rssi_c as i16) / 3) as i8;

        // Parse CSI matrix (30 subcarriers for Intel 5300)
        let csi_start = 20;
        let num_subcarriers = 30;
        let mut amplitudes = Vec::with_capacity(num_subcarriers);
        let mut phases = Vec::with_capacity(num_subcarriers);

        // CSI is stored as complex values (I/Q pairs)
        for i in 0..num_subcarriers {
            let offset = csi_start + i * 2;
            if offset + 1 < data.len() {
                let real = data[offset] as i8 as f64;
                let imag = data[offset + 1] as i8 as f64;

                let amplitude = (real * real + imag * imag).sqrt();
                let phase = imag.atan2(real);

                amplitudes.push(amplitude);
                phases.push(phase);
            }
        }

        Ok(CsiPacket {
            timestamp: Utc::now(),
            source_id: format!("intel5300_{}", bfee_count),
            amplitudes,
            phases,
            rssi,
            noise_floor: noise,
            metadata: CsiPacketMetadata {
                tx_mac: [0; 6],
                rx_mac: [0; 6],
                channel: 6, // Would need to be extracted from context
                bandwidth: Bandwidth::HT20,
                ntx,
                nrx: 3,
                rate: (rate & 0xFF) as u8,
                format: CsiPacketFormat::Intel5300Bfee,
                ..Default::default()
            },
            raw_data: Some(data.to_vec()),
        })
    }

    /// Parse Atheros CSI format
    fn parse_atheros(&self, data: &[u8]) -> Result<CsiPacket, AdapterError> {
        // Atheros CSI structure varies by driver
        if data.len() < 20 {
            return Err(AdapterError::DataFormat("Atheros packet too short".into()));
        }

        // Basic header (simplified)
        let rssi = data[0] as i8;
        let noise = data[1] as i8;
        let channel = data[2];
        let bandwidth = if data[3] == 1 {
            Bandwidth::HT40
        } else {
            Bandwidth::HT20
        };

        let num_subcarriers = match bandwidth {
            Bandwidth::HT20 => 56,
            Bandwidth::HT40 => 114,
            _ => 56,
        };

        // Parse CSI data
        let csi_start = 20;
        let mut amplitudes = Vec::with_capacity(num_subcarriers);
        let mut phases = Vec::with_capacity(num_subcarriers);

        for i in 0..num_subcarriers {
            let offset = csi_start + i * 4;
            if offset + 3 < data.len() {
                let real = i16::from_le_bytes([data[offset], data[offset + 1]]) as f64;
                let imag = i16::from_le_bytes([data[offset + 2], data[offset + 3]]) as f64;

                let amplitude = (real * real + imag * imag).sqrt();
                let phase = imag.atan2(real);

                amplitudes.push(amplitude);
                phases.push(phase);
            }
        }

        Ok(CsiPacket {
            timestamp: Utc::now(),
            source_id: "atheros".to_string(),
            amplitudes,
            phases,
            rssi,
            noise_floor: noise,
            metadata: CsiPacketMetadata {
                channel,
                bandwidth,
                format: CsiPacketFormat::AtherosCsi,
                ..Default::default()
            },
            raw_data: Some(data.to_vec()),
        })
    }

    /// Parse Nexmon CSI format
    fn parse_nexmon(&self, data: &[u8]) -> Result<CsiPacket, AdapterError> {
        // Nexmon CSI UDP packet format
        if data.len() < 18 {
            return Err(AdapterError::DataFormat("Nexmon packet too short".into()));
        }

        // Parse header
        let _magic = u16::from_le_bytes([data[0], data[1]]);
        let rssi = data[2] as i8;
        let fc = u16::from_le_bytes([data[3], data[4]]);
        let _src_mac = &data[5..11];
        let seq = u16::from_le_bytes([data[11], data[12]]);
        let _core_revid = u16::from_le_bytes([data[13], data[14]]);
        let chan_spec = u16::from_le_bytes([data[15], data[16]]);
        let chip = u16::from_le_bytes([data[17], data[18]]);

        // Determine bandwidth from chanspec
        let bandwidth = match (chan_spec >> 8) & 0x7 {
            0 => Bandwidth::HT20,
            1 => Bandwidth::HT40,
            2 => Bandwidth::VHT80,
            _ => Bandwidth::HT20,
        };

        let channel = (chan_spec & 0xFF) as u8;

        // Parse CSI data
        let csi_start = 18;
        let bytes_per_sc = 4; // 2 bytes real + 2 bytes imag
        let num_subcarriers = (data.len() - csi_start) / bytes_per_sc;

        let mut amplitudes = Vec::with_capacity(num_subcarriers);
        let mut phases = Vec::with_capacity(num_subcarriers);

        for i in 0..num_subcarriers {
            let offset = csi_start + i * bytes_per_sc;
            if offset + 3 < data.len() {
                let real = i16::from_le_bytes([data[offset], data[offset + 1]]) as f64;
                let imag = i16::from_le_bytes([data[offset + 2], data[offset + 3]]) as f64;

                amplitudes.push((real * real + imag * imag).sqrt());
                phases.push(imag.atan2(real));
            }
        }

        Ok(CsiPacket {
            timestamp: Utc::now(),
            source_id: format!("nexmon_{}", chip),
            amplitudes,
            phases,
            rssi,
            noise_floor: -92,
            metadata: CsiPacketMetadata {
                channel,
                bandwidth,
                sequence_num: seq,
                frame_control: fc,
                format: CsiPacketFormat::NexmonCsi,
                ..Default::default()
            },
            raw_data: Some(data.to_vec()),
        })
    }

    /// Parse PicoScenes format
    fn parse_picoscenes(&self, data: &[u8]) -> Result<CsiPacket, AdapterError> {
        // PicoScenes has a complex structure with multiple segments
        if data.len() < 100 {
            return Err(AdapterError::DataFormat("PicoScenes packet too short".into()));
        }

        // Simplified parsing - real implementation would parse all segments
        let rssi = data[20] as i8;
        let channel = data[24];

        // Placeholder - full implementation would parse the CSI segment
        Ok(CsiPacket {
            timestamp: Utc::now(),
            source_id: "picoscenes".to_string(),
            amplitudes: vec![],
            phases: vec![],
            rssi,
            noise_floor: -92,
            metadata: CsiPacketMetadata {
                channel,
                format: CsiPacketFormat::PicoScenes,
                ..Default::default()
            },
            raw_data: Some(data.to_vec()),
        })
    }

    /// Parse JSON CSI format
    fn parse_json(&self, data: &[u8]) -> Result<CsiPacket, AdapterError> {
        let json_str = std::str::from_utf8(data)
            .map_err(|e| AdapterError::DataFormat(format!("Invalid UTF-8: {}", e)))?;

        let json: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| AdapterError::DataFormat(format!("Invalid JSON: {}", e)))?;

        let rssi = json
            .get("rssi")
            .and_then(|v| v.as_i64())
            .unwrap_or(-50) as i8;

        let channel = json
            .get("channel")
            .and_then(|v| v.as_u64())
            .unwrap_or(6) as u8;

        let amplitudes: Vec<f64> = json
            .get("amplitudes")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_f64())
                    .collect()
            })
            .unwrap_or_default();

        let phases: Vec<f64> = json
            .get("phases")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_f64())
                    .collect()
            })
            .unwrap_or_default();

        let source_id = json
            .get("source_id")
            .and_then(|v| v.as_str())
            .unwrap_or("json")
            .to_string();

        Ok(CsiPacket {
            timestamp: Utc::now(),
            source_id,
            amplitudes,
            phases,
            rssi,
            noise_floor: -92,
            metadata: CsiPacketMetadata {
                channel,
                format: CsiPacketFormat::JsonCsi,
                ..Default::default()
            },
            raw_data: Some(data.to_vec()),
        })
    }

    /// Parse raw binary format (minimal processing)
    fn parse_raw_binary(&self, data: &[u8]) -> Result<CsiPacket, AdapterError> {
        // Just store raw data without parsing
        Ok(CsiPacket {
            timestamp: Utc::now(),
            source_id: "raw".to_string(),
            amplitudes: vec![],
            phases: vec![],
            rssi: 0,
            noise_floor: 0,
            metadata: CsiPacketMetadata {
                format: CsiPacketFormat::RawBinary,
                ..Default::default()
            },
            raw_data: Some(data.to_vec()),
        })
    }
}

/// Receiver statistics
#[derive(Debug, Clone, Default)]
pub struct ReceiverStats {
    /// Total packets received
    pub packets_received: u64,
    /// Successfully parsed packets
    pub packets_parsed: u64,
    /// Parse errors
    pub parse_errors: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Dropped packets (buffer overflow)
    pub packets_dropped: u64,
}

impl ReceiverStats {
    /// Get parse success rate
    pub fn success_rate(&self) -> f64 {
        if self.packets_received > 0 {
            self.packets_parsed as f64 / self.packets_received as f64
        } else {
            0.0
        }
    }

    /// Reset statistics
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Convert CsiPacket to CsiReadings for integration with HardwareAdapter
impl From<CsiPacket> for CsiReadings {
    fn from(packet: CsiPacket) -> Self {
        // Capture length before moving amplitudes
        let num_subcarriers = packet.amplitudes.len();

        CsiReadings {
            timestamp: packet.timestamp,
            readings: vec![SensorCsiReading {
                sensor_id: packet.source_id,
                amplitudes: packet.amplitudes,
                phases: packet.phases,
                rssi: packet.rssi as f64,
                noise_floor: packet.noise_floor as f64,
                tx_mac: Some(format!(
                    "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                    packet.metadata.tx_mac[0],
                    packet.metadata.tx_mac[1],
                    packet.metadata.tx_mac[2],
                    packet.metadata.tx_mac[3],
                    packet.metadata.tx_mac[4],
                    packet.metadata.tx_mac[5]
                )),
                rx_mac: Some(format!(
                    "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                    packet.metadata.rx_mac[0],
                    packet.metadata.rx_mac[1],
                    packet.metadata.rx_mac[2],
                    packet.metadata.rx_mac[3],
                    packet.metadata.rx_mac[4],
                    packet.metadata.rx_mac[5]
                )),
                sequence_num: Some(packet.metadata.sequence_num),
            }],
            metadata: CsiMetadata {
                device_type: match packet.metadata.format {
                    CsiPacketFormat::Esp32Csi => DeviceType::Esp32,
                    CsiPacketFormat::Intel5300Bfee => DeviceType::Intel5300,
                    CsiPacketFormat::AtherosCsi => {
                        DeviceType::Atheros(super::hardware_adapter::AtherosDriver::Ath10k)
                    }
                    _ => DeviceType::UdpReceiver,
                },
                channel: packet.metadata.channel,
                bandwidth: packet.metadata.bandwidth,
                num_subcarriers,
                rssi: Some(packet.rssi as f64),
                noise_floor: Some(packet.noise_floor as f64),
                fc_type: FrameControlType::Data,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_receiver_config_udp() {
        let config = ReceiverConfig::udp("0.0.0.0", 5500);
        assert!(matches!(config.source, CsiSource::Udp(_)));
    }

    #[test]
    fn test_receiver_config_serial() {
        let config = ReceiverConfig::serial("/dev/ttyUSB0", 921600);
        assert!(matches!(config.source, CsiSource::Serial(_)));
        assert_eq!(config.format, CsiPacketFormat::Esp32Csi);
    }

    #[test]
    fn test_receiver_config_pcap() {
        let config = ReceiverConfig::pcap("/tmp/test.pcap");
        assert!(matches!(config.source, CsiSource::Pcap(_)));
    }

    #[test]
    fn test_parser_detect_json() {
        let parser = CsiParser::new(CsiPacketFormat::Auto);
        let data = b"{\"rssi\": -50}";
        let format = parser.detect_format(data);
        assert_eq!(format, CsiPacketFormat::JsonCsi);
    }

    #[test]
    fn test_parser_detect_esp32() {
        let parser = CsiParser::new(CsiPacketFormat::Auto);
        let data = b"CSI_DATA,AA:BB:CC:DD:EE:FF,-45,6,128,1.0,0.5";
        let format = parser.detect_format(data);
        assert_eq!(format, CsiPacketFormat::Esp32Csi);
    }

    #[test]
    fn test_parse_json() {
        let parser = CsiParser::new(CsiPacketFormat::JsonCsi);
        let data = br#"{"rssi": -50, "channel": 6, "amplitudes": [1.0, 2.0, 3.0], "phases": [0.1, 0.2, 0.3]}"#;

        let packet = parser.parse(data).unwrap();
        assert_eq!(packet.rssi, -50);
        assert_eq!(packet.metadata.channel, 6);
        assert_eq!(packet.amplitudes.len(), 3);
    }

    #[test]
    fn test_parse_esp32() {
        let parser = CsiParser::new(CsiPacketFormat::Esp32Csi);
        let data = b"CSI_DATA,AA:BB:CC:DD:EE:FF,-45,6,128,1.0,0.5,2.0,0.6,3.0,0.7";

        let packet = parser.parse(data).unwrap();
        assert_eq!(packet.rssi, -45);
        assert_eq!(packet.metadata.channel, 6);
        assert_eq!(packet.amplitudes.len(), 3);
    }

    #[test]
    fn test_receiver_stats() {
        let mut stats = ReceiverStats::default();
        stats.packets_received = 100;
        stats.packets_parsed = 95;

        assert!((stats.success_rate() - 0.95).abs() < 0.001);

        stats.reset();
        assert_eq!(stats.packets_received, 0);
    }

    #[test]
    fn test_csi_packet_to_readings() {
        let packet = CsiPacket {
            timestamp: Utc::now(),
            source_id: "test".to_string(),
            amplitudes: vec![1.0, 2.0, 3.0],
            phases: vec![0.1, 0.2, 0.3],
            rssi: -45,
            noise_floor: -92,
            metadata: CsiPacketMetadata {
                channel: 6,
                ..Default::default()
            },
            raw_data: None,
        };

        let readings: CsiReadings = packet.into();
        assert_eq!(readings.readings.len(), 1);
        assert_eq!(readings.readings[0].amplitudes.len(), 3);
        assert_eq!(readings.metadata.channel, 6);
    }

    #[test]
    fn test_serial_receiver_buffer() {
        let config = ReceiverConfig::serial("/dev/ttyUSB0", 921600);
        // Skip actual port check in test
        let mut receiver = SerialCsiReceiver {
            config,
            port_path: "/dev/ttyUSB0".to_string(),
            buffer: VecDeque::new(),
            parser: CsiParser::new(CsiPacketFormat::Esp32Csi),
            stats: ReceiverStats::default(),
            running: true,
        };

        // Feed some data
        let test_data = b"CSI_DATA,AA:BB:CC:DD:EE:FF,-45,6,128,1.0,0.5\n";
        let expected_len = test_data.len() as u64;
        receiver.feed_data(test_data);
        assert_eq!(receiver.stats.bytes_received, expected_len);

        // Extract packet
        let packet_data = receiver.extract_packet_from_buffer();
        assert!(packet_data.is_some());
    }
}
