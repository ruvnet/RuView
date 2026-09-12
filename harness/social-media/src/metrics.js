// SPDX-License-Identifier: MIT

import { canonicalJson, sha256 } from './canonical.js';
import { normalizedIso } from './validation.js';
import { scanSensitive } from './sensitive.js';
import { resolveIdentityBinding } from './identities.js';

const DIGEST_RE = /^sha256:[a-f0-9]{64}$/u;
const IDENTITY_SCOPES = new Set(['ruv_personal', 'ruvnet', 'agentics', 'cognitum']);
const EVIDENCE_LABELS = new Set(['MEASURED', 'CLAIMED', 'SYNTHETIC']);
const COLLECTION_MODES = new Set(['PLATFORM_EXPORT', 'PUBLIC_PAGE', 'SYNTHETIC_FIXTURE']);
const QUALITY_FLAGS = new Set(['DELAYED', 'ESTIMATED', 'FILTERED', 'NONE', 'ROUNDED', 'SAMPLED']);
const COUNTERS = new Set([
  'clicks',
  'comments',
  'delivered',
  'engagements',
  'failed',
  'followers',
  'impressions',
  'linkClicks',
  'reach',
  'reactions',
  'replies',
  'saves',
  'sent',
  'shares',
  'views',
]);
const RATE_POLICIES = Object.freeze({
  clickThroughRate: { denominator: 'impressions', numerators: new Set(['clicks', 'linkClicks']) },
  deliveryRate: { denominator: 'sent', numerators: new Set(['delivered']) },
  engagementPerImpression: { denominator: 'impressions', numerators: new Set(['comments', 'engagements', 'reactions', 'replies', 'saves', 'shares']) },
  engagementPerReach: { denominator: 'reach', numerators: new Set(['comments', 'engagements', 'reactions', 'replies', 'saves', 'shares']) },
  failureRate: { denominator: 'sent', numerators: new Set(['failed']) },
  replyRate: { denominator: 'sent', numerators: new Set(['replies']) },
});
export const SUPPORTED_OPTIMIZATION_METRICS = Object.freeze(Object.keys(RATE_POLICIES));
const SNAPSHOT_FIELDS = new Set([
  'account',
  'collectedAt',
  'collectionMode',
  'connectorDefinitionVersion',
  'contentDigest',
  'contentId',
  'counters',
  'definitions',
  'evidenceLabel',
  'identityScope',
  'platform',
  'provenanceDigest',
  'quality',
  'qualityFlags',
  'rates',
  'schema',
  'snapshotDigest',
  'sourceDigest',
  'windowEnd',
  'windowStart',
]);

function requireIso(value, name) {
  if (!normalizedIso(value)) throw new TypeError(`${name} must be a normalized ISO timestamp`);
  return value;
}

function requireString(value, name, maximum = 256) {
  if (typeof value !== 'string' || value.trim().length === 0 || value.length > maximum) {
    throw new TypeError(`${name} must be a non-empty string of at most ${maximum} characters`);
  }
  return value.trim();
}

function requireDigest(value, name) {
  if (!DIGEST_RE.test(value || '')) throw new TypeError(`${name} must be sha256`);
  return value;
}

function requireCounters(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new TypeError('counters must be an object');
  const result = {};
  for (const [name, count] of Object.entries(value)) {
    if (!COUNTERS.has(name)) throw new TypeError(`Unsupported counter: ${name}`);
    if (!Number.isSafeInteger(count) || count < 0) throw new TypeError(`Counter ${name} must be a non-negative integer`);
    result[name] = count;
  }
  if (Object.keys(result).length === 0) throw new TypeError('At least one counter is required');
  return result;
}

function requireDefinitions(value, counters) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new TypeError('definitions must be an object');
  const missing = Object.keys(counters).filter((name) => typeof value[name] !== 'string' || value[name].trim() === '');
  const unknown = Object.keys(value).filter((name) => !Object.hasOwn(counters, name));
  if (missing.length) throw new TypeError(`Counter definitions are missing: ${missing.join(', ')}`);
  if (unknown.length) throw new TypeError(`Definitions reference absent counters: ${unknown.join(', ')}`);
  return Object.fromEntries(Object.entries(value).map(([name, definition]) => [name, requireString(definition, `definitions.${name}`, 512)]));
}

