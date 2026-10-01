//! Prepare a local public-CSI download for training and evaluation.
//!
//! This command validates a manually downloaded dataset directory, measures
//! its real characteristics from the files, and writes machine-readable
//! provenance artifacts:
//!
//! ```text
//! prepare-public-dataset --dataset mmfi
//!     --input data/public/mmfi --output data/processed/mmfi
//! ```
//!
//! Outputs (all JSON):
//!
//! - `dataset_manifest.json` — source URL, license, citation, measured
//!   counts, native subcarrier width, antenna layout, checksums.
//! - `split.json` — split policy, seed, subject lists, window counts.
//!
//! The command downloads nothing. MM-Fi and Wi-Pose both require manual
//! download through the dataset authors' release channels (see
//! `docs/cpu-inference-real-csi.md`); access controls must not be bypassed.
//! Full datasets must never be committed to Git — keep them under an ignored
//! `data/` directory.

use clap::Parser;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use wifi_densepose_train::dataset::{CsiDataset, MmFiDataset};
use wifi_densepose_train::public_dataset::{DatasetManifest, PreparedSplit, SplitPolicy};

/// Validate a downloaded public CSI dataset and write its manifests.
#[derive(Parser, Debug)]
#[command(name = "prepare-public-dataset", version)]
struct Args {
    /// Dataset key: `mmfi` (supported) or `wi-pose` (not yet implemented).
    #[arg(long, default_value = "mmfi")]
    dataset: String,

    /// Local directory holding the downloaded dataset (`SXX/AXX/*.npy`).
    #[arg(long)]
    input: PathBuf,

    /// Output directory for `dataset_manifest.json` + `split.json`.
    #[arg(long)]
    output: PathBuf,

    /// Frames per window (MM-Fi samples use 10).
    #[arg(long, default_value_t = 10)]
    window_frames: usize,

    /// Model input subcarrier width recorded in the manifests.
    #[arg(long, default_value_t = 56)]
    target_subcarriers: usize,

    /// Keypoints per frame (MM-Fi: 17 COCO).
    #[arg(long, default_value_t = 17)]
    num_keypoints: usize,

    /// Fraction of subjects assigned to test (subject-disjoint split).
    #[arg(long, default_value_t = 0.2)]
    test_fraction: f64,

    /// Deterministic seed for subject assignment.
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Restrict preparation to the first N sorted subjects (dev subset).
    /// Recorded in the manifest notes.
    #[arg(long)]
    subset_subjects: Option<usize>,

    /// Sampling rate in Hz, transcribed from the dataset documentation
    /// (not measurable from the `.npy` files themselves).
    #[arg(long, default_value_t = 100.0)]
    sampling_rate_hz: f32,

    /// Hash every discovered `.npy` file with SHA-256 (slow on full data).
    #[arg(long, default_value_t = false)]
    sha256: bool,

    /// Number of distinct environments / rooms. The `SXX/AXX` layout does
    /// not encode this — pass the documented value explicitly (MM-Fi: 4)
    /// or leave 0 (unknown).
    #[arg(long, default_value_t = 0)]
    environment_count: usize,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    run(args).map_err(|e| anyhow::anyhow!("{e}"))
}

