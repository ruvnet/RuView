# Retail & Hospitality Modules -- WiFi-DensePose Edge Intelligence

> Understand customer behavior without cameras or consent forms. Count queues, map foot traffic, track table turnover, measure shelf engagement -- all from WiFi signals that are already there.

## Overview

| Module | File | What It Does | Event IDs | Frame Budget |
|--------|------|--------------|-----------|--------------|
| Queue Length | `ret_queue_length.rs` | Estimates queue length and wait time using Little's Law | 400-403 | ~0.5 us/frame |
| Dwell Heatmap | `ret_dwell_heatmap.rs` | Tracks dwell time per spatial zone (3x3 grid) | 410-413 | ~1 us/frame |
| Customer Flow | `ret_customer_flow.rs` | Directional foot traffic counting (ingress/egress) | 420-423 | ~1.5 us/frame |
| Table Turnover | `ret_table_turnover.rs` | Restaurant table lifecycle tracking with turnover rate | 430-433 | ~0.3 us/frame |
| Shelf Engagement | `ret_shelf_engagement.rs` | Detects and classifies customer shelf interaction | 440-443 | ~1 us/frame |

All modules target the ESP32-S3 running WASM3 (ADR-040 Tier 3). They receive pre-processed CSI signals from Tier 2 DSP and emit structured events via `csi_emit_event()`.

---

## Modules

### Queue Length Estimation (`ret_queue_length.rs`)

**What it does**: Estimates the number of people waiting in a queue, computes arrival and service rates, estimates wait time using Little's Law (L = lambda x W), and fires alerts when the queue exceeds a configurable threshold.

**How it works**: The module tracks person count changes frame-to-frame to detect arrivals (count increased or new presence with variance spike) and departures (count decreased or presence edge with low motion). Over 30-second windows, it computes arrival rate (lambda) and service rate (mu) in persons-per-minute. The queue length is smoothed via EMA on the raw person count. Wait time is estimated as `queue_length / (arrival_rate / 60)`.

#### Events

| Event ID | Name | Value | When Emitted |
|----------|------|-------|--------------|
| 400 | `QUEUE_LENGTH` | Estimated queue length (0-20) | Every 20 frames (1s) |
| 401 | `WAIT_TIME_ESTIMATE` | Estimated wait in seconds | Every 600 frames (30s window) |
| 402 | `SERVICE_RATE` | Service rate (persons/min, smoothed) | Every 600 frames (30s window) |
| 403 | `QUEUE_ALERT` | Current queue length | When queue >= 5 (once, resets below 4) |

#### API

```rust
use wifi_densepose_wasm_edge::ret_queue_length::QueueLengthEstimator;

let mut q = QueueLengthEstimator::new();

// Per-frame: presence (0/1), person count, variance, motion energy
let events = q.process_frame(presence, n_persons, variance, motion_energy);

// Queries
q.queue_length()  // -> u8 (0-20, smoothed)
q.arrival_rate()  // -> f32 (persons/minute, EMA-smoothed)
q.service_rate()  // -> f32 (persons/minute, EMA-smoothed)
```

#### Configuration Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `REPORT_INTERVAL` | 20 frames (1s) | Queue length report interval |
| `SERVICE_WINDOW_FRAMES` | 600 frames (30s) | Window for rate computation |
| `QUEUE_EMA_ALPHA` | 0.1 | EMA smoothing for queue length |
| `RATE_EMA_ALPHA` | 0.05 | EMA smoothing for arrival/service rates |
| `JOIN_VARIANCE_THRESH` | 0.05 | Variance spike threshold for join detection |
| `DEPART_MOTION_THRESH` | 0.02 | Motion threshold for departure detection |
| `QUEUE_ALERT_THRESH` | 5.0 | Queue length that triggers alert |
| `MAX_QUEUE` | 20 | Maximum tracked queue length |

#### Example: Retail Queue Management

```python
# React to queue events
if event_id == 400:  # QUEUE_LENGTH
    queue_len = int(value)
    dashboard.update_queue(register_id, queue_len)

elif event_id == 401:  # WAIT_TIME_ESTIMATE
    wait_seconds = value
    signage.show(f"Estimated wait: {int(wait_seconds / 60)} min")

elif event_id == 403:  # QUEUE_ALERT
    staff_pager.send(f"Register {register_id}: {int(value)} in queue")
```

---

### Dwell Heatmap (`ret_dwell_heatmap.rs`)

