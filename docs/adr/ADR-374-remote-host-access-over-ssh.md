# ADR-374: Remote host access over SSH (read-only)

- **Status**: proposed
- **Date**: 2026-09-30
- **Deciders**: RuView maintainers; acceptance pending
- **Tags**: remote, ssh, hardware, least-authority, mcp
- **Related**: ADR-369, ADR-373

## Context

RuView hardware usually hangs off a different machine from the one the
operator or agent works on: a Raspberry Pi next to the ESP32s, a NUC running
the sensing server, a Cognitum Seed. Cloud agent sessions cannot reach local
USB at all.

"Host access" should not mean opening a new network service with its own
credentials: that adds attack surface and secret handling. Operators already
have SSH.

## Decision

1. **Where work runs.**
   - Hardware tools always execute on the machine the hardware is attached
     to.
   - Interactive work runs in a local Claude Code session on that machine. It
     can be driven from the Claude app through Remote Control
     (`claude remote-control` in the repo).
   - Other machines are reached over SSH.
2. **Hosts registry.**
   - Stored in `~/.config/ruview/hosts.json` (directory 0700, file 0600).
   - Written only by the CLI: `ruview hosts add --name --ssh [--port]
     [--version]`. MCP can list hosts but never add them.
   - Names must match `^[a-z0-9][a-z0-9-]{0,31}$`. Targets must be
     `[user@]hostname`: no options, spaces, shell metacharacters or leading
     dash.
3. **Transport.**
   - Local ssh arguments: `ssh -o BatchMode=yes -o ConnectTimeout=10 -o
     StrictHostKeyChecking=yes -T [-p N] -- <target> <command>`.
   - No password prompts, and no trust-on-first-use from the tool. The
     operator pins the host key once interactively.
   - The remote command is `npx -y @ruvnet/ruview@<exact version> call <tool>
     --read-only --args-json <json>`, with every token single-quoted for the
     remote POSIX shell.
4. **Read-only, twice.**
   - Locally, `ruview_host_run` accepts only tools that are read-only, use no
     credentials, and are not `ruview_host_*` (no host hopping). It validates
     arguments against the tool schema before sending.
   - Remotely, `call --read-only` repeats the check and schema validation.
   - Flash, train, calibrate and Cognitum Spaces reads are never forwarded.
5. **Grants.** Over MCP, `ruview_host_run` needs `remote-host` plus the
   remote tool's own grant (for example `device-access`).
6. **Failures** are classified with remedies: `host_key_unverified`,
   `ssh_auth_failed`, `host_unreachable`, `remote_node_missing`,
   `unknown_host`, `remote_tool_not_allowed`.

## Evidence

Tests in `harness/ruview/test/remote.test.mjs` cover:

- host-entry injection cases (`-oProxyCommand=…`, `;`, spaces, `$(…)`,
  backticks);
- quoting, verified by running the quoted command through a real `sh`;
- the refusal list;
- grant composition;
- the file mode;
- `call --read-only` refusing `ruview_node_flash` through the real CLI.

No SSH server was available in the development container, so a live
round-trip to a real host is still to be done.

## Consequences

- Remote hosts get the same tools with no new daemon, port or credential
  store.
- The remote needs Node 20+ and npm registry access (or a pre-installed
  version), and `ssh` must be on the local PATH. `doctor --group remote`
  checks the local side.
- Remote mutations stay manual by design.
