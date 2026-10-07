// SPDX-License-Identifier: MIT
// Cognitum Spaces adapter for the dependency-free RuView metaharness.
//
// OAuth stays in the Rust `wifi-densepose` CLI. This adapter never accepts a
// bearer token or API key, strips the compatibility API-key environment from
// the child, and validates the already-validated semantic projection again
// before returning it to a CLI or MCP caller.

import { DEFAULT_ENV_ALLOWLIST, runProcess } from './process-runner.js';
import { redact } from './redact.js';

const DEFAULT_BASE_URL = 'https://api.cognitum.one';
const MAX_CLI_JSON_BYTES = 2 * 1024 * 1024;
const MAX_JSON_DEPTH = 16;
const MAX_JSON_NODES = 10_000;
const MAX_ARRAY_ITEMS = 1000;
const MAX_OBJECT_KEYS = 128;
const MAX_STRING_BYTES = 4096;
const MAX_RESOURCES = 100;
const ID_RE = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,119}$/;
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
export const SPATIAL_RESOURCE_KINDS = Object.freeze([
  'sites', 'buildings', 'floors', 'spaces', 'zones', 'entities', 'events', 'alerts',
]);
const REQUIRED_EXCLUSIONS = Object.freeze([
  'raw_csi',
  'cir',
  'rf_tensors',
  'recordings',
  'pose_frames',
  'vital_waveforms',
  'identity_observations',
]);
const FORBIDDEN_FIELDS = new Set([
  ...REQUIRED_EXCLUSIONS.map(normalizeField),
  'csi', 'channelstateinformation', 'rawcir', 'channelimpulseresponse',
  'rftensor', 'rftensors', 'packetcapture', 'packetcaptures', 'pcap', 'recording', 'recordings', 'audiorecording',
  'videorecording', 'poseframe', 'skeleton', 'keypoints', 'vitalwaveform',
  'heartratewaveform', 'identityobservation', 'biometric', 'biometrics', 'face', 'faces', 'faceembedding',
]);
const SPACES_ENV_ALLOWLIST = Object.freeze([
  ...DEFAULT_ENV_ALLOWLIST,
  // Operators may bind an MCP server to a credential file without putting a
  // secret or an arbitrary file path in tool-call arguments.
  'RUVIEW_CREDENTIALS_PATH',
]);

function normalizeField(value) {
  return String(value).replace(/[^a-z0-9]/gi, '').toLowerCase();
}

function assertBoundedValue(value, depth = 0, state = { nodes: 0 }) {
  state.nodes += 1;
  if (state.nodes > MAX_JSON_NODES) throw new Error('JSON structure exceeds node bound');
  if (depth > MAX_JSON_DEPTH) throw new Error('JSON nesting is too deep');
  if (typeof value === 'string') {
    if (Buffer.byteLength(value, 'utf8') > MAX_STRING_BYTES) throw new Error('string exceeds bound');
    return;
  }
  if (Array.isArray(value)) {
    if (value.length > MAX_ARRAY_ITEMS) throw new Error('array exceeds bound');
    for (const item of value) assertBoundedValue(item, depth + 1, state);
    return;
  }
  if (!value || typeof value !== 'object') return;
  const entries = Object.entries(value);
  if (entries.length > MAX_OBJECT_KEYS) throw new Error('object exceeds bound');
  for (const [key, item] of entries) {
    if (Buffer.byteLength(key, 'utf8') > MAX_STRING_BYTES) throw new Error('object key exceeds bound');
    if (FORBIDDEN_FIELDS.has(normalizeField(key))) throw new Error(`forbidden raw field: ${key}`);
    assertBoundedValue(item, depth + 1, state);
  }
}

function nonEmptyString(value) {
  return typeof value === 'string' && value.length > 0;
}

