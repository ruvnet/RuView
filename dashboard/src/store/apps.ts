/* RuView Edge App Store registry.
 *
 * Catalog of every WASM edge module shipping in the workspace plus the
 * `nvsim` simulator itself. Each entry maps to a hot-loadable algorithm
 * the dashboard can run in-browser (WASM transport) or push to a real
 * ESP32-S3 mesh (WS transport, deployed via WASM3 — ADR-040 Tier 3).
 *
 * Categories (ADR-041 event-ID ranges):
 *   med  100–199  Medical & health
 *   sec  200–299  Security & safety
 *   bld  300–399  Smart building
 *   ret  400–499  Retail & hospitality
 *   ind  500–599  Industrial
 *   sig  600–619  Signal-processing primitives
 *   lrn  620–639  Online learning
 *   spt  640–659  Spatial / graph
 *   tmp  640–660  Temporal logic / planning
 *   ais  700–719  AI safety
 *   qnt  720–739  Quantum-flavoured signal
 *   aut  740–759  Autonomy / mesh
 *   exo  650–699  Exotic / research
 *   sim  —       Pipeline simulators (nvsim)
 *
 * The `crate` field names the Cargo crate that owns the implementation.
 * `wasmEdge` apps are compiled out of `wifi-densepose-wasm-edge`;
 * `nvsim` apps come from `nvsim`. Future apps may target other crates.
 */

export type AppCategory =
  | 'sim'
  | 'med'
  | 'sec'
  | 'bld'
  | 'ret'
  | 'ind'
  | 'sig'
  | 'lrn'
  | 'spt'
  | 'tmp'
  | 'ais'
  | 'qnt'
  | 'aut'
  | 'exo';

/** What actually happens when a card's toggle is on.
 * - `running` — the algorithm is genuinely running in the browser right now
 *   (e.g. `nvsim` itself, which is the simulator the dashboard fronts).
 * - `simulated` — a pared-down version of the algorithm runs against nvsim's
 *   live magnetic frame stream as a *proxy* for its native CSI input.
 *   Emits real i32 event IDs into the console feed; output is illustrative,
 *   not engineering-grade. Listed apps' Rust source is real, builds for
 *   wasm32-unknown-unknown, and passes its native unit tests.
 * - `mesh-only` — algorithm needs CSI subcarrier data from a real ESP32-S3
 *   mesh (or a future CSI simulator). Toggling persists the selection so
 *   the WS transport can push activation when connected. */
export type AppRuntime = 'running' | 'simulated' | 'mesh-only';

export interface AppManifest {
  /** Stable kebab-case id; matches the wasm-edge module name (e.g. `med_sleep_apnea`). */
  id: string;
  /** Human-readable name. */
  name: string;
  /** Category short-code. */
  category: AppCategory;
  /** Cargo crate the implementation lives in. */
  crate: 'nvsim' | 'wifi-densepose-wasm-edge' | string;
  /** One-liner description. */
  summary: string;
  /** Optional longer markdown body. */
  body?: string;
  /** Numeric event IDs this app emits (i32 codes from `event_types` mod). */
  events?: number[];
  /** Compute budget tier the module advertises. S=<5ms, M=<15ms, L=<50ms. */
  budget?: 'S' | 'M' | 'L';
  /** Default activation state when listed. */
  active?: boolean;
  /** Tags for fuzzy search and filtering. */
  tags?: string[];
  /** "Available", "Beta", or "Research" maturity. */
  status: 'available' | 'beta' | 'research';
  /** ADR back-reference. */
  adr?: string;
  /** What actually happens when active — see AppRuntime docs. */
  runtime?: AppRuntime;
}

