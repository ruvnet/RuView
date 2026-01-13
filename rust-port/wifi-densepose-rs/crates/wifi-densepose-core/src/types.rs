//! Core data types for the WiFi-DensePose system.
//!
//! This module defines the fundamental data structures used throughout the
//! WiFi-DensePose ecosystem for representing CSI data, processed signals,
//! and pose estimation results.
//!
//! # Type Categories
//!
//! - **CSI Types**: [`CsiFrame`], [`CsiMetadata`], [`AntennaConfig`]
//! - **Signal Types**: [`ProcessedSignal`], [`SignalFeatures`], [`FrequencyBand`]
//! - **Pose Types**: [`PoseEstimate`], [`PersonPose`], [`Keypoint`], [`KeypointType`]
//! - **Common Types**: [`Confidence`], [`Timestamp`], [`FrameId`], [`DeviceId`]

use chrono::{DateTime, Utc};
use ndarray::{Array1, Array2, Array3};
use num_complex::Complex64;
use uuid::Uuid;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};
use crate::{DEFAULT_CONFIDENCE_THRESHOLD, MAX_KEYPOINTS};

// =============================================================================
// Common Types
// =============================================================================

/// Unique identifier for a CSI frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FrameId(Uuid);

impl FrameId {
    /// Creates a new unique frame ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates a frame ID from an existing UUID.
    #[must_use]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the inner UUID.
    #[must_use]
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for FrameId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for FrameId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a `WiFi` device.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DeviceId(String);

impl DeviceId {
    /// Creates a new device ID from a string.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the device ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// High-precision timestamp for CSI data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Timestamp {
    /// Seconds since Unix epoch
    pub seconds: i64,
    /// Nanoseconds within the second
    pub nanos: u32,
}

impl Timestamp {
    /// Creates a new timestamp from seconds and nanoseconds.
    #[must_use]
    pub fn new(seconds: i64, nanos: u32) -> Self {
        Self { seconds, nanos }
    }

    /// Creates a timestamp from the current time.
    #[must_use]
    pub fn now() -> Self {
        let now = Utc::now();
        Self {
            seconds: now.timestamp(),
            nanos: now.timestamp_subsec_nanos(),
        }
    }

    /// Creates a timestamp from a `DateTime<Utc>`.
    #[must_use]
    pub fn from_datetime(dt: DateTime<Utc>) -> Self {
        Self {
            seconds: dt.timestamp(),
            nanos: dt.timestamp_subsec_nanos(),
        }
    }

    /// Converts to `DateTime<Utc>`.
    #[must_use]
    pub fn to_datetime(&self) -> Option<DateTime<Utc>> {
        DateTime::from_timestamp(self.seconds, self.nanos)
    }

    /// Returns the timestamp as total nanoseconds since epoch.
    #[must_use]
    pub fn as_nanos(&self) -> i128 {
        i128::from(self.seconds) * 1_000_000_000 + i128::from(self.nanos)
    }

    /// Returns the duration between two timestamps in seconds.
    #[must_use]
    pub fn duration_since(&self, earlier: &Self) -> f64 {
        let diff_nanos = self.as_nanos() - earlier.as_nanos();
        diff_nanos as f64 / 1_000_000_000.0
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::now()
    }
}

/// Confidence score in the range [0.0, 1.0].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Confidence(f32);

impl Confidence {
    /// Creates a new confidence value.
    ///
    /// # Errors
    ///
    /// Returns an error if the value is not in the range [0.0, 1.0].
    pub fn new(value: f32) -> CoreResult<Self> {
        if !(0.0..=1.0).contains(&value) {
            return Err(CoreError::validation(format!(
                "Confidence must be in [0.0, 1.0], got {value}"
            )));
        }
        Ok(Self(value))
    }

    /// Creates a confidence value without validation (for internal use).
    ///
    /// # Safety
    ///
    /// The caller must ensure the value is in [0.0, 1.0].
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn new_unchecked(value: f32) -> Self {
        debug_assert!((0.0..=1.0).contains(&value));
        Self(value)
    }

    /// Returns the raw confidence value.
    #[must_use]
    pub fn value(&self) -> f32 {
        self.0
    }

