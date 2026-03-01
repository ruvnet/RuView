# ADR-007: Distributed Genomics Consensus & Variant Database Federation

**Status**: Proposed
**Date**: 2026-02-11
**Authors**: System Architecture Designer
**Deciders**: Architecture Review Board
**Target Crates**: `ruvector-raft`, `ruvector-delta-consensus`, `ruvector-cluster`, `ruvector-replication`, `ruvector-delta-core`

---

## Context

Global genomic databases (ClinVar, gnomAD, GISAID) operate as centralized repositories with batch update cycles. This architecture fails during pandemics (GISAID delays: 2-14 days) and prevents real-time clinical decision-making (stale pharmacogenomic data could cause adverse drug reactions).

**Key challenges**:

1. **Clinical safety**: Patient genomic records require strong consistency (no stale reads)
2. **Surveillance speed**: Pathogen tracking demands sub-5-second global dissemination
3. **Data sovereignty**: GDPR/HIPAA prohibit cross-border replication of identified patient data

**State-of-the-art genomic federation**:

| System | Architecture | Consistency | Latency | Limitation |
|--------|-------------|-------------|---------|-----------|
| ClinVar | Centralized (NCBI) | Strong | Weekly batch | No real-time updates |
| gnomAD | Centralized (Broad) | Strong | Quarterly releases | Aggregates only, no raw data |
| GISAID | Centralized + mirrors | Eventual | 2-14 days | Manual curation bottleneck |
| GA4GH Beacon | Federated query | Eventual | Seconds | No write consensus |
| Nextstrain | GitHub-based | Eventual | Hours | Not a database, visualization only |

**RuVector advantage**: Existing distributed consensus infrastructure enables practical variant federation with tunable consistency.

---

## Decision

### Implement a Three-Tier Distributed Variant Database with Raft Consensus

We will build a `DistributedVariantDB` that:

1. Uses **Raft consensus** (`ruvector-raft`) for canonical variant catalog with strong consistency
2. Uses **delta encoding** (`ruvector-delta-core`) for incremental variant updates (1000x compression)
3. Uses **geographic sharding** (`ruvector-cluster`) for data sovereignty compliance
4. Provides **hot-standby failover** (`ruvector-replication`) for clinical uptime (< 5s RTO)

**What works today**: Raft consensus, delta compression, cluster management
**What needs building**: Variant-specific conflict resolution, GDPR-compliant replication filters

---

## Architecture

### 1. Variant Consensus Layer (Raft, Strong Consistency)

**Goal**: Canonical variant database where all institutions agree on variant coordinates and identifiers.

**CAP tradeoff**: Consistency + Partition Tolerance (CP). During network partitions, reject writes rather than risk divergent catalogs.

```rust
use ruvector_raft::{RaftNode, RaftNodeConfig, LogEntry};

pub struct VariantCatalog {
    raft: RaftNode,
    variants: HashMap<String, Variant>,  // variant_id -> Variant
}

pub struct Variant {
    pub id: String,           // e.g., "rs429358" or "chr19:44908684:C>T"
    pub chromosome: String,   // "chr19"
    pub position: u64,        // 44908684
    pub ref_allele: String,   // "C"
    pub alt_allele: String,   // "T"
    pub gene: Option<String>, // "APOE"
    pub consequence: String,  // "missense_variant"
}

impl VariantCatalog {
    pub fn new(cluster_members: Vec<String>) -> Self {
        let config = RaftNodeConfig {
            cluster_members,
            election_timeout_min: 500,    // WAN-tolerant
            election_timeout_max: 2000,
            heartbeat_interval: 200,
            max_entries_per_message: 500,
        };

        let raft = RaftNode::new("variant-catalog-node".into(), config);

        Self { raft, variants: HashMap::new() }
    }

    /// Register a new variant (linearizable write)
    pub async fn register_variant(&mut self, variant: Variant) -> Result<()> {
        let command = serde_json::to_vec(&VariantCommand::Register(variant.clone()))?;

        // Submit to Raft log (blocks until quorum commit)
        self.raft.submit_command(command).await?;

        Ok(())
    }

    /// Lookup variant by ID (linearizable read)
    pub async fn get_variant(&self, id: &str) -> Result<Option<Variant>> {
        // Read-index protocol: ensure we're reading from committed state
        self.raft.read_index().await?;

        Ok(self.variants.get(id).cloned())
    }

    /// Apply committed Raft log entry to state machine
    fn apply_entry(&mut self, entry: &LogEntry) {
        let command: VariantCommand = serde_json::from_slice(&entry.data).unwrap();

        match command {
            VariantCommand::Register(variant) => {
                self.variants.insert(variant.id.clone(), variant);
            }
            VariantCommand::Update(id, updates) => {
                if let Some(v) = self.variants.get_mut(&id) {
                    // Apply updates (e.g., liftover to new assembly)
                    if let Some(new_pos) = updates.position {
                        v.position = new_pos;
                    }
                }
            }
            VariantCommand::Deprecate(id, reason) => {
                self.variants.remove(&id);
                // Log deprecation for audit trail
            }
        }
    }
}

enum VariantCommand {
    Register(Variant),
    Update(String, VariantUpdates),
    Deprecate(String, String),
}

struct VariantUpdates {
    position: Option<u64>,
    gene: Option<String>,
}
```

