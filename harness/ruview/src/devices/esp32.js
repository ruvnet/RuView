// SPDX-License-Identifier: MIT
// CSI node network access (ADR-373): receive the node UDP stream and summarize
// it. Decodes ESP32 ADR-018 frames and edge packets, ADR-110 sync packets, and
// Realtek RTL8721Dx RAC1 CSI envelopes plus their RHB1 heartbeats (ADR-323).
// Receive-only: the socket never sends a byte to a node.

import dgram from 'node:dgram';

/** ESP32 packet magics (little-endian u32) from firmware/esp32-csi-node/main. */
export const PACKET_KINDS = Object.freeze({
  0xC5110001: 'csi',
  0xC5110002: 'vitals',
  0xC5110003: 'features',
  0xC5110004: 'fused-vitals',
  0xC5110005: 'compressed',
  0xC5110006: 'feature-state',
  0xC5110007: 'wasm-output',
  0xC511A110: 'sync',
});

/** ADR-081 mesh envelope (rv_mesh.h): 16-byte header, no one-byte node id. */
export const MESH_MAGIC = 0xC5118100;
const MESH_TYPES = Object.freeze({
  1: 'time-sync', 2: 'role-assign', 3: 'channel-plan', 4: 'calibration-start',
  5: 'feature-delta', 6: 'health', 7: 'anomaly-alert',
});

/** Realtek RTL8721Dx magics (ADR-323): ASCII "RAC1" and "RHB1" read as LE u32. */
export const RAC1_MAGIC = 0x31434152;
export const RHB1_MAGIC = 0x31424852;
const RAC1_HEADER_LEN = 49;

const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xEDB88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

/** CRC-32/IEEE, the RAC1 trailer checksum. */
export function crc32(buf, end = buf.length) {
  let c = 0xFFFFFFFF;
  for (let i = 0; i < end; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xFF] ^ (c >>> 8);
  return (c ^ 0xFFFFFFFF) >>> 0;
}

/**
 * Mean |IQ| over int8 pairs, and optionally the per-tone amplitude/phase arrays.
 * `imagFirst` matches the ESP-IDF buffer order (imag, real); Realtek is (I, Q).
 */
function iq8(buf, offset, tones, imagFirst, wantIq) {
  const v = new Int8Array(buf.buffer, buf.byteOffset + offset, tones * 2);
  let sum = 0;
  const amplitudes = wantIq ? new Array(tones) : null;
  const phases = wantIq ? new Array(tones) : null;
  for (let k = 0, j = 0; k < tones; k++, j += 2) {
    const re = imagFirst ? v[j + 1] : v[j];
    const im = imagFirst ? v[j] : v[j + 1];
    const a = Math.sqrt(re * re + im * im);
    sum += a;
    if (wantIq) { amplitudes[k] = a; phases[k] = Math.atan2(im, re); }
  }
  return { mean: sum / tones, amplitudes, phases };
}

function iq16(buf, offset, tones, wantIq) {
  let sum = 0;
  const amplitudes = wantIq ? new Array(tones) : null;
  const phases = wantIq ? new Array(tones) : null;
  for (let k = 0; k < tones; k++) {
    const re = buf.readInt16LE(offset + 4 * k);
    const im = buf.readInt16LE(offset + 4 * k + 2);
    const a = Math.sqrt(re * re + im * im);
    sum += a;
    if (wantIq) { amplitudes[k] = a; phases[k] = Math.atan2(im, re); }
  }
  return { mean: sum / tones, amplitudes, phases };
}

