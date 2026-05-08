# ADR-094: Live 3D Point Cloud Viewer — GitHub Pages Deployment with Optional Real-Data Stream

| Field | Value |
|---|---|
| **Status** | Proposed (2026-04-29) |
| **Date** | 2026-04-29 |
| **Authors** | ruv |
| **Related** | ADR-092 (nvsim dashboard Pages deployment), ADR-059 (live ESP32 CSI pipeline), ADR-079 (camera ground-truth training) |
| **Branch** | `feat/pointcloud-pages-demo` |

---

## 1. Context

The `wifi-densepose-pointcloud` crate ships a Three.js-based viewer
(`v2/crates/wifi-densepose-pointcloud/src/viewer.html`) that renders the
fused camera-depth + WiFi CSI + mmWave point cloud produced by the
`ruview-pointcloud serve` binary. Today the viewer is local-only:

- It is served by the Axum binary on `127.0.0.1:9880`.
- It polls `/api/splats` every 500 ms expecting a backend on the same
  origin.
- There is no GitHub Pages deployment, so the README's
  "▶ Live 3D Point Cloud" link points at the moved-content section in
  `docs/readme-details.md`, not at a hosted demo. The two sibling demos
  (Live Observatory, Dual-Modal Pose Fusion) are already hosted at
  `https://ruvnet.github.io/RuView/` and `…/pose-fusion.html`.

This is an asymmetry: a first-time visitor can preview the WiFi pose
demo and the Observatory in one click, but cannot preview the point
cloud without cloning the repo, building Rust, plugging in an ESP32,
and pointing a webcam at themselves. That gap suppresses the most
visually compelling demonstration of the v0.7+ sensor-fusion work.

A naive fix — drop the static HTML at `gh-pages/pointcloud/` — does
not work because the viewer's `fetch("/api/splats")` will 404 on Pages
and the canvas will hang at "Loading…". A second naive fix — bake in a
fixed sample dataset — solves the loading state but loses the live-data
story entirely, and forks the viewer into a "demo build" and a "real
build" that drift apart.

## 2. Decision

Ship **one** viewer that auto-selects its transport from URL parameters,
and publish it to `gh-pages/pointcloud/` alongside the other demos:

1. **Default mode** — when the viewer is opened with no query parameters
   on `https://ruvnet.github.io/RuView/pointcloud/`, present a "▶ Enable
   camera" CTA. On click the viewer requests webcam access, runs
   **MediaPipe Face Mesh** in-browser (~30 fps, 478 refined landmarks),
   and renders the visitor's own face as a point cloud — the closest
   browser equivalent of the local pipeline's depth-backprojected face
   geometry that motivated this ADR (`I could see the outline of my face
   in points`). The viewer mirrors x to match selfie convention and
   maps Face Mesh's relative-z to the same world-coordinate range the
   live `/api/splats` payload uses, so a single render path drives both.
   Badge reads `● DEMO Your Face (MediaPipe)`. If the user denies
   camera permission, dismisses the prompt, or visits on a device
   without a webcam, the viewer falls back automatically to a
   procedural scaffold (floor grid, walls, breathing figure, 17-keypoint
   skeleton). All processing is client-side; no frames leave the
   browser. ~480-500 splats from the face plus ~110 floor/wall context
   splats.
2. **Auto mode** (`?backend=auto`) — fetch from `/api/splats` on the same
   origin. This is the local-development case (`ruview-pointcloud serve`
   serves the viewer and the API together). On any failure (404, network
   error, CORS), fall back silently to synthetic-demo rendering so the
   tab never dies.
3. **Remote mode** (`?backend=<url>`) — fetch from `<url>/api/splats`.
   This is the **integrated-ESP32** path: the user runs
   `ruview-pointcloud serve --bind 127.0.0.1:9880` locally with an
   ESP32-S3 streaming CSI to UDP port 3333, then opens
   `https://ruvnet.github.io/RuView/pointcloud/?backend=http://127.0.0.1:9880`.
   The hosted Pages viewer becomes a thin client for the local Rust
   fusion pipeline (camera depth + WiFi CSI + mmWave) without a clone
   or rebuild. The viewer also exposes a "📡 Connect ESP32" button that
   prompts for the URL, persists it in `localStorage`, and reloads
   with the query param.

   For this to work the local server must answer the browser's CORS
   preflight. `stream.rs` therefore installs a `tower_http` `CorsLayer`
   that allows three origin classes:

   - `https://ruvnet.github.io` — the published Pages demo.
   - `http://localhost:*` and `http://127.0.0.1:*` — developer running
     the bundled `viewer.html` directly.
   - `null` — `file://` origins.

   Mixed-content (HTTPS Pages → HTTP loopback) is permitted because
   modern browsers (Chrome 94+, Firefox 116+, Safari 16.4+) classify
   `127.0.0.1` and `localhost` as "potentially trustworthy" origins.
   Any other origin (a public hostname, etc.) is denied — this is not
   a wildcard CORS posture. Badge reads `● REMOTE <url>`. Same silent
   demo fallback on failure.
4. **Strict-live mode** (`?live=1`) — disable the demo fallback. If the
   chosen transport fails, replace the info panel with an explicit offline
   message (`● OFFLINE — Live backend required but unreachable`). Useful
   for embedding the viewer in a status page or kiosk.

The synthetic frame returned by the in-browser generator matches the
JSON shape of the live `/api/splats` payload exactly (`splats`, `count`,
`frame`, `live`, `pipeline.{skeleton,vitals,…}`), so a single render path
drives both modes. There is no demo build vs real build — only one HTML
file, one render path, and one set of bugs.

