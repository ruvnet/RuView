//! ADR-117 P3.5 — Beamforming Feedback Loop Data (BFLD) bindings.
//!
//! BFLD is the transmitter-side, AP-station-loop view of the WiFi
//! channel — compressed beamforming feedback frames that 802.11ac/ax/be
//! stations send to the AP per sounding cycle. See ADR-117 §5.7a for
//! the design rationale and ADR-117 §11.11/12 for open questions.
//!
//! **Important**: there is NO Rust ingestion crate for BFLD yet. The
//! Python types in this module ship with a **stub Rust impl** that
//! accepts pre-parsed feedback matrices via numpy. When the future
//! `wifi-densepose-bfld` crate lands, it plugs in here without changing
//! the Python API.
//!
//! Today's user path:
//!
//! 1. Capture BFR frames with `tcpdump` / Wireshark + the BFR dissector
//!    (or via `mac80211` debugfs on Linux 6.10+)
//! 2. Parse the compressed feedback into a numpy complex128 ndarray
//!    `[Nr × Nc × Nsc]` using your favourite Python BFR parser
//! 3. Construct `BfldFrame.from_compressed_feedback(...)` to hand the
//!    matrix to RuView
//!
//! Tomorrow (post-v2.0): `wifi-densepose-bfld` does steps 1+2 for you.

use pyo3::prelude::*;
use numpy::{Complex64, PyArray3, PyUntypedArrayMethods, PyReadonlyArray3};

// ─── BfldKind ────────────────────────────────────────────────────────

/// 802.11 PHY variant of the captured BFR frame. Determines the
/// expected matrix dimensions + the quantization step of the
/// compressed angles.
///
/// Python:
/// ```python
/// from wifi_densepose import BfldKind
/// BfldKind.CompressedHE80   # 802.11ax 80 MHz compressed BFR
/// ```
#[pyclass(eq, eq_int, hash, frozen, name = "BfldKind")]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PyBfldKind {
    CompressedHE20 = 0,
    CompressedHE40 = 1,
    CompressedHE80 = 2,
    CompressedHE160 = 3,
    UncompressedHT20 = 4,
    UncompressedHT40 = 5,
}

#[pymethods]
impl PyBfldKind {
    /// Expected number of subcarriers for this BFLD variant.
    #[getter]
    fn n_subcarriers(&self) -> usize {
        match self {
            Self::CompressedHE20 => 242,
            Self::CompressedHE40 => 484,
            Self::CompressedHE80 => 996,
            Self::CompressedHE160 => 1992,
            Self::UncompressedHT20 => 52,
            Self::UncompressedHT40 => 108,
        }
    }

    /// Bandwidth in MHz for this BFLD variant.
    #[getter]
    fn bandwidth_mhz(&self) -> u16 {
        match self {
            Self::CompressedHE20 | Self::UncompressedHT20 => 20,
            Self::CompressedHE40 | Self::UncompressedHT40 => 40,
            Self::CompressedHE80 => 80,
            Self::CompressedHE160 => 160,
        }
    }

    /// True for 802.11ax (HE) variants, false for legacy HT.
    #[getter]
    fn is_he(&self) -> bool {
        matches!(
            self,
            Self::CompressedHE20
                | Self::CompressedHE40
                | Self::CompressedHE80
                | Self::CompressedHE160
        )
    }

    fn __repr__(&self) -> String {
        let name = match self {
            Self::CompressedHE20 => "CompressedHE20",
            Self::CompressedHE40 => "CompressedHE40",
            Self::CompressedHE80 => "CompressedHE80",
            Self::CompressedHE160 => "CompressedHE160",
            Self::UncompressedHT20 => "UncompressedHT20",
            Self::UncompressedHT40 => "UncompressedHT40",
        };
        format!("BfldKind.{}", name)
    }
}

// ─── BfldFrame ───────────────────────────────────────────────────────

