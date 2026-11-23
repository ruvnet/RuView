// ADR-375: terminal rendering. Pure functions, pinned without color; JSON stays
// the contract for pipes. Monitor starvation detection runs the real monitor
// script against a stand-in serial module.
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { bar, colorEnabled, renderResult, sparkline, table, createStyle } from '../src/cli-ui.js';
import { MONITOR_SCRIPT } from '../src/tools.js';

const capture = {
  ok: true, listen: '0.0.0.0:5005', seconds: 15, packets: 5199, decodedPackets: 5199, csiNodes: 2,
  evidence: 'MEASURED: live UDP packets received and decoded on this host',
  heartbeatOnlySenders: [{ address: '192.168.1.67', heartbeats: 50 }],
  nodes: [
    { source: 'esp32', nodeId: 42, csiRateHz: 4.73, csiLossFraction: 0, rssiMean: -63.1, csi: { shape: '1x64' }, csiShapes: [{}, {}], packets: { csi: 71 } },
    { source: 'realtek', nodeId: 3, csiRateHz: 324.67, csiLossFraction: 0.1, rssiMean: -39.3, csi: { shape: '1x52', synthetic: false }, seqReordered: 37, seqStrays: 7, packets: { csi: 4870 } },
  ],
  analysis: { ok: true, backend: 'wasm', integrity: 'verified', input: { frames: 12549, sampleRateHz: 313.7 }, summary: { last: { heart: { bpm: 99.06, status: 'unreliable' }, respiratory: { status: 'unavailable' } } }, note: 'no reference measurement' },
};