function requireQualityFlags(value) {
  if (!Array.isArray(value) || value.length === 0 || value.length > QUALITY_FLAGS.size) throw new TypeError('qualityFlags must be a non-empty bounded array');
  if (new Set(value).size !== value.length || value.some((flag) => !QUALITY_FLAGS.has(flag))) throw new TypeError('qualityFlags contains duplicate or unknown values');
  if (value.includes('NONE') && value.length !== 1) throw new TypeError('qualityFlags NONE cannot be combined with another flag');
  return [...value].sort();
}

function validateEvidenceMode(collectionMode, evidenceLabel) {
  const valid = (
    (collectionMode === 'SYNTHETIC_FIXTURE' && evidenceLabel === 'SYNTHETIC')
    || (collectionMode === 'PLATFORM_EXPORT' && evidenceLabel === 'MEASURED')
    || (collectionMode === 'PUBLIC_PAGE' && ['CLAIMED', 'MEASURED'].includes(evidenceLabel))
  );
  if (!valid) throw new TypeError(`evidenceLabel ${evidenceLabel} is incompatible with collectionMode ${collectionMode}`);
}

function normalizeRates(value, counters, definitions, platform, collectionMode, connectorDefinitionVersion) {
  if (!Array.isArray(value) || value.length === 0 || value.length > Object.keys(RATE_POLICIES).length) {
    throw new TypeError(`rateDefinitions must contain between one and ${Object.keys(RATE_POLICIES).length} explicit rates`);
  }
  const seen = new Set();
  return Object.fromEntries(value.map((definition, index) => {
    if (!definition || typeof definition !== 'object' || Array.isArray(definition)) throw new TypeError(`rateDefinitions[${index}] must be an object`);
    const unknown = Object.keys(definition).filter((key) => !['denominator', 'name', 'numerators'].includes(key));
    if (unknown.length) throw new TypeError(`rateDefinitions[${index}] contains unknown fields`);
    const name = requireString(definition.name, `rateDefinitions[${index}].name`, 64);
    const policy = RATE_POLICIES[name];
    if (!policy) throw new TypeError(`Unsupported rate definition: ${name}`);
    if (seen.has(name)) throw new TypeError(`Duplicate rate definition: ${name}`);
    seen.add(name);
    if (!Array.isArray(definition.numerators) || definition.numerators.length === 0 || definition.numerators.length > policy.numerators.size) {
      throw new TypeError(`rateDefinitions[${index}].numerators must be a non-empty bounded array`);
    }
    const numerators = [...new Set(definition.numerators)].sort();
    if (numerators.length !== definition.numerators.length || numerators.some((counter) => !policy.numerators.has(counter) || !Object.hasOwn(counters, counter))) {
      throw new TypeError(`Rate ${name} uses a duplicate, ineligible, or absent numerator`);
    }
    if (name === 'clickThroughRate' && numerators.length !== 1) {
      throw new TypeError('clickThroughRate requires exactly one non-overlapping click numerator');
    }
    if (name === 'engagementPerImpression' || name === 'engagementPerReach') {
      if (numerators.includes('engagements') && numerators.length !== 1) {
        throw new TypeError(`${name} cannot combine aggregate engagements with component counters`);
      }
      if (numerators.includes('comments') && numerators.includes('replies')) {
        throw new TypeError(`${name} cannot combine comments with the potentially overlapping replies counter`);
      }
    }
    const denominator = requireString(definition.denominator, `rateDefinitions[${index}].denominator`, 64);
    if (denominator !== policy.denominator || !Object.hasOwn(counters, denominator)) throw new TypeError(`Rate ${name} requires denominator ${policy.denominator}`);
    const numeratorValue = numerators.reduce((sum, counter) => sum + counters[counter], 0);
    const denominatorValue = counters[denominator];
    const semanticRecord = {
      platform,
      collectionMode,
      connectorDefinitionVersion,
      metric: name,
      unit: 'RATE',
      numerators,
      denominator,
      definitions: Object.fromEntries([...numerators, denominator]
        .filter((counter, itemIndex, all) => all.indexOf(counter) === itemIndex)
        .sort()
        .map((counter) => [counter, definitions[counter]])),
    };
    return [name, {
      unit: 'RATE',
      numerators,
      denominator,
      numeratorValue,
      denominatorValue,
      value: denominatorValue === 0 ? null : numeratorValue / denominatorValue,
      semanticsDigest: sha256(semanticRecord),
    }];
  }));
}

