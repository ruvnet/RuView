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

// ── ADR-040 flagship modules ─────────────────────────────────────────────────

pub mod gesture;
pub mod coherence;
pub mod adversarial;
pub mod rvf;
pub mod occupancy;
pub mod vital_trend;
pub mod intrusion;

// ── Category 1: Medical & Health (ADR-041, event IDs 100-199) ───────────────
pub mod med_sleep_apnea;
pub mod med_cardiac_arrhythmia;
pub mod med_respiratory_distress;
pub mod med_gait_analysis;
pub mod med_seizure_detect;

// ── Category 2: Security & Safety (ADR-041, event IDs 200-299) ──────────────
pub mod sec_perimeter_breach;
pub mod sec_weapon_detect;
pub mod sec_tailgating;
pub mod sec_loitering;
pub mod sec_panic_motion;

// ── Category 3: Smart Building (ADR-041, event IDs 300-399) ─────────────────
pub mod bld_hvac_presence;
pub mod bld_lighting_zones;
pub mod bld_elevator_count;
pub mod bld_meeting_room;
pub mod bld_energy_audit;

// ── Category 4: Retail & Hospitality (ADR-041, event IDs 400-499) ───────────
pub mod ret_queue_length;
pub mod ret_dwell_heatmap;
pub mod ret_customer_flow;
pub mod ret_table_turnover;
pub mod ret_shelf_engagement;

// ── Category 5: Industrial & Specialized (ADR-041, event IDs 500-599) ───────
pub mod ind_forklift_proximity;
pub mod ind_confined_space;
pub mod ind_clean_room;
pub mod ind_livestock_monitor;
pub mod ind_structural_vibration;

// ── Shared vendor utilities (ADR-041) ────────────────────────────────────────

pub mod vendor_common;

// ── Vendor-integrated modules (ADR-041 Category 7) ──────────────────────────
//
// 24 modules organised into 7 sub-categories.  Each module file lives in
// `src/` and follows the same pattern as the flagship modules: a no_std
// struct with `const fn new()` and a `process_frame`-style entry point.
//
// Signal Intelligence (wdp-sig-*, event IDs 680-727)
pub mod sig_coherence_gate;
pub mod sig_flash_attention;
pub mod sig_temporal_compress;
pub mod sig_sparse_recovery;
pub mod sig_mincut_person_match;
pub mod sig_optimal_transport;
//
// Adaptive Learning (wdp-lrn-*, event IDs 730-748)
pub mod lrn_dtw_gesture_learn;
pub mod lrn_anomaly_attractor;
pub mod lrn_meta_adapt;
pub mod lrn_ewc_lifelong;
//
// Spatial Reasoning (wdp-spt-*, event IDs 760-773)
pub mod spt_pagerank_influence;
pub mod spt_micro_hnsw;
pub mod spt_spiking_tracker;
//
// Temporal Analysis (wdp-tmp-*, event IDs 790-803)
pub mod tmp_pattern_sequence;
pub mod tmp_temporal_logic_guard;
pub mod tmp_goap_autonomy;
//
// AI Security (wdp-ais-*, event IDs 820-828)
pub mod ais_prompt_shield;
pub mod ais_behavioral_profiler;
//
// Quantum-Inspired (wdp-qnt-*, event IDs 850-857)
pub mod qnt_quantum_coherence;
pub mod qnt_interference_search;
//
// Autonomous Systems (wdp-aut-*, event IDs 880-888)
pub mod aut_psycho_symbolic;
pub mod aut_self_healing_mesh;
//
// Exotic / Research (wdp-exo-*, event IDs 600-699)
pub mod exo_time_crystal;
pub mod exo_hyperbolic_space;

// ── Category 6: Exotic & Research (ADR-041, event IDs 600-699) ──────────────
pub mod exo_dream_stage;
pub mod exo_emotion_detect;
pub mod exo_gesture_language;
pub mod exo_music_conductor;
pub mod exo_plant_growth;
pub mod exo_ghost_hunter;
pub mod exo_rain_detect;
pub mod exo_breathing_sync;