fn run(args: Args) -> Result<(), String> {
    if args.dataset != "mmfi" {
        return Err(format!(
            "dataset `{}` is not implemented in this slice: only `mmfi` is \
             supported. For `wi-pose`, convert the release `.mat` files to \
             the documented `.npy` layout first (see \
             docs/cpu-inference-real-csi.md) and track it for a follow-up.",
            args.dataset
        ));
    }
    if !args.input.is_dir() {
        return Err(format!(
            "input directory not found: {}. Download the dataset manually \
             first (see docs/cpu-inference-real-csi.md); access controls \
             must not be bypassed.",
            args.input.display()
        ));
    }

    let mut manifest = DatasetManifest::mmfi_template();
    manifest.local_path = Some(args.output.clone());
    manifest.sampling_rate_hz = Some(args.sampling_rate_hz);
    manifest.preprocessing_version =
        wifi_densepose_train::public_dataset::MANIFEST_PREPROCESSING_VERSION.to_string();

    // Discover recordings (validates layout + measures sample counts).
    let ds = MmFiDataset::discover(
        &args.input,
        args.window_frames,
        args.target_subcarriers,
        args.num_keypoints,
    )
    .map_err(|e| format!("dataset discovery failed: {e}"))?;
    if ds.is_empty() {
        return Err(format!(
            "no usable recordings under {}. Expected SXX/AXX directories \
             with wifi_csi.npy + gt_keypoints.npy (wifi_csi_phase.npy optional).",
            args.input.display()
        ));
    }

    // Measure the native subcarrier width from the first amplitude file —
    // never assumed, always read.
    let first_amp = first_file_with_name(&args.input, "wifi_csi.npy")
        .ok_or_else(|| format!("no wifi_csi.npy found under {}", args.input.display()))?;
    let native_shape = parse_npy_shape(&first_amp)?;
    let native_sc = *native_shape.last().ok_or_else(|| {
        format!(
            "cannot determine subcarrier width from {}",
            first_amp.display()
        )
    })?;
    manifest.original_subcarrier_count = native_sc;

    // Count files, subjects, and activities from the directory walk.
    let (subjects, activities, amp_files) = walk_layout(&args.input);
    if subjects.is_empty() {
        return Err(format!(
            "no subject directories (SXX) under {}",
            args.input.display()
        ));
    }
    let keep: BTreeSet<u32> = match args.subset_subjects {
        Some(n) => {
            if n == 0 || n > subjects.len() {
                return Err(format!(
                    "--subset-subjects must be in 1..={}, got {n}",
                    subjects.len()
                ));
            }
            subjects.iter().copied().take(n).collect()
        }
        None => subjects.clone(),
    };
    if keep.len() != subjects.len() {
        manifest.notes.push_str(&format!(
            "Development subset: first {} of {} subjects ({}). ",
            keep.len(),
            subjects.len(),
            sorted_list(&keep)
        ));
    }
    manifest.notes.push_str(
        "Sampling rate transcribed from the MM-Fi release documentation, not \
         measured from the files. ",
    );

    manifest.file_count = amp_files.len();
    manifest.subject_count = keep.len();
    manifest.activity_count = activities.len();
    manifest.environment_count = args.environment_count;
    manifest.characteristics_verified = true;

    // Sample count + split over the kept subjects.
    let full_subjects = ds.subjects();
    let missing: Vec<u32> = keep
        .difference(&full_subjects.iter().copied().collect())
        .copied()
        .collect();
    if !missing.is_empty() {
        return Err(format!("subjects {missing:?} have no loadable windows"));
    }
    let subset_ds = MmFiDataset::discover_subset(
        &args.input,
        args.window_frames,
        args.target_subcarriers,
        args.num_keypoints,
        &keep,
    )
    .map_err(|e| format!("subset discovery failed: {e}"))?;
    manifest.sample_count = subset_ds.len();

    let (train_view, test_view) = subset_ds
        .subject_disjoint_split(args.test_fraction, args.seed)
        .map_err(|e| format!("split failed: {e}"))?;
    let split = PreparedSplit {
        dataset: manifest.name.clone(),
        policy: SplitPolicy::SubjectDisjoint,
        seed: args.seed,
        train_subjects: train_view.subjects().iter().copied().collect(),
        test_subjects: test_view.subjects().iter().copied().collect(),
        train_windows: train_view.len(),
        test_windows: test_view.len(),
    };
    split.verify().map_err(|e| format!("split invalid: {e}"))?;

    if args.sha256 {
        for (i, rel) in amp_files.iter().enumerate() {
            if i % 50 == 0 {
                println!("hashing {}/{} …", i + 1, amp_files.len());
            }
            let full = args.input.join(rel);
            let hex = sha256_file(&full)?;
            manifest.record_checksum(rel.clone(), hex);
        }
    }

    manifest
        .validate()
        .map_err(|e| format!("manifest invalid: {e}"))?;

    std::fs::create_dir_all(&args.output)
        .map_err(|e| format!("cannot create {}: {e}", args.output.display()))?;
    let manifest_path = manifest
        .save()
        .map_err(|e| format!("manifest save failed: {e}"))?;
    let split_path = split
        .save(&args.output, "split.json")
        .map_err(|e| format!("split save failed: {e}"))?;

    println!("dataset : {} ({})", manifest.name, manifest.source_url);
    println!(
        "license : {} — {}",
        manifest.license, manifest.license_notes
    );
    println!("files   : {} amplitude files", manifest.file_count);
    println!("samples : {} windows", manifest.sample_count);
    println!(
        "subjects: {} kept ({})",
        manifest.subject_count,
        sorted_list(&keep)
    );
    println!("native subcarriers: {native_sc}");
    println!(
        "split   : {} train / {} test windows (seed {})",
        split.train_windows, split.test_windows, split.seed
    );
    println!("wrote   : {}", manifest_path.display());
    println!("wrote   : {}", split_path.display());
    Ok(())
}

