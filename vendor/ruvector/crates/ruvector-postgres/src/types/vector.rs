//! Primary vector type implementation (RuVector)
//!
//! This is the main vector type, compatible with pgvector's `vector` type.
//! Stores f32 elements with efficient SIMD operations and zero-copy access.
//!
//! Memory layout (varlena-based for zero-copy):
//! - VARHDRSZ (4 bytes) - PostgreSQL varlena header
//! - dimensions (2 bytes u16)
//! - unused (2 bytes for alignment)
//! - data (4 bytes per dimension as f32)

use pgrx::pgrx_sql_entity_graph::metadata::{
    ArgumentError, Returns, ReturnsError, SqlMapping, SqlTranslatable,
};
use pgrx::prelude::*;
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString};
use std::fmt;
use std::ptr;
use std::str::FromStr;

use super::VectorData;
use crate::MAX_DIMENSIONS;

// ============================================================================
// Zero-Copy Varlena Structure
// ============================================================================

/// Local varlena header structure for RuVector (pgvector-compatible layout)
/// This is different from the mod.rs VectorHeader which uses u32 dimensions
#[repr(C, align(8))]
struct RuVectorHeader {
    /// Number of dimensions (u16 for pgvector compatibility)
    dimensions: u16,
    /// Padding for alignment (ensures f32 data is 8-byte aligned)
    _unused: u16,
}

impl RuVectorHeader {
    const SIZE: usize = 4; // 2 (dimensions) + 2 (padding)
}

// ============================================================================
// RuVector: High-Level API with Zero-Copy Support
// ============================================================================

/// RuVector: Primary vector type for PostgreSQL
///
/// This structure provides a high-level API over the varlena-based storage.
/// For zero-copy operations, it can work directly with PostgreSQL memory.
///
/// Maximum dimensions: 16,000
#[derive(Clone, Serialize, Deserialize)]
pub struct RuVector {
    /// Vector dimensions (cached for fast access)
    dimensions: u32,
    /// Vector data (f32 elements)
    data: Vec<f32>,
}

impl RuVector {
    /// Create a new vector from a slice
    pub fn from_slice(data: &[f32]) -> Self {
        if data.len() > MAX_DIMENSIONS {
            pgrx::error!(
                "Vector dimension {} exceeds maximum {}",
                data.len(),
                MAX_DIMENSIONS
            );
        }

        Self {
            dimensions: data.len() as u32,
            data: data.to_vec(),
        }
    }

    /// Create a zero vector of given dimensions
    pub fn zeros(dimensions: usize) -> Self {
        if dimensions > MAX_DIMENSIONS {
            pgrx::error!(
                "Vector dimension {} exceeds maximum {}",
                dimensions,
                MAX_DIMENSIONS
            );
        }

        Self {
            dimensions: dimensions as u32,
            data: vec![0.0; dimensions],
        }
    }

    /// Get vector dimensions
    #[inline]
    pub fn dimensions(&self) -> usize {
        self.dimensions as usize
    }

    /// Get vector data as slice
    #[inline]
    pub fn as_slice(&self) -> &[f32] {
        &self.data
    }

    /// Get mutable vector data
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [f32] {
        &mut self.data
    }

    /// Convert to Vec<f32>
    pub fn into_vec(self) -> Vec<f32> {
        self.data
    }

    /// Calculate L2 norm
    pub fn norm(&self) -> f32 {
        self.data.iter().map(|x| x * x).sum::<f32>().sqrt()
    }

    /// Normalize to unit vector
    pub fn normalize(&self) -> Self {
        let norm = self.norm();
        if norm == 0.0 {
            return self.clone();
        }
        Self {
            dimensions: self.dimensions,
            data: self.data.iter().map(|x| x / norm).collect(),
        }
    }

    /// Element-wise addition
    pub fn add(&self, other: &Self) -> Self {
        assert_eq!(
            self.dimensions, other.dimensions,
            "Vector dimensions must match"
        );
        Self {
            dimensions: self.dimensions,
            data: self
                .data
                .iter()
                .zip(&other.data)
                .map(|(a, b)| a + b)
                .collect(),
        }
    }

