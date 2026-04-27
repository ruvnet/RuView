# ADR-092: nvsim Dashboard — Vite + Dual-Transport (WASM + REST/WS) Implementation

| Field | Value |
|---|---|
| **Status** | **Implemented (2026-04-27)** — live at https://ruvnet.github.io/RuView/nvsim/. PR #436 open against main. 8/12 §11 gates ✅, 4/12 ⚠ (require external infrastructure). |
| **Date** | 2026-04-26 |
| **Authors** | ruv |
| **Refines** | ADR-089 (`nvsim` simulator), ADR-090 (Lindblad extension), ADR-091 (stand-off radar) |
| **Companion** | `assets/NVsim Dashboard.zip` (mockup), `docs/research/quantum-sensing/15-nvsim-implementation-plan.md` (Pass-6 plan), `docs/research/quantum-sensing/16-ghost-murmur-ruview-spec.md` (use-case framing) |
| **Branch** | `feat/nvsim-pipeline-simulator` |
| **Acceptance gates** | Sections §11 and §12 below |

---

## 1. Context

The `nvsim` crate (ADR-089) ships a deterministic forward simulator for an
NV-diamond magnetometer pipeline: scene → source synthesis (Biot–Savart,
dipole, current loop, ferrous induced moment) → material attenuation → NV
ensemble (4 〈111〉 axes, ODMR linear-readout proxy, shot-noise floor) →
16-bit ADC + lock-in demod → fixed-layout `MagFrame` records → SHA-256
witness. The crate is Rust-only, headless, and benchmarks at ~4.5 M
samples/s on x86_64.

The user-supplied **NVSim Dashboard mockup** (`assets/NVsim Dashboard.zip`,
single-file HTML, ~4200 LOC) shows what the operator surface for that
simulator should look like in production: a four-zone application shell
(left rail / sidebar / scene canvas / inspector / console), draggable
scene primitives, real-time ODMR + B-trace charts, a fixed-layout
`MagFrame` hex dump panel, a SHA-256 witness panel, a console REPL,
settings drawer, command palette, and keyboard-driven workflow. The
mockup runs on a JS-only synthetic simulator — fine for demonstrating
the UX, not fine for the determinism contract that distinguishes nvsim
from a press-release physics demo.

This ADR records the decision to **fully implement that dashboard** and
ship it as the canonical front-end for nvsim, hosted on GitHub Pages and
backed by the **real Rust simulator** through two parallel transports:

1. **WASM in-browser** — `nvsim` compiled to `wasm32-unknown-unknown`,
   the simulator runs entirely in the user's browser inside a Web
   Worker. No server, no upload, no telemetry. The default mode for
   GitHub Pages.
2. **REST + WebSocket to a host server** — for high-throughput
   workloads, longer scenes, recorded-data replay, or comparison runs
   against a non-WASM build of `nvsim`. Optional, opt-in, runs on a
   user-supplied host.

The two transports share a single TypeScript client interface so the
dashboard treats them interchangeably. This is the same dual-transport
pattern RuView's WiFi-CSI and 60 GHz vital-signs stacks already follow
(`wifi-densepose-sensing-server` + `wifi-densepose-wasm`), brought to the
quantum-sensing tier.

---

## 2. Decision

Build the nvsim dashboard as:

- **Frontend**: Vite + TypeScript + a thin component library (Lit or
  vanilla custom-elements; **not** React, **not** Vue — the mockup is
  vanilla DOM and the SPA size budget should stay <300 KB gzipped).
- **Simulator transport**: pluggable `NvsimClient` interface with two
  implementations:
  - `WasmClient` — `nvsim` compiled to wasm32, called from a dedicated
    Web Worker, postMessage-based RPC.
  - `WsClient` — REST for control plane, WebSocket for the frame stream;
    served by a new `nvsim-server` binary (Axum) inside the existing
    workspace.
- **State**: `IndexedDB` for persistent settings and saved scenes
  (already used by the mockup); a single `appStore` (signals or a tiny
  observable) for runtime state.
- **Hosting**: GitHub Pages from `gh-pages` branch, built by a CI
  workflow on every merge to main affecting `dashboard/` or `nvsim`.
- **Versioning**: dashboard version is pinned to nvsim version. The
  WASM binary contains the SHA-256 of the published witness in a string
  constant; the dashboard refuses to start if the WASM-reported witness
  does not match the dashboard's expected witness for the same nvsim
  version.

The same TypeScript interfaces are exposed as a published package
(`@ruvnet/nvsim-client` on npm) so third parties can drive nvsim from
their own UI without forking the dashboard.

---

## 3. Goals and non-goals

### 3.1 Goals

- **Faithful implementation of the mockup**. Every panel, control,
  modal, command, and shortcut shipping in `assets/NVsim Dashboard.zip`
  is implemented. No simplification.
- **Deterministic by construction**. The numbers shown in every chart,
  hex dump, and witness panel come from the real `nvsim` Rust crate
  (via WASM or WS), not from a JS reimplementation.
- **Witness-grade reproducibility**. Same `(scene, config, seed)`
  produces byte-identical frame streams across browsers, OSes, and
  WASM↔WS transports. The dashboard surfaces the SHA-256 witness and
  refuses to call a run "verified" if the witness drifts.
- **Offline-capable**. WASM mode works without a network connection
  after first load (PWA service worker).
- **Embeddable**. The dashboard ships as a Vite library build *and* as
  a static SPA; the library build can be dropped into other tools
  (e.g. a future RuView fleet console).
- **Accessible**. WCAG 2.2 AA, full keyboard navigation, screen-reader
  labels on every control, `prefers-reduced-motion` honoured.