fn sorted_list(set: &BTreeSet<u32>) -> String {
    set.iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Walk `<root>/SXX/AXX`, collecting subject ids, activity ids, and the
/// relative paths of amplitude files.
fn walk_layout(root: &Path) -> (BTreeSet<u32>, BTreeSet<u32>, Vec<String>) {
    let mut subjects = BTreeSet::new();
    let mut activities = BTreeSet::new();
    let mut amp_files = Vec::new();
    let Ok(subject_dirs) = std::fs::read_dir(root) else {
        return (subjects, activities, amp_files);
    };
    for subj in subject_dirs.filter_map(|e| e.ok()) {
        if !subj.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = subj.file_name().to_string_lossy().into_owned();
        let Some(subject_id) = parse_id_suffix(&name) else {
            continue;
        };
        let Ok(action_dirs) = std::fs::read_dir(subj.path()) else {
            continue;
        };
        let mut any = false;
        for action in action_dirs.filter_map(|e| e.ok()) {
            if !action.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let aname = action.file_name().to_string_lossy().into_owned();
            let Some(action_id) = parse_id_suffix(&aname) else {
                continue;
            };
            let amp = action.path().join("wifi_csi.npy");
            if amp.is_file() {
                any = true;
                activities.insert(action_id);
                if let Ok(rel) = amp
                    .strip_prefix(root)
                    .map(|p| p.to_string_lossy().into_owned())
                {
                    amp_files.push(rel);
                }
            }
        }
        if any {
            subjects.insert(subject_id);
        }
    }
    amp_files.sort();
    (subjects, activities, amp_files)
}

/// Parse a trailing numeric id from `S01` / `A12`-style names.
fn parse_id_suffix(name: &str) -> Option<u32> {
    let digits: String = name
        .chars()
        .rev()
        .take_while(|c| c.is_numeric())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if digits.is_empty() {
        return None;
    }
    let prefix = &name[..name.len() - digits.len()];
    if prefix.len() != 1 || !prefix.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    digits.parse().ok()
}

/// First file named `file_name` under `root` (sorted walk).
fn first_file_with_name(root: &Path, file_name: &str) -> Option<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|n| n == file_name) {
                found.push(path);
            }
        }
    }
    found.sort();
    found.into_iter().next()
}

/// Parse the shape tuple from a NumPy `.npy` v1.0 header without reading the
/// array payload.
///
/// Format: magic `\x93NUMPY`, major/minor bytes, u16-LE header length, then
/// an ASCII dict like `{'descr': '<f4', 'fortran_order': False, 'shape':
/// (8, 1, 3, 114), }`. Only the shape is extracted; dtype is left for the
/// array loader to validate.
fn parse_npy_shape(path: &Path) -> Result<Vec<usize>, String> {
    use std::io::Read;
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let mut prefix = [0u8; 10];
    file.read_exact(&mut prefix)
        .map_err(|e| format!("cannot read header of {}: {e}", path.display()))?;
    if &prefix[0..6] != b"\x93NUMPY" {
        return Err(format!("{} is not a .npy file (bad magic)", path.display()));
    }
    if prefix[6] != 1 {
        return Err(format!(
            "{} uses .npy major version {} (only v1.x supported)",
            path.display(),
            prefix[6]
        ));
    }
    let header_len = u16::from_le_bytes([prefix[8], prefix[9]]) as usize;
    let mut header = vec![0u8; header_len];
    file.read_exact(&mut header)
        .map_err(|e| format!("cannot read header of {}: {e}", path.display()))?;
    let header =
        String::from_utf8(header).map_err(|_| format!("non-UTF8 header in {}", path.display()))?;
    parse_shape_tuple(&header)
        .ok_or_else(|| format!("cannot parse shape tuple from header of {}", path.display()))
}

/// Extract the integer tuple following `'shape': (` in an `.npy` header dict.
fn parse_shape_tuple(header: &str) -> Option<Vec<usize>> {
    let key = header.find("'shape'")?;
    let open = header[key..].find('(')? + key;
    let close = header[open..].find(')')? + open;
    let inner = header[open + 1..close].trim();
    if inner.is_empty() {
        return Some(Vec::new());
    }
    inner
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<usize>().ok())
        .collect()
}

/// SHA-256 hex digest of a file, streamed in 1 MiB chunks.
fn sha256_file(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1 << 20];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_npy_header_shape() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("a.npy");
        let arr = ndarray::Array4::<f32>::zeros((8, 1, 3, 114));
        ndarray_npy::write_npy(&path, &arr).expect("write");
        assert_eq!(parse_npy_shape(&path).expect("parse"), vec![8, 1, 3, 114]);
    }

    #[test]
    fn rejects_non_npy_magic() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("a.npy");
        std::fs::write(&path, b"definitely not numpy").expect("write");
        assert!(parse_npy_shape(&path).is_err());
    }

    #[test]
    fn parses_id_suffixes() {
        assert_eq!(parse_id_suffix("S01"), Some(1));
        assert_eq!(parse_id_suffix("A27"), Some(27));
        assert_eq!(parse_id_suffix("wifi_csi"), None);
        assert_eq!(parse_id_suffix("01"), None);
        assert_eq!(parse_id_suffix(""), None);
    }

    #[test]
    fn shape_tuple_edge_cases() {
        assert_eq!(
            parse_shape_tuple("{'descr': '<f4', 'fortran_order': False, 'shape': (8,), }"),
            Some(vec![8])
        );
        assert_eq!(
            parse_shape_tuple("{'shape': (8, 1, 3, 114), }"),
            Some(vec![8, 1, 3, 114])
        );
        assert_eq!(parse_shape_tuple("{'nope': 1}"), None);
    }
}
