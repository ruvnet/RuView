// SPDX-License-Identifier: MIT

import { canonicalJson, sha256 } from './canonical.js';
import { validateAutopilotRun } from './autopilot.js';
import { resolveIdentityBinding } from './identities.js';
import { loadPlatformRegistry, operationClass } from './platforms.js';
import { assertNoCredentialFields, isPlainObject } from './validation.js';

const DIGEST_RE = /^sha256:[a-f0-9]{64}$/u;
const RECEIPT_FIELDS = new Set(['event', 'previousHash', 'receiptHash', 'schema', 'sequence', 'timestamp']);
const DIGEST_FIELDS = new Set(['approvalDigest', 'contentDigest', 'evidenceDigest', 'intentDigest', 'policyDigest', 'targetDigest']);
const REQUIRED_EVENT_FIELDS = new Set(['account', 'actionResult', 'evidenceDigest', 'eventType', 'expiresAt', 'identityScope', 'operation', 'platform', 'policyDigest', 'targetDigest']);
const ROUTES = new Set(['API_ALLOWED', 'ATTENDED_MANUAL', 'DENY', 'LOCAL_READ_ONLY']);
const EVENT_CONTRACTS = Object.freeze({
  approval_challenge: {
    operation: 'registered_platform_external_effect',
    actionResults: new Set(['not_approved']),
    optional: new Set(['contentDigest', 'intentDigest', 'principal', 'requiredHumanAction', 'route']),
    required: new Set(['intentDigest']),
  },
  autopilot_run: {
    operation: 'proposal_evaluation',
    actionResults: new Set(['batch_complete', 'cycle_limit']),
    optional: new Set(),
    required: new Set(),
  },
  flywheel_evaluation: {
    operation: 'candidate_screening',
    actionResults: new Set(['independent_verification_required', 'rejected']),
    optional: new Set(['contentDigest']),
    required: new Set(),
  },
  metric_normalization: {
    operation: 'normalize_snapshot',
    actionResults: new Set(['normalized', 'rejected']),
    optional: new Set(['contentDigest']),
    required: new Set(),
  },
  policy_decision: {
    operation: 'registered_platform_operation',
    actionResults: new Set(['approval_required', 'attended_manual_handoff', 'deny', 'read_only_plan']),
    optional: new Set(['contentDigest', 'policyRule', 'principal', 'requiredHumanAction', 'route']),
    required: new Set(),
  },
});
const AUTOPILOT_CONTEXT_FIELDS = new Set(['account', 'expiresAt', 'identityScope', 'platform', 'policyDigest']);

function normalizedIso(value) {
  return typeof value === 'string'
    && Number.isFinite(Date.parse(value))
    && new Date(value).toISOString() === value;
}

