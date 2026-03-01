//! GraphQL schema definitions for 7sense API.
//!
//! This module defines the Query, Mutation, and Subscription roots
//! for the GraphQL API.

use async_graphql::*;
use futures::Stream;
use uuid::Uuid;

use super::types::*;
use crate::{AppContext, ProcessingStatus};

/// Root query type for GraphQL API.
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Find similar segments (neighbors) for a given segment.
    async fn neighbors(
        &self,
        ctx: &Context<'_>,
        segment_id: ID,
        #[graphql(default = 10)] k: i32,
        #[graphql(default)] min_similarity: f32,
    ) -> Result<Vec<Neighbor>> {
        let app_ctx = ctx.data::<AppContext>()?;

        let segment_uuid = Uuid::parse_str(segment_id.as_str())
            .map_err(|_| Error::new("Invalid segment ID"))?;

        // Get segment embedding
        let embedding = app_ctx
            .vector_index
            .get_embedding(&segment_uuid)
            .map_err(|e| Error::new(format!("Vector index error: {e}")))?
            .ok_or_else(|| Error::new(format!("Segment {} not found", segment_id.as_str())))?;

        // Search for neighbors
        let results = app_ctx
            .vector_index
            .search(&embedding, k as usize, min_similarity)
            .map_err(|e| Error::new(format!("Search error: {e}")))?;

        // Convert to GraphQL types
        let neighbors: Vec<Neighbor> = results
            .into_iter()
            .filter(|r| r.id != segment_uuid)
            .map(|r| Neighbor {
                segment_id: ID::from(r.id.to_string()),
                recording_id: ID::from(r.recording_id.to_string()),
                similarity: 1.0 - r.distance,
                distance: r.distance,
                start_time: r.start_time,
                end_time: r.end_time,
                species: r.species.map(|s| Species {
                    common_name: s.common_name,
                    scientific_name: s.scientific_name,
                    confidence: s.confidence,
                }),
            })
            .collect();

        Ok(neighbors)
    }

    /// List all discovered clusters.
    async fn clusters(&self, ctx: &Context<'_>) -> Result<Vec<Cluster>> {
        let app_ctx = ctx.data::<AppContext>()?;

        let cluster_data = app_ctx
            .cluster_engine
            .get_all_clusters()
            .map_err(|e| Error::new(format!("Analysis error: {e}")))?;

        let clusters: Vec<Cluster> = cluster_data
            .into_iter()
            .map(|c| Cluster {
                id: ID::from(c.id.to_string()),
                label: c.label,
                size: c.size as i32,
                density: c.density,
                exemplar_ids: c.exemplar_ids.into_iter().map(|id| ID::from(id.to_string())).collect(),
                species_distribution: c
                    .species_distribution
                    .into_iter()
                    .map(|(name, count, percentage)| SpeciesCount {
                        name,
                        scientific_name: None,
                        count: count as i32,
                        percentage,
                    })
                    .collect(),
                created_at: c.created_at,
            })
            .collect();

        Ok(clusters)
    }

    /// Get a specific cluster by ID.
    async fn cluster(&self, ctx: &Context<'_>, id: ID) -> Result<Option<Cluster>> {
        let app_ctx = ctx.data::<AppContext>()?;

        let cluster_uuid =
            Uuid::parse_str(id.as_str()).map_err(|_| Error::new("Invalid cluster ID"))?;

        let cluster_data = app_ctx
            .cluster_engine
            .get_cluster(&cluster_uuid)
            .map_err(|e| Error::new(format!("Analysis error: {e}")))?;

        Ok(cluster_data.map(|c| Cluster {
            id: ID::from(c.id.to_string()),
            label: c.label,
            size: c.size as i32,
            density: c.density,
            exemplar_ids: c.exemplar_ids.into_iter().map(|id| ID::from(id.to_string())).collect(),
            species_distribution: c
                .species_distribution
                .into_iter()
                .map(|(name, count, percentage)| SpeciesCount {
                    name,
                    scientific_name: None,
                    count: count as i32,
                    percentage,
                })
                .collect(),
            created_at: c.created_at,
        }))
    }

    /// System health check.
    async fn health(&self) -> HealthStatus {
        HealthStatus {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Root mutation type for GraphQL API.
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    /// Assign a label to a cluster.
    async fn assign_label(
        &self,
        ctx: &Context<'_>,
        cluster_id: ID,
        label: String,
    ) -> Result<Cluster> {
        let app_ctx = ctx.data::<AppContext>()?;

        let cluster_uuid =
            Uuid::parse_str(cluster_id.as_str()).map_err(|_| Error::new("Invalid cluster ID"))?;

        let cluster_data = app_ctx
            .cluster_engine
            .assign_label(&cluster_uuid, &label)
            .map_err(|e| Error::new(format!("Analysis error: {e}")))?
            .ok_or_else(|| Error::new(format!("Cluster {} not found", cluster_id.as_str())))?;

        Ok(Cluster {
            id: ID::from(cluster_data.id.to_string()),
            label: cluster_data.label,
            size: cluster_data.size as i32,
            density: cluster_data.density,
            exemplar_ids: cluster_data.exemplar_ids.into_iter().map(|id| ID::from(id.to_string())).collect(),
            species_distribution: cluster_data
                .species_distribution
                .into_iter()
                .map(|(name, count, percentage)| SpeciesCount {
                    name,
                    scientific_name: None,
                    count: count as i32,
                    percentage,
                })
                .collect(),
            created_at: cluster_data.created_at,
        })
    }
}

/// Root subscription type for GraphQL API.
pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    /// Subscribe to processing status updates for a recording.
    async fn processing_status(
        &self,
        ctx: &Context<'_>,
        recording_id: ID,
    ) -> Result<impl Stream<Item = ProcessingUpdate>> {
        let app_ctx = ctx.data::<AppContext>()?;
        let recording_uuid = Uuid::parse_str(recording_id.as_str())
            .map_err(|_| Error::new("Invalid recording ID"))?;

        let mut rx = app_ctx.subscribe_events();

        Ok(async_stream::stream! {
            while let Ok(event) = rx.recv().await {
                if event.recording_id == recording_uuid {
                    yield ProcessingUpdate {
                        recording_id: ID::from(event.recording_id.to_string()),
                        status: match event.status {
                            ProcessingStatus::Queued => ProcessingStatusGql::Queued,
                            ProcessingStatus::Loading => ProcessingStatusGql::Loading,
                            ProcessingStatus::Segmenting => ProcessingStatusGql::Segmenting,
                            ProcessingStatus::Embedding => ProcessingStatusGql::Embedding,
                            ProcessingStatus::Indexing => ProcessingStatusGql::Indexing,
                            ProcessingStatus::Analyzing => ProcessingStatusGql::Analyzing,
                            ProcessingStatus::Complete => ProcessingStatusGql::Complete,
                            ProcessingStatus::Failed => ProcessingStatusGql::Failed,
                        },
                        progress: event.progress,
                        message: event.message,
                    };
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status() {
        let status = HealthStatus {
            status: "healthy".to_string(),
            version: "0.1.0".to_string(),
        };
        assert_eq!(status.status, "healthy");
    }
}
