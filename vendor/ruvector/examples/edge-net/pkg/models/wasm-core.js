/**
 * @ruvector/edge-net WASM Core
 *
 * Pure WASM implementation for cross-platform edge support:
 * - Browsers (Chrome, Firefox, Safari, Edge)
 * - Node.js 16+
 * - Deno
 * - Cloudflare Workers
 * - Vercel Edge
 * - Bun
 *
 * Uses ruvector_edge_net WASM module for:
 * - Ed25519 signing/verification
 * - SHA256/SHA512 hashing
 * - Merkle tree operations
 * - Canonical JSON encoding
 *
 * @module @ruvector/edge-net/models/wasm-core
 */

// ============================================================================
// PLATFORM DETECTION
// ============================================================================

/**
 * Detect current runtime environment
 */
export function detectPlatform() {
    // Cloudflare Workers
    if (typeof caches !== 'undefined' && typeof HTMLRewriter !== 'undefined') {
        return 'cloudflare-workers';
    }

    // Deno
    if (typeof Deno !== 'undefined') {
        return 'deno';
    }

    // Bun
    if (typeof Bun !== 'undefined') {
        return 'bun';
    }

    // Node.js
    if (typeof process !== 'undefined' && process.versions?.node) {
        return 'node';
    }

    // Browser with WebAssembly
    if (typeof window !== 'undefined' && typeof WebAssembly !== 'undefined') {
        return 'browser';
    }

    // Generic WebAssembly environment
    if (typeof WebAssembly !== 'undefined') {
        return 'wasm';
    }

    return 'unknown';
}

/**
 * Platform capabilities
 */
export function getPlatformCapabilities() {
    const platform = detectPlatform();

    return {
        platform,
        hasWebAssembly: typeof WebAssembly !== 'undefined',
        hasSIMD: checkSIMDSupport(),
        hasThreads: checkThreadsSupport(),
        hasStreaming: typeof WebAssembly?.compileStreaming === 'function',
        hasIndexedDB: typeof indexedDB !== 'undefined',
        hasWebCrypto: typeof crypto?.subtle !== 'undefined',
        hasP2P: typeof RTCPeerConnection !== 'undefined',
        maxMemory: getMaxMemory(),
    };
}

function checkSIMDSupport() {
    try {
        return WebAssembly.validate(new Uint8Array([
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
            0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7b, 0x03,
            0x02, 0x01, 0x00, 0x0a, 0x0a, 0x01, 0x08, 0x00,
            0xfd, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x0b
        ]));
    } catch {
        return false;
    }
}

function checkThreadsSupport() {
    try {
        return typeof SharedArrayBuffer !== 'undefined' &&
            typeof Atomics !== 'undefined';
    } catch {
        return false;
    }
}

function getMaxMemory() {
    // Browser
    if (typeof navigator !== 'undefined' && navigator.deviceMemory) {
        return navigator.deviceMemory * 1024 * 1024 * 1024;
    }

    // Node.js
    if (typeof process !== 'undefined') {
        try {
            const os = require('os');
            return os.totalmem?.() || 4 * 1024 * 1024 * 1024;
        } catch {
            return 4 * 1024 * 1024 * 1024;
        }
    }

    // Default 4GB
    return 4 * 1024 * 1024 * 1024;
}

// ============================================================================
// WASM MODULE LOADING
// ============================================================================

let wasmModule = null;
let wasmInstance = null;
let wasmReady = false;

/**
 * Initialize WASM module with platform-specific loading
 */
