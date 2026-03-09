# Research Document 06: ESP32 Mesh Hardware Constraints for RF Topological Sensing

**Date**: 2026-03-08
**Status**: Research
**Scope**: Hardware constraints, mesh topology design, and computational feasibility
for ESP32-based RF topological sensing using CSI coherence edge weights and
minimum-cut boundary detection.

---

## Table of Contents

1. [ESP32 CSI Capabilities](#1-esp32-csi-capabilities)
2. [Mesh Topology Design](#2-mesh-topology-design)
3. [TDM Synchronized Sensing](#3-tdm-synchronized-sensing)
4. [Computational Budget](#4-computational-budget)
5. [Channel Hopping](#5-channel-hopping)
6. [Power and Thermal](#6-power-and-thermal)
7. [Firmware Architecture](#7-firmware-architecture)
8. [Edge vs Server Computing](#8-edge-vs-server-computing)

---

## 1. ESP32 CSI Capabilities

### 1.1 Subcarrier Counts by Bandwidth

The number of usable CSI subcarriers depends on the WiFi bandwidth mode and
the specific ESP32 variant. OFDM channel structure allocates subcarriers as
follows:

| Parameter              | HT20 (20 MHz)  | HT40 (40 MHz)  | HE20 (WiFi 6)  |
|------------------------|-----------------|-----------------|-----------------|
| Total OFDM subcarriers | 64              | 128             | 256             |
| Null subcarriers       | 12              | 14              | —               |
| Pilot subcarriers      | 4               | 6               | —               |
| Data subcarriers       | 48              | 108             | —               |
| CSI reported (ESP32)   | 52 (data+pilot) | 114 (data+pilot)| N/A             |
| CSI reported (ESP32-S3)| 52              | 114             | N/A             |
| CSI reported (ESP32-C6)| 52              | 114             | 52 (HE mode)    |

For RF topological sensing, each subcarrier provides an independent complex
measurement H(f_k) = |H(f_k)| * exp(j * phi(f_k)). More subcarriers yield
finer frequency-domain resolution, improving coherence estimation between
TX-RX pairs.

**Practical subcarrier usage for edge weight computation:**

```
HT20:  52 subcarriers  x  2 (real, imag)  =  104 values per CSI frame
HT40: 114 subcarriers  x  2 (real, imag)  =  228 values per CSI frame

Edge weight coherence = |<H_ab(f) * conj(H_ab_ref(f))>_f| / (|H_ab| * |H_ref|)
```

The 52-subcarrier HT20 mode is the recommended baseline for mesh sensing
because: (a) all ESP32 variants support it, (b) it avoids 40 MHz channel
bonding issues in dense 2.4 GHz environments, and (c) 52 subcarriers provide
sufficient frequency diversity for coherence estimation.

### 1.2 Sampling Rate Limits

CSI extraction rate is bounded by several factors:

| Constraint                    | Limit           | Notes                          |
|-------------------------------|-----------------|--------------------------------|
| WiFi beacon interval          | 100 ms (10 Hz)  | Default AP beacon rate         |
| ESP-NOW packet rate (burst)   | ~200 pps        | Per-node practical limit       |
| CSI callback processing       | ~50 us          | Copy + timestamp per frame     |
| TDM slot duration             | 2-5 ms          | Minimum slot for TX + CSI RX   |
| Practical mesh sensing rate   | 10-50 Hz        | Per TX-RX pair, TDM limited    |

For a 16-node mesh with 120 edges, if each edge requires one TDM slot of
3 ms, a full mesh sweep takes:

```
16 TX nodes x 3 ms/slot = 48 ms per full sweep
=> ~20 Hz full-mesh update rate
```

This 20 Hz rate is sufficient for human motion sensing (walking cadence
~2 Hz, gesture bandwidth ~5 Hz) while leaving headroom for processing.

### 1.3 Phase Noise Characteristics

Phase noise is the primary challenge for CSI-based coherence sensing. Sources
include:

| Source                          | Magnitude       | Mitigation                     |
|---------------------------------|-----------------|--------------------------------|
| Local oscillator (LO) offset   | 0 - 2*pi random | Phase calibration per packet   |
| Sampling frequency offset (SFO)| Linear drift    | Subcarrier slope correction    |
| Thermal noise (receiver)       | ~-90 dBm floor  | Averaging, >-70 dBm signal     |
| Multipath fading               | Rayleigh dist.  | Frequency diversity            |
| ADC quantization               | ~8 bits ESP32   | Limits dynamic range to ~48 dB |

**Phase calibration procedure for each CSI frame:**

```
1. Extract pilot subcarrier phases: phi_p[k] for k in {-21, -7, +7, +21}
2. Fit linear model: phi_p[k] = a*k + b  (SFO slope + LO offset)
3. Correct all subcarriers: phi_corrected[k] = phi_raw[k] - (a*k + b)
4. Residual phase noise after correction: typically < 0.3 rad (1-sigma)
```

The residual phase noise of ~0.3 rad after calibration means coherence
measurements between stable TX-RX pairs achieve values of 0.90-0.95 in
line-of-sight conditions, dropping to 0.3-0.6 when a person obstructs the
path. This contrast is the basis for edge-weight-based boundary detection.

### 1.4 MIMO Capabilities

| Feature           | ESP32           | ESP32-S3        | ESP32-C6        |
|-------------------|-----------------|-----------------|-----------------|
| WiFi standard     | 802.11 b/g/n    | 802.11 b/g/n    | 802.11 b/g/n/ax |
| TX antennas       | 1               | 1               | 1               |
| RX antennas       | 1               | 1               | 1               |
| MIMO CSI          | 1x1 only        | 1x1 only        | 1x1 only        |
| Antenna switching | GPIO-controlled | GPIO-controlled | GPIO-controlled  |
| External antenna  | U.FL connector  | U.FL connector  | PCB + U.FL      |

All current ESP32 variants provide only 1x1 SISO CSI. True MIMO would require
multiple RF chains, which these SoCs do not expose for CSI extraction. However,
spatial diversity can be achieved at the mesh level: with 16 nodes, each
location is observed from up to 15 different angles, providing far richer
spatial coverage than a single MIMO access point.

### 1.5 ESP32 Variant Comparison for Sensing

| Feature                | ESP32 (classic)  | ESP32-S3         | ESP32-C6         |
|------------------------|------------------|------------------|------------------|
| CPU                    | Dual Xtensa LX6  | Dual Xtensa LX7  | Single RISC-V    |
| Clock speed            | 240 MHz          | 240 MHz          | 160 MHz          |
| RAM                    | 520 KB SRAM      | 512 KB SRAM      | 512 KB SRAM      |
| PSRAM support          | Up to 8 MB       | Up to 8 MB       | Up to 4 MB       |
| WiFi                   | 2.4 GHz          | 2.4 GHz          | 2.4 GHz + 6 GHz* |
| WiFi 6 (802.11ax)     | No               | No               | Yes              |
| BLE                    | 4.2              | 5.0              | 5.0              |
| CSI extraction         | Yes (IDF 4.x+)   | Yes (IDF 5.x+)   | Yes (IDF 5.x+)   |
| ESP-NOW support        | Yes              | Yes              | Yes              |
| USB OTG                | No               | Yes              | No               |
| ULP coprocessor        | Yes (FSM)        | Yes (RISC-V)     | No               |
| Price (module, qty 100)| ~$2.50           | ~$3.00           | ~$2.80           |
| Power (active WiFi)    | ~160 mA          | ~150 mA          | ~130 mA          |
| CSI maturity           | Most tested      | Well tested      | Newer, less tested|

*ESP32-C6 supports WiFi 6 at 2.4 GHz. The 6 GHz band requires regional
regulatory compliance and is not yet broadly available for CSI extraction.

**Recommendation**: ESP32 (classic) for initial deployment due to mature CSI
support, dual-core architecture for concurrent TX/RX/processing, and lowest
cost. ESP32-C6 is the forward-looking choice for WiFi 6 HE-LTF CSI, which
provides longer training fields and potentially better channel estimation.

---

## 2. Mesh Topology Design

### 2.1 16-Node Perimeter Layout

For a 5m x 5m room, 16 nodes are placed around the perimeter at approximately
1 m spacing. The layout provides 4 nodes per wall:

```
         North Wall
    N1 --- N2 --- N3 --- N4
    |                     |
    |                     |
   N16                   N5
    |                     |
    |                     |
   N15    5m x 5m        N6
    |     sensing         |
    |     volume          |
   N14                   N7
    |                     |
    |                     |
    N13 -- N12 -- N11 -- N8
         South Wall

    Node spacing: ~1.25 m along each 5m wall
    Height: 1.0 m above floor (torso-level sensing)
```

### 2.2 Link Geometry and Edge Count

With 16 nodes, the maximum number of undirected edges is C(16,2) = 120.
Not all edges are equally useful for sensing:

| Edge category         | Count | Path length   | Sensing utility          |
|-----------------------|-------|---------------|--------------------------|
| Adjacent (same wall)  | 16    | 1.0 - 1.25 m  | Low: short path, grazing |
| Same-wall skip-1      | 12    | 2.0 - 2.5 m   | Medium: some penetration |
| Cross-room diagonal   | 24    | 5.0 - 7.1 m   | High: traverses interior |
| Opposite wall         | 16    | 5.0 m         | High: full penetration   |
| Adjacent wall corner  | 24    | 1.4 - 5.1 m   | Medium to high           |
| Other cross-links     | 28    | 2.5 - 6.0 m   | Medium to high           |
| **Total**             |**120**|               |                          |

**Coverage analysis**: Any point in the 5m x 5m room interior is traversed by
at least 20 TX-RX links. The center of the room is crossed by approximately
50 links. This density ensures that a person standing anywhere in the room
perturbs multiple edges, enabling robust boundary detection via minimum cut.

```
    Link density map (approx links crossing each 1m^2 cell):

         N1    N2    N3    N4
    N16 [ 22 | 28 | 28 | 22 ] N5
        [----+----+----+----|
    N15 [ 28 | 45 | 45 | 28 ] N6
        [----+----+----+----|
    N14 [ 28 | 45 | 45 | 28 ] N7
        [----+----+----+----|
    N13 [ 22 | 28 | 28 | 22 ] N8
         N12   N11   N10   N9

    Minimum link density: ~22 (corners)
    Maximum link density: ~45 (center)
```

### 2.3 Graph Properties for Minimum Cut

The 16-node complete graph K_16 has properties relevant to Stoer-Wagner
minimum cut computation:

| Property                      | Value           |
|-------------------------------|-----------------|
| Vertices                      | 16              |
| Edges                         | 120             |
| Graph diameter                | 1 (complete)    |
| Vertex connectivity           | 15              |
| Min-cut of unweighted K_16    | 15              |
| Adjacency matrix size         | 16 x 16 = 256   |
| Adjacency matrix (bytes)      | 256 x 4 = 1 KB  |

When edge weights represent CSI coherence (0.0 to 1.0), the minimum cut
partitions nodes into two groups where the sum of coherence weights across
the cut is minimized. This corresponds to the physical boundary where RF
propagation is most disrupted, typically where a person is standing or
where a wall partition exists.

### 2.4 Spatial Resolution

The achievable spatial resolution depends on link density and the Fresnel
zone width of each link:

```
Fresnel zone radius (first zone):
  r_F = sqrt(lambda * d1 * d2 / (d1 + d2))

For 2.4 GHz (lambda = 0.125 m), 5m cross-room link:
  r_F = sqrt(0.125 * 2.5 * 2.5 / 5.0) = 0.28 m

For 5 GHz (lambda = 0.06 m), 5m cross-room link:
  r_F = sqrt(0.06 * 2.5 * 2.5 / 5.0) = 0.19 m
```

With 120 links and Fresnel zones of ~0.2-0.3 m, the effective spatial
resolution for boundary detection is approximately 0.3-0.5 m. This is
sufficient to detect individual humans (shoulder width ~0.4 m) and to
distinguish between two people standing 1 m apart.

### 2.5 Installation Geometry

Practical mounting considerations for perimeter nodes:

```
    Side view (one wall):

    Ceiling (2.5m) ─────────────────────────
                     |
                     |  1.5 m clearance
                     |
    Node height ─── [N] ── 1.0 m above floor
                     |
                     |  1.0 m
                     |
    Floor (0.0m) ────────────────────────────

    Mounting: adhesive, screw mount, or magnetic
    Orientation: antenna perpendicular to wall
    Cable: USB-C power (5V, 500mA per node)
```

Nodes at 1.0 m height capture torso-level RF interactions, which provide
the strongest CSI perturbations from human presence (largest cross-section).
Ceiling mounting (2.5 m) is an alternative that avoids obstruction but
reduces sensitivity to seated or crouching individuals.

---

## 3. TDM Synchronized Sensing

### 3.1 Time-Division Multiplexing Protocol

In a 16-node mesh, only one node should transmit at a time to avoid packet
collisions that corrupt CSI measurements. TDM assigns each node a dedicated
time slot for transmission:

```
    TDM Frame Structure (one complete sweep):

    |<-- Slot 0 -->|<-- Slot 1 -->|<-- Slot 2 -->| ... |<-- Slot 15 -->|
    |   Node 1 TX  |   Node 2 TX  |   Node 3 TX  |     |  Node 16 TX  |
    |  all others  |  all others  |  all others  |     |  all others  |
    |  extract CSI |  extract CSI |  extract CSI |     |  extract CSI |
    |              |              |              |     |              |
    |<-- 3 ms ---->|<-- 3 ms ---->|<-- 3 ms ---->|     |<-- 3 ms ---->|

    Total frame: 16 * 3 ms = 48 ms => 20.8 Hz sweep rate
```

### 3.2 Slot Timing Breakdown

Each TDM slot contains multiple phases:

| Phase            | Duration | Purpose                                    |
|------------------|----------|--------------------------------------------|
| Guard interval   | 200 us   | Prevent overlap from clock drift           |
| TX preamble      | 100 us   | ESP-NOW packet transmission start          |
| TX payload       | 200 us   | Packet data (minimal, used for CSI trigger)|
| CSI extraction   | 50 us    | Hardware CSI capture at all RX nodes       |
| Processing       | 450 us   | Phase calibration, coherence update        |
| Idle/buffer      | 2000 us  | Margin for jitter and processing overrun   |
| **Total slot**   | **3 ms** |                                            |

### 3.3 ESP-NOW for TDM Coordination

ESP-NOW is the transport layer for TDM sensing packets. Key characteristics:

| Parameter                | Value                                       |
|--------------------------|---------------------------------------------|
| Protocol                 | Vendor-specific action frame (802.11)       |
| Max payload              | 250 bytes                                   |
| Encryption               | Optional (CCMP), adds ~50 us latency        |
| Broadcast latency        | ~1 ms (measured)                             |
| Unicast latency          | ~0.5 ms (measured)                           |
| Delivery confirmation    | Unicast only (ACK-based)                     |
| Max peers (encrypted)    | 6 (ESP32), 16 (ESP32-S3)                    |
| Max peers (unencrypted)  | 20                                           |
| CSI extraction on RX     | Yes, via wifi_csi_config_t callback          |

For TDM sensing, broadcast mode is used: the transmitting node sends one
ESP-NOW broadcast packet, and all 15 other nodes extract CSI from the
received frame simultaneously. This means each TDM slot produces 15 CSI
measurements (one per RX node), and a full 16-slot sweep produces
16 x 15 = 240 directional CSI measurements (120 unique TX-RX pairs,
each measured twice in both directions).

### 3.4 Synchronization Accuracy

TDM requires all nodes to agree on slot boundaries. Synchronization sources:

| Method                     | Accuracy      | Complexity | Notes              |
|----------------------------|---------------|------------|--------------------|
| NTP over WiFi              | 1-10 ms       | Low        | Requires AP        |
| ESP-NOW timestamp exchange | 100-500 us    | Medium     | Peer-to-peer       |
| Hardware timer + NTP seed  | 50-200 us     | Medium     | Drift correction   |
| GPIO pulse (wired sync)    | <1 us         | High       | Requires wiring    |
| Beacon timestamp (passive) | 1-5 ms        | Low        | Piggyback on AP    |

**Recommended approach**: ESP-NOW timestamp exchange with periodic
resynchronization. One node acts as the TDM coordinator (master), broadcasting
a sync beacon every 1 second containing its microsecond timer value. Other
nodes adjust their local slot counters to align.

```
    Synchronization protocol:

    Master (N1):  [SYNC_BEACON t=0] -----> all nodes
                  |
                  |  Each node computes offset:
                  |  offset = t_local_rx - t_master_tx - propagation_delay
                  |  propagation_delay ~ 17 ns (5m / c) => negligible
                  |
                  v
    Slave (Nk):   slot_start[i] = (t_master + offset) + i * SLOT_DURATION
                  Accuracy: ~200 us (sufficient for 3 ms slots)
```

With 200 us synchronization accuracy and 200 us guard intervals, the
probability of slot overlap is negligible. The 3 ms slot duration provides
a 14:1 ratio of useful time to guard time.

### 3.5 TDM Failure Modes and Recovery

| Failure                    | Detection                | Recovery                  |
|----------------------------|--------------------------|---------------------------|
| Node clock drift           | Increasing CSI jitter    | Resync on next beacon     |
| Missed sync beacon         | Beacon timeout (>2s)     | Free-run on local clock   |
| Packet collision           | CSI amplitude anomaly    | Skip frame, continue      |
| Node offline               | Missing CSI for N slots  | Remove from TDM schedule  |
| Master node failure        | No sync beacon for 5s    | Lowest-ID node takes over |

---

## 4. Computational Budget

### 4.1 Stoer-Wagner Minimum Cut on 16-Node Graph

The Stoer-Wagner algorithm finds the global minimum cut of an undirected
weighted graph in O(V^3) time (or O(V * E) with a priority queue). For
V = 16, E = 120:

```
    Stoer-Wagner complexity analysis:

    Algorithm: V-1 = 15 phases
    Each phase: MinimumCutPhase
      - Priority queue operations: O(V * log(V)) with binary heap
      - Edge weight updates: O(E) per phase

    Total operations:
      Phases:              15
      PQ operations/phase: 16 * log2(16) = 64
      Edge scans/phase:    120
      Total PQ ops:        15 * 64 = 960
      Total edge scans:    15 * 120 = 1,800

      Grand total:         ~2,760 operations (additions + comparisons)

    Simplified estimate:   ~2,000 operations (core arithmetic)
```

### 4.2 Operations Per Second at 20 Hz

```
    At 20 Hz full-mesh sweep rate:
      Stoer-Wagner per sweep:     ~2,000 ops
      Sweeps per second:          20
      Stoer-Wagner ops/sec:       40,000

    Additional per-sweep work:
      CSI coherence updates:      120 edges * 52 subcarriers = 6,240 complex multiplies
      Phase calibration:          15 RX * 4 pilot subcarriers = 60 linear fits
      Edge weight smoothing:      120 exponential moving averages

    Total compute per second:
      Stoer-Wagner:               40,000 ops
      Coherence estimation:       20 * 6,240 = 124,800 complex ops
      Phase calibration:          20 * 60 = 1,200 linear fits
      EMA smoothing:              20 * 120 = 2,400 multiply-adds

    Grand total:                  ~170,000 operations/second
```

### 4.3 ESP32 Computational Capacity

```
    ESP32 (dual-core Xtensa LX6 @ 240 MHz):

    Theoretical peak:
      Integer ops:        240 MIPS per core (single-issue)
      FP ops (SW):        ~30 MFLOPS (software float)
      FP ops (estimated): ~10-20 MFLOPS practical

    Our workload:         ~170,000 ops/sec = 0.17 MOPS

    Utilization:          0.17 / 240 = 0.07% of one core

    Available headroom:   99.93% of one core
                          Plus entire second core for WiFi stack
```

The Stoer-Wagner computation plus CSI processing consumes less than 0.1%
of one ESP32 core. This leaves enormous headroom for:

- Additional signal processing (filtering, spectral analysis)
- Local feature extraction
- Communication overhead
- Firmware housekeeping (watchdog, OTA updates)

### 4.4 Memory Budget

| Data structure               | Size              | Notes                    |
|------------------------------|-------------------|--------------------------|
| Adjacency matrix (16x16 f32) | 1,024 bytes       | Edge weights             |
| CSI buffer (1 frame, HT20)  | 208 bytes         | 52 complex values (i8)   |
| CSI ring buffer (16 frames)  | 3,328 bytes       | Last frame from each TX  |
| Phase calibration state      | 256 bytes         | Per-TX LO/SFO params     |
| Coherence accumulators       | 960 bytes         | 120 edges x 2 x f32     |
| Stoer-Wagner workspace       | 512 bytes         | Priority queue, merged[] |
| TDM scheduler state          | 128 bytes         | Slot counter, sync       |
| ESP-NOW peer table           | 480 bytes         | 16 peers x 30 bytes     |
| **Total sensing data**       | **~7 KB**         |                          |

Against 520 KB SRAM (or up to 8 MB PSRAM), the sensing data structures
consume approximately 1.3% of internal SRAM. Even without PSRAM, there is
ample memory for firmware, WiFi stack (~40 KB), and application logic.

### 4.5 Computational Comparison

| Operation              | Ops/sweep | At 20 Hz    | ESP32 capacity | Utilization |
|------------------------|-----------|-------------|----------------|-------------|
| Stoer-Wagner mincut    | 2,000     | 40,000/s    | 240 M/s        | 0.017%      |
| CSI coherence          | 6,240     | 124,800/s   | 240 M/s        | 0.052%      |
| Phase calibration      | 240       | 4,800/s     | 240 M/s        | 0.002%      |
| Edge weight EMA        | 120       | 2,400/s     | 240 M/s        | 0.001%      |
| **Total**              |**~8,600** |**~172,000/s**| **240 M/s**   | **0.072%**  |

The computation is trivially feasible on ESP32. The bottleneck is not
compute but rather the TDM sweep rate (limited by RF timing) and network
bandwidth for transmitting results to the server.

---

## 5. Channel Hopping

### 5.1 2.4 GHz Channel Plan

The 2.4 GHz ISM band provides 13 channels (14 in Japan), of which only
3 are non-overlapping:

```
    2.4 GHz Channel Map (20 MHz bandwidth):

    Ch 1:  2.401 - 2.423 GHz  [====]
    Ch 2:  2.406 - 2.428 GHz     [====]
    Ch 3:  2.411 - 2.433 GHz        [====]
    Ch 4:  2.416 - 2.438 GHz           [====]
    Ch 5:  2.421 - 2.443 GHz              [====]
    Ch 6:  2.426 - 2.448 GHz                 [====]
    Ch 7:  2.431 - 2.453 GHz                    [====]
    Ch 8:  2.436 - 2.458 GHz                       [====]
    Ch 9:  2.441 - 2.463 GHz                          [====]
    Ch 10: 2.446 - 2.468 GHz                             [====]
    Ch 11: 2.451 - 2.473 GHz                                [====]
    Ch 12: 2.456 - 2.478 GHz                                   [====]
    Ch 13: 2.461 - 2.483 GHz                                      [====]

    Non-overlapping: Ch 1, Ch 6, Ch 11
```

### 5.2 5 GHz Channel Plan (ESP32-C6 only)

The ESP32-C6 with WiFi 6 support can potentially access 5 GHz UNII bands,
though CSI extraction on 5 GHz channels is less mature:

| Band     | Channels       | Bandwidth | DFS required | Indoor only |
|----------|----------------|-----------|--------------|-------------|
| UNII-1   | 36, 40, 44, 48 | 20 MHz    | No           | No          |
| UNII-2   | 52, 56, 60, 64 | 20 MHz    | Yes          | No          |
| UNII-2E  | 100-144        | 20 MHz    | Yes          | No          |
| UNII-3   | 149-165        | 20 MHz    | No           | No          |

5 GHz advantages for sensing: shorter wavelength (6 cm vs 12.5 cm) provides
better spatial resolution, and the band is typically less congested.

### 5.3 Multi-Channel Sensing Strategy

Channel hopping serves two purposes: (a) frequency diversity improves
coherence robustness against narrowband interference, and (b) different
frequencies interact differently with the environment, providing
complementary information.

```
    Channel Hopping Schedule (3-channel rotation):

    Sweep 0:  Ch 1  -- all 16 TDM slots -- 48 ms
    Sweep 1:  Ch 6  -- all 16 TDM slots -- 48 ms
    Sweep 2:  Ch 11 -- all 16 TDM slots -- 48 ms
    [repeat]

    Channel switch overhead: ~5 ms (wifi_set_channel)
    Total 3-channel cycle: 3 * (48 + 5) = 159 ms => 6.3 Hz per channel
    Effective sensing rate: 6.3 Hz (per channel) or 18.9 Hz (combined)
```

### 5.4 Channel Switching Overhead

| Operation                        | Duration    | Notes                     |
|----------------------------------|-------------|---------------------------|
| wifi_set_channel()               | 2-5 ms      | PLL relock time           |
| CSI stabilization after switch   | 1-2 frames  | First frame may be noisy  |
| ESP-NOW peer re-association      | 0 ms        | Channel-agnostic          |
| Total overhead per switch        | ~5 ms       | Including stabilization   |

### 5.5 Interference Mitigation

Channel hopping provides resilience against common 2.4 GHz interference:

| Interference source       | Typical channel | Mitigation via hopping     |
|---------------------------|-----------------|----------------------------|
| WiFi access points        | 1, 6, or 11     | Hop to unused channels     |
| Bluetooth                 | Spread (1 MHz)   | Narrowband; averaged out   |
| Microwave ovens           | ~10 (2.45 GHz)   | Avoid Ch 9-11 during use   |
| Zigbee / Thread           | 15, 20, 25, 26   | Minimal overlap with WiFi  |
| Baby monitors             | Variable         | Hop provides resilience    |

**Adaptive channel selection**: Before starting the sensing session, perform
a quick spectrum survey (wifi_scan) to identify the least congested channels.
Periodically re-survey (every 60 seconds) and adjust the hopping pattern.

### 5.6 Multi-Band Fusion

When ESP32-C6 nodes provide both 2.4 GHz and 5 GHz CSI, the edge weight
can be computed as a weighted combination:

```
    w_edge(a,b) = alpha * coherence_2_4GHz(a,b) + (1 - alpha) * coherence_5GHz(a,b)

    Default alpha = 0.6 (favor 2.4 GHz for longer range, better penetration)

    Benefits:
    - 2.4 GHz: better wall penetration, longer range, diffraction around body
    - 5 GHz:   higher spatial resolution, less multipath spread
    - Combined: more robust boundary detection, reduced false positives
```

---

## 6. Power and Thermal

### 6.1 Power Consumption by Operating Mode

| Mode                    | Current (3.3V) | Power    | Notes                    |
|-------------------------|----------------|----------|--------------------------|
| Active TX (ESP-NOW)     | 180-240 mA     | 0.6-0.8W | During TDM TX slot       |
| Active RX (CSI listen)  | 95-120 mA      | 0.3-0.4W | During other TX slots    |
| Active RX + processing  | 130-160 mA     | 0.4-0.5W | CSI extraction + compute |
| Light sleep             | 0.8 mA         | 2.6 mW   | Between sweeps (if used) |
| Deep sleep              | 10 uA          | 33 uW    | Not useful for sensing   |
| Modem sleep             | 20 mA          | 66 mW    | WiFi off, CPU active     |

### 6.2 Continuous Sensing Power Budget

For continuous 20 Hz mesh sensing, each node cycles between TX and RX:

```
    Per-node duty cycle analysis (one sweep = 48 ms):

    TX slot:        1 slot  x 3 ms =  3 ms   @ 200 mA
    RX slots:      15 slots x 3 ms = 45 ms   @ 130 mA
    Total per sweep:                  48 ms

    Average current per sweep:
      I_avg = (3/48)*200 + (45/48)*130 = 12.5 + 121.9 = 134.4 mA

    At 20 sweeps/sec (continuous):
      No idle time between sweeps
      I_continuous = 134.4 mA @ 3.3V = 0.44 W per node

    16-node mesh total:
      P_total = 16 * 0.44 W = 7.04 W
```

### 6.3 Battery vs Mains Power

| Power source          | Capacity        | Runtime per node | Notes              |
|-----------------------|-----------------|------------------|--------------------|
| USB-C wall adapter    | Unlimited       | Unlimited        | Preferred for fixed|
| 18650 Li-ion (3.4 Ah)| 12.6 Wh         | ~28 hours        | 3.7V * 3.4Ah / 0.44W |
| 10000 mAh power bank | 37 Wh           | ~84 hours        | 3.5 days           |
| PoE (via splitter)    | Unlimited       | Unlimited        | Requires Ethernet  |
| Solar + battery       | Variable        | Indefinite*      | Outdoor only       |

**Recommended power strategy**:
- **Fixed installation**: USB-C 5V/1A wall adapters. Cost ~$3/node.
  Total 16-node mesh: $48 in adapters, ~7W from mains.
- **Temporary deployment**: 18650 battery holders. 24+ hour runtime.
  Swap batteries daily or use larger packs.

### 6.4 Thermal Analysis

```
    Heat dissipation per node:
      Power: 0.44 W continuous
      Package: QFN 5x5 mm (ESP32 module is 18x25 mm)
      Thermal resistance (junction to ambient): ~40 C/W (typical module)

    Temperature rise:
      dT = P * R_theta = 0.44 * 40 = 17.6 C above ambient

    At 25 C ambient:
      Junction temperature: 25 + 17.6 = 42.6 C
      ESP32 max operating: 105 C
      Margin: 62.4 C

    At 40 C ambient (warm room):
      Junction temperature: 40 + 17.6 = 57.6 C
      Margin: 47.4 C
```

Thermal management is not a concern for this application. The 0.44 W per
node is well within the passive cooling capability of a small PCB. No
heatsink or fan is required.

### 6.5 Power Optimization Strategies

If battery life must be extended beyond the baseline:

| Strategy                       | Savings   | Trade-off                  |
|--------------------------------|-----------|----------------------------|
| Reduce sweep rate to 10 Hz    | ~15%      | Lower temporal resolution  |
| Skip redundant edges (prune)  | ~20%      | Reduced spatial coverage   |
| Duty-cycle sensing (50% on)   | ~45%      | 10 Hz effective rate       |
| Light sleep between sweeps    | ~10%      | Wake-up jitter adds 1 ms   |
| Reduce TX power (-4 dBm)      | ~5%       | Shorter range, lower SNR   |
| Adaptive: sense only on motion| up to 80% | Requires motion trigger    |

The adaptive strategy is most effective: use a single always-on link to
detect motion, then wake all nodes for full mesh sensing only when
activity is detected.

---

## 7. Firmware Architecture

### 7.1 Dual-Core Task Assignment

The ESP32 has two cores (Core 0 and Core 1). FreeRTOS on ESP-IDF allows
pinning tasks to specific cores:

```
    Core 0 (Protocol Core)              Core 1 (Application Core)
    ========================            ==========================
    WiFi driver (pinned)                CSI processing task
    ESP-NOW TX/RX callbacks             Coherence computation
    TDM scheduler (timer ISR)           Edge weight update
    Sync beacon handler                 Stoer-Wagner mincut
    Channel hopping controller          Result serialization
    OTA update handler                  Telemetry / diagnostics

    Priority: RTOS ticks, WiFi > app    Priority: Sensing > logging
    Stack: 4 KB per task                Stack: 4-8 KB per task
```

### 7.2 Task Priorities and Scheduling

| Task                    | Core | Priority | Period     | Stack  |
|-------------------------|------|----------|------------|--------|
| WiFi driver             | 0    | 23 (max) | Event      | 4 KB   |
| TDM slot timer ISR      | 0    | 22       | 3 ms       | 2 KB   |
| ESP-NOW TX              | 0    | 20       | 48 ms      | 4 KB   |
| ESP-NOW RX callback     | 0    | 20       | Event      | 2 KB   |
| Sync beacon handler     | 0    | 18       | 1 s        | 2 KB   |
| CSI extraction callback | 0    | 19       | Event      | 2 KB   |
| CSI processing          | 1    | 15       | 48 ms      | 8 KB   |
| Coherence computation   | 1    | 14       | 48 ms      | 4 KB   |
| Mincut solver           | 1    | 12       | 48 ms      | 4 KB   |
| UART/MQTT reporting     | 1    | 10       | 100 ms     | 4 KB   |
| NVS config manager      | 1    | 5        | On-demand  | 4 KB   |
| Watchdog / health       | 0    | 3        | 5 s        | 2 KB   |

### 7.3 CSI Extraction Pipeline

```
    +-----------+     +------------+     +----------+     +-----------+
    | ESP-NOW   |---->| WiFi CSI   |---->| Ring     |---->| Phase     |
    | RX (HW)   |     | Callback   |     | Buffer   |     | Calibrate |
    +-----------+     +------------+     +----------+     +-----------+
         |                  |                  |                |
         | Core 0           | Core 0           | Shared mem     | Core 1
         | HW interrupt     | ISR context      | Lock-free      | Task context
         |                  |                  | SPSC queue     |
         v                  v                  v                v
    WiFi frame         CSI data copy     16-frame deep     Corrected CSI
    received           (208 bytes)       per-TX buffer     ready for
    from air           + timestamp                         coherence calc

    Latency: <100 us from frame RX to calibrated CSI available
```

### 7.4 Simultaneous TX/RX/CSI Coordination

A critical firmware design constraint is that a node cannot transmit and
receive simultaneously. The TDM protocol resolves this:

```
    Node N_k timeline (one sweep):

    Slot 0:  [RX from N1] --> extract CSI(1,k)
    Slot 1:  [RX from N2] --> extract CSI(2,k)
    ...
    Slot k-1:[RX from Nk-1]--> extract CSI(k-1,k)
    Slot k:  [TX broadcast] --> other nodes extract CSI(*,k)
    Slot k+1:[RX from Nk+1]--> extract CSI(k+1,k)
    ...
    Slot 15: [RX from N16] --> extract CSI(16,k)

    During TX slot: CSI extraction disabled (own transmission)
    During RX slots: CSI extracted from each transmitter
    Result: 15 CSI measurements per node per sweep
```

### 7.5 Firmware State Machine

```
    +----------+     +----------+     +----------+     +----------+
    |  INIT    |---->| DISCOVER |---->| SYNC     |---->| SENSING  |
    |          |     |          |     |          |     |          |
    | WiFi     |     | Find     |     | TDM time |     | Main     |
    | ESP-NOW  |     | peers    |     | alignment|     | loop     |
    | NVS load |     | Exchange |     | Master   |     | 20 Hz    |
    +----------+     | node IDs |     | election |     +----------+
         |           +----------+     +----------+          |
         |                |                |                |
         v                v                v                v
    On boot          5-10 sec          2-3 sec          Continuous
                     timeout           settle           operation

                                                             |
                                            +----------+     |
                                            | RESYNC   |<----+
                                            |          |  On drift
                                            | Re-align |  detected
                                            | TDM slots|  (>500us)
                                            +----------+
                                                 |
                                                 +----> back to SENSING
```

### 7.6 NVS Configuration Parameters

Node configuration stored in non-volatile storage (NVS):

| Key                  | Type   | Default | Description                      |
|----------------------|--------|---------|----------------------------------|
| `node_id`            | u8     | —       | Unique node ID (1-16)            |
| `mesh_size`          | u8     | 16      | Number of nodes in mesh          |
| `tdm_slot_ms`        | u16    | 3       | TDM slot duration (ms)           |
| `sweep_channels`     | u8[]   | [1,6,11]| Channel hopping sequence         |
| `tx_power_dbm`       | i8     | 8       | TX power (2-20 dBm)             |
| `sync_interval_ms`   | u32    | 1000    | Sync beacon period               |
| `report_interval_ms` | u32    | 100     | Result upload period             |
| `server_ip`          | u32    | —       | Backend server IP                |
| `server_port`        | u16    | 8080    | Backend server port              |
| `coherence_alpha`    | f32    | 0.1     | EMA smoothing factor             |
| `ota_url`            | string | —       | Firmware update endpoint         |

### 7.7 Error Handling and Watchdog

```
    Error hierarchy:

    Level 1 (recoverable):
      - Single CSI frame missing    --> skip, continue
      - Coherence value NaN/Inf     --> clamp to 0.0
      - MQTT publish timeout        --> retry next cycle

    Level 2 (resynchronize):
      - Clock drift > 500 us        --> trigger RESYNC state
      - Peer lost for > 5 sweeps    --> remove from schedule
      - Channel congestion detected  --> switch to backup channel

    Level 3 (restart):
      - WiFi driver crash           --> esp_restart()
      - Watchdog timeout (10s)      --> hardware reset
      - PSRAM parity error          --> esp_restart()
      - Stack overflow              --> panic handler, restart

    Hardware watchdog: 10 second timeout
    Task watchdog: 5 second timeout per core
    Heartbeat LED: blink pattern indicates state
      - Solid:    INIT
      - Slow blink: DISCOVER
      - Fast blink: SYNC
      - Breathing: SENSING (normal)
      - SOS:      ERROR
```

---

## 8. Edge vs Server Computing

### 8.1 Computation Partitioning

The fundamental question is: what runs on the ESP32 nodes, and what is
offloaded to a server? The division follows the principle of minimizing
data transfer while keeping latency-sensitive operations local.

```
    +---------------------------------------------------------+
    |                    ESP32 Node (Edge)                     |
    |                                                         |
    |  [CSI Extraction] --> [Phase Cal] --> [Coherence Est]   |
    |         |                                    |          |
    |         v                                    v          |
    |  [Ring Buffer]              [Edge Weight w(a,b)]        |
    |                                    |                    |
    |                                    v                    |
    |                          [Local Mincut]*                |
    |                                    |                    |
    |                                    v                    |
    |                          [MQTT / WebSocket]             |
    +-----------------------|--------------------------------+
                            |
                            | Edge weights (120 x f32 = 480 bytes)
                            | OR mincut result (32 bytes)
                            v
    +---------------------------------------------------------+
    |                   Server (Backend)                       |
    |                                                         |
    |  [Aggregate Edge Weights] --> [Global Mincut]           |
    |         |                          |                    |
    |         v                          v                    |
    |  [Time-series DB]        [Boundary Map]                 |
    |                                |                        |
    |                                v                        |
    |                    [ML Inference (DensePose)]            |
    |                                |                        |
    |                                v                        |
    |                    [Visualization / API]                 |
    +---------------------------------------------------------+

    * Local mincut is optional; server can compute from raw weights
```

### 8.2 What Runs on ESP32

| Function                 | Data volume      | Compute cost   | Why on-device    |
|--------------------------|------------------|----------------|------------------|
| CSI extraction           | 208 B/frame      | HW-assisted    | Hardware function |
| Phase calibration        | 4 pilots/frame   | Minimal        | Per-frame, latency|
| Coherence estimation     | 52 subcarriers   | ~6K ops/sweep  | Reduces TX data  |
| Edge weight (EMA)        | 1 float/edge     | 120 multiply   | Trivial compute  |
| TDM scheduling           | State machine    | Negligible     | Real-time req.   |
| Clock synchronization    | Timer comparison | Negligible     | Real-time req.   |
| Local mincut (optional)  | 16x16 matrix     | ~2K ops/sweep  | Low-latency mode |

**Data reduction on-device**: Raw CSI is 208 bytes per frame, with
240 frames per sweep (16 TX x 15 RX). Transmitting raw CSI would require
240 x 208 = 49,920 bytes per sweep at 20 Hz = ~1 MB/s. By computing
coherence on-device, the output is reduced to 120 edge weights x 4 bytes
= 480 bytes per sweep at 20 Hz = 9.6 KB/s. This is a 100x reduction
in network bandwidth.

### 8.3 What Runs on Server

| Function                 | Input              | Compute cost     | Why on server    |
|--------------------------|--------------------|------------------|------------------|
| Edge weight aggregation  | 480 B/sweep/node   | Minimal         | Central view     |
| Multi-channel fusion     | 3 channel weights  | 360 multiply    | Cross-channel    |
| Global mincut            | 120 edge weights   | ~2K ops         | Central graph    |
| Temporal analysis        | Weight time-series | Moderate        | History needed   |
| ML pose inference        | Edge weights       | ~100M ops       | GPU required     |
| Visualization            | Boundary map       | Render pipeline | Display          |
| Occupancy tracking       | Mincut sequence    | Moderate        | Multi-room state |
| Alert generation         | Boundary events    | Minimal         | Business logic   |

### 8.4 Communication Protocol

```
    ESP32 --> Server message format (MQTT or WebSocket):

    Header (8 bytes):
      node_id:      u8        # Source node
      sweep_id:     u32       # Monotonic counter
      channel:      u8        # WiFi channel used
      timestamp_ms: u16       # Milliseconds within second

    Payload (480 bytes):
      edge_weights: [f32; 120]  # Coherence values for all edges

    Optional (4 bytes):
      local_mincut_value: f32   # If computed on-device

    Total: 488-492 bytes per sweep per node
    At 20 Hz: ~9.8 KB/s per node

    16-node mesh aggregate:
      Each node sends its 15 observed edge weights
      Server reconstructs full 120-edge weight matrix
      Total bandwidth: 16 * 9.8 KB/s = 156.8 KB/s
```

### 8.5 Latency Budget

End-to-end latency from physical event to boundary detection:

| Stage                        | Latency     | Cumulative  |
|------------------------------|-------------|-------------|
| Physical perturbation occurs | 0 ms        | 0 ms        |
| Next TDM sweep includes edge | 0-48 ms     | 24 ms avg   |
| CSI extraction + calibration | 0.1 ms      | 24.1 ms     |
| Coherence estimation         | 0.05 ms     | 24.15 ms    |
| EMA smoothing (alpha=0.1)    | N/A (delay) | ~5 sweeps   |
| MQTT publish                 | 5-20 ms     | 44.15 ms    |
| Server mincut computation    | 0.01 ms     | 44.16 ms    |
| Visualization update         | 16 ms       | 60.16 ms    |
| **Total (excl. EMA delay)**  |             | **~60 ms**  |
| **Total (incl. EMA settle)** |             | **~300 ms** |

The ~300 ms total latency (including EMA settling) is suitable for
real-time occupancy and boundary detection. For faster response (e.g.,
gesture recognition), the EMA smoothing factor can be increased
(alpha = 0.3) at the cost of noisier measurements, reducing settle time
to ~150 ms.

### 8.6 Hybrid Architecture Decision Matrix

| Scenario                    | Edge-only  | Server-only | Hybrid (rec.)  |
|-----------------------------|------------|-------------|----------------|
| Single room, 16 nodes      | Feasible   | Overkill    | Best balance   |
| Multi-room, 64 nodes       | Complex    | Required    | Required       |
| Battery-powered nodes      | Preferred  | Not viable  | Edge-heavy     |
| ML pose estimation needed  | Not viable | Required    | Server for ML  |
| Low-latency alerts (<100ms)| Preferred  | Adds delay  | Edge for alerts|
| Historical analysis        | No storage | Required    | Server for DB  |
| Privacy-sensitive           | Preferred  | Risk        | Edge preferred |

### 8.7 Aggregation Node Architecture

For deployments where a dedicated server is impractical, one ESP32 node
(or an ESP32-S3 with PSRAM) can serve as the aggregation point:

```
    Standard Mesh Node (x15):
      - CSI extraction
      - Coherence computation
      - Report edge weights to aggregator

    Aggregation Node (x1, ESP32-S3 recommended):
      - All standard node functions
      - Receive edge weights from 15 peers
      - Assemble full graph
      - Run Stoer-Wagner mincut
      - Serve results via HTTP (optional)
      - Forward to cloud (optional)

    Aggregator requirements:
      RAM:  ~12 KB for edge weight history + graph state
      CPU:  <1% additional for mincut
      Net:  Receive 15 * 480 B/sweep = 7.2 KB/sweep
      Note: Well within ESP32-S3 capabilities
```

This fully edge-based architecture eliminates the need for any server
infrastructure, suitable for standalone deployments, field use, or
privacy-sensitive environments.

---

## Appendix A: Bill of Materials (16-Node Mesh)

| Item                        | Qty | Unit cost | Total    |
|-----------------------------|-----|-----------|----------|
| ESP32-DevKitC V4            | 16  | $6.00     | $96.00   |
| USB-C cable (1m)            | 16  | $2.00     | $32.00   |
| USB 5V/1A wall adapter      | 16  | $3.00     | $48.00   |
| 3D-printed wall mount       | 16  | $0.50     | $8.00    |
| External antenna (optional) | 16  | $2.00     | $32.00   |
| U.FL to SMA pigtail         | 16  | $1.50     | $24.00   |
| **Total (with antennas)**   |     |           |**$240.00**|
| **Total (PCB antenna only)**|     |           |**$184.00**|

## Appendix B: ESP-IDF CSI Configuration Reference

```c
// CSI configuration for sensing mode
wifi_csi_config_t csi_config = {
    .lltf_en           = true,   // Enable L-LTF (legacy long training field)
    .htltf_en          = true,   // Enable HT-LTF (high throughput)
    .stbc_htltf2_en    = false,  // Disable STBC second HT-LTF
    .ltf_merge_en      = true,   // Merge multiple LTF measurements
    .channel_filter_en = false,  // Disable channel filter (raw CSI)
    .manu_scale        = false,  // Disable manual scaling
    .shift             = false,  // Disable bit shifting
};

// CSI callback registration
esp_wifi_set_csi_config(&csi_config);
esp_wifi_set_csi_rx_cb(&csi_data_callback, NULL);
esp_wifi_set_csi(true);
```

## Appendix C: Key Formulas

**CSI Coherence (edge weight)**:
```
              | sum_k( H_ab(f_k, t) * conj(H_ab(f_k, t_ref)) ) |
gamma_ab = -------------------------------------------------------
            sqrt( sum_k |H_ab(f_k,t)|^2 ) * sqrt( sum_k |H_ref|^2 )

where:
  H_ab(f_k, t)     = CSI from node a to node b at subcarrier k, time t
  H_ab(f_k, t_ref) = Reference CSI (empty room calibration)
  gamma_ab in [0, 1]
  gamma_ab ~ 1.0   = unobstructed path (high coherence)
  gamma_ab ~ 0.3   = person blocking path (low coherence)
```

**Stoer-Wagner Minimum Cut**:
```
Input:  G = (V, E, w)  where |V| = 16, |E| = 120, w: E -> [0,1]
Output: min_cut_value, partition (S, V\S)

Algorithm:
  for phase = 1 to |V|-1:
    (s, t, cut_of_phase) = MinimumCutPhase(G)
    if cut_of_phase < best_cut:
      best_cut = cut_of_phase
      best_partition = current partition
    merge(s, t) in G
```

**Fresnel Zone Radius**:
```
r_F1 = sqrt( lambda * d1 * d2 / (d1 + d2) )

where:
  lambda = c / f    (wavelength)
  d1, d2 = distances from point to TX and RX
  For 2.4 GHz, 5m link: r_F1 = 0.28 m
  For 5 GHz, 5m link:   r_F1 = 0.19 m
```

---

## References

1. ESP-IDF Programming Guide: WiFi CSI (Espressif documentation)
2. Stoer, M. and Wagner, F. "A Simple Min-Cut Algorithm." JACM, 1997
3. ADR-028: ESP32 Capability Audit and Witness Verification
4. ADR-029: RuvSense Multistatic Sensing Mode
5. ADR-031: RuView Sensing-First RF Mode
6. ADR-032: Multistatic Mesh Security Hardening
7. Wilson, J. and Patwari, N. "Radio Tomographic Imaging with Wireless
   Networks." IEEE Trans. Mobile Computing, 2010
8. Wang, W. et al. "Understanding and Modeling of WiFi Signal Based Human
   Activity Recognition." MobiCom, 2015