function parseRac1(buf, wantIq) {
  if (buf.length < RAC1_HEADER_LEN + 4) return { kind: 'malformed', reason: 'rac1 header' };
  const version = buf.readUInt8(4);
  const headerLen = buf.readUInt16LE(5);
  const frameLen = buf.readUInt32LE(7);
  if (version !== 1 || headerLen !== RAC1_HEADER_LEN) return { kind: 'malformed', reason: 'rac1 version' };
  if (frameLen !== buf.length) return { kind: 'malformed', reason: 'rac1 length' };
  const tones = buf.readUInt16LE(37);
  const bitsPerTone = buf.readUInt8(39);
  const payloadLen = buf.readUInt32LE(45);
  if ((bitsPerTone !== 16 && bitsPerTone !== 32) || tones === 0 || payloadLen !== tones * (bitsPerTone / 8)
    || RAC1_HEADER_LEN + payloadLen + 4 !== frameLen) return { kind: 'malformed', reason: 'rac1 payload' };
  if (crc32(buf, frameLen - 4) !== buf.readUInt32LE(frameLen - 4)) return { kind: 'malformed', reason: 'rac1 crc' };
  const iq = bitsPerTone === 16 ? iq8(buf, RAC1_HEADER_LEN, tones, false, wantIq) : iq16(buf, RAC1_HEADER_LEN, tones, wantIq);
  const flags = buf.readUInt8(44);
  return {
    kind: 'csi', source: 'realtek', nodeId: buf.readUInt8(11), antennas: 1, subcarriers: tones,
    channel: buf.readUInt8(33), seq: buf.readUInt32LE(13), rssi: buf.readInt8(41),
    bw40: buf.readUInt8(34) === 1, protocol: buf.readUInt8(36), bitsPerTone,
    csiValid: buf.readUInt8(43) === 1, synthetic: Boolean(flags & 1),
    meanAmplitude: iq.mean, amplitudes: iq.amplitudes, phases: iq.phases,
  };
}

/**
 * Parse one datagram. Unknown or malformed input returns {kind:'unknown'|'malformed'}.
 * opts.iq adds per-tone `amplitudes`/`phases` to CSI packets (used for kernel analysis).
 */
export function parsePacket(buf, opts = {}) {
  if (!Buffer.isBuffer(buf) || buf.length < 4) return { kind: 'malformed', reason: 'short' };
  const magic = buf.readUInt32LE(0);
  if (magic === RAC1_MAGIC) return parseRac1(buf, Boolean(opts.iq));
  if (magic === RHB1_MAGIC) return { kind: 'heartbeat', source: 'realtek', nodeId: null };
  if (magic === MESH_MAGIC) {
    if (buf.length < 16) return { kind: 'malformed', reason: 'mesh header' };
    const type = buf.readUInt8(5);
    return { kind: 'mesh', source: 'esp32', nodeId: null, version: buf.readUInt8(4), msgType: MESH_TYPES[type] || `type-${type}` };
  }
  const kind = PACKET_KINDS[magic];
  if (!kind) return { kind: 'unknown', magic: `0x${magic.toString(16)}` };
  if (kind === 'csi') {
    if (buf.length < 20) return { kind: 'malformed', reason: 'csi header' };
    const antennas = buf.readUInt8(5);
    const subcarriers = buf.readUInt16LE(6);
    const tones = antennas * subcarriers;
    if (antennas === 0 || subcarriers === 0 || buf.length < 20 + tones * 2) return { kind: 'malformed', reason: 'csi payload' };
    const flags = buf.readUInt8(19);
    // Per-tone arrays only for single-antenna frames: the kernel takes one chain.
    const iq = iq8(buf, 20, tones, true, Boolean(opts.iq) && antennas === 1);
    // The waterfall (ADR-378) draws multi-antenna nodes from their first chain:
    // ADR-018 lays tones out antenna-major, so chain 0 is the first `subcarriers`.
    const chain0 = opts.firstChain && antennas > 1 ? iq8(buf, 20, subcarriers, true, true).amplitudes : undefined;
    return {
      kind, source: 'esp32', nodeId: buf.readUInt8(4), antennas, subcarriers,
      freqMhz: buf.readUInt32LE(8), seq: buf.readUInt32LE(12),
      rssi: buf.readInt8(16), noiseFloor: buf.readInt8(17), ppdu: buf.readUInt8(18),
      bw40: Boolean(flags & 1), firstWordZeroed: Boolean(flags & 0x20),
      meanAmplitude: iq.mean, amplitudes: iq.amplitudes, phases: iq.phases,
      ...(chain0 ? { chain0Amplitudes: chain0 } : {}),
    };
  }
  if (kind === 'vitals' || kind === 'fused-vitals') {
    if (buf.length < (kind === 'fused-vitals' ? 48 : 32)) return { kind: 'malformed', reason: `${kind} length` };
    const flags = buf.readUInt8(5);
    return {
      kind, source: 'esp32', nodeId: buf.readUInt8(4),
      presence: Boolean(flags & 1), fall: Boolean(flags & 2), motion: Boolean(flags & 4),
      breathingBpm: buf.readUInt16LE(6) / 100, heartBpm: buf.readUInt32LE(8) / 10000,
      rssi: buf.readInt8(12), persons: buf.readUInt8(13),
      motionEnergy: buf.readFloatLE(16), presenceScore: buf.readFloatLE(20), uptimeMs: buf.readUInt32LE(24),
      ...(kind === 'fused-vitals' ? {
        mmwavePresent: Boolean(flags & 8), mmwaveType: buf.readUInt8(14), fusionConfidence: buf.readUInt8(15),
        mmwaveHeartBpm: buf.readFloatLE(28), mmwaveBreathingBpm: buf.readFloatLE(32), mmwaveDistanceCm: buf.readFloatLE(36),
        mmwaveTargets: buf.readUInt8(40), mmwaveConfidence: buf.readUInt8(41),
      } : {}),
    };
  }
  if (kind === 'sync') {
    if (buf.length < 32) return { kind: 'malformed', reason: 'sync length' };
    return {
      kind, source: 'esp32', nodeId: buf.readUInt8(4), protoVer: buf.readUInt8(5),
      leader: Boolean(buf.readUInt8(6) & 1), highWaterSeq: buf.readUInt32LE(24),
    };
  }
  if (kind === 'features' && buf.length < 48) return { kind: 'malformed', reason: 'features length' };
  if (kind === 'compressed') {
    if (buf.length < 10) return { kind: 'malformed', reason: 'compressed header' };
    const originalLength = buf.readUInt16LE(6);
    const compressedLength = buf.readUInt16LE(8);
    if (!originalLength || !compressedLength || buf.length !== 10 + compressedLength) return { kind: 'malformed', reason: 'compressed payload' };
  }
  if (kind === 'feature-state') {
    if (buf.length !== 60) return { kind: 'malformed', reason: 'feature-state length' };
    if (crc32(buf, 56) !== buf.readUInt32LE(56)) return { kind: 'malformed', reason: 'feature-state crc' };
  }
  if (kind === 'wasm-output') {
    if (buf.length < 8) return { kind: 'malformed', reason: 'wasm-output header' };
    const events = buf.readUInt16LE(6);
    if (events > 16 || buf.length !== 8 + 5 * events) return { kind: 'malformed', reason: 'wasm-output payload' };
  }
  return { kind, source: 'esp32', nodeId: buf.length > 4 ? buf.readUInt8(4) : null, bytes: buf.length };
}

