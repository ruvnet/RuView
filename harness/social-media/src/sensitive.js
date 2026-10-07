// SPDX-License-Identifier: MIT

const SECRET_KEY_RE = /(?:api[_-]?key|access[_-]?token|refresh[_-]?token|client[_-]?secret|token|secret|password|passwd|authorization|cookie|private[_-]?key|setup[_-]?code|pairing[_-]?code|webhook[_-]?secret)/iu;

const VALUE_RULES = Object.freeze([
  ['inline_credential_assignment', /\b(?:api[_-]?key|access[_-]?token|refresh[_-]?token|client[_-]?secret|token|secret|password|passwd|authorization|cookie|private[_-]?key|webhook[_-]?secret)\s*[:=]\s*["']?[^\s"',;}{\]]{6,}/iu],
  ['authorization_header', /\b(?:(?:Bearer|Basic)\s+[A-Za-z0-9._~+/=,:-]{8,}|Digest\s+[A-Za-z][A-Za-z0-9_-]*=[A-Za-z0-9._~+/=,:-]{8,})/iu],
  ['private_key', /-----BEGIN (?:[A-Z0-9]+ )?PRIVATE KEY-----/u],
  ['openai_or_anthropic_key', /\b(?:sk|sk-ant|sk-proj)-[A-Za-z0-9_-]{16,}\b/u],
  ['github_token', /\bgh(?:p|o|u|s|r)_[A-Za-z0-9]{20,}\b/u],
  ['github_fine_grained_token', /\bgithub_pat_[A-Za-z0-9_]{20,}\b/u],
  ['slack_token', /\bxox[baprs]-[A-Za-z0-9-]{20,}\b/u],
  ['discord_webhook', /https?:\/\/(?:(?:canary|ptb)\.)?discord(?:app)?\.com\/api(?:\/v\d+)?\/webhooks\/[0-9]+\/[A-Za-z0-9_-]{16,}/iu],
  ['slack_webhook', /https?:\/\/hooks\.slack\.com\/services\/[A-Za-z0-9_-]+\/[A-Za-z0-9_-]+\/[A-Za-z0-9_-]+/iu],
  ['aws_access_key', /\b(?:AKIA|ASIA)[0-9A-Z]{16}\b/u],
  ['google_api_key', /\bAIza[0-9A-Za-z_-]{35}\b/u],
  ['jwt', /\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b/u],
  ['meta_access_token', /\bEAA[A-Za-z0-9]{40,}\b/u],
  ['discord_token', /\b(?:mfa\.[A-Za-z0-9_-]{20,}|[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{6}\.[A-Za-z0-9_-]{20,})\b/u],
  ['whatsapp_invite', /https?:\/\/chat\.whatsapp\.com\/[A-Za-z0-9_-]+/iu],
  ['discord_invite', /https?:\/\/(?:discord\.gg|discord\.com\/invite)\/[A-Za-z0-9_-]+/iu],
  ['capability_url', /https?:\/\/[^\s]+[?&](?:access_token|auth|code|key|sig|signature|token)=[^\s&#]+/iu],
]);

export const REDACTED = '[REDACTED]';

export function sensitiveStringRule(value) {
  if (typeof value !== 'string') return null;
  for (const [rule, pattern] of VALUE_RULES) {
    if (pattern.test(value)) return rule;
  }
  return null;
}

export function sensitiveKey(key) {
  return SECRET_KEY_RE.test(String(key));
}

const MAX_SCAN_DEPTH = 64;
const MAX_SCAN_NODES = 150_000;

function walkSensitive(value, path, state, depth) {
  state.nodes += 1;
  if (state.nodes > MAX_SCAN_NODES) return [{ path, rule: 'structure_node_limit_exceeded' }];
  if (depth > MAX_SCAN_DEPTH) return [{ path, rule: 'structure_depth_exceeded' }];
  if (typeof value === 'string') {
    const rule = sensitiveStringRule(value);
    return rule ? [{ path, rule }] : [];
  }
  if (!value || typeof value !== 'object') return [];
  if (state.seen.has(value)) return [{ path, rule: 'repeated_or_cyclic_object_reference' }];
  state.seen.add(value);
  if (Array.isArray(value)) {
    const findings = [];
    for (let index = 0; index < value.length; index += 1) {
      findings.push(...walkSensitive(value[index], `${path}[${index}]`, state, depth + 1));
      if (state.nodes > MAX_SCAN_NODES) break;
    }
    return findings;
  }
  const findings = [];
  for (const [key, item] of Object.entries(value)) {
    const current = `${path}.${key}`;
    if (sensitiveKey(key)) findings.push({ path: current, rule: 'credential_field_name' });
    findings.push(...walkSensitive(item, current, state, depth + 1));
    if (state.nodes > MAX_SCAN_NODES) break;
  }
  return findings;
}

export function scanSensitive(value, path = '$') {
  return walkSensitive(value, path, { nodes: 0, seen: new WeakSet() }, 0);
}

export function redactString(value) {
  let output = String(value ?? '');
  for (const [, pattern] of VALUE_RULES) output = output.replace(new RegExp(pattern.source, `${pattern.flags}g`.replaceAll('gg', 'g')), REDACTED);
  return output;
}

export function redactDeep(value) {
  if (typeof value === 'string') return redactString(value);
  if (Array.isArray(value)) return value.map(redactDeep);
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, sensitiveKey(key) ? REDACTED : redactDeep(item)]));
}
