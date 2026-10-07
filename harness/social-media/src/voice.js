// SPDX-License-Identifier: MIT

import { readFileSync } from 'node:fs';
import { canonicalJson, sha256 } from './canonical.js';
import { scanSensitive } from './sensitive.js';
import { normalizedIso } from './validation.js';
import { resolveIdentityBinding } from './identities.js';
import { loadPlatformRegistry, operationClass, platformOperationPolicyDigest } from './platforms.js';

const POLICY = Object.freeze(JSON.parse(readFileSync(new URL('../config/direction.json', import.meta.url), 'utf8')));
const DIRECTION_FIELDS = new Set(['account', 'approvalRequired', 'audience', 'authorityEvidenceDigest', 'claims', 'conditionEvidenceDigest', 'content', 'contextEvidenceDigest', 'expiresAt', 'identityScope', 'operation', 'platform', 'platformPolicyDigest', 'principal', 'scheduledAt', 'source', 'target']);
const CLAIM_FIELDS = new Set(['grade', 'measured_at', 'reproducer', 'source_url']);
const ENVELOPE_FIELDS = Object.freeze([
  'schema',
  'principal',
  'source',
  'platform',
  'account',
  'identityScope',
  'operation',
  'audience',
  'target',
  'targetDigest',
  'contentDigest',
  'claimsDigest',
  'directionPolicyDigest',
  'platformPolicyDigest',
  'conditionEvidenceDigest',
  'authorityEvidenceDigest',
  'contextEvidenceDigest',
  'scheduledAt',
  'expiresAt',
  'approvalRequired',
  'voiceIsAuthority',
]);
const DIRECTION_RECORD_FIELDS = new Set([...ENVELOPE_FIELDS, 'idempotencyKey', 'intentDigest', 'warnings']);
const UNVERIFIED_AUTHORITY_DIGEST = sha256({ status: 'AUTHORITY_UNVERIFIED' });
const UNBOUND_CONTEXT_DIGEST = sha256({ status: 'CONTEXT_NOT_BOUND' });

function cleanString(value, name, maximum = 256) {
  if (typeof value !== 'string' || value.trim().length === 0 || value.length > maximum) {
    throw new TypeError(`${name} must be a non-empty string of at most ${maximum} characters`);
  }
  return value.trim();
}

function cleanTarget(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new TypeError('target must be an object');
  const allowed = new Set(['id', 'kind', 'parentId']);
  const unknown = Object.keys(value).filter((key) => !allowed.has(key));
  if (unknown.length) throw new TypeError(`Unknown target field: ${unknown.join(', ')}`);
  const target = {
    kind: cleanString(value.kind, 'target.kind', 64).toLocaleLowerCase('en-US'),
    id: cleanString(value.id, 'target.id', 512),
  };
  if (value.parentId !== undefined) target.parentId = cleanString(value.parentId, 'target.parentId', 512);
  return target;
}

export function getDirectionPolicy() {
  return structuredClone(POLICY);
}

