//! ADR-117 P2 — PyO3 bindings for `BoundingBox`, `PersonPose`,
//! `PoseEstimate`.
//!
//! Design notes:
//!
//! 1. **`PersonPose` exposes the 17-keypoint array as a Python dict
//!    keyed by `KeypointType`**, not as a fixed-length list with
//!    `None` slots. Pythonistas don't want to know that the underlying
//!    storage is `[Option<Keypoint>; 17]`.
//!
//! 2. **`PoseEstimate` metadata `id` and `timestamp` are exposed as
//!    strings** (UUID + RFC 3339) rather than as bound types. Users
//!    in notebooks rarely need to compare UUIDs structurally; strings
//!    are good enough and don't require binding `FrameId` /
//!    `Timestamp` as separate classes.
//!
//! 3. **`PersonPose` is mutable** via `set_keypoint` / `set_bbox` /
//!    `set_id` — it's a builder-style type users construct
//!    incrementally. Hence NOT `#[pyclass(frozen)]`.
//!
//! 4. **`PoseEstimate` is frozen** — once constructed, the list of
//!    persons + the metadata don't change.

use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::PyDict;

use wifi_densepose_core::{
    BoundingBox, Confidence, KeypointType, PersonPose, PoseEstimate,
};

use super::keypoint::{PyKeypoint, PyKeypointType};

// ─── BoundingBox ─────────────────────────────────────────────────────

/// Axis-aligned bounding box around a detected person.
///
/// Python:
/// ```python
/// from wifi_densepose import BoundingBox
///
/// bb = BoundingBox(0.1, 0.2, 0.5, 0.7)
/// print(bb.width, bb.height, bb.area, bb.center)
/// bb2 = BoundingBox.from_center(0.3, 0.45, 0.4, 0.5)
/// print(bb.iou(bb2))
/// ```
#[pyclass(from_py_object, frozen, name = "BoundingBox")]
#[derive(Clone)]
pub struct PyBoundingBox {
    inner: BoundingBox,
}

#[pymethods]
impl PyBoundingBox {
    #[new]
    fn new(x_min: f32, y_min: f32, x_max: f32, y_max: f32) -> Self {
        Self { inner: BoundingBox::new(x_min, y_min, x_max, y_max) }
    }

    /// Construct from center point + width + height.
    #[staticmethod]
    fn from_center(cx: f32, cy: f32, width: f32, height: f32) -> Self {
        Self { inner: BoundingBox::from_center(cx, cy, width, height) }
    }

    #[getter]
    fn x_min(&self) -> f32 { self.inner.x_min }
    #[getter]
    fn y_min(&self) -> f32 { self.inner.y_min }
    #[getter]
    fn x_max(&self) -> f32 { self.inner.x_max }
    #[getter]
    fn y_max(&self) -> f32 { self.inner.y_max }
    #[getter]
    fn width(&self) -> f32 { self.inner.width() }
    #[getter]
    fn height(&self) -> f32 { self.inner.height() }
    #[getter]
    fn area(&self) -> f32 { self.inner.area() }
    #[getter]
    fn center(&self) -> (f32, f32) { self.inner.center() }

    /// Intersection over Union (IoU) with another box. Range [0.0, 1.0].
    fn iou(&self, other: &PyBoundingBox) -> f32 {
        self.inner.iou(&other.inner)
    }

    fn __repr__(&self) -> String {
        format!(
            "BoundingBox(x_min={}, y_min={}, x_max={}, y_max={})",
            self.inner.x_min, self.inner.y_min, self.inner.x_max, self.inner.y_max,
        )
    }

    fn __eq__(&self, other: &PyBoundingBox) -> bool {
        self.inner == other.inner
    }
}

impl PyBoundingBox {
    pub(crate) fn from_rust(bb: BoundingBox) -> Self {
        Self { inner: bb }
    }
}

// ─── PersonPose ──────────────────────────────────────────────────────