    /// Element-wise subtraction
    pub fn sub(&self, other: &Self) -> Self {
        assert_eq!(
            self.dimensions, other.dimensions,
            "Vector dimensions must match"
        );
        Self {
            dimensions: self.dimensions,
            data: self
                .data
                .iter()
                .zip(&other.data)
                .map(|(a, b)| a - b)
                .collect(),
        }
    }

    /// Scalar multiplication
    pub fn mul_scalar(&self, scalar: f32) -> Self {
        Self {
            dimensions: self.dimensions,
            data: self.data.iter().map(|x| x * scalar).collect(),
        }
    }

    /// Dot product
    pub fn dot(&self, other: &Self) -> f32 {
        assert_eq!(
            self.dimensions, other.dimensions,
            "Vector dimensions must match"
        );
        self.data.iter().zip(&other.data).map(|(a, b)| a * b).sum()
    }

    /// Memory size in bytes (data only, not including varlena header)
    pub fn data_memory_size(&self) -> usize {
        RuVectorHeader::SIZE + self.data.len() * std::mem::size_of::<f32>()
    }

    /// Create from varlena pointer (zero-copy read)
    ///
    /// # Safety
    /// The pointer must be a valid varlena structure with proper layout
    unsafe fn from_varlena(varlena_ptr: *const pgrx::pg_sys::varlena) -> Self {
        // Get the total size and validate
        let total_size = pgrx::varlena::varsize_any(varlena_ptr);
        if total_size < RuVectorHeader::SIZE + pgrx::pg_sys::VARHDRSZ {
            pgrx::error!("Invalid vector: size too small");
        }

        // Get pointer to our header (skip varlena header)
        let data_ptr = pgrx::varlena::vardata_any(varlena_ptr) as *const u8;

        // Read dimensions (at offset 0 from data_ptr)
        let dimensions = ptr::read_unaligned(data_ptr as *const u16);

        if dimensions as usize > MAX_DIMENSIONS {
            pgrx::error!(
                "Vector dimension {} exceeds maximum {}",
                dimensions,
                MAX_DIMENSIONS
            );
        }

        // Validate total size
        let expected_size = RuVectorHeader::SIZE + (dimensions as usize * 4);
        let actual_size = total_size - pgrx::pg_sys::VARHDRSZ;

        if actual_size != expected_size {
            pgrx::error!(
                "Invalid vector: expected {} bytes, got {}",
                expected_size,
                actual_size
            );
        }

        // Get pointer to f32 data (skip dimensions u16 + padding u16 = 4 bytes)
        let f32_ptr = data_ptr.add(4) as *const f32;

        // Copy data into Vec (this is the only copy we need)
        let data = std::slice::from_raw_parts(f32_ptr, dimensions as usize).to_vec();

        Self {
            dimensions: dimensions as u32,
            data,
        }
    }

    /// Convert to varlena (allocate in PostgreSQL memory)
    ///
    /// # Safety
    /// This allocates memory using PostgreSQL's allocator
    unsafe fn to_varlena(&self) -> *mut pgrx::pg_sys::varlena {
        let dimensions = self.dimensions as u16;

        // Calculate sizes
        let data_size = 4 + (dimensions as usize * 4); // 2 (dims) + 2 (padding) + n*4 (data)
        let total_size = pgrx::pg_sys::VARHDRSZ + data_size;

        // Allocate PostgreSQL memory
        let varlena_ptr = pgrx::pg_sys::palloc(total_size) as *mut pgrx::pg_sys::varlena;

        // Set varlena size
        pgrx::varlena::set_varsize_4b(varlena_ptr, total_size as i32);

        // Get data pointer
        let data_ptr = pgrx::varlena::vardata_any(varlena_ptr) as *mut u8;

        // Write dimensions (2 bytes)
        ptr::write_unaligned(data_ptr as *mut u16, dimensions);

        // Write padding (2 bytes of zeros)
        ptr::write_unaligned(data_ptr.add(2) as *mut u16, 0);

        // Write f32 data
        let f32_ptr = data_ptr.add(4) as *mut f32;
        ptr::copy_nonoverlapping(self.data.as_ptr(), f32_ptr, dimensions as usize);

        varlena_ptr
    }
}