export function lintContent({ platform, text, claims = [] }) {
  const channel = POLICY.channels[platform];
  if (!channel) return { ok: false, errors: [`Unknown platform: ${platform}`], warnings: [] };
  if (typeof text !== 'string' || text.trim().length === 0) {
    return { ok: false, errors: ['Content must be a non-empty string'], warnings: [] };
  }
  const errors = [];
  const warnings = [];
  const sensitive = scanSensitive({ platform, text, claims });
  if (sensitive.length) errors.push(`Content contains forbidden sensitive material (${sensitive[0].rule})`);
  if (text.length > channel.max_characters) errors.push(`Content exceeds ${channel.max_characters} characters`);

  for (const phrase of POLICY.flagged_phrases) {
    if (text.toLocaleLowerCase('en-US').includes(phrase)) warnings.push(`Flagged phrase requires evidence or removal: ${phrase}`);
  }

  const numeric = /(?:\b\d+(?:\.\d+)?(?:%|k|m|b)?\b|\$\d+)/iu.test(text);
  if (numeric && claims.length === 0) errors.push('Quantitative content requires at least one evidence record');
  claims.forEach((claim, index) => {
    if (!claim || typeof claim !== 'object' || Array.isArray(claim)) {
      errors.push(`claims[${index}] must be an object`);
      return;
    }
    const unknown = Object.keys(claim).filter((key) => !CLAIM_FIELDS.has(key));
    if (unknown.length) errors.push(`claims[${index}] contains unregistered fields`);
    if (!POLICY.claim_grades.includes(claim.grade)) errors.push(`claims[${index}].grade is invalid`);
    if (typeof claim.source_url !== 'string' || !/^https:\/\//u.test(claim.source_url)) errors.push(`claims[${index}].source_url must use https`);
    if (!normalizedIso(claim.measured_at)) errors.push(`claims[${index}].measured_at must be a normalized ISO timestamp`);
    if (claim.grade === 'MEASURED' && typeof claim.reproducer !== 'string') {
      warnings.push(`claims[${index}] is MEASURED without a reproducer`);
    }
  });

  return {
    ok: errors.length === 0,
    errors,
    warnings,
    channel: { maxCharacters: channel.max_characters, voice: channel.voice },
    contentDigest: sha256(text),
  };
}

export function createDirection(input, now = new Date()) {
  if (!input || typeof input !== 'object' || Array.isArray(input)) throw new TypeError('Direction input must be an object');
  const sensitive = scanSensitive(input);
  if (sensitive.length) throw new TypeError(`${sensitive[0].path} contains forbidden sensitive material (${sensitive[0].rule})`);
  const unknown = Object.keys(input).filter((key) => !DIRECTION_FIELDS.has(key));
  if (unknown.length) throw new TypeError(`Unknown direction field: ${unknown.join(', ')}`);
  const platform = cleanString(input.platform, 'platform', 32).toLocaleLowerCase('en-US');
  if (!POLICY.channels[platform]) throw new TypeError(`Unknown platform: ${platform}`);
  const operation = cleanString(input.operation, 'operation', 96).toLocaleLowerCase('en-US');
  const registry = loadPlatformRegistry();
  const operationPolicy = registry.platforms[platform]?.operations?.[operation];
  if (!operationPolicy || operationPolicy.route === 'DENY') throw new TypeError('Operation is not registered for direction planning');
  if (Date.parse(`${registry.review_expires_at}T23:59:59.999Z`) < now.getTime()) throw new TypeError('Platform policy review is expired');
  const classification = operationClass(operationPolicy);
  const account = cleanString(input.account, 'account', 128);
  const principal = cleanString(input.principal || POLICY.principal, 'principal', 128);
  const identityScope = cleanString(input.identityScope, 'identityScope', 64);
  if (!POLICY.identity_scopes.includes(identityScope)) throw new TypeError(`Unknown identity scope: ${identityScope}`);
  const identityBinding = resolveIdentityBinding(platform, account, identityScope);
  const source = input.source || 'text';
  if (!['text', 'voice'].includes(source)) throw new TypeError('source must be text or voice');
  const content = typeof input.content === 'string' ? input.content : '';
  const expiresAt = input.expiresAt;
  if (!normalizedIso(expiresAt) || Date.parse(expiresAt) <= now.getTime()) throw new TypeError('expiresAt must be a future normalized ISO timestamp');
  const scheduledAt = input.scheduledAt || null;
  if (scheduledAt !== null && !normalizedIso(scheduledAt)) throw new TypeError('scheduledAt must be null or a normalized ISO timestamp');
  if (scheduledAt !== null && Date.parse(scheduledAt) <= now.getTime()) throw new TypeError('scheduledAt must be in the future');
  if (scheduledAt !== null && Date.parse(scheduledAt) >= Date.parse(expiresAt)) throw new TypeError('scheduledAt must precede expiresAt');
  const claims = input.claims || [];
  const lint = content ? lintContent({ platform, text: content, claims }) : { ok: true, errors: [], warnings: [], contentDigest: sha256('') };
  if (!lint.ok) throw new TypeError(lint.errors.join('; '));

  const approvalRequired = input.approvalRequired === true || source === 'voice' || classification !== 'read';
  const targetRequired = classification === 'external_effect' || classification === 'setup' || approvalRequired;
  const target = targetRequired ? cleanTarget(input.target) : (input.target ? cleanTarget(input.target) : null);
  const digestField = (value, name, fallback) => {
    const result = value || fallback;
    if (!/^sha256:[a-f0-9]{64}$/u.test(result)) throw new TypeError(`${name} must be sha256`);
    return result;
  };
  const activePlatformPolicyDigest = platformOperationPolicyDigest(registry, platform, operation);
  if (input.platformPolicyDigest !== undefined && input.platformPolicyDigest !== activePlatformPolicyDigest) throw new TypeError('platformPolicyDigest does not match the active platform policy');
  const envelope = {
    schema: 'DirectionV1',
    principal,
    source,
    platform,
    account: identityBinding.account,
    identityScope,
    operation,
    audience: cleanString(input.audience || 'public', 'audience', 64),
    target,
    targetDigest: sha256(target),
    contentDigest: lint.contentDigest,
    claimsDigest: sha256(claims),
    directionPolicyDigest: sha256(POLICY),
    platformPolicyDigest: digestField(input.platformPolicyDigest, 'platformPolicyDigest', activePlatformPolicyDigest),
    conditionEvidenceDigest: digestField(input.conditionEvidenceDigest, 'conditionEvidenceDigest', sha256([])),
    authorityEvidenceDigest: digestField(input.authorityEvidenceDigest, 'authorityEvidenceDigest', UNVERIFIED_AUTHORITY_DIGEST),
    contextEvidenceDigest: digestField(input.contextEvidenceDigest, 'contextEvidenceDigest', UNBOUND_CONTEXT_DIGEST),
    scheduledAt,
    expiresAt,
    approvalRequired,
    voiceIsAuthority: false,
  };
  const intentDigest = sha256(envelope);
  return Object.freeze({
    ...envelope,
    intentDigest,
    idempotencyKey: `direction:${intentDigest.slice('sha256:'.length, 'sha256:'.length + 32)}`,
    warnings: lint.warnings,
  });
}

export function approvalChallenge(direction, { now = new Date() } = {}) {
  if (!direction || typeof direction !== 'object' || Array.isArray(direction) || direction.schema !== 'DirectionV1') throw new TypeError('DirectionV1 is required');
  const sensitive = scanSensitive(direction);
  if (sensitive.length) throw new TypeError(`${sensitive[0].path} contains forbidden sensitive material (${sensitive[0].rule})`);
  const unknown = Object.keys(direction).filter((key) => !DIRECTION_RECORD_FIELDS.has(key));
  const missing = [...DIRECTION_RECORD_FIELDS].filter((key) => !Object.hasOwn(direction, key));
  if (unknown.length || missing.length) throw new TypeError(`DirectionV1 fields are invalid: ${[...unknown, ...missing].join(', ')}`);
  if (!normalizedIso(direction.expiresAt) || Date.parse(direction.expiresAt) <= now.getTime()) throw new TypeError('DirectionV1 is expired or has invalid expiry');
  if (direction.scheduledAt !== null && (!normalizedIso(direction.scheduledAt) || Date.parse(direction.scheduledAt) <= now.getTime() || Date.parse(direction.scheduledAt) >= Date.parse(direction.expiresAt))) {
    throw new TypeError('DirectionV1 schedule is invalid');
  }
  const registry = loadPlatformRegistry();
  const operationPolicy = registry.platforms[direction.platform]?.operations?.[direction.operation];
  if (!operationPolicy || operationClass(operationPolicy) !== 'external_effect') throw new TypeError('DirectionV1 operation is not an external effect');
  if (Date.parse(`${registry.review_expires_at}T23:59:59.999Z`) < now.getTime()) throw new TypeError('Platform policy review is expired');
  resolveIdentityBinding(direction.platform, direction.account, direction.identityScope);
  const canonicalTarget = cleanTarget(direction.target);
  if (canonicalJson(canonicalTarget) !== canonicalJson(direction.target) || sha256(canonicalTarget) !== direction.targetDigest) throw new TypeError('DirectionV1 target binding is invalid');
  for (const field of ['authorityEvidenceDigest', 'claimsDigest', 'conditionEvidenceDigest', 'contentDigest', 'contextEvidenceDigest', 'directionPolicyDigest', 'platformPolicyDigest', 'targetDigest']) {
    if (!/^sha256:[a-f0-9]{64}$/u.test(direction[field] || '')) throw new TypeError(`DirectionV1 ${field} is invalid`);
  }
  if (direction.directionPolicyDigest !== sha256(POLICY)) throw new TypeError('DirectionV1 direction policy digest is stale');
  if (direction.platformPolicyDigest !== platformOperationPolicyDigest(registry, direction.platform, direction.operation)) throw new TypeError('DirectionV1 platform policy digest is stale');
  if (direction.authorityEvidenceDigest === UNVERIFIED_AUTHORITY_DIGEST) throw new TypeError('DirectionV1 authority evidence is unverified');
  if (direction.contextEvidenceDigest === UNBOUND_CONTEXT_DIGEST) throw new TypeError('DirectionV1 context evidence is unbound');
  if (direction.approvalRequired !== true || direction.voiceIsAuthority !== false) throw new TypeError('DirectionV1 approval boundary is invalid');
  if (!Array.isArray(direction.warnings) || direction.warnings.some((item) => typeof item !== 'string')) throw new TypeError('DirectionV1 warnings are invalid');
  const envelope = Object.fromEntries(ENVELOPE_FIELDS.map((field) => [field, direction[field]]));
  const expectedIntent = sha256(envelope);
  const expectedIdempotency = `direction:${expectedIntent.slice('sha256:'.length, 'sha256:'.length + 32)}`;
  if (direction.intentDigest !== expectedIntent || direction.idempotencyKey !== expectedIdempotency) throw new TypeError('DirectionV1 intent or idempotency binding is invalid');
  return {
    schema: 'ApprovalChallengeV1',
    intentDigest: direction.intentDigest,
    account: direction.account,
    platform: direction.platform,
    operation: direction.operation,
    target: direction.target,
    targetDigest: direction.targetDigest,
    contentDigest: direction.contentDigest,
    directionPolicyDigest: direction.directionPolicyDigest,
    platformPolicyDigest: direction.platformPolicyDigest,
    conditionEvidenceDigest: direction.conditionEvidenceDigest,
    authorityEvidenceDigest: direction.authorityEvidenceDigest,
    contextEvidenceDigest: direction.contextEvidenceDigest,
    expiresAt: direction.expiresAt,
    requiredFactor: 'device_bound_human_action',
    voiceAcceptedAsApproval: false,
    authorizationCreated: false,
    challengeDigest: sha256({ intentDigest: direction.intentDigest, requiredFactor: 'device_bound_human_action' }),
  };
}
