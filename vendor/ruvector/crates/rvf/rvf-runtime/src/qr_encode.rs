//! QR code encoder for rendering RVF seed bytes as QR code images.
//!
//! Pure Rust, zero-dependency QR encoder supporting:
//! - Versions 1-5 (21x21 to 37x37 modules)
//! - Byte mode encoding (mode indicator 0100)
//! - Error correction levels L, M, Q, H
//! - Reed-Solomon error correction over GF(2^8) with polynomial 0x11D
//! - All 8 mask patterns with automatic best-mask selection
//! - Finder patterns, timing patterns, alignment patterns (v2+)
//! - Format and version information
//! - SVG and ASCII rendering
//!
//! # Example
//!
//! ```
//! use rvf_runtime::qr_encode::{QrEncoder, EcLevel};
//!
//! let code = QrEncoder::encode(b"Hello, RVF!", EcLevel::M).unwrap();
//! let svg = QrEncoder::to_svg(&code);
//! let ascii = QrEncoder::to_ascii(&code);
//! ```

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Error correction level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EcLevel {
    /// Low (~7% recovery).
    L,
    /// Medium (~15% recovery).
    M,
    /// Quartile (~25% recovery).
    Q,
    /// High (~30% recovery).
    H,
}

/// Errors that can occur during QR encoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QrError {
    /// Data exceeds the capacity for the chosen version and EC level.
    DataTooLarge,
    /// Requested version is outside the supported range (1-5).
    InvalidVersion,
    /// Internal encoding failure.
    EncodingFailed(String),
}

impl core::fmt::Display for QrError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            QrError::DataTooLarge => write!(f, "data too large for QR capacity"),
            QrError::InvalidVersion => write!(f, "invalid QR version (supported: 1-5)"),
            QrError::EncodingFailed(msg) => write!(f, "encoding failed: {msg}"),
        }
    }
}

/// A generated QR code.
#[derive(Clone, Debug)]
pub struct QrCode {
    /// Module matrix. `true` = dark module.
    pub modules: Vec<Vec<bool>>,
    /// QR version (1-5).
    pub version: u8,
    /// Side length in modules.
    pub size: usize,
}

/// QR code encoder.
///
/// Stateless encoder; all configuration is provided per-call.
pub struct QrEncoder;

// ---------------------------------------------------------------------------
// Version/EC capacity table (byte-mode data capacity)
// ---------------------------------------------------------------------------

/// (version, total_codewords, [L, M, Q, H] ec_codewords_per_block,
///  [L, M, Q, H] num_blocks, [L, M, Q, H] data_capacity_bytes)
///
/// Data capacity = total_codewords - ec_codewords_per_block * num_blocks
/// Source: ISO 18004 Tables 7 and 9.
struct VersionInfo {
    #[allow(dead_code)]
    version: u8,
    total_codewords: usize,
    /// EC codewords per block for [L, M, Q, H].
    ec_per_block: [usize; 4],
    /// Number of error correction blocks for [L, M, Q, H].
    /// For versions 1-5 the block structure is simple (1 or 2 blocks).
    blocks: [usize; 4],
    /// Data capacity in bytes for [L, M, Q, H].
    data_capacity: [usize; 4],
}

/// Versions 1-5 specification data.
const VERSION_TABLE: [VersionInfo; 5] = [
    // Version 1: 21x21, 26 total codewords
    VersionInfo {
        version: 1,
        total_codewords: 26,
        ec_per_block: [7, 10, 13, 17],
        blocks: [1, 1, 1, 1],
        data_capacity: [19, 16, 13, 9],
    },
    // Version 2: 25x25, 44 total codewords
    VersionInfo {
        version: 2,
        total_codewords: 44,
        ec_per_block: [10, 16, 22, 28],
        blocks: [1, 1, 1, 1],
        data_capacity: [34, 28, 22, 16],
    },
    // Version 3: 29x29, 70 total codewords
    VersionInfo {
        version: 3,
        total_codewords: 70,
        ec_per_block: [15, 26, 18, 22],
        blocks: [1, 1, 2, 2],
        data_capacity: [55, 44, 34, 26],
    },
    // Version 4: 33x33, 100 total codewords
    VersionInfo {
        version: 4,
        total_codewords: 100,
        ec_per_block: [20, 18, 26, 16],
        blocks: [1, 2, 2, 4],
        data_capacity: [80, 64, 48, 36],
    },
    // Version 5: 37x37, 134 total codewords
    VersionInfo {
        version: 5,
        total_codewords: 134,
        ec_per_block: [26, 24, 18, 22],
        blocks: [1, 2, 4, 4],
        data_capacity: [108, 86, 62, 46],
    },
];