    /// Returns `true` if the confidence exceeds the default threshold.
    #[must_use]
    pub fn is_high(&self) -> bool {
        self.0 >= DEFAULT_CONFIDENCE_THRESHOLD
    }

    /// Returns `true` if the confidence exceeds the given threshold.
    #[must_use]
    pub fn exceeds(&self, threshold: f32) -> bool {
        self.0 >= threshold
    }

    /// Maximum confidence (1.0).
    pub const MAX: Self = Self(1.0);

    /// Minimum confidence (0.0).
    pub const MIN: Self = Self(0.0);
}

impl Default for Confidence {
    fn default() -> Self {
        Self(0.0)
    }
}

// =============================================================================
// CSI Types
// =============================================================================

/// `WiFi` frequency band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FrequencyBand {
    /// 2.4 GHz band (802.11b/g/n)
    Band2_4GHz,
    /// 5 GHz band (802.11a/n/ac)
    Band5GHz,
    /// 6 GHz band (802.11ax/WiFi 6E)
    Band6GHz,
}

impl FrequencyBand {
    /// Returns the center frequency in MHz.
    #[must_use]
    pub fn center_frequency_mhz(&self) -> u32 {
        match self {
            Self::Band2_4GHz => 2437,
            Self::Band5GHz => 5180,
            Self::Band6GHz => 5975,
        }
    }

    /// Returns the typical number of subcarriers for this band.
    #[must_use]
    pub fn typical_subcarriers(&self) -> usize {
        match self {
            Self::Band2_4GHz => 56,
            Self::Band5GHz => 114,
            Self::Band6GHz => 234,
        }
    }
}

/// Antenna configuration for MIMO systems.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AntennaConfig {
    /// Number of transmit antennas
    pub tx_antennas: u8,
    /// Number of receive antennas
    pub rx_antennas: u8,
    /// Antenna spacing in millimeters (if known)
    pub spacing_mm: Option<f32>,
}

impl AntennaConfig {
    /// Creates a new antenna configuration.
    #[must_use]
    pub fn new(tx_antennas: u8, rx_antennas: u8) -> Self {
        Self {
            tx_antennas,
            rx_antennas,
            spacing_mm: None,
        }
    }

    /// Sets the antenna spacing.
    #[must_use]
    pub fn with_spacing(mut self, spacing_mm: f32) -> Self {
        self.spacing_mm = Some(spacing_mm);
        self
    }

    /// Returns the total number of spatial streams.
    #[must_use]
    pub fn spatial_streams(&self) -> usize {
        usize::from(self.tx_antennas) * usize::from(self.rx_antennas)
    }

    /// Common 1x3 SIMO configuration.
    pub const SIMO_1X3: Self = Self {
        tx_antennas: 1,
        rx_antennas: 3,
        spacing_mm: None,
    };

    /// Common 2x2 MIMO configuration.
    pub const MIMO_2X2: Self = Self {
        tx_antennas: 2,
        rx_antennas: 2,
        spacing_mm: None,
    };

    /// Common 3x3 MIMO configuration.
    pub const MIMO_3X3: Self = Self {
        tx_antennas: 3,
        rx_antennas: 3,
        spacing_mm: None,
    };
}

impl Default for AntennaConfig {
    fn default() -> Self {
        Self::SIMO_1X3
    }
}

/// Metadata associated with a CSI frame.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CsiMetadata {
    /// Timestamp when the frame was captured
    pub timestamp: Timestamp,
    /// Source device identifier
    pub device_id: DeviceId,
    /// Frequency band
    pub frequency_band: FrequencyBand,
    /// Channel number
    pub channel: u8,
    /// Bandwidth in MHz
    pub bandwidth_mhz: u16,
    /// Antenna configuration
    pub antenna_config: AntennaConfig,
    /// Received Signal Strength Indicator (dBm)
    pub rssi_dbm: i8,
    /// Noise floor (dBm)
    pub noise_floor_dbm: i8,
    /// Frame sequence number
    pub sequence_number: u32,
}

impl CsiMetadata {
    /// Creates new CSI metadata with required fields.
    #[must_use]
    pub fn new(device_id: DeviceId, frequency_band: FrequencyBand, channel: u8) -> Self {
        Self {
            timestamp: Timestamp::now(),
            device_id,
            frequency_band,
            channel,
            bandwidth_mhz: 20,
            antenna_config: AntennaConfig::default(),
            rssi_dbm: -50,
            noise_floor_dbm: -90,
            sequence_number: 0,
        }
    }

