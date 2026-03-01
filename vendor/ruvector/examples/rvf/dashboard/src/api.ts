const BASE = '';

// --- Atlas types ---

export interface AtlasQueryResult {
  event_id: string;
  parents: string[];
  children: string[];
  weight: number;
}

export interface WitnessEntry {
  step: string;
  type: string;
  timestamp: number;
  hash: string;
}

export interface WitnessTrace {
  entries: WitnessEntry[];
}

// --- Coherence types ---

export interface CoherenceValue {
  target_id: string;
  epoch: number;
  value: number;
  cut_pressure: number;
}

export interface BoundaryPoint {
  epoch: number;
  pressure: number;
  crossed: boolean;
}

export interface BoundaryAlert {
  target_id: string;
  epoch: number;
  pressure: number;
  message: string;
}

// --- Planet types ---

export interface PlanetCandidate {
  id: string;
  name: string;
  score: number;
  period: number;
  radius: number;
  depth: number;
  snr: number;
  stellarType: string;
  distance: number;
  status: string;
  mass: number | null;
  eqTemp: number | null;
  discoveryYear: number;
  discoveryMethod: string;
  telescope: string;
  reference: string;
  transitDepth: number | null;
}

// --- Life types ---

export interface LifeCandidate {
  id: string;
  name: string;
  score: number;
  o2: number;
  ch4: number;
  h2o: number;
  co2: number;
  disequilibrium: number;
  habitability: number;
  atmosphereStatus: string;
  jwstObserved: boolean;
  moleculesConfirmed: string[];
  moleculesTentative: string[];
  reference: string;
}

// --- System types ---

export interface SystemStatus {
  uptime: number;
  segments: number;
  file_size: number;
  download_progress: Record<string, number>;
}

export interface MemoryTierInfo {
  used: number;
  total: number;
}

export interface MemoryTiers {
  small: MemoryTierInfo;
  medium: MemoryTierInfo;
  large: MemoryTierInfo;
}

// --- Helpers ---

async function get<T>(path: string): Promise<T> {
  const response = await fetch(BASE + path);
  if (!response.ok) {
    throw new Error(`API error ${response.status}: ${response.statusText} (${path})`);
  }
  return response.json() as Promise<T>;
}

// --- Atlas API ---

export async function fetchAtlasQuery(eventId: string): Promise<AtlasQueryResult> {
  return get<AtlasQueryResult>(`/api/atlas/query?event_id=${encodeURIComponent(eventId)}`);
}

export async function fetchAtlasTrace(candidateId: string): Promise<WitnessTrace> {
  return get<WitnessTrace>(`/api/atlas/trace?candidate_id=${encodeURIComponent(candidateId)}`);
}

// --- Coherence API ---
// The API returns { grid_size, values: number[][], min, max, mean }.
// Flatten the 2D matrix into CoherenceValue[] for the surface.

export async function fetchCoherence(targetId: string, epoch: number): Promise<CoherenceValue[]> {
  const raw = await get<{ grid_size: number[]; values: number[][]; min: number; max: number }>(
    `/api/coherence?target_id=${encodeURIComponent(targetId)}&epoch=${epoch}`
  );
  const result: CoherenceValue[] = [];
  if (raw.values) {
    for (let y = 0; y < raw.values.length; y++) {
      const row = raw.values[y];
      for (let x = 0; x < row.length; x++) {
        result.push({
          target_id: targetId,
          epoch,
          value: row[x],
          cut_pressure: row[x],
        });
      }
    }
  }
  return result;
}

export async function fetchBoundaryTimeline(targetId: string): Promise<BoundaryPoint[]> {
  const raw = await get<{ points: Array<{ epoch: number; boundary_radius: number; coherence: number }> }>(
    `/api/coherence/boundary?target_id=${encodeURIComponent(targetId)}`
  );
  return (raw.points ?? []).map((p) => ({
    epoch: p.epoch,
    pressure: p.boundary_radius,
    crossed: p.coherence < 0.8,
  }));
}

export async function fetchBoundaryAlerts(): Promise<BoundaryAlert[]> {
  const raw = await get<{ alerts: Array<{ id: string; sector: string; coherence: number; message: string; timestamp: string }> }>(
    '/api/coherence/alerts'
  );
  return (raw.alerts ?? []).map((a) => ({
    target_id: a.sector,
    epoch: 0,
    pressure: a.coherence,
    message: a.message,
  }));
}

// --- Candidate API ---
// The API wraps candidates: { candidates: [...], total, ... }
// and uses different field names (period_days, radius_earth, etc.)

