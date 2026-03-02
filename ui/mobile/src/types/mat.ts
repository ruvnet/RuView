export enum DisasterType {
  BuildingCollapse = 0,
  Earthquake = 1,
  Landslide = 2,
  Avalanche = 3,
  Flood = 4,
  MineCollapse = 5,
  Industrial = 6,
  TunnelCollapse = 7,
  Unknown = 8,
}

export enum TriageStatus {
  Immediate = 0,
  Delayed = 1,
  Minor = 2,
  Deceased = 3,
  Unknown = 4,
}

export enum ZoneStatus {
  Active = 0,
  Paused = 1,
  Complete = 2,
  Inaccessible = 3,
}

export enum AlertPriority {
  Critical = 0,
  High = 1,
  Medium = 2,
  Low = 3,
}

export interface DisasterEvent {
  event_id: string;
  disaster_type: DisasterType;
  latitude: number;
  longitude: number;
  description: string;
}

export interface RectangleZone {
  id: string;
  name: string;
  zone_type: 'rectangle';
  status: ZoneStatus;
  scan_count: number;
  detection_count: number;
  bounds_json: string;
}

export interface CircleZone {
  id: string;
  name: string;
  zone_type: 'circle';
  status: ZoneStatus;
  scan_count: number;
  detection_count: number;
  bounds_json: string;
}

export type ScanZone = RectangleZone | CircleZone;

export interface Survivor {
  id: string;
  zone_id: string;
  x: number;
  y: number;
  depth: number;
  triage_status: TriageStatus;
  triage_color: string;
  confidence: number;
  breathing_rate: number;
  heart_rate: number;
  first_detected: string;
  last_updated: string;
  is_deteriorating: boolean;
}

export interface Alert {
  id: string;
  survivor_id: string;
  priority: AlertPriority;
  title: string;
  message: string;
  recommended_action: string;
  triage_status: TriageStatus;
  location_x: number;
  location_y: number;
  created_at: string;
  priority_color: string;
}
