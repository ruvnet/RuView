#!/usr/bin/env node
/**
 * Post-install script for ruvllm-esp32
 * Downloads platform-specific binaries and checks prerequisites
 */

const os = require('os');
const path = require('path');
const fs = require('fs');

const platform = os.platform();
const arch = os.arch();

console.log('\n🔧 RuvLLM ESP32 Post-Install Setup\n');
console.log(`Platform: ${platform}/${arch}`);

// Check for Rust
try {
    require('child_process').execSync('rustc --version', { stdio: 'pipe' });
    console.log('✓ Rust is installed');
} catch {
    console.log('⚠ Rust not found. Install from https://rustup.rs');
}

// Check for cargo
try {
    require('child_process').execSync('cargo --version', { stdio: 'pipe' });
    console.log('✓ Cargo is installed');
} catch {
    console.log('⚠ Cargo not found. Install Rust from https://rustup.rs');
}

console.log('\n📦 Installation complete!');
console.log('Run: npx ruvllm-esp32 install    to setup ESP32 toolchain');
console.log('Run: npx ruvllm-esp32 --help     for all commands\n');
