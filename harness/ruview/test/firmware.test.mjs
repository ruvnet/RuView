import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { buildFlashArgs, CAPTURE_SCRIPT, flashFirmware, parseChip, resolveBundle, summarizeBootLog, validatePort } from '../src/firmware.js';
import { runTool } from '../src/tools.js';

const sha = (b) => createHash('sha256').update(b).digest('hex');

test('boot evidence requires positive CSI callbacks and recognizes C5 firmware', () => {
  for (const text of ['csi_collector: initialized', 'CSI cb #0: len=128']) {
    assert.equal(summarizeBootLog(text).hardwareValidated, false);
    assert.equal(summarizeBootLog(text).csiCallbacks, 0);
  }
  const result = summarizeBootLog('ESP32-C5 CSI Node (ADR-018 / ADR-110) v0.8.17\nCSI cb #1: len=128');
  assert.equal(result.hardwareValidated, true);
  assert.equal(result.firmwareVersion, '0.8.17');
  for (const failure of ['SPI flash busy detected(0x0f)', 'rst:0x7 (TG0_WDT_HPSYS)']) {
    const broken = summarizeBootLog(`CSI cb #1: len=106\n${failure}`);
    assert.equal(broken.panic, true);
    assert.equal(broken.hardwareValidated, false);
  }
});

function makeBundle({ otadata = true, sums = true, tamper = false, extra = {} } = {}) {
  const dir = mkdtempSync(join(tmpdir(), 'ruview-fw-'));
  const files = { 'bootloader.bin': 'BOOT', 'partition-table.bin': 'PT', 'esp32-csi-node.bin': 'APP', ...(otadata ? { 'ota_data_initial.bin': 'OTA' } : {}), ...extra };
  for (const [name, body] of Object.entries(files)) writeFileSync(join(dir, name), body);
  if (sums) {
    const lines = Object.entries(files).map(([name, body]) => `${tamper && name === 'esp32-csi-node.bin' ? '0'.repeat(64) : sha(body)}  ${name}`);
    writeFileSync(join(dir, 'SHA256SUMS.txt'), `${lines.join('\n')}\n`);
  }
  return dir;
}

/** Fake hardware: records every esptool/python invocation. */
function fakeDeps({ chip = 'ESP32-S3', toolVersion = '5.6.0', flashSize = '8MB', flashOk = true, bootLog = 'I (123) csi_collector: CSI cb #1: len=128\nMGMT+DATA\n' } = {}) {
  const calls = [];
  return {
    calls,
    python: () => '/usr/bin/python3',
    exec: async (cmd, args) => {
      calls.push([cmd, ...args]);
      if (args.at(-1) === 'version') return { ok: true, code: 0, stdout: `esptool v${toolVersion}\n${toolVersion}\n`, stderr: '' };
      if (args.includes('chip_id')) return { ok: Boolean(chip), code: 0, stdout: chip ? `Detecting chip type... ${chip}\nChip type: ${chip} (QFN56)` : '', stderr: chip ? '' : 'Failed to connect', error: null };
      if (args.includes('flash_id')) return { ok: true, code: 0, stdout: `Detected flash size: ${flashSize}\n`, stderr: '' };
      if (args.includes('write_flash')) return { ok: flashOk, code: flashOk ? 0 : 2, stdout: flashOk ? 'Hash of data verified.' : '', stderr: flashOk ? '' : 'A fatal error occurred', error: null };
      if (args[0] === '-c') return { ok: true, code: 0, stdout: bootLog, stderr: '', error: null };
      return { ok: false, code: 1, stdout: '', stderr: 'unexpected', error: null };
    },
  };
}

test('port validation rejects shell metacharacters and odd paths', () => {
  for (const good of ['COM7', 'COM123', '/dev/ttyUSB0', '/dev/ttyACM1', '/dev/cu.usbserial-0001']) assert.equal(validatePort(good), good);
  for (const bad of ['COM0', 'COM7;rm -rf /', '/etc/passwd', '/dev/sda', '../dev/ttyUSB0', '', 'COM1234']) {
    assert.throws(() => validatePort(bad), /port must look like/, bad);
  }
});

