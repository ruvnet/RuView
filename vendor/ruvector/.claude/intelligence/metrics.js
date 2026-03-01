#!/usr/bin/env node
/**
 * RuVector Intelligence Metrics
 *
 * Tracks effectiveness of the learning system:
 * - Prediction accuracy (did suggestions help?)
 * - Command success rate trends
 * - Agent routing accuracy
 * - Time-series analysis
 */

import { readFileSync, writeFileSync, existsSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const DATA_DIR = join(__dirname, 'data');
const METRICS_FILE = join(DATA_DIR, 'metrics.json');

/**
 * Load or initialize metrics
 */
function loadMetrics() {
  if (existsSync(METRICS_FILE)) {
    return JSON.parse(readFileSync(METRICS_FILE, 'utf-8'));
  }
  return {
    created: new Date().toISOString(),
    predictions: [],        // { predicted, actual, correct, timestamp }
    commandOutcomes: [],    // { type, success, hadWarning, timestamp }
    agentRoutings: [],      // { recommended, used, success, timestamp }
    dailyStats: {},         // { "2025-01-15": { commands: 10, successes: 8, ... } }
    calibration: {},        // { bucket: { predicted: 0.8, actual: 0.75 } }
  };
}

/**
 * Save metrics
 */
function saveMetrics(metrics) {
  metrics.lastUpdated = new Date().toISOString();
  writeFileSync(METRICS_FILE, JSON.stringify(metrics, null, 2));
}

/**
 * Record a prediction outcome
 */
export function recordPrediction(predicted, actual, metadata = {}) {
  const metrics = loadMetrics();
  const correct = predicted === actual;

  metrics.predictions.push({
    predicted,
    actual,
    correct,
    confidence: metadata.confidence || 0,
    timestamp: new Date().toISOString(),
    ...metadata
  });

  // Keep last 1000 predictions
  if (metrics.predictions.length > 1000) {
    metrics.predictions = metrics.predictions.slice(-1000);
  }

  // Update calibration buckets
  const bucket = Math.floor((metadata.confidence || 0) * 10) / 10; // 0.0, 0.1, ..., 0.9
  if (!metrics.calibration[bucket]) {
    metrics.calibration[bucket] = { total: 0, correct: 0 };
  }
  metrics.calibration[bucket].total++;
  if (correct) metrics.calibration[bucket].correct++;

  saveMetrics(metrics);
  return correct;
}

/**
 * Record command outcome with context
 */
export function recordCommandOutcome(cmdType, success, context = {}) {
  const metrics = loadMetrics();
  const today = new Date().toISOString().split('T')[0];

  metrics.commandOutcomes.push({
    type: cmdType,
    success,
    hadWarning: context.hadWarning || false,
    followedAdvice: context.followedAdvice,
    timestamp: new Date().toISOString()
  });

  // Keep last 2000 outcomes
  if (metrics.commandOutcomes.length > 2000) {
    metrics.commandOutcomes = metrics.commandOutcomes.slice(-2000);
  }

  // Update daily stats
  if (!metrics.dailyStats[today]) {
    metrics.dailyStats[today] = {
      commands: 0,
      successes: 0,
      withWarning: 0,
      warningHeeded: 0,
      warningHeededSuccess: 0
    };
  }
  metrics.dailyStats[today].commands++;
  if (success) metrics.dailyStats[today].successes++;
  if (context.hadWarning) {
    metrics.dailyStats[today].withWarning++;
    if (context.followedAdvice) {
      metrics.dailyStats[today].warningHeeded++;
      if (success) metrics.dailyStats[today].warningHeededSuccess++;
    }
  }

  saveMetrics(metrics);
}

/**
 * Record agent routing outcome
 */
export function recordAgentRouting(recommended, actualUsed, success) {
  const metrics = loadMetrics();

  metrics.agentRoutings.push({
    recommended,
    used: actualUsed,
    followed: recommended === actualUsed,
    success,
    timestamp: new Date().toISOString()
  });

  // Keep last 500 routings
  if (metrics.agentRoutings.length > 500) {
    metrics.agentRoutings = metrics.agentRoutings.slice(-500);
  }

  saveMetrics(metrics);
}

/**
 * Calculate effectiveness metrics
 */
export function calculateEffectiveness() {
  const metrics = loadMetrics();
  const results = {
    generated: new Date().toISOString(),
    summary: {},
    trends: {},
    calibration: {},
    recommendations: []
  };

  // === Prediction Accuracy ===
  if (metrics.predictions.length > 0) {
    const correct = metrics.predictions.filter(p => p.correct).length;
    results.summary.predictionAccuracy = {
      total: metrics.predictions.length,
      correct,
      rate: (correct / metrics.predictions.length).toFixed(3)
    };
  }

  // === Command Success Rates ===
  if (metrics.commandOutcomes.length > 0) {
    const outcomes = metrics.commandOutcomes;
    const successes = outcomes.filter(o => o.success).length;

    // Overall
    results.summary.commandSuccess = {
      total: outcomes.length,
      successes,
      rate: (successes / outcomes.length).toFixed(3)
    };

    // With vs without warnings
    const withWarning = outcomes.filter(o => o.hadWarning);
    const withoutWarning = outcomes.filter(o => !o.hadWarning);

    if (withWarning.length > 10 && withoutWarning.length > 10) {
      const warningSuccessRate = withWarning.filter(o => o.success).length / withWarning.length;
      const noWarningSuccessRate = withoutWarning.filter(o => o.success).length / withoutWarning.length;

      results.summary.warningImpact = {
        withWarning: { total: withWarning.length, rate: warningSuccessRate.toFixed(3) },
        withoutWarning: { total: withoutWarning.length, rate: noWarningSuccessRate.toFixed(3) },
        delta: (noWarningSuccessRate - warningSuccessRate).toFixed(3),
        interpretation: warningSuccessRate < noWarningSuccessRate
          ? "Warnings correctly identify risky commands"
          : "Warnings may be too aggressive"
      };
    }

    // Heeded vs ignored warnings
    const heeded = withWarning.filter(o => o.followedAdvice);
    const ignored = withWarning.filter(o => o.followedAdvice === false);

    if (heeded.length > 5 && ignored.length > 5) {
      const heededSuccess = heeded.filter(o => o.success).length / heeded.length;
      const ignoredSuccess = ignored.filter(o => o.success).length / ignored.length;

      results.summary.adviceValue = {
        heeded: { total: heeded.length, successRate: heededSuccess.toFixed(3) },
        ignored: { total: ignored.length, successRate: ignoredSuccess.toFixed(3) },
        delta: (heededSuccess - ignoredSuccess).toFixed(3),
        interpretation: heededSuccess > ignoredSuccess
          ? "Following advice improves outcomes"
          : "Advice may not be helpful"
      };
    }
  }

  // === Agent Routing Accuracy ===
  if (metrics.agentRoutings.length > 0) {
    const routings = metrics.agentRoutings;
    const followed = routings.filter(r => r.followed);
    const notFollowed = routings.filter(r => !r.followed);

    results.summary.agentRouting = {
      total: routings.length,
      followedRecommendation: followed.length,
      followRate: (followed.length / routings.length).toFixed(3)
    };

    if (followed.length > 5 && notFollowed.length > 5) {
      const followedSuccess = followed.filter(r => r.success).length / followed.length;
      const notFollowedSuccess = notFollowed.filter(r => r.success).length / notFollowed.length;

      results.summary.agentRouting.followedSuccessRate = followedSuccess.toFixed(3);
      results.summary.agentRouting.notFollowedSuccessRate = notFollowedSuccess.toFixed(3);
      results.summary.agentRouting.delta = (followedSuccess - notFollowedSuccess).toFixed(3);
      results.summary.agentRouting.interpretation = followedSuccess > notFollowedSuccess
        ? "Agent recommendations improve task success"
        : "Agent routing needs improvement";
    }
  }

  // === Calibration Analysis ===
  for (const [bucket, data] of Object.entries(metrics.calibration)) {
    if (data.total >= 5) {
      const actualRate = data.correct / data.total;
      const expectedRate = parseFloat(bucket) + 0.05; // midpoint of bucket
      results.calibration[bucket] = {
        predicted: expectedRate.toFixed(2),
        actual: actualRate.toFixed(3),
        samples: data.total,
        calibrationError: Math.abs(expectedRate - actualRate).toFixed(3)
      };
    }
  }

  // === Trend Analysis ===
  const days = Object.keys(metrics.dailyStats).sort();
  if (days.length >= 3) {
    const recentDays = days.slice(-7);
    const olderDays = days.slice(-14, -7);

    const recentRate = recentDays.reduce((sum, d) => {
      const s = metrics.dailyStats[d];
      return sum + (s.commands > 0 ? s.successes / s.commands : 0);
    }, 0) / recentDays.length;

    if (olderDays.length > 0) {
      const olderRate = olderDays.reduce((sum, d) => {
        const s = metrics.dailyStats[d];
        return sum + (s.commands > 0 ? s.successes / s.commands : 0);
      }, 0) / olderDays.length;

      results.trends.successRateTrend = {
        recent7Days: recentRate.toFixed(3),
        previous7Days: olderRate.toFixed(3),
        change: (recentRate - olderRate).toFixed(3),
        improving: recentRate > olderRate
      };
    }
  }

  // === Recommendations ===
  if (results.summary.adviceValue?.delta < 0) {
    results.recommendations.push({
      priority: 'high',
      issue: 'Advice not helping',
      action: 'Review Q-table thresholds and warning triggers'
    });
  }

  if (results.summary.agentRouting?.delta < 0) {
    results.recommendations.push({
      priority: 'medium',
      issue: 'Agent routing not improving outcomes',
      action: 'Retrain with more agent assignment data'
    });
  }

  const avgCalibrationError = Object.values(results.calibration)
    .reduce((sum, c) => sum + parseFloat(c.calibrationError), 0) /
    Math.max(1, Object.keys(results.calibration).length);

  if (avgCalibrationError > 0.15) {
    results.recommendations.push({
      priority: 'medium',
      issue: `Confidence poorly calibrated (avg error: ${avgCalibrationError.toFixed(2)})`,
      action: 'Adjust Q-value scaling or add temperature parameter'
    });
  }

  if (results.recommendations.length === 0) {
    results.recommendations.push({
      priority: 'info',
      issue: 'None detected',
      action: 'Continue collecting data for more insights'
    });
  }

  return results;
}

/**
 * CLI
 */
const command = process.argv[2];

switch (command) {
  case 'record-prediction': {
    const [,, , predicted, actual, confidence] = process.argv;
    const correct = recordPrediction(predicted, actual, { confidence: parseFloat(confidence) || 0 });
    console.log(JSON.stringify({ recorded: true, correct }));
    break;
  }

  case 'record-command': {
    const [,, , cmdType, success, hadWarning, followedAdvice] = process.argv;
    recordCommandOutcome(cmdType, success === 'true', {
      hadWarning: hadWarning === 'true',
      followedAdvice: followedAdvice === 'true' ? true : followedAdvice === 'false' ? false : undefined
    });
    console.log(JSON.stringify({ recorded: true }));
    break;
  }

  case 'record-routing': {
    const [,, , recommended, used, success] = process.argv;
    recordAgentRouting(recommended, used, success === 'true');
    console.log(JSON.stringify({ recorded: true }));
    break;
  }

  case 'effectiveness':
  case 'report': {
    const report = calculateEffectiveness();
    console.log(JSON.stringify(report, null, 2));
    break;
  }

  case 'reset': {
    if (existsSync(METRICS_FILE)) {
      const backup = METRICS_FILE + '.backup';
      writeFileSync(backup, readFileSync(METRICS_FILE));
      console.log(`Backed up to ${backup}`);
    }
    saveMetrics(loadMetrics()); // Creates fresh metrics
    console.log('Metrics reset');
    break;
  }

  default:
    console.log(`
📊 RuVector Intelligence Metrics

Commands:
  effectiveness     Show effectiveness report
  record-prediction <predicted> <actual> [confidence]
  record-command <type> <success> [hadWarning] [followedAdvice]
  record-routing <recommended> <used> <success>
  reset             Reset metrics (backs up existing)

Example:
  node metrics.js effectiveness
  node metrics.js record-command cargo true true true
`);
}
