# ADR-375: MCP Apps console, HTTP transport and terminal UI for `@ruvnet/ruview`

- **Status**: proposed
- **Date**: 2026-10-01
- **Deciders**: RuView maintainers; acceptance pending
- **Tags**: npm, mcp, mcp-apps, chatgpt, ui, cli, http, least-authority, performance
- **Related**: ADR-263 (harness hardening), ADR-265 (distribution and size
  budget), ADR-369 (operator surface and SDK), ADR-373 (device access),
  ADR-374 (remote hosts)

## Context

The harness exposes one tool registry through the CLI, an MCP stdio server and
an SDK (ADR-369). Hardware testing of the device tools (ADR-373 Amendment 2)
showed three gaps in how results reach people and models:

- **MCP results were text only.** Every tool returned pretty-printed JSON in a
  text block, with no `structuredContent`. The server declared protocol
  `2024-11-05`, and `resources/list` was always empty, so MCP Apps hosts and
  ChatGPT had nothing to render.
- **ChatGPT could not connect.** ChatGPT connectors reach MCP servers over
  HTTPS. The harness spoke stdio only.
- **The CLI printed raw JSON to humans.** A live capture of two nodes was a
  1.2 kB JSON object in the terminal, with alerts such as heartbeat-only
  senders easy to miss.

Reference implementations for the target hosts (the `web-based-chatgpt-mcp-starter`
and `signal-to-swarm` sites) declare a `ui://` HTML resource and point UI tools
at it through both the MCP Apps key (`_meta.ui.resourceUri`) and the ChatGPT
keys (`openai/outputTemplate`, `openai/widgetAccessible`).

The package must stay free of runtime dependencies (ADR-263) and inside a
reviewed size budget (ADR-265).

## Decision

### 1. One console widget, two host families

`src/ui/console-widget.js` holds one self-contained HTML resource,
`ui://ruview/console-v1.html`, with mimeType `text/html;profile=mcp-app`.

- **Tools.** `ruview_esp32_capture`, `ruview_devices_scan` and `ruview_doctor`
  carry `_meta.ui.resourceUri`, `ui/resourceUri`, `openai/outputTemplate` and
  `openai/widgetAccessible`.
- **Data intake.** The widget reads results from either host family:
  - MCP Apps: a `ui/initialize` handshake over `postMessage`, then
    `ui/notifications/tool-input` and `ui/notifications/tool-result`;
  - ChatGPT: `window.openai.toolOutput` and the `openai:set_globals` event.
- **Refresh.** A Refresh button re-runs the same tool with the same
  arguments, through `window.openai.callTool` or an MCP Apps `tools/call`
  request.
- **Safety.**
  - The resource declares an empty CSP allowlist and makes no network
    requests.
  - Every value reaches the DOM through `textContent`; there is no
    `innerHTML`.
  - Messages are accepted only from the parent frame.
  - A test pins all of this.
- **Design.** The visual language follows the reference sites: dark ground,
  `#d0ff72` accent, monospace labels, and plain wording about what each value
  is. Device vitals are labelled "device-reported (not validated)". Kernel
  outputs carry the "no reference measurement" note, and SYNTHETIC frames
  raise a warning.

### 2. Structured results and protocol negotiation

- **Protocol negotiation.** `initialize` negotiates `2025-06-18`,
  `2025-03-26` or `2024-11-05`, and advertises `resources`.
- **Result shape.** `tools/call` returns `structuredContent` (always an
  object), a text block with the same JSON in compact form, and
  `_meta['ruview/tool']` set to the canonical tool name (so an aliased call
  still refreshes correctly).
- **Compact text.** Compact rather than pretty-printed JSON cuts each result
  by 17–33% (MEASURED on this host: guidance 17.5%, device scan 20.7%, live
  capture 33.4%). Clients that read only `content` still receive the
  complete result.
- **Titles.** Tools gain `title` and `annotations.title`.

`handleRpc` is transport-neutral, and `createDispatcher` holds the FIFO tool
chain both transports share (ADR-263 O2).

### 3. Streamable HTTP transport

`ruview mcp start --http [--host 127.0.0.1] [--port 8790] [--allow-origin URL]`
starts `src/mcp-http.js`.

- **Authentication on every request.** A request must carry either
  `Authorization: Bearer <token>`, or the secret-path form `/mcp/<token>`.
  The second exists because ChatGPT connectors cannot send a static header.
  - Tokens are compared in constant time and must be at least 16 characters.
  - The token comes from `RUVIEW_MCP_TOKEN`, never argv. If unset, one is
    generated and printed once to stderr.
