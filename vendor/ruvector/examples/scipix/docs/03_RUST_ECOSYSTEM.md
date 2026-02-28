# Rust OCR and ML Ecosystem Analysis for ruvector-scipix

## Executive Summary

This document provides a comprehensive analysis of the Rust ecosystem for OCR (Optical Character Recognition) and machine learning, focusing on libraries suitable for the ruvector-scipix project. The analysis covers seven primary OCR/ML libraries, examines ONNX Runtime integration options, evaluates GPU acceleration capabilities, and provides technology stack recommendations optimized for performance, memory efficiency, and cross-platform deployment.

**Key Finding**: The optimal stack for ruvector-scipix combines `ort` (ONNX Runtime bindings) for inference, `image`/`imageproc` for preprocessing, with optional pure Rust alternatives (`tract`, `candle`) for WASM targets.

---

## 1. Library Comparison Matrix

### OCR Libraries

| Library | Type | Model Support | WASM Support | GPU Support | Maturity | Performance | Dependencies |
|---------|------|---------------|--------------|-------------|----------|-------------|--------------|
| **ocrs** | Native Rust | ONNX (RTen engine) | ✅ Yes | ❌ No | 🟡 Preview | Medium | Minimal (Pure Rust) |
| **oar-ocr** | ONNX Wrapper | PaddleOCR ONNX | ✅ Yes | ✅ CUDA | 🟢 Stable | High | ort (ONNX Runtime) |
| **kalosm-ocr** | Pure Rust | TrOCR (candle) | ✅ Yes | ✅ WGPU/Metal/CUDA | 🟡 Alpha | Medium | candle ML framework |
| **leptess** | FFI Bindings | Tesseract C++ | ❌ No | ❌ No | 🟢 Mature | High (CPU) | Tesseract C++ library |
| **paddle-ocr-rs** | ONNX Wrapper | PaddleOCR v4/v5 | ✅ Yes | ✅ CUDA/TensorRT | 🟢 Stable | Very High | ort (ONNX Runtime) |
| **pure-onnx-ocr** | Pure ONNX | PaddleOCR DBNet+SVTR | ✅ Yes | ✅ Via ONNX RT | 🟢 Active (2025) | High | No C/C++ deps |

### ML Inference Engines

| Library | Purpose | Model Format | WASM Support | GPU Support | Performance | Maturity |
|---------|---------|--------------|--------------|-------------|-------------|----------|
| **ort** | ONNX Runtime | ONNX | ✅ Yes | ✅ CUDA/TensorRT/OpenVINO | **Very High** | 🟢 Production |
| **candle** | ML Framework | Multiple | ✅ Yes | ✅ CUDA/Metal/WGPU | High | 🟢 Stable (HuggingFace) |
| **tract** | ONNX/TF Inference | ONNX, NNEF, TF | ✅ Yes | ❌ Limited | High (CPU) | 🟢 Mature (Sonos) |
| **burn** | Deep Learning | Multiple | ✅ Yes | ✅ CUDA/Metal/WGPU | Very High | 🟢 Active |

**Legend**: 🟢 Production-ready | 🟡 Active development | 🔴 Experimental

### Performance Benchmarks

Based on research findings:

- **ort + PaddleOCR**: 73.1% latency reduction for recognition, 40.4% for detection (NVIDIA T4)
- **ONNX conversion**: Up to 5x faster than PaddlePaddle native inference
- **tract**: 70μs (RPi Zero), 11μs (RPi 3) for CNN models
- **Tesseract (leptess)**: Baseline CPU performance, requires preprocessing
- **ocrs**: Early preview, moderate performance on clear text

---

## 2. ONNX Runtime Integration Options

### 2.1 The `ort` Crate (Recommended)

**Overview**: `ort` by pykeio is the premier ONNX Runtime binding for Rust, offering production-grade performance and extensive hardware acceleration support.

**Key Features**:
- **Hardware Acceleration**: CUDA, TensorRT, OpenVINO, Qualcomm QNN, Huawei CANN
- **Dynamic Loading**: Runtime linking for flexibility (`load-dynamic` feature)
- **Alternative Backends**: Support for tract and candle backends
- **Minimal Builds**: RTTI-free, optimized binary sizes for production
- **Float16/BFloat16**: Via `half` crate integration
- **Production Proven**: Used by Twitter (homepage recommendations), Google (Magika), Bloop, SurrealDB

**Cargo Features**:
```toml
[dependencies]
ort = { version = "2.0.0-rc", features = [
    "half",           # Float16/BFloat16 support
    "load-dynamic",   # Runtime dynamic linking
    "cuda",           # NVIDIA GPU acceleration (requires CUDA 11.6+)
    "tensorrt",       # TensorRT optimization (requires TensorRT 8.4+)
] }
```

**Performance Characteristics**:
- Significantly faster than PyTorch for inference
- Supports model quantization (int8, float16)
- Multi-GPU distribution via NCCL
- Optimal for batch processing and real-time inference

**Integration Example**:
```rust
use ort::{Session, Value};

// Load ONNX model
let session = Session::builder()?
    .with_optimization_level(GraphOptimizationLevel::Level3)?
    .with_intra_threads(4)?
    .commit_from_file("model.onnx")?;

// Run inference
let input = Value::from_array(session.allocator(), &input_tensor)?;
let outputs = session.run(vec![input])?;
```

### 2.2 Alternative: `tract` Backend

**Use Case**: When ONNX Runtime binaries are problematic or WASM target required

**Advantages**:
- Pure Rust implementation
- No external C++ dependencies
- Excellent WASM support
- Passes 85% of ONNX backend tests
- Lightweight and maintainable

**Limitations**:
- No tensor sequences or optional tensors
- Limited GPU support compared to ort
- TensorFlow 2 support via ONNX conversion only

### 2.3 Alternative: `candle` Backend

**Use Case**: When integrating with Hugging Face ecosystem or needing pure Rust

**Advantages**:
- Minimalist design, fast compilation
- Native Hugging Face model support (LLaMA, Whisper, Stable Diffusion)
- WASM + WebGPU acceleration
- Small binary size for serverless deployment
- CUDA, Metal, MKL, Accelerate backends

**Limitations**:
- Younger ecosystem than ONNX Runtime
- Fewer pre-optimized OCR models available
- Focus on inference over training

---

## 3. Pure Rust ML with Candle/Tract

### 3.1 Candle Framework (Hugging Face)

**Architecture**: Minimalist ML framework emphasizing inference efficiency and cross-platform deployment.

**Supported Models**:
- **Language Models**: LLaMA (v1/v2/v3), Mistral 7b, Mixtral 8x7b, Phi 1/2/3, Gemma, StarCoder
- **Vision Models**: Stable Diffusion (1.5, 2.1, SDXL), YOLO (v3/v8), Segment Anything
- **Speech**: Whisper ASR

**Backend Support**:
| Backend | Platform | Performance | Use Case |
|---------|----------|-------------|----------|
| CUDA | NVIDIA GPU | Very High | Production inference |
| Metal | Apple Silicon | High | macOS/iOS deployment |
| CPU (MKL) | x86 Intel | Medium-High | CPU-only servers |
| CPU (Accelerate) | Apple | Medium-High | macOS CPU fallback |
| WGPU | WebGPU-enabled | Medium | Browser deployment |

**Design Philosophy**:
- Remove Python from production workloads
- Minimize binary size (critical for edge/serverless)
- Fast startup times (first token ~120ms on M2 MacBook Air)
- Rust's safety guarantees for ML workloads

