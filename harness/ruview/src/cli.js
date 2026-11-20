// SPDX-License-Identifier: MIT
// `npx ruview` — the RuView WiFi-sensing operator harness (minted via metaharness,
// hardened per ADR-182). Plain ESM, no build step: ships and runs as-is.
//
// The `ruview.*` tools (onboard/verify/claim-check/…) and local host adapters are
// pure Node and run with zero runtime dependencies. Modules load on demand so
// `--help`/`--version`/`skills` never pay for the tool registry (ADR-375).

import { fileURLToPath } from 'node:url';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { colorEnabled, renderResult, withProgress } from './cli-ui.js';

const NAME = 'ruview';
const ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const SKILLS_DIR = join(ROOT, 'skills');
const tools = () => import('./tools.js');

// Map friendly CLI verbs → registry tool names (underscore-canonical, ADR-263).
const VERB_TO_TOOL = {
  onboard: 'ruview_onboard',
  verify: 'ruview_verify',
  'claim-check': 'ruview_claim_check',
  calibrate: 'ruview_calibrate',
  monitor: 'ruview_node_monitor',
  flash: 'ruview_node_flash',
  guidance: 'ruview_guidance',
  spaces: 'ruview_spaces_list',
  kernel: 'ruview_kernel_selftest',
  'flash-plan': 'ruview_firmware_plan',
  ports: 'ruview_firmware_ports',
  train: 'ruview_train',
  'train-plan': 'ruview_train_plan',
  'train-gate': 'ruview_train_gate',
  devices: 'ruview_devices_scan',
  esp32: 'ruview_esp32_capture',
  mmwave: 'ruview_mmwave_read',
  lidar: 'ruview_lidar_read',
};

// Verbs whose kebab-case flags map 1:1 onto snake_case schema fields (ADR-369).
const SNAKE_VERBS = new Set(['flash', 'flash-plan', 'train', 'train-plan', 'train-gate', 'devices', 'esp32', 'mmwave', 'lidar']);
const NUMERIC_FLAGS = new Set(['baud', 'boot_log_seconds', 'samples', 'model_score', 'baseline_score', 'n_test', 'seconds', 'udp_port', 'max_packets', 'max_frames', 'node_id', 'analyze_max_frames', 'api_port', 'spectrum_bins', 'spectrum_frames']);
const BOOLEAN_FLAGS = new Set(['confirm', 'cuda', 'allow_unverified', 'analyze', 'spectrum']);
// Presentation-only flags never reach a tool schema.
const UI_FLAGS = new Set(['json', 'watch', 'interval']);

// Leave absent flag values uncoerced so the tool schema rejects them.
const numericFlag = (value) => typeof value === 'boolean' || (typeof value === 'string' && value.trim() === '') ? value : Number(value);

function toSchemaArgs(flags) {
  const out = {};
  for (const [key, value] of Object.entries(flags)) {
    const k = key.replaceAll('-', '_');
    if (UI_FLAGS.has(k)) continue;
    if (NUMERIC_FLAGS.has(k)) out[k] = numericFlag(value);
    else if (BOOLEAN_FLAGS.has(k)) out[k] = value === true || value === 'true';
    else if (k === 'train_subjects' || k === 'test_subjects') out[k] = String(value).split(',').filter(Boolean);
    else out[k] = value;
  }
  return out;
}

function pjson(o) { console.log(JSON.stringify(o, null, 2)); }

/** JSON for pipes and --json; the terminal UI for a human at a TTY. */
function emit(tool, res, flags) {
  if (flags.json === true || !process.stdout.isTTY) pjson(res);
  else console.log(renderResult(tool, res, { color: colorEnabled() }));
}

/** Tools that hold the terminal for a known window get a countdown (TTY only). */
function progressLabel(tool, args) {
  if (tool === 'ruview_esp32_capture') return [`Listening on UDP ${args.bind || '0.0.0.0'}:${args.udp_port || 5005}`, args.seconds ?? 10];
  if (tool === 'ruview_node_monitor') return [`Reading ${args.port} at ${args.baud || 115200} baud`, args.seconds ?? 12];
  if (tool === 'ruview_mmwave_read' || tool === 'ruview_lidar_read') return [`Reading ${args.host ? `ESPHome ${args.host}` : args.port || args.url || 'device'}`, args.seconds ?? 10];
  return [null, 0];
}

