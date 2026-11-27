'use strict';

function timestampValue(value) {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

function earliestTimestamp(samples) {
  return samples.reduce(
    (earliest, sample) => Math.min(earliest, timestampValue(sample.timestamp)),
    Number.POSITIVE_INFINITY,
  );
}

/**
 * Deterministically hold out complete capture sessions. This prevents
 * overlapping or adjacent windows from the same room run appearing on both
 * sides of the reported evaluation boundary.
 */
function splitBySession(samples, evalFraction) {
  if (!Number.isFinite(evalFraction) || evalFraction <= 0 || evalFraction >= 1) {
    throw new Error('eval fraction must be finite and strictly between 0 and 1');
  }
  const groups = new Map();
  for (const sample of samples) {
    if (!sample.sessionId) {
      throw new Error('leakage-free evaluation requires at least two labelled sessions');
    }
    if (!groups.has(sample.sessionId)) groups.set(sample.sessionId, []);
    groups.get(sample.sessionId).push(sample);
  }
  if (groups.size < 2) {
    throw new Error('leakage-free evaluation requires at least two labelled sessions');
  }

  const ordered = Array.from(groups.entries()).sort(([, a], [, b]) =>
    earliestTimestamp(a) - earliestTimestamp(b));
  const targetEval = Math.max(1, Math.ceil(samples.length * evalFraction));
  const evalIds = new Set();
  let evalCount = 0;
  for (let i = ordered.length - 1; i >= 1 && evalCount < targetEval; i--) {
    evalIds.add(ordered[i][0]);
    evalCount += ordered[i][1].length;
  }

  const trainSet = samples.filter(sample => !evalIds.has(sample.sessionId));
  const evalSet = samples.filter(sample => evalIds.has(sample.sessionId));
  if (trainSet.length === 0 || evalSet.length === 0) {
    throw new Error('eval fraction must produce non-empty train and evaluation partitions');
  }
  return {
    trainSet,
    evalSet,
    strategy: 'session-disjoint',
  };
}

/** Pose training is single-grid and has no presence loss. */
function validatePoseDataset(samples, expectedTimeSteps) {
  const poseSamples = samples.filter(sample => sample.presence !== false);
  if (poseSamples.length === 0) {
    throw new Error('pose training requires at least one present sample');
  }
  const expectedDim = poseSamples[0].csiDim;
  for (const sample of poseSamples) {
    if (sample.csiDim !== expectedDim || sample.timeSteps !== expectedTimeSteps) {
      throw new Error(
        `heterogeneous CSI shape: expected [${expectedDim},${expectedTimeSteps}], ` +
        `got [${sample.csiDim},${sample.timeSteps}]`,
      );
    }
    if (sample.csi && sample.csi.length !== sample.csiDim * sample.timeSteps) {
      throw new Error(
        `invalid CSI payload length: expected ${sample.csiDim * sample.timeSteps}, ` +
        `got ${sample.csi.length}`,
      );
    }
  }
  return poseSamples;
}

module.exports = { splitBySession, validatePoseDataset };