**Example Usage**:
```rust
use candle_core::{Device, Tensor};
use candle_onnx;

// Load model
let model = candle_onnx::read_file("model.onnx")?;
let graph = model.graph.as_ref().unwrap();

// Create device (CUDA/Metal/CPU)
let device = Device::cuda_if_available(0)?;

// Run inference
let input = Tensor::randn(0f32, 1f32, (1, 3, 224, 224), &device)?;
let output = model.forward(&[input])?;
```

### 3.2 Tract Framework (Sonos)

**Architecture**: Pure Rust ONNX/TensorFlow inference engine optimized for embedded devices.

**Key Capabilities**:
- **ONNX Support**: 85% of ONNX backend tests passing
- **Operator Set**: ONNX 1.4.1 (opset 9) through 1.13.0 (opset 18)
- **Proven Models**: AlexNet, DenseNet, Inception, ResNet, VGG, SqueezeNet, etc.
- **Pulsing**: Streaming inference for time-series models (e.g., WaveNet)
- **Quantization**: Built-in int8 quantization support

**Performance Characteristics**:
- Optimized for CPU inference
- Excellent for edge devices (Raspberry Pi, embedded systems)
- Minimal memory footprint
- No RTTI or runtime overhead

**Example Usage**:
```rust
use tract_onnx::prelude::*;

// Load and optimize model
let model = tract_onnx::onnx()
    .model_for_path("model.onnx")?
    .with_input_fact(0, f32::fact([1, 3, 224, 224]).into())?
    .into_optimized()?
    .into_runnable()?;

// Run inference
let input = tract_ndarray::arr4(&[[...]]).into_dyn();
let result = model.run(tvec![input.into()])?;
```

**Quantization Support**:
```rust
let model = tract_onnx::onnx()
    .model_for_path("model.onnx")?
    .with_input_fact(0, f32::fact([1, 3, 224, 224]).into())?
    .quantize()?  // Automatic int8 quantization
    .into_optimized()?
    .into_runnable()?;
```

### 3.3 Comparison: Candle vs Tract vs ort

| Criterion | Candle | Tract | ort |
|-----------|--------|-------|-----|
| **Performance (GPU)** | Very High | N/A | Very High |
| **Performance (CPU)** | High | Very High | Very High |
| **Binary Size** | Small | Very Small | Large |
| **Startup Time** | Fast | Very Fast | Medium |
| **WASM Support** | Excellent | Excellent | Good (with backends) |
| **Model Ecosystem** | Hugging Face | ONNX/TF | ONNX (largest) |
| **GPU Backends** | CUDA/Metal/WGPU | Limited | CUDA/TensorRT/OpenVINO |
| **Quantization** | Manual | Built-in | Excellent (ONNX tools) |
| **Maturity** | Stable (2024+) | Mature (2018+) | Production (Microsoft) |

**Recommendation**:
- **ort**: Primary choice for maximum performance and hardware acceleration
- **candle**: Secondary choice for WASM targets or Hugging Face integration
- **tract**: Fallback for pure Rust requirements or extreme size constraints

---

## 4. Image Processing in Rust

### 4.1 The `image` Crate (Foundation)

**Purpose**: Core image encoding/decoding and basic manipulation.

**Supported Formats**:
- JPEG, PNG, GIF, WebP, TIFF, BMP, ICO, PNM, DDS, TGA, OpenEXR, AVIF

**Key Features**:
```rust
use image::{DynamicImage, ImageBuffer, Rgba, GenericImageView};

// Load image
let img = image::open("input.jpg")?;

// Basic operations (in imageops module)
let resized = img.resize(800, 600, image::imageops::FilterType::Lanczos3);
let grayscale = img.grayscale();
let blurred = imageops::blur(&img, 2.0);
let contrast_adjusted = imageops::contrast(&img, 30.0);
```

### 4.2 The `imageproc` Crate (Advanced Processing)

**Purpose**: Advanced image processing algorithms for computer vision.

**Modules**:
| Module | Capabilities |
|--------|-------------|
| **Contrast** | Histogram equalization, adaptive thresholding, CLAHE |
| **Corners** | Harris, FAST, Shi-Tomasi corner detection |
| **Distance Transform** | Euclidean distance maps, morphological operations |
| **Edges** | Canny edge detection, Sobel/Scharr operators |
| **Filter** | Gaussian, median, bilateral filtering |
| **Geometric** | Rotation, affine, projective transformations |
| **Morphology** | Erosion, dilation, opening, closing |
| **Drawing** | Shapes, text, anti-aliased primitives |
| **Contours** | Border tracing, contour extraction |

**Parallelism**: CPU-based multithreading via `rayon` (not GPU acceleration)

**OCR Preprocessing Example**:
```rust
use imageproc::contrast::{adaptive_threshold, ThresholdType};
use imageproc::filter::gaussian_blur_f32;
use imageproc::geometric_transformations::{rotate_about_center, Interpolation};

// Preprocessing pipeline for OCR
fn preprocess_for_ocr(img: &DynamicImage) -> GrayImage {
    // Convert to grayscale
    let gray = img.to_luma8();

    // Denoise with Gaussian blur
    let blurred = gaussian_blur_f32(&gray, 1.0);

    // Adaptive thresholding for varying lighting
    let binary = adaptive_threshold(&blurred, 21);

    // Deskew if needed
    let angle = detect_skew(&binary); // Custom function
    let deskewed = rotate_about_center(&binary, angle, Interpolation::Bilinear, Luma([255u8]));

    deskewed
}
```

### 4.3 GPU Acceleration Options for Image Processing

**Current State**: `imageproc` does NOT provide GPU acceleration. For GPU-accelerated image processing, consider:

**Option 1: `wgpu` + Custom Compute Shaders**
```rust
use wgpu;

// GPU compute shader for image processing
let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
    label: Some("Image Processing"),
    source: wgpu::ShaderSource::Wgsl(include_str!("process.wgsl")),
});
```

**Option 2: OpenCV-Rust Bindings** (if CUDA needed)
- Provides GPU-accelerated operations via CUDA
- Requires OpenCV C++ installation
- Not pure Rust

**Option 3: Integrate with ML Framework GPU Ops**
- Use candle/ort tensor operations for preprocessing
- Leverage existing GPU context
- Keep preprocessing on same device as inference

**Recommendation for ruvector-scipix**:
- Use `image` + `imageproc` for CPU preprocessing (fast enough for most cases)
- For GPU pipeline, implement preprocessing as ONNX graph nodes or candle operations
- Leverage rayon parallelism for batch processing

---

## 5. GPU Acceleration Options

### 5.1 Cross-Platform GPU Support in 2025

The Rust ML ecosystem has achieved robust cross-platform GPU support through standardization around WebGPU and established APIs.

**Unified Backend: `wgpu` (WebGPU Standard)**
- **Targets**: Vulkan (Linux/Windows/Android), Metal (macOS/iOS), DirectX 12 (Windows), WebGPU (browsers)
- **Use Case**: Portable GPU compute without vendor lock-in
- **Frameworks**: Burn, Candle (WGPU backend), kalosm

**Performance Profile**:
| Backend | Platform | Speedup vs CPU | Use Case |
|---------|----------|----------------|----------|
| CUDA | NVIDIA GPU | 10-50x | Production ML inference |
| TensorRT | NVIDIA GPU | 15-70x | Optimized ONNX models |
| Metal | Apple Silicon | 8-30x | macOS/iOS deployment |
| OpenVINO | Intel | 5-20x | Intel CPU/GPU optimization |
| WGPU | WebGPU-capable | 3-15x | Browser/cross-platform |
| ROCm | AMD GPU | 10-40x | AMD GPU acceleration |

### 5.2 CUDA Support

**Primary Library**: `cudarc` (Low-level CUDA bindings)

**Integration via ONNX Runtime**:
```toml
[dependencies]
ort = { version = "2.0", features = ["cuda"] }
```