async function callTool(tool, args) {
  const { runTool } = await tools();
  const [label, seconds] = progressLabel(tool, args);
  return label ? withProgress(label, seconds, () => runTool(tool, args, { source: 'cli' })) : runTool(tool, args, { source: 'cli' });
}

/** `esp32 --watch`: repeated short captures, redrawn in place, with per-node trend lines. */
async function watchCapture(args, flags) {
  const { runTool } = await tools();
  const window = { ...args, seconds: args.seconds ?? 3 };
  const history = new Map();
  let stop = false;
  process.once('SIGINT', () => { stop = true; });
  const tty = process.stdout.isTTY && flags.json !== true;
  while (!stop) {
    const res = await runTool('ruview_esp32_capture', window, { source: 'cli' });
    const present = new Set((res.nodes || []).map(n => `${n.source}:${n.nodeId}`));
    for (const [key, values] of history) {
      if (!present.has(key)) history.set(key, [...values, null].slice(-30));
    }
    for (const n of res.nodes || []) {
      const key = `${n.source}:${n.nodeId}`;
      history.set(key, [...(history.get(key) || []), n.csiRateHz].slice(-30));
    }
    const terminalError = ['port_in_use', 'invalid_port', 'invalid_bind', 'invalid_arguments'].includes(res.reason);
    if (!tty) { console.log(JSON.stringify(res)); if (terminalError) return 1; continue; }
    process.stdout.write('\x1b[2J\x1b[H');
    console.log(renderResult('ruview_esp32_capture', res, { color: colorEnabled(), history }));
    console.log(`\n${new Date().toLocaleTimeString()} · ${window.seconds}s windows · Ctrl+C to stop`);
    if (terminalError) return 1;
  }
  return 0;
}

function listSkills() {
  if (!existsSync(SKILLS_DIR)) return [];
  return readdirSync(SKILLS_DIR).filter((f) => f.endsWith('.md')).map((f) => f.replace(/\.md$/, ''));
}

async function doctor(flags) {
  const [{ formatDoctor, runDoctor, DOCTOR_GROUPS }, { OPERATOR_DEPS }] = await Promise.all([import('./doctor.js'), tools()]);
  const groups = typeof flags.group === 'string' ? flags.group.split(',').map((g) => g.trim()).filter(Boolean) : undefined;
  const unknown = (groups || []).filter((g) => !DOCTOR_GROUPS.includes(g));
  if (unknown.length) { console.error(`doctor: unknown group(s) ${unknown.join(', ')}; valid: ${DOCTOR_GROUPS.join(', ')}`); return 2; }
  const report = await runDoctor({
    groups,
    port: typeof flags.port === 'string' ? flags.port : undefined,
    probe_port: flags.probe === true,
    sensing_url: typeof flags.url === 'string' ? flags.url : undefined,
  }, OPERATOR_DEPS);
  if (flags.json === true) pjson(report);
  else console.log(formatDoctor(report));
  return report.ok ? 0 : 1;
}