impl fmt::Display for RuVector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, val) in self.data.iter().enumerate() {
            if i > 0 {
                write!(f, ",")?;
            }
            write!(f, "{}", val)?;
        }
        write!(f, "]")
    }
}

impl fmt::Debug for RuVector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RuVector(dims={}, {:?})", self.dimensions, &self.data)
    }
}

impl FromStr for RuVector {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Parse format: [1.0, 2.0, 3.0] or [1,2,3]
        let s = s.trim();
        if !s.starts_with('[') || !s.ends_with(']') {
            return Err(format!(
                "Invalid vector format: must be enclosed in brackets"
            ));
        }

        let inner = &s[1..s.len() - 1];
        if inner.is_empty() {
            return Ok(Self::zeros(0));
        }

        let values: Result<Vec<f32>, _> = inner
            .split(',')
            .map(|v| {
                let trimmed = v.trim();
                trimmed
                    .parse::<f32>()
                    .map_err(|e| format!("Invalid number '{}': {}", trimmed, e))
            })
            .collect();

        match values {
            Ok(data) => {
                // Check for NaN and Infinity
                for (i, val) in data.iter().enumerate() {
                    if val.is_nan() {
                        return Err(format!("NaN not allowed at position {}", i));
                    }
                    if val.is_infinite() {
                        return Err(format!("Infinity not allowed at position {}", i));
                    }
                }
                Ok(Self::from_slice(&data))
            }
            Err(e) => Err(e),
        }
    }
}

impl PartialEq for RuVector {
    fn eq(&self, other: &Self) -> bool {
        self.dimensions == other.dimensions && self.data == other.data
    }
}

impl Eq for RuVector {}

// ============================================================================
// VectorData Trait Implementation (Zero-Copy Interface)
// ============================================================================

impl VectorData for RuVector {
    unsafe fn data_ptr(&self) -> *const f32 {
        self.data.as_ptr()
    }

    unsafe fn data_ptr_mut(&mut self) -> *mut f32 {
        self.data.as_mut_ptr()
    }

    fn dimensions(&self) -> usize {
        self.dimensions as usize
    }

    fn as_slice(&self) -> &[f32] {
        &self.data
    }

    fn as_mut_slice(&mut self) -> &mut [f32] {
        &mut self.data
    }

    fn memory_size(&self) -> usize {
        RuVectorHeader::SIZE + self.data.len() * std::mem::size_of::<f32>()
    }
}

// ============================================================================
// PostgreSQL Type I/O Functions (Native Interface)
// ============================================================================
// Using pgrx pg_extern for proper function registration

/// Text input function: Parse '[1.0, 2.0, 3.0]' to RuVector
#[pg_extern(immutable, parallel_safe, sql = false)]
pub fn ruvector_in_fn(input: &std::ffi::CStr) -> RuVector {
    let input_str = match input.to_str() {
        Ok(s) => s,
        Err(_) => pgrx::error!("Invalid UTF-8 in vector input"),
    };

    match RuVector::from_str(input_str) {
        Ok(vec) => vec,
        Err(e) => pgrx::error!("Invalid vector format: {}", e),
    }
}

/// Text output function: Convert RuVector to '[1.0, 2.0, 3.0]'
#[pg_extern(immutable, parallel_safe, sql = false)]
pub fn ruvector_out_fn(v: RuVector) -> String {
    v.to_string()
}

// Low-level C functions for PostgreSQL type system
// These provide PG_FUNCTION_INFO_V1 compatible registration

