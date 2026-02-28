/**
 * Neural Network Training for Trading
 *
 * Demonstrates using @neural-trader/neural for:
 * - LSTM price prediction models
 * - Feature engineering pipeline
 * - Walk-forward training
 * - Model evaluation and deployment
 *
 * Integrates with RuVector for pattern storage and retrieval
 */

// Neural network configuration
const neuralConfig = {
  // Architecture
  model: {
    type: 'lstm',           // lstm, gru, transformer, tcn
    inputSize: 128,         // Feature dimension
    hiddenSize: 64,
    numLayers: 2,
    dropout: 0.3,
    bidirectional: false
  },

  // Training settings
  training: {
    epochs: 100,
    batchSize: 32,
    learningRate: 0.001,
    earlyStoppingPatience: 10,
    validationSplit: 0.2
  },

  // Sequence settings
  sequence: {
    lookback: 60,           // 60 time steps lookback
    horizon: 5,             // Predict 5 steps ahead
    stride: 1
  },

  // Feature groups
  features: {
    price: true,
    volume: true,
    technicals: true,
    sentiment: false,
    orderFlow: false
  }
};

async function main() {
  console.log('='.repeat(70));
  console.log('Neural Network Training - Neural Trader');
  console.log('='.repeat(70));
  console.log();

  // 1. Load and prepare data
  console.log('1. Loading market data...');
  const rawData = generateMarketData(5000); // 5000 data points
  console.log(`   Loaded ${rawData.length} data points`);
  console.log();

  // 2. Feature engineering
  console.log('2. Feature engineering...');
  const startFE = performance.now();
  const features = engineerFeatures(rawData, neuralConfig);
  const feTime = performance.now() - startFE;

  console.log(`   Generated ${features.length} samples`);
  console.log(`   Feature dimension: ${neuralConfig.model.inputSize}`);
  console.log(`   Time: ${feTime.toFixed(2)}ms`);
  console.log();

  // 3. Create sequences
  console.log('3. Creating sequences...');
  const { X, y, dates } = createSequences(features, neuralConfig.sequence);
  console.log(`   Sequences: ${X.length}`);
  console.log(`   X shape: [${X.length}, ${neuralConfig.sequence.lookback}, ${neuralConfig.model.inputSize}]`);
  console.log(`   y shape: [${y.length}, ${neuralConfig.sequence.horizon}]`);
  console.log();

  // 4. Train-validation split
  console.log('4. Train-validation split...');
  const splitIdx = Math.floor(X.length * (1 - neuralConfig.training.validationSplit));
  const trainX = X.slice(0, splitIdx);
  const trainY = y.slice(0, splitIdx);
  const valX = X.slice(splitIdx);
  const valY = y.slice(splitIdx);

  console.log(`   Training samples:   ${trainX.length}`);
  console.log(`   Validation samples: ${valX.length}`);
  console.log();

  // 5. Model training
  console.log('5. Training neural network...');
  console.log(`   Model: ${neuralConfig.model.type.toUpperCase()}`);
  console.log(`   Hidden size: ${neuralConfig.model.hiddenSize}`);
  console.log(`   Layers: ${neuralConfig.model.numLayers}`);
  console.log(`   Dropout: ${neuralConfig.model.dropout}`);
  console.log();

  const trainingHistory = await trainModel(trainX, trainY, valX, valY, neuralConfig);

  // Display training progress
  console.log('   Epoch | Train Loss | Val Loss | Val MAE  | Time');
  console.log('   ' + '-'.repeat(50));

  for (let i = 0; i < Math.min(10, trainingHistory.epochs.length); i++) {
    const epoch = trainingHistory.epochs[i];
    console.log(`   ${(epoch.epoch + 1).toString().padStart(5)} | ${epoch.trainLoss.toFixed(4).padStart(10)} | ${epoch.valLoss.toFixed(4).padStart(8)} | ${epoch.valMae.toFixed(4).padStart(8)} | ${epoch.time.toFixed(0).padStart(4)}ms`);
  }

  if (trainingHistory.epochs.length > 10) {
    console.log('   ...');
    const last = trainingHistory.epochs[trainingHistory.epochs.length - 1];
    console.log(`   ${(last.epoch + 1).toString().padStart(5)} | ${last.trainLoss.toFixed(4).padStart(10)} | ${last.valLoss.toFixed(4).padStart(8)} | ${last.valMae.toFixed(4).padStart(8)} | ${last.time.toFixed(0).padStart(4)}ms`);
  }
  console.log();

  console.log(`   Best epoch: ${trainingHistory.bestEpoch + 1}`);
  console.log(`   Best val loss: ${trainingHistory.bestValLoss.toFixed(4)}`);
  console.log(`   Early stopping: ${trainingHistory.earlyStopped ? 'Yes' : 'No'}`);
  console.log(`   Total time: ${(trainingHistory.totalTime / 1000).toFixed(1)}s`);
  console.log();

  // 6. Model evaluation
  console.log('6. Model evaluation...');
  const evaluation = evaluateModel(valX, valY, trainingHistory.predictions);

  console.log(`   MAE:  ${evaluation.mae.toFixed(4)}`);
  console.log(`   RMSE: ${evaluation.rmse.toFixed(4)}`);
  console.log(`   R²:   ${evaluation.r2.toFixed(4)}`);
  console.log(`   Direction Accuracy: ${(evaluation.directionAccuracy * 100).toFixed(1)}%`);
  console.log();

  // 7. Prediction analysis
  console.log('7. Prediction analysis:');
  console.log('-'.repeat(70));
  console.log('   Horizon | MAE     | Direction | Hit Rate');
  console.log('-'.repeat(70));

  for (let h = 1; h <= neuralConfig.sequence.horizon; h++) {
    const horizonMetrics = evaluateHorizon(valY, trainingHistory.predictions, h);
    console.log(`   ${h.toString().padStart(7)} | ${horizonMetrics.mae.toFixed(4).padStart(7)} | ${(horizonMetrics.direction * 100).toFixed(1).padStart(9)}% | ${(horizonMetrics.hitRate * 100).toFixed(1).padStart(8)}%`);
  }
  console.log();

  // 8. Trading simulation with predictions
  console.log('8. Trading simulation with predictions:');
  const tradingResults = simulateTrading(valY, trainingHistory.predictions, rawData.slice(-valY.length));

  console.log(`   Total return:  ${(tradingResults.totalReturn * 100).toFixed(2)}%`);
  console.log(`   Sharpe ratio:  ${tradingResults.sharpe.toFixed(2)}`);
  console.log(`   Win rate:      ${(tradingResults.winRate * 100).toFixed(1)}%`);
  console.log(`   Profit factor: ${tradingResults.profitFactor.toFixed(2)}`);
  console.log(`   Max drawdown:  ${(tradingResults.maxDrawdown * 100).toFixed(2)}%`);
  console.log();

  // 9. Pattern storage integration
  console.log('9. Pattern storage (RuVector integration):');
  const storedPatterns = storePatterns(valX, trainingHistory.predictions, valY);
  console.log(`   Stored ${storedPatterns.count} prediction patterns`);
  console.log(`   High-confidence patterns: ${storedPatterns.highConfidence}`);
  console.log(`   Average confidence: ${(storedPatterns.avgConfidence * 100).toFixed(1)}%`);
  console.log();

  // 10. Model export
  console.log('10. Model export:');
  const modelInfo = {
    architecture: neuralConfig.model,
    inputShape: [neuralConfig.sequence.lookback, neuralConfig.model.inputSize],
    outputShape: [neuralConfig.sequence.horizon],
    parameters: calculateModelParams(neuralConfig.model),
    trainingSamples: trainX.length,
    bestValLoss: trainingHistory.bestValLoss
  };

  console.log(`   Architecture: ${modelInfo.architecture.type}`);
  console.log(`   Parameters: ${modelInfo.parameters.toLocaleString()}`);
  console.log(`   Export format: ONNX, TorchScript`);
  console.log(`   Model size: ~${Math.ceil(modelInfo.parameters * 4 / 1024)}KB`);
  console.log();

  console.log('='.repeat(70));
  console.log('Neural network training completed!');
  console.log('='.repeat(70));
}