export async function initWasm(options = {}) {
    if (wasmReady && wasmInstance) {
        return wasmInstance;
    }

    const platform = detectPlatform();
    const capabilities = getPlatformCapabilities();

    console.log(`[WASM Core] Initializing on ${platform}`);

    try {
        // Try to import the main WASM module
        const wasmPath = options.wasmPath || findWasmPath();

        if (capabilities.hasStreaming && platform !== 'cloudflare-workers') {
            // Streaming compilation (faster)
            const response = await fetch(wasmPath);
            wasmModule = await WebAssembly.compileStreaming(response);
        } else {
            // Buffer-based compilation (Workers, fallback)
            const wasmBytes = await loadWasmBytes(wasmPath);
            wasmModule = await WebAssembly.compile(wasmBytes);
        }

        // Instantiate with imports
        wasmInstance = await WebAssembly.instantiate(wasmModule, getWasmImports());
        wasmReady = true;

        console.log(`[WASM Core] Ready with ${capabilities.hasSIMD ? 'SIMD' : 'no SIMD'}`);

        return wasmInstance;
    } catch (error) {
        console.warn(`[WASM Core] Native WASM failed, using JS fallback:`, error.message);

        // Use JavaScript fallback
        wasmInstance = createJSFallback();
        wasmReady = true;

        return wasmInstance;
    }
}

/**
 * Find WASM file path based on environment
 */
function findWasmPath() {
    const platform = detectPlatform();

    switch (platform) {
        case 'node':
        case 'bun':
            return new URL('../ruvector_edge_net_bg.wasm', import.meta.url).href;
        case 'deno':
            return new URL('../ruvector_edge_net_bg.wasm', import.meta.url).href;
        case 'browser':
            // Try multiple paths
            return './ruvector_edge_net_bg.wasm';
        case 'cloudflare-workers':
            // Workers use bundled WASM
            return '__WASM_MODULE__';
        default:
            return './ruvector_edge_net_bg.wasm';
    }
}

/**
 * Load WASM bytes based on platform
 */
async function loadWasmBytes(path) {
    const platform = detectPlatform();

    switch (platform) {
        case 'node': {
            // Node 18+ has native fetch, use it for URLs
            if (path.startsWith('http://') || path.startsWith('https://')) {
                const response = await fetch(path);
                return response.arrayBuffer();
            }
            // For file:// URLs or local paths
            const fs = await import('fs/promises');
            const { fileURLToPath } = await import('url');
            // Convert file:// URL to path if needed
            const filePath = path.startsWith('file://')
                ? fileURLToPath(path)
                : path;
            return fs.readFile(filePath);
        }
        case 'deno': {
            return Deno.readFile(path);
        }
        case 'bun': {
            return Bun.file(path).arrayBuffer();
        }
        default: {
            const response = await fetch(path);
            return response.arrayBuffer();
        }
    }
}

/**
 * Shared WASM memory - created once before instantiation
 */
let sharedMemory = null;

function getSharedMemory() {
    if (!sharedMemory) {
        // Reasonable memory limits for edge platforms:
        // initial: 256 pages (16MB), max: 1024 pages (64MB)
        sharedMemory = new WebAssembly.Memory({ initial: 256, maximum: 1024 });
    }
    return sharedMemory;
}

/**
 * WASM imports for the module
 * CRITICAL: Does NOT reference wasmInstance - uses shared memory instead
 */
function getWasmImports() {
    const memory = getSharedMemory();

    return {
        env: {
            // Memory - use shared instance created BEFORE wasm instantiation
            memory,

            // Console - uses shared memory buffer (safe, memory exists)
            console_log: (ptr, len) => {
                const bytes = new Uint8Array(memory.buffer, ptr, len);
                console.log(new TextDecoder().decode(bytes));
            },
            console_error: (ptr, len) => {
                const bytes = new Uint8Array(memory.buffer, ptr, len);
                console.error(new TextDecoder().decode(bytes));
            },

            // Time
            now_ms: () => Date.now(),

            // Random - uses shared memory buffer (safe)
            get_random_bytes: (ptr, len) => {
                const bytes = new Uint8Array(memory.buffer, ptr, len);
                crypto.getRandomValues(bytes);
            },
        },
        wasi_snapshot_preview1: {
            // Minimal WASI stubs for compatibility
            fd_write: () => 0,
            fd_read: () => 0,
            fd_close: () => 0,
            environ_get: () => 0,
            environ_sizes_get: () => 0,
            proc_exit: () => {},
        },
    };
}

// ============================================================================
// JAVASCRIPT FALLBACK
// ============================================================================

/**
 * Pure JavaScript fallback when WASM is unavailable
 */
