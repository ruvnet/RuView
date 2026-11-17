// SPDX-License-Identifier: MIT

import { canonicalJson, sha256 } from './canonical.js';
import { evaluateCandidate } from './flywheel.js';
import { loadIdentityRegistry, resolveIdentityBinding } from './identities.js';
import { loadPlatformRegistry } from './platforms.js';
import { scanSensitive } from './sensitive.js';
import { normalizedIso } from './validation.js';

const DIGEST_RE = /^sha256:[a-f0-9]{64}$/u;
const RUN_ID_RE = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$/u;
const IDENTITY_SCOPES = new Set(['agentics', 'cognitum', 'ruv_personal', 'ruvnet']);
const CHANGE_TYPES = new Set(['CONTENT_STRUCTURE', 'EVIDENCE_PRESENTATION', 'TIMING_HYPOTHESIS', 'VOICE_RULE']);
const INPUT_FIELDS = new Set(['checkpoint', 'maximumCycles', 'proposals', 'runId']);
const ENTRY_FIELDS = new Set(['evaluation', 'proposal']);
const PROPOSAL_FIELDS = new Set([
  'account',
  'changeType',
  'createdAt',
  'datasetDigest',
  'expectedEffect',
  'experimentPlanDigest',
  'expiresAt',
  'identityScope',
  'oneChange',
  'platform',
  'proposalDigest',
  'proposalId',
  'rationale',
  'rollback',
  'schema',
  'sourceDigests',
]);
const EVALUATION_FIELDS = new Set([
  'baseline',
  'datasetDigest',
  'experimentPlan',
  'experimentPlanDigest',
  'gateReceipts',
  'observationBindings',
  'variant',
]);
const CHECKPOINT_FIELDS = new Set([
  'batchDigest',
  'checkpointDigest',
  'identityRegistryDigest',
  'nextCursor',
  'previousCheckpointDigest',
  'processedProposalDigests',
  'runId',
  'schema',
  'scopeDigest',
]);
const RUN_FIELDS = new Set([
  'accountConnectionsCreated',
  'batchDigest',
  'checkpoint',
  'checkpointAuthorityVerified',
  'credentialStoresRead',
  'dispositions',
  'executionAuthorized',
  'externalActionsAttempted',
  'identityRegistryDigest',
  'independentVerificationQueue',
  'networkAttempted',
  'nextCursor',
  'processedCycles',
  'promotionAuthorized',
  'rejected',
  'reviewEligibilityEstablished',
  'runDigest',
  'runId',
  'schema',
  'scopeDigest',
  'selfMutationAuthorized',
  'startCursor',
  'stopReason',
  'totalProposals',
]);
const DISPOSITION_FIELDS = new Set(['disposition', 'evaluationDigest', 'promotionAuthorized', 'proposalDigest', 'recommendation', 'reviewEligibilityEstablished']);
const DISPOSITIONS = new Set(['QUEUED_FOR_INDEPENDENT_VERIFICATION', 'REJECTED_BINDING_MISMATCH', 'REJECTED_INVALID_EVIDENCE', 'REJECTED_SCREENING', 'REJECTED_UPSTREAM_AUTHORITY_DRIFT']);
const RECOMMENDATIONS = new Set(['REJECT_OR_COLLECT_MORE_EVIDENCE', 'SCREENING_PASSED_REQUIRES_INDEPENDENT_GATE_VERIFICATION']);

function exactObject(value, name, fields) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new TypeError(`${name} must be an object`);
  const unknown = Object.keys(value).filter((key) => !fields.has(key));
  const missing = [...fields].filter((key) => !Object.hasOwn(value, key));
  if (unknown.length || missing.length) throw new TypeError(`${name} fields are invalid: ${[...unknown, ...missing].join(', ')}`);
}

function boundedString(value, name, maximum) {
  if (typeof value !== 'string' || value.trim().length === 0 || value.length > maximum) throw new TypeError(`${name} is required and must be at most ${maximum} characters`);
  return value.trim();
}

function digest(value, name) {
  if (!DIGEST_RE.test(value || '')) throw new TypeError(`${name} must be sha256`);
  return value;
}

