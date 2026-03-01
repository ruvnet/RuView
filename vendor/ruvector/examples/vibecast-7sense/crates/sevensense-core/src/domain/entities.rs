//! # Domain Entities
//!
//! Core domain entities and value objects for the 7sense bioacoustics platform.
//!
//! ## Entity Types
//!
//! - `RecordingId`: Unique identifier for audio recordings
//! - `SegmentId`: Unique identifier for audio segments (portions of recordings)
//! - `EmbeddingId`: Unique identifier for vector embeddings
//! - `ClusterId`: Unique identifier for species/sound clusters
//! - `TaxonId`: Scientific taxonomy identifier for species
//!
//! ## Value Objects
//!
//! - `Timestamp`: UTC timestamp with nanosecond precision
//! - `GeoLocation`: Geographic coordinates with optional elevation
//! - `AudioMetadata`: Audio file technical specifications

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

// =============================================================================
// Identity Types (Entity IDs)
// =============================================================================

/// Unique identifier for an audio recording.
///
/// A recording represents a single continuous audio capture session,
/// which may contain multiple segments with different species vocalizations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RecordingId(Uuid);

impl RecordingId {
    /// Creates a new random `RecordingId`.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates a `RecordingId` from an existing UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the inner UUID value.
    #[must_use]
    pub const fn inner(&self) -> Uuid {
        self.0
    }