const shapeKey = (pkt) => `${pkt.antennas}x${pkt.subcarriers}`;

/** Jumps larger than this are a counter reset or a stray frame, not loss. */
export const SEQ_WINDOW = 1024;

/**
 * Loss accounting that survives UDP reordering and stray sequence values.
 * A late packet fills the hole it left instead of being re-counted. A single
 * out-of-window value is a stray (observed live on an RTL8721Dx: sporadic
 * seq 0 and +52,835 frames between in-order ones) and leaves the baseline
 * alone; two consecutive frames agreeing on a new range are a counter reset.
 */
function trackSequence(n, seq) {
  if (n.lastSeq === null) { n.lastSeq = seq; return; }
  const d = seq - n.lastSeq;
  if (d >= 1 && d <= SEQ_WINDOW) {
    // Only distinct packets corresponding to counted holes can recover loss.
    // Retain at most the existing reorder window; older gaps remain counted.
    for (const missing of n.missingSeq) {
      if (seq - missing >= SEQ_WINDOW) n.missingSeq.delete(missing);
    }
    for (let missing = n.lastSeq + 1; missing < seq; missing++) n.missingSeq.add(missing);
    n.seqGaps += d - 1;
    n.lastSeq = seq;
    n.pendingSeq = null;
  } else if (d <= 0 && d > -SEQ_WINDOW) {
    n.reordered += 1; // late or duplicate; a late packet fills one counted gap
    if (d < 0 && n.missingSeq.delete(seq)) n.seqGaps -= 1;
  } else if (n.pendingSeq != null && seq - n.pendingSeq >= 1 && seq - n.pendingSeq <= SEQ_WINDOW) {
    n.strays -= 1; // the previous out-of-window frame started a new range
    n.resyncs += 1;
    n.missingSeq.clear();
    n.lastSeq = seq;
    n.pendingSeq = null;
  } else {
    n.strays += 1;
    n.pendingSeq = seq;
  }
}