- **Least authority.**
  - It binds to loopback by default.
  - Browser `Origin`s are refused unless allowlisted (DNS-rebinding defence).
    Server-to-server clients send no Origin.
  - `workspace-write` and `hardware-write` are never honoured over HTTP, even
    when granted in `RUVIEW_MCP_GRANTS`. Flashing and calibration stay
    stdio/CLI-only.
- **Protocol surface.**
  - POST only; single JSON responses with no SSE stream and no sessions.
  - Batches and unsupported `MCP-Protocol-Version` values are refused.
  - Bodies are capped at 256 KiB.
  - Notifications return 202.
  - `/healthz` reports only the name and version.

Exposing the port to ChatGPT (a tunnel or reverse proxy with TLS) is the
operator's choice. The startup banner says so and warns on a non-loopback
bind.

### 4. Terminal UI

`src/cli-ui.js` renders results for a TTY:
- aligned tables, with unicode rate bars;
- colour that respects `NO_COLOR` and `TERM=dumb`;
- alerts and fixes ahead of the data;
- a stderr countdown for timed captures.

Pipes and `--json` keep receiving exact JSON. `ruview call` always prints
JSON, because remote hosts parse it.

`ruview esp32 --watch [--seconds 3]` redraws repeated capture windows with a
per-node rate sparkline.

`ruview monitor` now flags Ameba CSI report-buffer starvation (`lack of csi
buf` / `csi buf not enough`). This is the root cause of the Realtek
heartbeat-only stall, observed live: 399 starvation lines in 8 s, 0 CSI.

### 5. Startup

- **Launcher split.** `bin/cli.js` is a thin launcher, and `src/cli.js` loads
  the tool registry, doctor, brain, host adapters and transports on demand.
- **Measured effect.**
  - `--help`/`--version`/`skills` skip the registry: about 20% faster, median
    of 9 interleaved runs against the previous build on this host (MEASURED).
  - The MCP path needs the whole registry to list tools, so its start time is
    unchanged within noise.
- **Compile cache rejected.** Node's on-disk compile cache
  (`module.enableCompileCache`) was evaluated: module load was 27.7 ms with
  it and 28.5 ms without (25 interleaved runs, MEASURED). That is within
  noise, so it was not adopted.

### 6. Size budget

The additions cost about 38 kB:
- widget, 12.6 kB;
- HTTP transport;
- terminal UI;
- CLI split.

The `@ruvnet/ruview` unpacked budget rises from 320 KiB to 384 KiB in
`npm-packages.yml` and `ruview-npm-release.yml`. The package is about 338 kB.

## Evidence

All on Windows 11 x64, Node 24, with an ESP32-C6 (CP210x, COM16) and an
RTL8721Dx (PL2303GC, COM10) attached. All results MEASURED unless noted.

**Browser e2e.** A real MCP Apps host flow in headless Chrome:
- The host lists tools and reads the `ui://` resource over HTTP MCP, then
  loads it in a sandboxed iframe (`allow-scripts` only).
- It answers `ui/initialize` and delivers a live `ruview_esp32_capture`
  result.
- The widget's Refresh button triggered a second real capture through the
  host (108 → 105 packets).
- The devices and doctor views rendered, in dark and light themes. The only
  console error was the test host's own missing favicon.

**Security probes** against the live HTTP server:
- no token: 401;
- wrong token: 401;
- foreign Origin: 403;
- GET: 405;
- `ruview_node_flash` with `hardware-write` granted in the environment:
  `authority_denied`.

**Tests.**
- `npm test`: 160 pass, 1 skip.
- `test:security`: 83 pass, 1 skip.
- New suites: `mcp-apps.test.mjs` (protocol, metadata, widget safety, HTTP
  auth/origin/size/write denial) and `cli-ui.test.mjs` (renderers, colour
  rules, the starvation detector run against a stand-in serial module).

**Tarball smoke** from a clean install:
- CLI and SDK work;
- stdio MCP negotiates `2025-06-18` and serves the widget and structured
  results;
- HTTP MCP lists 22 tools, 3 with `ui://` metadata.

## Consequences

- MCP Apps hosts and ChatGPT render node streams, device scans and
  diagnostics instead of raw JSON, and can refresh them in place.
- Remote MCP clients can use the read tools over authenticated HTTP. Write
  tools remain local by construction.
- Each tool result costs fewer model tokens.
- The widget is a second place where result shapes are interpreted. Changes
  to the capture, devices or doctor result shapes must update it, and its
  version suffix (`console-v1`) changes when the contract does.
- Host behaviour for MCP Apps and ChatGPT keys is followed as documented by
  the reference implementations. A live ChatGPT connector session (public
  HTTPS URL) has not been run yet: CLAIMED, pending.