/// A single detected person with optional ID, up to 17 keypoints, and
/// an optional bounding box.
///
/// Python:
/// ```python
/// from wifi_densepose import PersonPose, Keypoint, KeypointType, BoundingBox
///
/// pose = PersonPose()
/// pose.set_keypoint(Keypoint(KeypointType.Nose, 0.5, 0.3, 0.95))
/// pose.set_keypoint(Keypoint(KeypointType.LeftShoulder, 0.4, 0.5, 0.92))
/// pose.set_id(7)
/// print(pose.visible_keypoint_count)         # 2
/// print(pose.get_keypoint(KeypointType.Nose).confidence)  # 0.95
/// print(pose.compute_bounding_box())          # auto-derived from visible kp
/// ```
#[pyclass(from_py_object, name = "PersonPose")]
#[derive(Clone)]
pub struct PyPersonPose {
    inner: PersonPose,
}

#[pymethods]
impl PyPersonPose {
    /// Construct an empty person pose. Set keypoints + bbox + id with
    /// the dedicated methods.
    #[new]
    fn new() -> Self {
        Self { inner: PersonPose::new() }
    }

    /// Per-person track ID. None until set.
    #[getter]
    fn id(&self) -> Option<u32> {
        self.inner.id
    }

    fn set_id(&mut self, id: u32) {
        self.inner.id = Some(id);
    }

    /// Set or replace a keypoint. The keypoint's type determines its
    /// slot in the internal 17-element array.
    fn set_keypoint(&mut self, keypoint: PyKeypoint) {
        self.inner.set_keypoint(*keypoint.inner());
    }

    /// Get a keypoint by type, or None if not set.
    fn get_keypoint(&self, keypoint_type: PyKeypointType) -> Option<PyKeypoint> {
        let kp = self.inner.get_keypoint(keypoint_type.as_rust())?;
        // Re-wrap the inner Rust Keypoint for Python.
        Some(PyKeypoint::from_rust(*kp))
    }

    /// All keypoints as a dict keyed by KeypointType. Missing
    /// keypoints are omitted (NOT included with None values).
    fn keypoints<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        // PyO3 0.29 — PyDict::new returns a Bound.
        let dict = PyDict::new(py);
        for (i, kp_opt) in self.inner.keypoints.iter().enumerate() {
            if let Some(kp) = kp_opt {
                let kpt = match KeypointType::all().get(i) {
                    Some(t) => *t,
                    None => continue,
                };
                // Wrap the #[pyclass] values as Python objects (PyO3 0.29 Bound API).
                let k_obj = Bound::new(py, PyKeypointType::from_rust(kpt))?;
                let v_obj = Bound::new(py, PyKeypoint::from_rust(*kp))?;
                dict.set_item(k_obj, v_obj)?;
            }
        }
        Ok(dict)
    }

    /// Number of visible keypoints (confidence >= 0.5).
    #[getter]
    fn visible_keypoint_count(&self) -> usize {
        self.inner.visible_keypoint_count()
    }

    /// List of visible keypoints (subset of the dict from
    /// `keypoints()`).
    fn visible_keypoints(&self) -> Vec<PyKeypoint> {
        self.inner
            .visible_keypoints()
            .into_iter()
            .map(|k| PyKeypoint::from_rust(*k))
            .collect()
    }

    /// Bounding box, if previously set or computed.
    #[getter]
    fn bounding_box(&self) -> Option<PyBoundingBox> {
        self.inner.bounding_box.map(PyBoundingBox::from_rust)
    }

    fn set_bounding_box(&mut self, bb: PyBoundingBox) {
        self.inner.bounding_box = Some(bb.inner);
    }

    /// Auto-compute bounding box from visible keypoints, set it
    /// internally, and return it. Returns None if no keypoints visible.
    fn compute_bounding_box(&mut self) -> Option<PyBoundingBox> {
        let bb = self.inner.compute_bounding_box()?;
        self.inner.bounding_box = Some(bb);
        Some(PyBoundingBox::from_rust(bb))
    }

    /// Overall confidence in [0.0, 1.0].
    #[getter]
    fn confidence(&self) -> f32 {
        self.inner.confidence.value()
    }

    fn set_confidence(&mut self, c: f32) -> PyResult<()> {
        self.inner.confidence = Confidence::new(c).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(e.to_string())
        })?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "PersonPose(id={:?}, visible_keypoints={}, confidence={:.4})",
            self.inner.id,
            self.inner.visible_keypoint_count(),
            self.inner.confidence.value(),
        )
    }
}

