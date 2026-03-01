//! Dataset loaders for WiFi-to-DensePose training pipeline (ADR-023 Phase 1).
//!
//! Provides unified data loading for MM-Fi (NeurIPS 2023) and Wi-Pose datasets,
//! with from-scratch .npy/.mat v5 parsers, subcarrier resampling, and a unified
//! `DataPipeline` for normalized, windowed training samples.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

// ── Error type ───────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum DatasetError {
    Io(io::Error),
    Format(String),
    Missing(String),
    Shape(String),
}

impl fmt::Display for DatasetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Format(s) => write!(f, "format error: {s}"),
            Self::Missing(s) => write!(f, "missing: {s}"),
            Self::Shape(s) => write!(f, "shape error: {s}"),
        }
    }
}

impl std::error::Error for DatasetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::Io(e) = self { Some(e) } else { None }
    }
}

impl From<io::Error> for DatasetError {
    fn from(e: io::Error) -> Self { Self::Io(e) }
}

pub type Result<T> = std::result::Result<T, DatasetError>;

// ── NpyArray ─────────────────────────────────────────────────────────────────

/// Dense array from .npy: flat f32 data with shape metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpyArray {
    pub shape: Vec<usize>,
    pub data: Vec<f32>,
}

impl NpyArray {
    pub fn len(&self) -> usize { self.data.len() }
    pub fn is_empty(&self) -> bool { self.data.is_empty() }
    pub fn ndim(&self) -> usize { self.shape.len() }
}

// ── NpyReader ────────────────────────────────────────────────────────────────

/// Minimal NumPy .npy format reader (f32/f64, v1/v2).
pub struct NpyReader;

impl NpyReader {
    pub fn read_file(path: &Path) -> Result<NpyArray> {
        Self::parse(&std::fs::read(path)?)
    }

    pub fn parse(buf: &[u8]) -> Result<NpyArray> {
        if buf.len() < 10 { return Err(DatasetError::Format("file too small for .npy".into())); }
        if &buf[0..6] != b"\x93NUMPY" {
            return Err(DatasetError::Format("missing .npy magic".into()));
        }
        let major = buf[6];
        let (header_len, header_start) = match major {
            1 => (u16::from_le_bytes([buf[8], buf[9]]) as usize, 10usize),
            2 | 3 => {
                if buf.len() < 12 { return Err(DatasetError::Format("truncated v2 header".into())); }
                (u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]) as usize, 12)
            }
            _ => return Err(DatasetError::Format(format!("unsupported .npy version {major}"))),
        };
        let header_end = header_start + header_len;
        if header_end > buf.len() { return Err(DatasetError::Format("header past EOF".into())); }
        let hdr = std::str::from_utf8(&buf[header_start..header_end])
            .map_err(|_| DatasetError::Format("non-UTF8 header".into()))?;

        let dtype = Self::extract_field(hdr, "descr")?;
        let is_f64 = dtype.contains("f8") || dtype.contains("float64");
        let is_f32 = dtype.contains("f4") || dtype.contains("float32");
        let is_big = dtype.starts_with('>');
        if !is_f32 && !is_f64 {
            return Err(DatasetError::Format(format!("unsupported dtype '{dtype}'")));
        }
        let fortran = Self::extract_field(hdr, "fortran_order")
            .unwrap_or_else(|_| "False".into()).contains("True");
        let shape = Self::parse_shape(hdr)?;
        let elem_sz: usize = if is_f64 { 8 } else { 4 };
        let total: usize = shape.iter().product::<usize>().max(1);
        if header_end + total * elem_sz > buf.len() {
            return Err(DatasetError::Format("data truncated".into()));
        }
        let raw = &buf[header_end..header_end + total * elem_sz];
        let mut data: Vec<f32> = if is_f64 {
            raw.chunks_exact(8).map(|c| {
                let v = if is_big { f64::from_be_bytes(c.try_into().unwrap()) }
                        else { f64::from_le_bytes(c.try_into().unwrap()) };
                v as f32
            }).collect()
        } else {
            raw.chunks_exact(4).map(|c| {
                if is_big { f32::from_be_bytes(c.try_into().unwrap()) }
                else { f32::from_le_bytes(c.try_into().unwrap()) }
            }).collect()
        };
        if fortran && shape.len() == 2 {
            let (r, c) = (shape[0], shape[1]);
            let mut cd = vec![0.0f32; data.len()];
            for ri in 0..r { for ci in 0..c { cd[ri*c+ci] = data[ci*r+ri]; } }
            data = cd;
        }
        let shape = if shape.is_empty() { vec![1] } else { shape };
        Ok(NpyArray { shape, data })
    }

    fn extract_field(hdr: &str, field: &str) -> Result<String> {
        for pat in &[format!("'{field}': "), format!("'{field}':"), format!("\"{field}\": ")] {
            if let Some(s) = hdr.find(pat.as_str()) {
                let rest = &hdr[s + pat.len()..];
                let end = rest.find(',').or_else(|| rest.find('}')).unwrap_or(rest.len());
                return Ok(rest[..end].trim().trim_matches('\'').trim_matches('"').into());
            }
        }
        Err(DatasetError::Format(format!("field '{field}' not found")))
    }

    fn parse_shape(hdr: &str) -> Result<Vec<usize>> {
        let si = hdr.find("'shape'").or_else(|| hdr.find("\"shape\""))
            .ok_or_else(|| DatasetError::Format("no 'shape'".into()))?;
        let rest = &hdr[si..];
        let ps = rest.find('(').ok_or_else(|| DatasetError::Format("no '('".into()))?;
        let pe = rest[ps..].find(')').ok_or_else(|| DatasetError::Format("no ')'".into()))?;
        let inner = rest[ps+1..ps+pe].trim();
        if inner.is_empty() { return Ok(vec![]); }
        inner.split(',').map(|s| s.trim()).filter(|s| !s.is_empty())
            .map(|s| s.parse::<usize>().map_err(|_| DatasetError::Format(format!("bad dim: '{s}'"))))
            .collect()
    }
}

