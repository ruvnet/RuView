// CSI node formats beyond ADR-018 (ADR-373 Amendment 2): Realtek RAC1/RHB1
// (ADR-323), ADR-110 sync packets, per-shape stats, honest failure modes, and
// live CSI → compute kernel analysis. Datagrams are built byte-for-byte from
// the ADR layouts and sent over a real loopback UDP socket.
import test from 'node:test';
import assert from 'node:assert/strict';
import dgram from 'node:dgram';
import { binAmplitudes, captureEsp32, crc32, parsePacket, RAC1_MAGIC, RHB1_MAGIC, SpectrumCollector, NodeStats, SEQ_WINDOW } from '../src/devices/esp32.js';
import { classifyPort } from '../src/devices/registry.js';
import { kernelAnalyzer } from '../src/kernel.js';
import { runTool, TOOLS } from '../src/tools.js';
import { validateArguments } from '../src/policy.js';

/** RAC1 envelope per ADR-323: 49-byte header, (I,Q) tones, CRC-32/IEEE trailer. */
function rac1({ node = 3, seq = 1, tones = 52, bits = 16, rssi = -40, channel = 153, synthetic = 0, iq = [3, 4] } = {}) {
  const payload = tones * (bits / 8);
  const b = Buffer.alloc(49 + payload + 4);
  b.writeUInt32LE(RAC1_MAGIC, 0); b.writeUInt8(1, 4); b.writeUInt16LE(49, 5); b.writeUInt32LE(b.length, 7);
  b.writeUInt8(node, 11); b.writeUInt8(0, 12); b.writeUInt32LE(seq, 13); b.writeUInt32LE(123456, 17);
  b.writeUInt8(channel, 33); b.writeUInt8(0, 34); b.writeUInt8(7, 35); b.writeUInt8(1, 36);
  b.writeUInt16LE(tones, 37); b.writeUInt8(bits, 39); b.writeUInt8(1, 40); b.writeInt8(rssi, 41);
  b.writeUInt8(1, 43); b.writeUInt8(synthetic, 44); b.writeUInt32LE(payload, 45);
  for (let k = 0; k < tones; k++) {
    if (bits === 16) { b.writeInt8(iq[0], 49 + 2 * k); b.writeInt8(iq[1], 50 + 2 * k); }
    else { b.writeInt16LE(iq[0] * 100, 49 + 4 * k); b.writeInt16LE(iq[1] * 100, 51 + 4 * k); }
  }
  b.writeUInt32LE(crc32(b, b.length - 4), b.length - 4);
  return b;
}
const rhb1 = () => { const b = Buffer.alloc(4); b.writeUInt32LE(RHB1_MAGIC, 0); return b; };
function esp32Csi({ node = 42, seq = 0, sub = 64, iq = [0, 5] } = {}) {
  const b = Buffer.alloc(20 + sub * 2);
  b.writeUInt32LE(0xC5110001, 0); b.writeUInt8(node, 4); b.writeUInt8(1, 5); b.writeUInt16LE(sub, 6);
  b.writeUInt32LE(2452, 8); b.writeUInt32LE(seq, 12); b.writeInt8(-52, 16); b.writeInt8(-95, 17);
  for (let i = 20; i < b.length; i += 2) { b.writeInt8(iq[0], i); b.writeInt8(iq[1], i + 1); }
  return b;
}
function syncPacket({ node = 42, leader = true, hw = 777 } = {}) {
  const b = Buffer.alloc(32);
  b.writeUInt32LE(0xC511A110, 0); b.writeUInt8(node, 4); b.writeUInt8(1, 5); b.writeUInt8(leader ? 1 : 0, 6);
  b.writeUInt32LE(hw, 24);
  return b;
}

async function capture(args, datagrams, deps) {
  const port = 20000 + Math.floor(Math.random() * 20000);
  const pending = captureEsp32({ udp_port: port, bind: '127.0.0.1', seconds: 2, ...args }, deps);
  await new Promise((r) => setTimeout(r, 150));
  const sender = dgram.createSocket('udp4');
  for (const d of datagrams) {
    await new Promise((r) => sender.send(d, port, '127.0.0.1', r));
    await new Promise((r) => setImmediate(r));
  }
  sender.close();
  return pending;
}