/// One BFR snapshot: a compressed beamforming feedback matrix tagged
/// with metadata (timestamp, sounding sequence, source MAC, kind).
///
/// Backing storage: a numpy complex128 ndarray `[Nr × Nc × Nsc]`. The
/// Python constructor accepts the ndarray directly; under the hood we
/// hold a `Vec<Complex64>` in row-major order.
///
/// Python:
/// ```python
/// import numpy as np
/// from wifi_densepose import BfldFrame, BfldKind
///
/// fb = np.zeros((2, 1, 996), dtype=np.complex128)  # Nr=2, Nc=1, Nsc=996
/// frame = BfldFrame.from_compressed_feedback(
///     timestamp_ms=1234,
///     sounding_index=42,
///     sta_mac="aa:bb:cc:dd:ee:ff",
///     kind=BfldKind.CompressedHE80,
///     feedback_matrix=fb,
/// )
/// print(frame.n_subcarriers, frame.kind, frame.n_rows, frame.n_cols)
/// ```
#[pyclass(frozen, name = "BfldFrame")]
pub struct PyBfldFrame {
    timestamp_ms: i64,
    sounding_index: u32,
    sta_mac: String,
    kind: PyBfldKind,
    n_rows: usize,
    n_cols: usize,
    n_subcarriers: usize,
    // Row-major storage of the [Nr × Nc × Nsc] complex matrix.
    // Length = n_rows * n_cols * n_subcarriers.
    matrix: Vec<Complex64>,
}

#[pymethods]
impl PyBfldFrame {
    /// Construct from a pre-parsed numpy complex128 ndarray of shape
    /// `[n_rows, n_cols, n_subcarriers]`. The last dimension MUST
    /// match `kind.n_subcarriers`.
    #[staticmethod]
    fn from_compressed_feedback<'py>(
        timestamp_ms: i64,
        sounding_index: u32,
        sta_mac: &str,
        kind: PyBfldKind,
        feedback_matrix: PyReadonlyArray3<'py, Complex64>,
    ) -> PyResult<Self> {
        let shape = feedback_matrix.shape();
        let n_rows = shape[0];
        let n_cols = shape[1];
        let n_subcarriers = shape[2];
        let expected = kind.n_subcarriers();
        if n_subcarriers != expected {
            return Err(pyo3::exceptions::PyValueError::new_err(format!(
                "feedback_matrix subcarrier dim {} does not match {:?}.n_subcarriers={}",
                n_subcarriers, kind, expected
            )));
        }
        // Copy into row-major Vec. This is the safe path; PyArray3 is
        // also row-major by default.
        let matrix: Vec<Complex64> = feedback_matrix
            .as_array()
            .iter()
            .copied()
            .collect();
        Ok(Self {
            timestamp_ms,
            sounding_index,
            sta_mac: sta_mac.to_string(),
            kind,
            n_rows,
            n_cols,
            n_subcarriers,
            matrix,
        })
    }

    #[getter]
    fn timestamp_ms(&self) -> i64 { self.timestamp_ms }

    #[getter]
    fn sounding_index(&self) -> u32 { self.sounding_index }

    #[getter]
    fn sta_mac(&self) -> &str { &self.sta_mac }

    #[getter]
    fn kind(&self) -> PyBfldKind { self.kind }

    #[getter]
    fn n_rows(&self) -> usize { self.n_rows }

    #[getter]
    fn n_cols(&self) -> usize { self.n_cols }

    #[getter]
    fn n_subcarriers(&self) -> usize { self.n_subcarriers }

    /// Mean amplitude across the entire matrix (sanity-check metric;
    /// production-grade sensing pipelines look at per-subcarrier or
    /// per-row stats instead).
    #[getter]
    fn mean_amplitude(&self) -> f64 {
        if self.matrix.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.matrix.iter().map(|c| c.norm()).sum();
        sum / self.matrix.len() as f64
    }

    /// Return the feedback matrix as a numpy complex128 ndarray of
    /// shape `[n_rows, n_cols, n_subcarriers]`. Allocates a fresh
    /// Python-owned array; the BfldFrame keeps its own copy.
    fn feedback_matrix<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray3<Complex64>> {
        PyArray3::from_vec3_bound(
            py,
            &self.reshape_to_vec3(),
        )
        .expect("Vec dimensions match the matrix shape — invariant of from_compressed_feedback")
    }

    fn __repr__(&self) -> String {
        format!(
            "BfldFrame(kind={:?}, nr={}, nc={}, nsc={}, sta={}, idx={}, mean_amp={:.4})",
            self.kind, self.n_rows, self.n_cols, self.n_subcarriers,
            self.sta_mac, self.sounding_index, self.mean_amplitude(),
        )
    }
}

