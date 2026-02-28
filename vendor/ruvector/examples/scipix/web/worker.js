/**
 * Web Worker for off-main-thread OCR processing
 */

import init, { setupWorker } from '../pkg/ruvector_mathpix.js';

let initialized = false;

// Initialize WASM in worker
async function initialize() {
  if (initialized) return;

  try {
    await init();
    setupWorker();
    initialized = true;

    self.postMessage({ type: 'Ready' });
  } catch (error) {
    console.error('Worker initialization failed:', error);
    self.postMessage({
      type: 'Error',
      error: error.message
    });
  }
}

// Auto-initialize
initialize();