test('Realtek RAC1 parses every ADR-323 field and verifies the CRC', () => {
  const p = parsePacket(rac1({ seq: 77 }));
  assert.deepEqual(
    [p.kind, p.source, p.nodeId, p.seq, p.subcarriers, p.channel, p.rssi, p.bitsPerTone, p.csiValid, p.synthetic, p.meanAmplitude],
    ['csi', 'realtek', 3, 77, 52, 153, -40, 16, true, false, 5],
  );
  const wide = parsePacket(rac1({ bits: 32, iq: [3, 4] }), { iq: true });
  assert.equal(wide.meanAmplitude, 500);
  assert.equal(wide.amplitudes.length, 52);
  assert.ok(Math.abs(wide.phases[0] - Math.atan2(400, 300)) < 1e-12, 'Realtek tones are (I, Q)');
  assert.equal(parsePacket(rac1({ synthetic: 1 })).synthetic, true);

  const corrupt = rac1(); corrupt[60] ^= 0xff;
  assert.deepEqual([parsePacket(corrupt).kind, parsePacket(corrupt).reason], ['malformed', 'rac1 crc']);
  const badLen = rac1(); badLen.writeUInt32LE(999, 7);
  assert.equal(parsePacket(badLen).reason, 'rac1 length');
  const badBits = rac1(); badBits.writeUInt8(24, 39); badBits.writeUInt32LE(crc32(badBits, badBits.length - 4), badBits.length - 4);
  assert.equal(parsePacket(badBits).reason, 'rac1 payload');
  assert.equal(parsePacket(rac1().subarray(0, 40)).reason, 'rac1 header');
  for (let end = 0; end < 60; end++) parsePacket(rac1().subarray(0, end)); // never throws on prefixes
  assert.deepEqual(parsePacket(rhb1()), { kind: 'heartbeat', source: 'realtek', nodeId: null });
});

test('ESP32 phase uses the ESP-IDF (imag, real) order; sync packets decode', () => {
  const p = parsePacket(esp32Csi({ iq: [4, 3] }), { iq: true });
  assert.equal(p.meanAmplitude, 5);
  assert.ok(Math.abs(p.phases[0] - Math.atan2(4, 3)) < 1e-12);
  assert.equal(parsePacket(esp32Csi()).amplitudes, null, 'per-tone arrays only on request');
  const s = parsePacket(syncPacket());
  assert.deepEqual([s.kind, s.nodeId, s.leader, s.highWaterSeq], ['sync', 42, true, 777]);
  assert.equal(parsePacket(syncPacket().subarray(0, 20)).reason, 'sync length');
  const mesh = Buffer.alloc(16); mesh.writeUInt32LE(0xC5118100, 0); mesh.writeUInt8(1, 4); mesh.writeUInt8(6, 5);
  assert.deepEqual(parsePacket(mesh), { kind: 'mesh', source: 'esp32', nodeId: null, version: 1, msgType: 'health' });
  assert.equal(parsePacket(mesh.subarray(0, 8)).reason, 'mesh header');
});

