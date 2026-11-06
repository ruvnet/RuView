//! Model-artifact reading for the ADR-298 release gate (issue #1521).
//!
//! ADR-298 defines the *gate* ([`crate::model_gates`]) but the repository had
//! no way to point it at a real artifact: the only entry point was a library
//! function, so a publish could ship a degenerate classifier without anything
//! noticing. This module supplies the missing half — a dependency-free reader
//! for the two artifact shapes a model release publishes:
//!
//! * a `.safetensors` container (the published `model.safetensors`), and
//! * the `presence-head.json` sidecar (`{"weights": [...], "bias": <number>}`).
//!
//! It also checks the container against the safetensors format spec (§2.1):
//! the JSON header must be padded with `0x20` spaces. The published
//! `model.safetensors` was padded with NUL bytes instead, which the reference
//! deserializer rejects (issue #1522) even though the in-tree reader is lenient
//! enough to load it.
//!
//! Nothing here executes or evaluates a model; it only reads weights.
//!
//! # Scope
//!
//! The reader supports `F32` and `F64` tensors, which is what the published
//! artifacts carry. Every other dtype is rejected with a typed error rather
//! than silently mis-decoded.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;
use thiserror::Error;

use crate::model_gates::{GateError, LinearHead};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors raised while reading a model artifact.
///
/// These are *malformed or unreadable artifact* errors. A well-formed artifact
/// that the release gate rejects produces a [`crate::model_gates::GateFailure`]
/// instead, and a container that violates the format spec produces a
/// [`FormatVerdict::Violation`].
#[derive(Debug, Error)]
pub enum ArtifactError {
    /// The file could not be read.
    #[error("cannot read `{path}`: {source}")]
    Io {
        /// Path that failed to read.
        path: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The file is shorter than the 8-byte header length prefix.
    #[error("the file is too short to hold a safetensors header length")]
    Truncated,

    /// The declared header length does not fit inside the file.
    #[error("the declared header length ({declared} bytes) does not fit in the file ({size} bytes)")]
    HeaderOutOfBounds {
        /// Header length declared by the file.
        declared: usize,
        /// Total file size.
        size: usize,
    },

    /// The header is not a JSON object describing tensors.
    #[error("the safetensors header is malformed: {detail}")]
    MalformedHeader {
        /// What exactly was wrong with the header.
        detail: String,
    },

    /// The requested tensor is not in the artifact.
    #[error("tensor `{name}` is not present in the artifact (available: {available})")]
    TensorNotFound {
        /// The tensor that was asked for.
        name: String,
        /// The tensors the artifact does carry.
        available: String,
    },

    /// The tensor's dtype is not one the gate can decode.
    #[error("tensor `{name}` has dtype `{dtype}`; the release gate reads F32 and F64 tensors")]
    UnsupportedDtype {
        /// The tensor that was asked for.
        name: String,
        /// The dtype the artifact declares.
        dtype: String,
    },

    /// The tensor's byte range falls outside the data buffer.
    #[error("tensor `{name}` spans bytes {start}..{end} of a {size}-byte data buffer")]
    DataOutOfBounds {
        /// The tensor that was asked for.
        name: String,
        /// First byte of the tensor's data.
        start: usize,
        /// One past the last byte of the tensor's data.
        end: usize,
        /// Size of the data buffer.
        size: usize,
    },

    /// A bias tensor does not hold exactly one element.
    #[error("tensor `{name}` holds {count} element(s); a bias tensor must hold exactly one")]
    UnexpectedElementCount {
        /// The tensor that was read.
        name: String,
        /// How many elements it actually holds.
        count: usize,
    },

    /// The presence-head JSON does not carry a required field.
    #[error("the presence-head JSON is missing `{field}`")]
    MissingField {
        /// The missing field name.
        field: &'static str,
    },

    /// The presence-head JSON carries a field of the wrong shape.
    #[error("the presence-head JSON field `{field}` is malformed: {detail}")]
    MalformedField {
        /// The offending field name.
        field: &'static str,
        /// What was wrong with it.
        detail: String,
    },

    /// The extracted weights do not form a usable linear head.
    #[error("the linear head is malformed: {0}")]
    Gate(#[from] GateError),
}

// ---------------------------------------------------------------------------
// Format verdict
// ---------------------------------------------------------------------------

/// The verdict of the safetensors container-format check.
///
/// The format spec (§2.1) requires the JSON header to be padded with `0x20`
/// space bytes. NUL padding is *not* JSON whitespace, so a NUL-padded header is
/// rejected by the reference deserializer — the defect issue #1522 reported on
/// the published `model.safetensors`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatVerdict {
    /// The container parses and satisfies the format spec.
    Compliant,
    /// The container parses but violates the format spec.
    Violation {
        /// Human-readable description of the violation.
        detail: String,
    },
}

impl FormatVerdict {
    /// Whether the container satisfies the format spec.
    #[must_use]
    pub fn is_compliant(&self) -> bool {
        matches!(self, FormatVerdict::Compliant)
    }
}

// ---------------------------------------------------------------------------
// Tensor info
// ---------------------------------------------------------------------------

/// One tensor entry from a safetensors header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TensorInfo {
    /// The safetensors dtype string (`F32`, `F64`, …).
    pub dtype: String,
    /// The tensor shape as declared in the header.
    pub shape: Vec<usize>,
    /// First byte of the tensor's data inside the data buffer.
    pub start: usize,
    /// One past the last byte of the tensor's data.
    pub end: usize,
}

