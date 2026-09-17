import assert from 'node:assert/strict';
import test from 'node:test';

import { evaluateCandidate, gateReceiptDigest, pairedDatasetDigest, planDigest } from '../src/flywheel.js';

const digest = (character) => `sha256:${character.repeat(64)}`;
const now = new Date('2026-08-29T12:00:00.000Z');

function experimentPlan(overrides = {}) {
  return {
    schema: 'ExperimentPlanV1',
    platform: 'linkedin',
    identityScope: 'ruv_personal',
    objective: 'source backed engagement',
    metric: 'engagementPerImpression',
    metricSemanticsDigest: digest('0'),
    direction: 'increase',
    minimumSamples: 20,
    minimumRelativeLift: 0.05,
    pairingRuleDigest: digest('1'),
    anchorSetDigest: digest('2'),
    policyDigest: digest('3'),
    registeredAt: '2026-08-28T00:00:00.000Z',
    observationStartsAt: '2026-08-29T00:00:00.000Z',
    ...overrides,
  };
}

function receipt(gate, bindings, overrides = {}) {
  const body = {
    schema: 'SocialEvaluationGateV1',
    gate,
    datasetDigest: bindings.datasetDigest,
    experimentPlanDigest: bindings.experimentPlanDigest,
    outcome: 'PASS',
    evidenceDigests: [digest(gate === 'anchor' ? '4' : gate === 'provenance' ? '5' : gate === 'security' ? '6' : '7')],
    issuerEvidenceDigest: digest('8'),
    issuedAt: '2026-08-29T11:00:00.000Z',
    expiresAt: '2026-09-28T11:00:00.000Z',
    ...(gate === 'blockedActions' ? { blockedActionCount: 0 } : {}),
    ...overrides,
  };
  return { ...body, receiptDigest: gateReceiptDigest(body) };
}

function candidate(overrides = {}) {
  const baseline = overrides.baseline || Array(20).fill(0.1);
  const variant = overrides.variant || Array(20).fill(0.11);
  const plan = overrides.experimentPlan || experimentPlan();
  const observationBindings = overrides.observationBindings || baseline.map((value, index) => ({
    schema: 'MetricObservationPairV1',
    pairingKeyDigest: `sha256:${index.toString(16).padStart(64, '0')}`,
    baselineSnapshotDigest: `sha256:${(index + 10_000).toString(16).padStart(64, '0')}`,
    variantSnapshotDigest: `sha256:${(index + 20_000).toString(16).padStart(64, '0')}`,
    baselineMetricSemanticsDigest: plan.metricSemanticsDigest,
    variantMetricSemanticsDigest: plan.metricSemanticsDigest,
  }));
  const bindings = { datasetDigest: pairedDatasetDigest(baseline, variant, observationBindings), experimentPlanDigest: planDigest(plan) };
  return {
    experimentPlan: plan,
    experimentPlanDigest: bindings.experimentPlanDigest,
    datasetDigest: bindings.datasetDigest,
    baseline,
    variant,
    observationBindings,
    gateReceipts: Object.fromEntries(['anchor', 'blockedActions', 'provenance', 'security'].map((gate) => [gate, receipt(gate, bindings)])),
    ...overrides,
  };
}

test('a passing screen remains untrusted and cannot establish review eligibility', () => {
  const result = evaluateCandidate(candidate(), { now });
  assert.equal(result.recommendation, 'SCREENING_PASSED_REQUIRES_INDEPENDENT_GATE_VERIFICATION');
  assert.equal(result.reviewEligibilityEstablished, false);
  assert.equal(result.promotionAuthorized, false);
  assert.equal(result.causalClaimAllowed, false);
  assert.equal(result.gates.gateAuthorityVerified, false);
  assert.equal(result.baseline.median, 0.1);
  assert.equal(result.variant.iqr, 0);
  assert.ok(Math.abs(result.pairedDifference.median - 0.01) < 1e-12);
  assert.equal(result.pairedWins, 20);
});

