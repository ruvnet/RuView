export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected' | 'simulated';

export interface SensingNode {
  node_id: number;
  rssi_dbm: number;
  position: [number, number, number];
  amplitude?: number[];
  subcarrier_count?: number;
}

export interface FeatureSet {
  mean_rssi: number;
  variance: number;
  motion_band_power: number;
  breathing_band_power: number;
  spectral_entropy: number;
  std?: number;
  dominant_freq_hz?: number;
  change_points?: number;
  spectral_power?: number;
}

export interface Classification {
  motion_level: 'absent' | 'present_still' | 'active';
  presence: boolean;
  confidence: number;
}

export interface SignalField {
  grid_size: [number, number, number];
  values: number[];
}

export interface VitalsData {
  breathing_bpm?: number;
  hr_proxy_bpm?: number;
  // Rust sensing server uses these field names
  breathing_rate_bpm?: number;
  breathing_confidence?: number;
  heart_rate_bpm?: number;
  heart_confidence?: number;
  confidence?: number;
}

export interface PoseKeypoint {
  name?: string;
  x: number;
  y: number;
  z: number;
  confidence: number;
}

export interface PersonDetection {
  id?: number;
  confidence: number;
  keypoints: PoseKeypoint[];
}

export interface SensingFrame {
  type?: string;
  timestamp?: number;
  source?: string;
  tick?: number;
  nodes: SensingNode[];
  features: FeatureSet;
  classification: Classification;
  signal_field: SignalField;
  vital_signs?: VitalsData;
  pose_keypoints?: [number, number, number, number][];
  persons?: PersonDetection[];
  posture?: string;
  signal_quality_score?: number;
  /** Estimated person count from CSI feature heuristics (1-3 for single ESP32). */
  estimated_persons?: number;
}
