# ADR-010: Witness Chains for Audit Trail Integrity

## Status
Proposed

## Date
2026-02-28

## Context

### Life-Critical Audit Requirements

The wifi-densepose-mat disaster detection module (ADR-001) makes triage classifications that directly affect rescue priority:

| Triage Level | Action | Consequence of Error |
|-------------|--------|---------------------|
| P1 (Immediate/Red) | Rescue NOW | False negative → survivor dies waiting |
| P2 (Delayed/Yellow) | Rescue within 1 hour | Misclassification → delayed rescue |
| P3 (Minor/Green) | Rescue when resources allow | Over-triage → resource waste |
| P4 (Deceased/Black) | No rescue attempted | False P4 → living person abandoned |

Post-incident investigations, liability proceedings, and operational reviews require:

1. **Non-repudiation**: Prove which device made which detection at which time
2. **Tamper evidence**: Detect if records were altered after the fact
3. **Completeness**: Prove no detections were deleted or hidden
4. **Causal chain**: Reconstruct the sequence of events leading to each triage decision
5. **Cross-device verification**: Corroborate detections across multiple APs

### Current State

Detection results are logged to the database (`wifi-densepose-db`) with standard INSERT operations. Logs can be:
- Silently modified after the fact
- Deleted without trace
- Backdated or reordered
- Lost if the database is corrupted

No cryptographic integrity mechanism exists.

### RuVector Witness Chains

RuVector implements hash-linked audit trails inspired by blockchain but without the consensus overhead:

- **Hash chain**: Each entry includes the SHAKE-256 hash of the previous entry, forming a tamper-evident chain
- **Signatures**: Chain anchors (every Nth entry) are signed with the device's key pair
- **Cross-chain attestation**: Multiple devices can cross-reference each other's chains
- **Compact**: Each chain entry is ~100-200 bytes (hash + metadata + signature reference)

## Decision

We will implement RuVector witness chains as the primary audit mechanism for all detection events, triage decisions, and model adaptation events in the WiFi-DensePose system.

### Witness Chain Structure

```
┌────────────────────────────────────────────────────────────────────┐
│                      Witness Chain                                  │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Entry 0          Entry 1          Entry 2          Entry 3        │
│  (Genesis)                                                          │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐     │
│  │ prev: ∅  │◀───│ prev: H0 │◀───│ prev: H1 │◀───│ prev: H2 │     │
│  │ event:   │    │ event:   │    │ event:   │    │ event:   │     │
│  │  INIT    │    │  DETECT  │    │  TRIAGE  │    │  ADAPT   │     │
│  │ hash: H0 │    │ hash: H1 │    │ hash: H2 │    │ hash: H3 │     │
│  │ sig: S0  │    │          │    │          │    │ sig: S1  │     │
│  │ (anchor) │    │          │    │          │    │ (anchor) │     │
│  └──────────┘    └──────────┘    └──────────┘    └──────────┘     │
│                                                                     │
│  H0 = SHAKE-256(INIT || device_id || timestamp)                    │
│  H1 = SHAKE-256(DETECT_DATA || H0 || timestamp)                   │
│  H2 = SHAKE-256(TRIAGE_DATA || H1 || timestamp)                   │
│  H3 = SHAKE-256(ADAPT_DATA || H2 || timestamp)                    │
│                                                                     │
│  Anchor signature S0 = ML-DSA-65.sign(H0, device_key)             │
│  Anchor signature S1 = ML-DSA-65.sign(H3, device_key)             │
│  Anchor interval: every 100 entries (configurable)                 │
└────────────────────────────────────────────────────────────────────┘
```

### Witnessed Event Types

```rust
/// Events recorded in the witness chain
#[derive(Serialize, Deserialize, Clone)]
pub enum WitnessedEvent {
    /// Chain initialization (genesis)
    ChainInit {
        device_id: DeviceId,
        firmware_version: String,
        config_hash: [u8; 32],
    },

    /// Human presence detected
    HumanDetected {
        detection_id: Uuid,
        confidence: f64,
        csi_features_hash: [u8; 32],  // Hash of input data, not raw data
        location_estimate: Option<GeoCoord>,
        model_version: String,
    },

    /// Triage classification assigned or changed
    TriageDecision {
        survivor_id: Uuid,
        previous_level: Option<TriageLevel>,
        new_level: TriageLevel,
        evidence_hash: [u8; 32],  // Hash of supporting evidence
        deciding_algorithm: String,
        confidence: f64,
    },

    /// False detection corrected
    DetectionCorrected {
        detection_id: Uuid,
        correction_type: CorrectionType,  // FalsePositive | FalseNegative | Reclassified
        reason: String,
        corrected_by: CorrectorId,  // Device or operator
    },

    /// Model adapted via SONA
    ModelAdapted {
        adaptation_id: Uuid,
        trigger: AdaptationTrigger,
        lora_delta_hash: [u8; 32],
        performance_before: f64,
        performance_after: f64,
    },

    /// Zone scan completed
    ZoneScanCompleted {
        zone_id: ZoneId,
        scan_duration_ms: u64,
        detections_count: usize,
        coverage_percentage: f64,
    },

    /// Cross-device attestation received
    CrossAttestation {
        attesting_device: DeviceId,
        attested_chain_hash: [u8; 32],
        attested_entry_index: u64,
    },

    /// Operator action (manual override)
    OperatorAction {
        operator_id: String,
        action: OperatorActionType,
        target: Uuid,  // What was acted upon
        justification: String,
    },
}
```

