import test from 'node:test';
import assert from 'node:assert/strict';
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  NodeConfigError, firmwareSource, nodeStatus, normalizeNodeConfig, parseNodeYaml, passwordFile, planNodes,
  provisionArgs, readRegistry, resolveNode, resolveTargetIp, applyBoard,
} from '../src/node-config.js';

const EXAMPLE = fileURLToPath(new URL('../../../firmware/esp32-csi-node/nodes.example.yaml', import.meta.url));
const NETS = { en0: [{ family: 'IPv4', internal: false, address: '192.168.1.20' }] };
const MAC = 'aa:bb:cc:dd:ee:01';

// Never read the operator's real ~/.ruview registry from tests.
const NO_REGISTRY = join(tmpdir(), `ruview-node-test-no-registry-${process.pid}.jsonl`);
const base = (extra = {}) => ({
  wifi: { ssid: 'Lab-2G', password_file: '/tmp/none' },
  registry: NO_REGISTRY,
  nodes: [{ mac: MAC, node_id: 1, zone: 'living-room', tdm_slot: 0 }, { mac: 'aa:bb:cc:dd:ee:02', node_id: 2, tdm_slot: 1 }],
  ...extra,
});
const cfg = (extra) => normalizeNodeConfig(base(extra));
const problems = (doc) => { try { normalizeNodeConfig(doc); return []; } catch (e) { assert.ok(e instanceof NodeConfigError); return e.errors || [e.message]; } };

// ─── YAML subset ───────────────────────────────────────────────────────────

test('YAML subset: maps, lists of maps, flow lists, scalars, comments', () => {
  const doc = parseNodeYaml(`
# comment
wifi:
  ssid: "Lab # 2G"          # hash inside quotes is data
  password_file: ~/.ruview/wifi-pass
steps: [device, flash, validate]
validate:
  min_pps: 20
  max_loss: 0.25
auto_assign: false
nodes:
  - mac: "AA:BB:CC:DD:EE:01"
    node_id: 1
    zone: 'living room'
  - mac: aa:bb:cc:dd:ee:02
    node_id: 2
empty:
`);
  assert.equal(doc.wifi.ssid, 'Lab # 2G');
  assert.deepEqual(doc.steps, ['device', 'flash', 'validate']);
  assert.equal(doc.validate.max_loss, 0.25);
  assert.equal(doc.auto_assign, false);
  assert.equal(doc.nodes.length, 2);
  assert.deepEqual(doc.nodes[0], { mac: 'AA:BB:CC:DD:EE:01', node_id: 1, zone: 'living room' });
  assert.equal(doc.nodes[1].mac, 'aa:bb:cc:dd:ee:02');
  assert.equal(doc.empty, null);
});

test('YAML subset fails closed on unsupported or ambiguous syntax', () => {
  const bad = {
    tabs: 'wifi:\n\tssid: x',
    anchor: 'a: &x 1',
    alias: 'a: *x',
    tag: 'a: !!str 1',
    block_scalar: 'a: |\n  text',
    flow_map: 'a: {b: 1}',
    duplicate: 'a: 1\na: 2',
    proto: '__proto__: 1',
    bad_indent: 'a:\n  b: 1\n    c: 2',
    unterminated: 'a: "x',
    top_list: '- a\n- b',
  };
  for (const [name, text] of Object.entries(bad)) assert.throws(() => parseNodeYaml(text), NodeConfigError, name);
});

test('the shipped nodes.example.yaml parses and validates', () => {
  const config = normalizeNodeConfig(parseNodeYaml(readFileSync(EXAMPLE, 'utf8')));
  assert.ok(config.nodes.length >= 1);
  assert.equal(config.firmware.source, 'auto');
});

// ─── config validation ─────────────────────────────────────────────────────

test('config: defaults are filled and paths expand ~', () => {
  const c = normalizeNodeConfig({ wifi: { ssid: 'x', password_file: '~/p' }, nodes: [] }, { home: '/home/u' });
  assert.equal(c.wifi.password_file, '/home/u/p');
  assert.deepEqual(c.aggregator, { ip: 'auto', port: 5005 });
  assert.equal(c.firmware.variant, 's3-8mb');
  assert.deepEqual(c.validate, { wait_secs: 60, udp_secs: 15, min_pps: 20, max_loss: null });
  assert.equal(c.auto_assign, true);
});

