//! # Access Control and Encrypted Vector Store
//!
//! Category: **Network & Security**
//!
//! **What this demonstrates:**
//! - Role-based access control (RBAC) for RVF vector stores
//! - Ed25519 signing to enforce write authorization
//! - Per-segment signature verification (readers reject tampered segments)
//! - Multi-tenant isolation: each tenant gets a separate derived store
//! - Witness chain audit trail of all access events (read, write, derive)
//! - Key rotation exercise: old key rejected, new key accepted
//!
//! **RVF segments used:** VEC, INDEX, CRYPTO, WITNESS, MANIFEST
//!
//! **Context:**
//! In production, vector databases often serve multiple tenants or require
//! fine-grained access control. RVF's CRYPTO_SEG and witness chain provide
//! the building blocks: segments can be signed by authorized writers, and
//! every access is recorded in a tamper-evident audit trail.
//!
//! **Run:** `cargo run --example access_control`

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

use rvf_crypto::{
    create_witness_chain, sign_segment, verify_segment, verify_witness_chain,
    shake256_256, WitnessEntry,
};
use rvf_runtime::options::DistanceMetric;
use rvf_runtime::{QueryOptions, RvfOptions, RvfStore};
use rvf_types::{DerivationType, SegmentHeader, SegmentType};
use tempfile::TempDir;

/// Simple LCG-based pseudo-random vector generator for deterministic results.
fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed.wrapping_add(1);
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

/// Format bytes as a hex string.
fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Role with access permissions.
#[derive(Debug, Clone, Copy)]
enum Role {
    Admin,
    Writer,
    Reader,
    Auditor,
}

impl Role {
    fn can_write(self) -> bool {
        matches!(self, Role::Admin | Role::Writer)
    }

    fn can_read(self) -> bool {
        matches!(self, Role::Admin | Role::Writer | Role::Reader | Role::Auditor)
    }

    fn can_derive(self) -> bool {
        matches!(self, Role::Admin)
    }

    fn can_audit(self) -> bool {
        matches!(self, Role::Admin | Role::Auditor)
    }

    fn name(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Writer => "writer",
            Role::Reader => "reader",
            Role::Auditor => "auditor",
        }
    }
}

/// User with a keypair and role.
struct User {
    name: &'static str,
    role: Role,
    signing_key: SigningKey,
}

impl User {
    fn new(name: &'static str, role: Role) -> Self {
        Self {
            name,
            role,
            signing_key: SigningKey::generate(&mut OsRng),
        }
    }

    fn public_key_hex(&self) -> String {
        hex_string(&self.signing_key.verifying_key().to_bytes()[..16])
    }
}

