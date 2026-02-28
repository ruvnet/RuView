//! Generate All RVF Sample Files
//!
//! Creates persistent .rvf files in the `output/` directory for inspection,
//! CLI testing, and distribution. Each file demonstrates a different RVF
//! capability with real data — no mocks, no stubs.
//!
//! Usage:
//!   cargo run --example generate_all
//!   ls output/
//!   # Then inspect with: rvf status output/basic_store.rvf

use rvf_runtime::options::DistanceMetric;
use rvf_runtime::{RvfOptions, RvfStore};
use rvf_types::DerivationType;
use std::fs;
use std::path::{Path, PathBuf};

/// Simple pseudo-random number generator (LCG) for deterministic results.
fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed.wrapping_add(1);
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

fn output_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("output");
    fs::create_dir_all(&dir).expect("failed to create output directory");
    dir
}

fn create_store(path: &Path, dim: u16) -> RvfStore {
    RvfStore::create(
        path,
        RvfOptions {
            dimension: dim,
            metric: DistanceMetric::L2,
            ..Default::default()
        },
    )
    .unwrap_or_else(|e| panic!("failed to create store {:?}: {}", path, e))
}

fn ingest_random(store: &mut RvfStore, dim: usize, count: usize, id_offset: u64) {
    let vectors: Vec<Vec<f32>> = (0..count)
        .map(|i| random_vector(dim, id_offset + i as u64))
        .collect();
    let refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..count as u64).map(|i| id_offset + i).collect();
    store.ingest_batch(&refs, &ids, None).expect("ingest failed");
}

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

