# RuView Desktop

> **Work in Progress** — This crate is under active development. APIs and UI are subject to change.

Cross-platform desktop application for managing ESP32 WiFi sensing networks. Built with **Tauri v2** (Rust backend) and **React + TypeScript** (frontend), following the [ADR-053 design system](../../docs/adr/ADR-053-ui-design-system.md).

## Overview

RuView Desktop provides a unified interface for node discovery, firmware management, over-the-air updates, WASM edge module deployment, real-time sensing data visualization, and mesh network topology monitoring — all from a single native application.

## Pages

| Page | Description | Status |
|------|-------------|--------|
| **Dashboard** | System overview with live stat cards, server panel, quick actions, and node grid | Done |
| **Nodes** | Sortable table of discovered ESP32 nodes with expandable detail rows | Done |
| **Flash** | 3-step serial firmware flash wizard (select port, pick firmware, flash + verify) | Done |
| **OTA Update** | Single-node and batch over-the-air firmware updates with strategy selection | Done |
| **Edge Modules** | WASM module upload, lifecycle management (start/stop/unload) per node | Done |
| **Sensing** | Server start/stop, live log viewer (pause/clear), activity feed with confidence bars | Done |
| **Mesh View** | Force-directed canvas graph showing mesh topology with click-to-inspect nodes | Done |
| **Settings** | Server configuration (ports, bind address, discovery interval, theme) | Done |

## Architecture

```
wifi-densepose-desktop/
├── src/
│   ├── main.rs              # Tauri app entry point
│   ├── lib.rs               # Command registration
│   ├── commands/            # Tauri IPC command handlers
│   │   ├── discovery.rs     # Node discovery (mDNS/UDP probe)
│   │   ├── flash.rs         # Serial firmware flashing
│   │   ├── ota.rs           # OTA update (single + batch)
│   │   ├── wasm.rs          # WASM module management
│   │   └── server.rs        # Sensing server lifecycle
│   └── domain/              # DDD domain models
│       ├── node.rs           # DiscoveredNode, NodeRegistry, HealthStatus
│       └── config.rs         # ProvisioningConfig with validation
├── ui/                       # React + TypeScript frontend
│   ├── src/
│   │   ├── App.tsx           # Shell with sidebar nav, live status bar
│   │   ├── design-system.css # ADR-053 design tokens and components
│   │   ├── types.ts          # TypeScript types mirroring Rust domain
│   │   ├── components/       # Shared UI components (StatusBadge, NodeCard)
│   │   ├── hooks/            # React hooks (useServer, useNodes)
│   │   └── pages/            # 8 page components
│   └── index.html
└── tauri.conf.json           # Tauri v2 configuration
```

## Tauri Commands

| Group | Command | Description |
|-------|---------|-------------|
| **Discovery** | `discover_nodes` | Scan network for ESP32 nodes via mDNS/UDP |
| **Flash** | `list_serial_ports` | List available serial ports |
| | `detect_chip` | Detect connected chip type |
| | `start_flash` | Flash firmware via serial |
| **OTA** | `ota_update` | Push firmware to a single node |
| | `batch_ota_update` | Push firmware to multiple nodes |
| **WASM** | `wasm_list` | List loaded WASM modules on a node |
| | `wasm_upload` | Upload a .wasm module to a node |
| | `wasm_control` | Start/stop/unload a WASM module |
| **Server** | `start_server` | Start the sensing HTTP/WS server |
| | `stop_server` | Stop the sensing server |
| | `server_status` | Get current server status |
| **Provision** | `get_provision_config` | Read provisioning configuration |
| | `save_provision_config` | Save provisioning configuration |

## Design System (ADR-053)

The UI follows a dark professional theme with the following design tokens:

| Token | Value | Usage |
|-------|-------|-------|
| `--bg-base` | `#0d1117` | Main background |
| `--bg-surface` | `#161b22` | Cards, sidebar, panels |
| `--bg-elevated` | `#1c2333` | Elevated elements |
| `--accent` | `#7c3aed` | Primary accent (purple) |
| `--status-online` | `#3fb950` | Online/success indicators |
| `--status-error` | `#f85149` | Error/offline indicators |
| `--font-mono` | JetBrains Mono | Technical data, code |
| `--font-sans` | Inter | UI text, labels |

### UI Features

- **Glassmorphism cards** with `backdrop-filter: blur(12px)`
- **Count-up animations** on dashboard stat numbers
- **Page transitions** with fade-in + scale on navigation
- **Gradient accents** on logo, nav indicator, primary buttons
- **Status dot glows** with ambient `box-shadow` per health state
- **Staggered fade-ins** for card grids
- **Force-directed graph** for mesh topology (pure Canvas 2D)

## Download

Pre-built binaries are available on the [Releases](https://github.com/ruvnet/RuView/releases) page.

| Platform | Download | Status |
|----------|----------|--------|
| Windows x64 | [v0.3.0-alpha](https://github.com/ruvnet/RuView/releases/tag/v0.3.0-desktop-alpha) | Debug build |
| macOS | — | Planned |
| Linux | — | Planned |

### Running the pre-built exe (Windows)

The current release is a **debug build** that loads the frontend from a local Vite dev server. Follow these steps:

```bash
# 1. Clone the repo (or download just the ui/ folder)
git clone https://github.com/ruvnet/RuView.git
cd RuView/rust-port/wifi-densepose-rs/crates/wifi-densepose-desktop/ui

# 2. Install frontend dependencies
npm install

# 3. Start the Vite dev server
npx vite --host

# 4. Download and run the exe from the release page
#    (or run from the repo if you built it locally)
#    The app window will open and connect to localhost:5173
```

> **Requirements:** Windows 10 (1803+) or Windows 11. WebView2 runtime is required (pre-installed on Windows 10 1803+ and all Windows 11).

> **Note:** Production builds will bundle the frontend assets directly into the exe, removing the need for a dev server.

## Build from Source

### Prerequisites

- [Rust 1.85+](https://rustup.rs/)
- [Node.js 20+](https://nodejs.org/)
- [Tauri v2 CLI](https://v2.tauri.app/start/prerequisites/)
- **Windows:** MSVC build tools + MinGW-w64 (for `dlltool`)
- **macOS:** Xcode Command Line Tools
- **Linux:** `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`

### Development mode

```bash
# Install frontend dependencies
cd ui && npm install

# Start in dev mode (hot-reload on both Rust and React)
cargo tauri dev
```

### Production build

```bash
# Build optimized release with bundled frontend
cargo tauri build
```

The installer/bundle will be in `target/release/bundle/` (`.msi` on Windows, `.dmg` on macOS, `.deb`/`.AppImage` on Linux).

## Domain Types

| Type | Fields | Description |
|------|--------|-------------|
| `Node` | ip, mac, hostname, node_id, firmware_version, chip, mesh_role, health, ... | Full node record |
| `HealthStatus` | online, offline, degraded, unknown | Node health state |
| `FlashSession` | port, firmware, chip, baud, progress | Active flash operation |
| `OtaResult` | node_ip, success, previous_version, new_version, duration_ms | OTA outcome |
| `WasmModule` | module_id, name, size_bytes, state, node_ip | Edge module record |
| `ServerStatus` | running, pid, http_port, ws_port | Sensing server state |
| `SensingUpdate` | timestamp, node_id, subcarrier_count, rssi, activity, confidence | Real-time data |

## License

MIT — see [LICENSE](../../LICENSE) for details.
