# RuVector Edge-Net Model Optimization and Distribution System

## Architecture Document v1.0

### Executive Summary

This document defines a comprehensive model optimization and distribution system for RuVector Edge-Net, enabling browser-based AI inference with optimized models delivered via CDN and IPFS. The system supports end-user fine-tuning through MicroLoRA adapters and provides a complete CLI/SDK for model management.

---

## 1. System Architecture Overview

```
+==============================================================================+
|                    MODEL OPTIMIZATION & DISTRIBUTION SYSTEM                   |
+==============================================================================+
|                                                                              |
|  +---------------------------+     +---------------------------+             |
|  |   OPTIMIZATION PIPELINE   |     |   DISTRIBUTION SYSTEM     |             |
|  +---------------------------+     +---------------------------+             |
|  |                           |     |                           |             |
|  |  +---------------------+  |     |  +---------------------+  |             |
|  |  | Quantization Engine |  |     |  | Google Cloud CDN    |  |             |
|  |  | - INT8/INT4/FP16    |  |     |  | - Primary delivery  |  |             |
|  |  | - GPTQ/AWQ/GGML     |  |---->|  | - Global edge PoPs  |  |             |
|  |  +---------------------+  |     |  +---------------------+  |             |
|  |           |               |     |           |               |             |
|  |  +---------------------+  |     |  +---------------------+  |             |
|  |  | Pruning & Distill   |  |     |  | IPFS Gateway        |  |             |
|  |  | - Unstructured      |  |     |  | - Decentralized     |  |             |
|  |  | - Knowledge distill |  |     |  | - Content-addressed |  |             |
|  |  +---------------------+  |     |  +---------------------+  |             |
|  |           |               |     |           |               |             |
|  |  +---------------------+  |     |  +---------------------+  |             |
|  |  | ONNX Export         |  |     |  | Model Registry      |  |             |
|  |  | - Graph optimization|  |     |  | - Versioning        |  |             |
|  |  | - WASM compatible   |  |     |  | - Checksums/Sigs    |  |             |
|  |  +---------------------+  |     |  +---------------------+  |             |
|  |                           |     |                           |             |
|  +---------------------------+     +---------------------------+             |
|                                                                              |
|  +---------------------------+     +---------------------------+             |
|  |   MICROLORA SYSTEM        |     |   CLI & SDK               |             |
|  +---------------------------+     +---------------------------+             |
|  |                           |     |                           |             |
|  |  +---------------------+  |     |  +---------------------+  |             |
|  |  | Browser Training    |  |     |  | edge-net-models CLI |  |             |
|  |  | - WebGPU/WASM       |  |     |  | - pull/push/list    |  |             |
|  |  | - Rank 1-16 adapters|  |<--->|  | - optimize/verify   |  |             |
|  |  +---------------------+  |     |  +---------------------+  |             |
|  |           |               |     |           |               |             |
|  |  +---------------------+  |     |  +---------------------+  |             |
|  |  | Adapter Merging     |  |     |  | JavaScript SDK      |  |             |
|  |  | - TIES-Merging      |  |     |  | - ModelLoader       |  |             |
|  |  | - DARE weights      |  |     |  | - Cache/Fallbacks   |  |             |
|  |  +---------------------+  |     |  +---------------------+  |             |
|  |           |               |     |           |               |             |
|  |  +---------------------+  |     |  +---------------------+  |             |
|  |  | P2P Adapter Sharing |  |     |  | Model Loader        |  |             |
|  |  | - Reputation-gated  |  |     |  | - Progressive load  |  |             |
|  |  | - Domain-specific   |  |     |  | - Streaming decode  |  |             |
|  |  +---------------------+  |     |  +---------------------+  |             |
|  |                           |     |                           |             |
|  +---------------------------+     +---------------------------+             |
|                                                                              |
+==============================================================================+
```

---

## 2. Supported Models

### 2.1 Language Models (Text Generation)

| Model | Params | Quantized Size | Use Case |
|-------|--------|----------------|----------|
| **Phi-1.5** | 1.3B | 700MB (Q4) | Code generation, reasoning |
| **Phi-2** | 2.7B | 1.4GB (Q4) | General text, complex tasks |
| **Qwen-0.5B** | 0.5B | 280MB (Q4) | Fast inference, edge |
| **Gemma-2B** | 2B | 1.1GB (Q4) | General purpose |

### 2.2 Embedding Models (Semantic Search)

| Model | Dims | Quantized Size | Use Case |
|-------|------|----------------|----------|
| **MiniLM-L6-v2** | 384 | 23MB (Q8) | Fast semantic search |
| **E5-small** | 384 | 34MB (Q8) | High-quality embeddings |
| **BGE-small-en** | 384 | 34MB (Q8) | Retrieval-optimized |
| **GTE-small** | 384 | 34MB (Q8) | General text embeddings |

### 2.3 Vision Models (Future)

| Model | Params | Quantized Size | Use Case |
|-------|--------|----------------|----------|
| **SigLIP-small** | 86M | 45MB (Q8) | Image understanding |
| **MobileCLIP** | 65M | 35MB (Q8) | Fast image-text matching |

---

## 3. Model Optimization Pipeline

### 3.1 Quantization Engine

