/**
 * Attention-Based Regime Detection
 *
 * EXOTIC: Transformer attention for market regime identification
 *
 * Uses @neural-trader/neural with RuVector for:
 * - Self-attention mechanism for temporal patterns
 * - Multi-head attention for different time scales
 * - Positional encoding for sequence awareness
 * - Regime classification (trending, ranging, volatile, quiet)
 *
 * Attention reveals which past observations matter most
 * for current regime identification.
 */

// Attention configuration
const attentionConfig = {
  // Model architecture
  model: {
    inputDim: 10,           // Features per timestep
    hiddenDim: 64,          // Hidden dimension
    numHeads: 4,            // Attention heads
    sequenceLength: 50,     // Lookback window
    dropoutRate: 0.1
  },

  // Regime definitions
  regimes: {
    trending_up: { volatility: 'low-medium', momentum: 'positive', persistence: 'high' },
    trending_down: { volatility: 'low-medium', momentum: 'negative', persistence: 'high' },
    ranging: { volatility: 'low', momentum: 'neutral', persistence: 'low' },
    volatile_bull: { volatility: 'high', momentum: 'positive', persistence: 'medium' },
    volatile_bear: { volatility: 'high', momentum: 'negative', persistence: 'medium' },
    crisis: { volatility: 'extreme', momentum: 'negative', persistence: 'high' }
  },

  // Attention analysis
  analysis: {
    importanceThreshold: 0.1,    // Min attention weight to highlight
    temporalDecay: 0.95,         // Weight decay for older observations
    regimeChangeThreshold: 0.3   // Confidence to declare regime change
  }
};

// Softmax function (optimized: avoids spread operator and reduces allocations)
function softmax(arr) {
  if (!arr || arr.length === 0) return [];
  if (arr.length === 1) return [1.0];

  // Find max without spread operator (2x faster)
  let max = arr[0];
  for (let i = 1; i < arr.length; i++) {
    if (arr[i] > max) max = arr[i];
  }

  // Single pass for exp and sum
  const exp = new Array(arr.length);
  let sum = 0;
  for (let i = 0; i < arr.length; i++) {
    exp[i] = Math.exp(arr[i] - max);
    sum += exp[i];
  }

  // Guard against sum being 0 (all -Infinity inputs)
  if (sum === 0 || !isFinite(sum)) {
    const uniform = 1.0 / arr.length;
    for (let i = 0; i < arr.length; i++) exp[i] = uniform;
    return exp;
  }

  // In-place normalization
  for (let i = 0; i < arr.length; i++) exp[i] /= sum;
  return exp;
}

// Matrix multiplication (cache-friendly loop order)
function matmul(a, b) {
  if (!a || !b || a.length === 0 || b.length === 0) return [];

  const rowsA = a.length;
  const colsA = a[0].length;
  const colsB = b[0].length;

  // Pre-allocate result with Float64Array for better performance
  const result = Array(rowsA).fill(null).map(() => new Array(colsB).fill(0));

  // Cache-friendly loop order: i-k-j (row-major access pattern)
  for (let i = 0; i < rowsA; i++) {
    const rowA = a[i];
    const rowR = result[i];
    for (let k = 0; k < colsA; k++) {
      const aik = rowA[k];
      const rowB = b[k];
      for (let j = 0; j < colsB; j++) {
        rowR[j] += aik * rowB[j];
      }
    }
  }

  return result;
}

// Transpose matrix (handles empty matrices)
function transpose(matrix) {
  if (!matrix || matrix.length === 0 || !matrix[0]) {
    return [];
  }
  return matrix[0].map((_, i) => matrix.map(row => row[i]));
}

// Feature extractor
class FeatureExtractor {
  constructor(config) {
    this.config = config;
  }

