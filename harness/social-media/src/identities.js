// SPDX-License-Identifier: MIT

import { readFileSync } from 'node:fs';

const REGISTRY_URL = new URL('../config/identities.json', import.meta.url);
const SCOPES = new Set(['agentics', 'cognitum', 'ruv_personal', 'ruvnet']);

function normalize(value) {
  return String(value || '').trim().toLocaleLowerCase('en-US');
}

export function loadIdentityRegistry() {
  const registry = JSON.parse(readFileSync(REGISTRY_URL, 'utf8'));
  if (registry.schema !== 'SocialIdentityRegistryV1' || registry.version !== 1) throw new TypeError('Identity registry schema mismatch');
  if (registry.authority !== 'PUBLIC_ATTRIBUTION_FOR_PLANNING_ONLY' || registry.writeAuthorityEstablished !== false) {
    throw new TypeError('Identity registry cannot grant write authority');
  }
  if (!Array.isArray(registry.bindings) || registry.bindings.length === 0) throw new TypeError('Identity registry bindings are required');
  const exact = new Set();
  const globalAccounts = new Map();
  for (const binding of registry.bindings) {
    const platform = normalize(binding.platform);
    const account = normalize(binding.account);
    if (!platform || !account || !SCOPES.has(binding.identityScope)) throw new TypeError('Identity binding is invalid');
    if (typeof binding.source !== 'string' || !binding.source.startsWith('https://')) throw new TypeError('Identity binding source must use https');
    const key = `${platform}:${account}`;
    if (exact.has(key)) throw new TypeError(`Duplicate identity binding: ${key}`);
    exact.add(key);
    const prior = globalAccounts.get(account);
    if (prior && prior !== binding.identityScope) throw new TypeError(`Account ${account} crosses identity scopes`);
    globalAccounts.set(account, binding.identityScope);
  }
  return registry;
}

export function resolveIdentityBinding(platform, account, identityScope, registry = loadIdentityRegistry()) {
  const platformKey = normalize(platform);
  const accountKey = normalize(account);
  const binding = registry.bindings.find((item) => normalize(item.platform) === platformKey && normalize(item.account) === accountKey);
  if (!binding) throw new TypeError(`Account ${accountKey || '<missing>'} is not registered for ${platformKey || '<missing>'}`);
  if (binding.identityScope !== identityScope) throw new TypeError(`Account ${binding.account} is bound to identity scope ${binding.identityScope}`);
  return Object.freeze({ ...binding, account: normalize(binding.account), platform: platformKey, writeAuthorityEstablished: false });
}