export function normalizeSnapshot(input, { now = new Date() } = {}) {
  if (!input || typeof input !== 'object' || Array.isArray(input)) throw new TypeError('Metrics snapshot must be an object');
  const sensitive = scanSensitive(input);
  if (sensitive.length) throw new TypeError(`${sensitive[0].path} contains forbidden sensitive material (${sensitive[0].rule})`);
  const platform = requireString(input.platform, 'platform', 32).toLocaleLowerCase('en-US');
  const account = requireString(input.account, 'account', 128);
  const identityScope = requireString(input.identityScope, 'identityScope', 64);
  if (!IDENTITY_SCOPES.has(identityScope)) throw new TypeError(`Unknown identityScope: ${identityScope}`);
  const identityBinding = resolveIdentityBinding(platform, account, identityScope);
  const collectionMode = requireString(input.collectionMode, 'collectionMode', 32);
  if (!COLLECTION_MODES.has(collectionMode)) throw new TypeError('collectionMode is invalid');
  const connectorDefinitionVersion = requireString(input.connectorDefinitionVersion, 'connectorDefinitionVersion', 128);
  const contentId = requireString(input.contentId, 'contentId', 512);
  const contentDigest = requireDigest(input.contentDigest, 'contentDigest');
  const sourceDigest = requireDigest(input.sourceDigest, 'sourceDigest');
  const provenanceDigest = requireDigest(input.provenanceDigest, 'provenanceDigest');
  if (!EVIDENCE_LABELS.has(input.evidenceLabel)) throw new TypeError('evidenceLabel is invalid');
  validateEvidenceMode(collectionMode, input.evidenceLabel);
  const windowStart = requireIso(input.windowStart, 'windowStart');
  const windowEnd = requireIso(input.windowEnd, 'windowEnd');
  const collectedAt = requireIso(input.collectedAt, 'collectedAt');
  if (Date.parse(windowEnd) <= Date.parse(windowStart)) throw new TypeError('windowEnd must follow windowStart');
  if (Date.parse(collectedAt) < Date.parse(windowEnd)) throw new TypeError('collectedAt must not precede windowEnd');
  if (Date.parse(windowEnd) > now.getTime() || Date.parse(collectedAt) > now.getTime()) throw new TypeError('metric evidence cannot be collected in the future');
  const qualityFlags = requireQualityFlags(input.qualityFlags);
  const counters = requireCounters(input.counters);
  const definitions = requireDefinitions(input.definitions, counters);
  const rates = normalizeRates(input.rateDefinitions, counters, definitions, platform, collectionMode, connectorDefinitionVersion);
  const record = {
    schema: 'NormalizedMetricsV1',
    platform,
    account: identityBinding.account,
    identityScope,
    collectionMode,
    connectorDefinitionVersion,
    contentId,
    contentDigest,
    sourceDigest,
    provenanceDigest,
    evidenceLabel: input.evidenceLabel,
    windowStart,
    windowEnd,
    collectedAt,
    qualityFlags,
    counters,
    definitions,
    rates,
    quality: {
      comparableWithinPlatformAndIdentityOnly: true,
      crossPlatformRankingAllowed: false,
      followersAreUsers: false,
      definitionsComplete: true,
      causalClaimAllowed: false,
    },
  };
  return { ...record, snapshotDigest: sha256(record) };
}

