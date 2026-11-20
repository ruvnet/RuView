// ADR-373 Amendment 3: ESPHome native API reader, driven against a fake
// ESPHome device (a real TCP server speaking the plaintext protocol).
import test from 'node:test';
import assert from 'node:assert/strict';
import net from 'node:net';
import { decodeFields, encodeFrame, encodeVarint, isPrivateAddress, readEsphome, splitFrames, summarizeEsphome } from '../src/devices/esphome.js';
import { runTool } from '../src/tools.js';
import { renderResult } from '../src/cli-ui.js';

const pbStr = (f, s) => { const b = Buffer.from(s); return Buffer.concat([encodeVarint((f << 3) | 2), encodeVarint(b.length), b]); };
const pbU = (f, n) => Buffer.concat([encodeVarint(f << 3), encodeVarint(n)]);
const pbKey = (f, k) => { const b = Buffer.alloc(4); b.writeUInt32LE(k); return Buffer.concat([encodeVarint((f << 3) | 5), b]); };
const pbFloat = (f, v) => { const b = Buffer.alloc(4); b.writeFloatLE(v); return Buffer.concat([encodeVarint((f << 3) | 5), b]); };

/** A fake MR60BHA2 kit. mode: 'ok' | 'encrypted' | 'password'. */
function fakeDevice({ mode = 'ok', states, distanceUnit = 'cm' } = {}) {
  const seen = [];
  const server = net.createServer((sock) => {
    sock.on('error', (error) => {
      // Rejecting encryption/password intentionally closes the reader socket.
      // Only that fixture lifecycle may reset; all unexpected errors still fail.
      if ((mode === 'encrypted' || mode === 'password') && error.code === 'ECONNRESET') return;
      throw error;
    });
    if (mode === 'encrypted') { sock.write(Buffer.from([0x01, 0x00, 0x00])); return; }
    let buf = Buffer.alloc(0);
    sock.on('data', (d) => {
      buf = Buffer.concat([buf, d]);
      const { frames, rest } = splitFrames(buf);
      buf = rest;
      for (const { type } of frames) {
        seen.push(type);
        if (type === 1) sock.write(encodeFrame(2, Buffer.concat([pbU(1, 1), pbU(2, 16), pbStr(3, '2026.9.0'), pbStr(4, 'fake-kit')])));
        if (type === 3) sock.write(encodeFrame(4, mode === 'password' ? pbU(1, 1) : Buffer.alloc(0)));
        if (type === 9) sock.write(encodeFrame(10, Buffer.concat([pbStr(2, 'fake-kit'), pbStr(4, '2026.9.0'), pbStr(8, 'seeedstudio.mr60bha2_kit'), pbStr(9, 'spaces-1.0')])));
        if (type === 11) {
          sock.write(encodeFrame(12, Buffer.concat([pbStr(1, 'person_information'), pbKey(2, 1), pbStr(3, 'Person Information')])));
          sock.write(encodeFrame(16, Buffer.concat([pbStr(1, 'real_time_heart_rate'), pbKey(2, 2), pbStr(3, 'Real-time heart rate'), pbStr(6, 'bpm')])));
          sock.write(encodeFrame(16, Buffer.concat([pbStr(1, 'real_time_respiratory_rate'), pbKey(2, 3), pbStr(3, 'Real-time respiratory rate')])));
          sock.write(encodeFrame(16, Buffer.concat([pbStr(1, 'distance'), pbKey(2, 4), pbStr(3, 'Distance to detection object'), pbStr(6, distanceUnit)])));
          sock.write(encodeFrame(15, pbStr(1, 'rgb_light'))); // a light: ignored by the read-only client
          sock.write(encodeFrame(19));
        }
        if (type === 20) {
          sock.write(encodeFrame(7)); // device pings the client; it must answer
          sock.write(Buffer.concat(states.map((s) => encodeFrame(s.type, s.payload))));
        }
      }
    });
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve({ server, port: server.address().port, seen })));
}

const sensor = (key, value, missing = false) => ({ type: 25, payload: Buffer.concat([pbKey(1, key), ...(value ? [pbFloat(2, value)] : []), ...(missing ? [pbU(3, 1)] : [])]) });
const binary = (key, on) => ({ type: 21, payload: Buffer.concat([pbKey(1, key), ...(on ? [pbU(2, 1)] : [])]) });