test('mixed ESP32 + Realtek fleet: per-source nodes, shapes, loss, heartbeats', async () => {
  const datagrams = [];
  for (let i = 0; i < 6; i++) datagrams.push(esp32Csi({ seq: i * 2, sub: 256 }), esp32Csi({ seq: i * 2 + 1, sub: 64 }));
  for (const seq of [10, 11, 13]) datagrams.push(rac1({ seq, node: 0 }));
  const mesh = Buffer.alloc(16); mesh.writeUInt32LE(0xC5118100, 0); mesh.writeUInt8(1, 5);
  datagrams.push(rhb1(), rhb1(), syncPacket(), mesh);
  const r = await capture({ max_packets: datagrams.length }, datagrams);
  assert.equal(r.ok, true, JSON.stringify(r));
  assert.equal(r.csiNodes, 2);
  assert.equal(r.heartbeats, 2);
  assert.equal(r.unknownPackets, 0);
  assert.equal(r.decodedPackets, datagrams.length);
  assert.deepEqual(r.meshMessages, { 'time-sync': 1 });
  assert.equal(r.nodes.length, 2, 'mesh envelopes never create a phantom node');
  const esp = r.nodes.find((n) => n.source === 'esp32');
  const rtk = r.nodes.find((n) => n.source === 'realtek');
  assert.deepEqual(esp.packets, { csi: 12, sync: 1 });
  assert.deepEqual(esp.csiShapes.map((s) => [s.shape, s.frames]), [['1x256', 6], ['1x64', 6]]);
  assert.equal(esp.csiLossFraction, 0);
  assert.deepEqual([rtk.nodeId, rtk.packets.csi, rtk.csi.channel, rtk.csiLossFraction], [0, 3, 153, 0.25]);
  assert.equal(r.heartbeatOnlySenders, undefined, 'the sender also delivered CSI');
});

test('loss survives reordering and stray sequence values (live RTL8721Dx pattern)', async () => {
  // B+0..B+19 (B = 50,000, live counters are ~80k) with: B+5 missing, B+8 arriving after B+9, B+12 duplicated, a
  // stray +52,835 frame after B+15, and a stray 0 after B+17 that hides a
  // real loss (B+18) — both stray patterns were seen live on COM10. Then a genuine
  // counter reset: 1, 2, 3.
  const B = 50000;
  const seqs = [];
  for (let i = 0; i < 20; i++) {
    if (i === 5 || i === 8 || i === 18) continue;
    seqs.push(B + i);
    if (i === 9) seqs.push(B + 8);
    if (i === 12) seqs.push(B + 12);
    if (i === 15) seqs.push(B + 15 + 52835);
    if (i === 17) seqs.push(0);
  }
  seqs.push(1, 2, 3);
  const r = await capture({ max_packets: seqs.length }, seqs.map((seq) => rac1({ seq })));
  const n = r.nodes[0];
  assert.equal(n.packets.csi, seqs.length);
  assert.deepEqual([n.seqReordered, n.seqStrays, n.seqResyncs], [2, 2, 1]);
  // B+5 and B+18 are truly lost: 2 gaps over csi + gaps slots.
  assert.equal(n.csiLossFraction, Number((2 / (seqs.length + 2)).toFixed(4)));
});

test('heartbeat-only and undecodable streams fail instead of claiming MEASURED', async () => {
  const hb = await capture({ max_packets: 3 }, [rhb1(), rhb1(), rhb1()]);
  assert.deepEqual([hb.ok, hb.reason, hb.evidence, hb.heartbeats], [false, 'heartbeat_only', null, 3]);
  assert.deepEqual(hb.heartbeatOnlySenders, [{ address: '127.0.0.1', heartbeats: 3 }]);
  assert.match(hb.remedy, /1500000/);

  const junk = await capture({ max_packets: 2 }, [Buffer.from('ABCDEFGH'), Buffer.from('ABCDxxxx')]);
  assert.deepEqual([junk.ok, junk.reason, junk.evidence], [false, 'no_decodable_packets', null]);
  assert.deepEqual(junk.unknownMagics, { '0x44434241': 2 });

  const bad = rac1(); bad[70] ^= 1;
  const crc = await capture({ max_packets: 1 }, [bad]);
  assert.deepEqual([crc.ok, crc.malformedReasons], [false, { 'rac1 crc': 1 }]);
});