**Consistency guarantees**:
- Variant registration: Linearizable (quorum commit)
- Variant lookup: Linearizable via read-index protocol
- Quorum: 3/5 nodes (tolerates 2 failures)
- Write latency: 150-400 ms (intercontinental RTT)

### 2. Delta Encoding for Variant Updates

**Problem**: A patient genome has ~4-5 million variants. Transmitting full genomes for every update saturates networks.

**Solution**: Use `ruvector-delta-core` to propagate only changed variant calls.

```rust
use ruvector_delta_core::{VectorDelta, DeltaStore};

pub struct PatientGenome {
    patient_id: String,
    variant_vector: Vec<f32>,  // 5M dimensions: 0.0 (ref), 0.5 (het), 1.0 (hom alt)
}

impl PatientGenome {
    /// Compute delta when re-analyzing with updated pipeline
    pub fn compute_delta(&self, new_calls: &[f32]) -> VectorDelta {
        VectorDelta::compute(&self.variant_vector, new_calls)
    }

    /// Apply delta from replication stream
    pub fn apply_delta(&mut self, delta: &VectorDelta) {
        delta.apply(&mut self.variant_vector);
    }
}

// Example: Pipeline update changes 500 variants out of 5 million
fn example_delta_replication() {
    let old_genome = PatientGenome {
        patient_id: "P123456".into(),
        variant_vector: vec![0.0; 5_000_000],  // Mostly reference
    };

    let mut new_calls = old_genome.variant_vector.clone();
    new_calls[123456] = 0.5;  // New het call discovered
    new_calls[234567] = 1.0;  // Revised to hom alt
    // ... 498 more changes

    let delta = old_genome.compute_delta(&new_calls);

    println!("Full genome size: {} bytes", 5_000_000 * 4);  // 19 MB
    println!("Delta size: {} bytes", delta.size_bytes());   // ~4 KB
    println!("Compression ratio: {}x", 19_000_000 / delta.size_bytes());
}
```

**Compression results**:
```
Typical variant call update (re-analysis with new pipeline):
  Changed positions: 500-5000 out of 5M
  Full genome: 19 MB (5M × 4 bytes)
  Delta: 4-40 KB
  Compression: 475x - 4750x
```

### 3. Geographic Sharding for Data Sovereignty

**Goal**: Patient data never leaves its jurisdiction (GDPR Article 44-49, HIPAA).

```rust
use ruvector_cluster::{ClusterManager, ConsistentHashRing, ShardStrategy};

pub struct GeographicVariantCluster {
    cluster: ClusterManager,
    jurisdictions: HashMap<String, Vec<String>>,  // jurisdiction -> node IDs
}

impl GeographicVariantCluster {
    pub fn new() -> Self {
        let cluster = ClusterManager::new(ClusterConfig {
            replication_factor: 3,
            shard_count: 256,
            heartbeat_interval: Duration::from_secs(5),
            enable_consensus: true,
            min_quorum_size: 2,
        });

        // Pin shards to jurisdictions
        let mut jurisdictions = HashMap::new();
        jurisdictions.insert("EU".into(), vec!["node-eu-1", "node-eu-2", "node-eu-3"]);
        jurisdictions.insert("US".into(), vec!["node-us-1", "node-us-2", "node-us-3"]);
        jurisdictions.insert("JP".into(), vec!["node-jp-1", "node-jp-2", "node-jp-3"]);

        Self { cluster, jurisdictions }
    }

    /// Route patient data to jurisdiction-local shard
    pub fn get_shard_for_patient(&self, patient_id: &str, jurisdiction: &str) -> Result<Vec<String>> {
        let local_nodes = self.jurisdictions.get(jurisdiction)
            .ok_or_else(|| anyhow!("Unknown jurisdiction: {}", jurisdiction))?;

        // Hash patient ID to select consistent shard within jurisdiction
        let shard_id = self.cluster.hash_ring.get_shard(patient_id.as_bytes());
        let nodes = self.cluster.get_shard_nodes(shard_id)?;

        // Filter to jurisdiction-local nodes only
        Ok(nodes.into_iter()
            .filter(|n| local_nodes.contains(n))
            .collect())
    }
}

// Example: GDPR-compliant patient routing
fn example_jurisdiction_routing() {
    let cluster = GeographicVariantCluster::new();

    let eu_patient = "EU-P123456";
    let us_patient = "US-P789012";

    let eu_shards = cluster.get_shard_for_patient(eu_patient, "EU").unwrap();
    let us_shards = cluster.get_shard_for_patient(us_patient, "US").unwrap();

    assert!(eu_shards.iter().all(|n| n.starts_with("node-eu")));
    assert!(us_shards.iter().all(|n| n.starts_with("node-us")));

    // Patient data NEVER crosses jurisdictions
}
```

