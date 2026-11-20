// SPDX-License-Identifier: MIT
// ESPHome native API reader (ADR-373 Amendment 3). Radar kits such as the
// Seeed MR60BHA2 (XIAO ESP32-C6 + 60 GHz radar) ship ESPHome firmware that owns
// the radar UART, so no raw frames reach USB. Their values are published
// through ESPHome's native API on TCP 6053.
//
// This is a dependency-free, read-only client for the plaintext protocol:
// hello → connect → device info → list entities → subscribe states, for a
// bounded window. It never sends a command to the device. Only private,
// link-local and loopback addresses are accepted. Encrypted APIs (Noise) are
// reported honestly as unsupported.

import net from 'node:net';
import { lookup } from 'node:dns/promises';

export const ESPHOME_DEFAULT_PORT = 6053;
const HOST_RE = /^(?=.{1,253}$)[A-Za-z0-9](?:[A-Za-z0-9-]{0,62})(?:\.[A-Za-z0-9](?:[A-Za-z0-9-]{0,62}))*\.?$/;
const MAX_BUFFER = 1024 * 1024;

// Message type ids (api.proto).
const T = Object.freeze({
  HELLO_REQ: 1, HELLO_RES: 2, CONNECT_REQ: 3, CONNECT_RES: 4, DISCONNECT_REQ: 5, DISCONNECT_RES: 6,
  PING_REQ: 7, PING_RES: 8, DEVICE_INFO_REQ: 9, DEVICE_INFO_RES: 10, LIST_REQ: 11,
  LIST_BINARY: 12, LIST_SENSOR: 16, LIST_TEXT: 18, LIST_DONE: 19, SUBSCRIBE_STATES: 20,
  STATE_BINARY: 21, STATE_SENSOR: 25, STATE_TEXT: 27,
});

// --- protobuf (proto3) primitives -------------------------------------------
export function encodeVarint(n) {
  const out = [];
  let v = n >>> 0;
  do { let b = v & 0x7f; v >>>= 7; if (v) b |= 0x80; out.push(b); } while (v);
  return Buffer.from(out);
}

export function readVarint(buf, offset) {
  let result = 0;
  let shift = 0;
  let i = offset;
  for (;;) {
    if (i >= buf.length) return null;
    const b = buf[i++];
    result += (b & 0x7f) * 2 ** shift;
    if (!(b & 0x80)) return [result, i];
    shift += 7;
    if (shift > 63) throw new Error('varint too long');
  }
}

/** Decode a protobuf message into { field: [{wt, v}] } (wire types 0, 1, 2, 5). */
export function decodeFields(buf) {
  const fields = {};
  let o = 0;
  while (o < buf.length) {
    const tag = readVarint(buf, o);
    if (!tag) throw new Error('truncated tag');
    o = tag[1];
    const field = Math.floor(tag[0] / 8);
    const wt = tag[0] & 7;
    let v;
    if (wt === 0) { const r = readVarint(buf, o); if (!r) throw new Error('truncated varint'); [v, o] = r; }
    else if (wt === 2) {
      const r = readVarint(buf, o); if (!r) throw new Error('truncated length');
      if (r[1] + r[0] > buf.length) throw new Error('truncated bytes');
      v = buf.subarray(r[1], r[1] + r[0]); o = r[1] + r[0];
    } else if (wt === 5) { if (o + 4 > buf.length) throw new Error('truncated fixed32'); v = buf.subarray(o, o + 4); o += 4; }
    else if (wt === 1) { if (o + 8 > buf.length) throw new Error('truncated fixed64'); v = buf.subarray(o, o + 8); o += 8; }
    else throw new Error(`unsupported wire type ${wt}`);
    (fields[field] ??= []).push({ wt, v });
  }
  return fields;
}

