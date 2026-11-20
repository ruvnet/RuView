import { existsSync } from 'node:fs';
import { DEFAULT_WASM_PATH, nativeCandidates } from '../src/index.js';

export const HAS_WASM = existsSync(DEFAULT_WASM_PATH);
export const HAS_NAPI = nativeCandidates().some((p) => existsSync(p));
// CI sets RUVIEW_KERNEL_REQUIRE_NAPI=1 after `npm run build:napi` so a missing
// addon is a failure there, while contributors without Rust can still run tests.
export const REQUIRE_NAPI = process.env.RUVIEW_KERNEL_REQUIRE_NAPI === '1';
export const napiSkip = !HAS_NAPI && !REQUIRE_NAPI ? 'napi addon not built (npm run build:napi)' : false;

export function requireWasm() {
  if (!HAS_WASM) throw new Error(`WASM kernel missing at ${DEFAULT_WASM_PATH}; run \`npm run build:wasm\` first`);
}

/** Smallest valid module that imports one host function ("env"."f"). */
export const MODULE_WITH_IMPORT = new Uint8Array([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
  0x01, 0x04, 0x01, 0x60, 0x00, 0x00,
  0x02, 0x09, 0x01, 0x03, 0x65, 0x6e, 0x76, 0x01, 0x66, 0x00, 0x00,
]);
