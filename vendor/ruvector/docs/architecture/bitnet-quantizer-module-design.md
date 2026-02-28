# PT-BitNet Quantizer Module Architecture Design

**Version:** 1.0
**Date:** 2026-02-03
**Status:** Design Specification
**Relates to:** ADR-017 (AD-1, AD-5, AD-18, AD-19), DDD Section 3.4/4.2/4.3

---

## Executive Summary

This document specifies the architecture for the **PT-BitNet post-training quantizer** module that converts FP16/BF16 GLM-4.7-Flash weights to BitNet b1.58 ternary {-1, 0, +1} format via absmean quantization. This is a **design-only specification** — implementation follows in Phase 0.

**Design Scope:**
- Module layout and file organization
- Complete struct definitions with field types
- Full function signatures (no implementations)
- GGUF integration points and format extensions
- Error handling strategy
- Testing approach

**Out of Scope:**
- Actual implementation code
- Performance benchmarks
- Calibration dataset selection

---

## A. Module Layout

### Directory Structure

```
crates/ruvllm/src/
├── bitnet/                          # NEW module
│   ├── mod.rs                       # Module exports and public API
│   ├── quantizer.rs                 # PtBitnetQuantizer + absmean algorithm
│   ├── ternary_tensor.rs            # TernaryTensor value object
│   ├── dequantize.rs                # BITNET_T158 dequantization kernel
│   └── config.rs                    # PtBitnetConfig configuration
│
├── gguf/
│   ├── mod.rs                       # Add pub mod bitnet export
│   ├── quantization.rs              # MODIFIED: Add BITNET_T158 enum variant
│   ├── parser.rs                    # Unchanged (reused as-is)
│   └── ...
│
└── kernels/
    └── matmul.rs                    # Reference for dispatch patterns
```

### Modified Files

#### `src/gguf/quantization.rs`

**Changes:**
1. Add `BITNET_T158 = 30` variant to `GgufQuantType` enum (after `Bf16 = 29`)
2. Update `try_from()` impl to handle type 30
3. Update `block_size()` to return 256 for `BITNET_T158`
4. Update `type_size()` to return 66 for `BITNET_T158` (64 bytes packed + 2 bytes FP16 scale)
5. Update `is_quantized()` to include `BITNET_T158`
6. Update `bits_per_weight()` to return 2.06 for `BITNET_T158`
7. Add new match arm in `dequantize_tensor()` → `BITNET_T158 => dequantize_bitnet_t158(data, output)`

**Exact enum addition:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum GgufQuantType {
    // ... existing variants 0-29 ...
    /// BitNet b1.58 ternary quantization (2-bit packed + FP16 scale per 256-element block)
    BITNET_T158 = 30,
}
```

---

## B. Struct Definitions

### 1. `PtBitnetConfig` (in `bitnet/config.rs`)

**Purpose:** Configuration for PT-BitNet quantization process

```rust
/// Configuration for PT-BitNet post-training quantization
#[derive(Debug, Clone)]
pub struct PtBitnetConfig {
    /// Block size for absmean scale computation (default: 256)
    pub block_size: usize,

    /// Epsilon for numerical stability in scale computation (default: 1e-8)
    pub epsilon: f32,

    /// Whether to run calibration pass to optimize scale factors
    pub use_calibration: bool,

    /// Number of calibration samples (if use_calibration = true)
    pub calibration_samples: usize,

    /// Maximum sequence length for calibration (default: 2048)
    pub calibration_max_seq_len: usize,

    /// Device for calibration pass ("cpu", "metal", "cuda:0")
    pub calibration_device: String,

    /// Clipping threshold for normalized weights before rounding
    /// (default: 1.0, range typically 0.95-1.05)
    pub clip_threshold: f32,

    /// Sparsity target: if > 0.0, bias rounding toward zero to achieve target sparsity
    pub target_sparsity: Option<f32>,
}

