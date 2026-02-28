//! Voice Disambiguation Example - Context-Aware Speech Understanding
//!
//! Demonstrates using RuVector semantic memory for disambiguating
//! voice commands on ESP32 voice assistants.
//!
//! # Problem
//! "Turn on the light" - which light?
//! "Play that song" - which song?
//! "Call him" - who?
//!
//! # Solution
//! Use semantic memory to track context and resolve ambiguity.

#![allow(unused)]

use heapless::Vec as HVec;
use heapless::String as HString;

const EMBED_DIM: usize = 32;
const MAX_CONTEXT: usize = 32;
const MAX_ENTITIES: usize = 64;

/// Entity that can be referenced
#[derive(Debug, Clone)]
struct Entity {
    id: u32,
    name: HString<32>,
    entity_type: EntityType,
    aliases: HVec<HString<16>, 4>,
    embedding: [i8; EMBED_DIM],
    /// Recent mention score (higher = more recently mentioned)
    recency: u16,
    /// Total mentions
    mention_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum EntityType {
    Person,
    Device,
    Location,
    Song,
    Playlist,
    Contact,
    Setting,
}

/// Context entry for conversation tracking
#[derive(Debug, Clone)]
struct ContextEntry {
    text: HString<64>,
    entities_mentioned: HVec<u32, 4>,
    timestamp: u32,
    embedding: [i8; EMBED_DIM],
}

/// Disambiguation result
#[derive(Debug)]
struct DisambiguationResult {
    resolved_entity: Option<Entity>,
    confidence: u8,
    candidates: HVec<(Entity, u8), 4>,  // (entity, score)
    needs_clarification: bool,
    clarification_prompt: Option<HString<64>>,
}

/// Voice Disambiguator using Semantic Memory
struct VoiceDisambiguator {
    entities: HVec<Entity, MAX_ENTITIES>,
    context: HVec<ContextEntry, MAX_CONTEXT>,
    next_entity_id: u32,
    current_time: u32,
}

impl VoiceDisambiguator {
    fn new() -> Self {
        Self {
            entities: HVec::new(),
            context: HVec::new(),
            next_entity_id: 0,
            current_time: 0,
        }
    }

    /// Register an entity
    fn register_entity(&mut self, name: &str, entity_type: EntityType, aliases: &[&str]) -> Result<u32, &'static str> {
        if self.entities.len() >= MAX_ENTITIES {
            return Err("Entity limit reached");
        }

        let id = self.next_entity_id;
        self.next_entity_id += 1;

        let mut name_str = HString::new();
        for c in name.chars().take(32) {
            name_str.push(c).map_err(|_| "Name overflow")?;
        }

        let mut alias_vec = HVec::new();
        for alias in aliases.iter().take(4) {
            let mut a = HString::new();
            for c in alias.chars().take(16) {
                let _ = a.push(c);
            }
            let _ = alias_vec.push(a);
        }

        let embedding = self.embed_text(name);

        let entity = Entity {
            id,
            name: name_str,
            entity_type,
            aliases: alias_vec,
            embedding,
            recency: 0,
            mention_count: 0,
        };

        self.entities.push(entity).map_err(|_| "Storage full")?;
        Ok(id)
    }

    /// Add context from conversation
    fn add_context(&mut self, text: &str, mentioned_entity_ids: &[u32]) {
        self.current_time += 1;

        // Update recency for mentioned entities
        for &id in mentioned_entity_ids {
            if let Some(entity) = self.entities.iter_mut().find(|e| e.id == id) {
                entity.recency = 1000;
                entity.mention_count += 1;
            }
        }

        // Decay recency for all entities
        for entity in self.entities.iter_mut() {
            entity.recency = entity.recency.saturating_sub(50);
        }

        // Add context entry
        if self.context.len() >= MAX_CONTEXT {
            self.context.remove(0);
        }

        let mut text_str = HString::new();
        for c in text.chars().take(64) {
            let _ = text_str.push(c);
        }

        let mut entities_mentioned = HVec::new();
        for &id in mentioned_entity_ids.iter().take(4) {
            let _ = entities_mentioned.push(id);
        }

        let embedding = self.embed_text(text);

        let entry = ContextEntry {
            text: text_str,
            entities_mentioned,
            timestamp: self.current_time,
            embedding,
        };

        let _ = self.context.push(entry);
    }