A new GitHub Actions workflow (`.github/workflows/pointcloud-pages.yml`)
copies the viewer to `gh-pages/pointcloud/index.html` on every push to
`main` that touches the viewer, using `peaceiris/actions-gh-pages@v4`
with `keep_files: true` to preserve the existing observatory, pose-fusion,
and nvsim deployments.

## 3. Consequences

### Positive

- **First-click demo.** Visitors clicking the README's
  "▶ Live 3D Point Cloud" link land on a working Three.js scene in <1 s,
  no toolchain required. Matches the parity of the other two demos.
- **Real-data on demand.** Users with their own `ruview-pointcloud serve`
  host can use the same hosted viewer URL with
  `?backend=https://their-host.example.com` — no clone, no rebuild. The
  hosted demo doubles as a thin client for self-hosted backends.
- **Single render path.** Synthetic frames flow through the same
  `handleData → updateSplats → drawSkeleton` pipeline as live frames, so
  visual regressions surface in the demo and the live build at the same
  time. This is the same dual-transport pattern ADR-092 chose for nvsim.
- **No backend deploy required.** Pages serves static HTML; the demo
  works without standing up an Axum host on the public internet, and
  there is no per-visitor CSI/camera plumbing to provision.
- **Preserves existing deployments.** `keep_files: true` plus the
  `pointcloud/` destination means observatory/, pose-fusion/, nvsim/,
  and the root index.html on gh-pages are untouched.

### Negative / tradeoffs

- **Face mesh ≠ CSI.** Browser webcam + MediaPipe gives real face
  geometry but does not produce CSI-derived pose. Visitors who want to
  see the *WiFi-driven* path still need `?backend=<their-host>`. The
  procedural fallback is not WiFi-driven either; it is purely visual
  scaffolding. We accept this — the goal of the hosted demo is to
  convey the *shape* of what the local pipeline produces (a point
  cloud of the user) rather than reproduce the WiFi physics in the
  browser. The latter is a future ADR (WASM port of the fusion crate).
- **CORS burden on remote mode.** Users who want to share their backend
  must add `Access-Control-Allow-Origin: https://ruvnet.github.io` (or
  `*`) to their `ruview-pointcloud serve` config. We document this in the
  workflow's generated README; we do **not** add a public proxy.
- **Synthetic generator lives in the viewer.** ~80 LOC of procedural JS
  is now part of `viewer.html`. Acceptable: the file is already the
  client-side render bundle, and the generator is bounded and inert
  (deterministic, no I/O, no eval).
- **No replay-from-recording in this ADR.** A future ADR may add a
  `?recording=<url>.jsonl` mode that replays captured frames at native
  rate; that is out of scope here.

### Neutral

- The local-dev experience is unchanged. `ruview-pointcloud serve` still
  serves `viewer.html` from the bundled asset and the viewer still hits
  `/api/splats` because `?backend` defaults to `auto`. Nothing in the
  Rust crate changes — this is HTML + workflow only.

## 4. Implementation

| File | Change |
|---|---|
| `v2/crates/wifi-densepose-pointcloud/src/viewer.html` | Add URL-param transport selector (`backend`, `live`), synthetic frame generator, demo-fallback path, transport-aware mode badge. ~120 LOC added, no removed behavior. |
| `.github/workflows/pointcloud-pages.yml` | New workflow: stage viewer to `_site/pointcloud/index.html`, deploy to `gh-pages/pointcloud/` with `keep_files: true`. Triggers on viewer changes and on manual dispatch. |
| `README.md` | Already updated — `▶ Live 3D Point Cloud` link will be retargeted to `https://ruvnet.github.io/RuView/pointcloud/` once the first deploy succeeds. (Tracked separately, not blocking this ADR.) |
| `docs/adr/README.md` | ADR index — add ADR-094 row. |

## 5. Acceptance Gates

This ADR is **Implemented** when all of the following hold:

1. Pushing to `main` with a viewer change triggers
   `pointcloud-pages.yml`, which deploys to `gh-pages/pointcloud/` in
   under 60 seconds.
2. `https://ruvnet.github.io/RuView/pointcloud/` loads, shows the
   "Enable camera" CTA, and on accept renders the visitor's face as a
   point cloud with badge `● DEMO Your Face (MediaPipe)` and non-zero
   splat + frame counts. On camera denial, falls back to the
   procedural scene with badge `● DEMO Synthetic`.
3. Existing demos at `https://ruvnet.github.io/RuView/` and
   `…/pose-fusion.html` and `…/nvsim/` are still reachable after the
   first deploy (smoke-tested manually).
4. `https://ruvnet.github.io/RuView/pointcloud/?live=1` shows the
   `● OFFLINE` panel (because no same-origin backend exists on Pages).
5. `https://ruvnet.github.io/RuView/pointcloud/?backend=https://example.invalid`
   falls back to demo within one poll interval (~500 ms) without
   throwing in the console.
6. Running `./target/release/ruview-pointcloud serve` locally and
   opening `http://127.0.0.1:9880/` (which serves the same HTML) still
   shows live-mode rendering with the `● LIVE Local Backend` badge.

## 6. Out of Scope

- Replaying recorded JSONL frames in the browser (future ADR).
- WASM-side execution of the fusion pipeline in the browser (would
  require porting the camera + mmWave path; deferred).
- Authentication / signed splats payloads — backend-side concern,
  unaffected by this client-side change.
- Hosting a public CORS proxy for users without their own backend.