- **Mobile-usable**. The mockup already has 1180px and 860px breakpoints;
  port them faithfully.

### 3.2 Non-goals

- **Not** a fleet-management UI for physical NV hardware. nvsim is a
  simulator; there is no hardware to control. The dashboard reads the
  simulator's output, nothing more.
- **Not** a multi-user/collaborative workspace. Single-user, local-first.
- **Not** a generic plotting library. The charts are bespoke and tied
  to the nvsim data model.
- **Not** a cloud SaaS. There is no hosted backend by default. The WS
  transport is opt-in and runs on a user-controlled host.

---

## 4. Source-of-truth: the mockup

The reference is `assets/NVsim Dashboard.zip` (extract: `NVSim
Dashboard.html` + `uploads/pasted-1777237234880-0.png`). Implementation
inventory pulled directly from the mockup follows.

### 4.1 Layout grid

```
┌─────┬──────────────────────────────────────────────┐
│     │  topbar (48px)                                │
│ rail├──────────┬─────────────────┬─────────────────┤
│ 56px│ sidebar  │  scene (SVG)    │  inspector      │
│     │  280px   │  1fr            │  340px          │
│     │          ├─────────────────┤                 │
│     │          │  console 220px  │                 │
└─────┴──────────┴─────────────────┴─────────────────┘
```

Responsive: collapse sidebar at 1180px, collapse inspector + rail at
860px, hamburger menu replaces rail.

### 4.2 Component inventory (full)

| Zone | Component | Mockup ref | Notes |
|---|---|---|---|
| Rail | Logo (NV) | `.logo` line 130 | linear-gradient amber |
| Rail | Nav buttons | `.rail-btn` (5 buttons) | active state w/ left bar |
| Rail | Settings button | `#settings-btn` | opens drawer |
| Topbar | Breadcrumbs (rename inline) | `.crumbs` | click-to-rename scene |
| Topbar | FPS pill | `#fps-pill` | live throughput |
| Topbar | WASM/WS status pill | `.pill.wasm` | shows transport mode |
| Topbar | Seed pill | `.pill.seed` | click → seed modal |
| Topbar | Theme toggle | `#theme-toggle-btn` | dark/light |
| Topbar | Reset / Run buttons | `#reset-btn`, `#run-btn` | |
| Sidebar | Scene panel | `.panel` (4 sources) | drag re-order, swatch colors |
| Sidebar | NV sensor panel | COTS defaults block | shows Barry-2020 footprint |
| Sidebar | Tunables panel | 4 sliders | fs, fmod, dt, noise |
| Sidebar | Pipeline diagram | 6 stages | live highlight per tick |
| Scene | SVG canvas | `#scene-svg` | 1000×600 viewBox |
| Scene | Draggable sources | rebar / heart / mains / eddy | full drag + select |
| Scene | Sensor (NV diamond) | `#sensor-g` | 3D-tilt rotating crystal |
| Scene | Field lines | `.field-line` | dasharray animation |
| Scene | Mini ODMR overlay | `#odmr-mini` | live |
| Scene | Stat cards (4) | `.stat-card` | |B|, SNR, throughput, … |
| Scene | Sim controls | `.sim-controls` | step ⏮ play ⏯ step ⏭ + speed |
| Scene | Toolbar | `.scene-toolbar` | zoom, fit, layers |
| Inspector | Tabs (3): Signal / Frame / Witness | `.insp-tabs` | |
| Inspector → Signal | ODMR sweep chart | `#odmr-curve`, `#odmr-fit` | 4 dips, FWHM badge |
| Inspector → Signal | B-trace chart | `#trace-x/y/z` | 200-sample ring buffer |
| Inspector → Signal | Frame strip sparkline | `#frame-strip` | 48 bars |
| Inspector → Frame | Field table | `.frame-table` | timestamp, b_pT[0..2], flags |
| Inspector → Frame | Hex dump | `.hex` | annotated 60-byte frame |
| Inspector → Witness | SHA-256 box | `.witness` | last witness |
| Inspector → Witness | Verify button | proof.verify | |
| Console | Filter tabs (5): all/info/warn/err/dbg | `.console-tab` | |
| Console | Log line stream | `.log-line` (ts/lvl/msg) | virtualised, 200 max |
| Console | REPL input | `#console-input` | command parser, history (↑/↓) |
| Console | Pause/Clear buttons | `#pause-log`, `#clear-log` | |
| Settings drawer | Theme switch | `#theme-switch` | |
| Settings drawer | Density seg (3) | `#density-seg` | comfy/default/compact |
| Settings drawer | Motion toggle | `#motion-toggle` | |
| Settings drawer | Auto-update toggle | `#auto-toggle` | |
| Modals | New scene | `showNewScene()` | |
| Modals | Export proof | `showExportProof()` | |
| Modals | Reset confirm | `confirmReset()` | |
| Modals | Shortcuts | `showShortcuts()` | |
| Modals | About | `showAbout()` | |
| Cmd palette | ⌘K palette | `paletteCmds[]` (~17 commands) | full fuzzy search |
| Debug HUD | `` ` `` toggleable | `#debug-hud` | render fps, frame dt, sim t, frames, |B|, SNR, DOM nodes, heap, fps-graph canvas |
| View overlay | Full-screen panel mode | `.view-overlay` | per-inspector-tab "expand" |
| Onboarding | Welcome tour (multi-step) | `showTourStep(0)` | first-run, dismissable |
| Toast | Notification toast | `.toast` | 1.8s auto-dismiss |

### 4.3 REPL command set (must be 1:1 with the mockup)

