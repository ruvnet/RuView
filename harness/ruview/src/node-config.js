// SPDX-License-Identifier: MIT
// Declarative node config for ESP32 CSI nodes: one YAML file describes the
// WiFi, the aggregator, the firmware and every node (matched by MAC).
// `ruview node plan|apply|status` brings attached boards to that state.
//
// Zero runtime dependencies: the YAML reader below accepts a strict subset
// (block maps, block lists, flow lists of scalars, quoted/plain scalars,
// comments) and rejects everything else (anchors, tags, multi-line scalars,
// flow maps, tabs, duplicate keys) instead of guessing.
//
// Secrets: the WiFi password and OTA key are referenced by file path only and
// handed to provision.py as paths, so credential values never pass through the
// harness (README "Firmware flashing"). Inline passwords and env-var sources
// are refused.

import {
  appendFileSync, existsSync, lstatSync, mkdirSync, readdirSync, readFileSync, statSync,
} from 'node:fs';
import { homedir, networkInterfaces } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { FIRMWARE_VARIANTS, FirmwareError, flashFirmware, resolveBundle, validatePort } from './firmware.js';

export class NodeConfigError extends Error {
  constructor(reason, message) { super(message); this.reason = reason; }
}

// ───────────────────────────────────────────────────────── YAML subset ────

function stripComment(line) {
  let quote = null;
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (quote) { if (c === quote) quote = null; else if (c === '\\' && quote === '"') i++; continue; }
    if (c === '"' || c === "'") quote = c;
    else if (c === '#' && (i === 0 || /\s/.test(line[i - 1]))) return line.slice(0, i);
  }
  return line;
}