**Requirements**:
- CUDA Toolkit 11.6+ (for ort)
- NVIDIA GPU: Maxwell (7xx series) or newer
- Compute Capability 5.0+

**Benefits**:
- Industry-standard ML acceleration
- Mature ecosystem and tooling
- Extensive operator coverage
- Best-in-class performance for training and inference

### 5.3 Metal Support (Apple Silicon)

**Framework Integration**:
- **Candle**: Native Metal backend via `metal` crate
- **Burn**: Metal support through `burn-metal` backend
- **ONNX Runtime**: CoreML execution provider (Metal-accelerated)

**Example (Candle)**:
```rust
use candle_core::Device;

let device = Device::new_metal(0)?; // First Metal device
let tensor = Tensor::randn(0f32, 1f32, (1024, 1024), &device)?;
```

**Performance**: 8-30x speedup vs CPU, optimized for M1/M2/M3 chips

### 5.4 WebGPU/WGPU

**Purpose**: Cross-platform GPU compute for WASM and native

**Frameworks with WGPU Support**:
- **Burn**: First-class WGPU backend
- **Candle**: WGPU support for browser deployment
- **Kalosm**: WGPU acceleration via Fusor (0.5 release)

**Browser Deployment**:
```rust
// WASM-compatible GPU inference
#[cfg(target_arch = "wasm32")]
use candle_core::Device;

let device = Device::Cpu; // Or Device::Metal/Cuda if available
```

**Benefits**:
- Browser-based ML inference without server
- Works on AMD GPUs (unlike CUDA)
- Portable across desktop and web
- Future-proof standard (W3C specification)

**Limitations**:
- Lower performance than native CUDA/Metal
- Browser memory constraints (typically 2-8GB)
- First token latency: ~120ms (acceptable for many use cases)

### 5.5 TensorRT (NVIDIA Optimization)

**Purpose**: Optimized ONNX model execution on NVIDIA GPUs

**Requirements**:
- NVIDIA GPU: GeForce 9xx series or newer
- TensorRT 8.4+
- CUDA 11.6+

**Integration**:
```toml
ort = { version = "2.0", features = ["cuda", "tensorrt"] }
```

**Benefits**:
- Automatic kernel fusion and layer optimization
- Mixed precision (FP32/FP16/INT8)
- Up to 2-5x faster than standard CUDA
- Optimal for high-throughput production deployment

### 5.6 OpenVINO (Intel)

**Target**: Intel CPUs (6th gen+) and Intel integrated GPUs

**Use Case**:
- Intel-based servers without discrete GPU
- Edge devices with Intel processors
- Cost-effective acceleration without NVIDIA hardware

**Integration**:
```toml
ort = { version = "2.0", features = ["openvino"] }
```

**Performance**: 5-20x CPU speedup depending on model and hardware

### 5.7 GPU Acceleration Recommendation for ruvector-scipix

**Tiered Approach**:

1. **Primary (Production)**: `ort` with CUDA/TensorRT
   - Maximum performance for server deployment
   - Best operator coverage for PaddleOCR models
   - Production-proven reliability

2. **Secondary (Apple Ecosystem)**: `candle` with Metal
   - Native Apple Silicon support
   - Good for macOS/iOS deployment
   - Smaller binary size than ONNX Runtime

3. **Tertiary (WASM/Browser)**: `candle` or `tract` with WGPU
   - Client-side OCR in browser
   - Privacy-preserving (no server upload)
   - Acceptable performance for interactive use

4. **Fallback (CPU-only)**: `tract` or `ort` with optimized CPU execution
   - MKL/OpenBLAS acceleration
   - Rayon parallelism
   - Still faster than Python alternatives

---

## 6. WebAssembly Compilation Considerations

### 6.1 WASM for ML: Current State (2025)

**Key Finding**: Rust + WASM is the optimal combination for browser-based ML inference, outperforming C++ and other alternatives.

**Performance Characteristics**:
- Rust compiles to WASM **faster** than C++
- Rust produces **smaller binaries** than C++ WASM
- **Memory efficiency**: Rust's ownership model translates well to WASM linear memory
- Consistent performance across browsers

### 6.2 Memory Constraints and Optimization

**Browser Memory Limits**:
- Typical: 2-4GB per tab (Chrome/Firefox)
- Maximum: 4-8GB (varies by browser/OS)
- **Critical Issue**: Running multiple models can exhaust memory quickly

**Memory Optimization Strategies**:

**1. Model Quantization**
```rust
// INT8 quantization reduces memory by 4x
// FP16 quantization reduces memory by 2x
let quantized_model = model.quantize(QuantizationType::QInt8)?;
```

**2. Memory Reuse**
```rust
// Pre-allocate tensors, reuse across inferences
struct InferenceContext {
    input_buffer: Vec<f32>,
    output_buffer: Vec<f32>,
}

impl InferenceContext {
    fn run_inference(&mut self, model: &Model, data: &[f32]) -> Result<&[f32]> {
        self.input_buffer.copy_from_slice(data);
        model.run(&self.input_buffer, &mut self.output_buffer)?;
        Ok(&self.output_buffer)
    }
}
```

**3. Lazy Loading with Streaming Compile**
```rust
// Use WebAssembly.instantiateStreaming for faster startup
// Load models on-demand, not at initialization
async fn load_model_lazy(url: &str) -> Result<Module> {
    let response = window.fetch(url).await?;
    let module = WebAssembly::instantiate_streaming(response).await?;
    Ok(module)
}
```

**4. wasm-opt Optimization**
```bash
# Optimize WASM binary size and performance
wasm-opt -Oz --enable-simd --enable-bulk-memory input.wasm -o output.wasm
```

**5. Model Cleanup**
```rust
// Explicit cleanup when switching models
impl Drop for ModelContext {
    fn drop(&mut self) {
        // Free GPU resources
        self.gpu_buffers.clear();
        // Trigger garbage collection hint (if available)
    }
}
```

### 6.3 Bundle Size Considerations

**Challenge**: Rust-derived WASM bundles often exceed 300KB (uncompressed), delaying first paint.

**Mitigation Strategies**:

**1. Code Splitting**
```rust
// Load OCR functionality separately from main bundle
#[wasm_bindgen]
pub async fn init_ocr() -> Result<OcrEngine, JsValue> {
    // Lazy-load OCR model
    let model = load_model("ocr.onnx").await?;
    Ok(OcrEngine::new(model))
}
```

**2. Minimal Features**
```toml
[dependencies]
ort = { version = "2.0", default-features = false, features = ["minimal-build"] }
tract-onnx = { version = "0.22", default-features = false }
```

**3. Compression**
```bash
# Brotli compression (recommended by Chrome)
brotli -q 11 output.wasm -o output.wasm.br

# Gzip fallback
gzip -9 output.wasm
```

**4. Tree Shaking**
```toml
[profile.release]
opt-level = "z"  # Optimize for size
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

**Expected Sizes**:
| Configuration | Uncompressed | Brotli | Gzip |
|---------------|--------------|--------|------|
| Minimal tract | ~800KB | ~250KB | ~320KB |
| Full ort | ~3MB | ~900KB | ~1.1MB |
| Candle (minimal) | ~600KB | ~180KB | ~240KB |

### 6.4 WASM-Specific Limitations

**1. Threading Constraints**
- SharedArrayBuffer required for multi-threading
- COEP/COOP headers needed for isolation
- Not all browsers support WASM threads

**2. SIMD Support**
- WASM SIMD enabled by default in modern browsers
- Significant performance boost for ML operations
- Check browser compatibility: `wasm-feature-detect`

**3. No Direct File System Access**
- Use IndexedDB or Cache API for model storage
- Stream models from network (HTTP/2)
- Consider embedding small models in binary

**4. GPU Access**
- WebGPU required for GPU acceleration
- Not universally supported (as of 2025, Chrome/Edge primarily)
- Fallback to CPU inference needed

### 6.5 Recommended WASM Frameworks for ruvector-scipix

**Primary: `candle` with WGPU**
- Smallest binary size
- Native WASM support
- WebGPU acceleration when available
- Hugging Face ecosystem

**Secondary: `tract`**
- Pure Rust, no C++ dependencies
- Excellent WASM support
- Proven in production (Sonos)
- CPU-optimized

**Alternative: `ort` with WASM backend**
- Full ONNX operator support
- Can use tract or candle as backend
- Larger bundle size

**Example WASM Integration**:
```rust
use wasm_bindgen::prelude::*;
use candle_core::{Device, Tensor};

