//! Repository traits for the audio domain.
//!
//! These traits define the persistence interface for domain entities,
//! following the repository pattern from DDD.

use async_trait::async_trait;
use sevensense_core::{GeoLocation, RecordingId, SegmentId};

use super::entities::{CallSegment, Recording, SignalQuality};
use crate::AudioError;

/// Repository trait for Recording entities.
///
/// Implementations handle the persistence of recordings and their
/// associated segments. This trait enables the domain layer to
/// remain independent of the specific storage mechanism.
#[async_trait]
pub trait RecordingRepository: Send + Sync {
    /// Saves a recording to the repository.
    ///
    /// If the recording already exists, it will be updated.
    async fn save(&self, recording: &Recording) -> Result<(), AudioError>;

    /// Finds a recording by its unique identifier.
    async fn find_by_id(&self, id: &RecordingId) -> Result<Option<Recording>, AudioError>;

    /// Finds all recordings within a radius of a geographic location.
    ///
    /// # Arguments
    /// * `loc` - The center point of the search
    /// * `radius_km` - Search radius in kilometers
    async fn find_by_location(
        &self,
        loc: &GeoLocation,
        radius_km: f64,
    ) -> Result<Vec<Recording>, AudioError>;

    /// Finds recordings by source file path pattern.
    async fn find_by_path_pattern(&self, pattern: &str) -> Result<Vec<Recording>, AudioError>;

    /// Deletes a recording and all its segments.
    async fn delete(&self, id: &RecordingId) -> Result<bool, AudioError>;

    /// Returns the total count of recordings.
    async fn count(&self) -> Result<u64, AudioError>;

    /// Lists recordings with pagination.
    async fn list(&self, offset: u64, limit: u64) -> Result<Vec<Recording>, AudioError>;
}

/// Repository trait for CallSegment entities.
///
/// While segments are part of the Recording aggregate, this repository
/// provides direct access for querying and analysis purposes.
#[async_trait]
pub trait SegmentRepository: Send + Sync {
    /// Saves a segment to the repository.
    async fn save(&self, segment: &CallSegment) -> Result<(), AudioError>;

    /// Saves multiple segments in batch.
    async fn save_batch(&self, segments: &[CallSegment]) -> Result<(), AudioError>;

    /// Finds a segment by its unique identifier.
    async fn find_by_id(&self, id: &SegmentId) -> Result<Option<CallSegment>, AudioError>;

    /// Finds all segments for a recording.
    async fn find_by_recording(&self, recording_id: &RecordingId) -> Result<Vec<CallSegment>, AudioError>;

    /// Finds segments by quality level.
    async fn find_by_quality(&self, quality: SignalQuality) -> Result<Vec<CallSegment>, AudioError>;

    /// Finds segments within a time range of a recording.
    async fn find_in_time_range(
        &self,
        recording_id: &RecordingId,
        start_ms: u64,
        end_ms: u64,
    ) -> Result<Vec<CallSegment>, AudioError>;

    /// Deletes a segment.
    async fn delete(&self, id: &SegmentId) -> Result<bool, AudioError>;

    /// Deletes all segments for a recording.
    async fn delete_by_recording(&self, recording_id: &RecordingId) -> Result<u64, AudioError>;
}

/// Query specification for finding recordings.
#[derive(Debug, Clone, Default)]
pub struct RecordingQuery {
    /// Filter by location and radius.
    pub location: Option<(GeoLocation, f64)>,
    /// Filter by minimum duration in milliseconds.
    pub min_duration_ms: Option<u64>,
    /// Filter by maximum duration in milliseconds.
    pub max_duration_ms: Option<u64>,
    /// Filter by minimum number of segments.
    pub min_segments: Option<usize>,
    /// Filter by source path pattern (glob-style).
    pub path_pattern: Option<String>,
    /// Pagination offset.
    pub offset: u64,
    /// Pagination limit.
    pub limit: u64,
}

impl RecordingQuery {
    /// Creates a new query with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            limit: 100,
            ..Default::default()
        }
    }

    /// Sets the location filter.
    #[must_use]
    pub fn with_location(mut self, loc: GeoLocation, radius_km: f64) -> Self {
        self.location = Some((loc, radius_km));
        self
    }

    /// Sets the minimum duration filter.
    #[must_use]
    pub fn with_min_duration(mut self, ms: u64) -> Self {
        self.min_duration_ms = Some(ms);
        self
    }

    /// Sets the maximum duration filter.
    #[must_use]
    pub fn with_max_duration(mut self, ms: u64) -> Self {
        self.max_duration_ms = Some(ms);
        self
    }

    /// Sets the minimum segments filter.
    #[must_use]
    pub fn with_min_segments(mut self, count: usize) -> Self {
        self.min_segments = Some(count);
        self
    }

    /// Sets the path pattern filter.
    #[must_use]
    pub fn with_path_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.path_pattern = Some(pattern.into());
        self
    }

    /// Sets pagination parameters.
    #[must_use]
    pub fn with_pagination(mut self, offset: u64, limit: u64) -> Self {
        self.offset = offset;
        self.limit = limit;
        self
    }
}

/// Extended repository trait with query support.
#[async_trait]
pub trait RecordingQueryRepository: RecordingRepository {
    /// Executes a query and returns matching recordings.
    async fn query(&self, query: &RecordingQuery) -> Result<Vec<Recording>, AudioError>;

    /// Counts recordings matching a query.
    async fn query_count(&self, query: &RecordingQuery) -> Result<u64, AudioError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_builder() {
        let query = RecordingQuery::new()
            .with_min_duration(1000)
            .with_max_duration(60000)
            .with_min_segments(5)
            .with_pagination(0, 50);

        assert_eq!(query.min_duration_ms, Some(1000));
        assert_eq!(query.max_duration_ms, Some(60000));
        assert_eq!(query.min_segments, Some(5));
        assert_eq!(query.limit, 50);
    }
}
