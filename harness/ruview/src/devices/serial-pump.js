// SPDX-License-Identifier: MIT
// Bounded raw serial capture through pyserial (ADR-373). Node has no built-in
// serial API and the package stays dependency-free, so a fixed Python script
// streams lowercase-hex chunks; all parameters are argv, never spliced into
// source. Hex (not base64) because process output passes through secret
// redaction, whose patterns cannot match the [0-9a-f] alphabet.

import { validatePort } from '../firmware.js';

export const PUMP_SCRIPT = [
  'import sys,time,binascii',
  'try:',
  ' import serial',
  'except Exception:',
  " print('NO_PYSERIAL'); sys.exit(3)",
  'port,baud,dur,maxb,start,stop,dtr=sys.argv[1],int(sys.argv[2]),float(sys.argv[3]),int(sys.argv[4]),sys.argv[5],sys.argv[6],sys.argv[7]',
  'try:',
  ' ser=serial.Serial(port,baud,timeout=0.2)',
  'except Exception as e:',
  " print('OPEN_ERROR '+str(e)[:200]); sys.exit(4)",
  'try:',
  " if dtr=='low': ser.dtr=False",
  " if dtr=='high': ser.dtr=True",
  'except Exception:',
  " print('DTR_UNSUPPORTED')",
  'if start: ser.write(binascii.unhexlify(start)); ser.flush()',
  'n=0; end=time.time()+dur',
  'while time.time()<end and n<maxb:',
  ' d=ser.read(4096)',
  ' if d:',
  '  d=d[:maxb-n]; n+=len(d)',
  "  print('HEX '+binascii.hexlify(d).decode()); sys.stdout.flush()",
  'if stop: ser.write(binascii.unhexlify(stop)); ser.flush()',
  'ser.close()',
  "print('END '+str(n))",
].join('\n');

const HEX_RE = /^(?:[0-9a-fA-F]{2}){0,64}$/;
export const SERIAL_BAUDS = Object.freeze([9600, 19200, 38400, 57600, 115200, 230400, 256000, 460800, 921600]);
export const MAX_CAPTURE_BYTES = 8 * 1024 * 1024;

/**
 * Capture raw bytes from a serial port.
 * opts: { port, baud, seconds, maxBytes, startHex, stopHex, dtr: 'keep'|'low'|'high' }
 * Returns { ok, bytes: Buffer, reason?, detail? }.
 */
export async function readSerial(opts, deps) {
  validatePort(opts.port);
  if (!SERIAL_BAUDS.includes(opts.baud)) throw Object.assign(new Error(`baud must be one of ${SERIAL_BAUDS.join(', ')}`), { reason: 'invalid_baud' });
  const seconds = Math.min(Math.max(Number(opts.seconds) || 5, 0.5), 120);
  const maxBytes = Math.min(Math.max(Number(opts.maxBytes) || 1_048_576, 1), MAX_CAPTURE_BYTES);
  const startHex = opts.startHex || '';
  const stopHex = opts.stopHex || '';
  if (!HEX_RE.test(startHex) || !HEX_RE.test(stopHex)) throw Object.assign(new Error('command bytes must be short hex strings'), { reason: 'invalid_command' });
  const dtr = opts.dtr || 'keep';
  if (!['keep', 'low', 'high'].includes(dtr)) throw Object.assign(new Error('dtr must be keep, low, or high'), { reason: 'invalid_dtr' });

  const python = deps.python();
  if (!python) return { ok: false, reason: 'python_missing', remedy: 'Install Python 3 and `pip install pyserial`.', bytes: Buffer.alloc(0) };
  const r = await deps.exec(python, ['-c', PUMP_SCRIPT, opts.port, String(opts.baud), String(seconds), String(maxBytes), startHex, stopHex, dtr], {
    timeoutMs: (seconds + 15) * 1000,
    maxOutputBytes: maxBytes * 2 + Math.ceil(maxBytes / 2048) * 8 + 65_536,
    fullOutput: true,
  });
  const out = r.stdout;
  if (out.includes('NO_PYSERIAL')) return { ok: false, reason: 'pyserial_missing', remedy: 'pip install pyserial', bytes: Buffer.alloc(0) };
  const openError = /OPEN_ERROR (.*)/.exec(out);
  if (openError) {
    return { ok: false, reason: 'port_open_failed', detail: openError[1], remedy: 'Close other serial monitors (ESP-IDF monitor, Arduino, the sensing server) and check permissions (Linux: dialout group).', bytes: Buffer.alloc(0) };
  }
  const chunks = [];
  for (const line of out.split(/\r?\n/)) if (/^HEX [0-9a-f]*$/.test(line)) chunks.push(Buffer.from(line.slice(4), 'hex'));
  const bytes = Buffer.concat(chunks);
  if (!r.ok && !bytes.length) return { ok: false, reason: 'capture_failed', detail: (r.stderr || r.error || '').slice(-400), bytes };
  const warnings = [];
  if (out.includes('DTR_UNSUPPORTED')) warnings.push('port does not support DTR control; an RPLIDAR A1 motor may not spin');
  if (!r.ok) warnings.push('serial capture process failed; partial bytes retained');
  return { ok: true, bytes, seconds, ...(!r.ok ? { captureIncomplete: true } : {}), ...(warnings.length ? { warnings } : {}) };
}
