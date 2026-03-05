/**
 * Demo Data Generator — RuView Observatory
 *
 * Generates synthetic CSI data matching the SensingUpdate contract.
 * 12 scenarios covering all edge module categories.
 * Each person includes pose, facing, and scenario-specific motion data.
 * Auto-cycles with cosine crossfade transitions.
 *
 * V2: Enhanced with temporally-correlated noise, spatially-coherent fields,
 * physiologically accurate vital signs, and realistic behavioral patterns.
 */

const SCENARIOS = [
  'empty_room',
  'single_breathing',
  'two_walking',
  'fall_event',
  'sleep_monitoring',
  'intrusion_detect',
  'gesture_control',
  'crowd_occupancy',
  'search_rescue',
  'elderly_care',
  'fitness_tracking',
  'security_patrol',
];

const CROSSFADE_DURATION = 2; // seconds

// ---------------------------------------------------------------------------
// Noise & utility functions (module-private)
// ---------------------------------------------------------------------------

/** Seeded PRNG for deterministic per-scenario noise. */
function _mulberry32(seed) {
  return function () {
    let t = (seed += 0x6d2b79f5);
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Temporally-correlated noise (1st-order IIR low-pass filtered white noise).
 * Returns a function noise(t) that produces smooth, non-teleporting values
 * in approximately [-amplitude, +amplitude].
 * `smoothing` controls correlation: higher = smoother (0.9-0.99 typical).
 */
function _makeCorrelatedNoise(seed, smoothing = 0.95, amplitude = 1) {
  const rng = _mulberry32(seed);
  let state = 0;
  let lastT = -1;
  return function (t) {
    // Step the filter forward for each new time tick
    const steps = Math.max(1, Math.round((t - lastT) * 60)); // ~60 Hz internal
    for (let i = 0; i < Math.min(steps, 120); i++) {
      state = smoothing * state + (1 - smoothing) * (rng() * 2 - 1);
    }
    lastT = t;
    return state * amplitude;
  };
}

/**
 * Perlin-like 1D noise via sine harmonics.
 * Deterministic, smooth, and cheap.
 */
function _harmonicNoise(t, seed, octaves = 3) {
  let v = 0, amp = 1, freq = 1;
  for (let i = 0; i < octaves; i++) {
    v += amp * Math.sin(t * freq + seed * (i + 1) * 1.618);
    amp *= 0.5;
    freq *= 2.17;
  }
  return v;
}

/** Smooth step (hermite interpolation) */
function _smoothstep(edge0, edge1, x) {
  const t = Math.max(0, Math.min(1, (x - edge0) / (edge1 - edge0)));
  return t * t * (3 - 2 * t);
}

/** Clamp */
function _clamp(v, lo, hi) { return Math.max(lo, Math.min(hi, v)); }

/** Lerp */
function _lerp(a, b, t) { return a + (b - a) * t; }

/** Gaussian blob value at distance d with given sigma */
function _gaussian(d, sigma) {
  return Math.exp(-(d * d) / (2 * sigma * sigma));
}

// ---------------------------------------------------------------------------
// Noise bank — pre-allocated correlated noise channels per scenario
// Each scenario gets its own set of noise functions so they don't interfere.
// ---------------------------------------------------------------------------
const _noiseBanks = {};
function _getNoiseBank(scenario) {
  if (!_noiseBanks[scenario]) {
    const idx = SCENARIOS.indexOf(scenario);
    const base = (idx + 1) * 1000;
    _noiseBanks[scenario] = {
      rssi:    _makeCorrelatedNoise(base + 1, 0.97, 1.5),
      breath:  _makeCorrelatedNoise(base + 2, 0.92, 0.3),
      hr:      _makeCorrelatedNoise(base + 3, 0.94, 1.0),
      motion:  _makeCorrelatedNoise(base + 4, 0.90, 0.5),
      field:   _makeCorrelatedNoise(base + 5, 0.96, 0.2),
      pos1:    _makeCorrelatedNoise(base + 6, 0.98, 0.15),
      pos2:    _makeCorrelatedNoise(base + 7, 0.98, 0.15),
      env:     _makeCorrelatedNoise(base + 8, 0.99, 1.0),
      spike:   _makeCorrelatedNoise(base + 9, 0.80, 1.0),
    };
  }
  return _noiseBanks[scenario];
}


export class DemoDataGenerator {
  constructor() {
    this._scenarioIndex = 0;
    this._elapsed = 0;
    this._paused = false;
    this._prevFrame = null;
    this._currFrame = null;
    this._cycleDuration = 30;
    this._autoMode = true;
  }

  get currentScenario() {
    return SCENARIOS[this._scenarioIndex];
  }

  get paused() { return this._paused; }
  set paused(v) { this._paused = v; }

  cycleScenario() {
    this._scenarioIndex = (this._scenarioIndex + 1) % SCENARIOS.length;
    this._elapsed = 0;
  }

  setScenario(name) {
    const idx = SCENARIOS.indexOf(name);
    if (idx >= 0) {
      this._scenarioIndex = idx;
      this._autoMode = false;
      this._elapsed = 0;
    } else if (name === 'auto') {
      this._autoMode = true;
    }
  }

  setCycleDuration(seconds) {
    this._cycleDuration = Math.max(5, seconds);
  }

  /** Call each frame; returns blended SensingUpdate object */
  update(dt) {
    if (this._paused) {
      return this._currFrame || this._generate(this._scenarioIndex, this._elapsed);
    }

    this._elapsed += dt;

    // Auto-cycle
    if (this._autoMode && this._elapsed >= this._cycleDuration) {
      this._elapsed -= this._cycleDuration;
      this._scenarioIndex = (this._scenarioIndex + 1) % SCENARIOS.length;
    }

    const t = this._elapsed;
    const frame = this._generate(this._scenarioIndex, t);

    // Crossfade near transition boundaries
    if (this._autoMode && t < CROSSFADE_DURATION) {
      const prevIdx = (this._scenarioIndex - 1 + SCENARIOS.length) % SCENARIOS.length;
      const prevFrame = this._generate(prevIdx, this._cycleDuration - CROSSFADE_DURATION + t);
      const alpha = 0.5 + 0.5 * Math.cos(Math.PI * (1 - t / CROSSFADE_DURATION));
      this._currFrame = this._blend(prevFrame, frame, alpha);
    } else {
      this._currFrame = frame;
    }

    return this._currFrame;
  }

  // ---- Scenario generators ----

  _generate(scenarioIdx, t) {
    const name = SCENARIOS[scenarioIdx];
    switch (name) {
      case 'empty_room':       return this._emptyRoom(t);
      case 'single_breathing': return this._singleBreathing(t);
      case 'two_walking':      return this._twoWalking(t);
      case 'fall_event':       return this._fallEvent(t);
      case 'sleep_monitoring': return this._sleepMonitoring(t);
      case 'intrusion_detect': return this._intrusionDetect(t);
      case 'gesture_control':  return this._gestureControl(t);
      case 'crowd_occupancy':  return this._crowdOccupancy(t);
      case 'search_rescue':    return this._searchRescue(t);
      case 'elderly_care':     return this._elderlyCare(t);
      case 'fitness_tracking': return this._fitnessTracking(t);
      case 'security_patrol':  return this._securityPatrol(t);
      default:                 return this._emptyRoom(t);
    }
  }

  // ---- Base template ----

  _baseFrame(overrides) {
    return {
      type: 'sensing_update',
      timestamp: Date.now() / 1000,
      source: 'demo',
      scenario: SCENARIOS[this._scenarioIndex],
      nodes: [{ node_id: 1, rssi_dbm: -45, position: [2, 0, 1.5], amplitude: new Float32Array(64), subcarrier_count: 64 }],
      features: { mean_rssi: -45, variance: 0.3, std: 0.55, motion_band_power: 0.02, breathing_band_power: 0.01, dominant_freq_hz: 0.05, spectral_power: 0.03 },
      classification: { motion_level: 'absent', presence: false, confidence: 0.92 },
      signal_field: { grid_size: [20, 1, 20], values: this._flatField(0.05) },
      vital_signs: { breathing_rate_bpm: 0, heart_rate_bpm: 0, breathing_confidence: 0, heart_rate_confidence: 0 },
      persons: [],
      estimated_persons: 0,
      edge_modules: {},
      _observatory: { subcarrier_iq: [], per_subcarrier_variance: new Float32Array(64).fill(0.02) },
      ...overrides,
    };
  }

  // ========================================================================
  // 1. Empty Room — environmental noise, interference spikes, day/night drift
  // ========================================================================

  _emptyRoom(t) {
    const n = _getNoiseBank('empty_room');

    // Day/night RSSI drift: slow sinusoidal cycle over the scenario duration
    const dayNightDrift = Math.sin(t * 0.08) * 3;
    // Occasional microwave/device interference spike
    const spikeRaw = n.spike(t);
    const interferenceSpike = spikeRaw > 0.7 ? (spikeRaw - 0.7) * 15 : 0;
    // Subtle HVAC cycling
    const hvacCycle = Math.sin(t * 0.4) * 0.5 + Math.sin(t * 1.1) * 0.2;

    const baseRssi = -45 + dayNightDrift + n.rssi(t) + interferenceSpike;

    const amplitude = new Float32Array(64);
    for (let i = 0; i < 64; i++) {
      // Base floor with harmonic variation per subcarrier
      const subNoise = _harmonicNoise(t, i * 0.37, 2) * 0.02;
      // Interference affects specific subcarrier bands (like a microwave in 2.4GHz)
      const microBand = (i >= 20 && i <= 35) ? interferenceSpike * 0.03 : 0;
      amplitude[i] = 0.1 + subNoise + microBand + Math.abs(hvacCycle) * 0.01;
    }

    // Signal field with subtle ripple patterns (standing waves in empty room)
    const vals = [];
    for (let iz = 0; iz < 20; iz++) {
      for (let ix = 0; ix < 20; ix++) {
        const standingWave = Math.sin(ix * 0.8 + t * 0.3) * Math.sin(iz * 0.6 + t * 0.2) * 0.015;
        const fieldNoise = _harmonicNoise(t + ix * 0.5 + iz * 0.7, ix + iz * 20, 2) * 0.008;
        const ripple = interferenceSpike > 0
          ? _gaussian(Math.sqrt((ix - 10) ** 2 + (iz - 10) ** 2), 8) * interferenceSpike * 0.02
          : 0;
        vals.push(_clamp(0.05 + standingWave + fieldNoise + ripple, 0, 1));
      }
    }

    return this._baseFrame({
      nodes: [{ node_id: 1, rssi_dbm: baseRssi, position: [2, 0, 1.5], amplitude, subcarrier_count: 64 }],
      features: {
        mean_rssi: baseRssi,
        variance: 0.3 + Math.abs(n.env(t)) * 0.15 + interferenceSpike * 0.5,
        std: 0.55 + interferenceSpike * 0.2,
        motion_band_power: 0.02 + interferenceSpike * 0.08 + Math.abs(hvacCycle) * 0.005,
        breathing_band_power: 0.01 + Math.abs(hvacCycle) * 0.003,
        dominant_freq_hz: interferenceSpike > 0.5 ? 2.45 : 0.05 + Math.abs(hvacCycle) * 0.02,
        spectral_power: 0.03 + interferenceSpike * 0.1,
      },
      signal_field: { grid_size: [20, 1, 20], values: vals },
      edge_modules: {
        environment: {
          interference_detected: interferenceSpike > 0.5,
          interference_band: interferenceSpike > 0.5 ? '2.4GHz_microwave' : 'none',
          ambient_drift: dayNightDrift.toFixed(2),
        },
      },
    });
  }

  // ========================================================================
  // 2. Single Breathing — HRV, respiratory sinus arrhythmia, natural irregularity
  // ========================================================================

  _singleBreathing(t) {
    const n = _getNoiseBank('single_breathing');

    // Natural breathing: ~16 BPM but with irregularity
    // Breathing rate varies slightly over time (14.5-17.5)
    const breathRateBase = 16 + _harmonicNoise(t, 1.23, 2) * 1.5;
    const breathFreq = breathRateBase / 60;
    // Accumulate phase for non-uniform period
    const breathPhase = Math.sin(2 * Math.PI * breathFreq * t + n.breath(t) * 0.4);
    // Inhale is slightly shorter than exhale (1:1.5 ratio via asymmetric wave)
    const breathSignal = breathPhase > 0
      ? Math.sin(Math.asin(breathPhase) * 1.3)
      : breathPhase * 0.85;

    // Heart Rate Variability (HRV): base 72 BPM, varies 68-76
    // Respiratory Sinus Arrhythmia: HR increases on inhale, decreases on exhale
    const rsaEffect = breathSignal * 3.0; // +/-3 BPM with breathing
    const hrvWander = _harmonicNoise(t, 7.77, 3) * 2.0; // slow HRV drift
    const instantHR = 72 + rsaEffect + hrvWander + n.hr(t) * 0.5;
    const hrFreq = instantHR / 60;
    const hrPhase = Math.sin(2 * Math.PI * hrFreq * t);

    const amplitude = new Float32Array(64);
    for (let i = 0; i < 64; i++) {
      const subBase = 0.4 + 0.2 * Math.sin(t * 0.5 + i * 0.15);
      const breathMod = breathSignal * 0.08 * (1 + 0.3 * Math.sin(i * 0.4)); // subcarrier-dependent
      const hrMod = hrPhase * 0.015 * (i > 20 && i < 45 ? 1.5 : 0.5); // HR stronger in mid-band
      amplitude[i] = subBase + breathMod + hrMod + _harmonicNoise(t, i * 0.13, 2) * 0.01;
    }

    const rssiBase = -42 + breathSignal * 1.5 + n.rssi(t) * 0.5;

    return this._baseFrame({
      nodes: [{ node_id: 1, rssi_dbm: rssiBase, position: [2, 0, 1.5], amplitude, subcarrier_count: 64 }],
      features: {
        mean_rssi: rssiBase,
        variance: 1.8 + breathSignal * 0.3 + Math.abs(n.motion(t)) * 0.1,
        std: 1.34 + Math.abs(breathSignal) * 0.1,
        motion_band_power: 0.04 + Math.abs(breathSignal) * 0.02,
        breathing_band_power: 0.12 + breathSignal * 0.04,
        dominant_freq_hz: breathFreq,
        spectral_power: 0.18 + Math.abs(hrPhase) * 0.03,
      },
      classification: { motion_level: 'present_still', presence: true, confidence: 0.88 + breathSignal * 0.03 },
      signal_field: { grid_size: [20, 1, 20], values: this._presenceField(10, 10, 2.5, t) },
      vital_signs: {
        breathing_rate_bpm: breathRateBase,
        heart_rate_bpm: instantHR,
        breathing_confidence: 0.85 + breathSignal * 0.05,
        heart_rate_confidence: 0.75 + hrPhase * 0.05,
        hrv_ms: 35 + _harmonicNoise(t, 3.14, 2) * 15, // RMSSD-like HRV metric
        rsa_active: true,
      },
      persons: [{ id: 'p0', position: [0 + n.pos1(t) * 0.05, 0, 0 + n.pos2(t) * 0.05], motion_score: 15, pose: 'standing', facing: 0 }],
      estimated_persons: 1,
      edge_modules: {
        vital_trend: { status: 'normal', trend: 'stable', hrv_quality: 'good' },
        cardiac_detail: { rsa_amplitude_bpm: Math.abs(rsaEffect).toFixed(1), hrv_rmssd_ms: (35 + _harmonicNoise(t, 3.14, 2) * 15).toFixed(0) },
      },
    });
  }

  // ========================================================================
  // 3. Two Walking — collision avoidance, phone pause, confidence dip at crossing
  // ========================================================================

  _twoWalking(t) {
    const n = _getNoiseBank('two_walking');

    // Person 1: walks a figure-8 with speed variation
    const p1speed = 0.5 + _harmonicNoise(t, 1.1, 2) * 0.1; // natural speed var
    const p1phase = t * p1speed;
    let p1x = Math.sin(p1phase) * 2.5;
    let p1z = Math.sin(p1phase * 0.7) * Math.cos(p1phase * 0.35) * 1.8;

    // Person 2: walks an ellipse, pauses at t~10-12 (checking phone)
    const phonePause = (t >= 10 && t < 12);
    const p2speedMod = phonePause ? 0.05 : 1.0; // nearly stopped during phone check
    const p2speed = (0.4 + _harmonicNoise(t, 2.2, 2) * 0.08) * p2speedMod;
    const p2phase = t * 0.4 + 1 + (phonePause ? 0 : _harmonicNoise(t, 3.3, 2) * 0.1);
    let p2x = -Math.sin(p2phase) * 2;
    let p2z = Math.cos(p2phase * 0.75 + 2) * 1.5;

    // Collision avoidance: repulsion when persons are close
    const dx = p1x - p2x;
    const dz = p1z - p2z;
    const dist = Math.sqrt(dx * dx + dz * dz);
    const minDist = 0.8;
    if (dist < minDist * 3 && dist > 0.01) {
      const repulsion = Math.max(0, 1 - dist / (minDist * 3)) * 0.6;
      const nx = dx / dist, nz = dz / dist;
      p1x += nx * repulsion;
      p1z += nz * repulsion;
      p2x -= nx * repulsion;
      p2z -= nz * repulsion;
    }

    // Confidence dip when persons are close (tracking confusion)
    const proxConfidence = dist < 1.5 ? 0.65 + dist * 0.1 : 0.82;
    const matchConfidence = dist < 1.2 ? 0.6 + dist * 0.2 : 0.91;

    const p1facing = Math.atan2(
      Math.cos(p1phase) * p1speed * 2.5,
      Math.cos(p1phase * 0.7) * 0.7 * Math.cos(p1phase * 0.35) * 1.8
    );
    const p2facing = phonePause
      ? Math.PI * 0.8 // looking down at phone
      : Math.atan2(-Math.cos(p2phase) * p2speed * 2, -Math.sin(p2phase * 0.75 + 2) * 0.75 * 1.5);

    const p1ms = 160 + _harmonicNoise(t, 4.4, 2) * 20;
    const p2ms = phonePause ? 8 + Math.abs(n.motion(t)) * 5 : 140 + _harmonicNoise(t, 5.5, 2) * 20;

    const amplitude = new Float32Array(64);
    for (let i = 0; i < 64; i++) {
      amplitude[i] = 0.3 + 0.3 * Math.abs(Math.sin(t * 2 + i * 0.3))
        + _harmonicNoise(t, i * 0.17, 2) * 0.02;
    }

    return this._baseFrame({
      nodes: [{ node_id: 1, rssi_dbm: -40 + Math.sin(t * 1.2) * 4 + n.rssi(t), position: [2, 0, 1.5], amplitude, subcarrier_count: 64 }],
      features: {
        mean_rssi: -40 + Math.sin(t * 1.2) * 4 + n.rssi(t),
        variance: 3.5 + Math.sin(t * 0.8) * 1.2 + (dist < 1.5 ? 2 : 0),
        std: 1.87 + (dist < 1.5 ? 0.5 : 0),
        motion_band_power: 0.25 + Math.abs(Math.sin(t * 1.5)) * 0.15 * (phonePause ? 0.3 : 1),
        breathing_band_power: 0.06,
        dominant_freq_hz: 1.2 + Math.sin(t * 0.5) * 0.3,
        spectral_power: 0.45,
      },
      classification: { motion_level: phonePause ? 'present_still' : 'active', presence: true, confidence: proxConfidence },
      signal_field: { grid_size: [20, 1, 20], values: this._twoPresenceField(10 + p1x * 2, 10 + p1z * 2, 10 + p2x * 2, 10 + p2z * 2, t) },
      vital_signs: { breathing_rate_bpm: 18, heart_rate_bpm: 85, breathing_confidence: 0.4, heart_rate_confidence: 0.35 },
      persons: [
        { id: 'p0', position: [p1x, 0, p1z], motion_score: p1ms, pose: 'walking', facing: p1facing },
        { id: 'p1', position: [p2x, 0, p2z], motion_score: p2ms, pose: phonePause ? 'standing' : 'walking', facing: p2facing },
      ],
      estimated_persons: 2,
      edge_modules: {
        person_match: { matched: 2, confidence: matchConfidence, proximity_warning: dist < 1.5 },
        tracking: { id_swap_risk: dist < 1.0, nearest_pair_dist: dist.toFixed(2) },
      },
    });
  }

  // ========================================================================
  // 4. Fall Event — pre-fall stumble, impact spike, micro-movements, shock HR
  // ========================================================================

  _fallEvent(t) {
    const n = _getNoiseBank('fall_event');

    // Timeline: 0-3 normal walk, 3-5 stumble, 5-5.8 fall, 5.8-8 micro-movement, 8+ still
    const stumbleStart = 3, fallStart = 5, fallEnd = 5.8;
    const microEnd = 8, stillPhase = t >= microEnd;

    const preStumble = t < stumbleStart;
    const stumbling = t >= stumbleStart && t < fallStart;
    const inFall = t >= fallStart && t < fallEnd;
    const microMovement = t >= fallEnd && t < microEnd;
    const postFall = t >= fallEnd;

    // Pre-fall stumble: unsteady gait (asymmetric, wobbly)
    const stumbleIntensity = stumbling ? _smoothstep(stumbleStart, fallStart, t) : 0;
    const wobble = stumbling ? Math.sin(t * 8) * stumbleIntensity * 0.4 : 0;

    // Fall impact spike: sharp gaussian at moment of impact
    const impactT = (fallStart + fallEnd) / 2;
    const impactSpike = Math.exp(-((t - impactT) ** 2) / 0.04) * 1.0;

    // Post-fall micro-movements (trying to get up)
    const microIntensity = microMovement
      ? (1 - _smoothstep(fallEnd, microEnd, t)) * 0.3
      : 0;
    const microSignal = microMovement
      ? Math.sin(t * 3) * microIntensity + Math.sin(t * 5.5) * microIntensity * 0.4
      : 0;

    // Heart rate: normal 72, elevated post-fall shock response 100-110 BPM
    let hrRate = 72;
    if (stumbling) hrRate = 72 + stumbleIntensity * 15; // anxiety rising
    else if (inFall) hrRate = 90 + impactSpike * 30;
    else if (postFall) hrRate = 108 - _smoothstep(fallEnd, fallEnd + 20, t) * 30; // slowly comes down
    hrRate += n.hr(t) * 1.5;

    // Breathing: elevated post-fall
    let breathRate = 16;
    if (postFall) breathRate = 24 - _smoothstep(fallEnd, fallEnd + 15, t) * 8;
    breathRate += n.breath(t) * 0.5;

    // Position: walking -> stumble -> fall -> ground
    let px = 0.3, pz = 0.2, py = 0, pose = 'standing', ms = 20;
    if (preStumble) {
      px = Math.sin(t * 0.4) * 1.5;
      pz = t * 0.3 - 1;
      pose = 'walking';
      ms = 80;
    } else if (stumbling) {
      const st = (t - stumbleStart) / (fallStart - stumbleStart);
      px = Math.sin(stumbleStart * 0.4) * 1.5 + wobble + st * 0.5;
      pz = (stumbleStart * 0.3 - 1) + st * 0.3;
      pose = 'walking'; // stumbling but still upright
      ms = 120 + stumbleIntensity * 80;
    } else if (inFall) {
      pose = 'falling';
      ms = 255;
      py = 0;
    } else if (microMovement) {
      pose = 'fallen';
      ms = _clamp(microIntensity * 100, 3, 40);
      px += microSignal * 0.1;
    } else {
      pose = 'fallen';
      ms = 2 + Math.abs(n.motion(t)) * 1;
    }

    const motionPower = preStumble ? 0.08
      : stumbling ? 0.15 + stumbleIntensity * 0.3
      : inFall ? 0.6 + impactSpike * 0.4
      : microMovement ? 0.05 + microIntensity * 0.15
      : 0.02 + Math.abs(n.motion(t)) * 0.005;

    const amplitude = new Float32Array(64);
    for (let i = 0; i < 64; i++) {
      const base = postFall && !microMovement ? 0.15 : 0.3;
      amplitude[i] = base + impactSpike * 0.5 + microSignal * 0.1
        + Math.sin(t * 0.5 + i * 0.1) * 0.1 * (1 - (stillPhase ? 0.7 : 0))
        + _harmonicNoise(t, i * 0.19, 2) * 0.01;
    }

    const rssi = -43 + impactSpike * 8 + wobble * 2 + n.rssi(t) * 0.8;

    return this._baseFrame({
      nodes: [{ node_id: 1, rssi_dbm: rssi, position: [2, 0, 1.5], amplitude, subcarrier_count: 64 }],
      features: {
        mean_rssi: rssi,
        variance: postFall && !microMovement ? 0.5 : (1.5 + impactSpike * 6 + stumbleIntensity * 2),
        std: postFall && !microMovement ? 0.7 : (1.22 + impactSpike * 2),
        motion_band_power: motionPower,
        breathing_band_power: postFall ? 0.08 + Math.abs(n.breath(t)) * 0.02 : 0.1,
        dominant_freq_hz: inFall ? 3.5 : (stumbling ? 1.8 + wobble : 0.15),
        spectral_power: inFall ? 0.9 : (postFall ? 0.1 : 0.2 + stumbleIntensity * 0.3),
      },
      classification: {
        motion_level: postFall && !microMovement ? 'present_still' : (inFall || stumbling ? 'active' : 'present_still'),
        presence: true,
        confidence: inFall ? 0.55 : (stumbling ? 0.7 : (postFall ? 0.6 : 0.85)),
        fall_detected: inFall || postFall,
        pre_fall_warning: stumbling,
      },
      signal_field: { grid_size: [20, 1, 20], values: this._presenceField(10 + px, 10 + pz, postFall ? 1.5 + microIntensity : 2.5, t) },
      vital_signs: {
        breathing_rate_bpm: breathRate,
        heart_rate_bpm: hrRate,
        breathing_confidence: postFall ? 0.5 + _smoothstep(fallEnd, fallEnd + 5, t) * 0.2 : 0.8,
        heart_rate_confidence: postFall ? 0.4 + _smoothstep(fallEnd, fallEnd + 5, t) * 0.2 : 0.7,
      },
      persons: [{ id: 'p0', position: [px, py, pz], motion_score: ms, pose, facing: 0.5, fallProgress: inFall ? (t - fallStart) / (fallEnd - fallStart) : (postFall ? 1 : 0) }],
      estimated_persons: 1,
      edge_modules: {
        fall_detect: {
          detected: inFall || postFall,
          severity: inFall ? 'critical' : (microMovement ? 'monitoring_movement' : (stillPhase ? 'monitoring_still' : 'none')),
          impact_time: postFall ? (fallEnd - fallStart).toFixed(2) : (inFall ? (t - fallStart).toFixed(2) : '0'),
          pre_fall_stumble: stumbling,
          post_fall_movement: microMovement,
          shock_hr_bpm: postFall ? hrRate.toFixed(0) : null,
        },
      },
    });
  }

  // ========================================================================
  // 5. Sleep Monitoring — sleep stages, REM, position changes, apnea buildup
  // ========================================================================

  _sleepMonitoring(t) {
    const n = _getNoiseBank('sleep_monitoring');

    // Sleep stages timeline (30s cycle compressed):
    // 0-4: light sleep (stage 1-2)
    // 4-10: deep sleep (stage 3-4)
    // 10-14: REM sleep
    // 14-16: position change
    // 16-18: light sleep again
    // 18-22: apnea warning signs (breathing gets irregular)
    // 22-26: apnea event
    // 26-30: recovery
    const cycleT = t % 30;

    let sleepStage = 'light';
    let breathRateBase = 14;
    let movementLevel = 0.03;
    let hrBase = 64;
    let eyeMovementArtifact = 0;
    let positionChangeActive = false;

    if (cycleT < 4) {
      // Light sleep: more body movement, higher breath rate
      sleepStage = 'light';
      breathRateBase = 14 + _harmonicNoise(t, 1.1, 2) * 1.5;
      movementLevel = 0.06 + Math.abs(n.motion(t)) * 0.03;
      hrBase = 64 + _harmonicNoise(t, 2.2, 2) * 3;
    } else if (cycleT < 10) {
      // Deep sleep: minimal movement, slow breathing, low HR
      sleepStage = 'deep';
      breathRateBase = 10 + _harmonicNoise(t, 1.3, 2) * 0.5;
      movementLevel = 0.01;
      hrBase = 56 + _harmonicNoise(t, 2.4, 2) * 1;
    } else if (cycleT < 14) {
      // REM: rapid eye movement creates signal artifacts, HR more variable
      sleepStage = 'REM';
      breathRateBase = 16 + _harmonicNoise(t, 1.5, 2) * 2;
      movementLevel = 0.02;
      hrBase = 68 + _harmonicNoise(t, 2.6, 3) * 5; // more variable in REM
      // Eye movement artifact: high-frequency bursts
      const remBurst = Math.sin(t * 12) * Math.sin(t * 7.3) * 0.5;
      eyeMovementArtifact = Math.max(0, remBurst) * 0.08;
    } else if (cycleT < 16) {
      // Position change: brief movement spike
      sleepStage = 'light';
      positionChangeActive = true;
      const changeProgress = (cycleT - 14) / 2;
      movementLevel = changeProgress < 0.5
        ? _smoothstep(0, 0.5, changeProgress) * 0.5
        : _smoothstep(1, 0.5, changeProgress) * 0.5;
      breathRateBase = 16;
      hrBase = 68;
    } else if (cycleT < 18) {
      sleepStage = 'light';
      breathRateBase = 13;
      movementLevel = 0.04;
      hrBase = 62;
    } else if (cycleT < 22) {
      // Pre-apnea: breathing becomes irregular
      sleepStage = 'light';
      const irregularity = _smoothstep(18, 22, cycleT);
      breathRateBase = 12 - irregularity * 6; // slowing down
      // Breathing becomes chaotic before stopping
      const chaotic = irregularity * Math.sin(t * 3 + Math.sin(t * 1.7) * 2) * 0.4;
      breathRateBase = Math.max(3, breathRateBase + chaotic * 5);
      movementLevel = 0.02;
      hrBase = 60 - irregularity * 4;
    } else if (cycleT < 26) {
      // Full apnea
      sleepStage = 'apnea';
      breathRateBase = 0 + Math.abs(n.breath(t)) * 0.5; // near-zero
      movementLevel = 0.01;
      hrBase = 54 + _smoothstep(22, 26, cycleT) * 8; // HR rises during apnea (stress)
    } else {
      // Recovery: gasp, then return to normal
      sleepStage = 'light';
      const recovery = _smoothstep(26, 28, cycleT);
      breathRateBase = 6 + recovery * 10; // gasping then normalizing
      movementLevel = cycleT < 27 ? 0.15 : 0.04; // body startles
      hrBase = 70 - recovery * 6;
    }

    const inApnea = sleepStage === 'apnea';
    const breathFreq = breathRateBase / 60;
    const breathPhase = Math.sin(2 * Math.PI * breathFreq * t + n.breath(t) * 0.3);
    const breathSignal = inApnea ? n.breath(t) * 0.05 : breathPhase;

    // Lying position: slight shifts over time, bigger shift during position change
    const posAngle = positionChangeActive
      ? Math.PI / 2 + _smoothstep(14, 16, cycleT) * Math.PI * 0.3
      : Math.PI / 2 + Math.sin(t * 0.02) * 0.1;
    const lyingX = 3.5 + (positionChangeActive ? Math.sin(cycleT * 2) * 0.3 : 0);

    const amplitude = new Float32Array(64);
    for (let i = 0; i < 64; i++) {
      const base = 0.25 + breathSignal * 0.04 * (1 - (inApnea ? 0.9 : 0));
      const rem = eyeMovementArtifact * (i > 30 && i < 50 ? 1.5 : 0.3); // REM artifact in upper band
      amplitude[i] = base + rem + movementLevel * Math.sin(t * 0.8 + i * 0.1)
        + _harmonicNoise(t, i * 0.11, 2) * 0.005;
    }

    return this._baseFrame({
      nodes: [{ node_id: 1, rssi_dbm: -44 + breathSignal * 0.5 + n.rssi(t) * 0.3, position: [2, 0, 1.5], amplitude, subcarrier_count: 64 }],
      features: {
        mean_rssi: -44 + breathSignal * 0.5 + n.rssi(t) * 0.3,
        variance: inApnea ? 0.15 : (0.6 + movementLevel * 3),
        std: inApnea ? 0.39 : (0.77 + movementLevel),
        motion_band_power: movementLevel + eyeMovementArtifact * 0.5,
        breathing_band_power: inApnea ? 0.02 : (0.1 + Math.abs(breathSignal) * 0.05),
        dominant_freq_hz: breathFreq,
        spectral_power: 0.08 + eyeMovementArtifact * 0.3,
      },
      classification: { motion_level: movementLevel > 0.1 ? 'active' : 'present_still', presence: true, confidence: 0.9, apnea_detected: inApnea },
      signal_field: { grid_size: [20, 1, 20], values: this._presenceField(15, 13, 1.8 + movementLevel * 2, t) },
      vital_signs: {
        breathing_rate_bpm: breathRateBase,
        heart_rate_bpm: hrBase + n.hr(t) * 1,
        breathing_confidence: inApnea ? 0.35 : (0.85 + breathSignal * 0.05),
        heart_rate_confidence: 0.82,
      },
      persons: [{ id: 'p0', position: [lyingX, 0.45, -3.5 + n.pos1(t) * 0.1], motion_score: _clamp(movementLevel * 100, 1, 50), pose: 'lying', facing: posAngle }],
      estimated_persons: 1,
      edge_modules: {
        sleep_staging: {
          stage: sleepStage,
          stage_duration_s: cycleT.toFixed(1),
          position_change: positionChangeActive,
          rem_density: eyeMovementArtifact > 0.02 ? 'high' : 'low',
        },
        sleep_apnea: {
          state: inApnea ? 'apnea_event' : (cycleT >= 18 && cycleT < 22 ? 'pre_apnea_warning' : 'normal'),
          duration_s: inApnea ? (cycleT - 22).toFixed(1) : 0,
          events_total: inApnea ? 1 : 0,
          breathing_irregularity: cycleT >= 18 && cycleT < 22 ? _smoothstep(18, 22, cycleT).toFixed(2) : '0',
        },
        cardiac_arrhythmia: { rhythm: 'sinus', hr_variability: (4.2 + _harmonicNoise(t, 8.8, 2) * 2).toFixed(1) },
      },
    });
  }

  // ========================================================================
  // 6. Intrusion Detection — door pressure, cautious movement, drawer search
  // ========================================================================

  _intrusionDetect(t) {
    const n = _getNoiseBank('intrusion_detect');

    // Timeline:
    // 0-2: baseline (quiet room)
    // 2-3: door opens (pressure change, environmental shift)
    // 3-6: cautious entry (pause-move-pause)
    // 6-10: checking corners
    // 10-14: checks room, pauses
    // 14-22: searches drawers near desk (oscillating position)
    // 22+: settles, loiters

    const doorOpen = t >= 2 && t < 3;
    const entered = t >= 3;
    const cautiousEntry = t >= 3 && t < 6;
    const checkingCorners = t >= 6 && t < 10;
    const settledSearch = t >= 14 && t < 22;
    const loitering = t >= 22;

    // Environmental baseline shift when door opens
    const doorPressure = doorOpen ? Math.sin((t - 2) * Math.PI) * 0.4 : 0;

    // Cautious movement: pause-move-pause pattern
    let px, pz, facing, ms, pose;
    if (!entered) {
      px = -5.5; pz = -2; facing = 0; ms = 0; pose = 'absent';
    } else if (cautiousEntry) {
      // Pause-move-pause pattern
      const entryT = t - 3;
      const movePhase = entryT % 1.5;
      const isMoving = movePhase > 0.6 && movePhase < 1.3; // move for 0.7s, pause for 0.8s
      const progress = Math.min(1, entryT / 3);
      px = -4.5 + progress * 3;
      pz = -1 + progress * 0.8;
      // Slight position jitter during pauses (looking around)
      if (!isMoving) {
        px += Math.sin(t * 4) * 0.05;
        facing = Math.sin(t * 2) * 0.5 + 0.8; // head scanning
      } else {
        facing = Math.atan2(3, 0.8); // heading into room
      }
      ms = isMoving ? 100 : 8;
      pose = 'crouching';
    } else if (checkingCorners) {
      // Move to corners, pause at each
      const cornerT = (t - 6) / 4;
      const cornerIdx = Math.floor(cornerT * 3) % 3;
      const corners = [[-2, -0.5], [0, 1], [2, 0]];
      const corner = corners[cornerIdx];
      const inTransit = (cornerT * 3) % 1 < 0.6;
      px = _lerp(corner[0], corners[(cornerIdx + 1) % 3][0], inTransit ? (cornerT * 3 % 1) / 0.6 : 0);
      pz = _lerp(corner[1], corners[(cornerIdx + 1) % 3][1], inTransit ? (cornerT * 3 % 1) / 0.6 : 0);
      facing = inTransit ? Math.atan2(corners[(cornerIdx + 1) % 3][0] - corner[0], corners[(cornerIdx + 1) % 3][1] - corner[1]) : Math.sin(t * 3) * Math.PI; // scanning while paused
      ms = inTransit ? 120 : 10;
      pose = 'crouching';
    } else if (settledSearch) {
      // Oscillating near desk area, opening drawers
      const searchT = t - 14;
      const deskX = 1.5, deskZ = -0.5;
      px = deskX + Math.sin(searchT * 1.2) * 0.6; // back and forth along desk
      pz = deskZ + Math.cos(searchT * 0.8) * 0.3;
      // Periodic reaching motion (drawer open/close every ~2s)
      const reaching = Math.sin(searchT * Math.PI) > 0.7;
      facing = reaching ? 0 : Math.PI * 0.5;
      ms = reaching ? 80 : 30;
      pose = reaching ? 'reaching' : 'standing';
    } else if (loitering) {
      px = 0.5 + n.pos1(t) * 0.2;
      pz = 0.5 + n.pos2(t) * 0.2;
      facing = Math.sin(t * 0.3) * Math.PI;
      ms = 12 + Math.abs(n.motion(t)) * 8;
      pose = 'standing';
    } else {
      // 10-14: general room check
      const checkT = (t - 10) / 4;
      px = -1 + Math.sin(checkT * Math.PI * 2) * 2;
      pz = Math.cos(checkT * Math.PI * 2) * 1.5;
      facing = Math.atan2(Math.cos(checkT * Math.PI * 2) * 2, -Math.sin(checkT * Math.PI * 2) * 1.5);
      ms = 90;
      pose = 'walking';
    }

    const isMovingNow = ms > 50;
    const amplitude = new Float32Array(64);
    for (let i = 0; i < 64; i++) {
      amplitude[i] = entered
        ? 0.35 + 0.15 * Math.sin(t * 1.5 + i * 0.2) + _harmonicNoise(t, i * 0.14, 2) * 0.01
        : 0.1 + doorPressure * 0.05 + _harmonicNoise(t, i * 0.14, 2) * 0.008;
    }

    const rssiBase = entered ? -38 + Math.sin(t * 2) * 3 + n.rssi(t) : -46 + doorPressure * 2 + n.rssi(t) * 0.3;

    return this._baseFrame({
      nodes: [{ node_id: 1, rssi_dbm: rssiBase, position: [2, 0, 1.5], amplitude, subcarrier_count: 64 }],
      features: {
        mean_rssi: rssiBase,
        variance: isMovingNow ? 4.5 : (entered ? 1.0 : 0.2 + Math.abs(doorPressure) * 1.5),
        std: isMovingNow ? 2.1 : 0.45,
        motion_band_power: isMovingNow ? 0.4 : (entered ? 0.03 + (settledSearch ? 0.08 : 0) : 0.01 + Math.abs(doorPressure) * 0.15),
        breathing_band_power: entered && !isMovingNow ? 0.08 : 0.01,
        dominant_freq_hz: isMovingNow ? 1.8 : (doorOpen ? 0.5 : 0.1),
        spectral_power: isMovingNow ? 0.55 : 0.03 + Math.abs(doorPressure) * 0.2,
      },
      classification: {
        motion_level: isMovingNow ? 'active' : (entered ? 'present_still' : 'absent'),
        presence: entered || doorOpen,
        confidence: entered ? 0.78 + (loitering ? 0.1 : 0) : (doorOpen ? 0.45 : 0.95),
        intrusion: entered,
        perimeter_breach: t >= 3 && t < 5,
        door_event: doorOpen,
      },
      signal_field: { grid_size: [20, 1, 20], values: entered ? this._presenceField(10 + px, 10 + pz, 2, t) : this._flatField(0.04 + Math.abs(doorPressure) * 0.06) },
      vital_signs: {
        breathing_rate_bpm: entered && !isMovingNow ? 20 + n.breath(t) : 0,
        heart_rate_bpm: entered ? 90 + _harmonicNoise(t, 4.4, 2) * 5 : 0, // elevated from adrenaline
        breathing_confidence: entered && !isMovingNow ? 0.6 : 0,
        heart_rate_confidence: entered && !isMovingNow ? 0.4 : 0,
      },
      persons: entered ? [{ id: 'p0', position: [px, 0, pz], motion_score: ms, pose, facing }] : [],
      estimated_persons: entered ? 1 : 0,
      edge_modules: {
        intrusion: {
          detected: entered,
          zone: cautiousEntry ? 'perimeter' : (settledSearch ? 'desk_area' : 'interior'),
          threat_level: cautiousEntry ? 'high' : (settledSearch ? 'high' : (loitering ? 'medium' : 'none')),
          behavior_pattern: cautiousEntry ? 'cautious_entry' : (checkingCorners ? 'corner_check' : (settledSearch ? 'searching' : (loitering ? 'loitering' : 'none'))),
        },
        loitering: { detected: loitering || settledSearch, duration_s: loitering ? (t - 22).toFixed(1) : (settledSearch ? (t - 14).toFixed(1) : 0) },
        door_sensor: { open_event: doorOpen, pressure_delta: doorPressure.toFixed(3) },
      },
    });
  }

  // ========================================================================
  // 7. Gesture Control — distinct gesture signatures, recognition feedback
  // ========================================================================

  _gestureControl(t) {
    const n = _getNoiseBank('gesture_control');

    const gestureCycle = 7; // seconds per gesture
    const gesturePhase = Math.floor(t / gestureCycle) % 4;
    const gestures = ['wave', 'swipe_left', 'circle', 'point'];
    const gestureT = t % gestureCycle;
    const isGesturing = gestureT >= 1.5 && gestureT < 5;
    const gestureProgress = isGesturing ? (gestureT - 1.5) / 3.5 : 0;
    const gestureEnvelope = isGesturing ? Math.sin(gestureProgress * Math.PI) : 0;

    // Recognition feedback: brief confidence spike when gesture completes (at ~80% progress)
    const recognitionMoment = isGesturing && gestureProgress > 0.75 && gestureProgress < 0.85;
    const recognitionBoost = recognitionMoment ? 0.15 : 0;

    // Gesture-specific signal characteristics
    let gestureSignal = 0;
    let dominantFreq = 0.2;
    let motionScore = 10;
    let gestureDetail = {};

    const g = gestures[gesturePhase];
    if (isGesturing) {
      switch (g) {
        case 'wave':
          // Fast oscillation (hand waving back and forth)
          gestureSignal = Math.sin(t * 14) * gestureEnvelope * 0.5
            + Math.sin(t * 21) * gestureEnvelope * 0.2; // harmonics
          dominantFreq = 4.0 + _harmonicNoise(t, 6.6, 2) * 0.3;
          motionScore = 150 * gestureEnvelope;
          gestureDetail = { oscillation_hz: 7, amplitude: gestureEnvelope.toFixed(2) };
          break;
        case 'swipe_left':
          // Clear directional shift: signal ramps in one direction
          gestureSignal = (gestureProgress - 0.5) * 2 * gestureEnvelope * 0.6;
          dominantFreq = 2.0;
          motionScore = 180 * gestureEnvelope;
          gestureDetail = { direction: 'left', displacement: gestureSignal.toFixed(3) };
          break;
        case 'circle':
          // Rotating phase pattern
          const circleAngle = gestureProgress * Math.PI * 2 * 1.5; // 1.5 rotations
          gestureSignal = Math.sin(circleAngle) * gestureEnvelope * 0.4;
          const phaseRotation = Math.cos(circleAngle) * gestureEnvelope * 0.4;
          dominantFreq = 3.0;
          motionScore = 130 * gestureEnvelope;
          gestureDetail = { rotation_angle: (circleAngle * 180 / Math.PI).toFixed(0), phase_i: gestureSignal.toFixed(3), phase_q: phaseRotation.toFixed(3) };
          break;
        case 'point':
          // Quick, decisive: sharp onset, brief hold, sharp offset
          const pointEnvelope = gestureProgress < 0.2
            ? _smoothstep(0, 0.2, gestureProgress) // fast rise
            : (gestureProgress < 0.6 ? 1.0 : _smoothstep(1, 0.6, gestureProgress)); // hold then drop
          gestureSignal = pointEnvelope * 0.55;
          dominantFreq = 1.5; // lower freq, more impulse-like
          motionScore = 200 * pointEnvelope;
          gestureDetail = { sharpness: pointEnvelope > 0.9 ? 'locked' : 'transitioning' };
          break;
      }
    }

    const amplitude = new Float32Array(64);
    for (let i = 0; i < 64; i++) {
      const base = 0.3 + _harmonicNoise(t, i * 0.13, 2) * 0.01;
      // Each gesture affects subcarriers differently
      let gestMod = 0;
      if (isGesturing) {
        if (g === 'wave') gestMod = Math.sin(t * 14 + i * 0.5) * gestureEnvelope * 0.15;
        else if (g === 'swipe_left') gestMod = gestureSignal * (i / 64) * 0.2; // gradient across band
        else if (g === 'circle') gestMod = Math.sin(t * 8 + i * 0.3) * gestureEnvelope * 0.12;
        else if (g === 'point') gestMod = gestureSignal * 0.2 * (i > 25 && i < 40 ? 1.5 : 0.5);
      }
      amplitude[i] = base + gestMod;
    }

    const rssi = -41 + gestureEnvelope * 3 + n.rssi(t) * 0.5;
    const confidence = isGesturing ? 0.7 + gestureEnvelope * 0.15 + recognitionBoost : 0;

    return this._baseFrame({
      nodes: [{ node_id: 1, rssi_dbm: rssi, position: [2, 0, 1.5], amplitude, subcarrier_count: 64 }],
      features: {
        mean_rssi: rssi,
        variance: 1.2 + gestureEnvelope * 2.5,
        std: 1.1 + gestureEnvelope * 0.5,
        motion_band_power: 0.05 + Math.abs(gestureSignal) * 0.6,
        breathing_band_power: 0.08,
        dominant_freq_hz: dominantFreq,
        spectral_power: 0.15 + gestureEnvelope * 0.4,
      },
      classification: {
        motion_level: isGesturing ? 'active' : 'present_still',
        presence: true,
        confidence: 0.85,
        gesture: isGesturing ? g : null,
        gesture_recognized: recognitionMoment,
      },
      signal_field: { grid_size: [20, 1, 20], values: this._presenceField(10, 10, 2 + gestureEnvelope * 0.8, t) },
      vital_signs: { breathing_rate_bpm: 16 + n.breath(t) * 0.5, heart_rate_bpm: 74 + n.hr(t) * 1, breathing_confidence: 0.7, heart_rate_confidence: 0.65 },
      persons: [{ id: 'p0', position: [0 + n.pos1(t) * 0.03, 0, 0.5 + n.pos2(t) * 0.03], motion_score: motionScore, pose: 'gesturing', facing: Math.PI, gestureType: g, gestureIntensity: gestureEnvelope, gestureDetail }],
      estimated_persons: 1,
      edge_modules: {
        gesture: {
          detected: isGesturing,
          type: isGesturing ? g : 'none',
          confidence: confidence,
          dtw_distance: isGesturing ? (12.3 - recognitionBoost * 8) : 999,
          recognition_feedback: recognitionMoment ? 'MATCHED' : null,
          detail: gestureDetail,
        },
      },
    });
  }

  // ========================================================================
  // 8. Crowd Occupancy — clustering, stationary person, rushing, entry/exit
  // ========================================================================

  _crowdOccupancy(t) {
    const n = _getNoiseBank('crowd_occupancy');

    // Points of interest for clustering
    const poi = [
      { x: -2, z: -1.5, label: 'display' },  // display/kiosk
      { x: 2, z: 1, label: 'counter' },       // service counter
      { x: 0, z: 0, label: 'center' },        // open area
    ];

    // 5 people with distinct behaviors
    const persons = [];
    const count = 5;

    // Person 0: sits stationary at desk
    {
      const px = -1.5 + n.pos1(t) * 0.02;
      const pz = 1.5 + n.pos2(t) * 0.02;
      persons.push({ id: 'p0', position: [px, 0, pz], motion_score: 3, pose: 'sitting', facing: Math.PI * 0.5 + _harmonicNoise(t, 11.1, 2) * 0.1 });
    }

    // Person 1: browses near display (clusters near POI 0)
    {
      const browseT = t * 0.3;
      const px = poi[0].x + Math.sin(browseT) * 0.8 + _harmonicNoise(t, 12.1, 2) * 0.1;
      const pz = poi[0].z + Math.cos(browseT * 0.7) * 0.6;
      const facing = Math.atan2(poi[0].x - px, poi[0].z - pz);
      persons.push({ id: 'p1', position: [px, 0, pz], motion_score: 40 + Math.abs(_harmonicNoise(t, 13.1, 2)) * 20, pose: 'walking', facing });
    }

    // Person 2: rushes through (faster speed, enters and exits)
    {
      const rushCycle = 20;
      const rushT = t % rushCycle;
      const inSpace = rushT > 2 && rushT < 15;
      const rushProgress = inSpace ? (rushT - 2) / 13 : 0;
      const rushSpeed = 1.5 + _harmonicNoise(t, 14.1, 2) * 0.2;
      const px = inSpace ? -4 + rushProgress * 8 : -5;
      const pz = inSpace ? -0.5 + Math.sin(rushProgress * Math.PI * 0.5) * 0.8 : -3;
      if (inSpace) {
        persons.push({ id: 'p2', position: [px, 0, pz], motion_score: 220, pose: 'walking', facing: 0.1, speed: rushSpeed });
      }
    }

    // Person 3: walks between display and counter (clusters near POIs)
    {
      const walkT = t * 0.15;
      const poiIdx = Math.floor(walkT) % 2;
      const target = poiIdx === 0 ? poi[0] : poi[1];
      const progress = walkT % 1;
      const other = poiIdx === 0 ? poi[1] : poi[0];
      const px = _lerp(other.x, target.x, _smoothstep(0, 0.7, progress))
        + _harmonicNoise(t, 15.1, 2) * 0.15;
      const pz = _lerp(other.z, target.z, _smoothstep(0, 0.7, progress))
        + _harmonicNoise(t, 16.1, 2) * 0.15;
      const facing = Math.atan2(target.x - other.x, target.z - other.z);
      const nearPoi = progress > 0.7;
      persons.push({ id: 'p3', position: [px, 0, pz], motion_score: nearPoi ? 15 : 100, pose: nearPoi ? 'standing' : 'walking', facing });
    }

    // Person 4: enters/exits periodically
    {
      const cycleLen = 25;
      const ct = t % cycleLen;
      const entering = ct < 3;
      const inside = ct >= 3 && ct < 18;
      const exiting = ct >= 18 && ct < 21;
      if (entering || inside || exiting) {
        let px, pz;
        if (entering) {
          px = -4.5 + (ct / 3) * 3;
          pz = -2 + (ct / 3) * 1;
        } else if (exiting) {
          const ep = (ct - 18) / 3;
          px = 1 + ep * 3;
          pz = 0.5 + ep * 1;
        } else {
          px = poi[2].x + Math.sin(t * 0.2) * 1.5;
          pz = poi[2].z + Math.cos(t * 0.15) * 1;
        }
        persons.push({ id: 'p4', position: [px, 0, pz], motion_score: (entering || exiting) ? 130 : 70, pose: 'walking', facing: entering ? 0.3 : (exiting ? -0.3 : Math.atan2(Math.cos(t * 0.2), -Math.sin(t * 0.15))) });
      }
    }

    const actualCount = persons.length;

    const amplitude = new Float32Array(64);
    for (let i = 0; i < 64; i++) {
      amplitude[i] = 0.35 + 0.25 * Math.abs(Math.sin(t * 1.5 + i * 0.2))
        + _harmonicNoise(t, i * 0.16, 2) * 0.015;
    }

    // Signal field with congestion patterns around POIs
    const vals = [];
    for (let iz = 0; iz < 20; iz++) {
      for (let ix = 0; ix < 20; ix++) {
        let v = 0;
        for (const p of persons) {
          const dx = (ix - 10) / 3 - p.position[0];
          const dz = (iz - 10) / 3 - p.position[2];
          v += _gaussian(Math.sqrt(dx * dx + dz * dz), 0.9) * 0.4;
        }
        // POI congestion haze
        for (const p of poi) {
          const dx = (ix - 10) / 3 - p.x;
          const dz = (iz - 10) / 3 - p.z;
          v += _gaussian(Math.sqrt(dx * dx + dz * dz), 2.0) * 0.08;
        }
        vals.push(_clamp(v + _harmonicNoise(t + ix * 0.3, iz * 0.4, 2) * 0.01, 0, 1));
      }
    }

    return this._baseFrame({
      nodes: [{ node_id: 1, rssi_dbm: -37 + Math.sin(t * 0.9) * 5 + n.rssi(t), position: [2, 0, 1.5], amplitude, subcarrier_count: 64 }],
      features: {
        mean_rssi: -37 + Math.sin(t * 0.9) * 5 + n.rssi(t),
        variance: 5.2 + Math.sin(t * 0.6) * 1.5 + actualCount * 0.3,
        std: 2.28 + actualCount * 0.1,
        motion_band_power: 0.25 + actualCount * 0.05,
        breathing_band_power: 0.04,
        dominant_freq_hz: 1.5,
        spectral_power: 0.45 + actualCount * 0.03,
      },
      classification: { motion_level: 'active', presence: true, confidence: 0.76 - (actualCount > 4 ? 0.05 : 0) },
      signal_field: { grid_size: [20, 1, 20], values: vals },
      vital_signs: { breathing_rate_bpm: 0, heart_rate_bpm: 0, breathing_confidence: 0.15, heart_rate_confidence: 0.1 },
      persons,
      estimated_persons: actualCount,
      edge_modules: {
        occupancy: {
          count: actualCount,
          zones: {
            display: persons.filter(p => Math.sqrt((p.position[0] - poi[0].x) ** 2 + (p.position[2] - poi[0].z) ** 2) < 2).length,
            counter: persons.filter(p => Math.sqrt((p.position[0] - poi[1].x) ** 2 + (p.position[2] - poi[1].z) ** 2) < 2).length,
            center: persons.filter(p => Math.sqrt((p.position[0] - poi[2].x) ** 2 + (p.position[2] - poi[2].z) ** 2) < 2).length,
          },
          density: (actualCount / 20).toFixed(2), // per sq meter
          congestion_zones: actualCount > 3 ? ['display'] : [],
        },
        customer_flow: {
          entries: Math.floor(t / 25) + 1,
          exits: Math.floor(t / 25),
          dwell_avg_s: 145 + _harmonicNoise(t, 17.1, 2) * 20,
        },
      },
    });
  }

  // ========================================================================
  // 9. Search & Rescue — scanning, false positives, triangulation, gradual lock-on
  // ========================================================================

  _searchRescue(t) {
    const n = _getNoiseBank('search_rescue');

    // Timeline:
    // 0-4: scanning phase (signal sweeps, no detection)
    // 4-7: first false positive (ghost echo)
    // 7-10: second scan, another brief false positive
    // 10-14: genuine signal detected, gradual lock-on
    // 14-20: confirmed detection, vital extraction (confidence building)
    // 20+: stable monitoring

    const scanning = t < 4;
    const falsePos1 = t >= 4 && t < 7;
    const scan2 = t >= 7 && t < 10;
    const falsePos2 = t >= 7 && t < 8.5;
    const genuineDetect = t >= 10;
    const lockingOn = t >= 10 && t < 14;
    const confirmed = t >= 14;
    const stableMonitor = t >= 20;

    // Scan sweep effect (nodes cycle through angles)
    const scanAngle = t * 0.8;

    // Triangulation: 3 sensor nodes with different signal strengths
    const targetPos = [3.5, 0, 0];
    const nodePositions = [[2, 0, 1.5], [-2, 0, 1.5], [0, 0, -2]];
    const nodes = [];

    for (let ni = 0; ni < 3; ni++) {
      const npos = nodePositions[ni];
      const dist = Math.sqrt((npos[0] - targetPos[0]) ** 2 + (npos[2] - targetPos[2]) ** 2);
      const baseSignal = -62 - dist * 3; // signal attenuation with distance
      const scanMod = scanning ? Math.sin(scanAngle + ni * 2.1) * 4 : 0;
      const falseSignal = (falsePos1 && ni === 0) ? Math.sin((t - 4) * 3) * 3 : 0;
      const genuineSignal = genuineDetect ? _smoothstep(10, 14, t) * 5 : 0;

      const amp = new Float32Array(64);
      for (let i = 0; i < 64; i++) {
        amp[i] = 0.08 + (genuineDetect ? 0.06 * _smoothstep(10, 16, t) : 0)
          * Math.sin(t * 0.4 + i * 0.15 + ni)
          + _harmonicNoise(t, i * 0.12 + ni * 100, 2) * 0.008;
      }

      nodes.push({
        node_id: ni + 1,
        rssi_dbm: baseSignal + scanMod + falseSignal + genuineSignal + n.rssi(t) * 0.5,
        position: npos,
        amplitude: amp,
        subcarrier_count: 64,
        distance_estimate: genuineDetect ? (dist + (1 - _smoothstep(10, 20, t)) * 3).toFixed(2) : null,
      });
    }

    // Confidence builds gradually during lock-on
    let confidence;
    if (scanning) confidence = 0.08 + Math.abs(n.motion(t)) * 0.05;
    else if (falsePos1) confidence = 0.25 + Math.sin((t - 4) * 2) * 0.15; // fluctuating
    else if (scan2 && !falsePos2) confidence = 0.1;
    else if (falsePos2) confidence = 0.2 + Math.sin((t - 7) * 3) * 0.1;
    else if (lockingOn) confidence = 0.2 + _smoothstep(10, 14, t) * 0.3;
    else if (confirmed && !stableMonitor) confidence = 0.5 + _smoothstep(14, 20, t) * 0.2;
    else confidence = 0.7 + n.env(t) * 0.03;

    // Vital sign extraction: gradual confidence over 10+ seconds after detection
    const vitalConfidence = confirmed ? _smoothstep(14, 25, t) : 0;
    const breathRate = genuineDetect ? 10 + _harmonicNoise(t, 3.3, 2) * 0.5 : 0;
    const breathPhase = Math.sin(2 * Math.PI * (breathRate / 60) * t);

    // Detected persons
    let detected = false;
    let triageColor = 'unknown';
    if (falsePos1) { detected = true; triageColor = 'unknown'; }
    else if (falsePos2) { detected = true; triageColor = 'unknown'; }
    else if (genuineDetect) { detected = true; triageColor = confidence > 0.5 ? 'yellow' : 'unknown'; }

    const persons = detected && genuineDetect
      ? [{ id: 'p0', position: targetPos, motion_score: 2, pose: 'lying', facing: 0, signal_strength: confidence }]
      : (detected ? [{ id: 'ghost', position: [falsePos1 ? 1 : -1, 0, falsePos2 ? 2 : -1], motion_score: 1, pose: 'unknown', facing: 0 }] : []);

    return this._baseFrame({
      nodes,
      features: {
        mean_rssi: nodes[0].rssi_dbm,
        variance: genuineDetect ? 0.4 + breathPhase * 0.1 * vitalConfidence : 0.15,
        std: 0.63,
        motion_band_power: 0.01 + (scanning ? Math.abs(Math.sin(scanAngle)) * 0.02 : 0),
        breathing_band_power: genuineDetect ? 0.05 * vitalConfidence + breathPhase * 0.02 * vitalConfidence : 0.005,
        dominant_freq_hz: genuineDetect && vitalConfidence > 0.3 ? 0.167 : (scanning ? 0.8 : 0.02),
        spectral_power: 0.04 + (scanning ? 0.03 : 0),
      },
      classification: {
        motion_level: genuineDetect ? 'present_still' : 'absent',
        presence: detected,
        confidence,
        through_wall: true,
        triage_color: triageColor,
        false_positive: (falsePos1 || falsePos2) && !genuineDetect,
        scan_phase: scanning ? 'sweeping' : (lockingOn ? 'locking_on' : (confirmed ? 'confirmed' : 'searching')),
      },
      signal_field: { grid_size: [20, 1, 20], values: this._searchRescueField(t, scanning, genuineDetect, confidence) },
      vital_signs: {
        breathing_rate_bpm: genuineDetect && vitalConfidence > 0.2 ? breathRate : 0,
        heart_rate_bpm: genuineDetect && vitalConfidence > 0.4 ? 55 + n.hr(t) * 2 : 0,
        breathing_confidence: genuineDetect ? vitalConfidence * 0.85 : 0,
        heart_rate_confidence: genuineDetect ? vitalConfidence * 0.5 : 0,
      },
      persons,
      estimated_persons: detected ? 1 : 0,
      edge_modules: {
        wifi_mat: {
          mode: scanning ? 'scanning' : (lockingOn ? 'locking_on' : (confirmed ? 'monitoring' : 'search')),
          survivors_detected: genuineDetect ? 1 : 0,
          triage: genuineDetect && confidence > 0.5 ? 'delayed' : 'searching',
          signal_through_material: 'concrete_30cm',
          false_positives_filtered: (falsePos1 || falsePos2) ? 1 : 0,
          triangulation_nodes: genuineDetect ? 3 : 0,
          vital_extraction_confidence: (vitalConfidence * 100).toFixed(0) + '%',
        },
      },
    });
  }

  // ========================================================================
  // 10. Elderly Care — gait asymmetry, gradual transitions, rest & recover
  // ========================================================================

  _elderlyCare(t) {
    const n = _getNoiseBank('elderly_care');

    // Timeline:
    // 0-12: walking with gait analysis
    // 12-14: slowing down
    // 14-16: reaching for chair (transitional)
    // 16-18: sitting transition
    // 18-24: resting (HR comes down)
    // 24+: light activity while seated

    const walkPhase = t < 12;
    const slowingDown = t >= 12 && t < 14;
    const reachingChair = t >= 14 && t < 16;
    const sittingTransition = t >= 16 && t < 18;
    const resting = t >= 18 && t < 24;
    const seated = t >= 18;

    // Walking speed decreases gradually
    const walkSpeed = walkPhase ? 0.6 - _smoothstep(8, 12, t) * 0.2 : (slowingDown ? 0.3 * (1 - _smoothstep(12, 14, t)) : 0);

    // Gait analysis: slight asymmetry in step timing (right step ~5% longer)
    const stepFreq = walkPhase ? 1.4 + _harmonicNoise(t, 1.1, 2) * 0.05 : 0;
    const stepPhaseR = Math.sin(2 * Math.PI * stepFreq * t);
    const stepPhaseL = Math.sin(2 * Math.PI * stepFreq * t + Math.PI + 0.15); // asymmetry
    const stepAsymmetry = Math.abs(stepPhaseR) - Math.abs(stepPhaseL);

    // Position
    let px, pz, facing, ms, pose;
    if (walkPhase) {
      const wp = t * walkSpeed;
      px = Math.sin(wp * 0.25) * 2;
      pz = Math.cos(wp * 0.15) * 1.2;
      facing = Math.atan2(Math.cos(wp * 0.25) * 0.25 * walkSpeed, -Math.sin(wp * 0.15) * 0.15 * walkSpeed);
      ms = 60 + stepAsymmetry * 10;
      pose = 'walking';
    } else if (slowingDown) {
      const sp = _smoothstep(12, 14, t);
      px = _lerp(Math.sin(12 * 0.6 * 0.25) * 2, 1, sp);
      pz = _lerp(Math.cos(12 * 0.6 * 0.15) * 1.2, -1.5, sp);
      facing = Math.atan2(1 - px, -1.5 - pz);
      ms = 30 * (1 - sp);
      pose = 'walking';
    } else if (reachingChair) {
      const rp = _smoothstep(14, 16, t);
      px = 1 + Math.sin(t * 2) * 0.05 * (1 - rp); // slight unsteadiness reaching
      pz = -1.5;
      facing = Math.PI * 0.25;
      ms = 20 * (1 - rp);
      pose = 'reaching';
    } else if (sittingTransition) {
      const sp = _smoothstep(16, 18, t);
      px = 1;
      pz = -1.5;
      facing = Math.PI * 0.25;
      ms = 15 * (1 - sp);
      pose = sp > 0.5 ? 'sitting' : 'reaching';
    } else {
      px = 1 + n.pos1(t) * 0.02;
      pz = -1.5 + n.pos2(t) * 0.02;
      facing = Math.PI * 0.25 + _harmonicNoise(t, 5.5, 2) * 0.1;
      ms = 5 + Math.abs(n.motion(t)) * 3;
      pose = 'sitting';
    }

    // Heart rate: walking ~82, elevated slightly from walking exertion,
    // then gradually comes down during rest (physiological recovery)
    let hrBase;
    if (walkPhase) hrBase = 82 + walkSpeed * 5;
    else if (slowingDown || reachingChair) hrBase = 78 - _smoothstep(12, 16, t) * 5;
    else if (resting) hrBase = 73 - _smoothstep(18, 24, t) * 5; // slow recovery
    else hrBase = 68;
    hrBase += n.hr(t) * 1.5;

    // Breathing: correlated with HR
    let breathRate;
    if (walkPhase) breathRate = 18 + walkSpeed * 3;
    else if (seated) breathRate = 14 - _smoothstep(18, 24, t) * 2;
    else breathRate = 16;
    breathRate += n.breath(t) * 0.5;

    // Blood pressure proxy: HR/breathing correlation
    const hrBreathRatio = hrBase / breathRate;

    const amplitude = new Float32Array(64);
    for (let i = 0; i < 64; i++) {
      amplitude[i] = 0.3 + (walkPhase ? 0.15 : 0.06) * Math.sin(t * 0.8 + i * 0.12)
        + stepAsymmetry * 0.02
        + _harmonicNoise(t, i * 0.11, 2) * 0.008;
    }

    return this._baseFrame({
      nodes: [{ node_id: 1, rssi_dbm: -41 + Math.sin(t * 0.5) * 2 + n.rssi(t) * 0.5, position: [2, 0, 1.5], amplitude, subcarrier_count: 64 }],
      features: {
        mean_rssi: -41 + Math.sin(t * 0.5) * 2 + n.rssi(t) * 0.5,
        variance: walkPhase ? 2.2 + stepAsymmetry * 0.5 : 0.8,
        std: walkPhase ? 1.48 : 0.89,
        motion_band_power: walkPhase ? 0.15 + Math.abs(stepPhaseR) * 0.05 : (ms > 10 ? 0.05 : 0.02),
        breathing_band_power: 0.1 + Math.abs(n.breath(t)) * 0.02,
        dominant_freq_hz: walkPhase ? stepFreq * 0.5 : 0.23,
        spectral_power: 0.22,
      },
      classification: { motion_level: walkPhase ? 'active' : (ms > 15 ? 'active' : 'present_still'), presence: true, confidence: 0.88 },
      signal_field: { grid_size: [20, 1, 20], values: this._presenceField(10 + px * 2, 10 + pz * 2, 2.5, t) },
      vital_signs: {
        breathing_rate_bpm: breathRate,
        heart_rate_bpm: hrBase,
        breathing_confidence: 0.82,
        heart_rate_confidence: 0.78,
        hr_breath_ratio: hrBreathRatio.toFixed(2),
      },
      persons: [{ id: 'p0', position: [px, 0, pz], motion_score: ms, pose, facing }],
      estimated_persons: 1,
      edge_modules: {
        gait_analysis: {
          step_frequency: walkPhase ? stepFreq.toFixed(2) : 0,
          stride_length_m: walkPhase ? (0.48 + _harmonicNoise(t, 7.7, 2) * 0.02).toFixed(3) : 0,
          symmetry: walkPhase ? (0.82 + stepAsymmetry * 0.1).toFixed(3) : null,
          asymmetry_side: walkPhase ? 'right_longer' : null,
          fall_risk: walkPhase ? 'low' : 'none',
        },
        vital_trend: {
          status: 'normal',
          hr_trend: resting ? 'recovering' : (walkPhase ? 'elevated' : 'stable'),
          recovery_phase: resting,
          bp_proxy: hrBreathRatio > 5.5 ? 'elevated' : 'normal',
        },
        pattern_sequence: {
          activity: walkPhase ? 'walking' : (reachingChair || sittingTransition ? 'transitioning' : 'resting'),
          transition: reachingChair ? 'reaching_chair' : (sittingTransition ? 'sitting_down' : (slowingDown ? 'slowing' : null)),
          routine_deviation: false,
        },
      },
    });
  }

  // ========================================================================
  // 11. Fitness Tracking — warm-up, intensity ramp, rest intervals, HR lag
  // ========================================================================

  _fitnessTracking(t) {
    const n = _getNoiseBank('fitness_tracking');

    // Timeline:
    // 0-3: warm-up (slow movements, gradually increasing)
    // 3-9: jumping jacks (high intensity)
    // 9-12: rest interval
    // 12-18: squats (medium intensity)
    // 18-21: rest interval
    // 21-27: jumping jacks again (peak intensity)
    // 27-30: cool-down

    const block = t % 30;
    let exerciseType = 'rest';
    let targetIntensity = 0; // 0-1 target exertion
    let actualMotion = 0;

    if (block < 3) {
      // Warm-up: ramp from 0 to 0.4
      exerciseType = 'warmup';
      targetIntensity = _smoothstep(0, 3, block) * 0.4;
      actualMotion = targetIntensity * 0.8;
    } else if (block < 9) {
      // Jumping jacks
      exerciseType = 'jumping_jacks';
      targetIntensity = 0.7 + _smoothstep(3, 5, block) * 0.2;
      // Rhythmic motion with 2 Hz cadence
      actualMotion = targetIntensity * (0.7 + 0.3 * Math.abs(Math.sin(t * Math.PI * 2)));
    } else if (block < 12) {
      // Rest
      exerciseType = 'rest';
      targetIntensity = 0.1 * (1 - _smoothstep(9, 11, block));
      actualMotion = 0.05 + Math.abs(n.motion(t)) * 0.03; // slight fidgeting
    } else if (block < 18) {
      // Squats: slower, deeper movement
      exerciseType = 'squats';
      targetIntensity = 0.6 + _smoothstep(12, 14, block) * 0.15;
      // Slower cadence (~0.5 Hz), smooth up/down
      const squatPhase = Math.sin(t * Math.PI * 0.5);
      actualMotion = targetIntensity * (0.5 + 0.5 * Math.abs(squatPhase));
    } else if (block < 21) {
      // Rest
      exerciseType = 'rest';
      targetIntensity = 0.1 * (1 - _smoothstep(18, 20, block));
      actualMotion = 0.05;
    } else if (block < 27) {
      // Jumping jacks peak
      exerciseType = 'jumping_jacks';
      targetIntensity = 0.85 + _smoothstep(21, 23, block) * 0.15;
      actualMotion = targetIntensity * (0.7 + 0.3 * Math.abs(Math.sin(t * Math.PI * 2.2)));
    } else {
      // Cool-down
      exerciseType = 'cooldown';
      targetIntensity = 0.3 * (1 - _smoothstep(27, 30, block));
      actualMotion = targetIntensity * 0.5;
    }

    // HR lags behind exertion by ~5-8 seconds (physiological delay)
    // Simulate with a slow-tracking variable
    const hrTarget = 70 + targetIntensity * 90; // 70 rest -> 160 max
    // IIR-filtered HR that follows target with delay
    const hrLagFactor = 0.92; // higher = more lag
    const hrDelayed = hrTarget + (70 - hrTarget) * Math.exp(-t * 0.15) * (exerciseType === 'rest' ? 0.5 : 0.2);
    // Use harmonic noise to approximate the lag behavior in a stateless way
    const hrSmooth = hrTarget - _harmonicNoise(t - 3, 8.8, 2) * 5 * targetIntensity;
    const hrRate = _clamp(hrSmooth + n.hr(t) * 2, 60, 185);

    // Breathing also lags but less
    const breathTarget = 14 + targetIntensity * 20;
    const breathRate = _clamp(breathTarget + n.breath(t) * 1.5 - _harmonicNoise(t - 1, 9.9, 2) * 2, 12, 40);

    const repCount = Math.floor(t * (exerciseType === 'jumping_jacks' ? 1.0 : (exerciseType === 'squats' ? 0.25 : 0)));

    // Vertical motion for exercises
    const verticalPos = exerciseType === 'jumping_jacks'
      ? Math.abs(Math.sin(t * Math.PI * 2)) * 0.15
      : (exerciseType === 'squats' ? -Math.abs(Math.sin(t * Math.PI * 0.5)) * 0.3 : 0);

    const amplitude = new Float32Array(64);
    for (let i = 0; i < 64; i++) {
      amplitude[i] = 0.35 + 0.3 * actualMotion * Math.abs(Math.sin(t * 2.5 + i * 0.25))
        + _harmonicNoise(t, i * 0.15, 2) * 0.01;
    }

    const ms = _clamp(actualMotion * 255, 5, 255);

    return this._baseFrame({
      nodes: [{ node_id: 1, rssi_dbm: -39 + actualMotion * 6 + n.rssi(t), position: [2, 0, 1.5], amplitude, subcarrier_count: 64 }],
      features: {
        mean_rssi: -39 + actualMotion * 6 + n.rssi(t),
        variance: 2 + actualMotion * 4,
        std: 1.4 + actualMotion * 1.5,
        motion_band_power: 0.05 + actualMotion * 0.55,
        breathing_band_power: 0.08 + targetIntensity * 0.1,
        dominant_freq_hz: exerciseType === 'jumping_jacks' ? 2.0 : (exerciseType === 'squats' ? 0.5 : 0.2),
        spectral_power: 0.1 + actualMotion * 0.5,
      },
      classification: { motion_level: actualMotion > 0.3 ? 'active' : (actualMotion > 0.1 ? 'present_still' : 'present_still'), presence: true, confidence: 0.8 },
      signal_field: { grid_size: [20, 1, 20], values: this._presenceField(10, 10, 2.5 + actualMotion * 1.5, t) },
      vital_signs: {
        breathing_rate_bpm: breathRate,
        heart_rate_bpm: hrRate,
        breathing_confidence: 0.7,
        heart_rate_confidence: 0.65,
        hr_zone: hrRate > 155 ? 'anaerobic' : (hrRate > 130 ? 'threshold' : (hrRate > 110 ? 'aerobic' : 'warmup')),
      },
      persons: [{
        id: 'p0',
        position: [0 + n.pos1(t) * 0.05, verticalPos, 0 + n.pos2(t) * 0.05],
        motion_score: ms,
        pose: exerciseType === 'rest' || exerciseType === 'cooldown' ? 'standing' : 'exercising',
        facing: 0,
        exerciseType,
        exercisePhase: exerciseType === 'jumping_jacks' ? 'high_cadence' : (exerciseType === 'squats' ? 'controlled' : 'recovery'),
      }],
      estimated_persons: 1,
      edge_modules: {
        breathing_sync: { cadence: breathRate.toFixed(1), sync_quality: actualMotion > 0.5 ? 0.7 : 0.85 },
        gesture: { type: exerciseType !== 'rest' && exerciseType !== 'cooldown' ? 'exercise_rep' : 'none', count: repCount },
        vital_trend: {
          status: hrRate > 170 ? 'warning' : (hrRate > 140 ? 'elevated' : 'normal'),
          hr_zone: hrRate > 155 ? 'anaerobic' : (hrRate > 130 ? 'threshold' : (hrRate > 110 ? 'aerobic' : (hrRate > 90 ? 'warmup' : 'resting'))),
          hr_lag_s: exerciseType === 'rest' ? 'recovering' : 'tracking',
          intensity: (targetIntensity * 100).toFixed(0) + '%',
          workout_phase: exerciseType,
        },
      },
    });
  }

  // ========================================================================
  // 12. Security Patrol — checkpoint pauses, speed variation, anomaly buildup
  // ========================================================================

  _securityPatrol(t) {
    const n = _getNoiseBank('security_patrol');

    // Patrol route: rectangular with checkpoint pauses at corners
    const patrolSpeed = 0.18; // slightly slower for realism
    const rawPatrolT = (t * patrolSpeed) % 1; // 0..1 around route

    // Checkpoint pauses: guard slows/pauses at each corner (0.25, 0.5, 0.75, 1.0)
    // Remap rawPatrolT to account for pauses
    const cornerDuration = 0.04; // proportion of circuit spent pausing at each corner
    let patrolT = rawPatrolT;
    let atCheckpoint = false;
    let checkpointCorner = -1;
    const corners = [0, 0.25, 0.5, 0.75];
    for (let ci = 0; ci < 4; ci++) {
      const c = corners[ci];
      const next = ci < 3 ? corners[ci + 1] : 1;
      if (rawPatrolT >= c && rawPatrolT < c + cornerDuration) {
        patrolT = c;
        atCheckpoint = true;
        checkpointCorner = ci;
        break;
      }
    }

    // Speed variation: faster on long stretches, slower near corners
    let px, pz, facing;
    if (patrolT < 0.25) {
      const p = patrolT / 0.25;
      px = -3 + p * 6; pz = -2; facing = 0;
    } else if (patrolT < 0.5) {
      const p = (patrolT - 0.25) / 0.25;
      px = 3; pz = -2 + p * 4; facing = Math.PI * 0.5;
    } else if (patrolT < 0.75) {
      const p = (patrolT - 0.5) / 0.25;
      px = 3 - p * 6; pz = 2; facing = Math.PI;
    } else {
      const p = (patrolT - 0.75) / 0.25;
      px = -3; pz = 2 - p * 4; facing = Math.PI * 1.5;
    }

    // At checkpoint: guard looks around (facing oscillates)
    if (atCheckpoint) {
      facing += Math.sin(t * 3) * 0.8; // scanning left-right
    }

    // Add natural movement noise
    px += n.pos1(t) * 0.05;
    pz += n.pos2(t) * 0.05;

    const guardSpeed = atCheckpoint ? 5 : (80 + _harmonicNoise(t, 10.1, 2) * 20);
    const zone = px > 0 ? (pz > 0 ? 'NE' : 'SE') : (pz > 0 ? 'NW' : 'SW');

    // Anomaly: starts as faint signal, builds confidence, guard responds
    const anomalyCycle = t % 25;
    const anomalyFaint = anomalyCycle >= 14 && anomalyCycle < 17; // first hints
    const anomalyBuilding = anomalyCycle >= 17 && anomalyCycle < 19; // confidence builds
    const anomalyConfirmed = anomalyCycle >= 19 && anomalyCycle < 22; // confirmed, guard responds
    const anomalyActive = anomalyFaint || anomalyBuilding || anomalyConfirmed;

    let anomalyScore = 0;
    if (anomalyFaint) anomalyScore = 0.15 + _smoothstep(14, 17, anomalyCycle) * 0.2;
    else if (anomalyBuilding) anomalyScore = 0.35 + _smoothstep(17, 19, anomalyCycle) * 0.35;
    else if (anomalyConfirmed) anomalyScore = 0.7 + _smoothstep(19, 20, anomalyCycle) * 0.15;

    // Anomaly position (opposite quadrant)
    const ax = -px * 0.5 + Math.sin(t * 0.3) * 0.3;
    const az = -pz * 0.4 + Math.cos(t * 0.25) * 0.2;

    // Guard changes path toward anomaly when confirmed
    if (anomalyConfirmed) {
      const redirectStrength = _smoothstep(19, 20, anomalyCycle);
      px = _lerp(px, ax, redirectStrength * 0.4);
      pz = _lerp(pz, az, redirectStrength * 0.4);
      facing = Math.atan2(ax - px, az - pz);
    }

    const amplitude = new Float32Array(64);
    for (let i = 0; i < 64; i++) {
      amplitude[i] = 0.3 + 0.15 * Math.sin(t * 1.0 + i * 0.15)
        + _harmonicNoise(t, i * 0.13, 2) * 0.01;
    }

    const persons = [{
      id: 'guard',
      position: [px, 0, pz],
      motion_score: guardSpeed,
      pose: atCheckpoint ? 'standing' : (anomalyConfirmed ? 'alert' : 'walking'),
      facing,
    }];
    if (anomalyActive) {
      persons.push({
        id: 'anomaly',
        position: [ax, 0, az],
        motion_score: anomalyFaint ? 8 : (anomalyBuilding ? 15 : 25),
        pose: 'crouching',
        facing: Math.atan2(px - ax, pz - az),
        signal_confidence: anomalyScore,
      });
    }

    return this._baseFrame({
      nodes: [
        { node_id: 1, rssi_dbm: -40 + Math.sin(t * 0.8) * 3 + n.rssi(t) * 0.5, position: [4, 2, -4], amplitude, subcarrier_count: 64 },
        { node_id: 2, rssi_dbm: -42 + Math.sin(t * 0.6) * 2 + n.env(t) * 0.3, position: [-4, 2, 4], amplitude: new Float32Array(amplitude), subcarrier_count: 64 },
      ],
      features: {
        mean_rssi: -40 + Math.sin(t * 0.8) * 3 + n.rssi(t) * 0.5,
        variance: 2.5 + Math.sin(t * 0.5) * 0.8 + (anomalyActive ? anomalyScore * 2 : 0),
        std: 1.58 + anomalyScore * 0.5,
        motion_band_power: atCheckpoint ? 0.03 : 0.18,
        breathing_band_power: 0.06,
        dominant_freq_hz: atCheckpoint ? 0.1 : 0.8,
        spectral_power: 0.3 + anomalyScore * 0.2,
      },
      classification: {
        motion_level: atCheckpoint ? 'present_still' : 'active',
        presence: true,
        confidence: 0.85,
        anomaly_zone: anomalyActive,
        anomaly_confidence: anomalyScore,
      },
      signal_field: {
        grid_size: [20, 1, 20],
        values: anomalyActive
          ? this._twoPresenceField(10 + px * 1.5, 10 + pz * 1.5, 10 + ax * 1.5, 10 + az * 1.5, t)
          : this._presenceField(10 + px * 1.5, 10 + pz * 1.5, 2.5, t),
      },
      vital_signs: {
        breathing_rate_bpm: 16 + n.breath(t) * 0.5,
        heart_rate_bpm: 78 + (anomalyConfirmed ? 12 : 0) + n.hr(t) * 1,
        breathing_confidence: 0.6,
        heart_rate_confidence: 0.5,
      },
      persons,
      estimated_persons: anomalyActive ? 2 : 1,
      edge_modules: {
        behavioral_profiler: {
          guard_zone: zone,
          coverage_pct: 72 + _harmonicNoise(t, 11.1, 2) * 3,
          anomaly_score: anomalyScore,
          checkpoint_active: atCheckpoint,
          checkpoint_corner: checkpointCorner >= 0 ? ['SW', 'SE', 'NE', 'NW'][checkpointCorner] : null,
          guard_response: anomalyConfirmed ? 'investigating' : (anomalyBuilding ? 'alerted' : 'patrolling'),
        },
        perimeter_breach: {
          detected: anomalyScore > 0.5,
          confidence: anomalyScore > 0.5 ? anomalyScore : 0,
          zone: anomalyActive ? (zone === 'NE' ? 'SW' : 'NE') : 'none',
          first_detected_s: anomalyFaint ? (anomalyCycle - 14).toFixed(1) : null,
          buildup_phase: anomalyFaint ? 'faint' : (anomalyBuilding ? 'building' : (anomalyConfirmed ? 'confirmed' : 'none')),
        },
      },
    });
  }

  // ---- Helpers ----

  _flatField(base) {
    const vals = [];
    // Spatially coherent noise: smooth gradient + gentle ripple
    for (let iz = 0; iz < 20; iz++) {
      for (let ix = 0; ix < 20; ix++) {
        const gradient = Math.sin(ix * 0.3) * Math.sin(iz * 0.25) * 0.01;
        const ripple = _harmonicNoise(ix * 0.5 + iz * 0.7, ix + iz * 20, 2) * 0.005;
        vals.push(_clamp(base + gradient + ripple, 0, 1));
      }
    }
    return vals;
  }

  _presenceField(cx, cz, radius, t) {
    const vals = [];
    for (let iz = 0; iz < 20; iz++) {
      for (let ix = 0; ix < 20; ix++) {
        const dx = ix - cx, dz = iz - cz;
        const d = Math.sqrt(dx * dx + dz * dz);
        // Spatially coherent noise (smooth, not random per cell)
        const noise = _harmonicNoise(t * 0.5 + ix * 0.4 + iz * 0.3, ix + iz * 20, 2) * 0.015;
        const v = _gaussian(d, radius) * 0.7 + noise;
        vals.push(_clamp(v, 0, 1));
      }
    }
    return vals;
  }

  _twoPresenceField(x1, z1, x2, z2, t) {
    const vals = [];
    for (let iz = 0; iz < 20; iz++) {
      for (let ix = 0; ix < 20; ix++) {
        const d1 = Math.sqrt((ix - x1) ** 2 + (iz - z1) ** 2);
        const d2 = Math.sqrt((ix - x2) ** 2 + (iz - z2) ** 2);
        const v1 = _gaussian(d1, 1.7) * 0.6;
        const v2 = _gaussian(d2, 1.7) * 0.55;
        const noise = _harmonicNoise(t * 0.5 + ix * 0.4 + iz * 0.3, ix + iz * 20, 2) * 0.012;
        vals.push(_clamp(v1 + v2 + noise, 0, 1));
      }
    }
    return vals;
  }

  /** Search & rescue field with scanning sweep and gradual target lock */
  _searchRescueField(t, scanning, detected, confidence) {
    const vals = [];
    const targetCx = 14, targetCz = 10;
    for (let iz = 0; iz < 20; iz++) {
      for (let ix = 0; ix < 20; ix++) {
        let v = 0;
        // Scan sweep (rotating beam)
        if (scanning) {
          const scanAngle = t * 0.8;
          const cellAngle = Math.atan2(iz - 10, ix - 10);
          const angleDiff = Math.abs(((cellAngle - scanAngle + Math.PI) % (2 * Math.PI)) - Math.PI);
          v += _gaussian(angleDiff, 0.5) * 0.15;
        }
        // Target presence (gradually intensifying)
        if (detected) {
          const d = Math.sqrt((ix - targetCx) ** 2 + (iz - targetCz) ** 2);
          v += _gaussian(d, 3.5 - confidence * 2) * confidence * 0.7;
        }
        // Background noise
        v += _harmonicNoise(t * 0.3 + ix * 0.5 + iz * 0.6, ix + iz * 20, 2) * 0.01;
        vals.push(_clamp(v, 0, 1));
      }
    }
    return vals;
  }

  _generateIQ(count, scale, t) {
    const iq = [];
    for (let i = 0; i < count; i++) {
      const phase = t * 0.5 + i * 0.2 + Math.sin(t * 0.3 + i * 0.1) * 0.5;
      const amp = scale * (0.5 + 0.5 * Math.sin(t * 0.2 + i * 0.15));
      iq.push({ i: amp * Math.cos(phase), q: amp * Math.sin(phase) });
    }
    return iq;
  }

  _generateVariance(count, scale, t) {
    const v = new Float32Array(count);
    for (let i = 0; i < count; i++) v[i] = scale * (0.3 + 0.7 * Math.abs(Math.sin(t * 0.4 + i * 0.25)));
    return v;
  }

  _blend(a, b, alpha) {
    const beta = 1 - alpha;
    const result = JSON.parse(JSON.stringify(b));

    if (a.features && b.features) {
      for (const key of Object.keys(b.features)) {
        if (typeof b.features[key] === 'number' && typeof a.features[key] === 'number')
          result.features[key] = a.features[key] * beta + b.features[key] * alpha;
      }
    }
    if (a.signal_field?.values && b.signal_field?.values) {
      const len = Math.min(a.signal_field.values.length, b.signal_field.values.length);
      for (let i = 0; i < len; i++) result.signal_field.values[i] = a.signal_field.values[i] * beta + b.signal_field.values[i] * alpha;
    }
    if (a.vital_signs && b.vital_signs) {
      for (const key of Object.keys(b.vital_signs)) {
        if (typeof b.vital_signs[key] === 'number' && typeof a.vital_signs[key] === 'number')
          result.vital_signs[key] = a.vital_signs[key] * beta + b.vital_signs[key] * alpha;
      }
    }
    if (a.nodes?.[0]?.amplitude && b.nodes?.[0]?.amplitude) {
      const ampA = a.nodes[0].amplitude, ampB = b.nodes[0].amplitude;
      const len = Math.min(ampA.length, ampB.length);
      const blended = new Float32Array(len);
      for (let i = 0; i < len; i++) blended[i] = ampA[i] * beta + ampB[i] * alpha;
      result.nodes[0].amplitude = blended;
    }
    return result;
  }
}