// ---------------------------------------------------------------------------
// SafeTensorsFile
// ---------------------------------------------------------------------------

/// A parsed safetensors container.
#[derive(Debug, Clone)]
pub struct SafeTensorsFile {
    /// Length of the header region (JSON plus padding), as declared by the file.
    header_len: usize,
    /// Length of the JSON object inside the header region.
    json_len: usize,
    /// The alignment padding bytes that follow the JSON object.
    padding: Vec<u8>,
    /// The tensor entries, keyed by tensor name.
    tensors: BTreeMap<String, TensorInfo>,
    /// The tensor data buffer.
    data: Vec<u8>,
}

impl SafeTensorsFile {
    /// Read and parse a safetensors file from disk.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactError::Io`] when the file cannot be read and
    /// [`ArtifactError::Truncated`] / [`ArtifactError::HeaderOutOfBounds`] /
    /// [`ArtifactError::MalformedHeader`] when it is not a parsable container.
    pub fn open(path: &Path) -> Result<Self, ArtifactError> {
        let bytes = std::fs::read(path).map_err(|source| ArtifactError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_bytes(bytes)
    }

    /// Parse a safetensors container from bytes.
    ///
    /// A container whose header carries non-whitespace padding still parses —
    /// the padding defect is reported by [`Self::format_verdict`] rather than
    /// hiding the tensors.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactError::Truncated`] when the file is shorter than the
    /// 8-byte header length prefix, [`ArtifactError::HeaderOutOfBounds`] when
    /// the declared header length does not fit, and
    /// [`ArtifactError::MalformedHeader`] when the header is not a JSON object
    /// describing tensors.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, ArtifactError> {
        if bytes.len() < 8 {
            return Err(ArtifactError::Truncated);
        }
        let mut len_bytes = [0u8; 8];
        len_bytes.copy_from_slice(&bytes[..8]);
        let declared = u64::from_le_bytes(len_bytes) as usize;
        let size = bytes.len();
        if declared > size - 8 {
            return Err(ArtifactError::HeaderOutOfBounds { declared, size });
        }
        let header_bytes = &bytes[8..8 + declared];
        let (json_len, padding) = split_header(header_bytes)?;
        let tensors = parse_tensors(&header_bytes[..json_len])?;
        Ok(Self {
            header_len: declared,
            json_len,
            padding,
            tensors,
            data: bytes[8 + declared..].to_vec(),
        })
    }

