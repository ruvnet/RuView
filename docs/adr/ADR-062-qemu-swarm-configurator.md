# ADR-062: QEMU ESP32-S3 Swarm Configurator

| Field       | Value                                          |
|-------------|------------------------------------------------|
| **Status**  | Accepted                                       |
| **Date**    | 2026-03-14                                     |
| **Authors** | RuView Team                                    |
| **Relates** | ADR-061 (QEMU testing platform), ADR-060 (channel/MAC filter), ADR-018 (binary frame), ADR-039 (edge intel) |

## Glossary

| Term | Definition |
|------|-----------|
| Swarm | A group of N QEMU ESP32-S3 instances running simultaneously |
| Topology | How nodes are connected: star, mesh, line, ring |
| Role | Node function: `sensor` (collects CSI), `coordinator` (aggregates + forwards), `gateway` (bridges to host) |
| Scenario matrix | Cross-product of topology × node count × NVS config × mock scenario |
| Health oracle | Python process that monitors all node UART logs and declares swarm health |

## Context

ADR-061 Layer 3 provides a basic multi-node mesh test: N identical nodes with sequential TDM slots connected via a Linux bridge. This is useful but limited:

1. **All nodes are identical** - real deployments have heterogeneous roles (sensor, coordinator, gateway)
2. **Single topology** - only fully-connected bridge; no star, line, or ring topologies
3. **No scenario variation per node** - all nodes run the same mock CSI scenario
4. **Manual configuration** - each test requires hand-editing env vars and arguments
5. **No swarm-level health monitoring** - validation checks individual nodes, not collective behavior
6. **No cross-node timing validation** - TDM slot ordering and inter-frame gaps aren't verified

Real WiFi-DensePose deployments use 3-8 ESP32-S3 nodes in various topologies. A single coordinator aggregates CSI from multiple sensors. The firmware must handle TDM conflicts, missing nodes, role-based behavior differences, and network partitions - none of which ADR-061 Layer 3 tests.

## Decision

Build a **QEMU Swarm Configurator** - a YAML-driven tool that defines multi-node test scenarios declaratively and orchestrates them under QEMU with swarm-level validation.

### Architecture

```
┌─────────────────────────────────────────────────────┐
│                 swarm_config.yaml                     │
│  nodes: [{role: sensor, scenario: 2, channel: 6}]   │
│  topology: star                                       │
│  duration: 60s                                        │
│  assertions: [all_nodes_boot, tdm_no_collision, ...]  │
└──────────────────────┬──────────────────────────────┘
                       │
          ┌────────────▼────────────┐
          │   qemu_swarm.py         │
          │   (orchestrator)        │
          └───┬────┬────┬───┬──────┘
              │    │    │   │
         ┌────▼┐ ┌▼──┐ ▼  ┌▼────┐
         │Node0│ │N1 │... │N(n-1)│   QEMU instances
         │sens │ │sen│    │coord │
         └──┬──┘ └─┬─┘    └──┬───┘
            │      │         │
         ┌──▼──────▼─────────▼──┐
         │  Virtual Network      │    TAP bridge / SLIRP
         │  (topology-shaped)    │
         └──────────┬───────────┘
                    │
         ┌──────────▼───────────┐
         │  Aggregator (Rust)    │    Collects frames
         └──────────┬───────────┘
                    │
         ┌──────────▼───────────┐
         │  Health Oracle        │    Swarm-level assertions
         │  (swarm_health.py)    │
         └──────────────────────┘
```

### YAML Configuration Schema

```yaml
# swarm_config.yaml
swarm:
  name: "3-sensor-star"
  duration_s: 60
  topology: star          # star | mesh | line | ring
  aggregator_port: 5005

nodes:
  - role: coordinator
    node_id: 0
    scenario: 0           # empty room (baseline)
    channel: 6
    edge_tier: 2
    is_gateway: true       # receives aggregated frames

  - role: sensor
    node_id: 1
    scenario: 2           # walking person
    channel: 6
    tdm_slot: 1           # TDM slot index (auto-assigned from node position if omitted)

  - role: sensor
    node_id: 2
    scenario: 3           # fall event
    channel: 6
    tdm_slot: 2

assertions:
  - all_nodes_boot
  - no_crashes
  - tdm_no_collision
  - all_nodes_produce_frames
  - coordinator_receives_from_all
  - fall_detected_by_node_2
  - frame_rate_above: 15    # Hz minimum per node
  - max_boot_time_s: 10
```

