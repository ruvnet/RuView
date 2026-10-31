// SPDX-License-Identifier: MIT
// Cross-platform ESP32 firmware flashing (ADR-370).
//
// Flow: resolve a firmware bundle directory → verify every image against its
// SHA256SUMS → build an esptool argv (never a shell string) → preflight the
// attached chip → only then write flash, and only with explicit confirmation.
// A successful write is NOT hardware validation: validation needs a captured
// boot/runtime log showing CSI callbacks (CLAUDE.md hardware-evidence rule).

import { createHash } from 'node:crypto';
import { existsSync, lstatSync, readFileSync, realpathSync, statSync } from 'node:fs';
import { basename, isAbsolute, join, relative, resolve } from 'node:path';

/** Supported board/flash-size variants and their esptool parameters. */
export const FIRMWARE_VARIANTS = Object.freeze({
  's3-8mb': Object.freeze({ chip: 'esp32s3', chipName: 'ESP32-S3', flashSize: '8MB' }),
  's3-4mb': Object.freeze({ chip: 'esp32s3', chipName: 'ESP32-S3', flashSize: '4MB' }),
  c6: Object.freeze({ chip: 'esp32c6', chipName: 'ESP32-C6', flashSize: '4MB' }),
  c5: Object.freeze({ chip: 'esp32c5', chipName: 'ESP32-C5', flashSize: '4MB' }),
});

/** Shared partition/app layout; C5 boots at 0x2000 instead of S3/C6's 0x0. */
const IMAGES = Object.freeze([
  { role: 'bootloader', offset: '0x0', names: ['bootloader.bin', 'bootloader/bootloader.bin'], required: true },
  { role: 'partition-table', offset: '0x8000', names: ['partition-table.bin', 'partition_table/partition-table.bin'], names4mb: ['partition-table-4mb.bin'], namesC6: ['partition-table-c6.bin'], namesC5: ['partition-table-c5.bin'], required: true },
  { role: 'otadata', offset: '0xf000', names: ['ota_data_initial.bin'], required: false },
  { role: 'app', offset: '0x20000', names: ['esp32-csi-node.bin'], names4mb: ['esp32-csi-node-4mb.bin'], namesC6: ['esp32-csi-node-c6.bin'], namesC5: ['esp32-csi-node-c5.bin'], required: true },
]);

const MAX_IMAGE_BYTES = 16 * 1024 * 1024;
export const BAUD_RATES = Object.freeze([115200, 230400, 460800, 921600]);
const PORT_RE = /^(?:COM[1-9][0-9]{0,2}|\/dev\/(?:tty|cu)[A-Za-z0-9._-]{1,64})$/;

export class FirmwareError extends Error {
  constructor(reason, message, details = {}) {
    super(message);
    this.reason = reason;
    Object.assign(this, details);
  }
}

export function validatePort(port) {
  if (typeof port !== 'string' || !PORT_RE.test(port)) {
    throw new FirmwareError('invalid_port', 'port must look like COM7, /dev/ttyUSB0, /dev/ttyACM0, or /dev/cu.usbserial-*');
  }
  return port;
}

const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

function isWithin(parent, child) {
  const rel = relative(parent, child);
  return rel === '' || (!rel.startsWith('..') && !isAbsolute(rel));
}

