//! Monocular depth estimation via MiDaS ONNX + backprojection to 3D points.
#![allow(dead_code)]

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
/// Tries MiDaS GPU server (127.0.0.1:9885) first, falls back to luminance+edges.
pub fn estimate_depth(
    image_data: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<f32>> {
    // Try MiDaS GPU server
    if let Ok(depth) = estimate_depth_midas_server(image_data, width, height) {
        return Ok(depth);
    }

    // Fallback: luminance + edge-based pseudo-depth
    let w = width as usize;
    let h = height as usize;
    let mut lum = vec![0.0f32; w * h];
    for i in 0..w * h {
        let ri = i * 3;
        if ri + 2 < image_data.len() {
            lum[i] = (0.299 * image_data[ri] as f32
                    + 0.587 * image_data[ri + 1] as f32
                    + 0.114 * image_data[ri + 2] as f32) / 255.0;
        }
    }
    let mut edges = vec![0.0f32; w * h];
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let gx = -lum[(y-1)*w+x-1] + lum[(y-1)*w+x+1]
                   - 2.0*lum[y*w+x-1] + 2.0*lum[y*w+x+1]
                   - lum[(y+1)*w+x-1] + lum[(y+1)*w+x+1];
            let gy = -lum[(y-1)*w+x-1] - 2.0*lum[(y-1)*w+x] - lum[(y-1)*w+x+1]
                   + lum[(y+1)*w+x-1] + 2.0*lum[(y+1)*w+x] + lum[(y+1)*w+x+1];
            edges[y * w + x] = (gx * gx + gy * gy).sqrt().min(1.0);
        }
    }
    let mut depth_map = vec![3.0f32; w * h];
    for i in 0..w * h {
        let base = 1.0 + (1.0 - lum[i]) * 3.5;
        let edge_boost = edges[i] * 1.5;
        depth_map[i] = (base - edge_boost).max(0.3);
    }
    Ok(depth_map)
}

/// Call MiDaS depth server running on GPU (127.0.0.1:9885).
fn estimate_depth_midas_server(rgb: &[u8], width: u32, height: u32) -> Result<Vec<f32>> {
    let expected = (width * height * 3) as usize;
    if rgb.len() < expected { anyhow::bail!("rgb too small"); }

    // Send RGB as JSON array to depth server
    let rgb_list: Vec<u8> = rgb[..expected].to_vec();
    let body = serde_json::json!({
        "width": width,
        "height": height,
        "rgb": rgb_list,
    });
    let body_bytes = serde_json::to_vec(&body)?;

    let client = std::net::TcpStream::connect_timeout(
        &"127.0.0.1:9885".parse()?, std::time::Duration::from_millis(500)
    )?;
    client.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    client.set_write_timeout(Some(std::time::Duration::from_secs(2)))?;

    use std::io::{Read, Write};
    let mut stream = client;
    let req = format!(
        "POST /depth HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        body_bytes.len()
    );
    stream.write_all(req.as_bytes())?;
    stream.write_all(&body_bytes)?;

    // Read response
    let mut resp = Vec::new();
    stream.read_to_end(&mut resp)?;

    // Skip HTTP headers
    let body_start = resp.windows(4).position(|w| w == b"\r\n\r\n")
        .map(|p| p + 4).unwrap_or(0);
    let depth_bytes = &resp[body_start..];

    let n = (width * height) as usize;
    if depth_bytes.len() < n * 4 { anyhow::bail!("depth response too small"); }

    let depth: Vec<f32> = depth_bytes[..n * 4].chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    Ok(depth)
}

/// Capture depth cloud from camera (placeholder — real impl uses nokhwa or v4l2).
pub async fn capture_depth_cloud(_frames: usize) -> Result<PointCloud> {
    eprintln!("Camera capture not available (no camera on this machine).");
    eprintln!("Use --demo for synthetic data, or run on a machine with a camera.");
    Ok(demo_depth_cloud())
}

/// Generate a demo depth point cloud (synthetic room scene).
pub fn demo_depth_cloud() -> PointCloud {
    let _cloud = PointCloud::new("demo_camera_depth");
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
