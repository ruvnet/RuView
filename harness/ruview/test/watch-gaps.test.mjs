import test from 'node:test';
import assert from 'node:assert/strict';
import dgram from 'node:dgram';
import { spawn } from 'node:child_process';
import { sparkline } from '../src/cli-ui.js';
const pause = ms => new Promise(r => setTimeout(r, ms));

// Exercise the actual watch command with terminal rendering enabled. Only the
// stdout terminal capability is supplied; capture and parsing remain real.
test('CLI watch preserves an absent capture window in a returning node trend', { timeout: 12000 }, async t => {
  const udp = dgram.createSocket('udp4');
  await new Promise(r => udp.bind(0, '127.0.0.1', r));
  const port = udp.address().port;
  await new Promise(r => udp.close(r));
  const sender = dgram.createSocket('udp4');
  t.after(() => sender.close());
  const cli = new URL('../src/cli.js', import.meta.url).href;
  const script = `Object.defineProperty(process.stdout,'isTTY',{value:true}); const {run}=await import(${JSON.stringify(cli)}); console.log('READY'); process.exitCode=await run(['esp32','--watch','--bind','127.0.0.1','--udp-port',${JSON.stringify(String(port))},'--seconds','1','--max-packets','1']);`;
  const child = spawn(process.execPath, ['--input-type=module', '-e', script], { env: { ...process.env, NO_COLOR: '1', FORCE_COLOR: '0' } });
  t.after(async () => {
    if (child.exitCode !== null || child.signalCode !== null) return;
    const closed = new Promise(r => child.once('close', r));
    child.kill('SIGKILL');
    await closed;
  });
  let output = ''; let errors = '';
  child.stdout.on('data', b => { output += b; });
  child.stderr.on('data', b => { errors += b; });
  async function waitFor(predicate) {
    for (let n = 0; n < 500; n++) {
      if (predicate()) return;
      if (child.exitCode !== null) assert.fail(`child exited ${child.exitCode}: ${errors}\n${output}`);
      await pause(10);
    }
    assert.fail(`CLI condition timeout: ${errors}\n${output}`);
  }
  const packet = Buffer.alloc(28);
  packet.writeUInt32LE(0xC5110001, 0); packet[4] = 9; packet[5] = 1; packet.writeUInt16LE(4, 6);
  packet.writeUInt32LE(2437, 8); packet.writeInt8(-52, 16); packet.writeInt8(-95, 17);
  for (let i = 20; i < 28; i += 2) { packet[i] = 3; packet[i + 1] = 4; }
  const windows = () => output.split('RUVIEW / NODE STREAM').length - 1;
  await waitFor(() => output.includes('READY')); await pause(200);
  sender.send(packet, port, '127.0.0.1');
  await waitFor(() => windows() === 1);
  await waitFor(() => windows() === 2); // no packets in the second window
  await pause(50); packet.writeUInt32LE(1, 12); sender.send(packet, port, '127.0.0.1');
  await waitFor(() => windows() === 3);
  const last = output.split('RUVIEW / NODE STREAM').at(-1);
  child.kill('SIGINT');
  const row = last.split('\n').find(l => l.startsWith('esp32'));
  assert.ok(row, last);
  // Packet-limited windows now use their measured elapsed times, so their
  // rates can differ. The missing window must still occupy the middle slot.
  assert.match(row, /[▁▂▃▄▅▆▇█]·[▁▂▃▄▅▆▇█]\s*$/, `one position per capture window: ${row}`);
});

test('sparkline keeps missing positions and finite scale controls', () => {
  assert.equal(sparkline([5, null, 5]), '▄·▄');
  assert.equal(sparkline([null, NaN]), '··');
  assert.equal(sparkline([0, null, 8]), '▁·█');
  assert.equal(sparkline([]), '');
});
