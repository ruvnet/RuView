# Pi 5 + Hailo Cluster: Building a Cognitive RF Observer with rvcsi

A field-tested tutorial for turning a 4-node Raspberry Pi 5 cluster into a
multistatic Wi-Fi CSI cognitive RF observer that learns room states,
predicts the next one, and flags anomalies — entirely from radio.

**Estimated time:** 4–6 hours (hardware 1h, firmware 1h, software 1h, calibration 1–3h)

**What you will build:** A self-learning 4-node cluster that captures Wi-Fi
Channel State Information from a stable RF beacon, encodes each frame into a
128-dimensional fingerprint on an on-device Hailo-8 NPU, clusters those
fingerprints into discrete room states with stable IDs across runs, models
state transitions with a 2nd-order Markov chain (with measurable predictive
skill above chance), and persists everything to a queryable brain corpus on
a workstation. The whole thing runs over Tailscale and is operated through
a single CLI with **34 subcommands**.

**Who this is for:** RF engineers, smart-home hackers, security researchers,
and ML/embedded folks comfortable with Linux + systemd. No specific signal-
processing background required — but you do need patience for hardware
quirks (nexmon_csi cross-compile is a known dead end; see step 3).

> **The TL;DR**: 4× Pi 5 + 2× Hailo-8 → CSI → 128-d embeddings → cosine
> k-means with warm-start → 2nd-order Markov → SQLite brain → 34-subcommand
> operator CLI. Production-grade signal: 39% top-1 ceiling on next-state
> prediction (16× chance baseline), continuous fleet/drift/anomaly
> monitoring, and a 12-category time-series corpus.

