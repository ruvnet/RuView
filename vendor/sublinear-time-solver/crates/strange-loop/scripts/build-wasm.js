#!/usr/bin/env node

/**
 * Build script for Strange Loop WASM modules
 *
 * This script automates the compilation of the Strange Loop Rust crate
 * into WebAssembly modules for use in the NPX CLI and SDK.
 */

const fs = require('fs-extra');
const path = require('path');
const { execSync } = require('child_process');
const chalk = require('chalk');

const PROJECT_ROOT = path.join(__dirname, '..');
const RUST_CRATE_PATH = path.join(PROJECT_ROOT, '..', 'crates', 'strange-loop');
const WASM_OUTPUT_PATH = path.join(PROJECT_ROOT, 'wasm');

console.log(chalk.cyan('🔧 Building Strange Loop WASM modules...\n'));

async function buildWasm() {
  try {
    // Ensure output directory exists
    await fs.ensureDir(WASM_OUTPUT_PATH);

    console.log(chalk.yellow('📦 Compiling Rust crate to WASM...'));

    // Change to Rust crate directory
    process.chdir(RUST_CRATE_PATH);

    // Build for web target
    console.log(chalk.gray('Building for web target...'));
    execSync('wasm-pack build --target web --features wasm --release', {
      stdio: 'inherit'
    });

    // Build for Node.js target
    console.log(chalk.gray('Building for Node.js target...'));
    execSync('wasm-pack build --target nodejs --features wasm --release --out-dir pkg-nodejs', {
      stdio: 'inherit'
    });

    // Copy web build to NPX package
    console.log(chalk.yellow('📁 Copying WASM files...'));

    const webPkgPath = path.join(RUST_CRATE_PATH, 'pkg');
    const nodePkgPath = path.join(RUST_CRATE_PATH, 'pkg-nodejs');

    // Copy web version
    if (await fs.pathExists(webPkgPath)) {
      await fs.copy(webPkgPath, path.join(WASM_OUTPUT_PATH, 'web'));
      console.log(chalk.green('✅ Web WASM files copied'));
    }

    // Copy Node.js version
    if (await fs.pathExists(nodePkgPath)) {
      await fs.copy(nodePkgPath, path.join(WASM_OUTPUT_PATH, 'nodejs'));
      console.log(chalk.green('✅ Node.js WASM files copied'));
    }

    // Create unified entry point
    await createUnifiedEntry();

    // Verify build
    await verifyBuild();

    console.log(chalk.green('\n🎉 WASM build completed successfully!'));

  } catch (error) {
    console.error(chalk.red(`\n❌ Build failed: ${error.message}`));
    process.exit(1);
  }
}

async function createUnifiedEntry() {
  console.log(chalk.yellow('🔗 Creating unified entry point...'));

  const entryContent = `
// Strange Loop WASM Entry Point
// Automatically detects environment and loads appropriate WASM module

let wasmModule = null;

async function init() {
  if (wasmModule) return wasmModule;

  try {
    if (typeof window !== 'undefined') {
      // Browser environment
      const wasmInit = await import('./web/strange_loop.js');
      wasmModule = await wasmInit.default();
    } else {
      // Node.js environment
      const wasmInit = require('./nodejs/strange_loop.js');
      wasmModule = await wasmInit();
    }

    return wasmModule;
  } catch (error) {
    throw new Error(\`Failed to initialize WASM module: \${error.message}\`);
  }
}

module.exports = { init };

if (typeof window !== 'undefined') {
  window.StrangeLoopWasm = { init };
}
`;

  await fs.writeFile(path.join(WASM_OUTPUT_PATH, 'index.js'), entryContent.trim());
  console.log(chalk.green('✅ Unified entry point created'));
}

async function verifyBuild() {
  console.log(chalk.yellow('🔍 Verifying build...'));

  const requiredFiles = [
    'web/strange_loop.wasm',
    'web/strange_loop.js',
    'nodejs/strange_loop.wasm',
    'nodejs/strange_loop.js',
    'index.js'
  ];

  for (const file of requiredFiles) {
    const filePath = path.join(WASM_OUTPUT_PATH, file);
    if (!(await fs.pathExists(filePath))) {
      throw new Error(`Required file missing: ${file}`);
    }
  }

  // Check file sizes
  const webWasmPath = path.join(WASM_OUTPUT_PATH, 'web', 'strange_loop.wasm');
  const webWasmStats = await fs.stat(webWasmPath);
  const webWasmSizeKB = Math.round(webWasmStats.size / 1024);

  console.log(chalk.green(`✅ Build verification passed`));
  console.log(chalk.gray(`   Web WASM size: ${webWasmSizeKB}KB`));
}

// Run build if script is executed directly
if (require.main === module) {
  buildWasm();
}

module.exports = { buildWasm };