test('config: inline passwords, duplicates, bad MACs and unknown keys are all reported', () => {
  const errs = problems({
    wifi: { ssid: 'x', password: 'hunter2' },
    color: 'blue',
    firmware: { erase: true, variant: 's2' },
    nodes: [{ mac: MAC, node_id: 1, tdm_slot: 0 }, { mac: MAC.toUpperCase(), node_id: 1, tdm_slot: 0 }, { mac: 'nope', node_id: 300, flavor: 1 }],
  });
  const text = errs.join('\n');
  for (const want of ['wifi.password', 'color: unknown key', 'firmware.erase', 'firmware.variant', 'nodes[1].mac: duplicates', 'nodes[1].node_id: duplicates', 'nodes[1].tdm_slot: duplicates', 'nodes[2].mac', 'nodes[2].node_id', 'nodes[2].flavor']) {
    assert.ok(text.includes(want), `missing "${want}" in:\n${text}`);
  }
  assert.ok(!text.includes('hunter2'), 'secret value must never appear in errors');
});

test('config: provisioning values are range-checked', () => {
  const text = problems(base({ defaults: { edge_tier: 3, channel: 15, hop_channels: [], zone: 'x;rm -rf /' } })).join('\n');
  for (const k of ['defaults.edge_tier', 'defaults.channel', 'defaults.hop_channels', 'defaults.zone']) assert.ok(text.includes(k), k);
});

// ─── node resolution + provisioning argv ───────────────────────────────────

test('resolveNode: config first, then registry, then next free id', () => {
  const c = cfg({ defaults: { edge_tier: 2 } });
  const listed = resolveNode(c, MAC.toUpperCase());
  assert.equal(listed.source, 'config');
  assert.deepEqual([listed.node.node_id, listed.node.tdm_slot, listed.node.tdm_total, listed.node.edge_tier], [1, 0, 2, 2]);
  const prior = resolveNode(c, '11:22:33:44:55:66', [{ mac: '11:22:33:44:55:66', node_id: 7 }]);
  assert.deepEqual([prior.source, prior.node.node_id], ['registry', 7]);
  const fresh = resolveNode(c, '11:22:33:44:55:77', [{ mac: '11:22:33:44:55:66', node_id: 3 }]);
  assert.deepEqual([fresh.source, fresh.node.node_id], ['auto-assigned', 4]);
  assert.equal(fresh.node.tdm_total, undefined, 'no tdm_total without a slot');
  assert.throws(() => resolveNode(cfg({ auto_assign: false }), '11:22:33:44:55:77'), /not in the node config/);
});

test('provisionArgs carries paths, never secret values', () => {
  const c = cfg({ ota: { psk_file: '/keys/ota' }, defaults: { hop_channels: [1, 6, 11] } });
  const { node } = resolveNode(c, MAC);
  const argv = provisionArgs(c, node, { port: '/dev/cu.usbmodem1', targetIp: '192.168.1.20', passwordFile: '/run/pw' });
  const at = (flag) => argv[argv.indexOf(flag) + 1];
  assert.equal(at('--password-file'), '/run/pw');
  assert.equal(at('--ota-psk-file'), '/keys/ota');
  assert.equal(at('--hop-channels'), '1,6,11');
  assert.equal(at('--tdm-total'), '2');
  assert.ok(!argv.includes('--password'));
  assert.throws(() => provisionArgs(c, node, { port: 'COM7;x', targetIp: '1.2.3.4', passwordFile: '/p' }), /port must look like/);
});