// ── Host API FFI bindings ────────────────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "csi")]
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
///   0-99:    Core (gesture, coherence, anomaly, custom)
///   100-199: Medical (vital trends, apnea, brady/tachycardia)
///   200-299: Security (intrusion, tamper, perimeter)
///   300-399: Smart Building (occupancy zones, HVAC, lighting)
///   400-499: Retail (foot traffic, dwell time)
///   500-599: Industrial (vibration, proximity)
///   600-699: Exotic (dream stage 600-603, emotion 610-613, gesture lang 620-623,
///            music conductor 630-634, time crystals 680-682, hyperbolic 685-687)
///   700-729: Vendor Signal Intelligence
///   730-759: Vendor Adaptive Learning
///   760-789: Vendor Spatial Reasoning
///   790-819: Vendor Temporal Analysis
///   820-849: Vendor AI Security
///   850-879: Vendor Quantum-Inspired
///   880-899: Vendor Autonomous Systems
pub mod event_types {
    // ── Core (0-99) ──────────────────────────────────────────────────────
    pub const GESTURE_DETECTED: i32 = 1;
    pub const COHERENCE_SCORE: i32 = 2;
    pub const ANOMALY_DETECTED: i32 = 3;
    pub const CUSTOM_METRIC: i32 = 10;

    // ── Medical (100-199) ────────────────────────────────────────────────
    pub const VITAL_TREND: i32 = 100;
    pub const BRADYPNEA: i32 = 101;
    pub const TACHYPNEA: i32 = 102;
    pub const BRADYCARDIA: i32 = 103;
    pub const TACHYCARDIA: i32 = 104;
    pub const APNEA: i32 = 105;

    // ── Security (200-299) ───────────────────────────────────────────────
    pub const INTRUSION_ALERT: i32 = 200;
    pub const INTRUSION_ZONE: i32 = 201;

    // sec_perimeter_breach (210-213)
    pub const PERIMETER_BREACH: i32 = 210;
    pub const APPROACH_DETECTED: i32 = 211;
    pub const DEPARTURE_DETECTED: i32 = 212;
    pub const SEC_ZONE_TRANSITION: i32 = 213;

    // sec_weapon_detect (220-222)
    pub const METAL_ANOMALY: i32 = 220;
    pub const WEAPON_ALERT: i32 = 221;
    pub const CALIBRATION_NEEDED: i32 = 222;

    // sec_tailgating (230-232)
    pub const TAILGATE_DETECTED: i32 = 230;
    pub const SINGLE_PASSAGE: i32 = 231;
    pub const MULTI_PASSAGE: i32 = 232;

    // sec_loitering (240-242)
    pub const LOITERING_START: i32 = 240;
    pub const LOITERING_ONGOING: i32 = 241;
    pub const LOITERING_END: i32 = 242;

    // sec_panic_motion (250-252)
    pub const PANIC_DETECTED: i32 = 250;
    pub const STRUGGLE_PATTERN: i32 = 251;
    pub const FLEEING_DETECTED: i32 = 252;

    // ── Smart Building (300-399) ─────────────────────────────────────────
    pub const ZONE_OCCUPIED: i32 = 300;
    pub const ZONE_COUNT: i32 = 301;
    pub const ZONE_TRANSITION: i32 = 302;

    // bld_hvac_presence (310-312)
    pub const HVAC_OCCUPIED: i32 = 310;
    pub const ACTIVITY_LEVEL: i32 = 311;
    pub const DEPARTURE_COUNTDOWN: i32 = 312;

    // bld_lighting_zones (320-322)
    pub const LIGHT_ON: i32 = 320;
    pub const LIGHT_DIM: i32 = 321;
    pub const LIGHT_OFF: i32 = 322;

    // bld_elevator_count (330-333)
    pub const ELEVATOR_COUNT: i32 = 330;
    pub const DOOR_OPEN: i32 = 331;
    pub const DOOR_CLOSE: i32 = 332;
    pub const OVERLOAD_WARNING: i32 = 333;

    // bld_meeting_room (340-343)
    pub const MEETING_START: i32 = 340;
    pub const MEETING_END: i32 = 341;
    pub const PEAK_HEADCOUNT: i32 = 342;
    pub const ROOM_AVAILABLE: i32 = 343;

