import { python } from './helpers/python.mjs';
import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
const script = fileURLToPath(new URL('../../../scripts/esp32_jsonl_to_rvcsi.py', import.meta.url));
function transcode(iq) {
  const dir = mkdtempSync(join(tmpdir(), 'ruview-transcode-'));
  try {
    const input = join(dir, 'in.jsonl'), output = join(dir, 'out.rvcsi');
    const frame = iq_hex => ({ type: 'raw_csi', iq_hex, rssi: -50, ts_ns: 123, channel: 6 });
    writeFileSync(input, [frame('0304'), frame(iq), frame('0608')].map(JSON.stringify).join('\n')+'\n');
    const result = spawnSync(python, [script, input, output], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    const frames = readFileSync(output, 'utf8').trim().split('\n').map(JSON.parse).slice(1);
    return { frames, stats: JSON.parse(result.stdout.split('transcode stats: ')[1].split('\n')[0]) };
  } finally { rmSync(dir, { recursive: true, force: true }); }
}
for (const [name, iq] of [['incomplete pair', '03047f'], ['invalid hex', 'not-hex']]) {
  test(`public transcode CLI quarantines ${name} and keeps later valid samples`, () => {
    const { frames, stats } = transcode(iq);
    assert.deepEqual(frames.map(f => f.amplitude), [[5], [10]]);
    assert.deepEqual(frames.map(f => f.frame_id), [0, 1]);
    assert.equal(stats.read, 3); assert.equal(stats.written, 2); assert.equal(stats.rej_len, 1);
  });
}
test('public transcode CLI retains complete signed IQ pairs', () => {
  const { frames, stats } = transcode('fa08');
  assert.deepEqual(frames[1].i_values, [8]); assert.deepEqual(frames[1].q_values, [-6]);
  assert.deepEqual(frames[1].amplitude, [10]); assert.equal(stats.rej_len, 0); assert.equal(stats.written, 3);
});