/// Text input function: Parse '[1.0, 2.0, 3.0]' to RuVector varlena
///
/// This is the PostgreSQL IN function for the ruvector type.
#[pg_guard]
#[no_mangle]
pub extern "C" fn ruvector_in(fcinfo: pg_sys::FunctionCallInfo) -> pg_sys::Datum {
    unsafe {
        let datum = (*fcinfo).args.as_ptr().add(0).read().value;
        let input_cstr = datum.cast_mut_ptr::<std::os::raw::c_char>();
        let input = CStr::from_ptr(input_cstr);

        let input_str = match input.to_str() {
            Ok(s) => s,
            Err(_) => pgrx::error!("Invalid UTF-8 in vector input"),
        };

        let vector = match RuVector::from_str(input_str) {
            Ok(vec) => vec,
            Err(e) => pgrx::error!("Invalid vector format: {}", e),
        };

        pg_sys::Datum::from(vector.to_varlena())
    }
}

// Register pg_finfo symbol
#[no_mangle]
pub extern "C" fn pg_finfo_ruvector_in() -> &'static pg_sys::Pg_finfo_record {
    static FINFO: pg_sys::Pg_finfo_record = pg_sys::Pg_finfo_record { api_version: 1 };
    &FINFO
}

/// Text output function: Convert RuVector to '[1.0, 2.0, 3.0]'
#[pg_guard]
#[no_mangle]
pub extern "C" fn ruvector_out(fcinfo: pg_sys::FunctionCallInfo) -> pg_sys::Datum {
    unsafe {
        let datum = (*fcinfo).args.as_ptr().add(0).read().value;
        let varlena_ptr = datum.cast_mut_ptr::<pg_sys::varlena>();

        // CRITICAL: Must detoast before reading - data may be compressed/external
        let detoasted_ptr = pg_sys::pg_detoast_datum(varlena_ptr);
        let vector = RuVector::from_varlena(detoasted_ptr);

        let output = vector.to_string();
        let cstring = match CString::new(output) {
            Ok(s) => s,
            Err(_) => pgrx::error!("Failed to create output string"),
        };

        let len = cstring.as_bytes_with_nul().len();
        let pg_str = pg_sys::palloc(len) as *mut std::os::raw::c_char;
        ptr::copy_nonoverlapping(cstring.as_ptr(), pg_str, len);

        pg_sys::Datum::from(pg_str)
    }
}

#[no_mangle]
pub extern "C" fn pg_finfo_ruvector_out() -> &'static pg_sys::Pg_finfo_record {
    static FINFO: pg_sys::Pg_finfo_record = pg_sys::Pg_finfo_record { api_version: 1 };
    &FINFO
}

/// Binary input function: Receive vector from network in binary format
#[pg_guard]
#[no_mangle]
pub extern "C" fn ruvector_recv(fcinfo: pg_sys::FunctionCallInfo) -> pg_sys::Datum {
    unsafe {
        let datum = (*fcinfo).args.as_ptr().add(0).read().value;
        let buf = datum.cast_mut_ptr::<pg_sys::StringInfoData>();
        let buf_ptr = buf;

        let dimensions = pg_sys::pq_getmsgint(buf_ptr, 2) as u16;

        if dimensions as usize > MAX_DIMENSIONS {
            pgrx::error!(
                "Vector dimension {} exceeds maximum {}",
                dimensions,
                MAX_DIMENSIONS
            );
        }

        let mut data = Vec::with_capacity(dimensions as usize);
        for _ in 0..dimensions {
            let int_bits = pg_sys::pq_getmsgint(buf_ptr, 4) as u32;
            let float_val = f32::from_bits(int_bits);

            if float_val.is_nan() {
                pgrx::error!("NaN not allowed in vector");
            }
            if float_val.is_infinite() {
                pgrx::error!("Infinity not allowed in vector");
            }

            data.push(float_val);
        }

        let vector = RuVector::from_slice(&data);
        pg_sys::Datum::from(vector.to_varlena())
    }
}

#[no_mangle]
pub extern "C" fn pg_finfo_ruvector_recv() -> &'static pg_sys::Pg_finfo_record {
    static FINFO: pg_sys::Pg_finfo_record = pg_sys::Pg_finfo_record { api_version: 1 };
    &FINFO
}

