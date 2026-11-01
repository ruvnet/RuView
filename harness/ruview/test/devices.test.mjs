// Device access (ADR-373). The ESP32 test uses a real UDP socket on loopback;
// serial devices are driven through the pyserial pump with injected output.
import test from 'node:test';
import assert from 'node:assert/strict';
import dgram from 'node:dgram';
import { captureEsp32, parsePacket } from '../src/devices/esp32.js';
import { Ld2410Parser, Mr60Parser, mr60Checksum, readMmwave } from '../src/devices/mmwave.js';
import { depthStats, parseRplidar, readIphoneLidar, readRplidar } from '../src/devices/lidar.js';
import { classifyPort } from '../src/devices/registry.js';
import { readSerial } from '../src/devices/serial-pump.js';
import { runTool } from '../src/tools.js';

// --- packet builders mirroring firmware structs -----------------------------
function csiPacket({ node = 1, seq = 0, sub = 4, ant = 1, rssi = -52 } = {}) {
  const b = Buffer.alloc(20 + sub * ant * 2);
  b.writeUInt32LE(0xC5110001, 0); b.writeUInt8(node, 4); b.writeUInt8(ant, 5); b.writeUInt16LE(sub, 6);
  b.writeUInt32LE(2437, 8); b.writeUInt32LE(seq, 12); b.writeInt8(rssi, 16); b.writeInt8(-95, 17);
  for (let i = 20; i < b.length; i += 2) { b.writeInt8(3, i); b.writeInt8(4, i + 1); }
  return b;
}
function vitalsPacket({ node = 1 } = {}) {
  const b = Buffer.alloc(32);
  b.writeUInt32LE(0xC5110002, 0); b.writeUInt8(node, 4); b.writeUInt8(0b101, 5);
  b.writeUInt16LE(1450, 6); b.writeUInt32LE(712000, 8); b.writeInt8(-50, 12); b.writeUInt8(1, 13);
  b.writeFloatLE(0.25, 16); b.writeFloatLE(0.8, 20); b.writeUInt32LE(123456, 24);
  return b;
}
const mr60Frame = (type, payload) => {
  const h = [0x01, 0x00, 0x01, payload.length >> 8, payload.length & 0xff, type >> 8, type & 0xff];
  return [...h, mr60Checksum(h), ...payload, mr60Checksum(payload)];
};
const f32 = (v) => { const b = Buffer.alloc(4); b.writeFloatLE(v); return [...b]; };
const ld2410Frame = (state, moving, stat) => {
  const data = [0x02, 0xaa, state, moving & 0xff, moving >> 8, 60, stat & 0xff, stat >> 8, 40, 0x2c, 0x01, 0x55, 0x00];
  return [0xf4, 0xf3, 0xf2, 0xf1, data.length, 0, ...data, 0xf8, 0xf7, 0xf6, 0xf5];
};
const rplidarNode = (angleDeg, mm, start = false, quality = 47) => {
  const a = Math.round(angleDeg * 64);
  const d = Math.round(mm * 4);
  return [(quality << 2) | (start ? 1 : 2), ((a & 0x7f) << 1) | 1, a >> 7, d & 0xff, d >> 8];
};
/** Fake python pump: returns the given bytes as HEX lines. */
const pumpDeps = (bytes, extra = '') => {
  const calls = [];
  return {
    calls,
    python: () => '/usr/bin/python3',
    exec: async (cmd, args, opts) => {
      calls.push({ args, opts });
      const hex = Buffer.from(bytes).toString('hex');
      return { ok: true, code: 0, stdout: `${extra}HEX ${hex}\nEND ${bytes.length}\n`, stderr: '', error: null };
    },
  };
};

