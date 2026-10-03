// SPDX-License-Identifier: MIT

import { readFileSync } from 'node:fs';
import { sha256 } from './canonical.js';

export const ROUTES = Object.freeze(['API_ALLOWED', 'ATTENDED_MANUAL', 'DENY']);
const REQUIRED_PLATFORMS = Object.freeze([
  'discord',
  'facebook',
  'gist',
  'github',
  'instagram',
  'linkedin',
  'reddit',
  'threads',
  'whatsapp',
  'x',
]);
const PLATFORM_CONSTRAINTS = new Set([
  'APPROVED_API_ONLY',
  'APPROVED_OAUTH_ONLY',
  'BOT_ONLY',
  'BUSINESS_PLATFORM_ONLY',
  'DYNAMIC_AI_REPLY_REQUIRES_WRITTEN_APPROVAL',
  'EXPLICIT_REDDIT_APPROVAL_REQUIRED',
  'GIST_SCOPE_REQUIRED',
  'GITHUB_APP_PREFERRED',
  'NO_BROWSER_AUTOMATION',
  'NO_BROWSER_FALLBACK',
  'NO_SCRAPING',
  'NO_SELF_BOT',
  'PAGE_ONLY',
  'PROFESSIONAL_ACCOUNT_ONLY',
]);

export function conditionId(platform, operation, index, description) {
  return `condition:${sha256({ platform, operation, index, description }).slice(7, 23)}`;
}

export function operationClass(policy) {
  if (policy.route === 'DENY') return 'deny';
  if (policy.route === 'ATTENDED_MANUAL') return 'setup';
  return policy.approval_required ? 'external_effect' : 'read';
}

export function platformOperationPolicyDigest(registry, platform, operation) {
  const platformPolicy = registry?.platforms?.[platform];
  const operationPolicy = platformPolicy?.operations?.[operation];
  if (!platformPolicy || !operationPolicy) throw new TypeError('Registered platform operation is required');
  return sha256({
    registrySchema: registry.schema,
    registryVersion: registry.version,
    reviewedAt: registry.reviewed_at,
    reviewExpiresAt: registry.review_expires_at,
    platform,
    platformConstraints: platformPolicy.platform_constraints,
    operation,
    operationPolicy,
  });
}

export function validatePlatformRegistry(registry) {
  const errors = [];
  if (!registry || typeof registry !== 'object' || Array.isArray(registry)) return ['registry must be an object'];
  if (registry.schema !== 'SocialPlatformRegistryV1') errors.push('schema must be SocialPlatformRegistryV1');
  if (!Number.isSafeInteger(registry.version) || registry.version < 1) errors.push('version must be a positive integer');
  if (!/^\d{4}-\d{2}-\d{2}$/u.test(registry.reviewed_at || '')) errors.push('reviewed_at must be YYYY-MM-DD');
  if (!/^\d{4}-\d{2}-\d{2}$/u.test(registry.review_expires_at || '')) errors.push('review_expires_at must be YYYY-MM-DD');
  if (registry.evidence_mode !== 'OFFICIAL_SOURCE_URLS_WITHOUT_ARCHIVE_DIGEST') errors.push('evidence_mode is invalid');
  if (!registry.platforms || typeof registry.platforms !== 'object' || Array.isArray(registry.platforms)) {
    return errors.concat('platforms must be an object');
  }
  for (const platform of REQUIRED_PLATFORMS) {
    const item = registry.platforms[platform];
    if (!item) {
      errors.push(`platform ${platform} is missing`);
      continue;
    }
    if (typeof item.identity_status !== 'string') errors.push(`${platform}.identity_status is required`);
    if (!Array.isArray(item.platform_constraints) || item.platform_constraints.length === 0) {
      errors.push(`${platform}.platform_constraints must be a non-empty array`);
    } else {
      for (const constraint of item.platform_constraints) {
        if (!PLATFORM_CONSTRAINTS.has(constraint)) errors.push(`${platform}.platform_constraints contains unknown value ${constraint}`);
      }
    }
    if (!Array.isArray(item.prerequisites)) errors.push(`${platform}.prerequisites must be an array`);
    if (!Array.isArray(item.official_sources) || item.official_sources.some((url) => typeof url !== 'string' || !url.startsWith('https://'))) {
      errors.push(`${platform}.official_sources must contain https URLs`);
    }
    if (!item.operations || typeof item.operations !== 'object' || Array.isArray(item.operations)) {
      errors.push(`${platform}.operations must be an object`);
      continue;
    }
    for (const [operation, policy] of Object.entries(item.operations)) {
      if (!policy || typeof policy !== 'object' || Array.isArray(policy)) {
        errors.push(`${platform}.${operation} must be an object`);
        continue;
      }
      if (!ROUTES.includes(policy.route)) errors.push(`${platform}.${operation}.route is invalid`);
      if (typeof policy.approval_required !== 'boolean') errors.push(`${platform}.${operation}.approval_required must be boolean`);
      if (!Array.isArray(policy.conditions)) errors.push(`${platform}.${operation}.conditions must be an array`);
      if (typeof policy.policy_source !== 'string' || !policy.policy_source.startsWith('https://')) {
        errors.push(`${platform}.${operation}.policy_source must use https`);
      }
    }
  }
  return errors;
}

export function loadPlatformRegistry() {
  const registry = JSON.parse(readFileSync(new URL('../config/platforms.json', import.meta.url), 'utf8'));
  const errors = validatePlatformRegistry(registry);
  if (errors.length) throw new TypeError(`Invalid platform registry: ${errors.join('; ')}`);
  return registry;
}

export function listPlatformCapabilities() {
  const registry = loadPlatformRegistry();
  return Object.entries(registry.platforms).map(([platform, item]) => ({
    platform,
    policyReviewedAt: registry.reviewed_at,
    policyReviewExpiresAt: registry.review_expires_at,
    evidenceMode: registry.evidence_mode,
    identityStatus: item.identity_status,
    platformConstraints: item.platform_constraints,
    prerequisites: item.prerequisites,
    operations: Object.fromEntries(Object.entries(item.operations).map(([operation, policy]) => [operation, {
      route: policy.route,
      operationClass: operationClass(policy),
      approvalRequired: policy.approval_required,
      conditions: policy.conditions.map((description, index) => ({
        id: conditionId(platform, operation, index, description),
        description,
      })),
      policySource: policy.policy_source,
    }])),
    liveLimitsAreAuthority: item.live_limits_are_authority !== false,
  }));
}
