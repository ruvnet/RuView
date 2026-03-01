//! C FFI for App Clip / mobile integration.
//!
//! These `extern "C"` functions can be compiled into a static library
//! (.a / .xcframework) and called directly from Swift or Kotlin.
//!
//! Build for iOS:
//!   cargo build --release --target aarch64-apple-ios --lib
//!   cargo build --release --target aarch64-apple-ios-sim --lib
//!
//! Build for Android:
//!   cargo build --release --target aarch64-linux-android --lib
//!
//! The App Clip contains ~50 KB of this library. Combined with the QR
//! seed payload, the user experience is: Scan → Boot → Intelligence.

use crate::compress;
use crate::qr_seed::ParsedSeed;
use crate::seed_crypto;
use rvf_types::qr_seed::{SeedHeader, SEED_HEADER_SIZE};

/// Result codes for FFI functions.
pub const RVQS_OK: i32 = 0;
pub const RVQS_ERR_NULL_PTR: i32 = -1;
pub const RVQS_ERR_TOO_SHORT: i32 = -2;
pub const RVQS_ERR_BAD_MAGIC: i32 = -3;
pub const RVQS_ERR_SIGNATURE_INVALID: i32 = -4;
pub const RVQS_ERR_HASH_MISMATCH: i32 = -5;
pub const RVQS_ERR_DECOMPRESS_FAIL: i32 = -6;
pub const RVQS_ERR_BUFFER_TOO_SMALL: i32 = -7;
pub const RVQS_ERR_PARSE_FAIL: i32 = -8;

/// Opaque header struct for C interop (mirrors SeedHeader layout).
#[repr(C)]
pub struct RvqsHeaderC {
    pub seed_magic: u32,
    pub seed_version: u16,
    pub flags: u16,
    pub file_id: [u8; 8],
    pub total_vector_count: u32,
    pub dimension: u16,
    pub base_dtype: u8,
    pub profile_id: u8,
    pub created_ns: u64,
    pub microkernel_offset: u32,
    pub microkernel_size: u32,
    pub download_manifest_offset: u32,
    pub download_manifest_size: u32,
    pub sig_algo: u16,
    pub sig_length: u16,
    pub total_seed_size: u32,
    pub content_hash: [u8; 8],
}

const _: () = assert!(core::mem::size_of::<RvqsHeaderC>() == SEED_HEADER_SIZE);

impl From<SeedHeader> for RvqsHeaderC {
    fn from(h: SeedHeader) -> Self {
        Self {
            seed_magic: h.seed_magic,
            seed_version: h.seed_version,
            flags: h.flags,
            file_id: h.file_id,
            total_vector_count: h.total_vector_count,
            dimension: h.dimension,
            base_dtype: h.base_dtype,
            profile_id: h.profile_id,
            created_ns: h.created_ns,
            microkernel_offset: h.microkernel_offset,
            microkernel_size: h.microkernel_size,
            download_manifest_offset: h.download_manifest_offset,
            download_manifest_size: h.download_manifest_size,
            sig_algo: h.sig_algo,
            sig_length: h.sig_length,
            total_seed_size: h.total_seed_size,
            content_hash: h.content_hash,
        }
    }
}

/// Parse a QR seed payload and extract the header.
///
/// # Safety
/// `data` must point to `data_len` valid bytes. `out` must point to a valid `RvqsHeaderC`.
#[no_mangle]
pub unsafe extern "C" fn rvqs_parse_header(
    data: *const u8,
    data_len: usize,
    out: *mut RvqsHeaderC,
) -> i32 {
    if data.is_null() || out.is_null() {
        return RVQS_ERR_NULL_PTR;
    }
    if data_len < SEED_HEADER_SIZE {
        return RVQS_ERR_TOO_SHORT;
    }

    let slice = core::slice::from_raw_parts(data, data_len);
    match SeedHeader::from_bytes(slice) {
        Ok(header) => {
            *out = header.into();
            RVQS_OK
        }
        Err(_) => RVQS_ERR_BAD_MAGIC,
    }
}

