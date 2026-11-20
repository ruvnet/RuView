# ADR-377: `ruview-live` — a Claude Code mod shipped inside `@ruvnet/ruview`

- **Status**: proposed
- **Date**: 2026-10-01
- **Deciders**: rUv (requested integration into the npm package); RuView maintainers
- **Tags**: claude-code, mods, function-hooks, ui, npm, least-authority
- **Related**: ADR-373 (device access), ADR-375 (MCP Apps console, terminal UI),
  ADR-376 (umbrella package); Claude Code mods (function hooks): the
  claude-code issue 91870 and the `mods/` built-ins (`diff` is the closest
  analogue)

## Context

Claude Code mods are plugins whose behaviour is a function-hooks module: one
`register(on, options)` that hooks engine events as `($, e, next)`. They can:
- register slash commands;
- open panes beside the transcript and draw them with the surface's elements;
- set a status line;
- run processes through `$.process.run`.

That fits RuView's goal of putting the live state of the sensors where an
operator or agent already works.

RuView already has:
- a read-only CLI with `--json` results (ADR-373, ADR-375);
- an MCP server and the ChatGPT/MCP Apps console;
- a plugin marketplace in this repository.

## Decision

1. **Ship a mod, `ruview-live`, inside `@ruvnet/ruview`** (`mod/`, packaged
   files `mod/.claude-plugin/`, `mod/hooks/`).
   - It is listed in the repository marketplace as `ruview-live@ruview`, with
     source `./harness/ruview/mod`.
   - One copy serves both installs: the npm package and the marketplace.
2. **Behaviour.**
   - **`/ruview`** toggles a pane (`ruview-live`). While it is open, a
     `$.clock.every` timer (default 15 s, minimum 5 s) runs one read-only
     capture.
   - **`/ruview refresh`** runs one capture and returns the summary.
   - **`/ruview off`** closes the pane.
   - **The pane** shows nodes (rate, loss, RSSI, shape, SYNTHETIC flag), the
     optional ESPHome radar (presence, distance, device-reported vitals), and
     alerts, including heartbeat-only boards with the CSI-buffer-starvation
     hint. It has Refresh (`r`) and Close (`c`) buttons.
   - **The status line** is set on every refresh.
3. **Data comes only from the harness CLI beside the mod.**
   - The mod runs `node <plugin root>/../bin/cli.js esp32 … --json` and
     `mmwave --source esphome … --json`, as fixed argv arrays, through
     `$.process.run`. So the pane shows what the tested tools return.
   - Mods may import nothing but their own files and `claude-code` (the
     validator refuses `node:url`). The CLI is therefore located through
     `$.plugin.root`.
4. **Least authority.**
   - The mod never runs a write verb (flash, calibrate, train). A unit test
     pins the argv.
   - The radar host is validated before use.
   - Options are bounded: ports 1024–65535, refresh ≥ 5 s, capture 1–10 s.
5. **Discovery.** `ruview mod` (and `ruview mod --json`) prints the installed
   path, the `claude --plugin-dir` command, the marketplace commands, usage,
   settings, and the early-access note.

## Evidence

All on Claude Code 2.1.286, Windows 11 x64.
- **Validation:** `claude plugin validate harness/ruview/mod` passes. It lists
  hooks `session.start`, `command.run{ruview}`, `ui.render{Pane}`,
  `ui.close{ruview-live}` and `session.end`, and calls `$.clock.every`,
  `$.clock.now`, `$.command.register`, `$.process.run` and `$.ui.*`.
  `claude plugin validate .` passes for the marketplace.
- **Engine tests:** `claude plugin test harness/ruview/mod`, with
  `CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1`, passed 2/2 in three consecutive
  runs. They check:
  - `/ruview` opens the pane and runs the exact read-only argv;
  - the status reads `RuView · 2 nodes · 1 alert`;
  - the timer refreshes while open and **never after close**;
  - an honest `no_packets` status.
- **Harness unit tests:** `test/mod.test.mjs`, 6/6, under plain Node:
  settings bounds, host validation, argv, failure parsing, model/status, pane
  drawing and button wiring, and the no-imports rule.
- **Real engine, headless (MEASURED):**
  `claude --plugin-dir ./mod -p "/ruview refresh"` answered
  `RuView · 0 nodes · 1 alert` (no CSI node was streaming). With `radarHost`
  set to the live Seeed MR60BHA2 kit, it answered
  `RuView · 0 nodes · radar clear · 1 alert`.
- **Note:** under Git Bash a leading `/ruview` is rewritten into a Windows path
  unless `MSYS_NO_PATHCONV=1` is set. This is not an issue inside the Claude
  Code prompt.

## Consequences

- An operator sees node and radar health beside the conversation without
  asking for it. Agents see the same facts through MCP.
- **Mods are early access.** The API may change between Claude Code releases,
  and hooks modules load only where function hooks are enabled. The mod is
  therefore optional, and nothing else in the package depends on it.
- **Trust.** Like every mod, it runs inside Claude Code with Claude Code's
  access. It is read-only by construction, and the README says to install it
  only from a trusted source.
- A refresh binds UDP 5005 for the capture window. While the sensing server
  holds the port, the pane reports `port_in_use` instead of reading.