test('analyze feeds the busiest node dominant shape to the kernel at the measured rate', async () => {
  const seen = [];
  const analyze = async (frames, config) => { seen.push({ frames, config }); return { ok: true, backend: 'fake', summary: { frames: frames.length } }; };
  const datagrams = [];
  for (let i = 0; i < 40; i++) datagrams.push(esp32Csi({ seq: i, sub: 64 }));
  for (let i = 0; i < 5; i++) datagrams.push(rac1({ seq: i }));
  const r = await capture({ analyze: true, max_packets: datagrams.length }, datagrams, { analyze });
  assert.equal(r.ok, true);
  assert.equal(seen.length, 1);
  assert.equal(seen[0].config.n_subcarriers, 64);
  assert.ok(seen[0].config.sample_rate_hz > 0);
  assert.equal(seen[0].frames[0].amplitudes.length, 64);
  assert.deepEqual([r.analysis.ok, r.analysis.input.source, r.analysis.input.nodeId, r.analysis.input.frames], [true, 'esp32', 42, 40]);
  assert.match(r.analysis.note, /no reference measurement/);

  const pinned = await capture({ analyze: true, node_id: 3, max_packets: datagrams.length }, datagrams, { analyze });
  assert.deepEqual([pinned.analysis.input.source, pinned.analysis.input.subcarriers], ['realtek', 52]);
  const missing = await capture({ analyze: true, node_id: 9, max_packets: 2 }, datagrams.slice(0, 2), { analyze });
  assert.deepEqual([missing.analysis.ok, missing.analysis.reason], [false, 'no_frames']);
  const many = Array.from({ length: 70 }, (_, i) => esp32Csi({ seq: i, sub: 64 }));
  const capped = await capture({ analyze: true, analyze_max_frames: 64, max_packets: 70 }, many, { analyze });
  assert.deepEqual([capped.analysis.input.frames, capped.analysis.input.droppedOverCap], [64, 6]);

  assert.equal((await captureEsp32({ analyze: true })).reason, 'analyze_unavailable');
  assert.equal((await captureEsp32({ node_id: 300 })).reason, 'invalid_node_id');
});

test('kernel analyzer fails closed without the package and reports kernel errors', async () => {
  const absent = await kernelAnalyzer({}, { importer: async () => { throw Object.assign(new Error('nope'), { code: 'ERR_MODULE_NOT_FOUND' }); } })([], {});
  assert.deepEqual([absent.ok, absent.reason], [false, 'kernel_not_installed']);
  const fake = { loadKernel: ({ backend }) => ({ backend, integrity: 'verified', analyze: (frames, config) => ({ config, summary: { frames: frames.length } }) }) };
  const ok = await kernelAnalyzer({ backend: 'napi' }, { importer: async () => fake })([{ amplitudes: [1], phases: [0] }], { n_subcarriers: 1 });
  assert.deepEqual([ok.ok, ok.backend, ok.summary.frames], [true, 'napi', 1]);
  const failing = { loadKernel: () => { throw Object.assign(new Error('no addon'), { code: 'backend_unavailable' }); } };
  const bad = await kernelAnalyzer({ backend: 'napi' }, { importer: async () => failing })([], {});
  assert.deepEqual([bad.ok, bad.reason], [false, 'backend_unavailable']);
});

test('Prolific PL2303GC is classified as a Realtek Ameba board', () => {
  const rtk = classifyPort({ port: 'COM10', description: 'Prolific PL2303GC USB Serial COM Port (COM10)', vid: '067B', pid: '23A3' });
  assert.deepEqual(rtk.likelyRoles, ['realtek']);
  assert.match(rtk.confirmWith[0], /--baud 1500000/);
  assert.match(rtk.confirmWith[0], /RAC1/);
});

test('tool schemas accept the new arguments and reject bad ones', async () => {
  const esp = TOOLS.ruview_esp32_capture.inputSchema;
  assert.deepEqual(validateArguments(esp, { analyze: true, node_id: 3, backend: 'napi', analyze_max_frames: 600 }), []);
  assert.notDeepEqual(validateArguments(esp, { backend: 'gpu' }), []);
  const mon = TOOLS.ruview_node_monitor.inputSchema;
  assert.deepEqual(validateArguments(mon, { port: 'COM10', baud: 1500000 }), []);
  assert.notDeepEqual(validateArguments(mon, { port: 'COM10', baud: 9600 }), []);
  const denied = await runTool('ruview_esp32_capture', { analyze: true }, { source: 'mcp', grants: [] });
  assert.equal(denied.reason, 'authority_denied');
});

