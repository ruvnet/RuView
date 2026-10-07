import assert from 'node:assert/strict';
import test from 'node:test';

import { compareSnapshots, normalizeSnapshot, validateNormalizedSnapshot } from '../src/metrics.js';

const digest = (character) => `sha256:${character.repeat(64)}`;
const provenanceDigest = digest('c');
const trustedNow = new Date('2026-08-29T23:00:00.000Z');

function input(overrides = {}) {
  return {
    platform: 'linkedin',
    account: 'reuvencohen',
    identityScope: 'ruv_personal',
    collectionMode: 'PLATFORM_EXPORT',
    connectorDefinitionVersion: 'linkedin-export-v1',
    contentId: 'post:public:123',
    contentDigest: digest('a'),
    sourceDigest: digest('b'),
    provenanceDigest,
    evidenceLabel: 'MEASURED',
    windowStart: '2026-08-01T00:00:00.000Z',
    windowEnd: '2026-08-08T00:00:00.000Z',
    collectedAt: '2026-08-09T00:00:00.000Z',
    qualityFlags: ['NONE'],
    counters: { impressions: 1000, reactions: 40, comments: 10, followers: 62000 },
    definitions: {
      impressions: 'Platform reported eligible content impressions',
      reactions: 'Platform reported reactions',
      comments: 'Platform reported comments',
      followers: 'Rounded public account follower relationship counter',
    },
    rateDefinitions: [{
      name: 'engagementPerImpression',
      numerators: ['reactions', 'comments'],
      denominator: 'impressions',
    }],
    ...overrides,
  };
}

function snapshot(overrides = {}) {
  return normalizeSnapshot(input(overrides), { now: trustedNow });
}

test('normalization binds identity, source, content, provenance, quality, numerator, and denominator', () => {
  const result = snapshot();
  assert.equal(result.rates.engagementPerImpression.value, 0.05);
  assert.equal(result.rates.engagementPerImpression.numeratorValue, 50);
  assert.equal(result.rates.engagementPerImpression.denominatorValue, 1000);
  assert.equal(result.rates.engagementPerImpression.unit, 'RATE');
  assert.equal(result.identityScope, 'ruv_personal');
  assert.equal(result.provenanceDigest, provenanceDigest);
  assert.equal(result.contentDigest, digest('a'));
  assert.equal(result.sourceDigest, digest('b'));
  assert.deepEqual(result.qualityFlags, ['NONE']);
  assert.equal(result.quality.crossPlatformRankingAllowed, false);
  assert.equal(result.quality.followersAreUsers, false);
  assert.equal(validateNormalizedSnapshot(result, { now: trustedNow }).snapshotDigest, result.snapshotDigest);
});

test('cross platform and cross identity comparisons are rejected', () => {
  const x = snapshot({ platform: 'x', account: 'ruv', connectorDefinitionVersion: 'x-export-v1' });
  assert.throws(() => compareSnapshots(snapshot(), x, 'engagementPerImpression', { now: trustedNow }), /Cross-platform/u);
  assert.throws(() => snapshot({ identityScope: 'ruvnet' }), /bound to identity scope/u);
});

test('missing definitions and unknown counters fail closed', () => {
  assert.throws(() => snapshot({
    counters: { impressions: 10, reactions: 2 },
    definitions: { impressions: 'Platform impressions' },
    rateDefinitions: [{ name: 'engagementPerImpression', numerators: ['reactions'], denominator: 'impressions' }],
  }), /definitions are missing/u);
  assert.throws(() => snapshot({
    counters: { inferred_unique_users: 10 },
    definitions: { inferred_unique_users: 'Invalid inference' },
    rateDefinitions: [{ name: 'engagementPerImpression', numerators: ['inferred_unique_users'], denominator: 'inferred_unique_users' }],
  }), /Unsupported counter/u);
});

test('metric timestamps require normalized, nonfuture UTC evidence', () => {
  assert.throws(() => snapshot({ windowStart: 'August 1, 2026 00:00:00' }), /normalized ISO/u);
  assert.throws(() => normalizeSnapshot(input({
    windowStart: '2098-01-01T00:00:00.000Z',
    windowEnd: '2098-01-02T00:00:00.000Z',
    collectedAt: '2098-01-03T00:00:00.000Z',
  }), { now: trustedNow }), /future/u);
});