    /// Returns the Signal-to-Noise Ratio in dB.
    #[must_use]
    pub fn snr_db(&self) -> f64 {
        f64::from(self.rssi_dbm) - f64::from(self.noise_floor_dbm)
    }
}

/// A single frame of Channel State Information (CSI) data.
///
/// CSI captures the frequency response of the wireless channel, encoding
/// information about signal amplitude and phase across multiple subcarriers
/// and antenna pairs.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CsiFrame {
    /// Unique frame identifier
    pub id: FrameId,
    /// Frame metadata
    pub metadata: CsiMetadata,
    /// Complex CSI data: [spatial_streams, subcarriers]
    #[cfg_attr(feature = "serde", serde(skip))]
    pub data: Array2<Complex64>,
    /// Amplitude data (magnitude of complex values)
    #[cfg_attr(feature = "serde", serde(skip))]
    pub amplitude: Array2<f64>,
    /// Phase data (angle of complex values, in radians)
    #[cfg_attr(feature = "serde", serde(skip))]
    pub phase: Array2<f64>,
}

impl CsiFrame {
    /// Creates a new CSI frame from raw complex data.
    pub fn new(metadata: CsiMetadata, data: Array2<Complex64>) -> Self {
        let amplitude = data.mapv(num_complex::Complex::norm);
        let phase = data.mapv(num_complex::Complex::arg);

        Self {
            id: FrameId::new(),
            metadata,
            data,
            amplitude,
            phase,
        }
    }

    /// Returns the number of spatial streams (antenna pairs).
    #[must_use]
    pub fn num_spatial_streams(&self) -> usize {
        self.data.nrows()
    }

    /// Returns the number of subcarriers.
    #[must_use]
    pub fn num_subcarriers(&self) -> usize {
        self.data.ncols()
    }

    /// Returns the mean amplitude across all subcarriers and streams.
    #[must_use]
    pub fn mean_amplitude(&self) -> f64 {
        self.amplitude.mean().unwrap_or(0.0)
    }

    /// Returns the amplitude variance, useful for motion detection.
    #[must_use]
    pub fn amplitude_variance(&self) -> f64 {
        self.amplitude.var(0.0)
    }
}

// =============================================================================
// Signal Types
// =============================================================================

/// Features extracted from processed CSI signals.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SignalFeatures {
    /// Doppler velocity estimates (m/s)
    pub doppler_velocities: Vec<f64>,
    /// Time-of-flight estimates (ns)
    pub time_of_flight: Vec<f64>,
    /// Angle-of-arrival estimates (radians)
    pub angle_of_arrival: Vec<f64>,
    /// Motion detection confidence
    pub motion_confidence: Confidence,
    /// Presence detection confidence
    pub presence_confidence: Confidence,
    /// Number of detected bodies
    pub body_count: u8,
}

impl Default for SignalFeatures {
    fn default() -> Self {
        Self {
            doppler_velocities: Vec::new(),
            time_of_flight: Vec::new(),
            angle_of_arrival: Vec::new(),
            motion_confidence: Confidence::MIN,
            presence_confidence: Confidence::MIN,
            body_count: 0,
        }
    }
}

/// Processed CSI signal ready for neural network inference.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ProcessedSignal {
    /// Source frame IDs that contributed to this processed signal
    pub source_frame_ids: Vec<FrameId>,
    /// Timestamp of the most recent source frame
    pub timestamp: Timestamp,
    /// Processed amplitude tensor: [time_steps, spatial_streams, subcarriers]
    #[cfg_attr(feature = "serde", serde(skip))]
    pub amplitude_tensor: Array3<f32>,
    /// Processed phase tensor: [time_steps, spatial_streams, subcarriers]
    #[cfg_attr(feature = "serde", serde(skip))]
    pub phase_tensor: Array3<f32>,
    /// Extracted signal features
    pub features: SignalFeatures,
    /// Device that captured this data
    pub device_id: DeviceId,
}

