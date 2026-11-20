// SPDX-License-Identifier: MIT
// Browser- and Node-safe WASM transport (ADR-368). No filesystem access here:
// callers supply the module bytes, so this file can be bundled for the web.
//
// The module imports nothing from the host (no fs, network, clock, or
// randomness); instantiation fails closed if that ever changes.

import { ABI_VERSION, KernelError, createKernel } from './abi.js';

const REQUIRED_EXPORTS = ['memory', 'rvk_abi_version', 'rvk_alloc', 'rvk_free', 'rvk_call'];
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });

/** Compile and validate a module; rejects any host import. */
export function compileKernelModule(bytes) {
  const module = new WebAssembly.Module(bytes);
  const imports = WebAssembly.Module.imports(module);
  if (imports.length !== 0) {
    throw new KernelError('untrusted_module', `kernel module must not import host functions (found ${imports.length})`);
  }
  const names = new Set(WebAssembly.Module.exports(module).map((e) => e.name));
  const missing = REQUIRED_EXPORTS.filter((n) => !names.has(n));
  if (missing.length) throw new KernelError('untrusted_module', `kernel module is missing exports: ${missing.join(', ')}`);
  return module;
}

function instantiate(module) {
  const { exports } = new WebAssembly.Instance(module, {});
  const abi = exports.rvk_abi_version();
  if (abi !== ABI_VERSION) throw new KernelError('abi_mismatch', `WASM ABI ${abi} != loader ABI ${ABI_VERSION}`);
  return exports;
}

function write(exports, text) {
  const bytes = encoder.encode(text);
  const ptr = exports.rvk_alloc(bytes.length);
  if (ptr === 0) throw new KernelError('out_of_memory', 'kernel allocation failed');
  new Uint8Array(exports.memory.buffer, ptr, bytes.length).set(bytes);
  return [ptr, bytes.length];
}

/**
 * Create a raw `(op, json) => json` transport. A trap (Rust panic, OOM)
 * poisons the instance; the next call gets a fresh one, and open sessions
 * from the trapped instance are lost (reported as `unknown_session`).
 */
export function createWasmTransport(module) {
  let exports = instantiate(module);
  let generation = 0;
  const transport = (op, json) => {
    if (!exports) { exports = instantiate(module); generation += 1; }
    try {
      const [opPtr, opLen] = write(exports, op);
      const [inPtr, inLen] = write(exports, json);
      const packed = exports.rvk_call(opPtr, opLen, inPtr, inLen);
      if (packed === 0n) throw new KernelError('out_of_memory', 'kernel response allocation failed');
      const ptr = Number(packed >> 32n);
      const len = Number(packed & 0xffffffffn);
      const text = decoder.decode(new Uint8Array(exports.memory.buffer, ptr, len).slice());
      exports.rvk_free(ptr, len);
      return text;
    } catch (error) {
      if (error instanceof KernelError) throw error;
      exports = null;
      throw new KernelError('trap', `WASM kernel trapped: ${error?.message || error}`);
    }
  };
  transport.generation = () => generation;
  transport.f64 = (op, json, data) => {
    if (!exports) { exports = instantiate(module); generation += 1; }
    if (typeof exports.rvk_call_f64 !== 'function') throw new KernelError('backend_unavailable', 'WASM module predates the binary transport');
    try {
      const [opPtr, opLen] = write(exports, op);
      const [inPtr, inLen] = write(exports, json);
      const bytes = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
      const dataPtr = exports.rvk_alloc(bytes.length);
      if (dataPtr === 0) throw new KernelError('out_of_memory', 'kernel allocation failed');
      new Uint8Array(exports.memory.buffer, dataPtr, bytes.length).set(bytes);
      const packed = exports.rvk_call_f64(opPtr, opLen, inPtr, inLen, dataPtr, bytes.length);
      if (packed === 0n) throw new KernelError('out_of_memory', 'kernel response allocation failed');
      const ptr = Number(packed >> 32n);
      const len = Number(packed & 0xffffffffn);
      const text = decoder.decode(new Uint8Array(exports.memory.buffer, ptr, len).slice());
      exports.rvk_free(ptr, len);
      return text;
    } catch (error) {
      if (error instanceof KernelError) throw error;
      exports = null;
      throw new KernelError('trap', `WASM kernel trapped: ${error?.message || error}`);
    }
  };
  return transport;
}

/** Build the high-level kernel API from WASM bytes (browser or Node). */
export function loadWasmKernelFromBytes(bytes, meta = {}) {
  const module = compileKernelModule(bytes);
  const transport = createWasmTransport(module);
  const hasF64 = WebAssembly.Module.exports(module).some((e) => e.name === 'rvk_call_f64');
  return createKernel(transport, { backend: 'wasm', abi: ABI_VERSION, ...meta }, hasF64 ? transport.f64 : null);
}
