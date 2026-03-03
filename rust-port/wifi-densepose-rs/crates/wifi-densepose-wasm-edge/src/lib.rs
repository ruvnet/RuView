//! WiFi-DensePose WASM Edge — Hot-loadable sensing algorithms for ESP32-S3.
//!
//! ADR-040 Tier 3: Compiled to `wasm32-unknown-unknown`, these modules run
//! inside the WASM3 interpreter on the ESP32-S3 after Tier 2 DSP completes.
//!
//! # Host API (imported from "csi" namespace)
//!
//! The ESP32 firmware exposes CSI data through imported functions:
//! - `csi_get_phase(subcarrier) -> f32`
//! - `csi_get_amplitude(subcarrier) -> f32`
//! - `csi_get_variance(subcarrier) -> f32`
//! - `csi_get_bpm_breathing() -> f32`
//! - `csi_get_bpm_heartrate() -> f32`
//! - `csi_get_presence() -> i32`
//! - `csi_get_motion_energy() -> f32`
//! - `csi_get_n_persons() -> i32`
//! - `csi_get_timestamp() -> i32`
//! - `csi_emit_event(event_type: i32, value: f32)`
//! - `csi_log(ptr: i32, len: i32)`
//! - `csi_get_phase_history(buf_ptr: i32, max_len: i32) -> i32`
//!
//! # Module lifecycle (exported to host)
//!
//! - `on_init()` — called once when module is loaded
//! - `on_frame(n_subcarriers: i32)` — called per CSI frame (~20 Hz)
//! - `on_timer()` — called at configurable interval (default 1 s)
//!
//! # Build
//!
//! ```bash
//! cargo build -p wifi-densepose-wasm-edge --target wasm32-unknown-unknown --release
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::missing_safety_doc)]
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

pub mod gesture;
pub mod coherence;
pub mod adversarial;
pub mod rvf;
pub mod occupancy;
pub mod vital_trend;
pub mod intrusion;

// ── Host API FFI bindings ────────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
extern "C" {
    #[link_name = "csi_get_phase"]
    pub fn host_get_phase(subcarrier: i32) -> f32;

    #[link_name = "csi_get_amplitude"]
    pub fn host_get_amplitude(subcarrier: i32) -> f32;

    #[link_name = "csi_get_variance"]
    pub fn host_get_variance(subcarrier: i32) -> f32;

    #[link_name = "csi_get_bpm_breathing"]
    pub fn host_get_bpm_breathing() -> f32;

    #[link_name = "csi_get_bpm_heartrate"]
    pub fn host_get_bpm_heartrate() -> f32;

    #[link_name = "csi_get_presence"]
    pub fn host_get_presence() -> i32;

    #[link_name = "csi_get_motion_energy"]
    pub fn host_get_motion_energy() -> f32;

    #[link_name = "csi_get_n_persons"]
    pub fn host_get_n_persons() -> i32;

    #[link_name = "csi_get_timestamp"]
    pub fn host_get_timestamp() -> i32;

    #[link_name = "csi_emit_event"]
    pub fn host_emit_event(event_type: i32, value: f32);

    #[link_name = "csi_log"]
    pub fn host_log(ptr: i32, len: i32);

    #[link_name = "csi_get_phase_history"]
    pub fn host_get_phase_history(buf_ptr: i32, max_len: i32) -> i32;
}

// ── Convenience wrappers ─────────────────────────────────────────────────────

/// Event type constants emitted via `csi_emit_event`.
///
/// Registry (ADR-041):
///   0-99:   Core (gesture, coherence, anomaly, custom)
///   100-199: Medical (vital trends, apnea, brady/tachycardia)
///   200-299: Security (intrusion, tamper, perimeter)
///   300-399: Smart Building (occupancy zones, HVAC, lighting)
///   400-499: Retail (foot traffic, dwell time)
///   500-599: Industrial (vibration, proximity)
///   600-699: Exotic (weather, wildlife, paranormal)
pub mod event_types {
    // Core (0-99)
    pub const GESTURE_DETECTED: i32 = 1;
    pub const COHERENCE_SCORE: i32 = 2;
    pub const ANOMALY_DETECTED: i32 = 3;
    pub const CUSTOM_METRIC: i32 = 10;