#[wasm_bindgen]
pub struct OcrEngine {
    model: candle_onnx::Model,
    device: Device,
}

#[wasm_bindgen]
impl OcrEngine {
    #[wasm_bindgen(constructor)]
    pub async fn new() -> Result<OcrEngine, JsValue> {
        // Use WebGPU if available, fallback to CPU
        let device = Device::Cpu; // Or Device::new_wgpu(0)?

        // Load model from URL
        let model_bytes = fetch_model("model.onnx").await?;
        let model = candle_onnx::read(&model_bytes)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(OcrEngine { model, device })
    }

    pub fn recognize_text(&self, image_data: &[u8]) -> Result<String, JsValue> {
        // Preprocess image
        let tensor = preprocess_image(image_data, &self.device)?;

        // Run inference
        let output = self.model.forward(&[tensor])
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        // Decode output
        let text = decode_predictions(output)?;
        Ok(text)
    }
}
```

### 6.6 WASM Deployment Checklist

- [ ] Enable WASM SIMD in build (`RUSTFLAGS='-C target-feature=+simd128'`)
- [ ] Optimize bundle size (`opt-level = "z"`, LTO, strip)
- [ ] Implement lazy loading for models
- [ ] Set up proper CORS headers for model fetching
- [ ] Add WebGPU feature detection with CPU fallback
- [ ] Configure Brotli/Gzip compression on CDN
- [ ] Test memory usage across browsers (especially mobile)
- [ ] Implement model cleanup on tab close
- [ ] Add loading indicators for async model initialization
- [ ] Consider service worker for model caching

---

## 7. Memory Management for Large Models

### 7.1 Memory Challenges in ML Inference

**Typical OCR Model Sizes**:
- PaddleOCR Detection: 3-10MB (FP32)
- PaddleOCR Recognition: 5-15MB (FP32)
- TrOCR: 50-300MB (depending on variant)
- Tesseract trained data: 10-50MB per language

**Memory Consumption Beyond Model Weights**:
- Input tensors: Image size × channels × precision
- Intermediate activations: Varies by architecture (can exceed model size)
- Output buffers: Sequence length × vocab size
- KV cache (for transformers): Context length × hidden size × layers

### 7.2 Quantization Strategies

**INT8 Quantization** (4x memory reduction)
```rust
// ONNX Runtime quantization
use ort::quantization::{QuantizationConfig, QuantizationType};

let config = QuantizationConfig::default()
    .with_per_channel(true)
    .with_reduce_range(true);

let quantized_model = ort::quantize("model.onnx", "model_int8.onnx", config)?;
```

**Benefits**:
- 75% memory reduction (FP32 → INT8)
- Minimal accuracy loss (typically <1% for OCR)
- Faster inference on integer-optimized hardware
- Reduced cache pressure

**FP16 Quantization** (2x memory reduction)
```rust
// Using ort with half crate
use half::f16;
use ort::tensor::OrtOwnedTensor;

let input_f16: Vec<f16> = input_f32.iter().map(|&x| f16::from_f32(x)).collect();
let tensor = OrtOwnedTensor::from_array(input_f16)?;
```

**Benefits**:
- Better accuracy preservation than INT8
- Native support on modern GPUs (Tensor Cores)
- Still significant memory savings

**Dynamic Quantization** (Runtime)
```rust
// tract supports dynamic quantization
let model = tract_onnx::onnx()
    .model_for_path("model.onnx")?
    .with_input_fact(0, InferenceFact::dt_shape(f32::datum_type(), dims))?
    .quantize()? // Automatic quantization
    .into_optimized()?
    .into_runnable()?;
```

### 7.3 Memory Pooling and Reuse

**Tensor Buffer Reuse**:
```rust
use std::sync::Arc;
use parking_lot::Mutex;

struct TensorPool {
    buffers: Vec<Arc<Mutex<Vec<f32>>>>,
    size: usize,
}

impl TensorPool {
    fn new(pool_size: usize, buffer_size: usize) -> Self {
        let buffers = (0..pool_size)
            .map(|_| Arc::new(Mutex::new(vec![0.0f32; buffer_size])))
            .collect();
        TensorPool { buffers, size: pool_size }
    }

    fn acquire(&self) -> Option<Arc<Mutex<Vec<f32>>>> {
        // Round-robin or availability-based selection
        self.buffers.first().cloned()
    }
}
```

**Session Pooling** (ONNX Runtime):
```rust
use once_cell::sync::Lazy;
use ort::Session;

static SESSION_POOL: Lazy<Vec<Session>> = Lazy::new(|| {
    (0..4).map(|_| {
        Session::builder()
            .unwrap()
            .commit_from_file("model.onnx")
            .unwrap()
    }).collect()
});

fn get_session() -> &'static Session {
    &SESSION_POOL[thread_id % 4]
}
```

### 7.4 Streaming and Batching

**Batch Processing** (Amortize overhead):
```rust
fn process_batch(images: &[DynamicImage], model: &Session) -> Result<Vec<String>> {
    let batch_size = images.len();

    // Create batched tensor [batch_size, channels, height, width]
    let mut batch_tensor = vec![0.0f32; batch_size * 3 * 224 * 224];

    for (i, img) in images.iter().enumerate() {
        let offset = i * 3 * 224 * 224;
        preprocess_into_buffer(img, &mut batch_tensor[offset..]);
    }

    // Single inference call for entire batch
    let output = model.run(vec![batch_tensor.into()])?;

    // Decode batch results
    decode_batch_predictions(output, batch_size)
}
```

**Streaming Inference** (For large documents):
```rust
async fn process_document_streaming(
    pages: impl Stream<Item = Image>,
    model: &Session,
) -> impl Stream<Item = Result<String>> {
    pages.map(|page| {
        // Process one page at a time
        let text = recognize_text(&page, model)?;
        Ok(text)
    })
}
```

### 7.5 Model Sharding and Lazy Loading

**Lazy Model Loading**:
```rust
use once_cell::sync::OnceCell;

static DETECTION_MODEL: OnceCell<Session> = OnceCell::new();
static RECOGNITION_MODEL: OnceCell<Session> = OnceCell::new();

fn get_detection_model() -> &'static Session {
    DETECTION_MODEL.get_or_init(|| {
        Session::builder()
            .unwrap()
            .commit_from_file("detection.onnx")
            .unwrap()
    })
}
```

**Conditional Loading**:
```rust
// Only load language-specific models when needed
struct OcrEngine {
    detection: Session,
    recognition_models: HashMap<Language, OnceCell<Session>>,
}

impl OcrEngine {
    fn recognize(&self, img: &Image, lang: Language) -> Result<String> {
        let boxes = self.detect(img)?;

        let rec_model = self.recognition_models
            .get(&lang)
            .unwrap()
            .get_or_init(|| load_recognition_model(lang));

        self.recognize_boxes(img, &boxes, rec_model)
    }
}
```

### 7.6 Memory Mapping (Large Models)

**Using `memmap2` for Model Files**:
```rust
use memmap2::Mmap;
use std::fs::File;