export const APPS: AppManifest[] = [
  // ── Pipeline simulators ──────────────────────────────────────────────────
  {
    id: 'nvsim',
    name: 'nvsim — NV-diamond magnetometer',
    category: 'sim',
    crate: 'nvsim',
    summary:
      'Deterministic forward simulator: scene → Biot–Savart → NV ensemble → ADC → MagFrame stream + SHA-256 witness.',
    budget: 'L',
    active: true,
    status: 'available',
    tags: ['quantum', 'magnetometer', 'simulator', 'witness', 'wasm'],
    adr: 'ADR-089',
    runtime: 'running',
  },

  // ── Core sensing primitives (ADR-014/040 flagship modules) ───────────────
  {
    id: 'gesture',
    name: 'Gesture (DTW)',
    category: 'sig',
    crate: 'wifi-densepose-wasm-edge',
    summary: 'Dynamic-Time-Warping gesture classifier from CSI motion templates.',
    events: [1],
    budget: 'M',
    status: 'available',
    tags: ['hci', 'csi', 'classifier', 'dtw'],
    adr: 'ADR-014',
    runtime: 'mesh-only',
  },
  {
    id: 'coherence',
    name: 'Coherence gate',
    category: 'sig',
    crate: 'wifi-densepose-wasm-edge',
    summary: 'Z-score coherence scoring + Accept/PredictOnly/Reject/Recalibrate gate.',
    events: [2],
    budget: 'S',
    status: 'available',
    tags: ['gate', 'csi', 'coherence', 'drift'],
    adr: 'ADR-029',
    runtime: 'simulated',
  },
  {
    id: 'adversarial',
    name: 'Adversarial-signal detector',
    category: 'ais',
    crate: 'wifi-densepose-wasm-edge',
    summary:
      'Physically-impossible-signal detector — multi-link consistency, used to flag spoofed CSI.',
    events: [3],
    budget: 'M',
    status: 'available',
    tags: ['security', 'csi', 'spoofing', 'mesh'],
    adr: 'ADR-032',
    runtime: 'simulated',
  },
  {
    id: 'rvf',
    name: 'RVF — Rust Verified Feature stream',
    category: 'sig',
    crate: 'wifi-densepose-wasm-edge',
    summary: 'Verified-frame builder with SHA-256 hash + version metadata for the feature stream.',
    budget: 'S',
    status: 'available',
    tags: ['witness', 'csi', 'hash'],
    adr: 'ADR-040',
  },
  {
    id: 'occupancy',
    name: 'Occupancy estimator',
    category: 'bld',
    crate: 'wifi-densepose-wasm-edge',
    summary: 'Through-wall presence + person-count via CSI amplitude perturbation.',
    events: [300, 301, 302],
    budget: 'S',
    status: 'available',
    tags: ['csi', 'building', 'presence'],
    runtime: 'simulated',
  },
  {
    id: 'vital_trend',
    name: 'Vital-trend monitor',
    category: 'med',
    crate: 'wifi-densepose-wasm-edge',
    summary: 'HR + BR trend tracking with bradycardia/tachycardia/apnea events.',
    events: [100, 101, 102, 103, 104, 105],
    budget: 'S',
    status: 'available',
    tags: ['medical', 'vitals', 'csi'],
    adr: 'ADR-021',
    runtime: 'simulated',
  },
  {
    id: 'intrusion',
    name: 'Intrusion detector',
    category: 'sec',
    crate: 'wifi-densepose-wasm-edge',
    summary: 'Zone-based intrusion alert from CSI motion patterns.',
    events: [200, 201],
    budget: 'S',
    status: 'available',
    tags: ['security', 'zone', 'csi'],
    runtime: 'simulated',
  },

  // ── Medical & Health (100-series) ────────────────────────────────────────
  { id: 'med_sleep_apnea', name: 'Sleep-apnea detector', category: 'med', crate: 'wifi-densepose-wasm-edge', summary: 'Episodic respiratory pause detection during sleep cycles.', events: [105], budget: 'S', status: 'available', tags: ['medical', 'sleep', 'breathing'] },
  { id: 'med_cardiac_arrhythmia', name: 'Cardiac arrhythmia', category: 'med', crate: 'wifi-densepose-wasm-edge', summary: 'Beat-to-beat irregularity classifier from cardiac micro-Doppler.', events: [103, 104], budget: 'M', status: 'available', tags: ['medical', 'cardiac', 'arrhythmia'] },
  { id: 'med_respiratory_distress', name: 'Respiratory distress', category: 'med', crate: 'wifi-densepose-wasm-edge', summary: 'Distress signature: rapid shallow breathing + accessory-muscle motion.', events: [101, 102], budget: 'S', status: 'available', tags: ['medical', 'breathing', 'icu'] },
  { id: 'med_gait_analysis', name: 'Gait analysis', category: 'med', crate: 'wifi-densepose-wasm-edge', summary: 'Stride length, cadence, asymmetry from through-wall CSI pose tracking.', budget: 'M', status: 'available', tags: ['medical', 'gait', 'pose'] },
  { id: 'med_seizure_detect', name: 'Seizure detector', category: 'med', crate: 'wifi-densepose-wasm-edge', summary: 'Tonic-clonic seizure motion signature.', budget: 'M', status: 'beta', tags: ['medical', 'neuro'] },

  // ── Security (200-series) ────────────────────────────────────────────────
  { id: 'sec_perimeter_breach', name: 'Perimeter breach', category: 'sec', crate: 'wifi-densepose-wasm-edge', summary: 'Approach/departure detection at user-defined boundary segments.', events: [210, 211, 212, 213], budget: 'S', status: 'available', tags: ['security', 'perimeter'] },
  { id: 'sec_weapon_detect', name: 'Metal anomaly / weapon', category: 'sec', crate: 'wifi-densepose-wasm-edge', summary: 'Metal-perturbation flag in CSI; potential weapon presence (research).', events: [220, 221, 222], budget: 'M', status: 'research', tags: ['security', 'metal', 'csi'] },
  { id: 'sec_tailgating', name: 'Tailgating detector', category: 'sec', crate: 'wifi-densepose-wasm-edge', summary: 'Detect 2+ persons crossing a single-passage threshold.', events: [230, 231, 232], budget: 'S', status: 'available', tags: ['security', 'access-control'] },
  { id: 'sec_loitering', name: 'Loitering detector', category: 'sec', crate: 'wifi-densepose-wasm-edge', summary: 'Stationary occupancy past a configurable dwell threshold.', events: [240, 241, 242], budget: 'S', status: 'available', tags: ['security', 'dwell'] },
  { id: 'sec_panic_motion', name: 'Panic motion', category: 'sec', crate: 'wifi-densepose-wasm-edge', summary: 'High-energy distress motion: struggle / fleeing pattern.', events: [250, 251, 252], budget: 'S', status: 'beta', tags: ['security', 'distress'] },

  // ── Smart Building (300-series) ──────────────────────────────────────────
  { id: 'bld_hvac_presence', name: 'HVAC presence', category: 'bld', crate: 'wifi-densepose-wasm-edge', summary: 'Occupied/activity-level/departure-countdown for HVAC zones.', events: [310, 311, 312], budget: 'S', status: 'available', tags: ['hvac', 'building', 'energy'] },
  { id: 'bld_lighting_zones', name: 'Lighting zones', category: 'bld', crate: 'wifi-densepose-wasm-edge', summary: 'Per-zone light on/dim/off cues from occupancy.', events: [320, 321, 322], budget: 'S', status: 'available', tags: ['lighting', 'building'] },
  { id: 'bld_elevator_count', name: 'Elevator count', category: 'bld', crate: 'wifi-densepose-wasm-edge', summary: 'Person count inside elevator car from CSI.', events: [330], budget: 'S', status: 'available', tags: ['elevator', 'building'] },
  { id: 'bld_meeting_room', name: 'Meeting-room utilization', category: 'bld', crate: 'wifi-densepose-wasm-edge', summary: 'Meeting size + duration analytics for booking systems.', budget: 'S', status: 'available', tags: ['meeting', 'analytics'] },
  { id: 'bld_energy_audit', name: 'Energy audit', category: 'bld', crate: 'wifi-densepose-wasm-edge', summary: 'Continuous occupancy-vs-HVAC-state audit for energy savings.', budget: 'M', status: 'available', tags: ['energy', 'audit'] },

  // ── Retail (400-series) ──────────────────────────────────────────────────
  { id: 'ret_queue_length', name: 'Queue length', category: 'ret', crate: 'wifi-densepose-wasm-edge', summary: 'Live queue-length tracking for checkout / kiosks.', budget: 'S', status: 'available', tags: ['retail', 'queue'] },
  { id: 'ret_dwell_heatmap', name: 'Dwell heatmap', category: 'ret', crate: 'wifi-densepose-wasm-edge', summary: 'Per-zone dwell time accumulation; analytics-only export.', budget: 'M', status: 'available', tags: ['retail', 'heatmap'] },
  { id: 'ret_customer_flow', name: 'Customer flow', category: 'ret', crate: 'wifi-densepose-wasm-edge', summary: 'Origin-destination flow graph through a store layout.', budget: 'M', status: 'available', tags: ['retail', 'flow'] },
  { id: 'ret_table_turnover', name: 'Table turnover', category: 'ret', crate: 'wifi-densepose-wasm-edge', summary: 'Restaurant table seat / vacate transitions.', budget: 'S', status: 'available', tags: ['retail', 'restaurant'] },
  { id: 'ret_shelf_engagement', name: 'Shelf engagement', category: 'ret', crate: 'wifi-densepose-wasm-edge', summary: 'Reach-to-shelf gestures and dwell at product zones.', budget: 'M', status: 'available', tags: ['retail', 'shelf'] },

  // ── Industrial (500-series) ──────────────────────────────────────────────
  { id: 'ind_forklift_proximity', name: 'Forklift proximity', category: 'ind', crate: 'wifi-densepose-wasm-edge', summary: 'Worker-near-forklift safety alert.', budget: 'S', status: 'available', tags: ['industrial', 'safety'] },
  { id: 'ind_confined_space', name: 'Confined-space monitor', category: 'ind', crate: 'wifi-densepose-wasm-edge', summary: 'Last-person-out detection + presence audit for OSHA confined-space entries.', budget: 'S', status: 'available', tags: ['industrial', 'osha'] },
  { id: 'ind_clean_room', name: 'Clean-room PPE / motion', category: 'ind', crate: 'wifi-densepose-wasm-edge', summary: 'Motion patterns consistent with proper PPE-clad movement.', budget: 'M', status: 'beta', tags: ['industrial', 'cleanroom'] },
  { id: 'ind_livestock_monitor', name: 'Livestock monitor', category: 'ind', crate: 'wifi-densepose-wasm-edge', summary: 'Vital-sign + activity tracking for stall-bound livestock.', budget: 'M', status: 'beta', tags: ['agriculture', 'livestock'] },
  { id: 'ind_structural_vibration', name: 'Structural vibration', category: 'ind', crate: 'wifi-densepose-wasm-edge', summary: 'Building/equipment micro-vibration via CSI phase derivative.', budget: 'M', status: 'research', tags: ['industrial', 'vibration'] },

  // ── Signal primitives (600-series) ───────────────────────────────────────
  { id: 'sig_coherence_gate', name: 'Coherence gate (extended)', category: 'sig', crate: 'wifi-densepose-wasm-edge', summary: 'Hysteresis + multi-state coherence gate driving downstream apps.', budget: 'S', status: 'available', tags: ['gate', 'csi'] },
  { id: 'sig_flash_attention', name: 'Flash attention (CSI)', category: 'sig', crate: 'wifi-densepose-wasm-edge', summary: 'Edge-friendly attention block for CSI subcarrier weighting.', budget: 'M', status: 'beta', tags: ['attention', 'csi'] },
  { id: 'sig_temporal_compress', name: 'Temporal-tensor compress', category: 'sig', crate: 'wifi-densepose-wasm-edge', summary: 'RuVector temporal-tensor compression on the CSI buffer.', budget: 'M', status: 'available', tags: ['compress', 'tensor'] },
  { id: 'sig_sparse_recovery', name: 'Sparse recovery', category: 'sig', crate: 'wifi-densepose-wasm-edge', summary: '114→56 subcarrier sparse interpolation via L1 solver.', budget: 'M', status: 'available', tags: ['sparse', 'csi'] },
  { id: 'sig_mincut_person_match', name: 'Mincut person-match', category: 'sig', crate: 'wifi-densepose-wasm-edge', summary: 'Min-cut person assignment across multistatic frames.', budget: 'M', status: 'available', tags: ['mincut', 'matching'] },
  { id: 'sig_optimal_transport', name: 'Optimal transport', category: 'sig', crate: 'wifi-densepose-wasm-edge', summary: 'OT-based feature alignment between mesh nodes.', budget: 'M', status: 'beta', tags: ['ot', 'alignment'] },

  // ── Online learning ──────────────────────────────────────────────────────
  { id: 'lrn_dtw_gesture_learn', name: 'DTW gesture learn', category: 'lrn', crate: 'wifi-densepose-wasm-edge', summary: 'On-device template learning for personalized gesture libraries.', budget: 'M', status: 'beta', tags: ['lifelong', 'gesture'] },
  { id: 'lrn_anomaly_attractor', name: 'Anomaly attractor', category: 'lrn', crate: 'wifi-densepose-wasm-edge', summary: 'Novelty detector with dynamic-attractor recall.', budget: 'M', status: 'research', tags: ['novelty', 'lifelong'] },
  { id: 'lrn_meta_adapt', name: 'Meta-adapt', category: 'lrn', crate: 'wifi-densepose-wasm-edge', summary: 'Meta-learning adapter for fast site-to-site transfer.', budget: 'L', status: 'research', tags: ['meta-learning'] },
  { id: 'lrn_ewc_lifelong', name: 'EWC++ lifelong', category: 'lrn', crate: 'wifi-densepose-wasm-edge', summary: 'Elastic-weight-consolidation gate to avoid catastrophic forgetting.', budget: 'M', status: 'beta', tags: ['lifelong', 'ewc'] },

  // ── Spatial / graph ──────────────────────────────────────────────────────
  { id: 'spt_pagerank_influence', name: 'PageRank influence', category: 'spt', crate: 'wifi-densepose-wasm-edge', summary: 'Graph-influence ranking on the multistatic mesh.', budget: 'M', status: 'beta', tags: ['graph', 'pagerank'] },
  { id: 'spt_micro_hnsw', name: 'µHNSW vector index', category: 'spt', crate: 'wifi-densepose-wasm-edge', summary: 'Tiny HNSW index for AETHER re-ID embeddings on-device.', budget: 'M', status: 'available', tags: ['hnsw', 'reid'] },
  { id: 'spt_spiking_tracker', name: 'Spiking tracker', category: 'spt', crate: 'wifi-densepose-wasm-edge', summary: 'Spiking-network multi-target tracker.', budget: 'L', status: 'research', tags: ['snn', 'tracker'] },

  // ── Temporal / planning ──────────────────────────────────────────────────
  { id: 'tmp_pattern_sequence', name: 'Pattern sequence', category: 'tmp', crate: 'wifi-densepose-wasm-edge', summary: 'Sequence-of-events pattern matcher (e.g. ingress→linger→egress).', budget: 'M', status: 'available', tags: ['temporal', 'pattern'] },
  { id: 'tmp_temporal_logic_guard', name: 'Temporal logic guard', category: 'tmp', crate: 'wifi-densepose-wasm-edge', summary: 'LTL/MTL safety-property guard over event streams.', budget: 'M', status: 'beta', tags: ['ltl', 'safety'] },
  { id: 'tmp_goap_autonomy', name: 'GOAP autonomy', category: 'tmp', crate: 'wifi-densepose-wasm-edge', summary: 'Goal-oriented action planning for adaptive routines.', budget: 'L', status: 'research', tags: ['planning', 'autonomy'] },

  // ── AI safety ────────────────────────────────────────────────────────────
  { id: 'ais_prompt_shield', name: 'Prompt shield', category: 'ais', crate: 'wifi-densepose-wasm-edge', summary: 'Edge-side LLM prompt-injection guard for on-device assistants.', budget: 'M', status: 'beta', tags: ['security', 'llm'] },
  { id: 'ais_behavioral_profiler', name: 'Behavioral profiler', category: 'ais', crate: 'wifi-densepose-wasm-edge', summary: 'Anomalous-behaviour profiler (drift in motion habits).', budget: 'M', status: 'beta', tags: ['anomaly', 'behaviour'] },

  // ── Quantum-flavoured ────────────────────────────────────────────────────
  { id: 'qnt_quantum_coherence', name: 'Quantum coherence', category: 'qnt', crate: 'wifi-densepose-wasm-edge', summary: 'Coherence diagnostics adapted for quantum-sensor signals.', budget: 'M', status: 'research', tags: ['quantum', 'coherence'] },
  { id: 'qnt_interference_search', name: 'Interference search', category: 'qnt', crate: 'wifi-densepose-wasm-edge', summary: 'Interferometric anomaly search across mesh viewpoints.', budget: 'L', status: 'research', tags: ['quantum', 'interference'] },

  // ── Autonomy / mesh ──────────────────────────────────────────────────────
  { id: 'aut_psycho_symbolic', name: 'Psycho-symbolic agent', category: 'aut', crate: 'wifi-densepose-wasm-edge', summary: 'Symbolic-rule + neural-feature hybrid for low-power autonomy loops.', budget: 'L', status: 'research', tags: ['autonomy', 'symbolic'] },
  { id: 'aut_self_healing_mesh', name: 'Self-healing mesh', category: 'aut', crate: 'wifi-densepose-wasm-edge', summary: 'Mesh-topology repair with per-node health gossip.', budget: 'M', status: 'beta', tags: ['mesh', 'health'] },

  // ── Exotic / Research (650-series) ───────────────────────────────────────
  { id: 'exo_ghost_hunter', name: 'Ghost hunter (anomaly)', category: 'exo', crate: 'wifi-densepose-wasm-edge', summary: 'Empty-room CSI anomaly detector — impulsive/periodic/drift/random + hidden-presence sub-detector.', events: [650, 651, 652, 653], budget: 'S', status: 'available', tags: ['anomaly', 'paranormal', 'csi'], adr: 'ADR-041', runtime: 'simulated' },
  { id: 'exo_breathing_sync', name: 'Breathing sync', category: 'exo', crate: 'wifi-densepose-wasm-edge', summary: 'Multi-person breathing synchrony analytics.', budget: 'M', status: 'beta', tags: ['breathing', 'sync'] },
  { id: 'exo_dream_stage', name: 'Dream-stage classifier', category: 'exo', crate: 'wifi-densepose-wasm-edge', summary: 'NREM/REM stage classification from breathing + micro-motion.', budget: 'M', status: 'research', tags: ['sleep', 'rem'] },
  { id: 'exo_emotion_detect', name: 'Emotion detector', category: 'exo', crate: 'wifi-densepose-wasm-edge', summary: 'Coarse arousal/valence from breathing + heart-rate variability.', budget: 'M', status: 'research', tags: ['affect'] },
  { id: 'exo_gesture_language', name: 'Gesture language', category: 'exo', crate: 'wifi-densepose-wasm-edge', summary: 'Sign-language pattern recognition.', budget: 'L', status: 'research', tags: ['hci', 'sign'] },
  { id: 'exo_happiness_score', name: 'Happiness score', category: 'exo', crate: 'wifi-densepose-wasm-edge', summary: 'Aggregate well-being score from co-occupancy + activity dynamics.', budget: 'M', status: 'research', tags: ['affect', 'wellbeing'] },
  { id: 'exo_hyperbolic_space', name: 'Hyperbolic space embed', category: 'exo', crate: 'wifi-densepose-wasm-edge', summary: 'Hyperbolic embeddings for hierarchical scene structure.', budget: 'L', status: 'research', tags: ['embedding', 'hyperbolic'] },
  { id: 'exo_music_conductor', name: 'Music conductor', category: 'exo', crate: 'wifi-densepose-wasm-edge', summary: 'Map gesture energy to MIDI tempo/dynamics.', budget: 'M', status: 'research', tags: ['midi', 'art'] },
  { id: 'exo_plant_growth', name: 'Plant-growth tracker', category: 'exo', crate: 'wifi-densepose-wasm-edge', summary: 'Slow CSI drift tracking for greenhouse foliage growth.', budget: 'L', status: 'research', tags: ['agriculture'] },
  { id: 'exo_rain_detect', name: 'Rain detector', category: 'exo', crate: 'wifi-densepose-wasm-edge', summary: 'Outdoor CSI signature of rainfall.', budget: 'M', status: 'research', tags: ['weather'] },
  { id: 'exo_time_crystal', name: 'Time-crystal periodicity', category: 'exo', crate: 'wifi-densepose-wasm-edge', summary: 'Periodicity diagnostics with anti-aliasing harmonics.', budget: 'M', status: 'research', tags: ['periodicity'] },
];