test('spectrum: binned amplitude frames per node for the waterfall (ADR-378)', async () => {
  assert.deepEqual(binAmplitudes([1, 2, 3, 4, 5, 6], 3), [1.5, 3.5, 5.5]);
  assert.deepEqual(binAmplitudes([1, 2], 8), [1, 2], 'never more bins than subcarriers');
  const ring = new SpectrumCollector(3, 2);
  for (let k = 0; k < 5; k++) ring.add({ kind: 'csi', source: 'esp32', nodeId: 1, subcarriers: 4, amplitudes: [k, k, k, k] }, k * 100);
  ring.add({ kind: 'heartbeat' }, 0);
  const [r] = ring.result();
  assert.deepEqual(r.frames.map((f) => f[0]), [2, 3, 4], 'keeps the newest frames');
  assert.deepEqual([r.framesSeen, r.rateHz, r.bins, r.synthetic], [5, 10, 2, false]);
  const res = await capture({ spectrum: true, spectrum_bins: 16, spectrum_frames: 8 }, Array.from({ length: 12 }, (_, seq) => esp32Csi({ seq })));
  assert.equal(res.ok, true);
  const [s] = res.spectrum;
  assert.deepEqual([s.source, s.nodeId, s.subcarriers, s.bins, s.frames.length], ['esp32', 42, 64, 16, 8]);
  assert.ok(s.frames.every((f) => f.every((v) => v === 5)), 'amplitude |(0,5)| = 5');
  const plain = await capture({}, [esp32Csi()]);
  assert.equal(plain.spectrum, undefined, 'only when asked');
  const tool = TOOLS.ruview_esp32_capture.inputSchema;
  assert.deepEqual(validateArguments(tool, { spectrum: true, spectrum_bins: 48, spectrum_frames: 64 }), []);
  assert.notDeepEqual(validateArguments(tool, { spectrum_bins: 4 }), []);
});

test('spectrum draws multi-antenna ESP32 nodes from their first chain (ADR-018 antenna-major)', async () => {
  const sub = 8;
  const b = Buffer.alloc(20 + 2 * sub * 2);
  b.writeUInt32LE(0xC5110001, 0); b.writeUInt8(9, 4); b.writeUInt8(2, 5); b.writeUInt16LE(sub, 6);
  for (let k = 0; k < sub; k++) { b.writeInt8(0, 20 + 2 * k); b.writeInt8(3, 21 + 2 * k); } // chain 0: |3|
  for (let k = 0; k < sub; k++) { b.writeInt8(0, 20 + 2 * (sub + k)); b.writeInt8(7, 21 + 2 * (sub + k)); } // chain 1: |7|
  assert.equal(parsePacket(b, { iq: true }).amplitudes, null, 'the kernel path still takes single-chain frames only');
  assert.deepEqual(parsePacket(b, { iq: true, firstChain: true }).chain0Amplitudes, Array(sub).fill(3));
  const res = await capture({ spectrum: true, spectrum_bins: 8, spectrum_frames: 8 }, [b, b, b]);
  const [s] = res.spectrum;
  assert.deepEqual([s.nodeId, s.shape, s.bins], [9, '2x8', 8]);
  assert.ok(s.frames.every((f) => f.every((v) => v === 3)));
});


test('only missing CSI sequence numbers recover counted packet loss', async () => {
  for (const [seqs, gaps] of [
    [[100, 103, 101, 101], 1], // duplicated recovery must not fill missing 102
    [[100, 102, 100], 1], // an already-received packet cannot fill missing 101
    [[100, 102, 99], 1], // a pre-capture packet cannot fill missing 101
    [[100, 103, 102, 101], 0], // distinct actual gap recoveries still work
  ]) {
    const result = await capture({ max_packets: seqs.length }, seqs.map((seq) => rac1({ seq })));
    assert.equal(result.nodes[0].csiLossFraction, Number((gaps / (seqs.length + gaps)).toFixed(4)),
      `loss for ${seqs.join(',')}`);
  }
});