```
+--------------------------------------------------------------------------+
|                         QUANTIZATION PIPELINE                             |
+--------------------------------------------------------------------------+
|                                                                          |
|   Source Model (FP32/FP16)                                               |
|         |                                                                |
|         v                                                                |
|   +----------------+     +----------------+     +----------------+       |
|   | Calibration    |---->| Quantization   |---->| Validation     |       |
|   | Dataset        |     | Algorithm      |     | (Perplexity)   |       |
|   +----------------+     +----------------+     +----------------+       |
|         |                      |                      |                  |
|         v                      v                      v                  |
|   +-----------+          +-----------+          +-----------+           |
|   | WikiText  |          | INT8      |          | PPL < 10  |           |
|   | C4        |          | INT4      |          | Accuracy  |           |
|   | Custom    |          | FP16      |          | Latency   |           |
|   +-----------+          +-----------+          +-----------+           |
|                                |                                         |
|                                v                                         |
|                    +------------------------+                            |
|                    | Optimized ONNX Model   |                            |
|                    | - Quantized weights    |                            |
|                    | - Fused operators      |                            |
|                    | - WASM compatible      |                            |
|                    +------------------------+                            |
|                                                                          |
+--------------------------------------------------------------------------+
```

### 3.2 Quantization Formats

| Format | Bits | Memory Reduction | Speed | Quality |
|--------|------|------------------|-------|---------|
| **FP16** | 16 | 2x | 1.2x | ~100% |
| **INT8** | 8 | 4x | 1.5x | 99%+ |
| **INT4** | 4 | 8x | 2x | 95-98% |
| **GPTQ** | 4 | 8x | 1.8x | 98%+ |
| **AWQ** | 4 | 8x | 2x | 97%+ |
| **GGML Q4_K** | 4 | 8x | 2.2x | 96%+ |

### 3.3 Optimization Configuration

```typescript
interface OptimizationConfig {
  // Quantization settings
  quantization: {
    format: 'INT8' | 'INT4' | 'FP16' | 'GPTQ' | 'AWQ';
    calibrationSamples: number;        // 512 recommended
    groupSize: number;                  // 128 for 4-bit
    dampingFactor: number;              // 0.01 for GPTQ
    useActivationQuantization: boolean; // Dynamic INT8 activations
  };

  // Pruning settings (optional)
  pruning?: {
    method: 'magnitude' | 'movement' | 'sparsegpt';
    targetSparsity: number;             // 0.0-0.9
    blockSize: number;                  // 4 or 8
  };

  // ONNX export settings
  onnx: {
    opset: number;                      // 17 recommended
    dynamicAxes: boolean;               // For variable seq length
    simplify: boolean;                  // Graph optimization
    optimizationLevel: 'basic' | 'extended' | 'all';
  };

  // Target runtime
  target: 'browser' | 'node' | 'edge';

  // Output artifacts
  outputs: {
    model: boolean;                     // Main ONNX model
    tokenizer: boolean;                 // Tokenizer config
    config: boolean;                    // Model config
    adapters: boolean;                  // LoRA adapter slots
  };
}
```

---

## 4. Distribution System

### 4.1 Storage Architecture

```
+--------------------------------------------------------------------------+
|                      MULTI-TIER STORAGE ARCHITECTURE                      |
+--------------------------------------------------------------------------+
|                                                                          |
|   +------------------------+                                             |
|   |    MODEL REGISTRY      |  (PostgreSQL + Redis)                       |
|   |    - Metadata          |                                             |
|   |    - Versions          |                                             |
|   |    - Checksums         |                                             |
|   |    - Signatures        |                                             |
|   +------------------------+                                             |
|              |                                                           |
|              v                                                           |
|   +------------------------+    +------------------------+               |
|   |  PRIMARY: GCS CDN      |    |  BACKUP: IPFS          |               |
|   |  - Fastest delivery    |    |  - Decentralized       |               |
|   |  - Global edge caching |<-->|  - Content-addressed   |               |
|   |  - 99.99% SLA          |    |  - Censorship-resistant|               |
|   +------------------------+    +------------------------+               |
|              |                              |                            |
|              v                              v                            |
|   +------------------------+    +------------------------+               |
|   |  Browser Cache         |    |  P2P Cache (WebRTC)    |               |
|   |  - IndexedDB           |    |  - Peer-to-peer        |               |
|   |  - Service Worker      |    |  - BitTorrent-style    |               |
|   |  - Cache API           |    |  - Gossip protocol     |               |
|   +------------------------+    +------------------------+               |
|                                                                          |
+--------------------------------------------------------------------------+
```

### 4.2 CDN Configuration (Google Cloud Storage)

```yaml
# bucket: edge-net-models.storage.googleapis.com
storage:
  bucket: edge-net-models
  region: us-central1
  class: STANDARD  # Multi-region for hot data

cdn:
  enabled: true
  cacheMode: CACHE_ALL_STATIC
  defaultTtl: 86400        # 24 hours
  maxTtl: 2592000          # 30 days
  negativeCaching: true
  signedUrls: true         # For private models

compression:
  enabled: true
  types:
    - application/octet-stream  # ONNX models
    - application/json          # Tokenizers/configs

cors:
  origins:
    - "*"
  methods: ["GET", "HEAD", "OPTIONS"]
  responseHeaders:
    - Content-Type
    - Content-Length
    - Content-Encoding
    - X-Model-Version
    - X-Model-Checksum
```

### 4.3 IPFS Integration