test('ESP32 packets parse exactly like the firmware structs', () => {
  const csi = parsePacket(csiPacket({ seq: 9 }));
  assert.deepEqual([csi.kind, csi.nodeId, csi.subcarriers, csi.freqMhz, csi.seq, csi.rssi, csi.meanAmplitude], ['csi', 1, 4, 2437, 9, -52, 5]);
  const v = parsePacket(vitalsPacket());
  assert.deepEqual([v.kind, v.presence, v.fall, v.motion, v.breathingBpm, v.heartBpm, v.persons], ['vitals', true, false, true, 14.5, 71.2, 1]);
  assert.equal(parsePacket(Buffer.from([1, 2])).kind, 'malformed');
  assert.equal(parsePacket(Buffer.from([0, 0, 0, 0, 0])).kind, 'unknown');
  const truncated = csiPacket().subarray(0, 22);
  assert.equal(parsePacket(truncated).kind, 'malformed');
});

test('ESP32 capture receives a live UDP stream on loopback and measures loss', async () => {
  const port = 20000 + Math.floor(Math.random() * 20000);
  const pending = captureEsp32({ udp_port: port, bind: '127.0.0.1', seconds: 5, max_packets: 7 });
  const sender = dgram.createSocket('udp4');
  await new Promise((r) => setTimeout(r, 150));
  const sends = [];
  for (const seq of [0, 1, 2, 5, 6]) sends.push(new Promise((r) => sender.send(csiPacket({ seq }), port, '127.0.0.1', r)));
  sends.push(new Promise((r) => sender.send(vitalsPacket(), port, '127.0.0.1', r)));
  sends.push(new Promise((r) => sender.send(Buffer.from('garbage!'), port, '127.0.0.1', r)));
  await Promise.all(sends);
  sender.close();
  const r = await pending;
  assert.equal(r.ok, true, JSON.stringify(r));
  assert.equal(r.packets, 7);
  assert.equal(r.csiNodes, 1);
  const node = r.nodes[0];
  assert.equal(node.packets.csi, 5);
  assert.equal(node.csiLossFraction, 0.2857, '2 missing of 7 sequence numbers');
  assert.equal(node.lastVitals.breathingBpm, 14.5);
  assert.equal(r.unknownPackets, 1);
  assert.match(r.evidence, /^MEASURED/);
});

test('ESP32 capture reports an idle network and a busy port honestly', async () => {
  const port = 20000 + Math.floor(Math.random() * 20000);
  const idle = await captureEsp32({ udp_port: port, bind: '127.0.0.1', seconds: 1 });
  assert.deepEqual([idle.ok, idle.reason], [false, 'no_packets']);
  const holder = dgram.createSocket('udp4');
  await new Promise((r) => holder.bind(port, '127.0.0.1', r));
  const busy = await captureEsp32({ udp_port: port, bind: '127.0.0.1', seconds: 1 });
  holder.close();
  assert.equal(busy.reason, 'port_in_use');
  assert.equal((await captureEsp32({ udp_port: 80 })).reason, 'invalid_port');
  assert.equal((await captureEsp32({ bind: '8.8.8.8' })).reason, 'invalid_bind');
});

test('MR60BHA2 parser matches the firmware checksum rules', () => {
  const p = new Mr60Parser();
  const good = mr60Frame(0x0a15, f32(72));
  const badHeader = [...good]; badHeader[7] ^= 0xff;
  const badData = [...mr60Frame(0x0a14, f32(15))]; badData[badData.length - 1] ^= 0x01;
  const frames = p.feed(Buffer.from([0x00, 0x42, ...badHeader, ...badData, ...good, ...mr60Frame(0x0f09, [0])]));
  assert.deepEqual(frames.map((f) => f.type), ['heart', 'presence']);
  assert.equal(p.errors, 2);
  const oversized = [0x01, 0, 1, 0, 31, 0x0a, 0x14]; // payload > 30 is rejected
  assert.deepEqual(new Mr60Parser().feed(Buffer.from([...oversized, mr60Checksum(oversized)])), []);
});

test('LD2410 parser decodes target reports', () => {
  const p = new Ld2410Parser();
  const frames = p.feed(Buffer.from([0xf4, 0x00, ...ld2410Frame(1, 120, 80), ...ld2410Frame(2, 0, 250)]));
  assert.deepEqual(frames.map((f) => [f.targetState, f.distanceCm]), [['moving', 120], ['static', 250]]);
});

