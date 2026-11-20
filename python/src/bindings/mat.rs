//! ADR-185 P3 — PyO3 bindings for MAT (Mass Casualty Assessment Tool, ADR-024
//! crate table): WiFi-based disaster-survivor detection + START triage.
//!
//! Bound behind the `[mat]` extra so the disaster/ML stack never enters the
//! default wheel.
//!
//! ## Honest scope vs ADR-185 §3.4
//!
//! - **`scan_once()`** — ADR-185 §3.4/§11.3 proposed adding a sync
//!   `scan_once()` wrapper Rust-side. That turned out to be unnecessary: the
//!   public async `DisasterResponse::start_scanning()` runs **exactly one**
//!   `scan_cycle` and returns when `continuous_monitoring == false`. So this
//!   binding forces `continuous_monitoring = false` and drives one scan on a
//!   private current-thread tokio runtime — no change to `wifi-densepose-mat`.
//! - **event + zone are required** — `scan_cycle` errors without an active
//!   event and an Active zone. ADR-185 §3.4's surface omitted this; the real
//!   pipeline needs `initialize_event(...)` + `add_zone(...)` first, so both
//!   are bound (documented additions, not fabrications).
//! - **`Survivor.vital_signs`** — the ADR implies a single `VitalSignsReading`;
//!   the real accessor returns a *history*. Bound here as
//!   `Survivor.latest_vitals -> Optional[VitalSignsReading]`.
//! - **`DisasterType`** has 9 variants at HEAD (adds Landslide, MineCollapse,
//!   Industrial, TunnelCollapse) vs the ADR's shorter list; all are bound.
//!
//! ## GIL release
//!
//! `push_csi_data` and `scan_once` release the GIL (`py.detach`) — the
//! detection pipeline + ensemble classifier are the compute-heavy part and
//! touch no Python state.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use wifi_densepose_mat::{
    DisasterConfig, DisasterResponse, DisasterType, ScanZone, Survivor, TriageStatus,
    VitalSignsReading, ZoneBounds,
};

// ─── DisasterType ────────────────────────────────────────────────────

/// Type of disaster event (shapes the debris/attenuation model).
#[pyclass(from_py_object, eq, eq_int, frozen, hash, name = "DisasterType")]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyDisasterType {
    BuildingCollapse = 0,
    Earthquake = 1,
    Landslide = 2,
    Avalanche = 3,
    Flood = 4,
    MineCollapse = 5,
    Industrial = 6,
    TunnelCollapse = 7,
    Unknown = 8,
}

impl PyDisasterType {
    fn as_rust(self) -> DisasterType {
        match self {
            Self::BuildingCollapse => DisasterType::BuildingCollapse,
            Self::Earthquake => DisasterType::Earthquake,
            Self::Landslide => DisasterType::Landslide,
            Self::Avalanche => DisasterType::Avalanche,
            Self::Flood => DisasterType::Flood,
            Self::MineCollapse => DisasterType::MineCollapse,
            Self::Industrial => DisasterType::Industrial,
            Self::TunnelCollapse => DisasterType::TunnelCollapse,
            Self::Unknown => DisasterType::Unknown,
        }
    }
}

#[pymethods]
impl PyDisasterType {
    fn __repr__(&self) -> String {
        format!("DisasterType.{:?}", self.as_rust())
    }
}

// ─── TriageStatus ────────────────────────────────────────────────────

/// START-protocol triage class.
#[pyclass(from_py_object, eq, eq_int, frozen, hash, name = "TriageStatus")]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyTriageStatus {
    Immediate = 0,
    Delayed = 1,
    Minor = 2,
    Deceased = 3,
    Unknown = 4,
}

impl PyTriageStatus {
    fn as_rust(self) -> TriageStatus {
        match self {
            Self::Immediate => TriageStatus::Immediate,
            Self::Delayed => TriageStatus::Delayed,
            Self::Minor => TriageStatus::Minor,
            Self::Deceased => TriageStatus::Deceased,
            Self::Unknown => TriageStatus::Unknown,
        }
    }
    fn from_rust(s: &TriageStatus) -> Self {
        match s {
            TriageStatus::Immediate => Self::Immediate,
            TriageStatus::Delayed => Self::Delayed,
            TriageStatus::Minor => Self::Minor,
            TriageStatus::Deceased => Self::Deceased,
            TriageStatus::Unknown => Self::Unknown,
        }
    }
}

#[pymethods]
impl PyTriageStatus {
    /// START priority (1 = highest / Immediate ... 5 = Unknown).
    #[getter]
    fn priority(&self) -> u8 {
        self.as_rust().priority()
    }
    fn __repr__(&self) -> String {
        format!("TriageStatus.{:?}", self.as_rust())
    }
}

