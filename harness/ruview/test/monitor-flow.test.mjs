import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
const python = ['python3', 'python'].find(p => spawnSync(p, ['--version'], { stdio: 'ignore' }).status === 0);
function monitor(t, lines) {
  const dir = mkdtempSync(join(tmpdir(), 'ruview-monitor-flow-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const hex = lines.map(l => Buffer.from(l + '\n').toString('hex'));
  writeFileSync(join(dir, 'serial.py'), [
    'import time', 'class Serial:',
    `    def __init__(self,*a,**k): self.lines=${JSON.stringify(hex)}`,
    '    def open(self): pass',
    '    def readline(self):',
    '        if self.lines: return bytes.fromhex(self.lines.pop(0))',
    '        time.sleep(0.01); return b""',
    '    def close(self): pass',
  ].join('\n'));
  const sdk = new URL('../src/sdk.js', import.meta.url).href;
  const script = `const {createRuView}=await import(${JSON.stringify(sdk)}); console.log(JSON.stringify(await createRuView().firmware.monitor('COM3',0.15,115200)));`;
  const child = spawnSync(process.execPath, ['--input-type=module', '-e', script], {
    env: { ...process.env, PYTHONPATH: dir }, encoding: 'utf8', timeout: 15000,
  });
  assert.equal(child.status, 0, child.stderr);
  return JSON.parse(child.stdout.trim());
}
const startup = [
  'I (100) csi_collector: Early capture node_id=1 (before WiFi init, #232/#390)',
  'W (200) csi_collector: self-ping: no gateway IP yet; CSI relies on ambient frames (#954)',
];

test('public SDK monitor does not certify CSI flow from startup/configuration tags', { skip: !python }, t => {
  const result = monitor(t, startup);
  assert.equal(result.ok, false, JSON.stringify(result));
  assert.equal(result.csi_callbacks, 0);
  assert.equal(result.lines, 2);
  assert.match(result.hint, /no CSI log lines/i);
});

test('public SDK monitor counts only actual ESP32 and Realtek flow markers', { skip: !python }, t => {
  const result = monitor(t, [...startup, 'I (300) csi_collector: CSI cb #1: len=128 rssi=-52 ch=6', 'RUVIEW_CSI: frame #10 seq=10']);
  assert.equal(result.ok, true, JSON.stringify(result));
  assert.equal(result.csi_callbacks, 2);
  assert.equal(result.lines, 4);
});

test('buffer starvation still refuses a monitor with a real frame marker', { skip: !python }, t => {
  const result = monitor(t, ['RUVIEW_CSI: frame #10 seq=10', '[WLAN-W] lack of csi buf!']);
  assert.equal(result.ok, false);
  assert.equal(result.reason, 'csi_buffer_starvation');
  assert.equal(result.csi_callbacks, 1);
  assert.equal(result.csi_buffer_starved, 1);
});

test('public SDK monitor rejects C5 flash-busy watchdog loops after initial CSI', { skip: !python }, t => {
  const result = monitor(t, ['CSI cb #1: len=106', 'SPI flash busy detected(0x0f)', 'rst:0x7 (TG0_WDT_HPSYS)']);
  assert.equal(result.ok, false);
  assert.equal(result.reason, 'firmware_fault');
  assert.equal(result.firmware_fault_lines, 2);
  assert.equal(result.csi_callbacks, 1);
});

test('public SDK monitor rejects zero callback counters', { skip: !python }, t => {
  const result = monitor(t, ['CSI cb #0: len=106', 'RUVIEW_CSI: frame #0']);
  assert.equal(result.ok, false);
  assert.equal(result.csi_callbacks, 0);
});
