#!/usr/bin/env node

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Create dist/wasm directory
const distWasmDir = path.join(__dirname, '..', 'dist', 'wasm');
const distWasmTemporalDir = path.join(distWasmDir, 'temporal-attractor');

// Create directories if they don't exist
if (!fs.existsSync(distWasmDir)) {
  fs.mkdirSync(distWasmDir, { recursive: true });
  console.log('Created dist/wasm directory');
}

if (!fs.existsSync(distWasmTemporalDir)) {
  fs.mkdirSync(distWasmTemporalDir, { recursive: true });
  console.log('Created dist/wasm/temporal-attractor directory');
}

// WASM files to copy and their source locations
const wasmFiles = [
  {
    source: 'crates/strange-loop/wasm/strange_loop_bg.wasm',
    dest: 'dist/wasm/strange_loop_bg.wasm',
    js: 'crates/strange-loop/wasm/strange_loop.js'
  },
  {
    source: 'crates/psycho-symbolic-reasoner/wasm-dist/extractors_bg.wasm',
    dest: 'dist/wasm/extractors_bg.wasm',
    js: 'crates/psycho-symbolic-reasoner/wasm-dist/extractors.js'
  },
  {
    source: 'crates/psycho-symbolic-reasoner/wasm-dist/planner_bg.wasm',
    dest: 'dist/wasm/planner_bg.wasm',
    js: 'crates/psycho-symbolic-reasoner/wasm-dist/planner.js'
  },
  {
    source: 'crates/psycho-symbolic-reasoner/wasm-dist/graph_reasoner_bg.wasm',
    dest: 'dist/wasm/graph_reasoner_bg.wasm',
    js: 'crates/psycho-symbolic-reasoner/wasm-dist/graph_reasoner.js'
  },
  {
    source: 'crates/temporal-neural-solver-wasm/dist/wasm/temporal_neural_solver_bg.wasm',
    dest: 'dist/wasm/temporal_neural_solver_bg.wasm',
    js: 'crates/temporal-neural-solver-wasm/dist/wasm/temporal_neural_solver.js'
  }
];

// Copy each WASM file
let copiedCount = 0;
let missingCount = 0;

wasmFiles.forEach(({ source, dest, js }) => {
  const sourcePath = path.join(__dirname, '..', source);
  const destPath = path.join(__dirname, '..', dest);
  const jsSourcePath = js ? path.join(__dirname, '..', js) : null;
  const jsDestPath = js ? destPath.replace('_bg.wasm', '.js') : null;

  // Copy WASM file
  if (fs.existsSync(sourcePath)) {
    try {
      fs.copyFileSync(sourcePath, destPath);
      console.log(`✓ Copied ${source} to ${dest}`);
      copiedCount++;
    } catch (error) {
      console.error(`✗ Failed to copy ${source}: ${error.message}`);
    }
  } else {
    console.warn(`⚠ Source file not found: ${source}`);
    missingCount++;
  }

  // Copy corresponding JS file if it exists
  if (jsSourcePath && jsDestPath && fs.existsSync(jsSourcePath)) {
    try {
      fs.copyFileSync(jsSourcePath, jsDestPath);
      console.log(`✓ Copied JS file: ${js}`);
    } catch (error) {
      console.error(`✗ Failed to copy JS file ${js}: ${error.message}`);
    }
  }
});

// Also check for alternative locations
const alternativeLocations = [
  {
    source: 'src/consciousness-explorer/wasm',
    files: ['extractors_bg.wasm', 'planner_bg.wasm', 'graph_reasoner_bg.wasm']
  }
];

alternativeLocations.forEach(({ source, files }) => {
  files.forEach(file => {
    const sourcePath = path.join(__dirname, '..', source, file);
    const destPath = path.join(distWasmDir, file);

    if (!fs.existsSync(destPath) && fs.existsSync(sourcePath)) {
      try {
        fs.copyFileSync(sourcePath, destPath);
        console.log(`✓ Copied ${file} from alternative location`);
        copiedCount++;
      } catch (error) {
        console.error(`✗ Failed to copy ${file}: ${error.message}`);
      }
    }
  });
});

console.log(`\nWASM copy complete: ${copiedCount} files copied, ${missingCount} missing`);

// Create a nano_consciousness_bg.wasm placeholder if needed
const nanoConsciousnessPath = path.join(distWasmDir, 'nano_consciousness_bg.wasm');
if (!fs.existsSync(nanoConsciousnessPath)) {
  console.log('⚠ nano_consciousness_bg.wasm not found, server will use fallback');
}