test('mmWave read auto-detects MR60BHA2 through the serial pump', async () => {
  const stream = [...mr60Frame(0x0a14, f32(14.5)), ...mr60Frame(0x0a15, f32(71)), ...mr60Frame(0x0f09, [1]), ...mr60Frame(0x0a16, [1, 0, 0, 0, ...f32(92.5)])];
  const deps = pumpDeps(stream);
  const r = await readMmwave({ port: '/dev/ttyUSB0', seconds: 3 }, deps);
  assert.equal(r.ok, true, JSON.stringify(r));
  // Probe (2 s) frames are kept and only the remaining 1 s is read again,
  // so the fake pump's 4-frame stream is seen twice.
  assert.deepEqual([r.model, r.detected, r.frames, r.breathingBpmMean, r.heartBpmMean, r.distanceCmMean, r.presentFraction], ['mr60bha2', 'mr60bha2', 8, 14.5, 71, 92.5, 1]);
  assert.deepEqual(deps.calls.map((c) => [c.args[3], c.args[4]]), [['115200', '2'], ['115200', '1']]);
  const ld = await readMmwave({ port: 'COM5', model: 'ld2410', seconds: 2 }, pumpDeps([...ld2410Frame(3, 90, 60), ...ld2410Frame(0, 0, 0)]));
  assert.deepEqual([ld.ok, ld.model, ld.frames], [true, 'ld2410', 2]);
  const noise = await readMmwave({ port: 'COM5', seconds: 2 }, pumpDeps([1, 2, 3, 4, 5]));
  assert.equal(noise.reason, 'no_valid_frames');
  assert.equal(noise.attempts.length, 2, 'both models were probed');
});

test('serial pump failures map to actionable reasons', async () => {
  const deps = (stdout) => ({ python: () => 'py', exec: async () => ({ ok: false, code: 4, stdout, stderr: '', error: 'x' }) });
  assert.equal((await readSerial({ port: 'COM3', baud: 115200 }, deps('NO_PYSERIAL\n'))).reason, 'pyserial_missing');
  assert.equal((await readSerial({ port: 'COM3', baud: 115200 }, deps('OPEN_ERROR could not open port\n'))).reason, 'port_open_failed');
  await assert.rejects(() => readSerial({ port: 'COM3;id', baud: 115200 }, deps('')), /port must look like/);
  await assert.rejects(() => readSerial({ port: 'COM3', baud: 12345 }, deps('')), /baud/);
  await assert.rejects(() => readSerial({ port: 'COM3', baud: 115200, startHex: "a5'; rm" }, deps('')), /hex/);
});

test('RPLIDAR scan stream parses with resync and summarizes coverage', async () => {
  const nodes = [];
  for (let rev = 0; rev < 2; rev++) for (let a = 0; a < 360; a += 10) nodes.push(...rplidarNode(a, 1000 + a, a === 0));
  const stream = [0x11, 0xa5, 0x5a, 0x05, 0x00, 0x00, 0x40, 0x81, ...nodes];
  const parsed = parseRplidar(Buffer.from(stream));
  assert.equal(parsed.points.length, 72);
  assert.equal(parsed.revolutions, 2);
  const deps = pumpDeps(stream);
  const r = await readRplidar({ port: '/dev/ttyUSB1', seconds: 2 }, deps);
  assert.equal(r.ok, true);
  assert.equal(r.angularCoverage, 1);
  assert.deepEqual(r.rangeMm, { min: 1000, median: 1175, max: 1350 });
  const pumpArgs = deps.calls[0].args;
  assert.deepEqual(pumpArgs.slice(-3), ['a520', 'a525', 'low'], 'SCAN on start, STOP on exit, A1 motor via DTR');
});