```typescript
interface IPFSModelConfig {
  // IPFS gateway endpoints (fallback order)
  gateways: [
    'https://cloudflare-ipfs.com/ipfs/',
    'https://ipfs.io/ipfs/',
    'https://gateway.pinata.cloud/ipfs/',
    'https://w3s.link/ipfs/',
  ];

  // Pinning services
  pinning: {
    primary: 'pinata';    // Pinata Cloud
    backup: 'web3storage'; // web3.storage
  };

  // Content addressing
  cid: {
    version: 1;           // CIDv1
    codec: 'dag-pb';      // For chunked files
    hashAlg: 'sha2-256';
  };

  // Chunking for large models
  chunking: {
    enabled: true;
    chunkSize: 262144;    // 256KB chunks
    parallel: 4;          // Parallel downloads
  };
}
```

### 4.4 Model Manifest Format

```json
{
  "$schema": "https://edge-net.ruvector.dev/schemas/model-manifest-v1.json",
  "id": "phi-1.5-q4",
  "name": "Phi-1.5 INT4 Quantized",
  "version": "1.0.0",
  "type": "text-generation",
  "family": "phi",

  "architecture": {
    "type": "transformer",
    "layers": 24,
    "hiddenSize": 2048,
    "vocabSize": 51200,
    "contextLength": 2048
  },

  "optimization": {
    "quantization": "INT4",
    "format": "GPTQ",
    "groupSize": 128,
    "originalSize": "2.7GB",
    "optimizedSize": "700MB"
  },

  "artifacts": {
    "model": {
      "filename": "model.onnx",
      "size": 734003200,
      "sha256": "a3f8c2d1e4b5...",
      "signature": "MEUCIQDx..."
    },
    "tokenizer": {
      "filename": "tokenizer.json",
      "size": 2500000,
      "sha256": "b4e9d3c2a1f6..."
    },
    "config": {
      "filename": "config.json",
      "size": 1024,
      "sha256": "c5f0e4d3b2a7..."
    }
  },

  "distribution": {
    "primary": {
      "type": "gcs",
      "baseUrl": "https://storage.googleapis.com/edge-net-models/phi-1.5-q4/v1.0.0/",
      "region": "us-central1"
    },
    "fallback": {
      "type": "ipfs",
      "cid": "bafybeig5yh2lj3k4m5n6o7p8q9r0s1t2u3v4w5x6y7z8a9b0c1d2e3f4g5h6",
      "gateways": ["cloudflare-ipfs.com", "ipfs.io"]
    }
  },

  "requirements": {
    "minMemory": 1073741824,   // 1GB
    "simdRequired": false,
    "webgpuOptional": true,
    "browserSupport": ["chrome-88", "firefox-89", "safari-15"]
  },

  "adapters": {
    "baseRank": 8,
    "microRank": 2,
    "slots": ["reasoning", "coding", "chat", "custom"]
  },

  "benchmarks": {
    "perplexity": 9.2,
    "tokensPerSecond": {
      "browser": 12,
      "node": 45,
      "webgpu": 85
    },
    "memoryUsage": 850000000
  },

  "license": "MIT",
  "homepage": "https://github.com/ruvnet/ruvector",
  "publishedAt": "2026-01-03T00:00:00Z",
  "publishedBy": "ruvector-team"
}
```

---

## 5. MicroLoRA Customization System

### 5.1 Architecture Overview

```
+--------------------------------------------------------------------------+
|                         MICROLORA CUSTOMIZATION                           |
+--------------------------------------------------------------------------+
|                                                                          |
|   +----------------------------+                                         |
|   |     BROWSER TRAINING       |                                         |
|   +----------------------------+                                         |
|   |                            |                                         |
|   |  User Data                 |                                         |
|   |    |                       |                                         |
|   |    v                       |                                         |
|   |  +----------------------+  |                                         |
|   |  | Training Pipeline    |  |     +-------------------+               |
|   |  | - WebGPU/WASM        |  |     | Adapter Storage   |               |
|   |  | - Gradient accum     |  |---->| - IndexedDB       |               |
|   |  | - Low-rank updates   |  |     | - P2P sharing     |               |
|   |  +----------------------+  |     +-------------------+               |
|   |           |                |              |                          |
|   |           v                |              v                          |
|   |  +----------------------+  |     +-------------------+               |
|   |  | LoRA Adapter         |  |     | Adapter Pool      |               |
|   |  | - Rank 1-16          |  |     | - LRU eviction    |               |
|   |  | - ~1-10MB per adapter|  |<----| - 16 slots max    |               |
|   |  +----------------------+  |     +-------------------+               |
|   |           |                |              |                          |
|   +-----------|----------------+              |                          |
|               v                               v                          |
|   +----------------------------+    +-------------------+                |
|   |     ADAPTER MERGING        |    | P2P SHARING       |                |
|   +----------------------------+    +-------------------+                |
|   |                            |    |                   |                |
|   |  +----------------------+  |    | - Reputation gate |                |
|   |  | TIES-Merging         |  |    | - Domain-specific |                |
|   |  | - Sign agreement     |  |    | - Version control |                |
|   |  | - Trim & scale       |  |    | - Quality scores  |                |
|   |  +----------------------+  |    +-------------------+                |
|   |           |                |                                         |
|   |           v                |                                         |
|   |  +----------------------+  |                                         |
|   |  | Merged Adapter       |  |                                         |
|   |  | - Combined expertise |  |                                         |
|   |  | - Optimized weights  |  |                                         |
|   |  +----------------------+  |                                         |
|   |                            |                                         |
|   +----------------------------+                                         |
|                                                                          |
+--------------------------------------------------------------------------+
```