    /// Disambiguate a reference
    fn disambiguate(&self, reference: &str, expected_type: Option<EntityType>) -> DisambiguationResult {
        let ref_embed = self.embed_text(reference);

        // Score all matching entities
        let mut candidates: HVec<(Entity, u8), MAX_ENTITIES> = HVec::new();

        for entity in self.entities.iter() {
            // Type filter
            if let Some(etype) = expected_type {
                if entity.entity_type != etype {
                    continue;
                }
            }

            // Calculate match score
            let mut score = 0u16;

            // Embedding similarity
            let dist = euclidean_distance(&ref_embed, &entity.embedding);
            let similarity_score = (1000u16).saturating_sub(dist as u16).min(100);
            score += similarity_score;

            // Recency bonus
            score += entity.recency / 10;

            // Mention count bonus
            score += (entity.mention_count as u16).min(50);

            // Context bonus - check if mentioned recently
            for ctx in self.context.iter().rev().take(5) {
                if ctx.entities_mentioned.contains(&entity.id) {
                    score += 100;
                    break;
                }
            }

            // Name/alias match bonus
            let ref_lower = reference.to_lowercase();
            let name_lower = entity.name.to_lowercase();

            if name_lower.contains(&ref_lower) || ref_lower.contains(&name_lower.as_str()) {
                score += 200;
            }

            for alias in entity.aliases.iter() {
                if alias.to_lowercase().contains(&ref_lower) {
                    score += 150;
                }
            }

            let _ = candidates.push((entity.clone(), score.min(255) as u8));
        }

        // Sort by score
        candidates.sort_by(|a, b| b.1.cmp(&a.1));

        // Take top 4
        let mut top_candidates = HVec::new();
        for (entity, score) in candidates.iter().take(4) {
            let _ = top_candidates.push((entity.clone(), *score));
        }

        // Determine result
        if top_candidates.is_empty() {
            let mut prompt = HString::new();
            let _ = prompt.push_str("I don't know what you're referring to.");
            return DisambiguationResult {
                resolved_entity: None,
                confidence: 0,
                candidates: top_candidates,
                needs_clarification: true,
                clarification_prompt: Some(prompt),
            };
        }

        let best = &top_candidates[0];

        // Check if clear winner
        let has_runner_up = top_candidates.len() > 1;
        let score_gap = if has_runner_up {
            best.1 as i16 - top_candidates[1].1 as i16
        } else {
            100
        };

        if best.1 >= 150 && score_gap > 30 {
            // Clear winner
            DisambiguationResult {
                resolved_entity: Some(best.0.clone()),
                confidence: best.1,
                candidates: top_candidates,
                needs_clarification: false,
                clarification_prompt: None,
            }
        } else if best.1 >= 80 {
            // Possible match, might need clarification
            let mut prompt = HString::new();
            let _ = prompt.push_str("Did you mean ");
            for c in best.0.name.chars() {
                let _ = prompt.push(c);
            }
            let _ = prompt.push_str("?");

            DisambiguationResult {
                resolved_entity: Some(best.0.clone()),
                confidence: best.1,
                candidates: top_candidates,
                needs_clarification: score_gap < 20,
                clarification_prompt: if score_gap < 20 { Some(prompt) } else { None },
            }
        } else {
            // Need clarification
            let mut prompt = HString::new();
            let _ = prompt.push_str("Which one: ");
            for (i, (entity, _)) in top_candidates.iter().take(3).enumerate() {
                if i > 0 {
                    let _ = prompt.push_str(", ");
                }
                for c in entity.name.chars().take(15) {
                    let _ = prompt.push(c);
                }
            }
            let _ = prompt.push_str("?");

            DisambiguationResult {
                resolved_entity: None,
                confidence: best.1,
                candidates: top_candidates,
                needs_clarification: true,
                clarification_prompt: Some(prompt),
            }
        }
    }

    /// Simple text embedding
    fn embed_text(&self, text: &str) -> [i8; EMBED_DIM] {
        let mut embed = [0i8; EMBED_DIM];
        let text_lower = text.to_lowercase();

        // Keyword features
        if text_lower.contains("light") || text_lower.contains("lamp") {
            embed[0] = 100;
        }
        if text_lower.contains("music") || text_lower.contains("song") || text_lower.contains("play") {
            embed[1] = 100;
        }
        if text_lower.contains("call") || text_lower.contains("phone") {
            embed[2] = 100;
        }
        if text_lower.contains("room") || text_lower.contains("kitchen") || text_lower.contains("bedroom") {
            embed[3] = 100;
        }

        // Character features
        for (i, b) in text.bytes().enumerate() {
            if 4 + (i % 28) < EMBED_DIM {
                embed[4 + (i % 28)] = ((b as i32) - 64).clamp(-127, 127) as i8;
            }
        }

        embed
    }
}

fn euclidean_distance(a: &[i8], b: &[i8]) -> i32 {
    let mut sum = 0i32;
    for (va, vb) in a.iter().zip(b.iter()) {
        let diff = *va as i32 - *vb as i32;
        sum += diff * diff;
    }
    sum
}

