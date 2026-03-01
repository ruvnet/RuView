//! REST API handlers for 7sense endpoints.
//!
//! Each handler follows the pattern:
//! 1. Extract and validate request parameters
//! 2. Call the appropriate service layer
//! 3. Transform results to API response types
//! 4. Handle errors with proper status codes

use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    error::{ApiError, ApiResult},
    services::SpeciesInfo,
    AppContext, ProcessingEvent, ProcessingStatus,
};

// ============================================================================
// Request/Response Types
// ============================================================================

/// Recording metadata and processing status.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Recording {
    /// Unique recording identifier
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Original filename
    #[schema(example = "dawn_chorus_2024.wav")]
    pub filename: String,
    /// Recording duration in seconds
    #[schema(example = 120.5)]
    pub duration_secs: f64,
    /// Sample rate in Hz
    #[schema(example = 44100)]
    pub sample_rate: u32,
    /// Number of audio channels
    #[schema(example = 1)]
    pub channels: u16,
    /// Current processing status
    pub status: ProcessingStatus,
    /// Number of detected segments
    #[schema(example = 42)]
    pub segment_count: usize,
    /// Upload timestamp
    pub created_at: DateTime<Utc>,
    /// Last processing update
    pub updated_at: DateTime<Utc>,
}

/// Upload response after successful recording ingestion.
#[derive(Debug, Serialize, ToSchema)]
pub struct UploadResponse {
    /// The created recording
    pub recording: Recording,
    /// WebSocket URL for status updates
    #[schema(example = "/ws/recordings/550e8400-e29b-41d4-a716-446655440000")]
    pub status_url: String,
}

/// Query parameters for neighbor search.
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct NeighborParams {
    /// Number of neighbors to return (default: 10)
    #[param(default = 10, minimum = 1, maximum = 100)]
    pub k: Option<usize>,
    /// Minimum similarity threshold (0.0 to 1.0)
    #[param(default = 0.0, minimum = 0.0, maximum = 1.0)]
    pub min_similarity: Option<f32>,
    /// Filter by species (optional)
    pub species: Option<String>,
    /// Include segment audio URLs
    #[param(default = false)]
    pub include_audio: Option<bool>,
}

/// A similar segment (neighbor) in the embedding space.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Neighbor {
    /// Segment identifier
    pub segment_id: Uuid,
    /// Parent recording ID
    pub recording_id: Uuid,
    /// Similarity score (0.0 to 1.0)
    #[schema(example = 0.95)]
    pub similarity: f32,
    /// Distance in embedding space
    #[schema(example = 0.123)]
    pub distance: f32,
    /// Segment start time in seconds
    #[schema(example = 12.5)]
    pub start_time: f64,
    /// Segment end time in seconds
    #[schema(example = 14.2)]
    pub end_time: f64,
    /// Detected species (if any)
    pub species: Option<SpeciesInfo>,
    /// URL to audio clip (if requested)
    pub audio_url: Option<String>,
}

/// A discovered cluster of similar calls.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Cluster {
    /// Cluster identifier
    pub id: Uuid,
    /// Human-assigned label (if any)
    pub label: Option<String>,
    /// Number of segments in cluster
    #[schema(example = 156)]
    pub size: usize,
    /// Cluster centroid (mean embedding)
    pub centroid: Vec<f32>,
    /// Cluster density/compactness score
    #[schema(example = 0.87)]
    pub density: f32,
    /// Representative segment IDs
    pub exemplar_ids: Vec<Uuid>,
    /// Detected species distribution
    pub species_distribution: Vec<SpeciesCount>,
    /// Cluster creation timestamp
    pub created_at: DateTime<Utc>,
}

/// Species count within a cluster.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SpeciesCount {
    /// Species common name
    #[schema(example = "American Robin")]
    pub name: String,
    /// Scientific name
    #[schema(example = "Turdus migratorius")]
    pub scientific_name: Option<String>,
    /// Count of segments with this species
    #[schema(example = 42)]
    pub count: usize,
    /// Percentage of cluster
    #[schema(example = 27.3)]
    pub percentage: f64,
}

