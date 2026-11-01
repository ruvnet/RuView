import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { tmpdir, userInfo } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { buildSshArgs, restrictToOwner, runRemote, saveHost, shellQuote, validateHost } from '../src/remote.js';
import { isRemoteReadOnlyTool, runTool, TOOLS } from '../src/tools.js';
import { validateArguments } from '../src/policy.js';

test('host entries reject ssh option injection and shell metacharacters', () => {
  for (const ssh of ['-oProxyCommand=id', 'pi;id', 'user@pi id', '$(id)@pi', 'user@-pi', 'pi`id`', '']) {
    assert.throws(() => validateHost({ name: 'pi', ssh }), /ssh must be/, ssh);
  }
  for (const name of ['Pi', '../x', 'a b', '-x']) assert.throws(() => validateHost({ name, ssh: 'pi' }), /name must/);
  assert.throws(() => validateHost({ name: 'pi', ssh: 'pi', version: 'latest' }), /semver/);
  assert.deepEqual(validateHost({ name: 'lab-pi', ssh: 'ruv@pi.local', port: 2222 }), { name: 'lab-pi', ssh: 'ruv@pi.local', port: 2222 });
});

test('remote command is fully shell-quoted and ssh options are fixed', t => {
  assert.equal(shellQuote("a'b"), `'a'\\''b'`);
  const argv = buildSshArgs({ ssh: 'ruv@pi.local', port: 2222 }, ['npx', '-y', '@ruvnet/ruview@0.7.0', 'call', 'ruview_doctor', '--read-only', '--args-json', '{"x":"$(id); `rm`"}']);
  assert.deepEqual(argv.slice(0, 11), ['-o', 'BatchMode=yes', '-o', 'ConnectTimeout=10', '-o', 'StrictHostKeyChecking=yes', '-T', '-p', '2222', '--', 'ruv@pi.local']);
  const remote = argv.at(-1);
  let shell = 'sh';
  if (process.platform === 'win32' && spawnSync(shell, ['-c', 'exit 0']).status !== 0) {
    const git = spawnSync('where.exe', ['git'], { encoding: 'utf8' }).stdout?.trim().split(/\r?\n/)[0];
    const gitShell = git && resolve(dirname(git), '..', 'bin', 'sh.exe');
    if (!gitShell || !existsSync(gitShell)) { t.skip('POSIX shell unavailable; Linux CI exercises shell interpretation'); return; }
    shell = gitShell;
  }
  // Every token is single-quoted, so the remote shell sees literal text.
  const sh = spawnSync(shell, ['-c', `for a in ${remote}; do printf '%s\\n' "$a"; done`], { encoding: 'utf8' });
  assert.equal(sh.status, 0, sh.stderr || sh.error?.message);
  assert.equal(sh.stdout.trim().split('\n').at(-1), '{"x":"$(id); `rm`"}');
});

async function withHosts(fn) {
  const dir = mkdtempSync(join(tmpdir(), 'ruview-hosts-'));
  const path = join(dir, 'ruview', 'hosts.json');
  try { return await fn(path); } finally { rmSync(dir, { recursive: true, force: true }); }
}

const baseDeps = (path, execResult) => {
  const calls = [];
  return {
    calls,
    hostsPath: path,
    which: (b) => (b === 'ssh' ? '/usr/bin/ssh' : null),
    version: '0.7.0',
    readOnlyTool: isRemoteReadOnlyTool,
    validate: (name, args) => (TOOLS[name] ? validateArguments(TOOLS[name].inputSchema, args) : ['unknown']),
    exec: async (cmd, args) => { calls.push([cmd, ...args]); return execResult; },
  };
};