test('CSI recovery bookkeeping stays within the reorder window', () => {
  const stats = new NodeStats();
  for (let seq = 0; seq <= SEQ_WINDOW * 10; seq += 64) {
    stats.add(parsePacket(rac1({ seq })), '127.0.0.1');
    const node = stats.nodes.get('realtek:3');
    assert.ok(node.missingSeq.size < SEQ_WINDOW);
  }
  const node = stats.nodes.get('realtek:3');
  const historicalGaps = node.seqGaps;
  stats.add(parsePacket(rac1({ seq: 1 })), '127.0.0.1');
  stats.add(parsePacket(rac1({ seq: 2 })), '127.0.0.1');
  assert.equal(node.missingSeq.size, 0, 'a confirmed counter reset clears old recovery identities');
  assert.equal(node.seqGaps, historicalGaps, 'historical loss remains counted');
});
test('packet-limited captures report the observed interval and rate', async () => {
  const r = await capture({ seconds: 30, max_packets: 2 }, [esp32Csi({ seq: 1 }), esp32Csi({ seq: 2 })]);
  assert.equal(r.truncated, true);
  assert.equal(r.seconds, 30, 'configured duration remains available');
  assert.ok(r.elapsedSeconds > 0 && r.elapsedSeconds < 5, JSON.stringify(r));
  assert.equal(r.nodes[0].csiRateHz, Number((2 / r.elapsedSeconds).toFixed(2)));
 });
test('synthetic Realtek frames retain provenance in kernel analysis', async () => {
  const r = await capture({ analyze: true, max_packets: 3 }, [rac1({ seq: 1, synthetic: 1 }), rac1({ seq: 2 }), rac1({ seq: 3 })], {
    analyze: async () => ({ ok: true, estimate: 1 }),
  });
  assert.equal(r.analysis.ok, true);
  assert.equal(r.analysis.input.synthetic, true);
  assert.match(r.analysis.note, /synthetic/i);
});

test('unmarked Realtek frames keep ordinary analysis provenance', async () => {
  const r = await capture({ analyze: true, max_packets: 2 }, [rac1({ seq: 1 }), rac1({ seq: 2 })], { analyze: async () => ({ ok: true }) });
  assert.equal(r.analysis.input.synthetic, false);
  assert.doesNotMatch(r.analysis.note, /synthetic/i);
});

test('ordinary CSI shape summaries retain synthetic provenance across the window', async () => {
  const result = await capture({ max_packets: 2 }, [rac1({ seq: 1, synthetic: 1 }), rac1({ seq: 2 })]);
  assert.equal(result.nodes[0].csi.frames, 2);
  assert.equal(result.nodes[0].csi.synthetic, true);
});

test('ordinary unmarked CSI shape summaries remain unmarked', async () => {
  const result = await capture({ max_packets: 2 }, [rac1({ seq: 1 }), rac1({ seq: 2 })]);
  assert.equal(result.nodes[0].csi.synthetic, false);
});

test('kernel analysis selects the busiest stream after both buffers reach their cap', async () => {
  const datagrams = [];
  for (let seq = 0; seq < 70; seq++) datagrams.push(esp32Csi({ node: 1, seq }));
  for (let seq = 0; seq < 100; seq++) datagrams.push(esp32Csi({ node: 2, seq }));
  const result = await capture({ analyze: true, analyze_max_frames: 64, max_packets: datagrams.length }, datagrams, { analyze: async () => ({ ok: true }) });
  assert.equal(result.analysis.input.nodeId, 2);
  assert.equal(result.analysis.input.frames, 64);
  assert.equal(result.analysis.input.droppedOverCap, 36);
});

test('requested node selection remains authoritative after analysis buffers fill', async () => {
  const datagrams = [];
  for (let seq = 0; seq < 70; seq++) datagrams.push(esp32Csi({ node: 1, seq }));
  for (let seq = 0; seq < 100; seq++) datagrams.push(esp32Csi({ node: 2, seq }));
  const result = await capture({ analyze: true, node_id: 1, analyze_max_frames: 64, max_packets: datagrams.length }, datagrams, { analyze: async () => ({ ok: true }) });
  assert.equal(result.analysis.input.nodeId, 1);
  assert.equal(result.analysis.input.frames, 64);
  assert.equal(result.analysis.input.droppedOverCap, 6);
});