### 4. Hot-Standby Failover for Clinical Uptime

**Goal**: < 5 second recovery time for patient genomic queries.

```rust
use ruvector_replication::{SyncManager, FailoverManager, SyncMode};

pub struct ClinicalGenomicDB {
    raft: RaftNode,
    sync_manager: SyncManager,
    failover: FailoverManager,
}

impl ClinicalGenomicDB {
    pub fn new() -> Self {
        let raft = RaftNode::new("clinical-primary".into(), RaftNodeConfig {
            cluster_members: vec![
                "clinical-primary".into(),
                "clinical-hot-standby".into(),
                "clinical-dr-site".into(),
            ],
            election_timeout_min: 150,  // LAN-local
            election_timeout_max: 300,
            heartbeat_interval: 50,
            max_entries_per_message: 100,
        });

        let sync_manager = SyncManager::new(SyncMode::Sync {
            replicas: vec!["clinical-hot-standby".into(), "clinical-dr-site".into()],
            sync_timeout: Duration::from_secs(2),
        });

        let failover = FailoverManager::new(FailoverConfig {
            auto_failover: true,
            health_check_interval: Duration::from_secs(2),
            health_check_timeout: Duration::from_millis(500),
            failure_threshold: 2,  // Promote after 2 failed checks
            min_quorum: 2,
            prevent_split_brain: true,
        });

        Self { raft, sync_manager, failover }
    }

    /// Write patient genome (synchronous replication to all nodes)
    pub async fn store_patient_genome(&mut self, patient_id: &str, genome: PatientGenome) -> Result<()> {
        let command = serde_json::to_vec(&GenomeCommand::Store(patient_id.into(), genome))?;

        // Raft commit (quorum)
        self.raft.submit_command(command.clone()).await?;

        // Synchronous replication (wait for ALL replicas)
        self.sync_manager.replicate(command).await?;

        Ok(())
    }
}

// Failover scenario
async fn example_failover() {
    let mut db = ClinicalGenomicDB::new();

    // Primary fails
    simulate_node_failure("clinical-primary");

    // FailoverManager detects failure after 4 seconds (2 checks × 2s)
    tokio::time::sleep(Duration::from_secs(4)).await;

    // Hot standby promoted
    let new_primary = db.failover.get_current_primary();
    assert_eq!(new_primary, "clinical-hot-standby");

    // RTO: < 5 seconds
    // RPO: 0 (synchronous replication)
}
```

**Failover timeline**:
```
T+0s:   Primary health check fails
T+2s:   Second consecutive failure
T+2.5s: Quorum check (hot-standby + DR healthy)
T+3s:   Promote hot-standby to primary
T+4s:   New primary serving reads and writes
RTO:    4 seconds
RPO:    0 (no data loss)
```

---

## Practical Variant Federation Example

**Use case**: Multi-institution pharmacogenomic database for warfarin dosing.

```rust
pub struct PharmacoGenomicFederation {
    variant_catalog: VariantCatalog,  // Raft consensus
    institution_clusters: HashMap<String, GeographicVariantCluster>,
}

impl PharmacoGenomicFederation {
    /// Register a clinically significant pharmacogenomic variant
    pub async fn register_pgx_variant(&mut self, variant: Variant) -> Result<()> {
        // Submit to global Raft consensus
        self.variant_catalog.register_variant(variant.clone()).await?;

        // Replicate to all institutions (selective, only PGx variants)
        for (institution, cluster) in &self.institution_clusters {
            if self.is_pgx_relevant(institution, &variant) {
                cluster.replicate_variant(&variant).await?;
            }
        }

        Ok(())
    }

    /// Query patient's CYP2C9 genotype for warfarin dosing
    pub async fn get_cyp2c9_genotype(&self, patient_id: &str, jurisdiction: &str) -> Result<Genotype> {
        let cluster = self.institution_clusters.get(jurisdiction)
            .ok_or_else(|| anyhow!("Unknown jurisdiction"))?;

        let shards = cluster.get_shard_for_patient(patient_id, jurisdiction)?;
        let genome = self.fetch_patient_genome(patient_id, &shards).await?;

        // Extract CYP2C9 *2 and *3 alleles
        let cyp2c9_star2 = genome.get_variant("rs1799853")?;  // 430C>T
        let cyp2c9_star3 = genome.get_variant("rs1057910")?;  // 1075A>C

        Ok(Genotype {
            star2: cyp2c9_star2,
            star3: cyp2c9_star3,
            metabolizer_status: self.classify_metabolizer(&cyp2c9_star2, &cyp2c9_star3),
        })
    }
}
```