// ── MatReader ────────────────────────────────────────────────────────────────

/// Minimal MATLAB .mat v5 reader for numeric arrays.
pub struct MatReader;

const MI_INT8: u32 = 1;
#[allow(dead_code)] const MI_UINT8: u32 = 2;
#[allow(dead_code)] const MI_INT16: u32 = 3;
#[allow(dead_code)] const MI_UINT16: u32 = 4;
const MI_INT32: u32 = 5;
const MI_UINT32: u32 = 6;
const MI_SINGLE: u32 = 7;
const MI_DOUBLE: u32 = 9;
const MI_MATRIX: u32 = 14;

impl MatReader {
    pub fn read_file(path: &Path) -> Result<HashMap<String, NpyArray>> {
        Self::parse(&std::fs::read(path)?)
    }

    pub fn parse(buf: &[u8]) -> Result<HashMap<String, NpyArray>> {
        if buf.len() < 128 { return Err(DatasetError::Format("too small for .mat v5".into())); }
        let swap = u16::from_le_bytes([buf[126], buf[127]]) == 0x4D49;
        let mut result = HashMap::new();
        let mut off = 128;
        while off + 8 <= buf.len() {
            let (dt, ds, ts) = Self::read_tag(buf, off, swap)?;
            let el_start = off + ts;
            let el_end = el_start + ds;
            if el_end > buf.len() { break; }
            if dt == MI_MATRIX {
                if let Ok((n, a)) = Self::parse_matrix(&buf[el_start..el_end], swap) {
                    result.insert(n, a);
                }
            }
            off = (el_end + 7) & !7;
        }
        Ok(result)
    }

    fn read_tag(buf: &[u8], off: usize, swap: bool) -> Result<(u32, usize, usize)> {
        if off + 4 > buf.len() { return Err(DatasetError::Format("truncated tag".into())); }
        let raw = Self::u32(buf, off, swap);
        let upper = (raw >> 16) & 0xFFFF;
        if upper != 0 && upper <= 4 { return Ok((raw & 0xFFFF, upper as usize, 4)); }
        if off + 8 > buf.len() { return Err(DatasetError::Format("truncated tag".into())); }
        Ok((raw, Self::u32(buf, off + 4, swap) as usize, 8))
    }

    fn parse_matrix(buf: &[u8], swap: bool) -> Result<(String, NpyArray)> {
        let (mut name, mut shape, mut data) = (String::new(), Vec::new(), Vec::new());
        let mut off = 0;
        while off + 4 <= buf.len() {
            let (st, ss, ts) = Self::read_tag(buf, off, swap)?;
            let ss_start = off + ts;
            let ss_end = (ss_start + ss).min(buf.len());
            match st {
                MI_UINT32 if shape.is_empty() && ss == 8 => {}
                MI_INT32 if shape.is_empty() => {
                    for i in 0..ss / 4 { shape.push(Self::i32(buf, ss_start + i*4, swap) as usize); }
                }
                MI_INT8 if name.is_empty() && ss_end <= buf.len() => {
                    name = String::from_utf8_lossy(&buf[ss_start..ss_end])
                        .trim_end_matches('\0').to_string();
                }
                MI_DOUBLE => {
                    for i in 0..ss / 8 {
                        let p = ss_start + i * 8;
                        if p + 8 <= buf.len() { data.push(Self::f64(buf, p, swap) as f32); }
                    }
                }
                MI_SINGLE => {
                    for i in 0..ss / 4 {
                        let p = ss_start + i * 4;
                        if p + 4 <= buf.len() { data.push(Self::f32(buf, p, swap)); }
                    }
                }
                _ => {}
            }
            off = (ss_end + 7) & !7;
        }
        if name.is_empty() { name = "unnamed".into(); }
        if shape.is_empty() && !data.is_empty() { shape = vec![data.len()]; }
        // Transpose column-major to row-major for 2D
        if shape.len() == 2 {
            let (r, c) = (shape[0], shape[1]);
            if r * c == data.len() {
                let mut cd = vec![0.0f32; data.len()];
                for ri in 0..r { for ci in 0..c { cd[ri*c+ci] = data[ci*r+ri]; } }
                data = cd;
            }
        }
        Ok((name, NpyArray { shape, data }))
    }