/// Binary output function: Send vector in binary format over network
///
/// This is the PostgreSQL SEND function for the ruvector type.
/// Binary format matches ruvector_recv.
#[no_mangle]
pub extern "C" fn ruvector_send(fcinfo: pg_sys::FunctionCallInfo) -> pg_sys::Datum {
    unsafe {
        // Access first argument (varlena vector)
        let datum = (*fcinfo).args.as_ptr().add(0).read().value;
        let varlena_ptr = datum.cast_mut_ptr::<pg_sys::varlena>();

        // CRITICAL: Must detoast before reading - data may be compressed/external
        let detoasted_ptr = pg_sys::pg_detoast_datum(varlena_ptr);
        let vector = RuVector::from_varlena(detoasted_ptr);

        // Create StringInfo for output
        let buf = pg_sys::makeStringInfo();

        // Write dimensions (2 bytes, big-endian) - pq_sendint expects u32 in pgrx 0.12
        pg_sys::pq_sendint(buf, vector.dimensions, 2);

        // Write f32 data
        for &val in vector.as_slice() {
            // Convert f32 to bits and send (network byte order)
            let int_bits = val.to_bits();
            pg_sys::pq_sendint(buf, int_bits, 4);
        }

        // Convert StringInfo to bytea
        let data_ptr = (*buf).data;
        let data_len = (*buf).len as usize;

        // Allocate bytea
        let bytea_size = pg_sys::VARHDRSZ + data_len;
        let bytea_ptr = pg_sys::palloc(bytea_size) as *mut pg_sys::bytea;

        // Set size
        pgrx::varlena::set_varsize_4b(bytea_ptr, bytea_size as i32);

        // Copy data
        let bytea_data = pgrx::varlena::vardata_any(bytea_ptr as *const pg_sys::varlena) as *mut u8;
        ptr::copy_nonoverlapping(data_ptr as *const u8, bytea_data, data_len);

        // Free StringInfo
        pg_sys::pfree(buf as *mut std::ffi::c_void);

        pg_sys::Datum::from(bytea_ptr)
    }
}

#[no_mangle]
pub extern "C" fn pg_finfo_ruvector_send() -> &'static pg_sys::Pg_finfo_record {
    static FINFO: pg_sys::Pg_finfo_record = pg_sys::Pg_finfo_record { api_version: 1 };
    &FINFO
}

// ============================================================================
// TypeMod Functions (for dimension specification like ruvector(384))
// ============================================================================

/// Typmod input function: parse dimension specification
/// Called when user specifies ruvector(dimensions) in a column type
#[pg_extern(immutable, strict, parallel_safe)]
fn ruvector_typmod_in_fn(list: pgrx::Array<&CStr>) -> i32 {
    // Should have exactly one element (dimensions)
    if list.len() != 1 {
        pgrx::error!("ruvector type modifier must have exactly one dimension");
    }

    // Get the first element
    let dim_str = list
        .get(0)
        .flatten()
        .ok_or_else(|| pgrx::error!("ruvector dimension cannot be null"))
        .unwrap();

    // Parse the dimension string
    let dim_str_rust = dim_str.to_str().unwrap_or("0");
    let dimensions: i32 = dim_str_rust.parse().unwrap_or_else(|_| {
        pgrx::error!("invalid dimension specification: {}", dim_str_rust);
    });

    // Validate dimensions
    if dimensions < 1 || dimensions > MAX_DIMENSIONS as i32 {
        pgrx::error!(
            "dimensions must be between 1 and {}, got {}",
            MAX_DIMENSIONS,
            dimensions
        );
    }

    dimensions
}

