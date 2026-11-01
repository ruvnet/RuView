// SPDX-License-Identifier: MIT
// @ruvnet/ruview SDK (ADR-369): one programmatic entry point over the same
// policy-checked tool registry the CLI and MCP server use.
//
//   import { createRuView } from '@ruvnet/ruview/sdk';
//   const ruview = createRuView();
//   const report = await ruview.doctor();
//   const plan = await ruview.firmware.plan({ port: 'COM7', bundle: './bundle' });
//
// SDK calls run with `source: 'sdk'` (trusted in-process code, like the CLI).
// Mutating operations still require `confirm: true` in their arguments, and
// every result is the tool's structured JSON, including honest negatives.

import { listTools, OPERATOR_DEPS, runTool } from './tools.js';
import { DOCTOR_GROUPS, runDoctor } from './doctor.js';
import { FIRMWARE_VARIANTS } from './firmware.js';
import { SPLITS, TRAIN_MODES } from './training.js';
import { GUIDANCE_TOPICS } from './guidance.js';

export { DOCTOR_GROUPS, FIRMWARE_VARIANTS, GUIDANCE_TOPICS, SPLITS, TRAIN_MODES };

/** Error thrown by `strict` SDK instances when a tool returns ok:false. */
export class RuViewError extends Error {
  constructor(tool, result) {
    super(`${tool}: ${result.reason || 'failed'}${result.detail ? ` — ${result.detail}` : ''}`);
    this.name = 'RuViewError';
    this.tool = tool;
    this.reason = result.reason;
    this.result = result;
  }
}

export function createRuView({ strict = false, deps = OPERATOR_DEPS } = {}) {
  const checked = (tool, result) => {
    if (strict && result?.ok === false && !result.dryRun) throw new RuViewError(tool, result);
    return result;
  };
  const call = async (tool, args = {}) => checked(tool, await runTool(tool, args, { source: 'sdk' }));
  return Object.freeze({
    /** Raw access to any registry tool by canonical name. */
    call,
    tools: () => listTools(),

    /** Structured diagnostics; options: { groups, port, probe_port, sensing_url }. */
    doctor: async (options = {}) => checked('ruview_doctor', await runDoctor(options, deps)),

    onboard: (path) => call('ruview_onboard', path ? { path } : {}),
    guidance: (args = {}) => call('ruview_guidance', args),
    memorySearch: (query, limit) => call('ruview_memory_search', limit !== undefined ? { query, limit } : { query }),
    claimCheck: (text) => call('ruview_claim_check', { text }),
    verify: (repo) => call('ruview_verify', repo ? { repo } : {}),
    spaces: (args = {}) => call('ruview_spaces_list', args),

    firmware: Object.freeze({
      variants: FIRMWARE_VARIANTS,
      ports: () => call('ruview_firmware_ports'),
      plan: (args) => call('ruview_firmware_plan', args),
      /** Writes flash only when args.confirm === true; otherwise returns the plan. */
      flash: (args) => call('ruview_node_flash', args),
      monitor: (port, seconds, baud) => call('ruview_node_monitor', { port, ...(seconds ? { seconds } : {}), ...(baud ? { baud } : {}) }),
    }),

    training: Object.freeze({
      modes: TRAIN_MODES,
      plan: (args = {}) => call('ruview_train_plan', args),
      /** Executes only when args.confirm === true. */
      run: (args = {}) => call('ruview_train', args),
      gate: (report) => call('ruview_train_gate', report),
      calibrate: (args = {}) => call('ruview_calibrate', args),
    }),

    devices: Object.freeze({
      scan: () => call('ruview_devices_scan'),
      esp32: (args = {}) => call('ruview_esp32_capture', args),
      mmwave: (args) => call('ruview_mmwave_read', args),
      lidar: (args) => call('ruview_lidar_read', args),
    }),

    hosts: Object.freeze({
      list: () => call('ruview_host_list'),
      /** Run a read-only tool on a configured SSH host. */
      run: (host, tool, args = {}) => call('ruview_host_run', { host, tool, args }),
    }),

    kernel: Object.freeze({
      selfTest: (args = {}) => call('ruview_kernel_selftest', args),
      /** Load @ruvnet/ruview-kernel directly (optional package) for streaming analysis. */
      async load(options = {}) {
        const mod = await deps.importer('@ruvnet/ruview-kernel');
        return mod.loadKernel(options);
      },
    }),

    /** Start the stdio MCP server in this process. */
    async startMcpServer() {
      const { startMcpServer } = await import('./mcp-server.js');
      return startMcpServer();
    },
  });
}

export default createRuView;