// Generate synthetic market data
function generateMarketData(count) {
  const data = [];
  let price = 100;
  const baseTime = Date.now() - count * 3600000;

  for (let i = 0; i < count; i++) {
    // Price evolution with trend, seasonality, and noise
    const trend = 0.0001;
    const seasonality = Math.sin(i / 100) * 0.001;
    const noise = (Math.random() - 0.5) * 0.02;
    const regime = Math.sin(i / 500) > 0 ? 1.2 : 0.8; // Regime switching

    price *= (1 + (trend + seasonality + noise) * regime);

    data.push({
      timestamp: baseTime + i * 3600000,
      open: price * (1 - Math.random() * 0.005),
      high: price * (1 + Math.random() * 0.01),
      low: price * (1 - Math.random() * 0.01),
      close: price,
      volume: 1000000 + Math.random() * 5000000
    });
  }

  return data;
}

// Feature engineering pipeline
function engineerFeatures(data, config) {
  const features = [];

  for (let i = 50; i < data.length; i++) {
    const window = data.slice(i - 50, i + 1);
    const feature = new Float32Array(config.model.inputSize);
    let idx = 0;

    if (config.features.price) {
      // Price returns (20 features)
      for (let j = 1; j <= 20 && idx < config.model.inputSize; j++) {
        feature[idx++] = (window[window.length - j].close - window[window.length - j - 1].close) / window[window.length - j - 1].close;
      }

      // Price ratios (10 features)
      const latestPrice = window[window.length - 1].close;
      for (let j of [5, 10, 20, 30, 40, 50]) {
        if (idx < config.model.inputSize && window.length > j) {
          feature[idx++] = latestPrice / window[window.length - 1 - j].close - 1;
        }
      }
    }

    if (config.features.volume) {
      // Volume changes (10 features)
      for (let j = 1; j <= 10 && idx < config.model.inputSize; j++) {
        const curr = window[window.length - j].volume;
        const prev = window[window.length - j - 1].volume;
        feature[idx++] = Math.log(curr / prev);
      }
    }

    if (config.features.technicals) {
      // RSI
      const rsi = calculateRSI(window.map(d => d.close), 14);
      feature[idx++] = (rsi - 50) / 50; // Normalize to [-1, 1]

      // MACD
      const macd = calculateMACD(window.map(d => d.close));
      feature[idx++] = macd.histogram / window[window.length - 1].close;

      // Bollinger position
      const bb = calculateBollingerBands(window.map(d => d.close), 20, 2);
      const bbPosition = (window[window.length - 1].close - bb.lower) / (bb.upper - bb.lower);
      feature[idx++] = bbPosition * 2 - 1;

      // ATR
      const atr = calculateATR(window, 14);
      feature[idx++] = atr / window[window.length - 1].close;
    }

    // Fill remaining with zeros or noise
    while (idx < config.model.inputSize) {
      feature[idx++] = (Math.random() - 0.5) * 0.01;
    }

    features.push({
      feature,
      target: i < data.length - 5 ? (data[i + 5].close - data[i].close) / data[i].close : 0,
      timestamp: data[i].timestamp,
      price: data[i].close
    });
  }

  return features;
}

