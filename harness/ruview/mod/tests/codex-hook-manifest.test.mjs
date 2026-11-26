import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// Codex's native hook loader accepts only top-level `description` and `hooks`.
// The Claude Code function-hook manifest declares `modules` and would otherwise
// fail with `unknown field 'modules'`. Ship a host-specific no-op via
// `.codex-plugin/plugin.json` (same convention as ruvnet/ruflo#4050) and leave
// the Claude hooks.json and register.mjs untouched.

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

test('ruview-live: Codex gets a host-specific no-op manifest instead of the Claude function-hook one', () => {
  const claudeManifest = JSON.parse(fs.readFileSync(path.join(root, '.claude-plugin/plugin.json'), 'utf8'));
  const codexManifestPath = path.join(root, '.codex-plugin/plugin.json');
  assert.ok(fs.existsSync(codexManifestPath));
  const codexManifest = JSON.parse(fs.readFileSync(codexManifestPath, 'utf8'));
  assert.equal(codexManifest.name, claudeManifest.name);
  assert.equal(codexManifest.version, claudeManifest.version);
  assert.equal(codexManifest.hooks, './hooks/codex-hooks.json');

  const codexHooks = JSON.parse(fs.readFileSync(path.join(root, 'hooks/codex-hooks.json'), 'utf8'));
  const unknown = Object.keys(codexHooks).filter((key) => !['description', 'hooks'].includes(key));
  assert.deepEqual(unknown, []);
  assert.equal(typeof codexHooks.description, 'string');
  assert.deepEqual(codexHooks.hooks, {});

  const claudeHooks = JSON.parse(fs.readFileSync(path.join(root, 'hooks/hooks.json'), 'utf8'));
  assert.deepEqual(claudeHooks.modules, ['./register.mjs']);
  assert.ok(fs.existsSync(path.join(root, 'hooks/register.mjs')));
});
