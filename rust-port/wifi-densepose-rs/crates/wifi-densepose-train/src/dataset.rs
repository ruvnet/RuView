//! Dataset abstractions and concrete implementations for WiFi-DensePose training.
//!
//! This module defines the [`CsiDataset`] trait plus two concrete implementations:
//!
//! - [`MmFiDataset`]: reads MM-Fi NPY/HDF5 files from disk.
//! - [`SyntheticCsiDataset`]: generates fully-deterministic CSI from a physics
//!   model; useful for unit tests, integration tests, and dry-run sanity checks.
//!   **Never uses random data.**
//!
//! A [`DataLoader`] wraps any [`CsiDataset`] and provides batched iteration with
//! optional deterministic shuffle (seeded).
//!
//! # Directory layout expected by `MmFiDataset`
//!
//! ```text
//! <root>/
//!   S01/
//!     A01/
//!       wifi_csi.npy          # amplitude  [T, n_tx, n_rx, n_sc]
//!       wifi_csi_phase.npy    # phase       [T, n_tx, n_rx, n_sc]
//!       gt_keypoints.npy      # keypoints   [T, 17, 3]  (x, y, vis)
//!     A02/
//!       ...
//!   S02/
//!     ...
//! ```
//!
//! Each subject/action pair produces one or more windowed [`CsiSample`]s.
//!
//! # Example – synthetic dataset
//!
//! ```rust
//! use wifi_densepose_train::dataset::{SyntheticCsiDataset, SyntheticConfig, CsiDataset};
//!
//! let cfg = SyntheticConfig::default();
//! let ds = SyntheticCsiDataset::new(64, cfg);
//!
//! assert_eq!(ds.len(), 64);
//! let sample = ds.get(0).unwrap();
//! assert_eq!(sample.amplitude.shape(), &[100, 3, 3, 56]);
//! ```

use ndarray::{Array1, Array2, Array4};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info, warn};

use crate::subcarrier::interpolate_subcarriers;

// ---------------------------------------------------------------------------
// CsiSample
// ---------------------------------------------------------------------------

/// A single windowed CSI observation paired with its ground-truth labels.
///
/// All arrays are stored in row-major (C) order. Keypoint coordinates are
/// normalised to `[0, 1]` with the origin at the **top-left** corner.
#[derive(Debug, Clone)]
pub struct CsiSample {
    /// CSI amplitude tensor.
    ///
    /// Shape: `[window_frames, n_tx, n_rx, n_subcarriers]`.
    pub amplitude: Array4<f32>,

    /// CSI phase tensor (radians, unwrapped).
    ///
    /// Shape: `[window_frames, n_tx, n_rx, n_subcarriers]`.
    pub phase: Array4<f32>,

    /// COCO 17-keypoint positions normalised to `[0, 1]`.
    ///
    /// Shape: `[17, 2]` – column 0 is x, column 1 is y.
    pub keypoints: Array2<f32>,

    /// Keypoint visibility flags.
    ///
    /// Shape: `[17]`. Values follow the COCO convention:
    /// - `0` – not labelled
    /// - `1` – labelled but not visible
    /// - `2` – visible
    pub keypoint_visibility: Array1<f32>,

    /// Subject identifier (e.g. 1 for `S01`).
    pub subject_id: u32,

    /// Action identifier (e.g. 1 for `A01`).
    pub action_id: u32,

    /// Absolute frame index within the original recording.
    pub frame_id: u64,
}

// ---------------------------------------------------------------------------
// CsiDataset trait
// ---------------------------------------------------------------------------

/// Common interface for all WiFi-DensePose datasets.
///
/// Implementations must be `Send + Sync` so they can be shared across
/// data-loading threads without additional synchronisation.
pub trait CsiDataset: Send + Sync {
    /// Total number of samples in this dataset.
    fn len(&self) -> usize;

    /// Load the sample at position `idx`.
    ///
    /// # Errors
    ///
    /// Returns [`DatasetError::IndexOutOfBounds`] when `idx >= self.len()` and
    /// dataset-specific errors for IO or format problems.
    fn get(&self, idx: usize) -> Result<CsiSample, DatasetError>;