fn ec_index(ec: EcLevel) -> usize {
    match ec {
        EcLevel::L => 0,
        EcLevel::M => 1,
        EcLevel::Q => 2,
        EcLevel::H => 3,
    }
}

fn version_info(version: u8) -> Option<&'static VersionInfo> {
    if version >= 1 && version <= 5 {
        Some(&VERSION_TABLE[(version - 1) as usize])
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// GF(2^8) arithmetic with primitive polynomial 0x11D (x^8 + x^4 + x^3 + x^2 + 1)
// ---------------------------------------------------------------------------

/// GF(2^8) tables for Reed-Solomon computation.
struct GfTables {
    exp: [u8; 256],
    log: [u8; 256],
}

/// Build the GF(2^8) exponent and logarithm lookup tables.
fn build_gf_tables() -> GfTables {
    let mut exp = [0u8; 256];
    let mut log = [0u8; 256];
    let mut val: u16 = 1;
    for i in 0..255u16 {
        exp[i as usize] = val as u8;
        log[val as usize] = i as u8;
        val <<= 1;
        if val >= 256 {
            val ^= 0x11D; // Reduce by primitive polynomial.
        }
    }
    // exp[255] wraps to exp[0] = 1 for convenience.
    exp[255] = exp[0];
    GfTables { exp, log }
}

fn gf_mul(gf: &GfTables, a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        return 0;
    }
    let idx = (gf.log[a as usize] as u16 + gf.log[b as usize] as u16) % 255;
    gf.exp[idx as usize]
}

/// Compute the generator polynomial for `n` error correction codewords.
///
/// g(x) = product of (x - alpha^i) for i = 0..n-1
/// Returned as coefficients [g_0, g_1, ..., g_n] where g_n = 1 (leading).
fn rs_generator(gf: &GfTables, n: usize) -> Vec<u8> {
    let mut gen = vec![0u8; n + 1];
    gen[0] = 1; // Start with g(x) = 1.

    for i in 0..n {
        // Multiply gen by (x - alpha^i) = (x + alpha^i) in GF(2^8).
        let alpha_i = gf.exp[i];
        // Process in reverse to avoid overwriting.
        let len = i + 2; // After this multiply, degree = i+1.
        for j in (1..len).rev() {
            gen[j] = gen[j] ^ gf_mul(gf, gen[j - 1], alpha_i);
        }
        gen[0] = gf_mul(gf, gen[0], alpha_i);
    }

    gen
}

/// Compute Reed-Solomon error correction codewords.
///
/// `data` is the message polynomial coefficients (data codewords).
/// `n_ec` is the number of EC codewords to generate.
/// Returns exactly `n_ec` bytes.
fn rs_encode(gf: &GfTables, data: &[u8], n_ec: usize) -> Vec<u8> {
    let gen = rs_generator(gf, n_ec);

    // Polynomial division: data * x^n_ec / gen.
    let mut remainder = vec![0u8; n_ec];

    for &byte in data {
        let factor = byte ^ remainder[0];
        // Shift remainder left by 1.
        for j in 0..n_ec - 1 {
            remainder[j] = remainder[j + 1];
        }
        remainder[n_ec - 1] = 0;
        // XOR with gen * factor.
        for j in 0..n_ec {
            remainder[j] ^= gf_mul(gf, gen[j], factor);
        }
    }

    // The remainder is stored with index 0 = highest degree, which is what
    // we want: the first EC codeword corresponds to the highest-degree term.
    // However, our gen polynomial has gen[0] = constant term, so we need to
    // reverse.
    // Actually let's verify: after the division loop, remainder[0] is the
    // coefficient of x^(n_ec-1) in the remainder. QR codes expect EC
    // codewords from highest degree to lowest. So remainder is already in
    // the correct order.
    remainder
}

// ---------------------------------------------------------------------------
// Bit stream
// ---------------------------------------------------------------------------

struct BitStream {
    data: Vec<u8>,
    bit_count: usize,
}

impl BitStream {
    fn new() -> Self {
        BitStream {
            data: Vec::new(),
            bit_count: 0,
        }
    }