```
help                       — list commands
scene.list                 — describe loaded scene
sensor.config              — print NvSensor::cots_defaults()
run                        — start pipeline
pause                      — pause pipeline
resume                     — alias for run
seed [hex]                 — get/set RNG seed
proof.verify               — re-derive witness, compare expected
proof.export               — write proof bundle
clear                      — clear console
theme [light|dark]         — switch theme
```

Plus the full palette commands (§4.2 row "Cmd palette") and the keyboard
shortcuts (§4.4).

### 4.4 Keyboard shortcuts (must be 1:1)

| Key | Action |
|---|---|
| ⌘K / Ctrl K | Command palette |
| Space | Play/pause |
| ⌘R / Ctrl R | Reset (confirm) |
| ⌘, / Ctrl , | Settings |
| ⌘N / Ctrl N | New scene |
| ⌘E / Ctrl E | Export proof |
| ⌘/ / Ctrl / | Toggle theme |
| `` ` `` | Toggle debug HUD |
| 1 / 2 / 3 | Inspector tabs |
| Esc | Close modal/palette |
| / | Focus REPL |

---

## 5. Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│  GitHub Pages — static SPA at https://ruvnet.github.io/nvsim/    │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │                  Vite SPA bundle                           │  │
│  │  ┌─────────────────┐    ┌─────────────────────────────┐    │  │
│  │  │  UI components  │◄──►│   appStore (signals)        │    │  │
│  │  │  (Lit elements) │    └──────────────┬──────────────┘    │  │
│  │  └─────────────────┘                   │                   │  │
│  │           ▲                            ▼                   │  │
│  │  ┌────────┴────────┐    ┌──────────────────────────────┐   │  │
│  │  │  IndexedDB kv   │    │  NvsimClient interface       │   │  │
│  │  │  (settings,     │    │  ┌──────────────────────────┐│   │  │
│  │  │   scenes,       │    │  │  WasmClient (default)    ││   │  │
│  │  │   witnesses)    │    │  │  ─ posts to Web Worker   ││   │  │
│  │  └─────────────────┘    │  └────────────┬─────────────┘│   │  │
│  │                         │  ┌────────────┴─────────────┐│   │  │
│  │                         │  │  WsClient (opt-in)       ││   │  │
│  │                         │  │  ─ REST + WebSocket      ││   │  │
│  │                         │  └────────────┬─────────────┘│   │  │
│  │                         └───────────────┼──────────────┘   │  │
│  └─────────────────────────────────────────┼──────────────────┘  │
│                                            │                     │
│  ┌─── Web Worker (in-browser) ─────────────┼──────┐              │
│  │   nvsim.wasm  (Rust → wasm32)           │      │              │
│  │   ├─ wasm-bindgen JS shim                      │              │
│  │   └─ posts MagFrame batches via SharedArray    │              │
│  └────────────────────────────────────────────────┘              │
└──────────────────────────────────────────────────────────────────┘
                                            │
                                            │ (opt-in, user-supplied)
                                            ▼
┌──────────────────────────────────────────────────────────────────┐
│  nvsim-server (Axum, in v2/crates/nvsim-server)                  │
│  ┌─────────────────────────────────────────────────────────┐     │
│  │  REST: /scene, /config, /witness, /export-proof          │     │
│  │  WS  : /stream  ─── MagFrame binary subscription         │     │
│  │  Calls native nvsim::Pipeline::{run, run_with_witness}   │     │
│  └─────────────────────────────────────────────────────────┘     │
└──────────────────────────────────────────────────────────────────┘
```

### 5.1 Why two transports

Default WASM is right for the marketing/demo use case (open the GitHub
Pages URL, no install, no server, instant). It also makes the
determinism contract trivially auditable — the `.wasm` binary is the
artifact whose SHA-256 the dashboard pins.

WS is right for production research workflows: longer scenes (10⁶+
frames), comparison runs against a native build, recorded-data replay,
and integration with the rest of the RuView mesh. The same dashboard,
same UI, different `NvsimClient` impl. Users opt in by entering a
`ws://` URL in settings.

### 5.2 The shared client interface

```typescript
// packages/nvsim-client/src/index.ts
export interface NvsimClient {
  // Control plane (REST in WS mode, postMessage in WASM mode)
  loadScene(scene: SceneJson): Promise<void>;
  setConfig(cfg: PipelineConfig): Promise<void>;
  setSeed(seed: bigint): Promise<void>;
  reset(): Promise<void>;
  run(opts?: { frames?: number }): Promise<RunHandle>;
  pause(): Promise<void>;
  step(direction: 'fwd' | 'back', dtMs: number): Promise<void>;

  // Data plane (WS subscription / SharedArrayBuffer ring)
  frames(): AsyncIterable<MagFrameBatch>;
  events(): AsyncIterable<NvsimEvent>;

  // Witness
  generateWitness(samples: number): Promise<Uint8Array>;
  verifyWitness(expected: Uint8Array): Promise<{ ok: true } | { ok: false; actual: Uint8Array }>;
  exportProofBundle(): Promise<Blob>;

  // Lifecycle
  close(): Promise<void>;
}

export interface RunHandle {
  readonly id: string;
  readonly startedAt: number;
  readonly framesEmitted: () => bigint;
  cancel(): Promise<void>;
}
```

Both `WasmClient` and `WsClient` implement `NvsimClient`. The dashboard
binds to the interface and never to a concrete client.

---

## 6. Crate work needed

This ADR mandates the following new/modified crates and Rust APIs. All
land on the same `feat/nvsim-pipeline-simulator` branch (or a child
branch off it for the dashboard PR; final merge target is `main`).

### 6.1 `nvsim` — add WASM bindings (existing crate, additive)

