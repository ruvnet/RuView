// SPDX-License-Identifier: MIT
// Non-throwing wrapper over the scrubbed-environment process runner, for
// operator tools (esptool, cargo, python). Never uses a shell.

import { DEFAULT_ENV_ALLOWLIST, runProcess } from './process-runner.js';

// Toolchain locations operators legitimately configure; no credentials.
export const TOOLCHAIN_ENV = Object.freeze([
  ...DEFAULT_ENV_ALLOWLIST,
  'CARGO_HOME', 'RUSTUP_HOME', 'RUSTUP_TOOLCHAIN', 'VIRTUAL_ENV', 'CONDA_PREFIX',
  'LIBTORCH', 'LIBTORCH_USE_PYTORCH', 'LIBTORCH_BYPASS_VERSION_CHECK', 'LD_LIBRARY_PATH', 'DYLD_LIBRARY_PATH', 'CUDA_HOME', 'IDF_PATH',
]);

const TAIL = 16_384;

/** Run a command; resolves {ok, code, stdout, stderr, error} and never rejects. */
export async function execTool(command, args, { cwd, timeoutMs = 120_000, maxOutputBytes = 4_194_304, fullOutput = false } = {}) {
  // fullOutput: return all of stdout (bounded by maxOutputBytes) for binary
  // captures; otherwise keep only a diagnostic tail.
  const tail = (text) => (fullOutput ? text : text.slice(-TAIL));
  try {
    const r = await runProcess(command, args, { cwd, timeoutMs, maxOutputBytes, envAllowlist: TOOLCHAIN_ENV });
    return { ok: true, code: r.code, stdout: tail(r.stdout), stderr: r.stderr.slice(-TAIL), error: null };
  } catch (error) {
    return {
      ok: false,
      code: error.code ?? null,
      stdout: tail(String(error.stdout || '')),
      stderr: String(error.stderr || '').slice(-TAIL),
      error: String(error.message || error).slice(0, 2000),
      timedOut: Boolean(error.timedOut),
    };
  }
}
