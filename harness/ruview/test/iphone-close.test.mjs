import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import { createHash } from 'node:crypto';
import { readIphoneLidar } from '../src/devices/lidar.js';

const hasNativeWebSocket = typeof globalThis.WebSocket === 'function';
const needsWebSocket = { skip: !hasNativeWebSocket && 'requires the native WebSocket client (Node 22+)' };

// A real owned loopback WebSocket producer: upgrade, optional depth frame,
// close handshake. No injected client event callbacks or hardware.
async function relay(t, { frame = true, code = 1000 } = {}) {
  const server = http.createServer();
  const sockets = new Set();
  server.on('connection', s => { sockets.add(s); s.on('close', () => sockets.delete(s)); });
  t.after(async () => { for (const s of sockets) s.destroy(); await new Promise(r => server.close(r)); });
  server.on('upgrade', (req, socket) => {
    const accept = createHash('sha1').update(req.headers['sec-websocket-key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64');
    socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
    if (frame) {
      const mm = Buffer.alloc(2); mm.writeUInt16LE(1234);
      const packet = { type: 'ruview.lidar.depth.v1', depth: { encoding: 'u16le-mm+u8-confidence', width: 1, height: 1, millimetersBase64: mm.toString('base64'), confidenceBase64: Buffer.from([1]).toString('base64') } };
      const payload = Buffer.from(JSON.stringify(packet));
      const header = Buffer.alloc(4); header[0] = 0x81; header[1] = 126; header.writeUInt16BE(payload.length, 2);
      socket.write(Buffer.concat([header, payload]));
    }
    const close = Buffer.from([0x88, 2, 0, 0]); close.writeUInt16BE(code, 2);
    socket.end(close);
  });
  await new Promise(r => server.listen(0, '127.0.0.1', r));
  return `ws://127.0.0.1:${server.address().port}/ws/lidar`;
}

test('public iPhone reader reports an unavailable WebSocket without connecting', { skip: hasNativeWebSocket && 'native WebSocket is available' }, async t => {
  const server = http.createServer();
  let connections = 0;
  server.on('connection', socket => { connections++; socket.destroy(); });
  await new Promise(r => server.listen(0, '127.0.0.1', r));
  t.after(() => new Promise(r => server.close(r)));
  const result = await readIphoneLidar({ url: `ws://127.0.0.1:${server.address().port}/ws/lidar`, seconds: 2 });
  assert.deepEqual(result, { ok: false, reason: 'websocket_unavailable', remedy: 'Use Node 22+ (built-in WebSocket client).' });
  await new Promise(r => setImmediate(r));
  assert.equal(connections, 0, 'unavailable transport must not open a connection');
});

test('public iPhone reader completes when a successful relay stream closes', needsWebSocket, async t => {
  const url = await relay(t);
  const started = performance.now();
  const result = await readIphoneLidar({ url, seconds: 2 });
  assert.equal(result.ok, true, JSON.stringify(result));
  assert.equal(result.frames, 1);
  assert.equal(result.last.medianDepthM, 1.234);
  assert.ok(performance.now() - started < 1000, 'closed relay must finish before the capture timeout');
});

test('normal empty relay close completes as no_frames', needsWebSocket, async t => {
  const url = await relay(t, { frame: false });
  const started = performance.now();
  const result = await readIphoneLidar({ url, seconds: 2 });
  assert.equal(result.ok, false);
  assert.equal(result.reason, 'no_frames');
  assert.ok(performance.now() - started < 1000, 'empty closed relay must finish before timeout');
});

test('abnormal empty relay close and packet limit controls retain semantics', needsWebSocket, async t => {
  const denied = await readIphoneLidar({ url: await relay(t, { frame: false, code: 1008 }), seconds: 2 });
  assert.equal(denied.reason, 'relay_closed');
  assert.match(denied.detail, /1008/);
  const limited = await readIphoneLidar({ url: await relay(t), seconds: 2, max_frames: 1 });
  assert.equal(limited.ok, true);
  assert.equal(limited.frames, 1);
});
