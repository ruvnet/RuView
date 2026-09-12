// SPDX-License-Identifier: MIT

import { readFileSync } from 'node:fs';
import { sensitiveStringRule } from './sensitive.js';

const BASELINE_URL = new URL('../research/ruvnet-social-baseline-2026-08-29.json', import.meta.url);
const LABELS = new Set(['CLAIMED', 'MEASURED', 'UNVERIFIED']);
const SCOPES = new Set(['agentics', 'cognitum', 'ruv_personal', 'ruvnet', 'unresolved']);
const EXPECTED_PLATFORMS = new Set(['Discord', 'Facebook', 'GitHub', 'GitHub Gists', 'Instagram', 'LinkedIn', 'Reddit', 'Threads', 'WhatsApp', 'X']);
const DATE_RE = /^\d{4}-\d{2}-\d{2}$/u;
const VERIFIED_MONITORING = new Set(['GitHub', 'GitHub Gists', 'LinkedIn', 'X']);
const HISTORICAL_OBSERVATION = new Set(['Instagram']);
const COMMUNITY_OWNER_MAPPING = new Set(['Discord', 'Reddit', 'WhatsApp']);
const UNRESOLVED_SURFACES = new Set(['Facebook', 'Threads']);

function validDate(value) {
  return typeof value === 'string' && DATE_RE.test(value) && Number.isFinite(Date.parse(`${value}T00:00:00.000Z`));
}

function httpsOrNull(value) {
  return value === null || (typeof value === 'string' && /^https:\/\/[^\s]+$/u.test(value));
}

function exactSet(value, expected) {
  return Array.isArray(value)
    && value.length === expected.size
    && new Set(value).size === value.length
    && value.every((item) => expected.has(item));
}