    /// Returns `true` when the dataset contains no samples.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Human-readable name for logging and progress display.
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// DataLoader
// ---------------------------------------------------------------------------

/// Batched, optionally-shuffled iterator over a [`CsiDataset`].
///
/// The shuffle order is fully deterministic: given the same `seed` and dataset
/// length the iteration order is always identical. This ensures reproducibility
/// across training runs.
pub struct DataLoader<'a> {
    dataset: &'a dyn CsiDataset,
    batch_size: usize,
    shuffle: bool,
    seed: u64,
}

impl<'a> DataLoader<'a> {
    /// Create a new `DataLoader`.
    ///
    /// # Parameters
    ///
    /// - `dataset`    – the underlying dataset.
    /// - `batch_size` – number of samples per batch. The last batch may be
    ///   smaller if the dataset length is not a multiple of `batch_size`.
    /// - `shuffle`    – if `true`, samples are shuffled deterministically using
    ///   `seed` at the start of each iteration.
    /// - `seed`       – fixed seed for the shuffle RNG.
    pub fn new(
        dataset: &'a dyn CsiDataset,
        batch_size: usize,
        shuffle: bool,
        seed: u64,
    ) -> Self {
        assert!(batch_size > 0, "batch_size must be > 0");
        DataLoader { dataset, batch_size, shuffle, seed }
    }

    /// Number of complete (or partial) batches yielded per epoch.
    pub fn num_batches(&self) -> usize {
        let n = self.dataset.len();
        if n == 0 {
            return 0;
        }
        (n + self.batch_size - 1) / self.batch_size
    }

    /// Return an iterator that yields `Vec<CsiSample>` batches.
    ///
    /// Failed individual sample loads are skipped with a `warn!` log rather
    /// than aborting the iterator.
    pub fn iter(&self) -> DataLoaderIter<'_> {
        // Build the index permutation once per epoch using a seeded Xorshift64.
        let n = self.dataset.len();
        let mut indices: Vec<usize> = (0..n).collect();
        if self.shuffle {
            xorshift_shuffle(&mut indices, self.seed);
        }
        DataLoaderIter {
            dataset: self.dataset,
            indices,
            batch_size: self.batch_size,
            cursor: 0,
        }
    }
}

/// Iterator returned by [`DataLoader::iter`].
pub struct DataLoaderIter<'a> {
    dataset: &'a dyn CsiDataset,
    indices: Vec<usize>,
    batch_size: usize,
    cursor: usize,
}

impl<'a> Iterator for DataLoaderIter<'a> {
    type Item = Vec<CsiSample>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.indices.len() {
            return None;
        }
        let end = (self.cursor + self.batch_size).min(self.indices.len());
        let batch_indices = &self.indices[self.cursor..end];
        self.cursor = end;

        let mut batch = Vec::with_capacity(batch_indices.len());
        for &idx in batch_indices {
            match self.dataset.get(idx) {
                Ok(sample) => batch.push(sample),
                Err(e) => {
                    warn!("Skipping sample {idx}: {e}");
                }
            }
        }
        if batch.is_empty() { None } else { Some(batch) }
    }
}

// ---------------------------------------------------------------------------
// Xorshift shuffle (deterministic, no external RNG state)
// ---------------------------------------------------------------------------

/// In-place Fisher-Yates shuffle using a 64-bit Xorshift PRNG seeded with
/// `seed`. This is reproducible across platforms and requires no external crate
/// in production paths.
fn xorshift_shuffle(indices: &mut [usize], seed: u64) {
    let n = indices.len();
    if n <= 1 {
        return;
    }
    let mut state = if seed == 0 { 0x853c49e6748fea9b } else { seed };
    for i in (1..n).rev() {
        // Xorshift64
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = (state as usize) % (i + 1);
        indices.swap(i, j);
    }
}

// ---------------------------------------------------------------------------
// MmFiDataset
// ---------------------------------------------------------------------------

