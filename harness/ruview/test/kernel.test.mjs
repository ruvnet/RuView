import test from 'node:test';
import assert from 'node:assert/strict';
import { importKernelPackage, kernelSelfTest, KERNEL_PACKAGE, setKernelImporter } from '../src/kernel.js';
import { authorizeTool } from '../src/policy.js';
import { runTool } from '../src/tools.js';

const fakeKernel = (overrides = {}) => ({
  loadKernel: ({ backend }) => ({ backend: backend === 'auto' ? 'wasm' : backend, requestedBackend: backend, fallbackReason: backend === 'auto' ? 'no addon' : null, integrity: 'verified', sha256: 'ab' }),
  selfTest: (kernel, { seconds }) => ({ ok: true, evidence: 'SYNTHETIC', backend: kernel.backend, seconds }),
  ...overrides,
});

test('kernel self-test fails closed when the optional package is absent', async () => {
  const result = await kernelSelfTest({}, { importer: async () => { const e = new Error('nope'); e.code = 'ERR_MODULE_NOT_FOUND'; throw e; } });
  assert.equal(result.ok, false);
  assert.equal(result.reason, 'kernel_not_installed');
  assert.match(result.remedy, /@ruvnet\/ruview-kernel/);
});

test('kernel self-test only imports the fixed package name', async () => {
  const seen = [];
  const result = await kernelSelfTest({ backend: 'auto', seconds: 45 }, { importer: async (s) => { seen.push(s); return fakeKernel(); } });
  assert.deepEqual(seen, [KERNEL_PACKAGE]);
  assert.equal(result.ok, true);
  assert.equal(result.backend, 'wasm');
  assert.equal(result.requestedBackend, 'auto');
  assert.equal(result.fallbackReason, 'no addon');
  assert.equal(result.seconds, 45);
  assert.equal(result.evidence, 'SYNTHETIC');
});

test('kernel self-test reports load failures and incompatible packages honestly', async () => {
  const failing = fakeKernel({ loadKernel: () => { const e = new Error('no addon'); e.code = 'backend_unavailable'; throw e; } });
  const r1 = await kernelSelfTest({ backend: 'napi' }, { importer: async () => failing });
  assert.deepEqual([r1.ok, r1.reason, r1.requestedBackend], [false, 'backend_unavailable', 'napi']);
  const r2 = await kernelSelfTest({}, { importer: async () => ({}) });
  assert.deepEqual([r2.ok, r2.reason], [false, 'kernel_incompatible']);
});

test('kernel tool is read-only for MCP and rejects module/path injection', async () => {
  assert.equal(authorizeTool('ruview_kernel_selftest', {}, { source: 'mcp', grants: [] }).ok, true);
  for (const args of [{ module: 'fs' }, { wasm_path: '/tmp/x.wasm' }, { backend: 'python' }, { seconds: 5 }]) {
    const result = await runTool('ruview_kernel_selftest', args, { source: 'mcp' });
    assert.equal(result.reason, 'invalid_arguments', JSON.stringify(args));
  }
});

// Cross-package end-to-end (ADR-368). CI installs the packed
// @ruvnet/ruview-kernel tarball with `npm install --no-save` before this runs;
// elsewhere the package is optional and the fail-closed path is covered above.
let installed = false;
try { await import(KERNEL_PACKAGE); installed = true; } catch { /* optional */ }

test('installed kernel passes its SYNTHETIC self-test through the harness tool', { skip: !installed && `${KERNEL_PACKAGE} not installed` }, async () => {
  const result = await runTool('ruview_kernel_selftest', { backend: 'wasm', seconds: 45 }, { source: 'mcp', grants: [] });
  assert.equal(result.ok, true, JSON.stringify(result.checks));
  assert.equal(result.backend, 'wasm');
  assert.equal(result.evidence, 'SYNTHETIC');
  assert.equal(result.integrity, 'verified');
});

test('an embedding package can supply the kernel importer (ADR-376)', async () => {
  const seen = [];
  setKernelImporter(async (specifier) => { seen.push(specifier); return { loadKernel: () => { throw Object.assign(new Error('stub'), { code: 'backend_unavailable' }); }, selfTest: () => ({}) }; });
  try {
    await importKernelPackage();
    const r = await kernelSelfTest({ backend: 'wasm' });
    assert.deepEqual(seen, [KERNEL_PACKAGE, KERNEL_PACKAGE]);
    assert.equal(r.reason, 'backend_unavailable');
    assert.throws(() => setKernelImporter('not a function'), /importer must be a function/);
  } finally {
    setKernelImporter((specifier) => import(specifier));
  }
});