test('reads presence and vitals from an ESPHome radar kit, honouring missing_state and proto3 zeros', async (t) => {
  const dev = await fakeDevice({ states: [binary(1, true), sensor(2, 72), sensor(2, 78), sensor(3, 15), sensor(3, 0, true), sensor(4, 40.5), sensor(4, 0)] });
  t.after(() => dev.server.close());
  const r = await readEsphome({ host: '127.0.0.1', api_port: dev.port, seconds: 1 });
  assert.equal(r.ok, true, JSON.stringify(r));
  assert.deepEqual([r.device.project, r.device.projectVersion, r.device.apiVersion], ['seeedstudio.mr60bha2_kit', 'spaces-1.0', '1.16']);
  assert.equal(r.presentNow, true);
  assert.equal(r.heartBpmMean, 75);
  assert.equal(r.breathingBpmMean, 15, 'missing_state samples are excluded');
  assert.equal(r.distanceCmMean, 40.5, 'zero distance (proto3 default) is not averaged in');
  assert.equal(r.stateUpdates, 7);
  assert.equal(r.entities.find((e) => e.role === 'distanceCm').updates, 2);
  assert.equal(r.entities.find((e) => e.role === 'distanceCm').droppedSamples, 0);
  assert.deepEqual(r.entities.map((e) => e.role), ['present', 'heartBpm', 'breathingBpm', 'distanceCm']);
  assert.ok(dev.seen.includes(8), 'answered the device ping');
  assert.ok(!dev.seen.some((ty) => [30, 31, 32, 33].includes(ty)), 'never sends a command');
  assert.match(r.evidence, /^MEASURED/);
  assert.match(renderResult('ruview_mmwave_read', r, { color: false }), /presence {2}● detected/);
});

test('encrypted and password-protected devices fail honestly', async (t) => {
  const enc = await fakeDevice({ mode: 'encrypted', states: [] });
  const pwd = await fakeDevice({ mode: 'password', states: [] });
  t.after(() => { enc.server.close(); pwd.server.close(); });
  assert.equal((await readEsphome({ host: '127.0.0.1', api_port: enc.port, seconds: 1 })).reason, 'esphome_encrypted');
  assert.equal((await readEsphome({ host: '127.0.0.1', api_port: pwd.port, seconds: 1 })).reason, 'esphome_password_required');
});

test('only private, link-local, CGNAT and loopback hosts are allowed', async () => {
  for (const ip of ['10.1.2.3', '172.16.0.1', '172.31.255.255', '192.168.1.102', '169.254.1.1', '127.0.0.1', '100.84.53.55', '::1', 'fd00::1', 'fe80::1', '::ffff:192.168.1.5']) assert.equal(isPrivateAddress(ip), true, ip);
  for (const ip of ['8.8.8.8', '172.32.0.1', '100.128.0.1', '2001:4860::8888', '::ffff:8.8.8.8']) assert.equal(isPrivateAddress(ip), false, ip);
  assert.equal((await readEsphome({ host: '8.8.8.8', seconds: 1 })).reason, 'host_not_private');
  assert.equal((await readEsphome({ host: 'public.example', seconds: 1 }, { lookup: async () => ({ address: '93.184.216.34' }) })).reason, 'host_not_private');
  assert.equal((await readEsphome({ host: 'bad host;rm', seconds: 1 })).reason, 'invalid_host');
  assert.equal((await readEsphome({ host: '127.0.0.1', api_port: 70000 })).reason, 'invalid_port');
});

test('unreachable devices and malformed frames are reported, not thrown', async () => {
  const closed = await new Promise((r) => { const s = net.createServer(); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => r(p)); }); });
  assert.equal((await readEsphome({ host: '127.0.0.1', api_port: closed, seconds: 1 })).reason, 'esphome_unreachable');
  assert.throws(() => decodeFields(Buffer.from([0x0a, 0x05, 0x41])), /truncated/);
  assert.throws(() => splitFrames(Buffer.from([0x07, 0x00])), /bad frame indicator/);
  assert.deepEqual(splitFrames(Buffer.from([0x00, 0x05])).frames, [], 'partial frame waits for more bytes');
});

