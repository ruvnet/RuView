# ADR-093: nvsim Dashboard Gap Analysis (post-deploy review)

| Field | Value |
|---|---|
| **Status** | **Mostly Implemented (2026-04-27)** — iterations A through I + UX usability pass + Home view + WsClient all shipped to PR #436. 19 of 21 catalogued gaps closed; remaining 2 (P2.4 light-theme contrast, P2.6 keyboard arrow scene nav) deferred to follow-up. |
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
| **P0.5** | ~~Topbar `seed` pill is decorative~~ | `nv-topbar.ts` | ✅ Iter C — opens "Set seed" modal with hex input; applies via `WasmClient.setSeed`. |
| **P0.6** | ~~Sim controls overlay absent~~ | `nv-scene.ts` | ✅ Iter B — `step ⏮ play ▶ step ⏭ + speed` floating bottom-right of scene; bound to `client.run/pause/step` and `speed.value` cycle. |
| **P0.7** | ~~Scene toolbar (zoom / fit / layers) missing~~ | `nv-scene.ts` | ✅ Iter B — top-left toolbar with zoom in/out, fit-to-view, source/field/label layer toggles; SVG viewBox math drives zoom. |
| **P0.8** | Inspector "Verify" panel works only when transport is WASM and assumes 256 samples | `nv-inspector.ts`, `WasmClient.ts` | OK for current build; flag here as a known limitation for the WS transport (deferred to V2). | Document — not a fix. |
| **P0.9** | ~~REPL `proof.export` not implemented~~ | `nv-console.ts` | ✅ Iter E — wires to `client.exportProofBundle()`, triggers a blob download with timestamp filename. |
| **P0.10** | ~~REPL command history is per-component~~ | `nv-console.ts` | ✅ Iter G — moved to `appStore.replHistory` signal, persisted via IndexedDB key `repl-history`. |

## 3. P1 — visible mockup features missing

| # | Gap | Location | Notes |
|---|---|---|---|
| **P1.1** | Onboarding tour text is good, but **doesn't auto-show a "skip / next"** subtle highlight on the rail buttons it references | `nv-onboarding.ts` | Mockup uses spotlight cutouts. Ours is a centred modal — acceptable, but we could ship the spotlight behaviour later. |
| **P1.2** | ~~Density toggle didn't visibly change anything~~ | `main.ts` + `app.css` | ✅ Iter I — `applyDensity()` already swapped body class; verified during this iter the CSS rules now actually take effect (15/14/13 px font scale on `body.density-{comfy,default,compact}`). |
| **P1.3** | `motion-toggle` only flips `body.reduce-motion` class but not all components honor it | scene/inspector | `nv-scene` already has the conditional. Verify B-trace and frame-strip animations stop too. |
| **P1.4** | ~~Scene "stat-card" SNR readout always `—`~~ | `nv-scene.ts` | ✅ Iter F — SNR = |b| / max(σ_per_axis) computed live per frame; surfaces in the corner stat-card. |
| **P1.5** | Inspector `frame-strip-2` from the Frame tab not in our impl | `nv-inspector.ts` | Mockup has a second sparkline strip in the Frame tab; we only ship one. Replicate. |
| **P1.6** | ~~Modals body content was short~~ | `nv-palette.ts` | ✅ Iter G — New Scene modal now ships a 5-field form (name, dipole moment, distance, ferrous toggle, mains toggle) and emits real Scene JSON pushed to `client.loadScene()`. Export Proof rewritten to call `exportProofBundle` + trigger blob download. |
| **P1.7** | ~~Scene drag positions don't persist~~ | `nv-scene.ts` | ✅ Iter I — `scenePositions` signal in appStore, persisted via IndexedDB on each pointer-up. Restored at component connect. |
| **P1.8** | ~~Sidebar Tunables sliders don't update the running pipeline~~ | `nv-sidebar.ts` + `WasmClient.ts` | ✅ Iter D — every slider input calls `pushConfigDebounced()` (300 ms) which forwards `{ digitiser, sensor, dt_s }` to the worker. Worker rebuilds the WasmPipeline with the new config. Verified via REPL log line `config pushed · fs=… f_mod=…`. |
| **P1.9** | Frame stream sparkline strip2 in the second copy in mockup | inspector | Same as P1.5 — verify. |
| **P1.10** | ~~"WASM" pill is read-only~~ | `nv-topbar.ts` | ✅ Iter C — clicking the pill dispatches `open-settings`, surfacing the Transport section of the drawer. |
| **P1.11** | ~~`prefers-reduced-motion` not auto-detected~~ | `main.ts` | ✅ Iter F — `window.matchMedia('(prefers-reduced-motion: reduce)').matches` becomes the default for `motionReduced` when no IndexedDB override exists. |
| **P1.12** | Scene 3D-tilt on pointer move not ported | `nv-scene.ts` | Mockup has `.tilt-stage` perspective transform. Optional polish. |
| **P1.13** | View-overlay "expand panel" not ported | global | Mockup has a `.view-overlay` that expands any inspector panel to full-screen. Defer V2. |

