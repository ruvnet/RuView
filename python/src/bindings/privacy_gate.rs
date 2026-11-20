//! ADR-118 / ADR-125 §2.1.d — Python binding for the BFLD `PrivacyClass`
//! enum and the HAP-eligibility gate.
//!
//! Python:
//! ```python
//! from wifi_densepose import PrivacyClass, allows_hap, allows_matter, allows_network
//!
//! PrivacyClass.Anonymous            # → 2
//! allows_hap(PrivacyClass.Raw)      # → False  (I1 invariant)
//! allows_hap(PrivacyClass.Anonymous)# → True
//! allows_matter(PrivacyClass.Restricted)  # → True (ADR-122 §2.4)
//! ```
//!
//! This is the SOTA replacement for the Python port that ships in
//! `scripts/c6-presence-watcher.py::PrivacyClass`. When the
//! `wifi-densepose` PyPI wheel lands (ADR-117 P5), runtimes flip from
//! the Python port to this Rust-backed binding and get the same enum
//! semantics as every other consumer of the published
//! `wifi-densepose-bfld 0.3.0` crate.

use pyo3::prelude::*;
use wifi_densepose_bfld::PrivacyClass;

/// Python-facing wrapper for [`wifi_densepose_bfld::PrivacyClass`].
///
/// Repr matches the Rust enum byte values 0..=3.
#[pyclass(from_py_object, eq, eq_int, hash, frozen, name = "PrivacyClass", module = "wifi_densepose")]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PyPrivacyClass {
    Raw = 0,
    Derived = 1,
    Anonymous = 2,
    Restricted = 3,
}

impl From<PrivacyClass> for PyPrivacyClass {
    fn from(c: PrivacyClass) -> Self {
        match c {
            PrivacyClass::Raw => Self::Raw,
            PrivacyClass::Derived => Self::Derived,
            PrivacyClass::Anonymous => Self::Anonymous,
            PrivacyClass::Restricted => Self::Restricted,
        }
    }
}

impl From<PyPrivacyClass> for PrivacyClass {
    fn from(c: PyPrivacyClass) -> Self {
        match c {
            PyPrivacyClass::Raw => Self::Raw,
            PyPrivacyClass::Derived => Self::Derived,
            PyPrivacyClass::Anonymous => Self::Anonymous,
            PyPrivacyClass::Restricted => Self::Restricted,
        }
    }
}

#[pymethods]
impl PyPrivacyClass {
    /// True if frames of this class may cross a `NetworkSink`.
    /// Class 0 (`Raw`) is local-only by structural invariant I1
    /// (ADR-118 §2.2).
    #[getter]
    fn allows_network(&self) -> bool {
        PrivacyClass::from(*self).allows_network()
    }

    /// True if frames of this class may cross the Matter boundary.
    /// Only classes 2 (`Anonymous`) and 3 (`Restricted`) qualify per
    /// ADR-122 §2.4 / ADR-125 §2.1.d.
    #[getter]
    fn allows_matter(&self) -> bool {
        PrivacyClass::from(*self).allows_matter()
    }

    /// True if frames of this class may cross the HomeKit Accessory
    /// Protocol boundary. Same set as `allows_matter` — class 2 or 3.
    #[getter]
    fn allows_hap(&self) -> bool {
        // HAP eligibility is the same shape as Matter eligibility per
        // ADR-125 §2.1.d; we don't add a separate Rust method until
        // there's a divergence to justify it.
        PrivacyClass::from(*self).allows_matter()
    }

    /// Byte value (0..=3) for serialization.
    #[getter]
    fn as_u8(&self) -> u8 {
        PrivacyClass::from(*self).as_u8()
    }

    fn __repr__(&self) -> String {
        match self {
            Self::Raw => "PrivacyClass.Raw",
            Self::Derived => "PrivacyClass.Derived",
            Self::Anonymous => "PrivacyClass.Anonymous",
            Self::Restricted => "PrivacyClass.Restricted",
        }
        .to_string()
    }

    /// Map a byte value 0..=3 to the corresponding `PrivacyClass`.
    /// Raises `ValueError` on out-of-range input.
    #[staticmethod]
    fn from_u8(v: u8) -> PyResult<Self> {
        PrivacyClass::try_from(v)
            .map(Self::from)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    /// Map a string ("raw" / "derived" / "anonymous" / "restricted",
    /// case-insensitive) to the corresponding `PrivacyClass`. Raises
    /// `ValueError` on unknown names.
    #[staticmethod]
    fn from_str(s: &str) -> PyResult<Self> {
        match s.to_ascii_lowercase().as_str() {
            "raw" => Ok(Self::Raw),
            "derived" => Ok(Self::Derived),
            "anonymous" => Ok(Self::Anonymous),
            "restricted" => Ok(Self::Restricted),
            _ => Err(pyo3::exceptions::PyValueError::new_err(format!(
                "invalid PrivacyClass name: {s:?} (expected raw/derived/anonymous/restricted)"
            ))),
        }
    }
}

/// Free-function helper: `True` iff `c` may cross the HAP boundary.
/// Convenience wrapper so Python callers can write
/// `allows_hap(PrivacyClass.Anonymous)` without method-call syntax.
#[pyfunction]
fn allows_hap(c: PyPrivacyClass) -> bool {
    c.allows_hap()
}

/// Free-function helper: `True` iff `c` may cross a `NetworkSink`.
#[pyfunction]
fn allows_network(c: PyPrivacyClass) -> bool {
    c.allows_network()
}

/// Free-function helper: `True` iff `c` may cross the Matter boundary.
#[pyfunction]
fn allows_matter(c: PyPrivacyClass) -> bool {
    c.allows_matter()
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPrivacyClass>()?;
    m.add_function(wrap_pyfunction!(allows_hap, m)?)?;
    m.add_function(wrap_pyfunction!(allows_network, m)?)?;
    m.add_function(wrap_pyfunction!(allows_matter, m)?)?;
    Ok(())
}