test('passwordFile: path only, owner-only regular files; env sources are refused', () => {
  assert.ok(problems({ wifi: { ssid: 'x', password_env: 'RV_PW' } }).join('\n').includes('credentials never pass through the harness'));
  const dir = mkdtempSync(join(tmpdir(), 'rv-pw-'));
  try {
    const f = join(dir, 'pw'); writeFileSync(f, 'x'); chmodSync(f, 0o644);
    const loose = normalizeNodeConfig({ wifi: { ssid: 'x', password_file: f } });
    if (process.platform !== 'win32') assert.throws(() => passwordFile(loose), /owner-only/);
    chmodSync(f, 0o600);
    assert.equal(passwordFile(loose), f);
    assert.throws(() => passwordFile(normalizeNodeConfig({ wifi: { ssid: 'x' } })), /password_file/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('resolveTargetIp picks a private LAN address or the configured one', () => {
  assert.equal(resolveTargetIp(cfg(), { lo: [{ family: 'IPv4', internal: true, address: '127.0.0.1' }], ...NETS }).ip, '192.168.1.20');
  assert.equal(resolveTargetIp(cfg({ aggregator: { ip: '10.0.0.5' } }), {}).ip, '10.0.0.5');
  assert.throws(() => resolveTargetIp(cfg(), { en0: [{ family: 'IPv4', internal: false, address: '8.8.8.8' }] }), /no private IPv4/);
});

// ─── firmware source ───────────────────────────────────────────────────────

function fakeRepo({ target = 'esp32s3', staleSource = false } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'rv-repo-'));
  const fw = join(root, 'firmware', 'esp32-csi-node');
  const b = join(fw, 'build');
  mkdirSync(join(b, 'bootloader'), { recursive: true });
  mkdirSync(join(b, 'partition_table'), { recursive: true });
  mkdirSync(join(fw, 'main'), { recursive: true });
  writeFileSync(join(fw, 'main', 'main.c'), 'int x;');
  writeFileSync(join(fw, 'provision.py'), '# fake');
  writeFileSync(join(fw, 'sdkconfig'), `CONFIG_IDF_TARGET="${target}"\n`);
  const old = new Date(Date.now() - 60_000);
  utimesSync(join(fw, 'main', 'main.c'), old, old);
  for (const [f, body] of [['bootloader/bootloader.bin', 'B'], ['partition_table/partition-table.bin', 'P'], ['ota_data_initial.bin', 'O'], ['esp32-csi-node.bin', 'A']]) writeFileSync(join(b, f), body);
  if (staleSource) { const later = new Date(Date.now() + 60_000); utimesSync(join(fw, 'main', 'main.c'), later, later); }
  return root;
}

test('firmwareSource: reuse local build, refuse stale auto, wrong target, and build', () => {
  const root = fakeRepo();
  const stale = fakeRepo({ staleSource: true });
  const wrong = fakeRepo({ target: 'esp32c6' });
  try {
    assert.equal(firmwareSource(cfg(), root).kind, 'local-build');
    assert.throws(() => firmwareSource(cfg(), stale), /changed after the last build/);
    assert.equal(firmwareSource(cfg({ firmware: { source: 'reuse' } }), stale).stale, true);
    assert.throws(() => firmwareSource(cfg(), wrong), /not built for esp32s3/);
    assert.throws(() => firmwareSource(cfg({ firmware: { source: 'build' } }), root), /does not build firmware/);
    assert.throws(() => firmwareSource(cfg(), null), /RuView checkout/);
  } finally { for (const d of [root, stale, wrong]) rmSync(d, { recursive: true, force: true }); }
});

// ─── flows with fake hardware ──────────────────────────────────────────────

function fakeDeps({ repoRoot, macOut = `MAC: ${MAC}`, provisionOk = true, rate = 30, loss = 0.01, silent = false } = {}) {
  const calls = [];
  return {
    calls,
    repoRoot,
    nets: NETS,
    now: () => new Date('2026-11-12T00:00:00Z'),
    python: () => '/usr/bin/python3',
    exec: async (cmd, args) => {
      calls.push(args);
      if (args.includes('read_mac')) return { ok: true, stdout: macOut, stderr: '' };
      if (args.includes('chip_id')) return { ok: true, stdout: 'Chip type: ESP32-S3 (QFN56)', stderr: '' };
      if (args.includes('write_flash')) return { ok: true, stdout: 'Hash of data verified.', stderr: '' };
      if (args[0] === '-c') return { ok: true, stdout: 'I (1) main: esp32-csi-node v0.8.15\nI (2) csi_collector: CSI cb #1\n', stderr: '' };
      if (String(args[0]).endsWith('provision.py')) return { ok: provisionOk, stdout: provisionOk ? 'ok' : '', stderr: provisionOk ? '' : 'NVS write failed' };
      return { ok: false, stdout: '', stderr: 'unexpected' };
    },
    capture: async () => (silent
      ? { ok: false, reason: 'no_packets', nodes: [] }
      : { ok: true, packets: 400, listen: '0.0.0.0:5005', seconds: 15, nodes: [{ source: 'esp32', nodeId: 1, address: '192.168.1.40', packets: { csi: 450 }, csiRateHz: rate, csiLossFraction: loss, rssiMean: -50, csi: { freqMhz: 2462 } }] }),
    fetch: async () => ({ ok: true, json: async () => ({ version: '0.8.15', ota_state: 'valid' }) }),
  };
}

function withSecrets(extra = {}) {
  const dir = mkdtempSync(join(tmpdir(), 'rv-sec-'));
  const pw = join(dir, 'pw'); writeFileSync(pw, 'pw'); chmodSync(pw, 0o600);
  const reg = join(dir, 'devices.jsonl');
  return { dir, reg, config: cfg({ wifi: { ssid: 'Lab-2G', password_file: pw }, registry: reg, ...extra }) };
}

test('apply without --confirm reads the MAC, plans, and writes nothing', async () => {
  const repo = fakeRepo(); const s = withSecrets();
  try {
    const deps = fakeDeps({ repoRoot: repo });
    const res = await applyBoard(s.config, { port: '/dev/cu.usbmodem1' }, deps);
    assert.equal(res.ok, true); assert.equal(res.dryRun, true);
    assert.deepEqual([res.mac, res.node_id, res.node_source], [MAC, 1, 'config']);
    assert.ok(res.plan.provision.includes('--password-file'));
    assert.ok(!deps.calls.some((a) => a.includes('write_flash') || String(a[0]).endsWith('provision.py')), 'no writes in a plan');
    assert.equal(existsSync(s.reg), false, 'no registry writes in a plan');
  } finally { rmSync(repo, { recursive: true, force: true }); rmSync(s.dir, { recursive: true, force: true }); }
});

test('apply --confirm: flash → provision → validate, recorded in the registry without secrets', async () => {
  const repo = fakeRepo(); const s = withSecrets();
  try {
    const deps = fakeDeps({ repoRoot: repo });
    const res = await applyBoard(s.config, { port: '/dev/cu.usbmodem1', confirm: true }, deps);
    assert.equal(res.ok, true, JSON.stringify(res));
    assert.deepEqual(Object.keys(res.stages), ['flash', 'wifi', 'validate']);
    assert.equal(res.stages.flash.firmwareVersion, '0.8.15');
    assert.equal(res.stages.validate.address, '192.168.1.40');
    const events = readRegistry(s.reg).map((r) => r.event);
    assert.deepEqual(events, ['chip_probed', 'flashed', 'provisioned', 'validated']);
    assert.ok(!readFileSync(s.reg, 'utf8').includes('"pw"'));
  } finally { rmSync(repo, { recursive: true, force: true }); rmSync(s.dir, { recursive: true, force: true }); }
});

test('firmware-only skips provisioning; failures stop the flow with a reason', async () => {
  const repo = fakeRepo(); const s = withSecrets({ validate: { max_loss: 0.2 } });
  try {
    const upd = await applyBoard(s.config, { port: '/dev/cu.usbmodem1', flow: 'firmware', confirm: true }, fakeDeps({ repoRoot: repo }));
    assert.deepEqual(Object.keys(upd.stages), ['flash', 'validate']);

    const lossy = await applyBoard(s.config, { port: '/dev/cu.usbmodem1', flow: 'firmware', confirm: true }, fakeDeps({ repoRoot: repo, loss: 0.37 }));
    assert.equal(lossy.ok, false); assert.equal(lossy.reason, 'below_threshold'); assert.match(lossy.detail, /loss 37\.0%/);

    const nvs = await applyBoard(s.config, { port: '/dev/cu.usbmodem1', confirm: true }, fakeDeps({ repoRoot: repo, provisionOk: false }));
    assert.equal(nvs.reason, 'provision_failed');

    const nomac = await applyBoard(s.config, { port: '/dev/cu.usbmodem1', confirm: true }, fakeDeps({ repoRoot: repo, macOut: 'Failed to connect' }));
    assert.equal(nomac.reason, 'mac_probe_failed');
  } finally { rmSync(repo, { recursive: true, force: true }); rmSync(s.dir, { recursive: true, force: true }); }
});

test('status joins live CSI, HTTP and config; flags offline and unlisted nodes', async () => {
  const s = withSecrets();
  try {
    const deps = fakeDeps();
    deps.capture = async () => ({ ok: true, packets: 10, listen: '0.0.0.0:5005', seconds: 15, nodes: [
      { source: 'esp32', nodeId: 1, address: '192.168.1.40', packets: { csi: 450 }, csiRateHz: 30, csiLossFraction: 0.01, rssiMean: -50 },
      { source: 'esp32', nodeId: 9, address: '8.8.8.8', packets: { csi: 30 }, csiRateHz: 2, csiLossFraction: 0, rssiMean: -80 },
    ] });
    const fetched = [];
    deps.fetch = async (url) => { fetched.push(url); return { ok: true, json: async () => ({ version: '0.8.15', ota_state: 'valid' }) }; };
    const res = await nodeStatus(s.config, deps);
    const by = Object.fromEntries(res.nodes.map((n) => [n.node_id, n]));
    assert.equal(by[1].health, 'healthy'); assert.equal(by[1].firmware, '0.8.15');
    assert.equal(by[2].health, 'offline');
    assert.equal(by[9].health, 'unlisted');
    assert.deepEqual(fetched, ['http://192.168.1.40:8032/ota/status'], 'never probes a public address');
    assert.equal(res.ok, false);
    writeFileSync(s.reg, `${JSON.stringify({ event: 'node_assigned', mac: '11:22:33:44:55:09', node_id: 9 })}\n`);
    const again = Object.fromEntries((await nodeStatus(s.config, deps)).nodes.map((n) => [n.node_id, n]));
    assert.deepEqual([again[9].mac, again[9].source], ['11:22:33:44:55:09', 'registry']);
    assert.notEqual(again[9].health, 'unlisted', 'auto-assigned nodes are judged, not reported as unlisted');
  } finally { rmSync(s.dir, { recursive: true, force: true }); }
});

test('plan resolves every node and warns when OTA is not configured', () => {
  const repo = fakeRepo();
  try {
    const res = planNodes({ ...cfg(), path: '/x/nodes.yaml' }, { repoRoot: repo, nets: NETS });
    assert.equal(res.ok, true);
    assert.equal(res.target, '192.168.1.20:5005');
    assert.deepEqual(res.nodes.map((n) => [n.node_id, n.tdm]), [[1, '0/2'], [2, '1/2']]);
    assert.ok(res.warnings.some((w) => w.includes('ota.psk_file')));
  } finally { rmSync(repo, { recursive: true, force: true }); }
});

// Review regressions: identity binding, assignment collisions and retained inventory.
test('interactive confirmation rejects a replacement board before writes or registry changes', async () => {
  const repo = fakeRepo(); const s = withSecrets({ steps: ['flash', 'wifi'] });
  try {
    const plan = await applyBoard(s.config, { port: '/dev/cu.usbmodem1' }, fakeDeps({ repoRoot: repo }));
    const deps = fakeDeps({ repoRoot: repo, macOut: 'MAC: 11:22:33:44:55:66' });
    const result = await applyBoard(s.config, { port: plan.port, confirm: true,
      expectedIdentity: { mac: plan.mac, nodeId: plan.node_id } }, deps);
    assert.equal(result.reason, 'confirmation_changed');
    assert.equal(deps.calls.length, 1, 'only the identity probe may run');
    assert.equal(existsSync(s.reg), false, 'replacement cannot reserve a node ID');
  } finally { rmSync(repo, { recursive: true, force: true }); rmSync(s.dir, { recursive: true, force: true }); }
});

test('interactive confirmation rejects a changed assignment for the same board', async () => {
  const repo = fakeRepo(); const s = withSecrets();
  try {
    const deps = fakeDeps({ repoRoot: repo });
    const result = await applyBoard(s.config, { port: '/dev/cu.usbmodem1', confirm: true,
      expectedIdentity: { mac: MAC, nodeId: 7 } }, deps);
    assert.equal(result.reason, 'confirmation_changed');
    assert.equal(deps.calls.length, 1);
    assert.equal(existsSync(s.reg), false);
  } finally { rmSync(repo, { recursive: true, force: true }); rmSync(s.dir, { recursive: true, force: true }); }
});

test('interactive confirmation proceeds for the same board and node assignment', async () => {
  const repo = fakeRepo(); const s = withSecrets();
  try {
    const result = await applyBoard(s.config, { port: '/dev/cu.usbmodem1', confirm: true,
      expectedIdentity: { mac: MAC, nodeId: 1 } }, fakeDeps({ repoRoot: repo }));
    assert.equal(result.ok, true);
  } finally { rmSync(repo, { recursive: true, force: true }); rmSync(s.dir, { recursive: true, force: true }); }
});

test('registry restoration refuses an ID now configured for a different MAC', () => {
  const other = '11:22:33:44:55:66';
  assert.throws(() => resolveNode(cfg(), other, [{ event: 'node_assigned', mac: other, node_id: 1 }]),
    error => error.reason === 'node_id_conflict');
});

test('explicit config also refuses an ID retained by another unlisted board', () => {
  const other = '11:22:33:44:55:66';
  assert.throws(() => resolveNode(cfg(), MAC, [{ event: 'node_assigned', mac: other, node_id: 1 }]),
    error => error.reason === 'node_id_conflict');
});

test('latest registry assignments detect duplicate IDs between unlisted boards', () => {
  const a = '11:22:33:44:55:66', b = '11:22:33:44:55:77';
  assert.throws(() => resolveNode(cfg(), a, [{ mac: a, node_id: 7 }, { mac: b, node_id: 7 }]),
    error => error.reason === 'node_id_conflict');
});

test('explicit reassignment supersedes a historical registry ID for the same MAC', () => {
  const other = '11:22:33:44:55:66';
  const registry = [{ mac: MAC, node_id: 7 }, { mac: other, node_id: 7 }];
  assert.equal(resolveNode(cfg(), other, registry).node.node_id, 7);
});

test('status retains offline auto-assigned nodes and observes their return', async () => {
  const s = withSecrets({ nodes: [] });
  try {
    writeFileSync(s.reg, JSON.stringify({ event: 'node_assigned', mac: MAC, node_id: 1 }) + '\n');
    const offline = await nodeStatus(s.config, fakeDeps({ silent: true }));
    assert.equal(offline.ok, false);
    assert.equal(offline.nodes.length, 1);
    assert.deepEqual([offline.nodes[0].node_id, offline.nodes[0].mac, offline.nodes[0].health], [1, MAC, 'offline']);
    assert.match(offline.summary, /^0\/1 known nodes healthy/);
    const online = await nodeStatus(s.config, fakeDeps());
    assert.equal(online.nodes[0].health, 'healthy');
    assert.equal(online.ok, true);
  } finally { rmSync(s.dir, { recursive: true, force: true }); }
});

test('malformed registry assignments cannot restore invalid IDs', () => {
  const other = '11:22:33:44:55:66';
  for (const bad of [null, { mac: other, node_id: null }, { mac: other, node_id: true },
    { mac: other, node_id: -1 }, { mac: other, node_id: 256 }]) {
    const result = resolveNode(cfg(), other, [bad]);
    assert.equal(result.source, 'auto-assigned');
    assert.equal(result.node.node_id, 3);
  }
});

test('status reports conflicting board assignments instead of a healthy source identity', async () => {
  const s = withSecrets();
  try {
    writeFileSync(s.reg, JSON.stringify({ event: 'node_assigned', mac: '11:22:33:44:55:66', node_id: 1 }) + '\n');
    const result = await nodeStatus(s.config, fakeDeps());
    const node = result.nodes.find(row => row.node_id === 1);
    assert.equal(node.health, 'id-conflict');
    assert.equal(node.mac, null);
    assert.equal(result.ok, false);
  } finally { rmSync(s.dir, { recursive: true, force: true }); }
});

test('status uses an explicit reassignment without retaining the same MAC under its old ID', async () => {
  const s = withSecrets();
  try {
    writeFileSync(s.reg, JSON.stringify({ event: 'node_assigned', mac: MAC, node_id: 7 }) + '\n');
    const result = await nodeStatus(s.config, fakeDeps({ silent: true }));
    assert.deepEqual(result.nodes.map(row => row.node_id), [1, 2]);
  } finally { rmSync(s.dir, { recursive: true, force: true }); }
});

// Plan/apply parity and retired boards.
test('plan reads the registry and reports the conflict apply would refuse', () => {
  const repo = fakeRepo(); const s = withSecrets();
  try {
    const old = '11:22:33:44:55:66';
    writeFileSync(s.reg, `${JSON.stringify({ event: 'node_assigned', mac: old, node_id: 1 })}\n`);
    const res = planNodes({ ...s.config, path: '/x/nodes.yaml' }, { repoRoot: repo, nets: NETS });
    assert.equal(res.ok, false, 'plan must not pass when apply would refuse');
    assert.equal(res.conflicts.length, 1);
    assert.equal(res.conflicts[0].reason, 'node_id_conflict');
    assert.match(res.conflicts[0].detail, /retired:/, 'the error points at the way out');
    assert.equal(res.nodes.find((n) => n.node_id === 1).error, 'node_id_conflict');
  } finally { rmSync(repo, { recursive: true, force: true }); rmSync(s.dir, { recursive: true, force: true }); }
});

test('retired: releases a dead board\'s retained ID for its replacement', () => {
  const old = '11:22:33:44:55:66';
  const registry = [{ event: 'node_assigned', mac: old, node_id: 1 }];
  assert.throws(() => resolveNode(cfg(), MAC, registry), (e) => e.reason === 'node_id_conflict');
  const freed = resolveNode(cfg({ retired: [old.toUpperCase()] }), MAC, registry);
  assert.deepEqual([freed.source, freed.node.node_id], ['config', 1]);
  // auto-assign may reuse the released ID too
  const fresh = resolveNode(normalizeNodeConfig({ wifi: { ssid: 'x' }, retired: [old] }), '11:22:33:44:55:77', registry);
  assert.deepEqual([fresh.source, fresh.node.node_id], ['auto-assigned', 1]);
});

test('a retired board plugged back in is refused, not silently re-used', () => {
  const old = '11:22:33:44:55:66';
  assert.throws(() => resolveNode(cfg({ retired: [old] }), old, [{ mac: old, node_id: 9 }]), (e) => e.reason === 'node_retired');
});

test('retired: is validated and cannot overlap active nodes', () => {
  const text = problems(base({ retired: ['nope', MAC] })).join('\n');
  assert.ok(text.includes('retired[0]: must be a MAC'), text);
  assert.ok(text.includes('retired[1]') && text.includes('also listed under nodes'), text);
  assert.ok(problems(base({ retired: 'aa:bb:cc:dd:ee:09' })).join('\n').includes('retired: must be a list'));
});

test('status drops retired boards from the known inventory', async () => {
  const s = withSecrets({ nodes: [], retired: [MAC] });
  try {
    writeFileSync(s.reg, `${JSON.stringify({ event: 'node_assigned', mac: MAC, node_id: 1 })}\n`);
    const res = await nodeStatus(s.config, fakeDeps({ silent: true }));
    assert.deepEqual(res.nodes, [], 'a retired, silent board is not reported offline');
    assert.equal(res.ok, true);
  } finally { rmSync(s.dir, { recursive: true, force: true }); }
});