async function mcp(rest, flags, handler) {
  if (rest[0] !== undefined && rest[0] !== 'start') { console.error('Usage: ruview mcp start [--http [--host 127.0.0.1] [--port 8790] [--allow-origin URL]]'); return 2; }
  if (flags.http !== true) {
    const { startMcpServer } = await import('./mcp-server.js');
    startMcpServer(handler ? { handler } : {});
    return new Promise(() => {}); // run until stdin closes
  }
  const { startMcpHttp, DEFAULT_HTTP_PORT } = await import('./mcp-http.js');
  const host = typeof flags.host === 'string' ? flags.host : '127.0.0.1';
  const port = flags.port === undefined ? DEFAULT_HTTP_PORT : Number(flags.port);
  if (!Number.isInteger(port) || port < 0 || port > 65535) { console.error('mcp start: --port must be 0..65535'); return 2; }
  const allowOrigins = typeof flags['allow-origin'] === 'string' ? flags['allow-origin'].split(',').map((o) => o.trim()).filter(Boolean) : [];
  // The token is read from the environment, never from argv (process lists are visible to other users).
  const srv = await startMcpHttp({ host, port, token: process.env.RUVIEW_MCP_TOKEN, allowOrigins, ...(handler ? { handler } : {}) });
  const generated = !process.env.RUVIEW_MCP_TOKEN;
  process.stderr.write([
    `ruview MCP over HTTP on ${srv.url}${srv.loopback ? '' : '  (non-loopback bind: anyone who can reach this port and holds the token can call read tools)'}`,
    `  grants: ${srv.grants.join(', ') || 'none (set RUVIEW_MCP_GRANTS=device-access for hardware reads)'}`,
    `  token: ${generated ? `${srv.token}  (generated; set RUVIEW_MCP_TOKEN to keep it stable)` : 'from RUVIEW_MCP_TOKEN'}`,
    '  clients with headers: Authorization: Bearer <token>',
    '  ChatGPT (no custom headers): expose the port over HTTPS (e.g. a tunnel) and add the connector URL',
    `    https://<public-host>/mcp/${generated ? srv.token : '<token>'}`,
    '',
  ].join('\n'));
  const shutdown = () => srv.close().then(() => process.exit(0));
  process.once('SIGINT', shutdown);
  process.once('SIGTERM', shutdown);
  return new Promise(() => {});
}

function help() {
  console.log(`Usage: ${NAME} <command> [options]

Operator tools:
  onboard [--path docker-demo|repo-build|live-esp32]   pick a setup path
  verify [--repo <dir>]                                 run the deterministic proof (VERDICT: PASS)
  claim-check --text "..."  |  --file <path>            lint accuracy claims (the honesty guardrail)
  calibrate --step baseline|enroll|train-room|room-watch
  monitor --port COM8 [--seconds 12] [--baud 1500000]   assert CSI is flowing on a node (no reset on open)
  flash --port COM8 --variant s3-8mb [--confirm]        build+flash firmware (Windows/ESP-IDF)
  guidance [--topic homecore] [--query "Wasmtime"]      source-cited code/capability map
  spaces [--resource sites|...|alerts] [--limit 50]     page OAuth-bound Cognitum spatial resources
  kernel [--backend wasm|napi|auto] [--seconds 60]      SYNTHETIC self-test of @ruvnet/ruview-kernel (optional)

Firmware (ADR-370, cross-platform esptool):
  ports                                                  list serial ports (pyserial)
  flash-plan --port <p> --bundle <dir> [--variant s3-8mb|s3-4mb|c6|c5]   verify + print the plan
  flash --port <p> --bundle <dir> [--variant ...] [--baud 460800] --confirm
        [--boot-log-seconds 15] [--allow-unverified]     write flash, then capture boot evidence

Devices (ADR-373) — run on the machine the hardware is attached to:
  devices                                                classify USB serial devices (ESP32, Realtek, mmWave, RPLIDAR)
  esp32 [--udp-port 5005] [--seconds 10] [--bind 0.0.0.0]  receive + summarize ESP32 / Realtek RAC1 node UDP streams
        [--analyze [--node-id N] [--backend wasm|napi|auto]]  run live CSI through @ruvnet/ruview-kernel
        [--watch [--seconds 3]]                          live view: repeated windows with per-node trends
  mmwave --port <p> [--model auto|mr60bha2|ld2410] [--seconds 10]   60/24 GHz radar readout
  mmwave --source esphome --host <ip> [--api-port 6053]   radar kit running ESPHome (e.g. Seeed MR60BHA2 kit)
  lidar --source rplidar --port <p> [--baud 115200]      RPLIDAR scan summary
  lidar --source iphone --url ws://HOST:8787/ws/lidar    iPhone LiDAR relay (token: RUVIEW_LIDAR_TOKEN)

Remote hosts (ADR-374) — SSH, read-only tools only:
  hosts add --name pi --ssh user@pi.local [--port 22] [--version X.Y.Z]
  hosts list
  hosts run --host pi --tool ruview_devices_scan [--args-json '{"seconds":10}']
  call <tool> [--read-only] [--args-json '{...}']       generic schema-validated tool call

Training (ADR-371):
  train-plan [--mode pose-smoke|pose|room] [...]         resolve the command, run nothing
  train --mode pose-smoke|pose|room [--config f] [--data-dir d] [--checkpoint-dir d]
        [--enrollment f] [--output f] [--samples 64] [--cuda] --confirm
  train-gate --file report.json | --model-score .59 --baseline-score .50 --split chronological ...

Harness:
  doctor [--json] [--group a,b] [--port P [--probe]] [--url http://host:port]
                         structured diagnostics with fixes (ADR-372)
  skills                 list bundled skills
  skill <name>           print a skill playbook
  mod [--json]           where the ruview-live Claude Code mod is, and how to load it (ADR-377)
  mcp start              run the ruview.* MCP server (stdio)
  mcp start --http [--host 127.0.0.1] [--port 8790] [--allow-origin URL]
                         MCP over HTTP for ChatGPT and remote clients (ADR-375);
                         token: RUVIEW_MCP_TOKEN (generated if unset), grants: RUVIEW_MCP_GRANTS
  install --host <h>     project the harness config into the current repo
  agent run --host claude-code|codex --prompt "..." [--repo <dir>]
  brain search --query "..." | verify | propose
  --version | --help

Output: a terminal gets a formatted view; pipes and --json get JSON.
Hosts implemented and tested locally: claude-code (-p), codex (exec)`);
  return 0;
}