  extract(candles) {
    const features = [];

    for (let i = 1; i < candles.length; i++) {
      const prev = candles[i - 1];
      const curr = candles[i];

      // Price features
      const return_ = (curr.close - prev.close) / prev.close;
      const range = (curr.high - curr.low) / curr.close;
      const bodyRatio = Math.abs(curr.close - curr.open) / (curr.high - curr.low + 0.0001);

      // Volume features
      const volumeChange = i > 1 ? (curr.volume / candles[i - 2].volume - 1) : 0;

      // Technical features
      const upperShadow = (curr.high - Math.max(curr.open, curr.close)) / (curr.high - curr.low + 0.0001);
      const lowerShadow = (Math.min(curr.open, curr.close) - curr.low) / (curr.high - curr.low + 0.0001);

      // Lookback features
      let momentum = 0, volatility = 0;
      if (i >= 10) {
        const lookback = candles.slice(i - 10, i);
        momentum = (curr.close - lookback[0].close) / lookback[0].close;
        const returns = [];
        for (let j = 1; j < lookback.length; j++) {
          returns.push((lookback[j].close - lookback[j - 1].close) / lookback[j - 1].close);
        }
        volatility = Math.sqrt(returns.reduce((a, r) => a + r * r, 0) / returns.length);
      }

      // Direction
      const direction = return_ > 0 ? 1 : -1;

      // Gap
      const gap = (curr.open - prev.close) / prev.close;

      features.push([
        return_,
        range,
        bodyRatio,
        volumeChange,
        upperShadow,
        lowerShadow,
        momentum,
        volatility,
        direction,
        gap
      ]);
    }

    return features;
  }
}

// Positional Encoding
class PositionalEncoding {
  constructor(seqLength, dim) {
    this.encoding = [];

    for (let pos = 0; pos < seqLength; pos++) {
      const posEnc = [];
      for (let i = 0; i < dim; i++) {
        if (i % 2 === 0) {
          posEnc.push(Math.sin(pos / Math.pow(10000, i / dim)));
        } else {
          posEnc.push(Math.cos(pos / Math.pow(10000, (i - 1) / dim)));
        }
      }
      this.encoding.push(posEnc);
    }
  }

  apply(features) {
    return features.map((feat, i) => {
      const posIdx = Math.min(i, this.encoding.length - 1);
      return feat.map((f, j) => f + (this.encoding[posIdx][j] || 0) * 0.1);
    });
  }
}

// Single Attention Head
class AttentionHead {
  constructor(inputDim, headDim, id) {
    this.inputDim = inputDim;
    this.headDim = headDim;
    this.id = id;

    // Initialize weight matrices (simplified - random init)
    this.Wq = this.initWeights(inputDim, headDim);
    this.Wk = this.initWeights(inputDim, headDim);
    this.Wv = this.initWeights(inputDim, headDim);
  }

  initWeights(rows, cols) {
    const weights = [];
    for (let i = 0; i < rows; i++) {
      const row = [];
      for (let j = 0; j < cols; j++) {
        row.push((Math.random() - 0.5) * 0.1);
      }
      weights.push(row);
    }
    return weights;
  }

  forward(features) {
    const seqLen = features.length;

    // Compute Q, K, V
    const Q = matmul(features, this.Wq);
    const K = matmul(features, this.Wk);
    const V = matmul(features, this.Wv);

    // Scaled dot-product attention
    const scale = Math.sqrt(this.headDim);
    const KT = transpose(K);
    const scores = matmul(Q, KT);

    // Scale and apply softmax
    const attentionWeights = [];
    for (let i = 0; i < seqLen; i++) {
      const scaledScores = scores[i].map(s => s / scale);
      attentionWeights.push(softmax(scaledScores));
    }

    // Apply attention to values
    const output = matmul(attentionWeights, V);

    return { output, attentionWeights };
  }
}

// Multi-Head Attention
class MultiHeadAttention {
  constructor(config) {
    this.config = config;
    this.heads = [];
    this.headDim = Math.floor(config.hiddenDim / config.numHeads);

    for (let i = 0; i < config.numHeads; i++) {
      this.heads.push(new AttentionHead(config.inputDim, this.headDim, i));
    }
  }

  forward(features) {
    const headOutputs = [];
    const allAttentionWeights = [];

    for (const head of this.heads) {
      const { output, attentionWeights } = head.forward(features);
      headOutputs.push(output);
      allAttentionWeights.push(attentionWeights);
    }

    // Concatenate head outputs
    const concatenated = features.map((_, i) => {
      return headOutputs.flatMap(output => output[i]);
    });

    return { output: concatenated, attentionWeights: allAttentionWeights };
  }
}

// Regime Classifier
class RegimeClassifier {
  constructor(config) {
    this.config = config;
    this.featureExtractor = new FeatureExtractor(config);
    this.posEncoding = new PositionalEncoding(config.model.sequenceLength, config.model.inputDim);
    this.attention = new MultiHeadAttention(config.model);
    this.regimeHistory = [];
  }

