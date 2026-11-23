// SPDX-License-Identifier: MIT
// mmWave radar access (ADR-373): Seeed MR60BHA2 (60 GHz, 115200 baud) and
// HLK-LD2410 (24 GHz, 256000 baud) over a USB-UART adapter. The parsers are
// byte-for-byte ports of firmware/esp32-csi-node/main/mmwave_sensor.c so the
// host and the node agree on what a valid frame is.

import { readSerial } from './serial-pump.js';

export const MMWAVE_MODELS = Object.freeze({
  mr60bha2: Object.freeze({ baud: 115200, band: '60 GHz', vendor: 'Seeed' }),
  ld2410: Object.freeze({ baud: 256000, band: '24 GHz', vendor: 'HLK' }),
});

const MR60 = Object.freeze({ SOF: 0x01, MAX_PAYLOAD: 30, BREATHING: 0x0a14, HEART: 0x0a15, DISTANCE: 0x0a16, PRESENCE: 0x0f09, PHASE: 0x0a13, POINTCLOUD: 0x0a04 });

/** ~XOR checksum used by the Seeed protocol. */
export function mr60Checksum(bytes) {
  let x = 0;
  for (const b of bytes) x ^= b;
  return (~x) & 0xff;
}

/** Streaming MR60BHA2 parser. feed() returns decoded frames. */
export class Mr60Parser {
  constructor() { this.reset(); this.errors = 0; }
  reset() { this.state = 'sof'; this.header = []; this.data = []; }
  feed(bytes) {
    const frames = [];
    for (const b of bytes) {
      if (this.state === 'sof') {
        if (b === MR60.SOF) { this.header = [b]; this.state = 'header'; }
      } else if (this.state === 'header') {
        this.header.push(b);
        if (this.header.length === 8) {
          if (mr60Checksum(this.header.slice(0, 7)) !== this.header[7]) { this.errors += 1; this.reset(); continue; }
          this.len = (this.header[3] << 8) | this.header[4];
          this.type = (this.header[5] << 8) | this.header[6];
          this.data = [];
          if (this.len > MR60.MAX_PAYLOAD) { this.errors += 1; this.reset(); } else this.state = this.len === 0 ? 'cksum' : 'data';
        }
      } else if (this.state === 'data') {
        this.data.push(b);
        if (this.data.length >= this.len) this.state = 'cksum';
      } else {
        if (this.len === 0 || mr60Checksum(this.data) === b) frames.push(decodeMr60(this.type, Buffer.from(this.data)));
        else this.errors += 1;
        this.reset();
      }
    }
    return frames;
  }
}

function decodeMr60(type, d) {
  switch (type) {
    case MR60.BREATHING: {
      const v = d.length >= 4 ? d.readFloatLE(0) : NaN;
      return { type: 'breathing', breathingBpm: v >= 0 && v <= 60 ? v : null };
    }
    case MR60.HEART: {
      const v = d.length >= 4 ? d.readFloatLE(0) : NaN;
      return { type: 'heart', heartBpm: v >= 0 && v <= 250 ? v : null };
    }
    case MR60.DISTANCE:
      return d.length >= 8 && d.readUInt32LE(0) !== 0 ? { type: 'distance', distanceCm: d.readFloatLE(4) } : { type: 'distance', distanceCm: null };
    case MR60.PRESENCE:
      return { type: 'presence', present: d.length >= 1 ? d[0] !== 0 : null };
    case MR60.PHASE: return { type: 'phase', bytes: d.length };
    case MR60.POINTCLOUD: return { type: 'pointcloud', bytes: d.length };
    default: return { type: `0x${type.toString(16).padStart(4, '0')}`, bytes: d.length };
  }
}

const LD_HEAD = [0xf4, 0xf3, 0xf2, 0xf1];
const LD_TAIL = [0xf8, 0xf7, 0xf6, 0xf5];

/** Streaming HLK-LD2410 report parser. */
export class Ld2410Parser {
  constructor() { this.state = 0; this.errors = 0; this.data = []; }
  feed(bytes) {
    const frames = [];
    for (const b of bytes) {
      if (this.state < 4) {
        this.state = b === LD_HEAD[this.state] ? this.state + 1 : (b === LD_HEAD[0] ? 1 : 0);
      } else if (this.state === 4) { this.len = b; this.state = 5; }
      else if (this.state === 5) {
        this.len |= b << 8;
        this.data = [];
        this.state = this.len === 0 || this.len > 256 ? 0 : 6;
      } else if (this.state === 6) {
        this.data.push(b);
        if (this.data.length >= this.len) this.state = 7;
      } else {
        const i = this.state - 7;
        if (b !== LD_TAIL[i]) { this.errors += 1; this.state = 0; continue; }
        if (i === 3) { const f = decodeLd2410(this.data); if (f) frames.push(f); this.state = 0; } else this.state += 1;
      }
    }
    return frames;
  }
}