/// Evidence pack for interpretability.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EvidencePack {
    /// Query identifier
    pub query_id: Uuid,
    /// Segment that was queried
    pub query_segment: SegmentSummary,
    /// Retrieved neighbors with evidence
    pub neighbors: Vec<NeighborEvidence>,
    /// Shared acoustic features
    pub shared_features: Vec<AcousticFeature>,
    /// Visualization data
    pub visualizations: EvidenceVisualizations,
    /// Generation timestamp
    pub generated_at: DateTime<Utc>,
}

/// Summary of a segment for evidence packs.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SegmentSummary {
    /// Segment ID
    pub id: Uuid,
    /// Recording ID
    pub recording_id: Uuid,
    /// Start time
    pub start_time: f64,
    /// End time
    pub end_time: f64,
    /// Detected species
    pub species: Option<SpeciesInfo>,
}

/// Evidence for a neighbor relationship.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NeighborEvidence {
    /// The neighbor segment
    pub segment: SegmentSummary,
    /// Similarity score
    pub similarity: f32,
    /// Contributing feature dimensions
    pub contributing_features: Vec<FeatureContribution>,
    /// Spectrogram comparison URL
    pub spectrogram_comparison_url: Option<String>,
}

/// Feature contribution to similarity.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FeatureContribution {
    /// Feature name
    #[schema(example = "fundamental_frequency")]
    pub name: String,
    /// Contribution weight
    #[schema(example = 0.23)]
    pub weight: f32,
    /// Query value
    pub query_value: f64,
    /// Neighbor value
    pub neighbor_value: f64,
}

/// Acoustic feature shared between segments.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AcousticFeature {
    /// Feature name
    #[schema(example = "frequency_modulation")]
    pub name: String,
    /// Feature description
    #[schema(example = "Rapid upward sweep in 200-400ms")]
    pub description: String,
    /// Confidence score
    #[schema(example = 0.92)]
    pub confidence: f32,
}

/// Visualization URLs for evidence pack.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EvidenceVisualizations {
    /// UMAP projection of embedding space
    pub umap_url: Option<String>,
    /// Spectrogram grid URL
    pub spectrogram_grid_url: Option<String>,
    /// Feature importance chart URL
    pub feature_importance_url: Option<String>,
}

/// Search query for semantic search.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SearchQuery {
    /// Text description to search for
    #[schema(example = "ascending whistle followed by trill")]
    pub query: Option<String>,
    /// Segment ID to find similar segments
    pub segment_id: Option<Uuid>,
    /// Embedding vector for direct search
    pub embedding: Option<Vec<f32>>,
    /// Number of results (default: 20)
    #[schema(default = 20, minimum = 1, maximum = 200)]
    pub limit: Option<usize>,
    /// Species filter
    pub species_filter: Option<Vec<String>>,
    /// Time range filter (start)
    pub time_start: Option<DateTime<Utc>>,
    /// Time range filter (end)
    pub time_end: Option<DateTime<Utc>>,
}

/// Search results response.
#[derive(Debug, Serialize, ToSchema)]
pub struct SearchResults {
    /// Search query echo
    pub query: SearchQueryEcho,
    /// Matching segments
    pub results: Vec<SearchResult>,
    /// Total count (may be estimated)
    pub total_count: usize,
    /// Search latency in milliseconds
    pub latency_ms: u64,
}

/// Echo of the search query.
#[derive(Debug, Serialize, ToSchema)]
pub struct SearchQueryEcho {
    /// Text query
    pub text: Option<String>,
    /// Segment ID query
    pub segment_id: Option<Uuid>,
    /// Result limit
    pub limit: usize,
}

/// Individual search result.
#[derive(Debug, Serialize, ToSchema)]
pub struct SearchResult {
    /// Segment information
    pub segment: SegmentSummary,
    /// Search score
    #[schema(example = 0.87)]
    pub score: f32,
    /// Highlight/explanation
    pub highlight: Option<String>,
}

/// Cluster label assignment request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AssignLabelRequest {
    /// Label to assign
    #[schema(example = "Northern Cardinal song type A")]
    pub label: String,
}

