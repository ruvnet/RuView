// SPDX-License-Identifier: MIT

import { sha256 } from './canonical.js';
import { SUPPORTED_OPTIMIZATION_METRICS } from './metrics.js';
import { scanSensitive } from './sensitive.js';
import { isPlainObject, normalizedIso } from './validation.js';

const DIGEST_RE = /^sha256:[a-f0-9]{64}$/u;
const GATES = Object.freeze(['anchor', 'blockedActions', 'provenance', 'security']);
const OPTIMIZATION_METRICS = new Set(SUPPORTED_OPTIMIZATION_METRICS);
const MAX_GATE_LIFETIME_MS = 30 * 24 * 60 * 60 * 1000;
const CANDIDATE_FIELDS = new Set(['baseline', 'datasetDigest', 'experimentPlan', 'experimentPlanDigest', 'gateReceipts', 'observationBindings', 'variant']);
const OBSERVATION_BINDING_FIELDS = new Set(['baselineMetricSemanticsDigest', 'baselineSnapshotDigest', 'pairingKeyDigest', 'schema', 'variantMetricSemanticsDigest', 'variantSnapshotDigest']);
const PLAN_FIELDS = new Set([
  'anchorSetDigest',
  'direction',
  'identityScope',
  'metric',
  'metricSemanticsDigest',
  'minimumRelativeLift',
  'minimumSamples',
  'objective',
  'observationStartsAt',
  'pairingRuleDigest',
  'platform',
  'policyDigest',
  'registeredAt',
  'schema',
]);

function series(value, name) {
  if (!Array.isArray(value) || value.some((item) => typeof item !== 'number' || !Number.isFinite(item))) {
    throw new TypeError(`${name} must be an array of finite numbers`);
  }
  return value;
}

function boundedString(value, name, maximum = 128) {
  if (typeof value !== 'string' || value.trim().length === 0 || value.length > maximum) throw new TypeError(`${name} is required`);
  return value.trim();
}

function digest(value, name) {
  if (!DIGEST_RE.test(value || '')) throw new TypeError(`${name} must be sha256`);
  return value;
}

function mean(values) {
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function quantile(values, probability) {
  const sorted = [...values].sort((left, right) => left - right);
  if (sorted.length === 0) return null;
  const position = (sorted.length - 1) * probability;
  const lower = Math.floor(position);
  const upper = Math.ceil(position);
  if (lower === upper) return sorted[lower];
  return sorted[lower] + (sorted[upper] - sorted[lower]) * (position - lower);
}

function statistics(values) {
  if (values.length === 0) return { mean: null, median: null, iqr: null };
  const q1 = quantile(values, 0.25);
  const q3 = quantile(values, 0.75);
  return { mean: mean(values), median: quantile(values, 0.5), iqr: q3 - q1 };
}

export function planDigest(plan) {
  return sha256(plan);
}

export function pairedDatasetDigest(baseline, variant, observationBindings) {
  return sha256({ schema: 'PairedMetricObservationsV1', baseline, variant, observationBindings });
}

export function gateReceiptDigest(receipt) {
  const { receiptDigest: omitted, ...body } = receipt;
  return sha256(body);
}

function normalizePlan(value, expectedDigest) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new TypeError('experimentPlan must be an object');
  const unknown = Object.keys(value).filter((key) => !PLAN_FIELDS.has(key));
  const missing = [...PLAN_FIELDS].filter((key) => !Object.hasOwn(value, key));
  if (unknown.length || missing.length) throw new TypeError(`experimentPlan fields are invalid: ${[...unknown, ...missing].join(', ')}`);
  if (value.schema !== 'ExperimentPlanV1') throw new TypeError('experimentPlan schema mismatch');
  const plan = {
    schema: 'ExperimentPlanV1',
    platform: boundedString(value.platform, 'experimentPlan.platform', 32).toLocaleLowerCase('en-US'),
    identityScope: boundedString(value.identityScope, 'experimentPlan.identityScope', 64),
    objective: boundedString(value.objective, 'experimentPlan.objective'),
    metric: boundedString(value.metric, 'experimentPlan.metric'),
    metricSemanticsDigest: digest(value.metricSemanticsDigest, 'experimentPlan.metricSemanticsDigest'),
    direction: value.direction,
    minimumSamples: value.minimumSamples,
    minimumRelativeLift: value.minimumRelativeLift,
    pairingRuleDigest: digest(value.pairingRuleDigest, 'experimentPlan.pairingRuleDigest'),
    anchorSetDigest: digest(value.anchorSetDigest, 'experimentPlan.anchorSetDigest'),
    policyDigest: digest(value.policyDigest, 'experimentPlan.policyDigest'),
    registeredAt: value.registeredAt,
    observationStartsAt: value.observationStartsAt,
  };
  if (!['increase', 'decrease'].includes(plan.direction)) throw new TypeError('experimentPlan.direction must be increase or decrease');
  if (!OPTIMIZATION_METRICS.has(plan.metric)) throw new TypeError('experimentPlan.metric is not a registered normalized rate');
  if (!Number.isSafeInteger(plan.minimumSamples) || plan.minimumSamples < 20 || plan.minimumSamples > 10_000) {
    throw new TypeError('experimentPlan.minimumSamples must be between 20 and 10000 in Phase 1');
  }
  if (typeof plan.minimumRelativeLift !== 'number' || !Number.isFinite(plan.minimumRelativeLift) || plan.minimumRelativeLift < 0.05 || plan.minimumRelativeLift > 10) {
    throw new TypeError('experimentPlan.minimumRelativeLift must be between 0.05 and 10 in Phase 1');
  }
  if (!normalizedIso(plan.registeredAt) || !normalizedIso(plan.observationStartsAt)) throw new TypeError('experimentPlan timestamps must be normalized ISO timestamps');
  if (Date.parse(plan.registeredAt) >= Date.parse(plan.observationStartsAt)) throw new TypeError('experimentPlan must be registered before observation starts');
  if (planDigest(plan) !== expectedDigest) throw new TypeError('experimentPlanDigest does not match the frozen plan');
  return Object.freeze(plan);
}