impl PyBfldFrame {
    fn reshape_to_vec3(&self) -> Vec<Vec<Vec<Complex64>>> {
        let mut out = Vec::with_capacity(self.n_rows);
        for r in 0..self.n_rows {
            let mut row = Vec::with_capacity(self.n_cols);
            for c in 0..self.n_cols {
                let start = (r * self.n_cols + c) * self.n_subcarriers;
                let end = start + self.n_subcarriers;
                row.push(self.matrix[start..end].to_vec());
            }
            out.push(row);
        }
        out
    }
}

// ─── BfldReport ──────────────────────────────────────────────────────

/// Aggregator over a window of `BfldFrame`s — the natural "all BFR
/// data in this 60-second scan" container. Mirrors how `VitalReading`
/// aggregates `VitalEstimate`s in the vitals pipeline.
#[pyclass(name = "BfldReport")]
pub struct PyBfldReport {
    frames: Vec<u32>, // sounding indices we hold (don't deep-copy the matrices)
    timestamp_first: Option<i64>,
    timestamp_last: Option<i64>,
    kind: Option<PyBfldKind>,
    mean_amplitudes: Vec<f64>, // one per frame
}

#[pymethods]
impl PyBfldReport {
    #[new]
    fn new() -> Self {
        Self {
            frames: Vec::new(),
            timestamp_first: None,
            timestamp_last: None,
            kind: None,
            mean_amplitudes: Vec::new(),
        }
    }

    /// Add a frame to the report. All frames must share the same
    /// `kind`; the call errors if they don't.
    fn add_frame(&mut self, frame: &PyBfldFrame) -> PyResult<()> {
        if let Some(k) = self.kind {
            if k != frame.kind {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "frame kind {:?} does not match report kind {:?}",
                    frame.kind, k
                )));
            }
        } else {
            self.kind = Some(frame.kind);
        }
        self.frames.push(frame.sounding_index);
        self.timestamp_first = Some(self.timestamp_first.unwrap_or(frame.timestamp_ms).min(frame.timestamp_ms));
        self.timestamp_last = Some(self.timestamp_last.unwrap_or(frame.timestamp_ms).max(frame.timestamp_ms));
        self.mean_amplitudes.push(frame.mean_amplitude());
        Ok(())
    }

    #[getter]
    fn n_frames(&self) -> usize { self.frames.len() }

    #[getter]
    fn timestamp_first(&self) -> Option<i64> { self.timestamp_first }

    #[getter]
    fn timestamp_last(&self) -> Option<i64> { self.timestamp_last }

    #[getter]
    fn kind(&self) -> Option<PyBfldKind> { self.kind }

    /// Mean of the per-frame mean amplitudes — coarse sanity metric
    /// for "the scan captured a stable signal over the window".
    #[getter]
    fn coherence_score(&self) -> f64 {
        if self.mean_amplitudes.is_empty() {
            return 0.0;
        }
        let mean = self.mean_amplitudes.iter().sum::<f64>()
            / self.mean_amplitudes.len() as f64;
        if mean == 0.0 {
            return 0.0;
        }
        // Inverse coefficient of variation, clamped to [0, 1].
        let var = self.mean_amplitudes.iter()
            .map(|m| (m - mean).powi(2))
            .sum::<f64>()
            / self.mean_amplitudes.len() as f64;
        let cv = var.sqrt() / mean;
        (1.0 - cv.min(1.0)).max(0.0)
    }

    fn __repr__(&self) -> String {
        format!(
            "BfldReport(n_frames={}, kind={:?}, coherence={:.3})",
            self.frames.len(), self.kind, self.coherence_score(),
        )
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBfldKind>()?;
    m.add_class::<PyBfldFrame>()?;
    m.add_class::<PyBfldReport>()?;
    Ok(())
}