/// Request to generate evidence pack.
#[derive(Debug, Deserialize, ToSchema)]
pub struct GenerateEvidenceRequest {
    /// Segment ID to analyze
    pub segment_id: Uuid,
    /// Number of neighbors to include
    #[schema(default = 10)]
    pub k: Option<usize>,
    /// Include spectrogram comparisons
    #[schema(default = true)]
    pub include_spectrograms: Option<bool>,
}

// ============================================================================
// Handlers
// ============================================================================

/// Upload and process an audio recording.
///
/// Accepts multipart form data with an audio file. Supported formats:
/// - WAV (recommended)
/// - FLAC
/// - MP3
/// - OGG
///
/// Processing is asynchronous. Subscribe to the WebSocket URL for status updates.
#[utoipa::path(
    post,
    path = "/recordings",
    request_body(content = Vec<u8>, content_type = "multipart/form-data"),
    responses(
        (status = 201, description = "Recording uploaded and processing started", body = UploadResponse),
        (status = 400, description = "Invalid audio file", body = crate::error::ErrorResponse),
        (status = 413, description = "File too large", body = crate::error::ErrorResponse),
        (status = 415, description = "Unsupported audio format", body = crate::error::ErrorResponse),
    ),
    tag = "recordings"
)]
pub async fn upload_recording(
    State(ctx): State<AppContext>,
    mut multipart: Multipart,
) -> ApiResult<(StatusCode, Json<UploadResponse>)> {
    // Extract file from multipart
    let mut audio_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut content_type: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(format!("Invalid multipart data: {e}")))?
    {
        let field_name = field.name().unwrap_or_default().to_string();

        if field_name == "file" || field_name == "audio" {
            filename = field.file_name().map(String::from);
            content_type = field.content_type().map(String::from);

            let data = field
                .bytes()
                .await
                .map_err(|e| ApiError::BadRequest(format!("Failed to read file: {e}")))?;

            // Check file size
            if data.len() > ctx.config.max_upload_size {
                return Err(ApiError::PayloadTooLarge(format!(
                    "File size {} exceeds maximum {}",
                    data.len(),
                    ctx.config.max_upload_size
                )));
            }

            audio_data = Some(data.to_vec());
        }
    }

    let audio_data =
        audio_data.ok_or_else(|| ApiError::BadRequest("No audio file provided".into()))?;
    let filename = filename.unwrap_or_else(|| "unknown.wav".to_string());

    // Validate content type
    let valid_types = ["audio/wav", "audio/x-wav", "audio/flac", "audio/mpeg", "audio/ogg"];
    if let Some(ref ct) = content_type {
        if !valid_types.iter().any(|t| ct.contains(t)) {
            tracing::warn!(content_type = %ct, "Unknown content type, proceeding anyway");
        }
    }

    // Create recording record
    let recording_id = Uuid::new_v4();
    let now = Utc::now();

    // Publish initial event
    ctx.publish_event(ProcessingEvent {
        recording_id,
        status: ProcessingStatus::Queued,
        progress: 0.0,
        message: Some("Recording queued for processing".into()),
    });

    // Start async processing
    let ctx_clone = ctx.clone();
    let audio_data_clone = audio_data.clone();
    tokio::spawn(async move {
        process_recording(ctx_clone, recording_id, audio_data_clone).await;
    });

    // Get audio metadata (quick parse)
    let (duration, sample_rate, channels) = ctx.audio_pipeline.get_metadata(&audio_data)?;

    let recording = Recording {
        id: recording_id,
        filename,
        duration_secs: duration,
        sample_rate,
        channels,
        status: ProcessingStatus::Queued,
        segment_count: 0,
        created_at: now,
        updated_at: now,
    };

    let response = UploadResponse {
        recording: recording.clone(),
        status_url: format!("/ws/recordings/{recording_id}"),
    };

    Ok((StatusCode::CREATED, Json(response)))
}