/** Parse and independently enforce the metaharness semantic boundary. */
export function parseSpacesOutput(stdout, expectedKind = undefined) {
  if (Buffer.byteLength(String(stdout), 'utf8') > MAX_CLI_JSON_BYTES) {
    throw new Error('CLI response exceeds bound');
  }
  let response;
  try {
    response = JSON.parse(String(stdout));
  } catch {
    throw new Error('CLI response is not JSON');
  }
  assertBoundedValue(response);
  if (!response || response.object !== 'list' || !Array.isArray(response.data) || response.data.length > MAX_RESOURCES) {
    throw new Error('invalid list envelope');
  }
  const versioned = response.schemaVersion !== undefined || response.kind !== undefined;
  if (versioned && (response.schemaVersion !== '1.0' || !SPATIAL_RESOURCE_KINDS.includes(response.kind)
    || (expectedKind !== undefined && response.kind !== expectedKind))) {
    throw new Error('invalid spatial contract version or kind');
  }
  if (!versioned && expectedKind !== undefined && expectedKind !== 'spaces') {
    throw new Error('legacy response is valid only for spaces');
  }
  const boundary = response.boundary;
  if (!boundary || boundary.authoritativeState !== 'HomeCore Edge' || !Array.isArray(boundary.excluded)
    || !boundary.excluded.every((item) => typeof item === 'string')) {
    throw new Error('incomplete edge privacy boundary');
  }
  for (const required of REQUIRED_EXCLUSIONS) {
    if (!boundary.excluded.includes(required)) throw new Error('incomplete edge privacy boundary');
  }
  for (const item of response.data) {
    if (!item || !ID_RE.test(String(item.id ?? '')) || !nonEmptyString(item.tenantId)) {
      throw new Error('spatial identity is incomplete');
    }
    if (!['P2', 'P3'].includes(item.privacy)) {
      throw new Error('non-semantic privacy class');
    }
    const confidence = versioned ? item.confidence : item.state?.confidence;
    if (confidence !== null && confidence !== undefined
      && (typeof confidence !== 'number' || !Number.isFinite(confidence) || confidence < 0 || confidence > 1)) {
      throw new Error('invalid confidence');
    }
    if (!versioned) {
      if (!nonEmptyString(item.siteId) || !nonEmptyString(item.name) || item.state?.classification !== 'P2') {
        throw new Error('space identity is incomplete');
      }
      continue;
    }
    if (!UUID_RE.test(String(item.workspaceId ?? '')) || item.kind !== response.kind
      || item.schemaVersion !== '1.0' || !nonEmptyString(item.messageId)
      || !Number.isSafeInteger(item.eventSequence) || item.eventSequence < 0
      || !Number.isSafeInteger(item.version) || item.version < 1
      || !nonEmptyString(item.observedAt) || !Number.isFinite(Date.parse(item.observedAt))
      || (item.expiresAt !== null && item.expiresAt !== undefined
        && (!nonEmptyString(item.expiresAt) || !Number.isFinite(Date.parse(item.expiresAt))))
      || !item.attributes || Array.isArray(item.attributes) || typeof item.attributes !== 'object'
      || !item.provenance || Array.isArray(item.provenance) || typeof item.provenance !== 'object') {
      throw new Error('versioned spatial identity is incomplete');
    }
    if (['buildings', 'floors', 'spaces', 'zones', 'entities', 'events', 'alerts'].includes(response.kind)
      && !nonEmptyString(item.siteId)) throw new Error('spatial parent is incomplete');
    if (response.kind === 'floors' && !nonEmptyString(item.buildingId)) throw new Error('spatial parent is incomplete');
    if (response.kind === 'spaces' && (!nonEmptyString(item.buildingId) || !nonEmptyString(item.floorId))) {
      throw new Error('spatial parent is incomplete');
    }
    if (['zones', 'entities', 'events', 'alerts'].includes(response.kind) && !nonEmptyString(item.spaceId)) {
      throw new Error('spatial parent is incomplete');
    }
    if (response.kind === 'entities'
      && (!['sensor', 'person', 'object', 'track'].includes(item.entityType)
        || (['person', 'track'].includes(item.entityType) && item.identityMode !== 'anonymous'))) {
      throw new Error('entity privacy contract is invalid');
    }
    if (response.kind === 'events' && !nonEmptyString(item.eventType)) throw new Error('event type is missing');
    if (response.kind === 'alerts'
      && (!nonEmptyString(item.alertType) || !['info', 'warning', 'critical'].includes(item.severity)
        || !['open', 'acknowledged', 'resolved'].includes(item.status))) {
      throw new Error('alert contract is invalid');
    }
  }
  if (versioned && response.nextCursor !== null && response.nextCursor !== undefined
    && (!nonEmptyString(response.nextCursor) || response.nextCursor.length > 512
      || /[\u0000-\u001f\u007f]/u.test(response.nextCursor))) {
    throw new Error('invalid next cursor');
  }
  return response;
}

