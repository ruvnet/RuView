// SPDX-License-Identifier: MIT
// Structured debugging doctor (ADR-372).
//
// Every check returns {id, group, status: pass|warn|fail|skip, detail, remedy}.
// `fail` means a requested capability cannot work; optional tooling that is
// absent is `warn`. The doctor is read-only: it never flashes, writes, or
// opens a serial port unless `probe_port` is explicitly requested (esptool
// chip_id resets the board but writes nothing).

import { createHash } from 'node:crypto';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { claimCheck } from './guardrails.js';
import { loadBrain } from './brain.js';
import { FIRMWARE_VARIANTS, listSerialPorts, parseChip, resolveBundle, validatePort } from './firmware.js';
import { classifyPort } from './devices/registry.js';
import { loadHosts } from './remote.js';

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
export const DOCTOR_GROUPS = Object.freeze(['runtime', 'harness', 'hosts', 'repo', 'rust', 'python', 'firmware', 'serial', 'devices', 'remote', 'sensing', 'kernel']);
const sha = (value) => createHash('sha256').update(value).digest('hex');

/** Verify the packaged provenance manifest (same rules as scripts/verify-manifest.mjs). */
export function verifyPackageManifest(root = ROOT) {
  const path = join(root, '.harness', 'manifest.json');
  if (!existsSync(path)) return { ok: false, findings: ['manifest.json:missing'] };
  let raw; let manifest; let entries;
  try {
    raw = readFileSync(path);
    manifest = JSON.parse(raw);
    entries = Object.entries(manifest.files || {});
  } catch {
    return { ok: false, files: 0, findings: ['manifest.json:unreadable-or-invalid'] };
  }
  const findings = [];
  for (const [name, expected] of entries) {
    const target = join(root, name);
    if (!existsSync(target)) findings.push(`${name}:missing`);
    else {
      try {
        if (sha(readFileSync(target, 'utf8').replace(/\r\n/g, '\n')) !== expected) findings.push(`${name}:hash-mismatch`);
      } catch { findings.push(`${name}:unreadable`); }
    }
  }
  const checksum = join(root, '.harness', 'manifest.sha256');
  if (!existsSync(checksum)) findings.push('manifest.sha256:missing');
  else {
    try {
      const outer = readFileSync(checksum, 'utf8').trim().split(/\s+/)[0];
      if (sha(raw) !== outer) findings.push('manifest.sha256:mismatch');
    } catch { findings.push('manifest.sha256:unreadable'); }
  }
  return { ok: findings.length === 0, files: entries.length, findings };
}

const firstLine = (text) => String(text || '').trim().split(/\r?\n/)[0]?.slice(0, 200) || '';

/**
 * Run diagnostics. args: { groups?, port?, probe_port?, sensing_url? }.
 * deps: { exec, which, findRepoRoot, python, importer, fetch }.
 */