function normalizeObservationBindings(value, expectedLength, metricSemanticsDigest) {
  if (!Array.isArray(value) || value.length !== expectedLength || value.length === 0 || value.length > 10_000) {
    throw new TypeError('observationBindings must contain one exact binding per paired observation');
  }
  const seenPairs = new Set();
  const seenBaselineSnapshots = new Set();
  const seenVariantSnapshots = new Set();
  return value.map((binding, index) => {
    if (!isPlainObject(binding)) throw new TypeError(`observationBindings[${index}] must be an object`);
    const unknown = Object.keys(binding).filter((key) => !OBSERVATION_BINDING_FIELDS.has(key));
    const missing = [...OBSERVATION_BINDING_FIELDS].filter((key) => !Object.hasOwn(binding, key));
    if (unknown.length || missing.length) throw new TypeError(`observationBindings[${index}] fields are invalid: ${[...unknown, ...missing].join(', ')}`);
    if (binding.schema !== 'MetricObservationPairV1') throw new TypeError(`observationBindings[${index}] schema mismatch`);
    ['baselineMetricSemanticsDigest', 'baselineSnapshotDigest', 'pairingKeyDigest', 'variantMetricSemanticsDigest', 'variantSnapshotDigest']
      .forEach((name) => digest(binding[name], `observationBindings[${index}].${name}`));
    if (binding.baselineMetricSemanticsDigest !== metricSemanticsDigest || binding.variantMetricSemanticsDigest !== metricSemanticsDigest) {
      throw new TypeError(`observationBindings[${index}] metric semantics do not match the frozen plan`);
    }
    if (seenPairs.has(binding.pairingKeyDigest)) throw new TypeError('observationBindings pairing keys must be unique');
    if (seenBaselineSnapshots.has(binding.baselineSnapshotDigest) || seenVariantSnapshots.has(binding.variantSnapshotDigest)) {
      throw new TypeError('observationBindings snapshot digests must be unique within each paired side');
    }
    seenPairs.add(binding.pairingKeyDigest);
    seenBaselineSnapshots.add(binding.baselineSnapshotDigest);
    seenVariantSnapshots.add(binding.variantSnapshotDigest);
    return Object.freeze({ ...binding });
  });
}