### 5.2 MicroLoRA Training Configuration

```typescript
interface MicroLoRAConfig {
  // Adapter architecture
  adapter: {
    rank: number;                // 1-16, typically 2-8
    alpha: number;               // Scaling factor (rank * 2 typical)
    dropout: number;             // 0.0-0.1
    targetModules: string[];     // ['q_proj', 'v_proj', 'k_proj', 'o_proj']
  };

  // Training hyperparameters
  training: {
    batchSize: number;           // 1-8 for browser
    gradientAccumulation: number;// 4-16 to simulate larger batch
    learningRate: number;        // 1e-4 to 1e-3
    warmupSteps: number;         // 10% of total
    maxSteps: number;            // 100-1000 for quick adapt
    optimizer: 'adamw' | 'sgd';
    scheduler: 'cosine' | 'linear' | 'constant';
  };

  // Memory optimization
  memory: {
    gradientCheckpointing: boolean;
    mixedPrecision: boolean;     // FP16 forward, FP32 gradients
    offloadOptimizer: boolean;   // Use IndexedDB for optimizer state
  };

  // Compute backend
  backend: 'wasm' | 'webgpu' | 'auto';

  // Data settings
  data: {
    maxLength: number;           // Max sequence length
    maskProbability: number;     // For MLM tasks
    shuffleBuffer: number;       // Data shuffling
  };
}
```

### 5.3 Adapter Format

```typescript
interface LoRAAdapter {
  // Metadata
  id: string;                    // UUID
  name: string;                  // Human-readable name
  description: string;
  version: string;
  createdAt: string;

  // Architecture
  baseModel: string;             // e.g., 'phi-1.5-q4'
  rank: number;
  alpha: number;
  targetModules: string[];

  // Weights (per target module)
  weights: {
    [moduleName: string]: {
      lora_A: Float32Array;      // Down projection
      lora_B: Float32Array;      // Up projection
    };
  };

  // Training info
  training: {
    steps: number;
    finalLoss: number;
    samples: number;
    duration: number;            // ms
  };

  // Quality metrics
  quality: {
    perplexityDelta: number;     // Change from base
    taskAccuracy?: number;       // If evaluated
    humanScore?: number;         // 1-5 rating
  };

  // Sharing
  sharing: {
    public: boolean;
    domain: string;              // 'general' | 'code' | 'legal' | etc.
    license: string;
  };

  // Integrity
  checksum: string;              // SHA-256 of weights
  signature?: string;            // Ed25519 signature
}
```

### 5.4 Adapter Merging Algorithms

```typescript
// TIES-Merging: Trim, Elect Sign, Disjoint Merge
interface TIESMergeConfig {
  method: 'ties';
  trimFraction: number;          // 0.1-0.3, remove low-magnitude deltas
  electSign: 'majority' | 'weighted';
  scalingCoef: number;           // 0.5-1.0
}

// DARE: Drop And Rescale
interface DAREMergeConfig {
  method: 'dare';
  dropRate: number;              // 0.1-0.5
  rescale: boolean;
}

// Task Arithmetic
interface TaskArithmeticConfig {
  method: 'task_arithmetic';
  weights: { [adapterId: string]: number };  // Weighted combination
}

function mergeAdapters(
  adapters: LoRAAdapter[],
  config: TIESMergeConfig | DAREMergeConfig | TaskArithmeticConfig
): LoRAAdapter {
  // Implementation based on method
  // Returns merged adapter with combined expertise
}
```

---

## 6. CLI: edge-net-models

### 6.1 Command Reference

```bash
# Installation
npm install -g @ruvector/edge-net-models

# ======== Model Management ========

# List available models
edge-net-models list
edge-net-models list --type embedding
edge-net-models list --quantization INT4

# Pull a model
edge-net-models pull phi-1.5-q4
edge-net-models pull phi-1.5-q4 --version 1.0.0
edge-net-models pull phi-1.5-q4 --source ipfs    # Use IPFS instead of CDN
edge-net-models pull phi-1.5-q4 --output ./models/

# Verify model integrity
edge-net-models verify phi-1.5-q4
edge-net-models verify phi-1.5-q4 --check-signature

# Show model info
edge-net-models info phi-1.5-q4
edge-net-models info phi-1.5-q4 --benchmarks
edge-net-models info phi-1.5-q4 --requirements

# ======== Model Optimization ========

# Quantize a model
edge-net-models optimize ./my-model.onnx \
  --quantization INT4 \
  --calibration-data ./calibration.jsonl \
  --output ./my-model-q4.onnx

# Full optimization pipeline
edge-net-models optimize ./my-model.onnx \
  --quantization INT4 \
  --pruning magnitude --sparsity 0.3 \
  --onnx-simplify \
  --target browser

# ======== Model Publishing ========

# Publish to registry
edge-net-models publish ./my-model-q4.onnx \
  --name "my-custom-model" \
  --version 1.0.0 \
  --type text-generation

# Sign model
edge-net-models sign ./my-model-q4.onnx \
  --key ./private-key.pem

# ======== Adapter Management ========

# List local adapters
edge-net-models adapters list

# Export adapter
edge-net-models adapters export my-adapter --output ./adapter.json

# Import adapter
edge-net-models adapters import ./adapter.json

# Share adapter to P2P network
edge-net-models adapters share my-adapter --domain coding

# Merge adapters
edge-net-models adapters merge adapter1 adapter2 \
  --method ties \
  --output merged-adapter

# ======== Cache Management ========

# Show cache info
edge-net-models cache info

# Clear cache
edge-net-models cache clear
edge-net-models cache clear --older-than 30d

# Prune unused models
edge-net-models cache prune

# ======== Configuration ========

# Show config
edge-net-models config show

# Set default source
edge-net-models config set defaultSource gcs
edge-net-models config set cacheDir ~/.edge-net/models
```

