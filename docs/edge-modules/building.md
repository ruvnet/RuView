# Smart Building Modules -- WiFi-DensePose Edge Intelligence

> Make any building smarter using WiFi signals you already have. Know which rooms are occupied, control HVAC and lighting automatically, count elevator passengers, track meeting room usage, and audit energy waste -- all without cameras or badges.

## Overview

| Module | File | What It Does | Event IDs | Frame Budget |
|--------|------|--------------|-----------|--------------|
| HVAC Presence | `bld_hvac_presence.rs` | Presence detection tuned for HVAC energy management | 310-312 | ~0.5 us/frame |
| Lighting Zones | `bld_lighting_zones.rs` | Per-zone lighting control (On/Dim/Off) based on spatial occupancy | 320-322 | ~1 us/frame |
| Elevator Count | `bld_elevator_count.rs` | Occupant counting in elevator cabins (1-12 persons) | 330-333 | ~1.5 us/frame |
| Meeting Room | `bld_meeting_room.rs` | Meeting lifecycle tracking with utilization metrics | 340-343 | ~0.3 us/frame |
| Energy Audit | `bld_energy_audit.rs` | 24x7 hourly occupancy histograms for scheduling optimization | 350-352 | ~0.2 us/frame |

All modules target the ESP32-S3 running WASM3 (ADR-040 Tier 3). They receive pre-processed CSI signals from Tier 2 DSP and emit structured events via `csi_emit_event()`.

---

## Modules

### HVAC Presence Control (`bld_hvac_presence.rs`)

**What it does**: Tells your HVAC system whether a room is occupied, with intentionally asymmetric timing -- fast arrival detection (10 seconds) so cooling/heating starts quickly, and slow departure timeout (5 minutes) to avoid premature shutoff when someone briefly steps out. Also classifies whether the occupant is sedentary (desk work, reading) or active (walking, exercising).

**How it works**: A four-state machine processes presence scores and motion energy each frame:

```
Vacant --> ArrivalPending --> Occupied --> DeparturePending --> Vacant
           (10s debounce)                 (5 min timeout)
```

Motion energy is smoothed with an exponential moving average (alpha=0.1) and classified against a threshold of 0.3 to distinguish sedentary from active behavior.

#### State Machine

| State | Entry Condition | Exit Condition |
|-------|----------------|----------------|
| `Vacant` | No presence detected | Presence score > 0.5 |
| `ArrivalPending` | Presence detected, debounce counting | 200 consecutive frames with presence -> Occupied; any absence -> Vacant |
| `Occupied` | Arrival debounce completed | First frame without presence -> DeparturePending |
| `DeparturePending` | Presence lost | 6000 frames without presence -> Vacant; any presence -> Occupied |

#### Events

| Event ID | Name | Value | When Emitted |
|----------|------|-------|--------------|
| 310 | `HVAC_OCCUPIED` | 1.0 (occupied) or 0.0 (vacant) | Every 20 frames |
| 311 | `ACTIVITY_LEVEL` | 0.0-0.99 (sedentary + EMA) or 1.0 (active) | Every 20 frames |
| 312 | `DEPARTURE_COUNTDOWN` | 0.0-1.0 (fraction of timeout remaining) | Every 20 frames during DeparturePending |

#### API

```rust
use wifi_densepose_wasm_edge::bld_hvac_presence::HvacPresenceDetector;

let mut det = HvacPresenceDetector::new();

// Per-frame processing
let events = det.process_frame(presence_score, motion_energy);
// events: &[(event_type: i32, value: f32)]

// Queries
det.state()       // -> HvacState (Vacant|ArrivalPending|Occupied|DeparturePending)
det.is_occupied()  // -> bool (true during Occupied or DeparturePending)
det.activity()     // -> ActivityLevel (Sedentary|Active)
det.motion_ema()   // -> f32 (smoothed motion energy)
```

#### Configuration Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `ARRIVAL_DEBOUNCE` | 200 frames (10s) | Frames of continuous presence before confirming occupancy |
| `DEPARTURE_TIMEOUT` | 6000 frames (5 min) | Frames of continuous absence before declaring vacant |
| `ACTIVITY_THRESHOLD` | 0.3 | Motion EMA above this = Active |
| `MOTION_ALPHA` | 0.1 | EMA smoothing factor for motion energy |
| `PRESENCE_THRESHOLD` | 0.5 | Minimum presence score to consider someone present |
| `EMIT_INTERVAL` | 20 frames (1s) | Event emission interval |

#### Example: BACnet Integration

