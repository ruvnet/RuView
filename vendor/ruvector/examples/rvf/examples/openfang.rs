//! OpenFang Agent OS — RVF Knowledge Base (Full Surface)
//!
//! Exercises **every major RVF capability** against a realistic agent-OS
//! registry: vector ingestion, filtered queries, quality envelopes, audited
//! queries, delete + compact, lineage derivation, COW branching, segment
//! embedding (WASM, kernel, eBPF, dashboard), membership filters, DoS
//! hardening, adversarial detection, AGI container packaging, file identity,
//! segment directory, witness chain, and persistence.
//!
//! Run with:
//!   cargo run --example openfang

use std::time::Duration;

use rvf_crypto::{create_witness_chain, shake256_256, verify_witness_chain, WitnessEntry};
use rvf_runtime::adversarial::{centroid_distance_cv, is_degenerate_distribution};
use rvf_runtime::filter::FilterValue;
use rvf_runtime::options::DistanceMetric;
use rvf_runtime::{
    AgiContainerBuilder, BudgetTokenBucket, FilterExpr, MembershipFilter, MetadataEntry,
    MetadataValue, NegativeCache, ParsedAgiManifest, ProofOfWork, QueryOptions, QuerySignature,
    RvfOptions, RvfStore, SearchResult,
};
use rvf_types::agi_container::ContainerSegments;
use rvf_types::DerivationType;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DIM: usize = 128;
const K: usize = 5;

const F_TYPE: u16 = 0;
const F_NAME: u16 = 1;
const F_DOMAIN: u16 = 2;
const F_TIER: u16 = 3;
const F_SEC: u16 = 4;

// Segment type constants (from RVF file format)
const SEG_VEC: u8 = 0x01;
const SEG_MFST: u8 = 0x02;
const SEG_JRNL: u8 = 0x03;
const SEG_WITN: u8 = 0x04;
const SEG_KERN: u8 = 0x05;
const SEG_EBPF: u8 = 0x06;
const SEG_EBPF2: u8 = 0x0F;
const SEG_WASM: u8 = 0x10;
const SEG_DASH: u8 = 0x11;

// ---------------------------------------------------------------------------
// Data definitions
// ---------------------------------------------------------------------------

struct Hand {
    name: &'static str,
    domain: &'static str,
    tier: u64,
    security: u64,
}

struct Tool {
    name: &'static str,
    category: &'static str,
}

struct Channel {
    name: &'static str,
    protocol: &'static str,
}

const HANDS: &[Hand] = &[
    Hand { name: "clip",       domain: "video-processing",  tier: 3, security: 60 },
    Hand { name: "lead",       domain: "sales-automation",   tier: 2, security: 70 },
    Hand { name: "collector",  domain: "osint-intelligence", tier: 4, security: 90 },
    Hand { name: "predictor",  domain: "forecasting",        tier: 3, security: 80 },
    Hand { name: "researcher", domain: "fact-checking",      tier: 3, security: 75 },
    Hand { name: "twitter",    domain: "social-media",       tier: 2, security: 65 },
    Hand { name: "browser",    domain: "web-automation",     tier: 4, security: 95 },
];

const TOOLS: &[Tool] = &[
    Tool { name: "http_fetch",       category: "network" },
    Tool { name: "web_search",       category: "network" },
    Tool { name: "web_scrape",       category: "network" },
    Tool { name: "file_read",        category: "filesystem" },
    Tool { name: "file_write",       category: "filesystem" },
    Tool { name: "file_list",        category: "filesystem" },
    Tool { name: "shell_exec",       category: "system" },
    Tool { name: "process_spawn",    category: "system" },
    Tool { name: "json_parse",       category: "transform" },
    Tool { name: "json_format",      category: "transform" },
    Tool { name: "csv_parse",        category: "transform" },
    Tool { name: "regex_match",      category: "transform" },
    Tool { name: "template_render",  category: "transform" },
    Tool { name: "llm_complete",     category: "inference" },
    Tool { name: "llm_embed",        category: "inference" },
    Tool { name: "llm_classify",     category: "inference" },
    Tool { name: "vector_store",     category: "memory" },
    Tool { name: "vector_search",    category: "memory" },
    Tool { name: "kv_get",           category: "memory" },
    Tool { name: "kv_set",           category: "memory" },
    Tool { name: "sql_query",        category: "database" },
    Tool { name: "sql_execute",      category: "database" },
    Tool { name: "screenshot",       category: "browser" },
    Tool { name: "click_element",    category: "browser" },
    Tool { name: "fill_form",        category: "browser" },
    Tool { name: "navigate",         category: "browser" },
    Tool { name: "pdf_extract",      category: "document" },
    Tool { name: "ocr_image",        category: "document" },
    Tool { name: "email_send",       category: "communication" },
    Tool { name: "email_read",       category: "communication" },
    Tool { name: "webhook_fire",     category: "integration" },
    Tool { name: "api_call",         category: "integration" },
    Tool { name: "schedule_cron",    category: "scheduling" },
    Tool { name: "schedule_delay",   category: "scheduling" },
    Tool { name: "crypto_sign",      category: "security" },
    Tool { name: "crypto_verify",    category: "security" },
    Tool { name: "secret_read",      category: "security" },
    Tool { name: "audit_log",        category: "security" },
];