function commandFailure(error, env) {
  const detail = redact(error?.message || error, { env }).slice(0, 1000);
  if (/lacks spaces:read/i.test(detail)) return { reason: 'spaces_scope_missing', detail };
  if (/no stored credentials|not logged in/i.test(detail)) return { reason: 'not_logged_in', detail };
  if (/refresh/i.test(detail)) return { reason: 'oauth_refresh_failed', detail };
  if (/rejected the credential|\b401\b|\b403\b/i.test(detail)) return { reason: 'authentication_failed', detail };
  return { reason: 'spaces_command_failed', detail };
}

/**
 * List Cognitum Spaces through the hardened Rust client.
 *
 * `binary` and `execute` are injectable so tests never need a real credential
 * or network. Production callers must pass a discovered installed binary; the
 * credentialed path never executes build scripts from an auto-detected repo.
 */
export async function listCognitumSpaces(input = {}, options = {}) {
  const source = options.source || 'library';
  if (source === 'mcp' && input.credentials_path !== undefined) {
    return {
      ok: false,
      reason: 'credentials_path_not_allowed',
      hint: 'Set RUVIEW_CREDENTIALS_PATH in the MCP server environment; credential paths are not accepted from tool calls.',
    };
  }

  const resource = input.resource || 'spaces';
  if (!SPATIAL_RESOURCE_KINDS.includes(resource)) {
    return { ok: false, reason: 'invalid_resource' };
  }
  const limit = input.limit === undefined ? 50 : input.limit;
  if (!Number.isSafeInteger(limit) || limit < 1 || limit > 100) {
    return { ok: false, reason: 'invalid_limit' };
  }
  if (input.cursor !== undefined
    && (typeof input.cursor !== 'string' || input.cursor.length === 0 || input.cursor.length > 512 || /[\u0000-\u001f\u007f]/u.test(input.cursor))) {
    return { ok: false, reason: 'invalid_cursor' };
  }
  const spacesArgs = [
    'spaces', '--json', '--base-url', DEFAULT_BASE_URL,
    '--resource', resource, '--limit', String(limit),
  ];
  if (input.cursor) spacesArgs.push('--cursor', input.cursor);
  if (input.credentials_path) spacesArgs.push('--credentials-path', input.credentials_path);

  let command;
  let args;
  let via;
  if (options.binary) {
    command = options.binary;
    args = spacesArgs;
    via = 'binary';
  } else {
    return {
      ok: false,
      reason: 'cli_missing',
      hint: 'Install the wifi-densepose binary; credentialed metaharness calls never execute Cargo build scripts.',
    };
  }

  const execute = options.execute || runProcess;
  let result;
  try {
    result = await execute(command, args, {
      timeoutMs: 120_000,
      maxOutputBytes: MAX_CLI_JSON_BYTES,
      env: options.env || process.env,
      envAllowlist: SPACES_ENV_ALLOWLIST,
    });
  } catch (error) {
    return { ok: false, authentication: 'oauth', via, ...commandFailure(error, options.env || process.env) };
  }

  let response;
  try {
    response = parseSpacesOutput(result.stdout, resource);
  } catch (error) {
    return {
      ok: false,
      authentication: 'oauth',
      via,
      reason: 'invalid_spaces_output',
      detail: String(error.message).slice(0, 300),
    };
  }
  return {
    ok: true,
    authentication: 'oauth',
    via,
    count: response.data.length,
    resource,
    schemaVersion: response.schemaVersion,
    nextCursor: response.nextCursor ?? null,
    data: response.data,
    boundary: response.boundary,
    authority: 'Read-only tenant/workspace projection; this result grants no action, write, pairing, or actuator authority.',
    credentialSideEffect: 'An expired OAuth session may rotate and persist its refresh credential before the read returns.',
  };
}
