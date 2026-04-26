# ADR-083: Per-Cluster Pi Compute Hop

| Field          | Value                                                                                |
|----------------|--------------------------------------------------------------------------------------|
| **Status**     | Proposed — pending field evidence on three-tier proposal scope                       |
| **Date**       | 2026-04-26                                                                           |
| **Authors**    | ruv                                                                                  |
| **Supersedes** | —                                                                                    |
| **Refines**    | ADR-028 (capability audit), ADR-081 (5-layer kernel), ADR-066 (swarm bridge)         |
| **Companion**  | `docs/research/architecture/three-tier-rust-node.md`, `docs/research/architecture/decision-tree.md`, `docs/research/sota/2026-Q2-rf-sensing-and-edge-rust.md` |

## Context

ADR-028 established the per-node BOM at ~$9 (ESP32-S3 8MB) — ~$15 with a
mmWave sensor — and ADR-081 framed the firmware as a 5-layer adaptive
kernel running entirely on a single ESP32-S3 die. Both decisions are
correct for the **per-node** dimension; deployments that fit the
"sensor talks UDP to a server somewhere" shape work fine on this stack.

The three-tier-node research exploration
(`docs/research/architecture/three-tier-rust-node.md`) raised a separate
question: **what changes when a deployment scales past one or two rooms,
and where should the heavy compute live?** The exploration's answer
("dual ESP32-S3 + Pi Zero 2W per node") is one shape, but the
companion decision-tree (`decision-tree.md` §1, §3 L3, §5) identifies a
materially cheaper path: keep today's single-S3 sensor node unchanged
and add **one Pi per cluster of 3–6 sensor nodes**. The 2026-Q2 SOTA
survey (`sota/2026-Q2-rf-sensing-and-edge-rust.md`) confirms that the
load this path needs to carry — model inference, QUIC backhaul, and a
real secure-boot story — fits comfortably on a Pi-class SoC, while the
load it doesn't need to carry — CSI capture, ISR-precise wake control —
is exactly what the ESP32-S3 already does well.

The three things this ADR is about, all of which the current single-S3
deployment shape pushes onto the cloud or onto every individual node:

1. **Per-deployment ML inference.** WiFlow / DT-Pose / GraphPose-Fi
   class models (4–10M params, 0.5–1.5 GFLOPs) want a Cortex-A53-class
   target. The ESP32-S3 cannot host these; the cloud can but only at
   the cost of round-trip latency. A per-cluster Pi inference hop is
   the natural home.
2. **QUIC backhaul.** `quinn` + `rustls` is mature on Linux but does
   not run on ESP32-class hardware in any production-grade form
   (SOTA §5). A Pi terminating QUIC for a cluster gives every sensor
   node QUIC's loss/handoff/multiplex properties without porting QUIC
   to the MCU.
3. **Secure-boot anchor for OTA.** ESP-IDF Secure Boot V2 covers each
   sensor node, but cluster-wide policy (which model is current, which
   sensor MCU image is canary, what is the rollout ring) needs a
   higher-trust local store. A Pi running buildroot + dm-verity +
   signed FIT is a defensible anchor without the BOM hit of CM4 / Pi 5
   (the latter is its own decision; see ADR-085 sketch below and
   decision-tree.md L6).

The cluster-Pi shape does **not** require any change to ADR-028 or
ADR-081. The sensor node continues to be a single-MCU ESP32-S3 running
the 5-layer kernel. Everything new lives at the cluster boundary.

## Decision

Adopt **a per-cluster Pi hop** as the canonical RuView mid-scale
deployment shape. A "cluster" is **3–6 ESP32-S3 sensor nodes within
WiFi mesh range of one Pi**.

Specifically:

1. **Sensor nodes are unchanged.** They continue to run the ADR-081
   5-layer kernel on a single ESP32-S3, emit `rv_feature_state_t`
   packets (60 byte, ~5 Hz, ~300 B/s) over UDP, and connect via
   ESP-WIFI-MESH or direct WiFi to the cluster Pi.
2. **Each cluster has exactly one Pi** acting as:
   - **Sensor aggregator**: ingests UDP from all cluster sensor
     nodes, runs feature-level fusion (multistatic + viewpoint
     attention from the existing `wifi-densepose-ruvector` crate).
   - **ML inference target**: hosts the WiFi-pose model and runs
     inference at the cluster boundary, not on each sensor MCU.
   - **QUIC client to the cloud / gateway**: terminates QUIC mTLS,
     batches cluster-level events.
   - **OTA + secure-boot anchor for its sensor nodes**: holds signed
     manifests, stages canary rollouts, owns provisioning state.
