// Wire format for the temporal-head weights blob.
//
// One blob describes one model. Both ends speak it:
//   - Host-side (this crate): training emits a blob via `WeightBlob::serialize`.
//   - Firmware-side (`firmware/esp32-csi-node/components/ruv_temporal`):
//     loads it via a mirrored parser. The blob is the *only* thing
//     that crosses the host/firmware boundary at deploy time, so the
//     format must be stable, self-describing, and version-gated.
//
// Layout (little-endian throughout):
//
//   header  16 B
//     [0x00..0x04)  magic         u32  = 0x52564E45 ("RVNE" — RuVector Neural Edge)
//     [0x04..0x06)  version       u16  = 1
//     [0x06..0x07)  flags         u8   bit 0 = 0:fp32 / 1:fp16 weights
//     [0x07..0x08)  reserved      u8
//     [0x08..0x0A)  input_dim     u16  per-frame feature dim
//     [0x0A..0x0C)  n_q_heads     u16  query head count
//     [0x0C..0x0E)  n_kv_heads    u16  key/value head count (≤ n_q_heads, divides it)
//     [0x0E..0x10)  head_dim      u16  per-head feature dim
//
//   body                          variable
//     [0x10..0x12)  n_layers      u16
//     [0x12..0x14)  n_classes     u16
//     [0x14..0x18)  weights_len   u32  bytes of weights payload (after this header)
//     [0x18..end-4) weights       weights_len bytes — flat per-layer arrays
//                                 in the order the kernel reads them
//   footer                        4 B
//     [end-4..end)  crc32         u32  IEEE 802.3, covers everything before
//
// Total size = 16 (header) + 2+2+4 (body header) + weights_len + 4 (crc) = 28 + weights_len
//
// Versioning: bumping `version` is a hard break — firmware refuses to
// load a blob whose version it doesn't know. Adding a *new* field is
// done by reserving a new flag bit and treating the field as
// post-weights when the bit is set; never reorder existing fields.

use crate::error::TemporalError;

pub const WEIGHT_BLOB_MAGIC: u32 = 0x5256_4E45; // "RVNE"
pub const WEIGHT_BLOB_VERSION: u16 = 1;
pub const WEIGHT_BLOB_HEADER_LEN: usize = 16 + 2 + 2 + 4; // 24
pub const WEIGHT_BLOB_FOOTER_LEN: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeightDtype {
    F32,
    F16,
}

#[derive(Clone, Debug)]
pub struct WeightBlobHeader {
    pub dtype: WeightDtype,
    pub input_dim: u16,
    pub n_q_heads: u16,
    pub n_kv_heads: u16,
    pub head_dim: u16,
    pub n_layers: u16,
    pub n_classes: u16,
}

impl WeightBlobHeader {
    /// Element size in bytes for the configured dtype.
    pub fn elem_bytes(&self) -> usize {
        match self.dtype {
            WeightDtype::F32 => 4,
            WeightDtype::F16 => 2,
        }
    }

    /// Validate that the structural numbers make sense — caught here
    /// rather than at first kernel call so the host-side training
    /// tool can't accidentally emit a blob the firmware will reject
    /// at boot.
    pub fn validate(&self) -> Result<(), TemporalError> {
        if self.input_dim == 0
            || self.n_q_heads == 0
            || self.n_kv_heads == 0
            || self.head_dim == 0
        {
            return Err(TemporalError::InvalidConfig(
                "header: zero-valued dimension(s)",
            ));
        }
        if self.n_q_heads % self.n_kv_heads != 0 {
            return Err(TemporalError::InvalidConfig(
                "header: n_q_heads must be divisible by n_kv_heads (GQA)",
            ));
        }
        if self.n_layers == 0 || self.n_classes < 2 {
            return Err(TemporalError::InvalidConfig(
                "header: n_layers must be ≥ 1 and n_classes ≥ 2",
            ));
        }
        Ok(())
    }
}

/// A complete weight blob: header + raw weights bytes.
///
/// Weights are kept as `Vec<u8>` rather than `Vec<f32>` / `Vec<f16>` so
/// the firmware loader (which is no_std and may not have the `half`
/// crate) can `mmap` the body and read either dtype directly.
pub struct WeightBlob {
    pub header: WeightBlobHeader,
    pub weights: Vec<u8>,
}

impl WeightBlob {
    pub fn new(header: WeightBlobHeader, weights: Vec<u8>) -> Result<Self, TemporalError> {
        header.validate()?;
        let elem = header.elem_bytes();
        if weights.len() % elem != 0 {
            return Err(TemporalError::InvalidConfig(
                "weights length is not a multiple of dtype element size",
            ));
        }
        Ok(Self { header, weights })
    }