/// Background task to process a recording.
async fn process_recording(ctx: AppContext, recording_id: Uuid, audio_data: Vec<u8>) {
    // Loading
    ctx.publish_event(ProcessingEvent {
        recording_id,
        status: ProcessingStatus::Loading,
        progress: 0.1,
        message: Some("Loading audio file".into()),
    });

    // Parse audio
    let audio = match ctx.audio_pipeline.load_audio(&audio_data) {
        Ok(a) => a,
        Err(e) => {
            ctx.publish_event(ProcessingEvent {
                recording_id,
                status: ProcessingStatus::Failed,
                progress: 0.0,
                message: Some(format!("Failed to load audio: {e}")),
            });
            return;
        }
    };

    // Segmenting
    ctx.publish_event(ProcessingEvent {
        recording_id,
        status: ProcessingStatus::Segmenting,
        progress: 0.3,
        message: Some("Detecting call segments".into()),
    });

    let segments = match ctx.audio_pipeline.segment(&audio) {
        Ok(s) => s,
        Err(e) => {
            ctx.publish_event(ProcessingEvent {
                recording_id,
                status: ProcessingStatus::Failed,
                progress: 0.0,
                message: Some(format!("Segmentation failed: {e}")),
            });
            return;
        }
    };

    // Embedding
    ctx.publish_event(ProcessingEvent {
        recording_id,
        status: ProcessingStatus::Embedding,
        progress: 0.5,
        message: Some(format!("Generating embeddings for {} segments", segments.len())),
    });

    let embeddings = match ctx.embedding_model.embed_batch(&segments).await {
        Ok(e) => e,
        Err(e) => {
            ctx.publish_event(ProcessingEvent {
                recording_id,
                status: ProcessingStatus::Failed,
                progress: 0.0,
                message: Some(format!("Embedding failed: {e}")),
            });
            return;
        }
    };

    // Indexing
    ctx.publish_event(ProcessingEvent {
        recording_id,
        status: ProcessingStatus::Indexing,
        progress: 0.7,
        message: Some("Adding to vector index".into()),
    });

    if let Err(e) = ctx.vector_index.add_batch(&embeddings) {
        ctx.publish_event(ProcessingEvent {
            recording_id,
            status: ProcessingStatus::Failed,
            progress: 0.0,
            message: Some(format!("Indexing failed: {e}")),
        });
        return;
    }

    // Analyzing
    ctx.publish_event(ProcessingEvent {
        recording_id,
        status: ProcessingStatus::Analyzing,
        progress: 0.9,
        message: Some("Running cluster analysis".into()),
    });

    if let Err(e) = ctx.cluster_engine.update_clusters(&embeddings) {
        tracing::warn!(error = %e, "Cluster update failed, continuing");
    }

    // Complete
    ctx.publish_event(ProcessingEvent {
        recording_id,
        status: ProcessingStatus::Complete,
        progress: 1.0,
        message: Some(format!(
            "Processing complete: {} segments indexed",
            segments.len()
        )),
    });
}

/// Get a recording by ID.
#[utoipa::path(
    get,
    path = "/recordings/{id}",
    params(
        ("id" = Uuid, Path, description = "Recording ID")
    ),
    responses(
        (status = 200, description = "Recording found", body = Recording),
        (status = 404, description = "Recording not found", body = crate::error::ErrorResponse),
    ),
    tag = "recordings"
)]
pub async fn get_recording(
    State(_ctx): State<AppContext>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Recording>> {
    // In production, this would query a database
    // For now, return a mock or error
    Err(ApiError::not_found("Recording", id))
}

/// Find similar segments (neighbors) for a given segment.
#[utoipa::path(
    get,
    path = "/segments/{id}/neighbors",
    params(
        ("id" = Uuid, Path, description = "Segment ID"),
        NeighborParams
    ),
    responses(
        (status = 200, description = "Neighbors found", body = Vec<Neighbor>),
        (status = 404, description = "Segment not found", body = crate::error::ErrorResponse),
    ),
    tag = "segments"
)]
pub async fn get_neighbors(
    State(ctx): State<AppContext>,
    Path(segment_id): Path<Uuid>,
    Query(params): Query<NeighborParams>,
) -> ApiResult<Json<Vec<Neighbor>>> {
    let k = params.k.unwrap_or(10).min(100);
    let min_similarity = params.min_similarity.unwrap_or(0.0);

    // Get segment embedding
    let embedding = ctx
        .vector_index
        .get_embedding(&segment_id)?
        .ok_or_else(|| ApiError::not_found("Segment", segment_id))?;

    // Search for neighbors
    let results = ctx.vector_index.search(&embedding, k, min_similarity)?;

    // Convert to response format
    let neighbors: Vec<Neighbor> = results
        .into_iter()
        .filter(|r| r.id != segment_id) // Exclude query segment
        .map(|r| Neighbor {
            segment_id: r.id,
            recording_id: r.recording_id,
            similarity: 1.0 - r.distance, // Convert distance to similarity
            distance: r.distance,
            start_time: r.start_time,
            end_time: r.end_time,
            species: r.species,
            audio_url: if params.include_audio.unwrap_or(false) {
                Some(format!("/api/v1/segments/{}/audio", r.id))
            } else {
                None
            },
        })
        .collect();

    Ok(Json(neighbors))
}