function checkpointDigest(value) {
  const { checkpointDigest: omitted, ...body } = value;
  return sha256(body);
}

export function proposalDigest(value) {
  const { proposalDigest: omitted, ...body } = value;
  return sha256(body);
}

function normalizeProposal(value, now, platforms, identityRegistry) {
  exactObject(value, 'proposal', PROPOSAL_FIELDS);
  if (value.schema !== 'OptimizationProposalV1') throw new TypeError('proposal schema mismatch');
  const platform = boundedString(value.platform, 'proposal.platform', 32).toLocaleLowerCase('en-US');
  if (!Object.hasOwn(platforms, platform)) throw new TypeError('proposal.platform is not registered');
  const identityScope = boundedString(value.identityScope, 'proposal.identityScope', 64);
  if (!IDENTITY_SCOPES.has(identityScope)) throw new TypeError('proposal.identityScope is not registered');
  const account = boundedString(value.account, 'proposal.account', 128).toLocaleLowerCase('en-US');
  resolveIdentityBinding(platform, account, identityScope, identityRegistry);
  if (!CHANGE_TYPES.has(value.changeType)) throw new TypeError('proposal.changeType is not registered');
  if (!normalizedIso(value.createdAt) || !normalizedIso(value.expiresAt)) throw new TypeError('proposal timestamps must be normalized ISO timestamps');
  const createdAt = Date.parse(value.createdAt);
  const expiresAt = Date.parse(value.expiresAt);
  if (createdAt > now.getTime()) throw new TypeError('proposal.createdAt cannot be in the future');
  if (expiresAt <= now.getTime() || expiresAt <= createdAt) throw new TypeError('proposal is expired or has invalid expiry');
  if (expiresAt - createdAt > 90 * 24 * 60 * 60 * 1000) throw new TypeError('proposal lifetime cannot exceed 90 days');
  if (!Array.isArray(value.sourceDigests) || value.sourceDigests.length < 2 || value.sourceDigests.length > 16) {
    throw new TypeError('proposal.sourceDigests must contain between two and 16 digests');
  }
  value.sourceDigests.forEach((item, index) => digest(item, `proposal.sourceDigests[${index}]`));
  if (new Set(value.sourceDigests).size !== value.sourceDigests.length) throw new TypeError('proposal.sourceDigests contains duplicates');
  const normalized = {
    schema: 'OptimizationProposalV1',
    proposalId: boundedString(value.proposalId, 'proposal.proposalId', 64),
    platform,
    account,
    identityScope,
    changeType: value.changeType,
    oneChange: boundedString(value.oneChange, 'proposal.oneChange', 2000),
    rationale: boundedString(value.rationale, 'proposal.rationale', 4000),
    expectedEffect: boundedString(value.expectedEffect, 'proposal.expectedEffect', 1024),
    rollback: boundedString(value.rollback, 'proposal.rollback', 2048),
    sourceDigests: [...value.sourceDigests],
    experimentPlanDigest: digest(value.experimentPlanDigest, 'proposal.experimentPlanDigest'),
    datasetDigest: digest(value.datasetDigest, 'proposal.datasetDigest'),
    createdAt: value.createdAt,
    expiresAt: value.expiresAt,
  };
  if (!normalized.sourceDigests.includes(normalized.experimentPlanDigest) || !normalized.sourceDigests.includes(normalized.datasetDigest)) {
    throw new TypeError('proposal.sourceDigests must bind the experiment plan and dataset');
  }
  if (proposalDigest(normalized) !== value.proposalDigest) throw new TypeError('proposal.proposalDigest mismatch');
  return Object.freeze({ ...normalized, proposalDigest: value.proposalDigest });
}

function validateEvaluationInput(value, name) {
  exactObject(value, name, EVALUATION_FIELDS);
  if (!Array.isArray(value.baseline) || !Array.isArray(value.variant)) throw new TypeError(`${name} observations must be arrays`);
  if (value.baseline.length > 10_000 || value.variant.length > 10_000) throw new TypeError(`${name} observations exceed the 10000 item bound`);
}