function decodeLd2410(d) {
  if (d.length < 12 || d[1] !== 0xaa) return null;
  const target = d[2];
  const moving = d[3] | (d[4] << 8);
  const stat = d[6] | (d[7] << 8);
  return {
    type: 'target',
    mode: d[0] === 0x01 ? 'engineering' : 'basic',
    targetState: ['none', 'moving', 'static', 'moving+static'][target] ?? `0x${target.toString(16)}`,
    present: target !== 0,
    movingCm: moving, movingEnergy: d[5], staticCm: stat, staticEnergy: d[8], detectCm: d[9] | (d[10] << 8),
    distanceCm: target === 1 || target === 3 ? moving : target === 2 ? stat : 0,
  };
}

const mean = (xs) => (xs.length ? Number((xs.reduce((a, b) => a + b, 0) / xs.length).toFixed(2)) : null);

/** Summarize decoded frames from either model. */
export function summarizeMmwave(model, frames, errors, bytes, seconds) {
  const pick = (key, include = () => true) => frames.filter(include).map((f) => f[key]).filter((v) => typeof v === 'number' && Number.isFinite(v));
  const presence = frames.filter((f) => typeof f.present === 'boolean');
  const types = {};
  for (const f of frames) types[f.type] = (types[f.type] || 0) + 1;
  return {
    model, band: MMWAVE_MODELS[model].band, bytes, frames: frames.length, checksumErrors: errors,
    frameRateHz: Number((frames.length / seconds).toFixed(2)), frameTypes: types,
    presentFraction: presence.length ? Number((presence.filter((f) => f.present).length / presence.length).toFixed(3)) : null,
    breathingBpmMean: mean(pick('breathingBpm')),
    heartBpmMean: mean(pick('heartBpm')),
    distanceCmMean: mean(pick('distanceCm', (f) => f.type !== 'target' || f.present !== false)),
    last: frames.slice(-5),
  };
}

export function parseMmwave(model, bytes) {
  const parser = model === 'ld2410' ? new Ld2410Parser() : new Mr60Parser();
  const frames = parser.feed(bytes);
  return { frames, errors: parser.errors };
}

/**
 * Read an mmWave radar. args: { port, model: auto|mr60bha2|ld2410, seconds }.
 * Auto-detect probes each model's baud for up to 2 s; the probe's frames are
 * kept and only the remainder of the window is read afterwards, so a request
 * for N seconds costs about N seconds rather than probe + N.
 */
export async function readMmwave(args, deps) {
  const seconds = args.seconds ?? 10;
  const auto = !args.model || args.model === 'auto';
  const models = auto ? ['mr60bha2', 'ld2410'] : [args.model];
  const probeSeconds = auto ? Math.min(seconds, 2) : seconds;
  const attempts = [];
  const capture = async (model, secs) => {
    try {
      return await readSerial({ port: args.port, baud: MMWAVE_MODELS[model].baud, seconds: secs }, deps);
    } catch (error) {
      return { ok: false, reason: error.reason || 'invalid_arguments', detail: error.message };
    }
  };
  for (const model of models) {
    const cap = await capture(model, probeSeconds);
    if (!cap.ok) return { ok: false, reason: cap.reason, detail: cap.detail, remedy: cap.remedy };
    let { frames, errors } = parseMmwave(model, cap.bytes);
    attempts.push({ model, bytes: cap.bytes.length, frames: frames.length, errors });
    if (frames.length < (auto ? 2 : 1)) continue;
    let bytes = cap.bytes.length;
    let elapsed = cap.seconds;
    let captureIncomplete = Boolean(cap.captureIncomplete);
    const warnings = [...(cap.warnings || [])];
    const remaining = seconds - probeSeconds;
    if (auto && remaining >= 0.5) {
      const more = await capture(model, remaining);
      if (more.ok) {
        // A fresh parser: the probe may have ended mid-frame.
        const extra = parseMmwave(model, more.bytes);
        frames = frames.concat(extra.frames);
        errors += extra.errors;
        bytes += more.bytes.length;
        elapsed += more.seconds;
        captureIncomplete ||= Boolean(more.captureIncomplete);
        warnings.push(...(more.warnings || []));
      } else {
        captureIncomplete = true;
        warnings.push('remaining serial capture failed; only probe frames retained');
      }
    }
    return {
      ok: true,
      ...summarizeMmwave(model, frames, errors, bytes, elapsed),
      ...(auto ? { detected: model } : {}),
      ...(captureIncomplete ? { captureIncomplete: true } : {}),
      ...(warnings.length ? { warnings } : {}),
      evidence: 'MEASURED: device-reported radar values read on this host (not an accuracy claim)',
    };
  }
  return {
    ok: false, reason: 'no_valid_frames', attempts,
    remedy: 'Check TX/RX wiring (crossed), 5 V power, and model/baud (MR60BHA2 115200, LD2410 256000). A C6+MR60 board streams through ESPHome/firmware instead — use its serial log or the node UDP stream.',
  };
}