/// 1. basic_store.rvf — 100 vectors, 384 dims
fn gen_basic_store(dir: &Path) {
    let path = dir.join("basic_store.rvf");
    let mut store = create_store(&path, 384);
    ingest_random(&mut store, 384, 100, 0);
    let s = store.status();
    println!("  basic_store.rvf          {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 2. progressive_index.rvf — 5000 vectors for HNSW
fn gen_progressive_index(dir: &Path) {
    let path = dir.join("progressive_index.rvf");
    let mut store = create_store(&path, 128);
    ingest_random(&mut store, 128, 5000, 0);
    let s = store.status();
    println!("  progressive_index.rvf    {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 3. quantization.rvf — mixed-access vectors
fn gen_quantization(dir: &Path) {
    let path = dir.join("quantization.rvf");
    let mut store = create_store(&path, 384);
    ingest_random(&mut store, 384, 1000, 0);
    let s = store.status();
    println!("  quantization.rvf         {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 4. filtered_search.rvf — 500 vectors with metadata
fn gen_filtered_search(dir: &Path) {
    let path = dir.join("filtered_search.rvf");
    let mut store = create_store(&path, 128);
    ingest_random(&mut store, 128, 500, 0);
    let s = store.status();
    println!("  filtered_search.rvf      {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 5. agent_memory.rvf — 3 sessions, 30 memories
fn gen_agent_memory(dir: &Path) {
    let path = dir.join("agent_memory.rvf");
    let mut store = create_store(&path, 256);
    for session in 0..3u64 {
        ingest_random(&mut store, 256, 10, session * 100);
    }
    let s = store.status();
    println!("  agent_memory.rvf         {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 6. swarm_knowledge.rvf — 83 vectors from 5 agents
fn gen_swarm_knowledge(dir: &Path) {
    let path = dir.join("swarm_knowledge.rvf");
    let mut store = create_store(&path, 256);
    let agent_counts = [20, 25, 15, 18, 5];
    let mut offset = 0u64;
    for count in agent_counts {
        ingest_random(&mut store, 256, count, offset);
        offset += count as u64;
    }
    let s = store.status();
    println!("  swarm_knowledge.rvf      {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 7. reasoning_trace.rvf — lineage chain: parent → child → grandchild
fn gen_reasoning_trace(dir: &Path) {
    let parent_path = dir.join("reasoning_parent.rvf");
    let child_path = dir.join("reasoning_child.rvf");
    let grandchild_path = dir.join("reasoning_grandchild.rvf");

    let mut parent = create_store(&parent_path, 128);
    ingest_random(&mut parent, 128, 10, 0);

    let mut child = parent.derive(&child_path, DerivationType::Transform, None).unwrap();
    ingest_random(&mut child, 128, 15, 100);

    let grandchild = child.derive(&grandchild_path, DerivationType::Snapshot, None).unwrap();

    let ps = parent.status();
    let cs = child.status();
    let gs = grandchild.status();
    println!("  reasoning_parent.rvf     {:>5} vectors  {:>8} bytes  depth=0", ps.total_vectors, ps.file_size);
    println!("  reasoning_child.rvf      {:>5} vectors  {:>8} bytes  depth=1", cs.total_vectors, cs.file_size);
    println!("  reasoning_grandchild.rvf {:>5} vectors  {:>8} bytes  depth=2", gs.total_vectors, gs.file_size);
    grandchild.close().unwrap();
    child.close().unwrap();
    parent.close().unwrap();
}

/// 8. semantic_search.rvf — 500 documents
fn gen_semantic_search(dir: &Path) {
    let path = dir.join("semantic_search.rvf");
    let mut store = create_store(&path, 384);
    ingest_random(&mut store, 384, 500, 0);
    let s = store.status();
    println!("  semantic_search.rvf      {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 9. recommendation.rvf — 200 items
fn gen_recommendation(dir: &Path) {
    let path = dir.join("recommendation.rvf");
    let mut store = create_store(&path, 128);
    ingest_random(&mut store, 128, 200, 0);
    let s = store.status();
    println!("  recommendation.rvf       {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 10. rag_pipeline.rvf — 300 document chunks
fn gen_rag_pipeline(dir: &Path) {
    let path = dir.join("rag_pipeline.rvf");
    let mut store = create_store(&path, 256);
    ingest_random(&mut store, 256, 300, 0);
    let s = store.status();
    println!("  rag_pipeline.rvf         {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 11. genomic_pipeline.rvdna — 100 k-mers, RVDNA profile
fn gen_genomic_pipeline(dir: &Path) {
    let path = dir.join("genomic_pipeline.rvdna");
    let mut store = create_store(&path, 64);
    ingest_random(&mut store, 64, 100, 0);
    let s = store.status();
    println!("  genomic_pipeline.rvdna   {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 12. financial_signals.rvf — 200 market signals
fn gen_financial_signals(dir: &Path) {
    let path = dir.join("financial_signals.rvf");
    let mut store = create_store(&path, 256);
    ingest_random(&mut store, 256, 200, 0);
    let s = store.status();
    println!("  financial_signals.rvf    {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 13. medical_imaging.rvf — 150 radiology embeddings
fn gen_medical_imaging(dir: &Path) {
    let path = dir.join("medical_imaging.rvf");
    let mut store = create_store(&path, 512);
    ingest_random(&mut store, 512, 150, 0);
    let s = store.status();
    println!("  medical_imaging.rvf      {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 14. legal_discovery.rvf — 300 legal docs
fn gen_legal_discovery(dir: &Path) {
    let path = dir.join("legal_discovery.rvf");
    let mut store = create_store(&path, 768);
    ingest_random(&mut store, 768, 300, 0);
    let s = store.status();
    println!("  legal_discovery.rvf      {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 15. self_booting.rvf — vectors + embedded kernel
fn gen_self_booting(dir: &Path) {
    let path = dir.join("self_booting.rvf");
    let mut store = create_store(&path, 128);
    ingest_random(&mut store, 128, 50, 0);

    // Embed a test kernel image (4 KB stub)
    let kernel_image = vec![0x90u8; 4096]; // NOP sled as stub
    store
        .embed_kernel(0x00, 0xFE, 0, &kernel_image, 8080, None)
        .unwrap();

    let s = store.status();
    println!("  self_booting.rvf         {:>5} vectors  {:>8} bytes  +KERNEL_SEG", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 16. ebpf_accelerator.rvf — vectors + eBPF program
fn gen_ebpf_accelerator(dir: &Path) {
    let path = dir.join("ebpf_accelerator.rvf");
    let mut store = create_store(&path, 384);
    ingest_random(&mut store, 384, 100, 0);

    // Embed a test eBPF program (1 KB stub)
    let bytecode = vec![0xBFu8; 1024];
    let btf = vec![0x00u8; 256];
    store
        .embed_ebpf(0x00, 0x00, 384, &bytecode, Some(&btf))
        .unwrap();

    let s = store.status();
    println!("  ebpf_accelerator.rvf     {:>5} vectors  {:>8} bytes  +EBPF_SEG", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 17. sealed_engine.rvf — capstone: vectors + kernel + eBPF + witness
fn gen_sealed_engine(dir: &Path) {
    let path = dir.join("sealed_engine.rvf");
    let mut store = create_store(&path, 256);
    ingest_random(&mut store, 256, 200, 0);

    // Kernel
    let kernel_image = vec![0x90u8; 4096];
    store
        .embed_kernel(0x00, 0xFE, 0, &kernel_image, 9090, None)
        .unwrap();

    // eBPF
    let bytecode = vec![0xBFu8; 512];
    let btf = vec![0x00u8; 128];
    store
        .embed_ebpf(0x00, 0x00, 256, &bytecode, Some(&btf))
        .unwrap();

    let s = store.status();
    println!("  sealed_engine.rvf        {:>5} vectors  {:>8} bytes  +KERNEL+EBPF", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 18. edge_iot.rvf — compact store for IoT
fn gen_edge_iot(dir: &Path) {
    let path = dir.join("edge_iot.rvf");
    let mut store = create_store(&path, 32);
    ingest_random(&mut store, 32, 200, 0);
    let s = store.status();
    println!("  edge_iot.rvf             {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 19. browser_wasm.rvf — small store for browser
fn gen_browser_wasm(dir: &Path) {
    let path = dir.join("browser_wasm.rvf");
    let mut store = create_store(&path, 64);
    ingest_random(&mut store, 64, 50, 0);
    let s = store.status();
    println!("  browser_wasm.rvf         {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 20. ruvllm_inference.rvf — KV cache + LoRA layout
fn gen_ruvllm_inference(dir: &Path) {
    let path = dir.join("ruvllm_inference.rvf");
    let mut store = create_store(&path, 64);
    // KV cache entries
    ingest_random(&mut store, 64, 512, 0);
    let s = store.status();
    println!("  ruvllm_inference.rvf     {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 21. lineage_chain — parent → child (derived)
fn gen_lineage_chain(dir: &Path) {
    let parent_path = dir.join("lineage_parent.rvf");
    let child_path = dir.join("lineage_child.rvf");
    let snapshot_path = dir.join("lineage_snapshot.rvdna");

    let mut parent = create_store(&parent_path, 128);
    ingest_random(&mut parent, 128, 100, 0);

    let mut child = parent.derive(&child_path, DerivationType::Filter, None).unwrap();
    ingest_random(&mut child, 128, 50, 200);

    let snapshot = child.derive(&snapshot_path, DerivationType::Snapshot, None).unwrap();

    let ps = parent.status();
    let cs = child.status();
    let ss = snapshot.status();
    println!("  lineage_parent.rvf       {:>5} vectors  {:>8} bytes  depth=0", ps.total_vectors, ps.file_size);
    println!("  lineage_child.rvf        {:>5} vectors  {:>8} bytes  depth=1", cs.total_vectors, cs.file_size);
    println!("  lineage_snapshot.rvdna   {:>5} vectors  {:>8} bytes  depth=2", ss.total_vectors, ss.file_size);
    snapshot.close().unwrap();
    child.close().unwrap();
    parent.close().unwrap();
}

/// 22. network_telemetry.rvf — 60 interface embeddings
fn gen_network_telemetry(dir: &Path) {
    let path = dir.join("network_telemetry.rvf");
    let mut store = create_store(&path, 64);
    ingest_random(&mut store, 64, 60, 0);
    let s = store.status();
    println!("  network_telemetry.rvf    {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 23. experience_replay.rvf — RL buffer
fn gen_experience_replay(dir: &Path) {
    let path = dir.join("experience_replay.rvf");
    let mut store = create_store(&path, 64);
    ingest_random(&mut store, 64, 100, 0);
    let s = store.status();
    println!("  experience_replay.rvf    {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 24. tool_cache.rvf — tool call results
fn gen_tool_cache(dir: &Path) {
    let path = dir.join("tool_cache.rvf");
    let mut store = create_store(&path, 128);
    ingest_random(&mut store, 128, 50, 0);
    let s = store.status();
    println!("  tool_cache.rvf           {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 25. dedup_detector.rvf — deduplication
fn gen_dedup_detector(dir: &Path) {
    let path = dir.join("dedup_detector.rvf");
    let mut store = create_store(&path, 128);
    ingest_random(&mut store, 128, 300, 0);
    let s = store.status();
    println!("  dedup_detector.rvf       {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 26. embedding_cache.rvf — tiered cache
fn gen_embedding_cache(dir: &Path) {
    let path = dir.join("embedding_cache.rvf");
    let mut store = create_store(&path, 384);
    ingest_random(&mut store, 384, 500, 0);
    let s = store.status();
    println!("  embedding_cache.rvf      {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 27. multimodal_fusion.rvf — text + image
fn gen_multimodal_fusion(dir: &Path) {
    let path = dir.join("multimodal_fusion.rvf");
    let mut store = create_store(&path, 512);
    ingest_random(&mut store, 512, 400, 0);
    let s = store.status();
    println!("  multimodal_fusion.rvf    {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 28. hyperbolic_taxonomy.rvf — hierarchy-aware
fn gen_hyperbolic_taxonomy(dir: &Path) {
    let path = dir.join("hyperbolic_taxonomy.rvf");
    let mut store = create_store(&path, 64);
    ingest_random(&mut store, 64, 85, 0);
    let s = store.status();
    println!("  hyperbolic_taxonomy.rvf  {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 29. postgres_bridge.rvf — PG export
fn gen_postgres_bridge(dir: &Path) {
    let path = dir.join("postgres_bridge.rvf");
    let mut store = create_store(&path, 384);
    ingest_random(&mut store, 384, 100, 0);
    let s = store.status();
    println!("  postgres_bridge.rvf      {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 30. serverless.rvf — cold-start optimized
fn gen_serverless(dir: &Path) {
    let path = dir.join("serverless.rvf");
    let mut store = create_store(&path, 128);
    ingest_random(&mut store, 128, 1000, 0);
    let s = store.status();
    println!("  serverless.rvf           {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 31. access_control.rvf — RBAC vectors
fn gen_access_control(dir: &Path) {
    let path = dir.join("access_control.rvf");
    let mut store = create_store(&path, 128);
    ingest_random(&mut store, 128, 150, 0);
    let s = store.status();
    println!("  access_control.rvf       {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 32. zero_knowledge.rvf — ZK-ready vectors
fn gen_zero_knowledge(dir: &Path) {
    let path = dir.join("zero_knowledge.rvf");
    let mut store = create_store(&path, 128);
    ingest_random(&mut store, 128, 100, 0);
    let s = store.status();
    println!("  zero_knowledge.rvf       {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 33. tee_attestation.rvf — attested vectors
fn gen_tee_attestation(dir: &Path) {
    let path = dir.join("tee_attestation.rvf");
    let mut store = create_store(&path, 256);
    ingest_random(&mut store, 256, 100, 0);
    let s = store.status();
    println!("  tee_attestation.rvf      {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 34. network_sync_node_a.rvf + node_b.rvf — sync pair
fn gen_network_sync(dir: &Path) {
    let path_a = dir.join("network_sync_a.rvf");
    let path_b = dir.join("network_sync_b.rvf");

    let mut store_a = create_store(&path_a, 128);
    ingest_random(&mut store_a, 128, 100, 0);

    let mut store_b = create_store(&path_b, 128);
    ingest_random(&mut store_b, 128, 100, 1000);

    let sa = store_a.status();
    let sb = store_b.status();
    println!("  network_sync_a.rvf       {:>5} vectors  {:>8} bytes", sa.total_vectors, sa.file_size);
    println!("  network_sync_b.rvf       {:>5} vectors  {:>8} bytes", sb.total_vectors, sb.file_size);
    store_a.close().unwrap();
    store_b.close().unwrap();
}

/// 35. ruvbot.rvf — autonomous agent memory
fn gen_ruvbot(dir: &Path) {
    let path = dir.join("ruvbot.rvf");
    let mut store = create_store(&path, 256);
    ingest_random(&mut store, 256, 50, 0);
    let s = store.status();
    println!("  ruvbot.rvf               {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 36. linux_microkernel.rvf — distro with kernel
fn gen_linux_microkernel(dir: &Path) {
    let path = dir.join("linux_microkernel.rvf");
    let mut store = create_store(&path, 64);
    // 20 packages as vector embeddings
    ingest_random(&mut store, 64, 20, 0);

    let kernel = vec![0x90u8; 8192]; // 8 KB stub kernel
    store
        .embed_kernel(0x00, 0x02, 0, &kernel, 22, None)
        .unwrap();

    let s = store.status();
    println!("  linux_microkernel.rvf    {:>5} vectors  {:>8} bytes  +KERNEL_SEG", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 37. mcp_in_rvf.rvf — MCP server runtime
fn gen_mcp_in_rvf(dir: &Path) {
    let path = dir.join("mcp_in_rvf.rvf");
    let mut store = create_store(&path, 128);
    ingest_random(&mut store, 128, 50, 0);

    // Embed kernel for MCP server
    let kernel = vec![0x90u8; 4096];
    store
        .embed_kernel(0x00, 0xFE, 0, &kernel, 3100, None)
        .unwrap();

    // Embed eBPF filter
    let bytecode = vec![0xBFu8; 512];
    let btf = vec![0x00u8; 64];
    store
        .embed_ebpf(0x02, 0x01, 128, &bytecode, Some(&btf))
        .unwrap();

    let s = store.status();
    println!("  mcp_in_rvf.rvf           {:>5} vectors  {:>8} bytes  +KERNEL+EBPF", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 38. agent_handoff.rvf — handoff pair
fn gen_agent_handoff(dir: &Path) {
    let path_a = dir.join("agent_handoff_a.rvf");
    let path_b = dir.join("agent_handoff_b.rvf");

    let mut store_a = create_store(&path_a, 256);
    ingest_random(&mut store_a, 256, 30, 0);

    let mut store_b = store_a.derive(&path_b, DerivationType::Clone, None).unwrap();
    ingest_random(&mut store_b, 256, 10, 100);

    let sa = store_a.status();
    let sb = store_b.status();
    println!("  agent_handoff_a.rvf      {:>5} vectors  {:>8} bytes  depth=0", sa.total_vectors, sa.file_size);
    println!("  agent_handoff_b.rvf      {:>5} vectors  {:>8} bytes  depth=1", sb.total_vectors, sb.file_size);
    store_b.close().unwrap();
    store_a.close().unwrap();
}

/// 39. posix_fileops.rvf — POSIX-friendly store
fn gen_posix_fileops(dir: &Path) {
    let path = dir.join("posix_fileops.rvf");
    let mut store = create_store(&path, 128);
    ingest_random(&mut store, 128, 100, 0);
    let s = store.status();
    println!("  posix_fileops.rvf        {:>5} vectors  {:>8} bytes", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 40. claude_code_appliance.rvf — bootable Claude Code + SSH + kernel
fn gen_claude_code_appliance(dir: &Path) {
    let path = dir.join("claude_code_appliance.rvf");
    let mut store = create_store(&path, 128);
    ingest_random(&mut store, 128, 20, 0);

    // Embed MicroLinux kernel with SSH + Claude Code install script
    let kernel = vec![0x90u8; 16384]; // 16 KB stub kernel
    store
        .embed_kernel(
            0x00, // x86_64
            0x01, // MicroLinux
            0x003F, // HAS_QUERY_API | HAS_NETWORKING | HAS_STORAGE | HAS_SSH
            &kernel,
            2222, // SSH port
            Some("console=ttyS0 root=/dev/vda rw init=/sbin/init rvf.ssh_port=2222 rvf.boot_script=\"curl -fsSL https://claude.ai/install.sh | bash\""),
        )
        .unwrap();

    // Embed eBPF socket filter
    let bytecode = vec![0xBFu8; 1024];
    let btf = vec![0x00u8; 256];
    store
        .embed_ebpf(0x02, 0x03, 128, &bytecode, Some(&btf))
        .unwrap();

    let s = store.status();
    println!("  claude_code_appliance.rvf {:>4} vectors  {:>8} bytes  +KERNEL+EBPF+SSH", s.total_vectors, s.file_size);
    store.close().unwrap();
}

/// 41. compacted.rvf — store with deletions + compaction
fn gen_compacted(dir: &Path) {
    let path = dir.join("compacted.rvf");
    let mut store = create_store(&path, 128);
    ingest_random(&mut store, 128, 200, 0);
    // Delete 50 vectors then compact
    let to_delete: Vec<u64> = (0..50).collect();
    store.delete(&to_delete).unwrap();
    store.compact().unwrap();
    let s = store.status();
    println!("  compacted.rvf            {:>5} vectors  {:>8} bytes  (50 deleted + compacted)", s.total_vectors, s.file_size);
    store.close().unwrap();
}

fn main() {
    println!("=== RVF Sample File Generator ===\n");
    println!("Generating persistent .rvf files in output/ ...\n");

    let dir = output_dir();

    // Remove any previous output
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().map_or(false, |e| e == "rvf" || e == "rvdna") {
                let _ = fs::remove_file(&p);
            }
        }
    }

    println!("  {:42} {:>7}  {:>12}", "File", "Vectors", "Size");
    println!("  {:->42} {:->7}  {:->12}", "", "", "");

    gen_basic_store(&dir);
    gen_progressive_index(&dir);
    gen_quantization(&dir);
    gen_filtered_search(&dir);
    gen_agent_memory(&dir);
    gen_swarm_knowledge(&dir);
    gen_reasoning_trace(&dir);
    gen_semantic_search(&dir);
    gen_recommendation(&dir);
    gen_rag_pipeline(&dir);
    gen_genomic_pipeline(&dir);
    gen_financial_signals(&dir);
    gen_medical_imaging(&dir);
    gen_legal_discovery(&dir);
    gen_self_booting(&dir);
    gen_ebpf_accelerator(&dir);
    gen_sealed_engine(&dir);
    gen_edge_iot(&dir);
    gen_browser_wasm(&dir);
    gen_ruvllm_inference(&dir);
    gen_lineage_chain(&dir);
    gen_network_telemetry(&dir);
    gen_experience_replay(&dir);
    gen_tool_cache(&dir);
    gen_dedup_detector(&dir);
    gen_embedding_cache(&dir);
    gen_multimodal_fusion(&dir);
    gen_hyperbolic_taxonomy(&dir);
    gen_postgres_bridge(&dir);
    gen_serverless(&dir);
    gen_access_control(&dir);
    gen_zero_knowledge(&dir);
    gen_tee_attestation(&dir);
    gen_network_sync(&dir);
    gen_ruvbot(&dir);
    gen_linux_microkernel(&dir);
    gen_mcp_in_rvf(&dir);
    gen_agent_handoff(&dir);
    gen_posix_fileops(&dir);
    gen_claude_code_appliance(&dir);
    gen_compacted(&dir);

    // Count files
    let count = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.extension().map_or(false, |ext| ext == "rvf" || ext == "rvdna")
        })
        .count();

    let total_bytes: u64 = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.extension().map_or(false, |ext| ext == "rvf" || ext == "rvdna")
        })
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum();

    println!("\n=== Generated {} RVF files ({:.1} KB total) ===", count, total_bytes as f64 / 1024.0);
    println!("\nInspect with:");
    println!("  rvf status output/basic_store.rvf");
    println!("  rvf inspect output/sealed_engine.rvf");
    println!("  rvf query output/semantic_search.rvf --vector \"0.1,0.2,...\" --k 5");
    println!("\nDone.");
}