function sanitizeEvent(event) {
  if (!isPlainObject(event)) throw new TypeError('Audit event must be an object');
  const sensitiveErrors = assertNoCredentialFields(event);
  if (sensitiveErrors.length) throw new TypeError(sensitiveErrors.join('; '));
  const missing = [...REQUIRED_EVENT_FIELDS].filter((key) => !Object.hasOwn(event, key));
  if (missing.length) throw new TypeError(`Audit event fields are required: ${missing.join(', ')}`);
  const contract = EVENT_CONTRACTS[event.eventType];
  if (!contract) throw new TypeError('eventType is not registered');
  const allowed = new Set([...REQUIRED_EVENT_FIELDS, ...contract.optional]);
  const unknown = Object.keys(event).filter((key) => !allowed.has(key));
  if (unknown.length) throw new TypeError(`Audit event field is not allowed: ${unknown.join(', ')}`);
  const missingContract = [...contract.required].filter((key) => !Object.hasOwn(event, key));
  if (missingContract.length) throw new TypeError(`Audit event contract fields are required: ${missingContract.join(', ')}`);
  const sanitized = {};
  for (const [key, value] of Object.entries(event)) {
    if (typeof value !== 'string' || value.length === 0 || value.length > 512 || /[\r\n]/u.test(value)) {
      throw new TypeError(`Audit field ${key} must be one bounded single-line string`);
    }
    if (DIGEST_FIELDS.has(key) && !DIGEST_RE.test(value)) {
      throw new TypeError(`Audit field ${key} must be a sha256 digest`);
    }
    sanitized[key] = value;
  }
  if (!normalizedIso(sanitized.expiresAt)) throw new TypeError('expiresAt must be a normalized ISO timestamp');
  if (!contract.actionResults.has(sanitized.actionResult)) throw new TypeError(`${sanitized.eventType} actionResult is not registered`);
  if (sanitized.route !== undefined && !ROUTES.has(sanitized.route)) throw new TypeError('Audit route is not registered');
  const identity = resolveIdentityBinding(sanitized.platform, sanitized.account, sanitized.identityScope);
  sanitized.platform = identity.platform;
  sanitized.account = identity.account;

  if (contract.operation === 'proposal_evaluation' || contract.operation === 'candidate_screening' || contract.operation === 'normalize_snapshot') {
    if (sanitized.operation !== contract.operation) throw new TypeError(`${sanitized.eventType} operation is not registered`);
    if (sanitized.route !== undefined && sanitized.route !== 'LOCAL_READ_ONLY') throw new TypeError(`${sanitized.eventType} route must be LOCAL_READ_ONLY`);
  } else {
    const registry = loadPlatformRegistry();
    const policy = registry.platforms[sanitized.platform]?.operations?.[sanitized.operation];
    if (!policy) throw new TypeError('Audit platform operation is not registered');
    if (sanitized.route !== undefined && sanitized.route !== policy.route) throw new TypeError('Audit route does not match the registered platform policy');
    const classification = operationClass(policy);
    if (contract.operation === 'registered_platform_external_effect') {
      if (classification !== 'external_effect') throw new TypeError('approval_challenge requires a registered external effect');
    } else {
      const expectedResult = classification === 'deny'
        ? 'deny'
        : classification === 'setup'
          ? 'attended_manual_handoff'
          : classification === 'read'
            ? 'read_only_plan'
            : 'approval_required';
      if (sanitized.actionResult !== expectedResult) throw new TypeError('policy_decision actionResult does not match the registered operation class');
    }
  }
  return Object.freeze(sanitized);
}

export function createAutopilotReceipt(run, context, options = {}) {
  validateAutopilotRun(run);
  if (!isPlainObject(context)) throw new TypeError('Autopilot audit context is required');
  const sensitiveErrors = assertNoCredentialFields(context);
  if (sensitiveErrors.length) throw new TypeError(sensitiveErrors.join('; '));
  const unknown = Object.keys(context).filter((key) => !AUTOPILOT_CONTEXT_FIELDS.has(key));
  const missing = [...AUTOPILOT_CONTEXT_FIELDS].filter((key) => !Object.hasOwn(context, key));
  if (unknown.length || missing.length) throw new TypeError(`Autopilot audit context fields are invalid: ${[...unknown, ...missing].join(', ')}`);
  const identity = resolveIdentityBinding(context.platform, context.account, context.identityScope);
  const scopeDigest = sha256({
    schema: 'AutopilotScopeV1',
    platform: identity.platform,
    account: identity.account,
    identityScope: identity.identityScope,
  });
  if (scopeDigest !== run.scopeDigest) throw new TypeError('Autopilot audit context does not match the run scope');
  return createReceipt({
    eventType: 'autopilot_run',
    platform: identity.platform,
    account: identity.account,
    identityScope: identity.identityScope,
    operation: 'proposal_evaluation',
    targetDigest: run.batchDigest,
    evidenceDigest: run.runDigest,
    policyDigest: context.policyDigest,
    actionResult: run.stopReason === 'BATCH_COMPLETE' ? 'batch_complete' : 'cycle_limit',
    expiresAt: context.expiresAt,
  }, options);
}

export function createReceipt(event, { previousHash = null, timestamp = new Date().toISOString(), sequence = 0 } = {}) {
  if (!Number.isSafeInteger(sequence) || sequence < 0) throw new TypeError('sequence must be a non-negative integer');
  if (!normalizedIso(timestamp)) throw new TypeError('timestamp must be a normalized ISO timestamp');
  if (previousHash !== null && !DIGEST_RE.test(previousHash)) throw new TypeError('previousHash must be null or sha256');
  const sanitized = sanitizeEvent(event);
  if (Date.parse(sanitized.expiresAt) <= Date.parse(timestamp)) throw new TypeError('event expiry must follow receipt timestamp');
  const body = {
    schema: 'SocialAuditReceiptV1',
    sequence,
    timestamp,
    previousHash,
    event: sanitized,
  };
  return Object.freeze({ ...body, receiptHash: sha256(body) });
}

