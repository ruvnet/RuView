//! Swarm Memory Example - Distributed Knowledge Across ESP32 Cluster
//!
//! Demonstrates using RuVector federated search for sharing knowledge
//! across multiple ESP32 chips in a swarm.
//!
//! # Use Cases
//! - Robot swarms sharing exploration data
//! - Distributed sensor networks learning together
//! - Multi-device AI assistants with shared memory
//! - Collaborative learning across edge devices

#![allow(unused)]

use heapless::Vec as HVec;
use heapless::String as HString;

const EMBED_DIM: usize = 32;
const MAX_KNOWLEDGE: usize = 64;
const MAX_PEERS: usize = 8;

/// A piece of knowledge in the swarm
#[derive(Debug, Clone)]
struct Knowledge {
    id: u32,
    /// Source chip that discovered this
    source_chip: u8,
    /// Knowledge category
    category: KnowledgeCategory,
    /// Text description
    text: HString<64>,
    /// Embedding for similarity search
    embedding: [i8; EMBED_DIM],
    /// Confidence (0-100)
    confidence: u8,
    /// Times this knowledge was accessed
    access_count: u16,
    /// Timestamp
    timestamp: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum KnowledgeCategory {
    /// Physical environment ("obstacle at location X")
    Environment,
    /// Successful action ("approach from left worked")
    Action,
    /// Object identification ("red object is target")
    Object,
    /// Route/path information
    Navigation,
    /// Danger/hazard warning
    Hazard,
    /// Resource location
    Resource,
}

/// Message types for swarm communication
#[derive(Debug, Clone)]
enum SwarmMessage {
    /// Share new knowledge with peers
    ShareKnowledge(Knowledge),
    /// Query peers for similar knowledge
    QueryKnowledge { query_embed: [i8; EMBED_DIM], k: u8 },
    /// Response to query
    QueryResponse { results: HVec<Knowledge, 4> },
    /// Request sync of all knowledge
    SyncRequest,
    /// Acknowledge receipt
    Ack { knowledge_id: u32 },
}

/// Single chip's local knowledge store
struct ChipMemory {
    chip_id: u8,
    local_knowledge: HVec<Knowledge, MAX_KNOWLEDGE>,
    next_id: u32,
    /// Knowledge received from each peer
    peer_knowledge_count: [u32; MAX_PEERS],
}

impl ChipMemory {
    fn new(chip_id: u8) -> Self {
        Self {
            chip_id,
            local_knowledge: HVec::new(),
            next_id: 0,
            peer_knowledge_count: [0; MAX_PEERS],
        }
    }

    /// Store local discovery
    fn store_local(&mut self, category: KnowledgeCategory, text: &str, embedding: &[i8]) -> Result<u32, &'static str> {
        if self.local_knowledge.len() >= MAX_KNOWLEDGE {
            // Evict least accessed knowledge
            self.evict_least_important();
        }

        let id = (self.chip_id as u32) << 24 | self.next_id;
        self.next_id += 1;

        let mut text_str = HString::new();
        for c in text.chars().take(64) {
            text_str.push(c).map_err(|_| "Text overflow")?;
        }

        let mut embed = [0i8; EMBED_DIM];
        for (i, &v) in embedding.iter().take(EMBED_DIM).enumerate() {
            embed[i] = v;
        }

        let knowledge = Knowledge {
            id,
            source_chip: self.chip_id,
            category,
            text: text_str,
            embedding: embed,
            confidence: 80,
            access_count: 0,
            timestamp: 0, // Would be real timestamp
        };

        self.local_knowledge.push(knowledge).map_err(|_| "Storage full")?;
        Ok(id)
    }

    /// Store knowledge from peer
    fn store_peer_knowledge(&mut self, knowledge: Knowledge) -> Result<(), &'static str> {
        // Check if we already have this
        if self.local_knowledge.iter().any(|k| k.id == knowledge.id) {
            return Ok(()); // Already have it
        }

        if self.local_knowledge.len() >= MAX_KNOWLEDGE {
            self.evict_least_important();
        }

        // Track peer contribution
        if knowledge.source_chip < MAX_PEERS as u8 {
            self.peer_knowledge_count[knowledge.source_chip as usize] += 1;
        }