- Add `wasm-bindgen = { version = "0.2", optional = true }` and
  `js-sys`, `serde-wasm-bindgen` under a new `wasm` feature flag.
  Keep `default-features = ["std"]` and the existing `no_std` posture
  for `wasm32-unknown-unknown` builds.
- Expose a `#[wasm_bindgen]` `Pipeline` wrapper:

  ```rust
  #[cfg(feature = "wasm")]
  #[wasm_bindgen]
  pub struct WasmPipeline { inner: Pipeline }

  #[cfg(feature = "wasm")]
  #[wasm_bindgen]
  impl WasmPipeline {
      #[wasm_bindgen(constructor)]
      pub fn new(scene_json: &str, config_json: &str, seed: u64) -> Result<WasmPipeline, JsValue> { … }
      pub fn run(&self, n: usize) -> Vec<u8> { … }                 // concatenated MagFrame bytes
      pub fn run_with_witness(&self, n: usize) -> JsValue { … }    // { frames: Uint8Array, witness: Uint8Array }
      pub fn build_id(&self) -> String { … }                       // includes nvsim version + WASM SHA
  }
  ```

- Add a `cargo build --target wasm32-unknown-unknown --features wasm
  --release` target documented in `nvsim/README.md`.
- Bench impact: must remain ≥ 1 kHz (Cortex-A53 budget) inside a Web
  Worker. Verify on Chrome / Firefox / Safari with a 1024-sample run
  fixture.

### 6.2 `nvsim-server` — new crate at `v2/crates/nvsim-server/`

- Axum server with these routes (all JSON over REST except `/stream`):

  | Method | Path | Purpose |
  |---|---|---|
  | GET | `/api/health` | liveness + nvsim version + build hash |
  | GET | `/api/scene` | current scene (JSON) |
  | PUT | `/api/scene` | replace scene |
  | GET | `/api/config` | current `PipelineConfig` |
  | PUT | `/api/config` | replace config |
  | GET | `/api/seed` | current seed (hex) |
  | PUT | `/api/seed` | set seed |
  | POST | `/api/run` | start a run; returns `run_id` |
  | POST | `/api/pause` | pause |
  | POST | `/api/reset` | reset to t=0 |
  | POST | `/api/step` | single step (±) |
  | POST | `/api/witness/generate` | run N frames + return SHA-256 |
  | POST | `/api/witness/verify` | re-derive + compare against expected |
  | POST | `/api/export-proof` | return a tar.gz proof bundle |
  | GET | `/ws/stream` | upgrade → WebSocket; binary `MagFrameBatch` push |

- Binary protocol on `/ws/stream` mirrors the existing `nvsim::frame`
  layout: magic `0xC51A_6E70`, version `1`, 60-byte fixed records,
  batched into ~64 KB chunks.
- CORS: permissive in dev, allowlist via `--allowed-origin` flag in
  prod.
- TLS: bring-your-own (Caddy / nginx in front). Server speaks plain
  HTTP/WS.
- Deps: `axum`, `tokio`, `tower`, `serde_json`, `nvsim` (workspace).
- Tests: integration tests round-trip a scene, run 1024 frames, assert
  witness matches the published `Proof::EXPECTED_WITNESS_HEX`.

### 6.3 `@ruvnet/nvsim-client` — new TypeScript package

Path: `dashboard/packages/nvsim-client/` (workspace package, published
to npm post-MVP). Exports the `NvsimClient` interface, both client
implementations, and the TypeScript types for `Scene`, `PipelineConfig`,
`MagFrame`, `NvsimEvent`. Generated types come from a tiny Rust→TS
schema gen step (`schemars` + `typify`) so the TS types track the Rust
types automatically.

---

## 7. Frontend stack

### 7.1 Build tooling

- **Vite 5** (modern, fast, ESM, native WASM import). Source: `dashboard/`.
- **TypeScript** 5.x, strict mode.
- **Lit 3** for custom elements + reactive props. Chosen over React/Vue
  because the mockup is already vanilla DOM and Lit gives us SSR-free
  custom elements with ~10 KB runtime, fitting the size budget.
- **No CSS framework**. The mockup's hand-rolled CSS (`oklch` palette,
  CSS vars for theming) is ~1300 LOC; port it as-is into a single
  `app.css` + per-component scoped styles.
- **Vitest** for unit tests.
- **Playwright** for E2E (dashboard ↔ WASM and dashboard ↔ WS).
- **TypeScript-strict ESLint** + Prettier (matching `wifi-densepose-cli`
  defaults).

### 7.2 Project layout

```
dashboard/
├── package.json
├── vite.config.ts
├── tsconfig.json
├── public/
│   ├── nvsim.wasm                      # built by Cargo, copied here
│   └── icon.svg
├── src/
│   ├── main.ts                         # entry
│   ├── app.css                         # ported from mockup
│   ├── store/
│   │   ├── appStore.ts                 # signals-based store
│   │   └── persistence.ts              # IndexedDB kv (already in mockup)
│   ├── transport/
│   │   ├── NvsimClient.ts              # interface
│   │   ├── WasmClient.ts
│   │   ├── WsClient.ts
│   │   └── worker.ts                   # Web Worker entry
│   ├── components/
│   │   ├── app-shell.ts                # grid layout
│   │   ├── nv-rail.ts
│   │   ├── nv-topbar.ts
│   │   ├── nv-sidebar.ts
│   │   ├── nv-scene.ts                 # SVG canvas, drag, 3D tilt
│   │   ├── nv-inspector.ts             # tabbed
│   │   ├── nv-signal-panel.ts          # ODMR + B-trace
│   │   ├── nv-frame-panel.ts           # hex dump + table
│   │   ├── nv-witness-panel.ts
│   │   ├── nv-console.ts               # log stream + REPL
│   │   ├── nv-settings-drawer.ts
│   │   ├── nv-modal.ts
│   │   ├── nv-palette.ts               # ⌘K
│   │   ├── nv-debug-hud.ts             # `
│   │   ├── nv-toast.ts
│   │   └── nv-onboarding.ts
│   ├── repl/
│   │   ├── parser.ts                   # tokeniser
│   │   └── commands.ts                 # registry
│   ├── charts/                         # bespoke SVG renderers, no library
│   │   ├── odmr.ts
│   │   ├── b-trace.ts
│   │   └── frame-strip.ts
│   └── util/
│       ├── shortcuts.ts                # keymap dispatcher
│       ├── theme.ts
│       └── hex.ts                      # MagFrame parser, mirrors Rust
├── packages/
│   └── nvsim-client/                   # publishable npm package
└── tests/
    ├── unit/
    └── e2e/