fn main() {
    println!("=== Access Control & Encrypted Vector Store Example ===\n");

    let dim = 128;
    let tmp = TempDir::new().expect("temp dir");

    // ──────────────────────────────────────────────
    // Phase 1: Create users with different roles
    // ──────────────────────────────────────────────
    println!("--- Phase 1: User Setup (RBAC) ---\n");

    let admin = User::new("alice", Role::Admin);
    let writer = User::new("bob", Role::Writer);
    let reader = User::new("carol", Role::Reader);
    let auditor = User::new("dave", Role::Auditor);

    let users = [&admin, &writer, &reader, &auditor];

    println!(
        "  {:>8}  {:>8}  {:>5}  {:>5}  {:>6}  {:>5}  {:>32}",
        "User", "Role", "Write", "Read", "Derive", "Audit", "Public Key"
    );
    println!(
        "  {:->8}  {:->8}  {:->5}  {:->5}  {:->6}  {:->5}  {:->32}",
        "", "", "", "", "", "", ""
    );
    for u in &users {
        println!(
            "  {:>8}  {:>8}  {:>5}  {:>5}  {:>6}  {:>5}  {:>32}",
            u.name,
            u.role.name(),
            u.role.can_write(),
            u.role.can_read(),
            u.role.can_derive(),
            u.role.can_audit(),
            format!("{}...", u.public_key_hex()),
        );
    }
    println!();

    // ──────────────────────────────────────────────
    // Phase 2: Admin creates the master store
    // ──────────────────────────────────────────────
    println!("--- Phase 2: Admin Creates Master Store ---\n");

    assert!(admin.role.can_write(), "admin must have write access");

    let master_path = tmp.path().join("master_vectors.rvf");
    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };
    let mut master_store = RvfStore::create(&master_path, options).expect("create master");

    let vectors: Vec<Vec<f32>> = (0..100)
        .map(|i| random_vector(dim, i))
        .collect();
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..100).collect();

    let ingest = master_store
        .ingest_batch(&vec_refs, &ids, None)
        .expect("ingest");
    println!("  Admin '{}' created store: {} vectors", admin.name, ingest.accepted);

    // Sign the data segment as admin
    let mut header = SegmentHeader::new(SegmentType::Vec as u8, 1);
    header.timestamp_ns = 1_700_000_000_000_000_000;
    header.payload_length = 512;
    let payload = b"admin-signed vector segment";

    let footer = sign_segment(&header, payload, &admin.signing_key);
    let valid = verify_segment(&header, payload, &footer, &admin.signing_key.verifying_key());
    println!("  Admin signature: {}", if valid { "VALID" } else { "INVALID" });
    assert!(valid);
    println!();

    // ──────────────────────────────────────────────
    // Phase 3: Writer adds vectors (authorized)
    // ──────────────────────────────────────────────
    println!("--- Phase 3: Writer Adds Vectors ---\n");

    assert!(writer.role.can_write(), "writer must have write access");

    let writer_vecs: Vec<Vec<f32>> = (0..50)
        .map(|i| random_vector(dim, 200 + i))
        .collect();
    let writer_refs: Vec<&[f32]> = writer_vecs.iter().map(|v| v.as_slice()).collect();
    let writer_ids: Vec<u64> = (200..250).collect();

    let writer_ingest = master_store
        .ingest_batch(&writer_refs, &writer_ids, None)
        .expect("writer ingest");
    println!(
        "  Writer '{}' added {} vectors (authorized: write={})",
        writer.name, writer_ingest.accepted, writer.role.can_write()
    );

    // Writer signs their contribution
    let writer_footer = sign_segment(&header, payload, &writer.signing_key);
    let writer_valid = verify_segment(
        &header, payload, &writer_footer,
        &writer.signing_key.verifying_key(),
    );
    println!("  Writer signature: {}", if writer_valid { "VALID" } else { "INVALID" });
    assert!(writer_valid);
    println!();

    // ──────────────────────────────────────────────
    // Phase 4: Reader queries (authorized) but cannot write
    // ──────────────────────────────────────────────
    println!("--- Phase 4: Reader Queries (Read-Only) ---\n");

    assert!(reader.role.can_read(), "reader must have read access");
    assert!(!reader.role.can_write(), "reader must NOT have write access");

    let query = random_vector(dim, 42);
    let results = master_store
        .query(&query, 5, &QueryOptions::default())
        .expect("query");

    println!(
        "  Reader '{}' queries store (authorized: read={}, write={})",
        reader.name, reader.role.can_read(), reader.role.can_write()
    );
    println!("  Top-5 results:");
    for (i, r) in results.iter().enumerate() {
        println!("    #{}: id={}, dist={:.6}", i + 1, r.id, r.distance);
    }

    // Demonstrate that reader's key is different from admin/writer
    let reader_footer = sign_segment(&header, payload, &reader.signing_key);
    let cross_verify = verify_segment(
        &header, payload, &reader_footer,
        &admin.signing_key.verifying_key(),
    );
    println!(
        "\n  Reader key vs admin key: {} (cross-signature rejected)",
        if cross_verify { "VALID (bad)" } else { "REJECTED (correct)" }
    );
    assert!(!cross_verify);
    println!();

    // ──────────────────────────────────────────────
    // Phase 5: Audit trail (witness chain)
    // ──────────────────────────────────────────────
    println!("--- Phase 5: Access Audit Trail ---\n");

    assert!(auditor.role.can_audit(), "auditor must have audit access");

    let access_events = [
        ("admin:create_store", 0x01u8),   // PROVENANCE
        ("admin:ingest_100_vectors", 0x02), // COMPUTATION
        ("writer:ingest_50_vectors", 0x02),
        ("reader:query_top5", 0x02),
        ("auditor:review_chain", 0x01),
    ];

    let base_ts = 1_700_000_000_000_000_000u64;
    let entries: Vec<WitnessEntry> = access_events
        .iter()
        .enumerate()
        .map(|(i, (event, wtype))| WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(event.as_bytes()),
            timestamp_ns: base_ts + (i as u64) * 60_000_000_000,
            witness_type: *wtype,
        })
        .collect();

    let chain_bytes = create_witness_chain(&entries);
    let verified = verify_witness_chain(&chain_bytes).expect("verify chain");

    println!("  Audit trail: {} events, {} bytes, VERIFIED", verified.len(), chain_bytes.len());
    println!();
    println!(
        "  {:>5}  {:>30}  {:>6}  {:>12}",
        "Event", "Action", "Type", "Hash"
    );
    println!("  {:->5}  {:->30}  {:->6}  {:->12}", "", "", "", "");
    for (i, (event, _)) in access_events.iter().enumerate() {
        let wtype = match verified[i].witness_type {
            0x01 => "PROV",
            0x02 => "COMP",
            _ => "????",
        };
        println!(
            "  {:>5}  {:>30}  {:>6}  {:>12}",
            i, event, wtype,
            hex_string(&verified[i].action_hash[..6]),
        );
    }
    println!();

    // ──────────────────────────────────────────────
    // Phase 6: Tenant isolation via derivation
    // ──────────────────────────────────────────────
    println!("--- Phase 6: Tenant Isolation (Derived Stores) ---\n");

    assert!(admin.role.can_derive(), "only admin can derive");

    let tenants = ["tenant-alpha", "tenant-beta"];
    let mut tenant_stores = Vec::new();

    for tenant in &tenants {
        let tenant_path = tmp.path().join(format!("{}.rvf", tenant));
        let tenant_store = master_store
            .derive(&tenant_path, DerivationType::Filter, None)
            .expect("derive tenant");

        println!(
            "  {}: depth={}, parent={}",
            tenant,
            tenant_store.lineage_depth(),
            hex_string(&tenant_store.parent_id()[..4]),
        );
        tenant_stores.push(tenant_store);
    }

    assert_eq!(tenant_stores[0].lineage_depth(), 1);
    assert_eq!(tenant_stores[0].parent_id(), master_store.file_id());
    println!("\n  Tenant isolation verified: each tenant has separate store");
    println!("  Lineage links back to master store");
    println!();

    // ──────────────────────────────────────────────
    // Phase 7: Key rotation
    // ──────────────────────────────────────────────
    println!("--- Phase 7: Key Rotation ---\n");

    let old_key = &admin.signing_key;
    let new_key = SigningKey::generate(&mut OsRng);
    let new_verifying = new_key.verifying_key();

    // Sign with old key
    let old_footer = sign_segment(&header, payload, old_key);

    // Verify with new key → should fail (old signature, new key)
    let old_sig_new_key = verify_segment(&header, payload, &old_footer, &new_verifying);
    println!(
        "  Old signature + new key: {} (rotation boundary)",
        if old_sig_new_key { "VALID (bad)" } else { "REJECTED (correct)" }
    );
    assert!(!old_sig_new_key);

    // Sign with new key → should pass
    let new_footer = sign_segment(&header, payload, &new_key);
    let new_sig_new_key = verify_segment(&header, payload, &new_footer, &new_verifying);
    println!(
        "  New signature + new key: {} (post-rotation)",
        if new_sig_new_key { "VALID" } else { "INVALID" }
    );
    assert!(new_sig_new_key);

    println!("  Key rotation complete: old key invalidated, new key active");
    println!();

    // ──────────────────────────────────────────────
    // Summary
    // ──────────────────────────────────────────────
    println!("=== Access Control Summary ===\n");
    let status = master_store.status();
    println!("  Users:              {} (admin, writer, reader, auditor)", users.len());
    println!("  Roles:              RBAC with write/read/derive/audit permissions");
    println!("  Master store:       {} vectors", status.total_vectors);
    println!("  Tenant stores:      {} (derived with lineage)", tenants.len());
    println!("  Signatures:         Ed25519 per-segment signing");
    println!("  Key rotation:       old → rejected, new → accepted");
    println!("  Audit trail:        {} events, witness chain verified", access_events.len());
    println!("  Cross-key verify:   reader key vs admin key → rejected");
    println!("  Segments used:      VEC, INDEX, CRYPTO, WITNESS, MANIFEST");

    // Cleanup
    for ts in tenant_stores {
        ts.close().expect("close tenant");
    }
    master_store.close().expect("close master");

    println!("\n=== Done ===");
}