impl Default for PtBitnetConfig {
    fn default() -> Self {
        Self {
            block_size: 256,
            epsilon: 1e-8,
            use_calibration: false,
            calibration_samples: 1000,
            calibration_max_seq_len: 2048,
            calibration_device: "metal".to_string(),
            clip_threshold: 1.0,
            target_sparsity: None,
        }
    }
}
```

### 2. `TernaryTensor` (in `bitnet/ternary_tensor.rs`)

**Purpose:** Immutable value object for packed ternary weights

```rust
/// Packed ternary tensor with per-block FP16 scales
#[derive(Debug, Clone)]
pub struct TernaryTensor {
    /// Packed 2-bit ternary values (4 weights per byte)
    /// Encoding: 00 = -1, 01 = 0, 10 = +1, 11 = reserved
    pub packed_data: Vec<u8>,

    /// Per-block FP16 scale factors (absmean values)
    pub scales: Vec<f16>,

    /// Tensor shape [out_features, in_features] or [rows, cols]
    pub shape: [usize; 2],

    /// Block size (always 256 for BitNet b1.58)
    pub block_size: usize,

    /// Total number of weights
    pub num_elements: usize,

    /// Number of blocks
    pub num_blocks: usize,

    /// Measured sparsity (fraction of zero weights)
    pub sparsity: f32,
}

impl TernaryTensor {
    /// Calculate total storage size in bytes
    pub fn storage_size(&self) -> usize;

    /// Get expected packed_data size for validation
    pub fn expected_packed_size(&self) -> usize;

    /// Validate internal consistency
    pub fn validate(&self) -> Result<()>;
}
```

### 3. `TernaryBlock` (in `bitnet/ternary_tensor.rs`)

**Purpose:** Single block of 256 ternary weights with scale

```rust
/// A single 256-element block with ternary weights and FP16 scale
#[derive(Debug, Clone)]
pub struct TernaryBlock {
    /// 64 bytes of packed 2-bit values (256 weights × 2 bits ÷ 8 bits/byte)
    pub packed: [u8; 64],

    /// FP16 absmean scale factor
    pub scale: f16,
}

impl TernaryBlock {
    /// Size in bytes when stored in GGUF (64 + 2 = 66)
    pub const STORAGE_SIZE: usize = 66;

    /// Number of elements in a block
    pub const BLOCK_SIZE: usize = 256;
}
```

### 4. `AbsmeanResult` (in `bitnet/quantizer.rs`)

**Purpose:** Result of absmean quantization on a single block

```rust
/// Result of absmean ternary quantization on a block
#[derive(Debug, Clone)]
pub struct AbsmeanResult {
    /// Ternary values {-1, 0, +1} for each weight in the block
    pub ternary_weights: Vec<i8>,

    /// Computed absmean scale factor (gamma = mean(|W|))
    pub scale: f32,

    /// Measured sparsity (fraction of zeros)
    pub sparsity: f32,

    /// Mean squared error vs original FP16 values (for calibration)
    pub mse: f32,
}
```

### 5. `QuantizationStats` (in `bitnet/quantizer.rs`)

**Purpose:** Statistics collected during quantization

```rust
/// Statistics from quantizing a single tensor
#[derive(Debug, Clone)]
pub struct QuantizationStats {
    /// Tensor name
    pub name: String,

    /// Mean of all block scales
    pub mean_scale: f32,

    /// Std dev of block scales
    pub std_scale: f32,

    /// Overall sparsity across all blocks
    pub sparsity: f32,

    /// Mean MSE across all blocks
    pub mean_mse: f32,