**What it does**: Divides the sensing area into a 3x3 grid (9 zones) and tracks how long customers spend in each zone. Identifies "hot zones" (highest dwell time) and "cold zones" (lowest dwell time). Emits session summaries when the space empties, enabling store layout optimization.

**How it works**: Subcarriers are divided into 9 groups, one per zone. Each zone's variance is smoothed via EMA and compared against a threshold. When variance exceeds the threshold and presence is detected, dwell time accumulates at 0.05 seconds per frame. Sessions start when someone enters and end after 100 frames (5 seconds) of empty space.

#### Events

| Event ID | Name | Value Encoding | When Emitted |
|----------|------|----------------|--------------|
| 410 | `DWELL_ZONE_UPDATE` | `zone_id * 1000 + dwell_seconds` | Every 600 frames (30s) per occupied zone |
| 411 | `HOT_ZONE` | `zone_id + dwell_seconds/1000` | Every 600 frames (30s) |
| 412 | `COLD_ZONE` | `zone_id + dwell_seconds/1000` | Every 600 frames (30s) |
| 413 | `SESSION_SUMMARY` | Session duration in seconds | When space empties after occupancy |

**Value decoding for DWELL_ZONE_UPDATE**: The zone ID is encoded in the thousands place. For example, `value = 2015.5` means zone 2 with 15.5 seconds of dwell time.

#### API

```rust
use wifi_densepose_wasm_edge::ret_dwell_heatmap::DwellHeatmapTracker;

let mut t = DwellHeatmapTracker::new();

// Per-frame: presence (0/1), per-subcarrier variances, motion energy, person count
let events = t.process_frame(presence, &variances, motion_energy, n_persons);

// Queries
t.zone_dwell(zone_id)       // -> f32 (seconds in current session)
t.zone_total_dwell(zone_id) // -> f32 (seconds across all sessions)
t.is_zone_occupied(zone_id) // -> bool
t.is_session_active()       // -> bool
```

#### Configuration Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `NUM_ZONES` | 9 | Spatial zones (3x3 grid) |
| `REPORT_INTERVAL` | 600 frames (30s) | Heatmap update interval |
| `ZONE_OCCUPIED_THRESH` | 0.015 | Variance threshold for zone occupancy |
| `ZONE_EMA_ALPHA` | 0.12 | EMA smoothing for zone variance |
| `EMPTY_FRAMES_FOR_SUMMARY` | 100 frames (5s) | Vacancy duration before session end |
| `MAX_EVENTS` | 12 | Maximum events per frame |

#### Zone Layout

The 3x3 grid maps to the physical space:

```
+-------+-------+-------+
|  Z0   |  Z1   |  Z2   |
|       |       |       |
+-------+-------+-------+
|  Z3   |  Z4   |  Z5   |
|       |       |       |
+-------+-------+-------+
|  Z6   |  Z7   |  Z8   |
|       |       |       |
+-------+-------+-------+
   Near    Mid      Far
```

Subcarriers are divided evenly: with 27 subcarriers, each zone gets 3 subcarriers. Lower-index subcarriers correspond to nearer Fresnel zones.

---

### Customer Flow Counting (`ret_customer_flow.rs`)

**What it does**: Counts people entering and exiting through a doorway or passage using directional phase gradient analysis. Maintains cumulative ingress/egress counts and reports net occupancy (in - out, clamped to zero). Emits hourly traffic summaries.

**How it works**: Subcarriers are split into two groups: low-index (near entrance) and high-index (far side). A person walking through the sensing area causes an asymmetric phase velocity pattern -- the near-side group's phase changes before the far-side group for ingress, and vice versa for egress. The directional gradient (low_gradient - high_gradient) is smoothed via EMA and thresholded. Combined with motion energy and amplitude spike detection, this discriminates genuine crossings from noise.

```
Ingress: positive smoothed gradient (low-side phase leads)
Egress:  negative smoothed gradient (high-side phase leads)
```

#### Events

| Event ID | Name | Value | When Emitted |
|----------|------|-------|--------------|
| 420 | `INGRESS` | Cumulative ingress count | On each detected entry |
| 421 | `EGRESS` | Cumulative egress count | On each detected exit |
| 422 | `NET_OCCUPANCY` | Current net occupancy (>= 0) | On crossing + every 100 frames |
| 423 | `HOURLY_TRAFFIC` | `ingress * 1000 + egress` | Every 72000 frames (1 hour) |

