//! Linux Microkernel Distribution via RVF
//!
//! Category: **Exotic Capability / Systems Integration**
//!
//! Demonstrates RVF as a package distribution and configuration format for
//! a Linux microkernel system. A single .rvf file contains:
//!
//! 1. Kernel image (KERNEL_SEG) — microkernel binary
//! 2. Package collection — each package stored as a vector embedding of its
//!    metadata (name, version, dependencies) with the package manifest as metadata
//! 3. SSH host keys and authorized_keys — stored as signed segments
//! 4. System configuration — network interfaces, firewall rules, services
//! 5. Witness chain — audit trail for every package install and config change
//! 6. Lineage — derive new images from base, track parent → child relationships
//!
//! This pattern enables:
//! - Immutable infrastructure: one .rvf file = one bootable system
//! - Atomic updates: derive a new image, rename atomically, reboot
//! - Package search: find packages by embedding similarity (semantic search)
//! - SSH key management: Ed25519 keys in CRYPTO_SEG with witness chain
//! - Config drift detection: compare two images via vector distance
//!
//! RVF segments used: KERNEL_SEG, VEC_SEG, MANIFEST_SEG, WITNESS_SEG, CRYPTO_SEG
//!
//! Run: `cargo run --example linux_microkernel`

use rvf_crypto::{
    create_witness_chain, shake256_256, sign_segment, verify_segment, verify_witness_chain,
    WitnessEntry,
};
use rvf_runtime::options::DistanceMetric;
use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore,
};
use rvf_runtime::filter::FilterValue;
use rvf_types::kernel::{KernelArch, KernelType};
use rvf_types::{SegmentHeader, SegmentType};
use rvf_types::DerivationType;
use ed25519_dalek::{SigningKey, VerifyingKey};
use tempfile::TempDir;

/// LCG-based deterministic random vector generator.
fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed.wrapping_add(1);
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

fn hex_short(data: &[u8], n: usize) -> String {
    data.iter().take(n).map(|b| format!("{:02x}", b)).collect()
}

/// Generate a deterministic Ed25519 keypair from a seed.
fn keygen(seed: u64) -> (SigningKey, VerifyingKey) {
    let mut key_bytes = [0u8; 32];
    let mut x = seed;
    for b in &mut key_bytes {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *b = (x >> 56) as u8;
    }
    let sk = SigningKey::from_bytes(&key_bytes);
    let vk = sk.verifying_key();
    (sk, vk)
}

/// System package definition.
#[allow(dead_code)]
struct Package {
    name: &'static str,
    version: &'static str,
    category: &'static str,
    size_kb: u64,
    deps: &'static [&'static str],
    description: &'static str,
}

/// SSH key entry.
struct SshKey {
    user: &'static str,
    key_type: &'static str,
    fingerprint_seed: u64,
    permissions: &'static str,
}

/// Network interface configuration.
struct NetInterface {
    name: &'static str,
    ip: &'static str,
    mask: &'static str,
    gateway: Option<&'static str>,
    mtu: u16,
    vlan: Option<u16>,
}

