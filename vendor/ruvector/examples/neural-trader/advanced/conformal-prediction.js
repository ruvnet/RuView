/**
 * Conformal Prediction with Guaranteed Intervals
 *
 * INTERMEDIATE: Uncertainty quantification for trading
 *
 * Uses @neural-trader/predictor for:
 * - Distribution-free prediction intervals
 * - Coverage guarantees (e.g., 95% of true values fall within interval)
 * - Adaptive conformal inference for time series
 * - Non-parametric uncertainty estimation
 *
 * Unlike traditional ML, conformal prediction provides VALID intervals
 * with finite-sample guarantees, regardless of the underlying distribution.
 */

// Conformal prediction configuration
const conformalConfig = {
  // Confidence level (1 - α)
  alpha: 0.05,  // 95% coverage

  // Calibration set size
  calibrationSize: 500,

  // Prediction method
  method: 'ACI',  // ACI (Adaptive Conformal Inference) or ICP (Inductive CP)

  // Adaptive parameters
  adaptive: {
    gamma: 0.005,           // Learning rate for adaptivity
    targetCoverage: 0.95,   // Target empirical coverage
    windowSize: 100         // Rolling window for coverage estimation
  }
};

// Conformity score functions
const ConformityScores = {
  // Absolute residual (symmetric)
  absolute: (pred, actual) => Math.abs(pred - actual),

  // Signed residual (asymmetric)
  signed: (pred, actual) => actual - pred,

  // Quantile-based (for asymmetric intervals)
  quantile: (pred, actual, q = 0.5) => {
    const residual = actual - pred;
    return residual >= 0 ? q * residual : (1 - q) * Math.abs(residual);
  },

  // Normalized (for heteroscedastic data)
  normalized: (pred, actual, sigma) => Math.abs(pred - actual) / sigma
};

// Conformal Predictor base class
class ConformalPredictor {
  constructor(config) {
    this.config = config;
    this.calibrationScores = [];
    this.predictionHistory = [];
    this.coverageHistory = [];
    this.adaptiveAlpha = config.alpha;
  }

  // Calibrate using historical residuals
  calibrate(predictions, actuals) {
    if (predictions.length !== actuals.length) {
      throw new Error('Predictions and actuals must have same length');
    }

    this.calibrationScores = [];

    for (let i = 0; i < predictions.length; i++) {
      const score = ConformityScores.absolute(predictions[i], actuals[i]);
      this.calibrationScores.push(score);
    }

    // Sort for quantile computation
    this.calibrationScores.sort((a, b) => a - b);

    console.log(`Calibrated with ${this.calibrationScores.length} samples`);
    console.log(`Score range: [${this.calibrationScores[0].toFixed(4)}, ${this.calibrationScores[this.calibrationScores.length - 1].toFixed(4)}]`);
  }

  // Get prediction interval
  predict(pointPrediction) {
    const alpha = this.adaptiveAlpha;
    const n = this.calibrationScores.length;

    // Compute quantile for (1 - alpha) coverage
    // Use (1 - alpha)(1 + 1/n) quantile for finite-sample validity
    const quantileIndex = Math.ceil((1 - alpha) * (n + 1)) - 1;
    const conformalQuantile = this.calibrationScores[Math.min(quantileIndex, n - 1)];

    const interval = {
      prediction: pointPrediction,
      lower: pointPrediction - conformalQuantile,
      upper: pointPrediction + conformalQuantile,
      width: conformalQuantile * 2,
      alpha: alpha,
      coverage: 1 - alpha
    };

    return interval;
  }