    /// Number of blocks
    pub num_blocks: usize,
}
```

---

## C. Function Signatures

### Core Quantization Functions (in `bitnet/quantizer.rs`)

#### 1. Primary Quantization Entry Point

```rust
/// Quantize an FP16/F32 tensor to ternary format using absmean quantization
///
/// # Arguments
/// * `tensor` - Input FP16 or F32 tensor data (flat vector)
/// * `shape` - Tensor shape [out_features, in_features]
/// * `config` - Quantization configuration
///
/// # Returns
/// * `TernaryTensor` - Packed ternary representation
/// * `QuantizationStats` - Statistics about the quantization process
///
/// # Errors
/// * `RuvLLMError::Quantization` if tensor size is not divisible by block_size
/// * `RuvLLMError::Quantization` if shape product doesn't match tensor length
pub fn quantize_tensor(
    tensor: &[f32],
    shape: [usize; 2],
    config: &PtBitnetConfig,
) -> Result<(TernaryTensor, QuantizationStats)>;
```

#### 2. Per-Block Quantization

```rust
/// Apply absmean quantization to a single block of weights
///
/// Algorithm:
/// 1. gamma = mean(|block|) + epsilon
/// 2. normalized = block / gamma
/// 3. ternary = round(clamp(normalized, -clip_threshold, +clip_threshold))
/// 4. Map to {-1, 0, +1}
///
/// # Arguments
/// * `block` - Block of FP16/F32 values (length = config.block_size)
/// * `config` - Configuration with epsilon and clip_threshold
///
/// # Returns
/// * `AbsmeanResult` with ternary values, scale, sparsity, MSE
///
/// # Panics
/// * If block.len() != config.block_size
pub fn absmean_ternary(
    block: &[f32],
    config: &PtBitnetConfig,
) -> AbsmeanResult;
```

#### 3. Packing Functions

```rust
/// Pack ternary {-1, 0, +1} values into 2-bit representation
///
/// Encoding: 00 = -1, 01 = 0, 10 = +1, 11 = reserved (unused)
/// 4 values packed per byte: [v3 v2 v1 v0] → byte
///
/// # Arguments
/// * `values` - Ternary values (must be {-1, 0, +1} only)
///
/// # Returns
/// * Packed bytes (length = ceil(values.len() / 4))
///
/// # Errors
/// * If any value is not in {-1, 0, +1}
pub fn pack_ternary(values: &[i8]) -> Result<Vec<u8>>;

/// Unpack 2-bit representation to ternary {-1, 0, +1} values
///
/// # Arguments
/// * `packed` - Packed 2-bit data
/// * `n` - Number of values to extract
///
/// # Returns
/// * Vector of ternary values (length = n)
pub fn unpack_ternary(packed: &[u8], n: usize) -> Vec<i8>;
```

#### 4. Calibration (Optional)

```rust
/// Run calibration pass to optimize scale factors
///
/// # Arguments
/// * `tensor` - Input FP16 tensor
/// * `shape` - Tensor shape
/// * `config` - Config with calibration settings
/// * `calibration_data` - Sample activations for this layer
///
/// # Returns
/// * Optimized `TernaryTensor` with calibrated scales
///
/// # Note
/// This is optional - if not used, falls back to plain absmean
pub fn quantize_with_calibration(
    tensor: &[f32],
    shape: [usize; 2],
    config: &PtBitnetConfig,
    calibration_data: &[Vec<f32>],
) -> Result<(TernaryTensor, QuantizationStats)>;
```

### Dequantization Functions (in `bitnet/dequantize.rs`)

```rust
/// Dequantize BITNET_T158 tensor to FP32
///
/// # Arguments
/// * `data` - Raw GGUF tensor bytes (packed ternary + scales)
/// * `scales` - Per-block FP16 scales (extracted from data)
/// * `n` - Total number of elements to dequantize
///
/// # Returns
/// * Vec<f32> of dequantized values
///
/// # Format
/// Each block: [64 bytes packed ternary][2 bytes FP16 scale]
pub fn dequantize_bitnet_t158(
    data: &[u8],
    scales: &[f16],
    n: usize,
) -> Vec<f32>;

