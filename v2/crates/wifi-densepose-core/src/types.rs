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
// ADR-136 — Canonical complex sample contract
// =============================================================================

/// Canonical complex sample for all RuView frame contracts (CSI, CIR, Doppler).
///
/// Wraps [`num_complex::Complex64`]. The `serde` impl and [`Self::to_le_bytes`]
/// write `(re, im)` as two little-endian `f64`, matching the ADR-119 endianness
/// guarantee so x86_64 (ruvultra), aarch64 (cognitum-v0), and Xtensa (ESP32-S3)
/// produce bit-identical bytes. Downstream `f32` paths (CIR taps, ADR-134;
/// NN inference, ADR-146) narrow on demand via [`Self::as_complex32`].
///
/// This is the *contract* representation used at stage boundaries and by the
/// deterministic [`CanonicalFrame`](crate::traits::CanonicalFrame) serialiser.
/// `CsiFrame.data` remains `Array2<Complex64>` for ndarray-native math.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(transparent)]
pub struct ComplexSample(pub Complex64);

impl ComplexSample {
    /// Construct from real/imaginary `f64` parts.
    #[must_use]
    pub fn new(re: f64, im: f64) -> Self {
        Self(Complex64::new(re, im))
    }

    /// Magnitude `|z|`.
    #[must_use]
    pub fn norm(&self) -> f64 {
        self.0.norm()
    }

    /// Phase angle `arg(z)` in radians.
    #[must_use]
    pub fn arg(&self) -> f64 {
        self.0.arg()
    }

    /// Narrow to `f32` complex for CIR (ADR-134) / NN (ADR-146) paths.
    ///
    /// This is a lossy *view*, never re-serialised as the witness form
    /// (ADR-136 §3.3 risk mitigation — one encoder only).
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // f64 -> f32 is the documented lossy narrowing above
    pub fn as_complex32(&self) -> num_complex::Complex32 {
        num_complex::Complex32::new(self.0.re as f32, self.0.im as f32)
    }

    /// Canonical 16-byte little-endian encoding: `re || im`, each `f64` LE.
    #[must_use]
    pub fn to_le_bytes(&self) -> [u8; 16] {
        let mut b = [0u8; 16];
        b[0..8].copy_from_slice(&self.0.re.to_le_bytes());
        b[8..16].copy_from_slice(&self.0.im.to_le_bytes());
        b
    }

    /// Decode from the canonical 16-byte little-endian encoding.
    #[must_use]
    pub fn from_le_bytes(b: [u8; 16]) -> Self {
        let mut re = [0u8; 8];
        let mut im = [0u8; 8];
        re.copy_from_slice(&b[0..8]);
        im.copy_from_slice(&b[8..16]);
        Self(Complex64::new(f64::from_le_bytes(re), f64::from_le_bytes(im)))
    }
}

impl From<Complex64> for ComplexSample {
    fn from(z: Complex64) -> Self {
        Self(z)
    }
}

impl From<ComplexSample> for Complex64 {
    fn from(s: ComplexSample) -> Self {
        s.0
    }
}

#[cfg(feature = "serde")]
impl Serialize for ComplexSample {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        // Two LE f64 — deterministic across architectures (ADR-136 §2.3).
        use serde::ser::SerializeTuple;
        let mut t = s.serialize_tuple(2)?;
        t.serialize_element(&self.0.re)?;
        t.serialize_element(&self.0.im)?;
        t.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for ComplexSample {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let (re, im) = <(f64, f64)>::deserialize(d)?;
        Ok(Self(Complex64::new(re, im)))
    }
}

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
#[cfg_attr(feature = "serde", serde(try_from = "f32"))]
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

impl TryFrom<f32> for Confidence {
    type Error = CoreError;

    fn try_from(value: f32) -> CoreResult<Self> {
        Self::new(value)
    }
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