### 6.2 CLI Configuration File

```yaml
# ~/.edge-net/models.yaml
version: 1

# Default model source
defaultSource: gcs

# Cache settings
cache:
  directory: ~/.edge-net/models
  maxSize: 10GB
  autoCleanup: true

# CDN settings
cdn:
  endpoint: https://storage.googleapis.com/edge-net-models
  timeout: 30000
  retries: 3

# IPFS settings
ipfs:
  gateways:
    - https://cloudflare-ipfs.com/ipfs/
    - https://ipfs.io/ipfs/
  parallel: 4
  timeout: 60000

# Model preferences
preferences:
  preferQuantization: INT4
  preferSmallModels: true
  autoUpdate: true

# Signing key (for publishing)
signing:
  keyPath: ~/.edge-net/keys/private.pem
  keyId: ruvector-team
```

---

## 7. JavaScript SDK

### 7.1 ModelLoader API

```typescript
import { ModelLoader, ModelConfig } from '@ruvector/edge-net-models';

// ======== Basic Usage ========

// Create loader with default settings
const loader = new ModelLoader();

// Load a model
const model = await loader.load('phi-1.5-q4');

// Use the model
const output = await model.generate('Hello, world!', {
  maxTokens: 100,
  temperature: 0.7,
});

// ======== Advanced Configuration ========

const loader = new ModelLoader({
  // Cache settings
  cache: {
    enabled: true,
    storage: 'indexeddb',        // 'indexeddb' | 'memory' | 'filesystem'
    maxSize: 2 * 1024 * 1024 * 1024,  // 2GB
  },

  // Source preferences
  sources: {
    primary: 'gcs',              // Google Cloud Storage CDN
    fallback: ['ipfs', 'p2p'],   // Fallback sources
    timeout: 30000,
  },

  // Download settings
  download: {
    parallel: 4,                  // Parallel chunk downloads
    retries: 3,
    progressCallback: (progress) => {
      console.log(`${progress.loaded}/${progress.total} bytes`);
    },
  },

  // Compute backend
  backend: 'auto',               // 'wasm' | 'webgpu' | 'auto'

  // Memory management
  memory: {
    maxModels: 2,                // Max models in memory
    evictionPolicy: 'lru',       // 'lru' | 'fifo' | 'priority'
  },
});

// ======== Model Loading ========

// Load with specific version
const model = await loader.load('phi-1.5-q4', { version: '1.0.0' });

// Load with adapter
const model = await loader.load('phi-1.5-q4', {
  adapter: 'coding-assistant',
});

// Load multiple models
const [llm, embedder] = await Promise.all([
  loader.load('phi-1.5-q4'),
  loader.load('minilm-l6-v2-q8'),
]);

// ======== Caching ========

// Check if model is cached
const isCached = await loader.isCached('phi-1.5-q4');

// Get cached models
const cachedModels = await loader.getCachedModels();

// Preload model
await loader.preload('phi-1.5-q4');

// Clear specific model from cache
await loader.clearCache('phi-1.5-q4');

// Clear all cache
await loader.clearAllCache();

// ======== Model Info ========

// Get model manifest
const manifest = await loader.getManifest('phi-1.5-q4');

// Get available models
const models = await loader.listModels({
  type: 'text-generation',
  quantization: 'INT4',
});

// Check requirements
const compatible = await loader.checkCompatibility('phi-1.5-q4');
```

### 7.2 InferenceSession API

```typescript
import { InferenceSession, GenerateConfig } from '@ruvector/edge-net-models';

// ======== Text Generation ========

const session = await InferenceSession.create('phi-1.5-q4');

// Simple generation
const text = await session.generate('Write a haiku about AI:');

// Streaming generation
const stream = session.generateStream('Once upon a time');
for await (const token of stream) {
  process.stdout.write(token);
}

// Generation with config
const text = await session.generate('Explain quantum computing:', {
  maxTokens: 200,
  temperature: 0.7,
  topP: 0.9,
  topK: 40,
  repetitionPenalty: 1.1,
  stopSequences: ['\n\n', '###'],
});

// Chat completion
const response = await session.chat([
  { role: 'system', content: 'You are a helpful assistant.' },
  { role: 'user', content: 'What is 2+2?' },
]);

// ======== Embeddings ========

const embedder = await InferenceSession.create('minilm-l6-v2-q8');

// Single embedding
const embedding = await embedder.embed('Hello world');

// Batch embeddings
const embeddings = await embedder.embedBatch([
  'Hello world',
  'How are you?',
  'Nice to meet you',
]);

// Similarity
const similarity = await embedder.similarity(
  'king is to queen',
  'man is to woman'
);

// ======== MicroLoRA Integration ========

// Enable adapter
await session.loadAdapter('coding-assistant');

// Train adapter on custom data
const trainer = session.createTrainer({
  rank: 4,
  learningRate: 1e-4,
  batchSize: 2,
});

await trainer.train([
  { input: 'Write a Python function', output: 'def hello():...' },
  { input: 'Create a class', output: 'class MyClass:...' },
]);

// Save adapter
const adapter = await trainer.getAdapter();
await adapter.save('my-coding-adapter');

// ======== Memory Management ========

// Get memory usage
const memoryInfo = session.getMemoryInfo();
console.log(`Model memory: ${memoryInfo.modelBytes / 1024 / 1024}MB`);
console.log(`KV cache: ${memoryInfo.kvCacheBytes / 1024 / 1024}MB`);

// Clear KV cache
session.clearKVCache();

// Dispose session
session.dispose();
```