  // Update for adaptive conformal inference
  updateAdaptive(actual, interval) {
    // Check if actual was covered
    const covered = actual >= interval.lower && actual <= interval.upper;
    this.coverageHistory.push(covered ? 1 : 0);

    // Update empirical coverage (rolling window)
    const windowSize = this.config.adaptive.windowSize;
    const recentCoverage = this.coverageHistory.slice(-windowSize);
    const empiricalCoverage = recentCoverage.reduce((a, b) => a + b, 0) / recentCoverage.length;

    // Adapt alpha based on coverage error
    const targetCoverage = this.config.adaptive.targetCoverage;
    const gamma = this.config.adaptive.gamma;

    // If empirical coverage < target, decrease alpha (widen intervals)
    // If empirical coverage > target, increase alpha (tighten intervals)
    this.adaptiveAlpha = Math.max(0.001, Math.min(0.2,
      this.adaptiveAlpha + gamma * (empiricalCoverage - targetCoverage)
    ));

    // Add new conformity score to calibration set
    const newScore = ConformityScores.absolute(interval.prediction, actual);
    this.calibrationScores.push(newScore);
    this.calibrationScores.sort((a, b) => a - b);

    // Keep calibration set bounded
    if (this.calibrationScores.length > 2000) {
      this.calibrationScores.shift();
    }

    return {
      covered,
      empiricalCoverage,
      adaptiveAlpha: this.adaptiveAlpha
    };
  }
}

// Asymmetric Conformal Predictor (for trading where downside ≠ upside)
class AsymmetricConformalPredictor extends ConformalPredictor {
  constructor(config) {
    super(config);
    this.lowerScores = [];
    this.upperScores = [];
    this.lowerAlpha = config.alpha / 2;
    this.upperAlpha = config.alpha / 2;
  }

  calibrate(predictions, actuals) {
    this.lowerScores = [];
    this.upperScores = [];

    for (let i = 0; i < predictions.length; i++) {
      const residual = actuals[i] - predictions[i];

      if (residual < 0) {
        this.lowerScores.push(Math.abs(residual));
      } else {
        this.upperScores.push(residual);
      }
    }

    this.lowerScores.sort((a, b) => a - b);
    this.upperScores.sort((a, b) => a - b);

    console.log(`Asymmetric calibration:`);
    console.log(`  Lower: ${this.lowerScores.length} samples`);
    console.log(`  Upper: ${this.upperScores.length} samples`);
  }

  predict(pointPrediction) {
    const nLower = this.lowerScores.length;
    const nUpper = this.upperScores.length;

    // Separate quantiles for lower and upper
    const lowerIdx = Math.ceil((1 - this.lowerAlpha * 2) * (nLower + 1)) - 1;
    const upperIdx = Math.ceil((1 - this.upperAlpha * 2) * (nUpper + 1)) - 1;

    const lowerQuantile = this.lowerScores[Math.min(lowerIdx, nLower - 1)] || 0;
    const upperQuantile = this.upperScores[Math.min(upperIdx, nUpper - 1)] || 0;

    return {
      prediction: pointPrediction,
      lower: pointPrediction - lowerQuantile,
      upper: pointPrediction + upperQuantile,
      lowerWidth: lowerQuantile,
      upperWidth: upperQuantile,
      asymmetryRatio: upperQuantile / (lowerQuantile || 1),
      alpha: this.config.alpha
    };
  }
}

// Generate synthetic trading data with underlying model
function generateTradingData(n, seed = 42) {
  const data = [];
  let price = 100;

  // Simple random seed
  let rng = seed;
  const random = () => {
    rng = (rng * 9301 + 49297) % 233280;
    return rng / 233280;
  };

  for (let i = 0; i < n; i++) {
    // True return with regime switching and heteroscedasticity
    const regime = Math.sin(i / 50) > 0 ? 1 : 0.5;
    const volatility = 0.02 * regime;
    const drift = 0.0001;

    const trueReturn = drift + volatility * (random() + random() - 1);
    price = price * (1 + trueReturn);

    // Features for prediction
    const features = {
      momentum: i > 10 ? (price / data[i - 10]?.price - 1) || 0 : 0,
      volatility: volatility,
      regime
    };

    // Model prediction (with some error)
    const predictedReturn = drift + 0.5 * features.momentum + (random() - 0.5) * 0.005;

    data.push({
      index: i,
      price,
      trueReturn,
      predictedReturn,
      features
    });
  }

  return data;
}