export function validateResearchBaseline(baseline) {
  const errors = [];
  if (!baseline || typeof baseline !== 'object' || Array.isArray(baseline)) return { ok: false, errors: ['baseline must be an object'] };
  if (baseline.schema !== 'ruvnet.social.baseline.v1') errors.push('schema mismatch');
  if (baseline.research_mode !== 'PUBLIC_UNAUTHENTICATED_READ_ONLY') errors.push('research mode mismatch');
  if (!validDate(baseline.snapshot_date)) errors.push('snapshot date is invalid');
  if (!baseline.claim_labels || [...LABELS].some((label) => typeof baseline.claim_labels[label] !== 'string')) errors.push('claim label vocabulary is incomplete');
  if (!Array.isArray(baseline.inputs) || baseline.inputs.length === 0) errors.push('source inputs are required');
  const inputUrls = new Set();
  for (const [index, input] of (baseline.inputs || []).entries()) {
    if (!input || typeof input !== 'object' || !/^https:\/\/[^\s]+$/u.test(input.url || '')) errors.push(`input ${index} source URL is invalid`);
    else inputUrls.add(input.url);
    if (typeof input?.kind !== 'string' || typeof input?.purpose !== 'string') errors.push(`input ${index} source metadata is incomplete`);
  }
  if (!Array.isArray(baseline.surfaces) || baseline.surfaces.length !== EXPECTED_PLATFORMS.size) errors.push('exactly ten platform surfaces are required');
  const platforms = new Set();
  const accountScopes = new Map();
  for (const [index, surface] of (baseline.surfaces || []).entries()) {
    const prefix = `surface ${index}`;
    if (!EXPECTED_PLATFORMS.has(surface?.platform) || platforms.has(surface.platform)) errors.push(`${prefix} platform is missing, unknown, or duplicated`);
    else platforms.add(surface.platform);
    if (!LABELS.has(surface?.evidence_label)) errors.push(`${prefix} evidence label is invalid`);
    if (!SCOPES.has(surface?.identity_scope)) errors.push(`${prefix} identity scope is invalid`);
    if (surface?.write_authority !== 'NOT_ESTABLISHED') errors.push(`${prefix} write authority must remain NOT_ESTABLISHED`);
    if (!httpsOrNull(surface?.canonical_url)) errors.push(`${prefix} canonical URL is invalid`);
    if (surface?.canonical_url && !inputUrls.has(surface.canonical_url)) errors.push(`${prefix} canonical URL has no source input`);
    if (surface?.account !== null && typeof surface?.account !== 'string') errors.push(`${prefix} account must be string or null`);
    if (typeof surface?.account === 'string') {
      const account = surface.account.trim().toLocaleLowerCase('en-US');
      const prior = accountScopes.get(account);
      if (prior && prior !== surface.identity_scope) errors.push(`${prefix} account crosses identity scopes`);
      accountScopes.set(account, surface.identity_scope);
    }
    if (surface?.measurement_date !== null && !validDate(surface?.measurement_date)) errors.push(`${prefix} measurement date is invalid`);
    if (validDate(surface?.measurement_date) && validDate(baseline.snapshot_date) && surface.measurement_date > baseline.snapshot_date) errors.push(`${prefix} measurement date is after snapshot`);
    if (surface?.public_metrics !== null) {
      if (!['CLAIMED', 'MEASURED'].includes(surface.evidence_label)) errors.push(`${prefix} metrics cannot be UNVERIFIED`);
      if (!validDate(surface.measurement_date) && !validDate(surface.observed_during_research_on)) errors.push(`${prefix} metrics need a date`);
      if (surface.evidence_label === 'CLAIMED' && !/^https:\/\/[^\s]+$/u.test(surface.source_url || '')) errors.push(`${prefix} claimed metrics need a source URL`);
      if (surface.evidence_label === 'MEASURED' && surface.measurement_date === null && typeof surface.source_freshness !== 'string') errors.push(`${prefix} undated measured metrics need a freshness limitation`);
    }
  }
  for (const platform of EXPECTED_PLATFORMS) if (!platforms.has(platform)) errors.push(`missing platform surface: ${platform}`);
  if (!Array.isArray(baseline.adjacent_identities) || baseline.adjacent_identities.some((item) => item.governance !== 'SEPARATE_FROM_PERSONAL_RUV_AND_RUVNET')) {
    errors.push('adjacent identities must remain separately governed');
  }
  if (!exactSet(baseline.decision?.verified_identity_surfaces_for_read_only_monitoring, VERIFIED_MONITORING)) {
    errors.push('verified read only monitoring surfaces are invalid');
  }
  if (!exactSet(baseline.decision?.historical_observation_surfaces, HISTORICAL_OBSERVATION)) {
    errors.push('historical observation surfaces are invalid');
  }
  if (!exactSet(baseline.decision?.community_surfaces_requiring_separate_owner_mapping, COMMUNITY_OWNER_MAPPING)) {
    errors.push('community owner mapping surfaces are invalid');
  }
  if (!exactSet(baseline.decision?.unresolved_surfaces, UNRESOLVED_SURFACES)) errors.push('unresolved surfaces are invalid');
  const monitored = new Set(baseline.decision?.verified_identity_surfaces_for_read_only_monitoring || []);
  const historical = new Set(baseline.decision?.historical_observation_surfaces || []);
  if ([...monitored].some((platform) => historical.has(platform))) errors.push('verified and historical monitoring surfaces overlap');
  const instagram = (baseline.surfaces || []).find(({ platform }) => platform === 'Instagram');
  if (instagram?.evidence_label !== 'UNVERIFIED' || instagram?.identity_status !== 'HISTORICAL_ASSOCIATION_UNVERIFIED_FOR_CURRENT_CONTROL') {
    errors.push('Instagram must remain historical and unverified for current control');
  }
  if (baseline.decision?.write_policy?.includes('No surface has write authority') !== true) errors.push('baseline write policy is missing');
  const sensitiveRule = sensitiveStringRule(JSON.stringify(baseline));
  if (sensitiveRule) errors.push(`baseline contains forbidden capability material (${sensitiveRule})`);
  return { ok: errors.length === 0, errors, surfaces: platforms.size, snapshotDate: baseline.snapshot_date };
}

export function loadResearchBaseline() {
  const baseline = JSON.parse(readFileSync(BASELINE_URL, 'utf8'));
  const validation = validateResearchBaseline(baseline);
  if (!validation.ok) throw new TypeError(`Research baseline invalid: ${validation.errors.join('; ')}`);
  return structuredClone(baseline);
}
