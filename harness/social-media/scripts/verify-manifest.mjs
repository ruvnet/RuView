#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { compareRecordedSurface, packageRelativeNames } from '../src/package-surface.js';

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const quiet = process.argv.includes('--quiet');
const sha = (value) => createHash('sha256').update(value).digest('hex');
const manifestPath = join(ROOT, '.harness', 'manifest.json');
const raw = readFileSync(manifestPath);
const manifest = JSON.parse(raw);
const findings = [];

if (manifest.schema !== 1 || manifest.authority !== 'read-only-zero-credential') findings.push('manifest:contract-mismatch');
findings.push(...compareRecordedSurface(Object.keys(manifest.files || {}), packageRelativeNames(ROOT)));
for (const [name, expected] of Object.entries(manifest.files || {})) {
  const target = join(ROOT, name);
  if (!existsSync(target)) findings.push(`${name}:missing`);
  else if (sha(readFileSync(target)) !== expected) findings.push(`${name}:hash-mismatch`);
}
if (sha(JSON.stringify(manifest.files || {})) !== manifest.filesDigest) findings.push('filesDigest:mismatch');
const expectedOuter = readFileSync(join(ROOT, '.harness', 'manifest.sha256'), 'utf8').trim().split(/\s+/u)[0];
if (sha(raw) !== expectedOuter) findings.push('manifest.sha256:mismatch');
if (!quiet) process.stdout.write(`${JSON.stringify({ ok: findings.length === 0, files: Object.keys(manifest.files || {}).length, findings }, null, 2)}\n`);
process.exitCode = findings.length ? 1 : 0;
