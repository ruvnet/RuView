# ADR-117 — Process Hygiene, Pose Path Honesty, and Audit Follow-ups

**Status**: Accepted
**Date**: 2026-05-17
**Scope**: `v2/crates/wifi-densepose-sensing-server/src/{main.rs,wiflow_v1.rs}`,
`v2/crates/wifi-densepose-sensing-server/tests/multi_node_test.rs`,
`ui/index.html`, `ui/components/LiveDemoTab.js`, `CHECKLIST.md`,
`docs/adr/ADR-115-fw-set-target-rest.md`,
`docs/references/{espectre-gap-analysis.md,ota-pipeline.md}`.

## Context

A 4-auditor pass (sensors, server, UI, docs) over ADR-100..116
surfaced two operational fires and a stack of correctness/honesty
issues. This ADR collects the immediate fixes.

* **Fire 1 — Ping zombies.** `ps` showed 250+ `/sbin/ping -i 0.040`
  processes — orphans from prior server lifetimes + 8 fresh pings to
  `127.0.0.1` parented to the current server. Root cause:
  `cargo test --workspace` sent UDP frames to `127.0.0.1:5005` from
  `tests/multi_node_test.rs` with `node_ids = [1,2,3,5,7]` — each
  unique nid registered in `NODE_ADDRS`, keepalive spawned one `ping`
  child per nid, macOS doesn't propagate parent death.
* **Fire 2 — Node 2 feature divergence.** `dominant_freq_hz` 0.05 (n2)
  vs 6.30 (n1), same room, 126×. Stale gain-lock from prior AP geometry.
  Fixed via `POST /ota/recalibrate` (ADR-109).
* **Correctness:** hardcoded keypoint `confidence: 1.0`, `wiflow_v1.rs`
  zero-pad path duplicated subcarrier 0, `nbvi_history.clone()` copied
  full 600-deep deque per tick, `run_wiflow_inference` ignored node
  staleness.
* **UI:** `/` served static API index (SPA was at `/ui/index.html`),
  `<section id="sensing">` was empty (no `sensing-container` div),
  `LiveDemoTab.fetchModels` overrode the operator's RVF selection on
  every poll.
* **Docs:** `CHECKLIST.md` header stale by 4 commits / 2 ADRs;
  `ADR-115` cited wrong sister ADRs ("ADR-100" → ADR-110, "ADR-111" → ADR-109);
  `espectre-gap-analysis.md` listed 8 shipped items as open;
  `ota-pipeline.md` never documented the post-flash REST endpoints.

## Decisions

### D1 — UDP receiver filters loopback before NODE_ADDRS

`main.rs::udp_receiver_task` now rejects loopback, unspecified, multicast,
and broadcast source addresses before inserting into `NODE_ADDRS`. Packets
still parse and feed the classifier — only the keepalive registration
is gated. Defends against any local sender (tests, simulators, future
tooling) accidentally driving ping spawn.

### D2 — Keepalive pre-reap at startup

`main.rs::csi_keepalive_task` runs `pkill -f "/sbin/ping -i 0.040"` and
`pkill -f "/usr/bin/ping -i 0.040"` once at task entry. Cleans up
orphans from prior server lifetimes without operator action. Cost: two
`pkill` invocations at startup, ~10 ms total. Idempotent.

### D3 — Real keypoint confidence

`run_wiflow_inference` now stamps `confidence = amp_classify_from_latest`
runtime classifier confidence onto all 17 keypoints (was `1.0` hardcoded).
The lite-scale wiflow has no per-keypoint uncertainty head; this signal
is the most honest stand-in. Currently reading **0.037** on the live
deployment — accurate reflection of "wiflow output is saturated, don't
trust these coords".

### D4 — Zero-pad fix in wiflow_v1

`build_input_from_history` now pushes `None` into `picks` for dead slots
and writes `0.0f32` into those rows. Prior code pushed `0usize` → all
unused channels read subcarrier-0 amplitudes, feeding the network 35×
the same signal.

### D5 — Tail-clone optimisation

`run_wiflow_inference` snapshots only the last 20 entries from
`nbvi_history` while holding the lock, not the full 600-deep deque. Lock
hold time dropped from ~µs * 600 to ~µs * 20 per tick.

### D6 — `/` → `/ui/index.html` permanent redirect

`main.rs::root_redirect` returns HTTP 308. API-index HTML moves to `/api`
for operators / curl debugging. Users typing the bare host land on the
SPA.

### D7 — Sensing tab container restored

`ui/index.html`: `<section id="sensing">` now contains `<div
id="sensing-container">` matching `app.js::SensingTab.mount`'s query
selector.

### D8 — LiveDemoTab WiFlow inject only when no model active

`LiveDemoTab.fetchModels` wraps the `activeModelId = 'wiflow-v1'`
assignment in `if (!this.modelState.activeModelId)`. RVF model loads
keep their displayed name.

### D9 — Multi-node test guards against external :5005 owner

