import assert from 'node:assert/strict';
import test from 'node:test';

import { createAutopilotReceipt, createReceipt, verifyReceiptChain } from '../src/audit.js';
import { sha256 } from '../src/canonical.js';

const digest = (character) => `sha256:${character.repeat(64)}`;

function event(overrides = {}) {
  return {
    eventType: 'policy_decision',
    platform: 'linkedin',
    account: 'reuvencohen',
    identityScope: 'ruv_personal',
    operation: 'publish_member_or_organization_post',
    targetDigest: digest('a'),
    evidenceDigest: digest('b'),
    policyDigest: digest('c'),
    actionResult: 'approval_required',
    expiresAt: '2030-01-01T00:00:00.000Z',
    ...overrides,
  };
}

function completedAutopilotRun(overrides = {}) {
  const scopeDigest = sha256({ schema: 'AutopilotScopeV1', platform: 'linkedin', account: 'reuvencohen', identityScope: 'ruv_personal' });
  const checkpointBody = {
    schema: 'AutopilotCheckpointV1',
    runId: 'run:audit',
    batchDigest: digest('1'),
    identityRegistryDigest: digest('2'),
    scopeDigest,
    nextCursor: 1,
    processedProposalDigests: [digest('3')],
    previousCheckpointDigest: null,
  };
  const body = {
    schema: 'AutopilotRunV1',
    runId: 'run:audit',
    batchDigest: digest('1'),
    identityRegistryDigest: digest('2'),
    scopeDigest,
    startCursor: 1,
    nextCursor: 1,
    processedCycles: 0,
    totalProposals: 1,
    stopReason: 'BATCH_COMPLETE',
    dispositions: [],
    independentVerificationQueue: [],
    rejected: [],
    checkpoint: { ...checkpointBody, checkpointDigest: sha256(checkpointBody) },
    networkAttempted: false,
    credentialStoresRead: false,
    accountConnectionsCreated: 0,
    externalActionsAttempted: 0,
    executionAuthorized: false,
    reviewEligibilityEstablished: false,
    promotionAuthorized: false,
    selfMutationAuthorized: false,
    checkpointAuthorityVerified: false,
    ...overrides,
  };
  return { ...body, runDigest: sha256(body) };
}

test('digest receipts form a verifiable context-bound chain', () => {
  const first = createReceipt(event({ actionResult: 'approval_required', contentDigest: digest('d') }), { sequence: 0, timestamp: '2026-08-29T12:00:00.000Z' });
  const second = createReceipt(event({ eventType: 'approval_challenge', actionResult: 'not_approved', intentDigest: digest('e') }), {
    sequence: 1,
    previousHash: first.receiptHash,
    timestamp: '2026-08-29T12:01:00.000Z',
  });
  assert.deepEqual(verifyReceiptChain([first, second], { expectedHead: second.receiptHash, expectedCount: 2 }), {
    ok: true,
    errors: [],
    head: second.receiptHash,
    count: 2,
  });
});

test('tampering is detected', () => {
  const receipt = createReceipt(event());
  const tampered = { ...receipt, event: { ...receipt.event, actionResult: 'allow' } };
  const result = verifyReceiptChain([tampered], { expectedHead: receipt.receiptHash, expectedCount: 1 });
  assert.equal(result.ok, false);
  assert.match(result.errors.join(' '), /digest mismatch/u);
});

test('raw content and credential fields cannot enter a receipt', () => {
  assert.throws(() => createReceipt(event({ message: 'private text' })), /not allowed/u);
  const sensitiveField = `access_${'token'}`;
  assert.throws(() => createReceipt(event({ [sensitiveField]: 'x' })), /sensitive material/u);
  assert.throws(() => createReceipt(event({ evidenceDigest: 'raw private text' })), /sha256 digest/u);
});