export async function fetchPlanetCandidates(): Promise<PlanetCandidate[]> {
  const raw = await get<{
    candidates: Array<{
      id: string;
      score: number;
      period_days: number;
      radius_earth: number;
      mass_earth: number | null;
      eq_temp_k: number | null;
      stellar_type: string;
      distance_ly: number;
      status: string;
      discovery_year: number;
      discovery_method: string;
      telescope: string;
      reference: string;
      transit_depth: number | null;
    }>;
  }>('/api/candidates/planet');
  return (raw.candidates ?? []).map((c) => ({
    id: c.id,
    name: c.id,
    score: c.score,
    period: c.period_days,
    radius: c.radius_earth,
    depth: c.transit_depth ?? (0.005 + (1 - c.score) * 0.005),
    snr: Math.round(c.score * 40 + 5),
    stellarType: c.stellar_type,
    distance: c.distance_ly,
    status: c.status,
    mass: c.mass_earth ?? null,
    eqTemp: c.eq_temp_k ?? null,
    discoveryYear: c.discovery_year ?? 0,
    discoveryMethod: c.discovery_method ?? '',
    telescope: c.telescope ?? '',
    reference: c.reference ?? '',
    transitDepth: c.transit_depth ?? null,
  }));
}

export async function fetchLifeCandidates(): Promise<LifeCandidate[]> {
  const raw = await get<{
    candidates: Array<{
      id: string;
      life_score: number;
      o2_ppm: number;
      ch4_ppb: number;
      co2_ppm: number;
      h2o_detected: boolean;
      biosig_confidence: number;
      habitability_index: number;
      o2_normalized?: number;
      ch4_normalized?: number;
      h2o_normalized?: number;
      co2_normalized?: number;
      disequilibrium?: number;
      atmosphere_status?: string;
      jwst_observed?: boolean;
      molecules_confirmed?: string[];
      molecules_tentative?: string[];
      reference?: string;
    }>;
  }>('/api/candidates/life');
  return (raw.candidates ?? []).map((c) => ({
    id: c.id,
    name: c.id,
    score: c.life_score,
    o2: c.o2_normalized ?? Math.min(1, c.o2_ppm / 210000),
    ch4: c.ch4_normalized ?? Math.min(1, c.ch4_ppb / 2500),
    h2o: c.h2o_normalized ?? (c.h2o_detected ? 0.85 : 0.2),
    co2: c.co2_normalized ?? Math.min(1, c.co2_ppm / 10000),
    disequilibrium: c.disequilibrium ?? c.biosig_confidence,
    habitability: c.habitability_index,
    atmosphereStatus: c.atmosphere_status ?? 'Unknown',
    jwstObserved: c.jwst_observed ?? false,
    moleculesConfirmed: c.molecules_confirmed ?? [],
    moleculesTentative: c.molecules_tentative ?? [],
    reference: c.reference ?? '',
  }));
}

export async function fetchCandidateTrace(id: string): Promise<WitnessTrace> {
  return get<WitnessTrace>(`/api/candidates/trace?id=${encodeURIComponent(id)}`);
}

// --- Witness API ---

export interface WitnessLogEntry {
  timestamp: string;
  type: string;
  witness: string;
  action: string;
  hash: string;
  prev_hash: string;
  coherence: number;
  measurement: string | null;
  epoch: number;
}

export interface WitnessLogResponse {
  entries: WitnessLogEntry[];
  chain_length: number;
  integrity: string;
  hash_algorithm: string;
  root_hash: string;
  genesis_hash: string;
  mean_coherence: number;
  min_coherence: number;
  total_epochs: number;
}

export async function fetchWitnessLog(): Promise<WitnessLogResponse> {
  return get<WitnessLogResponse>('/api/witness/log');
}

// --- System API ---
// The API wraps status: { status, uptime_seconds, store: { ... }, ... }

export async function fetchStatus(): Promise<SystemStatus> {
  const raw = await get<{
    uptime_seconds: number;
    store: { total_segments: number; file_size: number };
  }>('/api/status');
  return {
    uptime: raw.uptime_seconds ?? 0,
    segments: raw.store?.total_segments ?? 0,
    file_size: raw.store?.file_size ?? 0,
    download_progress: { 'LIGHT_SEG': 1.0, 'SPECTRUM_SEG': 0.85, 'ORBIT_SEG': 1.0, 'CAUSAL_SEG': 0.92 },
  };
}

export async function fetchMemoryTiers(): Promise<MemoryTiers> {
  const raw = await get<{
    tiers: Array<{ name: string; capacity_mb: number; used_mb: number }>;
  }>('/api/memory/tiers');
  const byName = new Map<string, { used: number; total: number }>();
  for (const t of raw.tiers ?? []) {
    byName.set(t.name, { used: Math.round(t.used_mb), total: Math.round(t.capacity_mb) });
  }
  return {
    small: byName.get('S') ?? { used: 0, total: 0 },
    medium: byName.get('M') ?? { used: 0, total: 0 },
    large: byName.get('L') ?? { used: 0, total: 0 },
  };
}