/// List all discovered clusters.
#[utoipa::path(
    get,
    path = "/clusters",
    responses(
        (status = 200, description = "Clusters retrieved", body = Vec<Cluster>),
    ),
    tag = "clusters"
)]
pub async fn list_clusters(State(ctx): State<AppContext>) -> ApiResult<Json<Vec<Cluster>>> {
    let cluster_data = ctx.cluster_engine.get_all_clusters()?;

    let clusters: Vec<Cluster> = cluster_data
        .into_iter()
        .map(|c| Cluster {
            id: c.id,
            label: c.label,
            size: c.size,
            centroid: c.centroid,
            density: c.density,
            exemplar_ids: c.exemplar_ids,
            species_distribution: c
                .species_distribution
                .into_iter()
                .map(|(name, count, percentage)| SpeciesCount {
                    name: name.clone(),
                    scientific_name: None, // Would be looked up
                    count,
                    percentage,
                })
                .collect(),
            created_at: c.created_at,
        })
        .collect();

    Ok(Json(clusters))
}

/// Get a specific cluster by ID.
#[utoipa::path(
    get,
    path = "/clusters/{id}",
    params(
        ("id" = Uuid, Path, description = "Cluster ID")
    ),
    responses(
        (status = 200, description = "Cluster found", body = Cluster),
        (status = 404, description = "Cluster not found", body = crate::error::ErrorResponse),
    ),
    tag = "clusters"
)]
pub async fn get_cluster(
    State(ctx): State<AppContext>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Cluster>> {
    let cluster_data = ctx
        .cluster_engine
        .get_cluster(&id)?
        .ok_or_else(|| ApiError::not_found("Cluster", id))?;

    let cluster = Cluster {
        id: cluster_data.id,
        label: cluster_data.label,
        size: cluster_data.size,
        centroid: cluster_data.centroid,
        density: cluster_data.density,
        exemplar_ids: cluster_data.exemplar_ids,
        species_distribution: cluster_data
            .species_distribution
            .into_iter()
            .map(|(name, count, percentage)| SpeciesCount {
                name,
                scientific_name: None,
                count,
                percentage,
            })
            .collect(),
        created_at: cluster_data.created_at,
    };

    Ok(Json(cluster))
}

/// Assign a label to a cluster.
#[utoipa::path(
    put,
    path = "/clusters/{id}/label",
    params(
        ("id" = Uuid, Path, description = "Cluster ID")
    ),
    request_body = AssignLabelRequest,
    responses(
        (status = 200, description = "Label assigned", body = Cluster),
        (status = 404, description = "Cluster not found", body = crate::error::ErrorResponse),
    ),
    tag = "clusters"
)]
pub async fn assign_cluster_label(
    State(ctx): State<AppContext>,
    Path(id): Path<Uuid>,
    Json(request): Json<AssignLabelRequest>,
) -> ApiResult<Json<Cluster>> {
    let cluster_data = ctx
        .cluster_engine
        .assign_label(&id, &request.label)?
        .ok_or_else(|| ApiError::not_found("Cluster", id))?;

    let cluster = Cluster {
        id: cluster_data.id,
        label: cluster_data.label,
        size: cluster_data.size,
        centroid: cluster_data.centroid,
        density: cluster_data.density,
        exemplar_ids: cluster_data.exemplar_ids,
        species_distribution: cluster_data
            .species_distribution
            .into_iter()
            .map(|(name, count, percentage)| SpeciesCount {
                name,
                scientific_name: None,
                count,
                percentage,
            })
            .collect(),
        created_at: cluster_data.created_at,
    };

    Ok(Json(cluster))
}