impl ProcessedSignal {
    /// Creates a new processed signal.
    #[must_use]
    pub fn new(
        source_frame_ids: Vec<FrameId>,
        timestamp: Timestamp,
        amplitude_tensor: Array3<f32>,
        phase_tensor: Array3<f32>,
        device_id: DeviceId,
    ) -> Self {
        Self {
            source_frame_ids,
            timestamp,
            amplitude_tensor,
            phase_tensor,
            features: SignalFeatures::default(),
            device_id,
        }
    }

    /// Returns the shape of the signal tensor [time, streams, subcarriers].
    #[must_use]
    pub fn shape(&self) -> (usize, usize, usize) {
        let shape = self.amplitude_tensor.shape();
        (shape[0], shape[1], shape[2])
    }

    /// Returns the total number of time steps in the signal.
    #[must_use]
    pub fn num_time_steps(&self) -> usize {
        self.amplitude_tensor.shape()[0]
    }
}

// =============================================================================
// Pose Types
// =============================================================================

/// Types of body keypoints following COCO format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum KeypointType {
    /// Nose
    Nose = 0,
    /// Left eye
    LeftEye = 1,
    /// Right eye
    RightEye = 2,
    /// Left ear
    LeftEar = 3,
    /// Right ear
    RightEar = 4,
    /// Left shoulder
    LeftShoulder = 5,
    /// Right shoulder
    RightShoulder = 6,
    /// Left elbow
    LeftElbow = 7,
    /// Right elbow
    RightElbow = 8,
    /// Left wrist
    LeftWrist = 9,
    /// Right wrist
    RightWrist = 10,
    /// Left hip
    LeftHip = 11,
    /// Right hip
    RightHip = 12,
    /// Left knee
    LeftKnee = 13,
    /// Right knee
    RightKnee = 14,
    /// Left ankle
    LeftAnkle = 15,
    /// Right ankle
    RightAnkle = 16,
}

impl KeypointType {
    /// Returns all keypoint types in order.
    #[must_use]
    pub fn all() -> &'static [Self; MAX_KEYPOINTS] {
        &[
            Self::Nose,
            Self::LeftEye,
            Self::RightEye,
            Self::LeftEar,
            Self::RightEar,
            Self::LeftShoulder,
            Self::RightShoulder,
            Self::LeftElbow,
            Self::RightElbow,
            Self::LeftWrist,
            Self::RightWrist,
            Self::LeftHip,
            Self::RightHip,
            Self::LeftKnee,
            Self::RightKnee,
            Self::LeftAnkle,
            Self::RightAnkle,
        ]
    }

    /// Returns the keypoint name as a string.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Nose => "nose",
            Self::LeftEye => "left_eye",
            Self::RightEye => "right_eye",
            Self::LeftEar => "left_ear",
            Self::RightEar => "right_ear",
            Self::LeftShoulder => "left_shoulder",
            Self::RightShoulder => "right_shoulder",
            Self::LeftElbow => "left_elbow",
            Self::RightElbow => "right_elbow",
            Self::LeftWrist => "left_wrist",
            Self::RightWrist => "right_wrist",
            Self::LeftHip => "left_hip",
            Self::RightHip => "right_hip",
            Self::LeftKnee => "left_knee",
            Self::RightKnee => "right_knee",
            Self::LeftAnkle => "left_ankle",
            Self::RightAnkle => "right_ankle",
        }
    }

    /// Returns `true` if this is a face keypoint.
    #[must_use]
    pub fn is_face(&self) -> bool {
        matches!(
            self,
            Self::Nose | Self::LeftEye | Self::RightEye | Self::LeftEar | Self::RightEar
        )
    }

    /// Returns `true` if this is an upper body keypoint.
    #[must_use]
    pub fn is_upper_body(&self) -> bool {
        matches!(
            self,
            Self::LeftShoulder
                | Self::RightShoulder
                | Self::LeftElbow
                | Self::RightElbow
                | Self::LeftWrist
                | Self::RightWrist
        )
    }

    /// Returns `true` if this is a lower body keypoint.
    #[must_use]
    pub fn is_lower_body(&self) -> bool {
        matches!(
            self,
            Self::LeftHip
                | Self::RightHip
                | Self::LeftKnee
                | Self::RightKnee
                | Self::LeftAnkle
                | Self::RightAnkle
        )
    }
}