    // Medical (100-199) — see vital_trend module
    pub const VITAL_TREND: i32 = 100;
    pub const BRADYPNEA: i32 = 101;
    pub const TACHYPNEA: i32 = 102;
    pub const BRADYCARDIA: i32 = 103;
    pub const TACHYCARDIA: i32 = 104;
    pub const APNEA: i32 = 105;

    // Security (200-299) — see intrusion module
    pub const INTRUSION_ALERT: i32 = 200;
    pub const INTRUSION_ZONE: i32 = 201;

    // Smart Building (300-399) — see occupancy module
    pub const ZONE_OCCUPIED: i32 = 300;
    pub const ZONE_COUNT: i32 = 301;
    pub const ZONE_TRANSITION: i32 = 302;
}

/// Log a message string to the ESP32 console (via host_log import).
#[cfg(target_arch = "wasm32")]
pub fn log_msg(msg: &str) {
    unsafe {
        host_log(msg.as_ptr() as i32, msg.len() as i32);
    }
}

/// Emit a typed event to the host output packet.
#[cfg(target_arch = "wasm32")]
pub fn emit(event_type: i32, value: f32) {
    unsafe {
        host_emit_event(event_type, value);
    }
}

// ── Panic handler (required for no_std WASM) ─────────────────────────────────

#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

// ── Default module entry points ──────────────────────────────────────────────
//
// Individual modules (gesture, coherence, adversarial) can define their own
// on_init/on_frame/on_timer.  This default implementation demonstrates the
// combined pipeline: gesture detection + coherence monitoring + anomaly check.

#[cfg(target_arch = "wasm32")]
static mut STATE: CombinedState = CombinedState::new();

struct CombinedState {
    gesture: gesture::GestureDetector,
    coherence: coherence::CoherenceMonitor,
    adversarial: adversarial::AnomalyDetector,
    frame_count: u32,
}

impl CombinedState {
    const fn new() -> Self {
        Self {
            gesture: gesture::GestureDetector::new(),
            coherence: coherence::CoherenceMonitor::new(),
            adversarial: adversarial::AnomalyDetector::new(),
            frame_count: 0,
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn on_init() {
    log_msg("wasm-edge: combined pipeline init");
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn on_frame(n_subcarriers: i32) {
    let n_sc = n_subcarriers as usize;
    let state = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
    state.frame_count += 1;

    // Collect phase/amplitude for top subcarriers (max 32).
    let max_sc = if n_sc > 32 { 32 } else { n_sc };
    let mut phases = [0.0f32; 32];
    let mut amps = [0.0f32; 32];

    for i in 0..max_sc {
        unsafe {
            phases[i] = host_get_phase(i as i32);
            amps[i] = host_get_amplitude(i as i32);
        }
    }

    // 1. Gesture detection (DTW template matching).
    if let Some(gesture_id) = state.gesture.process_frame(&phases[..max_sc]) {
        emit(event_types::GESTURE_DETECTED, gesture_id as f32);
    }

    // 2. Coherence monitoring (phase phasor).
    let coh_score = state.coherence.process_frame(&phases[..max_sc]);
    if state.frame_count % 20 == 0 {
        emit(event_types::COHERENCE_SCORE, coh_score);
    }

    // 3. Anomaly detection (signal consistency check).
    if state.adversarial.process_frame(&phases[..max_sc], &amps[..max_sc]) {
        emit(event_types::ANOMALY_DETECTED, 1.0);
    }
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn on_timer() {
    // Periodic summary.
    let state = unsafe { &*core::ptr::addr_of!(STATE) };
    let motion = unsafe { host_get_motion_energy() };
    emit(event_types::CUSTOM_METRIC, motion);

    if state.frame_count % 100 == 0 {
        log_msg("wasm-edge: heartbeat");
    }
}
