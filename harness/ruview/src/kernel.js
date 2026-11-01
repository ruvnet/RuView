// SPDX-License-Identifier: MIT
// Optional bridge to the `@ruvnet/ruview-kernel` compute package (ADR-368).
//
// @ruvnet/ruview stays free of runtime dependencies: the kernel package is
// resolved only when this tool runs, and its absence is an honest negative,
// never a fabricated pass. The importer is injectable for tests only; MCP
// arguments can never choose which module is loaded.

export const KERNEL_PACKAGE = '@ruvnet/ruview-kernel';
export const KERNEL_BACKENDS = Object.freeze(['wasm', 'napi', 'auto']);

let packageImporter = (specifier) => import(specifier);
const defaultImporter = (specifier) => packageImporter(specifier);

/**
 * Let an embedding package (the `ruview` umbrella, ADR-376) resolve the kernel
 * from its own dependency tree. Node resolves bare specifiers relative to the
 * importing file's real path, which misses a sibling dependency under linked or
 * pnpm-style installs. Only the embedding code can call this; MCP arguments
 * cannot choose the module.
 */
export function setKernelImporter(importer) {
  if (typeof importer !== 'function') throw new TypeError('importer must be a function');
  packageImporter = importer;
}

/** Import the kernel package through the active importer (used by doctor too). */
export const importKernelPackage = () => defaultImporter(KERNEL_PACKAGE);

async function importKernel(importer) {
  try {
    return { mod: await importer(KERNEL_PACKAGE) };
  } catch (error) {
    return {
      failure: {
        ok: false,
        reason: 'kernel_not_installed',
        detail: String(error?.code || error?.message || error),
        remedy: `npm install ${KERNEL_PACKAGE}  (or, in a RuView checkout: cd harness/ruview-kernel && npm run build)`,
      },
    };
  }
}

function load(mod, backend) {
  try {
    return { kernel: mod.loadKernel({ backend }) };
  } catch (error) {
    return { failure: { ok: false, reason: error?.code || 'kernel_unavailable', requestedBackend: backend, detail: String(error?.message || error) } };
  }
}

/** Run the kernel's SYNTHETIC end-to-end self-test on the requested backend. */
export async function kernelSelfTest(args = {}, { importer = defaultImporter } = {}) {
  const backend = args.backend || 'wasm';
  const { mod, failure } = await importKernel(importer);
  if (failure) return failure;
  if (typeof mod.loadKernel !== 'function' || typeof mod.selfTest !== 'function') {
    return { ok: false, reason: 'kernel_incompatible', detail: `${KERNEL_PACKAGE} does not export loadKernel/selfTest` };
  }
  const loaded = load(mod, backend);
  if (loaded.failure) return loaded.failure;
  const { kernel } = loaded;
  const result = mod.selfTest(kernel, { seconds: args.seconds ?? 60 });
  return {
    ...result,
    requestedBackend: kernel.requestedBackend,
    fallbackReason: kernel.fallbackReason,
    integrity: kernel.integrity,
    sha256: kernel.sha256,
    note: 'SYNTHETIC self-test of the signal pipeline; not evidence of real-world sensing accuracy.',
  };
}

/**
 * Bind the kernel's vitals pipeline to live frames: returns an async
 * (frames, config) → { ok, backend, summary } used by ruview_esp32_capture.
 */
export function kernelAnalyzer({ backend = 'wasm' } = {}, { importer = defaultImporter } = {}) {
  return async (frames, config) => {
    const { mod, failure } = await importKernel(importer);
    if (failure) return failure;
    if (typeof mod.loadKernel !== 'function') return { ok: false, reason: 'kernel_incompatible', detail: `${KERNEL_PACKAGE} does not export loadKernel` };
    const loaded = load(mod, backend);
    if (loaded.failure) return loaded.failure;
    const { kernel } = loaded;
    const report = kernel.analyze(frames, config); // throws KernelError on invalid input
    return { ok: true, backend: kernel.backend, integrity: kernel.integrity, config: report.config, summary: report.summary };
  };
}
