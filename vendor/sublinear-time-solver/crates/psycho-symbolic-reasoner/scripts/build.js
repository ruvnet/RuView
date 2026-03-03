#!/usr/bin/env node

/**
 * Build Script for Psycho-Symbolic Reasoner
 *
 * This script orchestrates the complete build process:
 * 1. Clean previous builds
 * 2. Build WASM components
 * 3. Build TypeScript
 * 4. Copy assets
 * 5. Validate build
 */

import { spawn } from 'child_process';
import { promises as fs } from 'fs';
import { join, resolve } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = resolve(__filename, '..');
const rootDir = resolve(__dirname, '..');

// Colors for console output
const colors = {
  reset: '\x1b[0m',
  bright: '\x1b[1m',
  red: '\x1b[31m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  magenta: '\x1b[35m',
  cyan: '\x1b[36m',
};

function log(color, prefix, message) {
  console.log(`${color}${prefix}${colors.reset} ${message}`);
}

function info(message) {
  log(colors.blue, '[INFO]', message);
}

function success(message) {
  log(colors.green, '[SUCCESS]', message);
}

function warn(message) {
  log(colors.yellow, '[WARN]', message);
}

function error(message) {
  log(colors.red, '[ERROR]', message);
}

async function runCommand(command, args = [], options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      stdio: 'inherit',
      cwd: options.cwd || rootDir,
      ...options
    });

    child.on('close', (code) => {
      if (code === 0) {
        resolve(code);
      } else {
        reject(new Error(`Command failed: ${command} ${args.join(' ')}`));
      }
    });

    child.on('error', reject);
  });
}

async function checkCommand(command) {
  try {
    await runCommand('which', [command], { stdio: 'ignore' });
    return true;
  } catch {
    try {
      await runCommand('where', [command], { stdio: 'ignore' });
      return true;
    } catch {
      return false;
    }
  }
}

async function ensureDir(dirPath) {
  try {
    await fs.mkdir(dirPath, { recursive: true });
  } catch (error) {
    if (error.code !== 'EEXIST') {
      throw error;
    }
  }
}

async function clean() {
  info('Cleaning previous builds...');

  const dirsToClean = [
    'dist',
    'wasm/pkg',
    'wasm/extractors',
    'wasm/planner'
  ];

  for (const dir of dirsToClean) {
    try {
      await fs.rm(resolve(rootDir, dir), { recursive: true, force: true });
      info(`Cleaned ${dir}`);
    } catch (error) {
      warn(`Could not clean ${dir}: ${error.message}`);
    }
  }

  success('Clean completed');
}

async function buildWasm() {
  info('Building WASM components...');

  // Check if wasm-pack is available
  if (!(await checkCommand('wasm-pack'))) {
    error('wasm-pack not found. Please install it with: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh');
    process.exit(1);
  }

  // Check if Rust is available
  if (!(await checkCommand('cargo'))) {
    error('Rust/Cargo not found. Please install Rust from https://rustup.rs/');
    process.exit(1);
  }

  // Ensure wasm directories exist
  await ensureDir(resolve(rootDir, 'wasm'));

  const wasmComponents = [
    { name: 'graph_reasoner', outDir: '../wasm/pkg' },
    { name: 'extractors', outDir: '../wasm/extractors' },
    { name: 'planner', outDir: '../wasm/planner' }
  ];

  for (const component of wasmComponents) {
    info(`Building WASM component: ${component.name}`);

    try {
      await runCommand('wasm-pack', [
        'build',
        '--target', 'nodejs',
        '--out-dir', component.outDir,
        component.name
      ]);
      success(`Built ${component.name}`);
    } catch (error) {
      error(`Failed to build ${component.name}: ${error.message}`);
      process.exit(1);
    }
  }

  success('WASM build completed');
}

async function buildTypeScript() {
  info('Building TypeScript...');

  // Check if TypeScript is available
  if (!(await checkCommand('tsc'))) {
    error('TypeScript compiler not found. Installing...');
    try {
      await runCommand('npm', ['install', '-g', 'typescript']);
    } catch (error) {
      error('Failed to install TypeScript globally. Please run: npm install -g typescript');
      process.exit(1);
    }
  }

  try {
    await runCommand('npx', ['tsc']);
    success('TypeScript build completed');
  } catch (error) {
    error(`TypeScript build failed: ${error.message}`);
    process.exit(1);
  }
}

