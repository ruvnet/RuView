//! Training pipeline — collect spatial observations and train depth/occupancy models.
//!
//! Three training modes:
//! 1. **Depth calibration**: capture camera frames + known distances → calibrate
//!    the luminance-to-depth mapping parameters
//! 2. **CSI occupancy training**: capture CSI with known occupancy ground truth →
//!    train the tomography weights for this room geometry
//! 3. **Brain integration**: store spatial observations as brain memories for
//!    DPO training — "this depth estimate was correct" vs "this was wrong"

use crate::fusion::OccupancyVolume;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Reject a user-supplied path that contains `..` components (path traversal
/// attempt) and return a normalised [`PathBuf`]. We only reject `..`; other
/// components (including relative prefixes and `~`) are accepted verbatim —
/// the caller is responsible for tilde expansion if needed.
pub fn sanitize_data_path(raw: &str) -> Result<PathBuf> {
    let p = PathBuf::from(raw);
    for comp in p.components() {
        if matches!(comp, std::path::Component::ParentDir) {
            return Err(anyhow!(
                "refusing to use data dir with `..` traversal component: {raw}"
            ));
        }
    }
    Ok(p)
}

/// Ensure `child` (after joining to `base`) stays inside the canonicalised
/// `base` directory. Returns the canonical child path on success. Used by
/// every filesystem write site in this module to prevent path-traversal
/// through user-supplied names.
fn safe_join(base: &Path, child: &str) -> Result<PathBuf> {
    // Reject absolute children and any `..` components up front.
    let child_path = Path::new(child);
    if child_path.is_absolute() {
        return Err(anyhow!("child path must be relative: {child}"));
    }
    for comp in child_path.components() {
        if matches!(comp, std::path::Component::ParentDir) {
            return Err(anyhow!("child path may not contain `..`: {child}"));
        }
    }

    let joined = base.join(child_path);
    // Canonicalise base (must exist) and verify joined starts with it. If the
    // joined file doesn't exist yet we canonicalise the parent.
    let canonical_base = base.canonicalize()
        .map_err(|e| anyhow!("data_dir not accessible {}: {e}", base.display()))?;
    let canonical_parent = joined
        .parent()
        .ok_or_else(|| anyhow!("no parent for {}", joined.display()))?;
    let canonical_parent = canonical_parent
        .canonicalize()
        .map_err(|e| anyhow!("parent not accessible {}: {e}", canonical_parent.display()))?;
    if !canonical_parent.starts_with(&canonical_base) {
        return Err(anyhow!(
            "refusing to write outside data_dir: {}",
            joined.display()
        ));
    }
    Ok(canonical_parent.join(
        joined.file_name().ok_or_else(|| anyhow!("no filename for {}", joined.display()))?,
    ))
}

/// Training data sample — a snapshot of the scene.
#[derive(Serialize, Deserialize)]
pub struct TrainingSample {
    pub timestamp_ms: i64,
    pub source: String,
    /// Camera depth map (downsampled, in meters)
    pub depth_map: Option<Vec<f32>>,
    pub depth_width: u32,
    pub depth_height: u32,
    /// WiFi occupancy grid
    pub occupancy: Option<OccupancyData>,
    /// Ground truth (if available)
    pub ground_truth: Option<GroundTruth>,
    /// Quality score (0.0-1.0, rated by user or self-eval)
    pub quality: f32,
}

#[derive(Serialize, Deserialize)]
pub struct OccupancyData {
    pub densities: Vec<f64>,
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
}

impl From<&OccupancyVolume> for OccupancyData {
    fn from(vol: &OccupancyVolume) -> Self {
        Self {
            densities: vol.densities.clone(),
            nx: vol.nx, ny: vol.ny, nz: vol.nz,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct GroundTruth {
    /// Known distances to reference points (e.g., wall at 3.0m)
    pub reference_distances: Vec<ReferencePoint>,
    /// Known occupancy state (person present/absent + location)
    pub occupancy_label: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ReferencePoint {
    pub name: String,
    pub x_pixel: u32,
    pub y_pixel: u32,
    pub true_distance_m: f32,
}

/// Training session — accumulates samples and learns calibration.
pub struct TrainingSession {
    pub samples: Vec<TrainingSample>,
    pub calibration: DepthCalibration,
    pub data_dir: PathBuf,
}

/// Depth calibration parameters — maps luminance to real depth.
#[derive(Clone, Serialize, Deserialize)]
pub struct DepthCalibration {
    pub scale: f32,       // multiplier for depth values
    pub offset: f32,      // additive offset
    pub near_clip: f32,   // minimum valid depth
    pub far_clip: f32,    // maximum valid depth
    pub gamma: f32,       // nonlinear correction (luminance^gamma → depth)
    pub samples_used: u32,
    pub rmse: f32,        // root mean square error against ground truth
}

impl Default for DepthCalibration {
    fn default() -> Self {
        Self {
            scale: 4.0,
            offset: 1.0,
            near_clip: 0.3,
            far_clip: 8.0,
            gamma: 1.0,
            samples_used: 0,
            rmse: f32::MAX,
        }
    }
}

impl TrainingSession {
    /// Create a new training session rooted at `data_dir`.
    ///
    /// `data_dir` must not contain `..` components — we reject path traversal
    /// attempts from CLI/API input. The directory is created if missing and
    /// then canonicalised so every subsequent write stays inside it.
    pub fn new(data_dir: &str) -> Result<Self> {
        let path = sanitize_data_path(data_dir)?;
        std::fs::create_dir_all(&path)
            .map_err(|e| anyhow!("failed to create data_dir {}: {e}", path.display()))?;
        // Canonicalise so path-traversal checks in safe_join have a fixed root.
        let path = path
            .canonicalize()
            .map_err(|e| anyhow!("cannot canonicalise data_dir {}: {e}", path.display()))?;

        // Load existing calibration if available
        let cal_path = safe_join(&path, "calibration.json")
            // safe_join needs the parent to exist; for initial load that's always data_dir
            .or_else(|_| Ok::<_, anyhow::Error>(path.join("calibration.json")))?;
        let calibration = if cal_path.exists() {
            let data = std::fs::read_to_string(&cal_path)?;
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            DepthCalibration::default()
        };

        Ok(Self {
            samples: Vec::new(),
            calibration,
            data_dir: path,
        })
    }

    /// Add a training sample with optional ground truth.
    pub fn add_sample(
        &mut self,
        depth_map: Option<Vec<f32>>,
        width: u32,
        height: u32,
        occupancy: Option<&OccupancyVolume>,
        ground_truth: Option<GroundTruth>,
        quality: f32,
    ) {
        let sample = TrainingSample {
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            source: "capture".to_string(),
            depth_map,
            depth_width: width,
            depth_height: height,
            occupancy: occupancy.map(OccupancyData::from),
            ground_truth,
            quality,
        };
        self.samples.push(sample);
    }

    /// Calibrate depth estimation using ground truth reference points.
    ///
    /// Finds optimal scale, offset, and gamma to minimize RMSE
    /// between estimated and true depths at reference points.
    pub fn calibrate_depth(&mut self) -> Result<DepthCalibration> {
        let mut best = self.calibration.clone();
        let mut best_rmse = f32::MAX;

        // Collect all reference points across samples
        let refs: Vec<(f32, f32)> = self.samples.iter()
            .filter_map(|s| {
                let gt = s.ground_truth.as_ref()?;
                let dm = s.depth_map.as_ref()?;
                Some(gt.reference_distances.iter().filter_map(|rp| {
                    let idx = (rp.y_pixel * s.depth_width + rp.x_pixel) as usize;
                    dm.get(idx).map(|&est| (est, rp.true_distance_m))
                }).collect::<Vec<_>>())
            })
            .flatten()
            .collect();

        if refs.is_empty() {
            eprintln!("  No reference points — using default calibration");
            return Ok(best);
        }

        eprintln!("  Calibrating with {} reference points...", refs.len());

        // Grid search over scale, offset, gamma
        for scale_i in 0..20 {
            let scale = 1.0 + scale_i as f32 * 0.5;
            for offset_i in 0..10 {
                let offset = offset_i as f32 * 0.5;
                for gamma_i in 5..15 {
                    let gamma = gamma_i as f32 * 0.2;

                    let rmse = refs.iter()
                        .map(|&(est, truth)| {
                            let calibrated = offset + est.powf(gamma) * scale;
                            (calibrated - truth).powi(2)
                        })
                        .sum::<f32>() / refs.len() as f32;
                    let rmse = rmse.sqrt();

                    if rmse < best_rmse {
                        best_rmse = rmse;
                        best = DepthCalibration {
                            scale, offset, gamma,
                            near_clip: 0.3, far_clip: 8.0,
                            samples_used: refs.len() as u32,
                            rmse,
                        };
                    }
                }
            }
        }

        eprintln!("  Best calibration: scale={:.2} offset={:.2} gamma={:.2} RMSE={:.4}m",
            best.scale, best.offset, best.gamma, best.rmse);

        self.calibration = best.clone();
        self.save_calibration()?;
        Ok(best)
    }

    /// Train CSI occupancy model — adjust tomography weights.
    ///
    /// Uses samples with known occupancy labels to optimize the
    /// attenuation-to-density mapping.
    pub fn train_occupancy(&self) -> Result<OccupancyCalibration> {
        let labeled: Vec<&TrainingSample> = self.samples.iter()
            .filter(|s| s.ground_truth.as_ref().and_then(|g| g.occupancy_label.as_ref()).is_some())
            .collect();

        if labeled.is_empty() {
            eprintln!("  No labeled occupancy samples — using defaults");
            return Ok(OccupancyCalibration::default());
        }

        eprintln!("  Training occupancy model with {} samples...", labeled.len());

        // Simple threshold optimization — find the density threshold
        // that best separates occupied vs unoccupied
        let mut best_threshold = 0.3f64;
        let mut best_accuracy = 0.0f64;

        for thresh_i in 1..20 {
            let threshold = thresh_i as f64 * 0.05;
            let mut correct = 0;
            let mut total = 0;

            for sample in &labeled {
                if let Some(ref occ) = sample.occupancy {
                    let label = sample.ground_truth.as_ref().unwrap()
                        .occupancy_label.as_ref().unwrap();
                    let is_occupied = label == "occupied" || label == "present";
                    let detected = occ.densities.iter().any(|&d| d > threshold);
                    if detected == is_occupied { correct += 1; }
                    total += 1;
                }
            }

            let accuracy = correct as f64 / total.max(1) as f64;
            if accuracy > best_accuracy {
                best_accuracy = accuracy;
                best_threshold = threshold;
            }
        }

        let cal = OccupancyCalibration {
            density_threshold: best_threshold,
            accuracy: best_accuracy,
            samples_used: labeled.len() as u32,
        };

        eprintln!("  Occupancy threshold={:.2} accuracy={:.1}%", cal.density_threshold, cal.accuracy * 100.0);

        // Save (path-traversal safe: constant filename under canonical data_dir)
        let path = safe_join(&self.data_dir, "occupancy_calibration.json")?;
        std::fs::write(&path, serde_json::to_string_pretty(&cal)?)?;

        Ok(cal)
    }

    /// Export training data as preference pairs for DPO training on the brain.
    ///
    /// Good samples (quality > 0.7) → chosen
    /// Bad samples (quality < 0.3) → rejected
    pub fn export_preference_pairs(&self) -> Result<Vec<PreferencePair>> {
        let mut pairs = Vec::new();

        let good: Vec<&TrainingSample> = self.samples.iter()
            .filter(|s| s.quality > 0.7)
            .collect();
        let bad: Vec<&TrainingSample> = self.samples.iter()
            .filter(|s| s.quality < 0.3)
            .collect();

        for (g, b) in good.iter().zip(bad.iter()) {
            pairs.push(PreferencePair {
                chosen: format!(
                    "Depth estimation at {}ms: {} points, quality {:.2}",
                    g.timestamp_ms,
                    g.depth_map.as_ref().map(|d| d.len()).unwrap_or(0),
                    g.quality
                ),
                rejected: format!(
                    "Depth estimation at {}ms: {} points, quality {:.2}",
                    b.timestamp_ms,
                    b.depth_map.as_ref().map(|d| d.len()).unwrap_or(0),
                    b.quality
                ),
            });
        }

        // Save pairs (path-traversal safe: constant filename under canonical data_dir)
        let path = safe_join(&self.data_dir, "preference_pairs.jsonl")?;
        let mut f = std::fs::File::create(&path)?;
        for pair in &pairs {
            use std::io::Write;
            writeln!(f, "{}", serde_json::to_string(pair)?)?;
        }

        eprintln!("  Exported {} preference pairs to {}", pairs.len(), path.display());
        Ok(pairs)
    }

    /// Send training results to the ruOS brain for storage.
    pub async fn submit_to_brain(&self, brain_url: &str) -> Result<u32> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        let mut stored = 0u32;

        // Store calibration as brain memory
        let _cal_json = serde_json::to_string(&self.calibration)?;
        let body = serde_json::json!({
            "category": "spatial-calibration",
            "content": format!("Depth calibration: scale={:.2} offset={:.2} gamma={:.2} RMSE={:.4}m ({} samples)",
                self.calibration.scale, self.calibration.offset, self.calibration.gamma,
                self.calibration.rmse, self.calibration.samples_used),
        });
        if client.post(format!("{brain_url}/memories"))
            .json(&body).send().await.is_ok() {
            stored += 1;
        }

        // Store good observations
        for sample in self.samples.iter().filter(|s| s.quality > 0.5) {
            let body = serde_json::json!({
                "category": "spatial-observation",
                "content": format!("Point cloud capture: {} depth points, quality {:.2}, occupancy {}",
                    sample.depth_map.as_ref().map(|d| d.len()).unwrap_or(0),
                    sample.quality,
                    sample.occupancy.as_ref().map(|o| format!("{}x{}x{}", o.nx, o.ny, o.nz)).unwrap_or("none".into())),
            });
            if client.post(format!("{brain_url}/memories"))
                .json(&body).send().await.is_ok() {
                stored += 1;
            }
        }

        eprintln!("  Submitted {} observations to brain", stored);
        Ok(stored)
    }

    /// Save current calibration to disk (path-traversal safe).
    fn save_calibration(&self) -> Result<()> {
        let path = safe_join(&self.data_dir, "calibration.json")?;
        std::fs::write(&path, serde_json::to_string_pretty(&self.calibration)?)?;
        Ok(())
    }

    /// Save all samples to disk (path-traversal safe).
    pub fn save_samples(&self) -> Result<()> {
        let path = safe_join(&self.data_dir, "samples.json")?;
        std::fs::write(&path, serde_json::to_string_pretty(&self.samples)?)?;
        eprintln!("  Saved {} samples to {}", self.samples.len(), path.display());
        Ok(())
    }

    /// Load samples from disk (path-traversal safe).
    pub fn load_samples(&mut self) -> Result<()> {
        let path = safe_join(&self.data_dir, "samples.json")?;
        if path.exists() {
            let data = std::fs::read_to_string(&path)?;
            self.samples = serde_json::from_str(&data)?;
            eprintln!("  Loaded {} samples", self.samples.len());
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct OccupancyCalibration {
    pub density_threshold: f64,
    pub accuracy: f64,
    pub samples_used: u32,
}

impl Default for OccupancyCalibration {
    fn default() -> Self {
        Self { density_threshold: 0.3, accuracy: 0.0, samples_used: 0 }
    }
}

#[derive(Serialize, Deserialize)]
pub struct PreferencePair {
    pub chosen: String,
    pub rejected: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_parent_dir_traversal() {
        assert!(sanitize_data_path("../etc/passwd").is_err());
        assert!(sanitize_data_path("foo/../bar").is_err());
        assert!(sanitize_data_path("/tmp/.. /evil").is_ok(), "`.. ` is not ParentDir");
    }

    #[test]
    fn sanitize_accepts_relative_child() {
        assert!(sanitize_data_path("data/ruview").is_ok());
        assert!(sanitize_data_path("./foo").is_ok());
    }

    #[test]
    fn training_session_new_rejects_traversal() {
        // Even if the filesystem has such a path, TrainingSession should refuse.
        let err = TrainingSession::new("../etc/passwd").err();
        assert!(err.is_some(), "traversal path must be rejected");
    }

    #[test]
    fn training_session_new_accepts_child_path() {
        // Use a unique tmpdir to avoid cross-test interference.
        let tmp = std::env::temp_dir().join(format!("ruview-train-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let sess = TrainingSession::new(tmp.to_str().unwrap())
            .expect("TrainingSession should accept a clean tmpdir");
        // data_dir should have been canonicalised to an absolute path.
        assert!(sess.data_dir.is_absolute());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
