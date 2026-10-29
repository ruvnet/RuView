#!/usr/bin/env node
// SYNTHETIC loopback-only regression against an actual helper. Node22+.
// Usage: node scripts/verify-sensing-stream-freshness.mjs /path/to/sensing-server
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawn } from 'node:child_process';
import net from 'node:net';
import dgram from 'node:dgram';
import { runInNewContext } from 'node:vm';
const binary = resolve(process.argv[2] ?? 'v2/target/release/sensing-server');
const binarySha256 = createHash('sha256').update(readFileSync(binary)).digest('hex');
const evidence = mkdtempSync(join(tmpdir(), 'ruview-stream-freshness-'));
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const legacySource = readFileSync(new URL('../ui/services/pose.service.js', import.meta.url), 'utf8')
  .replace(/^import[^\n]*\n/gm, '').replace('export class PoseService', 'class PoseService')
  .replace('export const poseService', 'const poseService');
const PoseService = runInNewContext(`${legacySource};PoseService;`, {
  console: {log() {}, warn() {}, error() {}, debug() {}}, API_CONFIG: {},
});

const freePort = () => new Promise(resolve => {
  const socket = net.createServer();
  socket.listen(0, '127.0.0.1', () => { const port = socket.address().port; socket.close(() => resolve(port)); });
});
const connect = async url => {
  const socket = new WebSocket(url), frames = [];
  socket.onmessage = event => { if (typeof event.data === 'string') frames.push(JSON.parse(event.data)); };
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  return { socket, frames };
};
async function scenario(privacy) {
  const root = join(evidence, privacy ? 'privacy' : 'normal');
  const [http, ws, udpPort] = [await freePort(), await freePort(), await freePort()];
  const args = ['--source', 'esp32', '--bind-addr', '127.0.0.1', '--udp-bind', '127.0.0.1',
    '--http-port', String(http), '--ws-port', String(ws), '--udp-port', String(udpPort),
    '--data-dir', root, '--no-mdns', ...(privacy ? ['--privacy-mode'] : [])];
  const child = spawn(binary, args, {env: {PATH: process.env.PATH, RUST_LOG: 'warn'}, stdio: ['ignore', 'pipe', 'pipe']});
  let logs = ''; child.stdout.on('data', b => logs += b); child.stderr.on('data', b => logs += b);
  const sockets = [], udp = dgram.createSocket('udp4');
  try {
    let ready = false;
    for (let i = 0; i < 100; i++) { try { await fetch(`http://127.0.0.1:${http}/health`); ready = true; break; } catch { await delay(50); } }
    assert(ready, 'isolated helper did not start');
    const first = await connect(`ws://127.0.0.1:${ws}/ws/sensing`); sockets.push(first.socket);
    async function send(sequence) {
      const tones = 56, packet = Buffer.alloc(20 + tones * 2);
      packet.writeUInt32LE(0xC5110001, 0); packet[4] = 7; packet[5] = 1;
      packet.writeUInt16LE(tones, 6); packet.writeUInt32LE(2437, 8); packet.writeUInt32LE(sequence, 12);
      packet.writeInt8(-50, 16); packet.writeInt8(-90, 17);
      for (let i = 0; i < tones; i++) { packet[20 + i * 2] = 5 + i % 40; packet[21 + i * 2] = i % 30; }
      await new Promise((resolve, reject) => udp.send(packet, udpPort, '127.0.0.1', e => e ? reject(e) : resolve()));
      await delay(33);
    }
    for (let sequence = 1; sequence <= 60; sequence++) await send(sequence);
    await delay(100); const beforeQuiet = first.frames.length;
    const last = first.frames.filter(f => f.type === 'sensing_update').at(-1);
    assert(last, 'live producer did not publish');
    await delay(6200);
    const quietFrames = first.frames.slice(beforeQuiet).filter(f => f.type === 'sensing_update');
    assert.equal(quietFrames.length, 1, 'quiet period must emit exactly one offline transition, not frozen snapshots');
    const offline = quietFrames[0];
    assert.equal(offline.source, 'esp32:offline'); assert.equal(offline.tick, last.tick); assert.equal(offline.timestamp, last.timestamp);
    if (!privacy) { assert(offline.node_features[0].stale); assert(offline.node_features[0].last_seen_ms >= 5000); }
    const count = first.frames.length;
    const late = await connect(`ws://127.0.0.1:${ws}/ws/sensing`); sockets.push(late.socket);
    const legacy = await connect(`ws://127.0.0.1:${http}/api/v1/stream/pose`); sockets.push(legacy.socket);
    await delay(250);
    assert.equal(first.frames.length, count, 'new subscriber must not broadcast its snapshot to existing clients');
    const initial = late.frames.filter(f => f.type === 'sensing_update');
    assert.equal(initial.length, 1); assert.equal(initial[0].tick, last.tick); assert.equal(initial[0].timestamp, last.timestamp);
    assert.equal(initial[0].source, 'esp32:offline');
    const pose = legacy.frames.filter(f => f.type === 'pose_data'); assert.equal(pose.length, 1);
    assert.equal(pose[0].timestamp, last.timestamp); assert.equal(pose[0].payload.metadata.source, 'esp32:offline');
    assert.equal(pose[0].payload.pose_source, 'unavailable'); assert.equal(pose[0].payload.confidence, null);
    assert.equal(pose[0].payload.metadata.estimated_persons, null); assert.deepEqual(pose[0].payload.pose.persons, []);
    assert.equal(pose[0].payload.metadata.evidence_state, 'unavailable');
    const consumer = new PoseService(); assert(consumer.validatePoseData(pose[0].payload).valid);
    const converted = consumer.convertZoneDataToRestFormat(pose[0].payload, 'zone_1', pose[0]);
    assert.equal(converted.pose_source, 'unavailable'); assert.equal(converted.metadata.confidence, null);
    assert.equal(JSON.stringify(converted.zone_summary), '{}');
    if (privacy) for (const frame of [offline, initial[0], pose[0]]) {
      const serialized = JSON.stringify(frame); assert(!serialized.includes('pose_keypoints')); assert(!serialized.includes('keypoints')); assert(!serialized.includes('vital_signs'));
    }
    for (let sequence = 61; sequence <= 70; sequence++) await send(sequence);
    await delay(100);
    assert(first.frames.some(f => f.type === 'sensing_update' && f.source === 'esp32' && f.tick > last.tick), 'fresh input must resume publication');
    return {privacy, quietFrames: quietFrames.length, lateInitialFrames: initial.length, legacyInitialFrames: pose.length, resumed: true};
  } finally {
    sockets.forEach(socket => socket.close()); udp.close(); child.kill('SIGTERM');
    await Promise.race([new Promise(resolve => child.once('exit', resolve)), delay(3000)]);
    if (child.exitCode === null) child.kill('SIGKILL');
    writeFileSync(join(evidence, privacy ? 'privacy.log' : 'normal.log'), logs);
  }
}
const results = [await scenario(false), await scenario(true)];
const receipt = {evidence: 'SYNTHETIC isolated actual-binary UDP/WS', binarySha256, results};
writeFileSync(join(evidence, 'result.json'), JSON.stringify(receipt, null, 2));
console.log(JSON.stringify({evidencePath: join(evidence, 'result.json'), ...receipt}, null, 2));