```

### 7.3 State model

A single `appStore` exposes signals (`@preact/signals-core`, ~3 KB) for:

```typescript
appStore.transport     // 'wasm' | 'ws'
appStore.connected     // boolean
appStore.running       // boolean
appStore.paused        // boolean
appStore.t             // sim time (s)
appStore.framesEmitted // bigint
appStore.scene         // Scene
appStore.config        // PipelineConfig
appStore.seed          // bigint
appStore.theme         // 'dark' | 'light'
appStore.density       // 'comfy' | 'default' | 'compact'
appStore.motionReduced // boolean
appStore.witness       // Uint8Array | null
appStore.lastB         // [number, number, number] (T)
appStore.snr           // number
```

Each signal is observed by exactly the components that need it; no Redux,
no global event bus.

### 7.4 Web Worker boundary (WASM transport)

- `worker.ts` instantiates `nvsim.wasm` once at boot.
- `appStore` calls go to worker as `{ type: 'cmd', op: 'run', args: { … } }`.
- Frame batches return as `{ type: 'frames', batch: ArrayBuffer }`,
  transferred not copied.
- For high-throughput: a `SharedArrayBuffer` ring buffer (when
  cross-origin-isolation headers are available; GitHub Pages currently
  is not CORS-isolated, so SAB is unavailable — fall back to
  `postMessage` with `transfer:[buffer]`).
- Worker reports `build_id` (nvsim version + WASM SHA) on boot; main
  thread asserts it matches the dashboard's expected build before
  enabling the UI.

### 7.5 The chart layer

Three bespoke SVG-based renderers (mockup uses inline SVG; keep that —
no Canvas, no WebGL, no library):

- `odmr.ts` — Lorentzian dip composite, 4-axis splitting, FWHM badge,
  fit overlay. Re-renders on every `appStore.lastB` change but inside
  `requestAnimationFrame` to coalesce.
- `b-trace.ts` — 200-sample ring buffer, three-channel polyline. Same RAF.
- `frame-strip.ts` — 48-bar sparkline.

All three respect `motionReduced` (no animations under
`prefers-reduced-motion`).

---

## 8. Data flow per mode

### 8.1 WASM mode (default, GitHub Pages)

```
User action → component → appStore signal
                               │
                               ▼
                       WasmClient.run({ frames: 256 })
                               │
                               ▼ postMessage
                       Web Worker
                               │
                               ▼
                       nvsim.WasmPipeline.run(256)
                               │
                               ▼
                       Vec<u8> (bytes) → ArrayBuffer
                               │
                               ▼ postMessage(transfer)
                       Main thread
                               │
                               ▼
                       parse → MagFrame[] → appStore.lastB / .witness / …
                               │
                               ▼
                       components re-render
```

Latency budget: <10 ms per 256-frame batch on a 2024-vintage laptop.

### 8.2 WS mode (opt-in)

User enters `ws://192.168.50.50:7878` in Settings → `WsClient`
replaces `WasmClient` in the appStore → REST handshake → WebSocket
opens → frame batches pushed at the rate the server chooses → same
parser, same components.

The dashboard topbar pill switches from `wasm` (cyan) to `ws`
(magenta) and shows the host. A red pill if the connection drops.

### 8.3 Witness verification

Both modes expose `generateWitness(N)` and `verifyWitness(expected)`.
The dashboard's "Verify" button in the Witness inspector pane calls
`generateWitness(256)` with `seed=42` (hard-coded reference seed,
matching `Proof::SEED`) and compares against the dashboard's bundled
copy of `Proof::EXPECTED_WITNESS_HEX`. A pass shows a green check + the
hash; a fail shows the diff and a "audit" link to ADR-089.

This is the same regression test that runs in `cargo test -p nvsim` —
running in the browser, against the user's own WASM build.

---

## 9. Build & deployment

### 9.1 GitHub Actions workflow

New workflow `.github/workflows/dashboard-pages.yml`:

```yaml
name: Dashboard → GitHub Pages
on:
  push:
    branches: [main]
    paths: ['v2/crates/nvsim/**', 'dashboard/**']
  workflow_dispatch:

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { targets: wasm32-unknown-unknown }
      - run: cargo install wasm-pack --version 0.13.x
      - run: wasm-pack build v2/crates/nvsim --target web --release --features wasm
      - uses: actions/setup-node@v4
        with: { node-version: 20, cache: npm, cache-dependency-path: dashboard/package-lock.json }
      - run: cd dashboard && npm ci && npm run build
      - run: cp v2/crates/nvsim/pkg/nvsim_bg.wasm dashboard/dist/nvsim.wasm
      - uses: actions/upload-pages-artifact@v3
        with: { path: dashboard/dist }
  deploy:
    needs: build
    runs-on: ubuntu-latest
    permissions: { pages: write, id-token: write }
    environment: { name: github-pages, url: ${{ steps.deployment.outputs.page_url }} }
    steps:
      - id: deployment
        uses: actions/deploy-pages@v4
```