    /// UUID of the ADR-135 empty-room baseline subtracted from this frame
    /// (ADR-136 §2.2). `None` ⇒ uncalibrated (no `BaselineCalibration::subtract()`
    /// applied). Set only by the calibration stage; append-only thereafter.
    #[cfg_attr(feature = "serde", serde(default))]
    pub calibration_id: Option<Uuid>,

    /// Identifier of the RF encoder / model family consuming this frame
    /// (ADR-136 §2.2, ADR-146). Stable across a deployment; `0` ⇒ unassigned.
    #[cfg_attr(feature = "serde", serde(default))]
    pub model_id: u16,

    /// Monotonic model version (ADR-119 §2.1 reserved-flag pattern: low byte
    /// minor, high byte major). `0` ⇒ unassigned. Set only by the model-binding
    /// stage; append-only thereafter.
    #[cfg_attr(feature = "serde", serde(default))]
    pub model_version: u16,
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
            // ADR-136 provenance: unassigned until calibration / model-binding stages.
            calibration_id: None,
            model_id: 0,
            model_version: 0,
        }
    }

    /// Binds the ADR-135 empty-room baseline that was subtracted from this
    /// frame (ADR-136 §2.4 boundary rule — only the calibration stage calls this).
    pub fn set_calibration(&mut self, calibration_id: Uuid) {
        self.calibration_id = Some(calibration_id);
    }

    /// Binds the RF model family/version that will consume this frame
    /// (ADR-136 §2.4 — only the model-binding stage calls this).
    pub fn set_model(&mut self, model_id: u16, model_version: u16) {
        self.model_id = model_id;
        self.model_version = model_version;
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

    /// Zero-allocation view of the complex payload as [`ComplexSample`]s in
    /// stream-major (`[stream][subcarrier]`) order — the canonical contract
    /// representation (ADR-136 §2.3) without copying the `ndarray` buffer.
    pub fn data_complex_samples(&self) -> impl Iterator<Item = ComplexSample> + '_ {
        self.data.iter().map(|z| ComplexSample(*z))
    }
}

impl crate::traits::CanonicalFrame for CsiFrame {
    /// Deterministic, architecture-independent encoding (ADR-136 §2.5).
    ///
    /// Layout: frame id (16 UUID bytes) ‖ metadata fields in declared order
    /// (each fixed-width LE; `device_id` length-prefixed; `calibration_id` as
    /// 16 UUID bytes or 16 zero bytes for `None`) ‖ `(nrows, ncols)` as u32 LE
    /// ‖ complex payload as `ComplexSample::to_le_bytes()` in stream-major order.
    ///
    /// # Panics
    /// If `calibration_id` is `Some(Uuid::nil())`: the nil UUID is the wire
    /// sentinel for `None`, so encoding it would alias two distinct frames to
    /// the same bytes (and the same witness hash) — a non-injective encoding
    /// is refused rather than silently produced.
    fn to_canonical_bytes(&self) -> Vec<u8> {
        let m = &self.metadata;
        // 16 (id) + ~48 (meta) + 8 (shape) + 16 * n_samples
        let mut b = Vec::with_capacity(88 + 16 * self.data.len());

        // Frame id.
        b.extend_from_slice(self.id.as_uuid().as_bytes());

        // Metadata, declared order.
        b.extend_from_slice(&m.timestamp.seconds.to_le_bytes());
        b.extend_from_slice(&m.timestamp.nanos.to_le_bytes());
        let dev = m.device_id.as_str().as_bytes();
        b.extend_from_slice(
            &u32::try_from(dev.len()).expect("device_id length fits u32").to_le_bytes(),
        );
        b.extend_from_slice(dev);
        b.push(match m.frequency_band {
            FrequencyBand::Band2_4GHz => 0,
            FrequencyBand::Band5GHz => 1,
            FrequencyBand::Band6GHz => 2,
        });
        b.push(m.channel);
        b.extend_from_slice(&m.bandwidth_mhz.to_le_bytes());
        b.push(m.antenna_config.tx_antennas);
        b.push(m.antenna_config.rx_antennas);
        if let Some(s) = m.antenna_config.spacing_mm {
            b.push(1);
            b.extend_from_slice(&s.to_le_bytes());
        } else {
            b.push(0);
            b.extend_from_slice(&[0u8; 4]);
        }
        b.extend_from_slice(&m.rssi_dbm.to_le_bytes());
        b.extend_from_slice(&m.noise_floor_dbm.to_le_bytes());
        b.extend_from_slice(&m.sequence_number.to_le_bytes());
        match m.calibration_id {
            Some(id) => {
                // Some(nil) would alias the None sentinel on the wire: the
                // bytes would decode to a *different* frame (calibration_id
                // None) with the same witness. Refuse the non-injective
                // encoding (see the trait-impl `# Panics` doc).
                assert!(
                    id != Uuid::nil(),
                    "calibration_id Some(Uuid::nil()) is unencodable: nil is the None sentinel"
                );
                b.extend_from_slice(id.as_bytes());
            }
            None => b.extend_from_slice(&[0u8; 16]),
        }
        b.extend_from_slice(&m.model_id.to_le_bytes());
        b.extend_from_slice(&m.model_version.to_le_bytes());

        // Shape, then complex payload stream-major.
        b.extend_from_slice(
            &u32::try_from(self.data.nrows()).expect("stream count fits u32").to_le_bytes(),
        );
        b.extend_from_slice(
            &u32::try_from(self.data.ncols()).expect("subcarrier count fits u32").to_le_bytes(),
        );
        for sample in self.data_complex_samples() {
            b.extend_from_slice(&sample.to_le_bytes());
        }
        b
    }
}