    fn u32(b: &[u8], o: usize, s: bool) -> u32 {
        let v = [b[o], b[o+1], b[o+2], b[o+3]];
        if s { u32::from_be_bytes(v) } else { u32::from_le_bytes(v) }
    }
    fn i32(b: &[u8], o: usize, s: bool) -> i32 {
        let v = [b[o], b[o+1], b[o+2], b[o+3]];
        if s { i32::from_be_bytes(v) } else { i32::from_le_bytes(v) }
    }
    fn f64(b: &[u8], o: usize, s: bool) -> f64 {
        let v: [u8; 8] = b[o..o+8].try_into().unwrap();
        if s { f64::from_be_bytes(v) } else { f64::from_le_bytes(v) }
    }
    fn f32(b: &[u8], o: usize, s: bool) -> f32 {
        let v = [b[o], b[o+1], b[o+2], b[o+3]];
        if s { f32::from_be_bytes(v) } else { f32::from_le_bytes(v) }
    }
}

// ── Core data types ──────────────────────────────────────────────────────────

/// A single CSI (Channel State Information) sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsiSample {
    pub amplitude: Vec<f32>,
    pub phase: Vec<f32>,
    pub timestamp_ms: u64,
}

/// UV coordinate map for a body part in DensePose representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyPartUV {
    pub part_id: u8,
    pub u_coords: Vec<f32>,
    pub v_coords: Vec<f32>,
}

/// Pose label: 17 COCO keypoints + optional DensePose body-part UVs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseLabel {
    pub keypoints: [(f32, f32, f32); 17],
    pub body_parts: Vec<BodyPartUV>,
    pub confidence: f32,
}

impl Default for PoseLabel {
    fn default() -> Self {
        Self { keypoints: [(0.0, 0.0, 0.0); 17], body_parts: Vec::new(), confidence: 0.0 }
    }
}

// ── SubcarrierResampler ──────────────────────────────────────────────────────

/// Resamples subcarrier data via linear interpolation or zero-padding.
pub struct SubcarrierResampler;

impl SubcarrierResampler {
    /// Resample: passthrough if equal, zero-pad if upsampling, interpolate if downsampling.
    pub fn resample(input: &[f32], from: usize, to: usize) -> Vec<f32> {
        if from == to || from == 0 || to == 0 { return input.to_vec(); }
        if from < to { Self::zero_pad(input, from, to) } else { Self::interpolate(input, from, to) }
    }

    /// Resample phase data with unwrapping before interpolation.
    pub fn resample_phase(input: &[f32], from: usize, to: usize) -> Vec<f32> {
        if from == to || from == 0 || to == 0 { return input.to_vec(); }
        let unwrapped = Self::phase_unwrap(input);
        let resampled = if from < to { Self::zero_pad(&unwrapped, from, to) }
                        else { Self::interpolate(&unwrapped, from, to) };
        let pi = std::f32::consts::PI;
        resampled.iter().map(|&p| {
            let mut w = p % (2.0 * pi);
            if w > pi { w -= 2.0 * pi; }
            if w < -pi { w += 2.0 * pi; }
            w
        }).collect()
    }

    fn zero_pad(input: &[f32], from: usize, to: usize) -> Vec<f32> {
        let pad_left = (to - from) / 2;
        let mut out = vec![0.0f32; to];
        for i in 0..from.min(input.len()) {
            if pad_left + i < to { out[pad_left + i] = input[i]; }
        }
        out
    }

    fn interpolate(input: &[f32], from: usize, to: usize) -> Vec<f32> {
        let n = input.len().min(from);
        if n <= 1 { return vec![input.first().copied().unwrap_or(0.0); to]; }
        (0..to).map(|i| {
            let pos = i as f64 * (n - 1) as f64 / (to - 1).max(1) as f64;
            let lo = pos.floor() as usize;
            let hi = (lo + 1).min(n - 1);
            let f = (pos - lo as f64) as f32;
            input[lo] * (1.0 - f) + input[hi] * f
        }).collect()
    }