export const CATEGORIES: Record<AppCategory, { label: string; color: string; range: string }> = {
  sim: { label: 'Simulators', color: 'oklch(0.78 0.14 70)', range: '—' },
  med: { label: 'Medical & Health', color: 'oklch(0.65 0.22 25)', range: '100–199' },
  sec: { label: 'Security & Safety', color: 'oklch(0.7 0.18 35)', range: '200–299' },
  bld: { label: 'Smart Building', color: 'oklch(0.78 0.12 195)', range: '300–399' },
  ret: { label: 'Retail & Hospitality', color: 'oklch(0.78 0.14 145)', range: '400–499' },
  ind: { label: 'Industrial', color: 'oklch(0.72 0.18 330)', range: '500–599' },
  sig: { label: 'Signal Processing', color: 'oklch(0.78 0.14 70)', range: '600–619' },
  lrn: { label: 'Online Learning', color: 'oklch(0.78 0.12 260)', range: '620–639' },
  spt: { label: 'Spatial / Graph', color: 'oklch(0.7 0.18 100)', range: '640–659' },
  tmp: { label: 'Temporal / Planning', color: 'oklch(0.7 0.16 50)', range: '660–679' },
  ais: { label: 'AI Safety', color: 'oklch(0.65 0.22 25)', range: '700–719' },
  qnt: { label: 'Quantum', color: 'oklch(0.72 0.18 290)', range: '720–739' },
  aut: { label: 'Autonomy', color: 'oklch(0.78 0.14 145)', range: '740–759' },
  exo: { label: 'Exotic / Research', color: 'oklch(0.72 0.18 330)', range: '650–699' },
};

