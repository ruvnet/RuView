#!/usr/bin/env node
/**
 * RuvLLM ESP32 CLI
 *
 * Cross-platform installation and flashing tool for RuvLLM on ESP32
 */

const { spawn, execSync } = require('child_process');
const fs = require('fs');
const path = require('path');
const os = require('os');

const VERSION = '0.3.0';
const SUPPORTED_TARGETS = ['esp32', 'esp32s2', 'esp32s3', 'esp32c3', 'esp32c6'];

// Colors for terminal output
const colors = {
    reset: '\x1b[0m',
    bright: '\x1b[1m',
    green: '\x1b[32m',
    yellow: '\x1b[33m',
    blue: '\x1b[34m',
    red: '\x1b[31m',
    cyan: '\x1b[36m'
};

function log(msg, color = 'reset') {
    console.log(`${colors[color]}${msg}${colors.reset}`);
}

function logStep(msg) {
    console.log(`${colors.cyan}▶${colors.reset} ${msg}`);
}

function logSuccess(msg) {
    console.log(`${colors.green}✓${colors.reset} ${msg}`);
}

function logError(msg) {
    console.error(`${colors.red}✗${colors.reset} ${msg}`);
}

function showHelp() {
    console.log(`
${colors.bright}RuvLLM ESP32 v${VERSION}${colors.reset}
Full-featured LLM inference engine for ESP32

${colors.yellow}USAGE:${colors.reset}
    npx ruvllm-esp32 <command> [options]

${colors.yellow}COMMANDS:${colors.reset}
    install          Install ESP32 toolchain (espup, espflash)
    build            Build the firmware
    flash [port]     Flash to ESP32 (auto-detect or specify port)
    monitor [port]   Monitor serial output
    config           Interactive configuration
    cluster          Setup multi-chip cluster
    info             Show system information

${colors.yellow}OPTIONS:${colors.reset}
    --target, -t     ESP32 variant: esp32, esp32s2, esp32s3, esp32c3, esp32c6
    --port, -p       Serial port (e.g., COM3, /dev/ttyUSB0)
    --release        Build in release mode
    --features       Cargo features: federation, full
    --help, -h       Show this help
    --version, -v    Show version

${colors.yellow}EXAMPLES:${colors.reset}
    npx ruvllm-esp32 install
    npx ruvllm-esp32 build --target esp32s3 --release
    npx ruvllm-esp32 flash --port COM6
    npx ruvllm-esp32 flash /dev/ttyUSB0
    npx ruvllm-esp32 cluster --chips 5

${colors.yellow}FEATURES:${colors.reset}
    - INT8/Binary quantized inference (~20KB RAM)
    - Product quantization (8-32x compression)
    - MicroLoRA on-device adaptation
    - HNSW vector search (1000+ vectors)
    - Semantic memory with RAG
    - Multi-chip federation (pipeline/tensor parallel)
    - Speculative decoding (2-4x speedup)
`);
}

function detectPlatform() {
    const platform = os.platform();
    const arch = os.arch();
    return { platform, arch };
}

function detectPort() {
    const { platform } = detectPlatform();

    try {
        if (platform === 'win32') {
            // Windows: Use PowerShell for better COM port detection
            try {
                const result = execSync(
                    'powershell -Command "[System.IO.Ports.SerialPort]::GetPortNames() | Sort-Object { [int]($_ -replace \'COM\', \'\') }"',
                    { encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] }
                );
                const ports = result.trim().split('\n').filter(p => p.match(/COM\d+/));
                if (ports.length > 0) {
                    return ports[0].trim();
                }
            } catch {
                // Fallback to wmic
                const result = execSync('wmic path Win32_SerialPort get DeviceID 2>nul', { encoding: 'utf8' });
                const ports = result.split('\n').filter(line => line.includes('COM')).map(line => line.trim());
                if (ports.length > 0) return ports[0];
            }
            return 'COM3';
        } else if (platform === 'darwin') {
            // macOS
            const files = fs.readdirSync('/dev').filter(f =>
                f.startsWith('cu.usbserial') ||
                f.startsWith('cu.SLAB') ||
                f.startsWith('cu.wchusbserial') ||
                f.startsWith('cu.usbmodem')
            );
            return files[0] ? `/dev/${files[0]}` : '/dev/cu.usbserial-0001';
        } else {
            // Linux
            const files = fs.readdirSync('/dev').filter(f => f.startsWith('ttyUSB') || f.startsWith('ttyACM'));
            return files[0] ? `/dev/${files[0]}` : '/dev/ttyUSB0';
        }
    } catch (e) {
        return platform === 'win32' ? 'COM3' : '/dev/ttyUSB0';
    }
}