function normalizeEntries(values, now, identityRegistry) {
  if (!Array.isArray(values) || values.length === 0 || values.length > 100) throw new TypeError('proposals must contain between one and 100 entries');
  const platforms = loadPlatformRegistry().platforms;
  const seen = new Set();
  let scope = null;
  let totalObservations = 0;
  const entries = values.map((entry, index) => {
    exactObject(entry, `proposals[${index}]`, ENTRY_FIELDS);
    const proposal = normalizeProposal(entry.proposal, now, platforms, identityRegistry);
    const proposalScope = { platform: proposal.platform, account: proposal.account, identityScope: proposal.identityScope };
    if (scope === null) scope = proposalScope;
    else if (canonicalJson(scope) !== canonicalJson(proposalScope)) throw new TypeError('one autopilot batch must remain within one exact account and identity scope');
    if (seen.has(proposal.proposalDigest)) throw new TypeError('proposal digests must be unique within one batch');
    seen.add(proposal.proposalDigest);
    validateEvaluationInput(entry.evaluation, `proposals[${index}].evaluation`);
    totalObservations += entry.evaluation.baseline.length + entry.evaluation.variant.length;
    if (totalObservations > 20_000) throw new TypeError('proposals exceed the 20000 aggregate observation bound');
    return Object.freeze({
      proposal,
      evaluation: entry.evaluation,
      evaluationInputDigest: sha256({ schema: 'FlywheelEvaluationInputV1', ...entry.evaluation }),
    });
  });
  return { entries, scope: Object.freeze(scope) };
}

function batchDigest(runId, identityRegistryDigest, scopeDigest, entries) {
  return sha256({
    schema: 'AutopilotBatchV1',
    runId,
    identityRegistryDigest,
    scopeDigest,
    entries: entries.map(({ proposal, evaluationInputDigest }) => ({
      proposalDigest: proposal.proposalDigest,
      evaluationInputDigest,
    })),
  });
}

function normalizeCheckpoint(value, bindings, entries) {
  if (value === undefined || value === null) return null;
  exactObject(value, 'checkpoint', CHECKPOINT_FIELDS);
  if (value.schema !== 'AutopilotCheckpointV1') throw new TypeError('checkpoint schema mismatch');
  if (
    value.runId !== bindings.runId
    || value.batchDigest !== bindings.batchDigest
    || value.identityRegistryDigest !== bindings.identityRegistryDigest
    || value.scopeDigest !== bindings.scopeDigest
  ) {
    throw new TypeError('checkpoint is not bound to this run, batch, and identity registry');
  }
  if (!Number.isSafeInteger(value.nextCursor) || value.nextCursor < 0 || value.nextCursor > entries.length) throw new TypeError('checkpoint.nextCursor is outside the batch');
  if (!Array.isArray(value.processedProposalDigests) || value.processedProposalDigests.length !== value.nextCursor) {
    throw new TypeError('checkpoint processed digest count does not match nextCursor');
  }
  value.processedProposalDigests.forEach((item, index) => digest(item, `checkpoint.processedProposalDigests[${index}]`));
  const expected = entries.slice(0, value.nextCursor).map(({ proposal }) => proposal.proposalDigest);
  if (canonicalJson(value.processedProposalDigests) !== canonicalJson(expected)) throw new TypeError('checkpoint processed digests do not match the batch prefix');
  if (value.previousCheckpointDigest !== null) digest(value.previousCheckpointDigest, 'checkpoint.previousCheckpointDigest');
  digest(value.checkpointDigest, 'checkpoint.checkpointDigest');
  if (checkpointDigest(value) !== value.checkpointDigest) throw new TypeError('checkpoint.checkpointDigest mismatch');
  return Object.freeze({ ...value, processedProposalDigests: [...value.processedProposalDigests] });
}

function createCheckpoint(bindings, entries, nextCursor, previousCheckpointDigest) {
  const body = {
    schema: 'AutopilotCheckpointV1',
    runId: bindings.runId,
    batchDigest: bindings.batchDigest,
    identityRegistryDigest: bindings.identityRegistryDigest,
    scopeDigest: bindings.scopeDigest,
    nextCursor,
    processedProposalDigests: entries.slice(0, nextCursor).map(({ proposal }) => proposal.proposalDigest),
    previousCheckpointDigest,
  };
  return Object.freeze({ ...body, checkpointDigest: sha256(body) });
}