/// Low-level wrapper for typmod_in (for CREATE TYPE)
///
/// This function parses dimension specifications like `ruvector(128)` from PostgreSQL.
/// It uses PostgreSQL's array accessor macros for robust array element access.
#[pg_guard]
#[no_mangle]
pub extern "C" fn ruvector_typmod_in(fcinfo: pg_sys::FunctionCallInfo) -> pg_sys::Datum {
    unsafe {
        // Get the cstring array argument
        let array_datum = (*fcinfo).args.as_ptr().add(0).read().value;
        let array_ptr = array_datum.cast_mut_ptr::<pg_sys::ArrayType>();

        if array_ptr.is_null() {
            pgrx::error!("ruvector type modifier cannot be null");
        }

        // Validate array dimensionality using PostgreSQL's ARR_NDIM macro equivalent
        let ndim = (*array_ptr).ndim;
        if ndim != 1 {
            pgrx::error!("ruvector type modifier must be a 1D array, got {}D", ndim);
        }

        // Get array dimensions using ARR_DIMS macro equivalent
        // ARR_DIMS returns pointer to first element of dims array (right after the header)
        let dims_ptr =
            (array_ptr as *const u8).add(std::mem::size_of::<pg_sys::ArrayType>()) as *const i32;
        let nelems = *dims_ptr;

        if nelems != 1 {
            pgrx::error!(
                "ruvector type modifier must have exactly one element, got {}",
                nelems
            );
        }

        // Calculate data offset using ARR_DATA_OFFSET macro equivalent
        // If dataoffset is 0, there's no null bitmap
        let data_offset = if (*array_ptr).dataoffset == 0 {
            // No null bitmap: header + dims + lbounds
            // dims and lbounds each have ndim i32 elements
            let header_size = std::mem::size_of::<pg_sys::ArrayType>();
            let dims_lbounds_size = (ndim as usize) * std::mem::size_of::<i32>() * 2;
            header_size + dims_lbounds_size
        } else {
            // dataoffset includes the null bitmap size
            (*array_ptr).dataoffset as usize
        };

        // Get pointer to first cstring element
        let first_elem_ptr = (array_ptr as *const u8).add(data_offset) as *const i8;

        if first_elem_ptr.is_null() {
            pgrx::error!("ruvector type modifier element is null");
        }

        // Parse the dimension string safely
        let dim_cstr = CStr::from_ptr(first_elem_ptr);
        let dim_str = dim_cstr.to_str().unwrap_or_else(|_| {
            pgrx::error!("ruvector type modifier contains invalid UTF-8");
        });

        // Trim whitespace and parse
        let dim_str_trimmed = dim_str.trim();
        if dim_str_trimmed.is_empty() {
            pgrx::error!("ruvector type modifier cannot be empty");
        }

        let dimensions: i32 = dim_str_trimmed.parse().unwrap_or_else(|e| {
            pgrx::error!(
                "invalid dimension specification '{}': {}",
                dim_str_trimmed,
                e
            );
        });

        // Validate dimension range
        if dimensions < 1 {
            pgrx::error!("dimensions must be at least 1, got {}", dimensions);
        }
        if dimensions > MAX_DIMENSIONS as i32 {
            pgrx::error!(
                "dimensions {} exceeds maximum allowed {}",
                dimensions,
                MAX_DIMENSIONS
            );
        }

        pg_sys::Datum::from(dimensions)
    }
}

#[no_mangle]
pub extern "C" fn pg_finfo_ruvector_typmod_in() -> &'static pg_sys::Pg_finfo_record {
    static FINFO: pg_sys::Pg_finfo_record = pg_sys::Pg_finfo_record { api_version: 1 };
    &FINFO
}

/// Typmod output function: format dimension specification for display
#[pg_guard]
#[no_mangle]
pub extern "C" fn ruvector_typmod_out(fcinfo: pg_sys::FunctionCallInfo) -> pg_sys::Datum {
    unsafe {
        let typmod = (*fcinfo).args.as_ptr().add(0).read().value.value() as i32;

        // Format as "(dimensions)"
        let output = format!("({})", typmod);
        let c_str = CString::new(output).unwrap();

        // Allocate in PostgreSQL memory
        let len = c_str.as_bytes_with_nul().len();
        let pg_str = pg_sys::palloc(len) as *mut i8;
        ptr::copy_nonoverlapping(c_str.as_ptr(), pg_str, len);

        pg_sys::Datum::from(pg_str)
    }
}