function createJSFallback() {
    return {
        exports: {
            // SHA256
            sha256: async (data) => {
                const hashBuffer = await crypto.subtle.digest('SHA-256', data);
                return new Uint8Array(hashBuffer);
            },

            // SHA512
            sha512: async (data) => {
                const hashBuffer = await crypto.subtle.digest('SHA-512', data);
                return new Uint8Array(hashBuffer);
            },

            // Ed25519 (using SubtleCrypto if available)
            // SECURITY: Fail closed - never return mock signatures
            ed25519_sign: async (message, privateKey) => {
                // SubtleCrypto Ed25519 support varies by platform
                try {
                    const key = await crypto.subtle.importKey(
                        'raw',
                        privateKey,
                        { name: 'Ed25519' },
                        false,
                        ['sign']
                    );
                    const signature = await crypto.subtle.sign('Ed25519', key, message);
                    return new Uint8Array(signature);
                } catch (error) {
                    // FAIL CLOSED: Do not return mock signatures - throw error
                    throw new Error(`[SECURITY] Ed25519 signing unavailable: ${error.message}. Install tweetnacl for platforms without native Ed25519.`);
                }
            },

            ed25519_verify: async (message, signature, publicKey) => {
                try {
                    const key = await crypto.subtle.importKey(
                        'raw',
                        publicKey,
                        { name: 'Ed25519' },
                        false,
                        ['verify']
                    );
                    return await crypto.subtle.verify('Ed25519', key, signature, message);
                } catch (error) {
                    // FAIL CLOSED: If verification unavailable, reject
                    console.error('[SECURITY] Ed25519 verify unavailable:', error.message);
                    return false; // Reject signature when verification unavailable
                }
            },

            // Merkle operations (pure JS)
            merkle_root: (hashes) => {
                return computeMerkleRootJS(hashes);
            },

            // Canonical JSON
            canonical_json: (obj) => {
                return canonicalizeJS(obj);
            },
        },
    };
}

/**
 * Pure JS Merkle root computation
 */
function computeMerkleRootJS(hashes) {
    if (hashes.length === 0) return new Uint8Array(32);
    if (hashes.length === 1) return hashes[0];

    let level = [...hashes];

    while (level.length > 1) {
        const nextLevel = [];
        for (let i = 0; i < level.length; i += 2) {
            const left = level[i];
            const right = level[i + 1] || left;
            // Concatenate and hash
            const combined = new Uint8Array(left.length + right.length);
            combined.set(left, 0);
            combined.set(right, left.length);
            // Use sync hash for fallback
            nextLevel.push(hashSync(combined));
        }
        level = nextLevel;
    }

    return level[0];
}

/**
 * Synchronous hash for fallback (uses simple hash if crypto.subtle unavailable)
 */
function hashSync(data) {
    // Simple FNV-1a hash for fallback (NOT cryptographically secure)
    // In production, use a proper sync hash library
    const FNV_PRIME = 0x01000193;
    const FNV_OFFSET = 0x811c9dc5;

    let hash = FNV_OFFSET;
    for (let i = 0; i < data.length; i++) {
        hash ^= data[i];
        hash = Math.imul(hash, FNV_PRIME);
    }

    // Expand to 32 bytes (not secure, just for structure)
    const result = new Uint8Array(32);
    for (let i = 0; i < 32; i++) {
        result[i] = (hash >> (i % 4) * 8) & 0xff;
        hash = Math.imul(hash, FNV_PRIME);
    }

    return result;
}

/**
 * Canonical JSON for JS fallback
 */
function canonicalizeJS(obj) {
    if (obj === null || obj === undefined) return 'null';
    if (typeof obj === 'boolean') return obj ? 'true' : 'false';
    if (typeof obj === 'number') {
        if (!Number.isFinite(obj)) throw new Error('Cannot canonicalize Infinity/NaN');
        return JSON.stringify(obj);
    }
    if (typeof obj === 'string') {
        return JSON.stringify(obj).replace(/[\u007f-\uffff]/g, (c) =>
            '\\u' + ('0000' + c.charCodeAt(0).toString(16)).slice(-4)
        );
    }
    if (Array.isArray(obj)) {
        return '[' + obj.map(canonicalizeJS).join(',') + ']';
    }
    if (typeof obj === 'object') {
        const keys = Object.keys(obj).sort();
        const pairs = keys
            .filter(k => obj[k] !== undefined)
            .map(k => canonicalizeJS(k) + ':' + canonicalizeJS(obj[k]));
        return '{' + pairs.join(',') + '}';
    }
    throw new Error(`Cannot canonicalize: ${typeof obj}`);
}