impl TryFrom<u8> for KeypointType {
    type Error = CoreError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Nose),
            1 => Ok(Self::LeftEye),
            2 => Ok(Self::RightEye),
            3 => Ok(Self::LeftEar),
            4 => Ok(Self::RightEar),
            5 => Ok(Self::LeftShoulder),
            6 => Ok(Self::RightShoulder),
            7 => Ok(Self::LeftElbow),
            8 => Ok(Self::RightElbow),
            9 => Ok(Self::LeftWrist),
            10 => Ok(Self::RightWrist),
            11 => Ok(Self::LeftHip),
            12 => Ok(Self::RightHip),
            13 => Ok(Self::LeftKnee),
            14 => Ok(Self::RightKnee),
            15 => Ok(Self::LeftAnkle),
            16 => Ok(Self::RightAnkle),
            _ => Err(CoreError::validation(format!(
                "Invalid keypoint type: {value}"
            ))),
        }
    }
}

/// A single body keypoint with position and confidence.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Keypoint {
    /// Type of keypoint
    pub keypoint_type: KeypointType,
    /// X coordinate (normalized 0.0-1.0 or absolute pixels)
    pub x: f32,
    /// Y coordinate (normalized 0.0-1.0 or absolute pixels)
    pub y: f32,
    /// Z coordinate (depth, if available)
    pub z: Option<f32>,
    /// Detection confidence
    pub confidence: Confidence,
}

impl Keypoint {
    /// Creates a new 2D keypoint.
    #[must_use]
    pub fn new(keypoint_type: KeypointType, x: f32, y: f32, confidence: Confidence) -> Self {
        Self {
            keypoint_type,
            x,
            y,
            z: None,
            confidence,
        }
    }

    /// Creates a new 3D keypoint.
    #[must_use]
    pub fn new_3d(
        keypoint_type: KeypointType,
        x: f32,
        y: f32,
        z: f32,
        confidence: Confidence,
    ) -> Self {
        Self {
            keypoint_type,
            x,
            y,
            z: Some(z),
            confidence,
        }
    }

    /// Returns `true` if this keypoint should be considered visible.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.confidence.is_high()
    }

    /// Returns the 2D position as a tuple.
    #[must_use]
    pub fn position_2d(&self) -> (f32, f32) {
        (self.x, self.y)
    }

    /// Returns the 3D position as a tuple, if available.
    #[must_use]
    pub fn position_3d(&self) -> Option<(f32, f32, f32)> {
        self.z.map(|z| (self.x, self.y, z))
    }

    /// Calculates the Euclidean distance to another keypoint.
    #[must_use]
    pub fn distance_to(&self, other: &Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        match (self.z, other.z) {
            (Some(z1), Some(z2)) => {
                let dz = z1 - z2;
                dz.mul_add(dz, dx.mul_add(dx, dy * dy)).sqrt()
            }
            _ => (dx * dx + dy * dy).sqrt(),
        }
    }
}

/// Axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BoundingBox {
    /// Left edge X coordinate
    pub x_min: f32,
    /// Top edge Y coordinate
    pub y_min: f32,
    /// Right edge X coordinate
    pub x_max: f32,
    /// Bottom edge Y coordinate
    pub y_max: f32,
}

impl BoundingBox {
    /// Creates a new bounding box.
    #[must_use]
    pub fn new(x_min: f32, y_min: f32, x_max: f32, y_max: f32) -> Self {
        Self {
            x_min,
            y_min,
            x_max,
            y_max,
        }
    }

    /// Creates a bounding box from center, width, and height.
    #[must_use]
    pub fn from_center(cx: f32, cy: f32, width: f32, height: f32) -> Self {
        let half_w = width / 2.0;
        let half_h = height / 2.0;
        Self {
            x_min: cx - half_w,
            y_min: cy - half_h,
            x_max: cx + half_w,
            y_max: cy + half_h,
        }
    }

    /// Returns the width of the bounding box.
    #[must_use]
    pub fn width(&self) -> f32 {
        self.x_max - self.x_min
    }

    /// Returns the height of the bounding box.
    #[must_use]
    pub fn height(&self) -> f32 {
        self.y_max - self.y_min
    }

    /// Returns the area of the bounding box.
    #[must_use]
    pub fn area(&self) -> f32 {
        self.width() * self.height()
    }