test('a blocked action receipt rejects the candidate', () => {
  const input = candidate();
  const bindings = { datasetDigest: input.datasetDigest, experimentPlanDigest: input.experimentPlanDigest };
  input.gateReceipts.blockedActions = receipt('blockedActions', bindings, { outcome: 'FAIL', blockedActionCount: 1 });
  const result = evaluateCandidate(input, { now });
  assert.equal(result.gates.declaredNoBlockedActions, false);
  assert.equal(result.recommendation, 'REJECT_OR_COLLECT_MORE_EVIDENCE');
});

test('caller booleans cannot replace bound gate receipts', () => {
  const input = candidate();
  delete input.gateReceipts;
  input.anchorRetention = true;
  input.provenanceVerified = true;
  assert.throws(() => evaluateCandidate(input, { now }), /Candidate fields are invalid/u);
});

test('unpaired evidence and an arbitrary dataset digest are rejected', () => {
  assert.throws(() => evaluateCandidate(candidate({ baseline: [1, 2], variant: [2] }), { now }), /paired/u);
  assert.throws(() => evaluateCandidate(candidate({ datasetDigest: digest('9') }), { now }), /paired observations/u);
});

test('Phase 1 floors and frozen plan digest prevent post hoc threshold lowering', () => {
  const lowPlan = experimentPlan({ minimumSamples: 5, minimumRelativeLift: 0 });
  const input = candidate({ experimentPlan: lowPlan, experimentPlanDigest: planDigest(lowPlan) });
  assert.throws(() => evaluateCandidate(input, { now }), /between 20 and 10000/u);

  const changed = candidate();
  changed.experimentPlan = { ...changed.experimentPlan, minimumRelativeLift: 0.2 };
  assert.throws(() => evaluateCandidate(changed, { now }), /does not match the frozen plan/u);
});

test('gate receipts must bind the exact frozen plan and dataset', () => {
  const input = candidate();
  const changed = { ...input.gateReceipts.anchor, datasetDigest: digest('a') };
  changed.receiptDigest = gateReceiptDigest(changed);
  input.gateReceipts.anchor = changed;
  assert.throws(() => evaluateCandidate(input, { now }), /not bound/u);
});

test('optimization requires registered metric semantics and bound snapshot pairs', () => {
  const arbitraryPlan = experimentPlan({ metric: 'madeUpVanityScore' });
  assert.throws(() => evaluateCandidate(candidate({
    experimentPlan: arbitraryPlan,
    experimentPlanDigest: planDigest(arbitraryPlan),
  }), { now }), /registered normalized rate/u);

  const input = candidate();
  input.observationBindings[0] = { ...input.observationBindings[0], baselineSnapshotDigest: digest('c') };
  assert.throws(() => evaluateCandidate(input, { now }), /paired observations/u);

  const wrongSemantics = candidate();
  wrongSemantics.observationBindings[0] = {
    ...wrongSemantics.observationBindings[0],
    baselineMetricSemanticsDigest: digest('d'),
  };
  assert.throws(() => evaluateCandidate(wrongSemantics, { now }), /metric semantics/u);

  const replayedSnapshot = candidate();
  replayedSnapshot.observationBindings[1] = {
    ...replayedSnapshot.observationBindings[1],
    baselineSnapshotDigest: replayedSnapshot.observationBindings[0].baselineSnapshotDigest,
  };
  assert.throws(() => evaluateCandidate(replayedSnapshot, { now }), /snapshot digests must be unique/u);
});

test('gate evidence lifetime is capped at 30 days', () => {
  const input = candidate();
  const bindings = { datasetDigest: input.datasetDigest, experimentPlanDigest: input.experimentPlanDigest };
  input.gateReceipts.security = receipt('security', bindings, { expiresAt: '2030-01-01T00:00:00.000Z' });
  assert.throws(() => evaluateCandidate(input, { now }), /lifetime exceeds 30 days/u);
});