// ============================================================================
// WASM CRYPTO API
// ============================================================================

/**
 * WASM-accelerated cryptographic operations
 */
export class WasmCrypto {
    constructor() {
        this.ready = false;
        this.instance = null;
    }

    async init() {
        this.instance = await initWasm();
        this.ready = true;
        return this;
    }

    /**
     * SHA256 hash
     */
    async sha256(data) {
        if (!this.ready) await this.init();

        const input = typeof data === 'string'
            ? new TextEncoder().encode(data)
            : new Uint8Array(data);

        if (this.instance.exports.sha256) {
            return this.instance.exports.sha256(input);
        }

        // Fallback to SubtleCrypto
        const hash = await crypto.subtle.digest('SHA-256', input);
        return new Uint8Array(hash);
    }

    /**
     * SHA256 as hex string
     */
    async sha256Hex(data) {
        const hash = await this.sha256(data);
        return Array.from(hash).map(b => b.toString(16).padStart(2, '0')).join('');
    }

    /**
     * Ed25519 sign
     */
    async sign(message, privateKey) {
        if (!this.ready) await this.init();

        const msgBytes = typeof message === 'string'
            ? new TextEncoder().encode(message)
            : new Uint8Array(message);

        return this.instance.exports.ed25519_sign(msgBytes, privateKey);
    }

    /**
     * Ed25519 verify
     */
    async verify(message, signature, publicKey) {
        if (!this.ready) await this.init();

        const msgBytes = typeof message === 'string'
            ? new TextEncoder().encode(message)
            : new Uint8Array(message);

        return this.instance.exports.ed25519_verify(msgBytes, signature, publicKey);
    }

    /**
     * Compute Merkle root from chunk hashes
     */
    async merkleRoot(chunkHashes) {
        if (!this.ready) await this.init();

        if (this.instance.exports.merkle_root) {
            return this.instance.exports.merkle_root(chunkHashes);
        }

        // JS fallback
        return computeMerkleRootJS(chunkHashes);
    }

    /**
     * Canonical JSON encoding
     */
    canonicalize(obj) {
        return canonicalizeJS(obj);
    }

    /**
     * Hash canonical JSON
     */
    async hashCanonical(obj) {
        const canonical = this.canonicalize(obj);
        return this.sha256Hex(canonical);
    }
}

// ============================================================================
// WASM MODEL INFERENCE
// ============================================================================

/**
 * WASM-accelerated model inference
 */
export class WasmInference {
    constructor(options = {}) {
        this.ready = false;
        this.model = null;
        this.crypto = new WasmCrypto();
        this.useSIMD = options.useSIMD ?? true;
        this.useThreads = options.useThreads ?? false;
    }

    async init() {
        await this.crypto.init();
        this.ready = true;

        const caps = getPlatformCapabilities();
        console.log(`[WASM Inference] Ready: SIMD=${caps.hasSIMD}, Threads=${caps.hasThreads}`);

        return this;
    }

    /**
     * Load ONNX model into WASM runtime
     */
    async loadModel(modelData, manifest) {
        if (!this.ready) await this.init();

        // Verify model integrity first
        const hash = await this.crypto.sha256Hex(modelData);
        // Support both manifest formats: artifacts.model.sha256 and artifacts[0].sha256
        const expected = manifest.artifacts?.model?.sha256 ||
                         manifest.artifacts?.[0]?.sha256 ||
                         manifest.model?.sha256;

        if (expected && hash !== expected) {
            throw new Error(`Model hash mismatch: ${hash} !== ${expected}`);
        }

        // In production, this would initialize ONNX runtime in WASM
        // For now, we store the model data
        this.model = {
            data: modelData,
            manifest,
            loadedAt: Date.now(),
        };

        return this;
    }

