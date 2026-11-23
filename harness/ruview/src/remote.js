// SPDX-License-Identifier: MIT
// Remote host access over SSH (ADR-374).
//
// Hardware is usually attached to another machine (a Raspberry Pi, a NUC, a
// Cognitum Seed). Instead of opening a new listening service, the harness uses
// the operator's existing SSH trust: it runs a pinned `@ruvnet/ruview` on the
// remote host with a READ-ONLY tool call, validated locally and remotely.
// Mutations (flash, train, calibrate) are never forwarded.

import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';

const NAME_RE = /^[a-z0-9][a-z0-9-]{0,31}$/;
const TARGET_RE = /^(?:[A-Za-z0-9._][A-Za-z0-9._-]{0,31}@)?[A-Za-z0-9][A-Za-z0-9.-]{0,252}$/;
const VERSION_RE = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?$/;

export function hostsFile(env = process.env) {
  const base = env.XDG_CONFIG_HOME || join(env.HOME || env.USERPROFILE || homedir(), '.config');
  return join(base, 'ruview', 'hosts.json');
}

export function validateHost(h) {
  if (!h || typeof h !== 'object') throw Object.assign(new Error('host entry must be an object'), { reason: 'invalid_host' });
  if (!NAME_RE.test(h.name || '')) throw Object.assign(new Error('name must match ^[a-z0-9][a-z0-9-]{0,31}$'), { reason: 'invalid_host' });
  if (!TARGET_RE.test(h.ssh || '')) throw Object.assign(new Error('ssh must be [user@]hostname (no options, spaces, or leading dash)'), { reason: 'invalid_host' });
  if (h.port !== undefined && (!Number.isInteger(h.port) || h.port < 1 || h.port > 65535)) throw Object.assign(new Error('port must be 1..65535'), { reason: 'invalid_host' });
  if (h.version !== undefined && !VERSION_RE.test(h.version)) throw Object.assign(new Error('version must be an exact semver'), { reason: 'invalid_host' });
  return { name: h.name, ssh: h.ssh, ...(h.port ? { port: h.port } : {}), ...(h.version ? { version: h.version } : {}), ...(h.description ? { description: String(h.description).slice(0, 200) } : {}) };
}

export function loadHosts(path = hostsFile()) {
  if (!existsSync(path)) return { path, hosts: [] };
  const data = JSON.parse(readFileSync(path, 'utf8'));
  return { path, hosts: (data.hosts || []).map(validateHost) };
}

// A fixed program receives the path as JSON on stdin, never PowerShell source.
// Construct a fresh protected DACL: icacls /inheritance:r /grant:r leaves other
// explicit grants intact (including grants on hosted Windows temporary files).
const OWNER_ONLY_ACL = [
  "$ErrorActionPreference = 'Stop'",
  '[Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false)',
  '$request = [Console]::In.ReadToEnd() | ConvertFrom-Json',
  '$sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User',
  '$acl = [System.Security.AccessControl.FileSecurity]::new()',
  '$acl.SetOwner($sid)',
  '$acl.SetAccessRuleProtection($true, $false)',
  '$rule = [System.Security.AccessControl.FileSystemAccessRule]::new($sid, [System.Security.AccessControl.FileSystemRights]::FullControl, [System.Security.AccessControl.AccessControlType]::Allow)',
  '$acl.AddAccessRule($rule)',
  '[System.IO.File]::SetAccessControl([string]$request.path, $acl)',
  '$applied = Get-Acl -LiteralPath $request.path',
  '$rules = @($applied.GetAccessRules($true, $true, [System.Security.Principal.SecurityIdentifier]))',
  'if (-not $applied.AreAccessRulesProtected -or $rules.Count -ne 1 -or $rules[0].IdentityReference.Value -ne $sid.Value -or $rules[0].AccessControlType -ne [System.Security.AccessControl.AccessControlType]::Allow -or $rules[0].FileSystemRights -ne [System.Security.AccessControl.FileSystemRights]::FullControl) { throw "owner-only ACL verification failed" }',
].join('\n');

/**
 * Windows ignores POSIX modes, so 0o600 alone leaves the file readable through
 * inherited ACLs. Replace them with a single full-control grant for the owner.
 * Fails closed: an unrestricted hosts file is an error, not a warning.
 */
