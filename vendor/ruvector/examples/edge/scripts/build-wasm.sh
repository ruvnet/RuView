#!/bin/bash
set -e

echo "🦀 Building RuVector Edge WASM package..."

# Change to edge directory
cd "$(dirname "$0")/.."

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "📦 Installing wasm-pack..."
    cargo install wasm-pack
fi

# Build for web (ES modules)
echo "📦 Building for web target..."
wasm-pack build --target web --out-dir pkg --features wasm --no-default-features

# Copy package.json template (wasm-pack generates one but we override)
echo "📝 Updating package.json..."
cat > pkg/package.json << 'EOF'
{
  "name": "@ruvector/edge",
  "version": "0.1.0",
  "description": "WASM bindings for RuVector Edge - Distributed AI swarm communication with post-quantum crypto, HNSW indexing, and neural networks",
  "main": "ruvector_edge.js",
  "module": "ruvector_edge.js",
  "types": "ruvector_edge.d.ts",
  "sideEffects": [
    "./snippets/*"
  ],
  "keywords": [
    "wasm",
    "rust",
    "ai",
    "swarm",
    "p2p",
    "cryptography",
    "post-quantum",
    "hnsw",
    "vector-search",
    "neural-network",
    "consensus",
    "raft",
    "ed25519",
    "aes-gcm"
  ],
  "author": "RuVector Team",
  "license": "MIT",
  "repository": {
    "type": "git",
    "url": "https://github.com/ruvnet/ruvector"
  },
  "homepage": "https://github.com/ruvnet/ruvector/tree/main/examples/edge",
  "files": [
    "ruvector_edge_bg.wasm",
    "ruvector_edge.js",
    "ruvector_edge.d.ts",
    "ruvector_edge_bg.wasm.d.ts"
  ],
  "exports": {
    ".": {
      "import": "./ruvector_edge.js",
      "require": "./ruvector_edge.js",
      "types": "./ruvector_edge.d.ts"
    }
  }
}
EOF

# Create README for npm
echo "📝 Creating npm README..."
cat > pkg/README.md << 'EOF'
# @ruvector/edge

WASM bindings for RuVector Edge - the most advanced distributed AI swarm communication framework.

## Features

- 🔐 **Ed25519/X25519 Cryptography** - Identity signing and key exchange
- 🔒 **AES-256-GCM Encryption** - Authenticated encryption for all messages
- 🛡️ **Post-Quantum Signatures** - Hybrid Ed25519 + Dilithium-style defense
- 🔍 **HNSW Vector Index** - O(log n) approximate nearest neighbor search
- 🎯 **Semantic Task Matching** - Intelligent agent routing with LSH
- 🗳️ **Raft Consensus** - Distributed coordination and leader election
- 🧠 **Spiking Neural Networks** - Temporal pattern recognition with STDP
- 📊 **Vector Quantization** - 4-32x compression for bandwidth optimization

## Installation

```bash
npm install @ruvector/edge
```

## Usage

```typescript
import init, {
  WasmIdentity,
  WasmCrypto,
  WasmHnswIndex,
  WasmSemanticMatcher,
  WasmRaftNode,
  WasmQuantizer
} from '@ruvector/edge';

// Initialize WASM
await init();

// Create identity for signing
const identity = new WasmIdentity();
console.log('Public key:', identity.publicKeyHex());

// Sign and verify messages
const signature = identity.sign('Hello, World!');
const valid = WasmIdentity.verify(
  identity.publicKeyHex(),
  'Hello, World!',
  signature
);
console.log('Signature valid:', valid);

// HNSW vector search
const index = new WasmHnswIndex();
index.insert('agent-1', [0.9, 0.1, 0.0, 0.0]);
index.insert('agent-2', [0.1, 0.9, 0.0, 0.0]);
index.insert('agent-3', [0.0, 0.0, 0.9, 0.1]);

const results = index.search([0.8, 0.2, 0.0, 0.0], 2);
console.log('Nearest agents:', results);

// Semantic task matching
const matcher = new WasmSemanticMatcher();
matcher.registerAgent('rust-dev', 'rust cargo compile build test');
matcher.registerAgent('ml-eng', 'python pytorch tensorflow train model');

const match = matcher.matchAgent('build rust library with cargo');
console.log('Best match:', match);

// Quantization for compression
const vector = [0.1, 0.2, 0.3, 0.4, 0.5];
const quantized = WasmQuantizer.scalarQuantize(vector);
const reconstructed = WasmQuantizer.scalarDequantize(quantized);
console.log('Compression ratio: 4x');
```

## API Reference

### WasmIdentity
- `new()` - Create new identity with Ed25519/X25519 keys
- `publicKeyHex()` - Get Ed25519 public key as hex
- `x25519PublicKeyHex()` - Get X25519 public key as hex
- `sign(message)` - Sign message, returns signature hex
- `verify(pubkey, message, signature)` - Static verify method
- `generateNonce()` - Generate random nonce

### WasmCrypto
- `sha256(data)` - SHA-256 hash as hex
- `generateCid(data)` - Generate content ID
- `encrypt(data, keyHex)` - AES-256-GCM encrypt
- `decrypt(encrypted, keyHex)` - AES-256-GCM decrypt

### WasmHnswIndex
- `new()` / `withParams(m, ef)` - Create index
- `insert(id, vector)` - Add vector
- `search(query, k)` - Find k nearest neighbors

### WasmSemanticMatcher
- `registerAgent(id, capabilities)` - Register agent
- `matchAgent(task)` - Find best matching agent
- `matchTopK(task, k)` - Find top k matches

### WasmRaftNode
- `new(nodeId, members)` - Create Raft node
- `state()` / `term()` / `isLeader()` - Get state
- `startElection()` - Initiate leader election
- `appendEntry(data)` - Append to log (leader only)

### WasmQuantizer
- `binaryQuantize(vector)` - 32x compression
- `scalarQuantize(vector)` - 4x compression
- `scalarDequantize(quantized)` - Reconstruct vector
- `hammingDistance(a, b)` - Binary vector distance

## License

MIT License
EOF

echo "✅ Build complete! Package ready in ./pkg/"
echo ""
echo "To publish to npm:"
echo "  cd pkg && npm publish --access public"
echo ""
echo "To use locally:"
echo "  npm link ./pkg"
