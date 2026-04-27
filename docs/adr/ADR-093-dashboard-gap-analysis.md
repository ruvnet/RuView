# ADR-093: nvsim Dashboard Gap Analysis (post-deploy review)

| Field | Value |
|---|---|
| **Status** | Proposed — implementation in progress on `feat/nvsim-pipeline-simulator`. |
| **Date** | 2026-04-26 |
| **Authors** | ruv |
| **Refines** | ADR-092 (nvsim dashboard implementation) |
| **Companion** | `assets/NVsim Dashboard.zip` (mockup, ~4200 LOC), live deploy https://ruvnet.github.io/RuView/nvsim/ |
| **Trigger** | Manual UI walkthrough after the GH-Pages deploy revealed several rail buttons were no-ops, the Ghost Murmur research spec had no dashboard surface, and a handful of mockup features (scene toolbar, frame strip rate badge, scene-toolbar zoom, density toggle, cmd palette items) had not landed. |

---

## 1. Method

A line-by-line inventory walk of the deployed dashboard against four
reference points:

1. **The mockup**: `assets/NVsim Dashboard.zip` → `NVSim Dashboard.html`.
   Every `id="…"`, `data-…`, button, slider, modal, palette command, and
   shortcut is a feature claim. We diff it against the live SPA.
2. **ADR-092 §4.2** — the canonical inventory table of 12 zones and ~50
   components. We mark each row as ✅ shipped / ⚠ partial / ❌ missing.
3. **ADR-092 §4.3** — REPL command set (10 commands).
4. **ADR-092 §4.4** — keyboard shortcuts (11 chords).

Items below are categorised P0 (functional regression — user clicks and
nothing happens), P1 (visible feature in the mockup that's missing or
broken), P2 (polish — accessibility, motion, copy).

The closing §5 is the iteration plan.

---

## 2. P0 — broken/missing functional surface

| # | Gap | Location | Root cause | Fix |
|---|---|---|---|---|
| **P0.1** | ~~Inspector rail button no-op~~ | `nv-rail.ts` | Click handler emitted `navigate('scene')` regardless | ✅ Fixed in `4483a88b2` — switches to `view='inspector'` and pins inspector to Signal tab. |
| **P0.2** | ~~Witness rail button no-op~~ | `nv-rail.ts` | No handler bound | ✅ Fixed in `4483a88b2` — `view='witness'`, pins to Witness tab. |
| **P0.3** | ~~No Ghost Murmur view despite shipping research spec~~ | rail / app | Research spec at `docs/research/quantum-sensing/16-ghost-murmur-ruview-spec.md` had no dashboard surface | ✅ Fixed in `4483a88b2` — new `<nv-ghost-murmur>` component, dedicated rail icon. |
| **P0.4** | Ghost Murmur view is **read-only** | `nv-ghost-murmur.ts` | Currently a static document. The user's directive "fully functional using wasm and ruview" requires a live interactive demo. | ⏳ §5 below — interactive distance/moment sliders that actually drive `nvsim::Pipeline` via WASM and report per-tier detectability. |
| **P0.5** | Topbar `seed` pill is decorative | `nv-topbar.ts` | Pill renders but click does nothing — should open the seed-set modal | ⏳ Wire to `openModal({ title: 'Set seed', … })`. |
| **P0.6** | Sim controls overlay (`step ⏮ play ⏯ step ⏭ + speed`) absent in scene | `nv-scene.ts` | Mockup ships `.sim-controls` floating widget; not ported | ⏳ Add as a corner overlay in `<nv-scene>`. |
| **P0.7** | Scene toolbar (zoom / fit / layers) missing | `nv-scene.ts` | Mockup ships `.scene-toolbar` top-left; not ported | ⏳ Implement zoom (SVG viewBox scale), fit-to-view, layer-toggle for each source class. |
| **P0.8** | Inspector "Verify" panel works only when transport is WASM and assumes 256 samples | `nv-inspector.ts`, `WasmClient.ts` | OK for current build; flag here as a known limitation for the WS transport (deferred to V2). | Document — not a fix. |
| **P0.9** | REPL `proof.export` command not implemented | `nv-console.ts` | Mockup has `proof.export` returning a downloadable bundle | ⏳ Wire to `client.exportProofBundle()` and trigger blob download. |
| **P0.10** | REPL command history is per-component, lost on view switch | `nv-console.ts` | `history` is instance-private | ⏳ Move to `appStore` so it survives view changes (low impact but expected). |

## 3. P1 — visible mockup features missing