/// An indexed entry in the MM-Fi directory scan.
#[derive(Debug, Clone)]
struct MmFiEntry {
    subject_id: u32,
    action_id: u32,
    /// Path to `wifi_csi.npy` (amplitude).
    amp_path: PathBuf,
    /// Path to `wifi_csi_phase.npy`.
    phase_path: PathBuf,
    /// Path to `gt_keypoints.npy`.
    kp_path: PathBuf,
    /// Number of temporal frames available in this clip.
    num_frames: usize,
    /// Window size in frames (mirrors config).
    window_frames: usize,
    /// First global sample index that maps into this clip.
    global_start_idx: usize,
}

impl MmFiEntry {
    /// Number of non-overlapping windows this clip contributes.
    fn num_windows(&self) -> usize {
        if self.num_frames < self.window_frames {
            0
        } else {
            self.num_frames - self.window_frames + 1
        }
    }
}

/// Dataset adapter for MM-Fi recordings stored as `.npy` files.
///
/// Scanning is performed once at construction via [`MmFiDataset::discover`].
/// Individual samples are loaded lazily from disk on each [`CsiDataset::get`]
/// call.
///
/// ## Subcarrier interpolation
///
/// When the loaded amplitude/phase arrays contain a different number of
/// subcarriers than `target_subcarriers`, [`interpolate_subcarriers`] is
/// applied automatically before the sample is returned.
pub struct MmFiDataset {
    entries: Vec<MmFiEntry>,
    /// Cumulative window count per entry (prefix sum, length = entries.len() + 1).
    cumulative: Vec<usize>,
    window_frames: usize,
    target_subcarriers: usize,
    num_keypoints: usize,
    root: PathBuf,
}

impl MmFiDataset {
    /// Scan `root` for MM-Fi recordings and build a sample index.
    ///
    /// The scan walks `root/{S??}/{A??}/` directories and looks for:
    /// - `wifi_csi.npy`       – CSI amplitude
    /// - `wifi_csi_phase.npy` – CSI phase
    /// - `gt_keypoints.npy`   – ground-truth keypoints
    ///
    /// # Errors
    ///
    /// Returns [`DatasetError::DirectoryNotFound`] if `root` does not exist, or
    /// [`DatasetError::Io`] for any filesystem access failure.
    pub fn discover(
        root: &Path,
        window_frames: usize,
        target_subcarriers: usize,
        num_keypoints: usize,
    ) -> Result<Self, DatasetError> {
        if !root.exists() {
            return Err(DatasetError::DirectoryNotFound {
                path: root.display().to_string(),
            });
        }

        let mut entries: Vec<MmFiEntry> = Vec::new();
        let mut global_idx = 0usize;

        // Walk subject directories (S01, S02, …)
        let mut subject_dirs: Vec<PathBuf> = std::fs::read_dir(root)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| e.path())
            .collect();
        subject_dirs.sort();

        for subj_path in &subject_dirs {
            let subj_name = subj_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let subject_id = parse_id_suffix(subj_name).unwrap_or(0);

            // Walk action directories (A01, A02, …)
            let mut action_dirs: Vec<PathBuf> = std::fs::read_dir(subj_path)?
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| e.path())
                .collect();
            action_dirs.sort();

            for action_path in &action_dirs {
                let action_name =
                    action_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let action_id = parse_id_suffix(action_name).unwrap_or(0);

                let amp_path = action_path.join("wifi_csi.npy");
                let phase_path = action_path.join("wifi_csi_phase.npy");
                let kp_path = action_path.join("gt_keypoints.npy");

                if !amp_path.exists() || !kp_path.exists() {
                    debug!(
                        "Skipping {}: missing required files",
                        action_path.display()
                    );
                    continue;
                }

                // Peek at the amplitude shape to get the frame count.
                let num_frames = match peek_npy_first_dim(&amp_path) {
                    Ok(n) => n,
                    Err(e) => {
                        warn!("Cannot read shape from {}: {e}", amp_path.display());
                        continue;
                    }
                };

                let entry = MmFiEntry {
                    subject_id,
                    action_id,
                    amp_path,
                    phase_path,
                    kp_path,
                    num_frames,
                    window_frames,
                    global_start_idx: global_idx,
                };
                global_idx += entry.num_windows();
                entries.push(entry);
            }
        }

        info!(
            "MmFiDataset: scanned {} clips, {} total windows (root={})",
            entries.len(),
            global_idx,
            root.display()
        );

        // Build prefix-sum cumulative array
        let mut cumulative = vec![0usize; entries.len() + 1];
        for (i, e) in entries.iter().enumerate() {
            cumulative[i + 1] = cumulative[i] + e.num_windows();
        }

        Ok(MmFiDataset {
            entries,
            cumulative,
            window_frames,
            target_subcarriers,
            num_keypoints,
            root: root.to_path_buf(),
        })
    }

    /// Resolve a global sample index to `(entry_index, frame_offset)`.
    fn locate(&self, idx: usize) -> Option<(usize, usize)> {
        let total = self.cumulative.last().copied().unwrap_or(0);
        if idx >= total {
            return None;
        }
        // Binary search in the cumulative prefix sums.
        let entry_idx = self
            .cumulative
            .partition_point(|&c| c <= idx)
            .saturating_sub(1);
        let frame_offset = idx - self.cumulative[entry_idx];
        Some((entry_idx, frame_offset))
    }
}

