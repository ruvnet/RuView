import type { SensingFrame } from './sensing';

export interface PoseStatus {
  status?: string;
  healthy?: boolean;
  services?: Record<string, unknown>;
  streaming?: {
    active?: boolean;
    active_connections?: number;
    total_messages?: number;
    uptime?: number;
    [key: string]: unknown;
  };
  timestamp?: string;
  [key: string]: unknown;
}

export interface ZoneConfig {
  id: string;
  name: string;
  type: 'rectangle' | 'circle' | 'polygon';
  status?: string;
  scan_count?: number;
  detection_count?: number;
  bounds?: Record<string, unknown>;
}

export interface HistoricalFrames {
  frames: SensingFrame[];
  limit?: number;
  total?: number;
}

export interface ApiError {
  message: string;
  status?: number;
  code?: string;
  details?: unknown;
}
