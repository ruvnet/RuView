import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import { validateResearchBaseline } from '../src/research.js';

const source = JSON.parse(readFileSync(new URL('../research/ruvnet-social-baseline-2026-08-29.json', import.meta.url), 'utf8'));
const clone = () => structuredClone(source);

test('research baseline has traceable sources, labels, authority, freshness, and identity separation', () => {
  const result = validateResearchBaseline(clone());
  assert.equal(result.ok, true, result.errors.join('\n'));
  assert.equal(result.surfaces, 10);
});

test('research baseline rejects missing sources and invalid evidence states', () => {
  const missingSource = clone();
  missingSource.surfaces.find(({ platform }) => platform === 'Discord').source_url = null;
  assert.match(validateResearchBaseline(missingSource).errors.join(' '), /source URL/u);

  const invalid = clone();
  invalid.surfaces[0].evidence_label = 'TRUST_ME';
  assert.match(validateResearchBaseline(invalid).errors.join(' '), /evidence label/u);
});

test('research baseline rejects write authority, stale ambiguity, and cross scope reuse', () => {
  const authority = clone();
  authority.surfaces[0].write_authority = 'GRANTED';
  assert.match(validateResearchBaseline(authority).errors.join(' '), /NOT_ESTABLISHED/u);

  const stale = clone();
  const x = stale.surfaces.find(({ platform }) => platform === 'X');
  delete x.source_freshness;
  assert.match(validateResearchBaseline(stale).errors.join(' '), /freshness limitation/u);

  const crossed = clone();
  const linkedIn = crossed.surfaces.find(({ platform }) => platform === 'LinkedIn');
  linkedIn.account = 'ruvnet';
  assert.match(validateResearchBaseline(crossed).errors.join(' '), /crosses identity scopes/u);
});

test('historical Instagram evidence cannot be promoted into the verified monitoring set', () => {
  const promoted = clone();
  promoted.decision.verified_identity_surfaces_for_read_only_monitoring.push('Instagram');
  promoted.decision.historical_observation_surfaces = [];
  assert.match(validateResearchBaseline(promoted).errors.join(' '), /verified read only|historical observation/u);

  const relabeled = clone();
  const instagram = relabeled.surfaces.find(({ platform }) => platform === 'Instagram');
  instagram.evidence_label = 'MEASURED';
  instagram.identity_status = 'STRONGLY_ATTRIBUTABLE';
  assert.match(validateResearchBaseline(relabeled).errors.join(' '), /historical and unverified/u);
});
