import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

import { proposalDigest, runAutopilot } from '../src/autopilot.js';
import { sha256 } from '../src/canonical.js';
import { gateReceiptDigest, pairedDatasetDigest, planDigest } from '../src/flywheel.js';
import { runTool } from '../src/tools.js';

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

function evaluation(overrides = {}) {
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

function proposal(candidate, id = 'proposal-1', overrides = {}) {
  const body = {
    schema: 'OptimizationProposalV1',
    proposalId: id,
    platform: 'linkedin',
    account: 'reuvencohen',
    identityScope: 'ruv_personal',
    changeType: 'EVIDENCE_PRESENTATION',
    oneChange: `Move the reproducer link above the first claim for ${id}.`,
    rationale: 'The frozen paired plan tests one presentation change.',
    expectedEffect: 'Increase engagement per impression by at least five percent.',
    rollback: 'Restore the prior evidence placement.',
    sourceDigests: [candidate.experimentPlanDigest, candidate.datasetDigest],
    experimentPlanDigest: candidate.experimentPlanDigest,
    datasetDigest: candidate.datasetDigest,
    createdAt: '2026-08-29T11:30:00.000Z',
    expiresAt: '2026-09-29T00:00:00.000Z',
    ...overrides,
  };
  return { ...body, proposalDigest: proposalDigest(body) };
}

function entry(id = 'proposal-1', overrides = {}) {
  const candidate = overrides.evaluation || evaluation();
  return {
    proposal: overrides.proposal || proposal(candidate, id),
    evaluation: candidate,
  };
}

test('bounded autopilot queues screening passes only for independent verification', () => {
  const result = runAutopilot({ runId: 'run:bounded', maximumCycles: 10, proposals: [entry()] }, { now });
  assert.equal(result.stopReason, 'BATCH_COMPLETE');
  assert.equal(result.processedCycles, 1);
  assert.equal(result.independentVerificationQueue.length, 1);
  assert.equal(result.dispositions[0].disposition, 'QUEUED_FOR_INDEPENDENT_VERIFICATION');
  assert.equal(result.networkAttempted, false);
  assert.equal(result.credentialStoresRead, false);
  assert.equal(result.externalActionsAttempted, 0);
  assert.equal(result.executionAuthorized, false);
  assert.equal(result.reviewEligibilityEstablished, false);
  assert.equal(result.promotionAuthorized, false);
  assert.equal(result.selfMutationAuthorized, false);
  assert.equal(result.checkpointAuthorityVerified, false);
  const { runDigest, ...body } = result;
  assert.equal(runDigest, sha256(body));
  assert.notEqual(runDigest, sha256({ ...body, promotionAuthorized: true }));
});

test('cycle limit emits a digest bound checkpoint and resume is deterministic', () => {
  const proposals = [entry('proposal-1'), entry('proposal-2')];
  const first = runAutopilot({ runId: 'run:resume', maximumCycles: 1, proposals }, { now });
  assert.equal(first.stopReason, 'CYCLE_LIMIT');
  assert.equal(first.nextCursor, 1);
  assert.equal(first.checkpoint.processedProposalDigests.length, 1);

  const resumed = runAutopilot({ runId: 'run:resume', maximumCycles: 1, proposals, checkpoint: first.checkpoint }, { now });
  assert.equal(resumed.startCursor, 1);
  assert.equal(resumed.nextCursor, 2);
  assert.equal(resumed.stopReason, 'BATCH_COMPLETE');
  assert.equal(resumed.checkpoint.previousCheckpointDigest, first.checkpoint.checkpointDigest);

  const repeated = runAutopilot({ runId: 'run:resume', maximumCycles: 1, proposals, checkpoint: first.checkpoint }, { now });
  assert.equal(repeated.runDigest, resumed.runDigest);
  assert.deepEqual(repeated, resumed);
});

test('tampered and replayed checkpoints fail closed', () => {
  const proposals = [entry('proposal-1'), entry('proposal-2')];
  const first = runAutopilot({ runId: 'run:tamper', maximumCycles: 1, proposals }, { now });
  assert.throws(() => runAutopilot({
    runId: 'run:tamper',
    maximumCycles: 1,
    proposals,
    checkpoint: { ...first.checkpoint, nextCursor: 2 },
  }, { now }), /processed digest count/u);
  assert.throws(() => runAutopilot({
    runId: 'run:different',
    maximumCycles: 1,
    proposals,
    checkpoint: first.checkpoint,
  }, { now }), /not bound/u);
  const changed = [entry('proposal-1'), entry('proposal-changed')];
  assert.throws(() => runAutopilot({
    runId: 'run:tamper',
    maximumCycles: 1,
    proposals: changed,
    checkpoint: first.checkpoint,
  }, { now }), /not bound/u);
});

test('proposal digest tampering and authority fields fail closed', () => {
  const valid = entry();
  assert.throws(() => runAutopilot({
    runId: 'run:digest',
    maximumCycles: 1,
    proposals: [{ ...valid, proposal: { ...valid.proposal, oneChange: 'Tampered after signing.' } }],
  }, { now }), /proposalDigest mismatch/u);

  const authority = { ...valid.proposal, writeAuthority: true };
  assert.throws(() => runAutopilot({
    runId: 'run:authority',
    maximumCycles: 1,
    proposals: [{ ...valid, proposal: authority }],
  }, { now }), /fields are invalid/u);
});

test('credential material and unbounded runs are rejected before evaluation', () => {
  const valid = entry();
  const apiKeyField = `api_${'key'}`;
  assert.throws(() => runAutopilot({
    runId: 'run:credential',
    maximumCycles: 1,
    proposals: [{ ...valid, proposal: { ...valid.proposal, [apiKeyField]: `value-${'x'.repeat(32)}` } }],
  }, { now }), /forbidden sensitive material/u);
  assert.throws(() => runAutopilot({ runId: 'run:unbounded', maximumCycles: 101, proposals: [valid] }, { now }), /between one and 100/u);
});

test('proposal and evaluation binding mismatches are rejected without side effects', () => {
  const candidate = evaluation();
  const wrongDataset = digest('a');
  const mismatched = proposal(candidate, 'proposal-mismatch', {
    datasetDigest: wrongDataset,
    sourceDigests: [candidate.experimentPlanDigest, wrongDataset],
  });
  const result = runAutopilot({
    runId: 'run:binding',
    maximumCycles: 1,
    proposals: [{ proposal: mismatched, evaluation: candidate }],
  }, { now });
  assert.equal(result.dispositions[0].disposition, 'REJECTED_BINDING_MISMATCH');
  assert.equal(result.independentVerificationQueue.length, 0);
  assert.equal(result.externalActionsAttempted, 0);
});

test('invalid flywheel evidence is isolated as a rejection and the loop advances', () => {
  const candidate = evaluation();
  const proposed = proposal(candidate, 'proposal-invalid-evidence');
  const result = runAutopilot({
    runId: 'run:invalid-evidence',
    maximumCycles: 1,
    proposals: [{ proposal: proposed, evaluation: { ...candidate, datasetDigest: digest('b') } }],
  }, { now });
  assert.equal(result.dispositions[0].disposition, 'REJECTED_INVALID_EVIDENCE');
  assert.equal(result.nextCursor, 1);
  assert.equal(result.stopReason, 'BATCH_COMPLETE');
});

test('proposals cannot predate the evidence receipts they claim to use', () => {
  const candidate = evaluation();
  const proposed = proposal(candidate, 'proposal-predated', { createdAt: '2026-08-29T10:00:00.000Z' });
  const result = runAutopilot({
    runId: 'run:predated',
    maximumCycles: 1,
    proposals: [{ proposal: proposed, evaluation: candidate }],
  }, { now });
  assert.equal(result.dispositions[0].disposition, 'REJECTED_BINDING_MISMATCH');
});

test('exact account and identity scope binding is mandatory', () => {
  const candidate = evaluation();
  const wrongAccount = proposal(candidate, 'proposal-wrong-account', { account: 'ruvnet' });
  assert.throws(() => runAutopilot({
    runId: 'run:account',
    maximumCycles: 1,
    proposals: [{ proposal: wrongAccount, evaluation: candidate }],
  }, { now }), /not registered/u);

  const wrongScope = proposal(candidate, 'proposal-wrong-scope', { identityScope: 'ruvnet' });
  assert.throws(() => runAutopilot({
    runId: 'run:scope',
    maximumCycles: 1,
    proposals: [{ proposal: wrongScope, evaluation: candidate }],
  }, { now }), /bound to identity scope/u);
});

test('one autopilot batch cannot mix accounts or identity scopes', () => {
  const githubPlan = experimentPlan({ platform: 'github', identityScope: 'ruvnet' });
  const githubEvaluation = evaluation({ experimentPlan: githubPlan });
  const githubProposal = proposal(githubEvaluation, 'proposal-github', {
    platform: 'github',
    account: 'ruvnet',
    identityScope: 'ruvnet',
  });
  assert.throws(() => runAutopilot({
    runId: 'run:mixed-scope',
    maximumCycles: 2,
    proposals: [entry('proposal-linkedin'), { proposal: githubProposal, evaluation: githubEvaluation }],
  }, { now }), /one exact account and identity scope/u);
});

test('direct callers cannot smuggle evaluation fields or exceed aggregate observations', () => {
  const valid = entry();
  assert.throws(() => runAutopilot({
    runId: 'run:evaluation-smuggling',
    maximumCycles: 1,
    proposals: [{ ...valid, evaluation: { ...valid.evaluation, publishNow: true } }],
  }, { now }), /evaluation fields are invalid/u);

  const oversized = Array.from({ length: 20 }, (_, index) => {
    const candidate = evaluation({ baseline: Array(501).fill(0.1), variant: Array(501).fill(0.11) });
    return { proposal: proposal(candidate, `proposal-large-${index}`), evaluation: candidate };
  });
  assert.throws(() => runAutopilot({ runId: 'run:aggregate', maximumCycles: 20, proposals: oversized }, { now }), /aggregate observation bound/u);
});

test('direct callers cannot use cyclic or excessively deep structures to bypass scanning', () => {
  const valid = entry();
  valid.evaluation.loop = valid;
  assert.throws(() => runAutopilot({ runId: 'run:cycle', maximumCycles: 1, proposals: [valid] }, { now }), /repeated_or_cyclic_object_reference/u);

  let nested = {};
  for (let index = 0; index < 70; index += 1) nested = { child: nested };
  assert.throws(() => runAutopilot({
    runId: 'run:depth',
    maximumCycles: 1,
    proposals: [{ ...entry(), nested }],
  }, { now }), /structure_depth_exceeded/u);
});

test('a completed checkpoint resumes as a stable no-op', () => {
  const proposals = [entry()];
  const complete = runAutopilot({ runId: 'run:complete', maximumCycles: 1, proposals }, { now });
  const resumed = runAutopilot({ runId: 'run:complete', maximumCycles: 1, proposals, checkpoint: complete.checkpoint }, { now });
  const repeated = runAutopilot({ runId: 'run:complete', maximumCycles: 1, proposals, checkpoint: resumed.checkpoint }, { now });
  assert.equal(resumed.processedCycles, 0);
  assert.equal(resumed.stopReason, 'BATCH_COMPLETE');
  assert.deepEqual(resumed.checkpoint, complete.checkpoint);
  assert.deepEqual(repeated, resumed);
});

test('chunked processing preserves one shot disposition order', () => {
  const proposals = [entry('proposal-1'), entry('proposal-2'), entry('proposal-3')];
  const oneShot = runAutopilot({ runId: 'run:chunks', maximumCycles: 3, proposals }, { now });
  const first = runAutopilot({ runId: 'run:chunks', maximumCycles: 1, proposals }, { now });
  const second = runAutopilot({ runId: 'run:chunks', maximumCycles: 1, proposals, checkpoint: first.checkpoint }, { now });
  const third = runAutopilot({ runId: 'run:chunks', maximumCycles: 1, proposals, checkpoint: second.checkpoint }, { now });
  assert.deepEqual([...first.dispositions, ...second.dispositions, ...third.dispositions], oneShot.dispositions);
  assert.equal(third.nextCursor, 3);
  assert.equal(third.executionAuthorized, false);
  assert.equal(third.promotionAuthorized, false);
});

test('read only tool integration invokes a complete autopilot run', async () => {
  const output = await runTool('social_autopilot_run', {
    runId: 'run:tool-integration',
    maximumCycles: 1,
    proposals: [entry()],
  });
  assert.equal(output.ok, true);
  assert.equal(output.run.stopReason, 'BATCH_COMPLETE');
  assert.equal(output.run.externalActionsAttempted, 0);
  assert.equal(output.run.checkpointAuthorityVerified, false);
});

test('fresh CLI process invokes the bounded autopilot', () => {
  const child = spawnSync(process.execPath, ['bin/cli.js', 'autopilot', 'run'], {
    cwd: new URL('..', import.meta.url),
    encoding: 'utf8',
    input: JSON.stringify({ runId: 'run:cli-integration', maximumCycles: 1, proposals: [entry()] }),
    timeout: 5000,
  });
  assert.equal(child.status, 0, child.stderr);
  const output = JSON.parse(child.stdout);
  assert.equal(output.ok, true);
  assert.equal(output.run.processedCycles, 1);
  assert.equal(output.run.networkAttempted, false);
  assert.equal(output.run.executionAuthorized, false);
});

test('fresh MCP process invokes the bounded autopilot', () => {
  const input = { runId: 'run:mcp-integration', maximumCycles: 1, proposals: [entry()] };
  const request = { jsonrpc: '2.0', id: 352, method: 'tools/call', params: { name: 'social_autopilot_run', arguments: input } };
  const child = spawnSync(process.execPath, ['bin/cli.js', 'mcp', 'start'], {
    cwd: new URL('..', import.meta.url),
    encoding: 'utf8',
    input: `${JSON.stringify(request)}\n`,
    timeout: 5000,
  });
  assert.equal(child.status, 0, child.stderr);
  const response = JSON.parse(child.stdout);
  const output = JSON.parse(response.result.content[0].text);
  assert.equal(response.result.isError, false);
  assert.equal(output.ok, true);
  assert.equal(output.run.processedCycles, 1);
  assert.equal(output.run.promotionAuthorized, false);
});
