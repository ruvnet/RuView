//! Training pipeline — collect spatial observations and train depth/occupancy models.
//!
//! Three training modes:
//! 1. **Depth calibration**: capture camera frames + known distances → calibrate
//!    the luminance-to-depth mapping parameters
//! 2. **CSI occupancy training**: capture CSI with known occupancy ground truth →
//!    train the tomography weights for this room geometry
//! 3. **Brain integration**: store spatial observations as brain memories for
//!    DPO training — "this depth estimate was correct" vs "this was wrong"

use crate::pointcloud::PointCloud;
use crate::fusion::OccupancyVolume;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    pub fn new(data_dir: &str) -> Result<Self> {
        let path = PathBuf::from(data_dir);
        std::fs::create_dir_all(&path)?;

        // Load existing calibration if available
        let cal_path = path.join("calibration.json");
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

        // Save
        let path = self.data_dir.join("occupancy_calibration.json");
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

        // Save pairs
        let path = self.data_dir.join("preference_pairs.jsonl");
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
        let cal_json = serde_json::to_string(&self.calibration)?;
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

    /// Save current calibration to disk.
    fn save_calibration(&self) -> Result<()> {
        let path = self.data_dir.join("calibration.json");
        std::fs::write(&path, serde_json::to_string_pretty(&self.calibration)?)?;
        Ok(())
    }

    /// Save all samples to disk.
    pub fn save_samples(&self) -> Result<()> {
        let path = self.data_dir.join("samples.json");
        std::fs::write(&path, serde_json::to_string_pretty(&self.samples)?)?;
        eprintln!("  Saved {} samples to {}", self.samples.len(), path.display());
        Ok(())
    }

    /// Load samples from disk.
    pub fn load_samples(&mut self) -> Result<()> {
        let path = self.data_dir.join("samples.json");
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