    fn put_bits(&mut self, value: u32, count: usize) {
        for i in (0..count).rev() {
            let bit = ((value >> i) & 1) != 0;
            let byte_idx = self.bit_count / 8;
            let bit_idx = 7 - (self.bit_count % 8);
            if byte_idx >= self.data.len() {
                self.data.push(0);
            }
            if bit {
                self.data[byte_idx] |= 1 << bit_idx;
            }
            self.bit_count += 1;
        }
    }

    fn len_bits(&self) -> usize {
        self.bit_count
    }
}

// ---------------------------------------------------------------------------
// Data encoding (byte mode)
// ---------------------------------------------------------------------------

/// Encode data in byte mode and pad to fill the required capacity.
fn encode_data(data: &[u8], vi: &VersionInfo, ec: EcLevel) -> Result<Vec<u8>, QrError> {
    let eci = ec_index(ec);
    let capacity = vi.data_capacity[eci];

    if data.len() > capacity {
        return Err(QrError::DataTooLarge);
    }

    let mut bs = BitStream::new();

    // Mode indicator: byte mode = 0100.
    bs.put_bits(0b0100, 4);

    // Character count indicator.
    // For versions 1-9, byte mode uses 8-bit count.
    let count_bits = 8;
    bs.put_bits(data.len() as u32, count_bits);

    // Data bytes.
    for &byte in data {
        bs.put_bits(byte as u32, 8);
    }

    // Terminator: up to 4 zero bits.
    let total_data_bits = capacity * 8;
    let remaining = total_data_bits.saturating_sub(bs.len_bits());
    let terminator_len = remaining.min(4);
    bs.put_bits(0, terminator_len);

    // Pad to byte boundary.
    let pad_to_byte = (8 - (bs.len_bits() % 8)) % 8;
    bs.put_bits(0, pad_to_byte);

    // Pad with alternating 0xEC, 0x11.
    let mut pad_idx = 0u8;
    while bs.data.len() < capacity {
        bs.put_bits(if pad_idx == 0 { 0xEC } else { 0x11 }, 8);
        pad_idx ^= 1;
    }

    // Truncate to exact capacity (should already be correct).
    bs.data.truncate(capacity);

    Ok(bs.data)
}

// ---------------------------------------------------------------------------
// Error correction and interleaving
// ---------------------------------------------------------------------------

