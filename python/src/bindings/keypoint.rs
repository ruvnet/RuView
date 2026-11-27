//! ADR-117 P2 — PyO3 bindings for `wifi_densepose_core::Keypoint` +
//! `KeypointType` + `Confidence`.
//!
//! Design notes (consequential for the Python API surface):
//!
//! 1. **`Confidence` is NOT bound as a separate Python class.** End
//!    users hate having to construct a wrapper just to pass a float.
//!    Python-side, confidence is just an `f32` in `[0.0, 1.0]`; the
//!    binding validates on the way in.
//!
//! 2. **`KeypointType` is bound as a `#[pyclass]` enum** (PyO3 0.22
//!    supports `#[pyclass(eq, eq_int)]` for C-like enums). Python-side
//!    it surfaces as `wifi_densepose.KeypointType.Nose`, etc.
//!
//! 3. **`Keypoint` constructor accepts `z` as `Optional[float]`** so
//!    Python users can pass `Keypoint(KeypointType.Nose, 0.5, 0.3,
//!    0.95)` for 2D or `Keypoint(..., z=0.1)` for 3D.

use pyo3::prelude::*;

use wifi_densepose_core::{Confidence, Keypoint, KeypointType};

// ─── KeypointType ────────────────────────────────────────────────────

/// COCO-17 keypoint identifier — re-export of the Rust core enum.
///
/// Python:
/// ```python
/// from wifi_densepose import KeypointType
/// kp = KeypointType.Nose
/// print(kp.name)  # "Nose"
/// ```
// `hash` makes the enum hashable in Python (usable as dict keys + set
// members) — derived from `Hash` on the Rust side. `frozen` is a
// hard requirement for `hash` per pyo3 contract.
#[pyclass(from_py_object, eq, eq_int, hash, frozen, name = "KeypointType")]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyKeypointType {
    Nose = 0,
    LeftEye = 1,
    RightEye = 2,
    LeftEar = 3,
    RightEar = 4,
    LeftShoulder = 5,
    RightShoulder = 6,
    LeftElbow = 7,
    RightElbow = 8,
    LeftWrist = 9,
    RightWrist = 10,
    LeftHip = 11,
    RightHip = 12,
    LeftKnee = 13,
    RightKnee = 14,
    LeftAnkle = 15,
    RightAnkle = 16,
}

#[pymethods]
impl PyKeypointType {
    /// Lowercase snake_case name (matches the COCO standard).
    #[getter]
    fn snake_name(&self) -> &'static str {
        self.as_rust().name()
    }

    /// Integer index 0–16 (COCO ordering).
    #[getter]
    fn index(&self) -> u8 {
        (*self).into()
    }

    /// True if this keypoint is on the face (nose, eyes, ears).
    fn is_face(&self) -> bool {
        self.as_rust().is_face()
    }

    /// True if this keypoint is in the upper body (shoulders, elbows, wrists).
    fn is_upper_body(&self) -> bool {
        self.as_rust().is_upper_body()
    }

    /// All 17 keypoint types in COCO order. Useful for Jupyter
    /// enumeration: `for kp in KeypointType.all(): ...`.
    #[staticmethod]
    fn all() -> Vec<Self> {
        KeypointType::all().iter().map(|k| PyKeypointType::from_rust(*k)).collect()
    }

    fn __repr__(&self) -> String {
        format!("KeypointType.{:?}", self.as_rust())
    }
}

impl PyKeypointType {
    pub(crate) fn as_rust(&self) -> KeypointType {
        // SAFETY equivalent: the enum variants line up 1:1 with the
        // Rust enum's `#[repr(u8)]` discriminants. The match below is
        // exhaustive on both sides so a future addition to either side
        // fails to compile until the other is updated.
        match self {
            Self::Nose => KeypointType::Nose,
            Self::LeftEye => KeypointType::LeftEye,
            Self::RightEye => KeypointType::RightEye,
            Self::LeftEar => KeypointType::LeftEar,
            Self::RightEar => KeypointType::RightEar,
            Self::LeftShoulder => KeypointType::LeftShoulder,
            Self::RightShoulder => KeypointType::RightShoulder,
            Self::LeftElbow => KeypointType::LeftElbow,
            Self::RightElbow => KeypointType::RightElbow,
            Self::LeftWrist => KeypointType::LeftWrist,
            Self::RightWrist => KeypointType::RightWrist,
            Self::LeftHip => KeypointType::LeftHip,
            Self::RightHip => KeypointType::RightHip,
            Self::LeftKnee => KeypointType::LeftKnee,
            Self::RightKnee => KeypointType::RightKnee,
            Self::LeftAnkle => KeypointType::LeftAnkle,
            Self::RightAnkle => KeypointType::RightAnkle,
        }
    }

    pub(crate) fn from_rust(k: KeypointType) -> Self {
        match k {
            KeypointType::Nose => Self::Nose,
            KeypointType::LeftEye => Self::LeftEye,
            KeypointType::RightEye => Self::RightEye,
            KeypointType::LeftEar => Self::LeftEar,
            KeypointType::RightEar => Self::RightEar,
            KeypointType::LeftShoulder => Self::LeftShoulder,
            KeypointType::RightShoulder => Self::RightShoulder,
            KeypointType::LeftElbow => Self::LeftElbow,
            KeypointType::RightElbow => Self::RightElbow,
            KeypointType::LeftWrist => Self::LeftWrist,
            KeypointType::RightWrist => Self::RightWrist,
            KeypointType::LeftHip => Self::LeftHip,
            KeypointType::RightHip => Self::RightHip,
            KeypointType::LeftKnee => Self::LeftKnee,
            KeypointType::RightKnee => Self::RightKnee,
            KeypointType::LeftAnkle => Self::LeftAnkle,
            KeypointType::RightAnkle => Self::RightAnkle,
        }
    }
}

