// Real loopback native-API exchanges; no ESPHome hardware is involved.
import test from 'node:test';
import assert from 'node:assert/strict';
import net from 'node:net';
import { EventEmitter } from 'node:events';
import { encodeFrame, splitFrames, readEsphome } from '../src/devices/esphome.js';

async function exchange(t, packets, { end = false } = {}) {
  const seen = [];
  const peers = new Set();
  let peerClosed;
  const server = net.createServer(socket => {
    peers.add(socket);
    let input = Buffer.alloc(0);
    peerClosed = new Promise(resolve => socket.once('close', resolve));
    socket.on('error', error => { if (error.code !== 'ECONNRESET') throw error; });
    socket.on('data', chunk => {
      input = Buffer.concat([input, chunk]);
      const decoded = splitFrames(input);
      input = decoded.rest;
      seen.push(...decoded.frames.map(frame => frame.type));
    });
    if (end) socket.end(Buffer.concat(packets));
    else socket.write(Buffer.concat(packets));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(async () => {
    for (const peer of peers) peer.destroy();
    await new Promise(resolve => server.close(resolve));
  });
  const result = await readEsphome({ host: '127.0.0.1', api_port: server.address().port, seconds: 1 });
  await peerClosed;
  return { result, seen };
}
const hello = () => encodeFrame(2, Buffer.from([8, 1, 16, 16]));
const disconnect = () => encodeFrame(5);
const ping = () => encodeFrame(7);
const trailingState = () => {
  const payload = Buffer.alloc(10);
  payload[0] = 13; payload.writeUInt32LE(1, 1);
  payload[5] = 21; payload.writeFloatLE(72, 6);
  return encodeFrame(25, payload);
};

for (const handshaken of [false, true]) {
  test(`server disconnect ${handshaken ? 'after' : 'before'} Hello is acknowledged and fails capture`, { timeout: 5000 }, async t => {
    const { result, seen } = await exchange(t, [...(handshaken ? [hello()] : []), disconnect(), trailingState(), ping()]);
    assert.equal(result.ok, false, 'a device-requested disconnect must not become timeout success');
    assert.equal(result.reason, 'esphome_closed');
    assert.equal(result.evidence, undefined);
    assert.deepEqual(seen.filter(type => [5, 6, 8].includes(type)), [6], 'acknowledge with6, not new request5 or trailing ping response8');
  });
}

test('ordinary completed capture retains client disconnect and ping behavior', { timeout: 5000 }, async t => {
  const { result, seen } = await exchange(t, [hello(), ping()]);
  assert.equal(result.ok, true);
  assert.match(result.evidence, /^MEASURED:/);
  assert.ok(seen.includes(8), 'ordinary ping answered');
  assert.deepEqual(seen.filter(type => [5, 6].includes(type)), [5]);
});

test('ordinary early peer close remains a failed capture', { timeout: 5000 }, async t => {
  const { result, seen } = await exchange(t, [hello()], { end: true });
  assert.equal(result.ok, false);
  assert.equal(result.reason, 'esphome_closed');
  assert.equal(seen.includes(6), false, 'no unsolicited disconnect acknowledgement');
});


test('blocked acknowledgement flush is bounded and late events cannot restart capture', { timeout: 5000 }, async t => {
  // A real socket holds the loop alive; this deliberately blocked fake needs its own guard.
  let guard;
  const deadline = new Promise((_, reject) => { guard = setTimeout(() => reject(new Error('bounded socket cleanup did not run')), 3000); });
  t.after(() => clearTimeout(guard));
  const socket = new EventEmitter();
  const writes = [];
  let destroyed = false;
  socket.write = data => { writes.push(...splitFrames(data).frames.map(frame => frame.type)); };
  socket.end = data => { writes.push(...splitFrames(data).frames.map(frame => frame.type)); }; // Deliberately never completes flushing.
  let closed;
  const closedPromise = new Promise(resolve => { closed = resolve; });
  socket.destroy = () => { destroyed = true; socket.emit('close'); closed(); };
  const pending = readEsphome({ host: '127.0.0.1', seconds: 1 }, { connect: () => socket });
  await new Promise(resolve => setImmediate(resolve));
  socket.emit('connect');
  socket.emit('data', Buffer.concat([hello(), disconnect(), trailingState(), ping()]));
  const result = await pending;
  assert.equal(result.reason, 'esphome_closed');
  assert.equal(destroyed, false, 'allow acknowledgement flushing before the close bound');
  assert.deepEqual(writes, [1, 3, 9, 11, 6]);
  socket.emit('data', ping());
  socket.emit('connect');
  assert.deepEqual(writes, [1, 3, 9, 11, 6], 'terminal session ignores late data and connect events');
  await Promise.race([closedPromise, deadline]);
  clearTimeout(guard);
  assert.equal(destroyed, true, 'uncompleted flush cannot retain the socket indefinitely');
});