// ─── VitalSignsReading ───────────────────────────────────────────────

/// A single vital-signs reading (optional breathing/heartbeat + movement).
#[pyclass(frozen, name = "VitalSignsReading")]
pub struct PyVitalSignsReading {
    breathing_rate_bpm: Option<f32>,
    heartbeat_rate_bpm: Option<f32>,
    movement_intensity: f32,
    confidence: f64,
}

impl PyVitalSignsReading {
    fn from_rust(r: &VitalSignsReading) -> Self {
        Self {
            breathing_rate_bpm: r.breathing.as_ref().map(|b| b.rate_bpm),
            heartbeat_rate_bpm: r.heartbeat.as_ref().map(|h| h.rate_bpm),
            movement_intensity: r.movement.intensity,
            confidence: r.confidence.value(),
        }
    }
}

#[pymethods]
impl PyVitalSignsReading {
    #[getter]
    fn breathing_rate_bpm(&self) -> Option<f32> {
        self.breathing_rate_bpm
    }
    #[getter]
    fn heartbeat_rate_bpm(&self) -> Option<f32> {
        self.heartbeat_rate_bpm
    }
    #[getter]
    fn movement_intensity(&self) -> f32 {
        self.movement_intensity
    }
    #[getter]
    fn confidence(&self) -> f64 {
        self.confidence
    }
    fn __repr__(&self) -> String {
        format!(
            "VitalSignsReading(breathing={:?}, heartbeat={:?}, movement={:.3}, confidence={:.3})",
            self.breathing_rate_bpm, self.heartbeat_rate_bpm, self.movement_intensity, self.confidence,
        )
    }
}

// ─── Survivor ────────────────────────────────────────────────────────

/// A detected survivor: id, triage class, confidence, optional 3-D location,
/// and the latest vital-signs reading.
#[pyclass(frozen, name = "Survivor")]
pub struct PySurvivor {
    id: String,
    triage_status: PyTriageStatus,
    confidence: f64,
    location: Option<(f64, f64, f64)>,
    latest_vitals: Option<Py<PyVitalSignsReading>>,
}

impl PySurvivor {
    fn from_rust(py: Python<'_>, s: &Survivor) -> PyResult<Self> {
        let latest_vitals = match s.vital_signs().latest() {
            Some(r) => Some(Py::new(py, PyVitalSignsReading::from_rust(r))?),
            None => None,
        };
        Ok(Self {
            id: s.id().as_uuid().to_string(),
            triage_status: PyTriageStatus::from_rust(s.triage_status()),
            confidence: s.confidence(),
            location: s.location().map(|c| (c.x, c.y, c.z)),
            latest_vitals,
        })
    }
}

#[pymethods]
impl PySurvivor {
    #[getter]
    fn id(&self) -> &str {
        &self.id
    }
    #[getter]
    fn triage_status(&self) -> PyTriageStatus {
        self.triage_status
    }
    #[getter]
    fn confidence(&self) -> f64 {
        self.confidence
    }
    #[getter]
    fn location(&self) -> Option<(f64, f64, f64)> {
        self.location
    }
    #[getter]
    fn latest_vitals(&self, py: Python<'_>) -> Option<Py<PyVitalSignsReading>> {
        self.latest_vitals.as_ref().map(|v| v.clone_ref(py))
    }
    fn __repr__(&self) -> String {
        format!(
            "Survivor(id={}, triage={:?}, confidence={:.3})",
            &self.id[..8.min(self.id.len())],
            self.triage_status.as_rust(),
            self.confidence,
        )
    }
}

// ─── DisasterConfig ──────────────────────────────────────────────────

/// Configuration for the disaster-response pipeline.
///
/// Note: the Python binding always runs **single-shot** scans (`scan_once`),
/// so `continuous_monitoring` is forced off internally.
#[pyclass(from_py_object, frozen, name = "DisasterConfig")]
#[derive(Clone)]
pub struct PyDisasterConfig {
    inner: DisasterConfig,
}

#[pymethods]
impl PyDisasterConfig {
    #[new]
    #[pyo3(signature = (
        disaster_type,
        sensitivity=0.8,
        confidence_threshold=0.5,
        max_depth=5.0,
        scan_interval_ms=500
    ))]
    fn new(
        disaster_type: PyDisasterType,
        sensitivity: f64,
        confidence_threshold: f64,
        max_depth: f64,
        scan_interval_ms: u64,
    ) -> Self {
        let inner = DisasterConfig::builder()
            .disaster_type(disaster_type.as_rust())
            .sensitivity(sensitivity)
            .confidence_threshold(confidence_threshold)
            .max_depth(max_depth)
            .scan_interval_ms(scan_interval_ms)
            .continuous_monitoring(false)
            .build();
        Self { inner }
    }

    #[getter]
    fn sensitivity(&self) -> f64 {
        self.inner.sensitivity
    }
    #[getter]
    fn confidence_threshold(&self) -> f64 {
        self.inner.confidence_threshold
    }
    #[getter]
    fn max_depth(&self) -> f64 {
        self.inner.max_depth
    }

    fn __repr__(&self) -> String {
        format!(
            "DisasterConfig(disaster_type={:?}, sensitivity={}, confidence_threshold={}, max_depth={})",
            self.inner.disaster_type,
            self.inner.sensitivity,
            self.inner.confidence_threshold,
            self.inner.max_depth,
        )
    }
}

