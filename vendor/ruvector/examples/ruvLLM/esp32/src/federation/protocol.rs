//! Inter-Chip Communication Protocol
//!
//! Defines the message format for ESP32-to-ESP32 communication.
//! Designed for low overhead on SPI/I2C/UART buses.

use heapless::Vec as HVec;

/// Maximum activation size that can be sent in one message
pub const MAX_ACTIVATION_SIZE: usize = 256;
/// Maximum message payload
pub const MAX_PAYLOAD_SIZE: usize = 512;
/// Protocol version
pub const PROTOCOL_VERSION: u8 = 1;

/// Chip identifier in the federation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ChipId(pub u8);

impl ChipId {
    pub const BROADCAST: ChipId = ChipId(0xFF);

    pub fn is_broadcast(&self) -> bool {
        self.0 == 0xFF
    }
}

/// Message types for federation protocol
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum MessageType {
    /// Heartbeat / keep-alive
    Heartbeat = 0x00,
    /// Cluster discovery
    Discovery = 0x01,
    /// Ready signal
    Ready = 0x02,

    /// Forward pass activation data
    Activation = 0x10,
    /// Attention K/V cache update
    KVCache = 0x11,
    /// Gradient (for future training)
    Gradient = 0x12,

    /// Token embedding request
    EmbedRequest = 0x20,
    /// Token embedding response
    EmbedResponse = 0x21,
    /// Output logits
    Logits = 0x22,
    /// Sampled token
    Token = 0x23,

    /// Speculative draft tokens
    DraftTokens = 0x30,
    /// Verification result
    VerifyResult = 0x31,

    /// Synchronization barrier
    Barrier = 0x40,
    /// Acknowledgment
    Ack = 0x41,
    /// Error
    Error = 0xFF,
}

impl From<u8> for MessageType {
    fn from(v: u8) -> Self {
        match v {
            0x00 => Self::Heartbeat,
            0x01 => Self::Discovery,
            0x02 => Self::Ready,
            0x10 => Self::Activation,
            0x11 => Self::KVCache,
            0x12 => Self::Gradient,
            0x20 => Self::EmbedRequest,
            0x21 => Self::EmbedResponse,
            0x22 => Self::Logits,
            0x23 => Self::Token,
            0x30 => Self::DraftTokens,
            0x31 => Self::VerifyResult,
            0x40 => Self::Barrier,
            0x41 => Self::Ack,
            _ => Self::Error,
        }
    }
}

/// Message header (8 bytes)
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct MessageHeader {
    /// Protocol version
    pub version: u8,
    /// Message type
    pub msg_type: u8,
    /// Source chip ID
    pub src: u8,
    /// Destination chip ID
    pub dst: u8,
    /// Sequence number (for ordering)
    pub seq: u16,
    /// Payload length
    pub payload_len: u16,
}

impl MessageHeader {
    pub const SIZE: usize = 8;

    pub fn new(msg_type: MessageType, src: ChipId, dst: ChipId, seq: u16, payload_len: u16) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            msg_type: msg_type as u8,
            src: src.0,
            dst: dst.0,
            seq,
            payload_len,
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> [u8; 8] {
        [
            self.version,
            self.msg_type,
            self.src,
            self.dst,
            (self.seq & 0xFF) as u8,
            (self.seq >> 8) as u8,
            (self.payload_len & 0xFF) as u8,
            (self.payload_len >> 8) as u8,
        ]
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 8 {
            return None;
        }
        Some(Self {
            version: bytes[0],
            msg_type: bytes[1],
            src: bytes[2],
            dst: bytes[3],
            seq: (bytes[4] as u16) | ((bytes[5] as u16) << 8),
            payload_len: (bytes[6] as u16) | ((bytes[7] as u16) << 8),
        })
    }

    /// Calculate simple checksum
    pub fn checksum(&self) -> u8 {
        let bytes = self.to_bytes();
        bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
    }
}

/// Complete federation message
#[derive(Debug, Clone)]
pub struct FederationMessage {
    /// Message header
    pub header: MessageHeader,
    /// Payload data
    pub payload: HVec<u8, MAX_PAYLOAD_SIZE>,
    /// Checksum
    pub checksum: u8,
}

impl FederationMessage {
    /// Create new message
    pub fn new(msg_type: MessageType, src: ChipId, dst: ChipId, seq: u16) -> Self {
        Self {
            header: MessageHeader::new(msg_type, src, dst, seq, 0),
            payload: HVec::new(),
            checksum: 0,
        }
    }

    /// Create activation message with INT8 data
    pub fn activation(
        src: ChipId,
        dst: ChipId,
        seq: u16,
        layer_idx: u8,
        position: u16,
        data: &[i8],
    ) -> crate::Result<Self> {
        let mut msg = Self::new(MessageType::Activation, src, dst, seq);

        // Payload format: [layer_idx:1][position:2][data:N]
        msg.payload.push(layer_idx).map_err(|_| crate::Error::BufferOverflow)?;
        msg.payload.push((position & 0xFF) as u8).map_err(|_| crate::Error::BufferOverflow)?;
        msg.payload.push((position >> 8) as u8).map_err(|_| crate::Error::BufferOverflow)?;

        for &d in data {
            msg.payload.push(d as u8).map_err(|_| crate::Error::BufferOverflow)?;
        }

        msg.header.payload_len = msg.payload.len() as u16;
        msg.update_checksum();
        Ok(msg)
    }

    /// Create token message
    pub fn token(src: ChipId, dst: ChipId, seq: u16, token_id: u16) -> Self {
        let mut msg = Self::new(MessageType::Token, src, dst, seq);
        let _ = msg.payload.push((token_id & 0xFF) as u8);
        let _ = msg.payload.push((token_id >> 8) as u8);
        msg.header.payload_len = 2;
        msg.update_checksum();
        msg
    }