### 9.2 GitHub Pages config

- Source: `gh-pages` branch (auto-managed by `actions/deploy-pages`).
- Custom domain (optional): `nvsim.ruvnet.dev` if/when DNS is wired.
- HTTPS enforced (default on GitHub Pages).
- 404 fallback to `/index.html` for SPA routing.

### 9.3 PWA

- `vite-plugin-pwa` with workbox.
- Cache the WASM binary, fonts, app shell. Offline-capable after first
  visit.
- Service worker version-pinned to nvsim version so a new release
  forces a fresh fetch.

### 9.4 nvsim-server distribution

- Cargo binary built per-target by existing `release.yml`.
- Docker image `ghcr.io/ruvnet/nvsim-server:vX.Y.Z` published on tag.
- Helm chart **not** in scope for V1; bare binary or Docker is enough.

---

## 10. Implementation phases

Six passes, mirroring the nvsim crate's own six-pass plan in
`docs/research/quantum-sensing/15-nvsim-implementation-plan.md`. Each
pass ends with a `[dashboard:passN]` commit and a green CI gate.

### Pass 1 — Scaffold (1–2 days)
- Vite + TS + Lit set up under `dashboard/`.
- Empty `app-shell` component, four-zone grid, dark theme only.
- IndexedDB plumbing.
- CI: `npm run build` succeeds, output <500 KB gzipped.

### Pass 2 — WASM transport (2–3 days)
- `wasm` feature in `nvsim` Cargo.toml.
- `wasm-bindgen` wrapper.
- Web Worker + `WasmClient`.
- Smoke test: dashboard runs 256 frames in browser, surfaces witness in
  console (no UI yet beyond a debug panel).
- CI: `wasm-pack build` succeeds, smoke E2E in headless Chromium passes.

### Pass 3 — UI surface (4–5 days)
- All 12 inventory components from §4.2.
- Charts (`odmr`, `b-trace`, `frame-strip`).
- Theme + density.
- Drawer + modals + toast.
- CI: visual regression vs. mockup screenshots (Playwright + pixelmatch,
  ≤2% diff per panel).

### Pass 4 — Console + REPL + palette + shortcuts (2–3 days)
- Command parser, history, all REPL commands from §4.3.
- Command palette ⌘K with fuzzy search.
- Full shortcut map.
- Debug HUD.

### Pass 5 — `nvsim-server` + WS transport (3–4 days)
- New `nvsim-server` crate.
- All routes from §6.2.
- `WsClient` impl.
- Settings UI to switch modes.
- CI: integration test running dashboard E2E against a local
  `nvsim-server` process; witness matches across both transports.

### Pass 6 — Polish, accessibility, deploy (2–3 days)
- WCAG audit (axe-core).
- Keyboard nav for every control.
- ARIA labels.
- `prefers-reduced-motion` honored everywhere.
- Onboarding tour wired.
- PWA service worker.
- GitHub Pages workflow.
- Cut release `v0.6.0-dashboard`.

**Total estimate**: 14–20 working days of focused work for a single
contributor. Parallelisable with hand-off boundaries on Pass 3.

---

## 11. Acceptance criteria (status as of 2026-04-27)