impl CsiDataset for MmFiDataset {
    fn len(&self) -> usize {
        self.cumulative.last().copied().unwrap_or(0)
    }

    fn get(&self, idx: usize) -> Result<CsiSample, DatasetError> {
        let total = self.len();
        let (entry_idx, frame_offset) = self
            .locate(idx)
            .ok_or(DatasetError::IndexOutOfBounds { idx, len: total })?;

        let entry = &self.entries[entry_idx];
        let t_start = frame_offset;
        let t_end = t_start + self.window_frames;

        // Load amplitude
        let amp_full = load_npy_f32(&entry.amp_path)?;
        let (t, n_tx, n_rx, n_sc) = amp_full.dim();
        if t_end > t {
            return Err(DatasetError::Format(format!(
                "window [{t_start}, {t_end}) exceeds clip length {t} in {}",
                entry.amp_path.display()
            )));
        }
        let amp_window = amp_full
            .slice(ndarray::s![t_start..t_end, .., .., ..])
            .to_owned();

        // Load phase (optional – return zeros if the file is absent)
        let phase_window = if entry.phase_path.exists() {
            let phase_full = load_npy_f32(&entry.phase_path)?;
            phase_full
                .slice(ndarray::s![t_start..t_end, .., .., ..])
                .to_owned()
        } else {
            Array4::zeros((self.window_frames, n_tx, n_rx, n_sc))
        };

        // Subcarrier interpolation (if needed)
        let amplitude = if n_sc != self.target_subcarriers {
            interpolate_subcarriers(&amp_window, self.target_subcarriers)
        } else {
            amp_window
        };

        let phase = if phase_window.dim().3 != self.target_subcarriers {
            interpolate_subcarriers(&phase_window, self.target_subcarriers)
        } else {
            phase_window
        };

        // Load keypoints [T, 17, 3] — take the first frame of the window
        let kp_full = load_npy_kp(&entry.kp_path, self.num_keypoints)?;
        let kp_frame = kp_full
            .slice(ndarray::s![t_start, .., ..])
            .to_owned();

        // Split into (x,y) and visibility
        let keypoints = kp_frame.slice(ndarray::s![.., 0..2]).to_owned();
        let keypoint_visibility = kp_frame.column(2).to_owned();

        Ok(CsiSample {
            amplitude,
            phase,
            keypoints,
            keypoint_visibility,
            subject_id: entry.subject_id,
            action_id: entry.action_id,
            frame_id: t_start as u64,
        })
    }

    fn name(&self) -> &str {
        "MmFiDataset"
    }
}

// ---------------------------------------------------------------------------
// NPY helpers (no-HDF5 path; HDF5 path is feature-gated below)
// ---------------------------------------------------------------------------

/// Load a 4-D float32 NPY array from disk.
///
/// The NPY format is read using `ndarray_npy`.
fn load_npy_f32(path: &Path) -> Result<Array4<f32>, DatasetError> {
    use ndarray_npy::ReadNpyExt;
    let file = std::fs::File::open(path)?;
    let arr: ndarray::ArrayD<f32> = ndarray::ArrayD::read_npy(file)
        .map_err(|e| DatasetError::Format(format!("NPY read error at {}: {e}", path.display())))?;
    arr.into_dimensionality::<ndarray::Ix4>().map_err(|e| {
        DatasetError::Format(format!(
            "Expected 4-D array in {}, got shape {:?}: {e}",
            path.display(),
            arr.shape()
        ))
    })
}