**Decoding HOURLY_TRAFFIC**: `ingress = int(value / 1000)`, `egress = int(value % 1000)`.

#### API

```rust
use wifi_densepose_wasm_edge::ret_customer_flow::CustomerFlowTracker;

let mut cf = CustomerFlowTracker::new();

// Per-frame: per-subcarrier phases, amplitudes, variance, motion energy
let events = cf.process_frame(&phases, &amplitudes, variance, motion_energy);

// Queries
cf.net_occupancy()    // -> i32 (ingress - egress, clamped to 0)
cf.total_ingress()    // -> u32 (cumulative entries)
cf.total_egress()     // -> u32 (cumulative exits)
cf.current_gradient() // -> f32 (smoothed directional gradient)
```

#### Configuration Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `PHASE_GRADIENT_THRESH` | 0.15 | Minimum gradient magnitude for crossing |
| `MOTION_THRESH` | 0.03 | Minimum motion energy for valid crossing |
| `AMPLITUDE_SPIKE_THRESH` | 1.5 | Amplitude change scale factor |
| `CROSSING_DEBOUNCE` | 10 frames (0.5s) | Debounce between crossing events |
| `GRADIENT_EMA_ALPHA` | 0.2 | EMA smoothing for gradient |
| `OCCUPANCY_REPORT_INTERVAL` | 100 frames (5s) | Net occupancy report interval |

#### Example: Store Occupancy Display

```python
# Real-time occupancy counter at store entrance
if event_id == 422:  # NET_OCCUPANCY
    occupancy = int(value)
    display.show(f"Currently in store: {occupancy}")

    if occupancy >= max_capacity:
        door_signal.set("WAIT")
    else:
        door_signal.set("ENTER")

elif event_id == 423:  # HOURLY_TRAFFIC
    ingress = int(value / 1000)
    egress = int(value % 1000)
    analytics.log_hourly(hour, ingress, egress)
```

---

### Table Turnover Tracking (`ret_table_turnover.rs`)

**What it does**: Tracks the full lifecycle of a restaurant table -- from guests sitting down, through eating, to departing and cleanup. Measures seating duration and computes a rolling turnover rate (turnovers per hour). Designed for one ESP32 node per table or table group.

**How it works**: A five-state machine processes presence, motion energy, and person count:

```
Empty --> Eating --> Departing --> Cooldown --> Empty
  |       (2s          (motion      (30s         |
  |       debounce)    increase)    cleanup)     |
  |                                              |
  +----------------------------------------------+
          (brief absence: stays in Eating)
```

The `Seating` state exists in the enum for completeness but transitions are handled directly (Empty -> Eating after debounce). The `Departing` state detects when guests show increased motion and reduced person count. Vacancy requires 5 seconds of confirmed absence to avoid false triggers from brief bathroom breaks.

#### Events

| Event ID | Name | Value | When Emitted |
|----------|------|-------|--------------|
| 430 | `TABLE_SEATED` | Person count at seating | After 40-frame debounce |
| 431 | `TABLE_VACATED` | Seating duration in seconds | After 100-frame absence debounce |
| 432 | `TABLE_AVAILABLE` | 1.0 | After 30-second cleanup cooldown |
| 433 | `TURNOVER_RATE` | Turnovers per hour (rolling) | Every 6000 frames (5 min) |

#### API

```rust
use wifi_densepose_wasm_edge::ret_table_turnover::TableTurnoverTracker;

let mut tt = TableTurnoverTracker::new();

// Per-frame: presence (0/1), motion energy, person count
let events = tt.process_frame(presence, motion_energy, n_persons);

// Queries
tt.state()             // -> TableState (Empty|Seating|Eating|Departing|Cooldown)
tt.total_turnovers()   // -> u32 (cumulative turnovers)
tt.session_duration_s() // -> f32 (current session length in seconds)
tt.turnover_rate()     // -> f32 (turnovers/hour, rolling window)
```

#### State Machine

| State | Entry Condition | Exit Condition |
|-------|----------------|----------------|
| `Empty` | Table is free | 40 frames (2s) of continuous presence |
| `Eating` | Guests confirmed seated | 100 frames (5s) of absence -> Cooldown; high motion + fewer people -> Departing |
| `Departing` | High motion with dropping count | 100 frames absence -> Cooldown; motion settles -> back to Eating |
| `Cooldown` | Table vacated, cleanup period | 600 frames (30s) -> Empty; presence during cooldown -> Eating (fast re-seat) |