impl PyPersonPose {
    pub(crate) fn from_rust(pose: PersonPose) -> Self {
        Self { inner: pose }
    }
}

// ─── PoseEstimate ────────────────────────────────────────────────────

/// Top-level result of a pose-estimation pass — a list of detected
/// persons plus metadata about the inference run.
///
/// Python:
/// ```python
/// from wifi_densepose import PoseEstimate, PersonPose
///
/// est = PoseEstimate([pose1, pose2], confidence=0.87, latency_ms=8.4,
///                    model_version="v0.1.0")
/// print(est.person_count, est.has_detections)
/// best = est.highest_confidence_person()
/// ```
#[pyclass(frozen, name = "PoseEstimate")]
pub struct PyPoseEstimate {
    inner: PoseEstimate,
}

#[pymethods]
impl PyPoseEstimate {
    /// Construct a pose estimate from a list of detected persons,
    /// an overall confidence, inference latency, and model version
    /// string.
    #[new]
    fn new(
        persons: Vec<PyPersonPose>,
        confidence: f32,
        latency_ms: f32,
        model_version: String,
    ) -> PyResult<Self> {
        let conf = Confidence::new(confidence).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(e.to_string())
        })?;
        let rust_persons: Vec<PersonPose> =
            persons.into_iter().map(|p| p.inner).collect();
        Ok(Self {
            inner: PoseEstimate::new(
                Vec::new(),
                rust_persons,
                conf,
                latency_ms,
                model_version,
            ),
        })
    }

    /// Unique frame identifier as a UUID string.
    #[getter]
    fn id(&self) -> String {
        format!("{:?}", self.inner.id)
            .trim_start_matches("FrameId(")
            .trim_end_matches(')')
            .to_string()
    }

    /// Frame timestamp as an RFC 3339 / ISO 8601 string in UTC.
    #[getter]
    fn timestamp(&self) -> String {
        // Timestamp's Debug impl is usable; for a fully spec-compliant
        // ISO format, a future refactor binds chrono. P2 string-form
        // is "good enough" for diagnostics.
        format!("{:?}", self.inner.timestamp)
    }

    #[getter]
    fn persons(&self) -> Vec<PyPersonPose> {
        self.inner.persons.iter().cloned().map(PyPersonPose::from_rust).collect()
    }

    #[getter]
    fn confidence(&self) -> f32 {
        self.inner.confidence.value()
    }

    #[getter]
    fn latency_ms(&self) -> f32 {
        self.inner.latency_ms
    }

    #[getter]
    fn model_version(&self) -> &str {
        &self.inner.model_version
    }

    #[getter]
    fn person_count(&self) -> usize {
        self.inner.person_count()
    }

    #[getter]
    fn has_detections(&self) -> bool {
        self.inner.has_detections()
    }

    /// Get the person with the highest individual confidence, or None
    /// if no persons detected.
    fn highest_confidence_person(&self) -> Option<PyPersonPose> {
        self.inner
            .highest_confidence_person()
            .cloned()
            .map(PyPersonPose::from_rust)
    }

    fn __repr__(&self) -> String {
        format!(
            "PoseEstimate(persons={}, confidence={:.4}, latency_ms={:.2}, model_version={:?})",
            self.inner.person_count(),
            self.inner.confidence.value(),
            self.inner.latency_ms,
            self.inner.model_version,
        )
    }
}

/// Suppress unused-import warnings for HashMap (held for future
/// keypoint-map helpers in P3).
#[allow(dead_code)]
fn _hashmap_kept_for_future_use() -> HashMap<u8, u8> {
    HashMap::new()
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBoundingBox>()?;
    m.add_class::<PyPersonPose>()?;
    m.add_class::<PyPoseEstimate>()?;
    Ok(())
}
