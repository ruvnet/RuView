import test from 'node:test';
import assert from 'node:assert/strict';
import { readRplidar } from '../src/devices/lidar.js';

function scan(count) {
  const bytes = Buffer.alloc(count * 5);
  for (let i = 0; i < count; i++) {
    const offset = i * 5;
    bytes[offset] = 2; // start flag false and inverse flag true
    bytes[offset + 1] = 1; // angle validity bit
    bytes.writeUInt16LE((1000 + i % 3) * 4, offset + 3);
  }
  return bytes;
}

for (const count of [3, 200_000]) {
  test(`public RPLIDAR reader reduces ${count} valid points within capture byte cap`, async () => {
    const bytes = scan(count);
    const result = await readRplidar({ port: '/dev/ttyUSB0', seconds: 2 }, {
      python: () => 'fixture-python',
      exec: async () => ({ ok: true, stdout: `HEX ${bytes.toString('hex')}\nEND ${bytes.length}\n` }),
    });
    assert.equal(result.ok, true);
    assert.equal(result.validPoints, count);
    assert.deepEqual(result.rangeMm, { min: 1000, median: 1001, max: 1002 });
    assert.equal(result.angularCoverage, 0.03);
  });
}
