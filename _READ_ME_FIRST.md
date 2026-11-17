# RuView — Cyber-HUD IPS contribution (PR staging copy)

This folder is a **copy** of the files added/changed during the Cyber-HUD +
indoor-positioning work. The originals remain in place in the repo; nothing here
is a move. Paths below mirror the real repo layout so you can review the exact
file set, then apply it to a fork.

## ⚠️ How to actually open the PR (important)

This working tree is **not a git repository** (`git rev-parse` fails here), and a
PR is *not* made by pushing a folder called `PR-github`. If you push this folder
as-is, the upstream repo would get files nested under `PR-github/…`, which is not
a valid contribution. Do it the normal way instead:

1. Fork `github.com/ruvnet/RuView` on GitHub → clone your fork.
2. `git checkout -b feature/cyber-hud-ips`
3. Copy each file below **to its real path** in the fork (the paths shown here,
   minus the `PR-github/` prefix — e.g. `examples/three.js/demos/06-cyber-hud.html`).
4. `git add -A && git commit` → `git push -u origin feature/cyber-hud-ips`
5. Open the PR on GitHub from your branch.

Once you've done that, you can delete this `PR-github/` folder — it's only a
staging convenience.

## Files in this contribution

### New files
| Path | What it is |
|------|-----------|
| `examples/three.js/demos/06-cyber-hud.html` | Cyber-HUD dashboard: rigged avatar, RTI backdrop, vitals, blueprint editor, multilateration IPS, multi-target roster, live calibration, intercept log |
| `examples/three.js/server/mock_udp_injector.py` | Mock multi-device UDP injector for hardware-free testing of the gateway |
| `v2/crates/wifi-densepose-signal/src/aoa_music.rs` | Pure-Rust multi-antenna MUSIC AoA (no BLAS), aperture-gated |
| `v2/crates/wifi-densepose-signal/src/multilateration.rs` | Range-only multilateration (2D/3D Gauss-Newton) + alpha filter |

### Modified files
| Path | Change |
|------|--------|
| `examples/three.js/index.html` | Registered demo 06 in the series index |
| `examples/three.js/server/serve-demo.py` | Added demo 06 to listing + optional UDP:5555→WebSocket:8770 gateway (`--gateway`) |
| `examples/through-wall/wiflow_infer.py` | Forwards real vitals + IPS `target`/`ranges` fields into `/pose` |
| `v2/crates/wifi-densepose-signal/src/phase_sanitizer.rs` | CFO/SFO linear phase de-trend (opt-in) |
| `v2/crates/wifi-densepose-signal/src/lib.rs` | Registered `aoa_music` + `multilateration` modules |
| `v2/crates/wifi-densepose-signal/src/ruvsense/tomography.rs` | Added Tikhonov ridge solver alongside ISTA-L1 |
| `v2/crates/wifi-densepose-sensing-server/src/csi.rs` | Wired CFO/SFO de-trend into the live path (env-gated) |
| `CHANGELOG.md` | `[Unreleased]` entries for all of the above |

## ⚠️ Before submitting — two things the maintainer will expect

1. **Compile the Rust.** It was authored where `cargo` was unavailable, so it was
   validated by mirrored numeric proofs but **not compiled**. Run and green it
   first: `cd v2 && cargo test -p wifi-densepose-signal -p wifi-densepose-sensing-server --no-default-features`
2. **Frontend/gateway JS + Python are verified** (syntax + extracted-block unit
   tests + a live UDP→WebSocket integration test), so those are ready to review.