    fn phase_unwrap(phase: &[f32]) -> Vec<f32> {
        let pi = std::f32::consts::PI;
        let mut out = vec![0.0f32; phase.len()];
        if phase.is_empty() { return out; }
        out[0] = phase[0];
        for i in 1..phase.len() {
            let mut d = phase[i] - phase[i - 1];
            while d > pi { d -= 2.0 * pi; }
            while d < -pi { d += 2.0 * pi; }
            out[i] = out[i - 1] + d;
        }
        out
    }
}

// ── MmFiDataset ──────────────────────────────────────────────────────────────

/// MM-Fi (NeurIPS 2023) dataset loader with 56 subcarriers and 17 COCO keypoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MmFiDataset {
    pub csi_frames: Vec<CsiSample>,
    pub labels: Vec<PoseLabel>,
    pub sample_rate_hz: f32,
    pub n_subcarriers: usize,
}

impl MmFiDataset {
    pub const SUBCARRIERS: usize = 56;

    /// Load from directory with csi_amplitude.npy/csi.npy and labels.npy/keypoints.npy.
    pub fn load_from_directory(path: &Path) -> Result<Self> {
        if !path.is_dir() {
            return Err(DatasetError::Missing(format!("directory not found: {}", path.display())));
        }
        let amp = NpyReader::read_file(&Self::find(path, &["csi_amplitude.npy", "csi.npy"])?)?;
        let n = amp.shape.first().copied().unwrap_or(0);
        let raw_sc = if amp.shape.len() >= 2 { amp.shape[1] } else { amp.data.len() / n.max(1) };
        let phase_arr = Self::find(path, &["csi_phase.npy"]).ok()
            .and_then(|p| NpyReader::read_file(&p).ok());
        let lab = NpyReader::read_file(&Self::find(path, &["labels.npy", "keypoints.npy"])?)?;

        let mut csi_frames = Vec::with_capacity(n);
        let mut labels = Vec::with_capacity(n);
        for i in 0..n {
            let s = i * raw_sc;
            if s + raw_sc > amp.data.len() { break; }
            let amplitude = SubcarrierResampler::resample(&amp.data[s..s+raw_sc], raw_sc, Self::SUBCARRIERS);
            let phase = phase_arr.as_ref().map(|pa| {
                let ps = i * raw_sc;
                if ps + raw_sc <= pa.data.len() {
                    SubcarrierResampler::resample_phase(&pa.data[ps..ps+raw_sc], raw_sc, Self::SUBCARRIERS)
                } else { vec![0.0; Self::SUBCARRIERS] }
            }).unwrap_or_else(|| vec![0.0; Self::SUBCARRIERS]);

            csi_frames.push(CsiSample { amplitude, phase, timestamp_ms: i as u64 * 50 });

            let ks = i * 17 * 3;
            let label = if ks + 51 <= lab.data.len() {
                let d = &lab.data[ks..ks + 51];
                let mut kp = [(0.0f32, 0.0, 0.0); 17];
                for k in 0..17 { kp[k] = (d[k*3], d[k*3+1], d[k*3+2]); }
                PoseLabel { keypoints: kp, body_parts: Vec::new(), confidence: 1.0 }
            } else { PoseLabel::default() };
            labels.push(label);
        }
        Ok(Self { csi_frames, labels, sample_rate_hz: 20.0, n_subcarriers: Self::SUBCARRIERS })
    }

    pub fn resample_subcarriers(&mut self, from: usize, to: usize) {
        for f in &mut self.csi_frames {
            f.amplitude = SubcarrierResampler::resample(&f.amplitude, from, to);
            f.phase = SubcarrierResampler::resample_phase(&f.phase, from, to);
        }
        self.n_subcarriers = to;
    }

    pub fn iter_windows(&self, ws: usize, stride: usize) -> impl Iterator<Item = (&[CsiSample], &[PoseLabel])> {
        let stride = stride.max(1);
        let n = self.csi_frames.len();
        (0..n).step_by(stride).filter(move |&s| s + ws <= n)
            .map(move |s| (&self.csi_frames[s..s+ws], &self.labels[s..s+ws]))
    }

    pub fn split_train_val(self, ratio: f32) -> (Self, Self) {
        let split = (self.csi_frames.len() as f32 * ratio.clamp(0.0, 1.0)) as usize;
        let (tc, vc) = self.csi_frames.split_at(split);
        let (tl, vl) = self.labels.split_at(split);
        let mk = |c: &[CsiSample], l: &[PoseLabel]| Self {
            csi_frames: c.to_vec(), labels: l.to_vec(),
            sample_rate_hz: self.sample_rate_hz, n_subcarriers: self.n_subcarriers,
        };
        (mk(tc, tl), mk(vc, vl))
    }

    pub fn len(&self) -> usize { self.csi_frames.len() }
    pub fn is_empty(&self) -> bool { self.csi_frames.is_empty() }
    pub fn get(&self, idx: usize) -> Option<(&CsiSample, &PoseLabel)> {
        self.csi_frames.get(idx).zip(self.labels.get(idx))
    }

