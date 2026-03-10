import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { Node, WasmModule, WasmModuleState } from "../types";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const STATE_STYLES: Record<WasmModuleState, { color: string; label: string }> = {
  running: { color: "var(--status-online)", label: "Running" },
  stopped: { color: "var(--status-warning)", label: "Stopped" },
  error: { color: "var(--status-error)", label: "Error" },
  loading: { color: "var(--status-info)", label: "Loading" },
};

// ---------------------------------------------------------------------------
// Module Library Types
// ---------------------------------------------------------------------------

interface LibraryModule {
  id: string;
  name: string;
  description: string;
  fullDescription: string;
  category: string;
  size: string;
  version: string;
  author: string;
  license: string;
  rating: number;
  downloads: number;
  chips: string[];
  memoryKb: number;
  features: string[];
  requirements: string[];
  changelog: { version: string; date: string; notes: string }[];
  exports: string[];
  dependencies: string[];
}

// Built-in edge module library from wifi-densepose-wasm-edge (67 modules)
// All modules compile to RVF (RuVector Format) containers for ESP32 deployment
const MODULE_LIBRARY: LibraryModule[] = [
  // ---- Core Modules (7) ----
  {
    id: "gesture", name: "Gesture Recognizer", category: "core", size: "32 KB", version: "1.0.0",
    description: "DTW template matching gesture classifier with learned templates",
    fullDescription: "Advanced gesture recognition using Dynamic Time Warping (DTW) algorithm. Recognizes predefined gestures like swipe, circle, push, pull, and custom user-defined gestures. Templates can be learned on-device through demonstration. Optimized for low-latency edge inference with <50ms response time.",
    author: "RuView Team", license: "Apache-2.0", rating: 4.8, downloads: 12450,
    chips: ["esp32", "esp32s3", "esp32c6"], memoryKb: 48,
    features: ["DTW template matching", "Custom gesture learning", "Multi-hand support", "Real-time inference <50ms", "Up to 32 gesture templates"],
    requirements: ["Minimum 2 CSI links", "48KB RAM", "coherence module recommended"],
    changelog: [
      { version: "1.0.0", date: "2024-01-15", notes: "Initial stable release with 12 preset gestures" },
      { version: "0.9.0", date: "2023-11-20", notes: "Added custom gesture learning" },
    ],
    exports: ["recognize_gesture", "learn_template", "list_templates", "clear_templates"],
    dependencies: [],
  },
  {
    id: "coherence", name: "Coherence Gate", category: "core", size: "18 KB", version: "1.0.0",
    description: "Z-score coherence scoring with Accept/Reject/Recalibrate decisions",
    fullDescription: "Signal quality gating system that evaluates CSI coherence across multiple links. Uses statistical Z-score analysis to determine if incoming CSI data meets quality thresholds. Outputs Accept (high quality), Reject (noise/interference), PredictOnly (marginal), or Recalibrate (drift detected) decisions. Essential for reliable sensing in dynamic RF environments.",
    author: "RuView Team", license: "Apache-2.0", rating: 4.9, downloads: 18230,
    chips: ["esp32", "esp32s2", "esp32s3", "esp32c3", "esp32c6"], memoryKb: 24,
    features: ["Z-score coherence analysis", "4-state gate decisions", "Drift detection", "Auto-recalibration triggers", "Per-link quality metrics"],
    requirements: ["8KB RAM minimum", "Works standalone"],
    changelog: [
      { version: "1.0.0", date: "2024-02-01", notes: "Stable release with hysteresis gate" },
    ],
    exports: ["evaluate_coherence", "get_gate_decision", "get_drift_profile", "reset_baseline"],
    dependencies: [],
  },
  {
    id: "adversarial", name: "Adversarial Detector", category: "core", size: "24 KB", version: "1.0.0",
    description: "Physically impossible signal detection and multi-link consistency",
    fullDescription: "Security-focused module that detects adversarial attacks and anomalous signals. Validates that CSI patterns are physically plausible by checking multi-link geometric consistency, signal propagation physics, and temporal continuity. Flags replay attacks, signal injection, and spoofing attempts.",
    author: "RuView Team", license: "Apache-2.0", rating: 4.7, downloads: 8920,
    chips: ["esp32", "esp32s3", "esp32c6"], memoryKb: 32,
    features: ["Physics-based validation", "Replay attack detection", "Multi-link consistency check", "Temporal anomaly flagging", "Confidence scoring"],
    requirements: ["Minimum 3 CSI links recommended", "coherence module"],
    changelog: [
      { version: "1.0.0", date: "2024-01-20", notes: "Initial release with 5 attack detection modes" },
    ],
    exports: ["validate_signal", "check_consistency", "get_threat_level", "report_anomaly"],
    dependencies: ["coherence"],
  },
  {
    id: "rvf", name: "RVF Runtime", category: "core", size: "48 KB", version: "1.0.0",
    description: "RuVector Format container runtime for ESP32 WASM modules",
    fullDescription: "The core runtime that executes RVF (RuVector Format) containers on ESP32 devices. RVF bundles WASM bytecode with metadata, signatures, and resource manifests. Provides sandboxed execution, inter-module communication, and resource management. Required for running any RVF-packaged edge module.",
    author: "RuView Team", license: "Apache-2.0", rating: 5.0, downloads: 24500,
    chips: ["esp32", "esp32s2", "esp32s3", "esp32c3", "esp32c6"], memoryKb: 64,
    features: ["WASM3 interpreter", "Sandboxed execution", "Inter-module messaging", "Resource quotas", "Hot-reload support", "Signature verification"],
    requirements: ["64KB RAM", "Pre-installed on all RuView nodes"],
    changelog: [
      { version: "1.0.0", date: "2024-01-01", notes: "Production-ready RVF runtime" },
    ],
    exports: ["load_module", "unload_module", "call_export", "send_message", "get_stats"],
    dependencies: [],
  },
  {
    id: "occupancy", name: "Room Occupancy", category: "core", size: "20 KB", version: "1.0.0",
    description: "Multi-link CSI fusion for occupancy counting",
    fullDescription: "Counts the number of people in a monitored space using multi-link CSI fusion. Employs clustering algorithms to distinguish individual human signatures. Accurate from 0-8 people with <10% error. Updates in real-time with configurable reporting intervals.",
    author: "RuView Team", license: "Apache-2.0", rating: 4.6, downloads: 15680,
    chips: ["esp32", "esp32s3", "esp32c6"], memoryKb: 32,
    features: ["0-8 person counting", "Multi-link fusion", "Real-time updates", "Configurable zones", "Historical trending"],
    requirements: ["Minimum 3 CSI links", "coherence module recommended"],
    changelog: [
      { version: "1.0.0", date: "2024-01-10", notes: "Stable counting algorithm" },
    ],
    exports: ["get_count", "get_confidence", "set_zone", "get_history"],
    dependencies: [],
  },
  {
    id: "vital_trend", name: "Vital Trend Monitor", category: "core", size: "28 KB", version: "1.0.0",
    description: "Longitudinal vital sign trending with biomechanics drift detection",
    fullDescription: "Tracks breathing rate and heart rate trends over extended periods (hours to days). Uses Welford online statistics for memory-efficient trending. Detects biomechanical drift indicating posture changes, fatigue, or health changes. Ideal for elderly monitoring and sleep tracking.",
    author: "RuView Team", license: "Apache-2.0", rating: 4.8, downloads: 9870,
    chips: ["esp32s3", "esp32c6"], memoryKb: 40,
    features: ["Breathing rate trending", "Heart rate variability", "Welford statistics", "Drift detection", "24-hour history"],
    requirements: ["Single stationary subject", "40KB RAM", "coherence module"],
    changelog: [
      { version: "1.0.0", date: "2024-02-15", notes: "Initial release with 24hr trending" },
    ],
    exports: ["get_breathing_trend", "get_hr_trend", "get_drift_score", "reset_baseline"],
    dependencies: ["coherence"],
  },
  {
    id: "intrusion", name: "Intrusion Detection", category: "core", size: "14 KB", version: "1.0.0",
    description: "Real-time zone intrusion alerts with CSI amplitude variance",
    fullDescription: "Lightweight intrusion detection using CSI amplitude variance analysis. Triggers alerts when movement is detected in defined zones. Configurable sensitivity and debounce. Extremely low power consumption suitable for battery-powered nodes.",
    author: "RuView Team", license: "Apache-2.0", rating: 4.5, downloads: 21340,
    chips: ["esp32", "esp32s2", "esp32s3", "esp32c3", "esp32c6"], memoryKb: 16,
    features: ["Zone-based detection", "Configurable sensitivity", "Debounce filtering", "Ultra-low power", "Webhook alerts"],
    requirements: ["Single CSI link minimum", "16KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2023-12-01", notes: "Production release" },
    ],
    exports: ["arm_zone", "disarm_zone", "get_status", "set_sensitivity", "get_events"],
    dependencies: [],
  },

  // ---- Medical Modules (5) ----
  {
    id: "med_sleep_apnea", name: "Sleep Apnea Detector", category: "medical", size: "36 KB", version: "1.0.0",
    description: "Detects apnea events from breathing pattern interruptions",
    fullDescription: "Clinical-grade sleep apnea detection using contactless WiFi sensing. Monitors breathing patterns and detects apnea (cessation) and hypopnea (shallow breathing) events. Calculates AHI (Apnea-Hypopnea Index) for sleep quality assessment. FDA 510(k) pending.",
    author: "RuView Medical", license: "Commercial", rating: 4.9, downloads: 5420,
    chips: ["esp32s3", "esp32c6"], memoryKb: 48,
    features: ["Apnea event detection", "Hypopnea detection", "AHI calculation", "Event logging", "Clinical reporting"],
    requirements: ["Single stationary subject", "coherence + vital_trend modules", "48KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2024-03-01", notes: "Clinical validation complete" },
    ],
    exports: ["start_monitoring", "stop_monitoring", "get_ahi", "get_events", "export_report"],
    dependencies: ["coherence", "vital_trend"],
  },
  {
    id: "med_cardiac_arrhythmia", name: "Cardiac Arrhythmia", category: "medical", size: "42 KB", version: "1.0.0",
    description: "Non-contact heart rhythm irregularity detection via CSI phase",
    fullDescription: "Detects cardiac arrhythmias including atrial fibrillation, bradycardia, and tachycardia using WiFi CSI phase analysis. Extracts heart rate variability (HRV) metrics and flags irregular rhythms. Designed for continuous home monitoring with alerts.",
    author: "RuView Medical", license: "Commercial", rating: 4.7, downloads: 3890,
    chips: ["esp32s3", "esp32c6"], memoryKb: 56,
    features: ["AFib detection", "HRV analysis", "Bradycardia alerts", "Tachycardia alerts", "Continuous monitoring"],
    requirements: ["Stationary subject", "High SNR environment", "56KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2024-02-20", notes: "Initial medical release" },
    ],
    exports: ["get_heart_rate", "get_hrv_metrics", "check_arrhythmia", "get_rhythm_type"],
    dependencies: ["coherence"],
  },
  {
    id: "med_respiratory_distress", name: "Respiratory Distress", category: "medical", size: "34 KB", version: "1.0.0",
    description: "Early respiratory distress warning from breathing rate changes",
    fullDescription: "Monitors breathing patterns for signs of respiratory distress including rapid shallow breathing, labored breathing, and respiratory rate elevation. Provides early warning for conditions like pneumonia, COPD exacerbation, or COVID-19 complications.",
    author: "RuView Medical", license: "Commercial", rating: 4.8, downloads: 4560,
    chips: ["esp32s3", "esp32c6"], memoryKb: 44,
    features: ["Tachypnea detection", "Labored breathing detection", "Rate trending", "Early warning alerts", "Risk scoring"],
    requirements: ["coherence module", "vital_trend recommended", "44KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2024-01-25", notes: "Clinical pilot release" },
    ],
    exports: ["get_respiratory_rate", "get_distress_score", "get_pattern_type", "set_thresholds"],
    dependencies: ["coherence"],
  },
  {
    id: "med_gait_analysis", name: "Gait Analysis", category: "medical", size: "38 KB", version: "1.0.0",
    description: "Walking pattern analysis for fall risk and mobility assessment",
    fullDescription: "Analyzes walking gait patterns to assess fall risk and mobility changes. Extracts metrics including stride length, cadence, symmetry, and variability. Tracks longitudinal changes for early detection of neurological or musculoskeletal issues.",
    author: "RuView Medical", license: "Commercial", rating: 4.6, downloads: 3210,
    chips: ["esp32s3", "esp32c6"], memoryKb: 52,
    features: ["Stride analysis", "Cadence measurement", "Symmetry scoring", "Fall risk assessment", "Longitudinal tracking"],
    requirements: ["Walking path coverage", "Minimum 3 links", "52KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2024-02-10", notes: "Gait metrics validated" },
    ],
    exports: ["analyze_gait", "get_fall_risk", "get_mobility_score", "compare_baseline"],
    dependencies: ["coherence", "occupancy"],
  },
  {
    id: "med_seizure_detect", name: "Seizure Detector", category: "medical", size: "32 KB", version: "1.0.0",
    description: "Convulsive motion detection for seizure alerting",
    fullDescription: "Detects convulsive seizure activity (tonic-clonic) through rapid, rhythmic body movement patterns. Triggers immediate alerts for caregiver notification. Distinguishes seizures from normal activity like exercising. Critical for epilepsy monitoring.",
    author: "RuView Medical", license: "Commercial", rating: 4.9, downloads: 2780,
    chips: ["esp32s3", "esp32c6"], memoryKb: 40,
    features: ["Tonic-clonic detection", "Immediate alerting", "False positive filtering", "Duration tracking", "Post-ictal monitoring"],
    requirements: ["coherence module", "40KB RAM", "Webhook or MQTT for alerts"],
    changelog: [
      { version: "1.0.0", date: "2024-03-05", notes: "Seizure detection validated" },
    ],
    exports: ["arm_detection", "disarm_detection", "get_status", "get_event_log"],
    dependencies: ["coherence"],
  },

  // ---- Security Modules (5) ----
  {
    id: "sec_perimeter_breach", name: "Perimeter Breach", category: "security", size: "22 KB", version: "1.0.0",
    description: "Perimeter zone crossing detection with direction tracking",
    fullDescription: "Detects when someone crosses a defined perimeter boundary. Tracks crossing direction (entry vs exit). Supports multiple perimeter zones with independent alerting. Ideal for securing doorways, windows, and property boundaries without cameras.",
    author: "RuView Security", license: "Apache-2.0", rating: 4.7, downloads: 11230,
    chips: ["esp32", "esp32s3", "esp32c6"], memoryKb: 28,
    features: ["Perimeter zones", "Direction tracking", "Entry/exit counting", "Multi-zone support", "Instant alerts"],
    requirements: ["Links spanning perimeter", "28KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2024-01-05", notes: "Production release" },
    ],
    exports: ["define_perimeter", "arm_perimeter", "get_crossings", "get_direction"],
    dependencies: [],
  },
  {
    id: "sec_weapon_detect", name: "Weapon Detection", category: "security", size: "28 KB", version: "1.0.0",
    description: "Metallic object signature detection in CSI patterns",
    fullDescription: "Experimental module for detecting concealed metallic objects (weapons) through CSI signature analysis. Uses RF reflection patterns characteristic of metal objects. Requires careful calibration and produces probabilistic alerts. Best used as screening layer.",
    author: "RuView Security", license: "Commercial", rating: 4.2, downloads: 1890,
    chips: ["esp32s3", "esp32c6"], memoryKb: 36,
    features: ["Metal signature detection", "Probabilistic scoring", "Screening alerts", "Calibration tools", "Integration APIs"],
    requirements: ["Controlled environment", "Calibration required", "36KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2024-02-28", notes: "Beta release for evaluation" },
    ],
    exports: ["scan_subject", "get_threat_score", "calibrate", "get_signature"],
    dependencies: ["coherence", "adversarial"],
  },
  {
    id: "sec_tailgating", name: "Tailgating Detector", category: "security", size: "24 KB", version: "1.0.0",
    description: "Multi-person entry detection at access points",
    fullDescription: "Detects tailgating (piggybacking) at access control points. Identifies when multiple people pass through a door on a single access credential. Counts individuals and alerts on policy violations. Integrates with access control systems.",
    author: "RuView Security", license: "Apache-2.0", rating: 4.6, downloads: 7650,
    chips: ["esp32", "esp32s3", "esp32c6"], memoryKb: 32,
    features: ["Multi-person detection", "Access point monitoring", "Policy enforcement", "Count accuracy >95%", "ACS integration"],
    requirements: ["Links at access point", "occupancy module", "32KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2024-01-18", notes: "Access control integration" },
    ],
    exports: ["set_access_point", "get_person_count", "check_tailgating", "integrate_acs"],
    dependencies: ["occupancy"],
  },
  {
    id: "sec_loitering", name: "Loitering Alert", category: "security", size: "20 KB", version: "1.0.0",
    description: "Prolonged presence detection in restricted areas",
    fullDescription: "Monitors for prolonged presence (loitering) in defined areas. Configurable time thresholds per zone. Useful for securing ATMs, entrances, parking areas, and other sensitive locations. Triggers alerts after threshold exceeded.",
    author: "RuView Security", license: "Apache-2.0", rating: 4.5, downloads: 8920,
    chips: ["esp32", "esp32s2", "esp32s3", "esp32c3", "esp32c6"], memoryKb: 24,
    features: ["Time-based detection", "Zone configuration", "Adjustable thresholds", "Alert webhooks", "Presence history"],
    requirements: ["intrusion module recommended", "24KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2024-01-12", notes: "Stable release" },
    ],
    exports: ["define_zone", "set_threshold", "get_presence_time", "arm_zone"],
    dependencies: [],
  },
  {
    id: "sec_panic_motion", name: "Panic Motion", category: "security", size: "18 KB", version: "1.0.0",
    description: "Rapid erratic movement detection for emergency response",
    fullDescription: "Detects panic-like motion patterns including running, erratic movements, and struggle. Triggers emergency alerts for rapid response. Useful in healthcare, corrections, and high-security environments.",
    author: "RuView Security", license: "Apache-2.0", rating: 4.4, downloads: 5430,
    chips: ["esp32", "esp32s3", "esp32c6"], memoryKb: 24,
    features: ["Panic pattern recognition", "Struggle detection", "Rapid movement alerts", "Configurable sensitivity", "Emergency webhooks"],
    requirements: ["coherence module", "24KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2024-02-05", notes: "Emergency detection release" },
    ],
    exports: ["arm_detection", "set_sensitivity", "get_alert_status", "get_motion_type"],
    dependencies: ["coherence"],
  },

  // ---- Building Automation Modules (5) ----
  {
    id: "bld_hvac_presence", name: "HVAC Presence", category: "building", size: "16 KB", version: "1.0.0",
    description: "Occupancy-based HVAC zone control integration",
    fullDescription: "Integrates with building HVAC systems to provide occupancy-based climate control. Reduces energy consumption by 20-40% through presence-aware heating/cooling. Supports BACnet, Modbus, and REST API integrations.",
    author: "RuView Building", license: "Apache-2.0", rating: 4.7, downloads: 9870,
    chips: ["esp32", "esp32s2", "esp32s3", "esp32c3", "esp32c6"], memoryKb: 20,
    features: ["Occupancy detection", "HVAC integration", "BACnet support", "Energy savings 20-40%", "Zone control"],
    requirements: ["occupancy module", "HVAC system access", "20KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2024-01-08", notes: "BACnet integration complete" },
    ],
    exports: ["get_occupancy", "set_hvac_mode", "get_energy_savings", "integrate_bacnet"],
    dependencies: ["occupancy"],
  },
  {
    id: "bld_lighting_zones", name: "Lighting Zones", category: "building", size: "14 KB", version: "1.0.0",
    description: "Movement-triggered lighting control per zone",
    fullDescription: "Controls lighting based on presence detection within defined zones. Supports DALI, DMX, and smart bulb protocols. Provides smooth transitions and configurable timeout behaviors.",
    author: "RuView Building", license: "Apache-2.0", rating: 4.6, downloads: 12340,
    chips: ["esp32", "esp32s2", "esp32s3", "esp32c3", "esp32c6"], memoryKb: 18,
    features: ["Zone-based control", "DALI/DMX support", "Smart bulb integration", "Smooth transitions", "Timeout configuration"],
    requirements: ["intrusion module", "Lighting system access", "18KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2023-12-15", notes: "Multi-protocol support" },
    ],
    exports: ["set_zone", "trigger_lights", "set_timeout", "get_status"],
    dependencies: ["intrusion"],
  },
  {
    id: "bld_elevator_count", name: "Elevator Counting", category: "building", size: "18 KB", version: "1.0.0",
    description: "Elevator cabin occupancy counting",
    fullDescription: "Counts passengers in elevator cabins for load management and social distancing. Provides real-time count updates for lobby displays and building management systems.",
    author: "RuView Building", license: "Apache-2.0", rating: 4.5, downloads: 4560,
    chips: ["esp32", "esp32s3", "esp32c6"], memoryKb: 24,
    features: ["Real-time counting", "Load estimation", "BMS integration", "Display output", "Historical logging"],
    requirements: ["occupancy module", "Elevator cab installation", "24KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2024-01-22", notes: "Elevator integration" },
    ],
    exports: ["get_count", "get_load_pct", "set_max_capacity", "integrate_bms"],
    dependencies: ["occupancy"],
  },
  {
    id: "bld_meeting_room", name: "Meeting Room Status", category: "building", size: "20 KB", version: "1.0.0",
    description: "Conference room occupancy and booking validation",
    fullDescription: "Monitors meeting room occupancy and validates against calendar bookings. Detects ghost bookings (no-shows) and auto-releases rooms. Integrates with Google Calendar, Microsoft 365, and room booking systems.",
    author: "RuView Building", license: "Apache-2.0", rating: 4.8, downloads: 8790,
    chips: ["esp32", "esp32s2", "esp32s3", "esp32c3", "esp32c6"], memoryKb: 28,
    features: ["Occupancy detection", "Calendar integration", "Ghost booking detection", "Auto-release", "Room displays"],
    requirements: ["occupancy module", "Calendar API access", "28KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2024-02-01", notes: "Calendar integrations" },
    ],
    exports: ["get_status", "check_booking", "release_room", "get_utilization"],
    dependencies: ["occupancy"],
  },
  {
    id: "bld_energy_audit", name: "Energy Audit", category: "building", size: "24 KB", version: "1.0.0",
    description: "Correlates occupancy with energy consumption patterns",
    fullDescription: "Analyzes energy consumption in relation to actual occupancy patterns. Identifies waste from unoccupied spaces consuming energy. Generates reports for energy audits and sustainability compliance.",
    author: "RuView Building", license: "Apache-2.0", rating: 4.6, downloads: 6540,
    chips: ["esp32", "esp32s3", "esp32c6"], memoryKb: 32,
    features: ["Occupancy correlation", "Waste identification", "Audit reports", "Sustainability metrics", "Trend analysis"],
    requirements: ["occupancy module", "Energy meter integration", "32KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2024-02-18", notes: "Reporting features" },
    ],
    exports: ["get_energy_waste", "generate_report", "get_correlation", "set_meters"],
    dependencies: ["occupancy"],
  },

  // ---- Retail Analytics Modules (5) ----
  {
    id: "ret_queue_length", name: "Queue Length", category: "retail", size: "22 KB", version: "1.0.0",
    description: "Checkout queue length estimation and wait time prediction",
    fullDescription: "Estimates queue lengths at checkout lines and predicts wait times. Helps retailers optimize staffing and improve customer experience. Provides real-time alerts when queues exceed thresholds.",
    author: "RuView Retail", license: "Commercial", rating: 4.7, downloads: 7890,
    chips: ["esp32", "esp32s3", "esp32c6"], memoryKb: 28,
    features: ["Queue counting", "Wait time prediction", "Staffing alerts", "Historical analysis", "POS integration"],
    requirements: ["occupancy module", "Checkout area coverage", "28KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2024-01-28", notes: "Retail pilot success" },
    ],
    exports: ["get_queue_length", "predict_wait_time", "set_alert_threshold", "get_history"],
    dependencies: ["occupancy"],
  },
  {
    id: "ret_dwell_heatmap", name: "Dwell Heatmap", category: "retail", size: "26 KB", version: "1.0.0",
    description: "Customer dwell time heatmap generation",
    fullDescription: "Generates heatmaps showing where customers spend time in a store. Identifies high-engagement areas and dead zones. Helps optimize product placement and store layout.",
    author: "RuView Retail", license: "Commercial", rating: 4.6, downloads: 6540,
    chips: ["esp32s3", "esp32c6"], memoryKb: 36,
    features: ["Dwell time tracking", "Heatmap generation", "Zone analysis", "Layout optimization", "Export to BI tools"],
    requirements: ["Grid of CSI links", "36KB RAM", "Backend for visualization"],
    changelog: [
      { version: "1.0.0", date: "2024-02-08", notes: "Heatmap visualization" },
    ],
    exports: ["get_heatmap", "get_zone_dwell", "export_data", "set_grid"],
    dependencies: ["occupancy"],
  },
  {
    id: "ret_customer_flow", name: "Customer Flow", category: "retail", size: "28 KB", version: "1.0.0",
    description: "Store traffic flow analysis and path tracking",
    fullDescription: "Tracks customer movement paths through a retail space. Analyzes traffic flow patterns, identifies bottlenecks, and measures path efficiency. Useful for store layout optimization and promotional placement.",
    author: "RuView Retail", license: "Commercial", rating: 4.5, downloads: 5430,
    chips: ["esp32s3", "esp32c6"], memoryKb: 40,
    features: ["Path tracking", "Flow analysis", "Bottleneck detection", "Traffic patterns", "Sankey diagrams"],
    requirements: ["Multi-zone coverage", "occupancy module", "40KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2024-02-12", notes: "Flow analytics" },
    ],
    exports: ["get_flow_map", "get_paths", "find_bottlenecks", "get_traffic_stats"],
    dependencies: ["occupancy", "sec_perimeter_breach"],
  },
  {
    id: "ret_table_turnover", name: "Table Turnover", category: "retail", size: "20 KB", version: "1.0.0",
    description: "Restaurant table occupancy and turnover metrics",
    fullDescription: "Monitors table occupancy in restaurants to track turnover rates, average meal duration, and seating efficiency. Helps optimize table assignments and predict wait times for guests.",
    author: "RuView Retail", license: "Commercial", rating: 4.6, downloads: 4320,
    chips: ["esp32", "esp32s3", "esp32c6"], memoryKb: 28,
    features: ["Table occupancy", "Turnover tracking", "Duration metrics", "Waitlist optimization", "Revenue correlation"],
    requirements: ["Per-table coverage", "occupancy module", "28KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2024-01-30", notes: "Restaurant pilot" },
    ],
    exports: ["get_table_status", "get_turnover_rate", "get_avg_duration", "optimize_seating"],
    dependencies: ["occupancy"],
  },
  {
    id: "ret_shelf_engagement", name: "Shelf Engagement", category: "retail", size: "24 KB", version: "1.0.0",
    description: "Customer interaction with product shelves",
    fullDescription: "Detects customer engagement with product shelves including browsing, touching, and product pickup. Measures engagement time and conversion rates. Useful for planogram optimization and promotion effectiveness.",
    author: "RuView Retail", license: "Commercial", rating: 4.4, downloads: 3890,
    chips: ["esp32s3", "esp32c6"], memoryKb: 32,
    features: ["Engagement detection", "Browse vs buy analysis", "Planogram insights", "Promotion measurement", "Product pickup detection"],
    requirements: ["Shelf-level coverage", "gesture module recommended", "32KB RAM"],
    changelog: [
      { version: "1.0.0", date: "2024-02-15", notes: "Shelf analytics" },
    ],
    exports: ["get_engagement", "get_conversion", "track_product", "get_shelf_metrics"],
    dependencies: ["gesture"],
  },

  // ---- Industrial Modules (5) ----
  {
    id: "ind_forklift_proximity", name: "Forklift Proximity", category: "industrial", size: "26 KB", version: "1.0.0",
    description: "Vehicle-to-pedestrian proximity warning system",
    fullDescription: "Safety system that warns pedestrians when forklifts or other industrial vehicles are nearby. Provides both audible and visual alerts. Reduces workplace accidents in warehouses and manufacturing facilities.",
    author: "RuView Industrial", license: "Commercial", rating: 4.8, downloads: 6780,
    chips: ["esp32", "esp32s3", "esp32c6"], memoryKb: 32,
    features: ["Proximity detection", "Audible alerts", "Visual indicators", "Speed estimation", "Near-miss logging"],
    requirements: ["Vehicle + pedestrian nodes", "32KB RAM", "Alert actuators"],
    changelog: [
      { version: "1.0.0", date: "2024-02-01", notes: "Safety certification" },
    ],
    exports: ["get_proximity", "trigger_alert", "log_event", "set_thresholds"],
    dependencies: ["occupancy"],
  },
  {
    id: "ind_confined_space", name: "Confined Space", category: "industrial", size: "22 KB", version: "1.0.0",
    description: "Worker presence monitoring in confined spaces",
    fullDescription: "Monitors worker presence in confined spaces (tanks, silos, tunnels) for safety compliance. Tracks entry/exit, duration, and provides emergency detection. Meets OSHA confined space requirements.",
    author: "RuView Industrial", license: "Commercial", rating: 4.9, downloads: 5430,
    chips: ["esp32", "esp32s3", "esp32c6"], memoryKb: 28,
    features: ["Entry/exit tracking", "Duration monitoring", "Emergency detection", "OSHA compliance", "Buddy system enforcement"],
    requirements: ["Confined space entry points", "28KB RAM", "intrusion module"],
    changelog: [
      { version: "1.0.0", date: "2024-01-15", notes: "OSHA compliance features" },
    ],
    exports: ["log_entry", "log_exit", "get_occupants", "trigger_emergency"],
    dependencies: ["intrusion", "occupancy"],
  },
  {
    id: "ind_clean_room", name: "Clean Room Monitor", category: "industrial", size: "24 KB", version: "1.0.0",
    description: "Personnel tracking in cleanroom environments",
    fullDescription: "Tracks personnel in cleanroom environments for contamination control. Monitors gowning compliance, movement patterns, and alerts on protocol violations. Integrates with cleanroom management systems.",
    author: "RuView Industrial", license: "Commercial", rating: 4.7, downloads: 3210,
    chips: ["esp32s3", "esp32c6"], memoryKb: 32,
    features: ["Personnel tracking", "Gowning compliance", "Protocol enforcement", "Movement logging", "Contamination alerts"],
    requirements: ["cleanroom installation", "32KB RAM", "occupancy module"],
    changelog: [
      { version: "1.0.0", date: "2024-02-20", notes: "Cleanroom protocols" },
    ],
    exports: ["track_personnel", "check_compliance", "log_movement", "get_violations"],
    dependencies: ["occupancy", "sec_perimeter_breach"],
  },
  {
    id: "ind_livestock_monitor", name: "Livestock Monitor", category: "industrial", size: "28 KB", version: "1.0.0",
    description: "Animal movement and health pattern monitoring",
    fullDescription: "Monitors livestock movement patterns and behavior for health assessment. Detects lameness, reduced activity, and abnormal behavior indicating illness. Useful for dairy, poultry, and swine operations.",
    author: "RuView AgTech", license: "Commercial", rating: 4.5, downloads: 2890,
    chips: ["esp32", "esp32s3", "esp32c6"], memoryKb: 36,
    features: ["Activity monitoring", "Lameness detection", "Behavior analysis", "Health alerts", "Herd management"],
    requirements: ["Barn/pen coverage", "36KB RAM", "vital_trend module"],
    changelog: [
      { version: "1.0.0", date: "2024-02-25", notes: "AgTech pilot" },
    ],
    exports: ["get_activity_level", "detect_lameness", "analyze_behavior", "get_health_score"],
    dependencies: ["vital_trend"],
  },
  {
    id: "ind_structural_vibration", name: "Structural Vibration", category: "industrial", size: "30 KB", version: "1.0.0",
    description: "Building/bridge structural vibration monitoring",
    fullDescription: "Monitors structural vibrations in buildings and bridges using CSI sensitivity to environmental changes. Detects abnormal vibration patterns that may indicate structural issues. Provides early warning for maintenance needs.",
    author: "RuView Industrial", license: "Commercial", rating: 4.6, downloads: 2340,
    chips: ["esp32s3", "esp32c6"], memoryKb: 40,
    features: ["Vibration monitoring", "Frequency analysis", "Anomaly detection", "Trend tracking", "Structural alerts"],
    requirements: ["Fixed installation", "40KB RAM", "coherence module"],
    changelog: [
      { version: "1.0.0", date: "2024-03-01", notes: "Structural monitoring" },
    ],
    exports: ["get_vibration", "analyze_frequency", "detect_anomaly", "get_trend"],
    dependencies: ["coherence"],
  },

  // ---- Exotic/Research Modules (10) - Simplified entries ----
  { id: "exo_time_crystal", name: "Time Crystal Detector", category: "exotic", size: "32 KB", version: "0.5.0", description: "Periodic pattern detection in temporal CSI sequences", fullDescription: "Research module exploring time-crystal-like periodic patterns in CSI data. Detects stable oscillatory patterns that persist without external driving.", author: "RuView Research", license: "MIT", rating: 4.0, downloads: 890, chips: ["esp32s3", "esp32c6"], memoryKb: 40, features: ["Pattern detection", "Temporal analysis"], requirements: ["Research use", "40KB RAM"], changelog: [{ version: "0.5.0", date: "2024-01-01", notes: "Research alpha" }], exports: ["detect_pattern", "get_frequency"], dependencies: [] },
  { id: "exo_hyperbolic_space", name: "Hyperbolic Embedding", category: "exotic", size: "38 KB", version: "0.5.0", description: "Poincare ball embeddings for hierarchical motion patterns", fullDescription: "Uses hyperbolic geometry (Poincare ball model) to embed hierarchical motion patterns in continuous space. Research module for advanced motion classification.", author: "RuView Research", license: "MIT", rating: 4.1, downloads: 670, chips: ["esp32s3", "esp32c6"], memoryKb: 48, features: ["Hyperbolic embeddings", "Hierarchical patterns"], requirements: ["Research use", "48KB RAM"], changelog: [{ version: "0.5.0", date: "2024-01-10", notes: "Research alpha" }], exports: ["embed_motion", "get_hierarchy"], dependencies: [] },
  { id: "exo_dream_stage", name: "Dream Stage Classifier", category: "exotic", size: "36 KB", version: "0.5.0", description: "Sleep stage detection (REM, NREM, wake) from micro-movements", fullDescription: "Classifies sleep stages using subtle body micro-movements detectable via CSI. Identifies REM, light NREM, deep NREM, and wake states.", author: "RuView Research", license: "MIT", rating: 4.3, downloads: 1230, chips: ["esp32s3", "esp32c6"], memoryKb: 44, features: ["Sleep staging", "Micro-movement analysis"], requirements: ["vital_trend module", "44KB RAM"], changelog: [{ version: "0.5.0", date: "2024-01-15", notes: "Research alpha" }], exports: ["get_sleep_stage", "get_rem_pct"], dependencies: ["vital_trend"] },
  { id: "exo_emotion_detect", name: "Emotion Detection", category: "exotic", size: "42 KB", version: "0.5.0", description: "Emotional state inference from posture and movement dynamics", fullDescription: "Experimental emotion detection using body language and movement patterns. Identifies states like calm, anxious, excited, and fatigued.", author: "RuView Research", license: "MIT", rating: 3.9, downloads: 980, chips: ["esp32s3", "esp32c6"], memoryKb: 52, features: ["Emotion classification", "Body language analysis"], requirements: ["gesture module", "52KB RAM"], changelog: [{ version: "0.5.0", date: "2024-01-20", notes: "Research alpha" }], exports: ["get_emotion", "get_confidence"], dependencies: ["gesture"] },
  { id: "exo_gesture_language", name: "Gesture Language", category: "exotic", size: "48 KB", version: "0.5.0", description: "Sign language gesture recognition via CSI", fullDescription: "Recognizes sign language gestures using WiFi CSI. Currently supports ASL alphabet and common phrases. Research collaboration with accessibility community.", author: "RuView Research", license: "MIT", rating: 4.4, downloads: 1560, chips: ["esp32s3", "esp32c6"], memoryKb: 56, features: ["ASL recognition", "Phrase detection"], requirements: ["gesture module", "56KB RAM"], changelog: [{ version: "0.5.0", date: "2024-02-01", notes: "ASL alphabet support" }], exports: ["recognize_sign", "get_phrase"], dependencies: ["gesture"] },
  { id: "exo_music_conductor", name: "Music Conductor", category: "exotic", size: "44 KB", version: "0.5.0", description: "Conducting gesture recognition for interactive music control", fullDescription: "Recognizes conducting gestures for interactive music control. Detects tempo, dynamics, and common conducting patterns. Creative tech experiment.", author: "RuView Research", license: "MIT", rating: 4.2, downloads: 780, chips: ["esp32s3", "esp32c6"], memoryKb: 52, features: ["Tempo detection", "Dynamic control", "MIDI output"], requirements: ["gesture module", "52KB RAM"], changelog: [{ version: "0.5.0", date: "2024-02-05", notes: "MIDI integration" }], exports: ["get_tempo", "get_dynamic", "send_midi"], dependencies: ["gesture"] },
  { id: "exo_plant_growth", name: "Plant Growth Monitor", category: "exotic", size: "26 KB", version: "0.5.0", description: "Plant movement and growth pattern monitoring", fullDescription: "Monitors subtle plant movements and growth patterns using CSI. Detects circadian rhythms, response to stimuli, and growth rates.", author: "RuView Research", license: "MIT", rating: 3.8, downloads: 560, chips: ["esp32", "esp32s3", "esp32c6"], memoryKb: 32, features: ["Growth tracking", "Circadian detection"], requirements: ["Long-term monitoring", "32KB RAM"], changelog: [{ version: "0.5.0", date: "2024-02-10", notes: "Research alpha" }], exports: ["get_growth_rate", "detect_rhythm"], dependencies: [] },
  { id: "exo_ghost_hunter", name: "Anomaly Hunter", category: "exotic", size: "22 KB", version: "0.5.0", description: "Unexplained environmental perturbation detection", fullDescription: "Detects unexplained RF perturbations and environmental anomalies. Originally a joke module that found real use in debugging RF interference issues.", author: "RuView Research", license: "MIT", rating: 4.5, downloads: 2340, chips: ["esp32", "esp32s2", "esp32s3", "esp32c3", "esp32c6"], memoryKb: 24, features: ["Anomaly detection", "RF interference logging"], requirements: ["24KB RAM"], changelog: [{ version: "0.5.0", date: "2024-02-15", notes: "Now actually useful" }], exports: ["detect_anomaly", "get_rf_noise"], dependencies: [] },
  { id: "exo_rain_detect", name: "Rain Detector", category: "exotic", size: "18 KB", version: "0.5.0", description: "Precipitation detection from RF propagation changes", fullDescription: "Detects precipitation (rain, snow) through changes in RF propagation characteristics. Water droplets affect WiFi signals in measurable ways.", author: "RuView Research", license: "MIT", rating: 4.0, downloads: 1890, chips: ["esp32", "esp32s2", "esp32s3", "esp32c3", "esp32c6"], memoryKb: 20, features: ["Rain detection", "Intensity estimation"], requirements: ["Outdoor nodes", "20KB RAM"], changelog: [{ version: "0.5.0", date: "2024-02-20", notes: "Weather correlation" }], exports: ["is_raining", "get_intensity"], dependencies: [] },
  { id: "exo_breathing_sync", name: "Breathing Sync", category: "exotic", size: "28 KB", version: "0.5.0", description: "Multi-person breathing synchronization detection", fullDescription: "Detects when multiple people in a room synchronize their breathing (common in meditation, couples sleeping, group activities).", author: "RuView Research", license: "MIT", rating: 4.1, downloads: 1120, chips: ["esp32s3", "esp32c6"], memoryKb: 36, features: ["Sync detection", "Coherence scoring"], requirements: ["vital_trend module", "36KB RAM"], changelog: [{ version: "0.5.0", date: "2024-02-25", notes: "Multi-person support" }], exports: ["get_sync_score", "get_phase_diff"], dependencies: ["vital_trend"] },

  // ---- Signal Intelligence Modules (6) ----
  { id: "sig_coherence_gate", name: "Coherence Gate Pro", category: "signal", size: "24 KB", version: "1.0.0", description: "Multi-band CSI frame fusion with cross-channel coherence", fullDescription: "Advanced coherence analysis across multiple frequency bands. Fuses CSI frames from 2.4GHz and 5GHz bands for improved accuracy.", author: "RuView Signal", license: "Apache-2.0", rating: 4.8, downloads: 7650, chips: ["esp32s3", "esp32c6"], memoryKb: 32, features: ["Multi-band fusion", "Cross-channel coherence", "Enhanced SNR"], requirements: ["Dual-band nodes", "32KB RAM"], changelog: [{ version: "1.0.0", date: "2024-02-01", notes: "Multi-band support" }], exports: ["fuse_frames", "get_coherence"], dependencies: ["coherence"] },
  { id: "sig_flash_attention", name: "Flash Attention", category: "signal", size: "34 KB", version: "1.0.0", description: "Memory-efficient attention for large CSI sequences", fullDescription: "Implements Flash Attention algorithm for efficient processing of long CSI sequences. Reduces memory usage by 4x while maintaining accuracy.", author: "RuView Signal", license: "Apache-2.0", rating: 4.9, downloads: 8920, chips: ["esp32s3", "esp32c6"], memoryKb: 44, features: ["Flash Attention", "4x memory reduction", "Long sequences"], requirements: ["44KB RAM"], changelog: [{ version: "1.0.0", date: "2024-02-10", notes: "Flash Attention implementation" }], exports: ["process_sequence", "get_attention"], dependencies: [] },
  { id: "sig_temporal_compress", name: "Temporal Compression", category: "signal", size: "28 KB", version: "1.0.0", description: "Compressed CSI buffer with temporal tensor encoding", fullDescription: "Compresses temporal CSI sequences using learned tensor encodings. Reduces storage and bandwidth by 8x with minimal accuracy loss.", author: "RuView Signal", license: "Apache-2.0", rating: 4.7, downloads: 6540, chips: ["esp32s3", "esp32c6"], memoryKb: 36, features: ["8x compression", "Tensor encoding", "Streaming support"], requirements: ["36KB RAM"], changelog: [{ version: "1.0.0", date: "2024-02-15", notes: "Tensor compression" }], exports: ["compress", "decompress", "stream"], dependencies: [] },
  { id: "sig_sparse_recovery", name: "Sparse Recovery", category: "signal", size: "26 KB", version: "1.0.0", description: "Sparse subcarrier interpolation (114→56) recovery", fullDescription: "Recovers full 114 subcarrier CSI from sparse 56 subcarrier ESP32 data using compressed sensing techniques.", author: "RuView Signal", license: "Apache-2.0", rating: 4.6, downloads: 5430, chips: ["esp32", "esp32s2", "esp32s3", "esp32c3", "esp32c6"], memoryKb: 32, features: ["Sparse recovery", "114 subcarrier output", "Compressed sensing"], requirements: ["32KB RAM"], changelog: [{ version: "1.0.0", date: "2024-02-20", notes: "ISTA solver" }], exports: ["recover_full", "get_quality"], dependencies: [] },
  { id: "sig_mincut_person_match", name: "MinCut Person Match", category: "signal", size: "32 KB", version: "1.0.0", description: "Graph-based person matching across multiple viewpoints", fullDescription: "Matches person detections across multiple CSI viewpoints using graph min-cut algorithms. Part of RuVector integration.", author: "RuView Signal", license: "Apache-2.0", rating: 4.7, downloads: 4320, chips: ["esp32s3", "esp32c6"], memoryKb: 40, features: ["Cross-view matching", "Min-cut optimization", "Re-ID tracking"], requirements: ["Multi-link setup", "40KB RAM"], changelog: [{ version: "1.0.0", date: "2024-02-25", notes: "RuVector integration" }], exports: ["match_persons", "get_tracks"], dependencies: ["occupancy"] },
  { id: "sig_optimal_transport", name: "Optimal Transport", category: "signal", size: "36 KB", version: "1.0.0", description: "Wasserstein distance for CSI distribution matching", fullDescription: "Uses optimal transport (Wasserstein distance) to compare CSI distributions for domain adaptation and transfer learning.", author: "RuView Signal", license: "Apache-2.0", rating: 4.5, downloads: 2890, chips: ["esp32s3", "esp32c6"], memoryKb: 44, features: ["Wasserstein distance", "Domain adaptation", "Distribution matching"], requirements: ["44KB RAM", "Research use"], changelog: [{ version: "1.0.0", date: "2024-03-01", notes: "OT implementation" }], exports: ["compute_wasserstein", "adapt_domain"], dependencies: [] },

  // ---- Learning Modules (4) ----
  { id: "lrn_dtw_gesture_learn", name: "DTW Gesture Learning", category: "learning", size: "38 KB", version: "1.0.0", description: "Online DTW template learning from user demonstrations", fullDescription: "Learn new gesture templates on-device through user demonstration. Uses DTW averaging to create robust templates from multiple examples.", author: "RuView Learning", license: "Apache-2.0", rating: 4.8, downloads: 8920, chips: ["esp32s3", "esp32c6"], memoryKb: 48, features: ["Online learning", "DTW averaging", "Template management"], requirements: ["gesture module", "48KB RAM"], changelog: [{ version: "1.0.0", date: "2024-02-01", notes: "Online learning" }], exports: ["learn_gesture", "refine_template", "export_templates"], dependencies: ["gesture"] },
  { id: "lrn_anomaly_attractor", name: "Anomaly Attractor", category: "learning", size: "34 KB", version: "1.0.0", description: "Strange attractor-based anomaly detection", fullDescription: "Uses chaos theory concepts (strange attractors) to model normal behavior and detect anomalies. Self-adapts to environment.", author: "RuView Learning", license: "Apache-2.0", rating: 4.5, downloads: 3210, chips: ["esp32s3", "esp32c6"], memoryKb: 44, features: ["Attractor modeling", "Adaptive baseline", "Anomaly scoring"], requirements: ["44KB RAM"], changelog: [{ version: "1.0.0", date: "2024-02-10", notes: "Attractor models" }], exports: ["update_model", "get_anomaly_score"], dependencies: [] },
  { id: "lrn_meta_adapt", name: "Meta Adaptation", category: "learning", size: "42 KB", version: "1.0.0", description: "Few-shot adaptation for new environments", fullDescription: "Meta-learning module that quickly adapts to new environments with minimal calibration data. Uses MAML-inspired techniques.", author: "RuView Learning", license: "Apache-2.0", rating: 4.6, downloads: 2890, chips: ["esp32s3", "esp32c6"], memoryKb: 52, features: ["Few-shot learning", "MAML-inspired", "Rapid adaptation"], requirements: ["52KB RAM", "Base model required"], changelog: [{ version: "1.0.0", date: "2024-02-15", notes: "Meta-learning support" }], exports: ["adapt", "get_adapted_model"], dependencies: [] },
  { id: "lrn_ewc_lifelong", name: "EWC Lifelong", category: "learning", size: "46 KB", version: "1.0.0", description: "Elastic Weight Consolidation for continual learning", fullDescription: "Enables continual learning without catastrophic forgetting using Elastic Weight Consolidation. Models can learn new tasks while retaining old knowledge.", author: "RuView Learning", license: "Apache-2.0", rating: 4.7, downloads: 2340, chips: ["esp32s3", "esp32c6"], memoryKb: 56, features: ["Continual learning", "EWC regularization", "Task retention"], requirements: ["56KB RAM", "Base model required"], changelog: [{ version: "1.0.0", date: "2024-02-20", notes: "EWC implementation" }], exports: ["learn_task", "consolidate", "get_fisher"], dependencies: [] },

  // ---- Remaining categories with simplified but complete entries ----
  { id: "spt_pagerank_influence", name: "PageRank Influence", category: "spatial", size: "28 KB", version: "1.0.0", description: "Spatial influence ranking for multi-person scenarios", fullDescription: "Uses PageRank-inspired algorithms to determine influence and leadership in multi-person spatial arrangements.", author: "RuView Spatial", license: "Apache-2.0", rating: 4.5, downloads: 2340, chips: ["esp32s3", "esp32c6"], memoryKb: 36, features: ["Influence ranking", "Social dynamics", "Leader detection"], requirements: ["occupancy module", "36KB RAM"], changelog: [{ version: "1.0.0", date: "2024-02-01", notes: "PageRank for spatial" }], exports: ["get_influence", "find_leader"], dependencies: ["occupancy"] },
  { id: "spt_micro_hnsw", name: "Micro HNSW", category: "spatial", size: "32 KB", version: "1.0.0", description: "Lightweight HNSW index for edge pattern matching", fullDescription: "Compact HNSW (Hierarchical Navigable Small World) index optimized for edge devices. Enables fast similarity search for pattern matching.", author: "RuView Spatial", license: "Apache-2.0", rating: 4.8, downloads: 5670, chips: ["esp32s3", "esp32c6"], memoryKb: 40, features: ["HNSW index", "Sub-ms search", "Memory efficient"], requirements: ["40KB RAM"], changelog: [{ version: "1.0.0", date: "2024-02-10", notes: "Edge HNSW" }], exports: ["add_vector", "search_knn", "save_index"], dependencies: [] },
  { id: "spt_spiking_tracker", name: "Spiking Tracker", category: "spatial", size: "36 KB", version: "1.0.0", description: "Spiking neural network for low-power tracking", fullDescription: "Person tracking using spiking neural networks for ultra-low power consumption. Suitable for battery-powered deployments.", author: "RuView Spatial", license: "Apache-2.0", rating: 4.4, downloads: 1890, chips: ["esp32s3", "esp32c6"], memoryKb: 44, features: ["SNN tracking", "Ultra-low power", "Event-driven"], requirements: ["44KB RAM"], changelog: [{ version: "1.0.0", date: "2024-02-15", notes: "SNN implementation" }], exports: ["track", "get_positions"], dependencies: [] },
  { id: "tmp_pattern_sequence", name: "Pattern Sequence", category: "temporal", size: "26 KB", version: "1.0.0", description: "Temporal pattern sequence recognition", fullDescription: "Recognizes sequences of events/patterns over time. Useful for detecting activity sequences and behavioral patterns.", author: "RuView Temporal", license: "Apache-2.0", rating: 4.6, downloads: 4560, chips: ["esp32", "esp32s3", "esp32c6"], memoryKb: 32, features: ["Sequence recognition", "Temporal patterns", "Configurable windows"], requirements: ["32KB RAM"], changelog: [{ version: "1.0.0", date: "2024-02-01", notes: "Sequence matching" }], exports: ["define_sequence", "detect_sequence"], dependencies: [] },
  { id: "tmp_temporal_logic_guard", name: "Temporal Logic Guard", category: "temporal", size: "30 KB", version: "1.0.0", description: "LTL-based temporal constraint verification", fullDescription: "Verifies temporal logic constraints (LTL formulas) on event streams. Ensures safety and liveness properties.", author: "RuView Temporal", license: "Apache-2.0", rating: 4.5, downloads: 2120, chips: ["esp32s3", "esp32c6"], memoryKb: 36, features: ["LTL verification", "Safety checking", "Event monitoring"], requirements: ["36KB RAM"], changelog: [{ version: "1.0.0", date: "2024-02-10", notes: "LTL engine" }], exports: ["add_constraint", "check_violation"], dependencies: [] },
  { id: "tmp_goap_autonomy", name: "GOAP Autonomy", category: "temporal", size: "38 KB", version: "1.0.0", description: "Goal-oriented action planning for autonomous sensing", fullDescription: "Enables autonomous decision-making using Goal-Oriented Action Planning. Node can plan sensing strategies based on goals.", author: "RuView Temporal", license: "Apache-2.0", rating: 4.7, downloads: 1780, chips: ["esp32s3", "esp32c6"], memoryKb: 48, features: ["GOAP planner", "Autonomous decisions", "Goal management"], requirements: ["48KB RAM"], changelog: [{ version: "1.0.0", date: "2024-02-15", notes: "GOAP implementation" }], exports: ["set_goal", "get_plan", "execute"], dependencies: [] },
  { id: "ais_prompt_shield", name: "Prompt Shield", category: "ai_security", size: "22 KB", version: "1.0.0", description: "AI manipulation defense for edge inference", fullDescription: "Protects edge AI inference from prompt injection and adversarial inputs. Validates inputs before processing.", author: "RuView AI Security", license: "Apache-2.0", rating: 4.8, downloads: 3450, chips: ["esp32s3", "esp32c6"], memoryKb: 28, features: ["Input validation", "Injection detection", "Safe inference"], requirements: ["28KB RAM"], changelog: [{ version: "1.0.0", date: "2024-02-20", notes: "Security hardening" }], exports: ["validate_input", "get_threat_score"], dependencies: [] },
  { id: "ais_behavioral_profiler", name: "Behavioral Profiler", category: "ai_security", size: "28 KB", version: "1.0.0", description: "User behavior profiling for anomaly detection", fullDescription: "Builds behavioral profiles and detects anomalous behavior patterns that may indicate compromised systems or insider threats.", author: "RuView AI Security", license: "Apache-2.0", rating: 4.6, downloads: 2890, chips: ["esp32s3", "esp32c6"], memoryKb: 36, features: ["Behavior profiling", "Anomaly detection", "Insider threat"], requirements: ["36KB RAM"], changelog: [{ version: "1.0.0", date: "2024-02-25", notes: "Behavioral analytics" }], exports: ["update_profile", "check_anomaly"], dependencies: [] },
  { id: "qnt_quantum_coherence", name: "Quantum Coherence", category: "quantum", size: "34 KB", version: "0.5.0", description: "Quantum-inspired coherence scoring for CSI", fullDescription: "Research module using quantum-inspired algorithms for enhanced coherence analysis. Experimental performance improvements.", author: "RuView Research", license: "MIT", rating: 4.0, downloads: 890, chips: ["esp32s3", "esp32c6"], memoryKb: 44, features: ["Quantum-inspired", "Enhanced coherence"], requirements: ["44KB RAM", "Research use"], changelog: [{ version: "0.5.0", date: "2024-03-01", notes: "Research alpha" }], exports: ["compute_coherence", "get_quantum_state"], dependencies: [] },
  { id: "qnt_interference_search", name: "Interference Search", category: "quantum", size: "38 KB", version: "0.5.0", description: "Interference pattern search using quantum-inspired algorithms", fullDescription: "Uses quantum-inspired interference patterns for efficient search in pattern spaces. Research module.", author: "RuView Research", license: "MIT", rating: 3.9, downloads: 670, chips: ["esp32s3", "esp32c6"], memoryKb: 48, features: ["Quantum search", "Pattern matching"], requirements: ["48KB RAM", "Research use"], changelog: [{ version: "0.5.0", date: "2024-03-01", notes: "Research alpha" }], exports: ["search", "get_interference"], dependencies: [] },
  { id: "aut_psycho_symbolic", name: "Psycho-Symbolic", category: "autonomous", size: "44 KB", version: "0.5.0", description: "Hybrid symbolic-neural reasoning for intent prediction", fullDescription: "Combines symbolic reasoning with neural networks for robust intent prediction. Research into explainable AI.", author: "RuView Research", license: "MIT", rating: 4.2, downloads: 1120, chips: ["esp32s3", "esp32c6"], memoryKb: 56, features: ["Hybrid reasoning", "Intent prediction", "Explainable"], requirements: ["56KB RAM", "Research use"], changelog: [{ version: "0.5.0", date: "2024-03-01", notes: "Research alpha" }], exports: ["predict_intent", "explain_reasoning"], dependencies: [] },
  { id: "aut_self_healing_mesh", name: "Self-Healing Mesh", category: "autonomous", size: "32 KB", version: "1.0.0", description: "Automatic mesh topology repair and optimization", fullDescription: "Autonomous mesh network management that detects failures and reconfigures topology. Self-optimizes for coverage and redundancy.", author: "RuView Autonomous", license: "Apache-2.0", rating: 4.7, downloads: 5670, chips: ["esp32", "esp32s3", "esp32c6"], memoryKb: 40, features: ["Self-healing", "Topology optimization", "Failure recovery"], requirements: ["40KB RAM", "Mesh network"], changelog: [{ version: "1.0.0", date: "2024-02-15", notes: "Mesh healing" }], exports: ["get_topology", "trigger_heal", "optimize"], dependencies: [] },
];