3. **Cluster Pi SoC choice is deferred** to a future ADR (sketched
   below as ADR-085). The acceptable candidates are Pi Zero 2W, Pi 4,
   Pi 5, and CM4. The decision tree's L6 distinguishes these by
   secure-boot threat model; this ADR does not pre-commit.
4. **The single-node deployment shape is not deprecated.** A
   home-lab / single-room / development deployment can still run a
   single ESP32-S3 talking UDP directly to the existing
   `wifi-densepose-sensing-server`, no Pi required. The cluster Pi
   becomes the recommended shape for fleets ≥ 3 sensor nodes.

### Boundary contract

The cluster Pi exposes two interfaces:

| Interface              | Direction         | Schema                                                                |
|------------------------|-------------------|-----------------------------------------------------------------------|
| **UDP `rv_feature_state_t` ingest** | sensor → Pi | Existing 60-byte packed struct from ADR-081 (magic `0xC5110006`)     |
| **QUIC mTLS uplink**   | Pi → gateway/cloud | New: cluster-level event envelope (CBOR), batched, ~10 KB/min upper bound |

Sensor → Pi is **the same wire as today's sensor → server**. Cluster Pi
uplink is **new** and is what the existing `wifi-densepose-sensing-server`
becomes — relocated from the user's laptop / container to the cluster
node. Concretely: the sensing server already exists in
`crates/wifi-densepose-sensing-server`; it cross-compiles to ARMv7 /
AArch64 today via `cargo build --target aarch64-unknown-linux-gnu`. The
relocation is a deployment change, not a re-implementation.

### Three-tier vs cluster hop

This ADR's cluster-Pi shape is the L3-hybrid path in
`decision-tree.md` §2 — **not** the full three-tier (dual-MCU + per-node
Pi) shape. It captures most of the value (ML, QUIC, secure-boot anchor)
at minimal BOM impact. The full three-tier shape remains the long-term
exploration target, blocked behind L4 (no_std CSI maturity) and L2
(per-node ISR-jitter evidence).

## Consequences

### Positive

- **Pose-grade ML on edge becomes deployable**, not just possible. A
  Pi (any of the eligible SoCs) hosts WiFlow-class models with
  ≤ 100 ms latency per cluster, vs ≥ 1 s round-trip if pose runs in the
  cloud (SOTA §1, §3).
- **QUIC arrives without an MCU port.** `quinn` + `rustls` runs on the
  Pi as it does on a server (SOTA §5). The sensor MCU keeps UDP — the
  cheapest, highest-tested wire it already speaks.
- **Cluster-level secure boot becomes coherent.** Per-sensor Secure
  Boot V2 + flash encryption (ADR-028 baseline) is unchanged. The Pi
  buildroot + dm-verity image is the cluster trust anchor and signs
  the OTA manifests for its sensors. The cluster-level threat model is
  expressible without per-sensor BOM regression.
- **No PCB respin.** Sensor nodes are bit-for-bit identical to today's
  ADR-028 baseline. The cluster Pi is a separate device on the cluster
  WiFi (and / or Ethernet, if available).
- **Deployment cost scales sub-linearly with sensor count.** One
  $25–$60 Pi per 3–6 sensor nodes adds ~$5–$20 per sensor amortized,
  vs ~$25–$50 per sensor for the per-node-Pi shape.

### Negative

- **The cluster Pi is a new piece of infrastructure to provision,
  monitor, and update.** It is the right place for cluster-level
  responsibilities, but it is not free; it adds a Linux box to every
  multi-room deployment. Mitigated by buildroot images and the
  existing OTA tooling story (see Implementation §4).
- **Cluster Pi failure takes the cluster offline** (sensor nodes
  cannot uplink without a working aggregator on the WiFi LAN). For
  high-availability deployments, this ADR is the floor; an HA-pair
  cluster Pi would be a follow-up.
- **One more network hop on the sensing path.** Sensor → Pi → cloud
  adds ~5–20 ms over Sensor → cloud (depending on link quality).
  Pose latency budgets are 100s of ms, so this is well inside spec.

### Neutral

- ADR-028 (capability audit), ADR-081 (5-layer kernel), and ADR-066
  (swarm bridge) are unchanged. This ADR adds a new device class above
  the sensor; it does not modify the sensor itself.
- The home-lab single-node shape continues to work; this ADR adds a
  recommended path for fleets, it does not deprecate the existing one.

## Implementation

The implementation is intentionally light because most of the pieces
already exist; the ADR is largely about formalizing where they live.