function dispositionFor(entry, now) {
  let evaluation;
  try {
    evaluation = evaluateCandidate(entry.evaluation, { now });
  } catch {
    return Object.freeze({
      proposalDigest: entry.proposal.proposalDigest,
      evaluationDigest: null,
      disposition: 'REJECTED_INVALID_EVIDENCE',
      recommendation: 'REJECT_OR_COLLECT_MORE_EVIDENCE',
      reviewEligibilityEstablished: false,
      promotionAuthorized: false,
    });
  }
  const proposal = entry.proposal;
  const plan = entry.evaluation.experimentPlan;
  const issuedAt = Object.values(entry.evaluation.gateReceipts).map((receipt) => Date.parse(receipt.issuedAt));
  const bindingsMatch = proposal.experimentPlanDigest === evaluation.experimentPlanDigest
    && proposal.datasetDigest === evaluation.datasetDigest
    && proposal.platform === evaluation.platform
    && proposal.identityScope === evaluation.identityScope
    && proposal.platform === String(plan.platform).trim().toLocaleLowerCase('en-US')
    && proposal.identityScope === plan.identityScope
    && issuedAt.every((time) => Number.isFinite(time) && Date.parse(proposal.createdAt) >= time);
  if (!bindingsMatch) {
    return Object.freeze({
      proposalDigest: proposal.proposalDigest,
      evaluationDigest: evaluation.evaluationDigest,
      disposition: 'REJECTED_BINDING_MISMATCH',
      recommendation: 'REJECT_OR_COLLECT_MORE_EVIDENCE',
      reviewEligibilityEstablished: false,
      promotionAuthorized: false,
    });
  }
  const queued = evaluation.recommendation === 'SCREENING_PASSED_REQUIRES_INDEPENDENT_GATE_VERIFICATION';
  const authorityInvariant = evaluation.gates?.gateAuthorityVerified === false
    && evaluation.reviewEligibilityEstablished === false
    && evaluation.promotionAuthorized === false
    && evaluation.causalClaimAllowed === false
    && ['REJECT_OR_COLLECT_MORE_EVIDENCE', 'SCREENING_PASSED_REQUIRES_INDEPENDENT_GATE_VERIFICATION'].includes(evaluation.recommendation);
  if (!authorityInvariant) {
    return Object.freeze({
      proposalDigest: proposal.proposalDigest,
      evaluationDigest: evaluation.evaluationDigest,
      disposition: 'REJECTED_UPSTREAM_AUTHORITY_DRIFT',
      recommendation: 'REJECT_OR_COLLECT_MORE_EVIDENCE',
      reviewEligibilityEstablished: false,
      promotionAuthorized: false,
    });
  }
  return Object.freeze({
    proposalDigest: proposal.proposalDigest,
    evaluationDigest: evaluation.evaluationDigest,
    disposition: queued ? 'QUEUED_FOR_INDEPENDENT_VERIFICATION' : 'REJECTED_SCREENING',
    recommendation: evaluation.recommendation,
    reviewEligibilityEstablished: false,
    promotionAuthorized: false,
  });
}