test('mmwave tool routes source=esphome and validates per-source arguments', async () => {
  assert.equal((await runTool('ruview_mmwave_read', { source: 'esphome' }, { source: 'cli' })).reason, 'invalid_arguments');
  assert.equal((await runTool('ruview_mmwave_read', {}, { source: 'cli' })).reason, 'invalid_arguments');
  assert.equal((await runTool('ruview_mmwave_read', { source: 'esphome', host: '8.8.8.8', seconds: 1 }, { source: 'cli' })).reason, 'host_not_private');
  const denied = await runTool('ruview_mmwave_read', { source: 'esphome', host: '192.168.1.2' }, { source: 'mcp', grants: [] });
  assert.equal(denied.reason, 'authority_denied');
  const empty = summarizeEsphome({ device: {}, entities: new Map(), updates: 0 }, 5);
  assert.deepEqual([empty.presentNow, empty.heartBpmMean, empty.updateRateHz], [null, null, 0]);
});

for (const [unit, distance] of [['mm', 500], ['m', 0.5], ['cm', 50]]) {
  test(`native ESPHome distance ${unit} is reported in centimetres`, async (t) => {
    const dev = await fakeDevice({ states: [sensor(4, distance)], distanceUnit: unit });
    t.after(() => dev.server.close());
    const result = await readEsphome({ host: '127.0.0.1', api_port: dev.port, seconds: 1 });
    assert.equal(result.ok, true, JSON.stringify(result));
    assert.equal(result.distanceCmMean, 50);
  });
}
test('entity update counters retain arrivals beyond the aggregation sample cap', async (t) => {
  const dev = await fakeDevice({ states: Array.from({ length: 10_001 }, () => sensor(4, 50)) });
  t.after(() => dev.server.close());
  const result = await readEsphome({ host: '127.0.0.1', api_port: dev.port, seconds: 1 });
  assert.equal(result.ok, true, JSON.stringify(result));
  assert.equal(result.stateUpdates, 10_001);
  const distance = result.entities.find((entity) => entity.role === 'distanceCm');
  assert.equal(distance.updates, 10_001);
  assert.equal(distance.sampledUpdates, 10_000);
  assert.equal(distance.droppedSamples, 1);
  assert.equal(result.distanceCmMean, 50);
});
for (const value of [NaN, Infinity, -Infinity]) {
  test(`native ESPHome treats nonfinite sensor value ${value} as unavailable`, async (t) => {
    const dev = await fakeDevice({ states: [sensor(4, 50), { type: 25, payload: Buffer.concat([pbKey(1, 4), pbFloat(2, value)]) }] });
    t.after(() => dev.server.close());
    const result = await readEsphome({ host: '127.0.0.1', api_port: dev.port, seconds: 1 });
    assert.equal(result.ok, true);
    assert.equal(result.distanceCmMean, 50);
    assert.equal(result.entities.find((entity) => entity.role === 'distanceCm').last, null);
    assert.equal(result.stateUpdates, 2);
  });
}
// Controlled resets reproduce the owned server-side rejection lifecycle without
// relying on OS scheduling to deliver a late ECONNRESET after the test ends.
async function withFixtureSocket(mode, inspect) {
  const dev = await fakeDevice({ mode, states: [] });
  let peer;
  dev.server.once('connection', socket => { peer = socket; });
  try {
    const report = await readEsphome({ host: '127.0.0.1', api_port: dev.port, seconds: 0.01 });
    assert.ok(peer, 'actual reader must connect to the owned fixture');
    inspect(peer, report);
  } finally {
    // Cleanup must not emit a second uncontrolled error in the failing baseline.
    peer?.on('error', () => {});
    peer?.destroy();
    await new Promise(resolve => dev.server.close(resolve));
  }
}
for (const mode of ['encrypted', 'password']) {
  test(`owned ${mode} rejection fixture handles its expected connection reset`, async () => {
    await withFixtureSocket(mode, (socket, report) => {
      assert.equal(report.reason, mode === 'encrypted' ? 'esphome_encrypted' : 'esphome_password_required');
      const reset = Object.assign(new Error('controlled rejection reset'), { code: 'ECONNRESET' });
      assert.doesNotThrow(() => socket.emit('error', reset));
    });
  });
}
test('owned rejection fixture still fails unexpected socket error codes', async () => {
  await withFixtureSocket('encrypted', socket => {
    const unexpected = Object.assign(new Error('unexpected fixture I/O'), { code: 'EIO' });
    assert.throws(() => socket.emit('error', unexpected), /unexpected fixture I\/O/);
  });
});
test('ordinary owned fixture still fails connection resets', async () => {
  await withFixtureSocket('ok', socket => {
    const reset = Object.assign(new Error('unexpected ordinary reset'), { code: 'ECONNRESET' });
    assert.throws(() => socket.emit('error', reset), /unexpected ordinary reset/);
  });
});
