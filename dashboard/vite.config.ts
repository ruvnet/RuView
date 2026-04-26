import { defineConfig } from 'vite';

// Dashboard for ADR-092 — Vite + Lit + WASM in a Web Worker.
// Hosted at /RuView/nvsim/ on GitHub Pages; base path is configurable
// via NVSIM_BASE so local dev (npm run dev) stays at "/".
const base = (globalThis as { process?: { env?: { NVSIM_BASE?: string } } }).process?.env?.NVSIM_BASE ?? '/';

export default defineConfig({
  base,
  publicDir: 'public',
  worker: {
    format: 'es',
  },
  build: {
    target: 'es2022',
    sourcemap: true,
    rollupOptions: {
      output: {
        manualChunks: {
          lit: ['lit'],
          signals: ['@preact/signals-core'],
        },
      },
    },
  },
  server: {
    port: 5173,
    strictPort: true,
    fs: {
      // wasm-pack output sits in public/nvsim-pkg; vite already serves it,
      // but allow fs reads from the workspace root for HMR convenience.
      allow: ['..', '.'],
    },
    headers: {
      // SAB ring buffer is opt-in; these headers are no-op without crossOriginIsolated
      // but make local dev parity with a future CORS-isolated host.
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
    },
  },
});