/// Verify the HMAC-SHA256 signature of a QR seed.
///
/// # Safety
/// All pointers must be valid for their respective lengths.
#[no_mangle]
pub unsafe extern "C" fn rvqs_verify_signature(
    data: *const u8,
    data_len: usize,
    key: *const u8,
    key_len: usize,
) -> i32 {
    if data.is_null() || key.is_null() {
        return RVQS_ERR_NULL_PTR;
    }
    if data_len < SEED_HEADER_SIZE {
        return RVQS_ERR_TOO_SHORT;
    }

    let slice = core::slice::from_raw_parts(data, data_len);
    let key_slice = core::slice::from_raw_parts(key, key_len);

    let parsed = match ParsedSeed::parse(slice) {
        Ok(p) => p,
        Err(_) => return RVQS_ERR_PARSE_FAIL,
    };

    let signature = match parsed.signature {
        Some(s) => s,
        None => return RVQS_ERR_SIGNATURE_INVALID,
    };

    let signed_payload = match parsed.signed_payload(slice) {
        Some(p) => p,
        None => return RVQS_ERR_SIGNATURE_INVALID,
    };

    if seed_crypto::verify_seed(key_slice, signed_payload, signature) {
        RVQS_OK
    } else {
        RVQS_ERR_SIGNATURE_INVALID
    }
}

/// Verify the content hash of a QR seed payload.
///
/// # Safety
/// `data` must point to `data_len` valid bytes.
#[no_mangle]
pub unsafe extern "C" fn rvqs_verify_content_hash(data: *const u8, data_len: usize) -> i32 {
    if data.is_null() {
        return RVQS_ERR_NULL_PTR;
    }
    if data_len < SEED_HEADER_SIZE {
        return RVQS_ERR_TOO_SHORT;
    }

    let slice = core::slice::from_raw_parts(data, data_len);
    let parsed = match ParsedSeed::parse(slice) {
        Ok(p) => p,
        Err(_) => return RVQS_ERR_PARSE_FAIL,
    };

    let microkernel = parsed.microkernel.unwrap_or(&[]);
    let manifest = parsed.manifest_bytes.unwrap_or(&[]);

    let mut hash_input = Vec::with_capacity(microkernel.len() + manifest.len());
    hash_input.extend_from_slice(microkernel);
    hash_input.extend_from_slice(manifest);

    if seed_crypto::verify_content_hash(&parsed.header.content_hash, &hash_input) {
        RVQS_OK
    } else {
        RVQS_ERR_HASH_MISMATCH
    }
}