    fn find(dir: &Path, names: &[&str]) -> Result<PathBuf> {
        for n in names { let p = dir.join(n); if p.exists() { return Ok(p); } }
        Err(DatasetError::Missing(format!("none of {names:?} in {}", dir.display())))
    }
}

// ── WiPoseDataset ────────────────────────────────────────────────────────────

/// Wi-Pose dataset loader: .mat v5, 30 subcarriers (-> 56), 18 keypoints (-> 17 COCO).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WiPoseDataset {
    pub csi_frames: Vec<CsiSample>,
    pub labels: Vec<PoseLabel>,
    pub sample_rate_hz: f32,
    pub n_subcarriers: usize,
}

impl WiPoseDataset {
    pub const RAW_SUBCARRIERS: usize = 30;
    pub const TARGET_SUBCARRIERS: usize = 56;
    pub const RAW_KEYPOINTS: usize = 18;
    pub const COCO_KEYPOINTS: usize = 17;

    pub fn load_from_mat(path: &Path) -> Result<Self> {
        let arrays = MatReader::read_file(path)?;
        let csi = arrays.get("csi").or_else(|| arrays.get("csi_data")).or_else(|| arrays.get("CSI"))
            .ok_or_else(|| DatasetError::Missing("no CSI variable in .mat".into()))?;
        let n = csi.shape.first().copied().unwrap_or(0);
        let raw = if csi.shape.len() >= 2 { csi.shape[1] } else { Self::RAW_SUBCARRIERS };
        let lab = arrays.get("keypoints").or_else(|| arrays.get("labels")).or_else(|| arrays.get("pose"));

        let mut csi_frames = Vec::with_capacity(n);
        let mut labels = Vec::with_capacity(n);
        for i in 0..n {
            let s = i * raw;
            if s + raw > csi.data.len() { break; }
            let amp = SubcarrierResampler::resample(&csi.data[s..s+raw], raw, Self::TARGET_SUBCARRIERS);
            csi_frames.push(CsiSample { amplitude: amp, phase: vec![0.0; Self::TARGET_SUBCARRIERS], timestamp_ms: i as u64 * 100 });
            let label = lab.and_then(|la| {
                let ks = i * Self::RAW_KEYPOINTS * 3;
                if ks + Self::RAW_KEYPOINTS * 3 <= la.data.len() {
                    Some(Self::map_18_to_17(&la.data[ks..ks + Self::RAW_KEYPOINTS * 3]))
                } else { None }
            }).unwrap_or_default();
            labels.push(label);
        }
        Ok(Self { csi_frames, labels, sample_rate_hz: 10.0, n_subcarriers: Self::TARGET_SUBCARRIERS })
    }

    /// Map 18 keypoints to 17 COCO: keep index 0 (nose), drop index 1, map 2..18 -> 1..16.
    fn map_18_to_17(data: &[f32]) -> PoseLabel {
        let mut kp = [(0.0f32, 0.0, 0.0); 17];
        if data.len() >= 18 * 3 {
            kp[0] = (data[0], data[1], data[2]);
            for i in 1..17 { let s = (i + 1) * 3; kp[i] = (data[s], data[s+1], data[s+2]); }
        }
        PoseLabel { keypoints: kp, body_parts: Vec::new(), confidence: 1.0 }
    }

    pub fn len(&self) -> usize { self.csi_frames.len() }
    pub fn is_empty(&self) -> bool { self.csi_frames.is_empty() }
}