    // bld_energy_audit (350-352)
    pub const SCHEDULE_SUMMARY: i32 = 350;
    pub const AFTER_HOURS_ALERT: i32 = 351;
    pub const UTILIZATION_RATE: i32 = 352;

    // ── Retail & Hospitality (400-499) ─────────────────────────────────────

    // ret_queue_length (400-403)
    pub const QUEUE_LENGTH: i32 = 400;
    pub const WAIT_TIME_ESTIMATE: i32 = 401;
    pub const SERVICE_RATE: i32 = 402;
    pub const QUEUE_ALERT: i32 = 403;

    // ret_dwell_heatmap (410-413)
    pub const DWELL_ZONE_UPDATE: i32 = 410;
    pub const HOT_ZONE: i32 = 411;
    pub const COLD_ZONE: i32 = 412;
    pub const SESSION_SUMMARY: i32 = 413;

    // ret_customer_flow (420-423)
    pub const INGRESS: i32 = 420;
    pub const EGRESS: i32 = 421;
    pub const NET_OCCUPANCY: i32 = 422;
    pub const HOURLY_TRAFFIC: i32 = 423;

    // ret_table_turnover (430-433)
    pub const TABLE_SEATED: i32 = 430;
    pub const TABLE_VACATED: i32 = 431;
    pub const TABLE_AVAILABLE: i32 = 432;
    pub const TURNOVER_RATE: i32 = 433;

    // ret_shelf_engagement (440-443)
    pub const SHELF_BROWSE: i32 = 440;
    pub const SHELF_CONSIDER: i32 = 441;
    pub const SHELF_ENGAGE: i32 = 442;
    pub const REACH_DETECTED: i32 = 443;

    // ── Industrial & Specialized (500-599) ────────────────────────────────

    // ind_forklift_proximity (500-502)
    pub const PROXIMITY_WARNING: i32 = 500;
    pub const VEHICLE_DETECTED: i32 = 501;
    pub const HUMAN_NEAR_VEHICLE: i32 = 502;

    // ind_confined_space (510-514)
    pub const WORKER_ENTRY: i32 = 510;
    pub const WORKER_EXIT: i32 = 511;
    pub const BREATHING_OK: i32 = 512;
    pub const EXTRACTION_ALERT: i32 = 513;
    pub const IMMOBILE_ALERT: i32 = 514;

    // ind_clean_room (520-523)
    pub const OCCUPANCY_COUNT: i32 = 520;
    pub const OCCUPANCY_VIOLATION: i32 = 521;
    pub const TURBULENT_MOTION: i32 = 522;
    pub const COMPLIANCE_REPORT: i32 = 523;

    // ind_livestock_monitor (530-533)
    pub const ANIMAL_PRESENT: i32 = 530;
    pub const ABNORMAL_STILLNESS: i32 = 531;
    pub const LABORED_BREATHING: i32 = 532;
    pub const ESCAPE_ALERT: i32 = 533;

    // ind_structural_vibration (540-543)
    pub const SEISMIC_DETECTED: i32 = 540;
    pub const MECHANICAL_RESONANCE: i32 = 541;
    pub const STRUCTURAL_DRIFT: i32 = 542;
    pub const VIBRATION_SPECTRUM: i32 = 543;

    // ── Exotic / Research (600-699) ──────────────────────────────────────

    // exo_dream_stage (600-603)
    pub const SLEEP_STAGE: i32 = 600;
    pub const SLEEP_QUALITY: i32 = 601;
    pub const REM_EPISODE: i32 = 602;
    pub const DEEP_SLEEP_RATIO: i32 = 603;

    // exo_emotion_detect (610-613)
    pub const AROUSAL_LEVEL: i32 = 610;
    pub const STRESS_INDEX: i32 = 611;
    pub const CALM_DETECTED: i32 = 612;
    pub const AGITATION_DETECTED: i32 = 613;

    // exo_gesture_language (620-623)
    pub const LETTER_RECOGNIZED: i32 = 620;
    pub const LETTER_CONFIDENCE: i32 = 621;
    pub const WORD_BOUNDARY: i32 = 622;
    pub const GESTURE_REJECTED: i32 = 623;

