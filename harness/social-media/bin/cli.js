#!/usr/bin/env node
// SPDX-License-Identifier: MIT

import { runDoctor } from '../src/doctor.js';
import { startHttpServer } from '../src/http-server.js';
import { startMcpServer } from '../src/mcp-server.js';
import { runTool } from '../src/tools.js';

const MAX_STDIN_BYTES = 1024 * 1024;

function print(value) {
  process.stdout.write(`${JSON.stringify(value, null, 2)}\n`);
}

async function readJsonStdin() {
  const chunks = [];
  let size = 0;
  for await (const chunk of process.stdin) {
    size += chunk.length;
    if (size > MAX_STDIN_BYTES) throw new Error('Input exceeds 1 MiB');
    chunks.push(chunk);
  }
  const text = Buffer.concat(chunks).toString('utf8').trim();
  if (!text) throw new Error('A JSON object is required on stdin');
  const value = JSON.parse(text);
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Input must be one JSON object');
  return value;
}

function help() {
  return {
    name: '@ruvnet/social-metaharness',
    authority: 'zero-credential read-only control plane',
    commands: [
      'doctor [--strict]',
      'platforms',
      'research baseline',
      'direction policy',
      'direction check < input.json',
      'action plan < input.json',
      'metrics normalize < input.json',
      'flywheel evaluate < input.json',
      'autopilot run < input.json',
      'audit verify < input.json',
      'serve',
      'mcp start'
    ],
  };
}

async function main(argv) {
  const [command, subcommand, ...rest] = argv;
  if (!command || command === 'help' || command === '--help' || command === '-h') return print(help());
  if (command === 'mcp' && subcommand === 'start' && rest.length === 0) return startMcpServer();
  if (command === 'serve' && subcommand === undefined) return startHttpServer();
  if (command === 'doctor') {
    if (argv.some((arg) => !['doctor', '--strict'].includes(arg))) throw new Error('Unknown doctor argument');
    const output = runDoctor();
    print(output);
    if (argv.includes('--strict') && !output.ok) process.exitCode = 1;
    return;
  }
  const noInput = new Map([
    ['platforms:', 'social_platforms'],
    ['research:baseline', 'social_research_baseline'],
    ['direction:policy', 'social_direction_policy'],
  ]);
  const noInputTool = noInput.get(`${command}:${subcommand || ''}`);
  if (noInputTool) return print(await runTool(noInputTool, {}));

  const inputTools = new Map([
    ['direction:check', 'social_direction_check'],
    ['action:plan', 'social_action_plan'],
    ['metrics:normalize', 'social_metrics_normalize'],
    ['flywheel:evaluate', 'social_flywheel_evaluate'],
    ['autopilot:run', 'social_autopilot_run'],
    ['audit:verify', 'social_audit_verify'],
  ]);
  const tool = inputTools.get(`${command}:${subcommand || ''}`);
  if (!tool || rest.length) throw new Error('Unknown command or unexpected arguments');
  print(await runTool(tool, await readJsonStdin()));
}

main(process.argv.slice(2)).catch((cause) => {
  process.stderr.write(`${cause instanceof Error ? cause.message : String(cause)}\n`);
  process.exitCode = 1;
});