`tests/multi_node_test.rs::test_multi_node_udp_send` probes
`127.0.0.1:5005` with a transient bind; if the bind fails, the test
skips its UDP send rather than polluting whoever owns the port. Belt-
and-braces with the server-side filter (D1).

### D10 — Docs sweep

* `CHECKLIST.md`: header to `head 0ec1e4b0`, count to **47 Done**,
  explicit note that ADR-111 is intentionally absent. Reference table
  range to `001-117`.
* `ADR-115`: "ADR-100" → "ADR-110", "ADR-108 / ADR-111" → "ADR-108 / ADR-109".
* `espectre-gap-analysis.md::Still open` table: 8 shipped items marked
  ✓ Done with commit hashes; remaining items annotated Deferred with
  reason or carry a Pack assignment. New items 15-16 added (ADR-115,
  ADR-117).
* `ota-pipeline.md`: new "Operator REST endpoints" section listing
  `/ota/status`, `/ota`, `/ota/recalibrate`, `/ota/set-target` with
  curl examples both unauthed and bearer-token authed.

## Files Touched

```
v2/crates/wifi-densepose-sensing-server/src/main.rs:
  + udp_receiver_task: loopback/unspecified/multicast/broadcast filter (D1)
  + csi_keepalive_task: pre-reap pkill at task entry (D2)
  + run_wiflow_inference: real classifier confidence (D3) + tail clone (D5)
  + Router: GET / → root_redirect (308), GET /api → info_page (D6)
  + info_page: expanded with new endpoints listed
v2/crates/wifi-densepose-sensing-server/src/wiflow_v1.rs:
  + build_input_from_history: None-pad → 0.0f32, not subcarrier-0 dup (D4)
v2/crates/wifi-densepose-sensing-server/tests/multi_node_test.rs:
  + ADR-117 guard: skip if 127.0.0.1:5005 is owned (D9)
ui/index.html:
  + <div id="sensing-container"> inside #sensing section (D7)
ui/components/LiveDemoTab.js:
  + fetchModels: guard wiflow inject behind !activeModelId (D8)
CHECKLIST.md:
  + header refresh + ADR range correction (D10)
docs/adr/ADR-115-fw-set-target-rest.md:
  + typo fixes ADR-100 → ADR-110, ADR-111 → ADR-109 (D10)
docs/references/espectre-gap-analysis.md:
  + Still-open table refresh — 8 items ✓ Done, 14/15 reclassified (D10)
docs/references/ota-pipeline.md:
  + Operator REST endpoints section (D10)
docs/adr/ADR-117-process-hygiene-and-audit-followups.md  (this)
```

Binary size delta: 3.0 MB → 3.1 MB (no significant change).

## Verified Acceptance

After restart with the new binary (PID 97903):

```
$ ps -axo pid,ppid,command | grep "ping.*-i.*0\.040" | grep -v grep | wc -l
2
$ ps -axo pid,ppid | grep "ping.*-i.*0\.040"
97921 97903   /sbin/ping -i 0.040 192.168.0.100
97922 97903   /sbin/ping -i 0.040 192.168.0.101
```

Exactly two ping children — one per real sensor — parented to the
running server. No 127.0.0.1, no orphans.

```
$ curl -sI http://localhost:8080/
HTTP/1.1 308 Permanent Redirect
location: /ui/index.html

$ curl http://localhost:8080/api/v1/pose/current | jq '.persons[0].keypoints[0]'
{ "name": "nose", "x": 0.999, "y": 0.0, "z": 0, "confidence": 0.037 }
```

`confidence: 0.037` — real runtime classifier signal, not hardcoded 1.0.
`cargo test --workspace` (release) passes 13 / 0 failed / 5 ignored.

## Out of Scope (intentional non-fixes)

* **Health endpoint fake constants** (cpu/mem/disk hardcoded) — adding
  `sysinfo` crate just for orchestrator telemetry is heavy. Deferred.
* **`derive_pose_from_sensing` call-site cleanup** — already returns
  `Vec::new()` (ADR-105); removing 5 call sites is no-op refactor.
* **`tracker_bridge:10` unused-imports warning** — module is integrated
  via `tracker_update` (4 callers); import list has dead names. Cosmetic.
* **CLI training flags** (`--train`, `--dataset`, `--epochs`,
  `--checkpoint-dir`, `--pretrain*`) — silent no-ops; training is via
  REST. Removing flags would break operator scripts. Deferred audit.
* **OTA PSK provisioning** — workflow change, not a code change.
  Logged in ADR-115 open items.

## References

* ADR-105 — no synthetic data in production runtime; this ADR extends
  the principle to keypoint confidence (was synthesised, now real).
* ADR-109 — gain-lock recalibrate REST; same endpoint used to fix node 2
  feature divergence as part of this audit pass.
* ADR-115 — set-target REST; typos fixed here.
* ADR-116 — WiFlow-v1 loader; the auditor's findings landed against
  this ADR's just-shipped integration.
* `tests/multi_node_test.rs` — the test whose accidental cross-talk with
  the production server triggered the 250+ ping zombie incident.