    /// Parses a `RecordingId` from a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the string is not a valid UUID.
    pub fn parse(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Default for RecordingId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RecordingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for RecordingId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<RecordingId> for Uuid {
    fn from(id: RecordingId) -> Self {
        id.0
    }
}

/// Unique identifier for an audio segment.
///
/// A segment is a time-bounded portion of a recording that contains
/// a single vocalization or sound event of interest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SegmentId(Uuid);

impl SegmentId {
    /// Creates a new random `SegmentId`.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates a `SegmentId` from an existing UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the inner UUID value.
    #[must_use]
    pub const fn inner(&self) -> Uuid {
        self.0
    }

    /// Parses a `SegmentId` from a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the string is not a valid UUID.
    pub fn parse(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Default for SegmentId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SegmentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for SegmentId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// Unique identifier for a vector embedding.
///
/// An embedding is a dense vector representation of an audio segment,
/// used for similarity search and clustering operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EmbeddingId(Uuid);

impl EmbeddingId {
    /// Creates a new random `EmbeddingId`.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates an `EmbeddingId` from an existing UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the inner UUID value.
    #[must_use]
    pub const fn inner(&self) -> Uuid {
        self.0
    }

    /// Parses an `EmbeddingId` from a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the string is not a valid UUID.
    pub fn parse(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Default for EmbeddingId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EmbeddingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for EmbeddingId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// Unique identifier for a cluster of similar sounds.
///
/// Clusters group together embeddings that likely represent
/// the same species or sound type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClusterId(Uuid);

impl ClusterId {
    /// Creates a new random `ClusterId`.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates a `ClusterId` from an existing UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the inner UUID value.
    #[must_use]
    pub const fn inner(&self) -> Uuid {
        self.0
    }

    /// Parses a `ClusterId` from a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the string is not a valid UUID.
    pub fn parse(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl Default for ClusterId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ClusterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for ClusterId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// Taxonomic identifier for a species.
///
/// Uses scientific naming conventions (e.g., "Turdus_migratorius" for American Robin).
/// Can also represent higher taxonomic levels (genus, family, order).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaxonId(String);

impl TaxonId {
    /// Creates a new `TaxonId` from a string.
    ///
    /// # Arguments
    ///
    /// * `id` - The taxonomic identifier string
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the inner string value.
    #[must_use]
    pub fn inner(&self) -> &str {
        &self.0
    }

    /// Returns the inner string, consuming self.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Checks if this is a species-level taxon (contains underscore).
    #[must_use]
    pub fn is_species(&self) -> bool {
        self.0.contains('_')
    }

    /// Extracts the genus from a species-level taxon.
    ///
    /// Returns `None` if this is not a species-level taxon.
    #[must_use]
    pub fn genus(&self) -> Option<&str> {
        if self.is_species() {
            self.0.split('_').next()
        } else {
            None
        }
    }

    /// Extracts the specific epithet from a species-level taxon.
    ///
    /// Returns `None` if this is not a species-level taxon.
    #[must_use]
    pub fn specific_epithet(&self) -> Option<&str> {
        if self.is_species() {
            self.0.split('_').nth(1)
        } else {
            None
        }
    }
}

impl fmt::Display for TaxonId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for TaxonId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for TaxonId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for TaxonId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// =============================================================================
// Value Objects
// =============================================================================

/// A UTC timestamp with nanosecond precision.
///
/// Used for recording timestamps, event times, and temporal queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    /// Creates a `Timestamp` for the current moment.
    #[must_use]
    pub fn now() -> Self {
        Self(Utc::now())
    }

    /// Creates a `Timestamp` from a `DateTime<Utc>`.
    #[must_use]
    pub const fn from_datetime(dt: DateTime<Utc>) -> Self {
        Self(dt)
    }

    /// Returns the inner `DateTime<Utc>` value.
    #[must_use]
    pub const fn inner(&self) -> DateTime<Utc> {
        self.0
    }

    /// Returns the Unix timestamp in seconds.
    #[must_use]
    pub fn unix_timestamp(&self) -> i64 {
        self.0.timestamp()
    }

    /// Returns the Unix timestamp in milliseconds.
    #[must_use]
    pub fn unix_timestamp_millis(&self) -> i64 {
        self.0.timestamp_millis()
    }

    /// Parses a `Timestamp` from an RFC 3339 string.
    ///
    /// # Errors
    ///
    /// Returns an error if the string is not a valid RFC 3339 timestamp.
    pub fn parse_rfc3339(s: &str) -> Result<Self, chrono::ParseError> {
        Ok(Self(DateTime::parse_from_rfc3339(s)?.with_timezone(&Utc)))
    }

    /// Formats the timestamp as an RFC 3339 string.
    #[must_use]
    pub fn to_rfc3339(&self) -> String {
        self.0.to_rfc3339()
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::now()
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_rfc3339())
    }
}

impl From<DateTime<Utc>> for Timestamp {
    fn from(dt: DateTime<Utc>) -> Self {
        Self(dt)
    }
}

impl From<Timestamp> for DateTime<Utc> {
    fn from(ts: Timestamp) -> Self {
        ts.0
    }
}

/// Geographic location with optional elevation.
///
/// Coordinates use WGS84 datum (standard GPS coordinates).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoLocation {
    /// Latitude in decimal degrees (-90 to 90).
    lat: f64,
    /// Longitude in decimal degrees (-180 to 180).
    lon: f64,
    /// Elevation above sea level in meters.
    elevation_m: Option<f32>,
}

impl GeoLocation {
    /// Creates a new `GeoLocation`.
    ///
    /// # Arguments
    ///
    /// * `lat` - Latitude in decimal degrees
    /// * `lon` - Longitude in decimal degrees
    /// * `elevation_m` - Optional elevation in meters
    ///
    /// # Panics
    ///
    /// Panics if latitude is not in range [-90, 90] or longitude is not in range [-180, 180].
    #[must_use]
    pub fn new(lat: f64, lon: f64, elevation_m: Option<f32>) -> Self {
        assert!(
            (-90.0..=90.0).contains(&lat),
            "Latitude must be between -90 and 90 degrees"
        );
        assert!(
            (-180.0..=180.0).contains(&lon),
            "Longitude must be between -180 and 180 degrees"
        );
        Self {
            lat,
            lon,
            elevation_m,
        }
    }

    /// Creates a new `GeoLocation`, returning `None` if coordinates are invalid.
    #[must_use]
    pub fn try_new(lat: f64, lon: f64, elevation_m: Option<f32>) -> Option<Self> {
        if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) {
            Some(Self {
                lat,
                lon,
                elevation_m,
            })
        } else {
            None
        }
    }

    /// Returns the latitude.
    #[must_use]
    pub const fn lat(&self) -> f64 {
        self.lat
    }

    /// Returns the longitude.
    #[must_use]
    pub const fn lon(&self) -> f64 {
        self.lon
    }

    /// Returns the elevation in meters, if available.
    #[must_use]
    pub const fn elevation_m(&self) -> Option<f32> {
        self.elevation_m
    }

    /// Calculates the Haversine distance to another location in meters.
    #[must_use]
    pub fn distance_to(&self, other: &Self) -> f64 {
        const EARTH_RADIUS_M: f64 = 6_371_000.0;

        let lat1_rad = self.lat.to_radians();
        let lat2_rad = other.lat.to_radians();
        let delta_lat = (other.lat - self.lat).to_radians();
        let delta_lon = (other.lon - self.lon).to_radians();

        let a = (delta_lat / 2.0).sin().powi(2)
            + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().asin();

        EARTH_RADIUS_M * c
    }
}

impl fmt::Display for GeoLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.elevation_m {
            Some(elev) => write!(f, "({:.6}, {:.6}, {:.1}m)", self.lat, self.lon, elev),
            None => write!(f, "({:.6}, {:.6})", self.lat, self.lon),
        }
    }
}

/// Supported audio formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    /// WAV format (uncompressed PCM).
    Wav,
    /// FLAC format (lossless compression).
    Flac,
    /// MP3 format (lossy compression).
    Mp3,
    /// Ogg Vorbis format (lossy compression).
    Ogg,
    /// Opus format (lossy compression, optimized for speech/audio).
    Opus,
}

impl AudioFormat {
    /// Returns the file extension for this format.
    #[must_use]
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Flac => "flac",
            Self::Mp3 => "mp3",
            Self::Ogg => "ogg",
            Self::Opus => "opus",
        }
    }

    /// Returns the MIME type for this format.
    #[must_use]
    pub const fn mime_type(&self) -> &'static str {
        match self {
            Self::Wav => "audio/wav",
            Self::Flac => "audio/flac",
            Self::Mp3 => "audio/mpeg",
            Self::Ogg => "audio/ogg",
            Self::Opus => "audio/opus",
        }
    }

    /// Returns whether this format uses lossless compression.
    #[must_use]
    pub const fn is_lossless(&self) -> bool {
        matches!(self, Self::Wav | Self::Flac)
    }
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.extension().to_uppercase())
    }
}