1. **Cluster-Pi cross-compile target.** Add to
   `rust-port/wifi-densepose-rs/.cargo/config.toml` (or the equivalent
   per-crate target spec) an `aarch64-unknown-linux-gnu` target so
   `wifi-densepose-sensing-server` builds for Pi 4 / 5 / CM4 by
   default. Also retain `armv7-unknown-linux-gnueabihf` for Pi Zero 2W
   compatibility while the Pi-SoC decision (ADR-085 sketch) is open.
2. **Cluster-Pi service unit.** Add a systemd unit file under
   `firmware/cluster-pi/` (new directory) that runs
   `wifi-densepose-sensing-server` with the cluster's UDP/QUIC ports
   and drops privileges. Buildroot integration is a separate ADR if
   the SoC choice goes to Pi Zero 2W (where there's no RPi-OS path).
3. **QUIC uplink module.** Add `wifi-densepose-sensing-server` a
   feature-gated `quic-uplink` module using `quinn` + `rustls`. The
   feature is **off by default** in the home-lab shape and on for the
   cluster Pi.
4. **OTA + signed-manifest flow.** Out of scope for this ADR; tracked
   as I4 in `decision-tree.md` §4. The cluster Pi's role is to *hold*
   the manifest store, not to define the manifest format. Use the
   existing ADR-066 swarm bridge channel for OTA staging.
5. **Documentation update.** README's hardware-table gains a
   "Cluster compute" row. CLAUDE.md gets a one-paragraph cluster-Pi
   section under Architecture. User-guide gets a cluster-deployment
   section.
6. **Validation.** A 3-sensor cluster + 1 Pi fixture in the lab.
   Pass criteria: end-to-end CSI → cluster fusion → cloud ingest;
   measured latency under 100 ms per cluster; cluster Pi reboot
   without sensor data loss > 5 s; OTA staging round-trip across all
   sensors in the cluster.

## Validation

This ADR is **proposed**, not accepted. Acceptance requires:

1. The cluster-Pi `wifi-densepose-sensing-server` cross-compiles
   cleanly on `aarch64-unknown-linux-gnu` and `armv7-unknown-linux-gnueabihf`
   targets with the existing workspace tests passing.
2. A 3-sensor + 1-Pi field test demonstrates ≥ 4 hours stable
   end-to-end CSI → fusion → cloud round-trip with latency
   ≤ 100 ms per cluster and zero phantom-skeleton regressions
   (ADR-082 holds across the new uplink).
3. The cluster-Pi ↔ sensor secure-boot story is approved alongside
   ADR-085's SoC choice.

When the above pass, this ADR moves from **Proposed** → **Accepted**
and the README + CLAUDE.md are updated to reflect cluster-Pi as the
recommended fleet-shape.

## Related ADRs (current and proposed)

- **ADR-028** (Accepted) — ESP32 capability audit. Single-node BOM
  baseline. Unchanged by this ADR.
- **ADR-029** (Proposed) — RuvSense multistatic sensing mode. Pairs
  naturally with cluster-Pi: cluster Pi is the natural home for
  multi-sensor fusion.
- **ADR-066** — Swarm bridge to coordinator. The cluster-Pi is the
  per-cluster swarm coordinator endpoint.
- **ADR-081** (Accepted) — 5-layer adaptive CSI mesh firmware kernel.
  Unchanged by this ADR.
- **ADR-082** (Accepted) — Pose tracker confirmed-track output filter.
  Holds across UDP and QUIC uplinks identically.
- **Future ADR (sketched in `decision-tree.md` L4)** — `no_std` CSI
  capture maturity benchmark. Gates the dual-MCU shape; not required
  for the cluster-Pi shape proposed here.
- **Future ADR (sketched in `decision-tree.md` L6)** — Cluster-Pi SoC
  choice (Pi Zero 2W vs CM4 vs Pi 5). Pure secure-boot decision.

## Open questions

- **Cluster size sweet spot.** "3–6 nodes" is a planning estimate. The
  3-sensor lab fixture in §Implementation will inform whether the
  upper bound is closer to 4, 6, or 8 in practice.
- **Cluster-Pi failure semantics.** Default behavior: sensor MCUs hold
  the last 60 s of feature packets in RAM and replay on reconnect.
  HA-pair cluster Pi is a separate ADR if needed.
- **Mesh control-plane interaction.** If the deployment moves to
  Thread (decision-tree.md L5), the cluster Pi may need a Thread
  Border Router role. This ADR doesn't pre-commit; it's compatible
  with both ESP-WIFI-MESH and Thread futures.
