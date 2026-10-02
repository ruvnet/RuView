// SPDX-License-Identifier: MIT
import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, chmodSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { makeProposal, loadBrain, verifyBrain } from '../src/brain.js';

const digest = createHash('sha256').update('Cited source line.').digest('hex');
const record = (id, path, line = 1) => ({ id, title: 'Citation fixture', content: 'Bounded repository evidence.', evidence: 'REPOSITORY', tags: ['citation'], reviewed: true, source: { path, line, endLine: line, digest } });

test('proposals reject empty or directory-root-only source paths', () => {
  for (const sourcePath of ['', '   ', '.', './', '.\\']) {
    const result = makeProposal({ id: 'candidate-source', title: 'Candidate', content: 'Bounded evidence.', sourcePath, sourceLine: 1 });
    assert.equal(result.ok, false, `reject ${JSON.stringify(sourcePath)}`);
    assert.ok(result.errors.some(error => error.includes('source.path')));
  }
  assert.equal(makeProposal({ id: 'candidate-source', title: 'Candidate', content: 'Bounded evidence.', sourcePath: 'README.md', sourceLine: 1 }).ok, true);
});

test('verification reports a directory citation and continues checking other records', () => {
  const root = mkdtempSync(join(tmpdir(), 'brain-file-citations-'));
  try {
    mkdirSync(join(root, 'citations'));
    writeFileSync(join(root, 'source.md'), 'Cited source line.');
    const corpus = join(root, 'corpus.jsonl');
    writeFileSync(corpus, [record('directory-source', 'citations'), record('valid-source', 'source.md'), record('missing-line', 'source.md', 9)].map(value => JSON.stringify(value)).join('\n') + '\n');
    assert.equal(loadBrain(corpus).records.length, 3, 'fixture satisfies canonical record requirements');
    const result = verifyBrain({ repo: root, path: corpus });
    assert.equal(result.ok, false);
    assert.equal(result.records, 3);
    assert.deepEqual(result.findings.map(value => [value.id, value.reason]), [['directory-source', 'source_not_file'], ['missing-line', 'source_line_missing']]);
  } finally { rmSync(root, { recursive: true, force: true }); }
});


test('verification reports an unreadable real file without losing later findings', { skip: process.platform === 'win32' || process.getuid?.() === 0 }, () => {
  const root = mkdtempSync(join(tmpdir(), 'brain-unreadable-citation-'));
  const source = join(root, 'source.md');
  try {
    writeFileSync(source, 'Cited source line.');
    writeFileSync(join(root, 'valid.md'), 'Cited source line.');
    const corpus = join(root, 'corpus.jsonl');
    writeFileSync(corpus, [record('unreadable-source', 'source.md'), record('valid-source', 'valid.md'), record('missing-line', 'valid.md', 9)].map(value => JSON.stringify(value)).join('\n') + '\n');
    chmodSync(source, 0o000);
    const result = verifyBrain({ repo: root, path: corpus });
    assert.equal(result.ok, false);
    assert.deepEqual(result.findings.map(value => [value.id, value.reason]), [['unreadable-source', 'source_unreadable'], ['missing-line', 'source_line_missing']]);
    assert.equal(result.findings[0].code, 'EACCES');
  } finally { chmodSync(source, 0o600); rmSync(root, { recursive: true, force: true }); }
});