const CHANNELS: &[Channel] = &[
    Channel { name: "telegram",      protocol: "bot-api" },
    Channel { name: "discord",       protocol: "gateway" },
    Channel { name: "slack",         protocol: "events-api" },
    Channel { name: "whatsapp",      protocol: "cloud-api" },
    Channel { name: "signal",        protocol: "signal-cli" },
    Channel { name: "matrix",        protocol: "client-server" },
    Channel { name: "email-smtp",    protocol: "smtp" },
    Channel { name: "email-imap",    protocol: "imap" },
    Channel { name: "teams",         protocol: "graph-api" },
    Channel { name: "google-chat",   protocol: "chat-api" },
    Channel { name: "linkedin",      protocol: "rest-api" },
    Channel { name: "twitter-x",     protocol: "api-v2" },
    Channel { name: "mastodon",      protocol: "activitypub" },
    Channel { name: "bluesky",       protocol: "at-proto" },
    Channel { name: "reddit",        protocol: "oauth-api" },
    Channel { name: "irc",           protocol: "irc-v3" },
    Channel { name: "xmpp",         protocol: "xmpp-core" },
    Channel { name: "webhook-in",    protocol: "http-post" },
    Channel { name: "webhook-out",   protocol: "http-post" },
    Channel { name: "grpc",          protocol: "grpc" },
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn random_vector(seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(DIM);
    let mut x = seed.wrapping_add(1);
    for _ in 0..DIM {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

fn biased_vector(seed: u64, bias: f32) -> Vec<f32> {
    let mut v = random_vector(seed);
    for d in v.iter_mut().take(16) {
        *d += bias;
    }
    v
}

fn category_bias(cat: &str) -> f32 {
    let h = cat.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    ((h % 200) as f32 - 100.0) * 0.003
}

fn push_meta(out: &mut Vec<MetadataEntry>, fid: u16, val: MetadataValue) {
    out.push(MetadataEntry { field_id: fid, value: val });
}

fn sv(s: &str) -> MetadataValue {
    MetadataValue::String(s.to_string())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{:02x}", b);
    }
    s
}

fn witness(entries: &mut Vec<WitnessEntry>, action: &str, ts_ns: u64, wtype: u8) {
    entries.push(WitnessEntry {
        prev_hash: [0u8; 32],
        action_hash: shake256_256(action.as_bytes()),
        timestamp_ns: ts_ns,
        witness_type: wtype,
    });
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

struct Registry {
    hand_base: u64,
    hand_count: u64,
    tool_base: u64,
    tool_count: u64,
    channel_base: u64,
    channel_count: u64,
}

impl Registry {
    fn new() -> Self {
        let hc = HANDS.len() as u64;
        let tc = TOOLS.len() as u64;
        let cc = CHANNELS.len() as u64;
        Self { hand_base: 0, hand_count: hc, tool_base: hc, tool_count: tc, channel_base: hc + tc, channel_count: cc }
    }
    fn total(&self) -> u64 { self.hand_count + self.tool_count + self.channel_count }
    fn identify(&self, id: u64) -> (&'static str, &'static str) {
        if id >= self.channel_base && id < self.channel_base + self.channel_count {
            ("channel", CHANNELS[(id - self.channel_base) as usize].name)
        } else if id >= self.tool_base && id < self.tool_base + self.tool_count {
            ("tool", TOOLS[(id - self.tool_base) as usize].name)
        } else if id >= self.hand_base && id < self.hand_base + self.hand_count {
            ("hand", HANDS[(id - self.hand_base) as usize].name)
        } else {
            ("unknown", "???")
        }
    }
}

fn print_results(results: &[SearchResult], reg: &Registry) {
    println!("    {:>4}  {:>10}  {:>8}  {:>20}", "ID", "Distance", "Type", "Name");
    println!("    {:->4}  {:->10}  {:->8}  {:->20}", "", "", "", "");
    for r in results {
        let (ty, nm) = reg.identify(r.id);
        println!("    {:>4}  {:>10.4}  {:>8}  {:>20}", r.id, r.distance, ty, nm);
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    println!("=== OpenFang Agent OS — RVF Full Surface Demo ===\n");

    let reg = Registry::new();
    let tmp = TempDir::new().expect("tmpdir");
    let store_path = tmp.path().join("openfang.rvf");
    let branch_path = tmp.path().join("openfang-staging.rvf");
    let derived_path = tmp.path().join("openfang-snapshot.rvf");

    let opts = RvfOptions {
        dimension: DIM as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut wit: Vec<WitnessEntry> = Vec::new();

    // -----------------------------------------------------------------------
    // 1. Create store
    // -----------------------------------------------------------------------
    println!("--- 1. Create Registry ---");
    let mut store = RvfStore::create(&store_path, opts).expect("create");
    println!("  Store: {:?}  ({}d, L2)", store_path, DIM);
    println!("  File ID: {}", hex(&store.file_id()[..8]));
    println!("  Lineage depth: {}", store.lineage_depth());

    // -----------------------------------------------------------------------
    // 2. Register Hands
    // -----------------------------------------------------------------------
    println!("\n--- 2. Register Hands ({}) ---", HANDS.len());
    {
        // Seeds: hands [100..218], tools [500..1647], channels [1000..1817] — non-overlapping
        let vecs: Vec<Vec<f32>> = HANDS.iter().enumerate()
            .map(|(i, h)| biased_vector(i as u64 * 17 + 100, h.tier as f32 * 0.1))
            .collect();
        let refs: Vec<&[f32]> = vecs.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (reg.hand_base..reg.hand_base + reg.hand_count).collect();
        let mut meta = Vec::with_capacity(HANDS.len() * 5);
        for h in HANDS {
            push_meta(&mut meta, F_TYPE, sv("hand"));
            push_meta(&mut meta, F_NAME, sv(h.name));
            push_meta(&mut meta, F_DOMAIN, sv(h.domain));
            push_meta(&mut meta, F_TIER, MetadataValue::U64(h.tier));
            push_meta(&mut meta, F_SEC, MetadataValue::U64(h.security));
        }
        let r = store.ingest_batch(&refs, &ids, Some(&meta)).expect("ingest hands");
        println!("  Ingested {} hands (epoch {})", r.accepted, r.epoch);
        for h in HANDS {
            println!("    {:12} {:22} tier={} sec={}", h.name, h.domain, h.tier, h.security);
        }
    }
    witness(&mut wit, &format!("REGISTER_HANDS:{}", HANDS.len()), 1_709_000_000_000_000_000, 0x01);

    // -----------------------------------------------------------------------
    // 3. Register Tools
    // -----------------------------------------------------------------------
    println!("\n--- 3. Register Tools ({}) ---", TOOLS.len());
    {
        let vecs: Vec<Vec<f32>> = TOOLS.iter().enumerate()
            .map(|(i, t)| biased_vector(i as u64 * 31 + 500, category_bias(t.category)))
            .collect();
        let refs: Vec<&[f32]> = vecs.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (reg.tool_base..reg.tool_base + reg.tool_count).collect();
        let mut meta = Vec::with_capacity(TOOLS.len() * 3);
        for t in TOOLS {
            push_meta(&mut meta, F_TYPE, sv("tool"));
            push_meta(&mut meta, F_NAME, sv(t.name));
            push_meta(&mut meta, F_DOMAIN, sv(t.category));
        }
        let r = store.ingest_batch(&refs, &ids, Some(&meta)).expect("ingest tools");
        println!("  Ingested {} tools (epoch {})", r.accepted, r.epoch);
        let mut cats: Vec<&str> = TOOLS.iter().map(|t| t.category).collect();
        cats.sort_unstable();
        cats.dedup();
        for c in &cats {
            let ns: Vec<&str> = TOOLS.iter().filter(|t| t.category == *c).map(|t| t.name).collect();
            println!("    [{:14}] {}", c, ns.join(", "));
        }
    }
    witness(&mut wit, &format!("REGISTER_TOOLS:{}", TOOLS.len()), 1_709_000_001_000_000_000, 0x01);

    // -----------------------------------------------------------------------
    // 4. Register Channels
    // -----------------------------------------------------------------------
    println!("\n--- 4. Register Channels ({}) ---", CHANNELS.len());
    {
        let vecs: Vec<Vec<f32>> = CHANNELS.iter().enumerate()
            .map(|(i, c)| biased_vector(i as u64 * 43 + 1000, category_bias(c.protocol)))
            .collect();
        let refs: Vec<&[f32]> = vecs.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = (reg.channel_base..reg.channel_base + reg.channel_count).collect();
        let mut meta = Vec::with_capacity(CHANNELS.len() * 3);
        for c in CHANNELS {
            push_meta(&mut meta, F_TYPE, sv("channel"));
            push_meta(&mut meta, F_NAME, sv(c.name));
            push_meta(&mut meta, F_DOMAIN, sv(c.protocol));
        }
        let r = store.ingest_batch(&refs, &ids, Some(&meta)).expect("ingest channels");
        println!("  Ingested {} channels (epoch {})", r.accepted, r.epoch);
        for c in CHANNELS {
            println!("    {:14} ({})", c.name, c.protocol);
        }
    }
    witness(&mut wit, &format!("REGISTER_CHANNELS:{}", CHANNELS.len()), 1_709_000_002_000_000_000, 0x01);

    println!("\n  Total registry: {} components", reg.total());

    // -----------------------------------------------------------------------
    // 5. Task routing — unfiltered + hands-only
    // -----------------------------------------------------------------------
    println!("\n--- 5. Task Routing ---");
    let query = biased_vector(42, 0.3);

    let all = store.query(&query, K, &QueryOptions::default()).expect("query");
    println!("  Unfiltered top-{}:", K);
    print_results(&all, &reg);

    let hands_only = QueryOptions {
        filter: Some(FilterExpr::Eq(F_TYPE, FilterValue::String("hand".into()))),
        ..Default::default()
    };
    let hand_res = store.query(&query, K, &hands_only).expect("query hands");
    println!("\n  Hands only:");
    print_results(&hand_res, &reg);
    witness(&mut wit, "ROUTE_TASK:k=5", 1_709_000_010_000_000_000, 0x02);

    // -----------------------------------------------------------------------
    // 6. Quality envelope query
    // -----------------------------------------------------------------------
    println!("\n--- 6. Quality Envelope ---");
    let envelope = store.query_with_envelope(&query, K, &QueryOptions::default())
        .expect("envelope query");
    println!("  Quality: {:?}", envelope.quality);
    println!("  HNSW candidates: {}", envelope.evidence.hnsw_candidate_count);
    println!("  Safety-net candidates: {}", envelope.evidence.safety_net_candidate_count);
    println!("  Budget total_us: {}", envelope.budgets.total_us);
    println!("  Results: {} (top match: id={}, d={:.4})",
        envelope.results.len(),
        envelope.results.first().map_or(0, |r| r.id),
        envelope.results.first().map_or(0.0, |r| r.distance));
    witness(&mut wit, "QUERY_ENVELOPE:k=5", 1_709_000_011_000_000_000, 0x02);

    // -----------------------------------------------------------------------
    // 7. Audited query (auto-appends witness)
    // -----------------------------------------------------------------------
    println!("\n--- 7. Audited Query ---");
    let audited = store.query_audited(&query, K, &QueryOptions::default())
        .expect("audited query");
    println!("  Returned {} results (witness auto-appended to store)", audited.len());
    println!("  Store witness hash: {}", hex(&store.last_witness_hash()[..8]));
    witness(&mut wit, "QUERY_AUDITED:k=5", 1_709_000_012_000_000_000, 0x02);

    // -----------------------------------------------------------------------
    // 8. Security + tier filters
    // -----------------------------------------------------------------------
    println!("\n--- 8. Security Filter (sec >= 80) ---");
    let sec_res = store.query(&query, K, &QueryOptions {
        filter: Some(FilterExpr::And(vec![
            FilterExpr::Eq(F_TYPE, FilterValue::String("hand".into())),
            FilterExpr::Ge(F_SEC, FilterValue::U64(80)),
        ])),
        ..Default::default()
    }).expect("sec query");
    print_results(&sec_res, &reg);
    println!("  {} agents pass threshold", sec_res.len());

    println!("\n--- 9. Tier-4 Autonomous Agents ---");
    let tier_res = store.query(&query, K, &QueryOptions {
        filter: Some(FilterExpr::And(vec![
            FilterExpr::Eq(F_TYPE, FilterValue::String("hand".into())),
            FilterExpr::Eq(F_TIER, FilterValue::U64(4)),
        ])),
        ..Default::default()
    }).expect("tier query");
    print_results(&tier_res, &reg);

    println!("\n--- 10. Security Tool Discovery ---");
    let tool_res = store.query(&query, 10, &QueryOptions {
        filter: Some(FilterExpr::And(vec![
            FilterExpr::Eq(F_TYPE, FilterValue::String("tool".into())),
            FilterExpr::Eq(F_DOMAIN, FilterValue::String("security".into())),
        ])),
        ..Default::default()
    }).expect("tool query");
    print_results(&tool_res, &reg);

    // -----------------------------------------------------------------------
    // 11. Membership filter — multi-tenant isolation
    // -----------------------------------------------------------------------
    println!("\n--- 11. Membership Filter (tenant isolation) ---");
    {
        let mut mf = MembershipFilter::new_include(reg.total());
        // Tenant A can only see tools (IDs 7..44)
        for id in reg.tool_base..reg.tool_base + reg.tool_count {
            mf.add(id);
        }
        println!("  Created include-mode filter: {} members of {} total",
            mf.member_count(), reg.total());
        println!("  Mode: {:?}, generation: {}", mf.mode(), mf.generation_id());

        // Verify containment
        let hand_visible = mf.contains(0); // clip hand
        let tool_visible = mf.contains(reg.tool_base); // http_fetch tool
        println!("  Hand 'clip' visible: {}  (expect false)", hand_visible);
        println!("  Tool 'http_fetch' visible: {}  (expect true)", tool_visible);
        assert!(!hand_visible);
        assert!(tool_visible);

        let serialized = mf.serialize();
        println!("  Serialized filter: {} bytes", serialized.len());
    }
    witness(&mut wit, "MEMBERSHIP_FILTER:tenant", 1_709_000_015_000_000_000, 0x01);

    // -----------------------------------------------------------------------
    // 12. DoS hardening — token bucket + negative cache + proof-of-work
    // -----------------------------------------------------------------------
    println!("\n--- 12. DoS Hardening ---");
    {
        // Token bucket: 1000 ops per second
        let mut bucket = BudgetTokenBucket::new(1000, Duration::from_secs(1));
        let cost = (K as u64) * (reg.total()); // k * N distance ops
        match bucket.try_consume(cost) {
            Ok(remaining) => println!("  Token bucket: consumed {} ops, {} remaining", cost, remaining),
            Err(deficit) => println!("  Token bucket: REJECTED (need {} more tokens)", deficit),
        }

        // Query signature + negative cache
        let sig = QuerySignature::from_query(&query);
        let mut neg_cache = NegativeCache::new(3, Duration::from_secs(60), 1000);
        let bl1 = neg_cache.record_degenerate(sig);
        let bl2 = neg_cache.record_degenerate(sig);
        let bl3 = neg_cache.record_degenerate(sig); // 3rd hit -> blacklisted
        println!("  Negative cache: hit1={} hit2={} hit3(blacklist)={}", bl1, bl2, bl3);
        println!("  Signature blacklisted: {}", neg_cache.is_blacklisted(&sig));

        // Proof-of-work
        let pow = ProofOfWork {
            challenge: [0x4F, 0x50, 0x45, 0x4E, 0x46, 0x41, 0x4E, 0x47, // "OPENFANG"
                        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            difficulty: 8,
        };
        match pow.solve() {
            Some(nonce) => {
                let valid = pow.verify(nonce);
                println!("  PoW (d=8): solved nonce={}, valid={}", nonce, valid);
            }
            None => println!("  PoW: no solution found within limit"),
        }
    }
    witness(&mut wit, "DOS_HARDENING:bucket+cache+pow", 1_709_000_016_000_000_000, 0x01);

    // -----------------------------------------------------------------------
    // 13. Adversarial detection
    // -----------------------------------------------------------------------
    println!("\n--- 13. Adversarial Detection ---");
    {
        // Natural distances — extract from query results
        let natural: Vec<f32> = all.iter().map(|r| r.distance).collect();
        let cv_natural = centroid_distance_cv(&natural, K);
        let degen_natural = is_degenerate_distribution(&natural, K);
        println!("  Natural query distances: {:?}", natural.iter().map(|d| format!("{:.2}", d)).collect::<Vec<_>>());
        println!("  CV={:.4}, degenerate={}", cv_natural, degen_natural);

        // Adversarial — uniform distances (attack vector)
        let uniform = vec![5.0f32; 100];
        let cv_uniform = centroid_distance_cv(&uniform, K);
        let degen_uniform = is_degenerate_distribution(&uniform, K);
        println!("  Uniform distances (simulated attack):");
        println!("  CV={:.4}, degenerate={}", cv_uniform, degen_uniform);
        assert!(degen_uniform, "uniform should be degenerate");
    }
    witness(&mut wit, "ADVERSARIAL:detect", 1_709_000_017_000_000_000, 0x02);

    // -----------------------------------------------------------------------
    // 14. Embed WASM module (query engine microkernel)
    // -----------------------------------------------------------------------
    println!("\n--- 14. Embed WASM Module ---");
    let fake_wasm = b"\x00asm\x01\x00\x00\x00"; // minimal WASM header
    let wasm_seg = store.embed_wasm(
        0x02,   // role: Microkernel
        0x01,   // target: wasm32
        0x0000, // no required features
        fake_wasm,
        1,      // export_count
        1,      // bootstrap_priority
        0,      // interpreter_type
    ).expect("embed wasm");
    println!("  Embedded WASM microkernel: seg_id={}, {} bytes", wasm_seg, fake_wasm.len());
    println!("  Self-bootstrapping: {}", store.is_self_bootstrapping());

    // Extract and verify round-trip
    let (hdr, bytecode) = store.extract_wasm().expect("extract wasm").expect("wasm present");
    println!("  Extracted: header={} bytes, bytecode={} bytes", hdr.len(), bytecode.len());
    assert_eq!(&bytecode, fake_wasm);
    witness(&mut wit, "EMBED_WASM:microkernel", 1_709_000_018_000_000_000, 0x01);

    // -----------------------------------------------------------------------
    // 15. Embed kernel image
    // -----------------------------------------------------------------------
    println!("\n--- 15. Embed Kernel ---");
    let fake_kernel = b"bzImage-openfang-v1.0-minimal";
    let kern_seg = store.embed_kernel(
        0x01,   // arch: x86_64
        0x01,   // kernel_type: linux
        0x0000, // flags
        fake_kernel,
        8080,   // api_port
        Some("console=ttyS0 root=/dev/vda rw"),
    ).expect("embed kernel");
    println!("  Embedded kernel: seg_id={}, {} bytes, port=8080", kern_seg, fake_kernel.len());

    let (khdr, kpayload) = store.extract_kernel().expect("extract kernel").expect("kernel present");
    println!("  Extracted: header={} bytes, payload={} bytes", khdr.len(), kpayload.len());
    // Payload contains cmdline + image; verify image bytes are present
    assert!(kpayload.windows(fake_kernel.len()).any(|w| w == fake_kernel),
        "kernel image not found in extracted payload");
    witness(&mut wit, "EMBED_KERNEL:linux", 1_709_000_019_000_000_000, 0x01);

    // -----------------------------------------------------------------------
    // 16. Embed eBPF program
    // -----------------------------------------------------------------------
    println!("\n--- 16. Embed eBPF ---");
    // 8 bytes = 1 eBPF instruction (mov64 r0, 0; exit)
    let fake_ebpf = &[0xb7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00u8,
                       0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let ebpf_seg = store.embed_ebpf(
        0x01,       // program_type: socket_filter
        0x01,       // attach_type: ingress
        DIM as u16, // max_dimension
        fake_ebpf,
        None,       // no BTF
    ).expect("embed ebpf");
    println!("  Embedded eBPF: seg_id={}, {} bytes (2 insns)", ebpf_seg, fake_ebpf.len());

    let (ehdr, eprog) = store.extract_ebpf().expect("extract ebpf").expect("ebpf present");
    println!("  Extracted: header={} bytes, program={} bytes", ehdr.len(), eprog.len());
    assert_eq!(&eprog, fake_ebpf);
    witness(&mut wit, "EMBED_EBPF:filter", 1_709_000_019_500_000_000, 0x01);

    // -----------------------------------------------------------------------
    // 17. Embed dashboard
    // -----------------------------------------------------------------------
    println!("\n--- 17. Embed Dashboard ---");
    let dashboard_html = br#"<!DOCTYPE html>
<html><head><title>OpenFang Registry</title></head>
<body><h1>OpenFang Agent Registry Dashboard</h1>
<p>7 Hands | 38 Tools | 20 Channels</p></body></html>"#;
    let dash_seg = store.embed_dashboard(
        0x01, // ui_framework: vanilla HTML
        dashboard_html,
        "index.html",
    ).expect("embed dashboard");
    println!("  Embedded dashboard: seg_id={}, {} bytes", dash_seg, dashboard_html.len());

    let (dhdr, dbundle) = store.extract_dashboard().expect("extract dash").expect("dash present");
    println!("  Extracted: header={} bytes, bundle={} bytes", dhdr.len(), dbundle.len());
    assert_eq!(&dbundle, dashboard_html);
    witness(&mut wit, "EMBED_DASHBOARD:html", 1_709_000_019_800_000_000, 0x01);

    // -----------------------------------------------------------------------
    // 18. Delete + Compact
    // -----------------------------------------------------------------------
    println!("\n--- 18. Delete + Compact (decommission 'twitter') ---");
    let twitter_id = HANDS.iter().position(|h| h.name == "twitter").unwrap() as u64 + reg.hand_base;
    let st_before = store.status();
    println!("  Before: {} vectors, {} bytes, dead={:.2}",
        st_before.total_vectors, st_before.file_size, st_before.dead_space_ratio);

    let del = store.delete(&[twitter_id]).expect("delete");
    println!("  Deleted {} vector(s) (epoch {})", del.deleted, del.epoch);

    let comp = store.compact().expect("compact");
    println!("  Compacted: {} segments, {} bytes reclaimed (epoch {})",
        comp.segments_compacted, comp.bytes_reclaimed, comp.epoch);

    let st_after = store.status();
    println!("  After: {} vectors, {} bytes, dead={:.2}",
        st_after.total_vectors, st_after.file_size, st_after.dead_space_ratio);

    let post_del = store.query(&query, K, &hands_only).expect("post-delete");
    for r in &post_del {
        assert_ne!(r.id, twitter_id, "twitter should be deleted");
    }
    println!("  Verified: 'twitter' absent from results");
    witness(&mut wit, "DELETE+COMPACT:twitter", 1_709_000_020_000_000_000, 0x01);

    // -----------------------------------------------------------------------
    // 19. Derive (lineage snapshot)
    // -----------------------------------------------------------------------
    println!("\n--- 19. Derive (Lineage Snapshot) ---");
    let parent_fid = hex(&store.file_id()[..8]);
    let parent_depth = store.lineage_depth();

    let child = store.derive(&derived_path, DerivationType::Snapshot, None).expect("derive");
    let child_fid = hex(&child.file_id()[..8]);
    let child_parent = hex(&child.parent_id()[..8]);
    let child_depth = child.lineage_depth();

    println!("  Parent:  fid={}  depth={}", parent_fid, parent_depth);
    println!("  Child:   fid={}  depth={}", child_fid, child_depth);
    println!("  Lineage: parent_id matches = {}", child_parent == parent_fid);
    assert_eq!(child_depth, parent_depth + 1);
    child.close().expect("close child");
    witness(&mut wit, "DERIVE:snapshot", 1_709_000_030_000_000_000, 0x01);

    // -----------------------------------------------------------------------
    // 20. COW Branch + freeze
    // -----------------------------------------------------------------------
    println!("\n--- 20. COW Branch (Staging) ---");
    store.freeze().expect("freeze");
    println!("  Parent frozen");

    let mut staging = store.branch(&branch_path).expect("branch");
    println!("  Branch: cow_child={}", staging.is_cow_child());
    if let Some(stats) = staging.cow_stats() {
        println!("  COW: {} clusters, {} local", stats.cluster_count, stats.local_cluster_count);
    }

    let exp_id = reg.total();
    let exp_vec = biased_vector(9999, 0.5);
    let mut exp_meta = Vec::with_capacity(5);
    push_meta(&mut exp_meta, F_TYPE, sv("hand"));
    push_meta(&mut exp_meta, F_NAME, sv("sentinel"));
    push_meta(&mut exp_meta, F_DOMAIN, sv("threat-detection"));
    push_meta(&mut exp_meta, F_TIER, MetadataValue::U64(4));
    push_meta(&mut exp_meta, F_SEC, MetadataValue::U64(99));

    let exp_r = staging.ingest_batch(&[exp_vec.as_slice()], &[exp_id], Some(&exp_meta))
        .expect("ingest sentinel");
    println!("  Added 'sentinel' to staging (epoch {})", exp_r.epoch);
    println!("  Staging: {} vectors (parent: {})", staging.status().total_vectors, st_after.total_vectors);

    if let Some(stats) = staging.cow_stats() {
        println!("  COW after write: {} clusters, {} local", stats.cluster_count, stats.local_cluster_count);
    }
    staging.close().expect("close staging");
    witness(&mut wit, "COW_BRANCH:sentinel", 1_709_000_040_000_000_000, 0x01);

    // -----------------------------------------------------------------------
    // 21. AGI Container manifest
    // -----------------------------------------------------------------------
    println!("\n--- 21. AGI Container ---");
    {
        let orchestrator = br#"{"claude_code":{"model":"claude-opus-4-6"},"claude_flow":{"topology":"hierarchical","max_agents":15}}"#;
        let tool_reg = br#"[{"name":"rvf_query","type":"vector_search"},{"name":"rvf_route","type":"task_routing"}]"#;
        let eval_tasks = br#"[{"id":1,"task":"route 1000 tasks under 10ms"}]"#;
        let eval_graders = br#"[{"type":"latency_p99","threshold_ms":10}]"#;

        let segs = ContainerSegments {
            kernel_present: true,
            kernel_size: fake_kernel.len() as u64,
            wasm_count: 1,
            wasm_total_size: fake_wasm.len() as u64,
            vec_segment_count: 1,
            witness_count: wit.len() as u32,
            orchestrator_present: true,
            world_model_present: true,
            ..Default::default()
        };

        let builder = AgiContainerBuilder::new([0xFA; 16], [0x01; 16])
            .with_model_id("claude-opus-4-6")
            .with_policy(b"autonomous-level-4", [0xBB; 8])
            .with_orchestrator(orchestrator)
            .with_tool_registry(tool_reg)
            .with_agent_prompts(b"You are an OpenFang routing agent.")
            .with_eval_tasks(eval_tasks)
            .with_eval_graders(eval_graders)
            .with_skill_library(b"[]")
            .with_project_instructions(b"# OpenFang CLAUDE.md\nRoute tasks to Hands.")
            .with_domain_profile(b"agent-os-registry-v1")
            .offline_capable()
            .with_segments(segs);

        let (payload, header) = builder.build().expect("build container");
        println!("  Container: {} bytes", payload.len());
        println!("  Magic valid: {}", header.is_valid_magic());
        println!("  Flags: kernel={} orchestrator={} eval={} offline={} tools={}",
            header.has_kernel(), header.has_orchestrator(),
            header.flags & 0x10 != 0,  // bit 4: eval suite present
            header.is_offline_capable(),
            header.flags & 0x400 != 0); // bit 10: tool registry present

        let parsed = ParsedAgiManifest::parse(&payload).expect("parse manifest");
        println!("  Model: {:?}", parsed.model_id_str());
        println!("  Autonomous capable: {}", parsed.is_autonomous_capable());
        println!("  Orchestrator: {} bytes", parsed.orchestrator_config.map_or(0, |c| c.len()));
        println!("  Tool registry: {} bytes", parsed.tool_registry.map_or(0, |c| c.len()));
        println!("  Project instructions: {} bytes", parsed.project_instructions.map_or(0, |c| c.len()));
    }
    witness(&mut wit, "AGI_CONTAINER:build+parse", 1_709_000_050_000_000_000, 0x01);

    // -----------------------------------------------------------------------
    // 22. Segment directory
    // -----------------------------------------------------------------------
    println!("\n--- 22. Segment Directory ---");
    let seg_dir: Vec<_> = store.segment_dir().to_vec();
    println!("  {} segments:", seg_dir.len());
    println!("    {:>6}  {:>8}  {:>8}  {:>6}", "SegID", "Offset", "Length", "Type");
    println!("    {:->6}  {:->8}  {:->8}  {:->6}", "", "", "", "");
    for &(seg_id, offset, length, seg_type) in &seg_dir {
        let tname = match seg_type {
            SEG_VEC  => "VEC",
            SEG_MFST => "MFST",
            SEG_JRNL => "JRNL",
            SEG_WITN => "WITN",
            SEG_KERN => "KERN",
            SEG_EBPF => "EBPF",
            SEG_EBPF2 => "EBPF2",
            SEG_WASM => "WASM",
            SEG_DASH => "DASH",
            _ => "????",
        };
        println!("    {:>6}  {:>8}  {:>8}  {:>6}", seg_id, offset, length, tname);
    }

    // -----------------------------------------------------------------------
    // 23. Witness chain
    // -----------------------------------------------------------------------
    println!("\n--- 23. Witness Chain ---");
    let chain = create_witness_chain(&wit);
    println!("  {} entries, {} bytes", wit.len(), chain.len());
    println!("  Store witness hash: {}", hex(&store.last_witness_hash()[..8]));

    match verify_witness_chain(&chain) {
        Ok(verified) => {
            println!("  Integrity: VALID\n");
            let labels = [
                "REGISTER_HANDS", "REGISTER_TOOLS", "REGISTER_CHANNELS",
                "ROUTE_TASK", "QUERY_ENVELOPE", "QUERY_AUDITED",
                "MEMBERSHIP", "DOS_HARDENING", "ADVERSARIAL",
                "EMBED_WASM", "EMBED_KERNEL", "EMBED_EBPF", "EMBED_DASH",
                "DELETE+COMPACT", "DERIVE", "COW_BRANCH", "AGI_CONTAINER",
            ];
            println!("    {:>2}  {:>4}  {:>22}  {}", "#", "Kind", "Timestamp", "Action");
            println!("    {:->2}  {:->4}  {:->22}  {:->20}", "", "", "", "");
            for (i, e) in verified.iter().enumerate() {
                let t = if e.witness_type == 0x01 { "PROV" } else { "COMP" };
                let l = labels.get(i).unwrap_or(&"???");
                println!("    {:>2}  {:>4}  {:>22}  {}", i, t, e.timestamp_ns, l);
            }
        }
        Err(e) => println!("  Integrity: FAILED ({:?})", e),
    }

    // -----------------------------------------------------------------------
    // 24. Persistence
    // -----------------------------------------------------------------------
    println!("\n--- 24. Persistence ---");
    // Capture baseline *after* delete+compact so the comparison is stable
    let pre_close = store.query(&query, K, &QueryOptions::default()).expect("pre-close query");
    let final_st = store.status();
    println!("  Before: {} vectors, {} segments, {} bytes",
        final_st.total_vectors, final_st.total_segments, final_st.file_size);
    drop(store);

    let reopened = RvfStore::open_readonly(&store_path).expect("reopen");
    let reopen_st = reopened.status();
    println!("  After:  {} vectors, epoch {}", reopen_st.total_vectors, reopen_st.current_epoch);
    println!("  File ID preserved: {}", hex(&reopened.file_id()[..8]) == parent_fid);

    // Verify segments survive persistence
    let wasm_ok = reopened.extract_wasm().expect("re-extract wasm").is_some();
    let kern_ok = reopened.extract_kernel().expect("re-extract kernel").is_some();
    let ebpf_ok = reopened.extract_ebpf().expect("re-extract ebpf").is_some();
    let dash_ok = reopened.extract_dashboard().expect("re-extract dash").is_some();
    println!("  WASM={} Kernel={} eBPF={} Dashboard={}", wasm_ok, kern_ok, ebpf_ok, dash_ok);

    let recheck = reopened.query(&query, K, &QueryOptions::default()).expect("recheck");
    assert_eq!(pre_close.len(), recheck.len(), "count mismatch after reopen");
    for (a, b) in pre_close.iter().zip(recheck.iter()) {
        assert_eq!(a.id, b.id, "id mismatch after reopen");
        assert!((a.distance - b.distance).abs() < 1e-6, "distance mismatch after reopen");
    }
    println!("  Persistence verified.");

    // -----------------------------------------------------------------------
    // Summary
    // -----------------------------------------------------------------------
    println!("\n=== Summary ===\n");
    println!("  Registry:     {} hands + {} tools + {} channels = {}",
        HANDS.len(), TOOLS.len(), CHANNELS.len(), reg.total());
    println!("  Queries:      basic, filtered, envelope, audited");
    println!("  Filters:      security (>=80), tier (==4), category, membership");
    println!("  Segments:     VEC + WASM + KERN + EBPF + DASH = {} total", seg_dir.len());
    println!("  Lifecycle:    delete + compact (twitter removed)");
    println!("  Lineage:      derive depth {}, COW branch with sentinel", child_depth);
    println!("  Security:     DoS bucket + negative cache + PoW + adversarial detect");
    println!("  AGI:          container manifest (autonomous capable)");
    println!("  Witness:      {} entries, chain verified", wit.len());
    println!("  File:         {} bytes, {} segments, persistence verified",
        final_st.file_size, final_st.total_segments);

    println!("\nDone — {} RVF capabilities exercised.", 24);
}