/** Aggregate parsed packets into a per-node summary. */
export class NodeStats {
  constructor() {
    this.nodes = new Map();
    this.unknown = 0;
    this.unknownMagics = new Map();
    this.malformed = 0;
    this.malformedReasons = new Map();
    this.senders = new Map(); // address → { packets, csi, heartbeats }
    this.mesh = new Map(); // msgType → count
  }

  add(pkt, sender) {
    const s = sender ? (this.senders.get(sender) || { packets: 0, csi: 0, heartbeats: 0 }) : null;
    if (s) { s.packets += 1; this.senders.set(sender, s); }
    if (pkt.kind === 'unknown') {
      this.unknown += 1;
      if (this.unknownMagics.size < 32 || this.unknownMagics.has(pkt.magic)) this.unknownMagics.set(pkt.magic, (this.unknownMagics.get(pkt.magic) || 0) + 1);
      return;
    }
    if (pkt.kind === 'malformed') {
      this.malformed += 1;
      this.malformedReasons.set(pkt.reason, (this.malformedReasons.get(pkt.reason) || 0) + 1);
      return;
    }
    if (pkt.kind === 'heartbeat') { if (s) s.heartbeats += 1; return; }
    if (pkt.kind === 'mesh') { this.mesh.set(pkt.msgType, (this.mesh.get(pkt.msgType) || 0) + 1); return; }
    if (s && pkt.kind === 'csi') s.csi += 1;
    const key = `${pkt.source}:${pkt.nodeId ?? -1}`;
    let n = this.nodes.get(key);
    if (!n) {
      n = { source: pkt.source, nodeId: pkt.nodeId ?? -1, packets: {}, rssiSum: 0, rssiCount: 0, lastSeq: null, pendingSeq: null, seqGaps: 0, missingSeq: new Set(), reordered: 0, strays: 0, resyncs: 0, shapes: new Map(), vitals: null };
      this.nodes.set(key, n);
    }
    n.packets[pkt.kind] = (n.packets[pkt.kind] || 0) + 1;
    if (pkt.kind === 'csi') {
      n.rssiSum += pkt.rssi; n.rssiCount += 1;
      trackSequence(n, pkt.seq);
      const shape = shapeKey(pkt);
      const sh = n.shapes.get(shape) || { count: 0, ampSum: 0, last: null, synthetic: false };
      sh.count += 1; sh.ampSum += pkt.meanAmplitude; sh.last = pkt; sh.synthetic ||= Boolean(pkt.synthetic);
      n.shapes.set(shape, sh);
    } else if (pkt.kind === 'vitals' || pkt.kind === 'fused-vitals') {
      n.rssiSum += pkt.rssi; n.rssiCount += 1;
      const { kind, nodeId, source, ...rest } = pkt;
      n.vitals = rest;
    }
  }