#[no_mangle]
pub extern "C" fn pg_finfo_ruvector_typmod_out() -> &'static pg_sys::Pg_finfo_record {
    static FINFO: pg_sys::Pg_finfo_record = pg_sys::Pg_finfo_record { api_version: 1 };
    &FINFO
}

// ============================================================================
// PostgreSQL Type Integration
// ============================================================================

unsafe impl SqlTranslatable for RuVector {
    fn argument_sql() -> Result<SqlMapping, ArgumentError> {
        Ok(SqlMapping::As(String::from("ruvector")))
    }

    fn return_sql() -> Result<Returns, ReturnsError> {
        Ok(Returns::One(SqlMapping::As(String::from("ruvector"))))
    }
}

impl pgrx::IntoDatum for RuVector {
    fn into_datum(self) -> Option<pgrx::pg_sys::Datum> {
        unsafe {
            let varlena_ptr = self.to_varlena();
            Some(pgrx::pg_sys::Datum::from(varlena_ptr))
        }
    }

    fn type_oid() -> pgrx::pg_sys::Oid {
        pgrx::pg_sys::Oid::INVALID
    }
}

impl pgrx::FromDatum for RuVector {
    unsafe fn from_polymorphic_datum(
        datum: pgrx::pg_sys::Datum,
        is_null: bool,
        _typoid: pgrx::pg_sys::Oid,
    ) -> Option<Self> {
        if is_null || datum.is_null() {
            return None;
        }

        // IMPORTANT: Must detoast before reading - varlena may be compressed/external
        // Use pg_detoast_datum_copy to always get a clean palloc'd copy
        let raw_ptr = datum.cast_mut_ptr::<pg_sys::varlena>();
        if raw_ptr.is_null() {
            return None;
        }

        // Detoast (handles TOAST compressed/external storage)
        // Use pg_detoast_datum which avoids copy if already detoasted
        let detoasted_ptr = pg_sys::pg_detoast_datum(raw_ptr);
        if detoasted_ptr.is_null() {
            return None;
        }

        // Use pgrx varlena helpers to read the detoasted data
        let total_size = pgrx::varlena::varsize_any(detoasted_ptr as *const _);
        if total_size < RuVectorHeader::SIZE + pg_sys::VARHDRSZ {
            pgrx::error!(
                "Invalid vector from storage: size too small ({})",
                total_size
            );
        }

        let data_ptr = pgrx::varlena::vardata_any(detoasted_ptr as *const _) as *const u8;
        if data_ptr.is_null() {
            return None;
        }

        // Read dimensions (at offset 0 from data_ptr)
        let dimensions = ptr::read_unaligned(data_ptr as *const u16);

        if dimensions as usize > MAX_DIMENSIONS {
            pgrx::error!(
                "Vector dimension {} exceeds maximum {}",
                dimensions,
                MAX_DIMENSIONS
            );
        }

        // Get pointer to f32 data (skip dimensions u16 + padding u16 = 4 bytes)
        let f32_ptr = data_ptr.add(4) as *const f32;

        // Copy data into Vec
        let data = std::slice::from_raw_parts(f32_ptr, dimensions as usize).to_vec();

        Some(Self {
            dimensions: dimensions as u32,
            data,
        })
    }
}

// ============================================================================
// ArgAbi and BoxRet Implementations for Native Type Support
// ============================================================================
// These implementations allow RuVector to be used directly in #[pg_extern] functions

unsafe impl<'fcx> pgrx::callconv::ArgAbi<'fcx> for RuVector {
    unsafe fn unbox_arg_unchecked(arg: pgrx::callconv::Arg<'_, 'fcx>) -> Self {
        // Use the helper method that leverages FromDatum
        arg.unbox_arg_using_from_datum::<RuVector>()
            .expect("ruvector argument must not be null")
    }