### Topologies

| Topology | Network | Description |
|----------|---------|-------------|
| `star` | All sensors connect to coordinator; coordinator has TAP to each sensor | Hub-and-spoke, most common |
| `mesh` | All nodes on same bridge (existing Layer 3 behavior) | Every node sees every other |
| `line` | Node 0 ↔ Node 1 ↔ Node 2 ↔ ... | Linear chain, tests multi-hop |
| `ring` | Like line but last connects to first | Circular, tests routing |

### Node Roles

| Role | Behavior | NVS Keys |
|------|----------|----------|
| `sensor` | Runs mock CSI, sends frames to coordinator | `node_id`, `tdm_slot`, `target_ip` |
| `coordinator` | Receives frames from sensors, runs edge aggregation | `node_id`, `tdm_slot=0`, `edge_tier=2` |
| `gateway` | Like coordinator but also bridges to host UDP | `node_id`, `target_ip=host`, `is_gateway=1` |

### Assertions (Swarm-Level)

| Assertion | What It Checks |
|-----------|---------------|
| `all_nodes_boot` | Every node's UART log shows boot indicators within timeout |
| `no_crashes` | No Guru Meditation, assert, panic in any log |
| `tdm_no_collision` | No two nodes transmit in the same TDM slot |
| `all_nodes_produce_frames` | Every sensor node's log contains CSI frame output |
| `coordinator_receives_from_all` | Coordinator log shows frames from each sensor's node_id |
| `fall_detected_by_node_N` | Node N's log reports a fall detection event |
| `frame_rate_above` | Each node produces at least N frames/second |
| `max_boot_time_s` | All nodes boot within N seconds |
| `no_heap_errors` | No OOM or heap corruption in any log |
| `network_partitioned_recovery` | After deliberate partition, nodes resume communication (future) |

### Preset Configurations

| Preset | Nodes | Topology | Purpose |
|--------|-------|----------|---------|
| `smoke` | 2 | star | Quick CI smoke test (15s) |
| `standard` | 3 | star | Default 3-node (sensor + sensor + coordinator) |
| `large-mesh` | 6 | mesh | Scale test with 6 fully-connected nodes |
| `line-relay` | 4 | line | Multi-hop relay chain |
| `ring-fault` | 4 | ring | Ring with fault injection mid-test |
| `heterogeneous` | 5 | star | Mixed scenarios: walk, fall, static, channel-sweep, empty |
| `ci-matrix` | 3 | star | CI-optimized preset (30s, minimal assertions) |

## File Layout

```
scripts/
├── qemu_swarm.py              # Main orchestrator (CLI entry point)
├── swarm_health.py            # Swarm-level health oracle
└── swarm_presets/
    ├── smoke.yaml
    ├── standard.yaml
    ├── large_mesh.yaml
    ├── line_relay.yaml
    ├── ring_fault.yaml
    ├── heterogeneous.yaml
    └── ci_matrix.yaml

.github/workflows/
└── firmware-qemu.yml          # MODIFIED: add swarm test job
```

## Consequences

### Benefits

1. **Declarative testing** - define swarm topology in YAML, not shell scripts
2. **Role-based nodes** - test coordinator/sensor/gateway interactions
3. **Topology variety** - star/mesh/line/ring match real deployment patterns
4. **Swarm-level assertions** - validate collective behavior, not just individual nodes
5. **Preset library** - quick CI smoke tests and thorough manual validation
6. **Reproducible** - YAML configs are version-controlled and shareable

### Limitations

1. **Still requires root** for TAP bridge topologies (star, line, ring); mesh can use SLIRP
2. **QEMU resource usage** - 6+ QEMU instances use ~2GB RAM, may slow CI runners
3. **No real RF** - inter-node communication is IP-based, not WiFi CSI multipath

## References

- ADR-061: QEMU ESP32-S3 firmware testing platform (Layers 1-9)
- ADR-060: Channel override and MAC address filter provisioning
- ADR-018: Binary CSI frame format (magic `0xC5110001`)
- ADR-039: Edge intelligence pipeline (biquad, vitals, fall detection)
