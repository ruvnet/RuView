// Device-access end-to-end suite (ADR-373), driven through the real CLI.
//   * MR60BHA2 and RPLIDAR are emulated on pseudo-terminals and read through
//     real pyserial. The harness deliberately refuses /dev/pts/* port names,
//     so each pty is exposed as /dev/ttyRUVIEWE2E<n> (root or `sudo -n`).
//   * The ESP32 node is a UDP sender of firmware-format packets.
//   * The iPhone LiDAR path uses the repository's real relay.
// Every number is SYNTHETIC. Set RUVIEW_E2E_REQUIRE=1 (CI) to turn missing
// prerequisites into failures instead of skips.
import { describe, it as test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import dgram from 'node:dgram';
import { existsSync, symlinkSync, unlinkSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const CLI = join(HERE, '..', '..', 'bin', 'cli.js');
const REPO = join(HERE, '..', '..', '..', '..');
const REQUIRE = process.env.RUVIEW_E2E_REQUIRE === '1';
const python = ['python3', 'python'].find((p) => spawnSync(p, ['-c', 'import serial'], { stdio: 'ignore' }).status === 0);
const posix = process.platform === 'linux' || process.platform === 'darwin';
const canLink = posix && (process.getuid?.() === 0 || spawnSync('sudo', ['-n', 'true'], { stdio: 'ignore' }).status === 0);
const skip = (cond, why) => (cond ? false : (REQUIRE ? false : why));

function cli(args, env = {}) {
  return new Promise((resolve) => {
    const child = spawn(process.execPath, [CLI, ...args], { env: { ...process.env, ...env } });
    let out = '';
    child.stdout.on('data', (d) => { out += d; });
    child.on('close', (code) => {
      try { resolve({ code, json: JSON.parse(out) }); } catch { resolve({ code, json: null, raw: out }); }
    });
  });
}

async function withEmulator(kind, seconds, index, fn) {
  assert.ok(python, 'python with pyserial is required');
  assert.ok(canLink, 'root or passwordless sudo is required to expose the pty as /dev/tty*');
  const child = spawn(python, [join(HERE, 'emulate_sensor.py'), kind, String(seconds)]);
  const pts = await new Promise((resolve, reject) => {
    child.stdout.on('data', (d) => { const m = /READY (\S+)/.exec(String(d)); if (m) resolve(m[1]); });
    child.on('exit', (code) => reject(new Error(`emulator exited ${code}`)));
  });
  const link = `/dev/ttyRUVIEWE2E${index}`;
  const root = process.getuid?.() === 0;
  if (root) { if (existsSync(link)) unlinkSync(link); symlinkSync(pts, link); }
  else assert.equal(spawnSync('sudo', ['-n', 'ln', '-sf', pts, link]).status, 0);
  try {
    return await fn(link);
  } finally {
    if (root) { try { unlinkSync(link); } catch { /* gone */ } } else spawnSync('sudo', ['-n', 'rm', '-f', link]);
    child.kill();
  }
}

describe('RuView device access e2e (SYNTHETIC devices)', { concurrency: true }, () => {
test('60 GHz MR60BHA2 over pyserial: auto-detect, zero checksum errors, decoded values', { skip: skip(python && canLink, 'needs pyserial + root/sudo for a /dev/tty* link') }, async () => {
  await withEmulator('mr60', 12, 0, async (port) => {
    const r = await cli(['mmwave', '--port', port, '--seconds', '5']);
    assert.equal(r.code, 0, JSON.stringify(r.json || r.raw));
    const d = r.json;
    assert.deepEqual([d.model, d.detected, d.checksumErrors, d.presentFraction], ['mr60bha2', 'mr60bha2', 0, 1]);
    assert.ok(d.frames >= 50, `frames=${d.frames}`);
    assert.equal(d.breathingBpmMean, 14.8);
    assert.equal(d.heartBpmMean, 68.5);
    assert.equal(d.distanceCmMean, 87);
  });
});

test('RPLIDAR over pyserial: SCAN/STOP handshake, full revolutions and coverage', { skip: skip(python && canLink, 'needs pyserial + root/sudo for a /dev/tty* link') }, async () => {
  await withEmulator('rplidar', 10, 1, async (port) => {
    const r = await cli(['lidar', '--source', 'rplidar', '--port', port, '--seconds', '3']);
    assert.equal(r.code, 0, JSON.stringify(r.json || r.raw));
    assert.ok(r.json.revolutions >= 10, `revolutions=${r.json.revolutions}`);
    assert.equal(r.json.angularCoverage, 1);
    assert.equal(r.json.resyncBytes, 0);
    // Each complete synthetic sweep has 18 points at each distance; the two
    // central distances are 1200 and 1500 mm, so their median is 1350 mm.
    assert.deepEqual(r.json.rangeMm, { min: 1200, median: 1350, max: 1500 });
  });
});

test('ESP32 node UDP stream: per-node rate, zero loss, vitals decoded', async () => {
  const port = 21000 + Math.floor(Math.random() * 20000);
  const pending = cli(['esp32', '--udp-port', String(port), '--bind', '127.0.0.1', '--seconds', '3']);
  await new Promise((r) => setTimeout(r, 700));
  const sock = dgram.createSocket('udp4');
  for (let seq = 0; seq < 40; seq++) {
    const b = Buffer.alloc(20 + 64 * 2);
    b.writeUInt32LE(0xC5110001, 0); b.writeUInt8(7, 4); b.writeUInt8(1, 5); b.writeUInt16LE(64, 6);
    b.writeUInt32LE(2437, 8); b.writeUInt32LE(seq, 12); b.writeInt8(-48, 16); b.writeInt8(-94, 17);
    await new Promise((r) => sock.send(b, port, '127.0.0.1', r));
    if (seq === 20) {
      const v = Buffer.alloc(32);
      v.writeUInt32LE(0xC5110002, 0); v.writeUInt8(7, 4); v.writeUInt8(1, 5); v.writeUInt16LE(1520, 6); v.writeUInt32LE(690000, 8);
      await new Promise((r) => sock.send(v, port, '127.0.0.1', r));
    }
    await new Promise((r) => setTimeout(r, 25));
  }
  sock.close();
  const r = await pending;
  assert.equal(r.code, 0, JSON.stringify(r.json));
  const node = r.json.nodes[0];
  assert.deepEqual([r.json.csiNodes, node.nodeId, node.packets.csi, node.csiLossFraction, node.lastVitals.breathingBpm, node.lastVitals.heartBpm], [1, 7, 40, 0, 15.2, 69]);
});

const relayDir = join(REPO, 'integrations', 'iphone-lidar', 'web');
const relayReady = existsSync(join(relayDir, 'node_modules', 'ws'));
test('iPhone LiDAR through the real relay: frames accepted, token enforced and never echoed', { skip: skip(relayReady && typeof WebSocket === 'function', 'run `npm ci` in integrations/iphone-lidar/web (Node 22+)') }, async () => {
  const port = 21000 + Math.floor(Math.random() * 20000);
  const token = `e2e${Math.random().toString(36).slice(2)}`;
  const relay = spawn(process.execPath, ['relay.mjs'], { cwd: relayDir, env: { ...process.env, PORT: String(port), HOST: '127.0.0.1', RUVIEW_LIDAR_TOKEN: token } });
  try {
    await new Promise((resolve) => relay.stdout.on('data', (d) => { if (/listening/.test(String(d))) resolve(); }));
    const w = 16; const h = 12;
    const mm = Buffer.alloc(w * h * 2);
    for (let i = 0; i < w * h; i++) mm.writeUInt16LE(1800 + (i % 50), i * 2);
    const pkt = JSON.stringify({ type: 'ruview.lidar.depth.v1', depth: { width: w, height: h, encoding: 'u16le-mm+u8-confidence', millimetersBase64: mm.toString('base64'), confidenceBase64: Buffer.alloc(w * h, 2).toString('base64') }, intrinsics: { fx: 200, fy: 200, cx: 8, cy: 6, imageWidth: w, imageHeight: h }, pose: { matrix: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1] } });
    const phone = new WebSocket(`ws://127.0.0.1:${port}/ws/lidar?token=${token}`);
    await new Promise((resolve, reject) => { phone.onopen = resolve; phone.onerror = reject; });
    const timer = setInterval(() => phone.send(pkt), 100);
    const url = `ws://127.0.0.1:${port}/ws/lidar`;
    const ok = await cli(['lidar', '--source', 'iphone', '--url', url, '--seconds', '2'], { RUVIEW_LIDAR_TOKEN: token });
    const bad = await cli(['lidar', '--source', 'iphone', '--url', url, '--seconds', '1'], { RUVIEW_LIDAR_TOKEN: 'wrong' });
    clearInterval(timer);
    phone.close();
    assert.equal(ok.code, 0, JSON.stringify(ok.json));
    assert.ok(ok.json.frames >= 10, `frames=${ok.json.frames}`);
    // For these 192 pixels, sorted positions 96/97 are 1823/1824 mm.
    assert.equal(ok.json.last.medianDepthM, 1.8235);
    assert.ok(!JSON.stringify(ok.json).includes(token), 'token never echoed');
    assert.equal(bad.json.reason, 'connect_failed');
  } finally {
    relay.kill();
  }
});
});