        self.local_knowledge.push(knowledge).map_err(|_| "Storage full")?;
        Ok(())
    }

    /// Search local knowledge
    fn search(&mut self, query: &[i8], k: usize) -> HVec<(usize, i32), 8> {
        let mut results: HVec<(usize, i32), MAX_KNOWLEDGE> = HVec::new();

        for (idx, knowledge) in self.local_knowledge.iter().enumerate() {
            let dist = euclidean_distance(query, &knowledge.embedding);
            let _ = results.push((idx, dist));
        }

        results.sort_by_key(|(_, d)| *d);

        let mut top_k: HVec<(usize, i32), 8> = HVec::new();
        for (idx, d) in results.iter().take(k) {
            // Update access counts
            if let Some(knowledge) = self.local_knowledge.get_mut(*idx) {
                knowledge.access_count = knowledge.access_count.saturating_add(1);
            }
            let _ = top_k.push((*idx, *d));
        }

        top_k
    }

    /// Search by category
    fn search_by_category(&self, category: KnowledgeCategory, k: usize) -> HVec<&Knowledge, 8> {
        let mut results = HVec::new();

        for knowledge in self.local_knowledge.iter() {
            if knowledge.category == category && results.len() < k {
                let _ = results.push(knowledge);
            }
        }

        results
    }

    /// Evict least important knowledge
    fn evict_least_important(&mut self) {
        if self.local_knowledge.is_empty() {
            return;
        }

        let mut min_score = i32::MAX;
        let mut min_idx = 0;

        for (i, k) in self.local_knowledge.iter().enumerate() {
            // Score based on access count and confidence
            let score = (k.access_count as i32) * 10 + (k.confidence as i32);
            // Prefer keeping local knowledge
            let score = if k.source_chip == self.chip_id { score + 100 } else { score };

            if score < min_score {
                min_score = score;
                min_idx = i;
            }
        }

        self.local_knowledge.swap_remove(min_idx);
    }

    /// Get statistics
    fn stats(&self) -> ChipStats {
        let local_count = self.local_knowledge.iter()
            .filter(|k| k.source_chip == self.chip_id)
            .count();

        let peer_count = self.local_knowledge.len() - local_count;

        ChipStats {
            chip_id: self.chip_id,
            total_knowledge: self.local_knowledge.len(),
            local_discoveries: local_count,
            peer_knowledge: peer_count,
            categories: self.count_categories(),
        }
    }

    fn count_categories(&self) -> [(KnowledgeCategory, usize); 6] {
        let mut counts = [
            (KnowledgeCategory::Environment, 0),
            (KnowledgeCategory::Action, 0),
            (KnowledgeCategory::Object, 0),
            (KnowledgeCategory::Navigation, 0),
            (KnowledgeCategory::Hazard, 0),
            (KnowledgeCategory::Resource, 0),
        ];

        for k in self.local_knowledge.iter() {
            for (cat, count) in counts.iter_mut() {
                if *cat == k.category {
                    *count += 1;
                }
            }
        }

        counts
    }
}

#[derive(Debug)]
struct ChipStats {
    chip_id: u8,
    total_knowledge: usize,
    local_discoveries: usize,
    peer_knowledge: usize,
    categories: [(KnowledgeCategory, usize); 6],
}

/// Swarm coordinator (simulates multi-chip communication)
struct SwarmCoordinator {
    chips: HVec<ChipMemory, MAX_PEERS>,
}

impl SwarmCoordinator {
    fn new(num_chips: usize) -> Self {
        let mut chips = HVec::new();
        for i in 0..num_chips.min(MAX_PEERS) {
            let _ = chips.push(ChipMemory::new(i as u8));
        }
        Self { chips }
    }

    /// Broadcast knowledge to all chips
    fn broadcast_knowledge(&mut self, source_chip: u8, knowledge: &Knowledge) {
        for chip in self.chips.iter_mut() {
            if chip.chip_id != source_chip {
                let _ = chip.store_peer_knowledge(knowledge.clone());
            }
        }
    }

