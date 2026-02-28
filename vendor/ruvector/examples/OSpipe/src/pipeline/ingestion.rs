//! Main ingestion pipeline.

use crate::capture::CapturedFrame;
use crate::config::OsPipeConfig;
use crate::error::Result;
use crate::graph::KnowledgeGraph;
use crate::pipeline::dedup::FrameDeduplicator;
use crate::safety::{SafetyDecision, SafetyGate};
use crate::search::enhanced::EnhancedSearch;
use crate::storage::embedding::EmbeddingEngine;
use crate::storage::vector_store::{SearchResult, VectorStore};
use uuid::Uuid;

/// Result of ingesting a single frame.
#[derive(Debug, Clone)]
pub enum IngestResult {
    /// The frame was successfully stored.
    Stored {
        /// ID of the stored frame.
        id: Uuid,
    },
    /// The frame was deduplicated (not stored).
    Deduplicated {
        /// ID of the existing similar frame.
        similar_to: Uuid,
        /// Cosine similarity score with the existing frame.
        similarity: f32,
    },
    /// The frame was denied by the safety gate.
    Denied {
        /// Reason for denial.
        reason: String,
    },
}

/// Statistics about the ingestion pipeline.
#[derive(Debug, Clone, Default)]
pub struct PipelineStats {
    /// Total frames successfully ingested.
    pub total_ingested: u64,
    /// Total frames deduplicated.
    pub total_deduplicated: u64,
    /// Total frames denied by safety gate.
    pub total_denied: u64,
    /// Total frames that had content redacted before storage.
    pub total_redacted: u64,
}

/// The main ingestion pipeline that processes captured frames.
///
/// Frames flow through:
///   Safety Gate -> Deduplication -> Embedding -> Storage -> Graph (extract entities)
///
/// Search flow:
///   Route -> Search -> Rerank (attention) -> Diversity (quantum) -> Return
pub struct IngestionPipeline {
    embedding_engine: EmbeddingEngine,
    vector_store: VectorStore,
    safety_gate: SafetyGate,
    dedup: FrameDeduplicator,
    stats: PipelineStats,
    /// Optional knowledge graph for entity extraction after storage.
    knowledge_graph: Option<KnowledgeGraph>,
    /// Optional enhanced search orchestrator (router + reranker + quantum).
    enhanced_search: Option<EnhancedSearch>,
}

impl IngestionPipeline {
    /// Create a new ingestion pipeline with the given configuration.
    pub fn new(config: OsPipeConfig) -> Result<Self> {
        let embedding_engine = EmbeddingEngine::new(config.storage.embedding_dim);
        let vector_store = VectorStore::new(config.storage.clone())?;
        let safety_gate = SafetyGate::new(config.safety.clone());
        let dedup = FrameDeduplicator::new(config.storage.dedup_threshold, 100);

        Ok(Self {
            embedding_engine,
            vector_store,
            safety_gate,
            dedup,
            stats: PipelineStats::default(),
            knowledge_graph: None,
            enhanced_search: None,
        })
    }

    /// Attach a knowledge graph for entity extraction on ingested frames.
    ///
    /// When a graph is attached, every successfully stored frame will have
    /// its text analysed for entities (persons, URLs, emails, mentions),
    /// which are then added to the graph as nodes linked to the frame.
    pub fn with_graph(mut self, kg: KnowledgeGraph) -> Self {
        self.knowledge_graph = Some(kg);
        self
    }

    /// Attach an enhanced search orchestrator.
    ///
    /// When attached, the [`search`](Self::search) method will route the
    /// query, fetch extra candidates, re-rank with attention, and apply
    /// quantum-inspired diversity selection before returning results.
    pub fn with_enhanced_search(mut self, es: EnhancedSearch) -> Self {
        self.enhanced_search = Some(es);
        self
    }

    /// Ingest a single captured frame through the pipeline.
    pub fn ingest(&mut self, frame: CapturedFrame) -> Result<IngestResult> {
        let text = frame.text_content().to_string();

        // Step 1: Safety check
        let safe_text = match self.safety_gate.check(&text) {
            SafetyDecision::Allow => text,
            SafetyDecision::AllowRedacted(redacted) => {
                self.stats.total_redacted += 1;
                redacted
            }
            SafetyDecision::Deny { reason } => {
                self.stats.total_denied += 1;
                return Ok(IngestResult::Denied { reason });
            }
        };

        // Step 2: Generate embedding from the (possibly redacted) text
        let embedding = self.embedding_engine.embed(&safe_text);

        // Step 3: Deduplication check
        if let Some((similar_id, similarity)) = self.dedup.is_duplicate(&embedding) {
            self.stats.total_deduplicated += 1;
            return Ok(IngestResult::Deduplicated {
                similar_to: similar_id,
                similarity,
            });
        }

        // Step 4: Store the frame
        // If the text was redacted, create a modified frame with the safe text
        let mut store_frame = frame;
        if safe_text != store_frame.text_content() {
            store_frame.content = match &store_frame.content {
                crate::capture::FrameContent::OcrText(_) => {
                    crate::capture::FrameContent::OcrText(safe_text)
                }
                crate::capture::FrameContent::Transcription(_) => {
                    crate::capture::FrameContent::Transcription(safe_text)
                }
                crate::capture::FrameContent::UiEvent(_) => {
                    crate::capture::FrameContent::UiEvent(safe_text)
                }
            };
        }

        self.vector_store.insert(&store_frame, &embedding)?;
        let id = store_frame.id;
        self.dedup.add(id, embedding);
        self.stats.total_ingested += 1;

        // Step 5: Graph entity extraction (if knowledge graph is attached)
        if let Some(ref mut kg) = self.knowledge_graph {
            let frame_id_str = id.to_string();
            let _ = kg.ingest_frame_entities(&frame_id_str, store_frame.text_content());
        }

        Ok(IngestResult::Stored { id })
    }

    /// Ingest a batch of frames.
    pub fn ingest_batch(&mut self, frames: Vec<CapturedFrame>) -> Result<Vec<IngestResult>> {
        let mut results = Vec::with_capacity(frames.len());
        for frame in frames {
            results.push(self.ingest(frame)?);
        }
        Ok(results)
    }

    /// Return current pipeline statistics.
    pub fn stats(&self) -> &PipelineStats {
        &self.stats
    }

    /// Return a reference to the underlying vector store.
    pub fn vector_store(&self) -> &VectorStore {
        &self.vector_store
    }

    /// Return a reference to the embedding engine.
    pub fn embedding_engine(&self) -> &EmbeddingEngine {
        &self.embedding_engine
    }

    /// Return a reference to the knowledge graph, if one is attached.
    pub fn knowledge_graph(&self) -> Option<&KnowledgeGraph> {
        self.knowledge_graph.as_ref()
    }

    /// Search the pipeline's vector store.
    ///
    /// If an [`EnhancedSearch`] orchestrator is attached, the query is routed,
    /// candidates are fetched with headroom, re-ranked with attention, and
    /// diversity-selected via quantum-inspired algorithms.
    ///
    /// Otherwise, a basic vector similarity search is performed.
    pub fn search(&self, query: &str, k: usize) -> Result<Vec<SearchResult>> {
        let embedding = self.embedding_engine.embed(query);

        if let Some(ref es) = self.enhanced_search {
            es.search(query, &embedding, &self.vector_store, k)
        } else {
            self.vector_store.search(&embedding, k)
        }
    }
}