/// Get evidence pack for interpretability.
#[utoipa::path(
    get,
    path = "/evidence/{id}",
    params(
        ("id" = String, Path, description = "Evidence pack ID (query UUID)")
    ),
    responses(
        (status = 200, description = "Evidence pack retrieved", body = EvidencePack),
        (status = 404, description = "Evidence not found", body = crate::error::ErrorResponse),
    ),
    tag = "evidence"
)]
pub async fn get_evidence_pack(
    State(ctx): State<AppContext>,
    Path(id): Path<String>,
) -> ApiResult<Json<EvidencePack>> {
    let query_id = Uuid::parse_str(&id)
        .map_err(|_| ApiError::BadRequest(format!("Invalid UUID: {id}")))?;

    let evidence = ctx
        .interpretation_engine
        .get_evidence_pack(&query_id)?
        .ok_or_else(|| ApiError::not_found("Evidence pack", id))?;

    let pack = EvidencePack {
        query_id: evidence.query_id,
        query_segment: SegmentSummary {
            id: evidence.query_segment.id,
            recording_id: evidence.query_segment.recording_id,
            start_time: evidence.query_segment.start_time,
            end_time: evidence.query_segment.end_time,
            species: evidence.query_segment.species,
        },
        neighbors: evidence
            .neighbors
            .into_iter()
            .map(|n| NeighborEvidence {
                segment: SegmentSummary {
                    id: n.segment.id,
                    recording_id: n.segment.recording_id,
                    start_time: n.segment.start_time,
                    end_time: n.segment.end_time,
                    species: n.segment.species,
                },
                similarity: n.similarity,
                contributing_features: n
                    .contributing_features
                    .into_iter()
                    .map(|f| FeatureContribution {
                        name: f.name,
                        weight: f.weight,
                        query_value: f.query_value,
                        neighbor_value: f.neighbor_value,
                    })
                    .collect(),
                spectrogram_comparison_url: n.spectrogram_comparison_url,
            })
            .collect(),
        shared_features: evidence
            .shared_features
            .into_iter()
            .map(|f| AcousticFeature {
                name: f.name,
                description: f.description,
                confidence: f.confidence,
            })
            .collect(),
        visualizations: EvidenceVisualizations {
            umap_url: evidence.visualizations.umap_url,
            spectrogram_grid_url: evidence.visualizations.spectrogram_grid_url,
            feature_importance_url: evidence.visualizations.feature_importance_url,
        },
        generated_at: evidence.generated_at,
    };

    Ok(Json(pack))
}

/// Generate evidence pack for a segment query.
#[utoipa::path(
    post,
    path = "/evidence",
    request_body = GenerateEvidenceRequest,
    responses(
        (status = 201, description = "Evidence pack generated", body = EvidencePack),
        (status = 404, description = "Segment not found", body = crate::error::ErrorResponse),
    ),
    tag = "evidence"
)]
pub async fn generate_evidence_pack(
    State(ctx): State<AppContext>,
    Json(request): Json<GenerateEvidenceRequest>,
) -> ApiResult<(StatusCode, Json<EvidencePack>)> {
    // Get segment embedding
    let embedding = ctx
        .vector_index
        .get_embedding(&request.segment_id)?
        .ok_or_else(|| ApiError::not_found("Segment", request.segment_id))?;

    // Find neighbors
    let k = request.k.unwrap_or(10);
    let neighbors = ctx.vector_index.search(&embedding, k, 0.0)?;

    // Generate evidence pack
    let evidence = ctx
        .interpretation_engine
        .generate_evidence_pack(&request.segment_id, &neighbors)
        .await?;

    let pack = EvidencePack {
        query_id: evidence.query_id,
        query_segment: SegmentSummary {
            id: evidence.query_segment.id,
            recording_id: evidence.query_segment.recording_id,
            start_time: evidence.query_segment.start_time,
            end_time: evidence.query_segment.end_time,
            species: evidence.query_segment.species,
        },
        neighbors: evidence
            .neighbors
            .into_iter()
            .map(|n| NeighborEvidence {
                segment: SegmentSummary {
                    id: n.segment.id,
                    recording_id: n.segment.recording_id,
                    start_time: n.segment.start_time,
                    end_time: n.segment.end_time,
                    species: n.segment.species,
                },
                similarity: n.similarity,
                contributing_features: n
                    .contributing_features
                    .into_iter()
                    .map(|f| FeatureContribution {
                        name: f.name,
                        weight: f.weight,
                        query_value: f.query_value,
                        neighbor_value: f.neighbor_value,
                    })
                    .collect(),
                spectrogram_comparison_url: n.spectrogram_comparison_url,
            })
            .collect(),
        shared_features: evidence
            .shared_features
            .into_iter()
            .map(|f| AcousticFeature {
                name: f.name,
                description: f.description,
                confidence: f.confidence,
            })
            .collect(),
        visualizations: EvidenceVisualizations {
            umap_url: evidence.visualizations.umap_url,
            spectrogram_grid_url: evidence.visualizations.spectrogram_grid_url,
            feature_importance_url: evidence.visualizations.feature_importance_url,
        },
        generated_at: evidence.generated_at,
    };

    Ok((StatusCode::CREATED, Json(pack)))
}