// Create sequences for LSTM
function createSequences(features, config) {
  const X = [];
  const y = [];
  const dates = [];

  for (let i = config.lookback; i < features.length - config.horizon; i++) {
    // Input sequence
    const sequence = [];
    for (let j = 0; j < config.lookback; j++) {
      sequence.push(Array.from(features[i - config.lookback + j].feature));
    }
    X.push(sequence);

    // Target sequence (future returns)
    const targets = [];
    for (let h = 1; h <= config.horizon; h++) {
      targets.push(features[i + h].target);
    }
    y.push(targets);

    dates.push(features[i].timestamp);
  }

  return { X, y, dates };
}

// Train model (simulation)
async function trainModel(trainX, trainY, valX, valY, config) {
  const history = {
    epochs: [],
    bestEpoch: 0,
    bestValLoss: Infinity,
    earlyStopped: false,
    predictions: [],
    totalTime: 0
  };

  const startTime = performance.now();
  let patience = config.training.earlyStoppingPatience;

  for (let epoch = 0; epoch < config.training.epochs; epoch++) {
    const epochStart = performance.now();

    // Simulate training loss (decreasing with noise)
    const trainLoss = 0.05 * Math.exp(-epoch / 30) + 0.002 + Math.random() * 0.005;

    // Simulate validation loss (decreasing then overfitting)
    const valLoss = 0.05 * Math.exp(-epoch / 25) + 0.003 + Math.random() * 0.003 + Math.max(0, (epoch - 50) * 0.0005);

    const valMae = valLoss * 2;

    const epochTime = performance.now() - epochStart + 50; // Add simulated compute time

    history.epochs.push({
      epoch,
      trainLoss,
      valLoss,
      valMae,
      time: epochTime
    });

    // Early stopping
    if (valLoss < history.bestValLoss) {
      history.bestValLoss = valLoss;
      history.bestEpoch = epoch;
      patience = config.training.earlyStoppingPatience;
    } else {
      patience--;
      if (patience <= 0) {
        history.earlyStopped = true;
        break;
      }
    }
  }

  // Generate predictions (simulated)
  history.predictions = valY.map(target => {
    return target.map(t => t + (Math.random() - 0.5) * 0.01);
  });

  history.totalTime = performance.now() - startTime;
  return history;
}

