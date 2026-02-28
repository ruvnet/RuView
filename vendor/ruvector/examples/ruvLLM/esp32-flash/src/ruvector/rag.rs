//! Micro RAG - Retrieval-Augmented Generation for ESP32

use heapless::Vec as HVec;
use heapless::String as HString;
use super::{MicroHNSW, HNSWConfig, MicroVector, DistanceMetric, SearchResult};

pub const MAX_KNOWLEDGE_ENTRIES: usize = 64;
pub const MAX_DOC_LEN: usize = 128;
pub const RAG_DIM: usize = 32;

#[derive(Debug, Clone)]
pub struct RAGConfig {
    pub top_k: usize,
    pub relevance_threshold: i32,
    pub max_context_tokens: usize,
    pub rerank: bool,
}

impl Default for RAGConfig {
    fn default() -> Self {
        Self { top_k: 3, relevance_threshold: 500, max_context_tokens: 256, rerank: true }
    }
}

#[derive(Debug, Clone)]
pub struct KnowledgeEntry {
    pub id: u32,
    pub text: HString<MAX_DOC_LEN>,
    pub embedding: HVec<i8, RAG_DIM>,
    pub source: HString<32>,
    pub importance: u8,
}

#[derive(Debug, Clone)]
pub struct RAGResult {
    pub entries: HVec<(KnowledgeEntry, i32), 8>,
    pub context: HString<256>,
    pub confidence: u8,
}

pub struct MicroRAG {
    config: RAGConfig,
    index: MicroHNSW<RAG_DIM, MAX_KNOWLEDGE_ENTRIES>,
    entries: HVec<KnowledgeEntry, MAX_KNOWLEDGE_ENTRIES>,
    next_id: u32,
}

impl MicroRAG {
    pub fn new(config: RAGConfig) -> Self {
        let hnsw_config = HNSWConfig { m: 4, m_max0: 8, ef_construction: 16, ef_search: 8, metric: DistanceMetric::Euclidean, binary_mode: false };
        Self { config, index: MicroHNSW::new(hnsw_config), entries: HVec::new(), next_id: 0 }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn add_knowledge(&mut self, text: &str, embedding: &[i8], source: &str, importance: u8) -> Result<u32, &'static str> {
        if self.entries.len() >= MAX_KNOWLEDGE_ENTRIES { return Err("Knowledge base full"); }

        let id = self.next_id;
        self.next_id += 1;

        let mut text_str = HString::new();
        for c in text.chars().take(MAX_DOC_LEN) { text_str.push(c).ok().ok_or("Text too long")?; }

        let mut embed_vec = HVec::new();
        for &v in embedding.iter().take(RAG_DIM) { embed_vec.push(v).ok().ok_or("Embedding too large")?; }

        let mut source_str = HString::new();
        for c in source.chars().take(32) { source_str.push(c).ok().ok_or("Source too long")?; }

        let entry = KnowledgeEntry { id, text: text_str, embedding: embed_vec.clone(), source: source_str, importance };
        let vec = MicroVector { data: embed_vec, id };
        self.index.insert(&vec)?;
        self.entries.push(entry).map_err(|_| "Entries full")?;
        Ok(id)
    }

    pub fn retrieve(&self, query_embedding: &[i8]) -> RAGResult {
        let results = self.index.search(query_embedding, self.config.top_k * 2);
        let mut entries: HVec<(KnowledgeEntry, i32), 8> = HVec::new();

        for result in results.iter() {
            if result.distance > self.config.relevance_threshold { continue; }
            if let Some(entry) = self.entries.iter().find(|e| e.id == result.id) {
                let score = self.compute_score(result.distance, entry.importance);
                let _ = entries.push((entry.clone(), score));
            }
        }

        if self.config.rerank {
            entries.sort_by(|a, b| b.1.cmp(&a.1));
        }
        while entries.len() > self.config.top_k { entries.pop(); }

        let context = self.build_context(&entries);
        let confidence = self.compute_confidence(&entries);

        RAGResult { entries, context, confidence }
    }

    pub fn query(&self, query_embedding: &[i8]) -> Option<&str> {
        let results = self.index.search(query_embedding, 1);
        if let Some(result) = results.first() {
            if result.distance <= self.config.relevance_threshold {
                return self.entries.iter().find(|e| e.id == result.id).map(|e| e.text.as_str());
            }
        }
        None
    }

    fn compute_score(&self, distance: i32, importance: u8) -> i32 {
        let dist_score = 1000 - distance.min(1000);
        let imp_score = importance as i32 * 4;
        (dist_score * 3 + imp_score) / 4
    }

    fn build_context(&self, entries: &HVec<(KnowledgeEntry, i32), 8>) -> HString<256> {
        let mut ctx = HString::new();
        for (entry, _) in entries.iter().take(3) {
            if ctx.len() + entry.text.len() + 2 > 256 { break; }
            for c in entry.text.chars() { let _ = ctx.push(c); }
            let _ = ctx.push(' ');
        }
        ctx
    }

    fn compute_confidence(&self, entries: &HVec<(KnowledgeEntry, i32), 8>) -> u8 {
        if entries.is_empty() { return 0; }
        let avg_score: i32 = entries.iter().map(|(_, s)| *s).sum::<i32>() / entries.len() as i32;
        ((avg_score * 255) / 1000).clamp(0, 255) as u8
    }

    pub fn remove(&mut self, id: u32) -> bool {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            self.entries.swap_remove(pos);
            true
        } else { false }
    }
}

impl Default for MicroRAG { fn default() -> Self { Self::new(RAGConfig::default()) } }
