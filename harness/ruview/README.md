# `npx @ruvnet/ruview` — RuView WiFi-sensing operator harness

An AI agent harness that knows how to operate **RuView** (WiFi-DensePose): onboard a
newcomer, provision an ESP32 CSI node, calibrate a room, train pose models, and —
crucially — **refuse to overstate accuracy**. Minted from the RuView monorepo via
[`metaharness`](https://www.npmjs.com/package/metaharness) and hardened per **ADR-182**.

WiFi sensing infers *coarse* pose/presence/breathing from Channel State Information.
It is **not a camera**. Every accuracy number this harness emits must be MEASURED
against a baseline — that rule is enforced in code (`ruview_claim_check`).

## Quick start

The source candidate adds `--variant c5` for ESP32-C5 bundles. It verifies
checksums and the attached chip before writing, and uses C5's `0x2000`
bootloader address rather than S3/C6's `0x0`. C5 writes require esptool >=5.3
for corrected security-layout handling; use an isolated active Python environment.
The [C5 record](../../docs/validation/2026-10-28-esp32-c5-candidate.md) contains
bounded real-board boot and CSI evidence, including AP-dependent cadence limits.
Capture a real boot and sustained CSI run for each installation. Both flash
validation and the SDK monitor reject watchdog/flash-busy evidence even if
initial CSI callbacks were present. C5 skips unconfigured mmWave probing to
avoid assigning the S3 sensor pins onto C5's flash/PSRAM bus.

```bash
npx @ruvnet/ruview                       # onboard — pick a setup path
npx @ruvnet/ruview claim-check --file REPORT.md   # the honesty guardrail (non-zero exit on untagged claims)
npx @ruvnet/ruview verify                # run the deterministic proof (VERDICT: PASS)
npx @ruvnet/ruview doctor                # structured diagnostics with fixes (ADR-372)
npx @ruvnet/ruview ports                 # find the ESP32 serial port
npx @ruvnet/ruview flash --port COM7 --bundle ./esp32-csi-node-s3-8mb   # plan only
npx @ruvnet/ruview train-plan --mode pose-smoke
npx @ruvnet/ruview train-gate --file eval-report.json
npx @ruvnet/ruview devices               # ESP32 / mmWave / LiDAR on this machine
npx @ruvnet/ruview hosts run --host lab-pi --tool ruview_devices_scan
npx @ruvnet/ruview guidance --topic homecore --query "Wasmtime plugins"
npx @ruvnet/ruview spaces --resource spaces
npx @ruvnet/ruview spaces --resource events --limit 25
npx @ruvnet/ruview --help
```

The operator tools are pure Node and the published package has no runtime
dependencies (ADR-263 O3). MetaHarness, Darwin and Flywheel are exact-pinned
development dependencies used only for scoring, evolution proposals and
replay verification.

## Tools (`ruview_*`)

Exposed both as CLI verbs and as an MCP server (`npx @ruvnet/ruview mcp start`):

| Tool | What it does |
|------|--------------|
| `ruview_onboard` | Pick docker-demo / repo-build / live-esp32; print the next command |
| `ruview_claim_check` | Lint text for untagged / overstated accuracy claims (guardrail) |
| `ruview_verify` | Run `verify.py` deterministic proof → VERDICT |
| `ruview_node_monitor` | Assert CSI is flowing on an ESP32 (read-only) |
| `ruview_calibrate` | ADR-151 room pipeline (baseline→enroll→train-room→room-watch) |
| `ruview_node_flash` | Cross-platform esptool flash with checksum + chip preflight and boot-log evidence (mutating, guarded) |
| `ruview_firmware_plan` | Verify a firmware bundle and return the exact esptool command (read-only) |
| `ruview_firmware_ports` | Enumerate serial ports via pyserial (read-only) |
| `ruview_doctor` | Structured diagnostics: runtime, integrity, hosts, repo, Rust, Python/esptool, bundles, serial, kernel |
| `ruview_train` | Run pose-smoke / pose / room training from the checkout (writes checkpoints, guarded) |
| `ruview_train_plan` | Resolve a training command without running it (read-only) |
| `ruview_train_gate` | Mean-pose-baseline + leakage gate that returns the only publishable claim wording |
| `ruview_devices_scan` / `ruview_esp32_capture` / `ruview_mmwave_read` / `ruview_lidar_read` | Host device access: USB discovery, ESP32 UDP stream, 60/24 GHz mmWave, RPLIDAR/iPhone LiDAR (`device-access` grant) |
| `ruview_host_list` / `ruview_host_run` | SSH remote hosts, read-only tools only (`remote-host` grant) |
| `ruview_guidance` | Source-cited code map, capability maturity, validation commands, and limitations |
| `ruview_spaces_list` | OAuth-only paging for sites/buildings/floors/spaces/zones/entities/events/alerts (guarded over MCP) |
| `ruview_memory_search` | Search the reviewed, source-cited contributor brain |
| `ruview_kernel_selftest` | SYNTHETIC self-test of the optional `@ruvnet/ruview-kernel` WASM/napi-rs compute package (ADR-368) |

Every tool is **fail-closed**: missing repo / python / binary / port → an honest
negative, never a fabricated success.

### MCP authority model

Read-only tools need no grant. Over MCP, `ruview_calibrate` and `ruview_train`
need the `workspace-write` grant plus `confirm: true`; `ruview_node_flash`
needs `hardware-write` plus `confirm: true`; `ruview_spaces_list` needs
`credential-use`. The plan tools (`ruview_firmware_plan`, `ruview_train_plan`)
cannot take `confirm`, so a read-only client can review exactly what would run.
The MCP doctor cannot probe a board or contact a network host; those are
CLI/SDK options (`--probe`, `--url`).

## Device and host access (ADR-373, ADR-374)

Run these on the machine the hardware is plugged into. A cloud agent cannot
reach your USB ports; use a local session or an SSH host.

```bash
npx @ruvnet/ruview devices                                   # classify USB serial devices by VID:PID
npx @ruvnet/ruview esp32 --seconds 10                         # ESP32 / Realtek RAC1 node UDP stream (default :5005)
npx @ruvnet/ruview esp32 --seconds 45 --analyze               # ...and run live CSI through @ruvnet/ruview-kernel
npx @ruvnet/ruview monitor --port COM10 --baud 1500000        # serial console without resetting the node
npx @ruvnet/ruview mmwave --port /dev/ttyUSB0                 # MR60BHA2 60 GHz / LD2410 24 GHz, auto-detected
npx @ruvnet/ruview lidar --source rplidar --port /dev/ttyUSB1 # RPLIDAR scan summary
RUVIEW_LIDAR_TOKEN=… npx @ruvnet/ruview lidar --source iphone --url ws://HOST:8787/ws/lidar
```

| Tool | Access | MCP grant |
|---|---|---|
| `ruview_devices_scan` | pyserial port list + VID:PID roles (opens no port) | `device-access` |
| `ruview_esp32_capture` | receive-only UDP; CSI/vitals/feature packets per node, loss, RSSI | `device-access` |
| `ruview_mmwave_read` | serial; firmware-identical MR60BHA2/LD2410 parsers | `device-access` |
| `ruview_lidar_read` | RPLIDAR SCAN over serial, or iPhone relay WebSocket (stats only) | `device-access` |
| `ruview_host_list` | configured SSH hosts | none |
| `ruview_host_run` | one read-only tool on an SSH host | `remote-host` + the tool's grant |

Remote hosts live in `~/.config/ruview/hosts.json` (mode 0600) and are added
only from the CLI:

```bash
npx @ruvnet/ruview hosts add --name lab-pi --ssh ruv@lab-pi.local
ssh ruv@lab-pi.local true      # pin the host key once; BatchMode + StrictHostKeyChecking are enforced
npx @ruvnet/ruview hosts run --host lab-pi --tool ruview_esp32_capture --args-json '{"seconds":10}'
```

The remote side runs `npx -y @ruvnet/ruview@<this version> call <tool>
--read-only`. Every argument is validated against the tool schema on both
ends and single-quoted for the remote shell. Flash, train, calibrate,
credentialed reads, and host hopping are never forwarded.

## Firmware flashing (ADR-370)

Works on Windows, macOS, and Linux through `python -m esptool` (`pip install
esptool pyserial`). The bundle directory is an extracted release flash bundle,
`firmware/esp32-csi-node/release_bins/<variant>`, or an ESP-IDF `build/`.

```bash
npx @ruvnet/ruview ports
npx @ruvnet/ruview flash-plan --port /dev/ttyUSB0 --bundle ./bundle --variant s3-8mb
npx @ruvnet/ruview flash --port /dev/ttyUSB0 --bundle ./bundle --variant s3-8mb --confirm
```

Before writing, every image is checked against `SHA256SUMS(.txt)` (a mismatch
is always refused), and `esptool chip_id` must report the chip the variant
targets. After writing, a 15 s serial capture is summarized. The result sets
`hardwareValidated: true` only when that log shows CSI callbacks without a
panic. A successful write alone is not hardware validation. WiFi provisioning
stays with `provision.py` (see the `provision-node` skill) so credentials never
pass through the harness.

## Training (ADR-371)

```bash
npx @ruvnet/ruview train-plan --mode pose-smoke        # review the command
npx @ruvnet/ruview train --mode pose-smoke --confirm   # SYNTHETIC smoke run (needs libtorch)
npx @ruvnet/ruview train --mode pose --data-dir data/mmfi --confirm
npx @ruvnet/ruview train --mode room --enrollment enrollment.json --confirm
npx @ruvnet/ruview train-gate --file eval-report.json  # PASS/FAIL + publishable wording
```

Pose training builds `wifi-densepose-train` with `tch-backend`. It needs
libtorch: tch 0.24 expects torch 2.11.0, via `LIBTORCH` or
`LIBTORCH_USE_PYTORCH=1`. Checkpoints and trainer logs go under
`v2/target/ruview-train/<mode>` unless `--checkpoint-dir` names another path
inside the checkout. The gate refuses any score that has no mean-pose baseline,
uses a random-frame split, shares subjects across a grouped-subject split,
overlaps in time, or fails to beat the baseline.

## Debugging doctor (ADR-372)

```bash
npx @ruvnet/ruview doctor                          # all groups, human output
npx @ruvnet/ruview doctor --json --group python,serial,firmware
npx @ruvnet/ruview doctor --port COM7 --probe      # esptool chip_id (resets the board, writes nothing)
npx @ruvnet/ruview doctor --url http://localhost:3000   # sensing-server /health
```

Each check is `pass`, `warn`, `fail`, or `skip`, and every non-passing check
carries a `fix:`. The exit code is non-zero only for failures.

## SDK (ADR-369)

```js
import { createRuView } from '@ruvnet/ruview/sdk';   // typed: src/sdk.d.ts

const ruview = createRuView();                  // { strict: true } throws RuViewError on ok:false
const report = await ruview.doctor({ groups: ['python', 'serial'] });
const plan = await ruview.firmware.plan({ port: 'COM7', bundle: './bundle', variant: 'c6' });
const flashed = await ruview.firmware.flash({ port: 'COM7', bundle: './bundle', variant: 'c6', confirm: true });
const gate = await ruview.training.gate({ model_score: 0.595, baseline_score: 0.501, split: 'chronological',
  train_end: '2026-03-01', test_start: '2026-03-02', n_test: 800, reproducer: 'python eval.py' });
const kernel = await ruview.kernel.load();      // optional @ruvnet/ruview-kernel
const radar = await ruview.devices.mmwave({ port: '/dev/ttyUSB0', model: 'auto' });
const nodes = await ruview.devices.esp32({ seconds: 10 });
const remote = await ruview.hosts.run('lab-pi', 'ruview_lidar_read', { source: 'rplidar', port: '/dev/ttyUSB1' });
```

The SDK calls the same policy-checked registry as the CLI and MCP server.
Mutating calls still need `confirm: true`.

### Cognitum Spaces OAuth

Activate the additional read scope through the Rust CLI, then use the same
validated client through the metaharness:

```bash
wifi-densepose login --spaces
wifi-densepose whoami
npx @ruvnet/ruview spaces
npx @ruvnet/ruview spaces --resource sites --limit 50
npx @ruvnet/ruview spaces --resource events --cursor '<opaque-next-cursor>'
```

The metaharness never accepts a bearer token or API key and removes
`COGNITUM_SPACES_API` from the child environment, so this surface cannot
silently fall back to the compatibility API-key path. The API origin is fixed
to `https://api.cognitum.one`, and the credentialed adapter requires an
installed `wifi-densepose` binary rather than running Cargo build scripts from
an auto-detected checkout. It returns only the bounded P2/P3 semantic
projection. `--resource` selects one of `sites`, `buildings`, `floors`,
`spaces`, `zones`, `entities`, `events`, or `alerts`; `--limit` is 1–100 and
`--cursor` is the opaque value from the prior page. An empty list is a valid
authenticated result, not sensing-quality evidence. An expired session may
rotate the stored refresh credential before the read completes.

MCP use is denied unless the server operator starts it with
`RUVIEW_MCP_GRANTS=credential-use`. Set `RUVIEW_CREDENTIALS_PATH` in the MCP
server environment when a non-default store is needed; MCP calls cannot choose
an arbitrary credential file or URL. `spaces:read` grants no write, pairing,
command, policy-approval, spending, or actuator authority. See the bundled
`cognitum-spaces` skill for the full playbook.

### Codebase guidance

`ruview_guidance` is the read-only starting point for unfamiliar work. Filter
by `architecture`, `sensing`, `hardware`, `training`, `homecore`,
`integrations`, `deployment`, `community`, or `testing`, and optionally add a
free-text query:

```bash
npx @ruvnet/ruview guidance --topic sensing --query "UDP CSI ingestion"
npx @ruvnet/ruview guidance --topic homecore --query "restore migration voice"
```

Each result separates implementation maturity from evidence, cites current
repository paths, names focused validation commands, and states known
limitations. In a RuView checkout, cited paths are checked before the result
passes. Outside a checkout, the tool labels them as a reviewed packaged
catalog. Related shared-brain records are bounded, reviewed, and treated only
as evidence.

## Skills

Host-neutral playbooks in `skills/` (`onboard`, `provision-node`, `calibrate-room`,
`train-pose`, `verify`, `cognitum-spaces`). `npx @ruvnet/ruview skill <name>`
prints one.

## Use as a Claude Code MCP server

The bundled `.claude/settings.json` registers the `ruview` MCP server
(`npx -y @ruvnet/ruview mcp start`). Drop this package's `.claude/` into a repo, or run
`npx @ruvnet/ruview install --host claude-code`.

## Claude Code mod: live sensing pane (ADR-377)

This package ships **`ruview-live`**, a Claude Code mod: a plugin whose behaviour is a function-hooks module. `/ruview` opens a pane beside the transcript, refreshed on a timer, with:
- your CSI nodes (ESP32, Realtek): rate, loss, RSSI and CSI shape;
- an optional ESPHome radar kit: presence, distance, and device-reported heart/breathing;
- alerts, such as a board that sends heartbeats but no CSI.

It also keeps a one-line status: `RuView · 2 nodes · radar present · 1 alert`.

**Showcase views (ADR-378).** The pane has three views, switched with keys `1`, `2` and `3`. The two live views are drawn in terminal cells (24-bit colour) and animated in place:

| Key | View | What you see |
|---|---|---|
| `1` | Overview | the cards above |
| `2` | CSI waterfall | per-subcarrier amplitude over time (`esp32 --spectrum`), replayed at the frames' arrival rate and labelled **MEASURED** or **SYNTHETIC** from the packets' own flag; `n` cycles nodes |
| `3` | Radar | a 120° range fan with a sonar ping out to the measured distance; heart and breathing charts, and a ♥ and breathing gauge that pulse at the device-reported rates (a metronome, not a waveform) |

The radar kit reports range, not bearing, so the arc covers every bearing at that range. Desktop and IDE surfaces show the overview, and a note in the live views.

```bash
npx @ruvnet/ruview mod                    # where the mod is in this install, and how to load it
claude --plugin-dir "<path printed above>"   # try it for one session
# or from the RuView marketplace:
#   /plugin marketplace add ruvnet/RuView
#   /plugin install ruview-live@ruview
```

In Claude Code:

| Command | What it does |
|---|---|
| `/ruview` | Open or close the pane |
| `/ruview refresh` | Run one capture now; the result goes in the status line |
| `/ruview off` | Close the pane |
| `/ruview waterfall`, `/ruview radar` | Open the pane on that view |

**Settings** (`claude plugin configure ruview-live`):
- `udpPort` (default 5005);
- `radarHost`, an ESPHome kit on your private network;
- `refreshSeconds` (default 15, minimum 5);
- `liveRefreshSeconds` for the waterfall and radar views (default 4, range 2–60);
- `captureSeconds` (default 3).

**How it reads:**
- The mod only runs this package's CLI, for read-only `esp32` and `mmwave --source esphome` captures, with `--json`, so the pane shows exactly what those tools return.
- It never flashes, provisions or writes to a device.
- Like every mod, it runs inside Claude Code with Claude Code's access, so install it only from a source you trust.

Mods are early access in Claude Code. If `/ruview` is missing, start Claude Code with `CLAUDE_CODE_ENABLE_FUNCTION_HOOKS=1`.

## ChatGPT and MCP Apps (ADR-375)

`ruview_esp32_capture`, `ruview_devices_scan` and `ruview_doctor` render in a
self-contained console widget (`ui://ruview/console-v1.html`) in ChatGPT and
other MCP Apps hosts, with a Refresh button that re-runs the tool. Results also
carry `structuredContent`.

```bash
# MCP over HTTP: loopback, token-authenticated, read tools only
export RUVIEW_MCP_TOKEN="$(node -e "console.log(require('crypto').randomBytes(24).toString('base64url'))")"
RUVIEW_MCP_GRANTS=device-access npx @ruvnet/ruview mcp start --http --port 8790
```

Clients that can send headers use `Authorization: Bearer <token>` on
`http://127.0.0.1:8790/mcp`. ChatGPT connectors cannot, so expose the port over
HTTPS (a tunnel or reverse proxy) and add `https://<host>/mcp/<token>` as the
connector URL. Write grants (`hardware-write`, `workspace-write`) are never
honoured over HTTP: flashing and calibration stay on the CLI and stdio. Malformed request targets return HTTP 400
with `invalid_request_target`; the transport continues serving subsequent requests.

In a terminal, commands print a formatted view; pipes and `--json` print JSON.
`ruview esp32 --watch` redraws live capture windows with per-node trends.

## Hosts

Claude Code and Codex are implemented directly and tested with the local,
non-interactive CLIs:

```bash
npx @ruvnet/ruview agent run --host claude-code --repo . --prompt "Map the sensing-server startup path"
npx @ruvnet/ruview agent run --host codex --repo . --prompt "Find the nearest tests for HomeCore restore state"
```

Prompts travel over stdin, never through a shell. Both adapters are read-only by
default (`claude -p --safe-mode` in plan mode; `codex exec` in its read-only
sandbox with user config and exec rules ignored), use a scrubbed environment,
bound output/time, redact secrets, and require a trusted RuView checkout.
Workspace writes require both `--allow-write` and `--confirm`; dangerous bypass
flags are never emitted.

## Shared contributor brain

The committed `brain/corpus/core.jsonl` is a small, reviewable source of
repository facts. Every record has a source citation, evidence tier, tags, and
review state:

```bash
npx @ruvnet/ruview brain search --query "darwin community memory"
npx @ruvnet/ruview brain verify --repo .
npx @ruvnet/ruview brain propose --id finding-id --title "Finding" \
  --content "Source-bound observation" --sourcePath README.md --sourceLine 1 \
  --tags onboarding,docs --contributor github-user
```

Proposals are unreviewed JSONL for a normal pull request. Local vector indexes,
private overlays, raw agent transcripts, CSI/person data, and credentials are
never part of the shared corpus. Retrieved text is quoted evidence, not an
instruction or authority grant.

## Ruflo + Darwin/Flywheel

Development tooling is exact-pinned in `devDependencies`: `metaharness@0.4.1`,
`@metaharness/darwin@0.8.0`, and `@metaharness/flywheel@0.1.7`. Ruflo remains an
optional contributor coordinator rather than cold-start weight for the
dependency-free published MCP server:

```bash
claude mcp add --scope project ruflo -- npx -y ruflo@3.32.26 mcp start
codex mcp add ruflo -- npx -y ruflo@3.32.26 mcp start
```

`npm run flywheel:plan` is read-only. Darwin execution is human-triggered with
`node flywheel/run.mjs --confirm`; it writes only an untrusted
`.metaharness/` proposal archive. The protected gate requires frozen-anchor
retention, holdout lift, security and legacy-test success, verified provenance,
and human approval. No contributor run can directly replace or publish the
champion.

## License

MIT © ruvnet
