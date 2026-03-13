/**
 * CNN Embedder — RuVector Attention-powered feature extractor.
 *
 * Uses the real ruvector-attention-wasm WASM module for Multi-Head Attention
 * and Flash Attention on CSI/video data. Falls back to a JS Conv2D pipeline
 * when WASM is not available.
 *
 * Pipeline: Conv2D → BatchNorm → ReLU → Pool → RuVector Attention → Project → L2 Normalize
 * Two instances are created: one for video frames, one for CSI pseudo-images.
 */

// Seeded PRNG for deterministic weight initialization
function mulberry32(seed) {
  return function() {
    let t = (seed += 0x6D2B79F5);
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export class CnnEmbedder {
  /**
   * @param {object} opts
   * @param {number} opts.inputSize   - Square input dimension (default 56 for speed)
   * @param {number} opts.embeddingDim - Output embedding dimension (default 128)
   * @param {boolean} opts.normalize  - L2 normalize output
   * @param {number} opts.seed        - PRNG seed for weight init
   */
  constructor(opts = {}) {
    this.inputSize = opts.inputSize || 56;
    this.embeddingDim = opts.embeddingDim || 128;
    this.normalize = opts.normalize !== false;
    this.wasmEmbedder = null;
    this.rvAttention = null;      // RuVector Multi-Head Attention (WASM)
    this.rvFlash = null;          // RuVector Flash Attention (WASM)
    this.rvHyperbolic = null;     // RuVector Hyperbolic Attention (hierarchical body)
    this.rvMoE = null;            // RuVector Mixture-of-Experts (body-region routing)
    this.rvLinear = null;         // RuVector Linear Attention (O(n) fast hand refinement)
    this.rvLocalGlobal = null;    // RuVector Local-Global Attention (detail + context)
    this.rvModule = null;         // RuVector WASM module reference
    this.useRuVector = false;

    // Initialize weights with deterministic PRNG
    const rng = mulberry32(opts.seed || 42);
    const randRange = (lo, hi) => lo + rng() * (hi - lo);

    // Conv 3x3: 3 input channels → 16 output channels
    this.convWeights = new Float32Array(3 * 3 * 3 * 16);
    for (let i = 0; i < this.convWeights.length; i++) {
      this.convWeights[i] = randRange(-0.15, 0.15);
    }

    // BatchNorm params (16 channels)
    this.bnGamma = new Float32Array(16).fill(1.0);
    this.bnBeta = new Float32Array(16).fill(0.0);
    this.bnMean = new Float32Array(16).fill(0.0);
    this.bnVar = new Float32Array(16).fill(1.0);

    // Projection: 16 → embeddingDim (used when RuVector not available)
    this.projWeights = new Float32Array(16 * this.embeddingDim);
    for (let i = 0; i < this.projWeights.length; i++) {
      this.projWeights[i] = randRange(-0.1, 0.1);
    }

    // Attention projection: attention_dim → embeddingDim
    this.attnProjWeights = new Float32Array(16 * this.embeddingDim);
    for (let i = 0; i < this.attnProjWeights.length; i++) {
      this.attnProjWeights[i] = randRange(-0.08, 0.08);
    }
  }

  /**
   * Try to load RuVector attention WASM, then fall back to ruvector-cnn-wasm
   * @param {string} wasmPath - Path to the WASM package directory
   */
  async tryLoadWasm(wasmPath) {
    // First try: RuVector Attention WASM (the real thing — browser ESM build)
    try {
      const attnBase = new URL('../pkg/ruvector-attention/ruvector_attention_browser.js', import.meta.url).href;
      const mod = await import(attnBase);
      await mod.default();  // async WASM init via fetch
      mod.init();

      // Create all 6 attention mechanisms
      this.rvAttention = new mod.WasmMultiHeadAttention(16, 4);
      this.rvFlash = new mod.WasmFlashAttention(16, 8);
      this.rvHyperbolic = new mod.WasmHyperbolicAttention(16, -1.0);
      this.rvMoE = new mod.WasmMoEAttention(16, 3, 2);
      this.rvLinear = new mod.WasmLinearAttention(16, 16);
      this.rvLocalGlobal = new mod.WasmLocalGlobalAttention(16, 4, 2);
      this.rvModule = mod;
      this.useRuVector = true;

      // Log available mechanisms
      const mechs = mod.available_mechanisms();
      console.log(`[CNN] RuVector WASM v${mod.version()} — all 6 attention mechanisms active`, mechs);
      return true;
    } catch (e) {
      console.log('[CNN] RuVector Attention WASM not available:', e.message);
    }

    // Second try: ruvector-cnn-wasm (legacy path)
    try {
      const mod = await import(`${wasmPath}/ruvector_cnn_wasm.js`);
      await mod.default();
      const config = new mod.EmbedderConfig();
      config.input_size = this.inputSize;
      config.embedding_dim = this.embeddingDim;
      config.normalize = this.normalize;
      this.wasmEmbedder = new mod.WasmCnnEmbedder(config);
      console.log('[CNN] WASM CNN embedder loaded successfully');
      return true;
    } catch (e) {
      console.log('[CNN] WASM CNN not available, using JS fallback:', e.message);
      return false;
    }
  }

  /**
   * Extract embedding from RGB image data
   * @param {Uint8Array} rgbData - RGB pixel data (H*W*3)
   * @param {number} width
   * @param {number} height
   * @returns {Float32Array} embedding vector
   */
  extract(rgbData, width, height) {
    if (this.wasmEmbedder) {
      try {
        const result = this.wasmEmbedder.extract(rgbData, width, height);
        return new Float32Array(result);
      } catch (_) { /* fallback to JS */ }
    }
    return this._extractJS(rgbData, width, height);
  }

  _extractJS(rgbData, width, height) {
    // 1. Resize to inputSize × inputSize if needed
    const sz = this.inputSize;
    let input;
    if (width === sz && height === sz) {
      input = new Float32Array(rgbData.length);
      for (let i = 0; i < rgbData.length; i++) input[i] = rgbData[i] / 255.0;
    } else {
      input = this._resize(rgbData, width, height, sz, sz);
    }

    // 2. ImageNet normalization
    const mean = [0.485, 0.456, 0.406];
    const std = [0.229, 0.224, 0.225];
    const pixels = sz * sz;
    for (let i = 0; i < pixels; i++) {
      input[i * 3]     = (input[i * 3]     - mean[0]) / std[0];
      input[i * 3 + 1] = (input[i * 3 + 1] - mean[1]) / std[1];
      input[i * 3 + 2] = (input[i * 3 + 2] - mean[2]) / std[2];
    }

    // 3. Conv2D 3x3 (3 → 16 channels)
    const convOut = this._conv2d3x3(input, sz, sz, 3, 16);

    // 4. BatchNorm
    this._batchNorm(convOut, 16);

    // 5. ReLU
    for (let i = 0; i < convOut.length; i++) {
      if (convOut[i] < 0) convOut[i] = 0;
    }

    // 6. Global average pooling → spatial tokens (each 16-dim)
    const outH = sz - 2, outW = sz - 2;
    const spatial = outH * outW;

    // 7. RuVector Attention (if loaded) — apply attention over spatial tokens
    if (this.useRuVector && this.rvAttention) {
      return this._extractWithAttention(convOut, spatial, 16);
    }

    // Fallback: simple global average pool + linear projection
    const pooled = new Float32Array(16);
    for (let i = 0; i < spatial; i++) {
      for (let c = 0; c < 16; c++) {
        pooled[c] += convOut[i * 16 + c];
      }
    }
    for (let c = 0; c < 16; c++) pooled[c] /= spatial;

    // Linear projection → embeddingDim
    const emb = new Float32Array(this.embeddingDim);
    for (let o = 0; o < this.embeddingDim; o++) {
      let sum = 0;
      for (let i = 0; i < 16; i++) {
        sum += pooled[i] * this.projWeights[i * this.embeddingDim + o];
      }
      emb[o] = sum;
    }

    // L2 normalize
    if (this.normalize) {
      let norm = 0;
      for (let i = 0; i < emb.length; i++) norm += emb[i] * emb[i];
      norm = Math.sqrt(norm);
      if (norm > 1e-8) {
        for (let i = 0; i < emb.length; i++) emb[i] /= norm;
      }
    }

    return emb;
  }

  /**
   * Full 6-stage RuVector WASM attention pipeline:
   * 1. Flash Attention (efficient O(n) pre-screening of spatial tokens)
   * 2. Multi-Head Attention (global spatial reasoning)
   * 3. Hyperbolic Attention (hierarchical body-part structure, Poincaré ball)
   * 4. Linear Attention (O(n) refinement for fine detail — hands/extremities)
   * 5. MoE Attention (body-region specialized expert routing)
   * 6. Local-Global Attention (local detail + global context fusion)
   * → Weighted blend + batch_normalize + project + L2 normalize
   */
  _extractWithAttention(convOut, numTokens, channels) {
    const mod = this.rvModule;

    // Subsample spatial tokens for attention (max 64 for speed)
    const maxTokens = 64;
    const step = numTokens > maxTokens ? Math.floor(numTokens / maxTokens) : 1;
    const tokens = [];
    for (let i = 0; i < numTokens && tokens.length < maxTokens; i += step) {
      const token = new Float32Array(channels);
      for (let c = 0; c < channels; c++) {
        token[c] = convOut[i * channels + c];
      }
      tokens.push(token);
    }

    const numQueries = Math.min(4, tokens.length);
    const queryStride = Math.floor(tokens.length / numQueries);

    // === Stage 1: Flash Attention (efficient pre-screening) ===
    const flashOut = new Float32Array(channels);
    try {
      // Flash attention with block size 8 for efficient O(n) screening
      const result = this.rvFlash.compute(tokens[0], tokens, tokens);
      for (let c = 0; c < channels; c++) flashOut[c] = result[c];
    } catch (_) {
      flashOut.set(tokens[0]);
    }

    // === Stage 2: Multi-Head Attention (global spatial reasoning) ===
    const mhaOut = new Float32Array(channels);
    for (let q = 0; q < numQueries; q++) {
      const queryToken = tokens[q * queryStride];
      try {
        const result = this.rvAttention.compute(queryToken, tokens, tokens);
        for (let c = 0; c < channels; c++) mhaOut[c] += result[c] / numQueries;
      } catch (_) {
        for (let c = 0; c < channels; c++) mhaOut[c] += queryToken[c] / numQueries;
      }
    }

    // === Stage 3: Hyperbolic Attention (hierarchical body structure) ===
    const hyOut = new Float32Array(channels);
    try {
      const result = this.rvHyperbolic.compute(mhaOut, tokens, tokens);
      for (let c = 0; c < channels; c++) hyOut[c] = result[c];
    } catch (_) {
      hyOut.set(mhaOut);
    }

    // === Stage 4: Linear Attention (O(n) fast refinement for extremities) ===
    const linOut = new Float32Array(channels);
    try {
      const result = this.rvLinear.compute(hyOut, tokens, tokens);
      for (let c = 0; c < channels; c++) linOut[c] = result[c];
    } catch (_) {
      linOut.set(hyOut);
    }

    // === Stage 5: MoE Attention (body-region expert routing) ===
    const moeOut = new Float32Array(channels);
    try {
      const result = this.rvMoE.compute(linOut, tokens, tokens);
      for (let c = 0; c < channels; c++) moeOut[c] = result[c];
    } catch (_) {
      moeOut.set(linOut);
    }

    // === Stage 6: Local-Global Attention (detail + context) ===
    const lgOut = new Float32Array(channels);
    try {
      const result = this.rvLocalGlobal.compute(moeOut, tokens, tokens);
      for (let c = 0; c < channels; c++) lgOut[c] = result[c];
    } catch (_) {
      lgOut.set(moeOut);
    }

    // === Blend all 6 outputs ===
    // Use WASM softmax on log-energy scores for dynamic stage weighting
    const blended = new Float32Array(channels);
    const stages = [flashOut, mhaOut, hyOut, linOut, moeOut, lgOut];
    // Use log-energy to prevent exp() overflow in softmax
    const logEnergies = new Float32Array(6);
    for (let s = 0; s < 6; s++) {
      const e = this._energy(stages[s]);
      logEnergies[s] = e > 1e-10 ? Math.log(e) : -20;
    }
    try { mod.softmax(logEnergies); } catch (_) {
      let max = -Infinity;
      for (let i = 0; i < 6; i++) max = Math.max(max, logEnergies[i]);
      let sum = 0;
      for (let i = 0; i < 6; i++) { logEnergies[i] = Math.exp(logEnergies[i] - max); sum += logEnergies[i]; }
      for (let i = 0; i < 6; i++) logEnergies[i] /= sum;
    }
    for (let c = 0; c < channels; c++) {
      for (let s = 0; s < 6; s++) {
        blended[c] += logEnergies[s] * stages[s][c];
      }
    }

    // Batch normalize only when we have enough diversity (skip for single vectors)
    // Single-vector batch norm collapses to zeros, killing embedding space
    let normed = blended;

    // Project to embeddingDim
    const emb = new Float32Array(this.embeddingDim);
    for (let o = 0; o < this.embeddingDim; o++) {
      let sum = 0;
      for (let i = 0; i < channels; i++) {
        sum += normed[i] * this.attnProjWeights[i * this.embeddingDim + o];
      }
      emb[o] = sum;
    }

    // L2 normalize using RuVector WASM
    if (this.normalize) {
      try { mod.normalize(emb); } catch (_) {
        let norm = 0;
        for (let i = 0; i < emb.length; i++) norm += emb[i] * emb[i];
        norm = Math.sqrt(norm);
        if (norm > 1e-8) for (let i = 0; i < emb.length; i++) emb[i] /= norm;
      }
    }

    return emb;
  }

  /** Compute vector energy (L2 norm squared) for attention weighting */
  _energy(vec) {
    let e = 0;
    for (let i = 0; i < vec.length; i++) e += vec[i] * vec[i];
    return e;
  }

  _conv2d3x3(input, H, W, Cin, Cout) {
    const outH = H - 2, outW = W - 2;
    const output = new Float32Array(outH * outW * Cout);
    for (let y = 0; y < outH; y++) {
      for (let x = 0; x < outW; x++) {
        for (let co = 0; co < Cout; co++) {
          let sum = 0;
          for (let ky = 0; ky < 3; ky++) {
            for (let kx = 0; kx < 3; kx++) {
              for (let ci = 0; ci < Cin; ci++) {
                const px = ((y + ky) * W + (x + kx)) * Cin + ci;
                const wt = (((ky * 3 + kx) * Cin) + ci) * Cout + co;
                sum += input[px] * this.convWeights[wt];
              }
            }
          }
          output[(y * outW + x) * Cout + co] = sum;
        }
      }
    }
    return output;
  }

  _batchNorm(data, channels) {
    const spatial = data.length / channels;
    for (let i = 0; i < spatial; i++) {
      for (let c = 0; c < channels; c++) {
        const idx = i * channels + c;
        data[idx] = this.bnGamma[c] * (data[idx] - this.bnMean[c]) / Math.sqrt(this.bnVar[c] + 1e-5) + this.bnBeta[c];
      }
    }
  }

  _resize(rgbData, srcW, srcH, dstW, dstH) {
    const output = new Float32Array(dstW * dstH * 3);
    const xRatio = srcW / dstW;
    const yRatio = srcH / dstH;
    for (let y = 0; y < dstH; y++) {
      for (let x = 0; x < dstW; x++) {
        const sx = Math.min(Math.floor(x * xRatio), srcW - 1);
        const sy = Math.min(Math.floor(y * yRatio), srcH - 1);
        const srcIdx = (sy * srcW + sx) * 3;
        const dstIdx = (y * dstW + x) * 3;
        output[dstIdx]     = rgbData[srcIdx]     / 255.0;
        output[dstIdx + 1] = rgbData[srcIdx + 1] / 255.0;
        output[dstIdx + 2] = rgbData[srcIdx + 2] / 255.0;
      }
    }
    return output;
  }

  /** Cosine similarity using WASM when available, JS fallback */
  cosineSim(a, b) {
    if (this.rvModule) {
      try { return this.rvModule.cosine_similarity(a, b); } catch (_) { /* fallback */ }
    }
    return CnnEmbedder.cosineSimilarity(a, b);
  }

  /** L2 norm using WASM when available */
  l2Norm(vec) {
    if (this.rvModule) {
      try { return this.rvModule.l2_norm(vec); } catch (_) { /* fallback */ }
    }
    let norm = 0;
    for (let i = 0; i < vec.length; i++) norm += vec[i] * vec[i];
    return Math.sqrt(norm);
  }

  /** Pairwise distance matrix using WASM (for skeleton validation) */
  pairwiseDistances(vectors) {
    if (this.rvModule) {
      try { return this.rvModule.pairwise_distances(vectors); } catch (_) { /* fallback */ }
    }
    return null;
  }

  /** Static JS fallback for cosine similarity */
  static cosineSimilarity(a, b) {
    let dot = 0, normA = 0, normB = 0;
    for (let i = 0; i < a.length; i++) {
      dot += a[i] * b[i];
      normA += a[i] * a[i];
      normB += b[i] * b[i];
    }
    normA = Math.sqrt(normA);
    normB = Math.sqrt(normB);
    if (normA < 1e-8 || normB < 1e-8) return 0;
    return dot / (normA * normB);
  }
}
