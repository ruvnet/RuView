//! Inter-Chip Communication Protocol

use heapless::Vec as HVec;

pub const MAX_ACTIVATION_SIZE: usize = 256;
pub const MAX_PAYLOAD_SIZE: usize = 512;
pub const PROTOCOL_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ChipId(pub u8);

impl ChipId {
    pub const BROADCAST: ChipId = ChipId(0xFF);
    pub fn is_broadcast(&self) -> bool { self.0 == 0xFF }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum MessageType {
    Heartbeat = 0x00,
    Discovery = 0x01,
    Ready = 0x02,
    Activation = 0x10,
    KVCache = 0x11,
    Gradient = 0x12,
    EmbedRequest = 0x20,
    EmbedResponse = 0x21,
    Logits = 0x22,
    Token = 0x23,
    DraftTokens = 0x30,
    VerifyResult = 0x31,
    Barrier = 0x40,
    Ack = 0x41,
    Error = 0xFF,
}

impl From<u8> for MessageType {
    fn from(v: u8) -> Self {
        match v {
            0x00 => Self::Heartbeat, 0x01 => Self::Discovery, 0x02 => Self::Ready,
            0x10 => Self::Activation, 0x11 => Self::KVCache, 0x12 => Self::Gradient,
            0x20 => Self::EmbedRequest, 0x21 => Self::EmbedResponse,
            0x22 => Self::Logits, 0x23 => Self::Token,
            0x30 => Self::DraftTokens, 0x31 => Self::VerifyResult,
            0x40 => Self::Barrier, 0x41 => Self::Ack,
            _ => Self::Error,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct MessageHeader {
    pub version: u8,
    pub msg_type: u8,
    pub src: u8,
    pub dst: u8,
    pub seq: u16,
    pub payload_len: u16,
}

impl MessageHeader {
    pub const SIZE: usize = 8;

    pub fn new(msg_type: MessageType, src: ChipId, dst: ChipId, seq: u16, payload_len: u16) -> Self {
        Self { version: PROTOCOL_VERSION, msg_type: msg_type as u8, src: src.0, dst: dst.0, seq, payload_len }
    }

    pub fn to_bytes(&self) -> [u8; 8] {
        [self.version, self.msg_type, self.src, self.dst,
         (self.seq & 0xFF) as u8, (self.seq >> 8) as u8,
         (self.payload_len & 0xFF) as u8, (self.payload_len >> 8) as u8]
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() < 8 { return None; }
        Some(Self {
            version: b[0], msg_type: b[1], src: b[2], dst: b[3],
            seq: (b[4] as u16) | ((b[5] as u16) << 8),
            payload_len: (b[6] as u16) | ((b[7] as u16) << 8),
        })
    }

    pub fn checksum(&self) -> u8 {
        self.to_bytes().iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
    }
}

#[derive(Debug, Clone)]
pub struct FederationMessage {
    pub header: MessageHeader,
    pub payload: HVec<u8, MAX_PAYLOAD_SIZE>,
    pub checksum: u8,
}

impl FederationMessage {
    pub fn new(msg_type: MessageType, src: ChipId, dst: ChipId, seq: u16) -> Self {
        Self {
            header: MessageHeader::new(msg_type, src, dst, seq, 0),
            payload: HVec::new(),
            checksum: 0,
        }
    }

    pub fn activation(src: ChipId, dst: ChipId, seq: u16, layer: u8, pos: u16, data: &[i8]) -> crate::Result<Self> {
        let mut msg = Self::new(MessageType::Activation, src, dst, seq);
        msg.payload.push(layer).map_err(|_| crate::Error::BufferOverflow)?;
        msg.payload.push((pos & 0xFF) as u8).map_err(|_| crate::Error::BufferOverflow)?;
        msg.payload.push((pos >> 8) as u8).map_err(|_| crate::Error::BufferOverflow)?;
        for &d in data {
            msg.payload.push(d as u8).map_err(|_| crate::Error::BufferOverflow)?;
        }
        msg.header.payload_len = msg.payload.len() as u16;
        msg.update_checksum();
        Ok(msg)
    }

    pub fn token(src: ChipId, dst: ChipId, seq: u16, token_id: u16) -> Self {
        let mut msg = Self::new(MessageType::Token, src, dst, seq);
        let _ = msg.payload.push((token_id & 0xFF) as u8);
        let _ = msg.payload.push((token_id >> 8) as u8);
        msg.header.payload_len = 2;
        msg.update_checksum();
        msg
    }

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

    pub fn barrier(src: ChipId, barrier_id: u16) -> Self {
        let mut msg = Self::new(MessageType::Barrier, src, ChipId::BROADCAST, 0);
        let _ = msg.payload.push((barrier_id & 0xFF) as u8);
        let _ = msg.payload.push((barrier_id >> 8) as u8);
        msg.header.payload_len = 2;
        msg.update_checksum();
        msg
    }

    pub fn update_checksum(&mut self) {
        let mut sum = self.header.checksum();
        for &b in &self.payload { sum = sum.wrapping_add(b); }
        self.checksum = sum;
    }

    pub fn verify_checksum(&self) -> bool {
        let mut sum = self.header.checksum();
        for &b in &self.payload { sum = sum.wrapping_add(b); }
        sum == self.checksum
    }

    pub fn to_bytes(&self) -> HVec<u8, { MAX_PAYLOAD_SIZE + 16 }> {
        let mut bytes = HVec::new();
        for b in self.header.to_bytes() { let _ = bytes.push(b); }
        for &b in &self.payload { let _ = bytes.push(b); }
        let _ = bytes.push(self.checksum);
        bytes
    }

    pub fn get_activation_data(&self) -> Option<(u8, u16, &[u8])> {
        if self.header.msg_type != MessageType::Activation as u8 || self.payload.len() < 3 { return None; }
        Some((self.payload[0], (self.payload[1] as u16) | ((self.payload[2] as u16) << 8), &self.payload[3..]))
    }

    pub fn get_token(&self) -> Option<u16> {
        if self.header.msg_type != MessageType::Token as u8 || self.payload.len() < 2 { return None; }
        Some((self.payload[0] as u16) | ((self.payload[1] as u16) << 8))
    }
}

#[derive(Debug, Default, Clone)]
pub struct CommStats {
    pub messages_sent: u32,
    pub messages_received: u32,
    pub bytes_sent: u32,
    pub bytes_received: u32,
    pub checksum_errors: u32,
    pub timeouts: u32,
}