### 7.3 Web Worker Integration

```typescript
import { ModelWorker } from '@ruvector/edge-net-models/worker';

// ======== Main Thread ========

const worker = new ModelWorker();

// Load model in worker
await worker.load('phi-1.5-q4');

// Generate in worker (non-blocking)
const result = await worker.generate('Hello!', { maxTokens: 50 });

// Streaming from worker
worker.onToken((token) => console.log(token));
await worker.generateStream('Once upon a time');

// Batch processing in worker
const results = await worker.batch([
  { id: '1', prompt: 'Hello' },
  { id: '2', prompt: 'World' },
]);

// Terminate worker
worker.terminate();

// ======== Worker Pool ========

import { ModelWorkerPool } from '@ruvector/edge-net-models/worker';

const pool = new ModelWorkerPool({
  workers: 4,           // Number of workers
  model: 'phi-1.5-q4',  // Pre-load model
});

// Parallel inference
const results = await Promise.all([
  pool.generate('Task 1'),
  pool.generate('Task 2'),
  pool.generate('Task 3'),
  pool.generate('Task 4'),
]);

pool.terminate();
```

---

## 8. File Structure

```
examples/edge-net/
|-- models/                          # Model optimization & distribution
|   |-- src/
|   |   |-- optimization/
|   |   |   |-- mod.rs               # Optimization pipeline
|   |   |   |-- quantization.rs      # INT8/INT4/FP16 quantization
|   |   |   |-- pruning.rs           # Magnitude/movement pruning
|   |   |   |-- distillation.rs      # Knowledge distillation
|   |   |   |-- onnx_export.rs       # ONNX conversion & optimization
|   |   |   |-- calibration.rs       # Calibration dataset handling
|   |   |   `-- validation.rs        # Quality validation
|   |   |
|   |   |-- distribution/
|   |   |   |-- mod.rs               # Distribution system
|   |   |   |-- gcs.rs               # Google Cloud Storage CDN
|   |   |   |-- ipfs.rs              # IPFS integration
|   |   |   |-- registry.rs          # Model registry
|   |   |   |-- manifest.rs          # Model manifest handling
|   |   |   |-- integrity.rs         # Checksums & signatures
|   |   |   `-- p2p.rs               # P2P model distribution
|   |   |
|   |   |-- lora/
|   |   |   |-- mod.rs               # MicroLoRA system
|   |   |   |-- adapter.rs           # LoRA adapter types
|   |   |   |-- training.rs          # Browser-based training
|   |   |   |-- merging.rs           # TIES/DARE merging
|   |   |   |-- pool.rs              # Adapter pool management
|   |   |   `-- sharing.rs           # P2P adapter sharing
|   |   |
|   |   |-- loader/
|   |   |   |-- mod.rs               # Model loader
|   |   |   |-- cache.rs             # Caching system
|   |   |   |-- streaming.rs         # Streaming decode
|   |   |   `-- fallback.rs          # Fallback handling
|   |   |
|   |   `-- lib.rs                   # Library entry point
|   |
|   |-- cli/
|   |   |-- src/
|   |   |   |-- main.rs              # CLI entry point
|   |   |   |-- commands/
|   |   |   |   |-- mod.rs           # Command handlers
|   |   |   |   |-- pull.rs          # Pull model
|   |   |   |   |-- push.rs          # Push model
|   |   |   |   |-- list.rs          # List models
|   |   |   |   |-- optimize.rs      # Optimize model
|   |   |   |   |-- verify.rs        # Verify integrity
|   |   |   |   |-- adapters.rs      # Adapter management
|   |   |   |   `-- cache.rs         # Cache management
|   |   |   |-- config.rs            # CLI configuration
|   |   |   `-- output.rs            # Output formatting
|   |   |-- Cargo.toml
|   |   `-- README.md
|   |
|   |-- sdk/
|   |   |-- package.json
|   |   |-- tsconfig.json
|   |   |-- src/
|   |   |   |-- index.ts             # SDK entry point
|   |   |   |-- loader.ts            # ModelLoader class
|   |   |   |-- session.ts           # InferenceSession class
|   |   |   |-- cache.ts             # Browser caching
|   |   |   |-- sources/
|   |   |   |   |-- gcs.ts           # GCS CDN source
|   |   |   |   |-- ipfs.ts          # IPFS source
|   |   |   |   `-- p2p.ts           # P2P source
|   |   |   |-- adapters/
|   |   |   |   |-- trainer.ts       # Browser training
|   |   |   |   |-- merger.ts        # Adapter merging
|   |   |   |   `-- pool.ts          # Adapter pool
|   |   |   |-- worker/
|   |   |   |   |-- index.ts         # Worker exports
|   |   |   |   |-- model-worker.ts  # Model worker class
|   |   |   |   |-- pool.ts          # Worker pool
|   |   |   |   `-- inference.worker.ts  # Worker implementation
|   |   |   `-- types.ts             # TypeScript types
|   |   `-- README.md
|   |
|   |-- manifests/                   # Model manifests
|   |   |-- phi-1.5-q4.json
|   |   |-- phi-2-q4.json
|   |   |-- qwen-0.5b-q4.json
|   |   |-- gemma-2b-q4.json
|   |   |-- minilm-l6-v2-q8.json
|   |   |-- e5-small-q8.json
|   |   `-- bge-small-q8.json
|   |
|   |-- Cargo.toml
|   `-- README.md
|
|-- src/
|   |-- ai/
|   |   |-- models/
|   |   |   |-- mod.rs               # Model integration
|   |   |   |-- inference.rs         # Inference engine
|   |   |   `-- tokenizer.rs         # Tokenization
|   |   |-- sona/
|   |   |   |-- lora.rs              # MicroLoRA (existing)
|   |   |   `-- ...
|   |   `-- ...
|   `-- ...
|
`-- ...
```