export function verifyReceiptChain(receipts, checkpoint) {
  if (!Array.isArray(receipts)) return { ok: false, errors: ['receipts must be an array'] };
  const errors = [];
  if (!isPlainObject(checkpoint)) {
    errors.push('trusted checkpoint with expectedHead and expectedCount is required');
  } else {
    const keys = Object.keys(checkpoint);
    if (keys.some((key) => !['expectedCount', 'expectedHead'].includes(key))) errors.push('checkpoint has unknown fields');
    if (!Number.isSafeInteger(checkpoint.expectedCount) || checkpoint.expectedCount < 0) errors.push('checkpoint expectedCount is invalid');
    if (checkpoint.expectedHead !== null && !DIGEST_RE.test(checkpoint.expectedHead || '')) errors.push('checkpoint expectedHead is invalid');
    if (checkpoint.expectedCount === 0 && checkpoint.expectedHead !== null) errors.push('empty checkpoint head must be null');
    if (checkpoint.expectedCount > 0 && checkpoint.expectedHead === null) errors.push('non-empty checkpoint head must be sha256');
  }
  const seen = new Set();
  let previousHash = null;
  let previousTime = null;
  receipts.forEach((receipt, index) => {
    if (!isPlainObject(receipt)) {
      errors.push(`receipt ${index} must be an object`);
      return;
    }
    const unknown = Object.keys(receipt).filter((key) => !RECEIPT_FIELDS.has(key));
    if (unknown.length) errors.push(`receipt ${index} has unknown fields: ${unknown.join(', ')}`);
    if (receipt.schema !== 'SocialAuditReceiptV1') errors.push(`receipt ${index} schema mismatch`);
    if (receipt.sequence !== index) errors.push(`receipt ${index} sequence mismatch`);
    if (!normalizedIso(receipt.timestamp)) errors.push(`receipt ${index} timestamp is invalid`);
    if (previousTime !== null && normalizedIso(receipt.timestamp) && Date.parse(receipt.timestamp) < previousTime) {
      errors.push(`receipt ${index} timestamp moved backwards`);
    }
    if (receipt.previousHash !== null && !DIGEST_RE.test(receipt.previousHash || '')) {
      errors.push(`receipt ${index} previous hash is invalid`);
    }
    if (receipt.previousHash !== previousHash) errors.push(`receipt ${index} previous hash mismatch`);
    if (!DIGEST_RE.test(receipt.receiptHash || '')) errors.push(`receipt ${index} receipt hash is invalid`);
    if (seen.has(receipt.receiptHash)) errors.push(`receipt ${index} replays an earlier receipt`);
    try {
      const event = sanitizeEvent(receipt.event);
      if (canonicalJson(event) !== canonicalJson(receipt.event)) errors.push(`receipt ${index} event is not canonical`);
      if (normalizedIso(receipt.timestamp) && Date.parse(event.expiresAt) <= Date.parse(receipt.timestamp)) {
        errors.push(`receipt ${index} event expiry does not follow timestamp`);
      }
    } catch (cause) {
      errors.push(`receipt ${index} event invalid: ${cause instanceof Error ? cause.message : String(cause)}`);
    }
    const { receiptHash, ...body } = receipt;
    if (sha256(body) !== receiptHash) errors.push(`receipt ${index} digest mismatch`);
    if (DIGEST_RE.test(receiptHash || '')) seen.add(receiptHash);
    previousHash = receiptHash;
    if (normalizedIso(receipt.timestamp)) previousTime = Date.parse(receipt.timestamp);
  });
  if (isPlainObject(checkpoint)) {
    if (receipts.length !== checkpoint.expectedCount) errors.push('receipt count does not match trusted checkpoint');
    if (previousHash !== checkpoint.expectedHead) errors.push('receipt head does not match trusted checkpoint');
  }
  return { ok: errors.length === 0, errors, head: previousHash, count: receipts.length };
}