/// Technical metadata for an audio file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AudioMetadata {
    /// Sample rate in Hz (e.g., 44100, 48000).
    sample_rate: u32,
    /// Number of audio channels (1 = mono, 2 = stereo).
    channels: u8,
    /// Duration in milliseconds.
    duration_ms: u64,
    /// Audio file format.
    format: AudioFormat,
}

impl AudioMetadata {
    /// Creates new `AudioMetadata`.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Sample rate in Hz
    /// * `channels` - Number of audio channels
    /// * `duration_ms` - Duration in milliseconds
    /// * `format` - Audio file format
    #[must_use]
    pub const fn new(sample_rate: u32, channels: u8, duration_ms: u64, format: AudioFormat) -> Self {
        Self {
            sample_rate,
            channels,
            duration_ms,
            format,
        }
    }

    /// Returns the sample rate in Hz.
    #[must_use]
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Returns the number of channels.
    #[must_use]
    pub const fn channels(&self) -> u8 {
        self.channels
    }

    /// Returns the duration in milliseconds.
    #[must_use]
    pub const fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    /// Returns the duration in seconds.
    #[must_use]
    pub fn duration_secs(&self) -> f64 {
        self.duration_ms as f64 / 1000.0
    }

    /// Returns the audio format.
    #[must_use]
    pub const fn format(&self) -> AudioFormat {
        self.format
    }

