//! Value objects for the 7sense domain.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// A geographic location with latitude, longitude, and optional elevation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeoLocation {
    /// Latitude in degrees (-90 to 90).
    latitude: f64,
    /// Longitude in degrees (-180 to 180).
    longitude: f64,
    /// Elevation above sea level in meters.
    elevation_m: Option<f64>,
}

impl GeoLocation {
    /// Creates a new GeoLocation with validation.
    pub fn new(latitude: f64, longitude: f64, elevation_m: Option<f64>) -> Result<Self, GeoLocationError> {
        if !(-90.0..=90.0).contains(&latitude) {
            return Err(GeoLocationError::InvalidLatitude(latitude));
        }
        if !(-180.0..=180.0).contains(&longitude) {
            return Err(GeoLocationError::InvalidLongitude(longitude));
        }
        Ok(Self { latitude, longitude, elevation_m })
    }

    /// Creates a GeoLocation without validation.
    #[must_use]
    pub const fn new_unchecked(latitude: f64, longitude: f64, elevation_m: Option<f64>) -> Self {
        Self { latitude, longitude, elevation_m }
    }

    /// Returns the latitude.
    #[must_use]
    pub const fn latitude(&self) -> f64 {
        self.latitude
    }

    /// Returns the longitude.
    #[must_use]
    pub const fn longitude(&self) -> f64 {
        self.longitude
    }

    /// Returns the elevation in meters.
    #[must_use]
    pub const fn elevation_m(&self) -> Option<f64> {
        self.elevation_m
    }

    /// Calculates Haversine distance to another location in km.
    #[must_use]
    pub fn distance_km(&self, other: &GeoLocation) -> f64 {
        const R: f64 = 6371.0;
        let lat1 = self.latitude.to_radians();
        let lat2 = other.latitude.to_radians();
        let dlat = (other.latitude - self.latitude).to_radians();
        let dlon = (other.longitude - self.longitude).to_radians();

        let a = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
        R * 2.0 * a.sqrt().asin()
    }
}

/// Errors for GeoLocation creation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum GeoLocationError {
    /// Invalid latitude value.
    #[error("Invalid latitude {0}: must be between -90 and 90")]
    InvalidLatitude(f64),
    /// Invalid longitude value.
    #[error("Invalid longitude {0}: must be between -180 and 180")]
    InvalidLongitude(f64),
}

/// A timestamp wrapper with domain operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    /// Creates a Timestamp for now.
    #[must_use]
    pub fn now() -> Self {
        Self(Utc::now())
    }

    /// Creates from DateTime.
    #[must_use]
    pub const fn from_datetime(dt: DateTime<Utc>) -> Self {
        Self(dt)
    }

    /// Returns the DateTime.
    #[must_use]
    pub const fn as_datetime(&self) -> &DateTime<Utc> {
        &self.0
    }

    /// Returns Unix timestamp in seconds.
    #[must_use]
    pub fn unix_timestamp(&self) -> i64 {
        self.0.timestamp()
    }

    /// Returns Unix timestamp in milliseconds.
    #[must_use]
    pub fn unix_timestamp_millis(&self) -> i64 {
        self.0.timestamp_millis()
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::now()
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.format("%Y-%m-%dT%H:%M:%S%.3fZ"))
    }
}

impl From<DateTime<Utc>> for Timestamp {
    fn from(dt: DateTime<Utc>) -> Self {
        Self(dt)
    }
}

/// Metadata about an audio file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioMetadata {
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Number of channels.
    pub channels: u16,
    /// Bits per sample.
    pub bits_per_sample: u16,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// File format (e.g., "wav", "flac").
    pub format: String,
    /// File size in bytes.
    pub file_size_bytes: u64,
    /// Codec information.
    pub codec: Option<String>,
}

impl AudioMetadata {
    /// Creates new AudioMetadata.
    #[must_use]
    pub fn new(
        sample_rate: u32,
        channels: u16,
        bits_per_sample: u16,
        duration_ms: u64,
        format: String,
        file_size_bytes: u64,
    ) -> Self {
        Self {
            sample_rate,
            channels,
            bits_per_sample,
            duration_ms,
            format,
            file_size_bytes,
            codec: None,
        }
    }

    /// Sets codec information.
    #[must_use]
    pub fn with_codec(mut self, codec: impl Into<String>) -> Self {
        self.codec = Some(codec.into());
        self
    }

    /// Returns duration as std Duration.
    #[must_use]
    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.duration_ms)
    }

    /// Returns total sample count.
    #[must_use]
    pub fn total_samples(&self) -> u64 {
        (self.sample_rate as u64 * self.duration_ms) / 1000
    }
}

/// Confidence score (0.0 to 1.0).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Confidence(f32);

impl Confidence {
    /// Creates a new Confidence with validation.
    pub fn new(value: f32) -> Result<Self, ConfidenceError> {
        if !(0.0..=1.0).contains(&value) {
            return Err(ConfidenceError::OutOfRange(value));
        }
        Ok(Self(value))
    }

    /// Creates a Confidence, clamping to valid range.
    #[must_use]
    pub fn clamped(value: f32) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    /// Returns the value.
    #[must_use]
    pub const fn value(&self) -> f32 {
        self.0
    }

    /// Returns as percentage.
    #[must_use]
    pub fn percentage(&self) -> f32 {
        self.0 * 100.0
    }
}

/// Errors for Confidence creation.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfidenceError {
    /// Value out of range.
    #[error("Confidence {0} out of range [0.0, 1.0]")]
    OutOfRange(f32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geolocation() {
        let loc = GeoLocation::new(45.5, -122.6, None).unwrap();
        assert_eq!(loc.latitude(), 45.5);
    }

    #[test]
    fn test_audio_metadata() {
        let meta = AudioMetadata::new(32000, 1, 16, 5000, "wav".to_string(), 320000);
        assert_eq!(meta.total_samples(), 160000);
    }

    #[test]
    fn test_confidence() {
        let conf = Confidence::new(0.85).unwrap();
        assert_eq!(conf.percentage(), 85.0);
    }
}
