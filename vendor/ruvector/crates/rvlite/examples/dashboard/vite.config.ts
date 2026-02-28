import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],

  // Configure optimizations
  optimizeDeps: {
    // Don't pre-bundle the WASM pkg - it's self-contained
    exclude: ['rvlite']
  },

  // Configure server to handle WASM files
  server: {
    headers: {
      // Enable SharedArrayBuffer if needed
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
    },
    // Allow serving files from public/pkg
    fs: {
      strict: false,
    },
  },

  // Ensure WASM files are handled correctly
  assetsInclude: ['**/*.wasm'],

  // Build configuration
  build: {
    // Don't inline WASM files
    assetsInlineLimit: 0,
    rollupOptions: {
      output: {
        // Keep WASM files as separate assets
        assetFileNames: (assetInfo) => {
          if (assetInfo.name?.endsWith('.wasm')) {
            return 'assets/[name][extname]';
          }
          return 'assets/[name]-[hash][extname]';
        },
      },
    },
  },
})
