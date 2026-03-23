//! Standalone Ghost Hunter WASM module for ESP32-S3.
//!
//! Compiles to a self-contained .wasm binary that runs the
//! GhostHunterDetector as a hot-loadable Tier 3 edge module.
//!
//! Build:
//!   cargo build --bin ghost_hunter --target wasm32-unknown-unknown --release
//!
//! The resulting .wasm file can be uploaded to an ESP32 running the
//! CSI firmware via the HTTP /api/wasm/upload endpoint.

#![cfg_attr(target_arch = "wasm32", no_std)]
#![cfg_attr(target_arch = "wasm32", no_main)]

// The lib crate already provides the panic handler for wasm32.
// We use its host API bindings and the GhostHunterDetector.

#[cfg(target_arch = "wasm32")]
use wifi_densepose_wasm_edge::{
    host_get_phase, host_get_amplitude, host_get_variance,
    host_get_presence, host_get_motion_energy,
    host_emit_event, host_log,
    exo_ghost_hunter::GhostHunterDetector,
};

#[cfg(target_arch = "wasm32")]
static mut DETECTOR: GhostHunterDetector = GhostHunterDetector::new();

// ── Helpers ────────────────────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
fn log_str(s: &str) {
    unsafe { host_log(s.as_ptr() as i32, s.len() as i32) }
}

#[cfg(target_arch = "wasm32")]
fn emit(event_type: i32, value: f32) {
    unsafe { host_emit_event(event_type, value) }
}

// ── WASM entry points (exported to host) ───────────────────────────────────

/// Called once when the module is loaded onto the ESP32.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn on_init() {
    log_str("ghost-hunter v1.0: anomaly detector active");
}

/// Called per CSI frame (~20 Hz) by the WASM3 runtime.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn on_frame(n_subcarriers: i32) {
    let n_sc = if n_subcarriers < 0 { 0 } else { n_subcarriers as usize };
    let max_sc = if n_sc > 32 { 32 } else { n_sc };
    if max_sc < 8 {
        return;
    }

    // Read CSI data from host
    let mut phases = [0.0f32; 32];
    let mut amplitudes = [0.0f32; 32];
    let mut variances = [0.0f32; 32];

    for i in 0..max_sc {
        unsafe {
            phases[i] = host_get_phase(i as i32);
            amplitudes[i] = host_get_amplitude(i as i32);
            variances[i] = host_get_variance(i as i32);
        }
    }

    let presence = unsafe { host_get_presence() };
    let motion_energy = unsafe { host_get_motion_energy() };

    let detector = unsafe { &mut *core::ptr::addr_of_mut!(DETECTOR) };
    let events = detector.process_frame(
        &phases[..max_sc],
        &amplitudes[..max_sc],
        &variances[..max_sc],
        presence,
        motion_energy,
    );

    for &(event_id, value) in events {
        emit(event_id, value);
    }
}

/// Called at configurable interval (default 1 second).
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn on_timer() {
    let detector = unsafe { &*core::ptr::addr_of!(DETECTOR) };
    let energy = detector.anomaly_energy();
    if energy > 0.001 {
        emit(650, energy);
    }
}

// ── Non-WASM main (for native host builds) ─────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    println!("Ghost Hunter WASM module");
    println!("Build: cargo build --bin ghost_hunter --target wasm32-unknown-unknown --release");
    println!("Upload: POST the .wasm to http://<esp32-ip>/api/wasm/upload");
}