    /// Returns the center point of the bounding box.
    #[must_use]
    pub fn center(&self) -> (f32, f32) {
        ((self.x_min + self.x_max) / 2.0, (self.y_min + self.y_max) / 2.0)
    }

    /// Computes the Intersection over Union (IoU) with another bounding box.
    #[must_use]
    pub fn iou(&self, other: &Self) -> f32 {
        let x_min = self.x_min.max(other.x_min);
        let y_min = self.y_min.max(other.y_min);
        let x_max = self.x_max.min(other.x_max);
        let y_max = self.y_max.min(other.y_max);

        if x_max <= x_min || y_max <= y_min {
            return 0.0;
        }

        let intersection = (x_max - x_min) * (y_max - y_min);
        let union = self.area() + other.area() - intersection;

        if union <= 0.0 {
            0.0
        } else {
            intersection / union
        }
    }

    /// Returns `true` if the point is inside the bounding box.
    #[must_use]
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x_min && x <= self.x_max && y >= self.y_min && y <= self.y_max
    }
}

/// Pose estimation for a single person.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PersonPose {
    /// Unique identifier for this person (for tracking)
    pub id: Option<u32>,
    /// All detected keypoints
    pub keypoints: [Option<Keypoint>; MAX_KEYPOINTS],
    /// Bounding box around the person
    pub bounding_box: Option<BoundingBox>,
    /// Overall pose confidence
    pub confidence: Confidence,
}

impl PersonPose {
    /// Creates a new empty person pose.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: None,
            keypoints: [None; MAX_KEYPOINTS],
            bounding_box: None,
            confidence: Confidence::MIN,
        }
    }

    /// Sets a keypoint.
    pub fn set_keypoint(&mut self, keypoint: Keypoint) {
        let idx = keypoint.keypoint_type as usize;
        if idx < MAX_KEYPOINTS {
            self.keypoints[idx] = Some(keypoint);
        }
    }

    /// Gets a keypoint by type.
    #[must_use]
    pub fn get_keypoint(&self, keypoint_type: KeypointType) -> Option<&Keypoint> {
        self.keypoints[keypoint_type as usize].as_ref()
    }

    /// Returns the number of visible keypoints.
    #[must_use]
    pub fn visible_keypoint_count(&self) -> usize {
        self.keypoints
            .iter()
            .filter(|kp| kp.as_ref().is_some_and(Keypoint::is_visible))
            .count()
    }

    /// Returns all visible keypoints.
    #[must_use]
    pub fn visible_keypoints(&self) -> Vec<&Keypoint> {
        self.keypoints
            .iter()
            .filter_map(|kp| kp.as_ref())
            .filter(|kp| kp.is_visible())
            .collect()
    }

    /// Computes the bounding box from visible keypoints.
    #[must_use]
    pub fn compute_bounding_box(&self) -> Option<BoundingBox> {
        let visible: Vec<_> = self.visible_keypoints();
        if visible.is_empty() {
            return None;
        }

        let mut x_min = f32::MAX;
        let mut y_min = f32::MAX;
        let mut x_max = f32::MIN;
        let mut y_max = f32::MIN;

        for kp in visible {
            x_min = x_min.min(kp.x);
            y_min = y_min.min(kp.y);
            x_max = x_max.max(kp.x);
            y_max = y_max.max(kp.y);
        }

        Some(BoundingBox::new(x_min, y_min, x_max, y_max))
    }

    /// Converts keypoints to a flat array [x0, y0, conf0, x1, y1, conf1, ...].
    #[must_use]
    pub fn to_flat_array(&self) -> Array1<f32> {
        let mut arr = Array1::zeros(MAX_KEYPOINTS * 3);
        for (i, kp_opt) in self.keypoints.iter().enumerate() {
            if let Some(kp) = kp_opt {
                arr[i * 3] = kp.x;
                arr[i * 3 + 1] = kp.y;
                arr[i * 3 + 2] = kp.confidence.value();
            }
        }
        arr
    }
}