/// Dequantize a single BITNET_T158 block
///
/// # Arguments
/// * `block_data` - 64 bytes of packed ternary data
/// * `scale` - FP16 scale factor
/// * `output` - Output buffer (must have capacity for 256 elements)
pub fn dequantize_bitnet_t158_block(
    block_data: &[u8; 64],
    scale: f16,
    output: &mut [f32],
);
```

### Tensor Conversion (in `bitnet/ternary_tensor.rs`)

```rust
impl TernaryTensor {
    /// Convert from packed storage to FP32 (for validation/testing)
    pub fn to_fp32(&self) -> Vec<f32>;

    /// Create from existing GGUF tensor data
    pub fn from_gguf_data(
        data: &[u8],
        shape: [usize; 2],
        block_size: usize,
    ) -> Result<Self>;

    /// Serialize to GGUF tensor bytes
    pub fn to_gguf_data(&self) -> Vec<u8>;
}
```

---

## D. GGUF Integration Points

### 1. New Quantization Type Variant

**File:** `crates/ruvllm/src/gguf/quantization.rs`

**Changes to `GgufQuantType` enum:**

```rust
#[repr(u32)]
pub enum GgufQuantType {
    // ... existing 0-29 ...

    /// BitNet b1.58 ternary quantization
    /// Block size: 256 elements
    /// Storage: 64 bytes packed (2-bit) + 2 bytes FP16 scale = 66 bytes/block
    /// Bits per weight: 2.06 bpw
    BITNET_T158 = 30,
}

impl GgufQuantType {
    pub fn block_size(&self) -> usize {
        match self {
            // ... existing cases ...
            Self::BITNET_T158 => 256,
        }
    }

    pub fn type_size(&self) -> usize {
        match self {
            // ... existing cases ...
            Self::BITNET_T158 => 66,  // 64 + 2
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            // ... existing cases ...
            Self::BITNET_T158 => "BITNET_T158",
        }
    }
}

impl TryFrom<u32> for GgufQuantType {
    fn try_from(value: u32) -> Result<Self> {
        match value {
            // ... existing 0-29 ...
            30 => Ok(Self::BITNET_T158),
            _ => Err(/* ... */),
        }
    }
}
```

### 2. Dequantization Dispatch

**File:** `crates/ruvllm/src/gguf/quantization.rs`

**Modification to `dequantize_tensor()` function:**

```rust
pub fn dequantize_tensor(
    data: &[u8],
    dtype: GgufQuantType,
    num_elements: usize,
) -> Result<Vec<f32>> {
    let mut output = vec![0.0f32; num_elements];

    match dtype {
        // ... existing cases ...
        GgufQuantType::BITNET_T158 => {
            // Extract scales and packed data
            let num_blocks = (num_elements + 255) / 256;
            let mut scales = Vec::with_capacity(num_blocks);

            for i in 0..num_blocks {
                let block_offset = i * 66;
                let scale_offset = block_offset + 64;
                let scale_bytes = [data[scale_offset], data[scale_offset + 1]];
                scales.push(f16::from_le_bytes(scale_bytes));
            }

            crate::bitnet::dequantize::dequantize_bitnet_t158(
                data,
                &scales,
                num_elements,
            );
        }
        _ => {
            return Err(RuvLLMError::Model(format!(
                "Dequantization not implemented for {:?}",
                dtype
            )));
        }
    }

    Ok(output)
}
```

### 3. GGUF Metadata Keys

**New metadata keys for BitNet models** (written during quantization, read during load):

```rust
// In quantizer when exporting GGUF
pub const BITNET_METADATA_KEYS: &[(&str, &str)] = &[
    ("craftsman.bitnet.version", "1"),
    ("craftsman.bitnet.weight_encoding", "absmean_ternary"),
    ("craftsman.bitnet.activation_bits", "8"),
    ("craftsman.bitnet.block_size", "256"),
    ("craftsman.bitnet.kernel_hint", "tl1"),  // or "tl2", "i2s"
];
```

**Metadata reading in model loader:**

```rust
// In backend when loading model
fn detect_bitnet_model(metadata: &HashMap<String, GgufValue>) -> bool {
    metadata.get("craftsman.bitnet.version")
        .and_then(|v| v.as_str())
        .map(|v| v == "1")
        .unwrap_or(false)
}
```

### 4. Tensor Info Extension

**No changes needed** - existing `TensorInfo` struct in `parser.rs` already supports:
- `name: String`
- `shape: Vec<usize>`
- `dtype: GgufQuantType` ← Will now include `BITNET_T158`
- `offset: u64`

---

## E. Error Handling Strategy

### Error Types

All errors use existing `RuvLLMError` enum from `crates/ruvllm/src/error.rs`:

```rust
pub enum RuvLLMError {
    // Existing variants...