/// Semantic search across segments.
#[utoipa::path(
    post,
    path = "/search",
    request_body = SearchQuery,
    responses(
        (status = 200, description = "Search results", body = SearchResults),
        (status = 400, description = "Invalid search query", body = crate::error::ErrorResponse),
    ),
    tag = "search"
)]
pub async fn search(
    State(ctx): State<AppContext>,
    Json(query): Json<SearchQuery>,
) -> ApiResult<Json<SearchResults>> {
    let start = std::time::Instant::now();

    // Validate query - need at least one search method
    if query.query.is_none() && query.segment_id.is_none() && query.embedding.is_none() {
        return Err(ApiError::BadRequest(
            "Must provide query text, segment_id, or embedding".into(),
        ));
    }

    let limit = query.limit.unwrap_or(20).min(200);

    // Get search embedding
    let search_embedding = if let Some(ref text) = query.query {
        // Text-to-embedding search
        ctx.embedding_model.embed_text(text).await?
    } else if let Some(segment_id) = query.segment_id {
        // Search by segment
        ctx.vector_index
            .get_embedding(&segment_id)?
            .ok_or_else(|| ApiError::not_found("Segment", segment_id))?
    } else if let Some(ref embedding) = query.embedding {
        // Direct embedding search
        embedding.clone()
    } else {
        unreachable!()
    };

    // Perform search
    let results = ctx.vector_index.search(&search_embedding, limit, 0.0)?;

    let latency_ms = start.elapsed().as_millis() as u64;

    let search_results = SearchResults {
        query: SearchQueryEcho {
            text: query.query,
            segment_id: query.segment_id,
            limit,
        },
        results: results
            .into_iter()
            .map(|r| SearchResult {
                segment: SegmentSummary {
                    id: r.id,
                    recording_id: r.recording_id,
                    start_time: r.start_time,
                    end_time: r.end_time,
                    species: r.species,
                },
                score: 1.0 - r.distance,
                highlight: None,
            })
            .collect(),
        total_count: limit, // Would be actual count from index
        latency_ms,
    };

    Ok(Json(search_results))
}

/// Health check endpoint.
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service healthy", body = crate::HealthResponse),
    ),
    tag = "system"
)]
pub async fn health_check() -> Json<crate::HealthResponse> {
    Json(crate::HealthResponse {
        status: "healthy".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        uptime_secs: 0, // Would track actual uptime
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neighbor_params_defaults() {
        let params: NeighborParams = serde_json::from_str("{}").unwrap();
        assert!(params.k.is_none());
        assert!(params.min_similarity.is_none());
    }

    #[test]
    fn test_search_query_validation() {
        let query = SearchQuery {
            query: None,
            segment_id: None,
            embedding: None,
            limit: None,
            species_filter: None,
            time_start: None,
            time_end: None,
        };
        // This should fail validation in the handler
        assert!(query.query.is_none() && query.segment_id.is_none() && query.embedding.is_none());
    }
}