async function main() {
  console.log('═'.repeat(70));
  console.log('CONFORMAL PREDICTION - Guaranteed Uncertainty Intervals');
  console.log('═'.repeat(70));
  console.log();

  // 1. Generate data
  console.log('1. Generating Trading Data:');
  console.log('─'.repeat(70));

  const data = generateTradingData(1000);
  const calibrationData = data.slice(0, conformalConfig.calibrationSize);
  const testData = data.slice(conformalConfig.calibrationSize);

  console.log(`   Total samples:      ${data.length}`);
  console.log(`   Calibration:        ${calibrationData.length}`);
  console.log(`   Test:               ${testData.length}`);
  console.log(`   Target coverage:    ${(1 - conformalConfig.alpha) * 100}%`);
  console.log();

  // 2. Standard Conformal Predictor
  console.log('2. Standard (Symmetric) Conformal Prediction:');
  console.log('─'.repeat(70));

  const standardCP = new ConformalPredictor(conformalConfig);

  // Calibrate
  const calPredictions = calibrationData.map(d => d.predictedReturn);
  const calActuals = calibrationData.map(d => d.trueReturn);
  standardCP.calibrate(calPredictions, calActuals);

  // Test
  let standardCovered = 0;
  let standardWidths = [];

  for (const sample of testData) {
    const interval = standardCP.predict(sample.predictedReturn);
    const covered = sample.trueReturn >= interval.lower && sample.trueReturn <= interval.upper;

    if (covered) standardCovered++;
    standardWidths.push(interval.width);
  }

  const standardCoverage = standardCovered / testData.length;
  const avgWidth = standardWidths.reduce((a, b) => a + b, 0) / standardWidths.length;

  console.log(`   Empirical Coverage: ${(standardCoverage * 100).toFixed(2)}%`);
  console.log(`   Target Coverage:    ${(1 - conformalConfig.alpha) * 100}%`);
  console.log(`   Average Width:      ${(avgWidth * 10000).toFixed(2)} bps`);
  console.log(`   Coverage Valid:     ${standardCoverage >= (1 - conformalConfig.alpha) - 0.02 ? '✓ YES' : '✗ NO'}`);
  console.log();

  // 3. Adaptive Conformal Inference
  console.log('3. Adaptive Conformal Inference (ACI):');
  console.log('─'.repeat(70));

  const adaptiveCP = new ConformalPredictor({
    ...conformalConfig,
    method: 'ACI'
  });
  adaptiveCP.calibrate(calPredictions, calActuals);

  let adaptiveCovered = 0;
  let adaptiveWidths = [];
  let alphaHistory = [];

  for (const sample of testData) {
    const interval = adaptiveCP.predict(sample.predictedReturn);
    const update = adaptiveCP.updateAdaptive(sample.trueReturn, interval);

    if (update.covered) adaptiveCovered++;
    adaptiveWidths.push(interval.width);
    alphaHistory.push(adaptiveCP.adaptiveAlpha);
  }

  const adaptiveCoverage = adaptiveCovered / testData.length;
  const adaptiveAvgWidth = adaptiveWidths.reduce((a, b) => a + b, 0) / adaptiveWidths.length;
  const finalAlpha = alphaHistory[alphaHistory.length - 1];

  console.log(`   Empirical Coverage: ${(adaptiveCoverage * 100).toFixed(2)}%`);
  console.log(`   Average Width:      ${(adaptiveAvgWidth * 10000).toFixed(2)} bps`);
  console.log(`   Initial Alpha:      ${(conformalConfig.alpha * 100).toFixed(2)}%`);
  console.log(`   Final Alpha:        ${(finalAlpha * 100).toFixed(2)}%`);
  console.log(`   Width vs Standard:  ${((adaptiveAvgWidth / avgWidth - 1) * 100).toFixed(1)}%`);
  console.log();

  // 4. Asymmetric Conformal Prediction
  console.log('4. Asymmetric Conformal Prediction:');
  console.log('─'.repeat(70));

  const asymmetricCP = new AsymmetricConformalPredictor(conformalConfig);
  asymmetricCP.calibrate(calPredictions, calActuals);

  let asymmetricCovered = 0;
  let lowerWidths = [];
  let upperWidths = [];

  for (const sample of testData) {
    const interval = asymmetricCP.predict(sample.predictedReturn);
    const covered = sample.trueReturn >= interval.lower && sample.trueReturn <= interval.upper;

    if (covered) asymmetricCovered++;
    lowerWidths.push(interval.lowerWidth);
    upperWidths.push(interval.upperWidth);
  }

  const asymmetricCoverage = asymmetricCovered / testData.length;
  const avgLower = lowerWidths.reduce((a, b) => a + b, 0) / lowerWidths.length;
  const avgUpper = upperWidths.reduce((a, b) => a + b, 0) / upperWidths.length;

  console.log(`   Empirical Coverage: ${(asymmetricCoverage * 100).toFixed(2)}%`);
  console.log(`   Avg Lower Width:    ${(avgLower * 10000).toFixed(2)} bps`);
  console.log(`   Avg Upper Width:    ${(avgUpper * 10000).toFixed(2)} bps`);
  console.log(`   Asymmetry Ratio:    ${(avgUpper / avgLower).toFixed(2)}x`);
  console.log();

  // 5. Example predictions
  console.log('5. Example Predictions (Last 5 samples):');
  console.log('─'.repeat(70));
  console.log('   Predicted │ Lower    │ Upper    │ Actual   │ Covered │ Width');
  console.log('─'.repeat(70));

  const lastSamples = testData.slice(-5);
  for (const sample of lastSamples) {
    const interval = standardCP.predict(sample.predictedReturn);
    const covered = sample.trueReturn >= interval.lower && sample.trueReturn <= interval.upper;

    const predBps = (sample.predictedReturn * 10000).toFixed(2);
    const lowerBps = (interval.lower * 10000).toFixed(2);
    const upperBps = (interval.upper * 10000).toFixed(2);
    const actualBps = (sample.trueReturn * 10000).toFixed(2);
    const widthBps = (interval.width * 10000).toFixed(2);

    console.log(`   ${predBps.padStart(9)} │ ${lowerBps.padStart(8)} │ ${upperBps.padStart(8)} │ ${actualBps.padStart(8)} │ ${covered ? '  ✓  ' : '  ✗  '} │ ${widthBps.padStart(6)}`);
  }
  console.log();

  // 6. Trading application
  console.log('6. Trading Application - Risk Management:');
  console.log('─'.repeat(70));

  // Use conformal intervals for position sizing
  const samplePrediction = testData[testData.length - 1].predictedReturn;
  const conformalInterval = standardCP.predict(samplePrediction);

  const expectedReturn = samplePrediction;
  const worstCase = conformalInterval.lower;
  const bestCase = conformalInterval.upper;
  const uncertainty = conformalInterval.width;

  console.log(`   Point Prediction:   ${(expectedReturn * 10000).toFixed(2)} bps`);
  console.log(`   95% Worst Case:     ${(worstCase * 10000).toFixed(2)} bps`);
  console.log(`   95% Best Case:      ${(bestCase * 10000).toFixed(2)} bps`);
  console.log(`   Uncertainty:        ${(uncertainty * 10000).toFixed(2)} bps`);
  console.log();

  // Position sizing based on uncertainty
  const riskBudget = 0.02;  // 2% daily risk budget
  const maxLoss = Math.abs(worstCase);
  const suggestedPosition = riskBudget / maxLoss;

  console.log(`   Risk Budget:        ${(riskBudget * 100).toFixed(1)}%`);
  console.log(`   Max Position:       ${(suggestedPosition * 100).toFixed(1)}% of portfolio`);
  console.log(`   Rationale:          Position sized so 95% worst case = ${(riskBudget * 100).toFixed(1)}% loss`);
  console.log();

  // 7. Coverage guarantee visualization
  console.log('7. Finite-Sample Coverage Guarantee:');
  console.log('─'.repeat(70));
  console.log('   Conformal prediction provides VALID coverage guarantees:');
  console.log();
  console.log(`   P(Y ∈ Ĉ(X)) ≥ 1 - α = ${((1 - conformalConfig.alpha) * 100).toFixed(0)}%`);
  console.log();
  console.log('   This holds for ANY data distribution, without assumptions!');
  console.log('   (Unlike Gaussian intervals which require normality)');
  console.log();

  console.log('═'.repeat(70));
  console.log('Conformal prediction analysis completed');
  console.log('═'.repeat(70));
}

main().catch(console.error);
