# ADR-009: RVF WASM Runtime for Edge Deployment

## Status
Proposed

## Date
2026-02-28

## Context

### Current WASM State

The wifi-densepose-wasm crate provides basic WebAssembly bindings that expose Rust types to JavaScript. It enables browser-based visualization and lightweight inference but has significant limitations:

1. **No self-contained operation**: WASM module depends on external model files loaded via fetch(). If the server is unreachable, the module is useless.

2. **No persistent state**: Browser WASM has no built-in persistent storage for fingerprint databases, model weights, or session data.

3. **No offline capability**: Without network access, the WASM module cannot load models or send results.

4. **Binary size**: Current WASM bundle is not optimized. Full inference + signal processing compiles to ~5-15 MB.

### Edge Deployment Requirements

| Scenario | Platform | Constraints |
|----------|----------|------------|
| Browser dashboard | Chrome/Firefox | <10 MB download, no plugins |
| IoT sensor node | ESP32/Raspberry Pi | 256 KB - 4 GB RAM, battery powered |
| Mobile app | iOS/Android WebView | Limited background execution |
| Drone payload | Embedded Linux + WASM | Weight/power limited, intermittent connectivity |
| Field tablet | Android tablet | Offline operation in disaster zones |

### RuVector's Edge Runtime

RuVector provides a 5.5 KB WASM runtime that boots in 125ms, with:
- Self-contained operation (models + data embedded in RVF container)
- Persistent storage via RVF container (written to IndexedDB in browser, filesystem on native)
- Offline-first architecture
- SIMD acceleration when available (WASM SIMD proposal)

## Decision

We will replace the current wifi-densepose-wasm approach with an RVF-based edge runtime that packages models, fingerprint databases, and the inference engine into a single deployable RVF container.

### Edge Runtime Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                RVF Edge Deployment Container                      │
│                    (.rvf.edge file)                                │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐    │
│  │  WASM    │ │  VEC     │ │  INDEX   │ │  MODEL (ONNX)    │    │
│  │  Runtime │ │  CSI     │ │  HNSW    │ │  + LoRA deltas   │    │
│  │  (5.5KB) │ │  Finger- │ │  Graph   │ │                  │    │
│  │          │ │  prints  │ │          │ │                  │    │
│  └──────────┘ └──────────┘ └──────────┘ └──────────────────┘    │
│                                                                   │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────────┐    │
│  │  CRYPTO  │ │  WITNESS │ │  COW_MAP │ │  CONFIG          │    │
│  │  Keys    │ │  Audit   │ │  Branches│ │  Runtime params  │    │
│  │          │ │  Chain   │ │          │ │                  │    │
│  └──────────┘ └──────────┘ └──────────┘ └──────────────────┘    │
│                                                                   │
│  Total container: 1-50 MB depending on model + fingerprint size  │
└──────────────────────────────────────────────────────────────────┘
        │
        │ Deploy to:
        ▼
┌───────────────────────────────────────────────────────────────┐
│                                                                │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────────────┐ │
│  │ Browser │  │   IoT   │  │ Mobile  │  │ Disaster Field  │ │
│  │         │  │ Device  │  │  App    │  │    Tablet       │ │
│  │ IndexedDB  │ Flash   │  │ App     │  │ Local FS        │ │
│  │ for state│  │ for     │  │ Sandbox │  │ for state       │ │
│  │         │  │ state   │  │ for     │  │                 │ │
│  │         │  │         │  │ state   │  │                 │ │
│  └─────────┘  └─────────┘  └─────────┘  └─────────────────┘ │
└───────────────────────────────────────────────────────────────┘
```

### Tiered Runtime Profiles

Different deployment targets get different container configurations:

```rust
/// Edge runtime profiles
pub enum EdgeProfile {
    /// Full-featured browser deployment
    /// ~10 MB container, full inference + HNSW + SONA
    Browser {
        model_quantization: Quantization::Int8,
        max_fingerprints: 100_000,
        enable_sona: true,
        storage_backend: StorageBackend::IndexedDB,
    },

    /// Minimal IoT deployment
    /// ~1 MB container, lightweight inference only
    IoT {
        model_quantization: Quantization::Int4,
        max_fingerprints: 1_000,
        enable_sona: false,
        storage_backend: StorageBackend::Flash,
    },

    /// Mobile app deployment
    /// ~5 MB container, inference + HNSW, limited SONA
    Mobile {
        model_quantization: Quantization::Int8,
        max_fingerprints: 50_000,
        enable_sona: true,
        storage_backend: StorageBackend::AppSandbox,
    },

