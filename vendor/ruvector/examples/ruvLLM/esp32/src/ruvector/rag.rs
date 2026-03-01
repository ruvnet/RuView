//! Micro RAG - Retrieval-Augmented Generation for ESP32
//!
//! Enables small language models to access external knowledge,
//! dramatically improving accuracy without larger models.
//!
//! # How RAG Works
//!
//! ```text
//! Question: "What's the capital of France?"
//!     │
//!     ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     MICRO RAG PIPELINE                      │
//! ├─────────────────────────────────────────────────────────────┤
//! │                                                             │
//! │  1. EMBED    Question ──▶ [0.2, 0.1, 0.8, ...]             │
//! │              │                                              │
//! │  2. SEARCH   ▼                                              │
//! │      ┌────────────────┐                                     │
//! │      │ Vector Index   │ ──▶ Top 3 relevant docs             │
//! │      │ (HNSW)         │                                     │
//! │      └────────────────┘                                     │
//! │              │                                              │
//! │  3. AUGMENT  ▼                                              │
//! │      Context: "France is a country in Europe.               │
//! │               Paris is the capital of France.               │
//! │               The Eiffel Tower is in Paris."                │
//! │              │                                              │
//! │  4. GENERATE ▼                                              │
//! │      ┌────────────────┐                                     │
//! │      │ Tiny LLM       │ ──▶ "Paris"                         │
//! │      └────────────────┘                                     │
//! │                                                             │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Benefits
//!
//! - 50K model + RAG ≈ 1M model accuracy for factual questions
//! - Knowledge can be updated without retraining
//! - Explainable: you can see which documents were used

use heapless::Vec as HVec;
use heapless::String as HString;
use super::{MicroHNSW, HNSWConfig, SearchResult, MicroVector, DistanceMetric};

/// Maximum documents in RAG index
pub const MAX_DOCUMENTS: usize = 256;
/// Maximum chunks per document
pub const MAX_CHUNKS: usize = 512;
/// Chunk embedding dimension
pub const CHUNK_DIM: usize = 32;
/// Maximum text per chunk
pub const MAX_CHUNK_TEXT: usize = 128;
/// Maximum context size for generation
pub const MAX_CONTEXT: usize = 256;

/// RAG Configuration
#[derive(Debug, Clone)]
pub struct RAGConfig {
    /// Number of documents to retrieve
    pub top_k: usize,
    /// Minimum similarity threshold (0-1000)
    pub min_similarity: i32,
    /// Maximum context tokens
    pub max_context_tokens: usize,
    /// Include source attribution
    pub include_sources: bool,
    /// Rerank retrieved documents
    pub enable_reranking: bool,
}

impl Default for RAGConfig {
    fn default() -> Self {
        Self {
            top_k: 3,
            min_similarity: 200, // Distance threshold
            max_context_tokens: 128,
            include_sources: true,
            enable_reranking: false,
        }
    }
}

/// A chunk of text with embedding
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Unique chunk ID
    pub id: u32,
    /// Parent document ID
    pub doc_id: u16,
    /// Chunk index within document
    pub chunk_idx: u8,
    /// Text content
    pub text: HString<MAX_CHUNK_TEXT>,
    /// Embedding
    pub embedding: HVec<i8, CHUNK_DIM>,
}

impl Chunk {
    /// Create new chunk
    pub fn new(id: u32, doc_id: u16, chunk_idx: u8, text: &str, embedding: &[i8]) -> Option<Self> {
        let mut text_str = HString::new();
        for c in text.chars().take(MAX_CHUNK_TEXT) {
            text_str.push(c).ok()?;
        }

        let mut embed = HVec::new();
        for &v in embedding.iter().take(CHUNK_DIM) {
            embed.push(v).ok()?;
        }

        Some(Self {
            id,
            doc_id,
            chunk_idx,
            text: text_str,
            embedding: embed,
        })
    }
}