/// Load a 3-D float32 NPY array (keypoints: `[T, J, 3]`).
fn load_npy_kp(path: &Path, _num_keypoints: usize) -> Result<ndarray::Array3<f32>, DatasetError> {
    use ndarray_npy::ReadNpyExt;
    let file = std::fs::File::open(path)?;
    let arr: ndarray::ArrayD<f32> = ndarray::ArrayD::read_npy(file)
        .map_err(|e| DatasetError::Format(format!("NPY read error at {}: {e}", path.display())))?;
    arr.into_dimensionality::<ndarray::Ix3>().map_err(|e| {
        DatasetError::Format(format!(
            "Expected 3-D keypoint array in {}, got shape {:?}: {e}",
            path.display(),
            arr.shape()
        ))
    })
}

/// Read only the first dimension of an NPY header (the frame count) without
/// loading the entire file into memory.
fn peek_npy_first_dim(path: &Path) -> Result<usize, DatasetError> {
    // Minimum viable NPY header parse: magic + version + header_len + header.
    use std::io::{BufReader, Read};
    let f = std::fs::File::open(path)?;
    let mut reader = BufReader::new(f);

    let mut magic = [0u8; 6];
    reader.read_exact(&mut magic)?;
    if &magic != b"\x93NUMPY" {
        return Err(DatasetError::Format(format!(
            "Not a valid NPY file: {}",
            path.display()
        )));
    }

    let mut version = [0u8; 2];
    reader.read_exact(&mut version)?;

    // Header length field: 2 bytes in v1, 4 bytes in v2
    let header_len: usize = if version[0] == 1 {
        let mut buf = [0u8; 2];
        reader.read_exact(&mut buf)?;
        u16::from_le_bytes(buf) as usize
    } else {
        let mut buf = [0u8; 4];
        reader.read_exact(&mut buf)?;
        u32::from_le_bytes(buf) as usize
    };

    let mut header = vec![0u8; header_len];
    reader.read_exact(&mut header)?;
    let header_str = String::from_utf8_lossy(&header);

    // Parse the shape tuple using a simple substring search.
    // Example header: "{'descr': '<f4', 'fortran_order': False, 'shape': (300, 3, 3, 114), }"
    if let Some(start) = header_str.find("'shape': (") {
        let rest = &header_str[start + "'shape': (".len()..];
        if let Some(end) = rest.find(')') {
            let shape_str = &rest[..end];
            let dims: Vec<usize> = shape_str
                .split(',')
                .filter_map(|s| s.trim().parse::<usize>().ok())
                .collect();
            if let Some(&first) = dims.first() {
                return Ok(first);
            }
        }
    }

    Err(DatasetError::Format(format!(
        "Cannot parse shape from NPY header in {}",
        path.display()
    )))
}

/// Parse the numeric suffix of a directory name like `S01` → `1` or `A12` → `12`.
fn parse_id_suffix(name: &str) -> Option<u32> {
    name.chars()
        .skip_while(|c| c.is_alphabetic())
        .collect::<String>()
        .parse::<u32>()
        .ok()
}

// ---------------------------------------------------------------------------
// SyntheticCsiDataset
// ---------------------------------------------------------------------------

/// Configuration for [`SyntheticCsiDataset`].
///
/// All fields are plain numbers; no randomness is involved.
#[derive(Debug, Clone)]
pub struct SyntheticConfig {
    /// Number of output subcarriers. Default: **56**.
    pub num_subcarriers: usize,
    /// Number of transmit antennas. Default: **3**.
    pub num_antennas_tx: usize,
    /// Number of receive antennas. Default: **3**.
    pub num_antennas_rx: usize,
    /// Temporal window length. Default: **100**.
    pub window_frames: usize,
    /// Number of body keypoints. Default: **17** (COCO).
    pub num_keypoints: usize,
    /// Carrier frequency for phase model. Default: **2.4e9 Hz**.
    pub signal_frequency_hz: f32,
}

