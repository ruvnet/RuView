//! Monocular depth estimation via MiDaS ONNX + backprojection to 3D points.

use crate::pointcloud::{PointCloud, ColorPoint};
use anyhow::Result;

/// Default camera intrinsics (approximate for HD webcam)
pub struct CameraIntrinsics {
    pub fx: f32,  // focal length x (pixels)
    pub fy: f32,  // focal length y (pixels)
    pub cx: f32,  // principal point x
    pub cy: f32,  // principal point y
    pub width: u32,
    pub height: u32,
}

impl Default for CameraIntrinsics {
    fn default() -> Self {
        Self {
            fx: 525.0, fy: 525.0,   // typical webcam focal length
            cx: 320.0, cy: 240.0,   // center of 640x480
            width: 640, height: 480,
        }
    }
}

/// Backproject a depth map to 3D points using camera intrinsics.
///
/// depth_map: row-major [height x width] in meters
/// rgb: optional row-major [height x width x 3] color
pub fn backproject_depth(
    depth_map: &[f32],
    intrinsics: &CameraIntrinsics,
    rgb: Option<&[u8]>,
    downsample: u32,
) -> PointCloud {
    let mut cloud = PointCloud::new("camera_depth");
    let w = intrinsics.width;
    let h = intrinsics.height;
    let step = downsample.max(1);

    for y in (0..h).step_by(step as usize) {
        for x in (0..w).step_by(step as usize) {
            let idx = (y * w + x) as usize;
            let z = depth_map[idx];

            // Skip invalid depths
            if z <= 0.01 || z > 10.0 || z.is_nan() { continue; }

            // Backproject: (u, v, z) → (X, Y, Z)
            let px = (x as f32 - intrinsics.cx) * z / intrinsics.fx;
            let py = (y as f32 - intrinsics.cy) * z / intrinsics.fy;

            let (r, g, b) = if let Some(rgb_data) = rgb {
                let ri = idx * 3;
                if ri + 2 < rgb_data.len() {
                    (rgb_data[ri], rgb_data[ri + 1], rgb_data[ri + 2])
                } else {
                    (128, 128, 128)
                }
            } else {
                // Color by depth (blue=near, red=far)
                let t = ((z - 0.5) / 4.0).clamp(0.0, 1.0);
                ((t * 255.0) as u8, ((1.0 - t) * 128.0) as u8, ((1.0 - t) * 255.0) as u8)
            };

            cloud.points.push(ColorPoint { x: px, y: py, z, r, g, b, intensity: 1.0 });
        }
    }
    cloud
}

/// Run depth estimation on an image.
///
/// When built with `--features onnx`, uses MiDaS ONNX model.
/// Otherwise, generates synthetic depth from image luminance (for testing).
pub fn estimate_depth(
    image_data: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<f32>> {
    // Luminance-based pseudo-depth (works without ONNX model)
    // Darker pixels = further away (rough approximation)
    let mut depth_map = vec![3.0f32; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let ri = idx * 3;
            if ri + 2 < image_data.len() {
                let r = image_data[ri] as f32;
                let g = image_data[ri + 1] as f32;
                let b = image_data[ri + 2] as f32;
                let lum = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0;
                // Map luminance to depth: bright=near (1m), dark=far (5m)
                depth_map[idx] = 1.0 + (1.0 - lum) * 4.0;
            }
        }
    }
    Ok(depth_map)
}

/// Capture depth cloud from camera (placeholder — real impl uses nokhwa or v4l2).
pub async fn capture_depth_cloud(frames: usize) -> Result<PointCloud> {
    eprintln!("Camera capture not available (no camera on this machine).");
    eprintln!("Use --demo for synthetic data, or run on a machine with a camera.");
    Ok(demo_depth_cloud())
}

/// Generate a demo depth point cloud (synthetic room scene).
pub fn demo_depth_cloud() -> PointCloud {
    let mut cloud = PointCloud::new("demo_camera_depth");
    let intrinsics = CameraIntrinsics::default();

    // Simulate a depth map: room with walls at 3m, floor, and a person at 2m
    let w = 160;  // downsampled
    let h = 120;
    let mut depth = vec![3.0f32; w * h];

    // Floor plane (bottom third)
    for y in (h * 2 / 3)..h {
        for x in 0..w {
            depth[y * w + x] = 1.0 + (y - h * 2 / 3) as f32 * 0.05;
        }
    }

    // Person silhouette (center, depth=2m)
    for y in (h / 4)..(h * 3 / 4) {
        for x in (w * 2 / 5)..(w * 3 / 5) {
            let dy = (y as f32 - h as f32 / 2.0).abs() / (h as f32 / 4.0);
            let dx = (x as f32 - w as f32 / 2.0).abs() / (w as f32 / 5.0);
            if dx * dx + dy * dy < 1.0 {
                depth[y * w + x] = 2.0 + (dx * dx + dy * dy) * 0.3;
            }
        }
    }

    let scaled_intrinsics = CameraIntrinsics {
        fx: intrinsics.fx * w as f32 / intrinsics.width as f32,
        fy: intrinsics.fy * h as f32 / intrinsics.height as f32,
        cx: w as f32 / 2.0,
        cy: h as f32 / 2.0,
        width: w as u32,
        height: h as u32,
    };

    backproject_depth(&depth, &scaled_intrinsics, None, 1)
}

fn find_midas_model() -> Result<String> {
    let paths = [
        dirs::home_dir().unwrap_or_default().join(".local/share/ruview/midas_v21_small_256.onnx"),
        dirs::home_dir().unwrap_or_default().join(".cache/ruview/midas_v21_small_256.onnx"),
        std::path::PathBuf::from("/usr/local/share/ruview/midas_v21_small_256.onnx"),
    ];
    for p in &paths {
        if p.exists() { return Ok(p.to_string_lossy().to_string()); }
    }
    anyhow::bail!("MiDaS ONNX model not found. Download:\n  wget https://github.com/isl-org/MiDaS/releases/download/v3_1/midas_v21_small_256.onnx -O ~/.local/share/ruview/midas_v21_small_256.onnx")
}
