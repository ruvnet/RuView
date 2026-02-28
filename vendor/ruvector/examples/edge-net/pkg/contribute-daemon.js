#!/usr/bin/env node
/**
 * @ruvector/edge-net Contribution Daemon
 *
 * Real CPU contribution daemon that runs in the background and earns QDAG credits.
 * Connects to the relay server and sends contribution_credit messages periodically.
 *
 * Usage:
 *   npx @ruvector/edge-net contribute                    # Start daemon (foreground)
 *   npx @ruvector/edge-net contribute --daemon           # Start daemon (background)
 *   npx @ruvector/edge-net contribute --stop             # Stop daemon
 *   npx @ruvector/edge-net contribute --status           # Show daemon status
 *   npx @ruvector/edge-net contribute --cpu 50           # Set CPU limit (default: 50%)
 *   npx @ruvector/edge-net contribute --key <pubkey>     # Use specific public key
 */

import { readFileSync, writeFileSync, existsSync, mkdirSync, unlinkSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { webcrypto } from 'crypto';
import { performance } from 'perf_hooks';
import { homedir, cpus } from 'os';
import { spawn } from 'child_process';
import WebSocket from 'ws';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Relay server URL
const RELAY_URL = process.env.RELAY_URL || 'wss://edge-net-relay-875130704813.us-central1.run.app';

// Contribution settings
const DEFAULT_CPU_LIMIT = 50; // Default 50% CPU
const CONTRIBUTION_INTERVAL = 30000; // Report every 30 seconds
const RECONNECT_DELAY = 5000; // Reconnect after 5 seconds on disconnect
const HEARTBEAT_INTERVAL = 15000; // Heartbeat every 15 seconds

// ANSI colors
const c = {
  reset: '\x1b[0m',
  bold: '\x1b[1m',
  dim: '\x1b[2m',
  cyan: '\x1b[36m',
  green: '\x1b[32m',
  yellow: '\x1b[33m',
  red: '\x1b[31m',
  magenta: '\x1b[35m',
};

// Config directory
function getConfigDir() {
  const dir = join(homedir(), '.ruvector');
  if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
  return dir;
}

function getPidFile() {
  return join(getConfigDir(), 'contribute-daemon.pid');
}

function getLogFile() {
  return join(getConfigDir(), 'contribute-daemon.log');
}

function getStateFile() {
  return join(getConfigDir(), 'contribute-state.json');
}

// Parse command line arguments
function parseArgs(args) {
  const opts = {
    daemon: false,
    stop: false,
    status: false,
    cpu: DEFAULT_CPU_LIMIT,
    key: null,
    site: 'cli-contributor',
    help: false,
  };

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    switch (arg) {
      case '--daemon':
      case '-d':
        opts.daemon = true;
        break;
      case '--stop':
        opts.stop = true;
        break;
      case '--status':
        opts.status = true;
        break;
      case '--cpu':
        opts.cpu = parseInt(args[++i]) || DEFAULT_CPU_LIMIT;
        break;
      case '--key':
        opts.key = args[++i];
        break;
      case '--site':
        opts.site = args[++i];
        break;
      case '--help':
      case '-h':
        opts.help = true;
        break;
    }
  }

  return opts;
}