---

## 9. API Contracts

### 9.1 Model Registry API

```yaml
openapi: 3.0.3
info:
  title: Edge-Net Model Registry API
  version: 1.0.0

paths:
  /v1/models:
    get:
      summary: List available models
      parameters:
        - name: type
          in: query
          schema:
            type: string
            enum: [text-generation, embedding, vision]
        - name: quantization
          in: query
          schema:
            type: string
            enum: [FP16, INT8, INT4]
      responses:
        '200':
          description: List of models
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/ModelSummary'

  /v1/models/{modelId}:
    get:
      summary: Get model details
      parameters:
        - name: modelId
          in: path
          required: true
          schema:
            type: string
      responses:
        '200':
          description: Model manifest
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/ModelManifest'

  /v1/models/{modelId}/download:
    get:
      summary: Get download URLs
      parameters:
        - name: modelId
          in: path
          required: true
          schema:
            type: string
        - name: source
          in: query
          schema:
            type: string
            enum: [gcs, ipfs]
            default: gcs
      responses:
        '200':
          description: Download URLs
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/DownloadUrls'

  /v1/adapters:
    get:
      summary: List public adapters
      parameters:
        - name: baseModel
          in: query
          schema:
            type: string
        - name: domain
          in: query
          schema:
            type: string
      responses:
        '200':
          description: List of adapters
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/AdapterSummary'

    post:
      summary: Share adapter
      requestBody:
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/AdapterUpload'
      responses:
        '201':
          description: Adapter shared
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/AdapterSummary'

components:
  schemas:
    ModelSummary:
      type: object
      properties:
        id:
          type: string
        name:
          type: string
        type:
          type: string
        quantization:
          type: string
        size:
          type: integer
        version:
          type: string

    ModelManifest:
      type: object
      # Full manifest as defined in section 4.4

    DownloadUrls:
      type: object
      properties:
        model:
          type: string
          format: uri
        tokenizer:
          type: string
          format: uri
        config:
          type: string
          format: uri
        signature:
          type: string
          format: uri

    AdapterSummary:
      type: object
      properties:
        id:
          type: string
        name:
          type: string
        baseModel:
          type: string
        rank:
          type: integer
        domain:
          type: string
        quality:
          type: number
        downloads:
          type: integer
```

### 9.2 P2P Adapter Protocol

```protobuf
syntax = "proto3";
package edgenet.adapters;

// Adapter discovery message
message AdapterAnnounce {
  string adapter_id = 1;
  string name = 2;
  string base_model = 3;
  uint32 rank = 4;
  string domain = 5;
  float quality_score = 6;
  bytes checksum = 7;
  bytes signature = 8;
  string peer_id = 9;
  uint64 timestamp = 10;
}

// Adapter request
message AdapterRequest {
  string adapter_id = 1;
  string requester_id = 2;
  uint64 timestamp = 3;
}

// Adapter response (chunked for large adapters)
message AdapterChunk {
  string adapter_id = 1;
  uint32 chunk_index = 2;
  uint32 total_chunks = 3;
  bytes data = 4;
  bytes chunk_hash = 5;
}

// Adapter quality report
message AdapterReport {
  string adapter_id = 1;
  string reporter_id = 2;
  float quality_score = 3;
  string feedback = 4;
  uint64 timestamp = 5;
  bytes signature = 6;
}
```

---

## 10. Security Considerations

### 10.1 Model Integrity

```
+--------------------------------------------------------------------------+
|                         MODEL INTEGRITY CHAIN                             |
+--------------------------------------------------------------------------+
|                                                                          |
|   Publisher                                                              |
|       |                                                                  |
|       v                                                                  |
|   +------------------------+                                             |
|   | 1. Generate Checksums  |                                             |
|   |    - SHA-256 per file  |                                             |
|   |    - Merkle root       |                                             |
|   +------------------------+                                             |
|              |                                                           |
|              v                                                           |
|   +------------------------+                                             |
|   | 2. Sign Manifest       |                                             |
|   |    - Ed25519 signature |                                             |
|   |    - Publisher identity|                                             |
|   +------------------------+                                             |
|              |                                                           |
|              v                                                           |
|   +------------------------+                                             |
|   | 3. Upload to CDN/IPFS  |                                             |
|   |    - Model artifacts   |                                             |
|   |    - Signed manifest   |                                             |
|   +------------------------+                                             |
|              |                                                           |
|              v                                                           |
|   Consumer                                                               |
|       |                                                                  |
|       v                                                                  |
|   +------------------------+                                             |
|   | 4. Download & Verify   |                                             |
|   |    - Check checksums   |                                             |
|   |    - Verify signature  |                                             |
|   |    - Validate manifest |                                             |
|   +------------------------+                                             |
|              |                                                           |
|              v                                                           |
|   +------------------------+                                             |
|   | 5. Load Model          |                                             |
|   |    - If valid: proceed |                                             |
|   |    - If invalid: abort |                                             |
|   +------------------------+                                             |
|                                                                          |
+--------------------------------------------------------------------------+
```