/** tiny flag parser: --k v / --k=v / --flag (boolean) */
function parseFlags(rest) {
  const f = {};
  for (let i = 0; i < rest.length; i++) {
    const a = rest[i];
    if (a.startsWith('--')) {
      const eq = a.indexOf('=');
      if (eq !== -1) { f[a.slice(2, eq)] = a.slice(eq + 1); }
      else if (i + 1 < rest.length && !rest[i + 1].startsWith('--')) { f[a.slice(2)] = rest[++i]; }
      else { f[a.slice(2)] = true; }
    }
  }
  return f;
}

/** Run the CLI. `opts.mcpHandler` lets an embedding package serve a merged MCP tool set. */
export async function run(args, opts = {}) {
  const cmd = args[0] ?? 'onboard';
  const rest = args.slice(1);
  const flags = parseFlags(rest);

  // Direct tool verbs.
  if (VERB_TO_TOOL[cmd]) {
    const tool = VERB_TO_TOOL[cmd];
    const toolArgs = { ...flags };
    for (const k of UI_FLAGS) delete toolArgs[k];
    if (cmd === 'claim-check') {
      if (flags.file) {
        toolArgs.text = readFileSync(flags.file, 'utf8');
        delete toolArgs.file;
      }
      // Fail closed (ADR-263 O1): an honesty gate must never PASS on no input.
      if (typeof toolArgs.text !== 'string' || toolArgs.text.trim().length === 0) {
        console.error('claim-check: no input — pass --text "..." or --file <path> (empty input is an error, not a PASS).');
        return 2;
      }
      const res = await callTool(tool, toolArgs);
      emit(tool, res, flags);
      return res.ok ? 0 : 1;
    }
    if (cmd === 'monitor' && flags.seconds) toolArgs.seconds = Number(flags.seconds);
    if (cmd === 'monitor' && flags.baud) toolArgs.baud = Number(flags.baud);
    if (cmd === 'guidance' && flags.limit) toolArgs.limit = Number(flags.limit);
    if (cmd === 'calibrate' && typeof flags.args === 'string') toolArgs.args = flags.args.split(',');
    if (cmd === 'kernel' && flags.seconds !== undefined) toolArgs.seconds = Number(flags.seconds);
    if (SNAKE_VERBS.has(cmd)) {
      let snake = toSchemaArgs(flags);
      if (cmd === 'train-gate' && typeof flags.file === 'string') {
        snake = JSON.parse(readFileSync(flags.file, 'utf8'));
      }
      if (cmd === 'esp32' && flags.watch === true) return watchCapture(snake, flags);
      const res = await callTool(tool, snake);
      emit(tool, res, flags);
      return res.ok ? 0 : 1;
    }
    if (cmd === 'spaces') {
      if (flags['credentials-path'] !== undefined) toolArgs.credentials_path = flags['credentials-path'];
      delete toolArgs['credentials-path'];
      if (flags.limit !== undefined) toolArgs.limit = Number(flags.limit);
    }
    const res = await callTool(tool, toolArgs);
    emit(tool, res, flags);
    return res.ok ? 0 : 1;
  }

  switch (cmd) {
    case 'doctor': return doctor(flags);
    case 'skills': console.log(listSkills().join('\n') || '(none)'); return 0;
    case 'skill': {
      const n = rest[0];
      const p = n && join(SKILLS_DIR, `${n}.md`);
      if (!p || !existsSync(p)) { console.error(`No skill "${n}". Try: ${listSkills().join(', ')}`); return 2; }
      console.log(readFileSync(p, 'utf8'));
      return 0;
    }
    case 'mcp': return mcp(rest, flags, opts.mcpHandler);
    case 'mod': {
      // The ruview-live Claude Code mod ships inside this package (ADR-377).
      const dir = join(ROOT, 'mod');
      const info = {
        ok: existsSync(join(dir, '.claude-plugin', 'plugin.json')),
        plugin: 'ruview-live',
        path: dir,
        run: `claude --plugin-dir "${dir}"`,
        marketplace: ['/plugin marketplace add ruvnet/RuView', '/plugin install ruview-live@ruview'],
        usage: ['/ruview            open or close the live pane', '/ruview refresh    one capture now, result in the status line', '/ruview off        close the pane'],
        configure: 'claude plugin configure ruview-live   (udpPort, radarHost, refreshSeconds, captureSeconds)',
        note: 'Mods are early access in Claude Code. If /ruview is missing, start Claude Code with CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1.',
      };
      if (flags.json === true || !process.stdout.isTTY) pjson(info);
      else console.log([`ruview-live mod: ${info.path}`, `  try it:   ${info.run}`, `  install:  ${info.marketplace.join('  then  ')}`, ...info.usage.map((u) => `  ${u}`), `  ${info.configure}`, `  ${info.note}`].join('\n'));
      return info.ok ? 0 : 1;
    }
    case 'agent': {
      if (rest[0] !== 'run') { console.error('Usage: ruview agent run --host claude-code|codex --prompt "..." [--repo <dir>]'); return 2; }
      const [{ findRepoRoot }, { getHost }] = await Promise.all([tools(), import('./hosts/index.js')]);
      const hostName = String(flags.host || 'codex');
      const prompt = String(flags.prompt || '');
      const repo = flags.repo ? resolve(flags.repo) : findRepoRoot();
      if (!repo) { console.error('agent run: trusted RuView repo not found; pass --repo <root>.'); return 2; }
      if (!prompt.trim()) { console.error('agent run: --prompt is required.'); return 2; }
      const allowWrite = flags['allow-write'] === true;
      if (allowWrite && flags.confirm !== true) { console.error('agent run: --allow-write also requires --confirm.'); return 2; }
      try {
        const result = await getHost(hostName).run({
          prompt, repoRoot: repo, trustedRoot: repo, allowWrite, confirm: flags.confirm === true,
        });
        pjson({ ok: true, host: hostName, mode: allowWrite ? 'workspace-write' : 'read-only', stdout: result.stdout, stderr: result.stderr });
        return 0;
      } catch (error) {
        pjson({ ok: false, host: hostName, error: error.message });
        return 1;
      }
    }
    case 'brain': {
      const { makeProposal, searchBrain, verifyBrain } = await import('./brain.js');
      const action = rest[0] || 'search';
      if (action === 'search') {
        const query = String(flags.query || '');
        if (!query.trim()) { console.error('brain search: --query is required.'); return 2; }
        pjson({ ok: true, results: searchBrain(query, { limit: flags.limit }) }); return 0;
      }
      if (action === 'verify') {
        const { findRepoRoot } = await tools();
        const repo = flags.repo ? resolve(flags.repo) : findRepoRoot();
        if (!repo) { console.error('brain verify: RuView repo not found.'); return 2; }
        const result = verifyBrain({ repo }); pjson(result); return result.ok ? 0 : 1;
      }
      if (action === 'propose') {
        const result = makeProposal(flags); pjson(result); return result.ok ? 0 : 1;
      }
      console.error('Usage: ruview brain search|verify|propose'); return 2;
    }
    case 'install': {
      const host = flags.host || 'claude-code';
      if (!['claude-code', 'codex'].includes(host)) {
        console.error(`Host "${host}" is not implemented. Supported: claude-code, codex.`);
        return 2;
      }
      try {
        const { getHost } = await import('./hosts/index.js');
        const adapter = getHost(host);
        console.log(`Projecting RuView harness for host "${host}" via ${adapter.name}.`);
        console.log('Add to your host config — MCP server command: npx -y ruview mcp start');
        console.log('Skills:', listSkills().join(', '));
        return 0;
      } catch {
        console.error(`Host adapter "${host}" is unavailable.`);
        return 1;
      }
    }
    case 'tools': { const { listTools } = await tools(); pjson(listTools()); return 0; }
    case 'call': {
      // Generic, schema-validated tool call (ADR-374). `--read-only` is what
      // remote hosts receive: it refuses anything but read-only tools.
      const { isRemoteReadOnlyTool, resolveToolName } = await tools();
      const name = resolveToolName(rest[0] || '');
      if (!name) { console.error('Usage: ruview call <tool> [--read-only] [--args-json \'{...}\']'); return 2; }
      if (flags['read-only'] === true && !isRemoteReadOnlyTool(name)) {
        pjson({ ok: false, reason: 'remote_tool_not_allowed', name }); return 1;
      }
      let toolArgs = {};
      if (typeof flags['args-json'] === 'string') {
        try { toolArgs = JSON.parse(flags['args-json']); } catch { pjson({ ok: false, reason: 'invalid_arguments', errors: ['--args-json is not valid JSON'] }); return 2; }
      }
      const res = await callTool(name, toolArgs);
      // Remote hosts parse this stream: `call` always prints JSON.
      pjson(res);
      return res.ok ? 0 : 1;
    }
    case 'hosts': {
      const action = rest[0] || 'list';
      if (action === 'list') { const res = await callTool('ruview_host_list', {}); pjson(res); return res.ok ? 0 : 1; }
      if (action === 'add') {
        const { saveHost } = await import('./remote.js');
        try {
          const out = saveHost({ name: flags.name, ssh: flags.ssh, port: flags.port ? Number(flags.port) : undefined, version: flags.version, description: flags.description });
          pjson({ ok: true, ...out, next: `Verify the host key once: ssh ${out.host.ssh} true` }); return 0;
        } catch (error) { pjson({ ok: false, reason: error.reason || 'invalid_host', detail: error.message }); return 2; }
      }
      if (action === 'run') {
        const { resolveToolName } = await tools();
        let toolArgs = {};
        if (typeof flags['args-json'] === 'string') {
          try { toolArgs = JSON.parse(flags['args-json']); } catch { console.error('--args-json is not valid JSON'); return 2; }
        }
        const tool = resolveToolName(String(flags.tool || '')) || String(flags.tool || '');
        const res = await callTool('ruview_host_run', { host: String(flags.host || ''), tool, args: toolArgs });
        pjson(res); return res.ok ? 0 : 1;
      }
      console.error('Usage: ruview hosts list | add --name pi --ssh user@pi.local [--port 22] [--version X.Y.Z] | run --host pi --tool ruview_devices_scan [--args-json {...}]');
      return 2;
    }
    case '--version': case '-v': {
      const pkg = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8'));
      console.log(pkg.version); return 0;
    }
    case '--help': case '-h': return help();
    default:
      console.error(`Unknown command: ${cmd}. Try \`${NAME} --help\`.`);
      return 2;
  }
}