const fStr = (f, n) => (f[n]?.[0]?.wt === 2 ? f[n][0].v.toString('utf8') : '');
const fKey = (f, n) => (f[n]?.[0]?.wt === 5 ? f[n][0].v.readUInt32LE(0) : null);
const fBool = (f, n) => (f[n]?.[0]?.wt === 0 ? f[n][0].v !== 0 : false);
const fFloat = (f, n) => (f[n]?.[0]?.wt === 5 ? f[n][0].v.readFloatLE(0) : 0); // proto3: absent float is 0
const fUint = (f, n) => (f[n]?.[0]?.wt === 0 ? f[n][0].v : 0);

const pbString = (field, s) => { const b = Buffer.from(s, 'utf8'); return Buffer.concat([encodeVarint((field << 3) | 2), encodeVarint(b.length), b]); };
const pbUint = (field, n) => Buffer.concat([encodeVarint(field << 3), encodeVarint(n)]);

/** Plaintext frame: 0x00, varint(length), varint(type), payload. */
export function encodeFrame(type, payload = Buffer.alloc(0)) {
  return Buffer.concat([Buffer.from([0]), encodeVarint(payload.length), encodeVarint(type), payload]);
}

/** Split complete frames off a buffer. Returns { frames, rest } or throws on an encrypted stream. */
export function splitFrames(buf) {
  const frames = [];
  let o = 0;
  while (o < buf.length) {
    if (buf[o] === 0x01) { const e = new Error('encrypted'); e.reason = 'esphome_encrypted'; throw e; }
    if (buf[o] !== 0x00) { const e = new Error(`bad frame indicator 0x${buf[o].toString(16)}`); e.reason = 'esphome_protocol_error'; throw e; }
    const len = readVarint(buf, o + 1);
    if (!len) break;
    const type = readVarint(buf, len[1]);
    if (!type) break;
    if (type[1] + len[0] > buf.length) break;
    frames.push({ type: type[0], payload: buf.subarray(type[1], type[1] + len[0]) });
    o = type[1] + len[0];
  }
  return { frames, rest: buf.subarray(o) };
}

// --- address policy ------------------------------------------------------------
export function isPrivateAddress(ip) {
  if (net.isIPv4(ip)) {
    const [a, b] = ip.split('.').map(Number);
    return a === 10 || a === 127 || (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168) || (a === 169 && b === 254) || (a === 100 && b >= 64 && b <= 127);
  }
  if (net.isIPv6(ip)) {
    const v = ip.toLowerCase();
    if (v.startsWith('::ffff:')) return isPrivateAddress(v.slice(7));
    return v === '::1' || v.startsWith('fc') || v.startsWith('fd') || v.startsWith('fe8') || v.startsWith('fe9') || v.startsWith('fea') || v.startsWith('feb');
  }
  return false;
}

async function resolveHost(host, deps) {
  if (typeof host !== 'string' || !(net.isIP(host) || HOST_RE.test(host))) {
    return { ok: false, reason: 'invalid_host', detail: 'host must be an IP address or a hostname' };
  }
  let address = host;
  if (!net.isIP(host)) {
    try { address = (await (deps.lookup || lookup)(host)).address; } catch (error) {
      return { ok: false, reason: 'host_unresolved', detail: `${host}: ${error.code || error.message}`, remedy: 'Use the device IP (the router DHCP table, or `arp -a`), or its mDNS name <name>.local.' };
    }
  }
  if (!isPrivateAddress(address)) {
    return { ok: false, reason: 'host_not_private', detail: `${host} resolves to ${address}`, remedy: 'ESPHome reads are limited to private, link-local, CGNAT/Tailscale and loopback addresses.' };
  }
  return { ok: true, address };
}

// --- radar roles -----------------------------------------------------------------
const ROLES = [
  ['heartBpm', (e) => e.kind === 'sensor' && /heart/i.test(e.name + e.objectId)],
  ['breathingBpm', (e) => e.kind === 'sensor' && /respir|breath/i.test(e.name + e.objectId)],
  ['distanceCm', (e) => e.kind === 'sensor' && /distance/i.test(e.name + e.objectId)],
  ['targets', (e) => /target/i.test(e.name + e.objectId) && e.kind === 'sensor'],
  ['present', (e) => e.kind === 'binary' && /person|presence|occup|motion|human/i.test(e.name + e.objectId)],
];
const roleOf = (e) => (ROLES.find(([, test]) => test(e)) || [null])[0];
const mean = (xs) => (xs.length ? Number((xs.reduce((a, b) => a + b, 0) / xs.length).toFixed(2)) : null);

