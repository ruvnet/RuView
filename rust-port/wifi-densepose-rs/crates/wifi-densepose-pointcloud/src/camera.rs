//! Camera capture — cross-platform frame grabber.
//!
//! Linux: direct V4L2 via `v4l` crate (no subprocess, no orphans)
//! macOS: ffmpeg -f avfoundation (subprocess)
//! Fallback: ffmpeg subprocess on all platforms
#![allow(dead_code)]

use anyhow::{bail, Result};
use std::path::PathBuf;
use std::process::Command;

/// Captured frame with raw RGB data.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
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
/// On Linux: uses direct V4L2 (no subprocess, no orphans).
/// On macOS: uses ffmpeg subprocess.
pub fn capture_frame(config: &CameraConfig) -> Result<Frame> {
    // Linux: direct V4L2 (preferred — no subprocess)
    #[cfg(target_os = "linux")]
    {
        match capture_v4l2_direct(config) {
            Ok(frame) => return Ok(frame),
            Err(e) => eprintln!("[camera] V4L2 direct failed: {e}, falling back to ffmpeg"),
        }
    }

    // Fallback: ffmpeg subprocess (with timeout to prevent orphans)
    let tmp = tmp_path();
    if let Ok(frame) = capture_ffmpeg_safe(config, &tmp) {
        return Ok(frame);
    }

    // macOS: screencapture
    #[cfg(target_os = "macos")]
    if let Ok(frame) = capture_macos(config, &tmp) {
        return Ok(frame);
    }

    bail!("No camera backend available")
}

// ============================================================
// Linux: Direct V4L2 capture (no subprocess, no orphans)
// ============================================================

#[cfg(target_os = "linux")]
fn capture_v4l2_direct(config: &CameraConfig) -> Result<Frame> {
    use v4l::buffer::Type;
    use v4l::io::mmap::Stream;
    use v4l::io::traits::CaptureStream;
    use v4l::video::Capture;
    use v4l::{Device, FourCC};

    let device_path = format!("/dev/video{}", config.device_index);
    if !std::path::Path::new(&device_path).exists() {
        bail!("no camera at {device_path}");
    }

    let dev = Device::with_path(&device_path)?;

    // Try MJPG first (most webcams support it), fall back to YUYV
    let mut fmt = dev.format()?;
    fmt.width = config.width;
    fmt.height = config.height;
    fmt.fourcc = FourCC::new(b"MJPG");
    let use_mjpg = dev.set_format(&fmt).is_ok();

    if !use_mjpg {
        fmt.fourcc = FourCC::new(b"YUYV");
        dev.set_format(&fmt)?;
    }

    let fmt = dev.format()?;
    let actual_w = fmt.width;
    let actual_h = fmt.height;

    // Stream one frame via mmap
    let mut stream = Stream::with_buffers(&dev, Type::VideoCapture, 2)?;
    let (buf, _meta) = stream.next()?;

    let rgb = if use_mjpg {
        decode_mjpeg_to_rgb(buf, actual_w, actual_h)?
    } else {
        yuyv_to_rgb(buf, actual_w, actual_h)
    };

    // Stream is dropped here — device released cleanly, no orphan process

    Ok(Frame {
        width: actual_w,
        height: actual_h,
        rgb,
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
    })
}

#[cfg(target_os = "linux")]
fn decode_mjpeg_to_rgb(data: &[u8], _w: u32, _h: u32) -> Result<Vec<u8>> {
    // Use a minimal JPEG decoder
    let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(data));
    let pixels = decoder.decode()?;
    let info = decoder.info().ok_or_else(|| anyhow::anyhow!("no JPEG info"))?;

    if info.pixel_format == jpeg_decoder::PixelFormat::RGB24 {
        Ok(pixels)
    } else if info.pixel_format == jpeg_decoder::PixelFormat::L8 {
        // Grayscale → RGB
        Ok(pixels.iter().flat_map(|&g| [g, g, g]).collect())
    } else {
        bail!("unsupported JPEG pixel format: {:?}", info.pixel_format)
    }
}

#[cfg(target_os = "linux")]
fn yuyv_to_rgb(data: &[u8], w: u32, h: u32) -> Vec<u8> {
    let pixel_count = (w * h) as usize;
    let mut rgb = Vec::with_capacity(pixel_count * 3);

    for chunk in data.chunks(4) {
        if chunk.len() < 4 { break; }
        let (y0, u, y1, v) = (chunk[0] as f32, chunk[1] as f32, chunk[2] as f32, chunk[3] as f32);

        for y in [y0, y1] {
            let r = (y + 1.402 * (v - 128.0)).clamp(0.0, 255.0) as u8;
            let g = (y - 0.344136 * (u - 128.0) - 0.714136 * (v - 128.0)).clamp(0.0, 255.0) as u8;
            let b = (y + 1.772 * (u - 128.0)).clamp(0.0, 255.0) as u8;
            rgb.extend_from_slice(&[r, g, b]);
        }
    }
    rgb.truncate(pixel_count * 3);
    rgb
}

// ============================================================
// Fallback: ffmpeg subprocess (with timeout + cleanup)
// ============================================================

fn capture_ffmpeg_safe(config: &CameraConfig, tmp: &PathBuf) -> Result<Frame> {
    let input = if cfg!(target_os = "macos") {
        format!("{}:none", config.device_index)
    } else {
        format!("/dev/video{}", config.device_index)
    };
    let format = if cfg!(target_os = "macos") { "avfoundation" } else { "v4l2" };

    // Spawn with timeout to prevent orphans
    let mut child = Command::new("ffmpeg")
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
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // Wait with 10-second timeout
    let timeout = std::time::Duration::from_secs(10);
    let start = std::time::Instant::now();

    loop {
        match child.try_wait()? {
            Some(status) => {
                if !status.success() {
                    bail!("ffmpeg capture failed (exit {})", status.code().unwrap_or(-1));
                }
                break;
            }
            None => {
                if start.elapsed() > timeout {
                    // Kill the stuck process — this is the orphan prevention
                    let _ = child.kill();
                    let _ = child.wait();
                    bail!("ffmpeg capture timed out after 10s — killed");
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }

    let rgb = std::fs::read(tmp)?;
    let expected = (config.width * config.height * 3) as usize;
    let _ = std::fs::remove_file(tmp);

    if rgb.len() < expected {
        bail!("frame too small: {} bytes, expected {}", rgb.len(), expected);
    }

    Ok(Frame {
        width: config.width,
        height: config.height,
        rgb: rgb[..expected].to_vec(),
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
    })
}

/// macOS: capture via swift/screencapture.
#[cfg(target_os = "macos")]
fn capture_macos(config: &CameraConfig, tmp: &PathBuf) -> Result<Frame> {
    let jpg_path = tmp.with_extension("jpg");
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
        let data = std::fs::read(&jpg_path)?;
        let _ = std::fs::remove_file(&jpg_path);
        return Ok(Frame {
            width: config.width,
            height: config.height,
            rgb: data,
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        });
    }
    bail!("macOS camera capture requires GUI session with camera permission")
}

fn tmp_path() -> PathBuf {
    std::env::temp_dir().join(format!("ruview-frame-{}.raw", std::process::id()))
}

/// Check if a camera is available.
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