    /**
     * Run inference (placeholder for ONNX WASM runtime)
     */
    async infer(input, options = {}) {
        if (!this.model) {
            throw new Error('No model loaded');
        }

        // In production, this would call ONNX WASM runtime
        // For now, return placeholder
        console.log('[WASM Inference] Would run inference on:', typeof input);

        return {
            output: null,
            timeMs: 0,
            platform: detectPlatform(),
        };
    }

    /**
     * Generate embeddings
     */
    async embed(texts) {
        if (!this.model) {
            throw new Error('No model loaded');
        }

        const inputTexts = Array.isArray(texts) ? texts : [texts];

        // Placeholder - in production, run through ONNX
        const embeddings = inputTexts.map(text => {
            // Generate deterministic pseudo-embedding from text hash
            const hash = this.crypto.canonicalize(text);
            const embedding = new Float32Array(384);
            for (let i = 0; i < 384; i++) {
                embedding[i] = (hash.charCodeAt(i % hash.length) - 64) / 64;
            }
            return embedding;
        });

        return {
            embeddings,
            model: this.model.manifest?.model?.id || 'unknown',
            platform: detectPlatform(),
        };
    }

    /**
     * Unload model and free memory
     */
    unload() {
        this.model = null;
    }
}

// ============================================================================
// GENESIS BIRTHING SYSTEM (WASM)
// ============================================================================

/**
 * WASM-native network genesis/birthing system
 *
 * Creates new network instances with:
 * - Cryptographic identity (Ed25519 keypair)
 * - Lineage tracking (Merkle DAG)
 * - Cross-platform compatibility
 * - Cryptographic signing of genesis blocks
 */
export class WasmGenesis {
    constructor(options = {}) {
        this.crypto = new WasmCrypto();
        this.ready = false;

        // Genesis configuration
        this.config = {
            networkName: options.networkName || 'edge-net',
            version: options.version || '1.0.0',
            parentId: options.parentId || null,
            traits: options.traits || {},
        };

        // Network keypair (generated during birth)
        this.keypair = null;
    }

    async init() {
        await this.crypto.init();
        this.ready = true;
        return this;
    }

    /**
     * Generate Ed25519 keypair for network identity
     * SECURITY: Uses WebCrypto for key generation
     */
    async _generateKeypair() {
        try {
            const keyPair = await crypto.subtle.generateKey(
                { name: 'Ed25519' },
                true, // extractable for export
                ['sign', 'verify']
            );

            // Export public key
            const publicKeyRaw = await crypto.subtle.exportKey('raw', keyPair.publicKey);

            return {
                privateKey: keyPair.privateKey,
                publicKey: keyPair.publicKey,
                publicKeyBytes: new Uint8Array(publicKeyRaw),
                publicKeyHex: Array.from(new Uint8Array(publicKeyRaw))
                    .map(b => b.toString(16).padStart(2, '0'))
                    .join(''),
            };
        } catch (error) {
            throw new Error(`[SECURITY] Cannot generate Ed25519 keypair: ${error.message}. Native Ed25519 required for genesis.`);
        }
    }

    /**
     * Sign data with network private key
     */
    async _signWithKeypair(data, keypair) {
        const dataBytes = typeof data === 'string'
            ? new TextEncoder().encode(data)
            : new Uint8Array(data);

        try {
            const signature = await crypto.subtle.sign(
                { name: 'Ed25519' },
                keypair.privateKey,
                dataBytes
            );

            return {
                signature: new Uint8Array(signature),
                signatureHex: Array.from(new Uint8Array(signature))
                    .map(b => b.toString(16).padStart(2, '0'))
                    .join(''),
            };
        } catch (error) {
            throw new Error(`[SECURITY] Signing failed: ${error.message}`);
        }
    }