/// Errors decoding a frame from its canonical bytes.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CanonicalDecodeError {
    /// The buffer ended before the layout was fully read.
    #[error("canonical buffer truncated at byte {at} (need {need} more)")]
    Truncated {
        /// Byte offset where reading failed.
        at: usize,
        /// How many more bytes were needed.
        need: usize,
    },
    /// A discriminant byte held an unknown value.
    #[error("invalid {field} discriminant {value}")]
    BadDiscriminant {
        /// Which field failed.
        field: &'static str,
        /// The offending byte.
        value: u8,
    },
    /// The device-id bytes were not UTF-8.
    #[error("device id is not valid UTF-8")]
    BadDeviceId,
    /// Shape (nrows × ncols) disagrees with the remaining payload length.
    #[error("payload length mismatch: shape {rows}x{cols} needs {expect} bytes, found {found}")]
    PayloadMismatch {
        /// Declared rows.
        rows: usize,
        /// Declared cols.
        cols: usize,
        /// Bytes the shape implies.
        expect: usize,
        /// Bytes actually present.
        found: usize,
    },
    /// Trailing bytes after the declared payload.
    #[error("{0} trailing bytes after payload")]
    TrailingBytes(usize),
    /// A reserved region that must be all-zero held nonzero bytes. Accepting
    /// them would let two distinct byte strings decode to the same frame
    /// (re-encoding could not reproduce the original — forged bytes would be
    /// indistinguishable after a replay round-trip).
    #[error("reserved bytes for {field} must be zero")]
    ReservedNotZero {
        /// Which field's reserved region was nonzero.
        field: &'static str,
    },
}

/// Byte cursor for the canonical layout.
struct Cursor<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], CanonicalDecodeError> {
        if self.b.len() - self.at < n {
            return Err(CanonicalDecodeError::Truncated {
                at: self.at,
                need: n - (self.b.len() - self.at),
            });
        }
        let s = &self.b[self.at..self.at + n];
        self.at += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, CanonicalDecodeError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, CanonicalDecodeError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, CanonicalDecodeError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i64(&mut self) -> Result<i64, CanonicalDecodeError> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, CanonicalDecodeError> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i8(&mut self) -> Result<i8, CanonicalDecodeError> {
        Ok(i8::from_le_bytes(self.take(1)?.try_into().unwrap()))
    }
    fn uuid(&mut self) -> Result<Uuid, CanonicalDecodeError> {
        Ok(Uuid::from_bytes(self.take(16)?.try_into().unwrap()))
    }
}