    /// Returns the total number of samples (all channels combined).
    #[must_use]
    pub fn total_samples(&self) -> u64 {
        let samples_per_channel =
            (self.sample_rate as u64 * self.duration_ms) / 1000;
        samples_per_channel * self.channels as u64
    }

    /// Returns whether this is mono audio.
    #[must_use]
    pub const fn is_mono(&self) -> bool {
        self.channels == 1
    }

    /// Returns whether this is stereo audio.
    #[must_use]
    pub const fn is_stereo(&self) -> bool {
        self.channels == 2
    }
}

impl fmt::Display for AudioMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}Hz {}ch {:.2}s",
            self.format,
            self.sample_rate,
            self.channels,
            self.duration_secs()
        )
    }
}

/// Time range within an audio recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeRange {
    /// Start time in milliseconds from the beginning of the recording.
    start_ms: u64,
    /// End time in milliseconds from the beginning of the recording.
    end_ms: u64,
}

impl TimeRange {
    /// Creates a new `TimeRange`.
    ///
    /// # Arguments
    ///
    /// * `start_ms` - Start time in milliseconds
    /// * `end_ms` - End time in milliseconds
    ///
    /// # Panics
    ///
    /// Panics if `start_ms` >= `end_ms`.
    #[must_use]
    pub fn new(start_ms: u64, end_ms: u64) -> Self {
        assert!(start_ms < end_ms, "Start time must be before end time");
        Self { start_ms, end_ms }
    }

    /// Creates a new `TimeRange`, returning `None` if invalid.
    #[must_use]
    pub fn try_new(start_ms: u64, end_ms: u64) -> Option<Self> {
        if start_ms < end_ms {
            Some(Self { start_ms, end_ms })
        } else {
            None
        }
    }

    /// Returns the start time in milliseconds.
    #[must_use]
    pub const fn start_ms(&self) -> u64 {
        self.start_ms
    }

    /// Returns the end time in milliseconds.
    #[must_use]
    pub const fn end_ms(&self) -> u64 {
        self.end_ms
    }

    /// Returns the duration in milliseconds.
    #[must_use]
    pub const fn duration_ms(&self) -> u64 {
        self.end_ms - self.start_ms
    }

    /// Returns the duration in seconds.
    #[must_use]
    pub fn duration_secs(&self) -> f64 {
        self.duration_ms() as f64 / 1000.0
    }

    /// Checks if this range overlaps with another.
    #[must_use]
    pub const fn overlaps(&self, other: &Self) -> bool {
        self.start_ms < other.end_ms && other.start_ms < self.end_ms
    }

    /// Checks if this range contains a point in time.
    #[must_use]
    pub const fn contains(&self, time_ms: u64) -> bool {
        time_ms >= self.start_ms && time_ms < self.end_ms
    }
}

impl fmt::Display for TimeRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:.3}s - {:.3}s",
            self.start_ms as f64 / 1000.0,
            self.end_ms as f64 / 1000.0
        )
    }
}

/// Confidence score for predictions (0.0 to 1.0).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Confidence(f32);

impl Confidence {
    /// Creates a new `Confidence` score.
    ///
    /// # Arguments
    ///
    /// * `value` - Confidence value between 0.0 and 1.0
    ///
    /// # Panics
    ///
    /// Panics if value is not in range [0.0, 1.0].
    #[must_use]
    pub fn new(value: f32) -> Self {
        assert!(
            (0.0..=1.0).contains(&value),
            "Confidence must be between 0.0 and 1.0"
        );
        Self(value)
    }

    /// Creates a new `Confidence` score, clamping to valid range.
    #[must_use]
    pub fn clamped(value: f32) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    /// Returns the inner value.
    #[must_use]
    pub const fn value(&self) -> f32 {
        self.0
    }

    /// Returns the confidence as a percentage (0-100).
    #[must_use]
    pub fn percentage(&self) -> f32 {
        self.0 * 100.0
    }