/// Decompress the microkernel from a QR seed.
///
/// # Safety
/// `data` must point to `data_len` valid bytes. `out` must point to `out_cap` bytes.
/// `out_len` will receive the actual decompressed size.
#[no_mangle]
pub unsafe extern "C" fn rvqs_decompress_microkernel(
    data: *const u8,
    data_len: usize,
    out: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> i32 {
    if data.is_null() || out.is_null() || out_len.is_null() {
        return RVQS_ERR_NULL_PTR;
    }

    let slice = core::slice::from_raw_parts(data, data_len);
    let parsed = match ParsedSeed::parse(slice) {
        Ok(p) => p,
        Err(_) => return RVQS_ERR_PARSE_FAIL,
    };

    let compressed = match parsed.microkernel {
        Some(m) => m,
        None => {
            *out_len = 0;
            return RVQS_OK;
        }
    };

    let decompressed = match compress::decompress(compressed) {
        Ok(d) => d,
        Err(_) => return RVQS_ERR_DECOMPRESS_FAIL,
    };

    if decompressed.len() > out_cap {
        return RVQS_ERR_BUFFER_TOO_SMALL;
    }

    let out_slice = core::slice::from_raw_parts_mut(out, out_cap);
    out_slice[..decompressed.len()].copy_from_slice(&decompressed);
    *out_len = decompressed.len();

    RVQS_OK
}

/// Get the download manifest URL from a parsed seed.
///
/// # Safety
/// All pointers must be valid. `url_buf` must have `url_cap` bytes available.
#[no_mangle]
pub unsafe extern "C" fn rvqs_get_primary_host_url(
    data: *const u8,
    data_len: usize,
    url_buf: *mut u8,
    url_cap: usize,
    url_len: *mut usize,
) -> i32 {
    if data.is_null() || url_buf.is_null() || url_len.is_null() {
        return RVQS_ERR_NULL_PTR;
    }

    let slice = core::slice::from_raw_parts(data, data_len);
    let parsed = match ParsedSeed::parse(slice) {
        Ok(p) => p,
        Err(_) => return RVQS_ERR_PARSE_FAIL,
    };

    let manifest = match parsed.parse_manifest() {
        Ok(m) => m,
        Err(_) => return RVQS_ERR_PARSE_FAIL,
    };

    let host = match manifest.hosts.first() {
        Some(h) => h,
        None => {
            *url_len = 0;
            return RVQS_OK;
        }
    };

    let url_bytes = &host.url[..host.url_length as usize];
    if url_bytes.len() > url_cap {
        return RVQS_ERR_BUFFER_TOO_SMALL;
    }

    let out_slice = core::slice::from_raw_parts_mut(url_buf, url_cap);
    out_slice[..url_bytes.len()].copy_from_slice(url_bytes);
    *url_len = url_bytes.len();

    RVQS_OK
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qr_seed::{make_host_entry, SeedBuilder};
    use rvf_types::qr_seed::*;

    fn build_signed_seed() -> Vec<u8> {
        let key = b"test-key-for-ffi-unit-testing-ok";
        let mk = crate::compress::compress(&[0xCA; 2000]);
        let host = make_host_entry("https://cdn.test.com/brain.rvf", 0, 1, [0xAA; 16]).unwrap();

        let builder = SeedBuilder::new([0x01; 8], 128, 1000)
            .with_microkernel(mk)
            .add_host(host);
        let (payload, _header) = builder.build_and_sign(key).unwrap();
        payload
    }

    #[test]
    fn ffi_parse_header() {
        let payload = build_signed_seed();
        let mut header = core::mem::MaybeUninit::<RvqsHeaderC>::uninit();
        let rc = unsafe { rvqs_parse_header(payload.as_ptr(), payload.len(), header.as_mut_ptr()) };
        assert_eq!(rc, RVQS_OK);
        let header = unsafe { header.assume_init() };
        assert_eq!(header.seed_magic, SEED_MAGIC);
        assert_eq!(header.dimension, 128);
    }

    #[test]
    fn ffi_verify_signature() {
        let key = b"test-key-for-ffi-unit-testing-ok";
        let payload = build_signed_seed();
        let rc = unsafe {
            rvqs_verify_signature(payload.as_ptr(), payload.len(), key.as_ptr(), key.len())
        };
        assert_eq!(rc, RVQS_OK);
    }

    #[test]
    fn ffi_verify_signature_wrong_key() {
        let payload = build_signed_seed();
        let bad_key = b"wrong-key-should-fail-verificatn";
        let rc = unsafe {
            rvqs_verify_signature(
                payload.as_ptr(),
                payload.len(),
                bad_key.as_ptr(),
                bad_key.len(),
            )
        };
        assert_eq!(rc, RVQS_ERR_SIGNATURE_INVALID);
    }

    #[test]
    fn ffi_verify_content_hash() {
        let payload = build_signed_seed();
        let rc = unsafe { rvqs_verify_content_hash(payload.as_ptr(), payload.len()) };
        assert_eq!(rc, RVQS_OK);
    }

    #[test]
    fn ffi_decompress_microkernel() {
        let payload = build_signed_seed();
        let mut out = vec![0u8; 8192];
        let mut out_len: usize = 0;
        let rc = unsafe {
            rvqs_decompress_microkernel(
                payload.as_ptr(),
                payload.len(),
                out.as_mut_ptr(),
                out.len(),
                &mut out_len,
            )
        };
        assert_eq!(rc, RVQS_OK);
        assert_eq!(out_len, 2000);
        assert_eq!(&out[..out_len], &[0xCA; 2000]);
    }

    #[test]
    fn ffi_get_primary_host_url() {
        let payload = build_signed_seed();
        let mut url_buf = vec![0u8; 256];
        let mut url_len: usize = 0;
        let rc = unsafe {
            rvqs_get_primary_host_url(
                payload.as_ptr(),
                payload.len(),
                url_buf.as_mut_ptr(),
                url_buf.len(),
                &mut url_len,
            )
        };
        assert_eq!(rc, RVQS_OK);
        let url = core::str::from_utf8(&url_buf[..url_len]).unwrap();
        assert_eq!(url, "https://cdn.test.com/brain.rvf");
    }

    #[test]
    fn ffi_null_ptr_returns_error() {
        let mut header = core::mem::MaybeUninit::<RvqsHeaderC>::uninit();
        let rc = unsafe { rvqs_parse_header(core::ptr::null(), 0, header.as_mut_ptr()) };
        assert_eq!(rc, RVQS_ERR_NULL_PTR);
    }
}