impl Default for PersonPose {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete pose estimation result for a frame.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PoseEstimate {
    /// Unique identifier for this estimate
    pub id: FrameId,
    /// Timestamp of the estimate
    pub timestamp: Timestamp,
    /// Source signal that produced this estimate
    pub source_signal_ids: Vec<FrameId>,
    /// All detected persons
    pub persons: Vec<PersonPose>,
    /// Overall inference confidence
    pub confidence: Confidence,
    /// Inference latency in milliseconds
    pub latency_ms: f32,
    /// Model version used for inference
    pub model_version: String,
}

impl PoseEstimate {
    /// Creates a new pose estimate.
    #[must_use]
    pub fn new(
        source_signal_ids: Vec<FrameId>,
        persons: Vec<PersonPose>,
        confidence: Confidence,
        latency_ms: f32,
        model_version: String,
    ) -> Self {
        Self {
            id: FrameId::new(),
            timestamp: Timestamp::now(),
            source_signal_ids,
            persons,
            confidence,
            latency_ms,
            model_version,
        }
    }

    /// Returns the number of detected persons.
    #[must_use]
    pub fn person_count(&self) -> usize {
        self.persons.len()
    }

    /// Returns `true` if any person was detected.
    #[must_use]
    pub fn has_detections(&self) -> bool {
        !self.persons.is_empty()
    }

    /// Returns the person with the highest confidence.
    #[must_use]
    pub fn highest_confidence_person(&self) -> Option<&PersonPose> {
        self.persons
            .iter()
            .max_by(|a, b| a.confidence.value().partial_cmp(&b.confidence.value()).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_validation() {
        assert!(Confidence::new(0.5).is_ok());
        assert!(Confidence::new(0.0).is_ok());
        assert!(Confidence::new(1.0).is_ok());
        assert!(Confidence::new(-0.1).is_err());
        assert!(Confidence::new(1.1).is_err());
    }

    #[test]
    fn test_confidence_threshold() {
        let high = Confidence::new(0.8).unwrap();
        let low = Confidence::new(0.3).unwrap();

        assert!(high.is_high());
        assert!(!low.is_high());
    }

    #[test]
    fn test_keypoint_distance() {
        let kp1 = Keypoint::new(KeypointType::Nose, 0.0, 0.0, Confidence::MAX);
        let kp2 = Keypoint::new(KeypointType::LeftEye, 3.0, 4.0, Confidence::MAX);

        let distance = kp1.distance_to(&kp2);
        assert!((distance - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_bounding_box_iou() {
        let box1 = BoundingBox::new(0.0, 0.0, 10.0, 10.0);
        let box2 = BoundingBox::new(5.0, 5.0, 15.0, 15.0);

        let iou = box1.iou(&box2);
        // Intersection: 5x5 = 25, Union: 100 + 100 - 25 = 175
        assert!((iou - 25.0 / 175.0).abs() < 0.001);
    }

    #[test]
    fn test_person_pose() {
        let mut pose = PersonPose::new();
        pose.set_keypoint(Keypoint::new(
            KeypointType::Nose,
            0.5,
            0.3,
            Confidence::new(0.95).unwrap(),
        ));
        pose.set_keypoint(Keypoint::new(
            KeypointType::LeftShoulder,
            0.4,
            0.5,
            Confidence::new(0.8).unwrap(),
        ));

        assert_eq!(pose.visible_keypoint_count(), 2);
        assert!(pose.get_keypoint(KeypointType::Nose).is_some());
        assert!(pose.get_keypoint(KeypointType::RightAnkle).is_none());
    }

    #[test]
    fn test_timestamp_duration() {
        let t1 = Timestamp::new(100, 0);
        let t2 = Timestamp::new(101, 500_000_000);

        let duration = t2.duration_since(&t1);
        assert!((duration - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_keypoint_type_conversion() {
        assert_eq!(KeypointType::try_from(0).unwrap(), KeypointType::Nose);
        assert_eq!(KeypointType::try_from(16).unwrap(), KeypointType::RightAnkle);
        assert!(KeypointType::try_from(17).is_err());
    }

    #[test]
    fn test_frequency_band() {
        assert_eq!(FrequencyBand::Band2_4GHz.typical_subcarriers(), 56);
        assert_eq!(FrequencyBand::Band5GHz.typical_subcarriers(), 114);
        assert!(FrequencyBand::Band5GHz.center_frequency_mhz() > 5000);
    }
}
