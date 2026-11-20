#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// `npx ruview` launcher. The logic lives in src/cli.js, which loads modules on
// demand (ADR-375); `run` is re-exported for tests and embedders.

import { realpathSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { argv } from 'node:process';
import { run } from '../src/cli.js';

export { run };

// CLI guard: run only when invoked directly (realpath both sides — npm/npx shims
// pass a non-normalized, possibly case-skewed argv[1] on Windows).
const invokedDirectly = (() => {
  if (!argv[1]) return false;
  try {
    const a = realpathSync(argv[1]);
    const b = realpathSync(fileURLToPath(import.meta.url));
    return process.platform === 'win32' ? a.toLowerCase() === b.toLowerCase() : a === b;
  } catch { return false; }
})();
if (invokedDirectly) {
  run(argv.slice(2)).then((code) => process.exit(code)).catch((err) => { console.error(err); process.exit(1); });
}