/** Summarize a session into the mmWave result shape (shared with the serial path). */
export function summarizeEsphome(session, seconds) {
  const entities = [...session.entities.values()];
  const byRole = {};
  for (const e of entities) { const r = roleOf(e); if (r && !byRole[r]) byRole[r] = e; }
  const values = (e) => (e ? e.samples.filter((s) => !s.missing).map((s) => s.value) : []);
  const presence = values(byRole.present);
  const toCm = (e) => {
    const unit = (e?.unit || '').toLowerCase();
    const scale = unit === 'm' ? 100 : unit === 'mm' ? 0.1 : 1;
    return values(e).map((value) => value * scale);
  };
  return {
    source: 'esphome',
    device: session.device,
    seconds,
    stateUpdates: session.updates,
    updateRateHz: Number((session.updates / seconds).toFixed(2)),
    presentFraction: presence.length ? Number((presence.filter(Boolean).length / presence.length).toFixed(3)) : null,
    presentNow: byRole.present && byRole.present.last && !byRole.present.last.missing ? Boolean(byRole.present.last.value) : null,
    heartBpmMean: mean(values(byRole.heartBpm).filter((v) => v > 0 && v <= 250)),
    breathingBpmMean: mean(values(byRole.breathingBpm).filter((v) => v > 0 && v <= 60)),
    distanceCmMean: mean(toCm(byRole.distanceCm).filter((v) => v > 0)),
    targetsMax: values(byRole.targets).length ? Math.max(...values(byRole.targets)) : null,
    entities: entities.map((e) => ({
      kind: e.kind, name: e.name, unit: e.unit || undefined, role: roleOf(e) || undefined,
      updates: e.updates ?? e.samples.length, sampledUpdates: e.samples.length, droppedSamples: Math.max(0, (e.updates ?? e.samples.length) - e.samples.length), last: e.last ? (e.last.missing ? null : e.last.value) : null,
    })),
  };
}

/**
 * Read an ESPHome device. args: { host, api_port=6053, seconds=10 }.
 * deps.connect(port, host) → net.Socket and deps.lookup are injectable for tests.
 */