// ─── ScanZone ────────────────────────────────────────────────────────

/// A rectangular or circular scan zone (new zones start Active).
#[pyclass(from_py_object, name = "ScanZone")]
#[derive(Clone)]
pub struct PyScanZone {
    inner: ScanZone,
}

#[pymethods]
impl PyScanZone {
    /// Rectangular zone with corner bounds (metres).
    #[staticmethod]
    fn rectangle(name: &str, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        Self {
            inner: ScanZone::new(name, ZoneBounds::rectangle(min_x, min_y, max_x, max_y)),
        }
    }

    /// Circular zone centred at `(center_x, center_y)` with `radius` (metres).
    #[staticmethod]
    fn circle(name: &str, center_x: f64, center_y: f64, radius: f64) -> Self {
        Self {
            inner: ScanZone::new(name, ZoneBounds::circle(center_x, center_y, radius)),
        }
    }

    #[getter]
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn __repr__(&self) -> String {
        format!("ScanZone(name={:?})", self.inner.name())
    }
}

// ─── DisasterResponse ────────────────────────────────────────────────

/// Main disaster-response coordinator: ingest CSI, run one scan cycle, query
/// detected survivors by START triage.
#[pyclass(name = "DisasterResponse")]
pub struct PyDisasterResponse {
    inner: DisasterResponse,
    rt: tokio::runtime::Runtime,
}

#[pymethods]
impl PyDisasterResponse {
    #[new]
    fn new(config: PyDisasterConfig) -> PyResult<Self> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .map_err(|e| PyValueError::new_err(format!("failed to build tokio runtime: {e}")))?;
        Ok(Self {
            inner: DisasterResponse::new(config.inner),
            rt,
        })
    }

    /// Initialize the active disaster event at map coordinate `(x, y)`.
    /// Required before `add_zone`/`scan_once`.
    fn initialize_event(&mut self, x: f64, y: f64, description: &str) -> PyResult<()> {
        self.inner
            .initialize_event(geo::Point::new(x, y), description)
            .map(|_| ())
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Add an (Active) scan zone to the current event. Raises if no event.
    fn add_zone(&mut self, zone: PyScanZone) -> PyResult<()> {
        self.inner
            .add_zone(zone.inner)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Push a raw CSI frame (equal-length `amplitudes`/`phases`) into the
    /// detection pipeline. Raises on empty/mismatched input. GIL released.
    fn push_csi_data(
        &self,
        py: Python<'_>,
        amplitudes: Vec<f64>,
        phases: Vec<f64>,
    ) -> PyResult<()> {
        py.detach(|| self.inner.push_csi_data(&amplitudes, &phases))
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Run exactly one scan cycle over the buffered CSI (detection → ensemble
    /// → localization → triage). Requires an initialized event with an Active
    /// zone. GIL released during the scan.
    fn scan_once(&mut self, py: Python<'_>) -> PyResult<()> {
        let rt = &self.rt;
        let inner = &mut self.inner;
        py.detach(|| rt.block_on(inner.start_scanning()))
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// All detected survivors.
    fn survivors(&self, py: Python<'_>) -> PyResult<Vec<PySurvivor>> {
        self.inner
            .survivors()
            .into_iter()
            .map(|s| PySurvivor::from_rust(py, s))
            .collect()
    }

    /// Survivors filtered by START triage class.
    fn survivors_by_triage(
        &self,
        py: Python<'_>,
        status: PyTriageStatus,
    ) -> PyResult<Vec<PySurvivor>> {
        self.inner
            .survivors_by_triage(status.as_rust())
            .into_iter()
            .map(|s| PySurvivor::from_rust(py, s))
            .collect()
    }

    fn __repr__(&self) -> String {
        "DisasterResponse()".to_string()
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyDisasterType>()?;
    m.add_class::<PyTriageStatus>()?;
    m.add_class::<PyVitalSignsReading>()?;
    m.add_class::<PySurvivor>()?;
    m.add_class::<PyDisasterConfig>()?;
    m.add_class::<PyScanZone>()?;
    m.add_class::<PyDisasterResponse>()?;
    Ok(())
}
