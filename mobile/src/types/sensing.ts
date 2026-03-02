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
  breathing_bpm: number;
  hr_proxy_bpm: number;
  confidence: number;
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
}