    /// Serialize to the wire format. Stable across rebuilds — this is
    /// the contract the firmware loader speaks.
    pub fn serialize(&self) -> Vec<u8> {
        let total = WEIGHT_BLOB_HEADER_LEN + self.weights.len() + WEIGHT_BLOB_FOOTER_LEN;
        let mut out = Vec::with_capacity(total);

        // header
        out.extend_from_slice(&WEIGHT_BLOB_MAGIC.to_le_bytes());
        out.extend_from_slice(&WEIGHT_BLOB_VERSION.to_le_bytes());
        let flags: u8 = match self.header.dtype {
            WeightDtype::F32 => 0,
            WeightDtype::F16 => 1,
        };
        out.push(flags);
        out.push(0); // reserved
        out.extend_from_slice(&self.header.input_dim.to_le_bytes());
        out.extend_from_slice(&self.header.n_q_heads.to_le_bytes());
        out.extend_from_slice(&self.header.n_kv_heads.to_le_bytes());
        out.extend_from_slice(&self.header.head_dim.to_le_bytes());

        // body header
        out.extend_from_slice(&self.header.n_layers.to_le_bytes());
        out.extend_from_slice(&self.header.n_classes.to_le_bytes());
        out.extend_from_slice(&(self.weights.len() as u32).to_le_bytes());

        // weights
        out.extend_from_slice(&self.weights);

        // footer: crc32 over everything written so far
        let crc = crc32_ieee(&out);
        out.extend_from_slice(&crc.to_le_bytes());
        out
    }

    /// Parse a blob, validating magic / version / size / CRC.
    pub fn parse(buf: &[u8]) -> Result<Self, TemporalError> {
        if buf.len() < WEIGHT_BLOB_HEADER_LEN + WEIGHT_BLOB_FOOTER_LEN {
            return Err(TemporalError::InvalidConfig("blob too short"));
        }

        let magic = u32::from_le_bytes(buf[0..4].try_into().unwrap());
        if magic != WEIGHT_BLOB_MAGIC {
            return Err(TemporalError::InvalidConfig("bad magic"));
        }
        let version = u16::from_le_bytes(buf[4..6].try_into().unwrap());
        if version != WEIGHT_BLOB_VERSION {
            return Err(TemporalError::InvalidConfig("unsupported blob version"));
        }
        let flags = buf[6];
        let dtype = match flags & 0x01 {
            0 => WeightDtype::F32,
            _ => WeightDtype::F16,
        };

        let input_dim = u16::from_le_bytes(buf[8..10].try_into().unwrap());
        let n_q_heads = u16::from_le_bytes(buf[10..12].try_into().unwrap());
        let n_kv_heads = u16::from_le_bytes(buf[12..14].try_into().unwrap());
        let head_dim = u16::from_le_bytes(buf[14..16].try_into().unwrap());

        let n_layers = u16::from_le_bytes(buf[16..18].try_into().unwrap());
        let n_classes = u16::from_le_bytes(buf[18..20].try_into().unwrap());
        let weights_len = u32::from_le_bytes(buf[20..24].try_into().unwrap()) as usize;

        // sanity-check size before slicing
        let expected = WEIGHT_BLOB_HEADER_LEN + weights_len + WEIGHT_BLOB_FOOTER_LEN;
        if buf.len() != expected {
            return Err(TemporalError::InvalidConfig(
                "blob length doesn't match weights_len in header",
            ));
        }

        // CRC check: cover everything before the trailing 4-byte CRC
        let stored_crc = u32::from_le_bytes(buf[buf.len() - 4..].try_into().unwrap());
        let computed = crc32_ieee(&buf[..buf.len() - 4]);
        if stored_crc != computed {
            return Err(TemporalError::InvalidConfig("blob CRC mismatch"));
        }

        let header = WeightBlobHeader {
            dtype,
            input_dim,
            n_q_heads,
            n_kv_heads,
            head_dim,
            n_layers,
            n_classes,
        };
        header.validate()?;

        let weights_start = WEIGHT_BLOB_HEADER_LEN;
        let weights_end = weights_start + weights_len;
        let weights = buf[weights_start..weights_end].to_vec();

        Ok(Self { header, weights })
    }
}

/// IEEE 802.3 CRC32 (poly 0xEDB88320), table-free. Same polynomial
/// the firmware-side loader uses (`temporal_task.c::crc32_ieee`) so a
/// blob produced here parses there.
pub(crate) fn crc32_ieee(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}
