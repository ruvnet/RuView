import { defineConfig } from 'vite';

export default defineConfig({
  base: '/',
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
    rollupOptions: {
      output: {
        manualChunks: {
          three: ['three'],
          d3: ['d3-scale', 'd3-axis', 'd3-shape', 'd3-selection'],
        },
      },
    },
  },
});
