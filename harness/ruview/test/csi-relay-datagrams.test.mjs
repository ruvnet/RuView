import { python } from './helpers/python.mjs';
import test from 'node:test';
import assert from 'node:assert/strict';
import dgram from 'node:dgram';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { once } from 'node:events';
const script = fileURLToPath(new URL('../../../scripts/csi-udp-relay.py', import.meta.url));
async function relay(payload) {
  const rx = dgram.createSocket('udp4'), probe = dgram.createSocket('udp4'), tx = dgram.createSocket('udp4');
  rx.bind(0, '127.0.0.1'); await once(rx, 'listening');
  probe.bind(0, '127.0.0.1'); await once(probe, 'listening');
  const listen = probe.address().port; await new Promise(resolve => probe.close(resolve));
  const child = spawn(python, ['-u', script, '--listen-host', '127.0.0.1', '--listen', String(listen), '--forward', String(rx.address().port), '--quiet']);
  let timer;
  try {
    await new Promise((resolve, reject) => {
      timer = setTimeout(() => reject(new Error('relay startup timeout')), 2000);
      child.stdout.once('data', () => { clearTimeout(timer); resolve(); });
      child.once('error', reject);
    });
    const result = new Promise((resolve, reject) => {
      timer = setTimeout(() => reject(new Error('relay did not forward datagram')), 700);
      rx.once('message', value => { clearTimeout(timer); resolve(value); });
    });
    tx.send(payload, listen, '127.0.0.1');
    return await result;
  } finally {
    clearTimeout(timer); tx.close(); rx.close();
    const exited = once(child, 'exit'); child.kill('SIGINT'); await exited;
  }
}
test('public loopback relay preserves full maximum ADR018 multi-antenna packet', async () => {
  const packet = Buffer.alloc(20 + 4 * 256 * 2);
  packet.writeUInt32LE(0xC5110001); packet[4] = 7; packet[5] = 4; packet.writeUInt16LE(256, 6);
  packet.writeUInt32LE(2437, 8); packet.writeUInt32LE(12, 12); packet.writeInt8(-50, 16);
  for (let i = 20; i < packet.length; i++) packet[i] = i % 256;
  assert.deepEqual(await relay(packet), packet);
});
test('public loopback relay preserves empty UDP datagrams', async () => {
  assert.deepEqual(await relay(Buffer.alloc(0)), Buffer.alloc(0));
});
test('public loopback relay retains ordinary small datagrams unchanged', async () => {
  const packet = Buffer.from([0, 255, 3, 4]); assert.deepEqual(await relay(packet), packet);
});
