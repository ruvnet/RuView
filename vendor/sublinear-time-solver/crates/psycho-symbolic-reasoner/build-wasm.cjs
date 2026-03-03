#!/usr/bin/env node

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

// Configuration
const CRATES = ['graph_reasoner', 'extractors', 'planner'];
const BUILD_MODE = process.argv.includes('--dev') ? 'dev' : 'release';
const INSTALL_DEPS = process.argv.includes('--install');

console.log(`🚀 Building WASM modules in ${BUILD_MODE} mode...`);

// Function to run command with proper error handling
function runCommand(command, cwd = process.cwd()) {
    try {
        console.log(`📦 Running: ${command} in ${cwd}`);
        const output = execSync(command, {
            cwd,
            stdio: 'inherit',
            env: { ...process.env, RUST_LOG: 'info' }
        });
        return output;
    } catch (error) {
        console.error(`❌ Error running command: ${command}`);
        console.error(error.message);
        process.exit(1);
    }
}

// Check if wasm-pack is installed
function checkWasmPack() {
    try {
        execSync('wasm-pack --version', { stdio: 'pipe' });
        console.log('✅ wasm-pack is installed');
    } catch (error) {
        console.error('❌ wasm-pack is not installed. Please install it with:');
        console.error('curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh');
        process.exit(1);
    }
}

// Build a single crate
function buildCrate(crateName) {
    const cratePath = path.join(__dirname, crateName);

    if (!fs.existsSync(cratePath)) {
        console.error(`❌ Crate directory not found: ${cratePath}`);
        return false;
    }

    console.log(`\n🔨 Building ${crateName}...`);

    // Build command based on mode
    const buildCommand = BUILD_MODE === 'dev'
        ? 'wasm-pack build --target web --dev'
        : 'wasm-pack build --target web --release';

    runCommand(buildCommand, cratePath);

    // Create/update package.json for the WASM package
    const pkgPath = path.join(cratePath, 'pkg');
    if (fs.existsSync(pkgPath)) {
        updatePackageJson(crateName, pkgPath);
        console.log(`✅ ${crateName} built successfully`);
        return true;
    } else {
        console.error(`❌ Build failed for ${crateName} - pkg directory not found`);
        return false;
    }
}

// Update package.json with proper exports and metadata
function updatePackageJson(crateName, pkgPath) {
    const packageJsonPath = path.join(pkgPath, 'package.json');

    if (!fs.existsSync(packageJsonPath)) {
        console.error(`❌ package.json not found in ${pkgPath}`);
        return;
    }

    const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'));

    // Add proper exports and metadata
    packageJson.exports = {
        ".": {
            "import": "./index.js",
            "require": "./index_bg.js",
            "types": "./index.d.ts"
        },
        "./package.json": "./package.json"
    };

    packageJson.types = "index.d.ts";
    packageJson.repository = {
        "type": "git",
        "url": "https://github.com/your-org/psycho-symbolic-reasoner.git",
        "directory": `psycho-symbolic-reasoner/${crateName}`
    };

    packageJson.keywords = [
        "wasm",
        "webassembly",
        "rust",
        "ai",
        "reasoning",
        "psycho-symbolic",
        crateName.replace('_', '-')
    ];

    packageJson.author = "Psycho-Symbolic AI Team";
    packageJson.license = "MIT";
    packageJson.engines = {
        "node": ">=14.0.0"
    };

    fs.writeFileSync(packageJsonPath, JSON.stringify(packageJson, null, 2));
    console.log(`📝 Updated package.json for ${crateName}`);
}

// Create a unified package.json for all modules
function createUnifiedPackage() {
    const unifiedPath = path.join(__dirname, 'wasm-dist');

    if (!fs.existsSync(unifiedPath)) {
        fs.mkdirSync(unifiedPath, { recursive: true });
    }

    const unifiedPackage = {
        "name": "@psycho-symbolic/reasoner",
        "version": "0.1.0",
        "description": "Complete WASM bindings for psycho-symbolic reasoning system",
        "main": "index.js",
        "types": "index.d.ts",
        "exports": {
            ".": {
                "import": "./index.js",
                "require": "./index.js",
                "types": "./index.d.ts"
            },
            "./graph": "./graph_reasoner.js",
            "./extractors": "./extractors.js",
            "./planner": "./planner.js"
        },
        "scripts": {
            "test": "node test.js"
        },
        "keywords": [
            "wasm",
            "webassembly",
            "rust",
            "ai",
            "reasoning",
            "psycho-symbolic",
            "graph",
            "nlp",
            "planning"
        ],
        "author": "Psycho-Symbolic AI Team",
        "license": "MIT",
        "engines": {
            "node": ">=14.0.0"
        },
        "dependencies": {},
        "devDependencies": {
            "@types/node": "^18.0.0"
        }
    };

    fs.writeFileSync(
        path.join(unifiedPath, 'package.json'),
        JSON.stringify(unifiedPackage, null, 2)
    );
    console.log(`📦 Created unified package.json`);
}

// Main build process
function main() {
    console.log('🏗️  Starting WASM build process...\n');

    // Check prerequisites
    checkWasmPack();

    // Install Rust target if needed
    if (INSTALL_DEPS) {
        console.log('📥 Installing wasm32-unknown-unknown target...');
        runCommand('rustup target add wasm32-unknown-unknown');
    }

    let successCount = 0;

    // Build each crate
    for (const crate of CRATES) {
        if (buildCrate(crate)) {
            successCount++;
        }
    }

    // Create unified package
    if (successCount === CRATES.length) {
        createUnifiedPackage();
        console.log(`\n🎉 Successfully built all ${CRATES.length} WASM modules!`);
        console.log('\n📋 Next steps:');
        console.log('  1. Run: npm run test-wasm');
        console.log('  2. Run: npm run bundle');
        console.log('  3. Publish to npm if needed');
    } else {
        console.error(`\n❌ Build completed with errors. ${successCount}/${CRATES.length} crates built successfully.`);
        process.exit(1);
    }
}

// Run if called directly
if (require.main === module) {
    main();
}

module.exports = {
    buildCrate,
    createUnifiedPackage,
    CRATES,
    runCommand
};