/** Parse SHA256SUMS / SHA256SUMS.txt lines: "<hex>  [*]<relative path>". */
function readChecksums(dir) {
  for (const name of ['SHA256SUMS', 'SHA256SUMS.txt']) {
    const path = join(dir, name);
    if (!existsSync(path)) continue;
    const sums = new Map();
    for (const line of readFileSync(path, 'utf8').split(/\r?\n/)) {
      const m = /^([0-9a-fA-F]{64})\s+\*?(.+)$/.exec(line.trim());
      if (m) {
        const file = m[2].replace(/^\.\//, '').replaceAll('\\', '/');
        sums.set(file, m[1].toLowerCase());
        sums.set(basename(file), m[1].toLowerCase());
      }
    }
    return { file: name, sums };
  }
  return null;
}

/**
 * Resolve and verify a bundle directory (an extracted release bundle, a
 * `release_bins/<variant>` folder, or an ESP-IDF `build/` directory).
 */
export function resolveBundle(bundleDir, variant, { requireChecksums = true } = {}) {
  const spec = FIRMWARE_VARIANTS[variant];
  if (!spec) throw new FirmwareError('invalid_variant', `variant must be one of ${Object.keys(FIRMWARE_VARIANTS).join(', ')}`);
  if (typeof bundleDir !== 'string' || !bundleDir) throw new FirmwareError('invalid_bundle', 'bundle directory is required');
  const dir = resolve(bundleDir);
  if (!existsSync(dir) || !statSync(dir).isDirectory()) throw new FirmwareError('bundle_missing', `bundle directory not found: ${dir}`);
  const root = realpathSync(dir);
  const checksums = readChecksums(root);
  if (!checksums && requireChecksums) {
    throw new FirmwareError('checksums_missing', 'bundle has no SHA256SUMS(.txt); pass allow_unverified only for a local build you produced');
  }
  const images = [];
  for (const image of IMAGES) {
    const variantNames = variant === 'c5' ? image.namesC5
      : variant === 'c6' ? image.namesC6 : variant === 's3-4mb' ? image.names4mb : [];
    const candidates = [...(variantNames ?? []), ...image.names];
    const found = candidates.map((n) => join(root, n)).find((p) => existsSync(p));
    if (!found) {
      if (image.required) throw new FirmwareError('image_missing', `bundle is missing ${image.role} (${candidates.join(' | ')})`);
      continue;
    }
    if (lstatSync(found).isSymbolicLink() || !isWithin(root, realpathSync(found))) {
      throw new FirmwareError('image_untrusted', `${image.role} must be a regular file inside the bundle`);
    }
    const stat = statSync(found);
    if (!stat.isFile() || stat.size === 0 || stat.size > MAX_IMAGE_BYTES) {
      throw new FirmwareError('image_invalid', `${image.role} must be a non-empty file under ${MAX_IMAGE_BYTES} bytes`);
    }
    const digest = sha256(readFileSync(found));
    const rel = relative(root, found).replaceAll('\\', '/');
    let integrity = 'unverified';
    if (checksums) {
      const expected = checksums.sums.get(rel) ?? checksums.sums.get(basename(found));
      if (!expected) {
        if (requireChecksums) throw new FirmwareError('checksum_missing', `${checksums.file} has no entry for ${rel}`);
      } else if (expected !== digest) {
        throw new FirmwareError('checksum_mismatch', `${rel} sha256 ${digest} does not match ${checksums.file}`);
      } else integrity = 'verified';
    }
    const offset = variant === 'c5' && image.role === 'bootloader' ? '0x2000' : image.offset;
    images.push({ role: image.role, offset, file: found, relative: rel, bytes: stat.size, sha256: digest, integrity });
  }
  // A 4 MB board must not receive the 8 MB partition table, and vice versa.
  const table = images.find((i) => i.role === 'partition-table');
  if (variant === 's3-8mb' && /4mb/i.test(table.relative)) throw new FirmwareError('variant_mismatch', '8 MB variant selected but bundle carries a 4 MB partition table');
  return { dir: root, variant, ...spec, checksums: checksums?.file ?? null, images };
}

/** Build the esptool argv (after `python -m esptool`). */
export function buildFlashArgs(bundle, { port, baud = 460800 }) {
  validatePort(port);
  if (!BAUD_RATES.includes(baud)) throw new FirmwareError('invalid_baud', `baud must be one of ${BAUD_RATES.join(', ')}`);
  return [
    '--chip', bundle.chip, '--port', port, '--baud', String(baud),
    'write_flash', '--flash_mode', 'dio', '--flash_size', bundle.flashSize,
    ...bundle.images.flatMap((i) => [i.offset, i.file]),
  ];
}

/** Parse `esptool chip_id` output into a chip name like "ESP32-S3". */
export function parseChip(output) {
  const m = /(?:Chip is|Detecting chip type\.\.\.|Chip type:)\s*(ESP32(?:-[A-Z0-9]+)?(?:\s*\(beta\d+\))?)/i.exec(output);
  return m ? m[1].replace(/\s+\(/g, '(').toUpperCase() : null;
}

/** Summarize a captured serial boot log into hardware evidence. */
export function summarizeBootLog(text) {
  const lines = String(text).split(/\r?\n/);
  const csi = lines.filter((l) => /\bCSI cb #[1-9]\d*\b/.test(l)).length;
  const panic = lines.some((l) => /Guru Meditation|abort\(\) was called|Backtrace:|SPI flash busy detected|TG[01]_WDT/.test(l));
  const version = (/(?:RuView|esp32-csi-node|ESP32(?:-[A-Z0-9]+)? CSI Node)[^\n]*?v?(\d+\.\d+\.\d+)/i.exec(text) || [])[1] || null;
  return {
    lines: lines.filter(Boolean).length,
    csiCallbacks: csi,
    mgmtDataUpgrade: /MGMT\+DATA/.test(text),
    panic,
    firmwareVersion: version,
    hardwareValidated: csi > 0 && !panic,
  };
}

// Read-only serial capture; port and duration are argv, never spliced into source.
export const CAPTURE_SCRIPT = [
  'import sys,time',
  'try:',
  ' import serial',
  'except Exception:',
  " print('NO_PYSERIAL'); sys.exit(3)",
  'ser=serial.Serial(sys.argv[1],115200,timeout=1)',
  'end=time.time()+float(sys.argv[2])',
  'while time.time()<end:',
  ' ln=ser.readline()',
  // Raw bytes: the scrubbed child env leaves Windows stdout on cp1252, and
  // firmware logs contain non-ASCII (e.g. U+2192), which would abort the capture.
  ' if ln: sys.stdout.buffer.write(ln); sys.stdout.buffer.flush()',
  'ser.close()',
].join('\n');

/**
 * Plan (default) or perform a flash. `deps` = { exec, python } is injected by
 * the tool layer so tests can run without hardware.
 */
export async function flashFirmware(args, deps) {
  const variant = args.variant || 's3-8mb';
  let bundle;
  let flashArgs;
  try {
    validatePort(args.port);
    bundle = resolveBundle(args.bundle, variant, { requireChecksums: args.allow_unverified !== true });
    flashArgs = buildFlashArgs(bundle, { port: args.port, baud: args.baud ?? 460800 });
  } catch (error) {
    if (error instanceof FirmwareError) return { ok: false, reason: error.reason, detail: error.message };
    throw error;
  }
  const plan = {
    variant, chip: bundle.chipName, flashSize: bundle.flashSize, port: args.port, checksums: bundle.checksums,
    images: bundle.images.map(({ role, offset, relative: rel, bytes, sha256: digest, integrity }) => ({ role, offset, file: rel, bytes, sha256: digest, integrity })),
    command: ['python', '-m', 'esptool', ...flashArgs],
    note: 'Writes the listed images only; NVS (WiFi/node config) is preserved.',
    warnings: bundle.images.some((i) => i.role === 'otadata') ? [] : [
      'Bundle has no ota_data_initial.bin: a node that last booted ota_1 keeps booting ota_1. Use a full release flash bundle for fresh installs.',
    ],
  };
  if (args.confirm !== true) {
    return { ok: true, dryRun: true, plan, next: 'Re-run with confirm to write flash. Nothing was written.' };
  }
  const python = deps.python();
  if (!python) return { ok: false, reason: 'python_missing', plan, remedy: 'Install Python 3 and `pip install esptool pyserial`.' };

  if (variant === 'c5') {
    // esptool 5.3 corrected C5 security-eFuse layouts. Never write a new C5
    // through an older tool that can misinterpret its flash security state.
    const version = await deps.exec(python, ['-m', 'esptool', 'version'], { timeoutMs: 15_000 });
    const match = /esptool(?:\.py)?\s+v(\d+)\.(\d+)/i.exec(version.stdout + version.stderr);
    const major = match ? Number(match[1]) : NaN;
    const minor = match ? Number(match[2]) : NaN;
    if (!version.ok || !Number.isSafeInteger(major) || !Number.isSafeInteger(minor) || major < 5 || (major === 5 && minor < 3)) {
      return { ok: false, reason: 'esptool_version_unsupported', plan,
        remedy: 'ESP32-C5 requires esptool >=5.3 for corrected security layouts. Install a current esptool in your active Python environment.' };
    }
  }

  const probe = await deps.exec(python, ['-m', 'esptool', '--port', args.port, 'chip_id'], { timeoutMs: 60_000 });
  if (/No module named '?esptool/i.test(probe.stderr + probe.stdout)) {
    return { ok: false, reason: 'esptool_missing', plan, remedy: 'pip install esptool' };
  }
  const detected = parseChip(probe.stdout + probe.stderr);
  if (!probe.ok || !detected) {
    return { ok: false, reason: 'chip_probe_failed', plan, detail: (probe.stderr || probe.error || '').slice(-800), remedy: 'Check the cable/port, hold BOOT while pressing RESET, or close other serial monitors.' };
  }
  if (detected !== bundle.chipName) {
    return { ok: false, reason: 'chip_mismatch', plan, detected, expected: bundle.chipName, detail: 'Refusing to flash firmware built for a different chip.' };
  }

  if (variant === 'c5') {
    const flash = await deps.exec(python, ['-m', 'esptool', '--port', args.port, 'flash_id'], { timeoutMs: 60_000 });
    const size = /Detected flash size:\s*(\d+)\s*(KB|MB)/i.exec(flash.stdout + flash.stderr);
    const bytes = size ? Number(size[1]) * (size[2].toUpperCase() === 'MB' ? 1024 * 1024 : 1024) : NaN;
    if (!flash.ok || !Number.isSafeInteger(bytes) || bytes <= 0) return { ok: false, reason: 'flash_capacity_unverified', plan, detail: 'C5 flash capacity could not be verified; nothing was written.' };
    if (bytes < 4 * 1024 * 1024) return { ok: false, reason: 'flash_capacity_too_small', plan, detail: 'The C5 release layout requires at least 4 MB flash; nothing was written.' };
  }

  const written = await deps.exec(python, ['-m', 'esptool', ...flashArgs], { timeoutMs: 300_000 });
  if (!written.ok) {
    return { ok: false, reason: 'flash_failed', plan, detected, tail: written.stdout.slice(-1500), stderr: written.stderr.slice(-800) };
  }
  const result = { ok: true, dryRun: false, plan, detected, flashTail: written.stdout.slice(-800), hardwareValidated: false };
  const seconds = args.boot_log_seconds ?? 15;
  if (seconds > 0) {
    const log = await deps.exec(python, ['-c', CAPTURE_SCRIPT, args.port, String(seconds)], { timeoutMs: (seconds + 15) * 1000 });
    if (log.stdout.includes('NO_PYSERIAL')) {
      result.bootLog = { captured: false, reason: 'pyserial_missing' };
    } else {
      const evidence = summarizeBootLog(log.stdout);
      result.bootLog = { captured: log.ok, ...evidence, tail: log.stdout.slice(-1500) };
      result.hardwareValidated = evidence.hardwareValidated;
    }
  }
  result.evidence = result.hardwareValidated
    ? 'MEASURED: boot log from real silicon shows CSI callbacks'
    : 'Flash written; hardware NOT validated (no CSI callbacks captured). Run `ruview monitor --port <port>` after provisioning WiFi.';
  return result;
}

/** List serial ports through pyserial (read-only). */
export async function listSerialPorts(deps) {
  const python = deps.python();
  if (!python) return { ok: false, reason: 'python_missing', ports: [] };
  const r = await deps.exec(python, ['-m', 'serial.tools.list_ports', '-v'], { timeoutMs: 20_000 });
  if (/No module named '?serial/i.test(r.stderr)) return { ok: false, reason: 'pyserial_missing', ports: [] };
  const ports = [];
  for (const raw of r.stdout.split(/\r?\n/)) {
    const line = raw.trim();
    const m = /^(COM\d+|\/dev\/\S+)/.exec(line);
    if (m) { ports.push({ port: m[1], description: line.slice(m[1].length).trim().slice(0, 200) }); continue; }
    const current = ports.at(-1);
    if (!current || !line) continue;
    const desc = /^desc:\s*(.*)$/.exec(line);
    const hwid = /^hwid:\s*(.*)$/.exec(line);
    if (desc) current.description = desc[1].slice(0, 200);
    else if (hwid) {
      current.hwid = hwid[1].slice(0, 200);
      const ids = /VID:PID=([0-9A-Fa-f]{4}):([0-9A-Fa-f]{4})/.exec(hwid[1]);
      if (ids) { current.vid = ids[1].toUpperCase(); current.pid = ids[2].toUpperCase(); }
      const ser = /SER=(\S+)/.exec(hwid[1]);
      if (ser) current.serial = ser[1].slice(0, 64);
    }
  }
  if (!r.ok && !ports.length) return { ok: false, reason: 'enumeration_failed', detail: (r.stderr || r.error || '').slice(-400), ports };
  return { ok: true, ports };
}