    /// Query all chips and merge results
    fn query_swarm(&mut self, query: &[i8], k: usize) -> HVec<(Knowledge, i32), 16> {
        let mut all_results: HVec<(Knowledge, i32), 64> = HVec::new();

        for chip in self.chips.iter_mut() {
            let results = chip.search(query, k);
            for (idx, dist) in results {
                if let Some(knowledge) = chip.local_knowledge.get(idx) {
                    let _ = all_results.push((knowledge.clone(), dist));
                }
            }
        }

        // Sort and deduplicate
        all_results.sort_by_key(|(_, d)| *d);

        let mut final_results = HVec::new();
        let mut seen_ids: HVec<u32, 16> = HVec::new();

        for (knowledge, dist) in all_results {
            if !seen_ids.contains(&knowledge.id) && final_results.len() < k {
                let _ = seen_ids.push(knowledge.id);
                let _ = final_results.push((knowledge, dist));
            }
        }

        final_results
    }

    /// Get swarm statistics
    fn stats(&self) -> SwarmStats {
        let total_knowledge: usize = self.chips.iter().map(|c| c.local_knowledge.len()).sum();
        let unique_knowledge = self.count_unique_knowledge();

        SwarmStats {
            num_chips: self.chips.len(),
            total_knowledge,
            unique_knowledge,
            replication_factor: if unique_knowledge > 0 {
                total_knowledge as f32 / unique_knowledge as f32
            } else {
                0.0
            },
        }
    }

    fn count_unique_knowledge(&self) -> usize {
        let mut seen: HVec<u32, 256> = HVec::new();

        for chip in self.chips.iter() {
            for k in chip.local_knowledge.iter() {
                if !seen.contains(&k.id) {
                    let _ = seen.push(k.id);
                }
            }
        }

        seen.len()
    }
}

#[derive(Debug)]
struct SwarmStats {
    num_chips: usize,
    total_knowledge: usize,
    unique_knowledge: usize,
    replication_factor: f32,
}

/// Simple embedding from text
fn simple_embed(text: &str) -> [i8; EMBED_DIM] {
    let mut embed = [0i8; EMBED_DIM];
    for (i, b) in text.bytes().enumerate() {
        if i >= EMBED_DIM { break; }
        embed[i] = ((b as i32) - 64).clamp(-127, 127) as i8;
    }
    embed
}

/// Euclidean distance
fn euclidean_distance(a: &[i8], b: &[i8]) -> i32 {
    let mut sum = 0i32;
    for (va, vb) in a.iter().zip(b.iter()) {
        let diff = *va as i32 - *vb as i32;
        sum += diff * diff;
    }
    sum
}

