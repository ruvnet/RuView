// SPDX-License-Identifier: MIT
// Device discovery (ADR-373): enumerate serial ports and classify them by USB
// VID:PID into likely RuView roles. Classification is a hint, not identity —
// many boards share generic USB-UART bridges, so each candidate lists the
// read command that confirms it. `realtek` marks RTL8721Dx (Ameba) CSI boards
// (ADR-323), which stream RAC1 over UDP and log at 1500000 baud.

import { listSerialPorts } from '../firmware.js';

/** Known USB bridges/native USB in the RuView hardware set. */
export const USB_IDS = Object.freeze({
  '303A:1001': { chip: 'Espressif native USB (ESP32-S3/C3/C6 USB-Serial/JTAG)', roles: ['esp32'] },
  '303A:4001': { chip: 'Espressif native USB (ESP32-S2/S3 CDC)', roles: ['esp32'] },
  '10C4:EA60': { chip: 'Silicon Labs CP210x', roles: ['esp32', 'rplidar', 'mmwave'] },
  '1A86:7523': { chip: 'WCH CH340', roles: ['esp32', 'mmwave'] },
  '1A86:55D4': { chip: 'WCH CH9102', roles: ['esp32', 'mmwave'] },
  '1A86:55D3': { chip: 'WCH CH343', roles: ['esp32', 'mmwave'] },
  '0403:6001': { chip: 'FTDI FT232R', roles: ['mmwave', 'rplidar', 'esp32'] },
  '0403:6015': { chip: 'FTDI FT231X', roles: ['mmwave', 'esp32'] },
  '067B:23A3': { chip: 'Prolific PL2303GC (Realtek Ameba RTL8721Dx boards)', roles: ['realtek'] },
  '067B:2303': { chip: 'Prolific PL2303', roles: ['realtek', 'esp32'] },
  '2341:0043': { chip: 'Arduino Uno', roles: [] },
});

const NEXT = Object.freeze({
  esp32: (p) => `ruview doctor --port ${p} --probe   |   ruview monitor --port ${p}`,
  mmwave: (p) => `ruview mmwave --port ${p} --model auto`,
  rplidar: (p) => `ruview lidar --source rplidar --port ${p}`,
  realtek: (p) => `ruview monitor --port ${p} --baud 1500000   |   ruview esp32 --seconds 10  (RAC1 over UDP)`,
});

export function classifyPort(port) {
  const key = port.vid && port.pid ? `${port.vid}:${port.pid}` : null;
  const known = key ? USB_IDS[key] : null;
  const text = `${port.description || ''} ${port.hwid || ''}`;
  let roles = known ? [...known.roles] : [];
  if (/slamtec|rplidar/i.test(text)) roles = ['rplidar'];
  else if (/espressif|esp32|usb jtag/i.test(text) && !roles.includes('esp32')) roles.unshift('esp32');
  const builtin = /ttyS\d+|Bluetooth|debug-console|wlan-debug/i.test(port.port + text);
  return {
    ...port,
    usb: key,
    bridge: known?.chip || null,
    likelyRoles: builtin ? [] : roles,
    builtin,
    confirmWith: builtin ? [] : roles.map((r) => NEXT[r](port.port)),
  };
}

export async function scanDevices(deps) {
  const listed = await listSerialPorts(deps);
  if (!listed.ok) return { ok: false, reason: listed.reason, remedy: listed.reason === 'pyserial_missing' ? 'pip install pyserial' : listed.remedy, devices: [] };
  const devices = listed.ports.map(classifyPort);
  const candidates = devices.filter((d) => d.likelyRoles.length);
  return {
    ok: true,
    devices,
    candidates: candidates.length,
    network: {
      esp32Udp: 'ESP32 nodes stream UDP to their provisioned target; run `ruview esp32 --seconds 10` on that host (default UDP 5005).',
      iphoneLidar: 'Start integrations/iphone-lidar/web relay, export RUVIEW_LIDAR_TOKEN, then `ruview lidar --source iphone --url ws://HOST:8787/ws/lidar`.',
    },
    note: candidates.length ? 'Roles are VID:PID hints; confirm with the listed read command.' : 'No USB serial devices matched known bridges. Check the cable (data-capable) and drivers (CP210x/CH34x/PL2303).',
  };
}