### Chain Entry Structure

```rust
/// A single entry in the witness chain
#[derive(Serialize, Deserialize)]
pub struct WitnessEntry {
    /// Sequential index in the chain
    index: u64,

    /// SHAKE-256 hash of the previous entry (32 bytes)
    previous_hash: [u8; 32],

    /// The witnessed event
    event: WitnessedEvent,

    /// Device that created this entry
    device_id: DeviceId,

    /// Monotonic timestamp (device-local, not wall clock)
    monotonic_timestamp: u64,

    /// Wall clock timestamp (best-effort, may be inaccurate)
    wall_timestamp: DateTime<Utc>,

    /// Vector clock for causal ordering (see ADR-008)
    vector_clock: VectorClock,

    /// This entry's hash: SHAKE-256(serialize(self without this field))
    entry_hash: [u8; 32],

    /// Anchor signature (present every N entries)
    anchor_signature: Option<HybridSignature>,
}
```

### Tamper Detection

```rust
/// Verify witness chain integrity
pub fn verify_chain(chain: &[WitnessEntry]) -> Result<ChainVerification, AuditError> {
    let mut verification = ChainVerification::new();

    for (i, entry) in chain.iter().enumerate() {
        // 1. Verify hash chain linkage
        if i > 0 {
            let expected_prev_hash = chain[i - 1].entry_hash;
            if entry.previous_hash != expected_prev_hash {
                verification.add_violation(ChainViolation::BrokenLink {
                    entry_index: entry.index,
                    expected_hash: expected_prev_hash,
                    actual_hash: entry.previous_hash,
                });
            }
        }

        // 2. Verify entry self-hash
        let computed_hash = compute_entry_hash(entry);
        if computed_hash != entry.entry_hash {
            verification.add_violation(ChainViolation::TamperedEntry {
                entry_index: entry.index,
            });
        }

        // 3. Verify anchor signatures
        if let Some(ref sig) = entry.anchor_signature {
            let device_keys = load_device_keys(&entry.device_id)?;
            if !sig.verify(&entry.entry_hash, &device_keys.ed25519, &device_keys.ml_dsa)? {
                verification.add_violation(ChainViolation::InvalidSignature {
                    entry_index: entry.index,
                });
            }
        }

        // 4. Verify monotonic timestamp ordering
        if i > 0 && entry.monotonic_timestamp <= chain[i - 1].monotonic_timestamp {
            verification.add_violation(ChainViolation::NonMonotonicTimestamp {
                entry_index: entry.index,
            });
        }

        verification.verified_entries += 1;
    }

    Ok(verification)
}
```

### Cross-Device Attestation

Multiple APs can cross-reference each other's chains for stronger guarantees:

```
Device A's chain:                    Device B's chain:
┌──────────┐                         ┌──────────┐
│ Entry 50 │                         │ Entry 73 │
│ H_A50    │◀────── cross-attest ───▶│ H_B73    │
└──────────┘                         └──────────┘

Device A records: CrossAttestation { attesting: B, hash: H_B73, index: 73 }
Device B records: CrossAttestation { attesting: A, hash: H_A50, index: 50 }

After cross-attestation:
- Neither device can rewrite entries before the attested point
  without the other device's chain becoming inconsistent
- An investigator can verify both chains agree on the attestation point
```

**Attestation frequency**: Every 5 minutes during connected operation, immediately on significant events (P1 triage, zone completion).

### Storage and Retrieval

Witness chains are stored in the RVF container's WITNESS segment:

```rust
/// Witness chain storage manager
pub struct WitnessChainStore {
    /// Current chain being appended to
    active_chain: Vec<WitnessEntry>,

    /// Anchor signature interval
    anchor_interval: usize,  // 100

    /// Device signing key
    device_key: DeviceKeyPair,

    /// Cross-attestation peers
    attestation_peers: Vec<DeviceId>,

    /// RVF container for persistence
    container: RvfContainer,
}

impl WitnessChainStore {
    /// Append an event to the chain
    pub fn witness(&mut self, event: WitnessedEvent) -> Result<u64, AuditError> {
        let index = self.active_chain.len() as u64;
        let previous_hash = self.active_chain.last()
            .map(|e| e.entry_hash)
            .unwrap_or([0u8; 32]);

        let mut entry = WitnessEntry {
            index,
            previous_hash,
            event,
            device_id: self.device_key.device_id(),
            monotonic_timestamp: monotonic_now(),
            wall_timestamp: Utc::now(),
            vector_clock: self.get_current_vclock(),
            entry_hash: [0u8; 32],  // Computed below
            anchor_signature: None,
        };

        // Compute entry hash
        entry.entry_hash = compute_entry_hash(&entry);

        // Add anchor signature at interval
        if index % self.anchor_interval as u64 == 0 {
            entry.anchor_signature = Some(
                self.device_key.sign_hybrid(&entry.entry_hash)?
            );
        }

        self.active_chain.push(entry);

        // Persist to RVF container
        self.container.append_witness(&self.active_chain.last().unwrap())?;

        Ok(index)
    }

    /// Query chain for events in a time range
    pub fn query_range(&self, start: DateTime<Utc>, end: DateTime<Utc>)
        -> Vec<&WitnessEntry>
    {
        self.active_chain.iter()
            .filter(|e| e.wall_timestamp >= start && e.wall_timestamp <= end)
            .collect()
    }

    /// Export chain for external audit
    pub fn export_for_audit(&self) -> AuditBundle {
        AuditBundle {
            chain: self.active_chain.clone(),
            device_public_key: self.device_key.public_keys(),
            cross_attestations: self.collect_cross_attestations(),
            chain_summary: self.compute_summary(),
        }
    }
}
```

### Performance Impact

| Operation | Latency | Notes |
|-----------|---------|-------|
| Append entry | 0.05 ms | Hash computation + serialize |
| Append with anchor signature | 0.5 ms | + ML-DSA-65 sign |
| Verify single entry | 0.02 ms | Hash comparison |
| Verify anchor | 0.3 ms | ML-DSA-65 verify |
| Full chain verify (10K entries) | 50 ms | Sequential hash verification |
| Cross-attestation | 1 ms | Sign + network round-trip |

### Storage Requirements

| Chain Length | Entries/Hour | Size/Hour | Size/Day |
|-------------|-------------|-----------|----------|
| Low activity | ~100 | ~20 KB | ~480 KB |
| Normal operation | ~1,000 | ~200 KB | ~4.8 MB |
| Disaster response | ~10,000 | ~2 MB | ~48 MB |
| High-intensity scan | ~50,000 | ~10 MB | ~240 MB |

## Consequences

### Positive
- **Tamper-evident**: Any modification to historical records is detectable
- **Non-repudiable**: Signed anchors prove device identity
- **Complete history**: Every detection, triage, and correction is recorded
- **Cross-verified**: Multi-device attestation strengthens guarantees
- **Forensically sound**: Exportable audit bundles for legal proceedings
- **Low overhead**: 0.05ms per entry; minimal storage for normal operation

### Negative
- **Append-only growth**: Chains grow monotonically; need archival strategy for long deployments
- **Key management**: Device keys must be provisioned and protected
- **Clock dependency**: Wall-clock timestamps are best-effort; monotonic timestamps are device-local
- **Verification cost**: Full chain verification of long chains takes meaningful time (50ms/10K entries)
- **Privacy tension**: Detailed audit trails contain operational intelligence

### Regulatory Alignment

| Requirement | How Witness Chains Address It |
|------------|------------------------------|
| GDPR (Right to erasure) | Event hashes stored, not personal data; original data deletable while chain proves historical integrity |
| HIPAA (Audit controls) | Complete access/modification log with non-repudiation |
| ISO 27001 (Information security) | Tamper-evident records, access logging, integrity verification |
| NIST SP 800-53 (AU controls) | Audit record generation, protection, and review capability |
| FEMA ICS (Incident Command) | Chain of custody for all operational decisions |

## References

- [Witness Chains in Distributed Systems](https://eprint.iacr.org/2019/747)
- [SHAKE-256 (FIPS 202)](https://csrc.nist.gov/pubs/fips/202/final)
- [Tamper-Evident Logging](https://www.usenix.org/legacy/event/sec09/tech/full_papers/crosby.pdf)
- [RuVector Witness Implementation](https://github.com/ruvnet/ruvector)
- ADR-001: WiFi-Mat Disaster Detection Architecture
- ADR-007: Post-Quantum Cryptography for Secure Sensing
- ADR-008: Distributed Consensus for Multi-AP Coordination
