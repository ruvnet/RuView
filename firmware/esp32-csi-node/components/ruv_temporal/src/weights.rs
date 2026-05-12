// Firmware-side mirror of `wifi-densepose-temporal::weights`. Same wire
// format, same magic, same CRC polynomial — a blob produced by the
// host's `WeightBlob::serialize()` parses here byte-for-byte.
//
// no_std + alloc. The host side keeps weights as `Vec<u8>` because it
// owns the buffer; the firmware loader takes a borrowed `&[u8]` slice
// (the blob lives in flash via EMBED_FILES, or a heap mmap from NVS,
// neither of which the loader should re-allocate).
//
// Stays *byte-exact* in lockstep with `v2/crates/wifi-densepose-temporal/src/weights.rs`.
// When the host format changes, this file changes in the same commit
// and bumps `BLOB_VERSION`; mismatched versions refuse to load.

use core::convert::TryInto;
use core::fmt;

pub const BLOB_MAGIC: u32 = 0x5256_4E45; // "RVNE"
pub const BLOB_VERSION: u16 = 1;
pub const BLOB_HEADER_LEN: usize = 24;
pub const BLOB_FOOTER_LEN: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeightDtype {
    F32,
    F16,
}

#[derive(Clone, Copy, Debug)]
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
    pub fn elem_bytes(&self) -> usize {
        match self.dtype {
            WeightDtype::F32 => 4,
            WeightDtype::F16 => 2,
        }
    }

    fn validate(&self) -> Result<(), WeightLoadError> {
        if self.input_dim == 0
            || self.n_q_heads == 0
            || self.n_kv_heads == 0
            || self.head_dim == 0
        {
            return Err(WeightLoadError::ZeroDim);
        }
        if self.n_q_heads % self.n_kv_heads != 0 {
            return Err(WeightLoadError::InvalidGqaRatio);
        }
        if self.n_layers == 0 || self.n_classes < 2 {
            return Err(WeightLoadError::DegenerateShape);
        }
        Ok(())
    }
}

/// A parsed view into a weights blob. Holds borrowed slices into the
/// caller-owned buffer — no allocation, no copy. The firmware's
/// kernel reads weights directly from this view.
#[derive(Clone, Copy)]
pub struct WeightBlobView<'a> {
    pub header: WeightBlobHeader,
    pub weights: &'a [u8],
}

impl<'a> WeightBlobView<'a> {
    /// Parse a blob, validating magic / version / size / CRC. Returns
    /// a borrowed view; the input `buf` must outlive the view.
    pub fn parse(buf: &'a [u8]) -> Result<Self, WeightLoadError> {
        if buf.len() < BLOB_HEADER_LEN + BLOB_FOOTER_LEN {
            return Err(WeightLoadError::TooShort);
        }

        let magic = u32::from_le_bytes(buf[0..4].try_into().unwrap());
        if magic != BLOB_MAGIC {
            return Err(WeightLoadError::BadMagic);
        }
        let version = u16::from_le_bytes(buf[4..6].try_into().unwrap());
        if version != BLOB_VERSION {
            return Err(WeightLoadError::WrongVersion(version));
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

        let expected = BLOB_HEADER_LEN + weights_len + BLOB_FOOTER_LEN;
        if buf.len() != expected {
            return Err(WeightLoadError::SizeMismatch);
        }

        let stored_crc = u32::from_le_bytes(buf[buf.len() - 4..].try_into().unwrap());
        let computed = crc32_ieee(&buf[..buf.len() - 4]);
        if stored_crc != computed {
            return Err(WeightLoadError::CrcMismatch);
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

        let weights_start = BLOB_HEADER_LEN;
        let weights_end = weights_start + weights_len;
        Ok(Self {
            header,
            weights: &buf[weights_start..weights_end],
        })
    }
}

/// Loader-side error. Distinct from the host-side `TemporalError` so
/// the firmware can map specific cases to specific `esp_err_t` codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeightLoadError {
    TooShort,
    BadMagic,
    WrongVersion(u16),
    SizeMismatch,
    CrcMismatch,
    ZeroDim,
    InvalidGqaRatio,
    DegenerateShape,
}

impl fmt::Display for WeightLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => write!(f, "weight blob too short"),
            Self::BadMagic => write!(f, "weight blob: bad magic"),
            Self::WrongVersion(v) => write!(f, "weight blob: unsupported version {}", v),
            Self::SizeMismatch => write!(f, "weight blob: declared length doesn't match buffer"),
            Self::CrcMismatch => write!(f, "weight blob: CRC32 mismatch"),
            Self::ZeroDim => write!(f, "weight blob: zero-valued dimension(s)"),
            Self::InvalidGqaRatio => write!(f, "weight blob: n_q_heads not divisible by n_kv_heads"),
            Self::DegenerateShape => write!(f, "weight blob: n_layers=0 or n_classes<2"),
        }
    }
}

/// Map loader errors to esp_err_t-style codes for the C ABI. Defined
/// here rather than in lib.rs so the mapping stays adjacent to the
/// error type and can't drift.
pub const fn weight_load_err_to_esp(err: &WeightLoadError) -> i32 {
    match err {
        WeightLoadError::TooShort
        | WeightLoadError::BadMagic
        | WeightLoadError::WrongVersion(_)
        | WeightLoadError::SizeMismatch => 0x102, // ESP_ERR_INVALID_ARG
        WeightLoadError::CrcMismatch => 0x10C, // ESP_ERR_INVALID_CRC
        WeightLoadError::ZeroDim
        | WeightLoadError::InvalidGqaRatio
        | WeightLoadError::DegenerateShape => 0x103, // ESP_ERR_INVALID_SIZE
    }
}

/// Same polynomial as `temporal_task.c::crc32_ieee` and the host-side
/// `wifi_densepose_temporal::weights::crc32_ieee`. The whole point of
/// keeping it bit-for-bit identical across all three sites is so a
/// blob round-trips without re-computing.
fn crc32_ieee(data: &[u8]) -> u32 {
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