// ── DataPipeline ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataSource {
    MmFi(PathBuf),
    WiPose(PathBuf),
    Combined(Vec<DataSource>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataConfig {
    pub source: DataSource,
    pub window_size: usize,
    pub stride: usize,
    pub target_subcarriers: usize,
    pub normalize: bool,
}

impl Default for DataConfig {
    fn default() -> Self {
        Self { source: DataSource::Combined(Vec::new()), window_size: 10, stride: 5,
               target_subcarriers: 56, normalize: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingSample {
    pub csi_window: Vec<Vec<f32>>,
    pub pose_label: PoseLabel,
    pub source: &'static str,
}

/// Unified pipeline: loads, resamples, windows, and normalizes training data.
pub struct DataPipeline { config: DataConfig }

impl DataPipeline {
    pub fn new(config: DataConfig) -> Self { Self { config } }

    pub fn load(&self) -> Result<Vec<TrainingSample>> {
        let mut out = Vec::new();
        self.load_source(&self.config.source, &mut out)?;
        if self.config.normalize && !out.is_empty() { Self::normalize_samples(&mut out); }
        Ok(out)
    }

    fn load_source(&self, src: &DataSource, out: &mut Vec<TrainingSample>) -> Result<()> {
        match src {
            DataSource::MmFi(p) => {
                let mut ds = MmFiDataset::load_from_directory(p)?;
                if ds.n_subcarriers != self.config.target_subcarriers {
                    let f = ds.n_subcarriers;
                    ds.resample_subcarriers(f, self.config.target_subcarriers);
                }
                self.extract_windows(&ds.csi_frames, &ds.labels, "mmfi", out);
            }
            DataSource::WiPose(p) => {
                let ds = WiPoseDataset::load_from_mat(p)?;
                self.extract_windows(&ds.csi_frames, &ds.labels, "wipose", out);
            }
            DataSource::Combined(srcs) => { for s in srcs { self.load_source(s, out)?; } }
        }
        Ok(())
    }

    fn extract_windows(&self, frames: &[CsiSample], labels: &[PoseLabel],
                        source: &'static str, out: &mut Vec<TrainingSample>) {
        let (ws, stride) = (self.config.window_size, self.config.stride.max(1));
        let mut s = 0;
        while s + ws <= frames.len() {
            let window: Vec<Vec<f32>> = frames[s..s+ws].iter().map(|f| f.amplitude.clone()).collect();
            let label = labels.get(s + ws / 2).cloned().unwrap_or_default();
            out.push(TrainingSample { csi_window: window, pose_label: label, source });
            s += stride;
        }
    }

    fn normalize_samples(samples: &mut [TrainingSample]) {
        let ns = samples.first().and_then(|s| s.csi_window.first()).map(|f| f.len()).unwrap_or(0);
        if ns == 0 { return; }
        let (mut sum, mut sq) = (vec![0.0f64; ns], vec![0.0f64; ns]);
        let mut cnt = 0u64;
        for s in samples.iter() {
            for f in &s.csi_window {
                for (j, &v) in f.iter().enumerate().take(ns) {
                    let v = v as f64; sum[j] += v; sq[j] += v * v;
                }
                cnt += 1;
            }
        }
        if cnt == 0 { return; }
        let mean: Vec<f64> = sum.iter().map(|s| s / cnt as f64).collect();
        let std: Vec<f64> = sq.iter().zip(mean.iter())
            .map(|(&s, &m)| (s / cnt as f64 - m * m).max(0.0).sqrt().max(1e-8)).collect();
        for s in samples.iter_mut() {
            for f in &mut s.csi_window {
                for (j, v) in f.iter_mut().enumerate().take(ns) {
                    *v = ((*v as f64 - mean[j]) / std[j]) as f32;
                }
            }
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_npy_f32(shape: &[usize], data: &[f32]) -> Vec<u8> {
        let ss = if shape.len() == 1 { format!("({},)", shape[0]) }
                 else { format!("({})", shape.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(", ")) };
        let hdr = format!("{{'descr': '<f4', 'fortran_order': False, 'shape': {ss}, }}");
        let total = 10 + hdr.len();
        let padded = ((total + 63) / 64) * 64;
        let hl = padded - 10;
        let mut buf = Vec::new();
        buf.extend_from_slice(b"\x93NUMPY\x01\x00");
        buf.extend_from_slice(&(hl as u16).to_le_bytes());
        buf.extend_from_slice(hdr.as_bytes());
        buf.resize(10 + hl, b' ');
        for &v in data { buf.extend_from_slice(&v.to_le_bytes()); }
        buf
    }

    fn make_npy_f64(shape: &[usize], data: &[f64]) -> Vec<u8> {
        let ss = if shape.len() == 1 { format!("({},)", shape[0]) }
                 else { format!("({})", shape.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(", ")) };
        let hdr = format!("{{'descr': '<f8', 'fortran_order': False, 'shape': {ss}, }}");
        let total = 10 + hdr.len();
        let padded = ((total + 63) / 64) * 64;
        let hl = padded - 10;
        let mut buf = Vec::new();
        buf.extend_from_slice(b"\x93NUMPY\x01\x00");
        buf.extend_from_slice(&(hl as u16).to_le_bytes());
        buf.extend_from_slice(hdr.as_bytes());
        buf.resize(10 + hl, b' ');
        for &v in data { buf.extend_from_slice(&v.to_le_bytes()); }
        buf
    }

    #[test]
    fn npy_header_parse_1d() {
        let buf = make_npy_f32(&[5], &[1.0, 2.0, 3.0, 4.0, 5.0]);
        let arr = NpyReader::parse(&buf).unwrap();
        assert_eq!(arr.shape, vec![5]);
        assert_eq!(arr.ndim(), 1);
        assert_eq!(arr.len(), 5);
        assert!((arr.data[0] - 1.0).abs() < f32::EPSILON);
        assert!((arr.data[4] - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn npy_header_parse_2d() {
        let data: Vec<f32> = (0..12).map(|i| i as f32).collect();
        let buf = make_npy_f32(&[3, 4], &data);
        let arr = NpyReader::parse(&buf).unwrap();
        assert_eq!(arr.shape, vec![3, 4]);
        assert_eq!(arr.ndim(), 2);
        assert_eq!(arr.len(), 12);
    }

    #[test]
    fn npy_header_parse_3d() {
        let data: Vec<f64> = (0..24).map(|i| i as f64 * 0.5).collect();
        let buf = make_npy_f64(&[2, 3, 4], &data);
        let arr = NpyReader::parse(&buf).unwrap();
        assert_eq!(arr.shape, vec![2, 3, 4]);
        assert_eq!(arr.ndim(), 3);
        assert_eq!(arr.len(), 24);
        assert!((arr.data[23] - 11.5).abs() < 1e-5);
    }

    #[test]
    fn subcarrier_resample_passthrough() {
        let input: Vec<f32> = (0..56).map(|i| i as f32).collect();
        let output = SubcarrierResampler::resample(&input, 56, 56);
        assert_eq!(output, input);
    }

    #[test]
    fn subcarrier_resample_upsample() {
        let input: Vec<f32> = (0..30).map(|i| (i + 1) as f32).collect();
        let out = SubcarrierResampler::resample(&input, 30, 56);
        assert_eq!(out.len(), 56);
        // pad_left = 13, leading zeros
        for i in 0..13 { assert!(out[i].abs() < f32::EPSILON, "expected zero at {i}"); }
        // original data in middle
        for i in 0..30 { assert!((out[13+i] - input[i]).abs() < f32::EPSILON); }
        // trailing zeros
        for i in 43..56 { assert!(out[i].abs() < f32::EPSILON, "expected zero at {i}"); }
    }

    #[test]
    fn subcarrier_resample_downsample() {
        let input: Vec<f32> = (0..114).map(|i| i as f32).collect();
        let out = SubcarrierResampler::resample(&input, 114, 56);
        assert_eq!(out.len(), 56);
        assert!((out[0]).abs() < f32::EPSILON);
        assert!((out[55] - 113.0).abs() < 0.1);
        for i in 1..56 { assert!(out[i] >= out[i-1], "not monotonic at {i}"); }
    }

    #[test]
    fn subcarrier_resample_preserves_dc() {
        let out = SubcarrierResampler::resample(&vec![42.0f32; 114], 114, 56);
        assert_eq!(out.len(), 56);
        for (i, &v) in out.iter().enumerate() {
            assert!((v - 42.0).abs() < 1e-5, "DC not preserved at {i}: {v}");
        }
    }

    #[test]
    fn mmfi_sample_structure() {
        let s = CsiSample { amplitude: vec![0.0; 56], phase: vec![0.0; 56], timestamp_ms: 100 };
        assert_eq!(s.amplitude.len(), 56);
        assert_eq!(s.phase.len(), 56);
    }

    #[test]
    fn wipose_zero_pad() {
        let raw: Vec<f32> = (1..=30).map(|i| i as f32).collect();
        let p = SubcarrierResampler::resample(&raw, 30, 56);
        assert_eq!(p.len(), 56);
        assert!(p[0].abs() < f32::EPSILON);
        assert!((p[13] - 1.0).abs() < f32::EPSILON);
        assert!((p[42] - 30.0).abs() < f32::EPSILON);
        assert!(p[55].abs() < f32::EPSILON);
    }

    #[test]
    fn wipose_keypoint_mapping() {
        let mut kp = vec![0.0f32; 18 * 3];
        kp[0] = 1.0; kp[1] = 2.0; kp[2] = 1.0; // nose
        kp[3] = 99.0; kp[4] = 99.0; kp[5] = 99.0; // extra (dropped)
        kp[6] = 3.0; kp[7] = 4.0; kp[8] = 1.0; // left eye -> COCO 1
        let label = WiPoseDataset::map_18_to_17(&kp);
        assert_eq!(label.keypoints.len(), 17);
        assert!((label.keypoints[0].0 - 1.0).abs() < f32::EPSILON);
        assert!((label.keypoints[1].0 - 3.0).abs() < f32::EPSILON); // not 99
    }

    #[test]
    fn train_val_split_ratio() {
        let mk = |n: usize| MmFiDataset {
            csi_frames: (0..n).map(|i| CsiSample { amplitude: vec![i as f32; 56], phase: vec![0.0; 56], timestamp_ms: i as u64 }).collect(),
            labels: (0..n).map(|_| PoseLabel::default()).collect(),
            sample_rate_hz: 20.0, n_subcarriers: 56,
        };
        let (train, val) = mk(100).split_train_val(0.8);
        assert_eq!(train.len(), 80);
        assert_eq!(val.len(), 20);
        assert_eq!(train.len() + val.len(), 100);
    }

    #[test]
    fn sliding_window_count() {
        let ds = MmFiDataset {
            csi_frames: (0..20).map(|i| CsiSample { amplitude: vec![i as f32; 56], phase: vec![0.0; 56], timestamp_ms: i as u64 }).collect(),
            labels: (0..20).map(|_| PoseLabel::default()).collect(),
            sample_rate_hz: 20.0, n_subcarriers: 56,
        };
        assert_eq!(ds.iter_windows(5, 5).count(), 4);
        assert_eq!(ds.iter_windows(5, 1).count(), 16);
    }

    #[test]
    fn sliding_window_overlap() {
        let ds = MmFiDataset {
            csi_frames: (0..10).map(|i| CsiSample { amplitude: vec![i as f32; 56], phase: vec![0.0; 56], timestamp_ms: i as u64 }).collect(),
            labels: (0..10).map(|_| PoseLabel::default()).collect(),
            sample_rate_hz: 20.0, n_subcarriers: 56,
        };
        let w: Vec<_> = ds.iter_windows(4, 2).collect();
        assert_eq!(w.len(), 4);
        assert!((w[0].0[0].amplitude[0]).abs() < f32::EPSILON);
        assert!((w[1].0[0].amplitude[0] - 2.0).abs() < f32::EPSILON);
        assert_eq!(w[0].0[2].amplitude[0], w[1].0[0].amplitude[0]); // overlap
    }

    #[test]
    fn data_pipeline_normalize() {
        let mut samples = vec![
            TrainingSample { csi_window: vec![vec![10.0, 20.0, 30.0]; 2], pose_label: PoseLabel::default(), source: "test" },
            TrainingSample { csi_window: vec![vec![30.0, 40.0, 50.0]; 2], pose_label: PoseLabel::default(), source: "test" },
        ];
        DataPipeline::normalize_samples(&mut samples);
        for j in 0..3 {
            let (mut s, mut c) = (0.0f64, 0u64);
            for sam in &samples { for f in &sam.csi_window { s += f[j] as f64; c += 1; } }
            assert!(( s / c as f64).abs() < 1e-5, "mean not ~0 for sub {j}");
            let mut vs = 0.0f64;
            let m = s / c as f64;
            for sam in &samples { for f in &sam.csi_window { vs += (f[j] as f64 - m).powi(2); } }
            assert!(((vs / c as f64).sqrt() - 1.0).abs() < 0.1, "std not ~1 for sub {j}");
        }
    }

    #[test]
    fn pose_label_default() {
        let l = PoseLabel::default();
        assert_eq!(l.keypoints.len(), 17);
        assert!(l.body_parts.is_empty());
        assert!(l.confidence.abs() < f32::EPSILON);
        for (i, kp) in l.keypoints.iter().enumerate() {
            assert!(kp.0.abs() < f32::EPSILON && kp.1.abs() < f32::EPSILON, "kp {i} not zero");
        }
    }

    #[test]
    fn body_part_uv_round_trip() {
        let bpu = BodyPartUV { part_id: 5, u_coords: vec![0.1, 0.2, 0.3], v_coords: vec![0.4, 0.5, 0.6] };
        let json = serde_json::to_string(&bpu).unwrap();
        let r: BodyPartUV = serde_json::from_str(&json).unwrap();
        assert_eq!(r.part_id, 5);
        assert_eq!(r.u_coords.len(), 3);
        assert!((r.u_coords[0] - 0.1).abs() < f32::EPSILON);
        assert!((r.v_coords[2] - 0.6).abs() < f32::EPSILON);
    }

    #[test]
    fn combined_source_merges_datasets() {
        let mk = |n: usize, base: f32| -> (Vec<CsiSample>, Vec<PoseLabel>) {
            let f: Vec<CsiSample> = (0..n).map(|i| CsiSample { amplitude: vec![base + i as f32; 56], phase: vec![0.0; 56], timestamp_ms: i as u64 * 50 }).collect();
            let l: Vec<PoseLabel> = (0..n).map(|_| PoseLabel::default()).collect();
            (f, l)
        };
        let pipe = DataPipeline::new(DataConfig { source: DataSource::Combined(Vec::new()),
            window_size: 3, stride: 1, target_subcarriers: 56, normalize: false });
        let mut all = Vec::new();
        let (fa, la) = mk(5, 0.0);
        pipe.extract_windows(&fa, &la, "mmfi", &mut all);
        assert_eq!(all.len(), 3);
        let (fb, lb) = mk(4, 100.0);
        pipe.extract_windows(&fb, &lb, "wipose", &mut all);
        assert_eq!(all.len(), 5);
        assert_eq!(all[0].source, "mmfi");
        assert_eq!(all[3].source, "wipose");
        assert!(all[0].csi_window[0][0] < 10.0);
        assert!(all[4].csi_window[0][0] > 90.0);
    }
}