export interface AppActivation {
  id: string;
  /** Active in the current session. */
  active: boolean;
  /** Last activation timestamp. */
  lastActivatedAt?: number;
  /** Last event count seen (for the cards' counter). */
  eventCount?: number;
}

export function defaultActivations(): AppActivation[] {
  return APPS.map((a) => ({ id: a.id, active: a.active === true, eventCount: 0 }));
}

export function appsByCategory(): Record<AppCategory, AppManifest[]> {
  const map = {} as Record<AppCategory, AppManifest[]>;
  for (const c of Object.keys(CATEGORIES) as AppCategory[]) map[c] = [];
  for (const a of APPS) map[a.category].push(a);
  return map;
}

export function findApp(id: string): AppManifest | undefined {
  return APPS.find((a) => a.id === id);
}

export function fuzzyMatch(query: string, app: AppManifest): number {
  if (!query) return 1;
  const q = query.toLowerCase();
  let score = 0;
  if (app.id.toLowerCase().includes(q)) score += 3;
  if (app.name.toLowerCase().includes(q)) score += 3;
  if (app.summary.toLowerCase().includes(q)) score += 1;
  if (app.tags?.some((t) => t.toLowerCase().includes(q))) score += 2;
  if (app.category === q) score += 5;
  return score;
}