    /**
     * Birth a new network instance with cryptographic signing
     */
    async birthNetwork(options = {}) {
        if (!this.ready) await this.init();

        const timestamp = Date.now();

        // Generate network keypair
        this.keypair = await this._generateKeypair();

        // Generate network identity
        const networkId = await this._generateNetworkId(timestamp);

        // Create genesis block (unsigned payload)
        const genesisBlock = {
            networkId,
            version: this.config.version,
            birthTimestamp: timestamp,
            parentId: this.config.parentId,
            traits: {
                ...this.config.traits,
                ...options.traits,
            },
            capabilities: options.capabilities || ['embed', 'generate'],
            platform: detectPlatform(),
            platformCapabilities: getPlatformCapabilities(),
            // Include public key for verification
            publicKey: this.keypair.publicKeyHex,
            keyAlgorithm: 'Ed25519',
        };

        // Compute canonical hash of genesis block
        const genesisCanonical = this.crypto.canonicalize(genesisBlock);
        const genesisHash = await this.crypto.sha256Hex(genesisCanonical);

        // SECURITY: Sign the genesis hash with network keypair
        const { signature, signatureHex } = await this._signWithKeypair(genesisHash, this.keypair);

        // Create network manifest with signature
        const manifest = {
            schemaVersion: '2.0.0',
            genesis: genesisBlock,
            integrity: {
                genesisHash,
                signatureAlgorithm: 'Ed25519',
                signature: signatureHex,
                signedPayload: 'genesis',
                merkleRoot: await this.crypto.merkleRoot([
                    await this.crypto.sha256(genesisCanonical),
                ]),
            },
            lineage: this.config.parentId ? {
                parentId: this.config.parentId,
                generation: options.generation || 1,
                inheritedTraits: options.inheritedTraits || [],
            } : null,
        };

        return {
            networkId,
            manifest,
            genesisHash,
            signature: signatureHex,
            publicKey: this.keypair.publicKeyHex,
            platform: detectPlatform(),
        };
    }

    /**
     * Generate unique network ID using cryptographic randomness
     * SECURITY: Uses crypto.getRandomValues instead of Math.random()
     */
    async _generateNetworkId(timestamp) {
        // Generate 16 bytes of cryptographic randomness
        const randomBytes = new Uint8Array(16);
        crypto.getRandomValues(randomBytes);
        const randomHex = Array.from(randomBytes)
            .map(b => b.toString(16).padStart(2, '0'))
            .join('');

        const seed = `${this.config.networkName}:${timestamp}:${randomHex}`;
        const hash = await this.crypto.sha256Hex(seed);
        return `net_${hash.slice(0, 16)}`;
    }

    /**
     * Verify a network's genesis signature
     * SECURITY: Validates cryptographic signature of genesis block
     */
    async verifyGenesis(manifest) {
        const genesis = manifest.genesis;
        const integrity = manifest.integrity;

        if (!genesis.publicKey || !integrity.signature) {
            return { valid: false, error: 'Missing public key or signature' };
        }

        try {
            // Reconstruct the canonical hash
            const genesisCanonical = this.crypto.canonicalize(genesis);
            const genesisHash = await this.crypto.sha256Hex(genesisCanonical);

            // Verify hash matches
            if (genesisHash !== integrity.genesisHash) {
                return { valid: false, error: 'Genesis hash mismatch' };
            }

            // Convert hex strings back to bytes
            const publicKeyBytes = new Uint8Array(
                genesis.publicKey.match(/.{2}/g).map(b => parseInt(b, 16))
            );
            const signatureBytes = new Uint8Array(
                integrity.signature.match(/.{2}/g).map(b => parseInt(b, 16))
            );
            const hashBytes = new TextEncoder().encode(genesisHash);

            // Import public key and verify
            const publicKey = await crypto.subtle.importKey(
                'raw',
                publicKeyBytes,
                { name: 'Ed25519' },
                false,
                ['verify']
            );

            const valid = await crypto.subtle.verify(
                { name: 'Ed25519' },
                publicKey,
                signatureBytes,
                hashBytes
            );

            return { valid, genesisHash };
        } catch (error) {
            return { valid: false, error: `Verification failed: ${error.message}` };
        }
    }