    /// Create draft tokens message for speculative decoding
    pub fn draft_tokens(src: ChipId, dst: ChipId, seq: u16, tokens: &[u16]) -> crate::Result<Self> {
        let mut msg = Self::new(MessageType::DraftTokens, src, dst, seq);

        msg.payload.push(tokens.len() as u8).map_err(|_| crate::Error::BufferOverflow)?;

        for &t in tokens {
            msg.payload.push((t & 0xFF) as u8).map_err(|_| crate::Error::BufferOverflow)?;
            msg.payload.push((t >> 8) as u8).map_err(|_| crate::Error::BufferOverflow)?;
        }

        msg.header.payload_len = msg.payload.len() as u16;
        msg.update_checksum();
        Ok(msg)
    }

    /// Create barrier synchronization message
    pub fn barrier(src: ChipId, barrier_id: u16) -> Self {
        let mut msg = Self::new(MessageType::Barrier, src, ChipId::BROADCAST, 0);
        let _ = msg.payload.push((barrier_id & 0xFF) as u8);
        let _ = msg.payload.push((barrier_id >> 8) as u8);
        msg.header.payload_len = 2;
        msg.update_checksum();
        msg
    }

    /// Update checksum
    pub fn update_checksum(&mut self) {
        let mut sum = self.header.checksum();
        for &b in &self.payload {
            sum = sum.wrapping_add(b);
        }
        self.checksum = sum;
    }

    /// Verify checksum
    pub fn verify_checksum(&self) -> bool {
        let mut sum = self.header.checksum();
        for &b in &self.payload {
            sum = sum.wrapping_add(b);
        }
        sum == self.checksum
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> HVec<u8, { MAX_PAYLOAD_SIZE + 16 }> {
        let mut bytes = HVec::new();

        // Header
        for b in self.header.to_bytes() {
            let _ = bytes.push(b);
        }

        // Payload
        for &b in &self.payload {
            let _ = bytes.push(b);
        }

        // Checksum
        let _ = bytes.push(self.checksum);

        bytes
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> crate::Result<Self> {
        if bytes.len() < MessageHeader::SIZE + 1 {
            return Err(crate::Error::InvalidModel("Message too short"));
        }

        let header = MessageHeader::from_bytes(bytes)
            .ok_or(crate::Error::InvalidModel("Invalid header"))?;

        let payload_end = MessageHeader::SIZE + header.payload_len as usize;
        if bytes.len() < payload_end + 1 {
            return Err(crate::Error::InvalidModel("Payload incomplete"));
        }

        let mut payload = HVec::new();
        for &b in &bytes[MessageHeader::SIZE..payload_end] {
            payload.push(b).map_err(|_| crate::Error::BufferOverflow)?;
        }

        let checksum = bytes[payload_end];

        let msg = Self {
            header,
            payload,
            checksum,
        };

        if !msg.verify_checksum() {
            return Err(crate::Error::InvalidModel("Checksum mismatch"));
        }

        Ok(msg)
    }

    /// Extract activation data from payload
    pub fn get_activation_data(&self) -> Option<(u8, u16, &[u8])> {
        if self.header.msg_type != MessageType::Activation as u8 {
            return None;
        }
        if self.payload.len() < 3 {
            return None;
        }

        let layer_idx = self.payload[0];
        let position = (self.payload[1] as u16) | ((self.payload[2] as u16) << 8);
        let data = &self.payload[3..];

        Some((layer_idx, position, data))
    }

    /// Extract token from payload
    pub fn get_token(&self) -> Option<u16> {
        if self.header.msg_type != MessageType::Token as u8 {
            return None;
        }
        if self.payload.len() < 2 {
            return None;
        }

        Some((self.payload[0] as u16) | ((self.payload[1] as u16) << 8))
    }
}

/// Communication statistics
#[derive(Debug, Default, Clone)]
pub struct CommStats {
    /// Messages sent
    pub messages_sent: u32,
    /// Messages received
    pub messages_received: u32,
    /// Bytes sent
    pub bytes_sent: u32,
    /// Bytes received
    pub bytes_received: u32,
    /// Checksum errors
    pub checksum_errors: u32,
    /// Timeouts
    pub timeouts: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_header() {
        let header = MessageHeader::new(
            MessageType::Activation,
            ChipId(0),
            ChipId(1),
            42,
            100,
        );

        let bytes = header.to_bytes();
        let decoded = MessageHeader::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.msg_type, MessageType::Activation as u8);
        assert_eq!(decoded.src, 0);
        assert_eq!(decoded.dst, 1);
        // Copy packed fields to avoid UB from unaligned references
        let seq = decoded.seq;
        let payload_len = decoded.payload_len;
        assert_eq!(seq, 42);
        assert_eq!(payload_len, 100);
    }

    #[test]
    fn test_activation_message() {
        let data: [i8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
        let msg = FederationMessage::activation(
            ChipId(0),
            ChipId(1),
            1,
            0,
            10,
            &data,
        ).unwrap();

        let bytes = msg.to_bytes();
        let decoded = FederationMessage::from_bytes(&bytes).unwrap();

        let (layer, pos, act_data) = decoded.get_activation_data().unwrap();
        assert_eq!(layer, 0);
        assert_eq!(pos, 10);
        assert_eq!(act_data.len(), 8);
    }

    #[test]
    fn test_token_message() {
        let msg = FederationMessage::token(ChipId(4), ChipId(0), 100, 12345);

        let bytes = msg.to_bytes();
        let decoded = FederationMessage::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.get_token(), Some(12345));
    }
}