#### Configuration Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `SEATED_DEBOUNCE_FRAMES` | 40 frames (2s) | Confirmation before marking seated |
| `VACATED_DEBOUNCE_FRAMES` | 100 frames (5s) | Absence confirmation before vacating |
| `AVAILABLE_COOLDOWN_FRAMES` | 600 frames (30s) | Cleanup time before marking available |
| `EATING_MOTION_THRESH` | 0.1 | Motion below this = settled/eating |
| `ACTIVE_MOTION_THRESH` | 0.3 | Motion above this = arriving/departing |
| `TURNOVER_REPORT_INTERVAL` | 6000 frames (5 min) | Rate report interval |
| `MAX_TURNOVERS` | 50 | Rolling window buffer for rate |

#### Example: Restaurant Operations Dashboard

```python
# Restaurant table management
if event_id == 430:  # TABLE_SEATED
    party_size = int(value)
    kitchen.notify(f"Table {table_id}: {party_size} guests seated")
    pos.start_timer(table_id)

elif event_id == 431:  # TABLE_VACATED
    duration_s = value
    analytics.log_seating(table_id, duration_s, peak_persons)
    staff.alert(f"Table {table_id}: needs bussing ({duration_s/60:.0f} min use)")

elif event_id == 432:  # TABLE_AVAILABLE
    hostess_display.mark_available(table_id)

elif event_id == 433:  # TURNOVER_RATE
    rate = value
    manager_dashboard.update(table_id, turnovers_per_hour=rate)
```

---

### Shelf Engagement Detection (`ret_shelf_engagement.rs`)

**What it does**: Detects when a customer stops in front of a shelf and classifies their engagement level: Browse (under 5 seconds), Consider (5-30 seconds), or Deep Engagement (over 30 seconds). Also detects reaching gestures (hand/arm movement toward the shelf). Uses the principle that a person standing still but interacting with products produces high-frequency phase perturbations with low translational motion.

**How it works**: The key insight is distinguishing two types of CSI phase changes:
- **Translational motion** (walking): Large uniform phase shifts across all subcarriers
- **Localized interaction** (reaching, examining): High spatial variance in frame-to-frame phase differences

The module computes the standard deviation of per-subcarrier phase differences. High std-dev with low overall motion indicates shelf interaction. A reach gesture produces a burst of high-frequency perturbation exceeding a higher threshold.

#### Engagement Classification

| Level | Duration | Description | Event ID |
|-------|----------|-------------|----------|
| None | -- | No engagement (absent or walking) | -- |
| Browse | < 5s | Brief glance, passing interest | 440 |
| Consider | 5-30s | Examining, reading label, comparing | 441 |
| Deep Engage | > 30s | Extended interaction, decision-making | 442 |

The `REACH_DETECTED` event (443) fires independently whenever a sudden high-frequency phase burst is detected while the customer is standing still.

#### Events

| Event ID | Name | Value | When Emitted |
|----------|------|-------|--------------|
| 440 | `SHELF_BROWSE` | Engagement duration in seconds | On classification (with cooldown) |
| 441 | `SHELF_CONSIDER` | Engagement duration in seconds | On level upgrade |
| 442 | `SHELF_ENGAGE` | Engagement duration in seconds | On level upgrade |
| 443 | `REACH_DETECTED` | Phase perturbation magnitude | Per reach burst |

#### API

```rust
use wifi_densepose_wasm_edge::ret_shelf_engagement::ShelfEngagementDetector;

let mut se = ShelfEngagementDetector::new();

// Per-frame: presence (0/1), motion energy, variance, per-subcarrier phases
let events = se.process_frame(presence, motion_energy, variance, &phases);

// Queries
se.engagement_level()     // -> EngagementLevel (None|Browse|Consider|DeepEngage)
se.engagement_duration_s() // -> f32 (seconds)
se.total_browse_events()   // -> u32
se.total_consider_events() // -> u32
se.total_engage_events()   // -> u32
se.total_reach_events()    // -> u32
```

#### Configuration Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `BROWSE_THRESH_S` | 5.0s (100 frames) | Engagement time for Browse |
| `CONSIDER_THRESH_S` | 30.0s (600 frames) | Engagement time for Consider |
| `STILL_MOTION_THRESH` | 0.08 | Motion below this = standing still |
| `PHASE_PERTURBATION_THRESH` | 0.04 | Phase variance for interaction |
| `REACH_BURST_THRESH` | 0.15 | Phase burst for reach detection |
| `STILL_DEBOUNCE` | 10 frames (0.5s) | Stillness confirmation before counting |
| `ENGAGEMENT_COOLDOWN` | 60 frames (3s) | Cooldown between engagement events |

