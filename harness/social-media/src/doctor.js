// SPDX-License-Identifier: MIT

import { readFileSync, statSync } from 'node:fs';
import { relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { getDirectionPolicy } from './voice.js';
import { loadPlatformRegistry } from './platforms.js';
import { sensitiveStringRule } from './sensitive.js';
import { listPackageSurface } from './package-surface.js';
import { loadIdentityRegistry } from './identities.js';
import { loadResearchBaseline } from './research.js';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const MAX_SCAN_FILE_BYTES = 1024 * 1024;
const UTF8 = new TextDecoder('utf-8', { fatal: true });

export function scanPackageSurface(root = ROOT) {
  const findings = [];
  for (const path of listPackageSurface(root)) {
    const name = relative(root, path).replaceAll('\\', '/');
    if (statSync(path).size > MAX_SCAN_FILE_BYTES) {
      findings.push({ path: name, rule: 'unscannable_file_too_large' });
      continue;
    }
    const bytes = readFileSync(path);
    if (bytes.includes(0)) {
      findings.push({ path: name, rule: 'binary_file_prohibited' });
      continue;
    }
    let contents;
    try {
      contents = UTF8.decode(bytes);
    } catch {
      findings.push({ path: name, rule: 'invalid_utf8_prohibited' });
      continue;
    }
    const rule = sensitiveStringRule(contents);
    if (rule) findings.push({ path: name, rule });
  }
  return findings;
}

export function runDoctor() {
  const checks = [];
  const add = (name, ok, detail) => checks.push({ name, ok, detail });
  try {
    const registry = loadPlatformRegistry();
    add('platform_registry', true, `${Object.keys(registry.platforms).length} platforms validated`);
    const expires = Date.parse(`${registry.review_expires_at}T23:59:59.999Z`);
    add('platform_policy_freshness', Number.isFinite(expires) && expires >= Date.now(), `review expires ${registry.review_expires_at}; URL evidence has no archive digest`);
  } catch (cause) {
    add('platform_registry', false, cause instanceof Error ? cause.message : String(cause));
  }
  try {
    const identities = loadIdentityRegistry();
    add('identity_registry', identities.writeAuthorityEstablished === false, `${identities.bindings.length} isolated planning bindings; zero write authority`);
  } catch (cause) {
    add('identity_registry', false, cause instanceof Error ? cause.message : String(cause));
  }
  try {
    const policy = getDirectionPolicy();
    add('direction_policy', policy.schema === 'SocialDirectionPolicyV1', `version ${policy.version}`);
  } catch (cause) {
    add('direction_policy', false, cause instanceof Error ? cause.message : String(cause));
  }
  try {
    const baseline = loadResearchBaseline();
    add('research_baseline', true, `${baseline.snapshot_date}; ${baseline.surfaces.length} validated surfaces; zero write authority`);
  } catch (cause) {
    add('research_baseline', false, cause instanceof Error ? cause.message : String(cause));
  }
  try {
    const pkg = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
    add('zero_runtime_dependencies', !pkg.dependencies || Object.keys(pkg.dependencies).length === 0, 'runtime dependencies must be empty');
  } catch (cause) {
    add('zero_runtime_dependencies', false, cause instanceof Error ? cause.message : String(cause));
  }
  try {
    const policy = JSON.parse(readFileSync(new URL('../.harness/mcp-policy.json', import.meta.url), 'utf8'));
    const forbidden = policy.readOnlyTools.filter((name) => /(?:connect|send|publish|reply|react|moderate|delete|spend|deploy|approve|promote)/u.test(name));
    const ok = policy.defaultDeny === true
      && policy.requireApprovalForDangerous === true
      && policy.readOnlyTools.length === 10
      && policy.cliOnlyTools.length === 0
      && forbidden.length === 0;
    add('mcp_authority', ok, ok ? 'ten read only tools and no execution tool' : 'MCP policy grants or names unsafe capability');
  } catch (cause) {
    add('mcp_authority', false, cause instanceof Error ? cause.message : String(cause));
  }
  const findings = scanPackageSurface();
  add('secret_and_invite_scan', findings.length === 0, findings.length ? `candidate material in ${findings.map(({ path, rule }) => `${path}:${rule}`).join(', ')}` : 'no credential value or capability link patterns found');
  add('node_version', Number(process.versions.node.split('.')[0]) >= 20, process.versions.node);
  return {
    ok: checks.every((check) => check.ok),
    schema: 'SocialDoctorV1',
    networkAttempted: false,
    credentialStoresRead: false,
    checks,
  };
}