export async function runDoctor(args = {}, deps) {
  const groups = new Set(args.groups?.length ? args.groups : DOCTOR_GROUPS);
  const checks = [];
  const add = (group, id, status, detail, remedy) => checks.push({ group, id, status, detail, ...(remedy ? { remedy } : {}) });
  const want = (g) => groups.has(g);

  if (want('runtime')) {
    const major = Number(process.versions.node.split('.')[0]);
    add('runtime', 'node-version', major >= 20 ? 'pass' : 'fail', `Node ${process.version} on ${process.platform}-${process.arch}`, major >= 20 ? null : 'Install Node.js 20 or newer.');
  }

  if (want('harness')) {
    const manifest = verifyPackageManifest();
    add('harness', 'package-integrity', manifest.ok ? 'pass' : 'warn',
      manifest.ok ? `${manifest.files} packaged files match .harness/manifest.json` : `manifest drift: ${manifest.findings.slice(0, 5).join(', ')}`,
      manifest.ok ? null : 'Reinstall the package; in a checkout run `npm run manifest:update` after intentional edits.');
    const guard = !claimCheck('We hit 100% accuracy on poses.').ok && claimCheck('Held-out PCK@20 59.5% (MEASURED vs mean-pose baseline, verify.py).').ok;
    add('harness', 'claim-guardrail', guard ? 'pass' : 'fail', guard ? 'claim_check flags overclaims and passes tagged claims' : 'claim_check self-test failed', guard ? null : 'Reinstall @ruvnet/ruview.');
    try {
      const brain = loadBrain();
      add('harness', 'shared-brain', 'pass', `${brain.records.length} reviewed records, digest ${brain.digest.slice(0, 12)}`);
    } catch (error) {
      add('harness', 'shared-brain', 'fail', error.message, 'Reinstall the package; the reviewed corpus is corrupt.');
    }
  }

  if (want('hosts')) {
    for (const [id, bin, remedy] of [['claude-code', 'claude', 'npm i -g @anthropic-ai/claude-code'], ['codex', 'codex', 'npm i -g @openai/codex']]) {
      const path = deps.which(bin);
      add('hosts', id, path ? 'pass' : 'warn', path ? `found ${path}` : `${bin} not on PATH (optional: agent run)`, path ? null : remedy);
    }
  }

  const repo = deps.findRepoRoot();
  if (want('repo')) {
    if (!repo) add('repo', 'checkout', 'warn', 'not inside a RuView checkout (verify/train/build need one)', 'git clone https://github.com/ruvnet/RuView && cd RuView');
    else {
      add('repo', 'checkout', 'pass', repo);
      const missing = ['vendor/rufield/Cargo.toml', 'v2/crates/ruview-swarm/Cargo.toml'].filter((p) => !existsSync(join(repo, p)));
      add('repo', 'submodules', missing.length ? 'warn' : 'pass', missing.length ? `uninitialized: ${missing.join(', ')}` : 'workspace submodules present',
        missing.length ? 'git submodule update --init --recursive' : null);
    }
  }

  if (want('rust')) {
    // Run inside v2/ so a pinned rust-toolchain file selects the same toolchain CI uses.
    const rustCwd = repo && existsSync(join(repo, 'v2')) ? join(repo, 'v2') : undefined;
    const cargo = deps.which('cargo');
    if (!cargo) add('rust', 'cargo', 'warn', 'cargo not on PATH (needed for training, calibration from source, kernel builds)', 'Install Rust: https://rustup.rs');
    else {
      const v = await deps.exec(cargo, ['--version'], { cwd: rustCwd, timeoutMs: 20_000 });
      add('rust', 'cargo', v.ok ? 'pass' : 'warn', firstLine(v.stdout) || v.error);
      const rustup = deps.which('rustup');
      if (rustup) {
        const t = await deps.exec(rustup, ['target', 'list', '--installed'], { cwd: rustCwd, timeoutMs: 20_000 });
        const wasm = t.stdout.includes('wasm32-unknown-unknown');
        add('rust', 'wasm32-target', wasm ? 'pass' : 'warn', wasm ? 'wasm32-unknown-unknown installed' : 'wasm32-unknown-unknown missing (kernel WASM builds)', wasm ? null : 'rustup target add wasm32-unknown-unknown');
      }
    }
    const bin = deps.which('wifi-densepose');
    add('rust', 'wifi-densepose-cli', bin ? 'pass' : 'warn', bin ? `found ${bin}` : 'wifi-densepose binary not on PATH; calibration/room training will use cargo run',
      bin ? null : 'cargo install --path v2/crates/wifi-densepose-cli');
  }

  const python = deps.python();
  if (want('python')) {
    if (!python) add('python', 'python', 'warn', 'python not on PATH (needed for flashing, serial monitor, proof)', 'Install Python 3.9+');
    else {
      const v = await deps.exec(python, ['--version'], { timeoutMs: 20_000 });
      add('python', 'python', 'pass', `${python}: ${firstLine(v.stdout || v.stderr)}`);
      const serial = await deps.exec(python, ['-c', 'import serial;print(serial.__version__)'], { timeoutMs: 20_000 });
      add('python', 'pyserial', serial.ok ? 'pass' : 'warn', serial.ok ? `pyserial ${firstLine(serial.stdout)}` : 'pyserial not importable', serial.ok ? null : 'pip install pyserial');
      const esp = await deps.exec(python, ['-m', 'esptool', 'version'], { timeoutMs: 30_000 });
      const espVersion = /v?(\d+\.\d+(?:\.\d+)?)/.exec(esp.stdout)?.[1];
      add('python', 'esptool', esp.ok && espVersion ? 'pass' : 'warn', esp.ok && espVersion ? `esptool ${espVersion}` : 'esptool not importable', esp.ok ? null : 'pip install esptool');
    }
  }

  if (want('firmware')) {
    const binsRoot = repo && join(repo, 'firmware', 'esp32-csi-node', 'release_bins');
    if (!binsRoot || !existsSync(binsRoot)) add('firmware', 'bundles', 'skip', 'no firmware/esp32-csi-node/release_bins in this checkout; pass --bundle <extracted release bundle> to flash');
    else {
      for (const name of readdirSync(binsRoot, { withFileTypes: true }).filter((d) => d.isDirectory()).map((d) => d.name).sort()) {
        const variant = name.startsWith('c6') ? 'c6' : 's3-8mb';
        try {
          const b = resolveBundle(join(binsRoot, name), variant);
          add('firmware', `bundle:${name}`, 'pass', `${b.chipName} ${b.images.length} images verified against ${b.checksums}`);
        } catch (error) {
          add('firmware', `bundle:${name}`, 'fail', error.message, 'Re-download the release bundle and verify SHA256SUMS before flashing.');
        }
      }
    }
    add('firmware', 'variants', 'pass', `supported: ${Object.keys(FIRMWARE_VARIANTS).join(', ')}`);
  }

  if (want('serial') && python) {
    const ports = await listSerialPorts(deps);
    if (!ports.ok) add('serial', 'ports', 'warn', `cannot enumerate ports (${ports.reason})`, ports.reason === 'pyserial_missing' ? 'pip install pyserial' : null);
    else add('serial', 'ports', ports.ports.length ? 'pass' : 'warn', ports.ports.length ? ports.ports.map((p) => `${p.port} ${p.description}`).join('; ').slice(0, 600) : 'no serial ports found',
      ports.ports.length ? null : 'Connect the board with a data-capable USB cable; install the CP210x/CH340 driver if needed.');
    if (args.port) {
      try {
        validatePort(args.port);
        const listed = ports.ports.some((p) => p.port === args.port);
        add('serial', 'port', listed ? 'pass' : 'warn', listed ? `${args.port} present` : `${args.port} not in enumerated ports`, listed ? null : 'Check the port name; on Linux add your user to the dialout group.');
        if (args.probe_port === true) {
          const probe = await deps.exec(python, ['-m', 'esptool', '--port', args.port, 'chip_id'], { timeoutMs: 60_000 });
          const chip = parseChip(probe.stdout + probe.stderr);
          add('serial', 'chip', chip ? 'pass' : 'fail', chip ? `detected ${chip}` : `no chip response: ${firstLine(probe.stderr || probe.error)}`,
            chip ? null : 'Close other serial monitors; hold BOOT and tap RESET; try --baud 115200.');
        }
      } catch (error) {
        add('serial', 'port', 'fail', error.message);
      }
    }
  }

  if (want('devices') && python) {
    const listed = await listSerialPorts(deps);
    if (listed.ok) {
      const devices = listed.ports.map(classifyPort).filter((d) => d.likelyRoles.length);
      if (!devices.length) add('devices', 'usb-serial', 'warn', 'no ESP32 / Realtek / mmWave / RPLIDAR candidates on USB', 'Use a data-capable cable; install CP210x, CH34x or PL2303 drivers; on Linux add your user to dialout.');
      for (const d of devices) add('devices', `usb:${d.port}`, 'pass', `${d.bridge || d.usb}: likely ${d.likelyRoles.join('/')} — confirm: ${d.confirmWith[0]}`);
    } else add('devices', 'usb-serial', 'warn', `cannot enumerate (${listed.reason})`, listed.reason === 'pyserial_missing' ? 'pip install pyserial' : null);
    add('devices', 'network-streams', 'skip', 'ESP32 UDP and iPhone LiDAR are network streams: `ruview esp32 --seconds 10`, `ruview lidar --source iphone --url ws://HOST:8787/ws/lidar`');
  }

  if (want('remote')) {
    try {
      const { path, hosts } = loadHosts();
      add('remote', 'hosts-file', 'pass', hosts.length ? `${hosts.length} host(s) in ${path}: ${hosts.map((h) => h.name).join(', ')}` : `no remote hosts configured (${path})`);
    } catch (error) {
      add('remote', 'hosts-file', 'fail', `invalid hosts file: ${error.message}`, 'Fix or remove ~/.config/ruview/hosts.json; re-add hosts with `ruview hosts add`.');
    }
    const ssh = deps.which('ssh');
    add('remote', 'ssh', ssh ? 'pass' : 'warn', ssh ? `found ${ssh}` : 'ssh client not on PATH (remote hosts)', ssh ? null : 'Install the OpenSSH client.');
  }

  if (want('sensing')) {
    if (!args.sensing_url) add('sensing', 'server', 'skip', 'pass --url http://<host>:<port> to check a running sensing server');
    else {
      try {
        const url = new URL(args.sensing_url);
        if (!['http:', 'https:'].includes(url.protocol)) throw new Error('sensing_url must be http(s)');
        const target = new URL('/health', url);
        const res = await deps.fetch(target, { signal: AbortSignal.timeout(5000) });
        add('sensing', 'server', res.ok ? 'pass' : 'fail', `${target.href} → HTTP ${res.status}`, res.ok ? null : 'Start the sensing server: cargo run -p wifi-densepose-sensing-server');
      } catch (error) {
        add('sensing', 'server', 'fail', `unreachable: ${error.message}`, 'Check host/port and firewall; ESP32 nodes stream UDP to the aggregator, the HTTP API is separate.');
      }
    }
  }

  if (want('kernel')) {
    try {
      const mod = await deps.importer('@ruvnet/ruview-kernel');
      const status = mod.kernelStatus();
      add('kernel', 'wasm', status.wasm.available ? 'pass' : 'fail', status.wasm.available ? `ruview-kernel ${status.wasm.version} (${status.wasm.integrity})` : status.wasm.reason);
      add('kernel', 'napi', status.napi.available ? 'pass' : 'warn', status.napi.available ? `native ${status.triple}` : `optional: ${status.napi.reason}`);
    } catch {
      add('kernel', 'package', 'warn', '@ruvnet/ruview-kernel not installed (optional compute/SDK kernel)', 'npm install @ruvnet/ruview-kernel');
    }
  }

  const summary = { pass: 0, warn: 0, fail: 0, skip: 0 };
  for (const c of checks) summary[c.status] += 1;
  return {
    ok: summary.fail === 0,
    summary,
    checks,
    nextSteps: checks.filter((c) => c.remedy && c.status !== 'pass').map((c) => `${c.group}/${c.id}: ${c.remedy}`),
  };
}

/** Human-readable rendering for the CLI. */
export function formatDoctor(report) {
  const icon = { pass: 'PASS', warn: 'WARN', fail: 'FAIL', skip: 'SKIP' };
  const lines = [];
  let group = null;
  for (const c of report.checks) {
    if (c.group !== group) { group = c.group; lines.push(`\n[${group}]`); }
    lines.push(`  ${icon[c.status]} ${c.id} — ${c.detail}`);
    if (c.remedy && c.status !== 'pass') lines.push(`       fix: ${c.remedy}`);
  }
  const s = report.summary;
  lines.push(`\nruview doctor: ${report.ok ? 'no failures' : 'failures found'} — ${s.pass} pass, ${s.warn} warn, ${s.fail} fail, ${s.skip} skip`);
  return lines.join('\n').trimStart();
}