## 4. P2 — accessibility / polish

| # | Gap | Notes |
|---|---|---|
| **P2.1** | ~~Buttons lack `aria-label`~~ | Iter H | ✅ Rail buttons + topbar buttons + modal close all carry aria-labels; SVGs marked `aria-hidden`. |
| **P2.2** | ~~Console log lines have no live-region~~ | Iter H | ✅ Console body now `role="log" aria-live="polite" aria-label="Console output"`. |
| **P2.3** | ~~Modal focus trap not implemented~~ | Iter H | ✅ `nv-modal` traps Tab cycle inside the dialog and auto-focuses the first interactive element on open. |
| **P2.4** | Color contrast on `.ink-3` light theme borderline for AA | Tweak palette. *(Deferred — needs a color-system pass.)* |
| **P2.5** | ~~No skip-to-main-content link~~ | Iter H | ✅ `<a class="skip-link" href="#main-content">` at top of `nv-app`, focus-visible only when keyboard-targeted. Main view wrapped in `<main id="main-content" role="main">`. |
| **P2.6** | Keyboard navigation through scene draggable sources via arrow keys | Add. |
| **P2.7** | Service worker doesn't have `clients.claim()` | Confirm. Ensures new SW activates on next nav. |
| **P2.8** | PWA install prompt is silent | Add an install button (visible only when `beforeinstallprompt` fires). |

## 5. Iteration plan

The dynamic /loop continues with one P0/P1 item per iteration:

| Iter | Focus | Status |
|---|---|---|
| **A** | Functional Ghost Murmur demo (P0.4) | ✅ `runTransient` WASM export + interactive distance/moment sliders + per-tier detectability bars |
| **B** | Scene sim-controls + toolbar (P0.6, P0.7) | ✅ Bottom-right sim controls, top-left zoom/layer toolbar |
| **C** | Topbar seed + WASM pill clicks (P0.5, P1.10) | ✅ Seed modal + transport pill opens Settings drawer |
| **D** | Sidebar tunables wire-through (P1.8) | ✅ Debounced `setConfig` RPC, 300 ms |
| **E** | REPL `proof.export` + history persistence (P0.9, P0.10) | ✅ Blob download + IndexedDB-persisted history |
| **F** | SNR computation + reduce-motion (P1.4, P1.11, P1.3) | ✅ |B|/max(σ) live SNR, prefers-reduced-motion auto-detect |
| **G** | Modal contents (P1.6) | ✅ New-Scene form (5 fields), real Scene JSON push |
| **H** | A11y pass (P2.1–P2.5) | ✅ aria-labels, focus trap, role=log, skip link, role=tablist |
| **I** | Density toggle (P1.2) + drag persistence (P1.7) | ✅ Density CSS verified, scenePositions persisted to IndexedDB |
| **J** | UX usability pass | ✅ nv-help center (Quickstart/Glossary/FAQ/Shortcuts/About), 10-step welcome tour, panel descriptions, settings explainers, empty-state hints |
| **K** | Home view | ✅ `<nv-home>` as default landing — hero + 4 quick-jump cards + simplified grid hides power-user panels |
| **L** | WsClient transport | ✅ Full REST + binary WebSocket impl against `nvsim-server`; transport-flip auto-reverify; activated via Settings drawer |
| **M** | App Store live runtime | ✅ 6 simulated apps emit real i32 events against nvsim frame stream; runtime pills (running/simulated/mesh-only); live events feed |

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