> **About the name "rvcsi" in this tutorial.** When this tutorial was
> first written, the cluster's per-Pi capture services were named with
> an `rvcsi` prefix (`cog-rvcsi-stream`, `cog-rvcsi-correlator`) as
> branding only — the actual code was Python and didn't depend on the
> upstream [`ruvnet/rvcsi`](https://github.com/ruvnet/rvcsi) Rust
> runtime. **As of 2026-05-13**, the v0-appliance project has accepted
> [ADR-207](https://github.com/ruvnet/v0-appliance/blob/main/docs/adr/ADR-207-rvcsi-library-integration.md)
> (rvCSI library integration — Option D) and shipped a Rust binary
> `cog-rvcsi-pi` built on rvcsi-runtime 0.3 that replaces the three
> Python services. The cutover is per-Pi, operator-driven, with
> one-command rollback (`scripts/rvcsi-pi/install-rvcsi-pi.sh` and
> `uninstall-rvcsi-pi.sh`). A given cluster may be running either
> stack while migration is in progress; the schema and operator
> surface are unchanged across the cutover. See ADR-207's
> Implementation log for the current state.

---

## Table of Contents

1. [Prerequisites](#1-prerequisites)
2. [Architecture overview](#2-architecture-overview)
3. [Per-node firmware: nexmon_csi on Pi 5](#3-per-node-firmware-nexmon_csi-on-pi-5)
4. [Per-node services](#4-per-node-services)
5. [Workstation pipeline](#5-workstation-pipeline)
6. [Calibration: getting from raw CSI to room states](#6-calibration-getting-from-raw-csi-to-room-states)
7. [Operating the cluster: the cog-query CLI](#7-operating-the-cluster-the-cog-query-cli)
8. [What you can measure](#8-what-you-can-measure)
9. [Troubleshooting](#9-troubleshooting)
10. [Next steps](#10-next-steps)

---

## 1. Prerequisites

### Hardware

| Item | Quantity | Approx. cost | Notes |
|------|----------|--------------|-------|
| Raspberry Pi 5 (8GB) | 4 | ~$80 each | 4GB works but tight under sustained load |
| Hailo-8 M.2 HAT (AI Kit) | 2 | ~$110 each | Only 2 needed — encoder is split across cluster-1 + cluster-2 |
| MicroSD (64GB, A2) | 4 | ~$10 each | A2 class strongly recommended for sustained writes |
| USB-C PD power supply (27W) | 4 | ~$12 each | Pi 5 draws 5A at full Hailo load |
| Active cooler | 4 | ~$5 each | Cluster-2 sustains thermal load — passive will throttle |
| Workstation (≥16GB RAM, Linux) | 1 | — | Hosts the brain HTTP service + clusterer + anomaly daemon |
| Stable Wi-Fi beacon | 1 | — | Any AP on the same 5 GHz channel. We use ch.149/80MHz. Stability matters more than identity. |

**Total parts cost:** ~$580 plus workstation.

> **Important:** All 4 Pi 5s must use the on-board `bcm43455c0` radio. USB
> Wi-Fi adapters with otherwise-similar chipsets **will not** work — nexmon's
> firmware patches are silicon-specific. See ADR-206 § "USB Wi-Fi dongle
> rabbit-hole" for the painful version of that lesson.

### Software prerequisites

| Component | Version | Notes |
|-----------|---------|-------|
| Pi OS Bookworm (Lite) | 64-bit, kernel 6.6+ | Use the Lite image — Desktop slows boot and burns SD writes |
| Tailscale | ≥1.60 | Mesh networking across the cluster |
| Rust toolchain | 1.78+ on workstation, 1.78+ on each Pi | For ruvector + adapter binaries |
| Python 3.11+ | system Python on workstation | numpy required |
| systemd-user | already present | Workstation timers run as user units |

---

## 2. Architecture overview

```
                              ┌─ workstation (Linux, ≥16GB) ──────────────────┐
                              │                                                │
                              │  brain HTTP (SQLite, port 9876)                │
                              │      ↑↑                                        │
                              │   ┌──┴┴──────────────────────────────────┐    │
                              │   │ rfmem-tail  ← ingests live brain     │    │
                              │   │ rfmem-recall  → posts category=      │    │
                              │   │              rfmem-recall when       │    │
                              │   │              current state ≈ past    │    │
                              │   │ rfmem-anomaly → 13-axis detector,    │    │
                              │   │              posts rfmem-anomaly &    │    │
                              │   │              rfmem-state-transition  │    │
                              │   │ cog-rfmem-states (timer, hourly)     │    │
                              │   │              re-clusters w/ warm-start│    │
                              │   │ cog-rfmem-insights (timer, nightly)  │    │
                              │   │              writes rfmem-insights    │    │
                              │   │ cog-rfmem-drift-check (timer, 05:00) │    │
                              │   │              audits cluster file state│    │
                              │   └───────────────────────────────────────┘    │
                              │                                                │
                              │  cog-query (CLI, 34 subcommands, 4 JSON modes)│
                              └────────────────────────────────────────────────┘
                                                ↑
                       Tailscale mesh ──────────┴───────────────────────────────┐
                              ↓                              ↓                  ↓
       ┌─ cluster-1 (Hailo) ┐  ┌─ cluster-2 (Hailo + fusion) ┐  ┌─ cluster-3 ┐ ┌─ v0 ┐
       │ cog-csi-emitter    │  │ cog-csi-emitter             │  │ same as    │ │ same│
       │ cog-csi-adapter    │  │ cog-csi-adapter             │  │ cluster-1  │ │ as  │
       │ cog-rvcsi-stream   │  │ cog-rvcsi-stream            │  │ minus      │ │ c-3 │
       │ cog-hailo-encoder  │  │ cog-hailo-encoder           │  │ Hailo &    │ │     │
       │                    │  │ cog-rvcsi-correlator (fusion)│  │ correlator │ │     │
       └────────────────────┘  └─────────────────────────────┘  └────────────┘ └─────┘
            4 svc                       5 svc                      3 svc       3 svc
       └─────────────────────── 15 expected services total ──────────────────────┘
```

**Why this split?** Multistatic fusion (combining CSI from 4 spatial vantage
points into a single weighted observation) is computationally cheap but
benefits from being on **one** node so the other three only do capture +
encode. Hailo-8 is the bottleneck cost, so we put two on the cluster
(one for redundancy, one for the fusion node) and let `cluster-3` + `v0`
run as pure capture sensors.

---

## 3. Per-node firmware: nexmon_csi on Pi 5

**Critical lesson learned (saved you a week):** the workstation x86_64
cross-compile path for nexmon_csi on Pi 5 **does not work**. The 39-hunk
patch series applies cleanly on a native Pi 5 ARM build, and fails in
subtle ways elsewhere.

The recipe that works:

```bash
# On each Pi 5 (not the workstation):
sudo apt update && sudo apt install -y \
    raspberrypi-kernel-headers bc bison flex libssl-dev make \
    gcc gawk qpdf cmake build-essential libpcap-dev clang gcc-arm-none-eabi

git clone https://github.com/seemoo-lab/nexmon.git ~/nexmon
cd ~/nexmon
source setup_env.sh
make

cd patches
git clone https://github.com/seemoo-lab/nexmon_csi.git
cd nexmon_csi

# Apply the Pi-5-friendly patch series — all 39 hunks should apply clean
# on native ARM. If you see "Hunk #N FAILED", you are almost certainly
# cross-compiling from x86_64. Stop. Build on the Pi.
./install.sh

# Switch on:
sudo mcp                       # 'monitor capability provisioning' — enable
sudo nexutil -Iwlan0 -s500 -b -l34 -v<86-char base64 capture filter>
```

> **Pi 5 kernel gotcha:** Pi OS Bookworm ships two kernels — `kernel8.img`
> (4K pages) and `kernel_2712.img` (16K pages, Pi 5 only). nexmon_csi
> currently builds clean against `kernel8.img`. Add `kernel=kernel8.img`
> to `/boot/firmware/config.txt` if you've switched. **After the switch,
> SSH by hostname via Tailscale** — host keys + DHCP gotchas otherwise.

> **Clock-skew first-boot trap:** Pi 5 has no RTC. First-boot apt will
> reject "future-dated" `Release` files. Patch your firstboot to wait for
> `systemd-timesyncd` before running `apt-get`.

The complete commands + full troubleshooting matrix is in the
[detailed gist](https://gist.github.com/ruvnet/88e7b053c41cb4f4af7a7ec4af873017) — section "Firmware: nexmon_csi on Pi 5".

---

## 4. Per-node services

Each cluster Pi runs a small fixed set of systemd services. Per-host
topology:

| Service | cluster-1 | cluster-2 | cluster-3 | v0 |
|---|:--:|:--:|:--:|:--:|
| `cog-csi-emitter` (raw CSI capture from nexmon) | ✓ | ✓ | ✓ | ✓ |
| `cog-csi-adapter` (Rust binary; CSI → 256-byte float frames) | ✓ | ✓ | ✓ | ✓ |
| `cog-rvcsi-stream` (publishes frames to rvcsi-correlator) | ✓ | ✓ | ✓ | ✓ |
| `cog-hailo-encoder` (frames → 128-d fingerprints on Hailo-8) | ✓ | ✓ | — | — |
| `cog-rvcsi-correlator` (multistatic fusion across 4 nodes) | — | ✓ | — | — |
| **Expected service count** | **4** | **5** | **3** | **3** |

The topology is encoded in the workstation's `cog-query fleet-status`
subcommand, which compares per-host expected services against live
`systemctl is-active` results. A flat-service check would falsely flag
cluster-3 and v0 as degraded (they have neither Hailo nor the correlator
— that's by design).

> **rvcsi cutover (ADR-207 Option D, 2026-05-13).** The three services
> `cog-csi-emitter`, `cog-csi-adapter`, and `cog-rvcsi-stream` are
> being consolidated into one Rust binary `cog-rvcsi-pi` built on
> [rvcsi-runtime](https://crates.io/crates/rvcsi-runtime). The new
> binary holds the same per-Pi role and the same expected-service
> count from the operator's view (`fleet-status` already understands
> both layouts). Deploy with
> `bash scripts/rvcsi-pi/install-rvcsi-pi.sh <pi-host>`; revert with
> `scripts/rvcsi-pi/uninstall-rvcsi-pi.sh`. The cutover is per-Pi,
> not flag-day — mixed Python/Rust clusters are supported. The Hailo
> encoder + correlator stay Python in this phase; their Rust ports
> are tracked as follow-on ADRs.

All unit files + the install script are in the
[detailed gist](https://gist.github.com/ruvnet/88e7b053c41cb4f4af7a7ec4af873017) — section "Per-node systemd units".

---

## 5. Workstation pipeline

The workstation runs ten user-mode units (3 daemons, 7 timers):

| Unit | Type | Cadence | Purpose |
|---|---|---|---|
| `cog-rfmem-tail` | daemon | continuous | Ingests live brain entries into the workstation mirror |
| `cog-rfmem-recall` | daemon | continuous | kNN-matches current fingerprint vs persisted ones, posts `rfmem-recall` |
| `cog-rfmem-anomaly` | daemon | continuous | 13-axis anomaly detector, posts `rfmem-anomaly` + `rfmem-state-transition` |
| `cog-rfmem-indexer` | timer | every 5 min | Updates HNSW index for kNN |
| `cog-rfmem-compress` | timer | hourly | Compresses old brain entries |
| `cog-rfmem-daily` | timer | nightly 04:00 | Per-day stats roll-up (`rfmem-daily`) |
| `cog-rfmem-states` | timer | hourly | Re-runs cosine k-means w/ warm-start (`rfmem-state-summary`) |
| `cog-rfmem-insights` | timer | nightly 04:55 | NL synthesis, posts `rfmem-insights` |
| `cog-rfmem-drift-check` | timer | nightly 05:00 | Audits cluster file/unit drift, posts `rfmem-drift` |
| `cog-rfmem-mirror` | timer | hourly | Mirrors cluster-2 brain → workstation read-replica |

Install in one shot:

```bash
git clone https://github.com/<your-fork>/v0-appliance.git
cd v0-appliance
bash scripts/rfmem/install-workstation.sh
```

The installer is **idempotent** — rerunning is safe and only enables
units that aren't yet enabled. It also wires a git post-commit hook
that auto-deploys + auto-smoke-tests on every commit touching
`scripts/rfmem/`. That closes the "I edited the repo but forgot to
deploy" gap that bit us repeatedly in early development.

---

## 6. Calibration: getting from raw CSI to room states

This is the longest step but largely passive — let it run.

### 6.1 Walk the room

For 30–60 minutes after the cluster is live, walk through every room you
want recognized. Sit, stand, move between rooms, repeat. The encoder is
learning to map "what the room looks like in CSI" into 128-d vectors;
diversity here matters more than total time.

### 6.2 First clustering pass

```bash
# Force-trigger the clusterer (it normally fires hourly):
systemctl --user start cog-rfmem-states.service
python3 scripts/rfmem/cog-query.py states
```

Output looks like:

```
=== rfmem-states — k=16, n=12,847 ===
  state #0   π=0.184  dwell=42.3s  centroid_drift=0.012  (default)
  state #1   π=0.121  dwell=18.1s  centroid_drift=0.003
  state #4   π=0.087  dwell=29.6s  centroid_drift=0.041
  ...
```

**Stable IDs across runs.** The warm-start k-means recipe matches new
centroids to the prior run's centroids by cosine similarity before
assigning IDs. This means state #4 stays state #4 between hourly runs —
otherwise downstream Markov transitions would scramble after every
re-cluster.

### 6.3 Let the Markov chain build

After a few thousand transitions (a few hours of activity), check:

```bash
python3 scripts/rfmem/cog-query.py prediction-accuracy
```

You should see something like:

```
=== prediction-accuracy — training-set top-1 ceilings ===
  1st-order:  37.1%  (16x chance baseline of 6.25%)
  2nd-order:  39.4%  (16x chance baseline of 6.25%, 1.06x gain over 1st)
```

The 2nd-order chain beats 1st-order because it conditions on the
**previous** state as well as the current one. Self-loops are excluded
from the argmax (a transition is by definition a state change).

### 6.4 Verify the room learned itself

```bash
python3 scripts/rfmem/cog-query.py insights
```

Reads like:

```
The cluster has observed 446,231 fingerprints, clustering them into
16 discrete RF states. The room exhibits moderately diverse (stationary
entropy 0.82/1.0). State #4 is the dominant 'default' state (π=0.214);
state #13 is the rarest baseline (π=0.018).
Prediction skill (last hour, 2nd-order): top-1 12.4% (1.98x chance),
top-3 31.0% (1.65x chance, 412 transitions) (training-set ceiling
39.4% — operating @ 31% of capacity).
```

That "operating @ 31% of capacity" line is the operational efficiency:
how close live performance is to the model's theoretical ceiling. Big
gap = the room is being noisy in ways the static cluster model doesn't
capture. Small gap = you're near SOTA for this static model.

---

## 7. Operating the cluster: the cog-query CLI

A single CLI binary with **34 subcommands** + 4 machine-readable JSON
modes. Practical ones (full list in the gist):

| Subcommand | What it does |
|---|---|
| `summary --hours 1` | Bird's-eye view of last hour: anomalies, transitions, recall hits |
| `top-events --hours 24 --limit 5` | Highest-info events in window (combines novelty + tier + recency) |
| `top-events --json` | Same, agent-consumable |
| `insights` | Natural-language synthesis (paragraph) — what the cluster thinks |
| `insights --json` | Same, structured |
| `insights --post` | Same, persisted to brain as `rfmem-insights` |
| `stats` | Corpus: per-category counts, dimensions, vector counts |
| `motion` | Recent motion events |
| `anomalies --sort info` | Anomalies sorted by composite info score (1.0–8.0) |
| `circadian` | 24-hour bin of activity — does the room have a daily rhythm? |
| `by-state` | Per-state metrics (dwell, σ-baseline, novelty distribution) |
| `markov` | Top transitions by frequency, both 1st + 2nd-order |
| `transitions --sort novelty` | Rare/surprising transitions |
| `dwell-times` | How long the room stays in each state |
| `prediction-accuracy` | 1st + 2nd-order top-1 ceilings |
| `baseline-drift` | Has the noise floor shifted? (slow change) |
| `centroid-drift` | Has any state's RF signature materially changed? |
| `fleet-status` | Per-host expected-service liveness check |
| `fleet-status --json` | Same, agent-consumable |
| `fleet-status --post` | Same, persisted to brain as `rfmem-fleet` (heartbeat) |
| `check-drift` | Workstation/cluster file + unit drift audit |
| `replica-status` | Hourly cluster-2 → workstation mirror health |

### The fleet-health triad

Three subcommands cover the operator's full health picture:

- `check-drift` — file content drift (what's deployed vs what's in git)
- `replica-status` — workstation mirror lag (last successful sync)
- `fleet-status` — service liveness across the 4 Pis (topology-aware)

If all three are green, the cluster is healthy. If any one fires, you
have a concrete starting point.

---

## 8. What you can measure

After a week of runtime, you can answer questions like:

- **"What's the room's most common 'baseline' state?"** → `states` shows
  the π-dominant cluster ID.
- **"Did anything weird happen last night?"** → `anomalies --sort info
  --hours 12` sorts by combined-information score (novelty × tier × state-
  rarity × calmness).
- **"How predictable is the room?"** → `insights` reports stationary
  entropy (0.0 = single state, 1.0 = uniform). Most rooms land 0.6–0.9.
- **"What's the most novel transition ever observed?"** → `transitions
  --sort novelty --limit 1`. We've seen transitions with
  `transition_p=0.0000` — never observed before in 446k+ embeddings.
- **"Is the room changing slowly?"** → `centroid-drift` flags states
  whose 128-d signature has moved > 0.05 cosine distance since the prior
  clusterer run. Common cause: a piece of furniture moved.
- **"What's the daily rhythm?"** → `circadian` bins activity by hour.
  Most rooms show clear morning/evening peaks.

---

## 9. Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `nexmon_csi` build fails with FAILED hunks | Cross-compiling from x86_64 | Build on the Pi natively |
| Pi 5 stops booting after kernel switch | Wrong `kernel=` in `/boot/firmware/config.txt` | Use `kernel=kernel8.img` |
| First boot fails on `apt update` | No RTC → clock skew, apt rejects "future-dated" Release files | Wait for `systemd-timesyncd` in firstboot |
| `cog-rfmem-now` times out | Workstation daemon swap-thrashing | Bump `MemoryMax=` in unit file (we run 1G) |
| `fleet-status` shows DEGRADED on cluster-3 / v0 | Topology unaware (old version) | Update to latest — per-host expected-services |
| Cluster-2 Hailo encoder silent | `cp -r` made encoder a directory, not a file | `install -m 0755` instead |
| 2nd-order Markov top-1 = 0% | Self-loop dominates argmax | Zero out self-loop before `.argmax()` |
| State IDs change between runs | No warm-start k-means | Update clusterer to match new centroids to prior run by cosine |
| HardFaults during embedded N6 bring-up | (Different topic, see [ADR-027](../adr/) for STM32N6 startup notes) | — |

---

## 10. Next steps

Once your cluster is producing stable predictions and clean fleet health,
the natural directions are:

1. **Cross-room correlation** — train a second cluster in another room
   and feed both into the workstation. The brain already supports
   multiple namespaces.
2. **Active sensing** — instead of passively observing whatever beacon is
   present, drive your own (e.g., dedicated 5 GHz beacon AP at fixed
   power). Eliminates upstream variability.
3. **Vital signs** — the RuView project has companion code for extracting
   heart-rate and breathing from CSI; the 128-d encoder output is a
   reasonable input feature.
4. **Federated training** — multiple physical sites publishing to a shared
   brain. Each site keeps its own clusters; transitions are the shared
   vocabulary.
5. **Push to upstream RuView** — if your cluster develops capabilities not
   in this tutorial (you'll know by the time you've written the README),
   send a PR.

---

## Reference material

- **[Detailed cookbook gist (all commands, configs, unit files)](https://gist.github.com/ruvnet/88e7b053c41cb4f4af7a7ec4af873017)**
- **[ADR-206: nexmon_csi on Pi 5 cluster](https://github.com/ruvnet/v0-appliance/blob/main/docs/adr/ADR-206-nexmon-csi-on-pi-5-cluster.md)** — the engineering decision record
  with full rationale, including the painful-but-instructive failures
- **[v0-appliance repo](https://github.com/ruvnet/v0-appliance)** — the
  source of truth for `scripts/rfmem/` operator tooling
- **[seemoo-lab/nexmon_csi](https://github.com/seemoo-lab/nexmon_csi)** —
  upstream CSI capture firmware
- **[Hailo-8 documentation](https://hailo.ai/products/hailo-8/)** — NPU
  reference

---

*This tutorial was built against the v0.5.0-cognitive-rf-observer milestone
of `v0-appliance`. The cluster has been running continuously for 6+ weeks
of development with 446k+ fingerprints observed, 16 stable RF states, and
a 2nd-order Markov model operating at 31% of its 39.4% theoretical
top-1 ceiling. SOTA is a moving target — but this is a real, working
cognitive RF observer that you can reproduce.*
