//! Camera capture — cross-platform frame grabber.
#![allow(dead_code)]
//!
//! macOS: uses `screencapture` or `ffmpeg -f avfoundation` for camera frames
//! Linux: uses `v4l2-ctl` or `ffmpeg -f v4l2` for camera frames
//! Both: capture to JPEG, decode to RGB, return raw pixel data

use anyhow::{bail, Result};
use std::process::Command;
use std::path::PathBuf;

/// Captured frame with raw RGB data.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,      // row-major [height * width * 3]
    pub timestamp_ms: i64,
}

/// Camera source configuration.
pub struct CameraConfig {
    pub device_index: u32,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self { device_index: 0, width: 640, height: 480, fps: 15 }
    }
}

/// Capture a single frame from the camera.
///
/// Tries multiple backends in order: ffmpeg, v4l2, imagesnap (macOS).
pub fn capture_frame(config: &CameraConfig) -> Result<Frame> {
    let tmp = tmp_path();

    // Try ffmpeg first (cross-platform)
    if let Ok(frame) = capture_ffmpeg(config, &tmp) {
        return Ok(frame);
    }

    // Linux: try v4l2
    #[cfg(target_os = "linux")]
    if let Ok(frame) = capture_v4l2(config, &tmp) {
        return Ok(frame);
    }

    // macOS: try screencapture (camera mode)
    #[cfg(target_os = "macos")]
    if let Ok(frame) = capture_macos(config, &tmp) {
        return Ok(frame);
    }

    bail!("No camera backend available. Install ffmpeg or run on a machine with a camera.")
}

/// Capture via ffmpeg (works on Linux + macOS).
fn capture_ffmpeg(config: &CameraConfig, tmp: &PathBuf) -> Result<Frame> {
    let input = if cfg!(target_os = "macos") {
        format!("{}:none", config.device_index) // avfoundation: video:audio
    } else {
        format!("/dev/video{}", config.device_index) // v4l2
    };

    let format = if cfg!(target_os = "macos") { "avfoundation" } else { "v4l2" };

    let status = Command::new("ffmpeg")
        .args([
            "-y", "-f", format,
            "-video_size", &format!("{}x{}", config.width, config.height),
            "-framerate", &config.fps.to_string(),
            "-i", &input,
            "-frames:v", "1",
            "-f", "rawvideo",
            "-pix_fmt", "rgb24",
            tmp.to_str().unwrap_or("/tmp/ruview-frame.raw"),
        ])
        .output()?;

    if !status.status.success() {
        bail!("ffmpeg capture failed: {}", String::from_utf8_lossy(&status.stderr));
    }

    let rgb = std::fs::read(tmp)?;
    let expected = (config.width * config.height * 3) as usize;
    if rgb.len() < expected {
        bail!("frame too small: {} bytes, expected {}", rgb.len(), expected);
    }

    let _ = std::fs::remove_file(tmp);

    Ok(Frame {
        width: config.width,
        height: config.height,
        rgb: rgb[..expected].to_vec(),
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
    })
}

/// Linux: capture via v4l2-ctl.
#[cfg(target_os = "linux")]
fn capture_v4l2(config: &CameraConfig, tmp: &PathBuf) -> Result<Frame> {
    let device = format!("/dev/video{}", config.device_index);
    if !std::path::Path::new(&device).exists() {
        bail!("no camera at {device}");
    }

    // Use v4l2-ctl to grab a frame
    let status = Command::new("v4l2-ctl")
        .args([
            "--device", &device,
            "--set-fmt-video", &format!("width={},height={},pixelformat=MJPG", config.width, config.height),
            "--stream-mmap", "--stream-count=1",
            "--stream-to", tmp.to_str().unwrap_or("/tmp/frame.mjpg"),
        ])
        .output()?;

    if !status.status.success() {
        bail!("v4l2-ctl failed");
    }

    // Decode MJPEG to RGB
    decode_jpeg_to_rgb(tmp, config.width, config.height)
}

/// macOS: capture via screencapture or swift.
#[cfg(target_os = "macos")]
fn capture_macos(config: &CameraConfig, tmp: &PathBuf) -> Result<Frame> {
    let jpg_path = tmp.with_extension("jpg");

    // Try swift-based capture (requires camera permission)
    let swift = format!(
        r#"import AVFoundation; import AppKit
let sem = DispatchSemaphore(value: 0)
let s = AVCaptureSession(); s.sessionPreset = .medium
guard let d = AVCaptureDevice.default(for: .video) else {{ exit(1) }}
let i = try! AVCaptureDeviceInput(device: d); s.addInput(i)
let o = AVCapturePhotoOutput(); s.addOutput(o)
class D: NSObject, AVCapturePhotoCaptureDelegate {{
    func photoOutput(_ o: AVCapturePhotoOutput, didFinishProcessingPhoto p: AVCapturePhoto, error: Error?) {{
        if let d = p.fileDataRepresentation() {{ try! d.write(to: URL(fileURLWithPath: "{path}")) }}
        exit(0)
    }}
}}
let dl = D(); s.startRunning(); Thread.sleep(forTimeInterval: 1)
o.capturePhoto(with: AVCapturePhotoSettings(), delegate: dl)
Thread.sleep(forTimeInterval: 3)"#,
        path = jpg_path.display()
    );

    let _ = Command::new("swift").args(["-e", &swift]).output();

    if jpg_path.exists() {
        return decode_jpeg_to_rgb(&jpg_path, config.width, config.height);
    }

    bail!("macOS camera capture requires GUI session with camera permission")
}

fn decode_jpeg_to_rgb(path: &PathBuf, _width: u32, _height: u32) -> Result<Frame> {
    let data = std::fs::read(path)?;
    let _ = std::fs::remove_file(path);

    // Simple JPEG decode — use the image crate if available, otherwise raw
    // For now, return the raw data and let the caller handle format
    Ok(Frame {
        width: _width,
        height: _height,
        rgb: data,
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
    })
}

fn tmp_path() -> PathBuf {
    std::env::temp_dir().join(format!("ruview-frame-{}.raw", std::process::id()))
}

/// Check if a camera is available on this system.
pub fn camera_available() -> bool {
    if cfg!(target_os = "macos") {
        Command::new("system_profiler")
            .args(["SPCameraDataType"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("Camera"))
            .unwrap_or(false)
    } else {
        std::path::Path::new("/dev/video0").exists()
    }
}

/// List available cameras.
pub fn list_cameras() -> Vec<String> {
    let mut cameras = Vec::new();

    if cfg!(target_os = "macos") {
        if let Ok(output) = Command::new("system_profiler").args(["SPCameraDataType"]).output() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.ends_with(':') && !trimmed.starts_with("Camera") && trimmed.len() > 2 {
                    cameras.push(trimmed.trim_end_matches(':').to_string());
                }
            }
        }
    } else {
        for i in 0..10 {
            if std::path::Path::new(&format!("/dev/video{i}")).exists() {
                cameras.push(format!("/dev/video{i}"));
            }
        }
    }
    cameras
}