function checkToolchain() {
    try {
        execSync('espup --version', { stdio: 'pipe' });
        return true;
    } catch {
        return false;
    }
}

async function installToolchain() {
    logStep('Installing ESP32 toolchain...');

    const { platform } = detectPlatform();

    try {
        if (platform === 'win32') {
            // Windows: Check if we have the PowerShell setup script
            const scriptsDir = path.join(__dirname, '..', 'scripts', 'windows');
            const setupScript = path.join(scriptsDir, 'setup.ps1');

            if (fs.existsSync(setupScript)) {
                logStep('Running Windows setup script...');
                execSync(`powershell -ExecutionPolicy Bypass -File "${setupScript}"`, { stdio: 'inherit' });
            } else {
                // Fallback: manual installation
                logStep('Installing espup...');

                // Download espup for Windows
                const espupUrl = 'https://github.com/esp-rs/espup/releases/latest/download/espup-x86_64-pc-windows-msvc.exe';
                const espupPath = path.join(os.tmpdir(), 'espup.exe');

                execSync(`powershell -Command "Invoke-WebRequest -Uri '${espupUrl}' -OutFile '${espupPath}'"`, { stdio: 'inherit' });

                logStep('Running espup install...');
                execSync(`"${espupPath}" install`, { stdio: 'inherit' });

                // Install espflash
                logStep('Installing espflash...');
                execSync('cargo install espflash ldproxy', { stdio: 'inherit' });
            }

            logSuccess('Toolchain installed successfully!');
            log('\nTo use the toolchain, run:', 'yellow');
            log('  . .\\scripts\\windows\\env.ps1', 'cyan');

        } else {
            // Linux/macOS
            logStep('Installing espup...');
            const arch = os.arch() === 'arm64' ? 'aarch64' : 'x86_64';
            const binary = platform === 'darwin'
                ? `espup-${arch}-apple-darwin`
                : `espup-${arch}-unknown-linux-gnu`;

            execSync(`curl -L https://github.com/esp-rs/espup/releases/latest/download/${binary} -o /tmp/espup && chmod +x /tmp/espup && /tmp/espup install`, { stdio: 'inherit' });

            // Install espflash
            logStep('Installing espflash...');
            execSync('cargo install espflash ldproxy', { stdio: 'inherit' });

            logSuccess('Toolchain installed successfully!');
            log('\nPlease restart your terminal or run:', 'yellow');
            log('  source $HOME/export-esp.sh', 'cyan');
        }

        return true;
    } catch (e) {
        logError(`Installation failed: ${e.message}`);
        return false;
    }
}

async function build(options = {}) {
    const target = options.target || 'esp32';
    const release = options.release !== false; // Default to release
    const features = options.features || '';
    const { platform } = detectPlatform();

    logStep(`Building for ${target}${release ? ' (release)' : ''}...`);

    const targetMap = {
        'esp32': 'xtensa-esp32-espidf',
        'esp32s2': 'xtensa-esp32s2-espidf',
        'esp32s3': 'xtensa-esp32s3-espidf',
        'esp32c3': 'riscv32imc-esp-espidf',
        'esp32c6': 'riscv32imac-esp-espidf'
    };

    const rustTarget = targetMap[target] || targetMap['esp32'];

    try {
        if (platform === 'win32') {
            // Windows: Use PowerShell build script if available
            const scriptsDir = path.join(__dirname, '..', 'scripts', 'windows');
            const buildScript = path.join(scriptsDir, 'build.ps1');

            if (fs.existsSync(buildScript)) {
                let psArgs = `-ExecutionPolicy Bypass -File "${buildScript}" -Target "${rustTarget}"`;
                if (release) psArgs += ' -Release';
                if (features) psArgs += ` -Features "${features}"`;

                execSync(`powershell ${psArgs}`, { stdio: 'inherit', cwd: process.cwd() });
            } else {
                // Fallback to direct cargo
                let cmd = `cargo build --target ${rustTarget}`;
                if (release) cmd += ' --release';
                if (features) cmd += ` --features ${features}`;
                execSync(cmd, { stdio: 'inherit', cwd: process.cwd() });
            }
        } else {
            // Linux/macOS
            let cmd = `cargo build --target ${rustTarget}`;
            if (release) cmd += ' --release';
            if (features) cmd += ` --features ${features}`;
            execSync(cmd, { stdio: 'inherit', cwd: process.cwd() });
        }

        logSuccess('Build completed!');
        return true;
    } catch (e) {
        logError(`Build failed: ${e.message}`);
        return false;
    }
}