impl Default for SyntheticConfig {
    fn default() -> Self {
        SyntheticConfig {
            num_subcarriers: 56,
            num_antennas_tx: 3,
            num_antennas_rx: 3,
            window_frames: 100,
            num_keypoints: 17,
            signal_frequency_hz: 2.4e9,
        }
    }
}

/// Fully-deterministic CSI dataset generated from a physical signal model.
///
/// No random number generator is used. Every sample at index `idx` is computed
/// analytically from `idx` alone, making the dataset perfectly reproducible
/// and portable across platforms.
///
/// ## Amplitude model
///
/// For sample `idx`, frame `t`, tx `i`, rx `j`, subcarrier `k`:
///
/// ```text
/// A = 0.5 + 0.3 × sin(2π × (idx × 0.01 + t × 0.1 + k × 0.05))
/// ```
///
/// ## Phase model
///
/// ```text
/// φ = (2π × k / num_subcarriers) × (i + 1) × (j + 1)
/// ```
///
/// ## Keypoint model
///
/// Joint `j` is placed at:
///
/// ```text
/// x = 0.5 + 0.1 × sin(2π × idx × 0.007 + j)
/// y = 0.3 + j × 0.04
/// ```
pub struct SyntheticCsiDataset {
    num_samples: usize,
    config: SyntheticConfig,
}

impl SyntheticCsiDataset {
    /// Create a new synthetic dataset with `num_samples` entries.
    pub fn new(num_samples: usize, config: SyntheticConfig) -> Self {
        SyntheticCsiDataset { num_samples, config }
    }

    /// Compute the deterministic amplitude value for the given indices.
    #[inline]
    fn amp_value(&self, idx: usize, t: usize, _tx: usize, _rx: usize, k: usize) -> f32 {
        let phase = 2.0 * std::f32::consts::PI
            * (idx as f32 * 0.01 + t as f32 * 0.1 + k as f32 * 0.05);
        0.5 + 0.3 * phase.sin()
    }

    /// Compute the deterministic phase value for the given indices.
    #[inline]
    fn phase_value(&self, _idx: usize, _t: usize, tx: usize, rx: usize, k: usize) -> f32 {
        let n_sc = self.config.num_subcarriers as f32;
        (2.0 * std::f32::consts::PI * k as f32 / n_sc)
            * (tx as f32 + 1.0)
            * (rx as f32 + 1.0)
    }

    /// Compute the deterministic keypoint (x, y) for joint `j` at sample `idx`.
    #[inline]
    fn keypoint_xy(&self, idx: usize, j: usize) -> (f32, f32) {
        let x = 0.5
            + 0.1 * (2.0 * std::f32::consts::PI * idx as f32 * 0.007 + j as f32).sin();
        let y = 0.3 + j as f32 * 0.04;
        (x, y)
    }
}

impl CsiDataset for SyntheticCsiDataset {
    fn len(&self) -> usize {
        self.num_samples
    }

    fn get(&self, idx: usize) -> Result<CsiSample, DatasetError> {
        if idx >= self.num_samples {
            return Err(DatasetError::IndexOutOfBounds {
                idx,
                len: self.num_samples,
            });
        }

        let cfg = &self.config;
        let (t, n_tx, n_rx, n_sc) =
            (cfg.window_frames, cfg.num_antennas_tx, cfg.num_antennas_rx, cfg.num_subcarriers);

        let amplitude = Array4::from_shape_fn((t, n_tx, n_rx, n_sc), |(frame, tx, rx, k)| {
            self.amp_value(idx, frame, tx, rx, k)
        });

        let phase = Array4::from_shape_fn((t, n_tx, n_rx, n_sc), |(frame, tx, rx, k)| {
            self.phase_value(idx, frame, tx, rx, k)
        });

        let mut keypoints = Array2::zeros((cfg.num_keypoints, 2));
        let mut keypoint_visibility = Array1::zeros(cfg.num_keypoints);
        for j in 0..cfg.num_keypoints {
            let (x, y) = self.keypoint_xy(idx, j);
            // Clamp to [0, 1] to keep coordinates valid.
            keypoints[[j, 0]] = x.clamp(0.0, 1.0);
            keypoints[[j, 1]] = y.clamp(0.0, 1.0);
            // All joints are visible in the synthetic model.
            keypoint_visibility[j] = 2.0;
        }

        Ok(CsiSample {
            amplitude,
            phase,
            keypoints,
            keypoint_visibility,
            subject_id: 0,
            action_id: 0,
            frame_id: idx as u64,
        })
    }