### 10.2 Adapter Security

- **Reputation-gated sharing**: Only nodes with reputation > 0.7 can share adapters
- **Quality thresholds**: Adapters must meet minimum quality scores
- **Signature verification**: All shared adapters must be signed
- **Sandboxed training**: Browser training runs in isolated context
- **Rate limiting**: Max 10 adapter shares per hour per node

### 10.3 WASM Sandbox

- Models run in isolated WASM sandbox
- No network access from inference
- Memory limits enforced
- Execution time limits
- No filesystem access

---

## 11. Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| **Model Load Time** | < 5s (cached), < 30s (download) | For 1GB model |
| **First Token Latency** | < 100ms (WASM), < 50ms (WebGPU) | After model loaded |
| **Tokens/Second** | 10-15 (WASM), 50-100 (WebGPU) | Phi-1.5 Q4 |
| **Memory Usage** | < 1.5x model size | Including KV cache |
| **Adapter Training** | 100 samples in < 5 min | Rank-4 adapter |
| **Adapter Load Time** | < 100ms | From IndexedDB |
| **Cache Hit Rate** | > 95% | After initial load |
| **CDN Latency** | < 50ms | To nearest PoP |

---

## 12. Future Enhancements

### Phase 2 (Q2 2026)
- Vision model support (SigLIP, MobileCLIP)
- WebGPU acceleration for all models
- Federated adapter training across P2P network
- Model sharding for very large models

### Phase 3 (Q3 2026)
- Mixture-of-Experts (MoE) support
- Speculative decoding
- Cross-model adapter transfer
- On-device model distillation

### Phase 4 (Q4 2026)
- Multimodal models (text + image)
- Audio models (Whisper variants)
- Real-time streaming inference
- Hardware acceleration (WebNN)

---

## Appendix A: Quantization Details

### A.1 INT4 Quantization Algorithm

```python
def quantize_int4_gptq(weights, calibration_data):
    """
    GPTQ-style INT4 quantization with optimal rounding.

    1. Compute Hessian approximation from calibration data
    2. For each weight column:
       a. Quantize to nearest INT4 value
       b. Compute quantization error
       c. Update remaining weights to minimize error
    """
    H = compute_hessian(weights, calibration_data)

    for i in range(weights.shape[1]):
        # Optimal rounding
        q = round(weights[:, i] / scale)
        q = clip(q, -8, 7)  # INT4 range

        # Error compensation (Cholesky update)
        error = (weights[:, i] - q * scale) / H[i, i]
        weights[:, i+1:] -= error * H[i, i+1:]

    return quantized_weights, scales, zeros
```

### A.2 Calibration Dataset Format

```jsonl
{"text": "The quick brown fox jumps over the lazy dog."}
{"text": "Machine learning models can be quantized for efficiency."}
{"text": "WebAssembly enables near-native performance in browsers."}
...
```

---

## Appendix B: IPFS Content Addressing

### B.1 Model CID Computation

```javascript
// Compute CID for model file
import { CID } from 'multiformats/cid';
import { sha256 } from 'multiformats/hashes/sha2';
import * as dagPB from '@ipld/dag-pb';

async function computeModelCID(modelBytes) {
  // Chunk the model (256KB chunks)
  const chunks = chunkify(modelBytes, 262144);

  // Build Merkle DAG
  const links = [];
  for (const chunk of chunks) {
    const hash = await sha256.digest(chunk);
    const cid = CID.create(1, dagPB.code, hash);
    links.push({ Hash: cid, Tsize: chunk.length });
  }

  // Create root node
  const node = dagPB.createNode(new Uint8Array(), links);
  const rootHash = await sha256.digest(dagPB.encode(node));

  return CID.create(1, dagPB.code, rootHash);
}
```

---

## Appendix C: Signature Verification

### C.1 Ed25519 Signature Format

```typescript
interface ModelSignature {
  // Signed data
  manifest: {
    id: string;
    version: string;
    artifacts: {
      [name: string]: {
        sha256: string;
        size: number;
      };
    };
    timestamp: string;
  };

  // Signature
  signature: string;    // Base64-encoded Ed25519 signature
  publicKey: string;    // Base64-encoded public key
  keyId: string;        // Key identifier (e.g., 'ruvector-team')
}

// Verification
async function verifyModelSignature(sig: ModelSignature): Promise<boolean> {
  const publicKey = base64ToBytes(sig.publicKey);
  const signature = base64ToBytes(sig.signature);
  const message = new TextEncoder().encode(JSON.stringify(sig.manifest));

  return ed25519.verify(signature, message, publicKey);
}
```

---

*Document Version: 1.0*
*Last Updated: 2026-01-03*
*Authors: RuVector Team*