export async function readEsphome(args = {}, deps = {}) {
  const seconds = Math.min(Math.max(args.seconds ?? 10, 1), 120);
  const port = args.api_port ?? ESPHOME_DEFAULT_PORT;
  if (!Number.isInteger(port) || port < 1 || port > 65535) return { ok: false, reason: 'invalid_port', detail: 'api_port must be 1..65535' };
  const target = await resolveHost(args.host, deps);
  if (!target.ok) return target;

  const session = { device: {}, entities: new Map(), updates: 0, hello: false };
  return new Promise((resolve) => {
    const socket = (deps.connect || ((p, h) => net.connect(p, h)))(port, target.address);
    let buf = Buffer.alloc(0);
    let done = false;
    const finish = (out) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      clearTimeout(connectTimer);
      try { socket.write(encodeFrame(T.DISCONNECT_REQ)); } catch { /* closing anyway */ }
      socket.destroy();
      resolve(out);
    };
    const fail = (reason, detail, remedy) => finish({ ok: false, reason, detail, ...(remedy ? { remedy } : {}), host: args.host, port });
    const connectTimer = setTimeout(() => fail('esphome_unreachable', `no TCP connection to ${target.address}:${port} within 5 s`, 'Check that the device is on the network and that `api:` is enabled in its ESPHome config.'), 5000);
    let timer = null;

    socket.on('connect', () => {
      clearTimeout(connectTimer);
      socket.write(encodeFrame(T.HELLO_REQ, Buffer.concat([pbString(1, 'ruview'), pbUint(2, 1), pbUint(3, 10)])));
      socket.write(encodeFrame(T.CONNECT_REQ));
      socket.write(encodeFrame(T.DEVICE_INFO_REQ));
      socket.write(encodeFrame(T.LIST_REQ));
      timer = setTimeout(() => {
        if (!session.hello) return fail('esphome_protocol_error', 'no HelloResponse from the device');
        const summary = summarizeEsphome(session, seconds);
        finish({
          ok: true, host: args.host, port, ...summary,
          evidence: 'MEASURED: device-reported radar values read on this host (not an accuracy claim)',
          note: 'Values are computed by the device firmware and are not validated against a reference.',
        });
      }, seconds * 1000);
    });
    socket.on('error', (error) => fail('esphome_unreachable', `${target.address}:${port}: ${error.code || error.message}`, 'Check that the device is on the network and that `api:` is enabled in its ESPHome config.'));
    socket.on('close', () => { if (!done && session.hello) fail('esphome_closed', 'the device closed the API connection early'); else if (!done) fail('esphome_unreachable', 'connection closed before the API handshake'); });
    socket.on('data', (chunk) => {
      buf = Buffer.concat([buf, chunk]);
      if (buf.length > MAX_BUFFER) return fail('esphome_protocol_error', 'response exceeded 1 MiB');
      let frames;
      try { ({ frames, rest: buf } = splitFrames(buf)); } catch (error) {
        if (error.reason === 'esphome_encrypted') return fail('esphome_encrypted', 'the device requires the encrypted (Noise) API', 'API encryption is not supported yet: read the value over USB logs, or use a build without `api: encryption:` on a trusted network.');
        return fail(error.reason || 'esphome_protocol_error', error.message);
      }
      for (const { type, payload } of frames) {
        let f;
        try { f = decodeFields(payload); } catch (error) { return fail('esphome_protocol_error', error.message); }
        switch (type) {
          case T.HELLO_RES:
            session.hello = true;
            session.device.apiVersion = `${fUint(f, 1)}.${fUint(f, 2)}`;
            session.device.serverInfo = fStr(f, 3);
            session.device.name = fStr(f, 4) || undefined;
            break;
          case T.CONNECT_RES:
            if (fBool(f, 1)) return fail('esphome_password_required', 'the device API requires a password', 'Password-protected ESPHome APIs are not supported; remove `api: password:` or use the USB log.');
            break;
          case T.DEVICE_INFO_RES:
            Object.assign(session.device, {
              name: fStr(f, 2) || session.device.name, esphomeVersion: fStr(f, 4) || undefined, compiled: fStr(f, 5) || undefined,
              model: fStr(f, 6) || undefined, project: fStr(f, 8) || undefined, projectVersion: fStr(f, 9) || undefined,
            });
            break;
          case T.PING_REQ:
            socket.write(encodeFrame(T.PING_RES));
            break;
          case T.LIST_BINARY: case T.LIST_SENSOR: case T.LIST_TEXT: {
            const key = fKey(f, 2);
            if (key !== null) session.entities.set(key, { kind: type === T.LIST_BINARY ? 'binary' : type === T.LIST_SENSOR ? 'sensor' : 'text', objectId: fStr(f, 1), name: fStr(f, 3), unit: type === T.LIST_SENSOR ? fStr(f, 6) : '', samples: [], last: null, updates: 0 });
            break;
          }
          case T.LIST_DONE:
            socket.write(encodeFrame(T.SUBSCRIBE_STATES));
            break;
          case T.STATE_BINARY: case T.STATE_SENSOR: case T.STATE_TEXT: {
            const e = session.entities.get(fKey(f, 1));
            if (!e) break;
            const value = type === T.STATE_SENSOR ? fFloat(f, 2) : type === T.STATE_BINARY ? fBool(f, 2) : fStr(f, 2);
            const sample = { value, missing: fBool(f, 3) || (type === T.STATE_SENSOR && !Number.isFinite(value)) };
            session.updates += 1;
            e.updates += 1;
            if (e.samples.length < 10_000) e.samples.push(sample);
            e.last = sample;
            break;
          }
          default: break; // lights, switches, etc. are ignored (read-only client)
        }
      }
    });
  });
}