fn load_model_mmap(path: &str) -> Result<Mmap> {
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    Ok(mmap)
}

// Model data stays on disk, paged in as needed
// Useful for models >100MB
```

**Benefits**:
- Reduced resident memory
- Faster startup (no full load)
- Shared memory across processes

**Limitations**:
- Not available in WASM
- Requires file system access
- May have higher latency on first access

### 7.7 GPU Memory Management

**CUDA Unified Memory**:
```rust
// ort automatically manages GPU memory
let session = Session::builder()?
    .with_execution_providers([ExecutionProvider::CUDA])?
    .commit_from_file("model.onnx")?;

// Tensors automatically transferred to/from GPU
```

**Manual GPU Memory Control** (candle):
```rust
use candle_core::{Device, Tensor};

let device = Device::new_cuda(0)?;

// Allocate on GPU
let tensor_gpu = Tensor::randn(0f32, 1f32, (1024, 1024), &device)?;

// Transfer to CPU when needed
let tensor_cpu = tensor_gpu.to_device(&Device::Cpu)?;

// Explicit cleanup
drop(tensor_gpu);
```

### 7.8 Memory Profiling and Monitoring

**Rust Memory Profiling Tools**:
- `valgrind --tool=massif`: Heap profiling
- `heaptrack`: Heap memory profiler (Linux)
- `dhat`: Dynamic heap analysis tool
- `tokio-console`: Async runtime monitoring

**Custom Memory Tracking**:
```rust
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TrackingAllocator;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size(), Ordering::SeqCst);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        ALLOCATED.fetch_sub(layout.size(), Ordering::SeqCst);
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

fn get_memory_usage() -> usize {
    ALLOCATED.load(Ordering::SeqCst)
}
```

### 7.9 Memory Optimization Recommendations for ruvector-scipix

**Priority Strategies**:

1. **Quantize Models** (INT8 for production)
   - 4x memory reduction
   - Minimal accuracy impact for OCR
   - Use ONNX Runtime quantization tools

2. **Implement Tensor Pooling**
   - Reuse buffers for repeated inferences
   - Align with ruvector-core's memory management patterns
   - Use `parking_lot` for efficient synchronization

3. **Lazy Load Language Models**
   - Only load recognition models for requested languages
   - Use `OnceCell` for thread-safe initialization
   - Share models across threads

4. **Batch Processing**
   - Group multiple images into single inference call
   - Amortize overhead, improve GPU utilization
   - Integrate with ruvector's parallel processing

5. **GPU Memory Awareness**
   - Monitor GPU memory usage
   - Implement fallback to CPU if GPU OOM
   - Use smaller batch sizes on memory-constrained devices

6. **Profile Real Workloads**
   - Measure memory with actual ruvector data
   - Identify bottlenecks (model weights vs activations)
   - Optimize based on data

---

## 8. Recommended Technology Stack for ruvector-scipix

### 8.1 Primary Stack (Production Deployment)

**Inference Engine**: `ort` (ONNX Runtime)
- **Version**: `2.0.0-rc` or latest stable
- **Features**: `cuda`, `tensorrt`, `half`, `load-dynamic`
- **Rationale**:
  - Best-in-class performance (73% latency reduction)
  - Extensive GPU support (CUDA, TensorRT, OpenVINO)
  - Production-proven (Twitter, Google, SurrealDB)
  - Largest ONNX model ecosystem

**OCR Models**: PaddleOCR v5 (ONNX format)
- **Detection**: `ch_PP-OCRv5_mobile_det.onnx`
- **Recognition**: `ch_PP-OCRv5_mobile_rec.onnx`
- **Rationale**:
  - State-of-the-art accuracy
  - Optimized for speed (5x faster in ONNX)
  - Multi-language support (80+ languages)
  - Active development (2025 updates)

**Image Processing**: `image` + `imageproc`
- **Version**: Latest stable
- **Rationale**:
  - Comprehensive format support
  - CPU parallelism via rayon (already in workspace)
  - Mature, well-tested
  - Pure Rust (no C++ dependencies)

**Dependencies Integration**:
```toml
[dependencies]
# Inference
ort = { version = "2.0.0-rc", features = ["cuda", "tensorrt", "half", "load-dynamic"] }

# Image processing
image = "0.25"
imageproc = "0.25"

# Existing ruvector-core dependencies (reuse)
rayon = { workspace = true }
ndarray = { workspace = true }
parking_lot = { workspace = true }
dashmap = { workspace = true }
tokio = { workspace = true }
thiserror = { workspace = true }
serde = { workspace = true }
```

### 8.2 Alternative Stack (WASM/Browser Deployment)

**Inference Engine**: `candle` with WGPU backend
- **Version**: Latest stable from Hugging Face
- **Features**: `wasm`, `webgpu`
- **Rationale**:
  - Smallest WASM bundle size
  - Native WebGPU support
  - Fast startup times
  - Pure Rust

**OCR Models**: TrOCR (via candle-onnx) or lightweight PaddleOCR
- Smaller models for browser constraints
- Quantized INT8 versions

**WASM-Specific Stack**:
```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
candle-core = { version = "0.8", default-features = false }
candle-onnx = { version = "0.8" }
wasm-bindgen = { workspace = true }
web-sys = { workspace = true }
```

### 8.3 Fallback Stack (Pure Rust/No External Dependencies)

**Inference Engine**: `tract`
- **Use Case**: When ONNX Runtime binaries unavailable or pure Rust required
- **Rationale**:
  - No C++ dependencies
  - Excellent WASM support
  - Mature (Sonos production use)
  - Passes 85% ONNX tests

**Stack**:
```toml
[dependencies]
tract-onnx = "0.22"
image = "0.25"
imageproc = "0.25"
```

### 8.4 Architecture Design

```
┌─────────────────────────────────────────────────────────────┐
│                    ruvector-scipix                          │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────────┐     ┌──────────────┐    ┌──────────────┐  │
│  │  Image Input │────▶│ Preprocessing│───▶│   Detection  │  │
│  │   (image)    │     │ (imageproc)  │    │  (ort/ONNX)  │  │
│  └──────────────┘     └──────────────┘    └──────┬───────┘  │
│                                                   │          │
│                                                   ▼          │
│                                          ┌──────────────┐    │
│                                          │  Text Boxes  │    │
│                                          └──────┬───────┘    │
│                                                 │            │
│                       ┌─────────────────────────┘            │
│                       │                                      │
│                       ▼                                      │
│              ┌──────────────┐      ┌──────────────┐          │
│              │ Recognition  │─────▶│  Post-Proc.  │          │
│              │  (ort/ONNX)  │      │   (decode)   │          │
│              └──────────────┘      └──────┬───────┘          │
│                                           │                  │
│                                           ▼                  │
│                                  ┌──────────────┐            │
│                                  │ Vector Store │            │
│                                  │ (ruvector-   │            │
│                                  │    core)     │            │
│                                  └──────────────┘            │
│                                                               │
└─────────────────────────────────────────────────────────────┘

GPU Acceleration Layers:
├─ CUDA/TensorRT (NVIDIA)
├─ Metal (Apple Silicon)
├─ OpenVINO (Intel)
└─ WGPU (Cross-platform/Browser)
```

### 8.5 Module Structure

```
examples/scipix/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API
│   ├── engine.rs           # OCR engine orchestration
│   ├── detection.rs        # Text detection (ONNX)
│   ├── recognition.rs      # Text recognition (ONNX)
│   ├── preprocessing.rs    # Image preprocessing (imageproc)
│   ├── postprocessing.rs   # Result decoding and formatting
│   ├── models.rs           # Model loading and management
│   └── config.rs           # Configuration
├── models/                 # ONNX model files (gitignored)
│   ├── detection.onnx
│   ├── recognition.onnx
│   └── dict.txt
├── tests/
│   ├── integration_test.rs
│   └── benchmark.rs
└── docs/
    ├── 01_REQUIREMENTS.md
    ├── 02_ARCHITECTURE.md
    └── 03_RUST_ECOSYSTEM.md  # This document