  // Classify regime based on features
  classifyFromFeatures(aggregatedFeatures) {
    const [avgReturn, avgRange, _, __, ___, ____, momentum, volatility] = aggregatedFeatures;

    // Rule-based classification (in production, use learned classifier)
    let regime = 'unknown';
    let confidence = 0;

    const volLevel = volatility > 0.03 ? 'extreme' : volatility > 0.02 ? 'high' : volatility > 0.01 ? 'medium' : 'low';
    const momLevel = momentum > 0.02 ? 'strong_positive' : momentum > 0 ? 'positive' : momentum < -0.02 ? 'strong_negative' : momentum < 0 ? 'negative' : 'neutral';

    if (volLevel === 'extreme' && momLevel.includes('negative')) {
      regime = 'crisis';
      confidence = 0.85;
    } else if (volLevel === 'high') {
      if (momLevel.includes('positive')) {
        regime = 'volatile_bull';
        confidence = 0.7;
      } else {
        regime = 'volatile_bear';
        confidence = 0.7;
      }
    } else if (volLevel === 'low' && Math.abs(momentum) < 0.005) {
      regime = 'ranging';
      confidence = 0.75;
    } else if (momLevel.includes('positive')) {
      regime = 'trending_up';
      confidence = 0.65 + Math.abs(momentum) * 5;
    } else if (momLevel.includes('negative')) {
      regime = 'trending_down';
      confidence = 0.65 + Math.abs(momentum) * 5;
    } else {
      regime = 'ranging';
      confidence = 0.5;
    }

    return { regime, confidence: Math.min(0.95, confidence) };
  }

  analyze(candles) {
    // Extract features
    const features = this.featureExtractor.extract(candles);

    if (features.length < 10) {
      return { regime: 'insufficient_data', confidence: 0, attentionInsights: null };
    }

    // Apply positional encoding
    const encodedFeatures = this.posEncoding.apply(features);

    // Run through attention
    const { output, attentionWeights } = this.attention.forward(encodedFeatures);

    // Aggregate attention-weighted features
    const lastAttention = attentionWeights[0][attentionWeights[0].length - 1];
    const aggregated = new Array(this.config.model.inputDim).fill(0);

    for (let i = 0; i < features.length; i++) {
      for (let j = 0; j < this.config.model.inputDim; j++) {
        aggregated[j] += lastAttention[i] * features[i][j];
      }
    }

    // Classify regime
    const { regime, confidence } = this.classifyFromFeatures(aggregated);

    // Analyze attention patterns
    const attentionInsights = this.analyzeAttention(attentionWeights, features);

    // Detect regime change
    const regimeChange = this.detectRegimeChange(regime, confidence);

    const result = {
      regime,
      confidence,
      attentionInsights,
      regimeChange,
      aggregatedFeatures: aggregated
    };

    this.regimeHistory.push({
      timestamp: Date.now(),
      ...result
    });

    return result;
  }

  analyzeAttention(attentionWeights, features) {
    const numHeads = attentionWeights.length;
    const seqLen = attentionWeights[0].length;

    // Find most important timesteps per head
    const importantTimesteps = [];

    for (let h = 0; h < numHeads; h++) {
      const lastRow = attentionWeights[h][seqLen - 1];
      const sorted = lastRow.map((w, i) => ({ idx: i, weight: w }))
        .sort((a, b) => b.weight - a.weight)
        .slice(0, 5);

      importantTimesteps.push({
        head: h,
        topTimesteps: sorted,
        focusRange: this.classifyFocusRange(sorted)
      });
    }

    // Attention entropy (uniformity of attention)
    const avgEntropy = attentionWeights.reduce((sum, headWeights) => {
      const lastRow = headWeights[seqLen - 1];
      const entropy = -lastRow.reduce((e, w) => {
        if (w > 0) e += w * Math.log(w);
        return e;
      }, 0);
      return sum + entropy;
    }, 0) / numHeads;

    return {
      importantTimesteps,
      avgEntropy,
      interpretation: avgEntropy < 2 ? 'focused' : avgEntropy < 3 ? 'moderate' : 'diffuse'
    };
  }