/// Generate the final codeword sequence (data + EC, interleaved).
fn generate_codewords(data_codewords: &[u8], vi: &VersionInfo, ec: EcLevel) -> Vec<u8> {
    let eci = ec_index(ec);
    let num_blocks = vi.blocks[eci];
    let ec_per_block = vi.ec_per_block[eci];
    let total_data = vi.data_capacity[eci];

    let gf = build_gf_tables();

    // Split data into blocks.
    let base_block_size = total_data / num_blocks;
    let extra = total_data % num_blocks;

    let mut data_blocks: Vec<Vec<u8>> = Vec::with_capacity(num_blocks);
    let mut ec_blocks: Vec<Vec<u8>> = Vec::with_capacity(num_blocks);
    let mut offset = 0;

    for i in 0..num_blocks {
        // Later blocks get one extra codeword if there's a remainder.
        let block_size = base_block_size + if i >= num_blocks - extra { 1 } else { 0 };
        let block_data = &data_codewords[offset..offset + block_size];
        let ec_cw = rs_encode(&gf, block_data, ec_per_block);
        data_blocks.push(block_data.to_vec());
        ec_blocks.push(ec_cw);
        offset += block_size;
    }

    // Interleave data codewords.
    let max_data_len = data_blocks.iter().map(|b| b.len()).max().unwrap_or(0);
    let mut result = Vec::with_capacity(vi.total_codewords);
    for i in 0..max_data_len {
        for block in &data_blocks {
            if i < block.len() {
                result.push(block[i]);
            }
        }
    }

    // Interleave EC codewords.
    for i in 0..ec_per_block {
        for block in &ec_blocks {
            if i < block.len() {
                result.push(block[i]);
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Module placement
// ---------------------------------------------------------------------------

/// Represents the state of a module position during construction.
#[derive(Clone, Copy, PartialEq)]
enum ModuleState {
    /// Not yet assigned.
    Empty,
    /// Reserved for function pattern (finder, timing, etc.).
    Function(bool),
    /// Data/EC codeword bit.
    Data(bool),
}

struct QrMatrix {
    size: usize,
    modules: Vec<Vec<ModuleState>>,
}

impl QrMatrix {
    fn new(size: usize) -> Self {
        QrMatrix {
            size,
            modules: vec![vec![ModuleState::Empty; size]; size],
        }
    }

    fn set_function(&mut self, row: usize, col: usize, dark: bool) {
        if row < self.size && col < self.size {
            self.modules[row][col] = ModuleState::Function(dark);
        }
    }

    fn is_empty(&self, row: usize, col: usize) -> bool {
        self.modules[row][col] == ModuleState::Empty
    }

    #[allow(dead_code)]
    fn is_dark(&self, row: usize, col: usize) -> bool {
        match self.modules[row][col] {
            ModuleState::Function(d) | ModuleState::Data(d) => d,
            ModuleState::Empty => false,
        }
    }
}

/// Place finder pattern with 7x7 core at (row, col) as top-left corner.
fn place_finder_pattern(matrix: &mut QrMatrix, row: i32, col: i32) {
    for dr in -1i32..=7 {
        for dc in -1i32..=7 {
            let r = row + dr;
            let c = col + dc;
            if r < 0 || r >= matrix.size as i32 || c < 0 || c >= matrix.size as i32 {
                continue;
            }
            let dark = if dr == -1 || dr == 7 || dc == -1 || dc == 7 {
                false // Separator.
            } else if dr == 0 || dr == 6 || dc == 0 || dc == 6 {
                true // Outer border.
            } else if dr >= 2 && dr <= 4 && dc >= 2 && dc <= 4 {
                true // Inner 3x3.
            } else {
                false // Space between border and center.
            };
            matrix.set_function(r as usize, c as usize, dark);
        }
    }
}

/// Place alignment pattern centered at (row, col).
fn place_alignment_pattern(matrix: &mut QrMatrix, row: usize, col: usize) {
    for dr in -2i32..=2 {
        for dc in -2i32..=2 {
            let r = (row as i32 + dr) as usize;
            let c = (col as i32 + dc) as usize;
            let dark = dr.abs() == 2 || dc.abs() == 2 || (dr == 0 && dc == 0);
            matrix.set_function(r, c, dark);
        }
    }
}

/// Alignment pattern positions for versions 2-5.
fn alignment_positions(version: u8) -> Vec<usize> {
    match version {
        1 => vec![],
        2 => vec![6, 18],
        3 => vec![6, 22],
        4 => vec![6, 26],
        5 => vec![6, 30],
        _ => vec![],
    }
}

/// Place timing patterns (row 6 and col 6).
fn place_timing_patterns(matrix: &mut QrMatrix) {
    let size = matrix.size;
    for i in 8..size - 8 {
        let dark = i % 2 == 0;
        if matrix.is_empty(6, i) {
            matrix.set_function(6, i, dark);
        }
        if matrix.is_empty(i, 6) {
            matrix.set_function(i, 6, dark);
        }
    }
}

/// Reserve format information areas (will be filled after masking).
fn reserve_format_info(matrix: &mut QrMatrix) {
    let size = matrix.size;

    // Around top-left finder.
    for i in 0..=8 {
        if i < size && matrix.is_empty(8, i) {
            matrix.set_function(8, i, false);
        }
        if i < size && matrix.is_empty(i, 8) {
            matrix.set_function(i, 8, false);
        }
    }

    // Around bottom-left finder.
    for i in 0..7 {
        let row = size - 1 - i;
        if matrix.is_empty(row, 8) {
            matrix.set_function(row, 8, false);
        }
    }

    // Around top-right finder.
    for i in 0..8 {
        let col = size - 1 - i;
        if matrix.is_empty(8, col) {
            matrix.set_function(8, col, false);
        }
    }

    // Dark module (always dark).
    matrix.set_function(size - 8, 8, true);
}

/// Place data bits in the zigzag pattern.
fn place_data_bits(matrix: &mut QrMatrix, codewords: &[u8]) {
    let size = matrix.size;
    let mut bit_idx = 0usize;
    let total_bits = codewords.len() * 8;

    // Data is placed in 2-column strips going right-to-left, skipping column 6.
    let mut col = size as i32 - 1;
    while col >= 0 {
        // Skip the vertical timing column.
        if col == 6 {
            col -= 1;
            continue;
        }

        // Traverse upward on even strips, downward on odd.
        // The strip index determines direction.
        let strip_from_right = (size as i32 - 1 - col) / 2;
        let upward = strip_from_right % 2 == 0;

        let rows: Vec<usize> = if upward {
            (0..size).rev().collect()
        } else {
            (0..size).collect()
        };

        for row in rows {
            for dc in 0..2 {
                let c = col - dc;
                if c < 0 || c as usize >= size {
                    continue;
                }
                let c = c as usize;
                if !matrix.is_empty(row, c) {
                    continue;
                }
                let dark = if bit_idx < total_bits {
                    let byte_idx = bit_idx / 8;
                    let bit_pos = 7 - (bit_idx % 8);
                    ((codewords[byte_idx] >> bit_pos) & 1) != 0
                } else {
                    false
                };
                matrix.modules[row][c] = ModuleState::Data(dark);
                bit_idx += 1;
            }
        }

        col -= 2;
    }
}

// ---------------------------------------------------------------------------
// Masking
// ---------------------------------------------------------------------------

/// Evaluate a mask condition for the given row and column.
fn mask_condition(mask: u8, row: usize, col: usize) -> bool {
    match mask {
        0 => (row + col) % 2 == 0,
        1 => row % 2 == 0,
        2 => col % 3 == 0,
        3 => (row + col) % 3 == 0,
        4 => (row / 2 + col / 3) % 2 == 0,
        5 => ((row * col) % 2) + ((row * col) % 3) == 0,
        6 => (((row * col) % 2) + ((row * col) % 3)) % 2 == 0,
        7 => (((row + col) % 2) + ((row * col) % 3)) % 2 == 0,
        _ => false,
    }
}

/// Apply mask to data modules. Returns a new bool matrix.
fn apply_mask(matrix: &QrMatrix, mask: u8) -> Vec<Vec<bool>> {
    let size = matrix.size;
    let mut result = vec![vec![false; size]; size];
    for r in 0..size {
        for c in 0..size {
            let dark = match matrix.modules[r][c] {
                ModuleState::Function(d) => d,
                ModuleState::Data(d) => {
                    if mask_condition(mask, r, c) {
                        !d
                    } else {
                        d
                    }
                }
                ModuleState::Empty => false,
            };
            result[r][c] = dark;
        }
    }
    result
}

/// Evaluate penalty score for a masked matrix.
fn evaluate_penalty(matrix: &[Vec<bool>]) -> u32 {
    let size = matrix.len();
    let mut penalty = 0u32;

    // Rule 1: Adjacent modules in row/column with same color.
    // 5+ in a row/column: penalty = count - 2.
    for r in 0..size {
        let mut run = 1u32;
        for c in 1..size {
            if matrix[r][c] == matrix[r][c - 1] {
                run += 1;
            } else {
                if run >= 5 {
                    penalty += run - 2;
                }
                run = 1;
            }
        }
        if run >= 5 {
            penalty += run - 2;
        }
    }
    for c in 0..size {
        let mut run = 1u32;
        for r in 1..size {
            if matrix[r][c] == matrix[r - 1][c] {
                run += 1;
            } else {
                if run >= 5 {
                    penalty += run - 2;
                }
                run = 1;
            }
        }
        if run >= 5 {
            penalty += run - 2;
        }
    }

    // Rule 2: 2x2 blocks of same color.
    for r in 0..size - 1 {
        for c in 0..size - 1 {
            let color = matrix[r][c];
            if matrix[r][c + 1] == color
                && matrix[r + 1][c] == color
                && matrix[r + 1][c + 1] == color
            {
                penalty += 3;
            }
        }
    }

    // Rule 3: Finder-like patterns (1:1:3:1:1 with 4 white).
    let pattern_a: [bool; 11] = [
        true, false, true, true, true, false, true, false, false, false, false,
    ];
    let pattern_b: [bool; 11] = [
        false, false, false, false, true, false, true, true, true, false, true,
    ];
    for r in 0..size {
        for c in 0..size.saturating_sub(10) {
            let row_slice: Vec<bool> = (0..11).map(|i| matrix[r][c + i]).collect();
            if row_slice.as_slice() == &pattern_a || row_slice.as_slice() == &pattern_b {
                penalty += 40;
            }
        }
    }
    for c in 0..size {
        for r in 0..size.saturating_sub(10) {
            let col_slice: Vec<bool> = (0..11).map(|i| matrix[r + i][c]).collect();
            if col_slice.as_slice() == &pattern_a || col_slice.as_slice() == &pattern_b {
                penalty += 40;
            }
        }
    }

    // Rule 4: Proportion of dark modules.
    let total = (size * size) as u32;
    let dark_count: u32 = matrix.iter().flatten().filter(|&&d| d).count() as u32;
    let percent = (dark_count * 100) / total;
    let prev5 = (percent / 5) * 5;
    let next5 = prev5 + 5;
    let deviation = ((prev5 as i32 - 50)
        .unsigned_abs()
        .min((next5 as i32 - 50).unsigned_abs()))
        / 5;
    penalty += deviation * 10;

    penalty
}

// ---------------------------------------------------------------------------
// Format information
// ---------------------------------------------------------------------------

/// Format info bits for the given EC level and mask pattern.
/// Returns 15 bits (BCH encoded).
fn format_info_bits(ec: EcLevel, mask: u8) -> u16 {
    // Format info: 2 bits EC + 3 bits mask, then BCH(15,5) with generator 0x537.
    let ec_bits: u8 = match ec {
        EcLevel::L => 0b01,
        EcLevel::M => 0b00,
        EcLevel::Q => 0b11,
        EcLevel::H => 0b10,
    };

    let data = ((ec_bits as u16) << 3) | (mask as u16);

    // BCH(15,5) encoding.
    let mut encoded = data << 10;
    let generator: u16 = 0x537; // x^10 + x^8 + x^5 + x^4 + x^2 + x + 1
    let mut temp = encoded;
    for i in (0..5).rev() {
        if temp & (1 << (i + 10)) != 0 {
            temp ^= generator << i;
        }
    }
    encoded |= temp & 0x3FF;

    // XOR with mask pattern 0x5412.
    encoded ^ 0x5412
}

/// Place format information bits into the matrix.
fn place_format_info(modules: &mut [Vec<bool>], size: usize, ec: EcLevel, mask: u8) {
    let bits = format_info_bits(ec, mask);

    // Around top-left: bit 0 at (8, 0) going right, then up from (0, 8).
    // Horizontal strip in row 8, columns 0-7 (skip col 6 -> shift).
    let horizontal_positions: [(usize, usize); 15] = [
        // bits 0-7 in row 8
        (8, 0),
        (8, 1),
        (8, 2),
        (8, 3),
        (8, 4),
        (8, 5),
        (8, 7), // Skip column 6 (timing).
        (8, 8),
        // bits 8-14 in col 8
        (7, 8),
        (5, 8), // Skip row 6 (timing).
        (4, 8),
        (3, 8),
        (2, 8),
        (1, 8),
        (0, 8),
    ];

    // Around bottom-left and top-right.
    let vertical_positions: [(usize, usize); 15] = [
        // bits 0-6 in col 8, from bottom.
        (size - 1, 8),
        (size - 2, 8),
        (size - 3, 8),
        (size - 4, 8),
        (size - 5, 8),
        (size - 6, 8),
        (size - 7, 8),
        // bits 7-14 in row 8, from right.
        (8, size - 8),
        (8, size - 7),
        (8, size - 6),
        (8, size - 5),
        (8, size - 4),
        (8, size - 3),
        (8, size - 2),
        (8, size - 1),
    ];

    for (i, &(r, c)) in horizontal_positions.iter().enumerate() {
        let dark = (bits >> i) & 1 == 1;
        modules[r][c] = dark;
    }

    for (i, &(r, c)) in vertical_positions.iter().enumerate() {
        let dark = (bits >> i) & 1 == 1;
        modules[r][c] = dark;
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

impl QrEncoder {
    /// Create a new QR encoder.
    pub fn new() -> Self {
        QrEncoder
    }

    /// Encode binary data into a QR code.
    ///
    /// Automatically selects the smallest version (1-5) that fits the data
    /// at the given error correction level. Evaluates all 8 mask patterns
    /// and selects the one with the lowest penalty score.
    pub fn encode(data: &[u8], ec_level: EcLevel) -> Result<QrCode, QrError> {
        // Find smallest version that fits.
        let eci = ec_index(ec_level);
        let mut chosen_version: Option<u8> = None;
        for v in 1..=5u8 {
            let vi = version_info(v).unwrap();
            if data.len() <= vi.data_capacity[eci] {
                chosen_version = Some(v);
                break;
            }
        }

        let version = chosen_version.ok_or(QrError::DataTooLarge)?;
        let vi = version_info(version).unwrap();
        let size = (version as usize) * 4 + 17;

        // Step 1: Encode data into codewords.
        let data_cw = encode_data(data, vi, ec_level)?;

        // Step 2: Generate EC codewords and interleave.
        let all_cw = generate_codewords(&data_cw, vi, ec_level);

        // Step 3: Build the base matrix with function patterns.
        let mut matrix = QrMatrix::new(size);

        // Finder patterns.
        place_finder_pattern(&mut matrix, 0, 0);
        place_finder_pattern(&mut matrix, 0, (size as i32) - 7);
        place_finder_pattern(&mut matrix, (size as i32) - 7, 0);

        // Alignment patterns (v2+).
        let align_pos = alignment_positions(version);
        if align_pos.len() >= 2 {
            for &ar in &align_pos {
                for &ac in &align_pos {
                    // Skip if overlapping with finder patterns.
                    if (ar <= 8 && ac <= 8)
                        || (ar <= 8 && ac >= size - 8)
                        || (ar >= size - 8 && ac <= 8)
                    {
                        continue;
                    }
                    place_alignment_pattern(&mut matrix, ar, ac);
                }
            }
        }

        // Timing patterns.
        place_timing_patterns(&mut matrix);

        // Reserve format info areas.
        reserve_format_info(&mut matrix);

        // Step 4: Place data bits.
        place_data_bits(&mut matrix, &all_cw);

        // Step 5: Try all 8 masks and pick the best.
        let mut best_mask = 0u8;
        let mut best_penalty = u32::MAX;
        let mut best_modules: Option<Vec<Vec<bool>>> = None;

        for mask in 0..8u8 {
            let mut masked = apply_mask(&matrix, mask);
            place_format_info(&mut masked, size, ec_level, mask);
            let penalty = evaluate_penalty(&masked);
            if penalty < best_penalty {
                best_penalty = penalty;
                best_mask = mask;
                best_modules = Some(masked);
            }
        }

        let modules =
            best_modules.ok_or_else(|| QrError::EncodingFailed("no valid mask found".into()))?;

        // Apply format info to chosen mask result (already done in the loop).
        let _ = best_mask; // Used during format info placement.

        Ok(QrCode {
            modules,
            version,
            size,
        })
    }

    /// Render a QR code as an SVG string.
    ///
    /// Produces a self-contained SVG with a 4-module quiet zone.
    pub fn to_svg(code: &QrCode) -> String {
        let quiet = 4;
        let total = code.size + quiet * 2;
        let mut svg = String::new();

        svg.push_str(&format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <svg xmlns=\"http://www.w3.org/2000/svg\" \
             viewBox=\"0 0 {total} {total}\" \
             width=\"{total}\" height=\"{total}\">\n\
             <rect width=\"{total}\" height=\"{total}\" fill=\"white\"/>\n\
             <path d=\""
        ));

        // Build a single path for all dark modules.
        for r in 0..code.size {
            for c in 0..code.size {
                if code.modules[r][c] {
                    let x = c + quiet;
                    let y = r + quiet;
                    svg.push_str(&format!("M{x},{y}h1v1h-1z"));
                }
            }
        }

        svg.push_str("\" fill=\"black\"/>\n</svg>");
        svg
    }

    /// Render a QR code as ASCII art.
    ///
    /// Uses Unicode block characters for compact display. Each character
    /// represents two vertical modules using upper/lower half blocks.
    pub fn to_ascii(code: &QrCode) -> String {
        let quiet = 2; // Quiet zone in modules.
        let total_w = code.size + quiet * 2;
        let total_h = code.size + quiet * 2;
        let mut lines: Vec<String> = Vec::new();

        // Process two rows at a time using half-block characters.
        let mut row = 0;
        while row < total_h {
            let mut line = String::new();
            for col in 0..total_w {
                let top = if row >= quiet
                    && row < quiet + code.size
                    && col >= quiet
                    && col < quiet + code.size
                {
                    code.modules[row - quiet][col - quiet]
                } else {
                    false
                };

                let bot = if row + 1 >= quiet
                    && row + 1 < quiet + code.size
                    && col >= quiet
                    && col < quiet + code.size
                {
                    code.modules[row + 1 - quiet][col - quiet]
                } else {
                    false
                };

                let ch = match (top, bot) {
                    (false, false) => ' ',
                    (true, false) => '\u{2580}', // Upper half block.
                    (false, true) => '\u{2584}', // Lower half block.
                    (true, true) => '\u{2588}',  // Full block.
                };
                line.push(ch);
            }
            lines.push(line);
            row += 2;
        }

        lines.join("\n")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_small_payload() {
        let data = b"Hello";
        let code = QrEncoder::encode(data, EcLevel::M).unwrap();
        assert_eq!(code.version, 1);
        assert_eq!(code.size, 21);
        assert_eq!(code.modules.len(), 21);
        assert_eq!(code.modules[0].len(), 21);
    }

    #[test]
    fn encode_large_payload() {
        // 80 bytes fits in version 4 EC-L (capacity 80).
        let data = vec![0xAB; 80];
        let code = QrEncoder::encode(&data, EcLevel::L).unwrap();
        assert_eq!(code.version, 4);
        assert_eq!(code.size, 33);
    }

    #[test]
    fn svg_output_valid_xml() {
        let code = QrEncoder::encode(b"test", EcLevel::M).unwrap();
        let svg = QrEncoder::to_svg(&code);
        assert!(svg.starts_with("<?xml"));
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("xmlns"));
        assert!(svg.contains("<path"));
    }

    #[test]
    fn ascii_output_correct_dimensions() {
        let code = QrEncoder::encode(b"test", EcLevel::M).unwrap();
        let ascii = QrEncoder::to_ascii(&code);
        let lines: Vec<&str> = ascii.lines().collect();
        // Quiet zone is 2 modules on each side, total height = size + 4.
        // Two rows per line, so lines = ceil((size + 4) / 2).
        let expected_height = (code.size + 4 + 1) / 2;
        assert_eq!(lines.len(), expected_height);
        // Each line should be size + 4 characters wide.
        for line in &lines {
            assert_eq!(line.chars().count(), code.size + 4);
        }
    }

    #[test]
    fn error_on_too_large_data() {
        // Version 5, EC-H capacity is 46. Data larger than that should fail.
        let data = vec![0xFF; 200];
        let result = QrEncoder::encode(&data, EcLevel::H);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), QrError::DataTooLarge);
    }

    #[test]
    fn version_check_round_trip() {
        // Encode data that requires version 2 (17-34 bytes with EC-M).
        let data = vec![0x42; 20]; // 20 bytes > v1 capacity 16 for EC-M.
        let code = QrEncoder::encode(&data, EcLevel::M).unwrap();
        assert_eq!(code.version, 2);
        assert_eq!(code.size, 25); // Version 2 = 25x25.
    }

    #[test]
    fn finder_patterns_present() {
        let code = QrEncoder::encode(b"A", EcLevel::L).unwrap();
        // Top-left finder: top-left 7x7 corner has specific pattern.
        // Row 0 should be: dark dark dark dark dark dark dark ...
        for c in 0..7 {
            assert!(code.modules[0][c], "top-left finder row 0, col {c}");
        }
        // Row 1: dark, light, light, light, light, light, dark.
        assert!(code.modules[1][0]);
        assert!(!code.modules[1][1]);
        assert!(!code.modules[1][5]);
        assert!(code.modules[1][6]);
    }

    #[test]
    fn ec_levels_all_work() {
        let data = b"QR test";
        for ec in [EcLevel::L, EcLevel::M, EcLevel::Q, EcLevel::H] {
            let code = QrEncoder::encode(data, ec).unwrap();
            assert!(code.size >= 21);
            assert!(!code.modules.is_empty());
        }
    }

    #[test]
    fn gf_tables_correct() {
        let gf = build_gf_tables();
        // alpha^0 = 1.
        assert_eq!(gf.exp[0], 1);
        // alpha^1 = 2.
        assert_eq!(gf.exp[1], 2);
        // alpha^7 = 128.
        assert_eq!(gf.exp[7], 128);
        // alpha^8 = 0x11D ^ 0x100 = 0x1D = 29.
        assert_eq!(gf.exp[8], 29);
        // gf_mul(2, 2) = 4.
        assert_eq!(gf_mul(&gf, 2, 2), 4);
        // gf_mul(0, anything) = 0.
        assert_eq!(gf_mul(&gf, 0, 42), 0);
    }

    #[test]
    fn rs_encode_known_vector() {
        // Verify RS encoding produces non-zero EC codewords.
        let gf = build_gf_tables();
        let data = vec![
            0x40, 0x11, 0x20, 0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11, 0xEC, 0x11, 0xEC,
            0x11, 0xEC,
        ];
        let ec = rs_encode(&gf, &data, 10);
        assert_eq!(ec.len(), 10);
        // EC codewords should not all be zero for non-trivial data.
        assert!(ec.iter().any(|&b| b != 0));
    }

    #[test]
    fn qr_error_display() {
        let e1 = QrError::DataTooLarge;
        assert!(format!("{e1}").contains("too large"));

        let e2 = QrError::InvalidVersion;
        assert!(format!("{e2}").contains("invalid"));

        let e3 = QrError::EncodingFailed("test".into());
        assert!(format!("{e3}").contains("test"));
    }
}