  summary(seconds) {
    const nodes = [...this.nodes.values()]
      .sort((a, b) => (a.source === b.source ? a.nodeId - b.nodeId : a.source.localeCompare(b.source)))
      .map((n) => {
        const csi = n.packets.csi || 0;
        const received = csi + n.seqGaps;
        const shapes = [...n.shapes.entries()].sort((a, b) => b[1].count - a[1].count);
        const describe = ([shape, sh]) => {
          const p = sh.last;
          return {
            shape, frames: sh.count, antennas: p.antennas, subcarriers: p.subcarriers,
            ...(p.source === 'esp32' ? { freqMhz: p.freqMhz, ppdu: p.ppdu } : { channel: p.channel, protocol: p.protocol, bitsPerTone: p.bitsPerTone, synthetic: sh.synthetic }),
            bw40: p.bw40, meanAmplitude: Number((sh.ampSum / sh.count).toFixed(2)),
          };
        };
        return {
          source: n.source,
          nodeId: n.nodeId,
          packets: n.packets,
          csiRateHz: Number((csi / seconds).toFixed(2)),
          csiLossFraction: received ? Number((n.seqGaps / received).toFixed(4)) : null,
          ...(n.reordered ? { seqReordered: n.reordered } : {}),
          ...(n.strays ? { seqStrays: n.strays } : {}),
          ...(n.resyncs ? { seqResyncs: n.resyncs } : {}),
          rssiMean: n.rssiCount ? Number((n.rssiSum / n.rssiCount).toFixed(1)) : null,
          csi: shapes.length ? describe(shapes[0]) : null,
          ...(shapes.length > 1 ? { csiShapes: shapes.map(describe) } : {}),
          lastVitals: n.vitals,
        };
      });
    const senders = [...this.senders.keys()].slice(0, 32);
    const heartbeatOnly = [...this.senders.entries()].filter(([, s]) => s.heartbeats > 0 && s.csi === 0).map(([a, s]) => ({ address: a, heartbeats: s.heartbeats }));
    return {
      nodes,
      unknownPackets: this.unknown,
      ...(this.unknown ? { unknownMagics: Object.fromEntries(this.unknownMagics) } : {}),
      malformedPackets: this.malformed,
      ...(this.malformed ? { malformedReasons: Object.fromEntries(this.malformedReasons) } : {}),
      heartbeats: [...this.senders.values()].reduce((a, s) => a + s.heartbeats, 0),
      ...(this.mesh.size ? { meshMessages: Object.fromEntries(this.mesh) } : {}),
      ...(heartbeatOnly.length ? { heartbeatOnlySenders: heartbeatOnly } : {}),
      senders,
    };
  }
}

/** Bounded per-node frame buffer for kernel analysis (one CSI shape per node). */
class FrameCollector {
  constructor(maxFrames) { this.max = maxFrames; this.byNode = new Map(); }
  add(pkt, t) {
    if (pkt.kind !== 'csi' || !pkt.amplitudes) return;
    const key = `${pkt.source}:${pkt.nodeId}|${shapeKey(pkt)}`;
    let c = this.byNode.get(key);
    if (!c) { c = { source: pkt.source, nodeId: pkt.nodeId, subcarriers: pkt.subcarriers, frames: [], first: t, last: t, dropped: 0, synthetic: false }; this.byNode.set(key, c); }
    if (c.frames.length >= this.max) { c.dropped += 1; return; }
    c.synthetic ||= Boolean(pkt.synthetic);
    c.frames.push({ amplitudes: pkt.amplitudes, phases: pkt.phases });
    c.last = t;
  }
  /** The requested node's (or the busiest node's) dominant shape. */
  pick(nodeId) {
    const all = [...this.byNode.values()].filter((c) => nodeId === undefined || c.nodeId === nodeId);
    return all.sort((a, b) => (b.frames.length + b.dropped) - (a.frames.length + a.dropped))[0] || null;
  }
}

/** Average a frame's per-subcarrier amplitudes into `bins` equal groups. */
export function binAmplitudes(amplitudes, bins) {
  const n = amplitudes.length;
  const b = Math.max(1, Math.min(bins, n));
  const out = new Array(b);
  for (let i = 0; i < b; i++) {
    const start = Math.floor((i * n) / b);
    const end = Math.max(start + 1, Math.floor(((i + 1) * n) / b));
    let sum = 0;
    for (let k = start; k < end; k++) sum += amplitudes[k];
    out[i] = Math.round((sum / (end - start)) * 10) / 10;
  }
  return out;
}

/**
 * The most recent amplitude frames per node, binned, for a waterfall view
 * (ADR-378). A ring buffer: unlike the analyzer's collector it keeps the
 * newest frames. Each node reports its dominant CSI shape only.
 */