async function flash(port, options = {}) {
    const actualPort = port || detectPort();
    const target = options.target || 'esp32';
    const { platform } = detectPlatform();

    logStep(`Flashing to ${actualPort}...`);

    const targetMap = {
        'esp32': 'xtensa-esp32-espidf',
        'esp32s2': 'xtensa-esp32s2-espidf',
        'esp32s3': 'xtensa-esp32s3-espidf',
        'esp32c3': 'riscv32imc-esp-espidf',
        'esp32c6': 'riscv32imac-esp-espidf'
    };
    const rustTarget = targetMap[target] || targetMap['esp32'];

    try {
        if (platform === 'win32') {
            // Windows: Use PowerShell flash script if available
            const scriptsDir = path.join(__dirname, '..', 'scripts', 'windows');
            const flashScript = path.join(scriptsDir, 'flash.ps1');

            if (fs.existsSync(flashScript)) {
                const psArgs = `-ExecutionPolicy Bypass -File "${flashScript}" -Port "${actualPort}" -Target "${rustTarget}"`;
                execSync(`powershell ${psArgs}`, { stdio: 'inherit', cwd: process.cwd() });
            } else {
                // Fallback
                const binary = `target\\${rustTarget}\\release\\ruvllm-esp32`;
                execSync(`espflash flash --monitor --port ${actualPort} ${binary}`, { stdio: 'inherit' });
            }
        } else {
            // Linux/macOS
            const binary = `target/${rustTarget}/release/ruvllm-esp32`;
            execSync(`espflash flash --monitor --port ${actualPort} ${binary}`, { stdio: 'inherit' });
        }

        logSuccess('Flash completed!');
        return true;
    } catch (e) {
        logError(`Flash failed: ${e.message}`);
        return false;
    }
}

async function monitor(port) {
    const actualPort = port || detectPort();
    logStep(`Monitoring ${actualPort}...`);

    try {
        execSync(`espflash monitor --port ${actualPort}`, { stdio: 'inherit' });
    } catch (e) {
        // Monitor exits normally with Ctrl+C
    }
}

function showInfo() {
    const { platform, arch } = detectPlatform();
    const hasToolchain = checkToolchain();

    console.log(`
${colors.bright}RuvLLM ESP32 System Information${colors.reset}
${'─'.repeat(40)}
Version:       ${VERSION}
Platform:      ${platform}
Architecture:  ${arch}
Toolchain:     ${hasToolchain ? `${colors.green}Installed${colors.reset}` : `${colors.red}Not installed${colors.reset}`}
Detected Port: ${detectPort()}

${colors.yellow}Supported Targets:${colors.reset}
  ${SUPPORTED_TARGETS.join(', ')}

${colors.yellow}Features:${colors.reset}
  - Binary quantization (32x compression)
  - Product quantization (8-32x)
  - Sparse attention patterns
  - MicroLoRA adaptation
  - HNSW vector index
  - Semantic memory
  - RAG retrieval
  - Anomaly detection
  - Pipeline parallelism
  - Tensor parallelism
  - Speculative decoding
`);
}

// Parse arguments
const args = process.argv.slice(2);
const command = args[0];

const options = {
    target: 'esp32',
    port: null,
    release: false,
    features: ''
};

for (let i = 1; i < args.length; i++) {
    const arg = args[i];
    if (arg === '--target' || arg === '-t') {
        options.target = args[++i];
    } else if (arg === '--port' || arg === '-p') {
        options.port = args[++i];
    } else if (arg === '--release') {
        options.release = true;
    } else if (arg === '--features') {
        options.features = args[++i];
    } else if (arg === '--help' || arg === '-h') {
        showHelp();
        process.exit(0);
    } else if (arg === '--version' || arg === '-v') {
        console.log(VERSION);
        process.exit(0);
    } else if (!arg.startsWith('-')) {
        // Positional argument (likely port)
        if (!options.port) options.port = arg;
    }
}

// Execute command
async function main() {
    switch (command) {
        case 'install':
            await installToolchain();
            break;
        case 'build':
            await build(options);
            break;
        case 'flash':
            await flash(options.port, options);
            break;
        case 'monitor':
            await monitor(options.port);
            break;
        case 'info':
            showInfo();
            break;
        case 'help':
        case undefined:
            showHelp();
            break;
        default:
            logError(`Unknown command: ${command}`);
            showHelp();
            process.exit(1);
    }
}

main().catch(e => {
    logError(e.message);
    process.exit(1);
});
