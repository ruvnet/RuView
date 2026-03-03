import {
  BREATHING_BAND_AMPLITUDE,
  BREATHING_BAND_MIN,
  BREATHING_BPM_MAX,
  BREATHING_BPM_MIN,
  HEART_BPM_MAX,
  HEART_BPM_MIN,
  MOTION_BAND_AMPLITUDE,
  MOTION_BAND_MIN,
  RSSI_AMPLITUDE_DBM,
  RSSI_BASE_DBM,
  SIMULATION_GRID_SIZE,
  SIMULATION_TICK_INTERVAL_MS,
  SIGNAL_FIELD_PRESENCE_LEVEL,
  VARIANCE_AMPLITUDE,
  VARIANCE_BASE,
} from '@/constants/simulation';
import type { SensingFrame } from '@/types/sensing';

function gaussian(x: number, y: number, cx: number, cy: number, sigma: number): number {
  const dx = x - cx;
  const dy = y - cy;
  return Math.exp(-(dx * dx + dy * dy) / (2 * sigma * sigma));
}

function clamp(v: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, v));
}

export function generateSimulatedData(timeMs = Date.now()): SensingFrame {
  const t = timeMs / 1000;

  const baseRssi = RSSI_BASE_DBM + Math.sin(t * 0.5) * RSSI_AMPLITUDE_DBM;
  const variance = VARIANCE_BASE + Math.sin(t * 0.1) * VARIANCE_AMPLITUDE;
  const motionBand = MOTION_BAND_MIN + Math.abs(Math.sin(t * 0.3)) * MOTION_BAND_AMPLITUDE;
  const breathingBand = BREATHING_BAND_MIN + Math.abs(Math.sin(t * 0.05)) * BREATHING_BAND_AMPLITUDE;

  const isPresent = variance > SIGNAL_FIELD_PRESENCE_LEVEL;
  const isActive = motionBand > 0.12;

  const grid = SIMULATION_GRID_SIZE;
  const cx = grid / 2;
  const cy = grid / 2;
  const bodyX = cx + 3 * Math.sin(t * 0.2);
  const bodyY = cy + 2 * Math.cos(t * 0.15);
  const breathX = cx + 4 * Math.sin(t * 0.04);
  const breathY = cy + 4 * Math.cos(t * 0.04);

  const values: number[] = [];
  for (let z = 0; z < grid; z += 1) {
    for (let x = 0; x < grid; x += 1) {
      let value = Math.max(0, 1 - Math.sqrt((x - cx) ** 2 + (z - cy) ** 2) / (grid * 0.7)) * 0.3;
      value += gaussian(x, z, bodyX, bodyY, 3.4) * (0.3 + motionBand * 3);
      value += gaussian(x, z, breathX, breathY, 6) * (0.15 + breathingBand * 2);
      if (!isPresent) {
        value *= 0.7;
      }
      values.push(clamp(value, 0, 1));
    }
  }

  const dominantFreqHz = 0.3 + Math.sin(t * 0.02) * 0.1;
  const breathingBpm = BREATHING_BPM_MIN + ((Math.sin(t * 0.07) + 1) * 0.5) * (BREATHING_BPM_MAX - BREATHING_BPM_MIN);
  const hrProxy = HEART_BPM_MIN + ((Math.sin(t * 0.09) + 1) * 0.5) * (HEART_BPM_MAX - HEART_BPM_MIN);
  const confidence = 0.6 + Math.abs(Math.sin(t * 0.03)) * 0.4;

  return {
    type: 'sensing_update',
    timestamp: timeMs,
    source: 'simulated',
    tick: Math.floor(t / (SIMULATION_TICK_INTERVAL_MS / 1000)),
    nodes: [
      {
        node_id: 1,
        rssi_dbm: baseRssi,
        position: [2, 0, 1.5],
        amplitude: [baseRssi],
        subcarrier_count: 1,
      },
    ],
    features: {
      mean_rssi: baseRssi,
      variance,
      motion_band_power: motionBand,
      breathing_band_power: breathingBand,
      spectral_entropy: 1 - clamp(Math.abs(dominantFreqHz - 0.3), 0, 1),
      std: Math.sqrt(Math.abs(variance)),
      dominant_freq_hz: dominantFreqHz,
      change_points: Math.max(0, Math.floor(variance * 2)),
      spectral_power: motionBand + breathingBand,
    },
    classification: {
      motion_level: isActive ? 'active' : isPresent ? 'present_still' : 'absent',
      presence: isPresent,
      confidence: isPresent ? 0.75 + Math.abs(Math.sin(t * 0.03)) * 0.2 : 0.5 + Math.abs(Math.cos(t * 0.03)) * 0.3,
    },
    signal_field: {
      grid_size: [grid, 1, grid],
      values,
    },
    vital_signs: {
      breathing_bpm: breathingBpm,
      hr_proxy_bpm: hrProxy,
      confidence,
    },
    estimated_persons: isPresent ? 1 : 0,
  };
}
