# ADR-372: Structured debugging doctor

- **Status**: proposed
- **Date**: 2026-09-30
- **Deciders**: RuView maintainers; acceptance pending
- **Tags**: diagnostics, onboarding, support, mcp
- **Related**: ADR-369, ADR-370, ADR-371, docs/TROUBLESHOOTING.md

## Context

Most failed RuView setups come down to a missing driver, pyserial, esptool, a
wrong port, uninitialized submodules, a missing wasm32 target, or a sensing
server that is not running. The old `doctor` checked only the harness itself,
gave no remedies, and had no machine-readable output for agents.

## Decision

`src/doctor.js` returns
`{ok, summary{pass,warn,fail,skip}, checks[{group,id,status,detail,remedy}], nextSteps}`.

Groups:

| Group | Checks |
|---|---|
| `runtime` | Node ≥ 20 |
| `harness` | packaged-file manifest integrity, claim-guardrail self-test, reviewed brain load |
| `hosts` | `claude` / `codex` on PATH |
| `repo` | checkout, workspace submodules |
| `rust` | cargo, wasm32 target (run inside `v2/` so the pinned toolchain is used), `wifi-densepose` binary |
| `python` | interpreter, pyserial, esptool version |
| `firmware` | checksum verification of every `release_bins` bundle |
| `serial` | pyserial port enumeration; optional named port; optional chip probe |
| `sensing` | optional sensing-server `/health` |
| `kernel` | optional `@ruvnet/ruview-kernel` WASM/napi status |

Rules:

- `fail` is reserved for a requested capability that cannot work. Absent
  optional tooling is `warn`. Every non-pass check carries a remedy. The CLI
  exit code is non-zero only on `fail`.
- The doctor is read-only. Two exceptions exist, both CLI/SDK options that are
  absent from the MCP schema:
  - `--probe` runs `esptool chip_id`, which resets the board but writes
    nothing.
  - `--url` makes an HTTP(S) request.
- Output is human-readable by default. `--json` gives the structured report.
  `--group a,b` limits the scope.

## Evidence

Development container (Linux x64, Node 22):

- Before `pip install pyserial esptool`, the doctor reported them as `warn`
  with the exact pip commands.
- After installation it showed `esptool 5.4.0` and enumerated `/dev/ttyS0`.
- All four firmware bundles verified.
- It caught manifest drift during development, and a missing wasm32 target
  when run outside `v2/`. That second case led to the `v2/` working-directory
  rule.

## Consequences

- Support threads can ask for `npx @ruvnet/ruview doctor --json`, and agents
  can act on `nextSteps`.
- Checks run real subprocesses, so a full run takes seconds; `--group` keeps
  targeted runs fast.
