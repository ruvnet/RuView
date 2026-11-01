import test from 'node:test';
import assert from 'node:assert/strict';
import { readIphoneLidar, readRplidar } from '../src/devices/lidar.js';

for (const [distances, expected] of [[[1000, 4000], 2500], [[4000, 1000, 2000], 2000], [[0, 1000, 4000], 2500]]) {
  test(`LiDAR public readers calculate median of ${distances}`, async () => {
    const bytes = Buffer.alloc(distances.length * 5);
    const mm = Buffer.alloc(distances.length * 2);
    distances.forEach((distance, i) => {
      bytes[i * 5] = 2; bytes[i * 5 + 1] = 1; bytes.writeUInt16LE(distance * 4, i * 5 + 3);
      mm.writeUInt16LE(distance, i * 2);
    });
    const scan = await readRplidar({ port: '/dev/ttyUSB0', seconds: 1 }, { python: () => 'fixture-python', exec: async () => ({ ok: true, stdout: `HEX ${bytes.toString('hex')}\n` }) });
    assert.equal(scan.rangeMm.median, expected);
    const packet = { type: 'ruview.lidar.depth.v1', depth: { width: distances.length, height: 1, encoding: 'u16le-mm+u8-confidence', millimetersBase64: mm.toString('base64'), confidenceBase64: Buffer.alloc(distances.length, 2).toString('base64') } };
    class FixtureWebSocket {
      constructor() { setImmediate(() => { this.onopen?.(); this.onmessage?.({ data: JSON.stringify(packet) }); }); }
      close() {}
    }
    const depth = await readIphoneLidar({ url: 'ws://127.0.0.1:8787/ws/lidar', max_frames: 1 }, { WebSocket: FixtureWebSocket, env: {} });
    assert.equal(depth.last.medianDepthM, expected / 1000);
  });
}