```python
# Python host reading events from ESP32 UDP packet
if event_id == 310:  # HVAC_OCCUPIED
    bacnet_write(device_id, "Occupancy", int(value))  # 1=occupied, 0=vacant
elif event_id == 311:  # ACTIVITY_LEVEL
    if value >= 1.0:
        bacnet_write(device_id, "CoolingSetpoint", 72)  # Active: cooler
    else:
        bacnet_write(device_id, "CoolingSetpoint", 76)  # Sedentary: warmer
elif event_id == 312:  # DEPARTURE_COUNTDOWN
    if value < 0.2:  # Less than 1 minute remaining
        bacnet_write(device_id, "FanMode", "low")  # Start reducing
```

---

### Lighting Zone Control (`bld_lighting_zones.rs`)

**What it does**: Manages up to 4 independent lighting zones, automatically transitioning each zone between On (occupied and active), Dim (occupied but sedentary for over 10 minutes), and Off (vacant for over 30 seconds). Uses per-zone variance analysis to determine which areas of the room have people.

**How it works**: Subcarriers are divided into groups (one per zone). Each group's amplitude variance is computed and compared against a calibrated baseline. Variance deviation above threshold indicates occupancy in that zone. A calibration phase (200 frames = 10 seconds) establishes the baseline with an empty room.

```
Off --> On (occupancy + activity detected)
On --> Dim (occupied but sedentary for 10 min)
On --> Dim (vacancy detected, grace period)
Dim --> Off (vacant for 30 seconds)
Dim --> On (activity resumes)
```

#### Events

| Event ID | Name | Value | When Emitted |
|----------|------|-------|--------------|
| 320 | `LIGHT_ON` | zone_id (0-3) | On state transition |
| 321 | `LIGHT_DIM` | zone_id (0-3) | Dim state transition |
| 322 | `LIGHT_OFF` | zone_id (0-3) | Off state transition |

Periodic summaries encode `zone_id + confidence` in the value field (integer part = zone, fractional part = occupancy score).

#### API

```rust
use wifi_densepose_wasm_edge::bld_lighting_zones::LightingZoneController;

let mut ctrl = LightingZoneController::new();

// Per-frame: pass subcarrier amplitudes and overall motion energy
let events = ctrl.process_frame(&amplitudes, motion_energy);

// Queries
ctrl.zone_state(zone_id) // -> LightState (Off|Dim|On)
ctrl.n_zones()           // -> usize (number of active zones, 1-4)
ctrl.is_calibrated()     // -> bool
```

#### Configuration Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `MAX_ZONES` | 4 | Maximum lighting zones |
| `OCCUPANCY_THRESHOLD` | 0.03 | Variance deviation ratio for occupancy |
| `ACTIVE_THRESHOLD` | 0.25 | Motion energy for active classification |
| `DIM_TIMEOUT` | 12000 frames (10 min) | Sedentary frames before dimming |
| `OFF_TIMEOUT` | 600 frames (30s) | Vacant frames before turning off |
| `BASELINE_FRAMES` | 200 frames (10s) | Calibration duration |

#### Example: DALI/KNX Lighting

```python
# Map zone events to DALI addresses
DALI_ADDR = {0: 1, 1: 2, 2: 3, 3: 4}

if event_id == 320:  # LIGHT_ON
    zone = int(value)
    dali_send(DALI_ADDR[zone], level=254)  # Full brightness
elif event_id == 321:  # LIGHT_DIM
    zone = int(value)
    dali_send(DALI_ADDR[zone], level=80)   # 30% brightness
elif event_id == 322:  # LIGHT_OFF
    zone = int(value)
    dali_send(DALI_ADDR[zone], level=0)    # Off
```

---

### Elevator Occupancy Counting (`bld_elevator_count.rs`)

**What it does**: Counts the number of people in an elevator cabin (0-12), detects door open/close events, and emits overload warnings when the count exceeds a configurable threshold. Uses the confined-space multipath characteristics of an elevator to correlate amplitude variance with body count.

**How it works**: In a small reflective metal box like an elevator, each additional person adds significant multipath scattering. The module calibrates on the empty cabin, then maps the ratio of current variance to baseline variance onto a person count. Frame-to-frame amplitude deltas detect sudden geometry changes (door open/close). Count estimate fuses the module's own variance-based estimate (40% weight) with the host's person count hint (60% weight) when available.

#### Events

| Event ID | Name | Value | When Emitted |
|----------|------|-------|--------------|
| 330 | `ELEVATOR_COUNT` | Person count (0-12) | Every 10 frames |
| 331 | `DOOR_OPEN` | Current count at time of opening | On door open detection |
| 332 | `DOOR_CLOSE` | Current count at time of closing | On door close detection |
| 333 | `OVERLOAD_WARNING` | Current count | When count >= overload threshold |

#### API