    // exo_music_conductor (630-634)
    pub const CONDUCTOR_BPM: i32 = 630;
    pub const BEAT_POSITION: i32 = 631;
    pub const DYNAMIC_LEVEL: i32 = 632;
    pub const GESTURE_CUTOFF: i32 = 633;
    pub const GESTURE_FERMATA: i32 = 634;

    // exo_plant_growth (640-643)
    pub const GROWTH_RATE: i32 = 640;
    pub const CIRCADIAN_PHASE: i32 = 641;
    pub const WILT_DETECTED: i32 = 642;
    pub const WATERING_EVENT: i32 = 643;

    // exo_ghost_hunter (650-653)
    pub const EXO_ANOMALY_DETECTED: i32 = 650;
    pub const EXO_ANOMALY_CLASS: i32 = 651;
    pub const HIDDEN_PRESENCE: i32 = 652;
    pub const ENVIRONMENTAL_DRIFT: i32 = 653;

    // exo_rain_detect (660-662)
    pub const RAIN_ONSET: i32 = 660;
    pub const RAIN_INTENSITY: i32 = 661;
    pub const RAIN_CESSATION: i32 = 662;

    // exo_breathing_sync (670-673)
    pub const SYNC_DETECTED: i32 = 670;
    pub const SYNC_PAIR_COUNT: i32 = 671;
    pub const GROUP_COHERENCE: i32 = 672;
    pub const SYNC_LOST: i32 = 673;

    // exo_time_crystal (680-682)
    pub const CRYSTAL_DETECTED: i32 = 680;
    pub const CRYSTAL_STABILITY: i32 = 681;
    pub const COORDINATION_INDEX: i32 = 682;

    // exo_hyperbolic_space (685-687)
    pub const HIERARCHY_LEVEL: i32 = 685;
    pub const HYPERBOLIC_RADIUS: i32 = 686;
    pub const LOCATION_LABEL: i32 = 687;

    // ── Signal Intelligence (700-729) ────────────────────────────────────

    // sig_flash_attention (700-702)
    pub const ATTENTION_PEAK_SC: i32 = 700;
    pub const ATTENTION_SPREAD: i32 = 701;
    pub const SPATIAL_FOCUS_ZONE: i32 = 702;

    // sig_temporal_compress (705-707)
    pub const COMPRESSION_RATIO: i32 = 705;
    pub const TIER_TRANSITION: i32 = 706;
    pub const HISTORY_DEPTH_HOURS: i32 = 707;

    // sig_coherence_gate (710-712)
    pub const GATE_DECISION: i32 = 710;
    pub const SIG_COHERENCE_SCORE: i32 = 711;
    pub const RECALIBRATE_NEEDED: i32 = 712;

    // sig_sparse_recovery (715-717)
    pub const RECOVERY_COMPLETE: i32 = 715;
    pub const RECOVERY_ERROR: i32 = 716;
    pub const DROPOUT_RATE: i32 = 717;

    // sig_mincut_person_match (720-722)
    pub const PERSON_ID_ASSIGNED: i32 = 720;
    pub const PERSON_ID_SWAP: i32 = 721;
    pub const MATCH_CONFIDENCE: i32 = 722;

    // sig_optimal_transport (725-727)
    pub const WASSERSTEIN_DISTANCE: i32 = 725;
    pub const DISTRIBUTION_SHIFT: i32 = 726;
    pub const SUBTLE_MOTION: i32 = 727;

    // ── Adaptive Learning (730-759) ──────────────────────────────────────

    // lrn_dtw_gesture_learn (730-733)
    pub const GESTURE_LEARNED: i32 = 730;
    pub const GESTURE_MATCHED: i32 = 731;
    pub const LRN_MATCH_DISTANCE: i32 = 732;
    pub const TEMPLATE_COUNT: i32 = 733;

    // lrn_anomaly_attractor (735-738)
    pub const ATTRACTOR_TYPE: i32 = 735;
    pub const LYAPUNOV_EXPONENT: i32 = 736;
    pub const BASIN_DEPARTURE: i32 = 737;
    pub const LEARNING_COMPLETE: i32 = 738;

    // lrn_meta_adapt (740-743)
    pub const PARAM_ADJUSTED: i32 = 740;
    pub const ADAPTATION_SCORE: i32 = 741;
    pub const ROLLBACK_TRIGGERED: i32 = 742;
    pub const META_LEVEL: i32 = 743;