fn main() {
    println!("🎤 Voice Disambiguation Example");
    println!("===============================\n");

    let mut disambiguator = VoiceDisambiguator::new();

    // Register entities
    println!("📝 Registering entities...\n");

    // People
    let mom_id = disambiguator.register_entity("Mom", EntityType::Person, &["mother", "mama"]).unwrap();
    let dad_id = disambiguator.register_entity("Dad", EntityType::Person, &["father", "papa"]).unwrap();
    let john_id = disambiguator.register_entity("John Smith", EntityType::Person, &["john", "johnny"]).unwrap();
    let jane_id = disambiguator.register_entity("Jane Doe", EntityType::Person, &["jane"]).unwrap();

    // Devices
    let living_light_id = disambiguator.register_entity("Living room light", EntityType::Device, &["living light", "main light"]).unwrap();
    let bedroom_light_id = disambiguator.register_entity("Bedroom light", EntityType::Device, &["bed light"]).unwrap();
    let kitchen_light_id = disambiguator.register_entity("Kitchen light", EntityType::Device, &["kitchen"]).unwrap();
    let porch_light_id = disambiguator.register_entity("Porch light", EntityType::Device, &["front light", "outside light"]).unwrap();

    // Songs
    let song1_id = disambiguator.register_entity("Bohemian Rhapsody", EntityType::Song, &["bohemian", "queen song"]).unwrap();
    let song2_id = disambiguator.register_entity("Hotel California", EntityType::Song, &["hotel", "eagles"]).unwrap();
    let song3_id = disambiguator.register_entity("Stairway to Heaven", EntityType::Song, &["stairway", "zeppelin"]).unwrap();

    println!("✅ Registered {} entities\n", disambiguator.entities.len());

    // Test disambiguation scenarios
    println!("🔍 Testing disambiguation:\n");

    // Scenario 1: Ambiguous reference without context
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Command: \"Turn on the light\"");
    println!("Context: None\n");

    let result = disambiguator.disambiguate("the light", Some(EntityType::Device));
    print_result(&result);

    // Scenario 2: Add context, then retry
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("User: \"I'm going to the kitchen\"");
    disambiguator.add_context("I'm going to the kitchen", &[kitchen_light_id]);

    println!("Command: \"Turn on the light\"");
    println!("Context: Kitchen was mentioned\n");

    let result = disambiguator.disambiguate("the light", Some(EntityType::Device));
    print_result(&result);

    // Scenario 3: Person disambiguation
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Command: \"Call him\"");
    println!("Context: None\n");

    let result = disambiguator.disambiguate("him", Some(EntityType::Person));
    print_result(&result);

    // Add context about John
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("User: \"I need to talk to John about the project\"");
    disambiguator.add_context("I need to talk to John about the project", &[john_id]);

    println!("Command: \"Call him\"");
    println!("Context: John was just mentioned\n");

    let result = disambiguator.disambiguate("him", Some(EntityType::Person));
    print_result(&result);

    // Scenario 4: Song disambiguation
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Command: \"Play that Queen song\"");

    let result = disambiguator.disambiguate("queen song", Some(EntityType::Song));
    print_result(&result);

    // Scenario 5: Direct name match
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Command: \"Turn on the porch light\"");

    let result = disambiguator.disambiguate("porch light", Some(EntityType::Device));
    print_result(&result);

    // Scenario 6: Alias match
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Command: \"Call mama\"");

    let result = disambiguator.disambiguate("mama", Some(EntityType::Person));
    print_result(&result);

    // Show context window
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\n📜 Current Context Window:\n");
    for (i, ctx) in disambiguator.context.iter().enumerate() {
        println!("   {}: \"{}\"", i + 1, ctx.text);
    }

    // Memory stats
    println!("\n📊 Memory Usage:");
    let entity_mem = disambiguator.entities.len() * core::mem::size_of::<Entity>();
    let context_mem = disambiguator.context.len() * core::mem::size_of::<ContextEntry>();
    let total = entity_mem + context_mem;
    println!("   Entities: {} bytes", entity_mem);
    println!("   Context: {} bytes", context_mem);
    println!("   Total: {} bytes ({:.1} KB)", total, total as f32 / 1024.0);

    println!("\n✨ Voice Disambiguation Demo Complete!");
    println!("\n💡 Key Benefits:");
    println!("   - Resolves ambiguous references using context");
    println!("   - Tracks conversation history for better understanding");
    println!("   - Supports aliases and partial matches");
    println!("   - Perfect for ESP32 voice assistants");
}

fn print_result(result: &DisambiguationResult) {
    if let Some(ref entity) = result.resolved_entity {
        println!("✅ Resolved: {} ({:?})", entity.name, entity.entity_type);
        println!("   Confidence: {}%", result.confidence);
    } else {
        println!("❓ Could not resolve");
    }

    if result.needs_clarification {
        if let Some(ref prompt) = result.clarification_prompt {
            println!("   🔊 Assistant: \"{}\"", prompt);
        }
    }

    if !result.candidates.is_empty() {
        println!("   Candidates:");
        for (entity, score) in result.candidates.iter().take(3) {
            println!("      - {} (score: {})", entity.name, score);
        }
    }
    println!();
}