function normalizeGateReceipt(value, name, bindings, now) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new TypeError(`gateReceipts.${name} is required`);
  const allowed = new Set(['datasetDigest', 'evidenceDigests', 'experimentPlanDigest', 'expiresAt', 'gate', 'issuedAt', 'issuerEvidenceDigest', 'outcome', 'receiptDigest', 'schema']);
  if (name === 'blockedActions') allowed.add('blockedActionCount');
  const unknown = Object.keys(value).filter((key) => !allowed.has(key));
  const missing = [...allowed].filter((key) => !Object.hasOwn(value, key));
  if (unknown.length || missing.length) throw new TypeError(`gateReceipts.${name} fields are invalid: ${[...unknown, ...missing].join(', ')}`);
  if (value.schema !== 'SocialEvaluationGateV1' || value.gate !== name) throw new TypeError(`gateReceipts.${name} schema or gate mismatch`);
  if (value.datasetDigest !== bindings.datasetDigest || value.experimentPlanDigest !== bindings.experimentPlanDigest) {
    throw new TypeError(`gateReceipts.${name} is not bound to this plan and dataset`);
  }
  digest(value.issuerEvidenceDigest, `gateReceipts.${name}.issuerEvidenceDigest`);
  if (!Array.isArray(value.evidenceDigests) || value.evidenceDigests.length === 0 || value.evidenceDigests.length > 32) {
    throw new TypeError(`gateReceipts.${name}.evidenceDigests must contain between one and 32 digests`);
  }
  if (new Set(value.evidenceDigests).size !== value.evidenceDigests.length) throw new TypeError(`gateReceipts.${name}.evidenceDigests contains duplicates`);
  value.evidenceDigests.forEach((item, index) => digest(item, `gateReceipts.${name}.evidenceDigests[${index}]`));
  if (!['FAIL', 'PASS'].includes(value.outcome)) throw new TypeError(`gateReceipts.${name}.outcome is invalid`);
  if (!normalizedIso(value.issuedAt) || !normalizedIso(value.expiresAt)) throw new TypeError(`gateReceipts.${name} timestamps must be normalized ISO timestamps`);
  if (Date.parse(value.issuedAt) < bindings.observationStartsAt || Date.parse(value.issuedAt) > now.getTime()) throw new TypeError(`gateReceipts.${name}.issuedAt is outside the observed interval`);
  if (Date.parse(value.expiresAt) <= now.getTime() || Date.parse(value.expiresAt) <= Date.parse(value.issuedAt)) throw new TypeError(`gateReceipts.${name} is expired or has invalid expiry`);
  if (Date.parse(value.expiresAt) - Date.parse(value.issuedAt) > MAX_GATE_LIFETIME_MS) throw new TypeError(`gateReceipts.${name} lifetime exceeds 30 days`);
  if (name === 'blockedActions' && (!Number.isSafeInteger(value.blockedActionCount) || value.blockedActionCount < 0)) {
    throw new TypeError('gateReceipts.blockedActions.blockedActionCount must be a non-negative integer');
  }
  digest(value.receiptDigest, `gateReceipts.${name}.receiptDigest`);
  if (gateReceiptDigest(value) !== value.receiptDigest) throw new TypeError(`gateReceipts.${name}.receiptDigest mismatch`);
  return Object.freeze({ ...value, evidenceDigests: [...value.evidenceDigests] });
}