impl CsiFrame {
    /// Reconstruct a frame from its [`to_canonical_bytes`] encoding — the
    /// replay half of the ADR-136 contract. Round-trip law (tested):
    /// `from_canonical_bytes(f.to_canonical_bytes())` yields a frame with the
    /// **same id, metadata, payload, and witness hash** as `f`.
    ///
    /// Amplitude/phase are recomputed from the complex payload (they are
    /// projections, not independent state).
    ///
    /// [`to_canonical_bytes`]: crate::traits::CanonicalFrame::to_canonical_bytes
    ///
    /// # Errors
    /// [`CanonicalDecodeError`] on truncation, bad discriminants, non-UTF-8
    /// device id, nonzero reserved bytes, shape/payload disagreement, or
    /// trailing bytes — every malformed input fails closed. Strictness
    /// guarantees injectivity on the accepted domain: any accepted byte
    /// string re-encodes to exactly itself.
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, CanonicalDecodeError> {
        let mut c = Cursor { b: bytes, at: 0 };

        let id = FrameId::from_uuid(c.uuid()?);

        let seconds = c.i64()?;
        let nanos = c.u32()?;
        let dev_len = c.u32()? as usize;
        let device_id = core::str::from_utf8(c.take(dev_len)?)
            .map_err(|_| CanonicalDecodeError::BadDeviceId)?
            .to_string();
        let frequency_band = match c.u8()? {
            0 => FrequencyBand::Band2_4GHz,
            1 => FrequencyBand::Band5GHz,
            2 => FrequencyBand::Band6GHz,
            v => {
                return Err(CanonicalDecodeError::BadDiscriminant {
                    field: "frequency_band",
                    value: v,
                })
            }
        };
        let channel = c.u8()?;
        let bandwidth_mhz = c.u16()?;
        let tx_antennas = c.u8()?;
        let rx_antennas = c.u8()?;
        let spacing_mm = match c.u8()? {
            1 => Some(c.f32()?),
            0 => {
                // Reserved padding must be zero (decoder strictness =
                // injectivity on the accepted domain): otherwise forged
                // nonzero padding would decode to the same frame as the
                // canonical encoding and re-encode differently.
                if c.take(4)? != [0u8; 4] {
                    return Err(CanonicalDecodeError::ReservedNotZero { field: "spacing_mm" });
                }
                None
            }
            v => {
                return Err(CanonicalDecodeError::BadDiscriminant {
                    field: "spacing_mm",
                    value: v,
                })
            }
        };
        let rssi_dbm = c.i8()?;
        let noise_floor_dbm = c.i8()?;
        let sequence_number = c.u32()?;
        let cal = c.uuid()?;
        let calibration_id = if cal == Uuid::nil() { None } else { Some(cal) };
        let model_id = c.u16()?;
        let model_version = c.u16()?;

        let rows = c.u32()? as usize;
        let cols = c.u32()? as usize;
        let expect = rows.saturating_mul(cols).saturating_mul(16);
        let found = bytes.len() - c.at;
        if found < expect {
            return Err(CanonicalDecodeError::PayloadMismatch { rows, cols, expect, found });
        }
        let mut samples = Vec::with_capacity(rows * cols);
        for _ in 0..rows * cols {
            let raw: [u8; 16] = c.take(16)?.try_into().unwrap();
            samples.push(ComplexSample::from_le_bytes(raw).0);
        }
        if c.at != bytes.len() {
            return Err(CanonicalDecodeError::TrailingBytes(bytes.len() - c.at));
        }
        let data = Array2::from_shape_vec((rows, cols), samples).map_err(|_| {
            CanonicalDecodeError::PayloadMismatch { rows, cols, expect, found }
        })?;

        let metadata = CsiMetadata {
            timestamp: Timestamp { seconds, nanos },
            device_id: DeviceId::new(device_id),
            frequency_band,
            channel,
            bandwidth_mhz,
            antenna_config: AntennaConfig { tx_antennas, rx_antennas, spacing_mm },
            rssi_dbm,
            noise_floor_dbm,
            sequence_number,
            calibration_id,
            model_id,
            model_version,
        };

        let amplitude = data.mapv(num_complex::Complex::norm);
        let phase = data.mapv(num_complex::Complex::arg);
        Ok(Self { id, metadata, data, amplitude, phase })
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
        (
            (self.x_min + self.x_max) / 2.0,
            (self.y_min + self.y_max) / 2.0,
        )
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
        self.persons.iter().max_by(|a, b| {
            a.confidence
                .value()
                .partial_cmp(&b.confidence.value())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
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

    // ===== ADR-136 acceptance tests =====
    use crate::traits::CanonicalFrame;

    /// Deterministic LCG so the test needs no external RNG dependency.
    fn lcg(state: &mut u64) -> f64 {
        *state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        // Map high bits into [-1e6, 1e6) for a wide exponent spread.
        ((*state >> 11) as f64 / (1u64 << 53) as f64).mul_add(2.0e6, -1.0e6)
    }

    /// AC1 — `ComplexSample` little-endian round-trip + endianness pin.
    #[test]
    fn ac1_complex_sample_le_roundtrip() {
        let mut st = 42u64;
        for _ in 0..10_000 {
            let (re, im) = (lcg(&mut st), lcg(&mut st));
            let s = ComplexSample::new(re, im);
            let bytes = s.to_le_bytes();
            assert_eq!(ComplexSample::from_le_bytes(bytes), s, "LE round-trip");
            // Byte 0 is the LSB of `re` encoded little-endian.
            assert_eq!(bytes[0], re.to_le_bytes()[0], "endianness pin on re LSB");
            assert_eq!(bytes[8], im.to_le_bytes()[0], "endianness pin on im LSB");
        }
        // NaN/inf survive the byte round-trip (bit-exact).
        let edge = ComplexSample::new(f64::NAN, f64::INFINITY);
        let rt = ComplexSample::from_le_bytes(edge.to_le_bytes());
        assert!(rt.0.re.is_nan() && rt.0.im.is_infinite());
    }

    /// AC2 — `FrameMeta` provenance defaults + append-only setters.
    #[test]
    fn ac2_frame_meta_provenance_defaults() {
        let mut m = CsiMetadata::new(DeviceId::new("esp32-s3-com9"), FrequencyBand::Band2_4GHz, 6);
        assert_eq!(m.calibration_id, None);
        assert_eq!(m.model_id, 0);
        assert_eq!(m.model_version, 0);

        let cal = uuid::Uuid::new_v4();
        m.set_calibration(cal);
        m.set_model(7, 0x0102);
        assert_eq!(m.calibration_id, Some(cal));
        assert_eq!(m.model_id, 7);
        assert_eq!(m.model_version, 0x0102);
    }

    /// AC6 (frame-level) — `CanonicalFrame` is deterministic across runs and
    /// sensitive to provenance changes.
    #[test]
    fn ac6_canonical_frame_witness_deterministic() {
        use ndarray::Array2;
        let meta = CsiMetadata::new(DeviceId::new("node-1"), FrequencyBand::Band5GHz, 36);
        let data = Array2::from_shape_fn((3, 56), |(r, c)| {
            Complex64::new((r * 56 + c) as f64 * 0.5, (c as f64).sin())
        });
        let frame = CsiFrame::new(meta, data);

        // Same frame hashes identically twice (replay determinism, AC6).
        assert_eq!(frame.witness_hash(), frame.witness_hash());
        let bytes = frame.to_canonical_bytes();
        assert_eq!(bytes.len(), frame.to_canonical_bytes().len());

        // Changing provenance changes the witness (no silent collisions).
        let mut frame2 = frame.clone();
        frame2.metadata.set_model(1, 1);
        assert_ne!(frame.witness_hash(), frame2.witness_hash());
    }

    /// AC7 — replay: `from_canonical_bytes` is the exact inverse of
    /// `to_canonical_bytes` — same id, metadata, payload, and witness hash.
    /// This is the capture-to-claim law: a stored canonical capture replays to
    /// a frame the pipeline cannot distinguish from the original.
    #[test]
    fn ac7_canonical_round_trip_replays_identically() {
        use ndarray::Array2;
        let mut meta = CsiMetadata::new(DeviceId::new("node-α"), FrequencyBand::Band6GHz, 37);
        meta.set_calibration(uuid::Uuid::new_v4());
        meta.set_model(9, 0x0203);
        meta.antenna_config.spacing_mm = Some(62.5);
        meta.rssi_dbm = -41;
        meta.sequence_number = 123_456;
        let data = Array2::from_shape_fn((2, 56), |(r, c)| {
            Complex64::new((r as f64 + 1.0) * (c as f64).cos(), (c as f64 * 0.1).tan())
        });
        let frame = CsiFrame::new(meta, data);

        let bytes = frame.to_canonical_bytes();
        let replayed = CsiFrame::from_canonical_bytes(&bytes).expect("decodes");

        assert_eq!(replayed.id, frame.id);
        // Field-wise metadata equality (CsiMetadata has no PartialEq; the
        // byte-identical re-encoding below covers every field regardless).
        assert_eq!(replayed.metadata.device_id, frame.metadata.device_id);
        assert_eq!(replayed.metadata.calibration_id, frame.metadata.calibration_id);
        assert_eq!(replayed.metadata.model_version, frame.metadata.model_version);
        assert_eq!(replayed.metadata.antenna_config.spacing_mm, Some(62.5));
        assert_eq!(replayed.data, frame.data);
        // Witness equality — the strongest statement of equivalence.
        assert_eq!(replayed.witness_hash(), frame.witness_hash());
        // Re-encoding is byte-identical.
        assert_eq!(replayed.to_canonical_bytes(), bytes);
        // Projections recomputed consistently.
        assert_eq!(replayed.amplitude, frame.amplitude);
    }

    /// AC8 — the decoder fails closed on every malformed-input class.
    #[test]
    fn ac8_canonical_decode_fails_closed() {
        use ndarray::Array2;
        let meta = CsiMetadata::new(DeviceId::new("n"), FrequencyBand::Band2_4GHz, 1);
        let data = Array2::from_shape_fn((1, 4), |(_, c)| Complex64::new(c as f64, 0.0));
        let frame = CsiFrame::new(meta, data);
        let bytes = frame.to_canonical_bytes();

        // Truncation anywhere fails: in the payload it is caught by the
        // shape-vs-length check (PayloadMismatch); in the header by Truncated.
        assert!(matches!(
            CsiFrame::from_canonical_bytes(&bytes[..bytes.len() - 1]),
            Err(CanonicalDecodeError::PayloadMismatch { .. })
        ));
        assert!(matches!(
            CsiFrame::from_canonical_bytes(&bytes[..10]),
            Err(CanonicalDecodeError::Truncated { .. })
        ));

        // Trailing junk fails.
        let mut padded = bytes.clone();
        padded.extend_from_slice(&[0u8; 3]);
        assert!(matches!(
            CsiFrame::from_canonical_bytes(&padded),
            Err(CanonicalDecodeError::TrailingBytes(3))
        ));

        // Bad frequency-band discriminant fails. Band byte sits right after
        // id(16) + seconds(8) + nanos(4) + dev_len(4) + dev("n" = 1).
        let mut bad = bytes.clone();
        bad[16 + 8 + 4 + 4 + 1] = 9;
        assert!(matches!(
            CsiFrame::from_canonical_bytes(&bad),
            Err(CanonicalDecodeError::BadDiscriminant { field: "frequency_band", value: 9 })
        ));

        // A nil calibration uuid decodes as None (the documented encoding).
        let replayed = CsiFrame::from_canonical_bytes(&bytes).unwrap();
        assert_eq!(replayed.metadata.calibration_id, None);
    }

    /// AC8b (review finding 7) — decoder strictness = injectivity on the
    /// accepted domain: forged nonzero bytes in the `spacing_mm` reserved
    /// region are rejected, so for accepted inputs `re-encode != original`
    /// is impossible.
    #[test]
    fn ac8b_forged_reserved_spacing_bytes_rejected() {
        use ndarray::Array2;
        let meta = CsiMetadata::new(DeviceId::new("n"), FrequencyBand::Band2_4GHz, 1);
        let data = Array2::from_shape_fn((1, 4), |(_, c)| Complex64::new(c as f64, 0.0));
        let frame = CsiFrame::new(meta, data);
        let bytes = frame.to_canonical_bytes();

        // Spacing tag sits after id(16)+secs(8)+nanos(4)+dev_len(4)+dev("n"=1)
        // + band(1)+channel(1)+bw(2)+tx(1)+rx(1); the 4 reserved bytes follow.
        let tag_off = 16 + 8 + 4 + 4 + 1 + 1 + 1 + 2 + 1 + 1;
        assert_eq!(bytes[tag_off], 0, "fixture must encode spacing_mm = None");
        assert_eq!(&bytes[tag_off + 1..tag_off + 5], &[0u8; 4]);

        // Sanity: the canonical bytes decode and re-encode byte-identically.
        let ok = CsiFrame::from_canonical_bytes(&bytes).unwrap();
        assert_eq!(ok.to_canonical_bytes(), bytes);

        // Forge each reserved byte: the decoder must fail closed (before the
        // fix it decoded to the same frame, whose re-encoding differed from
        // the forged original — a witness-replay ambiguity).
        for i in 1..=4 {
            let mut forged = bytes.clone();
            forged[tag_off + i] = 0xAB;
            assert!(matches!(
                CsiFrame::from_canonical_bytes(&forged),
                Err(CanonicalDecodeError::ReservedNotZero { field: "spacing_mm" })
            ));
        }
    }

    /// Security pin (review 2026-06, ADR-127) — `from_canonical_bytes` is a
    /// deserialisation boundary for replayed/forwarded captures. A forged header
    /// advertising an enormous `rows × cols` must be rejected by the
    /// shape-vs-length check (`expect` uses saturating multiplies) BEFORE the
    /// `Vec::with_capacity(rows * cols)` allocation — otherwise an attacker could
    /// drive a multi-GB allocation from a few header bytes (unbounded-memory
    /// DoS). The check guarantees `rows*cols*16 <= bytes.len()`, so the capacity
    /// is bounded by the input the caller already holds. This must not OOM.
    #[test]
    fn canonical_decode_oversized_shape_is_bounded_not_allocated() {
        use ndarray::Array2;
        let meta = CsiMetadata::new(DeviceId::new("n"), FrequencyBand::Band2_4GHz, 1);
        let data = Array2::from_shape_fn((1, 2), |(_, c)| Complex64::new(c as f64, 0.0));
        let mut bytes = CsiFrame::new(meta, data).to_canonical_bytes();

        // The (rows, cols) u32 pair is the last 8 bytes before the payload.
        // Overwrite with a maximal claim (u32::MAX × u32::MAX) and lop off the
        // payload so the buffer is tiny but the header lies enormously.
        let shape_off = bytes.len() - 8 - 2 * 16; // 2 samples × 16 bytes payload
        bytes[shape_off..shape_off + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        bytes[shape_off + 4..shape_off + 8].copy_from_slice(&u32::MAX.to_le_bytes());
        bytes.truncate(shape_off + 8); // drop the real payload

        // expect = MAX*MAX*16 (saturated) > found → PayloadMismatch, no alloc.
        assert!(matches!(
            CsiFrame::from_canonical_bytes(&bytes),
            Err(CanonicalDecodeError::PayloadMismatch { .. })
        ));
    }

    /// Security pin (review 2026-06) — the decoder must never panic on arbitrary
    /// bytes: every malformed input is a typed `CanonicalDecodeError`, never an
    /// unwinding panic (panic-on-adversarial-input = 0). Sweep truncations and a
    /// deterministic fuzz spread.
    #[test]
    #[allow(clippy::cast_possible_truncation)] // fuzz byte extraction: truncation IS the point
    fn canonical_decode_never_panics_on_arbitrary_bytes() {
        use ndarray::Array2;
        let mut meta = CsiMetadata::new(DeviceId::new("node"), FrequencyBand::Band5GHz, 36);
        meta.antenna_config.spacing_mm = Some(50.0);
        let data = Array2::from_shape_fn((2, 8), |(r, c)| Complex64::new(r as f64, c as f64));
        let good = CsiFrame::new(meta, data).to_canonical_bytes();

        // Every prefix of a valid encoding must decode without panicking.
        for n in 0..good.len() {
            let _ = CsiFrame::from_canonical_bytes(&good[..n]);
        }
        // Deterministic LCG fuzz over varied lengths.
        let mut st = 0xDEAD_BEEFu64;
        for len in 0..400usize {
            let buf: Vec<u8> = (0..len)
                .map(|_| {
                    st = st
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1_442_695_040_888_963_407);
                    (st >> 33) as u8
                })
                .collect();
            let _ = CsiFrame::from_canonical_bytes(&buf);
        }
    }

    /// AC8c (review finding 7) — `Some(Uuid::nil())` calibration is an
    /// encoding error: nil is the wire sentinel for `None`, so encoding it
    /// would alias two distinct frames to one byte string (and one witness).
    #[test]
    #[should_panic(expected = "nil is the None sentinel")]
    fn ac8c_nil_calibration_id_is_an_encoding_error() {
        use ndarray::Array2;
        let mut meta = CsiMetadata::new(DeviceId::new("n"), FrequencyBand::Band2_4GHz, 1);
        meta.calibration_id = Some(uuid::Uuid::nil());
        let data = Array2::from_shape_fn((1, 2), |(_, c)| Complex64::new(c as f64, 0.0));
        let _ = CsiFrame::new(meta, data).to_canonical_bytes();
    }

    /// AC3 — `serde(default)` forward-read of pre-ADR-136 metadata JSON.
    #[cfg(feature = "serde")]
    #[test]
    fn ac3_serde_forward_read_legacy_metadata() {
        // A pre-ADR-136 CsiMetadata payload without the three new fields.
        let legacy = r#"{
            "timestamp": {"seconds": 1700000000, "nanos": 0},
            "device_id": "legacy-node",
            "frequency_band": "Band2_4GHz",
            "channel": 1,
            "bandwidth_mhz": 20,
            "antenna_config": {"tx_antennas": 1, "rx_antennas": 3, "spacing_mm": null},
            "rssi_dbm": -50,
            "noise_floor_dbm": -90,
            "sequence_number": 0
        }"#;
        let m: CsiMetadata = serde_json::from_str(legacy).expect("legacy metadata must load");
        assert_eq!(m.calibration_id, None);
        assert_eq!(m.model_id, 0);
        assert_eq!(m.model_version, 0);
    }

    /// AC1b — `ComplexSample` serde tuple form is the two LE f64 contract.
    #[cfg(feature = "serde")]
    #[test]
    fn ac1b_complex_sample_serde_tuple() {
        let s = ComplexSample::new(1.5, -2.25);
        let j = serde_json::to_string(&s).unwrap();
        assert_eq!(j, "[1.5,-2.25]");
        let back: ComplexSample = serde_json::from_str(&j).unwrap();
        assert_eq!(back, s);
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
        assert_eq!(
            KeypointType::try_from(16).unwrap(),
            KeypointType::RightAnkle
        );
        assert!(KeypointType::try_from(17).is_err());
    }

    #[test]
    fn test_frequency_band() {
        assert_eq!(FrequencyBand::Band2_4GHz.typical_subcarriers(), 56);
        assert_eq!(FrequencyBand::Band5GHz.typical_subcarriers(), 114);
        assert!(FrequencyBand::Band5GHz.center_frequency_mhz() > 5000);
    }
}