    unsafe fn unbox_nullable_arg(
        arg: pgrx::callconv::Arg<'_, 'fcx>,
    ) -> pgrx::nullable::Nullable<Self> {
        match arg.unbox_arg_using_from_datum::<RuVector>() {
            Some(v) => pgrx::nullable::Nullable::Valid(v),
            None => pgrx::nullable::Nullable::Null,
        }
    }
}

unsafe impl pgrx::callconv::BoxRet for RuVector {
    unsafe fn box_into<'fcx>(
        self,
        fcinfo: &mut pgrx::callconv::FcInfo<'fcx>,
    ) -> pgrx::datum::Datum<'fcx> {
        match self.into_datum() {
            Some(datum) => fcinfo.return_raw_datum(datum),
            None => fcinfo.return_null(),
        }
    }
}

// ============================================================================
// SQL Helper Functions - Note: Using array-based functions for pgrx 0.12 compat
// ============================================================================
// The native ruvector type is used through the C-level I/O functions
// (ruvector_in, ruvector_out, ruvector_recv, ruvector_send) which bypass
// the pgrx ArgAbi/RetAbi trait requirements.

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_slice() {
        let v = RuVector::from_slice(&[1.0, 2.0, 3.0]);
        assert_eq!(v.dimensions(), 3);
        assert_eq!(v.as_slice(), &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_zeros() {
        let v = RuVector::zeros(5);
        assert_eq!(v.dimensions(), 5);
        assert_eq!(v.as_slice(), &[0.0, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_norm() {
        let v = RuVector::from_slice(&[3.0, 4.0]);
        assert!((v.norm() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_normalize() {
        let v = RuVector::from_slice(&[3.0, 4.0]);
        let n = v.normalize();
        assert!((n.norm() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_dot() {
        let a = RuVector::from_slice(&[1.0, 2.0, 3.0]);
        let b = RuVector::from_slice(&[4.0, 5.0, 6.0]);
        assert!((a.dot(&b) - 32.0).abs() < 1e-6);
    }

    #[test]
    fn test_add_sub() {
        let a = RuVector::from_slice(&[1.0, 2.0]);
        let b = RuVector::from_slice(&[3.0, 4.0]);
        assert_eq!(a.add(&b).as_slice(), &[4.0, 6.0]);
        assert_eq!(b.sub(&a).as_slice(), &[2.0, 2.0]);
    }

    #[test]
    fn test_parse() {
        let v: RuVector = "[1.0, 2.0, 3.0]".parse().unwrap();
        assert_eq!(v.as_slice(), &[1.0, 2.0, 3.0]);

        let v2: RuVector = "[1,2,3]".parse().unwrap();
        assert_eq!(v2.as_slice(), &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_parse_invalid() {
        assert!("not a vector".parse::<RuVector>().is_err());
        assert!("[1.0, nan, 3.0]".parse::<RuVector>().is_err());
        assert!("[1.0, inf, 3.0]".parse::<RuVector>().is_err());
    }

    #[test]
    fn test_display() {
        let v = RuVector::from_slice(&[1.0, 2.5, 3.0]);
        assert_eq!(v.to_string(), "[1,2.5,3]");
    }

    #[test]
    fn test_varlena_roundtrip() {
        unsafe {
            let v1 = RuVector::from_slice(&[1.0, 2.0, 3.0, 4.0, 5.0]);
            let varlena = v1.to_varlena();
            let v2 = RuVector::from_varlena(varlena);
            assert_eq!(v1, v2);
            pgrx::pg_sys::pfree(varlena as *mut std::ffi::c_void);
        }
    }

    #[test]
    fn test_memory_size() {
        let v = RuVector::from_slice(&[1.0, 2.0, 3.0]);
        let size = v.data_memory_size();
        // Header (4 bytes: 2 dims + 2 padding) + 3 * 4 bytes = 16 bytes
        assert_eq!(size, 16);
    }
}

// Note: PostgreSQL integration tests for the ruvector type are done via
// SQL-level testing since the type uses raw C calling conventions.