```

### 8.6 Performance Targets

Based on PaddleOCR benchmarks and Rust optimizations:

| Metric | Target | Hardware |
|--------|--------|----------|
| **Detection Latency** | <50ms | NVIDIA T4 (TensorRT) |
| **Recognition Latency** | <20ms | NVIDIA T4 (TensorRT) |
| **End-to-End (single image)** | <100ms | NVIDIA T4 |
| **Throughput (batched)** | >100 images/sec | NVIDIA T4 |
| **CPU Latency** | <500ms | Modern multi-core CPU |
| **WASM Latency** | <1s | Browser (WebGPU) |
| **Memory Usage** | <500MB | With INT8 quantization |

### 8.7 Development Phases

**Phase 1: Core Implementation (ort + PaddleOCR)**
- Implement detection and recognition pipelines
- Integrate with ruvector-core storage
- CPU-only inference initially
- Basic preprocessing (resize, normalize)

**Phase 2: GPU Acceleration**
- Add CUDA/TensorRT support
- Benchmark and optimize performance
- Implement batching for throughput
- Memory pooling and reuse

**Phase 3: Production Hardening**
- Model quantization (INT8)
- Error handling and fallbacks
- Metrics and monitoring
- Load testing

**Phase 4: WASM Support (Optional)**
- Port to candle or tract
- Browser deployment
- WebGPU acceleration
- Client-side OCR

### 8.8 Testing Strategy

**Unit Tests**:
- Image preprocessing correctness
- Model loading and initialization
- Tensor shape validation
- Output decoding accuracy

**Integration Tests**:
```rust
#[test]
fn test_end_to_end_ocr() {
    let engine = OcrEngine::new(Config::default()).unwrap();
    let img = image::open("tests/fixtures/sample.jpg").unwrap();
    let result = engine.recognize_text(&img).unwrap();
    assert!(result.contains("expected text"));
}
```

**Benchmarks** (using Criterion):
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_detection(c: &mut Criterion) {
    let engine = setup_engine();
    let img = load_test_image();

    c.bench_function("detection", |b| {
        b.iter(|| engine.detect(black_box(&img)))
    });
}

criterion_group!(benches, benchmark_detection);
criterion_main!(benches);
```

**Performance Tests**:
- Latency under various image sizes
- Throughput with batching
- Memory usage over time
- GPU utilization

---

## 9. Integration with ruvector-core Dependencies

### 9.1 Shared Workspace Dependencies

The ruvector-scipix implementation can leverage numerous existing workspace dependencies, minimizing new additions and ensuring consistency.

**Already Available (from workspace)**:

| Dependency | ruvector Use | scipix Use |
|------------|--------------|-------------|
| `rayon` | Parallel distance computation | Batch image preprocessing, parallel OCR |
| `ndarray` | Vector operations | Tensor manipulation, image arrays |
| `parking_lot` | Lock-free data structures | Model pool synchronization |
| `dashmap` | Concurrent hash maps | Model cache, result cache |
| `tokio` | Async runtime | Async inference, streaming |
| `serde` / `serde_json` | Serialization | Config, results serialization |
| `thiserror` / `anyhow` | Error handling | OCR error types |
| `tracing` | Logging | Inference timing, debugging |
| `uuid` | Unique identifiers | Request tracking |
| `chrono` | Timestamps | Inference metrics |

**Benefits**:
- **Minimal new dependencies**: Only add OCR-specific crates
- **Consistent patterns**: Same error handling, logging, async across codebase
- **Binary size**: Shared dependencies not duplicated
- **Maintenance**: Updates to workspace deps benefit all crates

### 9.2 Parallel Processing Integration

**Leverage rayon for Batch OCR**:
```rust
use rayon::prelude::*;

fn process_image_batch(images: &[DynamicImage], engine: &OcrEngine) -> Vec<OcrResult> {
    images.par_iter()
        .map(|img| engine.recognize_text(img))
        .collect()
}
```

**Consistency**: Matches ruvector-core's parallel distance computation pattern

### 9.3 Storage Integration

**Store OCR Results in ruvector-core**:
```rust
use ruvector_core::{VectorStore, Vector};

struct OcrResult {
    text: String,
    embedding: Vec<f32>,  // From embedding model
    bounding_boxes: Vec<BoundingBox>,
}

impl OcrResult {
    fn store_in_ruvector(&self, store: &mut VectorStore) -> Result<uuid::Uuid> {
        let vector = Vector::new(self.embedding.clone());
        let id = store.insert(vector)?;

        // Store metadata
        store.set_metadata(id, "text", &self.text)?;
        store.set_metadata(id, "boxes", &self.bounding_boxes)?;

        Ok(id)
    }
}
```

**Vector Search for OCR Results**:
```rust
// Find similar documents by text embedding
let query_embedding = embed_text("search query")?;
let similar_docs = store.search(&query_embedding, 10)?;
```

### 9.4 WASM Compatibility

**ruvector-core WASM Patterns**:
- `memory-only` feature for WASM targets
- `wasm-bindgen` for browser interop
- `getrandom` with `wasm_js` feature

**Apply to scipix**:
```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
candle-core = { version = "0.8", default-features = false }
wasm-bindgen = { workspace = true }
getrandom = { workspace = true, features = ["wasm_js"] }

[features]
default = ["ort-backend"]
ort-backend = ["ort"]
candle-backend = ["candle-core", "candle-onnx"]
wasm = ["candle-backend"]  # WASM uses candle
```

### 9.5 Error Handling Patterns