export function validateNormalizedSnapshot(value, { now = new Date() } = {}) {
  if (!value || typeof value !== 'object' || Array.isArray(value) || value.schema !== 'NormalizedMetricsV1') throw new TypeError('NormalizedMetricsV1 record is required');
  const unknown = Object.keys(value).filter((key) => !SNAPSHOT_FIELDS.has(key));
  const missing = [...SNAPSHOT_FIELDS].filter((key) => !Object.hasOwn(value, key));
  if (unknown.length || missing.length) throw new TypeError(`NormalizedMetricsV1 fields are invalid: ${[...unknown, ...missing].join(', ')}`);
  if (!value.rates || typeof value.rates !== 'object' || Array.isArray(value.rates)) throw new TypeError('NormalizedMetricsV1 rates are invalid');
  const regenerated = normalizeSnapshot({
    platform: value.platform,
    account: value.account,
    identityScope: value.identityScope,
    collectionMode: value.collectionMode,
    connectorDefinitionVersion: value.connectorDefinitionVersion,
    contentId: value.contentId,
    contentDigest: value.contentDigest,
    sourceDigest: value.sourceDigest,
    provenanceDigest: value.provenanceDigest,
    evidenceLabel: value.evidenceLabel,
    windowStart: value.windowStart,
    windowEnd: value.windowEnd,
    collectedAt: value.collectedAt,
    qualityFlags: value.qualityFlags,
    counters: value.counters,
    definitions: value.definitions,
    rateDefinitions: Object.entries(value.rates).map(([name, rate]) => ({ name, numerators: rate.numerators, denominator: rate.denominator })),
  }, { now });
  if (canonicalJson(regenerated) !== canonicalJson(value)) throw new TypeError('NormalizedMetricsV1 digest or derived values do not verify');
  return regenerated;
}

export function compareSnapshots(left, right, metric, { now = new Date() } = {}) {
  const verifiedLeft = validateNormalizedSnapshot(left, { now });
  const verifiedRight = validateNormalizedSnapshot(right, { now });
  if (verifiedLeft.platform !== verifiedRight.platform || verifiedLeft.account !== verifiedRight.account || verifiedLeft.identityScope !== verifiedRight.identityScope) {
    throw new TypeError('Cross-platform, cross-account, or cross-identity comparisons are prohibited');
  }
  if (verifiedLeft.collectionMode !== verifiedRight.collectionMode || verifiedLeft.evidenceLabel !== verifiedRight.evidenceLabel || verifiedLeft.connectorDefinitionVersion !== verifiedRight.connectorDefinitionVersion) {
    throw new TypeError('Collection mode, evidence label, and connector definition version must match');
  }
  if (canonicalJson(verifiedLeft.qualityFlags) !== canonicalJson(verifiedRight.qualityFlags)) throw new TypeError('Metric quality flags must match');
  if ((Date.parse(verifiedLeft.windowEnd) - Date.parse(verifiedLeft.windowStart)) !== (Date.parse(verifiedRight.windowEnd) - Date.parse(verifiedRight.windowStart))) {
    throw new TypeError('Attribution window duration must match');
  }
  const leftRate = verifiedLeft.rates[metric];
  const rightRate = verifiedRight.rates[metric];
  if (Boolean(leftRate) !== Boolean(rightRate)) throw new TypeError(`Metric ${metric} changes type between snapshots`);
  if (leftRate && leftRate.semanticsDigest !== rightRate.semanticsDigest) throw new TypeError(`Metric ${metric} denominator semantics do not match`);
  if (!leftRate && verifiedLeft.definitions[metric] !== verifiedRight.definitions[metric]) throw new TypeError(`Metric ${metric} definitions do not match`);
  const before = leftRate?.value ?? verifiedLeft.counters[metric];
  const after = rightRate?.value ?? verifiedRight.counters[metric];
  if (!Number.isFinite(before) || !Number.isFinite(after)) throw new TypeError(`Metric ${metric} is unavailable`);
  return {
    metric,
    before,
    after,
    absoluteChange: after - before,
    relativeChange: before === 0 ? null : (after - before) / Math.abs(before),
    causalClaimAllowed: false,
  };
}
