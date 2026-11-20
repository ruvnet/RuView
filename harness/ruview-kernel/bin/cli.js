#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// `npx @ruvnet/ruview-kernel` — run the RuView compute kernel from the shell
// (ADR-368). Zero runtime dependencies; WASM is the default backend.

import { closeSync, openSync, readFileSync, readSync, realpathSync, statSync, writeFileSync } from 'node:fs';
import { argv } from 'node:process';
import { fileURLToPath } from 'node:url';
import { MAX_INPUT_BYTES } from '../src/abi.js';
import { BACKENDS, kernelStatus, loadKernel, parity, selfTest } from '../src/index.js';

const pjson = (value) => console.log(JSON.stringify(value, null, 2));

function parseFlags(rest) {
  const flags = {};
  for (let i = 0; i < rest.length; i++) {
    const a = rest[i];
    if (!a.startsWith('--')) continue;
    const eq = a.indexOf('=');
    if (eq !== -1) flags[a.slice(2, eq)] = a.slice(eq + 1);
    else if (i + 1 < rest.length && !rest[i + 1].startsWith('--')) flags[a.slice(2)] = rest[++i];
    else flags[a.slice(2)] = true;
  }
  return flags;
}

function number(flags, key, fallback) {
  if (flags[key] === undefined) return fallback;
  const n = Number(flags[key]);
  if (!Number.isFinite(n)) throw new Error(`--${key} must be a number`);
  return n;
}

/** Bounded read of a caller-named regular file (JSON array/object or JSONL frames). */
function readFrames(path) {
  const stat = statSync(path);
  if (!stat.isFile()) throw new Error(`${path} is not a regular file`);
  if (stat.size > MAX_INPUT_BYTES) throw new Error(`${path} exceeds ${MAX_INPUT_BYTES} bytes`);
  const fd = openSync(path, 'r');
  let text;
  try {
    const buf = Buffer.alloc(stat.size);
    readSync(fd, buf, 0, stat.size, 0);
    text = buf.toString('utf8');
  } finally {
    closeSync(fd);
  }
  const trimmed = text.trim();
  let parsed;
  try {
    parsed = JSON.parse(trimmed);
  } catch {
    // Not a single JSON document: treat as JSONL, one frame per line.
    const frames = trimmed.split('\n').filter((l) => l.trim()).map((l, i) => {
      try { return JSON.parse(l); } catch { throw new Error(`line ${i + 1} is not valid JSON`); }
    });
    return { frames };
  }
  if (Array.isArray(parsed)) return { frames: parsed };
  if (Array.isArray(parsed?.frames)) return { frames: parsed.frames, config: parsed.config };
  if (parsed && Array.isArray(parsed.amplitudes)) return { frames: [parsed] };
  throw new Error('input must be a JSON array of frames, {frames, config}, or JSONL');
}

function help() {
  console.log(`Usage: ruview-kernel <command> [--backend wasm|napi|auto]

  doctor [--strict-napi]          report both backends, ABI, and artifact integrity
  info                            kernel build info and limits
  selftest [--seconds 60]         SYNTHETIC end-to-end pipeline check
  parity                          diff wasm vs napi outputs on the same SYNTHETIC input
  synth [--seconds 60] [--breathing 15] [--heart 72] [--subcarriers 56] [--seed 7] [--out file.json]
  analyze --file frames.json|frames.jsonl [--config '{"n_subcarriers":56}'] [--readings]
  bench [--seconds 120]           time analyze on SYNTHETIC input (MEASURED on this host)

Default backend: RUVIEW_KERNEL_BACKEND or wasm (zero-import, least authority).
Outputs are signal-processing estimates, not clinical or camera-grade measurements.`);
  return 0;
}

