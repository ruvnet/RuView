#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { packageRelativeNames } from '../src/package-surface.js';

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const sha = (value) => createHash('sha256').update(value).digest('hex');
const SELF_FILES = new Set(['.harness/manifest.json', '.harness/manifest.sha256']);
const files = packageRelativeNames(ROOT).filter((name) => !SELF_FILES.has(name));
const hashes = Object.fromEntries(files.map((name) => [name, sha(readFileSync(join(ROOT, name)))]));
const pkg = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8'));
const manifest = {
  schema: 1,
  generator: 'RuV social metaharness provenance v1',
  name: pkg.name,
  version: pkg.version,
  authority: 'read-only-zero-credential',
  files: hashes,
  filesDigest: sha(JSON.stringify(hashes)),
  meta: {
    surface: 'cli+mcp+http-policy',
    decisions: 'ADR-345..ADR-352',
  },
};
const json = `${JSON.stringify(manifest, null, 2)}\n`;
writeFileSync(join(ROOT, '.harness', 'manifest.json'), json);
writeFileSync(join(ROOT, '.harness', 'manifest.sha256'), `${sha(json)}  manifest.json\n`);
if (!process.argv.includes('--quiet')) process.stdout.write(`${JSON.stringify({ ok: true, files: files.length, digest: sha(json) })}\n`);