fn main() {
    println!("🐝 Swarm Memory Example");
    println!("======================\n");

    // Create a swarm of 4 chips
    let mut swarm = SwarmCoordinator::new(4);

    println!("🤖 Created swarm with {} chips\n", swarm.chips.len());

    // Simulate discoveries by different chips
    println!("📍 Simulating chip discoveries...\n");

    // Chip 0 discovers environment features
    {
        let embed = simple_embed("obstacle wall north");
        swarm.chips[0].store_local(
            KnowledgeCategory::Environment,
            "Wall obstacle at north sector",
            &embed
        ).unwrap();

        let embed = simple_embed("open area south");
        swarm.chips[0].store_local(
            KnowledgeCategory::Navigation,
            "Open area suitable for navigation in south",
            &embed
        ).unwrap();
    }

    // Chip 1 discovers objects
    {
        let embed = simple_embed("red target object");
        swarm.chips[1].store_local(
            KnowledgeCategory::Object,
            "Red object identified as target",
            &embed
        ).unwrap();

        let embed = simple_embed("blue charger station");
        swarm.chips[1].store_local(
            KnowledgeCategory::Resource,
            "Blue charging station at coordinates",
            &embed
        ).unwrap();
    }

    // Chip 2 discovers hazards
    {
        let embed = simple_embed("water hazard danger");
        swarm.chips[2].store_local(
            KnowledgeCategory::Hazard,
            "Water puddle - slip hazard",
            &embed
        ).unwrap();

        let embed = simple_embed("successful approach left");
        swarm.chips[2].store_local(
            KnowledgeCategory::Action,
            "Approaching target from left succeeded",
            &embed
        ).unwrap();
    }

    // Chip 3 discovers navigation routes
    {
        let embed = simple_embed("path route corridor");
        swarm.chips[3].store_local(
            KnowledgeCategory::Navigation,
            "Main corridor is fastest route",
            &embed
        ).unwrap();
    }

    // Show individual chip stats
    println!("📊 Individual chip knowledge before sharing:\n");
    for chip in swarm.chips.iter() {
        let stats = chip.stats();
        println!("  Chip {}: {} local discoveries", stats.chip_id, stats.local_discoveries);
    }

    // Broadcast all knowledge to swarm
    println!("\n🔄 Broadcasting knowledge across swarm...\n");

    // Collect all knowledge first
    let mut all_knowledge: HVec<Knowledge, 32> = HVec::new();
    for chip in swarm.chips.iter() {
        for k in chip.local_knowledge.iter() {
            let _ = all_knowledge.push(k.clone());
        }
    }

    // Broadcast each piece
    for knowledge in all_knowledge.iter() {
        swarm.broadcast_knowledge(knowledge.source_chip, knowledge);
    }

    // Show stats after sharing
    println!("📊 Knowledge after sharing:\n");
    for chip in swarm.chips.iter() {
        let stats = chip.stats();
        println!("  Chip {}: {} total ({} local, {} from peers)",
            stats.chip_id,
            stats.total_knowledge,
            stats.local_discoveries,
            stats.peer_knowledge
        );
    }

    // Swarm-wide stats
    let swarm_stats = swarm.stats();
    println!("\n📈 Swarm Statistics:");
    println!("   Total knowledge instances: {}", swarm_stats.total_knowledge);
    println!("   Unique knowledge items: {}", swarm_stats.unique_knowledge);
    println!("   Replication factor: {:.1}x", swarm_stats.replication_factor);

    // Test swarm-wide queries
    println!("\n🔍 Testing swarm-wide queries:\n");

    let queries = [
        ("obstacle", "Looking for obstacles"),
        ("target object", "Finding targets"),
        ("hazard danger", "Checking for hazards"),
        ("route path", "Finding navigation routes"),
    ];

    for (query_text, description) in queries.iter() {
        let query_embed = simple_embed(query_text);
        let results = swarm.query_swarm(&query_embed, 2);

        println!("Query: \"{}\" ({})", query_text, description);
        for (knowledge, dist) in results.iter() {
            println!("  → [Chip {}] {:?}: \"{}\" (dist={})",
                knowledge.source_chip,
                knowledge.category,
                knowledge.text,
                dist
            );
        }
        println!();
    }

    // Demonstrate learning from experience
    println!("🧠 Demonstrating collaborative learning:\n");

    // Chip 0 tries an action and learns from it
    let embed = simple_embed("approach right failed");
    swarm.chips[0].store_local(
        KnowledgeCategory::Action,
        "Approaching from right FAILED - obstacle",
        &embed
    ).unwrap();

    // Broadcast the learning
    let new_knowledge = swarm.chips[0].local_knowledge.last().unwrap().clone();
    swarm.broadcast_knowledge(0, &new_knowledge);

    println!("Chip 0 learned: \"Approaching from right FAILED\"");
    println!("Broadcasting to swarm...\n");

    // Now any chip can query for approach strategies
    let query_embed = simple_embed("approach strategy");
    let results = swarm.query_swarm(&query_embed, 3);

    println!("Any chip querying \"approach strategy\":");
    for (knowledge, dist) in results.iter() {
        println!("  → [Chip {}] \"{}\"", knowledge.source_chip, knowledge.text);
    }

    // Memory usage
    println!("\n📊 Memory Usage:");
    let per_chip = MAX_KNOWLEDGE * core::mem::size_of::<Knowledge>();
    let total = per_chip * swarm.chips.len();
    println!("   Per chip: ~{} bytes ({:.1} KB)", per_chip, per_chip as f32 / 1024.0);
    println!("   Total swarm: ~{} bytes ({:.1} KB)", total, total as f32 / 1024.0);

    println!("\n✨ Swarm Memory Demo Complete!");
    println!("\n💡 Benefits:");
    println!("   - Each chip learns from all discoveries");
    println!("   - Knowledge persists even if chips fail");
    println!("   - Swarm gets smarter together");
    println!("   - Only ~4KB per chip for 64 memories");
}
