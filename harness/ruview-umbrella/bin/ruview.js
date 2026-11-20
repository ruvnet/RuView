#!/usr/bin/env node
// SPDX-License-Identifier: MIT
// `npx ruview` — one CLI for every RuView component (ADR-376).
//   ruview homecore <cmd>      → Homecore developer metaharness
//   ruview kernel <subcmd>     → WASM vitals kernel CLI (doctor|info|selftest|parity|synth|analyze|bench)
//   ruview mcp start [--http]  → one MCP server: harness + Homecore tools, ui:// console
//   ruview capabilities        → what this install can do
//   ruview <anything else>     → @ruvnet/ruview operator harness

import { realpathSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { argv } from 'node:process';
import { KERNEL_COMMANDS, VERSION, capabilities, componentVersions, wireKernel } from '../src/index.js';

const HELP_HEADER = `ruview ${VERSION} — RuView in one install

Components:
  ruview <command>            operator harness (@ruvnet/ruview) — devices, radar, firmware, training, doctor
  ruview homecore <command>   Homecore developer metaharness (ruview homecore --help)
  ruview kernel <subcommand>  vitals kernel: ${KERNEL_COMMANDS.join('|')} (ruview kernel --help)
  ruview mcp start [--http]   one MCP server: harness + Homecore tools, ChatGPT/MCP Apps console
  ruview capabilities         list every component, version and tool
`;

async function mcp(rest) {
  const { unifiedHandler } = await import('../src/mcp.js');
  const flags = new Set(rest);
  if (rest[0] !== undefined && rest[0] !== 'start') { console.error('Usage: ruview mcp start [--http [--host 127.0.0.1] [--port 8790] [--allow-origin URL]]'); return 2; }
  if (!flags.has('--http')) {
    const { startMcpServer } = await import('@ruvnet/ruview/mcp');
    startMcpServer({ handler: unifiedHandler, label: 'harness + homecore tools' });
    return new Promise(() => {});
  }
  // Reuse the harness CLI's HTTP startup (flags, banner, token handling), with the unified handler.
  const { run } = await import('@ruvnet/ruview/cli');
  return run(['mcp', ...rest], { mcpHandler: unifiedHandler });
}

export async function run(args) {
  const [cmd, ...rest] = args;
  await wireKernel();
  if (cmd === 'homecore') {
    const { run: homecore } = await import('homecore/cli');
    return homecore(rest);
  }
  if (cmd === 'kernel' && (KERNEL_COMMANDS.includes(rest[0]) || rest[0] === '--help' || rest[0] === '-h')) {
    const { run: kernel } = await import('@ruvnet/ruview-kernel/cli');
    return kernel(rest);
  }
  if (cmd === 'mcp') return mcp(rest);
  if (cmd === 'capabilities') {
    const res = await capabilities();
    console.log(JSON.stringify(res, null, 2));
    return 0;
  }
  if (cmd === '--version' || cmd === '-v') {
    const versions = await componentVersions();
    console.log(`ruview ${VERSION}`);
    for (const [name, v] of Object.entries(versions)) console.log(`  ${name} ${v ?? 'not installed'}`);
    return 0;
  }
  const { run: harness } = await import('@ruvnet/ruview/cli');
  if (cmd === '--help' || cmd === '-h') { console.log(HELP_HEADER); return harness(['--help']); }
  return harness(args);
}

const invokedDirectly = (() => {
  if (!argv[1]) return false;
  try {
    const a = realpathSync(argv[1]);
    const b = realpathSync(fileURLToPath(import.meta.url));
    return process.platform === 'win32' ? a.toLowerCase() === b.toLowerCase() : a === b;
  } catch { return false; }
})();
if (invokedDirectly) {
  run(argv.slice(2)).then((code) => process.exit(code)).catch((err) => { console.error(err); process.exit(1); });
}