```rust
use wifi_densepose_wasm_edge::bld_elevator_count::ElevatorCounter;

let mut ec = ElevatorCounter::new();

// Per-frame: amplitudes, phases, motion energy, host person count hint
let events = ec.process_frame(&amplitudes, &phases, motion_energy, host_n_persons);

// Queries
ec.occupant_count()    // -> u8 (0-12)
ec.door_state()        // -> DoorState (Open|Closed)
ec.is_calibrated()     // -> bool

// Configuration
ec.set_overload_threshold(8); // Set custom overload limit
```

#### Configuration Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `MAX_OCCUPANTS` | 12 | Maximum tracked occupants |
| `DEFAULT_OVERLOAD` | 10 | Default overload warning threshold |
| `DOOR_VARIANCE_RATIO` | 4.0 | Delta magnitude for door detection |
| `DOOR_DEBOUNCE` | 3 frames | Debounce for door events |
| `DOOR_COOLDOWN` | 40 frames (2s) | Cooldown after door event |
| `BASELINE_FRAMES` | 200 frames (10s) | Calibration with empty cabin |

---

### Meeting Room Tracker (`bld_meeting_room.rs`)

**What it does**: Tracks the full lifecycle of meeting room usage -- from someone entering, to confirming a genuine multi-person meeting, to detecting when the meeting ends and the room is available again. Distinguishes actual meetings (2+ people for more than 3 seconds) from a single person briefly using the room. Tracks peak headcount and calculates room utilization rate.

**How it works**: A four-state machine processes presence and person count:

```
Empty --> PreMeeting --> Active --> PostMeeting --> Empty
          (someone        (2+ people       (everyone left,
           entered)        confirmed)       2 min cooldown)
```

The PreMeeting state has a 3-minute timeout: if only one person remains, the room is not promoted to "Active" (it is not counted as a meeting).

#### Events

| Event ID | Name | Value | When Emitted |
|----------|------|-------|--------------|
| 340 | `MEETING_START` | Current person count | On transition to Active |
| 341 | `MEETING_END` | Duration in minutes | On transition to PostMeeting |
| 342 | `PEAK_HEADCOUNT` | Peak person count | On meeting end + periodic during Active |
| 343 | `ROOM_AVAILABLE` | 1.0 | On transition from PostMeeting to Empty |

#### API

```rust
use wifi_densepose_wasm_edge::bld_meeting_room::MeetingRoomTracker;

let mut mt = MeetingRoomTracker::new();

// Per-frame: presence (0/1), person count, motion energy
let events = mt.process_frame(presence, n_persons, motion_energy);

// Queries
mt.state()            // -> MeetingState (Empty|PreMeeting|Active|PostMeeting)
mt.peak_headcount()   // -> u8
mt.meeting_count()    // -> u32 (total meetings since reset)
mt.utilization_rate() // -> f32 (fraction of time in meetings, 0.0-1.0)
```

#### Configuration Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `MEETING_MIN_PERSONS` | 2 | Minimum people for a "meeting" |
| `PRE_MEETING_TIMEOUT` | 3600 frames (3 min) | Max time waiting for meeting to form |
| `POST_MEETING_TIMEOUT` | 2400 frames (2 min) | Cooldown before marking room available |
| `MEETING_MIN_FRAMES` | 6000 frames (5 min) | Reference minimum meeting duration |

#### Example: Calendar Integration

```python
# Sync meeting room status with calendar system
if event_id == 340:  # MEETING_START
    calendar_api.mark_room_in_use(room_id, headcount=int(value))
elif event_id == 341:  # MEETING_END
    duration_min = value
    calendar_api.log_actual_usage(room_id, duration_min)
elif event_id == 343:  # ROOM_AVAILABLE
    calendar_api.mark_room_available(room_id)
    display_screen.show("Room Available")
```

---

### Energy Audit (`bld_energy_audit.rs`)

**What it does**: Builds a 7-day, 24-hour occupancy histogram (168 hourly bins) to identify energy waste patterns. Finds which hours are consistently unoccupied (candidates for HVAC/lighting shutoff), detects after-hours occupancy anomalies (security/safety concern), and reports overall building utilization.

**How it works**: Each frame increments the appropriate hour bin's counters. The module maintains its own simulated clock (hour/day) that advances by counting frames (72,000 frames = 1 hour at 20 Hz). The host can set the real time via `set_time()`. After-hours is defined as 22:00-06:00 (wraps midnight correctly). Sustained presence (30+ seconds) during after-hours triggers an alert.

#### Events