function lidarPacket(w = 4, h = 2) {
  const mm = Buffer.alloc(w * h * 2);
  for (let i = 0; i < w * h; i++) mm.writeUInt16LE(1500 + i, i * 2);
  return {
    type: 'ruview.lidar.depth.v1',
    depth: { width: w, height: h, encoding: 'u16le-mm+u8-confidence', millimetersBase64: mm.toString('base64'), confidenceBase64: Buffer.alloc(w * h, 2).toString('base64') },
  };
}

test('iPhone LiDAR relay client returns statistics only and keeps the token out of arguments', async () => {
  let opened = null;
  class FakeWS {
    constructor(url) {
      opened = url;
      setTimeout(() => { this.onopen?.(); this.onmessage?.({ data: '{"type":"bogus"}' }); this.onmessage?.({ data: JSON.stringify(lidarPacket()) }); }, 20);
    }
    close() {}
  }
  const r = await readIphoneLidar({ url: 'ws://10.0.0.5:8787/ws/lidar', seconds: 5, max_frames: 1 }, { WebSocket: FakeWS, env: { RUVIEW_LIDAR_TOKEN: 'tok123' } });
  assert.equal(r.ok, true, JSON.stringify(r));
  assert.equal(r.frames, 1);
  assert.equal(r.rejectedFrames, 1);
  assert.equal(r.last.medianDepthM, 1.5035);
  assert.match(opened, /token=tok123/);
  assert.equal(r.relay, 'ws://10.0.0.5:8787/ws/lidar', 'token never echoed');
  assert.ok(!JSON.stringify(r).includes('tok123'));
  class NeverOpens { constructor() { setTimeout(() => this.onerror?.({}), 10); } close() {} }
  const refused = await readIphoneLidar({ url: 'ws://10.0.0.5:8787/ws/lidar', seconds: 30 }, { WebSocket: NeverOpens, env: {} });
  assert.deepEqual([refused.ok, refused.reason], [false, 'connect_failed']);
  assert.equal((await readIphoneLidar({ url: 'ws://h/ws/lidar?token=x' })).reason, 'invalid_url');
  assert.equal((await readIphoneLidar({ url: 'http://h/ws' })).reason, 'invalid_url');
  assert.throws(() => depthStats({ ...lidarPacket(), depth: { ...lidarPacket().depth, width: 5 } }), /length mismatch/);
});

test('USB VID:PID classification suggests roles and confirm commands', () => {
  const esp = classifyPort({ port: 'COM9', description: 'USB JTAG/serial debug unit', vid: '303A', pid: '1001' });
  assert.deepEqual(esp.likelyRoles, ['esp32']);
  assert.match(esp.confirmWith[0], /--probe/);
  const cp = classifyPort({ port: '/dev/ttyUSB0', description: 'CP2102 USB to UART Bridge Controller', vid: '10C4', pid: 'EA60' });
  assert.deepEqual(cp.likelyRoles, ['esp32', 'rplidar', 'mmwave']);
  assert.deepEqual(classifyPort({ port: '/dev/ttyS0', description: 'n/a' }).likelyRoles, []);
  assert.deepEqual(classifyPort({ port: '/dev/ttyUSB2', description: 'Slamtec RPLIDAR', vid: '10C4', pid: 'EA60' }).likelyRoles, ['rplidar']);
});

test('MCP device tools require the device-access grant', async () => {
  for (const [name, args] of [['ruview_devices_scan', {}], ['ruview_esp32_capture', {}], ['ruview_mmwave_read', { port: 'COM3' }], ['ruview_lidar_read', { source: 'rplidar', port: 'COM3' }]]) {
    const denied = await runTool(name, args, { source: 'mcp', grants: [] });
    assert.deepEqual([denied.ok, denied.reason, denied.requiredGrant], [false, 'authority_denied', 'device-access'], name);
  }
  const bad = await runTool('ruview_lidar_read', { source: 'rplidar' }, { source: 'mcp', grants: ['device-access'] });
  assert.equal(bad.reason, 'invalid_arguments');
});
