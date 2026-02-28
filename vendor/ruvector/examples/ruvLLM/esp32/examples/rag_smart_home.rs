//! Smart Home RAG Example - Voice Assistant with Knowledge Base
//!
//! Demonstrates using RuVector RAG on ESP32 for a smart home assistant
//! that can answer questions about devices, schedules, and preferences.
//!
//! # Use Case
//! - "What time do I usually wake up?"
//! - "What's the temperature in the bedroom?"
//! - "When does the dishwasher usually run?"

#![allow(unused)]

use heapless::Vec as HVec;
use heapless::String as HString;

// Simulated imports (would use actual ruvector module)
const CHUNK_DIM: usize = 32;

/// Simple embedding generator for demonstration
/// In production, use a proper embedding model
fn simple_embed(text: &str) -> [i8; CHUNK_DIM] {
    let mut embedding = [0i8; CHUNK_DIM];
    let bytes = text.as_bytes();

    for (i, chunk) in bytes.chunks(4).enumerate() {
        if i >= CHUNK_DIM { break; }
        let sum: i32 = chunk.iter().map(|&b| b as i32).sum();
        embedding[i] = ((sum % 256) - 128) as i8;
    }

    // Add semantic features based on keywords
    if text.contains("wake") || text.contains("morning") {
        embedding[0] = 100;
    }
    if text.contains("temperature") || text.contains("temp") {
        embedding[1] = 100;
    }
    if text.contains("light") || text.contains("lamp") {
        embedding[2] = 100;
    }
    if text.contains("time") || text.contains("schedule") {
        embedding[3] = 100;
    }

    embedding
}

/// Smart Home Knowledge Entry
#[derive(Debug, Clone)]
struct KnowledgeEntry {
    id: u32,
    text: HString<128>,
    embedding: [i8; CHUNK_DIM],
    category: KnowledgeCategory,
}

#[derive(Debug, Clone, Copy)]
enum KnowledgeCategory {
    Schedule,
    DeviceState,
    Preference,
    Location,
    Automation,
}

/// Micro RAG for Smart Home
struct SmartHomeRAG {
    knowledge: HVec<KnowledgeEntry, 256>,
    next_id: u32,
}

impl SmartHomeRAG {
    fn new() -> Self {
        Self {
            knowledge: HVec::new(),
            next_id: 0,
        }
    }

    /// Add knowledge to the system
    fn add_knowledge(&mut self, text: &str, category: KnowledgeCategory) -> Result<u32, &'static str> {
        if self.knowledge.len() >= 256 {
            return Err("Knowledge base full");
        }

        let id = self.next_id;
        self.next_id += 1;

        let mut text_str = HString::new();
        for c in text.chars().take(128) {
            text_str.push(c).map_err(|_| "Text too long")?;
        }

        let embedding = simple_embed(text);

        let entry = KnowledgeEntry {
            id,
            text: text_str,
            embedding,
            category,
        };

        self.knowledge.push(entry).map_err(|_| "Storage full")?;
        Ok(id)
    }

    /// Search for relevant knowledge
    fn search(&self, query: &str, k: usize) -> HVec<(&KnowledgeEntry, i32), 8> {
        let query_embed = simple_embed(query);

        // Calculate distances
        let mut results: HVec<(&KnowledgeEntry, i32), 256> = HVec::new();

        for entry in self.knowledge.iter() {
            let dist = euclidean_distance(&query_embed, &entry.embedding);
            let _ = results.push((entry, dist));
        }

        // Sort by distance
        results.sort_by_key(|(_, d)| *d);

        // Return top k
        let mut top_k = HVec::new();
        for (entry, dist) in results.iter().take(k) {
            let _ = top_k.push((*entry, *dist));
        }

        top_k
    }

    /// Answer a question using RAG
    fn answer(&self, question: &str) -> HString<256> {
        let results = self.search(question, 3);

        let mut answer = HString::new();

        if results.is_empty() {
            let _ = answer.push_str("I don't have information about that.");
            return answer;
        }

        // Build context from retrieved knowledge
        let _ = answer.push_str("Based on what I know: ");

        for (i, (entry, dist)) in results.iter().enumerate() {
            if *dist > 500 { break; } // Skip low relevance

            if i > 0 {
                let _ = answer.push_str(" Also, ");
            }

            // Add relevant info (truncated to fit)
            for c in entry.text.chars().take(60) {
                if answer.len() >= 250 { break; }
                let _ = answer.push(c);
            }
        }

        answer
    }
}