impl From<PyKeypointType> for u8 {
    fn from(k: PyKeypointType) -> u8 {
        k as u8
    }
}

impl PyKeypoint {
    /// Rust-side accessor for the inner Keypoint (used by pose.rs).
    /// Not exposed to Python — Python users go through the
    /// #[pymethods] getters above.
    pub(crate) fn inner(&self) -> &Keypoint {
        &self.inner
    }

    /// Rust-side constructor from a core Keypoint (used by pose.rs
    /// when re-wrapping outputs of PersonPose methods).
    pub(crate) fn from_rust(k: Keypoint) -> Self {
        Self { inner: k }
    }
}

// ─── Keypoint ────────────────────────────────────────────────────────

/// Single skeletal joint with COCO type, 2D-or-3D position, and a
/// confidence score in [0.0, 1.0].
///
/// Python:
/// ```python
/// from wifi_densepose import Keypoint, KeypointType
///
/// kp = Keypoint(KeypointType.Nose, 0.5, 0.3, 0.95)
/// print(kp.x, kp.y, kp.confidence, kp.is_visible)
///
/// kp_3d = Keypoint(KeypointType.LeftWrist, 0.2, 0.4, 0.8, z=0.1)
/// print(kp_3d.position_3d)  # (0.2, 0.4, 0.1)
/// ```
#[pyclass(from_py_object, frozen, name = "Keypoint")]
#[derive(Clone)]
pub struct PyKeypoint {
    inner: Keypoint,
}

#[pymethods]
impl PyKeypoint {
    /// Construct a new keypoint. Confidence must be in [0.0, 1.0].
    /// `z` is optional — omit for a 2D keypoint, supply for 3D.
    #[new]
    #[pyo3(signature = (keypoint_type, x, y, confidence, *, z=None))]
    fn new(
        keypoint_type: PyKeypointType,
        x: f32,
        y: f32,
        confidence: f32,
        z: Option<f32>,
    ) -> PyResult<Self> {
        let conf = Confidence::new(confidence).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(e.to_string())
        })?;
        let inner = match z {
            Some(zv) => Keypoint::new_3d(keypoint_type.as_rust(), x, y, zv, conf),
            None => Keypoint::new(keypoint_type.as_rust(), x, y, conf),
        };
        Ok(Self { inner })
    }

    /// COCO keypoint type.
    #[getter]
    fn keypoint_type(&self) -> PyKeypointType {
        PyKeypointType::from_rust(self.inner.keypoint_type)
    }

    /// X coordinate.
    #[getter]
    fn x(&self) -> f32 {
        self.inner.x
    }

    /// Y coordinate.
    #[getter]
    fn y(&self) -> f32 {
        self.inner.y
    }

    /// Z coordinate, or None for 2D keypoints.
    #[getter]
    fn z(&self) -> Option<f32> {
        self.inner.z
    }

    /// Detection confidence in [0.0, 1.0].
    #[getter]
    fn confidence(&self) -> f32 {
        self.inner.confidence.value()
    }

    /// True if this keypoint clears the default visibility threshold
    /// (`confidence >= 0.5`).
    #[getter]
    fn is_visible(&self) -> bool {
        self.inner.is_visible()
    }

    /// 2D position as a tuple `(x, y)`.
    #[getter]
    fn position_2d(&self) -> (f32, f32) {
        self.inner.position_2d()
    }

    /// 3D position as a tuple `(x, y, z)`, or None for 2D keypoints.
    #[getter]
    fn position_3d(&self) -> Option<(f32, f32, f32)> {
        self.inner.position_3d()
    }

    /// Euclidean distance to another keypoint. If both are 3D the
    /// distance includes the z-axis; otherwise it's 2D only.
    fn distance_to(&self, other: &PyKeypoint) -> f32 {
        self.inner.distance_to(&other.inner)
    }

    fn __repr__(&self) -> String {
        match self.inner.z {
            Some(z) => format!(
                "Keypoint(KeypointType.{:?}, x={}, y={}, z={}, confidence={:.4})",
                self.inner.keypoint_type, self.inner.x, self.inner.y, z, self.inner.confidence.value()
            ),
            None => format!(
                "Keypoint(KeypointType.{:?}, x={}, y={}, confidence={:.4})",
                self.inner.keypoint_type, self.inner.x, self.inner.y, self.inner.confidence.value()
            ),
        }
    }

    fn __eq__(&self, other: &PyKeypoint) -> bool {
        self.inner.keypoint_type == other.inner.keypoint_type
            && self.inner.x == other.inner.x
            && self.inner.y == other.inner.y
            && self.inner.z == other.inner.z
            && (self.inner.confidence.value() - other.inner.confidence.value()).abs() < f32::EPSILON
    }
}

/// Register the binding types with the `_native` PyModule.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyKeypointType>()?;
    m.add_class::<PyKeypoint>()?;
    Ok(())
}