    fn name(&self) -> &str {
        "SyntheticCsiDataset"
    }
}

// ---------------------------------------------------------------------------
// DatasetError
// ---------------------------------------------------------------------------

/// Errors produced by dataset operations.
#[derive(Debug, Error)]
pub enum DatasetError {
    /// Requested index is outside the valid range.
    #[error("Index {idx} out of bounds (dataset has {len} samples)")]
    IndexOutOfBounds { idx: usize, len: usize },

    /// An underlying file-system error occurred.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// The file exists but does not match the expected format.
    #[error("File format error: {0}")]
    Format(String),

    /// The loaded array has a different subcarrier count than required.
    #[error("Subcarrier count mismatch: expected {expected}, got {actual}")]
    SubcarrierMismatch { expected: usize, actual: usize },

    /// The specified root directory does not exist.
    #[error("Directory not found: {path}")]
    DirectoryNotFound { path: String },
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    // ----- SyntheticCsiDataset --------------------------------------------

    #[test]
    fn synthetic_sample_shapes() {
        let cfg = SyntheticConfig::default();
        let ds = SyntheticCsiDataset::new(10, cfg.clone());
        let s = ds.get(0).unwrap();

        assert_eq!(s.amplitude.shape(), &[cfg.window_frames, cfg.num_antennas_tx, cfg.num_antennas_rx, cfg.num_subcarriers]);
        assert_eq!(s.phase.shape(), &[cfg.window_frames, cfg.num_antennas_tx, cfg.num_antennas_rx, cfg.num_subcarriers]);
        assert_eq!(s.keypoints.shape(), &[cfg.num_keypoints, 2]);
        assert_eq!(s.keypoint_visibility.shape(), &[cfg.num_keypoints]);
    }

    #[test]
    fn synthetic_is_deterministic() {
        let cfg = SyntheticConfig::default();
        let ds = SyntheticCsiDataset::new(10, cfg);
        let s0a = ds.get(3).unwrap();
        let s0b = ds.get(3).unwrap();
        assert_abs_diff_eq!(s0a.amplitude[[0, 0, 0, 0]], s0b.amplitude[[0, 0, 0, 0]], epsilon = 1e-7);
        assert_abs_diff_eq!(s0a.keypoints[[5, 0]], s0b.keypoints[[5, 0]], epsilon = 1e-7);
    }

    #[test]
    fn synthetic_different_indices_differ() {
        let cfg = SyntheticConfig::default();
        let ds = SyntheticCsiDataset::new(10, cfg);
        let s0 = ds.get(0).unwrap();
        let s1 = ds.get(1).unwrap();
        // The sinusoidal model ensures different idx gives different values.
        assert!((s0.amplitude[[0, 0, 0, 0]] - s1.amplitude[[0, 0, 0, 0]]).abs() > 1e-6);
    }

    #[test]
    fn synthetic_out_of_bounds() {
        let ds = SyntheticCsiDataset::new(5, SyntheticConfig::default());
        assert!(matches!(ds.get(5), Err(DatasetError::IndexOutOfBounds { idx: 5, len: 5 })));
    }

    #[test]
    fn synthetic_amplitude_in_valid_range() {
        // Model: 0.5 ± 0.3, so all values in [0.2, 0.8]
        let cfg = SyntheticConfig::default();
        let ds = SyntheticCsiDataset::new(4, cfg);
        for idx in 0..4 {
            let s = ds.get(idx).unwrap();
            for &v in s.amplitude.iter() {
                assert!(v >= 0.19 && v <= 0.81, "amplitude {v} out of [0.2, 0.8]");
            }
        }
    }

    #[test]
    fn synthetic_keypoints_in_unit_square() {
        let cfg = SyntheticConfig::default();
        let ds = SyntheticCsiDataset::new(8, cfg);
        for idx in 0..8 {
            let s = ds.get(idx).unwrap();
            for kp in s.keypoints.outer_iter() {
                assert!(kp[0] >= 0.0 && kp[0] <= 1.0, "x={} out of [0,1]", kp[0]);
                assert!(kp[1] >= 0.0 && kp[1] <= 1.0, "y={} out of [0,1]", kp[1]);
            }
        }
    }