/// Simple Euclidean distance
fn euclidean_distance(a: &[i8], b: &[i8]) -> i32 {
    let mut sum = 0i32;
    for (va, vb) in a.iter().zip(b.iter()) {
        let diff = *va as i32 - *vb as i32;
        sum += diff * diff;
    }
    sum
}

fn main() {
    println!("🏠 Smart Home RAG Example");
    println!("========================\n");

    // Create RAG system
    let mut rag = SmartHomeRAG::new();

    // Add smart home knowledge
    println!("📚 Loading smart home knowledge...\n");

    // Schedules
    rag.add_knowledge(
        "Wake up alarm is set for 6:30 AM on weekdays",
        KnowledgeCategory::Schedule
    ).unwrap();
    rag.add_knowledge(
        "Bedtime routine starts at 10:00 PM",
        KnowledgeCategory::Schedule
    ).unwrap();
    rag.add_knowledge(
        "Dishwasher runs automatically at 2:00 AM",
        KnowledgeCategory::Schedule
    ).unwrap();

    // Device states
    rag.add_knowledge(
        "Living room temperature is set to 72°F",
        KnowledgeCategory::DeviceState
    ).unwrap();
    rag.add_knowledge(
        "Bedroom lights are currently off",
        KnowledgeCategory::DeviceState
    ).unwrap();
    rag.add_knowledge(
        "Front door is locked",
        KnowledgeCategory::DeviceState
    ).unwrap();

    // Preferences
    rag.add_knowledge(
        "User prefers cooler temperatures at night (68°F)",
        KnowledgeCategory::Preference
    ).unwrap();
    rag.add_knowledge(
        "Morning coffee is preferred at 7:00 AM",
        KnowledgeCategory::Preference
    ).unwrap();

    // Automations
    rag.add_knowledge(
        "Lights automatically dim at sunset",
        KnowledgeCategory::Automation
    ).unwrap();
    rag.add_knowledge(
        "HVAC switches to eco mode when no one is home",
        KnowledgeCategory::Automation
    ).unwrap();

    println!("✅ Loaded {} knowledge entries\n", rag.knowledge.len());

    // Test queries
    let queries = [
        "What time do I wake up?",
        "What's the temperature?",
        "When does the dishwasher run?",
        "What are my light settings?",
        "Tell me about my morning routine",
    ];

    println!("🔍 Testing queries:\n");

    for query in queries.iter() {
        println!("Q: {}", query);

        let answer = rag.answer(query);
        println!("A: {}\n", answer);

        // Show retrieved sources
        let results = rag.search(query, 2);
        print!("   Sources: ");
        for (entry, dist) in results.iter() {
            print!("[{:?} d={}] ", entry.category, dist);
        }
        println!("\n");
    }

    // Memory usage
    let mem_bytes = rag.knowledge.len() * core::mem::size_of::<KnowledgeEntry>();
    println!("📊 Memory Usage:");
    println!("   Knowledge entries: {}", rag.knowledge.len());
    println!("   Approximate size: {} bytes ({:.1} KB)", mem_bytes, mem_bytes as f32 / 1024.0);
    println!("   Per entry: {} bytes", core::mem::size_of::<KnowledgeEntry>());

    println!("\n✨ Smart Home RAG Demo Complete!");
    println!("\n💡 On ESP32:");
    println!("   - Can store ~200+ knowledge entries in 64KB");
    println!("   - Answers questions in <10ms");
    println!("   - Perfect for voice assistants");
}