test('hosts file is private and remote runs are read-only, validated, and version-pinned', async () => {
  await withHosts(async (path) => {
    const saved = saveHost({ name: 'pi', ssh: 'ruv@pi.local' }, path);
    if (process.platform === 'win32') {
      // POSIX modes are ignored on Windows; privacy comes from an owner-only ACL.
      assert.equal(saved.access, 'windows-acl-owner-only');
      const acl = spawnSync('icacls', [path], { encoding: 'utf8' }).stdout;
      const grants = acl.split('\n').map((l) => l.replace(path, '').trim()).filter((l) => /:\(/.test(l));
      assert.equal(grants.length, 1, acl);
      assert.match(grants[0], /:\(F\)$/);
      assert.ok(grants[0].toLowerCase().includes(`${userInfo().username.toLowerCase()}:(F)`.toLowerCase()), acl);
    } else {
      assert.equal(saved.access, 'posix-0600');
      assert.equal(statSync(path).mode & 0o777, 0o600);
    }
    assert.equal(JSON.parse(readFileSync(path, 'utf8')).hosts[0].name, 'pi');

    const ok = baseDeps(path, { ok: true, code: 0, stdout: JSON.stringify({ ok: true, devices: [] }), stderr: '', error: null });
    const r = await runRemote({ host: 'pi', tool: 'ruview_devices_scan', args: {} }, ok);
    assert.equal(r.ok, true);
    assert.deepEqual(r.remote, { host: 'pi', ssh: 'ruv@pi.local', version: '0.7.0' });
    assert.match(ok.calls[0].at(-1), /'@ruvnet\/ruview@0\.7\.0' 'call' 'ruview_devices_scan' '--read-only'/);

    assert.equal((await runRemote({ host: 'pi', tool: 'ruview_node_flash', args: {} }, ok)).reason, 'remote_tool_not_allowed');
    assert.equal((await runRemote({ host: 'pi', tool: 'ruview_train', args: {} }, ok)).reason, 'remote_tool_not_allowed');
    assert.equal((await runRemote({ host: 'pi', tool: 'ruview_spaces_list', args: {} }, ok)).reason, 'remote_tool_not_allowed');
    assert.equal((await runRemote({ host: 'pi', tool: 'ruview_host_run', args: {} }, ok)).reason, 'remote_tool_not_allowed', 'no host hopping');
    assert.equal((await runRemote({ host: 'pi', tool: 'ruview_mmwave_read', args: { port: 'x;id' } }, ok)).reason, 'invalid_arguments');
    assert.equal((await runRemote({ host: 'nope', tool: 'ruview_doctor' }, ok)).reason, 'unknown_host');

    const keyFail = baseDeps(path, { ok: false, code: 255, stdout: '', stderr: 'Host key verification failed.', error: 'x' });
    const kr = await runRemote({ host: 'pi', tool: 'ruview_doctor', args: {} }, keyFail);
    assert.equal(kr.reason, 'host_key_unverified');
    assert.match(kr.remedy, /ssh ruv@pi\.local/);
    const auth = baseDeps(path, { ok: false, code: 255, stdout: '', stderr: 'Permission denied (publickey).', error: 'x' });
    assert.equal((await runRemote({ host: 'pi', tool: 'ruview_doctor', args: {} }, auth)).reason, 'ssh_auth_failed');
  });
});

test('MCP remote runs need remote-host plus the remote tool grant; list is read-only', async () => {
  assert.equal((await runTool('ruview_host_run', { host: 'pi', tool: 'ruview_doctor' }, { source: 'mcp', grants: [] })).requiredGrant, 'remote-host');
  const needsDevice = await runTool('ruview_host_run', { host: 'pi', tool: 'ruview_devices_scan' }, { source: 'mcp', grants: ['remote-host'] });
  assert.deepEqual([needsDevice.reason, needsDevice.requiredGrant], ['authority_denied', 'device-access']);
  const list = await runTool('ruview_host_list', {}, { source: 'mcp', grants: [] });
  assert.equal(typeof list.ok, 'boolean');
});

test('`call --read-only` refuses mutating tools (what remote hosts execute)', () => {
  const cli = join(import.meta.dirname, '..', 'bin', 'cli.js');
  const refused = spawnSync(process.execPath, [cli, 'call', 'ruview_node_flash', '--read-only', '--args-json', '{"port":"COM7","bundle":"/x","confirm":true}'], { encoding: 'utf8' });
  assert.equal(refused.status, 1);
  assert.equal(JSON.parse(refused.stdout).reason, 'remote_tool_not_allowed');
  const allowed = spawnSync(process.execPath, [cli, 'call', 'ruview_train_gate', '--read-only', '--args-json', '{"model_score":0.6}'], { encoding: 'utf8' });
  assert.equal(JSON.parse(allowed.stdout).verdict, 'FAIL');
});

test('Windows hosts-file restriction removes explicit grants and handles literal Unicode paths', { skip: process.platform !== 'win32' }, async () => {
  await withHosts(async (path) => {
    // Wildcards and PowerShell expressions are legal filename characters. A
    // literal path on stdin must neither expand nor execute any of this text.
    const literal = join(dirname(path), "hosts-é [x] $(throw 'path-executed').json");
    saveHost({ name: 'pi', ssh: 'ruv@pi.local' }, literal);
    const grant = spawnSync('icacls', [literal, '/grant', '*S-1-1-0:R', '*S-1-5-18:F'], { encoding: 'utf8' });
    assert.equal(grant.status, 0, grant.stderr || grant.stdout);
    const before = spawnSync('icacls', [literal], { encoding: 'utf8' });
    assert.equal(before.status, 0, before.stderr);
    assert.ok(before.stdout.split('\n').filter(line => /:\(/.test(line)).length >= 3, before.stdout);
    assert.equal(restrictToOwner(literal), 'windows-acl-owner-only');
    const after = spawnSync('icacls', [literal], { encoding: 'utf8' });
    assert.equal(after.status, 0, after.stderr);
    const grants = after.stdout.split('\n').map(line => line.replace(literal, '').trim()).filter(line => /:\(/.test(line));
    assert.equal(grants.length, 1, after.stdout);
    assert.ok(grants[0].toLowerCase().includes(`${userInfo().username}:(F)`.toLowerCase()), after.stdout);
    assert.equal(JSON.parse(readFileSync(literal, 'utf8')).hosts[0].name, 'pi');
  });
});

test('Windows ACL helper isolates incompatible inherited PowerShell modules', { skip: process.platform !== 'win32' }, async () => {
  await withHosts(async (path) => {
    const modules = join(dirname(path), 'modules');
    const utility = join(modules, 'Microsoft.PowerShell.Utility');
    mkdirSync(utility, { recursive: true });
    // Reproduce pwsh7 -> node -> Windows PowerShell5 without installing pwsh:
    // this higher-priority module advertises the cmdlet but requires PS7.
    writeFileSync(join(utility, 'Microsoft.PowerShell.Utility.psd1'),
      "@{ ModuleVersion='7.0.0'; PowerShellVersion='7.0'; CmdletsToExport=@('ConvertFrom-Json') }");
    const inherited = Object.fromEntries(Object.entries(process.env).filter(([key]) => key.toUpperCase() !== 'PSMODULEPATH'));
    inherited.PSModulePath = `${modules};${process.env.PSModulePath || ''}`;
    const original = { ...inherited };
    const parentPath = process.env.PSModulePath;
    const control = spawnSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', "'{}' | ConvertFrom-Json"], {
      env: inherited, encoding: 'utf8', windowsHide: true, timeout: 10_000,
    });
    assert.equal(control.status, 1, control.stderr);
    assert.match(control.stderr, /CouldNotAutoloadMatchingModule/);
    const saved = saveHost({ name: 'pi', ssh: 'ruv@pi.local' }, path, { env: inherited });
    assert.equal(saved.access, 'windows-acl-owner-only');
    assert.ok(Object.keys(inherited).length === Object.keys(original).length
      && Object.entries(original).every(([key, value]) => inherited[key] === value),
    'the caller environment is unchanged');
    assert.equal(process.env.PSModulePath, parentPath, 'the process environment is unchanged');
    const acl = spawnSync('icacls', [path], { encoding: 'utf8' });
    assert.equal(acl.status, 0, acl.stderr);
    const grants = acl.stdout.split('\n').filter(line => /:\(/.test(line));
    assert.equal(grants.length, 1, acl.stdout);
  });
});

test('Windows hosts-file restriction fails closed and keeps input out of PowerShell source', () => {
  const calls = [];
  const path = "C:/x/hosts $(throw 'injected').json";
  const env = { SystemRoot: 'C:/Windows', pSmOdUlEpAtH: 'incompatible-PS7-modules', KEEP: 'unchanged' };
  const ok = restrictToOwner(path, { platform: 'win32', env, exec: (cmd, args, options) => { calls.push({ cmd, args, options }); return { status: 0 }; } });
  assert.equal(ok, 'windows-acl-owner-only');
  assert.equal(calls[0].cmd, 'powershell.exe');
  assert.deepEqual(calls[0].args.slice(0, 4), ['-NoLogo', '-NoProfile', '-NonInteractive', '-Command']);
  assert.ok(!calls[0].args.at(-1).includes(path));
  assert.deepEqual(JSON.parse(calls[0].options.input), { path });
  assert.deepEqual(calls[0].options.env, { SystemRoot: 'C:/Windows', KEEP: 'unchanged' });
  assert.equal(env.pSmOdUlEpAtH, 'incompatible-PS7-modules');
  for (const result of [{ status: 5 }, { status: null, error: { code: 'ENOENT' } }, { status: null, error: { code: 'ETIMEDOUT' } }]) {
    assert.throws(() => restrictToOwner(path, { platform: 'win32', exec: () => result }), /not made private/);
  }
  assert.equal(restrictToOwner('/tmp/h.json', { platform: 'linux', exec: () => { throw new Error('must not run'); } }), 'posix-0600');
});