export function runAutopilot(input, { now = new Date() } = {}) {
  if (!input || typeof input !== 'object' || Array.isArray(input)) throw new TypeError('Autopilot input must be an object');
  const unknown = Object.keys(input).filter((key) => !INPUT_FIELDS.has(key));
  const missing = ['maximumCycles', 'proposals', 'runId'].filter((key) => !Object.hasOwn(input, key));
  if (unknown.length || missing.length) throw new TypeError(`Autopilot input fields are invalid: ${[...unknown, ...missing].join(', ')}`);
  if (typeof input.runId !== 'string' || !RUN_ID_RE.test(input.runId)) throw new TypeError('runId has an invalid format');
  if (!Number.isSafeInteger(input.maximumCycles) || input.maximumCycles < 1 || input.maximumCycles > 100) {
    throw new TypeError('maximumCycles must be between one and 100');
  }
  if (!Array.isArray(input.proposals) || input.proposals.length === 0 || input.proposals.length > 100) {
    throw new TypeError('proposals must contain between one and 100 entries');
  }
  const sensitive = scanSensitive(input);
  if (sensitive.length) throw new TypeError(`${sensitive[0].path} contains forbidden sensitive material (${sensitive[0].rule})`);
  if (!(now instanceof Date) || !Number.isFinite(now.getTime())) throw new TypeError('now must be a valid Date');
  const identityRegistry = loadIdentityRegistry();
  const identityRegistryDigest = sha256(identityRegistry);
  const normalized = normalizeEntries(input.proposals, now, identityRegistry);
  const entries = normalized.entries;
  const scopeDigest = sha256({ schema: 'AutopilotScopeV1', ...normalized.scope });
  const bindings = {
    runId: input.runId,
    identityRegistryDigest,
    scopeDigest,
    batchDigest: batchDigest(input.runId, identityRegistryDigest, scopeDigest, entries),
  };
  const prior = normalizeCheckpoint(input.checkpoint, bindings, entries);
  const startCursor = prior?.nextCursor || 0;
  const nextCursor = Math.min(entries.length, startCursor + input.maximumCycles);
  const dispositions = entries.slice(startCursor, nextCursor).map((entry) => dispositionFor(entry, now));
  const checkpoint = prior && startCursor === entries.length
    ? prior
    : createCheckpoint(bindings, entries, nextCursor, prior?.checkpointDigest || null);
  const independentVerificationQueue = dispositions
    .filter(({ disposition }) => disposition === 'QUEUED_FOR_INDEPENDENT_VERIFICATION')
    .map(({ proposalDigest: proposal, evaluationDigest }) => ({ proposalDigest: proposal, evaluationDigest }));
  const rejected = dispositions
    .filter(({ disposition }) => disposition !== 'QUEUED_FOR_INDEPENDENT_VERIFICATION')
    .map(({ proposalDigest: proposal, evaluationDigest, disposition }) => ({ proposalDigest: proposal, evaluationDigest, disposition }));
  const record = {
    schema: 'AutopilotRunV1',
    runId: input.runId,
    batchDigest: bindings.batchDigest,
    identityRegistryDigest,
    scopeDigest,
    startCursor,
    nextCursor,
    processedCycles: nextCursor - startCursor,
    totalProposals: entries.length,
    stopReason: nextCursor === entries.length ? 'BATCH_COMPLETE' : 'CYCLE_LIMIT',
    dispositions,
    independentVerificationQueue,
    rejected,
    checkpoint,
    networkAttempted: false,
    credentialStoresRead: false,
    accountConnectionsCreated: 0,
    externalActionsAttempted: 0,
    executionAuthorized: false,
    reviewEligibilityEstablished: false,
    promotionAuthorized: false,
    selfMutationAuthorized: false,
    checkpointAuthorityVerified: false,
  };
  return Object.freeze({ ...record, runDigest: sha256(record) });
}