/// RAG Result
#[derive(Debug)]
pub struct RAGResult {
    /// Retrieved context (concatenated chunks)
    pub context: HString<MAX_CONTEXT>,
    /// Source chunk IDs
    pub source_ids: HVec<u32, 8>,
    /// Relevance scores
    pub scores: HVec<i32, 8>,
    /// Whether context is truncated
    pub truncated: bool,
}

/// Micro RAG Engine
pub struct MicroRAG {
    /// Configuration
    config: RAGConfig,
    /// HNSW index for chunk retrieval
    index: MicroHNSW<CHUNK_DIM, MAX_CHUNKS>,
    /// Stored chunks
    chunks: HVec<Chunk, MAX_CHUNKS>,
    /// Document count
    doc_count: u16,
    /// Next chunk ID
    next_chunk_id: u32,
}

impl MicroRAG {
    /// Create new RAG engine
    pub fn new(config: RAGConfig) -> Self {
        let hnsw_config = HNSWConfig {
            m: 6,
            m_max0: 12,
            ef_construction: 24,
            ef_search: 16,
            metric: DistanceMetric::Euclidean,
            binary_mode: false,
        };

        Self {
            config,
            index: MicroHNSW::new(hnsw_config),
            chunks: HVec::new(),
            doc_count: 0,
            next_chunk_id: 0,
        }
    }

    /// Number of indexed chunks
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Number of documents
    pub fn doc_count(&self) -> u16 {
        self.doc_count
    }

    /// Memory usage in bytes
    pub fn memory_bytes(&self) -> usize {
        self.index.memory_bytes() + self.chunks.len() * core::mem::size_of::<Chunk>()
    }

    /// Add a document (split into chunks)
    pub fn add_document(&mut self, chunks: &[(&str, &[i8])]) -> Result<u16, &'static str> {
        let doc_id = self.doc_count;
        self.doc_count += 1;

        for (idx, (text, embedding)) in chunks.iter().enumerate() {
            if self.chunks.len() >= MAX_CHUNKS {
                return Err("Chunk limit reached");
            }

            let chunk_id = self.next_chunk_id;
            self.next_chunk_id += 1;

            let chunk = Chunk::new(chunk_id, doc_id, idx as u8, text, embedding)
                .ok_or("Failed to create chunk")?;

            // Add to HNSW index
            let vec = MicroVector {
                data: chunk.embedding.clone(),
                id: chunk_id,
            };
            self.index.insert(&vec)?;

            // Store chunk
            self.chunks.push(chunk).map_err(|_| "Chunk storage full")?;
        }

