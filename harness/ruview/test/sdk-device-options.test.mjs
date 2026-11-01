import test from 'node:test';
import assert from 'node:assert/strict';
import net from 'node:net';
import http from 'node:http';
import dgram from 'node:dgram';
import { createHash } from 'node:crypto';
import { createRuView } from '../src/sdk.js';
import { encodeFrame, encodeVarint, splitFrames } from '../src/devices/esphome.js';
const str = (f, s) => { const b = Buffer.from(s); return Buffer.concat([encodeVarint((f << 3) | 2), encodeVarint(b.length), b]); };
const key = (f, n) => { const b = Buffer.alloc(4); b.writeUInt32LE(n); return Buffer.concat([encodeVarint((f << 3) | 5), b]); };
const float = (f, v) => { const b = Buffer.alloc(4); b.writeFloatLE(v); return Buffer.concat([encodeVarint((f << 3) | 5), b]); };
const pause = ms => new Promise(r => setTimeout(r, ms));
const hasNativeWebSocket = typeof globalThis.WebSocket === 'function';

async function ownServer(t, server) {
  const sockets = new Set();
  server.on('connection', s => { sockets.add(s); s.on('close', () => sockets.delete(s)); s.on('error', () => {}); });
  await new Promise(r => server.listen(0, '127.0.0.1', r));
  t.after(async () => { for (const s of sockets) s.destroy(); await new Promise(r => server.close(r)); });
  return server.address().port;
}

test('public SDK accepts ESPHome source arguments and reads a numeric state', async t => {
  const server = net.createServer(sock => {
    let buf = Buffer.alloc(0);
    sock.on('data', data => {
      buf = Buffer.concat([buf, data]); const parsed = splitFrames(buf); buf = parsed.rest;
      for (const { type } of parsed.frames) {
        if (type === 1) sock.write(encodeFrame(2));
        if (type === 3) sock.write(encodeFrame(4));
        if (type === 11) {
          sock.write(encodeFrame(16, Buffer.concat([str(1, 'distance'), key(2, 1), str(3, 'Distance'), str(6, 'cm')])));
          sock.write(encodeFrame(19));
        }
        if (type === 20) sock.write(encodeFrame(25, Buffer.concat([key(1, 1), float(2, 50)])));
      }
    });
  });
  const port = await ownServer(t, server);
  const result = await createRuView().devices.mmwave({ source: 'esphome', host: '127.0.0.1', api_port: port, seconds: 1 });
  assert.equal(result.ok, true, JSON.stringify(result));
  assert.equal(result.source, 'esphome');
  assert.equal(result.distanceCmMean, 50);
});

test('public SDK accepts spectrum options and returns real UDP amplitude rows', async t => {
  const probe = dgram.createSocket('udp4'); await new Promise(r => probe.bind(0, '127.0.0.1', r));
  const port = probe.address().port; await new Promise(r => probe.close(r));
  const sender = dgram.createSocket('udp4'); t.after(() => sender.close());
  const pending = createRuView().devices.esp32({ bind: '127.0.0.1', udp_port: port, seconds: 1, max_packets: 1, spectrum: true, spectrum_bins: 8, spectrum_frames: 8 });
  await pause(100);
  const packet = Buffer.alloc(20 + 16 * 2); packet.writeUInt32LE(0xC5110001, 0); packet[4] = 1; packet[5] = 1; packet.writeUInt16LE(16, 6); packet.writeUInt32LE(2437, 8);
  for (let i = 20; i < packet.length; i += 2) { packet[i] = 3; packet[i + 1] = 4; }
  sender.send(packet, port, '127.0.0.1');
  const result = await pending;
  assert.equal(result.ok, true, JSON.stringify(result));
  assert.equal(result.spectrum[0].bins, 8);
  assert.deepEqual(result.spectrum[0].frames, [Array(8).fill(5)]);
});

test('public SDK reports unavailable iPhone transport without connecting', { skip: hasNativeWebSocket && 'native WebSocket is available' }, async t => {
  const server = http.createServer();
  let connections = 0;
  server.on('connection', socket => { connections++; socket.destroy(); });
  const port = await ownServer(t, server);
  const result = await createRuView().devices.lidar({ source: 'iphone', url: `ws://127.0.0.1:${port}/ws/lidar`, seconds: 2, max_frames: 1 });
  assert.deepEqual(result, { ok: false, reason: 'websocket_unavailable', remedy: 'Use Node 22+ (built-in WebSocket client).' });
  await new Promise(r => setImmediate(r));
  assert.equal(connections, 0, 'unavailable transport must not open a connection');
});

test('public SDK accepts iPhone max_frames and terminates at the packet cap', { skip: !hasNativeWebSocket && 'requires the native WebSocket client (Node 22+)' }, async t => {
  const server = http.createServer();
  server.on('upgrade', (req, sock) => {
    const accept = createHash('sha1').update(req.headers['sec-websocket-key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').digest('base64');
    sock.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
    const mm = Buffer.alloc(2); mm.writeUInt16LE(1500);
    const payload = Buffer.from(JSON.stringify({ type: 'ruview.lidar.depth.v1', depth: { encoding: 'u16le-mm+u8-confidence', width: 1, height: 1, millimetersBase64: mm.toString('base64'), confidenceBase64: Buffer.from([1]).toString('base64') } }));
    const head = Buffer.alloc(4); head[0] = 0x81; head[1] = 126; head.writeUInt16BE(payload.length, 2);
    // Leave the producer open: max_frames, rather than close/timeout, ends it.
    sock.write(Buffer.concat([head, payload]));
  });
  const port = await ownServer(t, server);
  const start = performance.now();
  const result = await createRuView().devices.lidar({ source: 'iphone', url: `ws://127.0.0.1:${port}/ws/lidar`, seconds: 2, max_frames: 1 });
  assert.equal(result.ok, true, JSON.stringify(result));
  assert.equal(result.frames, 1);
  assert.equal(result.last.medianDepthM, 1.5);
  assert.ok(performance.now() - start < 1000, 'packet cap terminates before timeout');
});
