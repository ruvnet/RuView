# ADR-008: Distributed Consensus for Multi-AP Coordination

## Status
Proposed

## Date
2026-02-28

## Context

### Multi-AP Sensing Architecture

WiFi-DensePose achieves higher accuracy and coverage with multiple access points (APs) observing the same space from different angles. The disaster detection module (wifi-densepose-mat, ADR-001) explicitly requires distributed deployment:

- **Portable**: Single TX/RX units deployed around a collapse site
- **Distributed**: Multiple APs covering a large disaster zone
- **Drone-mounted**: UAVs scanning from above with coordinated flight paths

Each AP independently captures CSI data, extracts features, and runs local inference. But the distributed system needs coordination:

1. **Consistent survivor registry**: All nodes must agree on the set of detected survivors, their locations, and triage classifications. Conflicting records cause rescue teams to waste time.

2. **Coordinated scanning**: Avoid redundant scans of the same zone. Dynamically reassign APs as zones are cleared.

3. **Model synchronization**: When SONA adapts a model on one node (ADR-005), other nodes should benefit from the adaptation without re-learning.

4. **Clock synchronization**: CSI timestamps must be aligned across nodes for multi-view pose fusion (the GNN multi-person disentanglement in ADR-006 requires temporal alignment).

5. **Partition tolerance**: In disaster scenarios, network connectivity is unreliable. The system must function during partitions and reconcile when connectivity restores.

### Current State

No distributed coordination exists. Each node operates independently. The Rust workspace has no consensus crate.

### RuVector's Distributed Capabilities

RuVector provides:
- **Raft consensus**: Leader election and replicated log for strong consistency
- **Vector clocks**: Logical timestamps for causal ordering without synchronized clocks
- **Multi-master replication**: Concurrent writes with conflict resolution
- **Delta consensus**: Tracks behavioral changes across nodes for anomaly detection
- **Auto-sharding**: Distributes data based on access patterns

## Decision

We will integrate RuVector's Raft consensus implementation as the coordination backbone for multi-AP WiFi-DensePose deployments, with vector clocks for causal ordering and CRDT-based conflict resolution for partition-tolerant operation.

### Consensus Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│              Multi-AP Coordination Architecture                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Normal Operation (Connected):                                      │
│                                                                      │
│  ┌─────────┐     Raft      ┌─────────┐     Raft      ┌─────────┐  │
│  │  AP-1   │◀────────────▶│  AP-2   │◀────────────▶│  AP-3   │  │
│  │ (Leader)│    Replicated  │(Follower│   Replicated  │(Follower│  │
│  │         │       Log      │        )│      Log      │        )│  │
│  └────┬────┘               └────┬────┘               └────┬────┘  │
│       │                         │                         │        │
│       ▼                         ▼                         ▼        │
│  ┌─────────┐              ┌─────────┐              ┌─────────┐    │
│  │ Local   │              │ Local   │              │ Local   │    │
│  │ RVF     │              │ RVF     │              │ RVF     │    │
│  │Container│              │Container│              │Container│    │
│  └─────────┘              └─────────┘              └─────────┘    │
│                                                                      │
│  Partitioned Operation (Disconnected):                              │
│                                                                      │
│  ┌─────────┐                              ┌──────────────────────┐  │
│  │  AP-1   │  ← operates independently →  │  AP-2    AP-3       │  │
│  │         │                              │  (form sub-cluster)  │  │
│  │ Local   │                              │  Raft between 2+3    │  │
│  │ writes  │                              │                      │  │
│  └─────────┘                              └──────────────────────┘  │
│       │                                            │                 │
│       └──────── Reconnect: CRDT merge ─────────────┘                │
└─────────────────────────────────────────────────────────────────────┘
```

### Replicated State Machine

The Raft log replicates these operations across all nodes:

```rust
/// Operations replicated via Raft consensus
#[derive(Serialize, Deserialize, Clone)]
pub enum ConsensusOp {
    /// New survivor detected
    SurvivorDetected {
        survivor_id: Uuid,
        location: GeoCoord,
        triage: TriageLevel,
        detecting_ap: ApId,
        confidence: f64,
        timestamp: VectorClock,
    },

    /// Survivor status updated (e.g., triage reclassification)
    SurvivorUpdated {
        survivor_id: Uuid,
        new_triage: TriageLevel,
        updating_ap: ApId,
        evidence: DetectionEvidence,
    },

    /// Zone assignment changed
    ZoneAssignment {
        zone_id: ZoneId,
        assigned_aps: Vec<ApId>,
        priority: ScanPriority,
    },

    /// Model adaptation delta shared
    ModelDelta {
        source_ap: ApId,
        lora_delta: Vec<u8>,  // Serialized LoRA matrices
        environment_hash: [u8; 32],
        performance_metrics: AdaptationMetrics,
    },

    /// AP joined or left the cluster
    MembershipChange {
        ap_id: ApId,
        action: MembershipAction,  // Join | Leave | Suspect
    },
}
```

### Vector Clocks for Causal Ordering

Since APs may have unsynchronized physical clocks, vector clocks provide causal ordering:

```rust
/// Vector clock for causal ordering across APs
#[derive(Clone, Serialize, Deserialize)]
pub struct VectorClock {
    /// Map from AP ID to logical timestamp
    clocks: HashMap<ApId, u64>,
}

impl VectorClock {
    /// Increment this AP's clock
    pub fn tick(&mut self, ap_id: &ApId) {
        *self.clocks.entry(ap_id.clone()).or_insert(0) += 1;
    }