export function restrictToOwner(path, { platform = process.platform, exec = spawnSync, env = process.env } = {}) {
  if (platform !== 'win32') return 'posix-0600';
  // pwsh -> node -> powershell.exe otherwise inherits PS7's incompatible
  // module paths. Let Windows PowerShell construct its own defaults without
  // changing the caller's environment or execution policy (about_PSModulePath).
  const childEnv = Object.fromEntries(Object.entries(env).filter(([key]) => key.toUpperCase() !== 'PSMODULEPATH'));
  const r = exec('powershell.exe', ['-NoLogo', '-NoProfile', '-NonInteractive', '-Command', OWNER_ONLY_ACL], {
    input: JSON.stringify({ path }), env: childEnv, encoding: 'utf8', windowsHide: true, timeout: 10_000, maxBuffer: 16_384,
  });
  if (r.error || r.status !== 0) {
    throw new Error(`could not restrict ${path} to the current user (ACL helper exit ${r.status ?? r.error?.code}); the hosts file was not made private`);
  }
  return 'windows-acl-owner-only';
}

/** CLI-only: add or replace a host entry. */
export function saveHost(entry, path = hostsFile(), deps = {}) {
  const host = validateHost(entry);
  const { hosts } = loadHosts(path);
  const next = [...hosts.filter((h) => h.name !== host.name), host].sort((a, b) => a.name.localeCompare(b.name));
  mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
  writeFileSync(path, `${JSON.stringify({ schema: 1, hosts: next }, null, 2)}\n`, { mode: 0o600 });
  const access = restrictToOwner(path, deps);
  return { path, host, access };
}

/** POSIX single-quote one argument for the remote login shell. */
export function shellQuote(value) {
  return `'${String(value).replaceAll("'", `'\\''`)}'`;
}

/** Build the ssh argv. The remote command is fully quoted. */
export function buildSshArgs(host, remoteArgv) {
  return [
    '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=10', '-o', 'StrictHostKeyChecking=yes',
    '-T', ...(host.port ? ['-p', String(host.port)] : []),
    // `--` ends ssh option parsing before the destination (defence in depth;
    // TARGET_RE already forbids a leading dash).
    '--', host.ssh,
    remoteArgv.map(shellQuote).join(' '),
  ];
}

/**
 * Run a read-only tool on a remote host.
 * args: { host, tool, args }. deps: { exec, which, readOnlyTool(name)→bool, validate(name,args)→errors[], version }.
 */
export async function runRemote(input, deps) {
  let hosts;
  try { hosts = loadHosts(deps.hostsPath).hosts; } catch (error) { return { ok: false, reason: 'hosts_file_invalid', detail: error.message }; }
  const host = hosts.find((h) => h.name === input.host);
  if (!host) return { ok: false, reason: 'unknown_host', known: hosts.map((h) => h.name), remedy: 'ruview hosts add --name <name> --ssh user@host' };
  if (!deps.readOnlyTool(input.tool)) return { ok: false, reason: 'remote_tool_not_allowed', detail: 'Only read-only tools run remotely; perform mutations on the host itself (or via Remote Control).' };
  const toolArgs = input.args || {};
  const errors = deps.validate(input.tool, toolArgs);
  if (errors.length) return { ok: false, reason: 'invalid_arguments', errors };
  const ssh = deps.which('ssh');
  if (!ssh) return { ok: false, reason: 'ssh_missing', remedy: 'Install OpenSSH client.' };
  const version = host.version || deps.version;
  const remoteArgv = ['npx', '-y', `@ruvnet/ruview@${version}`, 'call', input.tool, '--read-only', '--args-json', JSON.stringify(toolArgs)];
  const r = await deps.exec(ssh, buildSshArgs(host, remoteArgv), { timeoutMs: 420_000, maxOutputBytes: 8_388_608, fullOutput: true });
  let result = null;
  try { result = JSON.parse(r.stdout); } catch { /* non-JSON output */ }
  if (!result) {
    const text = `${r.stderr}\n${r.error || ''}`;
    const reason = /Host key verification failed/i.test(text) ? 'host_key_unverified'
      : /Permission denied/i.test(text) ? 'ssh_auth_failed'
        : /Could not resolve|Connection (?:timed out|refused)|No route/i.test(text) ? 'host_unreachable'
          : /npx: (?:command )?not found|node: not found/i.test(text) ? 'remote_node_missing' : 'remote_failed';
    const remedy = {
      host_key_unverified: `Connect once interactively (ssh ${host.ssh}) to verify and pin the host key.`,
      ssh_auth_failed: 'Load a key into ssh-agent or add it to the remote authorized_keys; password prompts are disabled (BatchMode).',
      host_unreachable: 'Check the hostname/IP, network, and that sshd is running.',
      remote_node_missing: 'Install Node.js 20+ on the remote host.',
    }[reason];
    return { ok: false, reason, host: host.name, detail: text.trim().slice(-600), ...(remedy ? { remedy } : {}) };
  }
  return { ...result, remote: { host: host.name, ssh: host.ssh, version } };
}