async function copyAssets() {
  info('Copying assets...');

  const assetsToCopy = [
    { src: 'wasm', dest: 'dist/wasm' },
    { src: 'examples', dest: 'dist/examples' },
    { src: 'docs', dest: 'dist/docs' }
  ];

  for (const asset of assetsToCopy) {
    const srcPath = resolve(rootDir, asset.src);
    const destPath = resolve(rootDir, asset.dest);

    try {
      await fs.access(srcPath);
      await ensureDir(destPath);
      await fs.cp(srcPath, destPath, { recursive: true });
      info(`Copied ${asset.src} to ${asset.dest}`);
    } catch (error) {
      warn(`Could not copy ${asset.src}: ${error.message}`);
    }
  }

  success('Assets copied');
}

async function validateBuild() {
  info('Validating build...');

  const requiredFiles = [
    'dist/index.js',
    'dist/index.d.ts',
    'dist/cli/index.js',
    'dist/mcp/index.js'
  ];

  let allValid = true;

  for (const file of requiredFiles) {
    try {
      await fs.access(resolve(rootDir, file));
      info(`✓ ${file}`);
    } catch (error) {
      error(`✗ Missing: ${file}`);
      allValid = false;
    }
  }

  // Check WASM files
  const wasmFiles = [
    'wasm/pkg/package.json',
    'wasm/extractors/package.json',
    'wasm/planner/package.json'
  ];

  for (const file of wasmFiles) {
    try {
      await fs.access(resolve(rootDir, file));
      info(`✓ ${file}`);
    } catch (error) {
      warn(`⚠ WASM file missing: ${file}`);
    }
  }

  if (allValid) {
    success('Build validation passed');
  } else {
    error('Build validation failed');
    process.exit(1);
  }
}

async function generatePackageInfo() {
  info('Generating package info...');

  const packageJson = JSON.parse(
    await fs.readFile(resolve(rootDir, 'package.json'), 'utf8')
  );

  const buildInfo = {
    name: packageJson.name,
    version: packageJson.version,
    buildTime: new Date().toISOString(),
    components: {
      typescript: true,
      wasm: {
        graph_reasoner: true,
        extractors: true,
        planner: true
      }
    }
  };

  await fs.writeFile(
    resolve(rootDir, 'dist/build-info.json'),
    JSON.stringify(buildInfo, null, 2)
  );

  success('Package info generated');
}

async function main() {
  const startTime = Date.now();

  try {
    log(colors.cyan, '[BUILD]', 'Starting build process...');

    await clean();
    await buildWasm();
    await buildTypeScript();
    await copyAssets();
    await generatePackageInfo();
    await validateBuild();

    const duration = ((Date.now() - startTime) / 1000).toFixed(2);
    success(`Build completed successfully in ${duration}s`);

  } catch (error) {
    error(`Build failed: ${error.message}`);
    process.exit(1);
  }
}

// Handle command line arguments
const args = process.argv.slice(2);

if (args.includes('--help') || args.includes('-h')) {
  console.log(`
Usage: node scripts/build.js [options]

Options:
  --help, -h          Show this help message
  --clean-only        Only clean, don't build
  --wasm-only         Only build WASM components
  --ts-only           Only build TypeScript
  --skip-validation   Skip build validation

Examples:
  node scripts/build.js                    # Full build
  node scripts/build.js --clean-only       # Clean only
  node scripts/build.js --wasm-only        # WASM only
  node scripts/build.js --ts-only          # TypeScript only
`);
  process.exit(0);
}

if (args.includes('--clean-only')) {
  clean().then(() => success('Clean completed')).catch(console.error);
} else if (args.includes('--wasm-only')) {
  buildWasm().then(() => success('WASM build completed')).catch(console.error);
} else if (args.includes('--ts-only')) {
  buildTypeScript().then(() => success('TypeScript build completed')).catch(console.error);
} else {
  main().catch(console.error);
}