        Ok(doc_id)
    }

    /// Add a single pre-chunked piece of knowledge
    pub fn add_knowledge(&mut self, text: &str, embedding: &[i8]) -> Result<u32, &'static str> {
        if self.chunks.len() >= MAX_CHUNKS {
            return Err("Chunk limit reached");
        }

        let chunk_id = self.next_chunk_id;
        self.next_chunk_id += 1;

        let chunk = Chunk::new(chunk_id, self.doc_count, 0, text, embedding)
            .ok_or("Failed to create chunk")?;

        let vec = MicroVector {
            data: chunk.embedding.clone(),
            id: chunk_id,
        };
        self.index.insert(&vec)?;
        self.chunks.push(chunk).map_err(|_| "Chunk storage full")?;

        self.doc_count += 1;
        Ok(chunk_id)
    }

    /// Retrieve relevant context for a query
    pub fn retrieve(&self, query_embedding: &[i8]) -> RAGResult {
        let search_results = self.index.search(query_embedding, self.config.top_k * 2);

        let mut context = HString::new();
        let mut source_ids = HVec::new();
        let mut scores = HVec::new();
        let mut truncated = false;

        let mut added = 0;
        for result in search_results.iter() {
            // Check similarity threshold
            if result.distance > self.config.min_similarity && added > 0 {
                continue;
            }

            if let Some(chunk) = self.find_chunk_by_id(result.id) {
                // Check if we have room
                if context.len() + chunk.text.len() + 2 > MAX_CONTEXT {
                    if added > 0 {
                        truncated = true;
                        break;
                    }
                }

                // Add separator
                if !context.is_empty() {
                    let _ = context.push_str(" | ");
                }

                // Add chunk text
                for c in chunk.text.chars() {
                    if context.push(c).is_err() {
                        truncated = true;
                        break;
                    }
                }

                let _ = source_ids.push(result.id);
                let _ = scores.push(result.distance);
                added += 1;

                if added >= self.config.top_k {
                    break;
                }
            }
        }

        RAGResult {
            context,
            source_ids,
            scores,
            truncated,
        }
    }

    /// Retrieve and format for LLM prompt
    pub fn retrieve_prompt(&self, query_embedding: &[i8], question: &str) -> HString<512> {
        let rag_result = self.retrieve(query_embedding);

        let mut prompt = HString::new();

        // Add context
        let _ = prompt.push_str("Context: ");
        for c in rag_result.context.chars() {
            let _ = prompt.push(c);
        }
        let _ = prompt.push_str("\n\nQuestion: ");
        for c in question.chars().take(128) {
            let _ = prompt.push(c);
        }
        let _ = prompt.push_str("\n\nAnswer: ");

        prompt
    }

    /// Find chunk by ID
    fn find_chunk_by_id(&self, id: u32) -> Option<&Chunk> {
        self.chunks.iter().find(|c| c.id == id)
    }

    /// Get all chunks for a document
    pub fn get_document_chunks(&self, doc_id: u16) -> HVec<&Chunk, 16> {
        let mut result = HVec::new();
        for chunk in self.chunks.iter() {
            if chunk.doc_id == doc_id {
                let _ = result.push(chunk);
            }
        }
        result.sort_by_key(|c| c.chunk_idx);
        result
    }
}

impl Default for MicroRAG {
    fn default() -> Self {
        Self::new(RAGConfig::default())
    }
}

/// Helper: Simple text chunker for preprocessing
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> HVec<HString<MAX_CHUNK_TEXT>, 16> {
    let mut chunks = HVec::new();
    let chars: HVec<char, 1024> = text.chars().collect();

    let mut start = 0;
    while start < chars.len() {
        let end = (start + chunk_size).min(chars.len());

        let mut chunk = HString::new();
        for &c in chars[start..end].iter() {
            let _ = chunk.push(c);
        }

        if !chunk.is_empty() {
            let _ = chunks.push(chunk);
        }

        if end >= chars.len() {
            break;
        }

        start = end.saturating_sub(overlap);
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rag_basic() {
        let mut rag = MicroRAG::default();

        // Add knowledge
        let embed1 = [10i8; CHUNK_DIM];
        let embed2 = [20i8; CHUNK_DIM];

        rag.add_knowledge("Paris is the capital of France", &embed1).unwrap();
        rag.add_knowledge("London is the capital of UK", &embed2).unwrap();

        assert_eq!(rag.chunk_count(), 2);
    }

    #[test]
    fn test_rag_retrieve() {
        let mut rag = MicroRAG::default();

        let embed1 = [10i8; CHUNK_DIM];
        let embed2 = [50i8; CHUNK_DIM];

        rag.add_knowledge("The sky is blue", &embed1).unwrap();
        rag.add_knowledge("Grass is green", &embed2).unwrap();

        // Query similar to first
        let query = [11i8; CHUNK_DIM];
        let result = rag.retrieve(&query);

        assert!(!result.context.is_empty());
        assert!(!result.source_ids.is_empty());
    }

    #[test]
    fn test_chunk_text() {
        let text = "Hello world this is a test";
        let chunks = chunk_text(text, 10, 3);
        assert!(!chunks.is_empty());
    }
}
