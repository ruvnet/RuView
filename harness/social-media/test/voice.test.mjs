import assert from 'node:assert/strict';
import test from 'node:test';

import { lintContent } from '../src/voice.js';

test('numeric claims require evidence', () => {
  const result = lintContent({ platform: 'linkedin', text: 'The repository has 92001 stars.' });
  assert.equal(result.ok, false);
  assert.match(result.errors.join(' '), /evidence/u);
});

test('measured evidence records are accepted but missing reproducers are warned', () => {
  const result = lintContent({
    platform: 'linkedin',
    text: 'The dated public snapshot showed 62000 followers.',
    claims: [{
      grade: 'MEASURED',
      source_url: 'https://www.linkedin.com/',
      measured_at: '2026-08-29T12:00:00.000Z',
    }],
  });
  assert.equal(result.ok, true);
  assert.match(result.warnings.join(' '), /reproducer/u);
});

test('unsupported superlatives are surfaced for review', () => {
  const result = lintContent({ platform: 'x', text: 'A revolutionary release.' });
  assert.equal(result.ok, true);
  assert.match(result.warnings.join(' '), /revolutionary/u);
});

test('claim payloads cannot smuggle approval or credential material', () => {
  const capability = `github_pat_${'x'.repeat(64)}`;
  const sensitive = lintContent({
    platform: 'linkedin',
    text: 'A dated metric is available.',
    claims: [{ grade: 'CLAIMED', source_url: 'https://example.com', measured_at: '2026-08-29T12:00:00.000Z', reproducer: capability }],
  });
  assert.equal(sensitive.ok, false);
  assert.match(sensitive.errors.join(' '), /forbidden sensitive material/u);

  const authority = lintContent({
    platform: 'linkedin',
    text: 'A source backed statement.',
    claims: [{ grade: 'CLAIMED', source_url: 'https://example.com', measured_at: '2026-08-29T12:00:00.000Z', approved: true }],
  });
  assert.equal(authority.ok, false);
  assert.match(authority.errors.join(' '), /unregistered fields/u);
});
