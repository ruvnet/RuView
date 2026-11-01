# ruview

**RuView is an ambient intelligence platform, not a sensor.** It turns the radio
signals already in a room (WiFi channel state from ESP32 and Realtek nodes,
60/24 GHz mmWave radar, LiDAR) into presence, motion, breathing and pose. All
of it is camera-free and reachable from the tools people and agents already use.

The sensing stack is ahead of packaged commercial options. The bottleneck has
been packaging. This package is RuView in one install: one CLI, one MCP
server, and a live console that renders inside ChatGPT and other MCP Apps hosts.

```bash
npx ruview --help
npx ruview capabilities          # every component, version and tool in this install
```

## What you get

| Component | Package | What it does |
|---|---|---|
| Operator harness | [`@ruvnet/ruview`](https://www.npmjs.com/package/@ruvnet/ruview) | Node capture and diagnostics for ESP32 and Realtek RTL8721Dx CSI, 60/24 GHz radar (raw UART or ESPHome kits such as the Seeed MR60BHA2), RPLIDAR and iPhone LiDAR, firmware flashing with boot evidence, training with a baseline/leakage gate, doctor, remote hosts over SSH |
| Homecore metaharness | [`homecore`](https://www.npmjs.com/package/homecore) | Source-cited guidance for the Homecore runtime, WASM kernel status, reviewed memory |
| Vitals kernel | [`@ruvnet/ruview-kernel`](https://www.npmjs.com/package/@ruvnet/ruview-kernel) | The Rust breathing/heart-rate pipeline as zero-import WebAssembly; `ruview esp32 --analyze` runs live CSI through it |

## Quick start

```bash
# What is plugged in? (USB VID:PID → ESP32, Realtek, radar, LiDAR)
npx ruview devices

# Live CSI from your nodes (UDP :5005), redrawn every few seconds
npx ruview esp32 --watch

# A 60 GHz radar kit running ESPHome, on your LAN
npx ruview mmwave --source esphome --host 192.168.1.50

# Diagnostics with fixes
npx ruview doctor

# Homecore and the kernel through the same CLI
npx ruview homecore guidance --topic plugins --query Wasmtime
npx ruview kernel selftest
```

A terminal gets a formatted view; pipes and `--json` get JSON.

In **Claude Code**, the bundled `ruview-live` mod puts a live sensing pane beside the conversation (`/ruview`). Run `npx ruview mod` to see how to load it (mods are early access).

## Agents: MCP, ChatGPT and MCP Apps

One server exposes the harness tools (`ruview_*`) and the Homecore tools
(`homecore_*`):

```bash
npx ruview mcp start                       # stdio (Claude Code, Codex, any MCP client)
```

Node captures, radar reads, device scans and doctor results render in the
**RuView console** (`ui://ruview/console-v1.html`) in ChatGPT and other MCP Apps
hosts. A Refresh button re-runs the tool in place.

For ChatGPT and other remote clients, serve MCP over HTTP:

```bash
export RUVIEW_MCP_TOKEN="$(node -e "console.log(require('crypto').randomBytes(24).toString('base64url'))")"
RUVIEW_MCP_GRANTS=device-access npx ruview mcp start --http --port 8790
# then expose it over HTTPS (tunnel / reverse proxy) and add the connector URL
#   https://<your-host>/mcp/<token>
```

**Least authority by default.**
- Hardware reads need the `device-access` grant.
- The HTTP transport requires a token.
- It binds to loopback.
- It refuses unknown browser origins.
- It **never** honours write grants: flashing and calibration stay on the
  local CLI.

## Honesty rules

- **Device-reported vitals** (from radar firmware) are labelled as such and
  are not validated against a reference.
- **Kernel estimates** carry "no reference measurement".
- **Synthetic data** (simulators, self-tests) is flagged `SYNTHETIC`.
- **Accuracy claims** must pass `ruview claim-check`, tagged MEASURED (with a
  reproducer), CLAIMED or SYNTHETIC.

WiFi sensing is not camera-grade, and RuView never presents it that way.

## Privacy

RuView senses without cameras, and nothing leaves your network unless you send
it. The MCP HTTP transport binds to localhost unless you choose otherwise, and
the console widget makes no network requests.

## Links

- Source and architecture decisions: <https://github.com/ruvnet/RuView>
  (ADR-373 device access, ADR-375 MCP Apps console/HTTP/terminal UI, ADR-376 this package)
- Issues: <https://github.com/ruvnet/RuView/issues>

MIT licensed.