| # | Gap | Location | Notes |
|---|---|---|---|
| **P1.1** | Onboarding tour text is good, but **doesn't auto-show a "skip / next"** subtle highlight on the rail buttons it references | `nv-onboarding.ts` | Mockup uses spotlight cutouts. Ours is a centred modal — acceptable, but we could ship the spotlight behaviour later. |
| **P1.2** | Density toggle in Settings drawer doesn't visibly change anything | `app.css` + `nv-settings-drawer` | CSS has `body.density-comfy/default/compact` rules but the application code only modifies `body.style.fontSize`. Wire the body class through `appStore.density` properly. |
| **P1.3** | `motion-toggle` only flips `body.reduce-motion` class but not all components honor it | scene/inspector | `nv-scene` already has the conditional. Verify B-trace and frame-strip animations stop too. |
| **P1.4** | Scene "stat-card" SNR readout is always `—` | `nv-scene.ts` | We never compute SNR from frames. Compute as |b| / max(σ_per_axis) and surface. |
| **P1.5** | Inspector `frame-strip-2` from the Frame tab not in our impl | `nv-inspector.ts` | Mockup has a second sparkline strip in the Frame tab; we only ship one. Replicate. |
| **P1.6** | Modals: New Scene + Export Proof + About defined in palette but body content is short | `nv-palette.ts` | Mockup's New-Scene dialog ships a full form (sources count, ferrous toggle, etc.). Ours is a placeholder. Implement form. |
| **P1.7** | Scene drag persistence | `nv-scene.ts` | Mockup persists drag positions via IndexedDB. Add. |
| **P1.8** | Sidebar Tunables sliders don't actually update the running pipeline | `nv-sidebar.ts` + `WasmClient.ts` | Slider changes the signal but worker isn't told to rebuild the pipeline with new fs/fmod. Wire `setConfig()` debounced. |
| **P1.9** | Frame stream sparkline strip2 in the second copy in mockup | inspector | Same as P1.5 — verify. |
| **P1.10** | "WASM" pill in topbar should show actual transport (`wasm` / `ws`); clicking should let user toggle | `nv-topbar.ts` | Pill is read-only. Make it a toggle that opens the Settings drawer at the Transport section. |
| **P1.11** | `prefers-reduced-motion` system preference not auto-detected | `main.ts` | Read once at boot, default `motionReduced` to `true` when set. |
| **P1.12** | Scene 3D-tilt on pointer move not ported | `nv-scene.ts` | Mockup has `.tilt-stage` perspective transform. Optional polish. |
| **P1.13** | View-overlay "expand panel" not ported | global | Mockup has a `.view-overlay` that expands any inspector panel to full-screen. Defer V2. |

## 4. P2 — accessibility / polish

| # | Gap | Notes |
|---|---|---|
| **P2.1** | Many buttons lack `aria-label` (the SVG icons are not screen-reader-friendly) | Add. |
| **P2.2** | Console log lines are text-only; `<div role="log" aria-live="polite">` recommended. | Add. |
| **P2.3** | Modal focus trap not implemented — Tab leaks to background | Add a small focus-trap to `nv-modal` and `nv-onboarding`. |
| **P2.4** | Color contrast on `.ink-3` light theme borderline for AA | Tweak palette. |
| **P2.5** | No skip-to-main-content link | Add. |
| **P2.6** | Keyboard navigation through scene draggable sources via arrow keys | Add. |
| **P2.7** | Service worker doesn't have `clients.claim()` | Confirm. Ensures new SW activates on next nav. |
| **P2.8** | PWA install prompt is silent | Add an install button (visible only when `beforeinstallprompt` fires). |

## 5. Iteration plan

The dynamic /loop continues with one P0/P1 item per iteration:

| Iter | Focus | Deliverable |
|---|---|---|
| **A** *(this turn)* | Functional Ghost Murmur demo (P0.4) | `WasmClient.runTransient(scene, n)` + interactive distance slider + per-tier detectability |
| **B** | Scene sim-controls + toolbar (P0.6, P0.7) | Floating sim-controls bottom-right of scene, zoom/fit/layer toolbar top-left |
| **C** | Topbar seed pill + WASM pill clicks (P0.5, P1.10) | Seed modal + transport toggle |
| **D** | Sidebar tunables wire-through (P1.8) | Debounced `setConfig` RPC propagates to pipeline |
| **E** | REPL `proof.export` + history persistence (P0.9, P0.10) | Blob download + appStore history |
| **F** | SNR computation + reduce-motion audit (P1.4, P1.11, P1.3) | Live SNR, system-pref auto-detect |
| **G** | Modal contents (P1.6) | New-Scene form with real Scene JSON output |
| **H** | A11y pass (P2.1–P2.6) | aria-labels, focus traps, skip link |
| **I** | Density toggle visual (P1.2), drag persistence (P1.7) | Polish |

Each iteration ends with: `npx tsc --noEmit` clean → production
build with `NVSIM_BASE=/RuView/nvsim/` → push to `gh-pages/nvsim/`
preserving siblings → `agent-browser` validation including console
errors → commit on `feat/nvsim-pipeline-simulator`.

The acceptance criteria from ADR-092 §11 still apply unchanged. This
ADR augments §11 rather than replacing it — every P0 item is a
prerequisite for declaring §11.1 (faithful UI) green.

## 6. References

- ADR-092 §4.2 — full UI inventory table (the contract).
- ADR-092 §11 — 12 acceptance gates.
- `assets/NVsim Dashboard.zip` — canonical mockup (committed).
- `docs/research/quantum-sensing/16-ghost-murmur-ruview-spec.md` — Ghost Murmur source material.
- Live deploy — https://ruvnet.github.io/RuView/nvsim/ (verified: rail buttons functional, witness verifies, App Store catalog renders, onboarding tour works).