function parseScalar(raw, lineNo) {
  const v = raw.trim();
  const fail = (what) => { throw new NodeConfigError('yaml_unsupported', `line ${lineNo}: ${what} is not supported in node-config YAML`); };
  if (v === '' || v === '~' || v === 'null') return null;
  if (/^[&*!|>{@`%]/.test(v)) fail(`"${v[0]}" (anchors, tags, block scalars, flow maps)`);
  if (v.startsWith('[')) {
    if (!v.endsWith(']')) fail('a multi-line flow list');
    const inner = v.slice(1, -1).trim();
    if (!inner) return [];
    if (/[[\]{}]/.test(inner)) fail('a nested flow collection');
    return inner.split(',').map((item) => parseScalar(item, lineNo));
  }
  if (v.startsWith('"')) {
    if (!/^"(?:[^"\\]|\\.)*"$/.test(v)) throw new NodeConfigError('yaml_invalid', `line ${lineNo}: unterminated double-quoted string`);
    return v.slice(1, -1).replace(/\\(.)/g, (_, ch) => ({ n: '\n', t: '\t', '"': '"', '\\': '\\' })[ch] ?? fail(`escape \\${ch}`));
  }
  if (v.startsWith("'")) {
    if (!/^'(?:[^']|'')*'$/.test(v)) throw new NodeConfigError('yaml_invalid', `line ${lineNo}: unterminated single-quoted string`);
    return v.slice(1, -1).replaceAll("''", "'");
  }
  if (v === 'true') return true;
  if (v === 'false') return false;
  if (/^-?\d+$/.test(v)) return Number(v);
  if (/^-?\d+\.\d+$/.test(v)) return Number(v);
  return v;
}

const KEY_RE = /^([A-Za-z_][A-Za-z0-9_-]*):(?:\s+(.*))?$/;
const isItem = (text) => text === '-' || text.startsWith('- ');

/** Parse the node-config YAML subset into plain objects/arrays/scalars. */
export function parseNodeYaml(text) {
  const lines = [];
  String(text).split(/\r?\n/).forEach((raw, idx) => {
    const lineNo = idx + 1;
    const body = stripComment(raw).replace(/\s+$/, '');
    if (!body.trim() || body.trim() === '---') return;
    const lead = /^[ \t]*/.exec(body)[0];
    if (lead.includes('\t')) throw new NodeConfigError('yaml_invalid', `line ${lineNo}: tabs are not allowed for indentation`);
    lines.push({ indent: lead.length, text: body.slice(lead.length), lineNo });
  });
  if (!lines.length) return {};

  function block(i, indent) {
    return isItem(lines[i].text) ? list(i, indent) : map(i, indent);
  }
  function map(i, indent) {
    const out = {};
    while (i < lines.length && lines[i].indent === indent && !isItem(lines[i].text)) {
      const { text, lineNo } = lines[i];
      const m = KEY_RE.exec(text);
      if (!m) throw new NodeConfigError('yaml_invalid', `line ${lineNo}: expected "key: value"`);
      const key = m[1];
      if (key === '__proto__' || key === 'constructor' || key === 'prototype') throw new NodeConfigError('yaml_invalid', `line ${lineNo}: reserved key "${key}"`);
      if (Object.hasOwn(out, key)) throw new NodeConfigError('yaml_invalid', `line ${lineNo}: duplicate key "${key}"`);
      i += 1;
      if (m[2] !== undefined && m[2].trim() !== '') {
        out[key] = parseScalar(m[2], lineNo);
      } else if (i < lines.length && (lines[i].indent > indent || (lines[i].indent === indent && isItem(lines[i].text)))) {
        [out[key], i] = block(i, lines[i].indent);
      } else {
        out[key] = null;
      }
    }
    if (i < lines.length && lines[i].indent > indent) throw new NodeConfigError('yaml_invalid', `line ${lines[i].lineNo}: unexpected indentation`);
    return [out, i];
  }
  function list(i, indent) {
    const out = [];
    while (i < lines.length && lines[i].indent === indent && isItem(lines[i].text)) {
      const { text, lineNo } = lines[i];
      const rest = text === '-' ? '' : text.slice(2);
      if (rest.startsWith(' ')) throw new NodeConfigError('yaml_invalid', `line ${lineNo}: use exactly one space after "-"`);
      if (!rest) {
        i += 1;
        if (i < lines.length && lines[i].indent > indent) [out[out.length], i] = block(i, lines[i].indent);
        else out.push(null);
      } else if (KEY_RE.test(rest)) {
        lines[i] = { indent: indent + 2, text: rest, lineNo }; // "- key: v" opens a map at indent+2
        const [value, next] = map(i, indent + 2);
        out.push(value); i = next;
      } else {
        out.push(parseScalar(rest, lineNo)); i += 1;
      }
    }
    if (i < lines.length && lines[i].indent > indent) throw new NodeConfigError('yaml_invalid', `line ${lines[i].lineNo}: unexpected indentation`);
    return [out, i];
  }

  const [value, end] = block(0, lines[0].indent);
  if (end < lines.length) throw new NodeConfigError('yaml_invalid', `line ${lines[end].lineNo}: unexpected content at lower indentation`);
  if (Array.isArray(value)) throw new NodeConfigError('yaml_invalid', 'node config must be a mapping at the top level');
  return value;
}

// ──────────────────────────────────────────────────────── config schema ───

export const DEFAULT_CONFIG_PATH = join(homedir(), '.ruview', 'nodes.yaml');
export const DEFAULT_REGISTRY_PATH = join(homedir(), '.ruview', 'esp32-setup', 'devices.jsonl');
export const STEPS = Object.freeze(['device', 'driver', 'flash', 'wifi', 'validate']);
const MAC_RE = /^([0-9a-f]{2}:){5}[0-9a-f]{2}$/;
const IPV4_RE = /^(25[0-5]|2[0-4]\d|1?\d?\d)(\.(25[0-5]|2[0-4]\d|1?\d?\d)){3}$/;

/** Provisioning keys a node (or `defaults`) may set → provision.py flag + check. */
const PROVISION_KEYS = Object.freeze({
  edge_tier: { flag: '--edge-tier', check: (v) => [0, 1, 2].includes(v), want: '0, 1 or 2' },
  channel: { flag: '--channel', check: (v) => Number.isInteger(v) && ((v >= 1 && v <= 14) || (v >= 36 && v <= 177)), want: '1-14 or 36-177' },
  filter_mac: { flag: '--filter-mac', check: (v) => typeof v === 'string' && MAC_RE.test(v.toLowerCase()), want: 'AA:BB:CC:DD:EE:FF' },
  hop_channels: { flag: '--hop-channels', check: (v) => Array.isArray(v) && v.length > 0 && v.every((c) => Number.isInteger(c) && c > 0 && c <= 177), want: 'a list of channels', format: (v) => v.join(',') },
  hop_dwell: { flag: '--hop-dwell', check: (v) => Number.isInteger(v) && v >= 10 && v <= 60000, want: '10-60000 ms' },
  pres_thresh: { flag: '--pres-thresh', check: (v) => Number.isInteger(v) && v >= 0, want: 'a non-negative integer' },
  fall_thresh: { flag: '--fall-thresh', check: (v) => Number.isInteger(v) && v >= 0, want: 'a non-negative integer' },
  vital_win: { flag: '--vital-win', check: (v) => Number.isInteger(v) && v > 0, want: 'a positive integer' },
  vital_int: { flag: '--vital-int', check: (v) => Number.isInteger(v) && v > 0, want: 'a positive integer' },
  subk_count: { flag: '--subk-count', check: (v) => Number.isInteger(v) && v > 0, want: 'a positive integer' },
  zone: { flag: '--zone', check: (v) => typeof v === 'string' && /^[\w .-]{1,32}$/.test(v), want: '1-32 chars of letters, digits, space, . _ -' },
});
const NODE_KEYS = new Set(['mac', 'node_id', 'tdm_slot', ...Object.keys(PROVISION_KEYS)]);
const SECTIONS = Object.freeze({
  wifi: new Set(['ssid', 'password_file', 'password_env', 'password']),
  aggregator: new Set(['ip', 'port']),
  firmware: new Set(['source', 'bundle', 'variant', 'allow_unverified', 'erase', 'baud']),
  validate: new Set(['wait_secs', 'udp_secs', 'min_pps', 'max_loss']),
  ota: new Set(['psk_file']),
});
const TOP_KEYS = new Set([...Object.keys(SECTIONS), 'defaults', 'nodes', 'steps', 'auto_assign', 'registry']);

const expandHome = (p, home = homedir()) => (typeof p === 'string' && (p === '~' || p.startsWith('~/')) ? join(home, p.slice(2)) : p);

/**
 * Validate a parsed node-config document and fill defaults. Collects every error
 * (fail closed) rather than stopping at the first one.
 */
export function normalizeNodeConfig(doc, { home = homedir() } = {}) {
  const errors = [];
  const err = (path, msg) => errors.push(`${path}: ${msg}`);
  const obj = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
  if (!obj(doc)) throw new NodeConfigError('config_invalid', 'node config must be a mapping');
  for (const k of Object.keys(doc)) if (!TOP_KEYS.has(k)) err(k, `unknown key (allowed: ${[...TOP_KEYS].join(', ')})`);
  const section = (name) => {
    const v = doc[name] ?? {};
    if (!obj(v)) { err(name, 'must be a mapping'); return {}; }
    for (const k of Object.keys(v)) if (!SECTIONS[name].has(k)) err(`${name}.${k}`, `unknown key (allowed: ${[...SECTIONS[name]].join(', ')})`);
    return v;
  };
  const wifi = section('wifi');
  const agg = section('aggregator');
  const fw = section('firmware');
  const val = section('validate');
  const ota = section('ota');

  // WiFi — secrets by reference only.
  if (wifi.password !== undefined) err('wifi.password', 'inline passwords are refused; use password_file (chmod 600)');
  if (wifi.password_env !== undefined) err('wifi.password_env', 'not supported: credentials never pass through the harness; use password_file (chmod 600)');
  if (wifi.ssid !== undefined && (typeof wifi.ssid !== 'string' || !wifi.ssid || wifi.ssid.length > 32)) err('wifi.ssid', 'must be a 1-32 character string');
  if (wifi.password_file !== undefined && typeof wifi.password_file !== 'string') err('wifi.password_file', 'must be a path');
  if (ota.psk_file !== undefined && typeof ota.psk_file !== 'string') err('ota.psk_file', 'must be a path');

  // Aggregator.
  const ip = agg.ip ?? 'auto';
  if (ip !== 'auto' && !(typeof ip === 'string' && IPV4_RE.test(ip))) err('aggregator.ip', 'must be "auto" or an IPv4 address');
  const port = agg.port ?? 5005;
  if (!Number.isInteger(port) || port < 1024 || port > 65535) err('aggregator.port', 'must be an integer 1024-65535');

  // Firmware.
  const source = fw.source ?? 'auto';
  if (!['auto', 'reuse', 'build', 'bundle'].includes(source)) err('firmware.source', 'must be auto, reuse, build or bundle');
  if (source === 'bundle' && typeof fw.bundle !== 'string') err('firmware.bundle', 'required when source is bundle');
  const variant = fw.variant ?? 's3-8mb';
  if (!FIRMWARE_VARIANTS[variant]) err('firmware.variant', `must be one of ${Object.keys(FIRMWARE_VARIANTS).join(', ')}`);
  if (fw.erase === true) err('firmware.erase', 'erase is not supported by `ruview node` (it would wipe NVS); erase manually with esptool if you must');
  for (const b of ['allow_unverified', 'erase']) if (fw[b] !== undefined && typeof fw[b] !== 'boolean') err(`firmware.${b}`, 'must be true or false');
  const baud = fw.baud ?? 460800;
  if (![115200, 230400, 460800, 921600].includes(baud)) err('firmware.baud', 'must be 115200, 230400, 460800 or 921600');

  // Validation thresholds.
  const num = (path, v, lo, hi, dflt) => {
    if (v === undefined || v === null) return dflt;
    if (typeof v !== 'number' || !Number.isFinite(v) || v < lo || v > hi) { err(path, `must be a number ${lo}-${hi}`); return dflt; }
    return v;
  };
  const validate = {
    wait_secs: num('validate.wait_secs', val.wait_secs, 5, 300, 60),
    udp_secs: num('validate.udp_secs', val.udp_secs, 1, 300, 15),
    min_pps: num('validate.min_pps', val.min_pps, 0, 10000, 20),
    max_loss: val.max_loss === undefined ? null : num('validate.max_loss', val.max_loss, 0, 1, null),
  };

  // Steps.
  let steps = doc.steps ?? STEPS;
  if (!Array.isArray(steps) || steps.some((s) => !STEPS.includes(s))) { err('steps', `must be a list of ${STEPS.join(', ')}`); steps = STEPS; }

  // Defaults + nodes.
  const checkProvision = (path, entry) => {
    for (const [k, spec] of Object.entries(PROVISION_KEYS)) {
      if (entry[k] !== undefined && entry[k] !== null && !spec.check(entry[k])) err(`${path}.${k}`, `must be ${spec.want}`);
    }
  };
  const defaults = doc.defaults ?? {};
  if (!obj(defaults)) err('defaults', 'must be a mapping');
  else {
    for (const k of Object.keys(defaults)) if (!PROVISION_KEYS[k]) err(`defaults.${k}`, `unknown key (allowed: ${Object.keys(PROVISION_KEYS).join(', ')})`);
    checkProvision('defaults', defaults);
  }
  const rawNodes = doc.nodes ?? [];
  const nodes = [];
  if (!Array.isArray(rawNodes)) err('nodes', 'must be a list');
  else {
    const seen = { mac: new Map(), node_id: new Map(), tdm_slot: new Map() };
    rawNodes.forEach((n, i) => {
      const path = `nodes[${i}]`;
      if (!obj(n)) { err(path, 'must be a mapping'); return; }
      for (const k of Object.keys(n)) if (!NODE_KEYS.has(k)) err(`${path}.${k}`, `unknown key (allowed: ${[...NODE_KEYS].join(', ')})`);
      const mac = typeof n.mac === 'string' ? n.mac.toLowerCase() : null;
      if (!mac || !MAC_RE.test(mac)) err(`${path}.mac`, 'required, like aa:bb:cc:dd:ee:01');
      if (!Number.isInteger(n.node_id) || n.node_id < 0 || n.node_id > 255) err(`${path}.node_id`, 'required integer 0-255');
      if (n.tdm_slot !== undefined && !(Number.isInteger(n.tdm_slot) && n.tdm_slot >= 0 && n.tdm_slot <= 255)) err(`${path}.tdm_slot`, 'must be an integer 0-255');
      checkProvision(path, n);
      for (const [k, v] of [['mac', mac], ['node_id', n.node_id], ['tdm_slot', n.tdm_slot]]) {
        if (v === undefined || v === null) continue;
        if (seen[k].has(v)) err(`${path}.${k}`, `duplicates ${seen[k].get(v)}`);
        else seen[k].set(v, path);
      }
      nodes.push({ ...n, mac });
    });
  }
  if (doc.auto_assign !== undefined && typeof doc.auto_assign !== 'boolean') err('auto_assign', 'must be true or false');
  if (doc.registry !== undefined && typeof doc.registry !== 'string') err('registry', 'must be a path');

  if (errors.length) {
    const e = new NodeConfigError('config_invalid', `node config has ${errors.length} problem(s)`);
    e.errors = errors;
    throw e;
  }
  return {
    wifi: {
      ssid: wifi.ssid ?? null,
      password_file: wifi.password_file ? expandHome(wifi.password_file, home) : null,
    },
    aggregator: { ip, port },
    firmware: { source, bundle: fw.bundle ? expandHome(fw.bundle, home) : null, variant, allow_unverified: fw.allow_unverified === true, baud },
    ota: { psk_file: ota.psk_file ? expandHome(ota.psk_file, home) : null },
    validate,
    steps,
    defaults: obj(defaults) ? defaults : {},
    nodes,
    auto_assign: doc.auto_assign !== false,
    registry: expandHome(doc.registry, home) || DEFAULT_REGISTRY_PATH,
    tdm_total: nodes.filter((n) => n.tdm_slot !== undefined).length,
  };
}

/** Read + parse + validate a node config file. */
export function loadNodeConfig(path = DEFAULT_CONFIG_PATH, opts = {}) {
  const file = resolve(expandHome(path, opts.home));
  if (!existsSync(file)) throw new NodeConfigError('config_missing', `node config not found: ${file} (copy firmware/esp32-csi-node/nodes.example.yaml)`);
  const config = normalizeNodeConfig(parseNodeYaml(readFileSync(file, 'utf8')), opts);
  return { ...config, path: file };
}

// ─────────────────────────────────────────────────────── node resolution ──

export function readRegistry(path) {
  if (!path || !existsSync(path)) return [];
  const out = [];
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    if (!line.trim()) continue;
    try { out.push(JSON.parse(line)); } catch { /* skip a torn line */ }
  }
  return out;
}

/** Append one event; never pass secrets in `fields`. */
export function appendRegistry(path, event, fields = {}, now = () => new Date()) {
  mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
  const rec = { ts: now().toISOString(), event };
  for (const [k, v] of Object.entries(fields)) if (v !== undefined && v !== null && v !== '') rec[k] = v;
  appendFileSync(path, `${JSON.stringify(rec)}\n`, { mode: 0o600 });
  return rec;
}

/**
 * Map a board MAC to its node entry: the config wins; otherwise the node_id
 * this MAC got before (registry); otherwise the next free id if auto_assign.
 */
export function resolveNode(config, mac, registry = []) {
  const m = String(mac || '').toLowerCase();
  if (!MAC_RE.test(m)) throw new NodeConfigError('invalid_mac', `not a MAC address: ${mac}`);
  const listed = config.nodes.find((n) => n.mac === m);
  let entry; let source;
  if (listed) { entry = listed; source = 'config'; }
  else {
    const prior = registry.filter((r) => r.mac === m && Number.isInteger(Number(r.node_id))).at(-1);
    if (prior) { entry = { mac: m, node_id: Number(prior.node_id) }; source = 'registry'; }
    else if (config.auto_assign) {
      const used = new Set([...config.nodes.map((n) => n.node_id), ...registry.map((r) => Number(r.node_id)).filter(Number.isInteger)]);
      let id = 1; while (used.has(id) && id < 256) id += 1;
      if (id > 255) throw new NodeConfigError('node_ids_exhausted', 'no free node_id left (0-255)');
      entry = { mac: m, node_id: id }; source = 'auto-assigned';
    } else {
      throw new NodeConfigError('node_unlisted', `MAC ${m} is not in the node config and auto_assign is false; add it under nodes:`);
    }
  }
  const node = { ...config.defaults, ...entry };
  if (node.tdm_slot !== undefined) node.tdm_total = config.tdm_total;
  return { node, source };
}

/** Host IPv4 for the aggregator: the configured one, or the first private LAN address. */
export function resolveTargetIp(config, nets = networkInterfaces()) {
  if (config.aggregator.ip !== 'auto') return { ip: config.aggregator.ip, candidates: [config.aggregator.ip] };
  const candidates = Object.values(nets).flat()
    .filter((a) => a && (a.family === 'IPv4' || a.family === 4) && !a.internal && /^(10\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.)/.test(a.address))
    .map((a) => a.address);
  if (!candidates.length) throw new NodeConfigError('no_lan_ip', 'no private IPv4 address on this host; set aggregator.ip');
  return { ip: candidates[0], candidates };
}

/** provision.py argv (no secrets — only file paths). */
export function provisionArgs(config, node, { port, targetIp, passwordFile }) {
  validatePort(port);
  if (!config.wifi.ssid) throw new NodeConfigError('ssid_missing', 'wifi.ssid is required to provision');
  if (!passwordFile) throw new NodeConfigError('password_missing', 'wifi.password_file is required to provision');
  const chip = FIRMWARE_VARIANTS[config.firmware.variant].chip;
  const argv = ['--port', port, '--chip', chip, '--ssid', config.wifi.ssid, '--password-file', passwordFile,
    '--target-ip', targetIp, '--target-port', String(config.aggregator.port), '--node-id', String(node.node_id)];
  if (node.tdm_slot !== undefined) argv.push('--tdm-slot', String(node.tdm_slot), '--tdm-total', String(node.tdm_total));
  for (const [k, spec] of Object.entries(PROVISION_KEYS)) {
    if (node[k] === undefined || node[k] === null) continue;
    argv.push(spec.flag, spec.format ? spec.format(node[k]) : String(node[k]));
  }
  if (config.ota.psk_file) argv.push('--ota-psk-file', config.ota.psk_file);
  return argv;
}

/** Owner-only regular file check for secret files (POSIX). */
function checkSecretFile(path, what) {
  if (!existsSync(path)) throw new NodeConfigError('secret_missing', `${what} not found: ${path}`);
  const st = lstatSync(path);
  if (!st.isFile()) throw new NodeConfigError('secret_untrusted', `${what} must be a regular file (not a symlink): ${path}`);
  if (process.platform !== 'win32' && (st.mode & 0o077)) throw new NodeConfigError('secret_permissions', `${what} must be owner-only (chmod 600): ${path}`);
}

/** The WiFi password file path, after an owner-only regular-file check. Never read. */
export function passwordFile(config) {
  if (!config.wifi.password_file) throw new NodeConfigError('password_missing', 'configure wifi.password_file (chmod 600)');
  checkSecretFile(config.wifi.password_file, 'wifi.password_file');
  return config.wifi.password_file;
}

/**
 * Firmware images for the configured source. `reuse`/`auto` use the repo's
 * ESP-IDF build/ (a local build you produced → unverified by design);
 * `bundle` requires SHA256SUMS unless allow_unverified.
 */
export function firmwareSource(config, repoRoot) {
  const { source, variant } = config.firmware;
  if (source === 'bundle') return { dir: config.firmware.bundle, allowUnverified: config.firmware.allow_unverified, kind: 'bundle' };
  if (source === 'build') {
    throw new NodeConfigError('build_unsupported', '`ruview node` does not build firmware. Build with Docker (firmware/esp32-csi-node/README.md "Building"), then use source: reuse');
  }
  if (!repoRoot) throw new NodeConfigError('repo_missing', 'source reuse/auto needs a RuView checkout (run inside the repo) or use source: bundle');
  const dir = join(repoRoot, 'firmware', 'esp32-csi-node', 'build');
  if (!existsSync(join(dir, 'esp32-csi-node.bin'))) {
    throw new NodeConfigError('build_missing', `no firmware build at ${dir}; build it first (see firmware/esp32-csi-node/README.md) or use source: bundle`);
  }
  const sdk = join(repoRoot, 'firmware', 'esp32-csi-node', 'sdkconfig');
  const want = FIRMWARE_VARIANTS[variant].chip;
  if (existsSync(sdk) && !readFileSync(sdk, 'utf8').includes(`CONFIG_IDF_TARGET="${want}"`)) {
    throw new NodeConfigError('build_target_mismatch', `build/ was not built for ${want} (sdkconfig CONFIG_IDF_TARGET); rebuild or fix firmware.variant`);
  }
  const app = statSync(join(dir, 'esp32-csi-node.bin')).mtimeMs;
  const stale = ['main', 'components', 'CMakeLists.txt', 'sdkconfig.defaults'].some((p) => newerThan(join(repoRoot, 'firmware', 'esp32-csi-node', p), app));
  if (stale && source === 'auto') {
    throw new NodeConfigError('build_stale', 'firmware sources changed after the last build; rebuild, or set source: reuse to flash the existing build anyway');
  }
  return { dir, allowUnverified: true, kind: 'local-build', stale };
}

function newerThan(path, t, depth = 0) {
  if (!existsSync(path) || depth > 6) return false;
  const st = statSync(path);
  if (st.isFile()) return st.mtimeMs > t;
  if (!st.isDirectory()) return false;
  for (const name of readdirSync(path)) if (newerThan(join(path, name), t, depth + 1)) return true;
  return false;
}

// ───────────────────────────────────────────────────────────── flows ──────

const ESPTOOL_MAC_RE = /MAC:\s*((?:[0-9a-f]{2}:){5}[0-9a-f]{2})/i;

/** Plan only: validate the config, resolve every listed node, never touch hardware. */
export function planNodes(config, { repoRoot, nets } = {}) {
  const warnings = [];
  let target = null;
  try { target = resolveTargetIp(config, nets); if (target.candidates.length > 1) warnings.push(`aggregator.ip auto picked ${target.ip} of ${target.candidates.join(', ')}; set it explicitly if wrong`); } catch (e) { warnings.push(e.message); }
  let firmware = null;
  try {
    const src = firmwareSource(config, repoRoot);
    const bundle = resolveBundle(src.dir, config.firmware.variant, { requireChecksums: !src.allowUnverified });
    firmware = { source: config.firmware.source, kind: src.kind, dir: bundle.dir, variant: bundle.variant, chip: bundle.chipName, flashSize: bundle.flashSize, images: bundle.images.map((i) => ({ role: i.role, offset: i.offset, file: i.relative, integrity: i.integrity })) };
    if (src.stale) warnings.push('build/ is older than the firmware sources (source: reuse flashes it anyway)');
  } catch (e) {
    if (!(e instanceof NodeConfigError || e instanceof FirmwareError)) throw e;
    firmware = { source: config.firmware.source, error: e.reason, detail: e.message };
  }
  const secrets = {
    wifi_password: config.wifi.password_file ? `file ${config.wifi.password_file}` : 'not configured',
    ota_psk: config.ota.psk_file ? `file ${config.ota.psk_file}` : 'not configured — remote OTA and config_push stay disabled',
  };
  if (!config.ota.psk_file) warnings.push('no ota.psk_file: provisioning disables OTA updates and remote config_push on these nodes');
  const nodes = config.nodes.map((n) => {
    const { node } = resolveNode(config, n.mac, []);
    return { mac: n.mac, node_id: node.node_id, zone: node.zone ?? null, tdm: node.tdm_slot !== undefined ? `${node.tdm_slot}/${node.tdm_total}` : null, edge_tier: node.edge_tier ?? null };
  });
  return {
    ok: !firmware.error,
    config: config.path,
    ssid: config.wifi.ssid,
    target: target ? `${target.ip}:${config.aggregator.port}` : null,
    steps: config.steps,
    secrets,
    firmware,
    validate: config.validate,
    auto_assign: config.auto_assign,
    registry: config.registry,
    nodes,
    warnings,
  };
}

/** Wait until `nodeId` shows up in short captures, then measure it once. */
async function measureNode(config, nodeId, deps) {
  const deadline = Date.now() + config.validate.wait_secs * 1000;
  let seen = null;
  while (Date.now() < deadline) {
    const probe = await deps.capture({ udp_port: config.aggregator.port, seconds: 3 });
    if (probe.reason === 'port_in_use') return { ok: false, reason: 'port_in_use', detail: probe.detail };
    seen = (probe.nodes || []).find((n) => n.nodeId === nodeId && (n.packets?.csi || 0) > 0);
    if (seen) break;
  }
  if (!seen) return { ok: false, reason: 'node_silent', detail: `node ${nodeId} sent no CSI to UDP ${config.aggregator.port} within ${config.validate.wait_secs}s` };
  const res = await deps.capture({ udp_port: config.aggregator.port, seconds: config.validate.udp_secs });
  const n = (res.nodes || []).find((x) => x.nodeId === nodeId);
  if (!n) return { ok: false, reason: 'node_silent', detail: `node ${nodeId} stopped sending during the ${config.validate.udp_secs}s measurement` };
  return judgeNode(config, n);
}

function judgeNode(config, n) {
  const problems = [];
  if (n.csiRateHz < config.validate.min_pps) problems.push(`CSI ${n.csiRateHz} Hz < ${config.validate.min_pps}`);
  if (config.validate.max_loss !== null && n.csiLossFraction !== null && n.csiLossFraction > config.validate.max_loss) {
    problems.push(`loss ${(n.csiLossFraction * 100).toFixed(1)}% > ${(config.validate.max_loss * 100).toFixed(1)}%`);
  }
  return {
    ok: problems.length === 0,
    reason: problems.length ? 'below_threshold' : undefined,
    problems,
    address: n.address ?? null,
    csiRateHz: n.csiRateHz,
    csiLossFraction: n.csiLossFraction,
    rssiMean: n.rssiMean,
    freqMhz: n.csi?.freqMhz ?? null,
    evidence: 'MEASURED: live UDP CSI from this node decoded on this host',
  };
}

/**
 * apply (config steps: flash → provision → validate) or firmware-only
 * (flash → validate) to one board. Plans only unless `confirm`. deps = { exec, python, capture, repoRoot, now, nets, log }.
 */
export async function applyBoard(config, { port, flow = 'apply', confirm = false }, deps) {
  validatePort(port);
  const steps = new Set(flow === 'firmware' ? ['flash', 'validate'] : config.steps);
  const registry = readRegistry(config.registry);
  const python = deps.python();
  if (!python) return { ok: false, reason: 'python_missing', remedy: 'Install Python 3 and `pip install esptool pyserial`.' };

  const log = deps.log || (() => {});
  // Identify the board (read-only).
  log(`reading MAC on ${port}`);
  const macProbe = await deps.exec(python, ['-m', 'esptool', '--port', port, 'read_mac'], { timeoutMs: 60_000 });
  const mac = (ESPTOOL_MAC_RE.exec(`${macProbe.stdout}\n${macProbe.stderr}`) || [])[1]?.toLowerCase();
  if (!mac) return { ok: false, reason: 'mac_probe_failed', port, detail: (macProbe.stderr || macProbe.error || '').slice(-600), remedy: 'Check the cable/port, close serial monitors, or hold BOOT while pressing RESET.' };
  let resolved;
  try { resolved = resolveNode(config, mac, registry); } catch (e) { if (e instanceof NodeConfigError) return { ok: false, reason: e.reason, port, mac, detail: e.message }; throw e; }
  const { node, source } = resolved;
  const result = { ok: false, flow, port, mac, node_id: node.node_id, zone: node.zone ?? null, node_source: source, steps: [...steps], stages: {} };
  if (source === 'auto-assigned' && confirm) appendRegistry(config.registry, 'node_assigned', { mac, node_id: node.node_id }, deps.now);

  let target;
  try { target = resolveTargetIp(config, deps.nets).ip; } catch (e) { return { ...result, reason: e.reason, detail: e.message }; }

  // Plan.
  let fwSrc = null;
  if (steps.has('flash')) {
    try { fwSrc = firmwareSource(config, deps.repoRoot); } catch (e) { if (e instanceof NodeConfigError) return { ...result, reason: e.reason, detail: e.message }; throw e; }
  }
  const provisionScript = deps.repoRoot ? join(deps.repoRoot, 'firmware', 'esp32-csi-node', 'provision.py') : null;
  if (steps.has('wifi') && !(provisionScript && existsSync(provisionScript))) {
    return { ...result, reason: 'provision_missing', detail: 'provision.py not found; run inside a RuView checkout' };
  }

  if (!confirm) {
    const pw = config.wifi.password_file;
    const plan = { target: `${target}:${config.aggregator.port}` };
    if (fwSrc) {
      const fwPlan = await flashFirmware({ port, bundle: fwSrc.dir, variant: config.firmware.variant, baud: config.firmware.baud, allow_unverified: fwSrc.allowUnverified }, deps);
      plan.flash = fwPlan.ok ? fwPlan.plan : { error: fwPlan.reason, detail: fwPlan.detail };
    }
    if (steps.has('wifi')) {
      try { plan.provision = ['python', 'provision.py', ...provisionArgs(config, node, { port, targetIp: target, passwordFile: pw || '<missing>' })]; } catch (e) { plan.provision = { error: e.reason, detail: e.message }; }
    }
    if (steps.has('validate')) plan.validate = { udp_port: config.aggregator.port, ...config.validate };
    return { ...result, ok: true, dryRun: true, plan, next: 'Re-run with --confirm (or answer yes at a terminal) to write. Nothing was written.' };
  }

  const reg = (event, fields) => appendRegistry(config.registry, event, { flow, port, mac, node_id: node.node_id, zone: node.zone, ...fields }, deps.now);
  reg('chip_probed', { node_src: source });

  // Flash (NVS preserved by the image set).
  if (steps.has('flash')) {
    log(`flashing ${fwSrc.kind} (${config.firmware.variant}) and capturing the boot log — ~1 min`);
    const fw = await flashFirmware({ port, bundle: fwSrc.dir, variant: config.firmware.variant, baud: config.firmware.baud, allow_unverified: fwSrc.allowUnverified, confirm: true, boot_log_seconds: 15 }, deps);
    result.stages.flash = { ok: fw.ok, reason: fw.reason, firmwareVersion: fw.bootLog?.firmwareVersion ?? null, panic: fw.bootLog?.panic ?? null, detail: fw.detail };
    if (!fw.ok || fw.bootLog?.panic) {
      reg('flash_failed', { reason: fw.reason || 'boot_panic' });
      return { ...result, reason: fw.reason || 'boot_panic', detail: fw.detail || 'boot log shows a panic' };
    }
    reg('flashed', { fw_version: fw.bootLog?.firmwareVersion, variant: config.firmware.variant, firmware_kind: fwSrc.kind });
  }

  // Provision NVS.
  if (steps.has('wifi')) {
    log(`provisioning WiFi "${config.wifi.ssid}" → ${target}:${config.aggregator.port}, node ${node.node_id}`);
    let pw;
    try {
      pw = passwordFile(config);
      if (config.ota.psk_file) checkSecretFile(config.ota.psk_file, 'ota.psk_file');
    } catch (e) { if (e instanceof NodeConfigError) return { ...result, reason: e.reason, detail: e.message }; throw e; }
    const run = await deps.exec(python, [provisionScript, ...provisionArgs(config, node, { port, targetIp: target, passwordFile: pw })], { timeoutMs: 180_000 });
    result.stages.wifi = { ok: run.ok, target: `${target}:${config.aggregator.port}`, ota: Boolean(config.ota.psk_file) };
    if (!run.ok) {
      reg('provision_failed', {});
      return { ...result, reason: 'provision_failed', detail: `${run.stdout}\n${run.stderr}`.slice(-800) };
    }
    reg('provisioned', { target: `${target}:${config.aggregator.port}`, ota: config.ota.psk_file ? 'enabled' : 'disabled' });
  }

  // Validate on the network.
  if (steps.has('validate')) {
    log(`waiting up to ${config.validate.wait_secs}s for node ${node.node_id} on UDP ${config.aggregator.port}, then measuring ${config.validate.udp_secs}s`);
    const v = await measureNode(config, node.node_id, deps);
    result.stages.validate = v;
    reg('validated', { ip: v.address, csi_pps: v.csiRateHz, csi_loss: v.csiLossFraction, rssi: v.rssiMean, health: v.ok ? 'healthy' : (v.reason || 'degraded') });
    if (!v.ok) return { ...result, reason: v.reason, detail: v.detail || v.problems?.join('; ') };
  }
  return { ...result, ok: true };
}

/** Read-only node health: one UDP capture + each node's :8032/ota/status. */
export async function nodeStatus(config, deps) {
  const cap = await deps.capture({ udp_port: config.aggregator.port, seconds: config.validate.udp_secs });
  if (cap.reason === 'port_in_use' || cap.reason === 'invalid_port') return { ok: false, reason: cap.reason, detail: cap.detail, remedy: cap.remedy };
  const heard = new Map((cap.nodes || []).filter((n) => n.source === 'esp32').map((n) => [n.nodeId, n]));
  const registry = readRegistry(config.registry);
  const lastIp = new Map();
  for (const r of registry) if (r.ip && r.node_id !== undefined) lastIp.set(Number(r.node_id), r.ip);
  // Nodes `node apply` auto-assigned are known too (registry), even if not in the YAML.
  const assigned = new Map();
  for (const r of registry) if (r.event === 'node_assigned' && r.mac && Number.isInteger(Number(r.node_id))) assigned.set(Number(r.node_id), r.mac);
  const known = (id) => config.nodes.find((n) => n.node_id === id)
    || (assigned.has(id) && !config.nodes.some((n) => n.mac === assigned.get(id)) ? { node_id: id, mac: assigned.get(id), zone: null, source: 'registry' } : null);
  const ids = new Set([...config.nodes.map((n) => n.node_id), ...heard.keys()]);
  const rows = [];
  for (const id of [...ids].sort((a, b) => a - b)) {
    const listed = known(id);
    const n = heard.get(id);
    const address = n?.address || lastIp.get(id) || null;
    let http = null;
    if (address && /^(10\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.|169\.254\.)/.test(address)) {
      try {
        const r = await deps.fetch(`http://${address}:8032/ota/status`, { signal: AbortSignal.timeout(2500) });
        const j = r.ok ? await r.json() : null;
        http = j ? { version: j.version ?? null, ota_state: j.ota_state ?? null } : { error: `HTTP ${r.status}` };
      } catch { http = { error: 'unreachable' }; }
    }
    const judged = n ? judgeNode(config, n) : null;
    const health = !listed ? 'unlisted' : judged?.ok ? 'healthy' : n ? 'degraded' : http && !http.error ? 'no-csi-here' : 'offline';
    rows.push({
      node_id: id, zone: listed?.zone ?? null, mac: listed?.mac ?? null, source: listed ? (listed.source || 'config') : null, address,
      firmware: http?.version ?? null, ota_state: http?.ota_state ?? null, http: http ? (http.error || 'ok') : null,
      csiRateHz: n?.csiRateHz ?? 0, csiLossFraction: n?.csiLossFraction ?? null, rssiMean: n?.rssiMean ?? null,
      health, problems: judged?.problems ?? [],
    });
  }
  const bad = rows.filter((r) => !['healthy', 'unlisted'].includes(r.health));
  return {
    ok: bad.length === 0,
    listen: cap.listen, seconds: cap.seconds, nodes: rows,
    summary: `${rows.filter((r) => r.health === 'healthy').length}/${rows.filter((r) => r.health !== 'unlisted').length} known nodes healthy (min ${config.validate.min_pps} Hz${config.validate.max_loss !== null ? `, max loss ${config.validate.max_loss * 100}%` : ''})`,
    notes: ['no-csi-here = node answers HTTP but streams to another host', 'unlisted = node heard on UDP but neither in the node config nor assigned by `node apply`'],
    evidence: cap.packets ? 'MEASURED: live UDP CSI decoded on this host' : null,
  };
}

