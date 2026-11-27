import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import test from 'node:test';

import { runTool } from '../src/tools.js';

test('source has no outbound client, shell, or child process import', () => {
  const sources = readdirSync(new URL('../src', import.meta.url))
    .filter((name) => name.endsWith('.js'))
    .map((name) => [name, readFileSync(new URL(`../src/${name}`, import.meta.url), 'utf8')]);
  for (const [name, source] of sources) {
    assert.doesNotMatch(source, /from ['"]node:(?:child_process|dns|https|net|tls)['"]/u, name);
    assert.doesNotMatch(source, /\b(?:fetch|WebSocket)\s*\(/u, name);
  }
  const http = sources.find(([name]) => name === 'http-server.js')[1];
  assert.match(http, /createServer/u);
  assert.doesNotMatch(http, /\brequest\s*\(/u);
});

test('representative tools remain offline when fetch is denied', async (context) => {
  const originalFetch = globalThis.fetch;
  let attempts = 0;
  globalThis.fetch = async () => {
    attempts += 1;
    throw new Error('outbound network denied by test');
  };
  context.after(() => {
    globalThis.fetch = originalFetch;
  });
  const calls = [
    ['social_doctor', {}],
    ['social_platforms', {}],
    ['social_research_baseline', {}],
    ['social_direction_policy', {}],
    ['social_action_plan', {
      platform: 'reddit',
      operation: 'automated_read_or_stats_before_reddit_approval',
      requestedRoute: 'api',
    }],
  ];
  for (const [name, args] of calls) await runTool(name, args);
  assert.equal(attempts, 0);
});
