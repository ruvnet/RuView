/**
 * @ruvector/rvf-wasm — JS glue for the RVF WASM microkernel.
 *
 * Loads the .wasm binary and re-exports all C-ABI functions plus the
 * WASM linear memory object.
 *
 * Works in Node.js (CJS/ESM) and browsers.
 */

var wasmInstance = null;

var _isNode = typeof process !== 'undefined' &&
  typeof process.versions !== 'undefined' &&
  typeof process.versions.node === 'string';

/**
 * Initialize the WASM module.
 * Returns the exports object with all rvf_* functions and `memory`.
 *
 * @param {ArrayBuffer|BufferSource|WebAssembly.Module|string} [input]
 *   Optional pre-loaded bytes, Module, or file path override.
 */
async function init(input) {
  if (wasmInstance) return wasmInstance;

  var wasmBytes;

  if (input instanceof ArrayBuffer || ArrayBuffer.isView(input)) {
    wasmBytes = input;
  } else if (typeof WebAssembly !== 'undefined' && input instanceof WebAssembly.Module) {
    var inst = await WebAssembly.instantiate(input, {});
    wasmInstance = inst.exports;
    return wasmInstance;
  } else if (_isNode) {
    // Node.js: always use readFile (fetch on file:// is unreliable)
    var fs = await import('node:fs/promises');
    var url = await import('node:url');
    var path = await import('node:path');
    var wasmPath;
    if (typeof input === 'string') {
      wasmPath = input;
    } else if (typeof __dirname !== 'undefined') {
      // CJS context
      wasmPath = path.default.join(__dirname, 'rvf_wasm_bg.wasm');
    } else {
      // ESM context — import.meta.url available
      var thisDir = path.default.dirname(url.default.fileURLToPath(import.meta.url));
      wasmPath = path.default.join(thisDir, 'rvf_wasm_bg.wasm');
    }
    wasmBytes = await fs.default.readFile(wasmPath);
  } else {
    // Browser: use fetch + instantiateStreaming
    var wasmUrl = new URL('rvf_wasm_bg.wasm', import.meta.url);
    if (typeof WebAssembly.instantiateStreaming === 'function') {
      var resp = await fetch(wasmUrl);
      var result = await WebAssembly.instantiateStreaming(resp, {});
      wasmInstance = result.instance.exports;
      return wasmInstance;
    }
    var resp2 = await fetch(wasmUrl);
    wasmBytes = await resp2.arrayBuffer();
  }

  var compiled = await WebAssembly.instantiate(wasmBytes, {});
  wasmInstance = compiled.instance.exports;
  return wasmInstance;
}

// Support both ESM (export default) and CJS (module.exports)
init.default = init;
if (typeof module !== 'undefined') module.exports = init;
export default init;