    /// Length of the header region in bytes (JSON plus padding).
    #[must_use]
    pub fn header_len(&self) -> usize {
        self.header_len
    }

    /// Length of the JSON object inside the header, in bytes.
    #[must_use]
    pub fn json_len(&self) -> usize {
        self.json_len
    }

    /// The alignment padding bytes that follow the JSON object.
    #[must_use]
    pub fn padding(&self) -> &[u8] {
        &self.padding
    }

    /// The names of every tensor in the container, in sorted order.
    #[must_use]
    pub fn tensor_names(&self) -> Vec<&str> {
        self.tensors.keys().map(String::as_str).collect()
    }

    /// The header entry for one tensor.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactError::TensorNotFound`] when the artifact does not
    /// carry that tensor.
    pub fn tensor(&self, name: &str) -> Result<&TensorInfo, ArtifactError> {
        self.tensors.get(name).ok_or_else(|| ArtifactError::TensorNotFound {
            name: name.to_string(),
            available: self.tensor_names().join(", "),
        })
    }

    /// Read one tensor as `f32` values.
    ///
    /// `F64` tensors are converted; every other dtype is rejected with
    /// [`ArtifactError::UnsupportedDtype`].
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactError::TensorNotFound`],
    /// [`ArtifactError::UnsupportedDtype`] or [`ArtifactError::DataOutOfBounds`].
    pub fn tensor_f32(&self, name: &str) -> Result<Vec<f32>, ArtifactError> {
        let info = self.tensor(name)?;
        let slice = self
            .data
            .get(info.start..info.end)
            .ok_or_else(|| ArtifactError::DataOutOfBounds {
                name: name.to_string(),
                start: info.start,
                end: info.end,
                size: self.data.len(),
            })?;
        match info.dtype.as_str() {
            "F32" => Ok(slice
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect()),
            "F64" => Ok(slice
                .chunks_exact(8)
                .map(|c| {
                    f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]) as f32
                })
                .collect()),
            other => Err(ArtifactError::UnsupportedDtype {
                name: name.to_string(),
                dtype: other.to_string(),
            }),
        }
    }

    /// Extract the linear classification head from two tensors.
    ///
    /// The weight tensor is flattened, so a head stored as `(128,)` or `(128, 1)`
    /// reads the same. The bias tensor must hold exactly one element.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactError::TensorNotFound`],
    /// [`ArtifactError::UnsupportedDtype`], [`ArtifactError::DataOutOfBounds`],
    /// [`ArtifactError::UnexpectedElementCount`] or [`ArtifactError::Gate`] when
    /// the extracted values do not form a usable head.
    pub fn linear_head(
        &self,
        weight_name: &str,
        bias_name: &str,
    ) -> Result<LinearHead, ArtifactError> {
        let weight = self.tensor_f32(weight_name)?;
        let bias = self.tensor_f32(bias_name)?;
        if bias.len() != 1 {
            return Err(ArtifactError::UnexpectedElementCount {
                name: bias_name.to_string(),
                count: bias.len(),
            });
        }
        Ok(LinearHead::new(weight, bias[0])?)
    }

    /// Check the container against the safetensors format spec.
    ///
    /// The only requirement checked here is the header padding: the format
    /// (§2.1) pads the JSON header with `0x20` space bytes, and NUL padding is
    /// rejected by the reference deserializer (issue #1522).
    ///
    /// Fix marker: RuView#1522 — a NUL-padded header is a violation, not a
    /// warning.
    #[must_use]
    pub fn format_verdict(&self) -> FormatVerdict {
        let mut nul = 0usize;
        let mut first_offset = 0usize;
        for (i, &byte) in self.padding.iter().enumerate() {
            if byte != 0x20 {
                if nul == 0 {
                    first_offset = i;
                }
                nul += 1;
            }
        }
        if nul == 0 {
            return FormatVerdict::Compliant;
        }
        FormatVerdict::Violation {
            detail: format!(
                "the JSON header is padded with {nul} non-space byte(s) (first at header offset \
                 {first_offset}, byte 0x{:02x}); the safetensors format requires 0x20 space padding, \
                 and a reference deserializer rejects this file",
                self.padding[first_offset]
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Presence-head JSON
// ---------------------------------------------------------------------------

/// Read a presence head from the `presence-head.json` sidecar.
///
/// The published sidecar is `{"weights": [...], "bias": <number>}` — the shape
/// the export script writes.
///
/// # Errors
///
/// Returns [`ArtifactError::MissingField`] when `weights` or `bias` is absent,
/// [`ArtifactError::MalformedField`] when a field has the wrong shape, and
/// [`ArtifactError::Gate`] when the values do not form a usable head.
pub fn presence_head_from_json(bytes: &[u8]) -> Result<LinearHead, ArtifactError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| ArtifactError::MalformedField {
        field: "root",
        detail: e.to_string(),
    })?;
    let object = value.as_object().ok_or_else(|| ArtifactError::MalformedField {
        field: "root",
        detail: "expected a JSON object".to_string(),
    })?;
    let raw_weights = object
        .get("weights")
        .and_then(Value::as_array)
        .ok_or(ArtifactError::MissingField { field: "weights" })?;
    let mut weight = Vec::with_capacity(raw_weights.len());
    for (i, value) in raw_weights.iter().enumerate() {
        let number = value.as_f64().ok_or_else(|| ArtifactError::MalformedField {
            field: "weights",
            detail: format!("element {i} is not a number"),
        })?;
        weight.push(number as f32);
    }
    let bias = object
        .get("bias")
        .and_then(Value::as_f64)
        .ok_or(ArtifactError::MissingField { field: "bias" })?;
    Ok(LinearHead::new(weight, bias as f32)?)
}

// ---------------------------------------------------------------------------
// Reference writer
// ---------------------------------------------------------------------------

/// Build a safetensors container from named `F32` tensors.
///
/// This is the reference writer the release-gate tests build their fixtures
/// with. `pad_byte` selects the alignment padding: `0x20` produces a
/// spec-compliant container, `0x00` reproduces the non-compliant padding the
/// published `model.safetensors` carried (issue #1522).
///
/// The header is padded to the next multiple of eight bytes strictly greater
/// than the JSON length, so a container always carries at least one padding
/// byte — that is what makes the NUL-padded fixture reproducible.
#[must_use]
pub fn build_safetensors(tensors: &[(&str, &[f32])], pad_byte: u8) -> Vec<u8> {
    let mut names: Vec<&str> = tensors.iter().map(|(name, _)| *name).collect();
    names.sort_unstable();
    names.dedup();

    let mut data: Vec<u8> = Vec::new();
    let mut header: BTreeMap<String, Value> = BTreeMap::new();
    for name in &names {
        let values = tensors
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, values)| *values)
            .unwrap_or(&[]);
        let start = data.len();
        for value in values {
            data.extend_from_slice(&value.to_le_bytes());
        }
        let end = data.len();
        header.insert(
            (*name).to_string(),
            serde_json::json!({
                "dtype": "F32",
                "shape": [values.len()],
                "data_offsets": [start, end],
            }),
        );
    }

    let json = serde_json::to_string(&header).unwrap_or_else(|_| "{}".to_string());
    let mut header_bytes = json.into_bytes();
    while header_bytes.len() % 8 == 0 {
        header_bytes.push(pad_byte);
    }
    while header_bytes.len() % 8 != 0 {
        header_bytes.push(pad_byte);
    }

    let mut out: Vec<u8> = Vec::with_capacity(8 + header_bytes.len() + data.len());
    out.extend_from_slice(&(header_bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(&header_bytes);
    out.extend_from_slice(&data);
    out
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Split a header region into its JSON object and the alignment padding.
///
/// The JSON object is read with a streaming deserializer so the byte offset it
/// stopped at is the exact end of the JSON — anything after it is padding,
/// whatever byte value it carries.
fn split_header(header: &[u8]) -> Result<(usize, Vec<u8>), ArtifactError> {
    let mut stream = serde_json::Deserializer::from_slice(header).into_iter::<Value>();
    let value = match stream.next() {
        Some(Ok(value)) => value,
        Some(Err(e)) => {
            return Err(ArtifactError::MalformedHeader {
                detail: e.to_string(),
            })
        }
        None => {
            return Err(ArtifactError::MalformedHeader {
                detail: "the header is empty".to_string(),
            })
        }
    };
    if !value.is_object() {
        return Err(ArtifactError::MalformedHeader {
            detail: "the header is not a JSON object".to_string(),
        });
    }
    let json_len = stream.byte_offset();
    Ok((json_len, header[json_len..].to_vec()))
}

/// Parse the tensor entries out of a safetensors JSON header.
fn parse_tensors(json: &[u8]) -> Result<BTreeMap<String, TensorInfo>, ArtifactError> {
    let value: Value = serde_json::from_slice(json).map_err(|e| ArtifactError::MalformedHeader {
        detail: e.to_string(),
    })?;
    let object = value.as_object().ok_or_else(|| ArtifactError::MalformedHeader {
        detail: "the header is not a JSON object".to_string(),
    })?;

    let mut tensors = BTreeMap::new();
    for (name, entry) in object {
        if name == "__metadata__" {
            continue;
        }
        let entry = entry.as_object().ok_or_else(|| ArtifactError::MalformedHeader {
            detail: format!("tensor `{name}` is not a JSON object"),
        })?;
        let dtype = entry
            .get("dtype")
            .and_then(Value::as_str)
            .ok_or_else(|| ArtifactError::MalformedHeader {
                detail: format!("tensor `{name}` has no dtype"),
            })?
            .to_string();
        let shape = entry
            .get("shape")
            .and_then(Value::as_array)
            .ok_or_else(|| ArtifactError::MalformedHeader {
                detail: format!("tensor `{name}` has no shape"),
            })?;
        let mut dims = Vec::with_capacity(shape.len());
        for dim in shape {
            dims.push(dim.as_u64().ok_or_else(|| ArtifactError::MalformedHeader {
                detail: format!("tensor `{name}` has a non-integer shape entry"),
            })? as usize);
        }
        let offsets = entry
            .get("data_offsets")
            .and_then(Value::as_array)
            .ok_or_else(|| ArtifactError::MalformedHeader {
                detail: format!("tensor `{name}` has no data_offsets"),
            })?;
        if offsets.len() != 2 {
            return Err(ArtifactError::MalformedHeader {
                detail: format!("tensor `{name}` has {} data_offsets; expected 2", offsets.len()),
            });
        }
        let start = offsets[0].as_u64().ok_or_else(|| ArtifactError::MalformedHeader {
            detail: format!("tensor `{name}` has a non-integer data offset"),
        })? as usize;
        let end = offsets[1].as_u64().ok_or_else(|| ArtifactError::MalformedHeader {
            detail: format!("tensor `{name}` has a non-integer data offset"),
        })? as usize;
        tensors.insert(
            name.clone(),
            TensorInfo {
                dtype,
                shape: dims,
                start,
                end,
            },
        );
    }
    Ok(tensors)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy_head_tensors() -> Vec<(&'static str, Vec<f32>)> {
        vec![
            ("presence_head.weights", vec![2.0, 2.0, 2.0, 2.0]),
            ("presence_head.bias", vec![0.0]),
        ]
    }

    #[test]
    fn reads_a_spec_compliant_container() {
        let tensors = healthy_head_tensors();
        let bytes = build_safetensors(
            &tensors
                .iter()
                .map(|(name, values)| (*name, values.as_slice()))
                .collect::<Vec<_>>(),
            0x20,
        );
        let file = SafeTensorsFile::from_bytes(bytes).expect("container parses");
        assert!(file.format_verdict().is_compliant());
        assert_eq!(
            file.tensor_names(),
            vec!["presence_head.bias", "presence_head.weights"]
        );
        let head = file
            .linear_head("presence_head.weights", "presence_head.bias")
            .expect("head reads");
        assert_eq!(head.dim(), 4);
        assert!((head.bias()).abs() < 1e-6);
        assert!((head.weight_norm() - 4.0).abs() < 1e-4);
    }

    #[test]
    fn detects_nul_padded_headers() {
        let tensors = healthy_head_tensors();
        let bytes = build_safetensors(
            &tensors
                .iter()
                .map(|(name, values)| (*name, values.as_slice()))
                .collect::<Vec<_>>(),
            0x00,
        );
        let file = SafeTensorsFile::from_bytes(bytes).expect("container parses");
        match file.format_verdict() {
            FormatVerdict::Violation { detail } => {
                assert!(detail.contains("0x00"), "detail names the byte: {detail}");
            }
            FormatVerdict::Compliant => panic!("a NUL-padded header must not be compliant"),
        }
        // The tensors are still readable — the defect is in the container, not
        // in the weights.
        assert!(file.tensor_f32("presence_head.weights").is_ok());
    }

    #[test]
    fn rejects_a_truncated_file() {
        assert!(matches!(
            SafeTensorsFile::from_bytes(vec![0, 1, 2]),
            Err(ArtifactError::Truncated)
        ));
    }

    #[test]
    fn rejects_a_header_length_that_does_not_fit() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&9999u64.to_le_bytes());
        bytes.extend_from_slice(b"{}");
        assert!(matches!(
            SafeTensorsFile::from_bytes(bytes),
            Err(ArtifactError::HeaderOutOfBounds { .. })
        ));
    }

    #[test]
    fn reports_a_missing_tensor_by_name() {
        let tensors = healthy_head_tensors();
        let bytes = build_safetensors(
            &tensors
                .iter()
                .map(|(name, values)| (*name, values.as_slice()))
                .collect::<Vec<_>>(),
            0x20,
        );
        let file = SafeTensorsFile::from_bytes(bytes).expect("container parses");
        let err = file.tensor("presence_head.missing").unwrap_err();
        assert!(matches!(err, ArtifactError::TensorNotFound { .. }));
        assert!(err.to_string().contains("presence_head.bias"));
    }

    #[test]
    fn rejects_a_bias_tensor_with_more_than_one_element() {
        let bytes = build_safetensors(
            &[
                ("presence_head.weights", &[1.0f32, 2.0][..]),
                ("presence_head.bias", &[0.5f32, 0.25][..]),
            ],
            0x20,
        );
        let file = SafeTensorsFile::from_bytes(bytes).expect("container parses");
        let err = file
            .linear_head("presence_head.weights", "presence_head.bias")
            .unwrap_err();
        assert!(matches!(
            err,
            ArtifactError::UnexpectedElementCount { count: 2, .. }
        ));
    }

    #[test]
    fn reads_the_published_presence_head_json_shape() {
        let head = presence_head_from_json(
            br#"{"weights":[0.5,0.5,0.5,0.5],"bias":0.25}"#,
        )
        .expect("head reads");
        assert_eq!(head.dim(), 4);
        assert!((head.bias() - 0.25).abs() < 1e-6);
    }

    #[test]
    fn reports_a_missing_field_in_the_presence_head_json() {
        let err = presence_head_from_json(br#"{"bias":0.25}"#).unwrap_err();
        assert!(matches!(err, ArtifactError::MissingField { field: "weights" }));
    }
}