export async function run(args) {
  const cmd = args[0] ?? '--help';
  const flags = parseFlags(args.slice(1));
  if (flags.backend !== undefined && !BACKENDS.includes(flags.backend)) {
    console.error(`--backend must be one of ${BACKENDS.join(', ')}`);
    return 2;
  }
  const load = () => loadKernel({ backend: flags.backend });
  switch (cmd) {
    case 'doctor': {
      const status = kernelStatus();
      const ok = status.wasm.available && (!flags['strict-napi'] || status.napi.available);
      pjson({ ok, ...status, note: status.napi.available ? 'both backends available' : 'napi optional; wasm backend active by default' });
      return ok ? 0 : 1;
    }
    case 'info': {
      const k = load();
      pjson({ backend: k.backend, requestedBackend: k.requestedBackend, fallbackReason: k.fallbackReason, integrity: k.integrity, ...k.info() });
      return 0;
    }
    case 'selftest': {
      const k = load();
      const result = selfTest(k, { seconds: number(flags, 'seconds', 60) });
      pjson({ ...result, requestedBackend: k.requestedBackend, fallbackReason: k.fallbackReason });
      return result.ok ? 0 : 1;
    }
    case 'parity': {
      const result = parity(loadKernel({ backend: 'wasm' }), loadKernel({ backend: 'napi' }));
      pjson(result);
      return result.ok ? 0 : 1;
    }
    case 'synth': {
      const out = load().synthesize({
        seconds: number(flags, 'seconds', 60),
        breathing_bpm: number(flags, 'breathing', 15),
        heart_bpm: number(flags, 'heart', 72),
        n_subcarriers: number(flags, 'subcarriers', 56),
        sample_rate_hz: number(flags, 'rate', 20),
        seed: number(flags, 'seed', 7),
      });
      const body = JSON.stringify({ evidence: out.evidence, config: { n_subcarriers: out.request.n_subcarriers, sample_rate_hz: out.request.sample_rate_hz }, frames: out.frames });
      if (typeof flags.out === 'string') {
        writeFileSync(flags.out, `${body}\n`, { flag: 'wx' }); // never overwrite an existing file
        pjson({ ok: true, evidence: out.evidence, frames: out.frames.length, out: flags.out });
      } else console.log(body);
      return 0;
    }
    case 'analyze': {
      if (typeof flags.file !== 'string') { console.error('analyze: --file <frames.json|frames.jsonl> is required'); return 2; }
      const input = readFrames(flags.file);
      const config = typeof flags.config === 'string' ? JSON.parse(flags.config) : (input.config || {});
      const k = load();
      pjson({ ok: true, backend: k.backend, ...k.analyze(input.frames, config, { includeReadings: flags.readings === true }) });
      return 0;
    }
    case 'bench': {
      const k = load();
      const seconds = number(flags, 'seconds', 120);
      const { frames } = k.synthesize({ seconds });
      k.analyze(frames.slice(0, 200)); // warm-up
      const iterations = 5;
      const time = (fn) => { const t0 = performance.now(); for (let i = 0; i < iterations; i++) fn(); return (performance.now() - t0) / iterations; };
      const ms = time(() => k.analyze(frames));
      const jsonMs = time(() => k.call('analyze', { config: {}, frames, include_readings: false }));
      pjson({
        evidence: 'MEASURED',
        reproducer: `npx @ruvnet/ruview-kernel bench --backend ${k.backend} --seconds ${seconds}`,
        host: { node: process.version, platform: process.platform, arch: process.arch },
        backend: k.backend, frames: frames.length, transport: k.binary ? 'binary' : 'json',
        msPerAnalyze: Number(ms.toFixed(2)), framesPerSecond: Math.round(frames.length / (ms / 1000)),
        jsonTransportMs: Number(jsonMs.toFixed(2)), speedupVsJson: Number((jsonMs / ms).toFixed(2)),
        note: 'Includes transfer across the ABI; single-host timing, not a cross-host claim.',
      });
      return 0;
    }
    case '--version': case '-v': {
      console.log(JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8')).version);
      return 0;
    }
    case '--help': case '-h': case 'help': return help();
    default:
      console.error(`Unknown command: ${cmd}. Try \`ruview-kernel --help\`.`);
      return 2;
  }
}

const invokedDirectly = (() => {
  if (!argv[1]) return false;
  try {
    const a = realpathSync(argv[1]);
    const b = realpathSync(fileURLToPath(import.meta.url));
    return process.platform === 'win32' ? a.toLowerCase() === b.toLowerCase() : a === b;
  } catch { return false; }
})();
if (invokedDirectly) {
  run(argv.slice(2)).then((code) => process.exit(code)).catch((error) => {
    pjson({ ok: false, code: error.code || 'error', error: error.message });
    process.exit(1);
  });
}