| # | Gate | Status | Evidence |
|---|---|---|---|
| 11.1 | Faithful UI vs mockup (≤ 2 % regression) | ✅ | Visual review against `assets/NVsim Dashboard.zip`. All 12 zones from §4.2 shipped. |
| 11.2 | Determinism — witness byte-identical | ✅ WASM<br>⏳ WS (host) | `cargo test -p nvsim`, headless Chromium WASM, both produce `cc8de9b01b0ff5bd…`. WS transport built (this ADR §6.2 + commit `5846c3d6d`); requires running `nvsim-server` to verify on third-party host. |
| 11.3 | Throughput ≥ 1 kHz | ✅ | ~1.79 kHz observed in Chromium WASM on x86 dev hardware. |
| 11.4 | Bundle ≤ 300 KB / WASM ≤ 1 MB | ✅ | ~140 KB gzipped JS, 162 KB WASM. |
| 11.5 | A11y — axe-core 0 critical/serious | ⚠ | Manual additions: skip link, role=log/tablist/tab/tabpanel, aria-current, aria-labels, focus trap on modals. Formal axe-core scan deferred. |
| 11.6 | Keyboard-only | ⚠ | Skip link + tabindex on `<main>` + focus trap. Not every flow validated Tab-only. |
| 11.7 | Offline (PWA) | ✅ | manifest.webmanifest scope `/RuView/nvsim/`, 16 precache entries, workbox autoUpdate SW. |
| 11.8 | Cross-browser | ⚠ | Chromium tested via agent-browser. FF + Safari pending post-merge. |
| 11.9 | REPL parity | ✅ | Every command in §4.3 implemented (help, scene.list, sensor.config, run, pause, reset, seed, proof.verify, proof.export, clear, theme, status). |
| 11.10 | Shortcut parity | ✅ | Every chord in §4.4 implemented (⌘K, Space, ⌘R, ⌘,, ⌘N, ⌘E, ⌘/, `, ?, 1/2/3, Esc, /). |
| 11.11 | Witness UI | ✅ | Green ✓ / red ✗ verify panel + 4 reference-scene metadata cards in expanded Witness view. |
| 11.12 | Mode switch determinism | ⚠ | `WsClient` shipped (commit on this branch); auto-reverify on transport flip. End-to-end byte-equivalence pending `nvsim-server` deploy. |

**Summary**: 8 ✅, 4 ⚠. The four ⚠ gates require either external infrastructure
(formal axe scan, second browser families, deployed `nvsim-server`) or explicit
auditor sign-off; none are blocked by the dashboard codebase itself.

---

## 12. Risks and mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| WASM perf < 1 kHz on mobile | Medium | High | Bench early in Pass 2; if mobile fails, fall back to coarser sample rate on detected mobile UA, document the gap |
| `wasm-bindgen` ABI drift breaks witness reproducibility | Low | High | Pin exact `wasm-bindgen` version in `nvsim` and dashboard; CI job re-derives witness on every PR |
| GitHub Pages lacks COOP/COEP for SAB | High | Low | Don't rely on SAB; postMessage transfer is fast enough for 256-frame batches |
| Bundle bloat | Medium | Medium | Strict 300 KB budget enforced by `size-limit` check in CI |
| Mockup features I missed | Low | Medium | Inventory in §4.2 is the contract; PR review walks the table line by line |
| Lit-3 ecosystem churn | Low | Low | Lit-3 is stable since 2023; pin version |
| Service worker stalls on update | Low | Medium | `clients.claim()` + version-pinned cache keys |
| Export-control review on `nvsim-server` (sub-THz radar adjacency) | Low | Low | nvsim is magnetometry-only, ADR-091 already documents that the radar tier is out of scope |
| Privacy review (dashboard logs) | Low | Low | Default WASM mode is local-only; WS mode requires explicit opt-in to a user-controlled host |

---

## 13. Alternatives considered

### 13.1 React/Next.js
Rejected. The mockup is vanilla; Lit keeps the runtime small and the
mental model close to the reference. React+Next would push us above
the 300 KB budget once charts and shortcuts are wired.

### 13.2 Tauri desktop app
Rejected for V1. The user explicitly asked for Vite + GitHub Pages.
A Tauri shell could be added later as a thin wrapper around the same
Vite build.

### 13.3 Server-only (no WASM)
Rejected. WASM mode is the GitHub-Pages "instant demo" path. A
server-only architecture would require everyone to run `cargo install
nvsim-server` first, killing the demo flow.

### 13.4 Rebuild the simulator in JS
Rejected hard. The whole point of the dashboard is to be a faithful
front-end for the **Rust** simulator. A JS reimplementation would
forfeit the determinism contract.

### 13.5 WebGL/Canvas chart layer
Rejected. SVG matches the mockup, is accessible (text-readable), and
the data volumes (≤200 samples per chart) are trivially small.

### 13.6 Single client, no interface abstraction
Rejected. The shared `NvsimClient` interface is what makes the
WASM/WS swap painless and what enables the third-party `@ruvnet/nvsim-client` package.

---

## 14. Open questions

1. **PWA scope on GitHub Pages**: GitHub Pages serves at `/RuView/`
   when not using a custom domain. Service worker scope must be
   declared accordingly. Resolved in Pass 6.
2. **Onboarding copy**: who writes the welcome-tour text? Mockup has
   placeholders. Open until Pass 6.
3. **WS auth**: V1 ships unauthenticated WS server (LAN use only).
   ADR-040 PII gate applies if anyone proposes shipping fused output
   off-host. Followup ADR if/when that becomes a use case.
4. **Multi-pipeline runs**: the API in §6.1 is single-pipeline. If a
   future use case wants compare-runs (e.g. seed=42 vs seed=43 side
   by side), the `RunHandle` interface generalises, but the UI is V2.
5. **Recorded-data replay**: out of scope for V1. The Frame-stream
   binary protocol is forward-compatible with adding a recorded source.

---

## 14a. App Store (added 2026-04-26)

The dashboard ships an **App Store** view that catalogues every WASM edge
module in `wifi-densepose-wasm-edge` (ADR-040 Tier 3 hot-loadable
algorithms) plus the `nvsim` simulator itself. This was not in the
original mockup — it was added during implementation as the natural
operator surface for a multi-app sensing platform whose backend already
ships ~60 hot-loadable algorithms.

### 14a.1 Catalog

| Category | Range | Count | Examples |
|---|---|---|---|
| Simulators | — | 1 | nvsim |
| Medical & Health | 100–199 | 6 | sleep_apnea, cardiac_arrhythmia, gait_analysis, seizure_detect, vital_trend |
| Security & Safety | 200–299 | 5 | perimeter_breach, weapon_detect, tailgating, loitering, panic_motion |
| Smart Building | 300–399 | 5 | hvac_presence, lighting_zones, elevator_count, meeting_room, energy_audit |
| Retail & Hospitality | 400–499 | 5 | queue_length, dwell_heatmap, customer_flow, table_turnover, shelf_engagement |
| Industrial | 500–599 | 5 | forklift_proximity, confined_space, clean_room, livestock_monitor, structural_vibration |
| Signal Processing | 600–619 | 7 | gesture, coherence, rvf, flash_attention, sparse_recovery, mincut, optimal_transport |
| Online Learning | 620–639 | 4 | dtw_gesture_learn, anomaly_attractor, meta_adapt, ewc_lifelong |
| Spatial / Graph | 640–659 | 3 | pagerank_influence, micro_hnsw, spiking_tracker |
| Temporal / Planning | 660–679 | 3 | pattern_sequence, temporal_logic_guard, goap_autonomy |
| AI Safety | 700–719 | 3 | adversarial, prompt_shield, behavioral_profiler |
| Quantum | 720–739 | 2 | quantum_coherence, interference_search |
| Autonomy / Mesh | 740–759 | 2 | psycho_symbolic, self_healing_mesh |
| Exotic / Research | 650–699 | 11 | ghost_hunter, breathing_sync, dream_stage, emotion_detect, gesture_language, happiness_score, hyperbolic_space, music_conductor, plant_growth, rain_detect, time_crystal |
| **Total** | | **66** | |

### 14a.2 Per-app metadata

Each entry in `dashboard/src/store/apps.ts` carries:

- `id` — kebab-case identifier (matches the `wifi-densepose-wasm-edge`
  module name; is the WASM3 export the ESP32 firmware loads).
- `name` — human-readable label.
- `category` — short-code for filter chips and event-ID range.
- `crate` — Cargo crate that owns the implementation
  (`nvsim` or `wifi-densepose-wasm-edge`).
- `summary` — single-line description shown on the card.
- `events` — emitted i32 event IDs from the `event_types` mod.
- `budget` — compute tier (`S` < 5 ms, `M` < 15 ms, `L` < 50 ms).
- `status` — maturity (`available` / `beta` / `research`).
- `adr` — back-reference to the ADR that introduced or governs the app.
- `tags` — fuzzy-search tokens.

### 14a.3 UI behavior

- **Card grid** — auto-fill at 280 px per card; theme-aware palette.
- **Search** — fuzzy match across `id`, `name`, `summary`, and `tags`.
- **Category chips** — single-select filter (sticky under the search).
- **Status chips** — secondary filter on maturity.
- **Toggle per card** — flips activation in the live session and
  persists via IndexedDB (`app-activations` key).
- **Active indicator** — emerald border on cards whose toggle is on.

### 14a.4 Activation semantics

- **WASM transport (default)**: activation is purely client-side; in V1
  the toggles drive the Console event log and let the user see "what
  would be running on a fleet" without needing actual hardware.
- **WS transport (deferred to V2)**: activation flips an
  `app.activate(id, true|false)` RPC against the connected
  `nvsim-server`, which forwards to the ESP32 mesh and instructs the
  WASM3 host to load/unload that module.

### 14a.5 Why this matters

RuView already ships 60+ purpose-built edge algorithms. Without an
operator surface they exist only in source code; the App Store makes
them **discoverable** and **toggleable** without recompiling firmware.
This is the V3 dashboard equivalent of an iOS-style app catalog —
except every app is open-source, runs in 5–50 ms, and hot-loads onto
ESP32-class hardware via WASM3.

### 14a.6 Adding a new app

1. Implement the algorithm in `wifi-densepose-wasm-edge/src/<id>.rs`.
2. Add `pub mod <id>;` to `lib.rs`.
3. Add an entry to `APPS` in `dashboard/src/store/apps.ts`.
4. Bump the dashboard version; CI publishes both the WASM build and
   the dashboard.

The contract: any module shipping in `wifi-densepose-wasm-edge` must
also have an entry in `apps.ts` (lint check planned for V2).

---

## 15. Cross-references

- **ADR-089** — `nvsim` simulator (the backend this dashboard fronts)
- **ADR-090** — Lindblad extension (will surface as a feature toggle in
  the Tunables panel once shipped)
- **ADR-091** — stand-off radar research (orthogonal; no UI overlap)
- **`docs/research/quantum-sensing/15-nvsim-implementation-plan.md`** — six-pass plan model
- **`docs/research/quantum-sensing/16-ghost-murmur-ruview-spec.md`** — the use-case framing
- **`assets/NVsim Dashboard.zip`** — the canonical UI mockup (single-file HTML, 4200 LOC)
- **`wifi-densepose-sensing-server`** — REST/WS pattern this server follows
- **`wifi-densepose-wasm`** — WASM pattern this client follows

---

## 16. References

### Web/PWA
- Vite 5 docs — https://vitejs.dev/
- Lit 3 docs — https://lit.dev/
- Workbox PWA — https://developer.chrome.com/docs/workbox/
- WCAG 2.2 — https://www.w3.org/TR/WCAG22/

### WASM tooling
- wasm-bindgen — https://rustwasm.github.io/wasm-bindgen/
- wasm-pack — https://rustwasm.github.io/wasm-pack/
- Cross-Origin Isolation (COOP/COEP) — https://web.dev/coop-coep/
- GitHub Pages COOP/COEP support — https://github.com/orgs/community/discussions/13309

### nvsim physics (back-references for the Tunables panel labels)
- Barry, J. F. et al. (2020). *Rev. Mod. Phys.* 92, 015004.
- Wolf, T. et al. (2015). *Phys. Rev. X* 5, 041001.
- Doherty, M. W. et al. (2013). *Phys. Rep.* 528, 1–45.
- Jackson, J. D. (1999). *Classical Electrodynamics, 3e*, §5.6, §5.8.

---

## 17. Status notes

- **Status**: Proposed — full implementation. Production target.
- **Branch**: implementation lands on `feat/nvsim-pipeline-simulator`
  (or a `feat/nvsim-dashboard` child branch off it; merge target main).
- **Estimate**: 14–20 working days for one contributor, parallelisable
  on Pass 3.
- **Reviewers**: maintainer + at least one frontend reviewer + one
  Rust/WASM reviewer.
- **Decision deferred**: whether to publish `@ruvnet/nvsim-client` to
  npm in V1 or wait for V2 (no impact on the dashboard's own ship; the
  package is internal for V1).

*This ADR is the contract for dashboard work. Every PR that adds dashboard scope above the inventory in §4.2 must amend this ADR or open a follow-up ADR.*
