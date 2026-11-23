import test from 'node:test';
import assert from 'node:assert/strict';
import net from 'node:net';
import { encodeFrame, encodeVarint, readEsphome, splitFrames } from '../src/devices/esphome.js';
const str = (f, s) => { const b = Buffer.from(s); return Buffer.concat([encodeVarint((f << 3) | 2), encodeVarint(b.length), b]); };
const uint = (f, n) => Buffer.concat([encodeVarint(f << 3), encodeVarint(n)]);
const key = (f, k) => { const b = Buffer.alloc(4); b.writeUInt32LE(k); return Buffer.concat([encodeVarint((f << 3) | 5), b]); };
const float = (f, v) => { const b = Buffer.alloc(4); b.writeFloatLE(v); return Buffer.concat([encodeVarint((f << 3) | 5), b]); };
async function device(t, numeric) {
  const sockets = new Set();
  const server = net.createServer(sock => {
    sockets.add(sock); sock.on('close', () => sockets.delete(sock)); sock.on('error', () => {});
    let buf = Buffer.alloc(0);
    sock.on('data', bytes => {
      buf = Buffer.concat([buf, bytes]); const decoded = splitFrames(buf); buf = decoded.rest;
      for (const { type } of decoded.frames) {
        if (type === 1) sock.write(encodeFrame(2));
        if (type === 3) sock.write(encodeFrame(4));
        if (type === 11) {
          // Common alarm/status entities precede the corresponding scalar.
          sock.write(encodeFrame(12, Buffer.concat([str(1, 'heart_alarm'), key(2, 1), str(3, 'Heart alarm')])));
          sock.write(encodeFrame(18, Buffer.concat([str(1, 'distance_status'), key(2, 2), str(3, 'Distance status')])));
          sock.write(encodeFrame(12, Buffer.concat([str(1, 'breathing_alarm'), key(2, 3), str(3, 'Breathing alarm')])));
          if (numeric) {
            for (const [k, name] of [[4, 'Heart rate'], [5, 'Distance'], [6, 'Breathing rate']]) {
              sock.write(encodeFrame(16, Buffer.concat([str(1, name.toLowerCase().replaceAll(' ', '_')), key(2, k), str(3, name), str(6, k === 5 ? 'cm' : 'bpm')])));
            }
          }
          sock.write(encodeFrame(19));
        }
        if (type === 20) {
          sock.write(encodeFrame(21, Buffer.concat([key(1, 1), uint(2, 1)])));
          sock.write(encodeFrame(27, Buffer.concat([key(1, 2), str(2, '50')])));
          sock.write(encodeFrame(21, Buffer.concat([key(1, 3), uint(2, 1)])));
          if (numeric) for (const [k, value] of [[4, 72], [5, 120], [6, 15]]) sock.write(encodeFrame(25, Buffer.concat([key(1, k), float(2, value)])));
        }
      }
    });
  });
  await new Promise(r => server.listen(0, '127.0.0.1', r));
  t.after(async () => { for (const s of sockets) s.destroy(); await new Promise(r => server.close(r)); });
  return readEsphome({ host: '127.0.0.1', api_port: server.address().port, seconds: 1 });
}

test('public ESPHome reader selects numeric sensors over named alarm/status entities', async t => {
  const result = await device(t, true);
  assert.equal(result.ok, true, JSON.stringify(result));
  assert.deepEqual([result.heartBpmMean, result.distanceCmMean, result.breathingBpmMean], [72, 120, 15]);
  assert.deepEqual(result.entities.slice(0, 3).map(e => e.role), [undefined, undefined, undefined]);
  assert.deepEqual(result.entities.slice(0, 3).map(e => e.last), [true, '50', true], 'non-numeric entities remain available as raw states');
});

test('named non-numeric states alone do not manufacture numeric radar measurements', async t => {
  const result = await device(t, false);
  assert.equal(result.ok, true);
  assert.deepEqual([result.heartBpmMean, result.distanceCmMean, result.breathingBpmMean], [null, null, null]);
  assert.equal(result.stateUpdates, 3);
});
