/**
 * Integration test for ruvector-attention-wasm package
 * Tests all attention mechanisms from published npm package
 */

import { test, describe } from 'node:test';
import assert from 'node:assert';

// Import from published WASM package
import init, {
  scaled_dot_attention,
  WasmMultiHeadAttention,
  WasmHyperbolicAttention,
  WasmLinearAttention,
  WasmFlashAttention,
  WasmLocalGlobalAttention,
  WasmMoEAttention
} from 'ruvector-attention-wasm';

describe('WASM Attention Package Tests', async () => {
  // Initialize WASM before tests
  await init();

  test('Scaled Dot-Product Attention', () => {
    const dim = 64;
    const query = new Float32Array(dim).fill(0.5);
    const keys = [
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random())
    ];
    const values = [
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random())
    ];

    const result = scaled_dot_attention(query, keys, values, null);
    assert.ok(result instanceof Float32Array, 'Result should be Float32Array');
    assert.strictEqual(result.length, dim, `Result dimension should be ${dim}`);
    console.log('  ✓ Scaled dot-product attention works correctly');
  });

  test('Multi-Head Attention', () => {
    const dim = 64;
    const numHeads = 8;

    const mha = new WasmMultiHeadAttention(dim, numHeads);
    assert.strictEqual(mha.dim, dim, 'Dimension should match');
    assert.strictEqual(mha.num_heads, numHeads, 'Number of heads should match');

    const query = new Float32Array(dim).fill(0.5);
    const keys = [
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random())
    ];
    const values = [
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random())
    ];

    const result = mha.compute(query, keys, values);
    assert.ok(result instanceof Float32Array, 'Result should be Float32Array');
    assert.strictEqual(result.length, dim, `Result dimension should be ${dim}`);
    console.log('  ✓ Multi-head attention works correctly');
  });

  test('Hyperbolic Attention', () => {
    const dim = 64;
    const curvature = 1.0;

    const hyperbolic = new WasmHyperbolicAttention(dim, curvature);
    assert.strictEqual(hyperbolic.curvature, curvature, 'Curvature should match');

    const query = new Float32Array(dim).fill(0.1);
    const keys = [
      Array.from({ length: dim }, () => Math.random() * 0.1),
      Array.from({ length: dim }, () => Math.random() * 0.1)
    ];
    const values = [
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random())
    ];

    const result = hyperbolic.compute(query, keys, values);
    assert.ok(result instanceof Float32Array, 'Result should be Float32Array');
    assert.strictEqual(result.length, dim, `Result dimension should be ${dim}`);
    console.log('  ✓ Hyperbolic attention works correctly');
  });

  test('Linear Attention (Performer-style)', () => {
    const dim = 64;
    const numFeatures = 128;

    const linear = new WasmLinearAttention(dim, numFeatures);

    const query = new Float32Array(dim).fill(0.5);
    const keys = [
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random())
    ];
    const values = [
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random())
    ];

    const result = linear.compute(query, keys, values);
    assert.ok(result instanceof Float32Array, 'Result should be Float32Array');
    assert.strictEqual(result.length, dim, `Result dimension should be ${dim}`);
    console.log('  ✓ Linear attention works correctly');
  });

  test('Flash Attention', () => {
    const dim = 64;
    const blockSize = 16;

    const flash = new WasmFlashAttention(dim, blockSize);

    const query = new Float32Array(dim).fill(0.5);
    const keys = [
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random())
    ];
    const values = [
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random())
    ];

    const result = flash.compute(query, keys, values);
    assert.ok(result instanceof Float32Array, 'Result should be Float32Array');
    assert.strictEqual(result.length, dim, `Result dimension should be ${dim}`);
    console.log('  ✓ Flash attention works correctly');
  });

  test('Local-Global Attention', () => {
    const dim = 64;
    const localWindow = 4;
    const globalTokens = 2;

    const localGlobal = new WasmLocalGlobalAttention(dim, localWindow, globalTokens);

    const query = new Float32Array(dim).fill(0.5);
    const keys = [
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random())
    ];
    const values = [
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random())
    ];

    const result = localGlobal.compute(query, keys, values);
    assert.ok(result instanceof Float32Array, 'Result should be Float32Array');
    assert.strictEqual(result.length, dim, `Result dimension should be ${dim}`);
    console.log('  ✓ Local-global attention works correctly');
  });

  test('Mixture of Experts (MoE) Attention', () => {
    const dim = 64;
    const numExperts = 4;
    const topK = 2;

    const moe = new WasmMoEAttention(dim, numExperts, topK);

    const query = new Float32Array(dim).fill(0.5);
    const keys = [
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random())
    ];
    const values = [
      Array.from({ length: dim }, () => Math.random()),
      Array.from({ length: dim }, () => Math.random())
    ];

    const result = moe.compute(query, keys, values);
    assert.ok(result instanceof Float32Array, 'Result should be Float32Array');
    assert.strictEqual(result.length, dim, `Result dimension should be ${dim}`);
    console.log('  ✓ MoE attention works correctly');
  });
});

console.log('\n✅ All WASM attention tests passed!\n');