test('bundle resolution verifies every image and builds a shell-free esptool argv', () => {
  const dir = makeBundle();
  try {
    const b = resolveBundle(dir, 's3-8mb');
    assert.deepEqual(b.images.map((i) => [i.role, i.offset, i.integrity]), [
      ['bootloader', '0x0', 'verified'], ['partition-table', '0x8000', 'verified'], ['otadata', '0xf000', 'verified'], ['app', '0x20000', 'verified'],
    ]);
    const args = buildFlashArgs(b, { port: 'COM7' });
    assert.deepEqual(args.slice(0, 10), ['--chip', 'esp32s3', '--port', 'COM7', '--baud', '460800', 'write_flash', '--flash_mode', 'dio', '--flash_size']);
    assert.equal(args[10], '8MB');
    assert.throws(() => buildFlashArgs(b, { port: 'COM7', baud: 12345 }), /baud/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

// Creating symlinks on Windows needs Developer Mode or an elevated shell.
function trySymlink(target, path) {
  try { symlinkSync(target, path); return true; } catch (error) {
    if (process.platform === 'win32' && error.code === 'EPERM') return false;
    throw error;
  }
}

test('checksum mismatch, missing checksums, and escaping symlinks are refused', (t) => {
  const tampered = makeBundle({ tamper: true });
  const unsigned = makeBundle({ sums: false });
  const outside = mkdtempSync(join(tmpdir(), 'ruview-outside-'));
  const linked = makeBundle();
  try {
    assert.throws(() => resolveBundle(tampered, 's3-8mb'), (e) => e.reason === 'checksum_mismatch');
    assert.throws(() => resolveBundle(unsigned, 's3-8mb'), (e) => e.reason === 'checksums_missing');
    assert.equal(resolveBundle(unsigned, 's3-8mb', { requireChecksums: false }).images[0].integrity, 'unverified');
    writeFileSync(join(outside, 'evil.bin'), 'X');
    rmSync(join(linked, 'esp32-csi-node.bin'));
    if (trySymlink(join(outside, 'evil.bin'), join(linked, 'esp32-csi-node.bin'))) {
      assert.throws(() => resolveBundle(linked, 's3-8mb', { requireChecksums: false }), (e) => e.reason === 'image_untrusted');
    } else {
      t.diagnostic('symlink case skipped: this Windows account cannot create symlinks (EPERM)');
    }
    assert.throws(() => resolveBundle(tampered, 'esp8266'), (e) => e.reason === 'invalid_variant');
  } finally {
    for (const d of [tampered, unsigned, outside, linked]) rmSync(d, { recursive: true, force: true });
  }
});

test('4 MB variant prefers 4 MB images and 8 MB refuses a 4 MB partition table', () => {
  const dir = mkdtempSync(join(tmpdir(), 'ruview-fw4-'));
  try {
    const files = { 'bootloader.bin': 'B', 'partition-table-4mb.bin': 'P4', 'esp32-csi-node-4mb.bin': 'A4' };
    for (const [n, b] of Object.entries(files)) writeFileSync(join(dir, n), b);
    writeFileSync(join(dir, 'SHA256SUMS'), Object.entries(files).map(([n, b]) => `${sha(b)}  ${n}`).join('\n'));
    const b = resolveBundle(dir, 's3-4mb');
    assert.equal(b.flashSize, '4MB');
    assert.equal(b.images.find((i) => i.role === 'app').relative, 'esp32-csi-node-4mb.bin');
    assert.throws(() => resolveBundle(dir, 's3-8mb'), (e) => e.reason === 'image_missing');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('official C6 bundle names verify checksums and remain scoped to the C6 variant', () => {
  const dir = mkdtempSync(join(tmpdir(), 'ruview-fw-c6-'));
  try {
    const files = {
      'bootloader.bin': 'C6_BOOT',
      'partition-table-c6.bin': 'C6_PT',
      'esp32-csi-node-c6.bin': 'C6_APP',
      'ota_data_initial.bin': 'C6_OTA',
    };
    for (const [name, body] of Object.entries(files)) writeFileSync(join(dir, name), body);
    writeFileSync(join(dir, 'SHA256SUMS.txt'), Object.entries(files).map(([name, body]) => `${sha(body)}  ${name}`).join('\n'));
    const bundle = resolveBundle(dir, 'c6');
    assert.equal(bundle.chip, 'esp32c6');
    assert.equal(bundle.flashSize, '4MB');
    assert.deepEqual(bundle.images.map(({ relative, integrity }) => [relative, integrity]), [
      ['bootloader.bin', 'verified'], ['partition-table-c6.bin', 'verified'],
      ['ota_data_initial.bin', 'verified'], ['esp32-csi-node-c6.bin', 'verified'],
    ]);
    for (const variant of ['s3-8mb', 's3-4mb']) {
      assert.throws(() => resolveBundle(dir, variant), (error) => error.reason === 'image_missing');
    }
    // A local build may also be copied into the folder: prefer the release's
    // explicit C6 images, never accidentally fall back to an unrelated app.
    writeFileSync(join(dir, 'esp32-csi-node.bin'), 'OTHER_APP');
    assert.equal(resolveBundle(dir, 'c6').images.find((image) => image.role === 'app').relative, 'esp32-csi-node-c6.bin');
    writeFileSync(join(dir, 'esp32-csi-node-c6.bin'), 'TAMPERED');
    assert.throws(() => resolveBundle(dir, 'c6'), (error) => error.reason === 'checksum_mismatch');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('C6 keeps generic ESP-IDF build directory support', () => {
  const dir = makeBundle();
  try {
    const bundle = resolveBundle(dir, 'c6');
    assert.equal(bundle.chip, 'esp32c6');
    assert.equal(bundle.images.find((image) => image.role === 'app').relative, 'esp32-csi-node.bin');
    assert.ok(bundle.images.every((image) => image.integrity === 'verified'));
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('C5 release bundle is checksum verified and rejects other attached chips before writes', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'ruview-fw-c5-'));
  try {
    const files = { 'bootloader.bin': 'C5_BOOT', 'partition-table-c5.bin': 'C5_PT', 'esp32-csi-node-c5.bin': 'C5_APP' };
    for (const [name, body] of Object.entries(files)) writeFileSync(join(dir, name), body);
    writeFileSync(join(dir, 'SHA256SUMS.txt'), Object.entries(files).map(([name, body]) => `${sha(body)}  ${name}`).join('\n'));
    const bundle = resolveBundle(dir, 'c5');
    assert.equal(bundle.chip, 'esp32c5');
    assert.equal(bundle.flashSize, '4MB');
    assert.deepEqual(bundle.images.map((i) => [i.role, i.offset]), [
      ['bootloader', '0x2000'], ['partition-table', '0x8000'], ['app', '0x20000'],
    ]);
    assert.equal(bundle.images.find((i) => i.role === 'app').relative, 'esp32-csi-node-c5.bin');
    assert.ok(bundle.images.every((i) => i.integrity === 'verified'));
    for (const toolVersion of ['4.12.0', '5.1.0', '5.2.0', 'unknown']) {
      const old = fakeDeps({ chip: 'ESP32-C5', toolVersion });
      const result = await flashFirmware({ port: 'COM8', variant: 'c5', bundle: dir, confirm: true }, old);
      assert.equal(result.reason, 'esptool_version_unsupported');
      assert.ok(!old.calls.some((call) => call.includes('chip_id') || call.includes('write_flash')));
    }
    for (const variant of ['s3-8mb', 's3-4mb', 'c6']) {
      assert.throws(() => resolveBundle(dir, variant), (e) => e.reason === 'image_missing');
    }
    for (const chip of ['ESP32-S3', 'ESP32-C6', 'ESP32-C5(beta3)']) {
      const deps = fakeDeps({ chip });
      const result = await flashFirmware({ port: 'COM8', variant: 'c5', bundle: dir, confirm: true }, deps);
      assert.equal(result.reason, 'chip_mismatch');
      assert.ok(!deps.calls.some((call) => call.includes('write_flash')));
    }
    for (const [flashSize, reason] of [['2MB', 'flash_capacity_too_small'], ['unknown', 'flash_capacity_unverified']]) {
      const deps = fakeDeps({ chip: 'ESP32-C5', flashSize });
      const result = await flashFirmware({ port: 'COM8', variant: 'c5', bundle: dir, confirm: true }, deps);
      assert.equal(result.reason, reason);
      assert.ok(!deps.calls.some((call) => call.includes('write_flash')));
    }
    for (const flashSize of ['4MB', '8MB']) {
      const deps = fakeDeps({ chip: 'ESP32-C5', flashSize });
      assert.equal((await flashFirmware({ port: 'COM8', variant: 'c5', bundle: dir, confirm: true }, deps)).ok, true);
      const write = deps.calls.find((call) => call.includes('write_flash'));
      assert.equal(write[write.indexOf('--chip') + 1], 'esp32c5');
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('flash without confirm is a plan and touches no hardware', async () => {
  const dir = makeBundle({ otadata: false });
  const deps = fakeDeps();
  try {
    const r = await flashFirmware({ port: '/dev/ttyUSB0', bundle: dir }, deps);
    assert.equal(r.ok, true);
    assert.equal(r.dryRun, true);
    assert.equal(deps.calls.length, 0);
    assert.equal(r.plan.warnings.length, 1, 'missing otadata is called out');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('confirmed flash probes the chip, refuses a mismatch, and records boot evidence', async () => {
  const dir = makeBundle();
  try {
    const mismatch = fakeDeps({ chip: 'ESP32-C6' });
    const r1 = await flashFirmware({ port: 'COM7', bundle: dir, variant: 's3-8mb', confirm: true }, mismatch);
    assert.deepEqual([r1.ok, r1.reason, r1.detected], [false, 'chip_mismatch', 'ESP32-C6']);
    assert.ok(!mismatch.calls.some((c) => c.includes('write_flash')), 'no write after a chip mismatch');

    const good = fakeDeps();
    const r2 = await flashFirmware({ port: 'COM7', bundle: dir, confirm: true, boot_log_seconds: 5 }, good);
    assert.equal(r2.ok, true);
    assert.equal(r2.hardwareValidated, true);
    assert.equal(r2.bootLog.csiCallbacks, 1);
    assert.match(r2.evidence, /^MEASURED/);
    const write = good.calls.find((c) => c.includes('write_flash'));
    assert.equal(write[write.indexOf('--port') + 1], 'COM7', 'port travels as its own argv element');

    const silent = fakeDeps({ bootLog: 'rst:0x1 (POWERON)\nboot:0x8\n' });
    const r3 = await flashFirmware({ port: 'COM7', bundle: dir, confirm: true }, silent);
    assert.equal(r3.ok, true);
    assert.equal(r3.hardwareValidated, false, 'a write alone is not hardware validation');

    const noBoard = await flashFirmware({ port: 'COM7', bundle: dir, confirm: true }, fakeDeps({ chip: null }));
    assert.equal(noBoard.reason, 'chip_probe_failed');
    const failed = await flashFirmware({ port: 'COM7', bundle: dir, confirm: true }, fakeDeps({ flashOk: false }));
    assert.equal(failed.reason, 'flash_failed');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('chip and boot-log parsers handle esptool 4.x and 5.x output', () => {
  assert.equal(parseChip('Chip is ESP32-S3 (QFN56) (revision v0.2)'), 'ESP32-S3');
  assert.equal(parseChip('Detecting chip type... ESP32-C6'), 'ESP32-C6');
  assert.equal(parseChip('Chip type: ESP32-C5 (revision v1.2)'), 'ESP32-C5');
  assert.equal(parseChip('Chip type: ESP32-C5 (beta3)'), 'ESP32-C5(BETA3)');
  assert.equal(parseChip('Chip type:          ESP32-S3 (QFN56)'), 'ESP32-S3');
  assert.equal(parseChip('nothing'), null);
  const panic = summarizeBootLog('CSI cb\nGuru Meditation Error: Core 0 panic\n');
  assert.equal(panic.panic, true);
  assert.equal(panic.hardwareValidated, false);
});

test('MCP flash requires a hardware-write grant and confirmation; plan is read-only', async () => {
  const denied = await runTool('ruview_node_flash', { port: 'COM7', bundle: '/x', confirm: true }, { source: 'mcp', grants: [] });
  assert.equal(denied.reason, 'authority_denied');
  const unconfirmed = await runTool('ruview_node_flash', { port: 'COM7', bundle: '/x' }, { source: 'mcp', grants: ['hardware-write'] });
  assert.equal(unconfirmed.reason, 'not_confirmed');
  const plan = await runTool('ruview_firmware_plan', { port: 'COM7', bundle: '/definitely/missing' }, { source: 'mcp', grants: [] });
  assert.equal(plan.reason, 'bundle_missing');
  const injected = await runTool('ruview_firmware_plan', { port: 'COM7', bundle: '/x', confirm: true }, { source: 'mcp', grants: [] });
  assert.equal(injected.reason, 'invalid_arguments', 'plan tool cannot be coerced into a write');
});

test('boot-log capture survives non-ASCII firmware logs on a cp1252 stdout', (t) => {
  const py = ['python3', 'python'].find((p) => spawnSync(p, ['--version'], { stdio: 'ignore' }).status === 0);
  if (!py) { t.skip('python not available'); return; }
  const fake = mkdtempSync(join(tmpdir(), 'ruview-fakeserial-'));
  try {
    // Stand-in pyserial: two lines, the second containing U+2192 (as the C6 firmware logs).
    writeFileSync(join(fake, 'serial.py'), [
      'class Serial:',
      '    def __init__(self, *a, **k): self.lines = [b"I (10) csi_collector: CSI cb #1\\n", "I (11) main: sync \\u2192 leader\\n".encode()]',
      '    def readline(self): return self.lines.pop(0) if self.lines else b""',
      '    def close(self): pass',
    ].join('\n'));
    const r = spawnSync(py, ['-c', CAPTURE_SCRIPT, 'COM1', '0.5'], { env: { PATH: process.env.PATH, SYSTEMROOT: process.env.SYSTEMROOT || '', PYTHONPATH: fake, PYTHONIOENCODING: 'cp1252' }, encoding: 'utf8' });
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stdout, /sync \u2192 leader/);
    assert.equal(summarizeBootLog(r.stdout).csiCallbacks, 1);
  } finally {
    rmSync(fake, { recursive: true, force: true });
  }
});