**Consistent with ruvector-core**:
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OcrError {
    #[error("Model loading failed: {0}")]
    ModelLoadError(String),

    #[error("Inference failed: {0}")]
    InferenceError(String),

    #[error("Image preprocessing failed: {0}")]
    PreprocessingError(#[from] image::ImageError),

    #[error("ONNX Runtime error: {0}")]
    OrtError(#[from] ort::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, OcrError>;
```

### 9.6 Configuration Pattern

**Similar to ruvector-core config**:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrConfig {
    /// Path to detection model
    pub detection_model_path: String,

    /// Path to recognition model
    pub recognition_model_path: String,

    /// Use GPU acceleration if available
    pub use_gpu: bool,

    /// Batch size for parallel processing
    pub batch_size: usize,

    /// Detection confidence threshold
    pub detection_threshold: f32,

    /// Number of inference threads
    pub num_threads: usize,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            detection_model_path: "models/detection.onnx".into(),
            recognition_model_path: "models/recognition.onnx".into(),
            use_gpu: true,
            batch_size: 8,
            detection_threshold: 0.7,
            num_threads: rayon::current_num_threads(),
        }
    }
}
```

### 9.7 Async Integration

**Use tokio for async OCR**:
```rust
use tokio::task;

pub struct AsyncOcrEngine {
    engine: Arc<OcrEngine>,
}

impl AsyncOcrEngine {
    pub async fn recognize_text(&self, image: DynamicImage) -> Result<OcrResult> {
        let engine = Arc::clone(&self.engine);

        // Run blocking OCR in tokio threadpool
        task::spawn_blocking(move || {
            engine.recognize_text_sync(&image)
        }).await?
    }

    pub async fn process_stream(
        &self,
        images: impl Stream<Item = DynamicImage>,
    ) -> impl Stream<Item = Result<OcrResult>> {
        images.then(move |img| {
            let engine = Arc::clone(&self.engine);
            async move {
                engine.recognize_text(img).await
            }
        })
    }
}
```

### 9.8 Metrics Integration

**Use existing tracing infrastructure**:
```rust
use tracing::{info, debug, instrument};

#[instrument(skip(self, image))]
pub fn recognize_text(&self, image: &DynamicImage) -> Result<OcrResult> {
    let start = std::time::Instant::now();

    debug!("Starting OCR for image {}x{}", image.width(), image.height());

    let preprocessed = self.preprocess(image)?;
    debug!("Preprocessing took {:?}", start.elapsed());

    let boxes = self.detect(&preprocessed)?;
    debug!("Detection found {} boxes in {:?}", boxes.len(), start.elapsed());

    let text = self.recognize(&preprocessed, &boxes)?;

    info!(
        "OCR completed in {:?}, extracted {} characters",
        start.elapsed(),
        text.len()
    );

    Ok(OcrResult { text, boxes })
}
```

### 9.9 Testing Infrastructure Reuse

**Use workspace test dependencies**:
```toml
[dev-dependencies]
criterion = { workspace = true }
proptest = { workspace = true }
mockall = { workspace = true }
tempfile = "3.13"
```

**Property-Based Testing** (like ruvector-core):
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_preprocessing_preserves_aspect_ratio(
        width in 100u32..2000u32,
        height in 100u32..2000u32
    ) {
        let img = DynamicImage::new_rgb8(width, height);
        let processed = preprocess_image(&img)?;

        let original_ratio = width as f32 / height as f32;
        let processed_ratio = processed.width() as f32 / processed.height() as f32;

        prop_assert!((original_ratio - processed_ratio).abs() < 0.01);
    }
}
```

### 9.10 Dependency Summary for scipix

**New Dependencies Required**:
```toml
[dependencies]
# OCR/ML (new)
ort = { version = "2.0.0-rc", features = ["cuda", "tensorrt", "half"] }
image = "0.25"
imageproc = "0.25"

# Reuse from workspace (no version needed)
rayon = { workspace = true }
ndarray = { workspace = true }
parking_lot = { workspace = true }
dashmap = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }

# Integration with ruvector-core
ruvector-core = { path = "../../crates/ruvector-core" }
```

**Total New Dependencies**: 3 (ort, image, imageproc)
**Reused Dependencies**: 12 from workspace

---

## 10. License Compatibility

### 10.1 ruvector Project License

**Current License**: MIT (from workspace `Cargo.toml`)

**Requirement**: All dependencies must be MIT-compatible for redistribution.

### 10.2 Recommended Dependencies License Analysis

| Crate | License | Compatible? | Notes |
|-------|---------|-------------|-------|
| **ort** | MIT OR Apache-2.0 | ✅ Yes | Dual-licensed, fully compatible |
| **candle** | MIT OR Apache-2.0 | ✅ Yes | Hugging Face, dual-licensed |
| **tract** | MIT OR Apache-2.0 | ✅ Yes | Dual-licensed (except ONNX protos) |
| **image** | MIT OR Apache-2.0 | ✅ Yes | Pure Rust, dual-licensed |
| **imageproc** | MIT | ✅ Yes | Permissive, MIT-only |
| **ndarray** | MIT OR Apache-2.0 | ✅ Yes | Already in workspace |
| **rayon** | MIT OR Apache-2.0 | ✅ Yes | Already in workspace |
| **wasm-bindgen** | MIT OR Apache-2.0 | ✅ Yes | Already in workspace |

**Incompatible Libraries (Avoid)**:

| Crate | License | Issue |
|-------|---------|-------|
| **leptess** | MIT (wrapper) | ❌ Depends on Tesseract (Apache-2.0 with restrictions) |
| **opencv-rust** | MIT (wrapper) | ❌ Depends on OpenCV (Apache-2.0, complex) |

### 10.3 ONNX Model Licenses

PaddleOCR models used in ONNX format have **Apache-2.0** license.

**Compatibility**:
- ✅ Apache-2.0 code can be used in MIT-licensed projects
- ✅ ONNX models (weights) are typically considered data, not code
- ✅ Distribution of pre-trained models is permitted
- ⚠️ Derivative works of Apache-2.0 code require patent grant preservation

**Best Practice**:
- Download PaddleOCR ONNX models from official sources
- Include LICENSE file in `models/` directory
- Document model provenance in README
- Do not modify Apache-2.0 code (use as-is via ONNX)

### 10.4 Rust Dual-Licensing Best Practices

**Why Rust Uses MIT OR Apache-2.0**:
- **MIT**: Maximum permissiveness, minimal restrictions
- **Apache-2.0**: Patent protection, better for corporate use
- **Dual License**: Users choose which applies to them

**For ruvector-scipix**:

**Option 1: Keep MIT-only (Current)**
- ✅ Simplest licensing
- ✅ Maximum compatibility
- ✅ Minimal legal overhead
- ✅ All dependencies are MIT-compatible

**Option 2: Adopt Dual MIT/Apache-2.0**
- ✅ Better patent protection
- ✅ Aligns with Rust ecosystem norms
- ✅ More attractive to enterprise users
- ⚠️ Slightly more complex

**Recommendation**: Keep MIT-only for simplicity, unless patent concerns arise.

### 10.5 License Compliance Checklist

**For Production Deployment**:

- [ ] Verify all direct dependencies are MIT or MIT/Apache-2.0
- [ ] Check transitive dependencies for license conflicts
- [ ] Include LICENSE file in repository
- [ ] Document third-party licenses in NOTICE file
- [ ] Include PaddleOCR model license in `models/LICENSE`
- [ ] Add copyright headers to source files (optional for MIT)
- [ ] Review ONNX Runtime's license (MIT, but check binary distribution terms)
- [ ] Ensure no GPL/LGPL dependencies (incompatible with MIT)

**Automated License Checking**:
```bash
# Use cargo-license to audit dependencies
cargo install cargo-license
cargo license --all-features

# Fail build on incompatible licenses
cargo deny check licenses
```

**`deny.toml` Configuration**:
```toml
[licenses]
unlicensed = "deny"
allow = [
    "MIT",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Unicode-DFS-2016",
]
deny = [
    "GPL-2.0",
    "GPL-3.0",
    "AGPL-3.0",
]
```

### 10.6 Attribution Requirements

**MIT License Requirements**:
- Include copyright notice
- Include permission notice (LICENSE file)
- No obligation to disclose source code modifications

**For PaddleOCR Models (Apache-2.0)**:
- Include NOTICE file if provided
- Preserve copyright and patent notices
- Document significant modifications (if any)

**Recommended NOTICE File**:
```
ruvector-scipix
Copyright 2025 Ruvector Team

This software includes components from:

1. ONNX Runtime
   Copyright Microsoft Corporation
   Licensed under MIT License

2. PaddleOCR Models
   Copyright PaddlePaddle Authors
   Licensed under Apache License 2.0
   Model files located in models/ directory

3. Candle ML Framework
   Copyright Hugging Face, Inc.
   Licensed under MIT OR Apache-2.0

Complete license texts available in the LICENSE and models/LICENSE files.
```

### 10.7 License Compatibility Summary

**✅ SAFE TO USE** (Recommended Stack):
- `ort` - MIT/Apache-2.0
- `image` - MIT/Apache-2.0
- `imageproc` - MIT
- `candle` - MIT/Apache-2.0
- `tract` - MIT/Apache-2.0
- PaddleOCR ONNX models - Apache-2.0 (data)

**⚠️ USE WITH CAUTION**:
- `leptess` - Requires Tesseract C++ library (complex licensing)
- `opencv-rust` - Requires OpenCV (large dependency, Apache-2.0)

**❌ AVOID**:
- Any GPL/LGPL libraries (incompatible with MIT for proprietary use)
- Proprietary OCR engines (licensing fees, redistribution restrictions)

**Final Recommendation**: The proposed stack (`ort` + PaddleOCR + `image`/`imageproc`) is **fully compatible** with ruvector's MIT license and follows Rust ecosystem best practices.

---

## 11. Final Recommendations

### 11.1 Optimal Technology Stack

**Primary Recommendation (Production)**:
```toml
[dependencies]
# Inference: Best performance, production-proven
ort = { version = "2.0.0-rc", features = ["cuda", "tensorrt", "half", "load-dynamic"] }

# Image processing: Pure Rust, mature
image = "0.25"
imageproc = "0.25"

# OCR models: PaddleOCR v5 ONNX (download separately)
# - Detection: ch_PP-OCRv5_mobile_det.onnx
# - Recognition: ch_PP-OCRv5_mobile_rec.onnx

# Reuse workspace dependencies
rayon = { workspace = true }
ndarray = { workspace = true }
parking_lot = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
thiserror = { workspace = true }

# Integration
ruvector-core = { path = "../../crates/ruvector-core" }
```

**Rationale**:
1. **Performance**: `ort` provides 73% latency reduction vs alternatives
2. **Ecosystem**: Largest ONNX model selection (PaddleOCR, TrOCR, etc.)
3. **GPU Support**: CUDA, TensorRT, OpenVINO, Metal (via CoreML)
4. **Production Ready**: Used by Twitter, Google, SurrealDB
5. **License**: MIT/Apache-2.0 dual-license (fully compatible)
6. **Maintenance**: Active development, Microsoft backing

### 11.2 Alternative Stacks by Use Case

**WASM/Browser Deployment**:
```toml
candle-core = { version = "0.8", features = ["wasm", "webgpu"] }
candle-onnx = "0.8"
```
- Smallest bundle size (~180KB Brotli)
- WebGPU acceleration
- Fast startup (120ms first token)

**Pure Rust / No External Deps**:
```toml
tract-onnx = "0.22"
```
- No C++ dependencies
- Excellent for embedded/restrictive environments
- 85% ONNX compatibility

**Edge Devices / Raspberry Pi**:
```toml
tract-onnx = { version = "0.22", features = ["pulse"] }
```
- Optimized for CPU inference
- Minimal memory footprint
- Proven on RPi (11μs for CNN models)

### 11.3 Implementation Roadmap

**Week 1-2: Core Infrastructure**
- Set up `examples/scipix` crate structure
- Integrate `ort` and `image`/`imageproc`
- Implement model loading (detection + recognition)
- Basic end-to-end pipeline (CPU-only)

**Week 3-4: GPU Acceleration**
- Enable CUDA/TensorRT support
- Implement batching for throughput
- Benchmark performance vs targets
- Memory pooling and optimization

**Week 5-6: Production Hardening**
- Model quantization (INT8)
- Error handling and recovery
- Metrics and monitoring (tracing)
- Integration tests and benchmarks

**Week 7-8: ruvector Integration**
- Store OCR results in ruvector-core
- Implement vector search for documents
- Async API with tokio
- Documentation and examples

**Optional (Week 9-10): WASM Support**
- Port to candle for browser deployment
- WebGPU acceleration
- Client-side OCR demo

### 11.4 Key Metrics to Track

**Performance**:
- Detection latency: Target <50ms (GPU), <200ms (CPU)
- Recognition latency: Target <20ms (GPU), <100ms (CPU)
- End-to-end: Target <100ms (GPU), <500ms (CPU)
- Throughput: Target >100 images/sec (batched, GPU)

**Memory**:
- Model size: ~15-30MB (FP32), ~5-10MB (INT8)
- Runtime memory: Target <500MB
- GPU memory: Monitor for OOM

**Accuracy**:
- Character accuracy: Target >95% (clean text)
- Word accuracy: Target >90%
- Benchmark against Tesseract and commercial APIs

### 11.5 Risk Mitigation

**Model Availability**:
- ✅ PaddleOCR models freely available
- ✅ Multiple model versions for fallback
- ⚠️ Verify ONNX export quality (may need custom conversion)

**Dependency Stability**:
- ✅ `ort` actively maintained (2.0 rc, stable release expected)
- ✅ `image`/`imageproc` mature, widely used
- ⚠️ Monitor for breaking changes during updates

**Performance Variability**:
- ⚠️ GPU performance depends on driver versions
- ⚠️ WASM performance varies by browser
- ✅ Comprehensive benchmarking before production

**License Compliance**:
- ✅ All recommended dependencies MIT-compatible
- ✅ PaddleOCR Apache-2.0 (compatible for use)
- ⚠️ Review licenses before adding new dependencies

### 11.6 Success Criteria

The ruvector-scipix implementation is successful if:

1. **Performance**: Meets or exceeds latency/throughput targets
2. **Accuracy**: Character accuracy >95% on clean text
3. **Integration**: Seamlessly stores results in ruvector-core
4. **Portability**: Runs on Linux/macOS/Windows, CPU and GPU
5. **Memory**: Operates within <500MB budget
6. **License**: Maintains MIT compatibility
7. **Maintainability**: Uses idiomatic Rust, well-documented
8. **Scalability**: Handles batch processing efficiently

### 11.7 Next Steps

1. **Review this document** with ruvector team for alignment
2. **Download PaddleOCR models** (detection + recognition ONNX)
3. **Set up `examples/scipix` crate** with recommended dependencies
4. **Implement basic OCR pipeline** (end-to-end proof of concept)
5. **Benchmark initial implementation** against targets
6. **Iterate and optimize** based on real-world data
7. **Document API** and usage examples
8. **Integrate with ruvector-core** for vector storage

---

## References and Resources

### Documentation
- [ort Documentation](https://ort.pyke.io/) - ONNX Runtime Rust bindings by pykeio
- [Candle GitHub](https://github.com/huggingface/candle) - Minimalist ML framework for Rust
- [tract GitHub](https://github.com/sonos/tract) - Tiny, no-nonsense ONNX/TF inference
- [PaddleOCR GitHub](https://github.com/PaddlePaddle/PaddleOCR) - OCR models and documentation
- [imageproc Docs](https://docs.rs/imageproc) - Rust image processing library

### Performance Benchmarks
- [Rust at the Metal: GPU Layer Driving Modern AI](https://rustacean.ai/p/issue-2-rust-at-the-metal-the-gpu-layer-driving-modern-ai)
- [Rust for Machine Learning in 2025](https://markaicode.com/rust-machine-learning-framework-comparison-2025/)
- [PaddleOCR 3.0 High-Performance Inference](http://www.paddleocr.ai/main/en/version3.x/deployment/high_performance_inference.html)

### WASM Resources
- [WebAssembly 3.0 Performance: Rust vs C++ Benchmarks](https://markaicode.com/webassembly-3-performance-rust-cpp-benchmarks-2025/)
- [3W for In-Browser AI: WebLLM + WASM + WebWorkers](https://blog.mozilla.ai/3w-for-in-browser-ai-webllm-wasm-webworkers/)

### License Information
- [Rust API Guidelines: Licensing](https://rust-lang.github.io/api-guidelines/necessities.html)
- [PaddleOCR License](https://github.com/PaddlePaddle/PaddleOCR/blob/main/LICENSE) - Apache-2.0
- [ONNX Runtime License](https://github.com/microsoft/onnxruntime/blob/main/LICENSE) - MIT

---

**Document Version**: 1.0
**Last Updated**: 2025-11-28
**Author**: Research and Analysis Agent
**Status**: Complete