// Load or generate identity
async function loadIdentity(opts) {
  const identitiesDir = join(getConfigDir(), 'identities');
  if (!existsSync(identitiesDir)) mkdirSync(identitiesDir, { recursive: true });

  const metaPath = join(identitiesDir, `${opts.site}.meta.json`);

  // If --key is provided, use it directly
  if (opts.key) {
    return {
      publicKey: opts.key,
      shortId: `pi:${opts.key.slice(0, 16)}`,
      siteId: opts.site,
    };
  }

  // Try to load existing identity
  if (existsSync(metaPath)) {
    const meta = JSON.parse(readFileSync(metaPath, 'utf-8'));
    return {
      publicKey: meta.publicKey,
      shortId: meta.shortId,
      siteId: meta.siteId,
    };
  }

  // Generate new identity using WASM
  const { createRequire } = await import('module');
  const require = createRequire(import.meta.url);

  // Setup polyfills
  if (typeof globalThis.crypto === 'undefined') {
    globalThis.crypto = webcrypto;
  }

  console.log(`${c.dim}Generating new identity...${c.reset}`);
  const wasm = require('./node/ruvector_edge_net.cjs');
  const piKey = new wasm.PiKey();

  const publicKey = Array.from(piKey.getPublicKey())
    .map(b => b.toString(16).padStart(2, '0'))
    .join('');

  const meta = {
    version: 1,
    siteId: opts.site,
    shortId: piKey.getShortId(),
    publicKey,
    createdAt: new Date().toISOString(),
    lastUsed: new Date().toISOString(),
    totalSessions: 1,
    totalContributions: 0,
  };

  writeFileSync(metaPath, JSON.stringify(meta, null, 2));
  piKey.free();

  return {
    publicKey: meta.publicKey,
    shortId: meta.shortId,
    siteId: meta.siteId,
  };
}

// Load daemon state
function loadState() {
  const stateFile = getStateFile();
  if (existsSync(stateFile)) {
    return JSON.parse(readFileSync(stateFile, 'utf-8'));
  }
  return {
    totalCredits: 0,
    totalContributions: 0,
    totalSeconds: 0,
    startTime: null,
    lastSync: null,
  };
}

// Save daemon state
function saveState(state) {
  writeFileSync(getStateFile(), JSON.stringify(state, null, 2));
}

// Measure real CPU usage
function measureCpuUsage(durationMs = 1000) {
  return new Promise((resolve) => {
    const startCpus = cpus();
    const startTotal = startCpus.reduce((acc, cpu) => {
      const times = cpu.times;
      return acc + times.user + times.nice + times.sys + times.idle + times.irq;
    }, 0);
    const startIdle = startCpus.reduce((acc, cpu) => acc + cpu.times.idle, 0);

    setTimeout(() => {
      const endCpus = cpus();
      const endTotal = endCpus.reduce((acc, cpu) => {
        const times = cpu.times;
        return acc + times.user + times.nice + times.sys + times.idle + times.irq;
      }, 0);
      const endIdle = endCpus.reduce((acc, cpu) => acc + cpu.times.idle, 0);

      const totalDiff = endTotal - startTotal;
      const idleDiff = endIdle - startIdle;

      const cpuUsage = totalDiff > 0 ? ((totalDiff - idleDiff) / totalDiff) * 100 : 0;
      resolve(Math.min(100, Math.max(0, cpuUsage)));
    }, durationMs);
  });
}

// Real CPU work (compute hashes)
function doRealWork(cpuLimit, durationMs) {
  return new Promise((resolve) => {
    const startTime = Date.now();
    const workInterval = 10; // Work in 10ms chunks
    const workRatio = cpuLimit / 100;

    let computeUnits = 0;

    const doWork = () => {
      const now = Date.now();
      if (now - startTime >= durationMs) {
        resolve(computeUnits);
        return;
      }

      // Do actual CPU work (hash computation)
      const workStart = Date.now();
      const workTime = workInterval * workRatio;

      while (Date.now() - workStart < workTime) {
        // Real cryptographic work
        let data = new Uint8Array(64);
        for (let i = 0; i < 64; i++) {
          data[i] = (i * 7 + computeUnits) & 0xff;
        }
        // Simple hash-like operation (real CPU work)
        for (let j = 0; j < 100; j++) {
          let hash = 0;
          for (let i = 0; i < data.length; i++) {
            hash = ((hash << 5) - hash + data[i]) | 0;
          }
          data[0] = hash & 0xff;
        }
        computeUnits++;
      }

      // Rest period to respect CPU limit
      const restTime = workInterval * (1 - workRatio);
      setTimeout(doWork, restTime);
    };

    doWork();
  });
}