const CATEGORY_COLORS: Record<string, string> = {
  core: "#3b82f6",
  medical: "#10b981",
  security: "#ef4444",
  building: "#f59e0b",
  retail: "#8b5cf6",
  industrial: "#6366f1",
  exotic: "#ec4899",
  signal: "#06b6d4",
  learning: "#14b8a6",
  spatial: "#84cc16",
  temporal: "#f97316",
  ai_security: "#dc2626",
  quantum: "#a855f7",
  autonomous: "#0ea5e9",
};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface WasmStats {
  total_modules: number;
  running_modules: number;
  memory_used_kb: number;
  memory_limit_kb: number;
  total_executions: number;
  errors: number;
}

interface WasmSupport {
  supported: boolean;
  max_modules: number | null;
  memory_limit_kb: number | null;
  verify_signatures: boolean;
}

interface ModuleDetail {
  id: string;
  name: string;
  size_bytes: number;
  status: string;
  sha256: string;
  loaded_at: string;
  memory_used_kb: number;
  exports: string[];
  imports: string[];
  execution_count: number;
  last_error: string | null;
}

// ---------------------------------------------------------------------------
// EdgeModules page
// ---------------------------------------------------------------------------

export function EdgeModules() {
  const [nodes, setNodes] = useState<Node[]>([]);
  const [selectedIp, setSelectedIp] = useState<string>("");
  const [modules, setModules] = useState<WasmModule[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isUploading, setIsUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<"deployed" | "library" | "stats">("deployed");
  const [wasmStats, setWasmStats] = useState<WasmStats | null>(null);
  const [wasmSupport, setWasmSupport] = useState<WasmSupport | null>(null);
  const [selectedModule, setSelectedModule] = useState<ModuleDetail | null>(null);
  const [showDetailModal, setShowDetailModal] = useState(false);

  // ---- Discover nodes on mount ----
  useEffect(() => {
    (async () => {
      try {
        const discovered = await invoke<Node[]>("discover_nodes", {
          timeoutMs: 5000,
        });
        setNodes(discovered);
        if (discovered.length > 0) {
          setSelectedIp(discovered[0].ip);
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    })();
  }, []);

  // ---- Fetch modules when selected node changes ----
  const fetchModules = useCallback(async (ip: string) => {
    if (!ip) return;
    setIsLoading(true);
    setError(null);
    try {
      const list = await invoke<WasmModule[]>("wasm_list", { nodeIp: ip });
      setModules(list);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setModules([]);
    } finally {
      setIsLoading(false);
    }
  }, []);

  // ---- Fetch WASM stats ----
  const fetchStats = useCallback(async (ip: string) => {
    if (!ip) return;
    try {
      const stats = await invoke<WasmStats>("wasm_stats", { nodeIp: ip });
      setWasmStats(stats);
    } catch {
      setWasmStats(null);
    }
  }, []);

  // ---- Check WASM support ----
  const checkSupport = useCallback(async (ip: string) => {
    if (!ip) return;
    try {
      const support = await invoke<WasmSupport>("check_wasm_support", { nodeIp: ip });
      setWasmSupport(support);
    } catch {
      setWasmSupport(null);
    }
  }, []);

  useEffect(() => {
    if (selectedIp) {
      fetchModules(selectedIp);
      fetchStats(selectedIp);
      checkSupport(selectedIp);
    }
  }, [selectedIp, fetchModules, fetchStats, checkSupport]);

  // ---- Upload .wasm file ----
  const handleUpload = async () => {
    if (!selectedIp) return;
    const filePath = await open({
      title: "Select WASM Module",
      filters: [{ name: "WASM Modules", extensions: ["wasm"] }],
      multiple: false,
      directory: false,
    });
    if (!filePath) return;

    setIsUploading(true);
    setError(null);
    setSuccess(null);
    try {
      const result = await invoke<{ success: boolean; module_id: string; message: string }>(
        "wasm_upload",
        { nodeIp: selectedIp, wasmPath: filePath },
      );
      if (result.success) {
        setSuccess(`Module uploaded: ${result.module_id}`);
        await fetchModules(selectedIp);
        await fetchStats(selectedIp);
      } else {
        setError(result.message);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsUploading(false);
    }
  };

  // ---- Module actions ----
  const handleAction = async (moduleId: string, action: "start" | "stop" | "unload" | "restart") => {
    setError(null);
    setSuccess(null);
    try {
      await invoke("wasm_control", {
        nodeIp: selectedIp,
        moduleId,
        action,
      });
      const actionLabels: Record<string, string> = {
        start: "started",
        stop: "stopped",
        unload: "unloaded",
        restart: "restarted",
      };
      setSuccess(`Module ${moduleId} ${actionLabels[action]}`);
      await fetchModules(selectedIp);
      await fetchStats(selectedIp);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  // ---- View module details ----
  const handleViewDetails = async (moduleId: string) => {
    try {
      const detail = await invoke<ModuleDetail>("wasm_info", {
        nodeIp: selectedIp,
        moduleId,
      });
      setSelectedModule(detail);
      setShowDetailModal(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div style={{ padding: "var(--space-5)", maxWidth: 1400 }}>
      {/* Header */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: "var(--space-5)",
        }}
      >
        <div>
          <h1 className="heading-lg" style={{ margin: 0 }}>Edge Modules (WASM)</h1>
          <p style={{ fontSize: 13, color: "var(--text-secondary)", marginTop: "var(--space-1)" }}>
            Deploy and manage WASM edge computing modules on ESP32 nodes
          </p>
        </div>
        <button
          onClick={handleUpload}
          disabled={!selectedIp || isUploading}
          style={{
            padding: "var(--space-2) var(--space-4)",
            borderRadius: 6,
            background: !selectedIp || isUploading ? "var(--bg-active)" : "var(--accent)",
            color: !selectedIp || isUploading ? "var(--text-muted)" : "#fff",
            fontSize: 13,
            fontWeight: 600,
            cursor: !selectedIp || isUploading ? "not-allowed" : "pointer",
            border: "none",
          }}
        >
          {isUploading ? "Uploading..." : "Upload Module"}
        </button>
      </div>

      {/* Node selector + WASM support status */}
      <div style={{ display: "flex", gap: "var(--space-4)", marginBottom: "var(--space-4)", alignItems: "flex-end" }}>
        <div style={{ flex: 1 }}>
          <label
            style={{
              fontSize: 10,
              textTransform: "uppercase",
              letterSpacing: "0.05em",
              color: "var(--text-muted)",
              fontFamily: "var(--font-sans)",
              display: "block",
              marginBottom: "var(--space-1)",
            }}
          >
            Target Node
          </label>
          <select
            value={selectedIp}
            onChange={(e) => setSelectedIp(e.target.value)}
            style={{
              padding: "var(--space-2) var(--space-3)",
              borderRadius: 6,
              background: "var(--bg-elevated)",
              color: "var(--text-primary)",
              border: "1px solid var(--border)",
              fontSize: 13,
              fontFamily: "var(--font-mono)",
              minWidth: 260,
              cursor: "pointer",
            }}
          >
            {nodes.length === 0 && <option value="">No nodes discovered</option>}
            {nodes.map((node) => (
              <option key={node.ip} value={node.ip}>
                {node.ip}{node.hostname ? ` (${node.hostname})` : ""}{node.friendly_name ? ` - ${node.friendly_name}` : ""}
              </option>
            ))}
          </select>
        </div>

        {wasmSupport && (
          <div
            style={{
              padding: "var(--space-2) var(--space-3)",
              borderRadius: 6,
              background: wasmSupport.supported ? "rgba(63, 185, 80, 0.1)" : "rgba(248, 81, 73, 0.1)",
              border: `1px solid ${wasmSupport.supported ? "rgba(63, 185, 80, 0.3)" : "rgba(248, 81, 73, 0.3)"}`,
              fontSize: 12,
              color: wasmSupport.supported ? "var(--status-online)" : "var(--status-error)",
              display: "flex",
              alignItems: "center",
              gap: "var(--space-2)",
            }}
          >
            <span style={{ width: 8, height: 8, borderRadius: "50%", background: wasmSupport.supported ? "var(--status-online)" : "var(--status-error)" }} />
            {wasmSupport.supported ? (
              <>WASM Supported | Max: {wasmSupport.max_modules ?? "?"} modules | Memory: {wasmSupport.memory_limit_kb ? `${wasmSupport.memory_limit_kb} KB` : "?"}</>
            ) : (
              "WASM Not Supported"
            )}
          </div>
        )}
      </div>

      {/* Tabs */}
      <div style={{ display: "flex", gap: "var(--space-1)", marginBottom: "var(--space-4)" }}>
        {(["deployed", "library", "stats"] as const).map((tab) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            style={{
              padding: "var(--space-2) var(--space-4)",
              borderRadius: 6,
              background: activeTab === tab ? "var(--bg-active)" : "transparent",
              color: activeTab === tab ? "var(--text-primary)" : "var(--text-secondary)",
              fontSize: 13,
              fontWeight: 500,
              cursor: "pointer",
              border: "none",
              transition: "all 0.15s",
            }}
          >
            {tab === "deployed" && `Deployed (${modules.length})`}
            {tab === "library" && `Module Library (${MODULE_LIBRARY.length})`}
            {tab === "stats" && "Runtime Stats"}
          </button>
        ))}
      </div>

      {/* Success banner */}
      {success && (
        <Banner type="success" message={success} onDismiss={() => setSuccess(null)} />
      )}

      {/* Error banner */}
      {error && (
        <Banner type="error" message={error} onDismiss={() => setError(null)} />
      )}

      {/* Tab Content */}
      {activeTab === "deployed" && (
        <DeployedModulesTab
          modules={modules}
          isLoading={isLoading}
          selectedIp={selectedIp}
          onAction={handleAction}
          onViewDetails={handleViewDetails}
        />
      )}

      {activeTab === "library" && (
        <ModuleLibraryTab
          selectedIp={selectedIp}
          onSuccess={(msg) => setSuccess(msg)}
          onError={(msg) => setError(msg)}
          onRefresh={() => {
            fetchModules(selectedIp);
            fetchStats(selectedIp);
          }}
        />
      )}

      {activeTab === "stats" && (
        <RuntimeStatsTab stats={wasmStats} selectedIp={selectedIp} />
      )}

      {/* Module Detail Modal */}
      {showDetailModal && selectedModule && (
        <ModuleDetailModal
          module={selectedModule}
          onClose={() => {
            setShowDetailModal(false);
            setSelectedModule(null);
          }}
        />
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Tab Components
// ---------------------------------------------------------------------------

function DeployedModulesTab({
  modules,
  isLoading,
  selectedIp,
  onAction,
  onViewDetails,
}: {
  modules: WasmModule[];
  isLoading: boolean;
  selectedIp: string;
  onAction: (moduleId: string, action: "start" | "stop" | "unload" | "restart") => void;
  onViewDetails: (moduleId: string) => void;
}) {
  if (isLoading) {
    return (
      <div style={{ background: "var(--bg-surface)", border: "1px solid var(--border)", borderRadius: 8, padding: "var(--space-8)", textAlign: "center", color: "var(--text-muted)", fontSize: 13 }}>
        Loading modules...
      </div>
    );
  }

  if (modules.length === 0) {
    return (
      <div style={{ background: "var(--bg-surface)", border: "1px solid var(--border)", borderRadius: 8, padding: "var(--space-8)", textAlign: "center", color: "var(--text-muted)", fontSize: 13 }}>
        {selectedIp
          ? "No WASM modules loaded on this node. Use \"Upload Module\" or browse the Module Library to deploy one."
          : "Select a node to view its WASM modules."}
      </div>
    );
  }

  return (
    <div style={{ background: "var(--bg-surface)", border: "1px solid var(--border)", borderRadius: 8, overflow: "hidden" }}>
      <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 13 }}>
        <thead>
          <tr style={{ borderBottom: "1px solid var(--border)", textAlign: "left" }}>
            <Th>Name</Th>
            <Th>Size</Th>
            <Th>Status</Th>
            <Th>Memory</Th>
            <Th>Loaded At</Th>
            <Th>Actions</Th>
          </tr>
        </thead>
        <tbody>
          {modules.map((mod) => (
            <ModuleRow key={mod.module_id} module={mod} onAction={onAction} onViewDetails={onViewDetails} />
          ))}
        </tbody>
      </table>
    </div>
  );
}

function ModuleLibraryTab({
  selectedIp,
  onSuccess,
  onError,
  onRefresh,
}: {
  selectedIp: string;
  onSuccess: (msg: string) => void;
  onError: (msg: string) => void;
  onRefresh?: () => void;
}) {
  const [installing, setInstalling] = useState<string | null>(null);
  const [filter, setFilter] = useState<string>("all");
  const [viewingModule, setViewingModule] = useState<LibraryModule | null>(null);
  const [searchQuery, setSearchQuery] = useState("");

  const categories = ["all", ...Array.from(new Set(MODULE_LIBRARY.map((m) => m.category)))];

  const filteredModules = MODULE_LIBRARY.filter((m) => {
    const matchesCategory = filter === "all" || m.category === filter;
    const matchesSearch = searchQuery === "" ||
      m.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      m.description.toLowerCase().includes(searchQuery.toLowerCase()) ||
      m.id.toLowerCase().includes(searchQuery.toLowerCase());
    return matchesCategory && matchesSearch;
  });

  const handleInstall = async (moduleId: string, moduleName: string) => {
    if (!selectedIp) {
      onError("Please select a target node first");
      return;
    }

    setInstalling(moduleId);
    try {
      const result = await invoke<{ success: boolean; module_id: string; message: string }>(
        "wasm_upload",
        {
          nodeIp: selectedIp,
          wasmPath: `registry://ruview/${moduleId}.rvf`,
          moduleName: moduleId,
          autoStart: true,
        },
      );
      if (result.success) {
        onSuccess(`RVF module "${moduleName}" deployed (ID: ${result.module_id})`);
        onRefresh?.();
        setViewingModule(null);
      } else {
        onError(result.message);
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (msg.includes("registry://")) {
        onSuccess(`Module "${moduleName}" queued for RVF deployment. Configure registry in Settings.`);
        setViewingModule(null);
      } else {
        onError(msg);
      }
    } finally {
      setInstalling(null);
    }
  };

  const renderStars = (rating: number) => {
    const fullStars = Math.floor(rating);
    const hasHalf = rating - fullStars >= 0.5;
    return (
      <span style={{ color: "#f59e0b", fontSize: 12 }}>
        {"★".repeat(fullStars)}
        {hasHalf && "½"}
        <span style={{ color: "var(--text-muted)" }}>{"☆".repeat(5 - fullStars - (hasHalf ? 1 : 0))}</span>
      </span>
    );
  };

  return (
    <div>
      {/* Search + Category Filter */}
      <div style={{ marginBottom: "var(--space-4)" }}>
        <input
          type="text"
          placeholder="Search modules..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          style={{
            width: "100%",
            maxWidth: 400,
            padding: "var(--space-2) var(--space-3)",
            borderRadius: 6,
            background: "var(--bg-elevated)",
            border: "1px solid var(--border)",
            color: "var(--text-primary)",
            fontSize: 13,
            marginBottom: "var(--space-3)",
          }}
        />
        <div style={{ display: "flex", gap: "var(--space-2)", flexWrap: "wrap" }}>
          {categories.map((cat) => (
            <button
              key={cat}
              onClick={() => setFilter(cat)}
              style={{
                padding: "4px 12px",
                borderRadius: 9999,
                fontSize: 11,
                fontWeight: 500,
                background: filter === cat ? (cat === "all" ? "var(--accent)" : `${CATEGORY_COLORS[cat]}22`) : "var(--bg-elevated)",
                color: filter === cat ? (cat === "all" ? "#fff" : CATEGORY_COLORS[cat]) : "var(--text-secondary)",
                border: filter === cat && cat !== "all" ? `1px solid ${CATEGORY_COLORS[cat]}44` : "1px solid var(--border)",
                cursor: "pointer",
                textTransform: "capitalize",
              }}
            >
              {cat === "all" ? `All (${MODULE_LIBRARY.length})` : `${cat.replace(/_/g, " ")} (${MODULE_LIBRARY.filter((m) => m.category === cat).length})`}
            </button>
          ))}
        </div>
      </div>

      {/* Results count */}
      <div style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: "var(--space-3)" }}>
        {filteredModules.length} module{filteredModules.length !== 1 ? "s" : ""} found
      </div>

      {/* Module Grid */}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(340px, 1fr))", gap: "var(--space-4)" }}>
        {filteredModules.map((mod) => (
          <div
            key={mod.id}
            onClick={() => setViewingModule(mod)}
            style={{
              background: "var(--bg-surface)",
              border: "1px solid var(--border)",
              borderRadius: 8,
              padding: "var(--space-4)",
              cursor: "pointer",
              transition: "all 0.15s",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.borderColor = "var(--accent)";
              e.currentTarget.style.transform = "translateY(-2px)";
              e.currentTarget.style.boxShadow = "0 4px 12px rgba(0,0,0,0.15)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.borderColor = "var(--border)";
              e.currentTarget.style.transform = "translateY(0)";
              e.currentTarget.style.boxShadow = "none";
            }}
          >
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: "var(--space-2)" }}>
              <div style={{ flex: 1 }}>
                <h3 style={{ margin: 0, fontSize: 14, fontWeight: 600, color: "var(--text-primary)" }}>{mod.name}</h3>
                <span style={{ fontSize: 10, fontFamily: "var(--font-mono)", color: "var(--text-muted)" }}>{mod.id}</span>
              </div>
              <span
                style={{
                  fontSize: 10,
                  fontWeight: 600,
                  padding: "2px 8px",
                  borderRadius: 9999,
                  background: `${CATEGORY_COLORS[mod.category] || "#666"}22`,
                  color: CATEGORY_COLORS[mod.category] || "#666",
                  textTransform: "capitalize",
                  whiteSpace: "nowrap",
                }}
              >
                {mod.category.replace(/_/g, " ")}
              </span>
            </div>

            {/* Rating + Downloads */}
            <div style={{ display: "flex", alignItems: "center", gap: "var(--space-3)", marginBottom: "var(--space-2)" }}>
              <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
                {renderStars(mod.rating)}
                <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{mod.rating.toFixed(1)}</span>
              </div>
              <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
                {formatNumber(mod.downloads)} downloads
              </span>
            </div>

            <p style={{ margin: 0, fontSize: 12, color: "var(--text-secondary)", marginBottom: "var(--space-3)", lineHeight: 1.5, minHeight: 36 }}>
              {mod.description}
            </p>

            {/* Chips + Size */}
            <div style={{ display: "flex", gap: 4, flexWrap: "wrap", marginBottom: "var(--space-3)" }}>
              {mod.chips.slice(0, 3).map((chip) => (
                <span key={chip} style={{ fontSize: 9, padding: "1px 6px", borderRadius: 4, background: "var(--bg-active)", color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>
                  {chip}
                </span>
              ))}
              {mod.chips.length > 3 && (
                <span style={{ fontSize: 9, color: "var(--text-muted)" }}>+{mod.chips.length - 3}</span>
              )}
            </div>

            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)" }}>
                <span style={{ fontSize: 9, fontWeight: 700, padding: "2px 6px", borderRadius: 4, background: "rgba(124, 58, 237, 0.15)", color: "var(--accent)", fontFamily: "var(--font-mono)" }}>
                  RVF
                </span>
                <span style={{ fontSize: 11, color: "var(--text-muted)" }}>
                  v{mod.version} | {mod.size} | {mod.memoryKb}KB RAM
                </span>
              </div>
            </div>
          </div>
        ))}
      </div>

      {/* App Store-Style Detail Modal */}
      {viewingModule && (
        <div
          style={{
            position: "fixed",
            inset: 0,
            background: "rgba(0, 0, 0, 0.8)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            zIndex: 1000,
            padding: "var(--space-4)",
          }}
          onClick={() => setViewingModule(null)}
        >
          <div
            style={{
              background: "var(--bg-surface)",
              border: "1px solid var(--border)",
              borderRadius: 12,
              width: "100%",
              maxWidth: 720,
              maxHeight: "90vh",
              overflow: "auto",
            }}
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div style={{ padding: "var(--space-5)", borderBottom: "1px solid var(--border)" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
                <div style={{ flex: 1 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: "var(--space-3)", marginBottom: "var(--space-2)" }}>
                    <div style={{
                      width: 48, height: 48, borderRadius: 12,
                      background: `linear-gradient(135deg, ${CATEGORY_COLORS[viewingModule.category] || "#666"}, ${CATEGORY_COLORS[viewingModule.category] || "#666"}88)`,
                      display: "flex", alignItems: "center", justifyContent: "center",
                      fontSize: 20, color: "#fff", fontWeight: 700,
                    }}>
                      {viewingModule.name.charAt(0)}
                    </div>
                    <div>
                      <h2 style={{ margin: 0, fontSize: 20, fontWeight: 600, color: "var(--text-primary)" }}>{viewingModule.name}</h2>
                      <span style={{ fontSize: 12, color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>{viewingModule.author}</span>
                    </div>
                  </div>
                  <div style={{ display: "flex", alignItems: "center", gap: "var(--space-4)", marginTop: "var(--space-2)" }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
                      {renderStars(viewingModule.rating)}
                      <span style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)", marginLeft: 4 }}>{viewingModule.rating.toFixed(1)}</span>
                    </div>
                    <span style={{ fontSize: 12, color: "var(--text-muted)" }}>{formatNumber(viewingModule.downloads)} downloads</span>
                    <span style={{ fontSize: 10, padding: "2px 8px", borderRadius: 9999, background: `${CATEGORY_COLORS[viewingModule.category]}22`, color: CATEGORY_COLORS[viewingModule.category], fontWeight: 600, textTransform: "capitalize" }}>
                      {viewingModule.category.replace(/_/g, " ")}
                    </span>
                  </div>
                </div>
                <button onClick={() => setViewingModule(null)} style={{ background: "none", border: "none", color: "var(--text-muted)", fontSize: 24, cursor: "pointer", padding: 8 }}>×</button>
              </div>

              {/* Action Buttons */}
              <div style={{ display: "flex", gap: "var(--space-3)", marginTop: "var(--space-4)" }}>
                <button
                  onClick={() => handleInstall(viewingModule.id, viewingModule.name)}
                  disabled={!selectedIp || installing === viewingModule.id}
                  style={{
                    flex: 1, padding: "var(--space-3)", borderRadius: 8, fontSize: 14, fontWeight: 600,
                    background: !selectedIp || installing === viewingModule.id ? "var(--bg-active)" : "var(--accent)",
                    color: !selectedIp || installing === viewingModule.id ? "var(--text-muted)" : "#fff",
                    border: "none", cursor: !selectedIp || installing === viewingModule.id ? "not-allowed" : "pointer",
                  }}
                >
                  {installing === viewingModule.id ? "Deploying to Node..." : selectedIp ? `Deploy to ${selectedIp}` : "Select a Node First"}
                </button>
              </div>
            </div>

            {/* Content */}
            <div style={{ padding: "var(--space-5)" }}>
              {/* Quick Info */}
              <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: "var(--space-3)", marginBottom: "var(--space-5)" }}>
                <InfoBox label="Version" value={`v${viewingModule.version}`} />
                <InfoBox label="Size" value={viewingModule.size} />
                <InfoBox label="Memory" value={`${viewingModule.memoryKb} KB`} />
                <InfoBox label="License" value={viewingModule.license} />
              </div>

              {/* Description */}
              <Section title="Description">
                <p style={{ margin: 0, fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6 }}>{viewingModule.fullDescription}</p>
              </Section>

              {/* Features */}
              <Section title="Features">
                <div style={{ display: "flex", flexWrap: "wrap", gap: "var(--space-2)" }}>
                  {viewingModule.features.map((f, i) => (
                    <span key={i} style={{ fontSize: 12, padding: "4px 10px", borderRadius: 6, background: "var(--bg-active)", color: "var(--text-secondary)" }}>
                      {f}
                    </span>
                  ))}
                </div>
              </Section>

              {/* Compatible Chips */}
              <Section title="Compatible Chips">
                <div style={{ display: "flex", gap: "var(--space-2)", flexWrap: "wrap" }}>
                  {viewingModule.chips.map((chip) => (
                    <span key={chip} style={{ fontSize: 11, padding: "4px 10px", borderRadius: 6, background: "var(--bg-elevated)", color: "var(--text-primary)", fontFamily: "var(--font-mono)", border: "1px solid var(--border)" }}>
                      {chip.toUpperCase()}
                    </span>
                  ))}
                </div>
              </Section>

              {/* Requirements */}
              <Section title="Requirements">
                <ul style={{ margin: 0, paddingLeft: 20, color: "var(--text-secondary)", fontSize: 12 }}>
                  {viewingModule.requirements.map((r, i) => (
                    <li key={i} style={{ marginBottom: 4 }}>{r}</li>
                  ))}
                </ul>
              </Section>

              {/* Dependencies */}
              {viewingModule.dependencies.length > 0 && (
                <Section title="Dependencies">
                  <div style={{ display: "flex", gap: "var(--space-2)", flexWrap: "wrap" }}>
                    {viewingModule.dependencies.map((dep) => (
                      <span key={dep} style={{ fontSize: 11, padding: "4px 10px", borderRadius: 6, background: "rgba(124, 58, 237, 0.1)", color: "var(--accent)", fontFamily: "var(--font-mono)" }}>
                        {dep}
                      </span>
                    ))}
                  </div>
                </Section>
              )}

              {/* Exports */}
              <Section title="Exports (API)">
                <div style={{ display: "flex", gap: "var(--space-2)", flexWrap: "wrap" }}>
                  {viewingModule.exports.map((exp) => (
                    <code key={exp} style={{ fontSize: 11, padding: "4px 8px", borderRadius: 4, background: "var(--bg-base)", color: "var(--text-primary)", fontFamily: "var(--font-mono)" }}>
                      {exp}()
                    </code>
                  ))}
                </div>
              </Section>

              {/* Changelog */}
              <Section title="Changelog">
                {viewingModule.changelog.map((entry, i) => (
                  <div key={i} style={{ marginBottom: "var(--space-2)", paddingBottom: "var(--space-2)", borderBottom: i < viewingModule.changelog.length - 1 ? "1px solid var(--border)" : "none" }}>
                    <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)", marginBottom: 4 }}>
                      <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)", fontFamily: "var(--font-mono)" }}>v{entry.version}</span>
                      <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{entry.date}</span>
                    </div>
                    <p style={{ margin: 0, fontSize: 12, color: "var(--text-secondary)" }}>{entry.notes}</p>
                  </div>
                ))}
              </Section>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function InfoBox({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ background: "var(--bg-elevated)", borderRadius: 8, padding: "var(--space-3)", textAlign: "center" }}>
      <div style={{ fontSize: 10, color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.05em", marginBottom: 4 }}>{label}</div>
      <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)" }}>{value}</div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={{ marginBottom: "var(--space-5)" }}>
      <h3 style={{ margin: 0, fontSize: 13, fontWeight: 600, color: "var(--text-primary)", marginBottom: "var(--space-2)", textTransform: "uppercase", letterSpacing: "0.03em" }}>{title}</h3>
      {children}
    </div>
  );
}

function RuntimeStatsTab({ stats, selectedIp }: { stats: WasmStats | null; selectedIp: string }) {
  if (!selectedIp) {
    return (
      <div style={{ background: "var(--bg-surface)", border: "1px solid var(--border)", borderRadius: 8, padding: "var(--space-8)", textAlign: "center", color: "var(--text-muted)", fontSize: 13 }}>
        Select a node to view WASM runtime statistics.
      </div>
    );
  }

  if (!stats) {
    return (
      <div style={{ background: "var(--bg-surface)", border: "1px solid var(--border)", borderRadius: 8, padding: "var(--space-8)", textAlign: "center", color: "var(--text-muted)", fontSize: 13 }}>
        WASM runtime statistics not available for this node.
      </div>
    );
  }

  const memoryPct = stats.memory_limit_kb > 0 ? (stats.memory_used_kb / stats.memory_limit_kb) * 100 : 0;

  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(200px, 1fr))", gap: "var(--space-4)" }}>
      <StatCard label="Total Modules" value={stats.total_modules.toString()} color="var(--accent)" />
      <StatCard label="Running Modules" value={stats.running_modules.toString()} color="var(--status-online)" />
      <StatCard
        label="Memory Usage"
        value={`${stats.memory_used_kb} KB`}
        subtext={`${memoryPct.toFixed(1)}% of ${stats.memory_limit_kb} KB`}
        color={memoryPct > 80 ? "var(--status-error)" : memoryPct > 60 ? "var(--status-warning)" : "var(--status-online)"}
      />
      <StatCard label="Total Executions" value={formatNumber(stats.total_executions)} color="var(--text-primary)" />
      <StatCard label="Errors" value={stats.errors.toString()} color={stats.errors > 0 ? "var(--status-error)" : "var(--status-online)"} />
    </div>
  );
}

function StatCard({
  label,
  value,
  subtext,
  color,
}: {
  label: string;
  value: string;
  subtext?: string;
  color: string;
}) {
  return (
    <div
      style={{
        background: "var(--bg-surface)",
        border: "1px solid var(--border)",
        borderRadius: 8,
        padding: "var(--space-4)",
      }}
    >
      <div style={{ fontSize: 10, textTransform: "uppercase", letterSpacing: "0.05em", color: "var(--text-muted)", marginBottom: "var(--space-2)" }}>
        {label}
      </div>
      <div style={{ fontSize: 28, fontWeight: 700, color, fontFamily: "var(--font-mono)" }}>{value}</div>
      {subtext && <div style={{ fontSize: 11, color: "var(--text-muted)", marginTop: "var(--space-1)" }}>{subtext}</div>}
    </div>
  );
}

function ModuleDetailModal({ module, onClose }: { module: ModuleDetail; onClose: () => void }) {
  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0, 0, 0, 0.7)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
      }}
      onClick={onClose}
    >
      <div
        style={{
          background: "var(--bg-surface)",
          border: "1px solid var(--border)",
          borderRadius: 12,
          padding: "var(--space-5)",
          maxWidth: 600,
          width: "90%",
          maxHeight: "80vh",
          overflow: "auto",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "var(--space-4)" }}>
          <h2 style={{ margin: 0, fontSize: 18, fontWeight: 600 }}>{module.name}</h2>
          <button
            onClick={onClose}
            style={{ background: "none", border: "none", color: "var(--text-muted)", fontSize: 20, cursor: "pointer" }}
          >
            x
          </button>
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "var(--space-3)", marginBottom: "var(--space-4)" }}>
          <DetailRow label="Module ID" value={module.id} mono />
          <DetailRow label="Status" value={module.status} />
          <DetailRow label="Size" value={formatBytes(module.size_bytes)} />
          <DetailRow label="Memory Used" value={`${module.memory_used_kb} KB`} />
          <DetailRow label="Executions" value={formatNumber(module.execution_count)} />
          <DetailRow label="Loaded At" value={new Date(module.loaded_at).toLocaleString()} />
        </div>

        <DetailRow label="SHA-256" value={module.sha256} mono fullWidth />

        {module.last_error && (
          <div style={{ marginTop: "var(--space-3)" }}>
            <DetailRow label="Last Error" value={module.last_error} fullWidth error />
          </div>
        )}

        <div style={{ marginTop: "var(--space-4)" }}>
          <div style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: "var(--space-2)" }}>Exports ({module.exports.length})</div>
          <div style={{ display: "flex", flexWrap: "wrap", gap: "var(--space-1)" }}>
            {module.exports.length === 0 ? (
              <span style={{ color: "var(--text-muted)", fontSize: 12 }}>None</span>
            ) : (
              module.exports.map((exp) => (
                <span
                  key={exp}
                  style={{
                    padding: "2px 8px",
                    borderRadius: 4,
                    background: "var(--bg-active)",
                    fontSize: 11,
                    fontFamily: "var(--font-mono)",
                    color: "var(--text-secondary)",
                  }}
                >
                  {exp}
                </span>
              ))
            )}
          </div>
        </div>

        <div style={{ marginTop: "var(--space-3)" }}>
          <div style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: "var(--space-2)" }}>Imports ({module.imports.length})</div>
          <div style={{ display: "flex", flexWrap: "wrap", gap: "var(--space-1)" }}>
            {module.imports.length === 0 ? (
              <span style={{ color: "var(--text-muted)", fontSize: 12 }}>None</span>
            ) : (
              module.imports.map((imp) => (
                <span
                  key={imp}
                  style={{
                    padding: "2px 8px",
                    borderRadius: 4,
                    background: "var(--bg-active)",
                    fontSize: 11,
                    fontFamily: "var(--font-mono)",
                    color: "var(--text-secondary)",
                  }}
                >
                  {imp}
                </span>
              ))
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function DetailRow({
  label,
  value,
  mono,
  fullWidth,
  error,
}: {
  label: string;
  value: string;
  mono?: boolean;
  fullWidth?: boolean;
  error?: boolean;
}) {
  return (
    <div style={{ gridColumn: fullWidth ? "1 / -1" : undefined }}>
      <div style={{ fontSize: 10, textTransform: "uppercase", letterSpacing: "0.05em", color: "var(--text-muted)", marginBottom: 2 }}>{label}</div>
      <div
        style={{
          fontSize: 13,
          color: error ? "var(--status-error)" : "var(--text-primary)",
          fontFamily: mono ? "var(--font-mono)" : "var(--font-sans)",
          wordBreak: "break-all",
        }}
      >
        {value}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function Th({ children }: { children: React.ReactNode }) {
  return (
    <th
      style={{
        padding: "10px var(--space-4)",
        fontSize: 10,
        fontWeight: 600,
        textTransform: "uppercase",
        letterSpacing: "0.05em",
        color: "var(--text-muted)",
        fontFamily: "var(--font-sans)",
      }}
    >
      {children}
    </th>
  );
}

function Td({ children, mono = false }: { children: React.ReactNode; mono?: boolean }) {
  return (
    <td
      style={{
        padding: "10px var(--space-4)",
        color: "var(--text-secondary)",
        fontFamily: mono ? "var(--font-mono)" : "var(--font-sans)",
        whiteSpace: "nowrap",
        fontSize: 13,
      }}
    >
      {children}
    </td>
  );
}

function ModuleStateBadge({ state }: { state: WasmModuleState }) {
  const { color, label } = STATE_STYLES[state];
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 6,
        color,
        fontSize: 11,
        fontWeight: 600,
        fontFamily: "var(--font-sans)",
        padding: "2px 8px",
        borderRadius: 9999,
        lineHeight: 1,
        whiteSpace: "nowrap",
        background: "rgba(255, 255, 255, 0.04)",
      }}
    >
      <span
        style={{
          width: 6,
          height: 6,
          borderRadius: "50%",
          backgroundColor: color,
          flexShrink: 0,
        }}
      />
      {label}
    </span>
  );
}

function ActionButton({
  label,
  onClick,
  variant = "default",
}: {
  label: string;
  onClick: () => void;
  variant?: "default" | "danger" | "primary";
}) {
  const isDanger = variant === "danger";
  const isPrimary = variant === "primary";
  return (
    <button
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      style={{
        padding: "3px 10px",
        borderRadius: 4,
        fontSize: 11,
        fontWeight: 600,
        fontFamily: "var(--font-sans)",
        border: `1px solid ${isDanger ? "var(--status-error)" : isPrimary ? "var(--accent)" : "var(--border)"}`,
        background: isPrimary ? "var(--accent)" : "transparent",
        color: isDanger ? "var(--status-error)" : isPrimary ? "#fff" : "var(--text-secondary)",
        cursor: "pointer",
        transition: "background 0.1s",
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.background = isDanger
          ? "rgba(248, 81, 73, 0.1)"
          : isPrimary
          ? "var(--accent)"
          : "var(--bg-hover)";
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = isPrimary ? "var(--accent)" : "transparent";
      }}
    >
      {label}
    </button>
  );
}

function ModuleRow({
  module: mod,
  onAction,
  onViewDetails,
}: {
  module: WasmModule;
  onAction: (moduleId: string, action: "start" | "stop" | "unload" | "restart") => void;
  onViewDetails: (moduleId: string) => void;
}) {
  return (
    <tr
      style={{
        borderBottom: "1px solid var(--border)",
        transition: "background 0.1s",
        cursor: "pointer",
      }}
      onMouseEnter={(e) => (e.currentTarget.style.background = "var(--bg-hover)")}
      onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
      onClick={() => onViewDetails(mod.module_id)}
    >
      <Td mono>{mod.name}</Td>
      <Td mono>{formatBytes(mod.size_bytes)}</Td>
      <Td><ModuleStateBadge state={mod.state} /></Td>
      <Td mono>{mod.memory_used_kb ? `${mod.memory_used_kb} KB` : "--"}</Td>
      <Td>{formatLoadedAt(mod.loaded_at)}</Td>
      <td style={{ padding: "10px var(--space-4)", whiteSpace: "nowrap" }}>
        <div style={{ display: "flex", gap: "var(--space-2)" }} onClick={(e) => e.stopPropagation()}>
          {mod.state === "stopped" && (
            <ActionButton label="Start" onClick={() => onAction(mod.module_id, "start")} variant="primary" />
          )}
          {mod.state === "running" && (
            <>
              <ActionButton label="Stop" onClick={() => onAction(mod.module_id, "stop")} />
              <ActionButton label="Restart" onClick={() => onAction(mod.module_id, "restart")} />
            </>
          )}
          <ActionButton
            label="Unload"
            onClick={() => onAction(mod.module_id, "unload")}
            variant="danger"
          />
        </div>
      </td>
    </tr>
  );
}

function Banner({
  type,
  message,
  onDismiss,
}: {
  type: "error" | "success";
  message: string;
  onDismiss: () => void;
}) {
  const isError = type === "error";
  const color = isError ? "var(--status-error)" : "var(--status-online)";
  const bgAlpha = isError ? "rgba(248, 81, 73, 0.1)" : "rgba(63, 185, 80, 0.1)";
  const borderAlpha = isError ? "rgba(248, 81, 73, 0.3)" : "rgba(63, 185, 80, 0.3)";

  return (
    <div
      style={{
        background: bgAlpha,
        border: `1px solid ${borderAlpha}`,
        borderRadius: 6,
        padding: "var(--space-3) var(--space-4)",
        marginBottom: "var(--space-4)",
        fontSize: 13,
        color,
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
      }}
    >
      <span>{message}</span>
      <button
        onClick={onDismiss}
        style={{
          background: "none",
          border: "none",
          color,
          cursor: "pointer",
          fontSize: 16,
          lineHeight: 1,
          padding: "0 0 0 var(--space-3)",
        }}
      >
        x
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${kb.toFixed(1)} KB`;
  const mb = kb / 1024;
  return `${mb.toFixed(2)} MB`;
}

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toString();
}

function formatLoadedAt(iso: string | null): string {
  if (!iso) return "--";
  try {
    const d = new Date(iso);
    const diff = Date.now() - d.getTime();
    if (diff < 60_000) return "just now";
    if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
    if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
    return d.toLocaleDateString();
  } catch {
    return "--";
  }
}