export function validateAutopilotRun(value) {
  exactObject(value, 'AutopilotRunV1', RUN_FIELDS);
  if (value.schema !== 'AutopilotRunV1' || !RUN_ID_RE.test(value.runId || '')) throw new TypeError('AutopilotRunV1 schema or runId is invalid');
  ['batchDigest', 'identityRegistryDigest', 'scopeDigest', 'runDigest'].forEach((name) => digest(value[name], `AutopilotRunV1.${name}`));
  const { runDigest, ...body } = value;
  if (sha256(body) !== runDigest) throw new TypeError('AutopilotRunV1 runDigest mismatch');
  if (!Number.isSafeInteger(value.totalProposals) || value.totalProposals < 1 || value.totalProposals > 100) throw new TypeError('AutopilotRunV1 totalProposals is invalid');
  if (
    !Number.isSafeInteger(value.startCursor)
    || !Number.isSafeInteger(value.nextCursor)
    || !Number.isSafeInteger(value.processedCycles)
    || value.startCursor < 0
    || value.nextCursor < value.startCursor
    || value.nextCursor > value.totalProposals
    || value.processedCycles !== value.nextCursor - value.startCursor
  ) throw new TypeError('AutopilotRunV1 cursor state is invalid');
  const expectedStop = value.nextCursor === value.totalProposals ? 'BATCH_COMPLETE' : 'CYCLE_LIMIT';
  if (value.stopReason !== expectedStop) throw new TypeError('AutopilotRunV1 stopReason is invalid');
  if (!Array.isArray(value.dispositions) || value.dispositions.length !== value.processedCycles) throw new TypeError('AutopilotRunV1 dispositions are invalid');
  for (const disposition of value.dispositions) {
    exactObject(disposition, 'AutopilotRunV1 disposition', DISPOSITION_FIELDS);
    digest(disposition.proposalDigest, 'disposition.proposalDigest');
    if (disposition.evaluationDigest !== null) digest(disposition.evaluationDigest, 'disposition.evaluationDigest');
    if (!DISPOSITIONS.has(disposition.disposition) || !RECOMMENDATIONS.has(disposition.recommendation)) throw new TypeError('AutopilotRunV1 disposition enum is invalid');
    if (disposition.reviewEligibilityEstablished !== false || disposition.promotionAuthorized !== false) throw new TypeError('AutopilotRunV1 disposition authority invariant failed');
  }
  const expectedQueue = value.dispositions
    .filter(({ disposition }) => disposition === 'QUEUED_FOR_INDEPENDENT_VERIFICATION')
    .map(({ proposalDigest: proposal, evaluationDigest }) => ({ proposalDigest: proposal, evaluationDigest }));
  const expectedRejected = value.dispositions
    .filter(({ disposition }) => disposition !== 'QUEUED_FOR_INDEPENDENT_VERIFICATION')
    .map(({ proposalDigest: proposal, evaluationDigest, disposition }) => ({ proposalDigest: proposal, evaluationDigest, disposition }));
  if (canonicalJson(value.independentVerificationQueue) !== canonicalJson(expectedQueue) || canonicalJson(value.rejected) !== canonicalJson(expectedRejected)) {
    throw new TypeError('AutopilotRunV1 queue partition is invalid');
  }
  exactObject(value.checkpoint, 'AutopilotRunV1 checkpoint', CHECKPOINT_FIELDS);
  if (
    value.checkpoint.schema !== 'AutopilotCheckpointV1'
    || value.checkpoint.runId !== value.runId
    || value.checkpoint.batchDigest !== value.batchDigest
    || value.checkpoint.identityRegistryDigest !== value.identityRegistryDigest
    || value.checkpoint.scopeDigest !== value.scopeDigest
    || value.checkpoint.nextCursor !== value.nextCursor
    || !Array.isArray(value.checkpoint.processedProposalDigests)
    || value.checkpoint.processedProposalDigests.length !== value.nextCursor
  ) throw new TypeError('AutopilotRunV1 checkpoint bindings are invalid');
  value.checkpoint.processedProposalDigests.forEach((item, index) => digest(item, `checkpoint.processedProposalDigests[${index}]`));
  if (value.checkpoint.previousCheckpointDigest !== null) digest(value.checkpoint.previousCheckpointDigest, 'checkpoint.previousCheckpointDigest');
  digest(value.checkpoint.checkpointDigest, 'checkpoint.checkpointDigest');
  if (checkpointDigest(value.checkpoint) !== value.checkpoint.checkpointDigest) throw new TypeError('AutopilotRunV1 checkpoint digest mismatch');
  if (
    value.networkAttempted !== false
    || value.credentialStoresRead !== false
    || value.accountConnectionsCreated !== 0
    || value.externalActionsAttempted !== 0
    || value.executionAuthorized !== false
    || value.reviewEligibilityEstablished !== false
    || value.promotionAuthorized !== false
    || value.selfMutationAuthorized !== false
    || value.checkpointAuthorityVerified !== false
  ) throw new TypeError('AutopilotRunV1 authority invariant failed');
  return value;
}
