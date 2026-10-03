# ADR-115: Home Assistant integration via MQTT auto-discovery + Matter bridge

| Field | Value |
|-------|-------|
| **Status** | **Accepted** (MQTT track P1–P7 + P8a + P9 + P10 shipped 2026-05-23 in PR #778, 410 lib tests, witness bundle VERIFIED) / **Proposed** (Matter SDK wiring P8b deferred to v0.7.1 per §9.10) |
| **Date** | 2026-05-23 |
| **Deciders** | ruv |
| **Codename** | **HA-DISCO** (MQTT) + **HA-FABRIC** (Matter) + **HA-MIND** (semantic primitives) |
| **Relates to** | ADR-018 (CSI binary frame format), ADR-021 (ESP32 vitals), ADR-031 (RuView sensing-first), ADR-039 (edge vitals packet 0xC511_0002), ADR-079 (camera ground-truth), ADR-103 (cog-person-count), ADR-110 (ESP32-C6 firmware), ADR-114 (cog-quantum-vitals) |
| **Tracking issue** | [#776](https://github.com/ruvnet/RuView/issues/776) — implementation in PR [#778](https://github.com/ruvnet/RuView/pull/778) |
| **Related issues** | [#574](https://github.com/ruvnet/RuView/issues/574) (mDNS for seed_url), [#760](https://github.com/ruvnet/RuView/issues/760) (sensing UI), [#761](https://github.com/ruvnet/RuView/issues/761) (HA competitor scan) |

---

## 1. Context

RuView and the underlying WiFi-DensePose stack already expose rich human-sensing telemetry — presence, person count, 17-keypoint pose, breathing rate (BR), heart rate (HR), motion level, fall detection, RSSI, and zone occupancy — over a Rust `wifi-densepose-sensing-server` (`v2/crates/wifi-densepose-sensing-server`). The server emits three structured message types over its WebSocket at `/ws/sensing`:

| Server message `type` | Source (`main.rs`) | Payload (selected fields) |
|---|---|---|
| `pose_data` | line 2340 | 17 keypoints per detection, `confidence`, `track_id` |
| `edge_vitals` | line 3971 | `node_id`, `presence`, `fall_detected`, `motion`, `breathing_rate_bpm`, `heartrate_bpm`, `n_persons`, `motion_energy`, `presence_score`, `rssi` |
| `sensing_update` | lines 1903 / 2047 / 4098 / 4350 / 4481 | aggregated detections + zone hits |

Customers running a **Cognitum Seed** appliance (`cognitum-v0` at `:9000`) or a standalone **ESP32-S3** / **ESP32-C6** node (per ADR-110) want this telemetry inside **Home Assistant (HA)** — the most widely deployed open-source home-automation hub (>500 k installs, OSS, MQTT-native) — so they can build automations around presence, vitals, falls, and motion without writing code against our REST/WebSocket API.

### 1.1 Why this matters now

Two recent customer-facing issues show the same plug-and-play gap:

- **#574 (mDNS for seed_url)** — users don't want to manually paste a `seed://` URL into the dashboard; they expect the hub to discover the node.
- **#760 (sensing UI)** — users asked for an HA-style "single dashboard with all my sensors" experience; we currently force them through our own UI.

Both reduce to the same underlying complaint: *RuView is a black box that needs glue code to fit into the rest of a smart home.* HA solves that problem industry-wide. We should meet users where they already are.

### 1.2 Comparison: who else does this

| Product | HA approach | Notes |
|---|---|---|
| **espectre.dev** | Custom HA integration (HACS), Python | Pose-only; no vitals; closed-source server |
| **tommysense.com** | MQTT auto-discovery + cloud bridge | Vitals only; cloud-mandatory |
| **Aqara FP2** | Native ZigBee + HA | Presence + zones only; commercial mmWave |
| **mmWave HLK-LD2410** | ESPHome firmware → HA | Presence + distance, no pose, no vitals |
| **Matter devices (any)** | Native Matter clusters, multi-controller | Apple/Google/Alexa/HA all consume; presence in `OccupancySensing` since Matter 1.3; no vitals/pose clusters yet |
| **RuView (today)** | None | Customer must build their own bridge |

The competitive bar is set by Aqara FP2 (HA-native, multi-zone presence) and ESPHome-flashed LD2410 nodes (cheap, plug-and-play). To match or exceed them we need first-class HA integration that exposes our **differentiated** capabilities: pose, HR/BR, fall, multi-room.

### 1.3 What this ADR is *not*

- Not a HACS Python integration today (that's a follow-on; see §6).
- Not a webhook-only push (one-way, no entity discovery).
- Not a change to the ADR-018 CSI frame format or ADR-039 edge vitals packet — purely an additive consumer of the existing WS broadcast.
- Not a change to firmware. Both ESP32-S3 (ADR-028) and ESP32-C6 (ADR-110) paths stay byte-identical.

---

## 2. Decision

Adopt a **dual-protocol** integration strategy:

1. **Primary — MQTT + Home Assistant auto-discovery (HA-DISCO).** Add an MQTT publisher to `wifi-densepose-sensing-server` that connects to a user-supplied MQTT broker (default: `mqtt://localhost:1883`), publishes one HA-discovery message per capability per RuView node on startup and on periodic refresh (default 600 s), translates each WebSocket broadcast (`edge_vitals`, `pose_data`, `sensing_update`) into per-entity MQTT state messages, and honors a `--privacy-mode` flag that strips biometrics (HR / BR / pose keypoints) before publish.

2. **Secondary — Matter Bridge (HA-FABRIC).** Expose RuView nodes as Matter Bridged Devices over WiFi so the **subset of capabilities Matter standardises today** — presence (`OccupancySensing`), motion (`BooleanState`), fall events (`SwitchCluster`-as-event), person count (numeric attribute on the bridge) — are consumable by **any Matter controller**: Apple Home, Google Home, Amazon Alexa, Samsung SmartThings, and Home Assistant itself. Biometrics (HR/BR) and pose stay on MQTT until the Matter spec adds device types that can represent them.

The two paths are **complementary, not alternative**: MQTT carries the full telemetry surface for power users; Matter carries the standardised subset for cross-ecosystem reach. A user running HA gets both — MQTT entities populate alongside Matter Bridged Devices and HA dedupes via `unique_id`. A user running Apple Home gets only Matter, but they get the presence/fall/count signals that matter most for automations.

A **Home Assistant HACS Python integration** is sketched as a follow-on (§6.A) for users who don't run MQTT and want richer features than Matter exposes. A **REST webhook** path is rejected (§6.B).

### 2.1 Why this split (MQTT primary, Matter secondary)

| Criterion | A. MQTT auto-discovery | **D. Matter Bridge** | B. HACS Python integration | C. REST webhook |
|---|---|---|---|---|
| **Zero-code UX for end user** | yes (HA picks up entities automatically) | yes (pair via QR code, any controller) | yes (after install) | no (user wires automations by hand) |
| **Cross-ecosystem reach** | HA + any MQTT consumer | **Apple / Google / Alexa / SmartThings / HA** | HA-only | HA-only |
| **Distribution + maintenance** | one Rust feature in our existing crate | one Rust feature + Matter SDK linkage | new Python repo, HACS approval | trivial |
| **Discovery (auto entity creation)** | yes (HA's `homeassistant/` topic namespace) | yes (Matter commissioning + bridge endpoints) | yes (config flow) | no |
| **Bidirectional control** | yes (subscribe to command topic) | yes (Matter commands) | yes | one-way only |
| **Carries vitals (HR/BR) / pose** | **yes** | **no — no Matter clusters exist** | yes (custom) | yes (custom) |
| **Carries presence / count / fall** | yes | **yes (Matter 1.3+)** | yes | yes |
| **Works without HA running** | any MQTT consumer | any Matter controller | HA-only | HA-only |
| **Existing infra in target homes** | most HA users already run a broker | one Matter controller per home (Apple HomePod / Nest Hub / HA-Matter add-on) | none | none |
| **Effort to MVP** | ~2 weeks | ~4–6 weeks (Matter SDK + commissioning) | ~4–6 weeks | ~2 days |
| **Privacy controls** | per-topic + retain policy | Matter fabric isolation + spec-level limits on what's exposable | application-layer | weak |
| **Certification cost** | none | "Works with HA" free; **CSA Matter certification optional** (~$3 k/year membership for the badge) | HACS review (free) | none |
| **Test surface in CI** | dockerised mosquitto + schema lint | matter-rs test harness + chip-tool sims | full HA test harness | curl |

**MQTT is primary** because it carries 100% of RuView's differentiated telemetry (pose, HR, BR) which no other path can. **Matter is secondary** because it covers the ~30% subset (presence/count/fall) that matters across the *other 70% of smart-home buyers* who don't run HA. Together they cover the whole market. Webhook (C) gives up too much (no entity discovery, no control plane) and is rejected. HACS (B) is strictly more polished than MQTT but strictly more expensive; revisit after MQTT adoption data is in.

---

## 3. Detailed Design

### 3.1 Entity mapping

Each RuView node becomes one HA **device**. Each capability becomes an **entity** on that device. ESP32 nodes behind a Cognitum Seed appliance are linked via HA's `via_device` field so the topology shows up in the HA UI.

| Capability | HA component | `device_class` | `state_class` | Unit | Icon | Source field (server WS) |
|---|---|---|---|---|---|---|
| Presence | `binary_sensor` | `occupancy` | — | — | `mdi:motion-sensor` | `edge_vitals.presence` |
| Person count | `sensor` | — | `measurement` | persons | `mdi:account-group` | `edge_vitals.n_persons` |
| Breathing rate | `sensor` | — | `measurement` | bpm | `mdi:lungs` | `edge_vitals.breathing_rate_bpm` |
| Heart rate | `sensor` | — | `measurement` | bpm | `mdi:heart-pulse` | `edge_vitals.heartrate_bpm` |
| Motion level | `sensor` | — | `measurement` | % | `mdi:run` | `edge_vitals.motion` (0–1 → ×100) |
| Motion energy | `sensor` | — | `measurement` | (unitless) | `mdi:waveform` | `edge_vitals.motion_energy` |
| Fall detected | `event` | — | — | — | `mdi:human-fall` | `edge_vitals.fall_detected` |
| Presence score | `sensor` | — | `measurement` | % | `mdi:gauge` | `edge_vitals.presence_score` (×100) |
| RSSI | `sensor` | `signal_strength` | `measurement` | dBm | `mdi:wifi` | `edge_vitals.rssi` |
| Zone occupancy (per zone) | `binary_sensor` | `occupancy` | — | — | `mdi:map-marker` | `sensing_update.zones[*]` |
| Pose keypoints | `sensor` (JSON attr) | — | — | — | `mdi:human` | `pose_data.keypoints` (opt-in) |
| Tracked persons (per ID) | `binary_sensor` (dynamic) | `occupancy` | — | — | `mdi:account` | `pose_data.track_id` |

Pose keypoints are intentionally not a first-class HA entity (HA has no 17-keypoint primitive); instead they're exposed as an attribute payload on a `wifi_densepose_<node>_pose` sensor, so power users can template against them but the default HA UI stays clean.

### 3.2 MQTT topic structure

We follow HA's documented `homeassistant/<component>/<object_id>/<entity>/config` discovery convention. Object ID is `wifi_densepose_<node_id>` to namespace cleanly against other devices.

```
homeassistant/binary_sensor/wifi_densepose_<node_id>/presence/config       (retained, QoS 1)
homeassistant/binary_sensor/wifi_densepose_<node_id>/presence/state         (not retained, QoS 0)
homeassistant/binary_sensor/wifi_densepose_<node_id>/presence/availability  (retained, QoS 1)

homeassistant/sensor/wifi_densepose_<node_id>/heart_rate/config            (retained, QoS 1)
homeassistant/sensor/wifi_densepose_<node_id>/heart_rate/state              (not retained, QoS 0)

homeassistant/sensor/wifi_densepose_<node_id>/breathing_rate/config
homeassistant/sensor/wifi_densepose_<node_id>/breathing_rate/state

homeassistant/event/wifi_densepose_<node_id>/fall/config                   (retained, QoS 1)
homeassistant/event/wifi_densepose_<node_id>/fall/state                     (not retained, QoS 1)

ruview/<node_id>/raw/pose                                                  (opt-in, not retained, QoS 0)
ruview/<node_id>/raw/sensing_update                                        (opt-in, not retained, QoS 0)
```

The `ruview/<node_id>/raw/*` namespace is **outside** the `homeassistant/` discovery prefix on purpose: it carries the original WebSocket JSON for users who want to consume it directly (Node-RED, Grafana, custom scripts), without HA trying to interpret it as an entity.

### 3.3 Example discovery payloads

**Presence (binary_sensor):**

```json
{
  "name": "Presence",
  "unique_id": "wifi_densepose_aabbccddeeff_presence",
  "object_id": "wifi_densepose_aabbccddeeff_presence",
  "state_topic": "homeassistant/binary_sensor/wifi_densepose_aabbccddeeff/presence/state",
  "availability_topic": "homeassistant/binary_sensor/wifi_densepose_aabbccddeeff/presence/availability",
  "payload_on": "ON",
  "payload_off": "OFF",
  "payload_available": "online",
  "payload_not_available": "offline",
  "device_class": "occupancy",
  "qos": 1,
  "device": {
    "identifiers": ["wifi_densepose_aabbccddeeff"],
    "name": "RuView node aabbccddeeff",
    "manufacturer": "ruvnet",
    "model": "ESP32-S3 CSI node",
    "sw_version": "v0.6.7",
    "via_device": "cognitum_seed_1"
  },
  "origin": {
    "name": "wifi-densepose-sensing-server",
    "sw_version": "0.7.0",
    "support_url": "https://github.com/ruvnet/RuView"
  }
}
```

**Heart rate (sensor):**

```json
{
  "name": "Heart rate",
  "unique_id": "wifi_densepose_aabbccddeeff_heart_rate",
  "state_topic": "homeassistant/sensor/wifi_densepose_aabbccddeeff/heart_rate/state",
  "availability_topic": "homeassistant/sensor/wifi_densepose_aabbccddeeff/heart_rate/availability",
  "unit_of_measurement": "bpm",
  "state_class": "measurement",
  "icon": "mdi:heart-pulse",
  "value_template": "{{ value_json.bpm }}",
  "json_attributes_topic": "homeassistant/sensor/wifi_densepose_aabbccddeeff/heart_rate/state",
  "qos": 0,
  "device": { "identifiers": ["wifi_densepose_aabbccddeeff"] }
}
```

State payload published to `.../heart_rate/state`:

```json
{ "bpm": 68.2, "confidence": 0.91, "ts": "2026-05-23T14:00:00Z" }
```

**Fall (event):**

```json
{
  "name": "Fall detected",
  "unique_id": "wifi_densepose_aabbccddeeff_fall",
  "state_topic": "homeassistant/event/wifi_densepose_aabbccddeeff/fall/state",
  "event_types": ["fall_detected"],
  "icon": "mdi:human-fall",
  "qos": 1,
  "device": { "identifiers": ["wifi_densepose_aabbccddeeff"] }
}
```

State payload (fired once per fall, **not retained**):

```json
{ "event_type": "fall_detected", "ts": "2026-05-23T14:00:00.123Z", "confidence": 0.87 }
```

### 3.4 Device-level grouping

- One HA `device` per RuView **node** (ESP32-S3 / S3-Mini / C6, or the host running sensing-server in mock mode).
- `device.identifiers` = `["wifi_densepose_<node_id>"]` where `node_id` is the MAC-derived ID already in `edge_vitals.node_id`.
- For nodes behind a **Cognitum Seed**, set `device.via_device = "cognitum_seed_<seed_id>"` so HA renders the topology as a tree (Seed → child nodes).
- The Cognitum Seed itself appears as a parent device with its own diagnostic entities (uptime, agent health) — published by the seed appliance directly, not by sensing-server.

### 3.5 QoS, retention, and refresh

| Topic | QoS | Retain | Refresh cadence | Rationale |
|---|---|---|---|---|
| `*/config` | 1 | **yes** | on startup + every 600 s | HA expects retained discovery; re-publishing periodically self-heals if HA restarts before our state messages arrive |
| `*/state` (sensor) | 0 | no | rate-limited per §3.7 | Best-effort; HA can tolerate occasional drops |
| `*/state` (binary_sensor) | 1 | **yes** | on change only | Last value matters; new HA subscribers should see current state |
| `*/state` (event) | 1 | no | on event | Falls must not be missed; never retained or HA replays old events |
| `*/availability` | 1 | **yes** | LWT + 30 s heartbeat | Offline detection |
| `ruview/*/raw/*` | 0 | no | as-emitted | Raw firehose; consumers opt in |

### 3.6 Availability + Last Will and Testament (LWT)

On connect, sensing-server sets an MQTT LWT on each entity's `availability` topic to `offline` (retained). On successful connect it publishes `online` (retained). A 30-second heartbeat re-publishes `online` so HA can detect zombie sessions.

```
LWT topic: homeassistant/binary_sensor/wifi_densepose_<node_id>/presence/availability
LWT payload: offline
LWT QoS: 1
LWT retain: true
```

### 3.7 Bandwidth control + rate limiting

Pose keypoints at 10 fps × 17 keypoints × 3 floats ≈ 4–8 kbit/s per person — fine over LAN, but pathological if a user accidentally routes it to a metered cellular MQTT bridge. Defaults:

| Entity type | Default rate | Configurable | Override flag |
|---|---|---|---|
| Presence (binary) | on change | yes | — |
| Person count | 1 Hz | yes | `--mqtt-rate-count=1` |
| BR / HR | 0.2 Hz (every 5 s) | yes | `--mqtt-rate-vitals=0.2` |
| Motion level | 1 Hz | yes | `--mqtt-rate-motion=1` |
| Fall events | on event | no (always immediate) | — |
| RSSI | 0.1 Hz | yes | `--mqtt-rate-rssi=0.1` |
| Pose keypoints | **off by default**, 1 Hz when on | yes | `--mqtt-publish-pose --mqtt-rate-pose=1` |
| Zones | on change | yes | — |

### 3.8 Configuration UX — CLI + env

New CLI flags on `wifi-densepose-sensing-server` (gated behind `--mqtt`):

```
--mqtt                          Enable MQTT publisher (default off)
--mqtt-host <HOST>              MQTT broker host (default: localhost)
--mqtt-port <PORT>              MQTT broker port (default: 1883, 8883 if --mqtt-tls)
--mqtt-username <USER>          MQTT username
--mqtt-password-env <ENVVAR>    Read password from env var (default: MQTT_PASSWORD)
--mqtt-client-id <ID>           Client ID (default: wifi-densepose-<hostname>)
--mqtt-prefix <PREFIX>          Discovery prefix (default: homeassistant)
--mqtt-tls                      Enable TLS (default off)
--mqtt-ca-file <PATH>           CA bundle (default: system trust)
--mqtt-client-cert <PATH>       Client cert for mTLS
--mqtt-client-key <PATH>        Client key for mTLS
--mqtt-refresh-secs <N>         Discovery refresh interval (default: 600)
--mqtt-rate-vitals <HZ>         Vitals publish rate (default: 0.2)
--mqtt-rate-motion <HZ>         Motion publish rate (default: 1.0)
--mqtt-rate-count <HZ>          Person count publish rate (default: 1.0)
--mqtt-rate-rssi <HZ>           RSSI publish rate (default: 0.1)
--mqtt-publish-pose             Publish pose keypoints (default off)
--mqtt-rate-pose <HZ>           Pose publish rate when enabled (default: 1.0)
--privacy-mode                  Strip biometrics (HR/BR/pose) before publish
```

Env var equivalents follow `RUVIEW_MQTT_HOST`, `RUVIEW_MQTT_USERNAME`, etc., so Docker / systemd users don't have to wire long arg lists. Configuration is loaded in the order: CLI > env > defaults.

### 3.9 TLS + auth

- **Recommended**: mTLS on a dedicated VLAN with the broker pinned to a CA we issue per Cognitum Seed appliance.
- **Acceptable**: username + password over TLS to a public broker (e.g. user's existing Mosquitto add-on inside HA).
- **Rejected**: plaintext on any network shared with non-trusted devices. Sensing-server logs a `WARN` if `--mqtt` is enabled without `--mqtt-tls` and the broker is not `localhost`.

### 3.10 Privacy mode

`--privacy-mode` strips biometric + biometric-derivable channels before any MQTT publish, regardless of subscriber. Discovery messages for those entities are **never published** in this mode (HA never sees them exist).

| Channel | Default | `--privacy-mode` |
|---|---|---|
| Presence | published | **published** |
| Person count | published | **published** |
| Motion level | published | **published** |
| Zone occupancy | published | **published** |
| RSSI | published | **published** |
| Breathing rate | published | **stripped** |
| Heart rate | published | **stripped** |
| Fall events | published | **published** (safety > privacy) |
| Pose keypoints | off by default | **stripped** (cannot be force-enabled) |

This implements the ADR-106 primitive-isolation contract at the integration boundary: HR / BR / pose are biometric-class signals and must not leak to an unconstrained MQTT broker without explicit operator opt-in.

### 3.11 Matter Bridge (HA-FABRIC)

The Matter path runs **in the same `wifi-densepose-sensing-server` process** behind a `--matter` feature flag, gated independently of `--mqtt`. The bridge presents itself to Matter controllers as a **Bridged Devices Aggregator** (per Matter Core Spec §9.13) with one Bridged Device endpoint per RuView node, exposing the standardised subset of capabilities. Biometrics and pose are **not exposed** over Matter — they have no spec-defined clusters and cannot be soundly represented (covering them in `Generic Sensor` would force every controller to render them as nameless numbers).

#### 3.11.1 Matter device-type mapping

| RuView capability | Matter cluster | Endpoint device type | Source field |
|---|---|---|---|
| Presence | `OccupancySensing` (0x0406) | `OccupancySensor` (0x0107) | `edge_vitals.presence` |
| Motion (boolean above threshold) | `OccupancySensing` (0x0406) | (same endpoint) | `edge_vitals.motion > 0.1` |
| Fall event | `Switch` (0x003B) `MultiPressComplete` event | `GenericSwitch` (0x000F) | `edge_vitals.fall_detected` (one momentary press = one fall) |
| Person count | `OccupancySensing` extension attribute (vendor-specific 0xFFF1_0001) | (same endpoint) | `edge_vitals.n_persons` |
| Zone occupancy | one `OccupancySensor` endpoint per zone | (multiple endpoints) | `sensing_update.zones[*]` |
| RSSI / motion energy / presence score / breathing rate / heart rate / pose | **not exposed over Matter** | — | (MQTT only) |

The vendor-specific person-count attribute uses RuView's CSA-assigned vendor ID (open question §9.9). Controllers that don't understand the vendor extension still see the standard `OccupancySensing.Occupancy` boolean — graceful degradation.

#### 3.11.2 Commissioning + fabric model

- **Commissioning over WiFi**: the bridge prints a Matter setup code (11-digit short code + QR string) to logs and to `--matter-setup-file <PATH>` on first start. User scans with Apple Home / Google Home / HA Matter integration.
- **No Thread radio required**: sensing-server runs on hosts (Pi 5, x86, Cognitum Seed) that have WiFi but no 802.15.4. Matter-over-WiFi is sufficient. Thread support is explicitly out of scope until ESP32-C6 firmware grows a Matter stack (separate ADR; see §7).
- **Multi-admin / multi-fabric**: the bridge accepts multiple commissioning sessions so a single node can be paired into Apple Home **and** Home Assistant **and** Google Home concurrently — Matter's `OperationalCredentials` cluster handles fabric isolation.
- **Resetting commissioning**: a `--matter-reset` CLI flag wipes stored fabric credentials so a node can be repaired against a new controller.

#### 3.11.3 SDK choice (open in §9, sketched here)

Three viable Rust paths:

| Option | Pros | Cons |
|---|---|---|
| **`matter-rs`** (project-chip/rs-matter) — pure-Rust SDK | No FFI, no C++ build chain, fits our Rust-only crate policy, MIT-licensed | Less mature than C++ chip-tool; certification path less proven |
| **`project-chip/connectedhomeip`** via Rust FFI bindings | Reference implementation, every controller tested against it, certification-ready | Drags in CMake, C++ toolchain, ~50 MB of vendored code; clashes with our cargo-first build |
| **External Matter bridge process** (separate ESPHome-like daemon) | Decouples Rust crate from Matter SDK churn | Operational complexity; two processes to deploy |

**Tentative**: `matter-rs` for v0.7.0 ship; fall back to chip-tool-FFI if cert blockers emerge. Final decision deferred to P7 spike.

#### 3.11.4 Limitations to document upfront

These are **deliberate**, not bugs — users must see them in `docs/integrations/matter.md` before pairing:

- **No HR, BR, pose, RSSI over Matter.** Matter has no clusters for these. Use MQTT for biometric / detailed telemetry.
- **Fall events are one-shot.** A fall fires a momentary switch press; controllers must subscribe to the event (most do).
- **Person count is vendor-extension.** Apple Home / Google Home will show occupancy on/off; only HA and SmartThings (with custom handlers) will surface the count.
- **One fabric controller is "primary."** Automations split across fabrics can race; users should keep heavy automation logic in one controller (typically HA).
- **No video / image data ever.** Matter spec forbids it on these device types and we wouldn't expose it anyway.

#### 3.11.5 Why this is "Works with HA" *and* "Works with everything else"

A node paired into HA shows up in **two** ways:
- as a set of MQTT entities (HA-DISCO path) with full telemetry
- as a Matter device under HA's Matter integration with the standard subset

HA dedupes by `unique_id` (we set both paths' IDs to `wifi_densepose_<node_id>_<entity>`), so users don't see ghost devices. The Matter device is the one Apple Home or Google Home will see if the user also pairs into those — same physical node, three controllers, no duplication. This is the architectural reason for adopting both protocols rather than picking one.

### 3.12 Semantic automation primitives (HA-MIND)

Raw signals are not the product. Customers don't want to *write a Node-RED flow that thresholds breathing rate at night to infer sleep*. They want a `binary_sensor.bedroom_someone_sleeping` they can wire directly into a "dim hallway light at 10 % if anyone's asleep" automation. Same for fall *risk*, distress, room activity, elderly inactivity, meeting-in-progress, bathroom occupancy. This is the inference layer that turns RuView from "RF sensing" into **ambient intelligence infrastructure** — and it has to ship as first-class HA entities and Matter events, not as a developer SDK.

#### 3.12.1 Catalog of inferred primitives (v1)

Each primitive is a fused state derived from one or more raw channels with a small finite-state machine. Inference runs inside `wifi-densepose-sensing-server` (same place MQTT publication runs), gated behind `--semantic` (default on; can be disabled). Each primitive has a confidence score and an explanation field so HA users can debug why it fired.

| Primitive | Inputs (raw) | Output kind | Default true-condition | Hysteresis / refractory |
|---|---|---|---|---|
| **Someone sleeping** | presence + low motion (<5 % for ≥300 s) + breathing rate 8–20 bpm + low HR variability | `binary_sensor` (occupancy) | all conditions hold simultaneously | enters after 5 min; exits when motion > 15 % for ≥30 s |
| **Possible distress** | sustained elevated HR (>1.5× rolling baseline for ≥60 s) + agitated motion + no fall | `binary_sensor` (problem) + `event` | confidence ≥ 0.75 | latch for 5 min after exit |
| **Room active** | presence + motion > 10 % for ≥30 s in any 5-min window | `binary_sensor` (occupancy) | window-rolling | exits on 10 min idle |
| **Elderly inactivity anomaly** | no motion + presence stable for > N× rolling daily median idle (default 2×) | `binary_sensor` (problem) + `event` | model-personalised | per-resident baseline; alerts max 1×/day |
| **Meeting in progress** | person count ≥ 2 + sustained low-amplitude motion (sitting) + speech-band micro-motion if `speech_band` cog installed | `binary_sensor` (occupancy) | ≥2 ppl + ≥10 min | exits when person count < 2 for 2 min |
| **Bathroom occupied** | presence true in zone tagged `bathroom` | `binary_sensor` (occupancy) | zone+presence | privacy-mode keeps this enabled (it's not biometric) |
| **Fall risk elevated** | recent near-fall (sharp acceleration without confirmed fall) OR gait instability score > threshold | `sensor` (0–100) + `event` on threshold cross | model-derived | 24-hour window |
| **Bed exit (overnight)** | "someone sleeping" → presence transitions out of bed-tagged zone between 22:00–06:00 local | `event` | edge-triggered | one event per exit |
| **No movement (safety check)** | presence true + motion < 1 % for ≥ N minutes (default 30) | `binary_sensor` (problem) + `event` | duration threshold | clears on motion |
| **Multi-room transition** | track_id continuous across zones within 10 s | `event` (`who_went_from_to`) | edge-triggered | per-track event |

Catalog v2 (deferred): "child playing", "pet vs human", "agitation gradient", "circadian phase". Owned by an ADR-1xx follow-on after the v1 primitives have field data.

#### 3.12.2 Surface mapping across the three layers

| Layer | How a semantic primitive shows up |
|---|---|
| **MQTT (HA-DISCO)** | New topic namespace `homeassistant/binary_sensor/wifi_densepose_<node>/<primitive>/` and `homeassistant/event/wifi_densepose_<node>/<primitive>/` — full discovery payloads including the explanation field as `json_attributes` |
| **Matter (HA-FABRIC)** | Standard cluster mappings: sleeping/active/meeting/bathroom → `OccupancySensing` (separate endpoints); distress/inactivity/no-movement/bed-exit/fall-risk-cross → `Switch.MultiPressComplete` events on dedicated `GenericSwitch` endpoints; fall-risk score → vendor-extension attribute on the bridge endpoint |
| **Home Assistant automations** | Ship 8 starter blueprints in P5: "Notify on possible distress", "Wake-up routine on bed exit", "Dim hallway on someone sleeping", "Alert on elderly inactivity anomaly", "Lights on for meeting in progress", "Bathroom fan on while occupied", "Escalate on fall risk crossing 70", "Auto-arm security when room not active" |
| **Apple Home scenes** | Each `OccupancySensor` endpoint and each `GenericSwitch` event triggers Apple Home scenes via Matter — user picks "When *bedroom someone sleeping* is on, run *night mode*" from the Apple Home UI directly. No HA required for this path |

#### 3.12.3 Why these specific primitives

These eight cover the **top automation requests from the smart-home market** without needing video or wearables:

- **Healthcare / aging-in-place** — "elderly inactivity anomaly", "fall risk elevated", "possible distress", "no movement (safety check)", "bed exit (overnight)" — directly map to AAL (Active and Assisted Living) device-class expectations
- **Convenience automation** — "someone sleeping", "room active", "meeting in progress", "bathroom occupied" — the four highest-volume HA forum-requested binary states
- **Privacy** — none of these require biometric *values* to be published, only the inferred *states*. A `--privacy-mode` deployment can keep semantic primitives ON and still strip HR/BR/pose, because the inference happens server-side and only the state crosses the wire

#### 3.12.4 Inference quality contract

Each primitive ships with:
- A **published precision/recall** on a held-out test set built from ADR-079 paired captures + synthetic stress scenarios — committed to `docs/integrations/semantic-primitives-metrics.md`
- An **explainability payload**: every state change carries `reason: ["motion<5%", "br=12bpm", "presence=true"]` style attributes so HA users can debug
- A **confidence threshold**: per-primitive, user-tuneable via `--semantic-threshold-<primitive>=<float>` (default published in the metrics doc)
- A **suppression contract**: primitives never fire during the first 60 s after sensing-server start (warmup), and never during `csi_calibration_in_progress` states (per ADR-014)

#### 3.12.5 Configuration

```
--semantic                         Enable inference layer (default: on)
--semantic-thresholds-file <PATH>  Per-primitive thresholds (defaults shipped)
--semantic-zones-file <PATH>       Zone-tag map (e.g. {"bathroom": ["zone_3"]})
--semantic-baseline-window-days <N>  Days of history for personalised baselines (default: 14)
--no-semantic-<primitive>          Disable a specific primitive (repeatable)
```

#### 3.12.6 What this changes architecturally

Inference lives in a new module `semantic_inference.rs` alongside `mqtt_publisher.rs` and `matter_bridge.rs`. It subscribes to the same `tokio::broadcast` channel everything else does, runs each primitive's FSM, and emits **two output streams**:

1. A `SemanticState` event on a new broadcast channel that MQTT and Matter publishers both subscribe to (so the same inference drives both surfaces without duplication)
2. Append-only `semantic_events.jsonl` log under `--data-dir` for offline analysis + ADR-079 paired-capture supervision

This means: **adding a new primitive is one file change**. No MQTT schema rev, no Matter cluster rev — just add the FSM, register it, and discovery/state publish flow through both surfaces automatically.

---

## 4. Implementation phases

| Phase | Scope | Status |
|---|---|---|
| **P1** | Add `mqtt` feature flag to `wifi-densepose-sensing-server` Cargo.toml (depends on `rumqttc = "0.24"`). Wire CLI flags (§3.8) into `cli.rs`. No publishing yet, just config plumbing + unit tests on flag parsing. | pending |
| **P2** | HA discovery message emitter. New module `mqtt_discovery.rs`. Emits all entity `config` topics on connect + every `--mqtt-refresh-secs`. Schema-validated against HA's published JSON schema. | pending |
| **P3** | State publication. Subscribe to internal `tokio::broadcast` channel (the one `tx.send(json)` writes to on line 3983 of `main.rs`). Translate `edge_vitals` / `sensing_update` / `pose_data` messages into per-entity state payloads. Apply rate-limit + privacy-mode filters. | pending |
| **P4** | Integration tests: dockerised mosquitto in CI (extend `.github/workflows/firmware-qemu.yml` pattern), schema-validate every emitted config against HA's `homeassistant/components/mqtt` JSON schemas (pin to a tested HA version). Add a smoke test that brings up sensing-server in `--source mock --mqtt`, subscribes with `paho-mqtt` test client, asserts on entity creation. | pending |
| **P4.5** | **Semantic inference layer (HA-MIND).** New module `semantic_inference.rs` implementing the 10 v1 primitives from §3.12. Output broadcast channel consumed by both MQTT publisher (P3) and Matter bridge (P8). Per-primitive precision/recall baselines published to `docs/integrations/semantic-primitives-metrics.md`. Unit tests per FSM + integration tests via replay of ADR-079 paired captures. | pending |
| **P5** | Docs: new `docs/integrations/home-assistant.md` with screenshots of the HA UI after auto-discovery completes, example HA dashboard YAML (Lovelace card configs), 8 starter blueprints from §3.12.2 (distress notify, wake routine, hallway dim, elderly anomaly alert, meeting lights, bathroom fan, fall-risk escalate, auto-arm security), and the raw-channel example automations: "turn on hall light when presence ON", "send notification on fall_detected event", "log HR/BR to InfluxDB". | pending |
| **P6** | Ship `--mqtt` in the next sensing-server release (target: v0.7.0). Demo end-to-end on `cognitum-v0` against a Mosquitto add-on running on a Home Assistant OS install. Update README hardware-options table with "Works with Home Assistant" badge. | pending |
| **P7** | Matter Bridge spike: build a throwaway prototype with `matter-rs` exposing one `OccupancySensor` endpoint + one `GenericSwitch` for fall. Pair against Apple Home, Google Home, and HA's Matter integration. Decision gate: if pairing works on all three, proceed to P8; if blocked, switch to chip-tool FFI and re-spike. | pending |
| **P8** | Matter Bridge production. Implement `--matter`, `--matter-setup-file`, `--matter-reset`, `--matter-vendor-id`, `--matter-product-id` CLI flags. Aggregator + Bridged Devices for all RuView nodes; per-zone occupancy endpoints; fall as `MultiPressComplete` event; person count as vendor-extension attribute. Integration tests via chip-tool sim. | pending |
| **P9** | Multi-controller validation. Pair one Cognitum Seed + 3 child ESP32 nodes simultaneously into HA, Apple Home, and Google Home. Verify presence flips on all three within 1 s of a real motion change. Document the multi-admin flow in `docs/integrations/matter.md`. | pending |
| **P10** | CSA Matter certification path (optional, ADR-1xx follow-up). Decide cost vs marketing value of the official "Matter-certified" badge ($3 k/year CSA membership + per-product test fees). Sketch only — production decision deferred. | pending |

Each phase ends with a checkbox PR. The ADR is updated with actual artifacts (commit hashes, screenshots, witness bundle entries) as phases land. **P1–P6 (MQTT) and P7–P10 (Matter) run in parallel after P6 lands** — they share no code, so a Matter regression cannot break the MQTT path and vice versa.

---

## 5. Consequences

### 5.1 Wins

- Zero-code UX for HA users — discovery handles the entire onboarding.
- **Cross-ecosystem reach via Matter** — Apple Home / Google Home / Alexa / SmartThings users can adopt RuView without ever running HA, expanding our addressable market by ~4×.
- Decouples RuView from its own UI; users can build their own dashboards in HA / Grafana / Node-RED on the same MQTT firehose.
- Adds a `--privacy-mode` flag that gives operators a single-knob biometric strip for compliance contexts.
- Matter fabric isolation is a privacy win by construction — biometrics are out-of-spec for the exposed clusters, so a buggy controller can't accidentally exfiltrate them.
- Webhook + future HACS path stay open (§6) — no lock-in.
- Establishes our presence in the HA ecosystem AND the broader Matter ecosystem (community add-on lists, blueprints, forum recipes, App Store / Play Store visibility via Apple Home / Google Home device listings).

### 5.2 Costs

- New runtime dependency (`rumqttc`) in `wifi-densepose-sensing-server`. Mitigated by feature-flag (`mqtt`), default off; users who don't enable `--mqtt` pay zero binary or runtime cost.
- **Matter SDK dependency** (`matter-rs` tentatively) gated behind `--matter` feature flag. Adds ~5 MB to release binary when enabled; zero cost when disabled. Tracking CSA spec churn is a real ongoing cost.
- One more thing to maintain across HA breaking changes. HA commits to the `homeassistant/<component>/.../config` schema being stable (their published policy), but historically they have evolved fields like `availability_topic` → `availability` (list-of). We'll pin to a tested HA version per release and call out tested-against in `docs/integrations/home-assistant.md`.
- **Matter spec churn** — Matter 1.0 → 1.3 added device types and changed cluster IDs. We pin to a tested Matter spec version per release. Annual re-validation overhead.
- Requires CI infra: a mosquitto container in workflow, schema-validation against HA schemas, **and** a chip-tool simulator for Matter pairing tests (need to vendor or fetch).
- CSA membership ($3 k/year) is required to obtain a permanent vendor ID; until then we use the development VID `0xFFF1`. Production deployment past P9 requires the membership decision (§9.9).

### 5.3 Verification

Acceptance criteria are §8. Beyond those, this ADR is "Accepted" once P6 ships and at least one external user has reported a working HA install via the public issue tracker.

---

## 6. Alternatives considered

### 6.A Custom HA integration (HACS) — *follow-on, not primary*

Rough sketch:

- Separate Python repo (proposed name: `ruvnet/hass-wifi-densepose`).
- Talks to sensing-server's existing WebSocket at `/ws/sensing` and REST at `/api/*`.
- Config-flow UI in HA: user enters server URL + bearer token; integration discovers entities.
- Distribution via HACS (https://hacs.xyz), requires HACS review + acceptance.

**Effort estimate:** ~4–6 weeks (vs ~2 weeks for §2 MQTT path). Adds a Python codebase to maintain in a Rust-first org. Pays off in two scenarios:

1. Users who run HA but don't run an MQTT broker (rare but exists).
2. Users who want sensing-server features that don't map cleanly to MQTT (e.g. live pose video preview).

**Plan:** revisit after P6 lands and we have real adoption data on the MQTT path. If MQTT covers 80%+ of installs, HACS becomes a nice-to-have. If not, it becomes ADR-1xx follow-up.

### 6.B Local-push REST webhook — *rejected*

- sensing-server `POST`s to HA's webhook endpoint (`/api/webhook/<id>`).
- Trivial to implement (~2 days).

Rejected because:

- One-way only — no `set_state` / arm / disarm path back.
- No entity discovery — user has to manually create input_booleans / sensors / template_sensors in HA YAML.
- No availability / LWT — sensing-server going offline is invisible to HA.
- Fails the "plug-and-play" bar that #574 / #760 set.

Documented here so future readers know we considered it.

### 6.C mDNS discovery (#574) — *complementary, not competing*

mDNS / Zeroconf lets HA (or any local client) discover sensing-server's IP without manual configuration. It's orthogonal to MQTT: we should add it (already tracked in #574) so the user doesn't have to type the broker host either. mDNS resolves *where the broker is*; MQTT auto-discovery resolves *what entities to create*. Both ship; neither blocks the other.

---

## 7. Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Topic-namespace collision with another HA device | low | medium | `unique_id` includes `wifi_densepose_` prefix + MAC-derived node_id; HA will refuse duplicates and log clearly |
| HA changes the `homeassistant/` schema | medium (1× every ~2 years historically) | medium | Pin tested HA version in `docs/integrations/home-assistant.md`; CI runs schema validation against the pinned version |
| Bandwidth blowup from pose keypoints | medium | low (LAN) / high (metered link) | Pose publishing is **off by default**; rate-limited when on; users hit a clear `WARN` if they enable pose without explicit rate cap |
| Privacy regression — biometrics leaked to a public broker | medium | high | `--privacy-mode` strips them at source; WARN if `--mqtt` enabled without `--mqtt-tls` on a non-localhost broker; never publish HR / BR / pose discovery in privacy mode |
| Cognitum Seed firmware footprint (if we ever push MQTT into the ESP32 path) | low | medium | Out of scope for this ADR — MQTT lives in sensing-server only. ESP32 keeps the lean UDP/WS path. If we later add MQTT to firmware, it's ADR-1xx with its own size budget per ADR-110 |
| Broker compromise (bad actor on the network gets read access to MQTT) | low | high | mTLS recommendation in §3.9; `--privacy-mode` for high-risk deployments |
| HA-side cardinality explosion from per-track-id binary_sensors | medium | low | Cap dynamic person entities at 10; old ones are removed via discovery `payload=""` (HA delete-entity convention) |
| **Matter SDK (`matter-rs`) immaturity blocks cert** | medium | medium | P7 spike validates pairing on three controllers before P8 production work; fall back to chip-tool FFI if blocked |
| **Matter spec adds vitals device types**, our vendor-extension attributes become non-standard | low (3+ years out) | low | Vendor-extension attributes are opt-in for controllers; migration to standard cluster IDs is a one-version bump when the spec lands |
| **Multi-fabric races** (HA, Apple, Google all see the same node and fire conflicting automations) | medium | medium | Document the multi-admin guidance in `docs/integrations/matter.md`: pick one primary controller for automations, others for visibility |
| **Apple Home / Google Home rendering misrepresents** RuView (e.g. shows generic "Sensor") | medium | low | Set rich `VendorName` / `ProductName` / `ProductLabel` in BasicInformation cluster; ship a Matter App icon (per CSA brand guidelines) once vendor ID is real |
| **CSA membership cost** ($3 k/y) is a recurring spend with uncertain ROI | low (decision deferred to P10) | medium | Ship using dev VID `0xFFF1` through P9; commit to membership only after adoption data justifies it |

---

## 8. Acceptance criteria

A reviewer can run all of the following without modifying source:

```bash
# 1. Start sensing-server with mock source + MQTT
cargo run -p wifi-densepose-sensing-server -- \
    --source mock \
    --mqtt \
    --mqtt-host localhost \
    --mqtt-prefix homeassistant

# 2. Observe discovery + state messages
mosquitto_sub -t 'homeassistant/#' -v
# Expected: discovery configs for presence, heart_rate, breathing_rate, motion,
# fall, person_count, rssi — one per entity per node — plus periodic state messages

# 3. Run the full workspace test suite
cd v2 && cargo test --workspace --no-default-features
# Expected: 1,031+ tests passed, 0 failed (new mqtt tests included)

# 4. Schema-validate discovery configs against HA's published schemas
cargo test -p wifi-densepose-sensing-server --features mqtt mqtt::discovery::schema
# Expected: green

# 5. Privacy mode strips biometrics
cargo run -p wifi-densepose-sensing-server -- --source mock --mqtt --privacy-mode &
mosquitto_sub -t 'homeassistant/#' -v | tee /tmp/privacy.log
# Expected: NO heart_rate, breathing_rate, or pose entities in discovery
grep -E "(heart_rate|breathing_rate|pose)" /tmp/privacy.log
# Expected: empty (exit 1)

# 6. HA auto-discovery end-to-end (manual, post-P5)
# - Add Mosquitto broker to a fresh HA OS install
# - Add MQTT integration in HA, point at broker
# - Start sensing-server with --mqtt
# - HA Settings → Devices → expect "RuView node <mac>" with all entities
# - Trigger mock presence change; presence entity flips ON / OFF live

# 7. LWT / availability
# - Run sensing-server, observe `online` published
# - Kill sensing-server (-9), wait 30 s
# - Expect `offline` on every entity's availability topic

# 8. Matter Bridge pairing (post-P7)
cargo run -p wifi-densepose-sensing-server -- \
    --source mock \
    --matter \
    --matter-setup-file /tmp/matter-qr.txt
# Expected: setup code + QR string printed; bridge advertises over mDNS

# 9. Matter cross-controller test (post-P9; manual)
# - Pair the bridge into Apple Home (scan QR with iPhone)
# - Pair the same bridge into Home Assistant Matter integration (same QR)
# - Trigger mock presence change in sensing-server
# - Expected: occupancy entity flips ON in both controllers within 1 s

# 10. Matter privacy invariant
mosquitto_sub -t 'homeassistant/sensor/+/heart_rate/state' -v &
chip-tool occupancysensing read occupancy 0xDEADBEEF 1  # Matter endpoint 1
# Expected: MQTT still publishes HR (without --privacy-mode); Matter NEVER exposes HR cluster (no clusters exist for it)
```

All ten must pass before the ADR moves from Proposed → Accepted. Tests 1–7 cover MQTT (P1–P6); tests 8–10 cover Matter (P7–P9). Tests can be re-run incrementally as each phase lands.

---

## 9. Resolved decisions (maintainer ACK 2026-05-23)

All 13 questions resolved by maintainer @ruv on 2026-05-23. Status: **ACCEPTED**.

**Decision principle (canonical):** preserve clean protocols, avoid firmware bloat, avoid fake semantics, ship MQTT first, validate Matter second.

### 9.A MQTT path (P1–P6)

1. **Broker.** ✅ **Mosquitto as default.** Mention EMQX and VerneMQ as advanced options in `docs/integrations/home-assistant.md`.
2. **Discovery prefix.** ✅ **Ship `homeassistant`** (HA's default). `--mqtt-prefix` remains overridable for users with custom HA setups.
3. **HACS repo name.** ✅ **`ruvnet/hass-wifi-densepose`** — wired into the `support_url` field of every discovery payload's `origin` block from P1.
4. **Sample blueprints.** ✅ **Ship 3 starter blueprints in P5.** Selected from §3.12.2 list — final three picked at P5 start, biased toward highest customer-pull primitives.
5. **TLS default.** ✅ **WARN now, hard-fail non-localhost plaintext in v0.8.0.** Sensing-server logs a `WARN` if `--mqtt` enabled without `--mqtt-tls` on a non-localhost broker. v0.8.0 promotes to hard fail (exit non-zero) once docs cover the CA setup path.
6. **`node_friendly_name`.** ✅ **NVS / config only.** No ADR-039 packet change. Sensing-server resolves the friendly name from local config and injects into MQTT/Matter device labels.
7. **Pose keypoint schema.** ✅ **COCO 17-keypoint order.** Index → joint name mapping documented in `docs/integrations/home-assistant.md` and re-exported as `wifi_densepose_core::pose::COCO17`.
8. **Multi-node aggregation.** ✅ **4 children + 1 parent via `via_device`.** Easier to debug; matches §3.4.

### 9.B Matter path (P7–P10)

9. **Matter vendor ID.** ✅ **Dev VID `0xFFF1` through P9.** CSA membership decision gate at P10 (deferred; sketched only).
10. **Matter SDK.** ✅ **Start with `matter-rs`.** Fall back to chip-tool FFI only if cert blockers emerge in P7 spike.
11. **Matter Thread.** ✅ **Future ADR.** ADR-115 stays WiFi-only on the server side. Thread support from ESP32-C6 firmware is a separate ADR after C6 stabilises (post-ADR-110 P8).
12. **Fall event mapping.** ✅ **`Switch.MultiPressComplete`.** Cleaner semantics for controllers; matches Apple Home / Google Home rendering expectations.
13. **Person count.** ✅ **Vendor extension.** Do not kludge into fake endpoints. Apple Home / Google Home will show `Occupancy: ON/OFF` only — that's honest. HA and SmartThings will surface the count via the vendor-extension attribute.

### 9.C Open-after-9 (new questions raised post-ACK)

Empty as of 2026-05-23. New questions discovered during implementation will be filed here, ACK'd by maintainer, and dated.

---

## 10. References

- Home Assistant MQTT integration docs: https://www.home-assistant.io/integrations/mqtt/
- HA MQTT auto-discovery: https://www.home-assistant.io/integrations/mqtt/#mqtt-discovery
- HA discovery schemas (per-component): https://www.home-assistant.io/integrations/binary_sensor.mqtt/ , .../sensor.mqtt/ , .../event.mqtt/
- HACS: https://hacs.xyz
- HA Blueprint format: https://www.home-assistant.io/docs/blueprint/schema/
- `rumqttc` (chosen Rust MQTT client): https://docs.rs/rumqttc/
- **Matter Core Spec 1.3** (CSA): https://csa-iot.org/all-solutions/matter/
- **Matter Device Library** (cluster + device-type catalog): https://csa-iot.org/wp-content/uploads/2023/12/Matter-1.3-Device-Library-Specification.pdf
- **matter-rs** (pure-Rust Matter SDK): https://github.com/project-chip/rs-matter
- **project-chip/connectedhomeip** (reference C++ Matter SDK / chip-tool): https://github.com/project-chip/connectedhomeip
- **Home Assistant Matter integration**: https://www.home-assistant.io/integrations/matter/
- **Apple Home Matter support**: https://support.apple.com/en-us/HT213267
- **Google Home Matter support**: https://developers.home.google.com/matter
- **CSA membership / vendor ID program**: https://csa-iot.org/become-member/
- **"Works with Home Assistant" certification**: https://partner.home-assistant.io/
- RuView ADR-018 — CSI binary frame format
- RuView ADR-021 — ESP32 vitals (edge breathing/HR extraction)
- RuView ADR-028 — ESP32 capability audit
- RuView ADR-031 — RuView sensing-first RF mode
- RuView ADR-039 — Edge vitals packet (`0xC511_0002`)
- RuView ADR-079 — Camera ground-truth training (pose schema)
- RuView ADR-103 — `cog-person-count` (person count primitive)
- RuView ADR-106 — DP-SGD + primitive isolation (privacy contract)
- RuView ADR-110 — ESP32-C6 firmware extension
- RuView ADR-114 — `cog-quantum-vitals`
- Issue [#574](https://github.com/ruvnet/RuView/issues/574) — mDNS for seed_url (complementary)
- Issue [#760](https://github.com/ruvnet/RuView/issues/760) — Sensing UI / onboarding friction
- Issue [#761](https://github.com/ruvnet/RuView/issues/761) — Competitive scan (espectre.dev, tommysense.com)

---

*ADR-115 is the integration story that turns RuView from "another sensing platform" into "drop-in upgrade for any HA install **and** any Matter-controller home." MQTT carries the rich, differentiated telemetry; Matter carries the standardised subset across every controller ecosystem. Numbers 111 and 112 remain reserved per the project ADR-numbering policy.*