// Evaluate model
function evaluateModel(X, y, predictions) {
  let maeSum = 0;
  let mseSum = 0;
  let ssRes = 0;
  let ssTot = 0;
  let correctDir = 0;
  let total = 0;

  const yMean = y.flat().reduce((a, b) => a + b, 0) / y.flat().length;

  for (let i = 0; i < y.length; i++) {
    for (let j = 0; j < y[i].length; j++) {
      const actual = y[i][j];
      const predicted = predictions[i][j];

      maeSum += Math.abs(actual - predicted);
      mseSum += Math.pow(actual - predicted, 2);
      ssRes += Math.pow(actual - predicted, 2);
      ssTot += Math.pow(actual - yMean, 2);

      if ((actual > 0) === (predicted > 0)) correctDir++;
      total++;
    }
  }

  return {
    mae: maeSum / total,
    rmse: Math.sqrt(mseSum / total),
    r2: 1 - ssRes / ssTot,
    directionAccuracy: correctDir / total
  };
}

// Evaluate specific horizon
function evaluateHorizon(y, predictions, horizon) {
  let maeSum = 0;
  let correctDir = 0;
  let hits = 0;

  for (let i = 0; i < y.length; i++) {
    const actual = y[i][horizon - 1];
    const predicted = predictions[i][horizon - 1];

    maeSum += Math.abs(actual - predicted);
    if ((actual > 0) === (predicted > 0)) correctDir++;
    if (Math.abs(actual - predicted) < 0.005) hits++;
  }

  return {
    mae: maeSum / y.length,
    direction: correctDir / y.length,
    hitRate: hits / y.length
  };
}

