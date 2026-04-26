# wifi-densepose-wasm

[![Crates.io](https://img.shields.io/crates/v/wifi-densepose-wasm.svg)](https://crates.io/crates/wifi-densepose-wasm)
[![Documentation](https://docs.rs/wifi-densepose-wasm/badge.svg)](https://docs.rs/wifi-densepose-wasm)
[![License](https://img.shields.io/crates/l/wifi-densepose-wasm.svg)](LICENSE)

WebAssembly bindings for running WiFi-DensePose directly in the browser.

## Overview

`wifi-densepose-wasm` compiles the WiFi-DensePose stack to `wasm32-unknown-unknown` and exposes a
JavaScript API via [wasm-bindgen](https://rustwasm.github.io/wasm-bindgen/). The primary export is
`MatDashboard` -- a fully client-side disaster response dashboard that manages scan zones, tracks
survivors, generates triage alerts, and renders to an HTML Canvas element.

The crate also provides utility functions (`init`, `getVersion`, `isMatEnabled`, `getTimestamp`) and
a logging bridge that routes Rust `log` output to the browser console.

## Features

- **MatDashboard** -- Create disaster events, add rectangular and circular scan zones, subscribe to
  survivor-detected and alert-generated callbacks, and render zone/survivor overlays on Canvas.
- **Real-time callbacks** -- Register JavaScript closures for `onSurvivorDetected` and
  `onAlertGenerated` events, called from the Rust event loop.
- **Canvas rendering** -- Draw zone boundaries, survivor markers (colour-coded by triage status),
  and alert indicators directly to a `CanvasRenderingContext2d`.
- **WebSocket integration** -- Connect to a sensing server for live CSI data via `web-sys` WebSocket
  bindings.
- **Panic hook** -- `console_error_panic_hook` provides human-readable stack traces in the browser
  console on panic.
- **Optimised WASM** -- Release profile uses `-O4` wasm-opt with mutable globals for minimal binary
  size.

### Feature flags

| Flag                       | Default | Description |
|----------------------------|---------|-------------|
| `console_error_panic_hook` | yes     | Better panic messages in the browser console |
| `mat`                      | no      | Enable MAT disaster detection dashboard |

## Quick Start

### Build

```bash
# Build with wasm-pack (recommended)
wasm-pack build --target web --features mat

# Or with cargo directly
cargo build --target wasm32-unknown-unknown --features mat
```

### JavaScript Usage

```javascript
import init, {
  MatDashboard,
  initLogging,
  getVersion,
  isMatEnabled,
} from './wifi_densepose_wasm.js';

async function main() {
  await init();
  initLogging('info');

  console.log('Version:', getVersion());
  console.log('MAT enabled:', isMatEnabled());

  const dashboard = new MatDashboard();

  // Create a disaster event
  const eventId = dashboard.createEvent(
    'earthquake', 37.7749, -122.4194, 'Bay Area Earthquake'
  );

  // Add scan zones
  dashboard.addRectangleZone('Building A', 50, 50, 200, 150);
  dashboard.addCircleZone('Search Area B', 400, 200, 80);

  // Subscribe to real-time events
  dashboard.onSurvivorDetected((survivor) => {
    console.log('Survivor:', survivor);
  });

  dashboard.onAlertGenerated((alert) => {
    console.log('Alert:', alert);
  });

  // Render to canvas
  const canvas = document.getElementById('map');
  const ctx = canvas.getContext('2d');

  function render() {
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    dashboard.renderZones(ctx);
    dashboard.renderSurvivors(ctx);
    requestAnimationFrame(render);
  }
  render();
}

main();
```

## Exported API

| Export | Kind | Description |
|--------|------|-------------|
| `init()` | Function | Initialise the WASM module (called automatically via `wasm_bindgen(start)`) |
| `initLogging(level)` | Function | Set log level: `trace`, `debug`, `info`, `warn`, `error` |
| `getVersion()` | Function | Return the crate version string |
| `isMatEnabled()` | Function | Check whether the MAT feature is compiled in |
| `getTimestamp()` | Function | High-resolution timestamp via `Performance.now()` |
| `MatDashboard` | Class | Disaster response dashboard (zones, survivors, alerts, rendering) |

## Related Crates

| Crate | Role |
|-------|------|
| [`wifi-densepose-mat`](../wifi-densepose-mat) | MAT engine (linked when `mat` feature enabled) |
| [`wifi-densepose-core`](../wifi-densepose-core) | Shared types and traits |
| [`wifi-densepose-cli`](../wifi-densepose-cli) | Terminal-based MAT interface |
| [`wifi-densepose-sensing-server`](../wifi-densepose-sensing-server) | Backend sensing server for WebSocket data |

## License

MIT OR Apache-2.0
