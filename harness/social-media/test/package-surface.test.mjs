import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import { compareRecordedSurface } from '../src/package-surface.js';
import { scanPackageSurface } from '../src/doctor.js';

test('manifest comparison rejects a newly packaged unmanifested file', () => {
  assert.deepEqual(
    compareRecordedSurface(['src/known.js'], ['src/known.js', 'src/unmanifested.js', '.harness/manifest.json', '.harness/manifest.sha256']),
    ['src/unmanifested.js:unmanifested-packaged-file'],
  );
});

test('package scanner fails closed on secret bearing unknown extensions', () => {
  const root = mkdtempSync(join(tmpdir(), 'ruv-social-scan-'));
  mkdirSync(join(root, 'src'));
  writeFileSync(join(root, 'package.json'), `${JSON.stringify({ files: ['src/'] })}\n`);
  writeFileSync(join(root, 'src', 'safe.js'), 'export const safe = true;\n');
  writeFileSync(join(root, 'src', 'unexpected.pem'), `-----BEGIN ${'PRIVATE'} KEY-----\nnot-real\n`);
  const findings = scanPackageSurface(root);
  assert.deepEqual(findings, [{ path: 'src/unexpected.pem', rule: 'private_key' }]);
});