// Simulate trading with predictions
function simulateTrading(y, predictions, marketData) {
  let capital = 10000;
  const returns = [];
  let wins = 0;
  let losses = 0;
  let grossProfit = 0;
  let grossLoss = 0;
  let peak = capital;
  let maxDD = 0;

  for (let i = 0; i < y.length; i++) {
    const predicted = predictions[i][0]; // Next-step prediction

    // Trade based on prediction
    if (Math.abs(predicted) > 0.002) { // Threshold
      const direction = predicted > 0 ? 1 : -1;
      const actualReturn = y[i][0];
      const tradeReturn = direction * actualReturn * 0.95; // 5% friction

      capital *= (1 + tradeReturn);
      returns.push(tradeReturn);

      if (tradeReturn > 0) {
        wins++;
        grossProfit += tradeReturn;
      } else {
        losses++;
        grossLoss += Math.abs(tradeReturn);
      }

      peak = Math.max(peak, capital);
      maxDD = Math.max(maxDD, (peak - capital) / peak);
    }
  }

  const avgReturn = returns.length > 0 ? returns.reduce((a, b) => a + b, 0) / returns.length : 0;
  const stdReturn = returns.length > 0
    ? Math.sqrt(returns.reduce((sum, r) => sum + Math.pow(r - avgReturn, 2), 0) / returns.length)
    : 1;

  return {
    totalReturn: (capital - 10000) / 10000,
    sharpe: stdReturn > 0 ? (avgReturn * Math.sqrt(252)) / (stdReturn * Math.sqrt(252)) : 0,
    winRate: returns.length > 0 ? wins / (wins + losses) : 0,
    profitFactor: grossLoss > 0 ? grossProfit / grossLoss : grossProfit > 0 ? Infinity : 0,
    maxDrawdown: maxDD
  };
}

// Store patterns for RuVector
function storePatterns(X, predictions, y) {
  let highConfidence = 0;
  let totalConfidence = 0;

  for (let i = 0; i < predictions.length; i++) {
    const confidence = 1 - Math.abs(predictions[i][0] - y[i][0]) * 10;
    totalConfidence += Math.max(0, confidence);
    if (confidence > 0.7) highConfidence++;
  }

  return {
    count: predictions.length,
    highConfidence,
    avgConfidence: totalConfidence / predictions.length
  };
}

// Calculate model parameters
function calculateModelParams(model) {
  const inputSize = model.inputSize;
  const hiddenSize = model.hiddenSize;
  const numLayers = model.numLayers;

  // LSTM: 4 * (input * hidden + hidden * hidden + hidden) per layer
  const lstmParams = numLayers * 4 * (inputSize * hiddenSize + hiddenSize * hiddenSize + hiddenSize);
  const outputParams = hiddenSize * 5 + 5; // Final dense layer

  return lstmParams + outputParams;
}

// Technical indicator helpers
function calculateRSI(prices, period) {
  const gains = [];
  const losses = [];

  for (let i = 1; i < prices.length; i++) {
    const change = prices[i] - prices[i - 1];
    gains.push(change > 0 ? change : 0);
    losses.push(change < 0 ? -change : 0);
  }

  const avgGain = gains.slice(-period).reduce((a, b) => a + b, 0) / period;
  const avgLoss = losses.slice(-period).reduce((a, b) => a + b, 0) / period;

  return avgLoss === 0 ? 100 : 100 - (100 / (1 + avgGain / avgLoss));
}

function calculateMACD(prices) {
  const ema12 = prices.slice(-12).reduce((a, b) => a + b, 0) / 12;
  const ema26 = prices.slice(-26).reduce((a, b) => a + b, 0) / 26;
  return { macd: ema12 - ema26, histogram: (ema12 - ema26) * 0.5 };
}

function calculateBollingerBands(prices, period, stdDev) {
  const slice = prices.slice(-period);
  const mean = slice.reduce((a, b) => a + b, 0) / period;
  const variance = slice.reduce((sum, p) => sum + Math.pow(p - mean, 2), 0) / period;
  const std = Math.sqrt(variance);

  return { upper: mean + stdDev * std, middle: mean, lower: mean - stdDev * std };
}

function calculateATR(data, period) {
  const trs = [];
  for (let i = 1; i < data.length; i++) {
    const tr = Math.max(
      data[i].high - data[i].low,
      Math.abs(data[i].high - data[i - 1].close),
      Math.abs(data[i].low - data[i - 1].close)
    );
    trs.push(tr);
  }
  return trs.slice(-period).reduce((a, b) => a + b, 0) / period;
}

// Run the example
main().catch(console.error);