test('capture renders nodes, alerts and analysis without color codes', () => {
  const out = renderResult('ruview_esp32_capture', capture, { color: false });
  assert.doesNotMatch(out, /\x1b\[/);
  assert.match(out, /^RUVIEW \/ NODE STREAM {3}● MEASURED/);
  assert.match(out, /heartbeat-only: 192\.168\.1\.67 \(50 heartbeats, no CSI\)/);
  assert.match(out, /esp32 +42 +4\.7 Hz/);
  assert.match(out, /realtek +3 +324\.7 Hz +████████/);
  assert.match(out, /10\.0%/);
  assert.match(out, /1x64 \+1/);
  assert.match(out, /37r 7s/);
  assert.match(out, /heart 99\.1 bpm \(unreliable\) {3}breathing unavailable/);
  const lines = out.split('\n');
  const header = lines.find((l) => l.startsWith('SOURCE'));
  const row = lines.find((l) => l.startsWith('realtek'));
  assert.equal(row.indexOf('324.7'), header.indexOf('CSI RATE'), 'columns align');
});

test('failures lead with the reason and the fix; color is opt-in', () => {
  const out = renderResult('ruview_esp32_capture', { ok: false, reason: 'heartbeat_only', remedy: 'reset the board', packets: 3 }, { color: false });
  assert.match(out, /✖ heartbeat only/);
  assert.match(out, /fix {2}reset the board/);
  assert.match(renderResult('ruview_esp32_capture', capture, { color: true }), /\x1b\[38;5;155m/);
  assert.equal(colorEnabled({ isTTY: true }, {}), true);
  assert.equal(colorEnabled({ isTTY: true }, { NO_COLOR: '' }), false);
  assert.equal(colorEnabled({ isTTY: true }, { TERM: 'dumb' }), false);
  assert.equal(colorEnabled({ isTTY: false }, {}), false);
});

test('devices, monitor, flash and generic views', () => {
  const dev = renderResult('ruview_devices_scan', { ok: true, devices: [{ port: 'COM10', bridge: 'Prolific PL2303GC', likelyRoles: ['realtek'], confirmWith: ['ruview monitor --port COM10 --baud 1500000'] }, { port: 'COM3', builtin: true }], note: 'hints' }, { color: false });
  assert.match(dev, /COM10 +Prolific PL2303GC +realtek +ruview monitor --port COM10 --baud 1500000/);
  assert.doesNotMatch(dev, /COM3/, 'built-in ports hidden');
  const mon = renderResult('ruview_node_monitor', { ok: false, reason: 'csi_buffer_starvation', hint: 'reset', csi_callbacks: 0, lines: 629, baud: 1500000, reset_on_open: false }, { color: false });
  assert.match(mon, /✖ csi buffer starvation/);
  assert.match(mon, /reset on open: no/);
  const flash = renderResult('ruview_firmware_plan', { ok: true, dryRun: true, plan: { chip: 'ESP32-C6', flashSize: '4MB', port: 'COM16', checksums: 'SHA256SUMS.txt', images: [{ offset: '0x0', role: 'bootloader', bytes: 19568, integrity: 'verified' }] } }, { color: false });
  assert.match(flash, /RUVIEW \/ FLASH PLAN/);
  assert.match(flash, /0x0 +bootloader +19568 +verified/);
  const gen = renderResult('ruview_claim_check', { ok: true, verdict: 'PASS', findings: [] }, { color: false });
  assert.match(gen, /RUVIEW \/ CLAIM CHECK/);
  assert.match(gen, /verdict +PASS/);
  assert.match(gen, /use --json/);
});

test('bar, sparkline and table primitives', () => {
  assert.equal(bar(1, 4), '████');
  assert.equal(bar(0, 4), '    ');
  assert.equal(bar(0.5, 4).length, 4);
  assert.equal(sparkline([1, 2, 3, 4, 5, 6, 7, 8]), '▁▂▃▄▅▆▇█');
  assert.equal(sparkline([5, 5]), '▄▄');
  assert.equal(sparkline([]), '');
  assert.equal(table([['A', 'B'], ['xx', 'y']], createStyle(false)), 'A   B\nxx  y');
});

test('monitor script flags Ameba CSI buffer starvation', (t) => {
  const py = ['python3', 'python'].find((p) => spawnSync(p, ['--version'], { stdio: 'ignore' }).status === 0);
  if (!py) { t.skip('python not available'); return; }
  const fake = mkdtempSync(join(tmpdir(), 'ruview-fakeserial-'));
  try {
    writeFileSync(join(fake, 'serial.py'), [
      'class Serial:',
      '    def __init__(self, *a, **k): self.lines = [b"[WLAN-W] lack of csi buf!\\n", b"[WLAN-W] csi buf not enough\\n", b"RUVIEW_CSI: heartbeat tick=9\\n"]',
      '    def open(self): pass',
      '    def readline(self): return self.lines.pop(0) if self.lines else b""',
      '    def close(self): pass',
    ].join('\n'));
    const r = spawnSync(py, ['-c', MONITOR_SCRIPT, 'COM1', '0.4', '1500000'], { env: { PATH: process.env.PATH, SYSTEMROOT: process.env.SYSTEMROOT || '', PYTHONPATH: fake }, encoding: 'utf8' });
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stdout, /LINES=3 CSI=0 STARVED=2/);
  } finally {
    rmSync(fake, { recursive: true, force: true });
  }
});


test('piped watch exits after one permanent input failure', () => {
  const cli = fileURLToPath(new URL('../bin/cli.js', import.meta.url));
  for (const options of [['--udp-port', '1024.5'], ['--bind', 'invalid-bind']]) {
    const child = spawnSync(process.execPath, [cli, 'esp32', '--watch', '--json', ...options],
      { encoding: 'utf8', timeout: 2000, maxBuffer: 256 * 1024 });
    assert.equal(child.status, 1, 'permanent errors must exit instead of retrying forever');
    assert.equal(child.error, undefined);
    const lines = child.stdout.trim().split('\n');
    assert.equal(lines.length, 1, 'one structured failure is emitted');
    const result = JSON.parse(lines[0]);
    assert.equal(result.ok, false);
    assert.ok(['invalid_port', 'invalid_arguments'].includes(result.reason));
  }
});