    /// Merge with another clock (take max of each component)
    pub fn merge(&mut self, other: &VectorClock) {
        for (ap_id, &ts) in &other.clocks {
            let entry = self.clocks.entry(ap_id.clone()).or_insert(0);
            *entry = (*entry).max(ts);
        }
    }

    /// Check if self happened-before other
    pub fn happened_before(&self, other: &VectorClock) -> bool {
        self.clocks.iter().all(|(k, &v)| {
            other.clocks.get(k).map_or(false, |&ov| v <= ov)
        }) && self.clocks != other.clocks
    }
}
```

### CRDT-Based Conflict Resolution

During network partitions, concurrent updates may conflict. We use CRDTs (Conflict-free Replicated Data Types) for automatic resolution:

```rust
/// Survivor registry using Last-Writer-Wins Register CRDT
pub struct SurvivorRegistry {
    /// LWW-Element-Set: each survivor has a timestamp-tagged state
    survivors: HashMap<Uuid, LwwRegister<SurvivorState>>,
}

/// Triage uses Max-wins semantics:
/// If partition A says P1 (Red/Immediate) and partition B says P2 (Yellow/Delayed),
/// after merge the survivor is classified P1 (more urgent wins)
/// Rationale: false negative (missing critical) is worse than false positive
impl CrdtMerge for TriageLevel {
    fn merge(a: Self, b: Self) -> Self {
        // Lower numeric priority = more urgent
        if a.urgency() >= b.urgency() { a } else { b }
    }
}
```

**CRDT merge strategies by data type**:

| Data Type | CRDT Type | Merge Strategy | Rationale |
|-----------|-----------|---------------|-----------|
| Survivor set | OR-Set | Union (never lose a detection) | Missing survivors = fatal |
| Triage level | Max-Register | Most urgent wins | Err toward caution |
| Location | LWW-Register | Latest timestamp wins | Survivors may move |
| Zone assignment | LWW-Map | Leader's assignment wins | Need authoritative coord |
| Model deltas | G-Set | Accumulate all deltas | All adaptations valuable |

### Node Discovery and Health

```rust
/// AP cluster management
pub struct ApCluster {
    /// This node's identity
    local_ap: ApId,

    /// Raft consensus engine
    raft: RaftEngine<ConsensusOp>,

    /// Failure detector (phi-accrual)
    failure_detector: PhiAccrualDetector,

    /// Cluster membership
    members: HashSet<ApId>,
}

impl ApCluster {
    /// Heartbeat interval for failure detection
    const HEARTBEAT_MS: u64 = 500;

    /// Phi threshold for suspecting node failure
    const PHI_THRESHOLD: f64 = 8.0;

    /// Minimum cluster size for Raft (need majority)
    const MIN_CLUSTER_SIZE: usize = 3;
}
```

### Performance Characteristics

| Operation | Latency | Notes |
|-----------|---------|-------|
| Raft heartbeat | 500 ms interval | Configurable |
| Log replication | 1-5 ms (LAN) | Depends on payload size |
| Leader election | 1-3 seconds | After leader failure detected |
| CRDT merge (partition heal) | 10-100 ms | Proportional to divergence |
| Vector clock comparison | <0.01 ms | O(n) where n = cluster size |
| Model delta replication | 50-200 ms | ~70 KB LoRA delta |

### Deployment Configurations

| Scenario | Nodes | Consensus | Partition Strategy |
|----------|-------|-----------|-------------------|
| Single room | 1-2 | None (local only) | N/A |
| Building floor | 3-5 | Raft (3-node quorum) | CRDT merge on heal |
| Disaster site | 5-20 | Raft (5-node quorum) + zones | Zone-level sub-clusters |
| Urban search | 20-100 | Hierarchical Raft | Regional leaders |

## Consequences

### Positive
- **Consistent state**: All APs agree on survivor registry via Raft
- **Partition tolerant**: CRDT merge allows operation during disconnection
- **Causal ordering**: Vector clocks provide logical time without NTP
- **Automatic failover**: Raft leader election handles AP failures
- **Model sharing**: SONA adaptations propagate across cluster

### Negative
- **Minimum 3 nodes**: Raft requires odd-numbered quorum for leader election
- **Network overhead**: Heartbeats and log replication consume bandwidth (~1-10 KB/s per node)
- **Complexity**: Distributed systems are inherently harder to debug
- **Latency for writes**: Raft requires majority acknowledgment before commit (1-5ms LAN)
- **Split-brain risk**: If cluster splits evenly (2+2), neither partition has quorum

### Disaster-Specific Considerations

| Challenge | Mitigation |
|-----------|------------|
| Intermittent connectivity | Aggressive CRDT merge on reconnect; local operation during partition |
| Power failures | Raft log persisted to local SSD; recovery on restart |
| Node destruction | Raft tolerates minority failure; data replicated across survivors |
| Drone mobility | Drone APs treated as ephemeral members; data synced on landing |
| Bandwidth constraints | Delta-only replication; compress LoRA deltas |

## References

- [Raft Consensus Algorithm](https://raft.github.io/raft.pdf)
- [CRDTs: Conflict-free Replicated Data Types](https://hal.inria.fr/inria-00609399)
- [Vector Clocks](https://en.wikipedia.org/wiki/Vector_clock)
- [Phi Accrual Failure Detector](https://www.computer.org/csdl/proceedings-article/srds/2004/22390066/12OmNyQYtlC)
- [RuVector Distributed Consensus](https://github.com/ruvnet/ruvector)
- ADR-001: WiFi-Mat Disaster Detection Architecture
- ADR-002: RuVector RVF Integration Strategy