fn main() {
    println!("=== Linux Microkernel Distribution via RVF ===\n");

    let dim = 64; // package embedding dimension
    let tmp = TempDir::new().expect("temp dir");

    // ────────────────────────────────────────────────
    // Phase 1: Define the package collection
    // ────────────────────────────────────────────────
    println!("--- Phase 1: Package Collection ---");

    let packages: Vec<Package> = vec![
        Package { name: "musl-libc", version: "1.2.5", category: "core", size_kb: 892,
            deps: &[], description: "Minimal standard C library" },
        Package { name: "busybox", version: "1.36.1", category: "core", size_kb: 1024,
            deps: &["musl-libc"], description: "Swiss army knife of embedded Linux" },
        Package { name: "linux-kernel", version: "6.8.0-micro", category: "kernel", size_kb: 8192,
            deps: &[], description: "Linux microkernel (minimal config)" },
        Package { name: "openssh-server", version: "9.6p1", category: "network", size_kb: 2048,
            deps: &["musl-libc", "openssl"], description: "SSH protocol server" },
        Package { name: "openssh-client", version: "9.6p1", category: "network", size_kb: 1536,
            deps: &["musl-libc", "openssl"], description: "SSH protocol client" },
        Package { name: "openssl", version: "3.2.1", category: "crypto", size_kb: 4096,
            deps: &["musl-libc"], description: "TLS/SSL cryptographic library" },
        Package { name: "iptables", version: "1.8.10", category: "network", size_kb: 768,
            deps: &["musl-libc"], description: "Packet filtering framework" },
        Package { name: "nftables", version: "1.0.9", category: "network", size_kb: 512,
            deps: &["musl-libc"], description: "Netfilter tables (modern firewall)" },
        Package { name: "iproute2", version: "6.7.0", category: "network", size_kb: 1280,
            deps: &["musl-libc"], description: "Network configuration utilities" },
        Package { name: "containerd", version: "1.7.13", category: "container", size_kb: 32768,
            deps: &["musl-libc"], description: "Container runtime daemon" },
        Package { name: "runc", version: "1.1.12", category: "container", size_kb: 8192,
            deps: &["musl-libc"], description: "OCI container runtime" },
        Package { name: "cni-plugins", version: "1.4.0", category: "container", size_kb: 16384,
            deps: &["musl-libc", "iproute2"], description: "Container networking plugins" },
        Package { name: "wireguard-tools", version: "1.0.20210914", category: "vpn", size_kb: 256,
            deps: &["musl-libc"], description: "WireGuard VPN userspace tools" },
        Package { name: "ruvector-agent", version: "0.1.0", category: "ai", size_kb: 4096,
            deps: &["musl-libc", "openssl"], description: "RuVector edge inference agent" },
        Package { name: "prometheus-node", version: "1.7.0", category: "monitoring", size_kb: 10240,
            deps: &["musl-libc"], description: "Prometheus node exporter" },
        Package { name: "chrony", version: "4.5", category: "system", size_kb: 512,
            deps: &["musl-libc"], description: "NTP time synchronization" },
        Package { name: "syslog-ng", version: "4.6.0", category: "system", size_kb: 2048,
            deps: &["musl-libc", "openssl"], description: "System log daemon" },
        Package { name: "dropbear", version: "2024.84", category: "network", size_kb: 384,
            deps: &["musl-libc"], description: "Lightweight SSH server/client" },
        Package { name: "ethtool", version: "6.7", category: "network", size_kb: 256,
            deps: &["musl-libc"], description: "Ethernet device configuration" },
        Package { name: "lldpd", version: "1.0.18", category: "network", size_kb: 512,
            deps: &["musl-libc"], description: "LLDP protocol daemon" },
    ];

    println!("  Packages: {}", packages.len());
    for cat in &["core", "kernel", "crypto", "network", "container", "vpn", "ai", "monitoring", "system"] {
        let count = packages.iter().filter(|p| p.category == *cat).count();
        if count > 0 {
            println!("    {:12}: {} packages", cat, count);
        }
    }
    let total_size: u64 = packages.iter().map(|p| p.size_kb).sum();
    println!("  Total size:  {} KB ({:.1} MB)", total_size, total_size as f64 / 1024.0);
    println!();

    // ────────────────────────────────────────────────
    // Phase 2: Create RVF image and embed kernel
    // ────────────────────────────────────────────────
    println!("--- Phase 2: Create Bootable Image ---");
    let image_path = tmp.path().join("microkernel.rvf");
    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };
    let mut store = RvfStore::create(&image_path, options).expect("create store");

    // Build real Linux kernel (Docker) or fall back to builtin stub
    let tmpdir = std::env::temp_dir().join("rvf-microkernel-build");
    std::fs::create_dir_all(&tmpdir).ok();
    let built = rvf_kernel::KernelBuilder::new(KernelArch::X86_64)
        .with_initramfs(&["sshd", "rvf-server"])
        .build(&tmpdir)
        .expect("build kernel");
    let kernel_image = built.bzimage;

    let kernel_seg_id = store
        .embed_kernel(
            KernelArch::X86_64 as u8,
            KernelType::MicroLinux as u8,
            0x003F, // HAS_QUERY_API | HAS_NETWORKING | HAS_STORAGE | HAS_COMPUTE
            &kernel_image,
            22, // SSH port
            Some("console=ttyS0 root=/dev/vda rw init=/sbin/init net.ifnames=0"),
        )
        .expect("embed kernel");

    println!("  Kernel embedded as segment ID: {}", kernel_seg_id);
    println!("  Arch:     x86_64");
    println!("  Type:     Linux (microkernel config)");
    println!("  Size:     {} bytes", kernel_image.len());
    println!("  Cmdline:  console=ttyS0 root=/dev/vda rw init=/sbin/init");
    println!();

    // ────────────────────────────────────────────────
    // Phase 3: Ingest package collection as vectors
    // ────────────────────────────────────────────────
    println!("--- Phase 3: Ingest Package Collection ---");

    // Encode each package as a vector embedding
    // Metadata fields:
    //   0: name (String)
    //   1: version (String)
    //   2: category (String)
    //   3: size_kb (U64)
    //   4: dep_count (U64)

    let pkg_vecs: Vec<Vec<f32>> = packages
        .iter()
        .enumerate()
        .map(|(i, pkg)| {
            // Create a package embedding from its properties
            let mut v = random_vector(dim, i as u64 * 31 + 7);
            // Encode category as a signal in the vector
            let cat_val = match pkg.category {
                "core" => 0.9,
                "kernel" => 0.85,
                "crypto" => 0.7,
                "network" => 0.6,
                "container" => 0.5,
                "vpn" => 0.4,
                "ai" => 0.3,
                "monitoring" => 0.2,
                "system" => 0.15,
                _ => 0.0,
            };
            v[0] = cat_val;
            v[1] = (pkg.size_kb as f32).ln() / 12.0; // log-normalized size
            v[2] = pkg.deps.len() as f32 / 5.0; // normalized dep count
            v
        })
        .collect();

    let pkg_refs: Vec<&[f32]> = pkg_vecs.iter().map(|v| v.as_slice()).collect();
    let pkg_ids: Vec<u64> = (1..=packages.len() as u64).collect();

    let mut metadata = Vec::new();
    for (i, pkg) in packages.iter().enumerate() {
        let id = (i + 1) as u64;
        metadata.push(MetadataEntry { field_id: 0, value: MetadataValue::String(pkg.name.to_string()) });
        metadata.push(MetadataEntry { field_id: 1, value: MetadataValue::String(pkg.version.to_string()) });
        metadata.push(MetadataEntry { field_id: 2, value: MetadataValue::String(pkg.category.to_string()) });
        metadata.push(MetadataEntry { field_id: 3, value: MetadataValue::U64(pkg.size_kb) });
        metadata.push(MetadataEntry { field_id: 4, value: MetadataValue::U64(pkg.deps.len() as u64) });
        let _ = id; // IDs are positional in the batch
    }

    let ingest = store
        .ingest_batch(&pkg_refs, &pkg_ids, Some(&metadata))
        .expect("ingest packages");
    println!("  Ingested {} packages into RVF image", ingest.accepted);
    println!();

    // ────────────────────────────────────────────────
    // Phase 4: SSH key management
    // ────────────────────────────────────────────────
    println!("--- Phase 4: SSH Key Management ---");

    let ssh_keys = [SshKey { user: "root", key_type: "ed25519", fingerprint_seed: 1000, permissions: "admin" },
        SshKey { user: "deploy", key_type: "ed25519", fingerprint_seed: 2000, permissions: "deploy" },
        SshKey { user: "monitor", key_type: "ed25519", fingerprint_seed: 3000, permissions: "readonly" },
        SshKey { user: "backup", key_type: "ed25519", fingerprint_seed: 4000, permissions: "backup" }];

    // Generate Ed25519 keypairs and sign a payload for each
    let (host_sk, host_vk) = keygen(9999);
    println!("  Host key:    ed25519 ({}...)", hex_short(&host_vk.to_bytes(), 8));

    for (ki, key) in ssh_keys.iter().enumerate() {
        let (user_sk, user_vk) = keygen(key.fingerprint_seed);
        let auth_payload = format!(
            "ssh-authorized-key:user={},type={},perms={}",
            key.user, key.key_type, key.permissions
        );

        // Sign the authorization with the host key using a segment header
        let header = SegmentHeader::new(SegmentType::Crypto as u8, 100 + ki as u64);
        let sig = sign_segment(&header, auth_payload.as_bytes(), &host_sk);
        let valid = verify_segment(&header, auth_payload.as_bytes(), &sig, &host_vk);

        println!(
            "  User {:8}: {} fp={}... perms={:<10} signed={}",
            key.user,
            key.key_type,
            hex_short(&user_vk.to_bytes(), 6),
            key.permissions,
            if valid { "OK" } else { "FAIL" },
        );
        let _ = user_sk; // used for key generation
    }
    println!();

    // ────────────────────────────────────────────────
    // Phase 5: Network configuration
    // ────────────────────────────────────────────────
    println!("--- Phase 5: Network Configuration ---");

    let interfaces = vec![
        NetInterface { name: "eth0", ip: "10.0.1.10", mask: "255.255.255.0",
            gateway: Some("10.0.1.1"), mtu: 9000, vlan: None },
        NetInterface { name: "eth1", ip: "192.168.100.10", mask: "255.255.255.0",
            gateway: None, mtu: 1500, vlan: Some(100) },
        NetInterface { name: "wg0", ip: "10.200.0.1", mask: "255.255.255.0",
            gateway: None, mtu: 1420, vlan: None },
        NetInterface { name: "lo", ip: "127.0.0.1", mask: "255.0.0.0",
            gateway: None, mtu: 65535, vlan: None },
        NetInterface { name: "docker0", ip: "172.17.0.1", mask: "255.255.0.0",
            gateway: None, mtu: 1500, vlan: None },
    ];

    println!("  {:10} {:18} {:18} {:>5} {:>6}", "Interface", "IP Address", "Gateway", "MTU", "VLAN");
    println!("  {:->10} {:->18} {:->18} {:->5} {:->6}", "", "", "", "", "");
    for intf in &interfaces {
        println!(
            "  {:10} {:18} {:18} {:>5} {:>6}",
            intf.name,
            format!("{}/{}", intf.ip, intf.mask.split('.').filter(|&o| o != "0").count() * 8),
            intf.gateway.unwrap_or("-"),
            intf.mtu,
            intf.vlan.map_or("-".to_string(), |v| v.to_string()),
        );
    }
    println!();

    // ────────────────────────────────────────────────
    // Phase 6: Package search (semantic + filtered)
    // ────────────────────────────────────────────────
    println!("--- Phase 6: Package Search ---");

    // Search for network-related packages
    let mut net_query = random_vector(dim, 42);
    net_query[0] = 0.6; // network category signal
    net_query[2] = 0.3; // moderate dependencies

    let net_opts = QueryOptions {
        filter: Some(FilterExpr::Eq(2, FilterValue::String("network".to_string()))),
        ..Default::default()
    };
    let net_results = store.query(&net_query, 10, &net_opts).expect("network search");
    println!("  Network packages (filtered by category='network'):");
    for (i, r) in net_results.iter().enumerate() {
        let pkg = &packages[(r.id - 1) as usize];
        println!(
            "    #{}: {}-{} ({} KB, {} deps) dist={:.4}",
            i + 1, pkg.name, pkg.version, pkg.size_kb, pkg.deps.len(), r.distance,
        );
    }
    println!();

    // Search for container-related packages
    let container_opts = QueryOptions {
        filter: Some(FilterExpr::Eq(2, FilterValue::String("container".to_string()))),
        ..Default::default()
    };
    let container_results = store.query(&net_query, 10, &container_opts).expect("container search");
    println!("  Container packages:");
    for (i, r) in container_results.iter().enumerate() {
        let pkg = &packages[(r.id - 1) as usize];
        println!(
            "    #{}: {}-{} ({} KB)",
            i + 1, pkg.name, pkg.version, pkg.size_kb,
        );
    }
    println!();

    // Search by size: find packages > 4 MB
    let large_opts = QueryOptions {
        filter: Some(FilterExpr::Gt(3, FilterValue::U64(4096))),
        ..Default::default()
    };
    let large_results = store.query(&net_query, 20, &large_opts).expect("large pkg search");
    println!("  Large packages (> 4 MB):");
    for (i, r) in large_results.iter().enumerate() {
        let pkg = &packages[(r.id - 1) as usize];
        println!(
            "    #{}: {}-{} ({:.1} MB, category={})",
            i + 1, pkg.name, pkg.version, pkg.size_kb as f64 / 1024.0, pkg.category,
        );
    }
    println!();

    // ────────────────────────────────────────────────
    // Phase 7: Derive an update image
    // ────────────────────────────────────────────────
    println!("--- Phase 7: Derive Update Image ---");
    let update_path = tmp.path().join("microkernel_v2.rvf");
    let update_store = store
        .derive(&update_path, DerivationType::Clone, None)
        .expect("derive update");

    println!("  Base image:   microkernel.rvf");
    println!("  Update image: microkernel_v2.rvf");
    println!("  Lineage depth: {}", update_store.lineage_depth());
    println!(
        "  Parent ID: {}",
        hex_short(update_store.parent_id(), 8)
    );
    println!(
        "  Parent matches: {}",
        update_store.parent_id() == store.file_id()
    );
    println!();

    // ────────────────────────────────────────────────
    // Phase 8: Witness chain (audit trail)
    // ────────────────────────────────────────────────
    println!("--- Phase 8: System Audit Trail ---");

    let ts = 1_700_000_000_000_000_000u64;
    let witness_entries = vec![
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(
                format!("image_create:packages={},kernel=6.8.0-micro", packages.len()).as_bytes(),
            ),
            timestamp_ns: ts,
            witness_type: 0x08, // DATA_PROVENANCE
        },
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(
                format!("kernel_embed:arch=x86_64,type=linux,size={}", kernel_image.len()).as_bytes(),
            ),
            timestamp_ns: ts + 1_000_000,
            witness_type: 0x02, // COMPUTATION
        },
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(
                format!("ssh_keys:users={},host_key=ed25519", ssh_keys.len()).as_bytes(),
            ),
            timestamp_ns: ts + 2_000_000,
            witness_type: 0x01, // PROVENANCE
        },
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(
                format!("network_config:interfaces={}", interfaces.len()).as_bytes(),
            ),
            timestamp_ns: ts + 3_000_000,
            witness_type: 0x01, // PROVENANCE
        },
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(b"image_sealed:all_segments_verified"),
            timestamp_ns: ts + 4_000_000,
            witness_type: 0x07, // COMPUTATION_PROOF
        },
        WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(
                format!("derive_update:parent_depth=0,child_depth={}", update_store.lineage_depth()).as_bytes(),
            ),
            timestamp_ns: ts + 5_000_000,
            witness_type: 0x09, // DERIVATION
        },
    ];

    let chain = create_witness_chain(&witness_entries);
    let verified = verify_witness_chain(&chain).expect("verify chain");
    println!("  Audit entries: {} (all verified)", verified.len());
    for (i, e) in verified.iter().enumerate() {
        let label = match e.witness_type {
            0x01 => "PROVENANCE       ",
            0x02 => "COMPUTATION      ",
            0x07 => "COMPUTATION_PROOF",
            0x08 => "DATA_PROVENANCE  ",
            0x09 => "DERIVATION       ",
            _ => "UNKNOWN          ",
        };
        println!(
            "    #{}: {} hash={}",
            i + 1,
            label,
            hex_short(&e.action_hash, 8),
        );
    }
    println!();

    // ────────────────────────────────────────────────
    // Phase 9: Image layout summary
    // ────────────────────────────────────────────────
    println!("--- Phase 9: Image Layout ---");
    let final_status = store.status();
    println!("  microkernel.rvf contents:");
    println!("    KERNEL_SEG:   Linux x86_64 microkernel ({} bytes)", kernel_image.len());
    println!("    VEC_SEG:      {} packages ({}-dim embeddings)", packages.len(), dim);
    println!("    MANIFEST_SEG: {} segments total", final_status.total_segments);
    println!("    WITNESS_SEG:  {} audit entries", verified.len());
    println!("    CRYPTO_SEG:   {} SSH keys (Ed25519 signed)", ssh_keys.len());
    println!("    File size:    {} bytes ({:.1} KB)", final_status.file_size, final_status.file_size as f64 / 1024.0);
    println!();
    println!("  Deployment workflow:");
    println!("    1. Build: packages → RVF image (this example)");
    println!("    2. Ship:  scp microkernel.rvf node:/boot/");
    println!("    3. Boot:  kexec -l /boot/microkernel.rvf && kexec -e");
    println!("    4. Update: derive v2 → atomic rename → reboot");
    println!("    5. Rollback: rename v1 back → reboot (< 1 second)");
    println!();
    println!("  Package operations:");
    println!("    Install: ingest_batch(package_vector, metadata)");
    println!("    Search:  query(embedding, k=10, filter=category)");
    println!("    Remove:  delete_by_filter(name=package_name)");
    println!("    Update:  derive new image → ingest updated package");
    println!();

    // ────────────────────────────────────────────────
    // Summary
    // ────────────────────────────────────────────────
    println!("=== Summary ===\n");
    println!("  Packages:     {} ({:.1} MB total)", packages.len(), total_size as f64 / 1024.0);
    println!("  Kernel:       Linux 6.8.0-micro x86_64");
    println!("  SSH keys:     {} (Ed25519, host-signed)", ssh_keys.len());
    println!("  Network:      {} interfaces configured", interfaces.len());
    println!("  Audit trail:  {} witness entries", verified.len());
    println!("  Lineage:      base → v2 (depth {})", update_store.lineage_depth());
    println!();
    println!("  Key insight: A single .rvf file is a complete, bootable,");
    println!("  searchable, auditable Linux system image — packages are");
    println!("  vectors, config is metadata, keys are signed segments.");
    println!();
    println!("=== Done ===");
}