export function evaluateCandidate(input, { now = new Date() } = {}) {
  if (!input || typeof input !== 'object' || Array.isArray(input)) throw new TypeError('Candidate must be an object');
  const unknown = Object.keys(input).filter((key) => !CANDIDATE_FIELDS.has(key));
  const missing = [...CANDIDATE_FIELDS].filter((key) => !Object.hasOwn(input, key));
  if (unknown.length || missing.length) throw new TypeError(`Candidate fields are invalid: ${[...unknown, ...missing].join(', ')}`);
  const sensitive = scanSensitive(input);
  if (sensitive.length) throw new TypeError(`${sensitive[0].path} contains forbidden sensitive material (${sensitive[0].rule})`);
  const baseline = series(input.baseline, 'baseline');
  const variant = series(input.variant, 'variant');
  if (baseline.length !== variant.length) throw new TypeError('Baseline and variant must be paired and have equal lengths');
  const expectedPlanDigest = digest(input.experimentPlanDigest, 'experimentPlanDigest');
  const plan = normalizePlan(input.experimentPlan, expectedPlanDigest);
  const observationBindings = normalizeObservationBindings(input.observationBindings, baseline.length, plan.metricSemanticsDigest);
  const expectedDatasetDigest = pairedDatasetDigest(baseline, variant, observationBindings);
  if (input.datasetDigest !== expectedDatasetDigest) throw new TypeError('datasetDigest does not match the paired observations');
  const gateReceipts = input.gateReceipts;
  if (!gateReceipts || typeof gateReceipts !== 'object' || Array.isArray(gateReceipts)) throw new TypeError('gateReceipts is required');
  const gateNames = Object.keys(gateReceipts);
  if (gateNames.length !== GATES.length || GATES.some((name) => !Object.hasOwn(gateReceipts, name))) throw new TypeError('All four exact gate receipts are required');
  const bindings = { datasetDigest: expectedDatasetDigest, experimentPlanDigest: expectedPlanDigest, observationStartsAt: Date.parse(plan.observationStartsAt) };
  const receipts = Object.fromEntries(GATES.map((name) => [name, normalizeGateReceipt(gateReceipts[name], name, bindings, now)]));

  const baselineStats = statistics(baseline);
  const variantStats = statistics(variant);
  const pairedDifferenceStats = statistics(variant.map((value, index) => value - baseline[index]));
  const absoluteChange = baselineStats.mean === null ? null : variantStats.mean - baselineStats.mean;
  const relativeLift = baselineStats.mean === 0 || baselineStats.mean === null ? null : absoluteChange / Math.abs(baselineStats.mean);
  const directionPassed = relativeLift !== null && (
    plan.direction === 'increase' ? relativeLift >= plan.minimumRelativeLift : relativeLift <= -plan.minimumRelativeLift
  );
  const pairedWins = baseline.reduce((count, value, index) => {
    const won = plan.direction === 'increase' ? variant[index] > value : variant[index] < value;
    return count + Number(won);
  }, 0);
  const gates = {
    sampleCount: baseline.length >= plan.minimumSamples,
    effectDirection: directionPassed,
    gateReceiptIntegrity: true,
    gateReceiptBindings: true,
    declaredAnchorPass: receipts.anchor.outcome === 'PASS',
    declaredProvenancePass: receipts.provenance.outcome === 'PASS',
    declaredSecurityPass: receipts.security.outcome === 'PASS',
    declaredNoBlockedActions: receipts.blockedActions.outcome === 'PASS' && receipts.blockedActions.blockedActionCount === 0,
    gateAuthorityVerified: false,
  };
  const screeningSignal = Object.entries(gates).filter(([name]) => name !== 'gateAuthorityVerified').every(([, passed]) => passed);
  const record = {
    schema: 'FlywheelEvaluationV1',
    experimentPlanDigest: expectedPlanDigest,
    platform: plan.platform,
    identityScope: plan.identityScope,
    objective: plan.objective,
    metric: plan.metric,
    metricSemanticsDigest: plan.metricSemanticsDigest,
    direction: plan.direction,
    datasetDigest: expectedDatasetDigest,
    observationBindingsDigest: sha256(observationBindings),
    sampleCount: baseline.length,
    baseline: baselineStats,
    variant: variantStats,
    pairedDifference: pairedDifferenceStats,
    pairedWins,
    pairedWinRate: baseline.length === 0 ? null : pairedWins / baseline.length,
    absoluteChange,
    relativeLift,
    minimumSamples: plan.minimumSamples,
    minimumRelativeLift: plan.minimumRelativeLift,
    gates,
    declaredGateOutcomes: Object.fromEntries(GATES.map((name) => [name, receipts[name].outcome])),
    gateDigests: Object.fromEntries(GATES.map((name) => [name, receipts[name].receiptDigest])),
    evidenceClass: 'UNTRUSTED_SCREENING_ONLY',
    recommendation: screeningSignal ? 'SCREENING_PASSED_REQUIRES_INDEPENDENT_GATE_VERIFICATION' : 'REJECT_OR_COLLECT_MORE_EVIDENCE',
    reviewEligibilityEstablished: false,
    promotionAuthorized: false,
    causalClaimAllowed: false,
  };
  return { ...record, evaluationDigest: sha256(record) };
}