    // Quantization-specific errors
    Quantization(String),  // Use this variant for all quantization errors
    Model(String),         // For GGUF format issues
    Config(String),        // For invalid configuration
}
```

### Error Scenarios and Handling

| Scenario | Error Type | Recovery Strategy |
|----------|-----------|-------------------|
| Tensor size not divisible by block_size | `Quantization` | Pad last block with zeros |
| Invalid ternary value during packing | `Quantization` | Fail-fast - indicates bug |
| GGUF file has wrong BITNET_T158 block size | `Model` | Fail-fast - corrupted file |
| Calibration device unavailable | `Config` | Fall back to non-calibrated quantization |
| Out of memory during quantization | System panic | Let Rust OOM handler catch |
| Shape mismatch in tensor | `Quantization` | Fail-fast - validate before processing |
| FP16 scale is NaN/Inf | `Quantization` | Clamp to epsilon value |
| Empty tensor / zero elements | `Quantization` | Skip with warning |

### Validation Functions

```rust
/// Validate quantization config
pub fn validate_config(config: &PtBitnetConfig) -> Result<()> {
    if config.block_size == 0 || config.block_size % 4 != 0 {
        return Err(RuvLLMError::Config(
            "block_size must be non-zero and divisible by 4".into()
        ));
    }

    if config.epsilon <= 0.0 {
        return Err(RuvLLMError::Config(
            "epsilon must be positive".into()
        ));
    }

    if config.clip_threshold <= 0.0 || config.clip_threshold > 2.0 {
        return Err(RuvLLMError::Config(
            "clip_threshold must be in range (0.0, 2.0]".into()
        ));
    }

    Ok(())
}

/// Validate tensor shape and size
pub fn validate_tensor(
    tensor: &[f32],
    shape: [usize; 2],
    block_size: usize,
) -> Result<()> {
    let expected_size = shape[0] * shape[1];

    if tensor.len() != expected_size {
        return Err(RuvLLMError::Quantization(format!(
            "Tensor length {} doesn't match shape {:?} (expected {})",
            tensor.len(), shape, expected_size
        )));
    }

    if expected_size % block_size != 0 {
        // Could pad, but for simplicity require exact multiple
        return Err(RuvLLMError::Quantization(format!(
            "Tensor size {} is not divisible by block_size {}",
            expected_size, block_size
        )));
    }

    Ok(())
}
```

---

## F. Testing Strategy

### Unit Tests

#### 1. Absmean Quantization Correctness

**File:** `crates/ruvllm/src/bitnet/tests/quantizer_tests.rs`

```rust
#[test]
fn test_absmean_ternary_basic() {
    // Test that absmean correctly quantizes known values
    let config = PtBitnetConfig::default();

    // Block with known mean(|x|) = 1.0
    let block: Vec<f32> = vec![
        2.0, -2.0, 1.0, -1.0,  // gamma = mean(2,2,1,1,...) ≈ 1.0
        0.5, -0.5, 0.0, 0.0,
        // ... (pad to 256 elements)
    ];

    let result = absmean_ternary(&block, &config);

    // After normalization: 2.0/1.0 = 2.0 → clamp to 1.0 → round to +1
    assert_eq!(result.ternary_weights[0], 1);  // 2.0 → +1
    assert_eq!(result.ternary_weights[1], -1); // -2.0 → -1
    assert_eq!(result.ternary_weights[2], 1);  // 1.0 → +1
    assert_eq!(result.ternary_weights[6], 0);  // 0.0 → 0

    assert!(result.scale > 0.9 && result.scale < 1.1); // gamma ≈ 1.0
}