    /// Checks if this is a high confidence prediction (>= 0.8).
    #[must_use]
    pub fn is_high(&self) -> bool {
        self.0 >= 0.8
    }

    /// Checks if this is a medium confidence prediction (>= 0.5 and < 0.8).
    #[must_use]
    pub fn is_medium(&self) -> bool {
        self.0 >= 0.5 && self.0 < 0.8
    }

    /// Checks if this is a low confidence prediction (< 0.5).
    #[must_use]
    pub fn is_low(&self) -> bool {
        self.0 < 0.5
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1}%", self.percentage())
    }
}

impl Default for Confidence {
    fn default() -> Self {
        Self(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recording_id_new() {
        let id1 = RecordingId::new();
        let id2 = RecordingId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_recording_id_parse() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let id = RecordingId::parse(uuid_str).unwrap();
        assert_eq!(id.to_string(), uuid_str);
    }

    #[test]
    fn test_taxon_id_species() {
        let taxon = TaxonId::new("Turdus_migratorius");
        assert!(taxon.is_species());
        assert_eq!(taxon.genus(), Some("Turdus"));
        assert_eq!(taxon.specific_epithet(), Some("migratorius"));
    }

    #[test]
    fn test_taxon_id_genus() {
        let taxon = TaxonId::new("Turdus");
        assert!(!taxon.is_species());
        assert_eq!(taxon.genus(), None);
    }

    #[test]
    fn test_geo_location_valid() {
        let loc = GeoLocation::new(37.7749, -122.4194, Some(10.0));
        assert_eq!(loc.lat(), 37.7749);
        assert_eq!(loc.lon(), -122.4194);
        assert_eq!(loc.elevation_m(), Some(10.0));
    }

    #[test]
    #[should_panic(expected = "Latitude must be between")]
    fn test_geo_location_invalid_lat() {
        GeoLocation::new(91.0, 0.0, None);
    }

    #[test]
    fn test_geo_location_distance() {
        let sf = GeoLocation::new(37.7749, -122.4194, None);
        let la = GeoLocation::new(34.0522, -118.2437, None);
        let distance = sf.distance_to(&la);
        // Distance should be approximately 559 km
        assert!((distance - 559_000.0).abs() < 10_000.0);
    }

    #[test]
    fn test_audio_metadata() {
        let meta = AudioMetadata::new(48000, 1, 30000, AudioFormat::Wav);
        assert_eq!(meta.sample_rate(), 48000);
        assert_eq!(meta.channels(), 1);
        assert_eq!(meta.duration_ms(), 30000);
        assert_eq!(meta.duration_secs(), 30.0);
        assert!(meta.is_mono());
        assert!(!meta.is_stereo());
        assert_eq!(meta.total_samples(), 48000 * 30);
    }

    #[test]
    fn test_time_range() {
        let range = TimeRange::new(1000, 5000);
        assert_eq!(range.duration_ms(), 4000);
        assert_eq!(range.duration_secs(), 4.0);
        assert!(range.contains(2000));
        assert!(!range.contains(5000));
    }

    #[test]
    fn test_time_range_overlap() {
        let range1 = TimeRange::new(1000, 5000);
        let range2 = TimeRange::new(4000, 8000);
        let range3 = TimeRange::new(6000, 9000);
        assert!(range1.overlaps(&range2));
        assert!(!range1.overlaps(&range3));
    }

    #[test]
    fn test_confidence() {
        let high = Confidence::new(0.9);
        let medium = Confidence::new(0.6);
        let low = Confidence::new(0.3);

        assert!(high.is_high());
        assert!(medium.is_medium());
        assert!(low.is_low());
        assert_eq!(high.percentage(), 90.0);
    }

    #[test]
    fn test_timestamp() {
        let ts = Timestamp::now();
        let parsed = Timestamp::parse_rfc3339(&ts.to_rfc3339()).unwrap();
        assert_eq!(ts, parsed);
    }
}