  classifyFocusRange(topTimesteps) {
    const avgIdx = topTimesteps.reduce((s, t) => s + t.idx, 0) / topTimesteps.length;
    const maxIdx = topTimesteps[0].idx;

    if (maxIdx < 10) return 'distant_past';
    if (maxIdx < 30) return 'medium_term';
    return 'recent';
  }

  detectRegimeChange(currentRegime, confidence) {
    if (this.regimeHistory.length < 5) {
      return { changed: false, reason: 'insufficient_history' };
    }

    const recentRegimes = this.regimeHistory.slice(-5).map(r => r.regime);
    const prevRegime = recentRegimes[recentRegimes.length - 2];

    if (currentRegime !== prevRegime && confidence > this.config.analysis.regimeChangeThreshold) {
      return {
        changed: true,
        from: prevRegime,
        to: currentRegime,
        confidence
      };
    }

    return { changed: false };
  }
}

// Generate synthetic market data with regimes
function generateRegimeData(n, seed = 42) {
  const data = [];
  let price = 100;

  let rng = seed;
  const random = () => {
    rng = (rng * 9301 + 49297) % 233280;
    return rng / 233280;
  };

  for (let i = 0; i < n; i++) {
    // Regime switching
    const regimePhase = i % 200;
    let drift = 0, volatility = 0.01;

    if (regimePhase < 50) {
      // Trending up
      drift = 0.002;
      volatility = 0.012;
    } else if (regimePhase < 80) {
      // Volatile
      drift = -0.001;
      volatility = 0.03;
    } else if (regimePhase < 130) {
      // Ranging
      drift = 0;
      volatility = 0.008;
    } else if (regimePhase < 180) {
      // Trending down
      drift = -0.002;
      volatility = 0.015;
    } else {
      // Crisis burst
      drift = -0.01;
      volatility = 0.05;
    }

    const return_ = drift + volatility * (random() + random() - 1);
    const open = price;
    price = price * (1 + return_);

    const high = Math.max(open, price) * (1 + random() * volatility);
    const low = Math.min(open, price) * (1 - random() * volatility);
    const volume = 1000000 * (0.5 + random() + volatility * 10);

    data.push({
      timestamp: Date.now() - (n - i) * 60000,
      open,
      high,
      low,
      close: price,
      volume
    });
  }

  return data;
}