#[test]
fn test_absmean_all_zeros() {
    let config = PtBitnetConfig::default();
    let block = vec![0.0; 256];

    let result = absmean_ternary(&block, &config);

    // All zeros → scale = epsilon, all ternary = 0
    assert_eq!(result.scale, config.epsilon);
    assert!(result.ternary_weights.iter().all(|&x| x == 0));
    assert_eq!(result.sparsity, 1.0);
}
```

#### 2. Pack/Unpack Round-Trip

```rust
#[test]
fn test_pack_unpack_roundtrip() {
    let original = vec![1i8, -1, 0, 1, 0, -1, 1, 0];

    let packed = pack_ternary(&original).unwrap();
    assert_eq!(packed.len(), 2); // 8 values → 2 bytes

    let unpacked = unpack_ternary(&packed, 8);
    assert_eq!(unpacked, original);
}

#[test]
fn test_pack_invalid_value() {
    let invalid = vec![1i8, 2, 0]; // 2 is not ternary

    let result = pack_ternary(&invalid);
    assert!(result.is_err());
}
```

#### 3. Tensor Validation

```rust
#[test]
fn test_validate_tensor_shape_mismatch() {
    let tensor = vec![1.0; 100];
    let shape = [10, 11]; // 10*11 = 110 ≠ 100

    let result = validate_tensor(&tensor, shape, 256);
    assert!(result.is_err());
}

#[test]
fn test_validate_tensor_block_alignment() {
    let tensor = vec![1.0; 257]; // Not divisible by 256
    let shape = [1, 257];

    let result = validate_tensor(&tensor, shape, 256);
    assert!(result.is_err());
}
```

### Integration Tests

#### 4. Full Quantization Pipeline

```rust
#[test]
fn test_quantize_tensor_full_pipeline() {
    let config = PtBitnetConfig::default();

    // Create a 512-element tensor (2 blocks)
    let tensor: Vec<f32> = (0..512).map(|i| (i as f32) / 512.0).collect();
    let shape = [2, 256];

    let (ternary, stats) = quantize_tensor(&tensor, shape, &config).unwrap();

    assert_eq!(ternary.num_blocks, 2);
    assert_eq!(ternary.packed_data.len(), 2 * 64); // 2 blocks × 64 bytes
    assert_eq!(ternary.scales.len(), 2);
    assert_eq!(stats.num_blocks, 2);

    // Verify reconstruction quality
    let reconstructed = ternary.to_fp32();
    assert_eq!(reconstructed.len(), 512);
}
```

#### 5. GGUF Round-Trip

```rust
#[test]
fn test_gguf_serialization_roundtrip() {
    let config = PtBitnetConfig::default();
    let tensor = vec![1.0; 256];
    let shape = [1, 256];

    let (ternary, _) = quantize_tensor(&tensor, shape, &config).unwrap();

    // Serialize to GGUF format
    let gguf_data = ternary.to_gguf_data();
    assert_eq!(gguf_data.len(), 66); // 1 block = 66 bytes

    // Deserialize
    let recovered = TernaryTensor::from_gguf_data(&gguf_data, shape, 256).unwrap();

    assert_eq!(recovered.packed_data, ternary.packed_data);
    assert_eq!(recovered.scales, ternary.scales);
}
```

### Benchmark Tests

#### 6. Performance Regression

```rust
#[bench]
fn bench_absmean_ternary_256(b: &mut Bencher) {
    let config = PtBitnetConfig::default();
    let block: Vec<f32> = (0..256).map(|i| (i as f32) / 256.0).collect();

    b.iter(|| {
        let _ = absmean_ternary(&block, &config);
    });
}