test('sparse context, wrong schema, and arbitrary event fields never verify', () => {
  assert.throws(() => createReceipt({ eventType: 'policy_decision', actionResult: 'deny' }), /fields are required/u);
  const body = {
    schema: 'WrongSchema',
    sequence: 0,
    timestamp: '2026-08-29T12:00:00.000Z',
    previousHash: null,
    event: event({ message: 'private text' }),
  };
  const result = verifyReceiptChain([{ ...body, receiptHash: sha256(body) }], { expectedHead: sha256(body), expectedCount: 1 });
  assert.equal(result.ok, false);
  assert.match(result.errors.join(' '), /schema mismatch/u);
  assert.match(result.errors.join(' '), /event invalid/u);
});

test('middle, tail, all deletion, and replay are detected against a checkpoint', () => {
  const first = createReceipt(event(), { sequence: 0, timestamp: '2026-08-29T12:00:00.000Z' });
  const second = createReceipt(event(), { sequence: 1, previousHash: first.receiptHash, timestamp: '2026-08-29T12:01:00.000Z' });
  const third = createReceipt(event(), { sequence: 2, previousHash: second.receiptHash, timestamp: '2026-08-29T12:02:00.000Z' });
  const checkpoint = { expectedHead: third.receiptHash, expectedCount: 3 };
  assert.equal(verifyReceiptChain([first, third], checkpoint).ok, false);
  assert.equal(verifyReceiptChain([first, second], checkpoint).ok, false);
  assert.match(verifyReceiptChain([first, second], checkpoint).errors.join(' '), /trusted checkpoint/u);
  assert.equal(verifyReceiptChain([], checkpoint).ok, false);
  const replay = verifyReceiptChain([first, first], { expectedHead: first.receiptHash, expectedCount: 2 });
  assert.equal(replay.ok, false);
  assert.match(replay.errors.join(' '), /replays an earlier receipt/u);
});

test('verification without an externally retained checkpoint fails closed', () => {
  const receipt = createReceipt(event());
  assert.equal(verifyReceiptChain([receipt]).ok, false);
  assert.match(verifyReceiptChain([receipt]).errors.join(' '), /checkpoint/u);
});

test('autopilot runs can be recorded only as digest bound non-execution events', () => {
  const run = completedAutopilotRun();
  const receipt = createAutopilotReceipt(run, {
    platform: 'linkedin',
    account: 'reuvencohen',
    identityScope: 'ruv_personal',
    policyDigest: digest('4'),
    expiresAt: '2030-01-01T00:00:00.000Z',
  }, { timestamp: '2026-08-29T12:00:00.000Z' });
  const verified = verifyReceiptChain([receipt], { expectedHead: receipt.receiptHash, expectedCount: 1 });
  assert.equal(verified.ok, true);
  assert.equal(JSON.stringify(receipt).includes('oneChange'), false);
  assert.equal(receipt.event.actionResult, 'batch_complete');
  assert.equal(receipt.event.targetDigest, run.batchDigest);
  assert.equal(receipt.event.evidenceDigest, run.runDigest);
});

test('audit events reject false autopilot success and cross identity bindings', () => {
  assert.throws(() => createReceipt(event({
    eventType: 'autopilot_run',
    operation: 'proposal_evaluation',
    actionResult: 'published_successfully',
  })), /actionResult is not registered/u);
  assert.throws(() => createReceipt(event({ account: 'ruvnet', identityScope: 'ruvnet' })), /not registered/u);

  const promoted = completedAutopilotRun({ promotionAuthorized: true });
  assert.throws(() => createAutopilotReceipt(promoted, {
    platform: 'linkedin',
    account: 'reuvencohen',
    identityScope: 'ruv_personal',
    policyDigest: digest('4'),
    expiresAt: '2030-01-01T00:00:00.000Z',
  }), /authority invariant/u);
});

test('semantically equivalent but noncanonical identity receipts fail verification', () => {
  const canonical = createReceipt(event(), { timestamp: '2026-08-29T12:00:00.000Z' });
  const body = {
    ...canonical,
    event: { ...canonical.event, platform: 'LinkedIn', account: 'ReuvenCohen' },
  };
  delete body.receiptHash;
  const forged = { ...body, receiptHash: sha256(body) };
  const result = verifyReceiptChain([forged], { expectedHead: forged.receiptHash, expectedCount: 1 });
  assert.equal(result.ok, false);
  assert.match(result.errors.join(' '), /not canonical/u);
});