export class SpectrumCollector {
  constructor(frames, bins, maxNodes = 8) { this.frames = frames; this.bins = bins; this.maxNodes = maxNodes; this.byKey = new Map(); }
  add(pkt, t) {
    const amplitudes = pkt.amplitudes ?? pkt.chain0Amplitudes;
    if (pkt.kind !== 'csi' || !amplitudes) return;
    const key = `${pkt.source}:${pkt.nodeId}|${shapeKey(pkt)}`;
    let c = this.byKey.get(key);
    if (!c) {
      if (this.byKey.size >= this.maxNodes * 4) return;
      c = { source: pkt.source, nodeId: pkt.nodeId, shape: shapeKey(pkt), subcarriers: pkt.subcarriers, rows: [], count: 0, first: t, last: t, synthetic: false };
      this.byKey.set(key, c);
    }
    c.rows.push(binAmplitudes(amplitudes, this.bins));
    if (c.rows.length > this.frames) c.rows.shift();
    c.count += 1;
    c.last = t;
    c.synthetic ||= Boolean(pkt.synthetic);
  }
  result() {
    const best = new Map();
    for (const c of this.byKey.values()) {
      const node = `${c.source}:${c.nodeId}`;
      if (!best.has(node) || best.get(node).count < c.count) best.set(node, c);
    }
    return [...best.values()]
      .sort((a, b) => (a.source === b.source ? a.nodeId - b.nodeId : a.source.localeCompare(b.source)))
      .slice(0, this.maxNodes)
      .map((c) => ({
        source: c.source, nodeId: c.nodeId, shape: c.shape, subcarriers: c.subcarriers, bins: c.rows[0]?.length ?? 0,
        frames: c.rows, framesSeen: c.count,
        rateHz: c.last > c.first ? Number((((c.count - 1) * 1000) / (c.last - c.first)).toFixed(2)) : null,
        synthetic: c.synthetic,
      }));
  }
}

const BIND_HOSTS = new Set(['0.0.0.0', '127.0.0.1', '::', '::1']);
const NODE_PACKETS = (summary) => summary.nodes.reduce((a, n) => a + Object.values(n.packets).reduce((x, y) => x + y, 0), 0);

/**
 * Listen for node packets. args: { udp_port=5005, bind='0.0.0.0', seconds=10, max_packets=200000,
 * analyze=false, node_id, analyze_max_frames=6000, spectrum=false, spectrum_bins=48, spectrum_frames=64 }.
 * deps.createSocket is injectable for tests;
 * deps.analyze(frames, config) runs the compute kernel when args.analyze is set.
 */