/** Terminal rendering (JSON is the default for pipes). */
export function formatNodes(kind, res) {
  const lines = [];
  const table = (rows, cols) => {
    const w = cols.map(([h, f]) => Math.max(h.length, ...rows.map((r) => String(f(r) ?? '-').length)));
    lines.push(`  ${cols.map(([h], i) => h.padEnd(w[i])).join('  ')}`);
    for (const r of rows) lines.push(`  ${cols.map(([, f], i) => String(f(r) ?? '-').padEnd(w[i])).join('  ')}`);
  };
  if (!res.ok && res.reason && !res.nodes) {
    lines.push(`✘ ${res.reason}${res.detail ? `: ${res.detail}` : ''}`);
    for (const e of res.errors || []) lines.push(`  - ${e}`);
    if (res.remedy) lines.push(`  → ${res.remedy}`);
    return lines.join('\n');
  }
  if (kind === 'plan') {
    lines.push(`Node plan — ${res.config}`, `  WiFi: ${res.ssid ?? '(none)'} · target ${res.target ?? '?'} · steps ${res.steps.join(',')}`);
    lines.push(`  Secrets: wifi ${res.secrets.wifi_password}; ota ${res.secrets.ota_psk}`);
    lines.push(res.firmware.error ? `  Firmware: ✘ ${res.firmware.error} — ${res.firmware.detail}` : `  Firmware: ${res.firmware.kind} ${res.firmware.dir} (${res.firmware.chip} ${res.firmware.flashSize}; ${res.firmware.images.map((i) => `${i.role}:${i.integrity}`).join(', ')})`);
    lines.push(`  Validate: wait ${res.validate.wait_secs}s, measure ${res.validate.udp_secs}s, min ${res.validate.min_pps} Hz${res.validate.max_loss !== null ? `, max loss ${res.validate.max_loss}` : ''}`, '');
    table(res.nodes, [['NODE', (r) => r.node_id], ['MAC', (r) => r.mac], ['ZONE', (r) => r.zone], ['TDM', (r) => r.tdm], ['TIER', (r) => r.edge_tier]]);
    for (const w of res.warnings) lines.push(`  ! ${w}`);
  } else if (kind === 'status') {
    table(res.nodes, [['NODE', (r) => r.node_id], ['ZONE', (r) => r.zone], ['MAC', (r) => r.mac], ['IP', (r) => r.address], ['FW', (r) => r.firmware], ['HTTP', (r) => r.http], ['CSI Hz', (r) => r.csiRateHz], ['LOSS', (r) => (r.csiLossFraction === null ? null : `${(r.csiLossFraction * 100).toFixed(1)}%`)], ['RSSI', (r) => r.rssiMean], ['HEALTH', (r) => r.health]]);
    lines.push('', `  ${res.summary}`);
    for (const r of res.nodes) for (const p of r.problems) lines.push(`  ! node ${r.node_id}: ${p}`);
  } else {
    lines.push(`${res.ok ? '✔' : '✘'} ${res.flow} ${res.port} · MAC ${res.mac ?? '?'} · node ${res.node_id ?? '?'}${res.zone ? ` (${res.zone})` : ''} [${res.node_source ?? '-'}]`);
    if (res.dryRun) { lines.push(JSON.stringify(res.plan, null, 2).split('\n').map((l) => `  ${l}`).join('\n')); if (res.next) lines.push(`  → ${res.next}`); return lines.join('\n'); }
    for (const [stage, s] of Object.entries(res.stages || {})) {
      const extra = stage === 'flash' ? `fw ${s.firmwareVersion ?? '?'}` : stage === 'wifi' ? `→ ${s.target} (OTA ${s.ota ? 'on' : 'off'})` : `${s.csiRateHz ?? 0} Hz, loss ${s.csiLossFraction === null || s.csiLossFraction === undefined ? '?' : `${(s.csiLossFraction * 100).toFixed(1)}%`}, RSSI ${s.rssiMean ?? '?'}, ip ${s.address ?? '?'}`;
      lines.push(`  ${s.ok ? '✔' : '✘'} ${stage}: ${extra}`);
    }
    if (!res.ok) lines.push(`  ✘ ${res.reason}${res.detail ? `: ${res.detail}` : ''}`);
  }
  return lines.join('\n');
}
