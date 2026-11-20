// SPDX-License-Identifier: MIT
// `ruview` umbrella (ADR-376): one install for the RuView operator harness
// (@ruvnet/ruview), the Homecore developer metaharness (homecore) and the WASM
// vitals kernel (@ruvnet/ruview-kernel). Components load lazily so `--help`
// and the harness commands never pay for Homecore or the kernel.

import { readFileSync } from 'node:fs';

const PKG = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
export const VERSION = PKG.version;

/**
 * Resolve the kernel from this package's dependency tree for every harness
 * code path (esp32 --analyze, kernel self-test, doctor), including linked and
 * pnpm-style installs where the harness cannot see its sibling.
 */
export async function wireKernel() {
  const { setKernelImporter } = await import('@ruvnet/ruview/kernel');
  setKernelImporter((specifier) => import(specifier));
}

/** Kernel CLI subcommands; `ruview kernel` with none of these stays the harness self-test. */
export const KERNEL_COMMANDS = Object.freeze(['doctor', 'info', 'selftest', 'parity', 'synth', 'analyze', 'bench']);

export const COMPONENTS = Object.freeze([
  { name: '@ruvnet/ruview', role: 'operator harness: devices, firmware, training, doctor, MCP, ChatGPT/MCP Apps console' },
  { name: 'homecore', role: 'Homecore developer metaharness: source-cited guidance, WASM kernel status, verification' },
  { name: '@ruvnet/ruview-kernel', role: 'portable vitals kernel (WASM, optional napi) used by `esp32 --analyze`' },
]);

/** Installed component versions, read from each package's own package.json. */
export async function componentVersions(importer = (s) => import(s, { with: { type: 'json' } })) {
  const out = {};
  for (const c of COMPONENTS) {
    try {
      const url = import.meta.resolve(c.name === '@ruvnet/ruview-kernel' ? `${c.name}/package.json` : c.name);
      const pkgUrl = c.name === '@ruvnet/ruview-kernel' ? url : new URL('../package.json', url).href;
      out[c.name] = (await importer(pkgUrl)).default.version;
    } catch {
      out[c.name] = null;
    }
  }
  return out;
}

/** Everything this install can do, grouped by component (used by `ruview capabilities`). */
export async function capabilities() {
  const [{ listTools: ruviewTools }, homecore, versions] = await Promise.all([
    import('@ruvnet/ruview'),
    import('homecore').catch(() => null),
    componentVersions(),
  ]);
  return {
    ok: true,
    package: 'ruview',
    version: VERSION,
    components: COMPONENTS.map((c) => ({ ...c, version: versions[c.name] })),
    tools: {
      harness: ruviewTools().map((t) => t.name),
      homecore: homecore ? homecore.listTools().map((t) => t.name) : [],
      kernelCli: [...KERNEL_COMMANDS],
    },
    entryPoints: {
      cli: 'ruview <command>  (ruview --help)',
      homecore: 'ruview homecore <command>',
      kernel: `ruview kernel ${KERNEL_COMMANDS.join('|')}`,
      mcpStdio: 'ruview mcp start',
      mcpHttp: 'ruview mcp start --http   (ChatGPT / remote MCP; token: RUVIEW_MCP_TOKEN)',
    },
  };
}