#### Example: Planogram Analytics

```python
# Shelf performance analytics
shelf_stats = defaultdict(lambda: {"browse": 0, "consider": 0, "engage": 0, "reaches": 0})

if event_id == 440:  # SHELF_BROWSE
    shelf_stats[shelf_id]["browse"] += 1
elif event_id == 441:  # SHELF_CONSIDER
    shelf_stats[shelf_id]["consider"] += 1
elif event_id == 442:  # SHELF_ENGAGE
    shelf_stats[shelf_id]["engage"] += 1
    duration_s = value
    if duration_s > 60:
        analytics.flag_decision_difficulty(shelf_id)
elif event_id == 443:  # REACH_DETECTED
    shelf_stats[shelf_id]["reaches"] += 1

# Conversion funnel: Browse -> Consider -> Engage
# Low consider-to-engage ratio = poor shelf placement or pricing
```

---

## Use Cases

### Retail Store Layout Optimization

Deploy ESP32 nodes at key locations:
- **Entrance**: Customer Flow module counts foot traffic and peak hours
- **Checkout lanes**: Queue Length module monitors wait times, triggers "open register" alerts
- **Aisles**: Dwell Heatmap identifies high-traffic zones for premium product placement
- **Endcaps/displays**: Shelf Engagement measures which displays convert attention to interaction

```
                    Entrance
                  (CustomerFlow)
                       |
        +--------------+--------------+
        |              |              |
   Aisle 1         Aisle 2        Aisle 3
 (DwellHeatmap)  (DwellHeatmap) (DwellHeatmap)
        |              |              |
   [Shelf A]       [Shelf B]      [Shelf C]
 (ShelfEngage)   (ShelfEngage)  (ShelfEngage)
        |              |              |
        +--------------+--------------+
                       |
                  Checkout Area
                 (QueueLength x3)
```

### Restaurant Operations

Deploy per-table ESP32 nodes plus entrance/exit nodes:

- **Entrance**: Customer Flow tracks customer arrivals
- **Each table**: Table Turnover monitors seating lifecycle
- **Host stand**: Queue Length estimates wait time for walk-ins
- **Kitchen view**: Dwell Heatmap identifies server traffic patterns

Key metrics:
- Average seating duration per table
- Turnovers per hour (efficiency)
- Peak vs. off-peak utilization
- Wait time vs. party size correlation

### Shopping Mall Analytics

Multi-floor, multi-zone deployment:

- **Mall entrances** (4-8 nodes): Customer Flow for total foot traffic + directionality
- **Food court**: Table Turnover + Queue Length per restaurant
- **Anchor store entrances**: Customer Flow per store
- **Common areas**: Dwell Heatmap for seating area utilization
- **Kiosks/pop-ups**: Shelf Engagement for promotional display effectiveness

### Event Venue Management

- **Gates**: Customer Flow for entry/exit counting, capacity monitoring
- **Concession stands**: Queue Length with staff dispatch alerts
- **Seating sections**: Dwell Heatmap for section utilization
- **Merchandise areas**: Shelf Engagement for product interest

---

## Integration Architecture

```
ESP32 Nodes (per zone)
    |
    v  UDP events (port 5005)
Sensing Server (wifi-densepose-sensing-server)
    |
    v  REST API + WebSocket
+---+---+---+---+
|   |   |   |   |
v   v   v   v   v
POS Dashboard  Staff   Analytics
             Pager    Backend
```

### Event Packet Format

Each event is a `(event_type: i32, value: f32)` pair. Multiple events per frame are packed into a single UDP packet. The sensing server deserializes and exposes them via:

- `GET /api/v1/sensing/latest` -- latest raw events
- `GET /api/v1/sensing/events?type=400-403` -- filtered by event type
- WebSocket `/ws/events` -- real-time stream

### Privacy Considerations

These modules process WiFi CSI data (channel amplitude and phase), not video or personally identifiable information. No MAC addresses, device identifiers, or individual tracking data leaves the ESP32. All output is aggregate metrics: counts, durations, zone labels. This makes WiFi sensing suitable for jurisdictions with strict privacy requirements (GDPR, CCPA) where camera-based analytics would require consent forms or impact assessments.