---

## Implementation Status

### ✅ What Works Today

- **Raft consensus**: `ruvector-raft::RaftNode` provides leader election, log replication
- **Delta compression**: `ruvector-delta-core::VectorDelta` computes sparse diffs
- **Cluster management**: `ruvector-cluster::ClusterManager` with consistent hashing
- **Synchronous replication**: `ruvector-replication::SyncManager` with timeout
- **Failover**: `ruvector-replication::FailoverManager` with split-brain prevention

### 🚧 What Needs Building

- **Variant-specific conflict resolution**: When two institutions register the same variant with different IDs, need merge logic

- **GDPR replication filters**: Enforce jurisdiction boundaries in `ReplicationStream`

- **Audit trail**: Tamper-evident log for patient data access (HIPAA requirement)

- **Cross-jurisdiction aggregates**: Anonymous variant frequency sharing without raw data

---

## Performance Targets

| Metric | Target | Mechanism |
|--------|--------|-----------|
| Variant registration (global) | < 500 ms | Raft quorum commit (5 nodes, WAN) |
| Variant lookup (regional) | < 10 ms | Leader read-index (same continent) |
| Patient genome write (clinical) | < 50 ms | Sync replication (3 nodes, LAN) |
| Clinical failover | < 5 seconds | FailoverManager auto-promotion |
| Delta encoding | < 50 ms | Sparse diff over 5M variants |
| Storage compression | 100-1000x | Delta encoding + sparse format |

---

## SOTA Comparison

| System | Consistency | Write Latency | Failover | Data Sovereignty |
|--------|------------|--------------|----------|-----------------|
| ClinVar | Strong | Days (batch) | N/A (centralized) | ❌ |
| gnomAD | Strong | Months (quarterly) | N/A (centralized) | ❌ |
| GISAID | Eventual | 2-14 days | N/A (centralized) | ❌ |
| GA4GH Beacon | Eventual | Seconds | ❌ | ✅ (federated) |
| **RuVector** | Strong (Raft) | 500 ms | < 5s | ✅ (shard pinning) |

**RuVector advantage**: Only system combining strong consistency, sub-second writes, automatic failover, and data sovereignty.

---

## Consequences

### Positive

- **Clinical safety**: Strong consistency prevents stale pharmacogenomic reads
- **Storage efficiency**: Delta encoding achieves 100-1000x compression
- **Data sovereignty**: Jurisdiction-pinned shards comply with GDPR/HIPAA
- **High availability**: Hot-standby failover provides < 5s RTO

### Negative

- **WAN latency**: Raft quorum across continents adds 150-400 ms write latency
- **Complexity**: Three-tier architecture (Raft + delta + sharding) increases operational overhead
- **Limited to structured variants**: VCF-like data only, not raw sequencing reads

### Risks

- **Intercontinental partition**: If continent loses quorum, writes rejected (availability sacrifice)
- **Shard rebalancing**: Adding/removing nodes requires careful migration to maintain jurisdiction boundaries
- **Delta composition errors**: Long chains of deltas may accumulate floating-point errors

---

## References

1. Ongaro, D., Ousterhout, J. (2014). "In Search of an Understandable Consensus Algorithm (Raft)." *USENIX ATC*.

2. Rehm, H.L., et al. (2015). "ClinGen — The Clinical Genome Resource." *New England Journal of Medicine*, 372, 2235-2242.

3. Karczewski, K.J., et al. (2020). "The mutational constraint spectrum quantified from variation in 141,456 humans." *Nature*, 581, 434-443. (gnomAD)

4. Shu, Y., McCauley, J. (2017). "GISAID: Global initiative on sharing all influenza data." *Euro Surveillance*, 22(13).

5. Fiume, M., et al. (2019). "Federated discovery and sharing of genomic data using Beacons." *Nature Biotechnology*, 37, 220-224. (GA4GH Beacon)

---

## Related ADRs

- **ADR-001**: RuVector Core Architecture (HNSW index for variant similarity)
- **ADR-003**: Genomic Vector Index (variant embeddings)
- **ADR-005**: Protein Graph Engine (variant→protein effect prediction)
