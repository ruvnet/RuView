#!/usr/bin/env node
/**
 * Test script to validate WASM embeddings package works
 */

import {
    version,
    simd_available,
    cosineSimilarity,
    normalizeL2,
    WasmEmbedderConfig
} from './pkg/ruvector_onnx_embeddings_wasm.js';

console.log('🧪 RuVector ONNX Embeddings WASM - Validation Test\n');

// Test 1: Version check
console.log('Test 1: Version check');
const ver = version();
console.log(`  ✅ Version: ${ver}`);

// Test 2: SIMD availability
console.log('\nTest 2: SIMD availability');
const simd = simd_available();
console.log(`  ✅ SIMD available: ${simd}`);

// Test 3: Cosine similarity utility
console.log('\nTest 3: Cosine similarity utility');
const a = new Float32Array([1.0, 0.0, 0.0]);
const b = new Float32Array([1.0, 0.0, 0.0]);
const c = new Float32Array([0.0, 1.0, 0.0]);

const simSame = cosineSimilarity(a, b);
const simDiff = cosineSimilarity(a, c);

console.log(`  ✅ Same vectors similarity: ${simSame.toFixed(4)} (expected: 1.0)`);
console.log(`  ✅ Orthogonal vectors similarity: ${simDiff.toFixed(4)} (expected: 0.0)`);

if (Math.abs(simSame - 1.0) > 0.0001) {
    throw new Error('Cosine similarity failed for same vectors');
}
if (Math.abs(simDiff) > 0.0001) {
    throw new Error('Cosine similarity failed for orthogonal vectors');
}

// Test 4: L2 normalization utility
console.log('\nTest 4: L2 normalization utility');
const unnormalized = new Float32Array([3.0, 4.0]);
const normalized = normalizeL2(unnormalized);

const norm = Math.sqrt(normalized[0]**2 + normalized[1]**2);
console.log(`  ✅ Normalized vector: [${normalized[0].toFixed(4)}, ${normalized[1].toFixed(4)}]`);
console.log(`  ✅ L2 norm: ${norm.toFixed(4)} (expected: 1.0)`);

if (Math.abs(norm - 1.0) > 0.0001) {
    throw new Error('L2 normalization failed');
}

// Test 5: Config creation
console.log('\nTest 5: WasmEmbedderConfig creation');
const config = new WasmEmbedderConfig()
    .setMaxLength(256)
    .setNormalize(true)
    .setPooling(0);
console.log('  ✅ Config created and chained successfully');

// Note: config is consumed by chaining, no need to free

console.log('\n' + '='.repeat(50));
console.log('✅ All utility tests passed!');
console.log('='.repeat(50));
console.log('\n📝 Note: Full embedder test requires ONNX model + tokenizer files.');
console.log('   The core WASM bindings are working correctly.\n');