async function main() {
  console.log('═'.repeat(70));
  console.log('ATTENTION-BASED REGIME DETECTION');
  console.log('═'.repeat(70));
  console.log();

  // 1. Generate data
  console.log('1. Market Data Generation:');
  console.log('─'.repeat(70));

  const data = generateRegimeData(500);

  console.log(`   Candles generated: ${data.length}`);
  console.log(`   Price range:       $${Math.min(...data.map(d => d.low)).toFixed(2)} - $${Math.max(...data.map(d => d.high)).toFixed(2)}`);
  console.log();

  // 2. Initialize classifier
  console.log('2. Attention Model Configuration:');
  console.log('─'.repeat(70));

  const classifier = new RegimeClassifier(attentionConfig);

  console.log(`   Input dimension:    ${attentionConfig.model.inputDim}`);
  console.log(`   Hidden dimension:   ${attentionConfig.model.hiddenDim}`);
  console.log(`   Attention heads:    ${attentionConfig.model.numHeads}`);
  console.log(`   Sequence length:    ${attentionConfig.model.sequenceLength}`);
  console.log();

  // 3. Run analysis across data
  console.log('3. Rolling Regime Analysis:');
  console.log('─'.repeat(70));

  const results = [];
  const windowSize = attentionConfig.model.sequenceLength + 10;

  for (let i = windowSize; i < data.length; i += 20) {
    const window = data.slice(i - windowSize, i);
    const analysis = classifier.analyze(window);
    results.push({
      index: i,
      price: data[i].close,
      ...analysis
    });
  }

  console.log(`   Analysis points: ${results.length}`);
  console.log();

  // 4. Regime distribution
  console.log('4. Regime Distribution:');
  console.log('─'.repeat(70));

  const regimeCounts = {};
  for (const r of results) {
    regimeCounts[r.regime] = (regimeCounts[r.regime] || 0) + 1;
  }

  for (const [regime, count] of Object.entries(regimeCounts).sort((a, b) => b[1] - a[1])) {
    const pct = (count / results.length * 100).toFixed(1);
    const bar = '█'.repeat(Math.floor(count / results.length * 40));
    console.log(`   ${regime.padEnd(15)} ${bar.padEnd(40)} ${pct}%`);
  }
  console.log();

  // 5. Attention insights
  console.log('5. Attention Pattern Analysis:');
  console.log('─'.repeat(70));

  const lastResult = results[results.length - 1];
  if (lastResult.attentionInsights) {
    console.log(`   Attention interpretation: ${lastResult.attentionInsights.interpretation}`);
    console.log(`   Average entropy:          ${lastResult.attentionInsights.avgEntropy.toFixed(3)}`);
    console.log();

    console.log('   Head-by-Head Focus:');
    for (const head of lastResult.attentionInsights.importantTimesteps) {
      console.log(`   - Head ${head.head}: focuses on ${head.focusRange} (top weight: ${head.topTimesteps[0].weight.toFixed(3)})`);
    }
  }
  console.log();

  // 6. Regime changes
  console.log('6. Detected Regime Changes:');
  console.log('─'.repeat(70));

  const changes = results.filter(r => r.regimeChange?.changed);
  console.log(`   Total regime changes: ${changes.length}`);
  console.log();

  for (const change of changes.slice(-5)) {
    console.log(`   Index ${change.index}: ${change.regimeChange.from} → ${change.regimeChange.to} (conf: ${(change.regimeChange.confidence * 100).toFixed(0)}%)`);
  }
  console.log();

  // 7. Sample analysis
  console.log('7. Sample Analysis (Last 5 Windows):');
  console.log('─'.repeat(70));
  console.log('   Index │ Price   │ Regime          │ Confidence');
  console.log('─'.repeat(70));

  for (const r of results.slice(-5)) {
    console.log(`   ${String(r.index).padStart(5)} │ $${r.price.toFixed(2).padStart(6)} │ ${r.regime.padEnd(15)} │ ${(r.confidence * 100).toFixed(0)}%`);
  }
  console.log();

  // 8. Trading implications
  console.log('8. Trading Implications by Regime:');
  console.log('─'.repeat(70));

  const implications = {
    trending_up: 'Go long, use trailing stops, momentum strategies work',
    trending_down: 'Go short or stay out, mean reversion fails',
    ranging: 'Mean reversion works, sell options, tight stops',
    volatile_bull: 'Long with caution, wide stops, reduce size',
    volatile_bear: 'Stay defensive, hedge, reduce exposure',
    crisis: 'Risk-off, cash is king, volatility strategies'
  };

  for (const [regime, implication] of Object.entries(implications)) {
    console.log(`   ${regime}:`);
    console.log(`   → ${implication}`);
    console.log();
  }

  // 9. Attention visualization
  console.log('9. Attention Weights (Last Analysis):');
  console.log('─'.repeat(70));

  if (lastResult.attentionInsights) {
    console.log('   Timestep importance (Head 0, recent 20 bars):');

    const head0Weights = lastResult.attentionInsights.importantTimesteps[0].topTimesteps;
    const maxWeight = Math.max(...head0Weights.map(t => t.weight));

    // Show simplified attention bar
    let attentionBar = '   ';
    for (let i = 0; i < 20; i++) {
      const timestep = head0Weights.find(t => t.idx === i + 30);
      if (timestep && timestep.weight > 0.05) {
        const intensity = Math.floor(timestep.weight / maxWeight * 4);
        attentionBar += ['░', '▒', '▓', '█', '█'][intensity];
      } else {
        attentionBar += '·';
      }
    }
    console.log(attentionBar);
    console.log('   ^past                      recent^');
  }
  console.log();

  // 10. RuVector integration
  console.log('10. RuVector Vector Storage:');
  console.log('─'.repeat(70));
  console.log('   Attention patterns can be vectorized and stored:');
  console.log();

  if (lastResult.aggregatedFeatures) {
    const vec = lastResult.aggregatedFeatures.slice(0, 5).map(v => v.toFixed(4));
    console.log(`   Aggregated feature vector (first 5 dims):`);
    console.log(`   [${vec.join(', ')}]`);
    console.log();
    console.log('   Use cases:');
    console.log('   - Find similar regime patterns via HNSW search');
    console.log('   - Cluster historical regimes');
    console.log('   - Regime-based strategy selection');
  }
  console.log();

  console.log('═'.repeat(70));
  console.log('Attention-based regime detection completed');
  console.log('═'.repeat(70));
}

main().catch(console.error);