export function captureEsp32(args = {}, deps = {}) {
  const port = args.udp_port ?? 5005;
  const bind = args.bind ?? '0.0.0.0';
  const seconds = Math.min(Math.max(args.seconds ?? 10, 1), 300);
  const maxPackets = Math.min(args.max_packets ?? 200_000, 1_000_000);
  const analyze = Boolean(args.analyze);
  const maxFrames = Math.min(Math.max(args.analyze_max_frames ?? 6000, 64), 20_000);
  const spectrum = Boolean(args.spectrum);
  const spectrumBins = Math.min(Math.max(Math.round(args.spectrum_bins ?? 48), 8), 128);
  const spectrumFrames = Math.min(Math.max(Math.round(args.spectrum_frames ?? 64), 8), 256);
  if (!Number.isInteger(port) || port < 1024 || port > 65535) return Promise.resolve({ ok: false, reason: 'invalid_port', detail: 'udp_port must be an integer in 1024..65535' });
  if (!BIND_HOSTS.has(bind)) return Promise.resolve({ ok: false, reason: 'invalid_bind', detail: `bind must be one of ${[...BIND_HOSTS].join(', ')}` });
  if (args.node_id !== undefined && (!Number.isInteger(args.node_id) || args.node_id < 0 || args.node_id > 255)) return Promise.resolve({ ok: false, reason: 'invalid_node_id', detail: 'node_id must be an integer in 0..255' });
  if (analyze && typeof deps.analyze !== 'function') return Promise.resolve({ ok: false, reason: 'analyze_unavailable', detail: 'no compute kernel bridge was provided' });
  const createSocket = deps.createSocket || ((type) => dgram.createSocket({ type, reuseAddr: false }));
  const stats = new NodeStats();
  const collector = analyze ? new FrameCollector(maxFrames) : null;
  const spectra = spectrum ? new SpectrumCollector(spectrumFrames, spectrumBins) : null;
  const parseOpts = { iq: analyze || spectrum, firstChain: spectrum };
  let packets = 0;
  return new Promise((resolve) => {
    const socket = createSocket(bind.includes(':') ? 'udp6' : 'udp4');
    let timer;
    let startedAt = null;
    let done = false;
    const finish = async (extra = {}) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      try { socket.close(); } catch { /* already closed */ }
      const elapsedSeconds = startedAt === null ? 0 : Math.max((performance.now() - startedAt) / 1000, Number.EPSILON);
      const summary = stats.summary(elapsedSeconds || seconds);
      const csiNodes = summary.nodes.filter((n) => (n.packets.csi || 0) > 0).length;
      const decoded = packets - summary.unknownPackets - summary.malformedPackets;
      let failure = null;
      if (packets === 0) {
        failure = { reason: 'no_packets', remedy: `No node packets reached ${bind}:${port}. Check the node's target IP/port (provision.py --target-ip/--target-port), that this host is on the same network, and the firewall (UDP ${port}).` };
      } else if (NODE_PACKETS(summary) === 0 && !summary.meshMessages && summary.heartbeats > 0) {
        failure = { reason: 'heartbeat_only', remedy: 'Realtek nodes are alive (RHB1 heartbeats) but no RAC1 CSI arrived. Run `ruview monitor --port <p> --baud 1500000`: "lack of csi buf" lines mean CSI report-buffer starvation in the firmware (reset the board); otherwise check csi_sequence and channel contention (ADR-323).' };
      } else if (decoded === 0) {
        failure = { reason: 'no_decodable_packets', remedy: 'Packets arrived but none matched a known RuView format (see unknownMagics). Check the sender and firmware version.' };
      }
      const result = {
        ok: !failure,
        ...(failure || {}),
        listen: `${bind}:${port}`, seconds, elapsedSeconds, packets, decodedPackets: decoded, csiNodes, ...summary,
        evidence: failure ? null : 'MEASURED: live UDP packets received and decoded on this host',
        ...extra,
      };
      if (spectra && !failure) result.spectrum = spectra.result();
      if (collector && !failure) result.analysis = await runAnalysis(collector, args.node_id, deps.analyze);
      resolve(result);
    };
    socket.on('error', (error) => {
      if (error.code === 'EADDRINUSE') {
        done = true;
        clearTimeout(timer);
        try { socket.close(); } catch { /* ignore */ }
        resolve({ ok: false, reason: 'port_in_use', detail: `UDP ${port} is already bound (is the sensing server running?)`, remedy: 'Stop the sensing server or capture on another port the nodes target.' });
      } else finish({ ok: false, reason: 'socket_error', detail: error.message });
    });
    socket.on('message', (msg, rinfo) => {
      if (done) return;
      packets += 1;
      const pkt = parsePacket(msg, parseOpts);
      stats.add(pkt, rinfo?.address);
      if (collector || spectra) {
        const t = performance.now();
        if (collector) collector.add(pkt, t);
        if (spectra) spectra.add(pkt, t);
      }
      if (packets >= maxPackets) finish({ truncated: true });
    });
    socket.bind(port, bind, () => { startedAt = performance.now(); timer = setTimeout(() => finish(), seconds * 1000); });
  });
}

async function runAnalysis(collector, nodeId, analyzeFn) {
  const c = collector.pick(nodeId);
  if (!c || c.frames.length < 2) {
    return { ok: false, reason: 'no_frames', detail: nodeId === undefined ? 'no single-antenna CSI frames were captured' : `no single-antenna CSI frames from node ${nodeId}` };
  }
  const spanSec = (c.last - c.first) / 1000;
  const sampleRateHz = spanSec > 0 ? (c.frames.length - 1) / spanSec : 0;
  const input = { source: c.source, nodeId: c.nodeId, frames: c.frames.length, droppedOverCap: c.dropped, synthetic: c.synthetic, subcarriers: c.subcarriers, sampleRateHz: Number(sampleRateHz.toFixed(3)) };
  if (!(sampleRateHz > 0)) return { ok: false, input, reason: 'no_frames', detail: 'frames arrived without a measurable time span' };
  const config = { n_subcarriers: c.subcarriers, sample_rate_hz: input.sampleRateHz };
  try {
    const out = await analyzeFn(c.frames, config);
    return {
      input, ...out,
      note: c.synthetic
        ? 'Signal-processing estimates include firmware-marked synthetic CSI with no reference measurement; not clinical or camera-grade. The sample rate is the measured arrival rate.'
        : 'Signal-processing estimates from live CSI with no reference measurement; not clinical or camera-grade. The sample rate is the measured arrival rate.',
    };
  } catch (error) {
    return { ok: false, input, reason: error?.code || 'kernel_error', detail: String(error?.message || error) };
  }
}
