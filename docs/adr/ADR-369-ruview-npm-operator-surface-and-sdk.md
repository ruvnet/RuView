# ADR-369: npm RuView operator surface — CLI, MCP, and SDK over one registry

- **Status**: proposed
- **Date**: 2026-09-30
- **Deciders**: RuView maintainers; acceptance pending
- **Tags**: npm, mcp, sdk, cli, metaharness, least-authority
- **Related**: ADR-263/265 (npm harness review and distribution), ADR-283
  (community metaharness), ADR-368 (compute kernel), ADR-370 (flashing),
  ADR-371 (training), ADR-372 (doctor)

## Context

`@ruvnet/ruview` 0.5.x could onboard, lint claims, verify the proof, monitor a
node, and read Cognitum Spaces. Three operator jobs still left the package:

- **Flashing** was a Windows-only stub that returned `manual_step_required`.
- **Training** had only a skill document; nothing ran a trainer or checked
  results mechanically.
- **Diagnostics**: `doctor` printed six PASS/FAIL lines without remedies.

There was also no programmatic API. Integrators had to shell out to the CLI or
speak MCP.

## Decision

1. **One registry, three front ends.** Every capability is a `ruview_*` tool
   in `src/tools.js`, with a JSON schema, a `TOOL_POLICY` class, and a
   structured, fail-closed result. The CLI verbs, the MCP server, and the new
   SDK all go through `runTool`. There is one argument validator and one
   authorization path.
2. **SDK** (`@ruvnet/ruview/sdk`, typed by `src/sdk.d.ts`). `createRuView()`
   returns a frozen object with `doctor`, `firmware.{ports,plan,flash,monitor}`,
   `training.{plan,run,gate,calibrate}`, `kernel.{selfTest,load}`, `guidance`,
   `claimCheck`, `verify`, `spaces`, `memorySearch`, `call`, `tools`, and
   `startMcpServer`.
   - Calls run with `source: 'sdk'`: trusted in-process code, like the CLI.
     Mutating tools still refuse to act without `confirm: true`.
   - `{ strict: true }` turns `ok:false` into a `RuViewError`.
3. **Plan/act split for every mutation.**
   - `ruview_firmware_plan` and `ruview_train_plan` are read-only and their
     schemas do not accept `confirm`. An MCP client without write grants can
     review the exact command.
   - The acting tools (`ruview_node_flash`, `ruview_train`) keep the existing
     grant-plus-confirm gate: `hardware-write` and `workspace-write`.
4. **New tools**: `ruview_doctor`, `ruview_firmware_plan`,
   `ruview_firmware_ports`, `ruview_train`, `ruview_train_plan`,
   `ruview_train_gate`. `ruview_node_flash` is re-implemented (ADR-370).
   `TOOL_POLICY`, `.harness/claims.json`, and `.harness/mcp-policy.json` are
   kept in lock-step, and a test fails on any drift.
5. **Process execution** for operator tools goes through `src/exec.js`:
   - argv only, never a shell;
   - a scrubbed environment: the base allowlist plus toolchain locations such
     as `LIBTORCH`, `CARGO_HOME`, `VIRTUAL_ENV` and `IDF_PATH`, and no
     credentials;
   - bounded output, and secrets redacted.
6. **Zero runtime dependencies stays.** The optional compute kernel
   (ADR-368) is loaded dynamically. Version 0.6.0. The unpacked-size ceiling
   rises from 160 KiB to 224 KiB for the new modules, with the budget recorded
   in both npm workflows.

## Consequences

- Agents, scripts, and humans get the same behaviour and the same refusals.
- The package now drives real hardware and training. Keeping the plan/act
  split and argv-only execution is essential, and tests pin both.
- The surface is larger (16 MCP tools). Hosts that cap tool counts can
  filter by annotations (`readOnlyHint`).

## Acceptance

- `cd harness/ruview && npm test && npm run test:security` (drift test in
  `test/sdk.test.mjs`).
- `npx @ruvnet/ruview tools` lists 16 tools. `node -e "import('@ruvnet/ruview/sdk')"`
  resolves from an installed tarball.