#[bench]
fn bench_pack_ternary_1024(b: &mut Bencher) {
    let values = vec![1i8; 1024];

    b.iter(|| {
        let _ = pack_ternary(&values);
    });
}
```

### Correctness Validation Tests

#### 7. Bit-Exact Validation Against Reference

```rust
#[test]
fn test_dequantize_matches_reference() {
    // Reference implementation (naive)
    fn reference_dequant(ternary: &[i8], scale: f32) -> Vec<f32> {
        ternary.iter().map(|&t| (t as f32) * scale).collect()
    }

    let config = PtBitnetConfig::default();
    let tensor = vec![1.5, -2.3, 0.1, -0.4]; // Extend to 256
    let tensor_256 = /* pad to 256 */;
    let shape = [1, 256];

    let (ternary, _) = quantize_tensor(&tensor_256, shape, &config).unwrap();

    // Unpack and dequantize
    let unpacked = unpack_ternary(&ternary.packed_data, 256);
    let reference = reference_dequant(&unpacked, ternary.scales[0].to_f32());
    let optimized = ternary.to_fp32();

    // Allow small floating-point error
    for (r, o) in reference.iter().zip(optimized.iter()) {
        assert!((r - o).abs() < 1e-5);
    }
}
```

### Test Organization

```
crates/ruvllm/src/bitnet/tests/
├── quantizer_tests.rs     # absmean, pack/unpack
├── tensor_tests.rs        # TernaryTensor validation
├── dequantize_tests.rs    # BITNET_T158 dequant
├── integration_tests.rs   # Full pipeline, GGUF round-trip
└── benches.rs             # Performance benchmarks
```

---

## G. Implementation Phases

### Phase 0.1: Core Data Structures (~2-3 days)
1. `bitnet/mod.rs` - module structure
2. `bitnet/config.rs` - `PtBitnetConfig`
3. `bitnet/ternary_tensor.rs` - `TernaryTensor`, `TernaryBlock`
4. Unit tests for validation

### Phase 0.2: Quantization Algorithm (~3-4 days)
1. `bitnet/quantizer.rs` - `absmean_ternary()`
2. Pack/unpack functions
3. `quantize_tensor()` main entry point
4. Unit tests for correctness

### Phase 0.3: Dequantization (~2 days)
1. `bitnet/dequantize.rs` - block and tensor dequant
2. Integration with existing `quantization.rs`
3. Round-trip tests

### Phase 0.4: GGUF Integration (~2-3 days)
1. Modify `gguf/quantization.rs` - add `BITNET_T158` enum variant
2. Add metadata keys
3. GGUF serialization/deserialization
4. Integration tests

### Phase 0.5: Validation & Benchmarks (~2 days)
1. Full pipeline integration tests
2. Performance benchmarks
3. Bit-exact validation
4. Documentation

**Total Estimated Effort:** ~13-16 days for clean, well-tested implementation

---

## H. Open Design Questions

| # | Question | Impact | Recommendation |
|---|----------|--------|----------------|
| 1 | Use `IQ1_S` (type 19) or new `BITNET_T158` (type 30)? | Compatibility | **New type 30** - cleaner separation, avoids confusion with IQ1_S's codebook format |
| 2 | Padding strategy for last block if not aligned? | Correctness | **Zero-pad** - simplest, matches BitNet spec |
| 3 | Should calibration be mandatory or optional? | Quality vs Speed | **Optional** - Phase 0 can work without it, add later if needed |
| 4 | F16 or F32 for internal scale computation? | Precision | **F32 internally, store as F16** - extra precision during compute |
| 5 | Handle NaN/Inf in input tensors? | Robustness | **Fail-fast** - corrupted weights should not be silently ignored |
| 6 | Support block sizes other than 256? | Flexibility | **No** - BitNet spec is 256, simplifies code |
| 7 | Multi-threading for per-block quantization? | Performance | **Not in Phase 0** - can add via rayon later |
| 8 | Store sparsity per-block in GGUF? | Kernel optimization | **No** - compute on-the-fly during dequant, saves space |

---

## I. Dependencies and Prerequisites

### Existing RuvLLM Components (Reused)
- `crates/ruvllm/src/error.rs` - `RuvLLMError` enum
- `crates/ruvllm/src/gguf/parser.rs` - GGUF parsing (unchanged)
- `crates/ruvllm/src/gguf/quantization.rs` - Enum + dispatch (modified)
- `half` crate - FP16 support (already in Cargo.toml)

### New External Dependencies
None - uses only existing dependencies

### Minimum Rust Version
Same as RuvLLM (likely 1.70+)

---

## J. Non-Goals (Out of Scope)

1. **Calibration implementation** - Deferred to future phase
2. **TL1/TL2 kernel implementation** - Separate ADR/DDD
3. **Model loader integration** - Separate backend implementation
4. **Performance optimization** - Phase 0 is correctness-first
5. **WASM support** - Desktop/server only for Phase 0
6. **Dynamic quantization** - Only post-training static
7. **Mixed-precision strategies** - All-or-nothing ternary for Phase 0

---

## K. Success Criteria

**This design is complete when:**

1. All struct definitions have complete field specifications
2. All function signatures are documented with arguments, returns, errors
3. Module organization is clear and follows Rust conventions
4. GGUF integration points are precisely specified
5. Error handling covers all failure modes
6. Test plan covers correctness, integration, and performance
7. Implementation phases are realistic and sequenced
8. Open questions are documented with recommendations

**Implementation is successful when:**

1. All unit tests pass
2. Round-trip GGUF serialization is bit-exact
3. Dequantization produces correct FP32 output
4. Integration with existing GGUF pipeline works
5. Quantization of GLM-4.7-Flash completes without errors
6. Exported GGUF file is loadable by model loader

---

## Appendix A: Code Size Estimates

| File | Estimated Lines | Complexity |
|------|----------------|------------|
| `bitnet/mod.rs` | ~50 | Low |
| `bitnet/config.rs` | ~80 | Low |
| `bitnet/ternary_tensor.rs` | ~200 | Medium |
| `bitnet/quantizer.rs` | ~350 | High |
| `bitnet/dequantize.rs` | ~150 | Medium |
| `gguf/quantization.rs` (changes) | ~100 | Low |
| Tests | ~800 | Medium |
| **Total** | **~1,730 lines** | |

**Comparison to ADR-018 estimate:** ~200-300 lines core quantizer → Actual ~350 lines (reasonable given struct overhead)

---

## Appendix B: Memory Layout Examples

### TernaryBlock Storage (66 bytes)

```
Byte Offset | Content
------------|--------
0-63        | Packed 2-bit ternary (256 values)
64-65       | FP16 scale (little-endian)
```

### 2-Bit Packing Example

```
Values: [+1, -1, 0, +1]
Encoding: [10, 00, 01, 10]
Packed byte: 10_00_01_10 = 0x86
```

### GGUF Tensor Data Layout

```
[TensorInfo] (in header)
  name: "model.layers.0.mlp.gate_proj.weight"
  shape: [4096, 11008]
  dtype: BITNET_T158 (30)
  offset: 0x1000

[Tensor Data] (at offset 0x1000)
  Block 0: [64 bytes packed][2 bytes scale]
  Block 1: [64 bytes packed][2 bytes scale]
  ...
  Block N: [64 bytes packed][2 bytes scale]
```

---

**End of Design Document**