    /// Disaster field deployment (maximum capability)
    /// ~50 MB container, full stack including multi-AP consensus
    Field {
        model_quantization: Quantization::Float16,
        max_fingerprints: 1_000_000,
        enable_sona: true,
        storage_backend: StorageBackend::FileSystem,
    },
}
```

### Container Size Budget

| Segment | Browser | IoT | Mobile | Field |
|---------|---------|-----|--------|-------|
| WASM runtime | 5.5 KB | 5.5 KB | 5.5 KB | 5.5 KB |
| Model (ONNX) | 3 MB (int8) | 0.5 MB (int4) | 3 MB (int8) | 12 MB (fp16) |
| HNSW index | 4 MB | 100 KB | 2 MB | 40 MB |
| Fingerprint vectors | 2 MB | 50 KB | 1 MB | 10 MB |
| Config + crypto | 50 KB | 10 KB | 50 KB | 100 KB |
| **Total** | **~10 MB** | **~0.7 MB** | **~6 MB** | **~62 MB** |

### Offline-First Data Flow

```
┌────────────────────────────────────────────────────────────────────┐
│                    Offline-First Operation                          │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. BOOT (125ms)                                                   │
│     ├── Open RVF container from local storage                      │
│     ├── Memory-map WASM runtime segment                            │
│     ├── Load HNSW index into memory                                │
│     └── Initialize inference engine with embedded model            │
│                                                                     │
│  2. OPERATE (continuous)                                           │
│     ├── Receive CSI data from local hardware interface             │
│     ├── Process through local pipeline (no network needed)         │
│     ├── Search HNSW index against local fingerprints              │
│     ├── Run SONA adaptation on local data                          │
│     ├── Append results to local witness chain                      │
│     └── Store updated vectors to local container                   │
│                                                                     │
│  3. SYNC (when connected)                                          │
│     ├── Push new vectors to central RVF container                  │
│     ├── Pull updated fingerprints from other nodes                 │
│     ├── Merge SONA deltas via Raft (ADR-008)                      │
│     ├── Extend witness chain with cross-node attestation           │
│     └── Update local container with merged state                   │
│                                                                     │
│  4. SLEEP (battery conservation)                                   │
│     ├── Flush pending writes to container                          │
│     ├── Close memory-mapped segments                               │
│     └── Resume from step 1 on wake                                │
└────────────────────────────────────────────────────────────────────┘
```

### Browser-Specific Integration

```rust
/// Browser WASM entry point
#[wasm_bindgen]
pub struct WifiDensePoseEdge {
    container: RvfContainer,
    inference_engine: InferenceEngine,
    hnsw_index: HnswIndex,
    sona: Option<SonaAdapter>,
}

#[wasm_bindgen]
impl WifiDensePoseEdge {
    /// Initialize from an RVF container loaded via fetch or IndexedDB
    #[wasm_bindgen(constructor)]
    pub async fn new(container_bytes: &[u8]) -> Result<WifiDensePoseEdge, JsValue> {
        let container = RvfContainer::from_bytes(container_bytes)?;
        let engine = InferenceEngine::from_container(&container)?;
        let index = HnswIndex::from_container(&container)?;
        let sona = SonaAdapter::from_container(&container).ok();

        Ok(Self { container, inference_engine: engine, hnsw_index: index, sona })
    }

    /// Process a single CSI frame (called from JavaScript)
    #[wasm_bindgen]
    pub fn process_frame(&mut self, csi_json: &str) -> Result<String, JsValue> {
        let csi_data: CsiData = serde_json::from_str(csi_json)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let features = self.extract_features(&csi_data)?;
        let detection = self.detect(&features)?;
        let pose = if detection.human_detected {
            Some(self.estimate_pose(&features)?)
        } else {
            None
        };

        serde_json::to_string(&PoseResult { detection, pose })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Save current state to IndexedDB
    #[wasm_bindgen]
    pub async fn persist(&self) -> Result<(), JsValue> {
        let bytes = self.container.serialize()?;
        // Write to IndexedDB via web-sys
        save_to_indexeddb("wifi-densepose-state", &bytes).await
    }
}
```

### Model Quantization Strategy

| Quantization | Size Reduction | Accuracy Loss | Suitable For |
|-------------|---------------|---------------|-------------|
| Float32 (baseline) | 1x | 0% | Server/desktop |
| Float16 | 2x | <0.5% | Field tablets, GPUs |
| Int8 (PTQ) | 4x | <2% | Browser, mobile |
| Int4 (GPTQ) | 8x | <5% | IoT, ultra-constrained |
| Binary (1-bit) | 32x | ~15% | MCU/ultra-edge (experimental) |

## Consequences

### Positive
- **Single-file deployment**: Copy one `.rvf.edge` file to deploy anywhere
- **Offline operation**: Full functionality without network connectivity
- **125ms boot**: Near-instant readiness for emergency scenarios
- **Platform universal**: Same container format for browser, IoT, mobile, server
- **Battery efficient**: No network polling in offline mode

### Negative
- **Container size**: Even compressed, field containers are 50+ MB
- **WASM performance**: 2-5x slower than native Rust for compute-heavy operations
- **Browser limitations**: IndexedDB has storage quotas; WASM SIMD support varies
- **Update latency**: Offline devices miss updates until reconnection
- **Quantization accuracy**: Int4/Int8 models lose some detection sensitivity

## References

- [WebAssembly SIMD Proposal](https://github.com/WebAssembly/simd)
- [IndexedDB API](https://developer.mozilla.org/en-US/docs/Web/API/IndexedDB_API)
- [ONNX Runtime Web](https://onnxruntime.ai/docs/tutorials/web/)
- [Model Quantization Techniques](https://arxiv.org/abs/2103.13630)
- [RuVector WASM Runtime](https://github.com/ruvnet/ruvector)
- ADR-002: RuVector RVF Integration Strategy
- ADR-003: RVF Cognitive Containers for CSI Data
