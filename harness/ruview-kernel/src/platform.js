// SPDX-License-Identifier: MIT
// napi-rs platform triple resolution (matches @napi-rs/cli naming).

import { readFileSync } from 'node:fs';

function isMusl() {
  if (process.platform !== 'linux') return false;
  try {
    const report = process.report?.getReport?.();
    const header = typeof report === 'string' ? JSON.parse(report).header : report?.header;
    if (header && 'glibcVersionRuntime' in header) return !header.glibcVersionRuntime;
  } catch { /* fall through */ }
  try {
    return readFileSync('/usr/bin/ldd', 'utf8').includes('musl');
  } catch {
    return false;
  }
}

/** e.g. `linux-x64-gnu`, `darwin-arm64`, `win32-x64-msvc`. */
export function platformTriple(platform = process.platform, arch = process.arch) {
  if (platform === 'linux') return `linux-${arch}-${isMusl() ? 'musl' : 'gnu'}`;
  if (platform === 'win32') return `win32-${arch}-msvc`;
  return `${platform}-${arch}`;
}