    // lrn_ewc_lifelong (745-748)
    pub const KNOWLEDGE_RETAINED: i32 = 745;
    pub const NEW_TASK_LEARNED: i32 = 746;
    pub const FISHER_UPDATE: i32 = 747;
    pub const FORGETTING_RISK: i32 = 748;

    // ── Spatial Reasoning (760-789) ──────────────────────────────────────

    // spt_pagerank_influence (760-762)
    pub const DOMINANT_PERSON: i32 = 760;
    pub const INFLUENCE_SCORE: i32 = 761;
    pub const INFLUENCE_CHANGE: i32 = 762;

    // spt_micro_hnsw (765-768)
    pub const NEAREST_MATCH_ID: i32 = 765;
    pub const HNSW_MATCH_DISTANCE: i32 = 766;
    pub const CLASSIFICATION: i32 = 767;
    pub const LIBRARY_SIZE: i32 = 768;

    // spt_spiking_tracker (770-773)
    pub const TRACK_UPDATE: i32 = 770;
    pub const TRACK_VELOCITY: i32 = 771;
    pub const SPIKE_RATE: i32 = 772;
    pub const TRACK_LOST: i32 = 773;

    // ── Temporal Analysis (790-819) ──────────────────────────────────────

    // tmp_pattern_sequence (790-793)
    pub const PATTERN_DETECTED: i32 = 790;
    pub const PATTERN_CONFIDENCE: i32 = 791;
    pub const ROUTINE_DEVIATION: i32 = 792;
    pub const PREDICTION_NEXT: i32 = 793;

    // tmp_temporal_logic_guard (795-797)
    pub const LTL_VIOLATION: i32 = 795;
    pub const LTL_SATISFACTION: i32 = 796;
    pub const COUNTEREXAMPLE: i32 = 797;

    // tmp_goap_autonomy (800-803)
    pub const GOAL_SELECTED: i32 = 800;
    pub const MODULE_ACTIVATED: i32 = 801;
    pub const MODULE_DEACTIVATED: i32 = 802;
    pub const PLAN_COST: i32 = 803;

    // ── AI Security (820-849) ────────────────────────────────────────────

    // ais_prompt_shield (820-823)
    pub const REPLAY_ATTACK: i32 = 820;
    pub const INJECTION_DETECTED: i32 = 821;
    pub const JAMMING_DETECTED: i32 = 822;
    pub const SIGNAL_INTEGRITY: i32 = 823;

    // ais_behavioral_profiler (825-828)
    pub const BEHAVIOR_ANOMALY: i32 = 825;
    pub const PROFILE_DEVIATION: i32 = 826;
    pub const NOVEL_PATTERN: i32 = 827;
    pub const PROFILE_MATURITY: i32 = 828;

    // ── Quantum-Inspired (850-879) ───────────────────────────────────────

    // qnt_quantum_coherence (850-852)
    pub const ENTANGLEMENT_ENTROPY: i32 = 850;
    pub const DECOHERENCE_EVENT: i32 = 851;
    pub const BLOCH_DRIFT: i32 = 852;

    // qnt_interference_search (855-857)
    pub const HYPOTHESIS_WINNER: i32 = 855;
    pub const HYPOTHESIS_AMPLITUDE: i32 = 856;
    pub const SEARCH_ITERATIONS: i32 = 857;

    // ── Autonomous Systems (880-899) ─────────────────────────────────────

    // aut_psycho_symbolic (880-883)
    pub const INFERENCE_RESULT: i32 = 880;
    pub const INFERENCE_CONFIDENCE: i32 = 881;
    pub const RULE_FIRED: i32 = 882;
    pub const CONTRADICTION: i32 = 883;

    // aut_self_healing_mesh (885-888)
    pub const NODE_DEGRADED: i32 = 885;
    pub const MESH_RECONFIGURE: i32 = 886;
    pub const COVERAGE_SCORE: i32 = 887;
    pub const HEALING_COMPLETE: i32 = 888;
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
    // M-01 fix: treat negative host values as 0 instead of wrapping to usize::MAX.
    let n_sc = if n_subcarriers < 0 { 0 } else { n_subcarriers as usize };
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