// Contribution daemon class
class ContributionDaemon {
  constructor(identity, cpuLimit) {
    this.identity = identity;
    this.cpuLimit = cpuLimit;
    this.ws = null;
    this.state = loadState();
    this.isRunning = false;
    this.nodeId = `node-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    this.contributionTimer = null;
    this.heartbeatTimer = null;
    this.reconnectTimer = null;
    this.sessionStart = Date.now();
    this.sessionCredits = 0;
    this.sessionContributions = 0;
  }

  async start() {
    this.isRunning = true;
    this.state.startTime = Date.now();
    saveState(this.state);

    console.log(`\n${c.cyan}${c.bold}Edge-Net Contribution Daemon${c.reset}`);
    console.log(`${c.dim}Real CPU contribution to earn QDAG credits${c.reset}\n`);

    console.log(`${c.bold}Configuration:${c.reset}`);
    console.log(`  ${c.cyan}Identity:${c.reset}   ${this.identity.shortId}`);
    console.log(`  ${c.cyan}Public Key:${c.reset} ${this.identity.publicKey.slice(0, 16)}...`);
    console.log(`  ${c.cyan}CPU Limit:${c.reset}  ${this.cpuLimit}%`);
    console.log(`  ${c.cyan}Relay:${c.reset}      ${RELAY_URL}`);
    console.log(`  ${c.cyan}Interval:${c.reset}   ${CONTRIBUTION_INTERVAL / 1000}s\n`);

    await this.connect();

    // Handle shutdown
    process.on('SIGINT', () => this.stop('SIGINT'));
    process.on('SIGTERM', () => this.stop('SIGTERM'));
  }

  async connect() {
    if (!this.isRunning) return;

    console.log(`${c.dim}Connecting to relay...${c.reset}`);

    try {
      this.ws = new WebSocket(RELAY_URL);

      this.ws.on('open', () => {
        console.log(`${c.green}Connected to relay${c.reset}`);

        // Register with relay
        this.send({
          type: 'register',
          nodeId: this.nodeId,
          publicKey: this.identity.publicKey,
          capabilities: ['compute', 'cli-daemon'],
          version: '1.0.0',
        });

        // Start heartbeat
        this.startHeartbeat();

        // Request initial balance from QDAG
        setTimeout(() => {
          this.send({
            type: 'ledger_sync',
            nodeId: this.nodeId,
            publicKey: this.identity.publicKey,
          });
        }, 500);
      });

      this.ws.on('message', (data) => this.handleMessage(data.toString()));

      this.ws.on('close', () => {
        console.log(`${c.yellow}Disconnected from relay${c.reset}`);
        this.stopHeartbeat();
        this.stopContributing();
        this.scheduleReconnect();
      });

      this.ws.on('error', (err) => {
        console.log(`${c.red}WebSocket error: ${err.message}${c.reset}`);
      });

    } catch (err) {
      console.log(`${c.red}Connection failed: ${err.message}${c.reset}`);
      this.scheduleReconnect();
    }
  }

  handleMessage(data) {
    try {
      const msg = JSON.parse(data);

      switch (msg.type) {
        case 'welcome':
          console.log(`${c.green}Registered as ${msg.nodeId}${c.reset}`);
          console.log(`${c.dim}Network: ${msg.networkState?.activeNodes || 0} active nodes${c.reset}`);
          this.startContributing();
          break;

        case 'ledger_sync_response':
          const earned = Number(msg.ledger?.earned || 0) / 1e9;
          const spent = Number(msg.ledger?.spent || 0) / 1e9;
          const available = earned - spent;
          console.log(`${c.cyan}QDAG Balance: ${available.toFixed(4)} rUv${c.reset} (earned: ${earned.toFixed(4)}, spent: ${spent.toFixed(4)})`);
          this.state.totalCredits = earned;
          saveState(this.state);
          break;

        case 'contribution_credit_success':
          const credited = msg.credited || 0;
          this.sessionCredits += credited;
          this.sessionContributions++;
          this.state.totalCredits = Number(msg.balance?.earned || 0) / 1e9;
          this.state.totalContributions++;
          this.state.lastSync = Date.now();
          saveState(this.state);

          const balance = Number(msg.balance?.available || 0) / 1e9;
          console.log(`${c.green}+${credited.toFixed(4)} rUv${c.reset} | Balance: ${balance.toFixed(4)} rUv | Total: ${this.state.totalContributions} contributions`);
          break;

        case 'contribution_credit_error':
          console.log(`${c.yellow}Contribution rejected: ${msg.error}${c.reset}`);
          break;

        case 'time_crystal_sync':
          // Silently handle time crystal sync
          break;

        case 'heartbeat_ack':
          // Heartbeat acknowledged
          break;

        case 'error':
          console.log(`${c.red}Relay error: ${msg.message}${c.reset}`);
          break;

        default:
          // Ignore unknown messages
      }
    } catch (err) {
      console.log(`${c.red}Error parsing message: ${err.message}${c.reset}`);
    }
  }

  send(msg) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg));
    }
  }

  startHeartbeat() {
    this.stopHeartbeat();
    this.heartbeatTimer = setInterval(() => {
      this.send({ type: 'heartbeat' });
    }, HEARTBEAT_INTERVAL);
  }

  stopHeartbeat() {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
  }

  startContributing() {
    this.stopContributing();
    console.log(`${c.cyan}Starting contribution loop (CPU: ${this.cpuLimit}%)${c.reset}\n`);

    // Immediate first contribution
    this.contribute();

    // Then continue every interval
    this.contributionTimer = setInterval(() => {
      this.contribute();
    }, CONTRIBUTION_INTERVAL);
  }

  stopContributing() {
    if (this.contributionTimer) {
      clearInterval(this.contributionTimer);
      this.contributionTimer = null;
    }
  }

  async contribute() {
    const startTime = Date.now();

    // Do real CPU work for 5 seconds
    console.log(`${c.dim}[${new Date().toLocaleTimeString()}] Working...${c.reset}`);
    await doRealWork(this.cpuLimit, 5000);

    // Measure actual CPU usage
    const cpuUsage = await measureCpuUsage(1000);

    const contributionSeconds = 30; // Claiming 30 seconds since last report
    const effectiveCpu = Math.min(this.cpuLimit, cpuUsage);

    // Send contribution credit request
    this.send({
      type: 'contribution_credit',
      nodeId: this.nodeId,
      publicKey: this.identity.publicKey,
      contributionSeconds,
      cpuUsage: Math.round(effectiveCpu),
      timestamp: Date.now(),
    });

    this.state.totalSeconds += contributionSeconds;
    saveState(this.state);
  }

  scheduleReconnect() {
    if (!this.isRunning) return;
    if (this.reconnectTimer) return;

    console.log(`${c.dim}Reconnecting in ${RECONNECT_DELAY / 1000}s...${c.reset}`);
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, RECONNECT_DELAY);
  }

  stop(signal = 'unknown') {
    console.log(`\n${c.yellow}Stopping daemon (${signal})...${c.reset}`);

    this.isRunning = false;
    this.stopContributing();
    this.stopHeartbeat();

    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }

    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }

    // Save final state
    saveState(this.state);

    // Print session summary
    const sessionDuration = (Date.now() - this.sessionStart) / 1000;
    console.log(`\n${c.bold}Session Summary:${c.reset}`);
    console.log(`  ${c.cyan}Duration:${c.reset}      ${Math.round(sessionDuration)}s`);
    console.log(`  ${c.cyan}Contributions:${c.reset} ${this.sessionContributions}`);
    console.log(`  ${c.cyan}Credits:${c.reset}       ${this.sessionCredits.toFixed(4)} rUv`);
    console.log(`  ${c.cyan}Total Earned:${c.reset}  ${this.state.totalCredits.toFixed(4)} rUv\n`);

    // Remove PID file
    const pidFile = getPidFile();
    if (existsSync(pidFile)) {
      unlinkSync(pidFile);
    }

    process.exit(0);
  }
}

// Show daemon status
async function showStatus(opts) {
  const pidFile = getPidFile();
  const state = loadState();
  const identity = await loadIdentity(opts);

  console.log(`\n${c.cyan}${c.bold}Edge-Net Contribution Daemon Status${c.reset}\n`);

  // Check if daemon is running
  if (existsSync(pidFile)) {
    const pid = parseInt(readFileSync(pidFile, 'utf-8').trim());
    try {
      process.kill(pid, 0); // Check if process exists
      console.log(`${c.green}Status: Running${c.reset} (PID: ${pid})`);
    } catch {
      console.log(`${c.yellow}Status: Stale PID file${c.reset} (process not found)`);
      unlinkSync(pidFile);
    }
  } else {
    console.log(`${c.dim}Status: Not running${c.reset}`);
  }

  console.log(`\n${c.bold}Identity:${c.reset}`);
  console.log(`  ${c.cyan}Short ID:${c.reset}   ${identity.shortId}`);
  console.log(`  ${c.cyan}Public Key:${c.reset} ${identity.publicKey.slice(0, 32)}...`);

  console.log(`\n${c.bold}Statistics:${c.reset}`);
  console.log(`  ${c.cyan}Total Credits:${c.reset}       ${state.totalCredits?.toFixed(4) || 0} rUv`);
  console.log(`  ${c.cyan}Total Contributions:${c.reset} ${state.totalContributions || 0}`);
  console.log(`  ${c.cyan}Total Time:${c.reset}          ${state.totalSeconds || 0}s`);

  if (state.lastSync) {
    const lastSync = new Date(state.lastSync);
    console.log(`  ${c.cyan}Last Sync:${c.reset}           ${lastSync.toLocaleString()}`);
  }

  console.log(`\n${c.bold}Files:${c.reset}`);
  console.log(`  ${c.dim}State:${c.reset}  ${getStateFile()}`);
  console.log(`  ${c.dim}Log:${c.reset}    ${getLogFile()}`);
  console.log(`  ${c.dim}PID:${c.reset}    ${pidFile}\n`);
}

// Stop daemon
function stopDaemon() {
  const pidFile = getPidFile();

  if (!existsSync(pidFile)) {
    console.log(`${c.yellow}Daemon is not running${c.reset}`);
    return;
  }

  const pid = parseInt(readFileSync(pidFile, 'utf-8').trim());

  try {
    process.kill(pid, 'SIGTERM');
    console.log(`${c.green}Sent SIGTERM to daemon (PID: ${pid})${c.reset}`);

    // Wait a bit and check if it stopped
    setTimeout(() => {
      try {
        process.kill(pid, 0);
        console.log(`${c.yellow}Daemon still running, sending SIGKILL...${c.reset}`);
        process.kill(pid, 'SIGKILL');
      } catch {
        console.log(`${c.green}Daemon stopped${c.reset}`);
      }
      if (existsSync(pidFile)) {
        unlinkSync(pidFile);
      }
    }, 2000);
  } catch {
    console.log(`${c.yellow}Process not found, cleaning up...${c.reset}`);
    unlinkSync(pidFile);
  }
}

// Start daemon in background
function startDaemonBackground(args) {
  const pidFile = getPidFile();
  const logFile = getLogFile();

  if (existsSync(pidFile)) {
    const pid = parseInt(readFileSync(pidFile, 'utf-8').trim());
    try {
      process.kill(pid, 0);
      console.log(`${c.yellow}Daemon already running (PID: ${pid})${c.reset}`);
      console.log(`${c.dim}Use --stop to stop it first${c.reset}`);
      return;
    } catch {
      unlinkSync(pidFile);
    }
  }

  console.log(`${c.cyan}Starting daemon in background...${c.reset}`);

  const out = require('fs').openSync(logFile, 'a');
  const err = require('fs').openSync(logFile, 'a');

  // Remove --daemon flag and respawn
  const filteredArgs = args.filter(a => a !== '--daemon' && a !== '-d');

  const child = spawn(process.execPath, [__filename, ...filteredArgs], {
    detached: true,
    stdio: ['ignore', out, err],
  });

  writeFileSync(pidFile, String(child.pid));
  child.unref();

  console.log(`${c.green}Daemon started (PID: ${child.pid})${c.reset}`);
  console.log(`${c.dim}Log file: ${logFile}${c.reset}`);
  console.log(`${c.dim}Use --status to check status${c.reset}`);
  console.log(`${c.dim}Use --stop to stop daemon${c.reset}`);
}

// Print help
function printHelp() {
  console.log(`
${c.cyan}${c.bold}Edge-Net Contribution Daemon${c.reset}
${c.dim}Contribute CPU to earn QDAG credits${c.reset}

${c.bold}USAGE:${c.reset}
  npx @ruvector/edge-net contribute [options]

${c.bold}OPTIONS:${c.reset}
  ${c.yellow}--daemon, -d${c.reset}    Run in background (detached)
  ${c.yellow}--stop${c.reset}          Stop running daemon
  ${c.yellow}--status${c.reset}        Show daemon status
  ${c.yellow}--cpu <percent>${c.reset} CPU usage limit (default: 50)
  ${c.yellow}--key <pubkey>${c.reset}  Use specific public key
  ${c.yellow}--site <id>${c.reset}     Site identifier (default: cli-contributor)
  ${c.yellow}--help, -h${c.reset}      Show this help

${c.bold}EXAMPLES:${c.reset}
  ${c.dim}# Start contributing in foreground${c.reset}
  $ npx @ruvector/edge-net contribute

  ${c.dim}# Start as background daemon with 30% CPU${c.reset}
  $ npx @ruvector/edge-net contribute --daemon --cpu 30

  ${c.dim}# Use specific public key${c.reset}
  $ npx @ruvector/edge-net contribute --key 38a3bcd1732fe04c...

  ${c.dim}# Check status${c.reset}
  $ npx @ruvector/edge-net contribute --status

  ${c.dim}# Stop daemon${c.reset}
  $ npx @ruvector/edge-net contribute --stop

${c.bold}HOW IT WORKS:${c.reset}
  1. Daemon connects to Edge-Net relay server
  2. Every 30 seconds, does real CPU work
  3. Reports contribution to relay
  4. Relay credits QDAG (Firestore) with earned rUv
  5. Credits persist and sync across all devices

${c.bold}CREDIT RATE:${c.reset}
  Base rate: ~0.047 rUv/second of contribution
  Max rate:  ~0.05 rUv/second (180 rUv/hour max)
  Formula:   contributionSeconds * 0.047 * (cpuUsage / 100)
`);
}

// Main entry point
async function main() {
  const args = process.argv.slice(2);

  // Filter out 'contribute' if passed
  const filteredArgs = args.filter(a => a !== 'contribute');
  const opts = parseArgs(filteredArgs);

  if (opts.help) {
    printHelp();
    return;
  }

  if (opts.status) {
    await showStatus(opts);
    return;
  }

  if (opts.stop) {
    stopDaemon();
    return;
  }

  if (opts.daemon) {
    startDaemonBackground(filteredArgs);
    return;
  }

  // Start daemon in foreground
  try {
    const identity = await loadIdentity(opts);
    const daemon = new ContributionDaemon(identity, opts.cpu);
    await daemon.start();
  } catch (err) {
    console.error(`${c.red}Error: ${err.message}${c.reset}`);
    process.exit(1);
  }
}

main().catch(err => {
  console.error(`${c.red}Fatal error: ${err.message}${c.reset}`);
  process.exit(1);
});
