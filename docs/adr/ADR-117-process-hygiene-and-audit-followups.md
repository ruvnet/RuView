# ADR-117 — Process Hygiene, Pose Path Honesty, and Audit Follow-ups

**Status**: Accepted
**Date**: 2026-05-17
**Scope**: `v2/crates/wifi-densepose-sensing-server/src/{main.rs,wiflow_v1.rs}`,
`v2/crates/wifi-densepose-sensing-server/tests/multi_node_test.rs`,
`ui/index.html`, `ui/components/LiveDemoTab.js`, `CHECKLIST.md`,
`docs/adr/ADR-115-fw-set-target-rest.md`,
`docs/references/{espectre-gap-analysis.md,ota-pipeline.md}`.

## Context

A deep audit pass (4 parallel auditors covering sensors, server, UI, docs)
surfaced two operational fires and a stack of correctness/honesty issues
that had accumulated across ADR-100..116. This ADR collects the immediate
fixes.

### Fire 1 — Runaway ping zombies

Live `ps` showed **250+ `/sbin/ping -i 0.040` processes** on the Mac, most
parented to PID 1 (orphans from prior server lifetimes) and **8 fresh
pings to `127.0.0.1` parented to the current server**.

Root cause: a `cargo test --workspace` run sent UDP packets to
`127.0.0.1:5005` from `tests/multi_node_test.rs::test_multi_node_udp_send`
while the production server was bound to `0.0.0.0:5005`. The integration
test injects 55 synthetic frames with `node_ids = [1, 2, 3, 5, 7]`. Each
distinct `node_id` byte in a CSI magic packet triggered a fresh entry in
`NODE_ADDRS`, and the keepalive task spawned exactly one `ping` child
per entry. Combined with macOS not propagating parent death to children
(killed servers leave ping orphans), the count accumulated rapidly.

### Fire 2 — Per-node feature divergence on node 2

Node 2 (192.168.0.100) showed `dominant_freq_hz: 0.05` vs node 1 (.101)
`6.30` — a 126× split in the same room. Pointed to stale gain-lock on
node 2 from a different AP/orientation. Cleared via
`POST /ota/recalibrate` (ADR-109) — sensor re-runs the 300-packet
calibration sampler at next boot.

### Correctness issues (server auditor)

* `run_wiflow_inference` hardcoded keypoint `confidence: 1.0` — lied about
  data quality. Real signal: the runtime classifier's `confidence`.
* `wiflow_v1.rs` zero-pad path duplicated subcarrier index 0 instead of
  zero-padding when < 35 finite subcarriers — comment said "zero the
  rest", code did the opposite.
* `nbvi_history.clone()` cloned the entire 600-deep VecDeque (≈270 KB) on
  every inference, while only the last 20 frames are used.
* `run_wiflow_inference` picked the node with longest history regardless
  of recency — stale data from a dead sensor would keep producing pose.

### UI issues (UI auditor)

* `/` served a static API-index HTML page; users typing `localhost:8080`
  never reached the SPA at `/ui/index.html`.
* `<section id="sensing">` was empty; `app.js::SensingTab.mount` queried
  `#sensing-container` and rendered into nothing — the Sensing tab was
  permanently blank.
* `LiveDemoTab.fetchModels` unconditionally overwrote `activeModelId =
  'wiflow-v1'` whenever `/api/v1/info` reported `pose_estimation: true`,
  even when the operator had just loaded an RVF model. Dropdown silently
  flipped back to WiFlow on every refresh.

### Docs issues (docs auditor)

* `CHECKLIST.md` header: `head c827cde6`, count `43 Done` — stale
  by 4 commits and 2 ADRs.
* `ADR-115 References` cited "ADR-100 — TP-Link WISP" (it's ADR-110)
  and "ADR-108 / ADR-111" (ADR-111 doesn't exist — folded into ADR-109).
* `espectre-gap-analysis.md::Still open` table listed 8 items as open
  that had already shipped (ADR-104, ADR-109, ADR-112, ADR-114).
* `ota-pipeline.md` documented OTA flashing but never mentioned
  `/ota/set-target` (ADR-115) or `/ota/recalibrate` (ADR-109) — operator
  hitting the "Mac moved networks" scenario wouldn't find the recovery
  path.

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

* **Health endpoint fake constants** (cpu:2.5, mem:1.8, disk:15.0) —
  flagged by the auditor as critical. Replacing with `sysinfo` crate
  would add a dependency for low-value telemetry; the orchestrator
  readiness probe today is only used by Docker compose, not Kubernetes
  liveness. Deferred. Real fix: `/health/ready` only reports
  `model_loaded` + `node_count > 0`.
* **`derive_pose_from_sensing` call-site cleanup** — function returns
  `Vec::new()` since ADR-105; removing the 5 call sites is a no-op
  refactor with no behaviour change. Skipped to keep diff focused.
* **`tracker_bridge:10` unused imports warning** — module is integrated
  via `tracker_bridge::tracker_update` (4 callers), the import list
  just has dead names. Cosmetic. `cargo fix` deferred.
* **CLI training flags** (`--train`, `--dataset`, `--epochs`,
  `--checkpoint-dir`, `--pretrain*`) — silent no-ops; training is via
  REST. Removing the flags would break any operator script that passes
  them harmlessly. Deferred to a separate flag-audit pass.
* **OTA PSK provisioning** — operator workflow change, not a code
  change. Note added to ADR-115 open items. Operator can set
  `security/ota_psk` via USB provision.py whenever convenient.

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