    /**
     * Verify a network's lineage with cryptographic validation
     */
    async verifyLineage(manifest, parentManifest = null) {
        // First verify genesis signature
        const genesisResult = await this.verifyGenesis(manifest);
        if (!genesisResult.valid) {
            return { valid: false, error: `Genesis invalid: ${genesisResult.error}` };
        }

        if (!manifest.lineage) {
            return { valid: true, isRoot: true, genesisVerified: true };
        }

        if (!parentManifest) {
            return { valid: false, error: 'Parent manifest required for lineage verification' };
        }

        // Verify parent genesis signature
        const parentGenesisResult = await this.verifyGenesis(parentManifest);
        if (!parentGenesisResult.valid) {
            return { valid: false, error: `Parent genesis invalid: ${parentGenesisResult.error}` };
        }

        // Verify parent ID matches
        if (manifest.lineage.parentId !== parentManifest.genesis.networkId) {
            return { valid: false, error: 'Parent ID mismatch' };
        }

        // Verify generation is sequential
        const parentGen = parentManifest.lineage?.generation || 0;
        if (manifest.lineage.generation !== parentGen + 1) {
            return { valid: false, error: 'Generation sequence broken' };
        }

        return {
            valid: true,
            parentId: manifest.lineage.parentId,
            generation: manifest.lineage.generation,
            genesisVerified: true,
            parentVerified: true,
        };
    }

    /**
     * Create a child network (reproduction)
     */
    async reproduce(parentManifest, options = {}) {
        if (!this.ready) await this.init();

        // Mutate traits from parent
        const mutatedTraits = this._mutateTraits(
            parentManifest.genesis.traits,
            options.mutationRate || 0.1
        );

        // Birth child network
        const child = await this.birthNetwork({
            traits: mutatedTraits,
            capabilities: options.capabilities || parentManifest.genesis.capabilities,
            generation: (parentManifest.lineage?.generation || 0) + 1,
            inheritedTraits: Object.keys(parentManifest.genesis.traits),
        });

        // Update config for lineage
        child.manifest.lineage = {
            parentId: parentManifest.genesis.networkId,
            generation: (parentManifest.lineage?.generation || 0) + 1,
            inheritedTraits: Object.keys(parentManifest.genesis.traits),
            mutatedTraits: Object.keys(mutatedTraits).filter(
                k => mutatedTraits[k] !== parentManifest.genesis.traits[k]
            ),
        };

        return child;
    }

    /**
     * Mutate traits for evolution using cryptographic randomness
     * SECURITY: Uses crypto.getRandomValues for unpredictable mutations
     */
    _mutateTraits(parentTraits, mutationRate) {
        const mutated = { ...parentTraits };

        // Generate random bytes for mutation decisions
        const randomBytes = new Uint8Array(Object.keys(mutated).length * 2);
        crypto.getRandomValues(randomBytes);

        let idx = 0;
        for (const [key, value] of Object.entries(mutated)) {
            // Use crypto random for mutation probability check
            const shouldMutate = (randomBytes[idx++] / 255) < mutationRate;

            if (typeof value === 'number' && shouldMutate) {
                // Use crypto random for mutation amount (+/- 10%)
                const mutationFactor = ((randomBytes[idx++] / 255) - 0.5) * 0.2;
                mutated[key] = value * (1 + mutationFactor);
            } else {
                idx++; // Skip unused random byte
            }
        }

        return mutated;
    }
}

// ============================================================================
// SINGLETON INSTANCES
// ============================================================================

let cryptoInstance = null;
let genesisInstance = null;

export async function getCrypto() {
    if (!cryptoInstance) {
        cryptoInstance = new WasmCrypto();
        await cryptoInstance.init();
    }
    return cryptoInstance;
}

export async function getGenesis(options = {}) {
    if (!genesisInstance) {
        genesisInstance = new WasmGenesis(options);
        await genesisInstance.init();
    }
    return genesisInstance;
}

// ============================================================================
// EXPORTS
// ============================================================================

export default {
    detectPlatform,
    getPlatformCapabilities,
    initWasm,
    WasmCrypto,
    WasmInference,
    WasmGenesis,
    getCrypto,
    getGenesis,
};