| Event ID | Name | Value | When Emitted |
|----------|------|-------|--------------|
| 350 | `SCHEDULE_SUMMARY` | Current hour's occupancy rate (0.0-1.0) | Every 1200 frames (1 min) |
| 351 | `AFTER_HOURS_ALERT` | Current hour (22-5) | After 600 frames (30s) of after-hours presence |
| 352 | `UTILIZATION_RATE` | Overall utilization (0.0-1.0) | Every 1200 frames (1 min) |

#### API

```rust
use wifi_densepose_wasm_edge::bld_energy_audit::EnergyAuditor;

let mut ea = EnergyAuditor::new();

// Set real time from host
ea.set_time(0, 8); // Monday 8 AM (day 0-6, hour 0-23)

// Per-frame: presence (0/1), person count
let events = ea.process_frame(presence, n_persons);

// Queries
ea.utilization_rate()          // -> f32 (overall)
ea.hourly_rate(day, hour)      // -> f32 (occupancy rate for specific slot)
ea.hourly_headcount(day, hour) // -> f32 (average headcount)
ea.unoccupied_hours(day)       // -> u8 (hours below 10% occupancy)
ea.current_time()              // -> (day, hour)
```

#### Configuration Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `FRAMES_PER_HOUR` | 72000 | Frames in one hour at 20 Hz |
| `SUMMARY_INTERVAL` | 1200 frames (1 min) | How often to emit summaries |
| `AFTER_HOURS_START` | 22 (10 PM) | Start of after-hours window |
| `AFTER_HOURS_END` | 6 (6 AM) | End of after-hours window |
| `USED_THRESHOLD` | 0.1 | Minimum occupancy rate to consider an hour "used" |
| `AFTER_HOURS_ALERT_FRAMES` | 600 frames (30s) | Sustained presence before alert |

#### Example: Energy Optimization Report

```python
# Generate weekly energy optimization report
for day in range(7):
    unused = auditor.unoccupied_hours(day)
    print(f"{DAY_NAMES[day]}: {unused} hours could have HVAC off")

    for hour in range(24):
        rate = auditor.hourly_rate(day, hour)
        if rate < 0.1:
            print(f"  {hour:02d}:00 - unused ({rate:.0%} occupancy)")
```

---

## Integration Guide

### Connecting to BACnet / HVAC Systems

All five building modules emit events via the standard `csi_emit_event()` interface. A typical integration path:

1. **ESP32 firmware** receives events from the WASM module
2. **UDP packet** carries events to the aggregator server (port 5005)
3. **Sensing server** (`wifi-densepose-sensing-server`) exposes events via REST API
4. **BMS integration script** polls the API and writes BACnet/Modbus objects

Key BACnet object mappings:

| Module | BACnet Object Type | Property |
|--------|--------------------|----------|
| HVAC Presence | Binary Value | Occupancy (310: 1=occupied) |
| HVAC Presence | Analog Value | Activity Level (311: 0-1) |
| Lighting Zones | Multi-State Value | Zone State (320-322: Off/Dim/On) |
| Elevator Count | Analog Value | Occupant Count (330: 0-12) |
| Meeting Room | Binary Value | Room In Use (340/343) |
| Energy Audit | Analog Value | Utilization Rate (352: 0-1.0) |

### Lighting Control Integration (DALI, KNX)

The `bld_lighting_zones` module emits zone-level On/Dim/Off transitions. Map each zone to a DALI address group or KNX group address:

- Event 320 (LIGHT_ON) -> DALI command `DAPC(254)` or KNX `DPT_Switch ON`
- Event 321 (LIGHT_DIM) -> DALI command `DAPC(80)` or KNX `DPT_Scaling 30%`
- Event 322 (LIGHT_OFF) -> DALI command `DAPC(0)` or KNX `DPT_Switch OFF`

### BMS (Building Management System) Integration

For full BMS integration combining all five modules:

```
ESP32 Nodes (per room/zone)
    |
    v  UDP events
Aggregator Server
    |
    v  REST API / WebSocket
BMS Gateway Script
    |
    +-- HVAC Controller (BACnet/Modbus)
    +-- Lighting Controller (DALI/KNX)
    +-- Elevator Display Panel
    +-- Meeting Room Booking System
    +-- Energy Dashboard
```

### Deployment Considerations

- **Calibration**: Lighting and Elevator modules require a 10-second calibration with an empty room/cabin. Schedule calibration during known unoccupied periods.
- **Clock sync**: The Energy Audit module needs `set_time()` called at startup. Use NTP on the aggregator or pass timestamp via the host API.
- **Multiple ESP32s**: For open-plan offices, deploy one ESP32 per zone. Each runs its own HVAC Presence and Lighting Zones instance. The aggregator merges zone-level data.
- **Event rate**: All modules throttle events to at most one emission per second (EMIT_INTERVAL = 20 frames). Total bandwidth per module is under 100 bytes/second.
