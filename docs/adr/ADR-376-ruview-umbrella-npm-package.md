# ADR-376: `ruview` — one npm install for every RuView component

- **Status**: proposed
- **Date**: 2026-10-01
- **Deciders**: rUv (requested and approved the release); RuView maintainers
- **Tags**: npm, packaging, umbrella, mcp, homecore, kernel, release
- **Related**: ADR-263 (harness hardening), ADR-265 (distribution, D2 CI-only
  publishing, D4 bin ownership), ADR-285 (Homecore metaharness), ADR-368
  (kernel), ADR-369 (operator surface), ADR-373 (device access), ADR-375 (MCP
  Apps console, HTTP transport, terminal UI)

## Context

RuView is positioned as an ambient intelligence platform rather than a sensor.
Its sensing is ahead of comparable packaged options; packaging is the
bottleneck. Today the capabilities are split across three packages:

| Package | Contents | Published before this ADR |
|---|---|---|
| `@ruvnet/ruview` | operator harness | 0.5.1 |
| `homecore` | Homecore developer metaharness | never |
| `@ruvnet/ruview-kernel` | WASM vitals kernel | never |

The unscoped name `ruview` was unclaimed. A user has to know all three
packages, install them separately, and run three CLIs and two MCP servers.

## Decision

1. **The umbrella.** Publish an unscoped `ruview` package
   (`harness/ruview-umbrella`). It depends on exact versions of the three
   component packages and adds no capability of its own:
   - **CLI.** One binary, `ruview`:
     - `ruview homecore …` → the Homecore CLI;
     - `ruview kernel <doctor|info|selftest|parity|synth|analyze|bench>` → the
       kernel CLI;
     - bare `ruview kernel [--backend …]` keeps its harness meaning (the
       SYNTHETIC self-test);
     - `ruview capabilities` lists every component, version and tool;
     - everything else goes to the `@ruvnet/ruview` CLI unchanged.
   - **MCP server.** One server, `ruview mcp start [--http]`. It serves the
     harness tools (`ruview_*`, including the `ui://` console) plus the
     Homecore tools (`homecore_*`, already prefixed, so no collision).
     - Non-Homecore traffic goes to the harness handler, so the harness
       authority model, grants and HTTP rules (ADR-375) apply unchanged.
     - Homecore calls go to Homecore's `runTool`, with Homecore's own
       policy.
2. **Bin ownership (amends ADR-265 D4).** The `ruview` command name moves to
   the umbrella. `@ruvnet/ruview`'s bin is renamed `ruview-harness`.
   - **Why:** a clean-directory tarball install showed the collision. With
     both packages declaring `ruview`, `node_modules/.bin/ruview` linked to
     the harness CLI, so `ruview capabilities` and `ruview homecore` failed.
     Which package wins that link is not guaranteed.
   - **Unaffected:** `npx @ruvnet/ruview …` still works, because npx runs a
     package's only bin whatever its name. The umbrella delegates every
     non-umbrella verb to the harness, so `ruview <verb>` behaves as before.
   - **Changed:** a global install of `@ruvnet/ruview` alone now provides
     `ruview-harness`.
3. **Harness changes needed to embed it** (no new runtime dependencies):
   - **New exports:** `./cli`, `./cli-ui`, `./mcp`, `./mcp-http`, `./ui`,
     `./esphome` and `./kernel`.
   - **Injectable MCP handler:** `run(args, { mcpHandler })` and
     `startMcpServer({ handler })`, so the merged server reuses the stdio and
     HTTP transports.
   - **Kernel declared as an optional peer:** `@ruvnet/ruview-kernel`.
   - **`setKernelImporter`:** lets the umbrella resolve the kernel from its
     own dependency tree. Node resolves bare specifiers from the importing
     file's real path, which misses a sibling dependency under linked or
     pnpm-style installs.
     - Found while testing: `kernel_not_installed` through a linked umbrella.
     - Only embedding code can call it; MCP arguments cannot choose the
       module.
   - **Homecore** gains a `./cli` export; **the kernel** gains `./cli`.
4. **Versions for this release.**

   | Package | Version | Notes |
   |---|---|---|
   | `@ruvnet/ruview` | 0.8.0 | 0.7.0 was never published |
   | `homecore` | 0.1.0 | first publish |
   | `@ruvnet/ruview-kernel` | 0.1.0 | first publish; WASM only |
   | `ruview` | 0.8.0 | umbrella |

   The kernel ships WASM only. The napi backend stays optional and absent,
   because a workstation build would cover only win32-x64. `loadKernel({
   backend: 'auto' })` reports the honest fallback.
5. **Publishing exception (one-off, amends ADR-265 D2 for this release).**
   - rUv explicitly chose a local `npm publish` from the operator
     workstation (ruvzen) over the CI-only release workflow.
   - Packages are published in dependency order: kernel → homecore →
     @ruvnet/ruview → ruview.
   - Each is published only after its full gate passes locally (tests,
     security tests, manifest, version-literal and pack-content/size gates),
     plus a clean-directory tarball install and smoke test of the umbrella.
   - These versions carry no npm provenance attestation.
   - Subsequent releases return to the CI workflow. Adding `ruview` and
     `@ruvnet/ruview-kernel` to `ruview-npm-release.yml` is follow-up work.

## Release outcome (2026-10-01)

**Published from ruvzen** (exact tarballs that passed the clean-directory
smoke test; sha256 in the release thread):

| Package | Version |
|---|---|
| `@ruvnet/ruview-kernel` | 0.1.0 |
| `homecore` | 0.1.0 |
| `@ruvnet/ruview` | 0.8.0 |

All three were re-installed from the registry and smoke-tested:
- `npx -y @ruvnet/ruview@0.8.0` works;
- the harness self-test resolves the published kernel;
- a live ESPHome radar read succeeded.

**`ruview` was refused by npm:** `403 Forbidden — Package name too similar to
existing package iview` (typosquat protection).
- The umbrella is ready in `harness/ruview-umbrella`. Until npm support grants
  the name, it is unpublished.
- If the request is declined, the fallback is a scoped name
  (`@ruvnet/ruview-platform`).

## Consequences

- `npx ruview` is the single entry point for operators and agents. One MCP
  connector gives an agent, or ChatGPT, every RuView and Homecore tool.
- The component packages stay independently installable and testable.
  `@ruvnet/ruview` remains free of runtime dependencies.
- The umbrella pins exact component versions, so each component release needs
  an umbrella release to reach `npx ruview` users.
- The local publish trades provenance for speed for this release only. The
  exception is recorded here and in the #swarm coordination thread.

## Evidence

- **Umbrella tests:** 4/4, covering CLI dispatch to all three components,
  merged MCP tool listing and routing, the harness authority model through
  the merged server, and stdio and HTTP transports.
- **Component gates:**
  - harness: 166 tests, 89 security tests;
  - Homecore: 27 tests, 23 security tests;
  - kernel: 21 tests, 3 skipped native cases; 12 security tests;
  - all manifests verified, 0 audit findings.
- **Clean-directory tarball smoke:** recorded in the release commit and the
  #development post.