test('collection mode cannot launder the evidence label', () => {
  assert.throws(() => snapshot({ collectionMode: 'SYNTHETIC_FIXTURE', evidenceLabel: 'MEASURED' }), /incompatible/u);
  const synthetic = snapshot({ collectionMode: 'SYNTHETIC_FIXTURE', evidenceLabel: 'SYNTHETIC' });
  assert.equal(synthetic.evidenceLabel, 'SYNTHETIC');
});

test('engagement rate cannot substitute followers for eligible impressions', () => {
  assert.throws(() => snapshot({
    counters: { followers: 1000, reactions: 40, comments: 10 },
    definitions: {
      followers: 'Platform follower relationships',
      reactions: 'Platform reactions',
      comments: 'Platform comments',
    },
    rateDefinitions: [{ name: 'engagementPerImpression', numerators: ['reactions', 'comments'], denominator: 'followers' }],
  }), /requires denominator impressions/u);
});

test('aggregate and component numerators cannot be double counted', () => {
  assert.throws(() => snapshot({
    counters: { impressions: 100, engagements: 10, reactions: 8, comments: 2 },
    definitions: {
      impressions: 'Platform impressions',
      engagements: 'Platform aggregate engagements including reactions and comments',
      reactions: 'Platform reactions',
      comments: 'Platform comments',
    },
    rateDefinitions: [{ name: 'engagementPerImpression', numerators: ['engagements', 'reactions', 'comments'], denominator: 'impressions' }],
  }), /cannot combine aggregate engagements/u);
  assert.throws(() => snapshot({
    counters: { impressions: 100, clicks: 10, linkClicks: 7 },
    definitions: {
      impressions: 'Platform impressions',
      clicks: 'All platform clicks including link clicks',
      linkClicks: 'Platform link clicks',
    },
    rateDefinitions: [{ name: 'clickThroughRate', numerators: ['clicks', 'linkClicks'], denominator: 'impressions' }],
  }), /exactly one non-overlapping click numerator/u);
  assert.throws(() => snapshot({
    counters: { impressions: 100, comments: 10, replies: 4 },
    definitions: {
      impressions: 'Platform impressions',
      comments: 'Platform comments including replies',
      replies: 'Platform replies',
    },
    rateDefinitions: [{ name: 'engagementPerImpression', numerators: ['comments', 'replies'], denominator: 'impressions' }],
  }), /potentially overlapping replies/u);
});

test('same named rates with changed numerator semantics cannot be compared', () => {
  const left = snapshot();
  const right = snapshot({
    contentId: 'post:public:456',
    contentDigest: digest('d'),
    windowStart: '2026-08-10T00:00:00.000Z',
    windowEnd: '2026-08-17T00:00:00.000Z',
    collectedAt: '2026-08-18T00:00:00.000Z',
    rateDefinitions: [{ name: 'engagementPerImpression', numerators: ['reactions'], denominator: 'impressions' }],
  });
  assert.throws(() => compareSnapshots(left, right, 'engagementPerImpression', { now: trustedNow }), /denominator semantics/u);
});

test('tampered normalized values and stale snapshot digests are rejected', () => {
  const left = snapshot();
  const right = structuredClone(snapshot({
    contentId: 'post:public:456',
    contentDigest: digest('d'),
    windowStart: '2026-08-10T00:00:00.000Z',
    windowEnd: '2026-08-17T00:00:00.000Z',
    collectedAt: '2026-08-18T00:00:00.000Z',
  }));
  right.rates.engagementPerImpression.value = 999;
  assert.throws(() => compareSnapshots(left, right, 'engagementPerImpression', { now: trustedNow }), /derived values do not verify/u);
});

test('direct metric API rejects credential material without echoing it', () => {
  const capability = `github_pat_${'p'.repeat(64)}`;
  assert.throws(() => normalizeSnapshot({
    platform: 'github',
    account: capability,
    identityScope: 'ruvnet',
    collectionMode: 'SYNTHETIC_FIXTURE',
  }), (error) => {
    assert.match(error.message, /forbidden sensitive material/u);
    assert.doesNotMatch(error.message, new RegExp(capability, 'u'));
    return true;
  });
});