    #[test]
    fn synthetic_all_joints_visible() {
        let cfg = SyntheticConfig::default();
        let ds = SyntheticCsiDataset::new(3, cfg.clone());
        let s = ds.get(0).unwrap();
        assert!(s.keypoint_visibility.iter().all(|&v| (v - 2.0).abs() < 1e-6));
    }

    // ----- DataLoader -------------------------------------------------------

    #[test]
    fn dataloader_num_batches() {
        let cfg = SyntheticConfig::default();
        let ds = SyntheticCsiDataset::new(10, cfg);
        // 10 samples, batch_size=3 → ceil(10/3) = 4
        let dl = DataLoader::new(&ds, 3, false, 42);
        assert_eq!(dl.num_batches(), 4);
    }

    #[test]
    fn dataloader_iterates_all_samples_no_shuffle() {
        let cfg = SyntheticConfig::default();
        let ds = SyntheticCsiDataset::new(10, cfg);
        let dl = DataLoader::new(&ds, 3, false, 42);
        let total: usize = dl.iter().map(|b| b.len()).sum();
        assert_eq!(total, 10);
    }

    #[test]
    fn dataloader_iterates_all_samples_shuffle() {
        let cfg = SyntheticConfig::default();
        let ds = SyntheticCsiDataset::new(17, cfg);
        let dl = DataLoader::new(&ds, 4, true, 42);
        let total: usize = dl.iter().map(|b| b.len()).sum();
        assert_eq!(total, 17);
    }

    #[test]
    fn dataloader_shuffle_is_deterministic() {
        let cfg = SyntheticConfig::default();
        let ds = SyntheticCsiDataset::new(20, cfg);
        let dl1 = DataLoader::new(&ds, 5, true, 99);
        let dl2 = DataLoader::new(&ds, 5, true, 99);
        let ids1: Vec<u64> = dl1.iter().flatten().map(|s| s.frame_id).collect();
        let ids2: Vec<u64> = dl2.iter().flatten().map(|s| s.frame_id).collect();
        assert_eq!(ids1, ids2);
    }

    #[test]
    fn dataloader_different_seeds_differ() {
        let cfg = SyntheticConfig::default();
        let ds = SyntheticCsiDataset::new(20, cfg);
        let dl1 = DataLoader::new(&ds, 20, true, 1);
        let dl2 = DataLoader::new(&ds, 20, true, 2);
        let ids1: Vec<u64> = dl1.iter().flatten().map(|s| s.frame_id).collect();
        let ids2: Vec<u64> = dl2.iter().flatten().map(|s| s.frame_id).collect();
        assert_ne!(ids1, ids2, "different seeds should produce different orders");
    }

    #[test]
    fn dataloader_empty_dataset() {
        let cfg = SyntheticConfig::default();
        let ds = SyntheticCsiDataset::new(0, cfg);
        let dl = DataLoader::new(&ds, 4, false, 42);
        assert_eq!(dl.num_batches(), 0);
        assert_eq!(dl.iter().count(), 0);
    }

    // ----- Helpers ----------------------------------------------------------

    #[test]
    fn parse_id_suffix_works() {
        assert_eq!(parse_id_suffix("S01"), Some(1));
        assert_eq!(parse_id_suffix("A12"), Some(12));
        assert_eq!(parse_id_suffix("foo"), None);
        assert_eq!(parse_id_suffix("S"), None);
    }

    #[test]
    fn xorshift_shuffle_is_permutation() {
        let mut indices: Vec<usize> = (0..20).collect();
        xorshift_shuffle(&mut indices, 42);
        let mut sorted = indices.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn xorshift_shuffle_is_deterministic() {
        let mut a: Vec<usize> = (0..20).collect();
        let mut b: Vec<usize> = (0..20).collect();
        xorshift_shuffle(&mut a, 123);
        xorshift_shuffle(&mut b, 123);
        assert_eq!(a, b);
    }
}
