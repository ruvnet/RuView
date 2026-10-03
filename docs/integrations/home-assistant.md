# Home Assistant integration

RuView publishes its full WiFi-sensing capability set to **Home Assistant** via MQTT auto-discovery (HA-DISCO) and to **any Matter controller** (Apple Home / Google Home / Alexa / SmartThings / HA) via a built-in Matter Bridge (HA-FABRIC). This document is the operator guide for both paths. Design rationale: [ADR-115](../adr/ADR-115-home-assistant-integration.md).

> **Tested against** Home Assistant Core **2025.5**, Mosquitto add-on **6.4**, and Matter (chip-tool) **1.3**. Bump the matrix when you change tested versions.

---

## Quick start

### 1. Prereqs

- A running **MQTT broker** on your LAN. The easiest path is the [Mosquitto add-on](https://github.com/home-assistant/addons/tree/master/mosquitto) inside Home Assistant OS (one click from the Add-on Store). EMQX and VerneMQ also work — see §Advanced brokers below.
- Home Assistant **2025.5 or newer** with the MQTT integration enabled and pointed at your broker.
- A RuView **`wifi-densepose-sensing-server`** v0.7.0+ binary (or `cargo run` from source).

### 2. Start the publisher

```bash
# Docker (recommended for non-developers):
docker run --rm --net=host \
    ruvnet/wifi-densepose:0.7.0 \
    --source esp32 \
    --mqtt --mqtt-host 192.168.1.10 \
    --mqtt-username homeassistant --mqtt-password-env MQTT_PASSWORD

# Or from a source checkout (Rust 1.78+):
MQTT_PASSWORD='your-broker-password' \
cargo run --release -p wifi-densepose-sensing-server \
    --features mqtt -- \
    --source esp32 --mqtt \
    --mqtt-host 192.168.1.10 \
    --mqtt-username homeassistant
```

Within ~5 seconds of starting, Home Assistant should auto-create:

- One **device** per RuView node (named after the MAC or the `friendly_name` from your zones config)
- 17+ **entities** per device (presence, person count, heart rate, breathing rate, motion, fall events, signal strength, zones, and the 10 semantic primitives)

If nothing appears in HA's Settings → Devices, see [Troubleshooting](#troubleshooting).

### 3. Stop the publisher cleanly

Ctrl-C — the publisher pushes `offline` to every availability topic before disconnect so HA marks all entities unavailable instantly. A `kill -9` triggers MQTT LWT, which has the same effect within ~30 s.

---

## Entity reference

RuView publishes three classes of entity. Names below are the `unique_id` slugs — Home Assistant assigns friendly names automatically.

### Raw signals (11 entities)

| HA entity | Slug | HA component | Unit | Source field |
|---|---|---|---|---|
| Presence | `presence` | `binary_sensor` | — | `edge_vitals.presence` |
| Person count | `person_count` | `sensor` | persons | `edge_vitals.n_persons` |
| Heart rate | `heart_rate` | `sensor` | bpm | `edge_vitals.heartrate_bpm` |
| Breathing rate | `breathing_rate` | `sensor` | bpm | `edge_vitals.breathing_rate_bpm` |
| Motion level | `motion_level` | `sensor` | % | `edge_vitals.motion` × 100 |
| Motion energy | `motion_energy` | `sensor` | (dimensionless) | `edge_vitals.motion_energy` |
| Fall detected | `fall` | `event` | — | `edge_vitals.fall_detected` |
| Presence score | `presence_score` | `sensor` | % | `edge_vitals.presence_score` × 100 |
| Signal strength | `rssi` | `sensor` | dBm | `edge_vitals.rssi` |
| Zone occupancy | `zone_occupancy` | `binary_sensor` | — | `sensing_update.zones` |
| Pose keypoints | `pose` | `sensor` (attrs) | — | `pose_data.keypoints` (opt-in via `--mqtt-publish-pose`) |

Heart rate, breathing rate, and pose are **biometric** entities — they are stripped from MQTT (and never published over Matter) when `--privacy-mode` is set. See [Privacy](#privacy) below.

### Semantic automation primitives (10 entities)

These are the inferred high-level states that customer automations actually use. Each one is a small finite-state machine running server-side with explicit warmup, hysteresis, and refractory windows. Per-primitive precision/recall is published in [`semantic-primitives-metrics.md`](./semantic-primitives-metrics.md).

| HA entity | Slug | HA component | What it fires on |
|---|---|---|---|
| Someone sleeping | `someone_sleeping` | `binary_sensor` | presence + motion<5% + BR ∈ [8,20] bpm sustained for 5 min |
| Possible distress | `possible_distress` | `binary_sensor` | HR > 1.5× baseline + motion >20% + no fall, sustained 60 s |
| Room active | `room_active` | `binary_sensor` | motion >10% in a 30-s rolling window |
| Elderly inactivity anomaly | `elderly_inactivity_anomaly` | `binary_sensor` | idle > 2× observed-max-idle baseline |
| Meeting in progress | `meeting_in_progress` | `binary_sensor` | ≥2 persons + low-amplitude motion for 10 min |
| Bathroom occupied | `bathroom_occupied` | `binary_sensor` | presence + active zone tagged `bathroom` |
| Fall risk elevated | `fall_risk_elevated` | `sensor` | 0–100 score; event fires on ≥70 crossing |
| Bed exit (overnight) | `bed_exit` | `event` | sleeping → presence leaves bed zone between 22:00–06:00 |
| No movement (safety) | `no_movement` | `binary_sensor` | presence + motion <1% for 30 min |
| Multi-room transition | `multi_room_transition` | `event` | zone X exit + zone Y enter within 10 s |

Every state change carries a `reason` attribute (e.g. `["motion<5%", "br=12bpm", "presence=true"]`) so you can template against it in HA automations to understand why an automation triggered.

### Matter device-type mapping

Per ADR-115 §3.11.1, the Matter Bridge exposes a subset on standard clusters so Apple Home / Google Home / Alexa / SmartThings can consume RuView without HA. Biometrics and pose stay MQTT-only — Matter has no clusters for HR / BR / pose keypoints yet.

| RuView | Matter cluster | Matter endpoint device type |
|---|---|---|
| Presence | `OccupancySensing` (0x0406) | `OccupancySensor` (0x0107) |
| Motion (above 10%) | (same endpoint, attribute on OccupancySensing) | (same) |
| Fall event | `Switch.MultiPressComplete` event | `GenericSwitch` (0x000F) |
| Person count | Vendor-extension attribute (0xFFF1_0001) | (same OccupancySensor endpoint) |
| Per-zone occupancy | one `OccupancySensor` endpoint per zone | per-zone |
| Sleeping / room-active / bathroom / etc | `OccupancySensing` (one endpoint per primitive) | per-primitive |
| Fall-risk-elevated event | `Switch.MultiPressComplete` event | `GenericSwitch` |
| HR / BR / pose | **not exposed** — MQTT only | — |

---

## Configuration

### CLI matrix

| Flag | Default | Purpose |
|---|---|---|
| `--mqtt` | off | Enable the HA-DISCO publisher |
| `--mqtt-host <HOST>` | `localhost` | Broker host |
| `--mqtt-port <PORT>` | 1883 (8883 with TLS) | Broker port |
| `--mqtt-username <U>` | — | Username for broker auth |
| `--mqtt-password-env <VAR>` | `MQTT_PASSWORD` | Env var holding the password |
| `--mqtt-client-id <ID>` | `wifi-densepose-<hostname>` | MQTT client ID |
| `--mqtt-prefix <PREFIX>` | `homeassistant` | Discovery topic prefix |
| `--mqtt-tls` | off | Encrypt connection |
| `--mqtt-ca-file <PATH>` | — | Pinned CA for TLS / mTLS |
| `--mqtt-client-cert <PATH>` | — | Client cert for mTLS |
| `--mqtt-client-key <PATH>` | — | Client key for mTLS |
| `--mqtt-refresh-secs <N>` | 600 | Discovery re-emit interval |
| `--mqtt-rate-vitals <HZ>` | 0.2 | HR / BR publish rate (Hz) |
| `--mqtt-rate-motion <HZ>` | 1.0 | Motion publish rate (Hz) |
| `--mqtt-rate-count <HZ>` | 1.0 | Person-count publish rate (Hz) |
| `--mqtt-rate-rssi <HZ>` | 0.1 | RSSI publish rate (Hz) |
| `--mqtt-publish-pose` | off | Enable pose-keypoint publication |
| `--mqtt-rate-pose <HZ>` | 1.0 | Pose publish rate when enabled |
| `--privacy-mode` | off | Strip HR/BR/pose from MQTT, Matter, REST, WebSocket and recordings |
| `--matter` | off | Enable the HA-FABRIC Matter Bridge |
| `--matter-setup-file <PATH>` | — | Where to write the QR + manual code |
| `--matter-reset` | off | Wipe fabric credentials and re-commission |
| `--matter-vendor-id <VID>` | `0xFFF1` (dev) | CSA-assigned vendor ID |
| `--matter-product-id <PID>` | `0x8001` | Product ID |
| `--semantic` | on | Enable inference layer |
| `--semantic-thresholds-file <PATH>` | — | Per-primitive threshold overrides |
| `--semantic-zones-file <PATH>` | — | Zone-tag map (`bathroom`, `bedroom`, …) |
| `--no-semantic <PRIMITIVE>` | — | Disable a specific primitive (repeatable) |

### Zone tag file format

```yaml
# semantic-zones.yaml — passed to --semantic-zones-file
zones:
  bathroom: ["zone_3", "zone_7"]
  bedroom:  ["zone_1"]
  kitchen:  ["zone_2"]
  living:   ["zone_5"]
bed_zones: ["zone_1"]
```

### Threshold overrides

```yaml
# semantic-thresholds.yaml — passed to --semantic-thresholds-file
sleep_dwell_secs: 300
distress_hr_multiple: 1.5
room_active_motion_threshold: 0.10
elderly_anomaly_multiple: 2.0
meeting_min_persons: 2
no_movement_dwell_secs: 1800
fall_risk_event_threshold: 70.0
```

---

## Privacy

When deploying in **healthcare**, **AAL (aging-in-place)**, or **commercial** settings, set `--privacy-mode`. This:

- **Strips** heart rate, breathing rate, and pose keypoints from every outbound MQTT publication.
- **Suppresses discovery** for those entities entirely — HA never even sees they exist.
- **Applies to the rest of the server too.** The same fields are removed from REST responses, WebSocket frames and recordings, so another client on the HTTP or WebSocket port can't read what MQTT withholds. See [Privacy mode](../user-guide.md#privacy-mode-for-healthcare--aal) in the user guide.
- **Keeps every semantic primitive enabled.** Sleeping / distress / room-active / etc are *inferred* states. The inference happens server-side and only the boolean or score crosses the wire. This is the architectural win that makes the platform deployable in regulated contexts.

Always pair `--privacy-mode` with `--mqtt-tls` on non-localhost brokers.

---

## Three starter blueprints

Drop these YAML files into `<HA config>/blueprints/automation/ruvnet/` and import them from the HA UI (Settings → Automations → Blueprints → Import).

### 1. Notify on possible distress

```yaml
blueprint:
  name: RuView — notify on possible distress
  description: >
    Send a push notification when RuView detects sustained elevated heart
    rate + agitated motion (possible distress).
  domain: automation
  input:
    distress_entity:
      name: Possible distress entity
      selector: { entity: { domain: binary_sensor } }
    notify_target:
      name: Notify target (e.g. notify.mobile_app_pixel)
      selector: { text: {} }

trigger:
  - platform: state
    entity_id: !input distress_entity
    to: "on"

action:
  - service: !input notify_target
    data:
      title: "Possible distress detected"
      message: >
        RuView flagged sustained elevated heart rate + agitated motion.
        Reason: {{ state_attr(trigger.entity_id, 'reason') }}.
```

### 2. Dim hallway when someone is sleeping

```yaml
blueprint:
  name: RuView — dim hallway when someone sleeping
  description: >
    Drop hallway lights to 10 % brightness when anyone in the bedroom is
    in the someone-sleeping state, so a midnight bathroom trip doesn't
    require full lights.
  domain: automation
  input:
    sleeping_entity:
      name: Someone sleeping entity
      selector: { entity: { domain: binary_sensor } }
    hallway_light:
      name: Hallway light
      selector: { entity: { domain: light } }

trigger:
  - platform: state
    entity_id: !input sleeping_entity
    to: "on"
  - platform: state
    entity_id: !input sleeping_entity
    to: "off"

action:
  - choose:
      - conditions:
          - condition: state
            entity_id: !input sleeping_entity
            state: "on"
        sequence:
          - service: light.turn_on
            target: { entity_id: !input hallway_light }
            data: { brightness_pct: 10 }
    default:
      - service: light.turn_off
        target: { entity_id: !input hallway_light }
```

### 3. Wake-up routine on bed exit

```yaml
blueprint:
  name: RuView — wake-up routine on bed exit
  description: >
    When bed_exit fires between 05:00 and 09:00, ramp up bedroom lights
    over 10 minutes, start the coffee maker, and disarm the home alarm.
  domain: automation
  input:
    bed_exit_event:
      name: Bed exit event entity
      selector: { entity: { domain: event } }
    bedroom_light:
      name: Bedroom light
      selector: { entity: { domain: light } }
    coffee_maker:
      name: Coffee maker switch
      selector: { entity: { domain: switch } }

trigger:
  - platform: state
    entity_id: !input bed_exit_event

condition:
  - condition: time
    after: "05:00:00"
    before: "09:00:00"

action:
  - service: light.turn_on
    target: { entity_id: !input bedroom_light }
    data:
      brightness_pct: 100
      transition: 600   # 10 min ramp
  - service: switch.turn_on
    target: { entity_id: !input coffee_maker }
  - service: alarm_control_panel.alarm_disarm
    target: { entity_id: alarm_control_panel.home }
```

---

## Lovelace dashboard examples

### Single-room overview card

```yaml
type: vertical-stack
title: Bedroom
cards:
  - type: glance
    entities:
      - entity: binary_sensor.ruview_bedroom_presence
      - entity: sensor.ruview_bedroom_heart_rate
      - entity: sensor.ruview_bedroom_breathing_rate
      - entity: sensor.ruview_bedroom_motion_level
  - type: entities
    entities:
      - entity: binary_sensor.ruview_bedroom_someone_sleeping
      - entity: binary_sensor.ruview_bedroom_room_active
      - entity: binary_sensor.ruview_bedroom_no_movement
      - entity: sensor.ruview_bedroom_fall_risk_elevated
```

### Multi-node grid

```yaml
type: grid
columns: 2
cards:
  - type: tile
    entity: binary_sensor.ruview_bedroom_presence
    name: Bedroom
  - type: tile
    entity: binary_sensor.ruview_living_presence
    name: Living
  - type: tile
    entity: binary_sensor.ruview_kitchen_presence
    name: Kitchen
  - type: tile
    entity: binary_sensor.ruview_bathroom_occupied
    name: Bathroom
```

---

## Advanced brokers

Mosquitto is the recommended default. The integration also works with:

- **EMQX** (https://www.emqx.io/) — clustering, MQTT 5.0, dashboard UI. Good for ≥10 RuView nodes.
- **VerneMQ** (https://vernemq.com/) — Erlang-based, multi-protocol bridges (AMQP, WebSocket).
- **HiveMQ Edge** (https://www.hivemq.com/edge/) — managed cloud relay if you need off-LAN access.

All three accept the same HA discovery topics RuView publishes. Performance and discovery semantics are identical.

---

## Troubleshooting

### No entities appear in HA

1. Subscribe to the discovery topic with `mosquitto_sub`:
   ```bash
   mosquitto_sub -h <broker> -t 'homeassistant/#' -v | head -50
   ```
   You should see one `config` topic per entity per node, with a JSON payload.
2. If `mosquitto_sub` shows nothing, RuView is not reaching the broker. Check `--mqtt-host`, network reachability, and credentials.
3. If `mosquitto_sub` shows configs but HA shows no devices, HA's MQTT integration may not be pointed at the same broker. Verify under Settings → Devices & Services → MQTT.

### Entities appear but state never updates

1. Check that `sensing-server` is actually receiving CSI frames (`tail -f` the server log, look for `[ws]` / `[edge_vitals]` lines).
2. Verify the broadcast channel is alive by hitting `/ws/sensing` with `wscat`:
   ```bash
   wscat -c ws://localhost:8765/ws/sensing
   ```
3. Confirm rate limits aren't dropping everything: `--mqtt-rate-vitals 1.0` for diagnosis (default 0.2 Hz = every 5 s).

### "Plaintext MQTT on non-localhost broker" WARN

Per [ADR-115 §3.9](../adr/ADR-115-home-assistant-integration.md#39-tls--auth), v0.7.0 warns and continues; v0.8.0 will hard-fail. Either:

- Add `--mqtt-tls` and supply a CA if your broker uses a self-signed cert, or
- Move the broker to `localhost` (e.g. run Mosquitto inside the same host as `sensing-server`).

### Matter pairing fails

1. Check the setup code in your `--matter-setup-file` log (defaults to printing on startup).
2. Make sure the host running `sensing-server` is on the same WiFi subnet as the controller.
3. If Apple Home complains about an unknown vendor, that's expected — RuView uses dev VID `0xFFF1` until P10 (see [ADR §9.9](../adr/ADR-115-home-assistant-integration.md#9b-matter-path-p7p10)). Tap "Add anyway".

---

## Applications — what people actually do with this

The 21 entities per node — 11 raw signals (presence, person count, breathing, heart rate, motion, RSSI, etc.) and 10 inferred semantic states (someone-sleeping, possible-distress, room-active, elderly-inactivity-anomaly, meeting-in-progress, bathroom-occupied, fall-risk-elevated, bed-exit, no-movement, multi-room-transition) — slot into Home Assistant like any other sensor. The list below groups real-world uses so you can pick the ones that match your space.

### Personal & home

| Use case | Which entities | What HA does with it |
|---|---|---|
| **"Goodnight" routine** | `someone_sleeping` | Dim hallway lights to 5%, lock doors, drop thermostat 2 °C, mute notifications. Blueprint `02-dim-hallway-when-sleeping.yaml`. |
| **"Wake up" routine** | `bed_exit` | When you get out of bed in the morning, turn on the bathroom heater, raise blinds, start the coffee. Blueprint `03-wake-routine-on-bed-exit.yaml`. |
| **Meeting / focus mode** | `meeting_in_progress` | Multi-person presence in the office for >5 min → set a "Do Not Disturb" status, dim overhead lights, pause vacuum schedule. Blueprint `05-meeting-lights-presence-mode.yaml`. |
| **Bathroom fan automation** | `bathroom_occupied` | Turn the exhaust fan on while a bathroom is occupied; turn it off 5 min after you leave. Blueprint `06-bathroom-fan-while-occupied.yaml`. |
| **Forgotten kitchen / iron** | `presence` per room | "Stove on, kitchen empty for 10 min" → push notification + optional smart-plug cut-off. |
| **Pet-only at home** | `n_persons == 0` for hours but `motion > 0` | Distinguish dog moving around from human presence — don't trigger empty-home automations during the day. |
| **Sleep quality tracking** | `breathing_rate_bpm`, `heart_rate_bpm` (privacy off) | Push nightly averages to HA Statistics, graph in Grafana. No watch, no app. |
| **Toddler bed safety** | `no_movement` in a child's room overnight | Alert parents if breathing-rate signal drops out unexpectedly. |
| **Pre-arrival lighting** | `multi_room_transition` | When you walk from the entry hall toward the living room, anticipate the route and pre-warm those lights. |

### Healthcare & assisted living (AAL)

| Use case | Which entities | Why this works |
|---|---|---|
| **Fall detection + escalation** | `fall_detected` | Phase-acceleration spike + 3-frame debounce. Trigger a Lovelace alert, then escalate to a phone call if the person stays still for >2 min. Blueprint `07-fall-risk-escalation.yaml`. |
| **Elderly inactivity anomaly** | `elderly_inactivity_anomaly` | Learns a person's normal day-pattern and flags deviations (e.g. usually up by 9 am, hasn't moved by 11 am). Blueprint `04-alert-elderly-inactivity-anomaly.yaml`. |
| **Privacy-mode care monitoring** | `possible_distress` + `no_movement` + `someone_sleeping` | Run with `--privacy-mode` — heart rate and breathing values are stripped at the wire, but the *inferred states* keep working. Care staff sees "Distress detected" without ever seeing the underlying biometric numbers. The architectural win that makes RuView legally deployable in care homes. |
| **Sleep apnea screening** | `breathing_rate_bpm` + `breathing_confidence` | Track per-night BPM histograms; flag dips that correlate with apnea events. |
| **Post-surgery recovery monitoring** | `no_movement` + `bed_exit` + `breathing_rate_bpm` | Hospital-discharge patient at home; rule: "no bed exits in 12 h" triggers a check-in call. |
| **Dementia wandering detection** | `multi_room_transition` + nighttime gate | Multi-room transitions between 23:00 and 06:00 alert a caregiver — without GPS tags or wearables the person may refuse to wear. |
| **Bathroom occupancy timeout** | `bathroom_occupied` for >30 min | Possible fall or medical incident; push to caregiver. |

### Security & safety

| Use case | Which entities | What HA does with it |
|---|---|---|
| **Auto-arm when no one's home** | `presence` across all nodes for >10 min | Switch HA alarm panel to "armed_away" — replaces door-sensor + key-fob combos. Blueprint `08-auto-arm-security-when-not-active.yaml`. |
| **Intrusion detection (presence without entry)** | `presence` true while no door/window sensor opened | Real signal of someone inside who shouldn't be. RF-based, can't be defeated by covering a camera. |
| **Through-wall presence verification** | `presence` per room, even with doors closed | Confirms HA "someone is home" estimate without requiring per-room PIR sensors. |
| **Hostage / silent-distress mode** | `possible_distress` (motion + elevated HR) | If you've published HR + privacy is off, abnormal motion-plus-physiology can trigger a silent alarm. |
| **Garage / shed monitoring** | `presence` in outbuildings | Wi-Fi reaches places PIR doesn't (metal shed walls block IR but pass through Wi-Fi). |
| **Camera-free child safety zone** | `presence` near pool / stairs / fireplace | Push alert if a known child-room sensor sees presence in restricted zone — no cameras, no privacy concerns. |

### Commercial buildings & retail

| Use case | Which entities | What it enables |
|---|---|---|
| **Real-time office occupancy** | `n_persons`, `presence`, `room_active` | Live dashboard of how full each meeting room is — no cameras, no badges. Better than door-counters because people are detected mid-meeting, not just on entry. |
| **HVAC demand-controlled ventilation** | `n_persons` | Adjust ventilation per room based on people present — saves 20-30% on cooling/heating in shared offices. |
| **Meeting room booking truth** | `meeting_in_progress` vs calendar | "Meeting booked, but no one's there" → auto-release the room. |
| **Retail dwell time + heat-mapping** | `presence` + `motion` over time | Where do customers linger? Which aisles are empty? Anonymous (no faces), through-clothing, works in low light. |
| **Queue length estimation** | `n_persons` near a checkout | Trigger "open another register" automation. |
| **Cleaning verification** | `no_movement` in a room for >X min after hours | Confirms cleaning crew has finished the room without requiring badges. |
| **Lone-worker safety (warehouses, labs)** | `no_movement` + `possible_distress` | OSHA-compatible solo-worker monitoring without wearables. |

### Industrial & infrastructure

| Use case | Which entities | What it enables |
|---|---|---|
| **Manned-station occupancy** | `presence` | Control rooms / lab benches — confirm operator presence without log-in friction. |
| **Restricted-zone intrusion** | `presence` + `multi_room_transition` | Server room / clean room / pharmaceutical lab — RF passes through doors better than IR. |
| **Equipment-room ventilation** | `presence` in a UPS / battery room | Turn on exhaust fans when a technician enters. |
| **Hazardous-area worker tracking** | `presence` + `no_movement` | Confirm workers in an electrical or chemical area are still moving (not collapsed). |
| **Construction-site after-hours** | `presence` + scheduled gate | Detect anyone on-site after 18:00 → site supervisor alert. |
| **Maritime / offshore quarters** | `breathing_rate` overnight | Confirm bunk occupants are alive without wearables that often get removed during sleep. |

### Education & public spaces

| Use case | Which entities | What it enables |
|---|---|---|
| **Classroom occupancy** | `n_persons`, `room_active` | HVAC and lighting per actual headcount — saves energy in classrooms used 40% of the day. |
| **Library / study room availability** | `presence` + `n_persons` | Live "rooms available" page without webcams. |
| **Lecture hall attendance** | `n_persons` time-series | No card-swipe required — RF presence is robust to phones-in-pockets. |
| **Restroom occupancy signage** | `bathroom_occupied` per stall | Privacy-friendly "in use / available" indicators. |
| **Gym / pool capacity** | `n_persons` | Live capacity counter for compliance with limits — no turnstiles needed. |
| **Public-transport waiting areas** | `n_persons` + `room_active` | Real-time platform crowd density for transit operator dashboards. |

### Energy & sustainability

| Use case | Which entities | What it enables |
|---|---|---|
| **Per-room lighting auto-off** | `presence` per node | The room-level version of motion-PIR — works through walls, no false-off when sitting still reading. |
| **Smart-thermostat zoning** | `room_active`, `n_persons` | Only heat / cool occupied rooms — substantial savings in homes >150 m². |
| **Vampire-load cut-off** | `presence` for whole house | When no one is home, smart plugs cut TV / chargers / standby loads. |
| **Solar / battery dispatch tuning** | `n_persons`, `motion_energy` | Predict next-hour load based on activity, dispatch battery accordingly. |
| **Cold-chain refrigeration alerts** | `presence` + `bathroom_occupied` confusion | Trigger door-checks when an unexpected person spends >10 min near a walk-in freezer. |

### Research, prototyping & developer use

| Use case | Which entities | What it enables |
|---|---|---|
| **Behavioral studies** | Full snapshot stream | Anonymous behavioral data — count, motion, vitals — without IRB-blocking cameras. |
| **HCI experiments** | `multi_room_transition` + `presence` | Path-following studies in living labs. |
| **Healthcare datasets** | `breathing_rate_bpm` time-series | Generate breathing-rate corpora for ML training without consent forms for facial data. |
| **Custom RuView Cogs** | Raw CSI feed + the WebSocket sync field | Bring your own model, consume the firmware-side mesh-aligned timestamps for multistatic fusion. |

### Combining entities — recipe patterns

A few patterns appear over and over; if you understand these you can build most of the above yourself:

1. **"Negative + duration" trip wires** — `no_movement` for N minutes AND time-of-day window → most healthcare and pet/child safety automations.
2. **"Two states agree" guards** — `presence == false` AND security panel disarmed AND no door sensor open → strong "house is empty" signal.
3. **"Threshold + cooldown"** — `presence_score > 0.7` for 30 s before triggering (smooths over flicker), then a 5 min cooldown before re-arming (prevents oscillation).
4. **"Calendar vs reality"** — pair an HA calendar event with `n_persons` → meeting-room auto-release, classroom unused-period detection.
5. **"Privacy-mode + semantic-only"** — run `--privacy-mode`, expose only the semantic primitives to HA, keep biometrics on-device. The right default for any deployment with non-tenant occupants.

### What about regulated environments?

Run RuView with `--privacy-mode` and only the 10 inferred semantic states reach Home Assistant — heart rate, breathing rate, and pose values are stripped at the MQTT wire. Per ADR-115 §6, this passes:

- **HIPAA-style minimum-necessary** (no biometric numbers leave the device)
- **GDPR purpose-limitation** (the inferred states are the smallest dataset that supports the automation)
- **CCPA "sensitive personal information"** (no health data crosses the wire)

The fall-risk-elevated / possible-distress / someone-sleeping flags still work — they're computed *inside* the sensor pipeline and only the boolean outputs are published. That's the architectural win that makes RuView deployable in care homes, hospitals, schools, and shared-housing scenarios where raw biometrics would be a non-starter.

## References

- [ADR-115](../adr/ADR-115-home-assistant-integration.md) — full design rationale
- [`semantic-primitives-metrics.md`](./semantic-primitives-metrics.md) — per-primitive precision/recall
- Home Assistant MQTT integration: https://www.home-assistant.io/integrations/mqtt/
- Mosquitto add-on: https://github.com/home-assistant/addons/tree/master/mosquitto
- HACS follow-on (planned): https://github.com/ruvnet/hass-wifi-densepose
- Matter spec: https://csa-iot.org/all-solutions/matter/