function fusedVitals() {
  // Exact packed edge_fused_vitals_pkt_t offsets from edge_processing.h.
  const packet = Buffer.alloc(48);
  packet.writeUInt32LE(0xC5110004, 0); packet[4] = 7; packet[5] = 9;
  packet.writeUInt16LE(1500, 6); packet.writeUInt32LE(720000, 8);
  packet.writeInt8(-45, 12); packet[13] = 1; packet[14] = 2; packet[15] = 85;
  packet.writeFloatLE(0.25, 16); packet.writeFloatLE(0.75, 20); packet.writeUInt32LE(12345, 24);
  packet.writeFloatLE(73, 28); packet.writeFloatLE(16, 32); packet.writeFloatLE(120, 36);
  packet[40] = 2; packet[41] = 90;
  return packet;
}

test('firmware fused vitals update public node diagnostics with wire units', async () => {
  const result = await capture({ max_packets: 1 }, [fusedVitals()]);
  assert.equal(result.ok, true);
  const node = result.nodes[0];
  assert.equal(node.packets['fused-vitals'], 1);
  assert.equal(node.rssiMean, -45);
  assert.equal(node.lastVitals.heartBpm, 72);
  assert.equal(node.lastVitals.breathingBpm, 15);
  assert.equal(node.lastVitals.uptimeMs, 12345);
  assert.equal(node.lastVitals.mmwavePresent, true);
  assert.equal(node.lastVitals.mmwaveHeartBpm, 73);
  assert.equal(node.lastVitals.mmwaveBreathingBpm, 16);
  assert.equal(node.lastVitals.mmwaveDistanceCm, 120);
  assert.equal(node.lastVitals.mmwaveTargets, 2);
  assert.equal(node.lastVitals.mmwaveConfidence, 90);
  assert.equal(node.lastVitals.fusionConfidence, 85);
});

test('truncated fused-vitals packets are not accepted as decoded node reports', () => {
  for (const length of [4, 28, 32, 47]) {
    assert.equal(parsePacket(fusedVitals().subarray(0, length)).kind, 'malformed');
  }
});

function auxiliaryPackets() {
  const features = Buffer.alloc(48); features.writeUInt32LE(0xC5110003, 0); features[4] = 7;
  const compressed = Buffer.alloc(12); compressed.writeUInt32LE(0xC5110005, 0); compressed[4] = 7;
  compressed.writeUInt16LE(4, 6); compressed.writeUInt16LE(2, 8); compressed[11] = 4; // identical prior frame: RLE [0, 4]
  const state = Buffer.alloc(60); state.writeUInt32LE(0xC5110006, 0); state[4] = 7; state.writeUInt32LE(crc32(state, 56), 56);
  const wasm = Buffer.alloc(13); wasm.writeUInt32LE(0xC5110007, 0); wasm[4] = 7; wasm.writeUInt16LE(1, 6); wasm[8] = 1; wasm.writeFloatLE(50, 9);
  return [features, compressed, state, wasm];
}

test('auxiliary node reports require complete wire payloads and feature-state CRC', async () => {
  const valid = auxiliaryPackets();
  const prefixes = valid.map((packet) => packet.subarray(0, 4));
  const corruptState = Buffer.from(valid[2]); corruptState[20] ^= 1;
  const shortCompressed = valid[1].subarray(0, 11);
  const shortWasm = valid[3].subarray(0, 12);
  const shortFeatures = valid[0].subarray(0, 47);
  const invalid = [...prefixes, corruptState, shortCompressed, shortWasm, shortFeatures];
  const result = await capture({ max_packets: invalid.length + valid.length }, [...invalid, ...valid]);
  assert.equal(result.ok, true);
  assert.equal(result.packets, 12);
  assert.equal(result.malformedPackets, 8);
  assert.equal(result.decodedPackets, 4);
  assert.equal(result.nodes.length, 1, 'invalid prefixes must not create phantom node -1');
  assert.deepEqual(result.nodes[0].packets, { features: 1, compressed: 1, 'feature-state': 1, 'wasm-output': 1 });
});
