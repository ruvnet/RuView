// RuView Web App - Interactive Explorer & Learning Platform
// Self-contained SPA with mock data for safe exploration

(function () {
  'use strict';

  // ─── Data ───────────────────────────────────────────────

  const CRATES = [
    { name: 'wifi-densepose-core', tag: 'core', color: 'blue', desc: 'Core types, traits, error types, CSI frame primitives', deps: [], order: 1 },
    { name: 'wifi-densepose-signal', tag: 'processing', color: 'cyan', desc: 'SOTA signal processing + RuvSense multistatic sensing (14 modules)', deps: ['core'], order: 7 },
    { name: 'wifi-densepose-nn', tag: 'ml', color: 'purple', desc: 'Neural network inference (ONNX, PyTorch, Candle backends)', deps: [], order: 8 },
    { name: 'wifi-densepose-train', tag: 'ml', color: 'purple', desc: 'Training pipeline with ruvector integration + ruview_metrics', deps: ['signal', 'nn'], order: 10 },
    { name: 'wifi-densepose-mat', tag: 'application', color: 'orange', desc: 'Mass Casualty Assessment Tool — disaster survivor detection via WiFi', deps: ['core', 'signal', 'nn'], order: 11 },
    { name: 'wifi-densepose-hardware', tag: 'hardware', color: 'yellow', desc: 'ESP32 aggregator, TDM protocol, channel hopping firmware', deps: [], order: 4 },
    { name: 'wifi-densepose-ruvector', tag: 'ml', color: 'purple', desc: 'RuVector v2.0.4 integration + cross-viewpoint fusion (5 modules)', deps: [], order: 9 },
    { name: 'wifi-densepose-api', tag: 'api', color: 'green', desc: 'REST API (Axum) — HTTP endpoints for pose data', deps: [], order: 12 },
    { name: 'wifi-densepose-db', tag: 'infra', color: 'blue', desc: 'Database layer (Postgres, SQLite, Redis)', deps: [], order: 6 },
    { name: 'wifi-densepose-config', tag: 'infra', color: 'blue', desc: 'Configuration management', deps: [], order: 5 },
    { name: 'wifi-densepose-wasm', tag: 'web', color: 'cyan', desc: 'WebAssembly bindings for browser deployment', deps: ['mat'], order: 13 },
    { name: 'wifi-densepose-cli', tag: 'tool', color: 'green', desc: 'CLI tool (wifi-densepose binary)', deps: ['mat'], order: 15 },
    { name: 'wifi-densepose-sensing-server', tag: 'api', color: 'green', desc: 'Lightweight Axum server for WiFi sensing UI', deps: ['wifiscan'], order: 14 },
    { name: 'wifi-densepose-wifiscan', tag: 'hardware', color: 'yellow', desc: 'Multi-BSSID WiFi scanning (ADR-022)', deps: [], order: 3 },
    { name: 'wifi-densepose-vitals', tag: 'hardware', color: 'yellow', desc: 'ESP32 CSI-grade vital sign extraction (ADR-021)', deps: [], order: 2 },
  ];

  const RUVSENSE_MODULES = [
    { name: 'multiband', desc: 'Multi-band CSI frame fusion, cross-channel coherence' },
    { name: 'phase_align', desc: 'Iterative LO phase offset estimation, circular mean' },
    { name: 'multistatic', desc: 'Attention-weighted fusion, geometric diversity' },
    { name: 'coherence', desc: 'Z-score coherence scoring, DriftProfile' },
    { name: 'coherence_gate', desc: 'Accept/PredictOnly/Reject/Recalibrate gate decisions' },
    { name: 'pose_tracker', desc: '17-keypoint Kalman tracker with AETHER re-ID embeddings' },
    { name: 'field_model', desc: 'SVD room eigenstructure, perturbation extraction' },
    { name: 'tomography', desc: 'RF tomography, ISTA L1 solver, voxel grid' },
    { name: 'longitudinal', desc: 'Welford stats, biomechanics drift detection' },
    { name: 'intention', desc: 'Pre-movement lead signals (200-500ms)' },
    { name: 'cross_room', desc: 'Environment fingerprinting, transition graph' },
    { name: 'gesture', desc: 'DTW template matching gesture classifier' },
    { name: 'adversarial', desc: 'Physically impossible signal detection, multi-link consistency' },
  ];

  const KEYPOINT_NAMES = [
    'nose', 'left_eye', 'right_eye', 'left_ear', 'right_ear',
    'left_shoulder', 'right_shoulder', 'left_elbow', 'right_elbow',
    'left_wrist', 'right_wrist', 'left_hip', 'right_hip',
    'left_knee', 'right_knee', 'left_ankle', 'right_ankle'
  ];

  const SKELETON_EDGES = [
    [0,1],[0,2],[1,3],[2,4],[5,6],[5,7],[7,9],[6,8],[8,10],
    [5,11],[6,12],[11,12],[11,13],[13,15],[12,14],[14,16]
  ];

  const API_ENDPOINTS = [
    { method: 'GET', path: '/', desc: 'Root — API info and version' },
    { method: 'GET', path: '/health/health', desc: 'System health with component status' },
    { method: 'GET', path: '/health/ready', desc: 'Readiness probe' },
    { method: 'GET', path: '/health/live', desc: 'Liveness probe' },
    { method: 'GET', path: '/api/v1/info', desc: 'Detailed API information' },
    { method: 'GET', path: '/api/v1/status', desc: 'Service & streaming status' },
    { method: 'GET', path: '/api/v1/pose/current', desc: 'Current pose detections' },
    { method: 'GET', path: '/api/v1/pose/zones/summary', desc: 'Zone occupancy summary' },
    { method: 'GET', path: '/api/v1/pose/stats', desc: 'Detection statistics' },
    { method: 'GET', path: '/api/v1/stream/status', desc: 'WebSocket stream status' },
    { method: 'POST', path: '/api/v1/stream/start', desc: 'Start pose streaming' },
    { method: 'POST', path: '/api/v1/stream/stop', desc: 'Stop pose streaming' },
    { method: 'WS', path: '/api/v1/stream/pose', desc: 'Real-time pose WebSocket' },
    { method: 'WS', path: '/api/v1/stream/events', desc: 'System events WebSocket' },
  ];

  // ─── Mock Data Generators ──────────────────────────────

  function generateKeypoints(cx, cy) {
    const offsets = [
      [0,-70],[- 8,-80],[8,-80],[-16,-75],[16,-75],
      [-35,-35],[35,-35],[-50,10],[50,10],[-55,50],[55,50],
      [-18,50],[18,50],[-22,105],[22,105],[-22,160],[22,160]
    ];
    return offsets.map(([dx,dy], i) => ({
      x: cx + dx + (Math.random()-0.5)*8,
      y: cy + dy + (Math.random()-0.5)*8,
      confidence: 0.7 + Math.random()*0.28,
      name: KEYPOINT_NAMES[i]
    }));
  }

  function generateCSIFrame(numSubcarriers) {
    const frame = [];
    const t = Date.now() / 1000;
    for (let i = 0; i < numSubcarriers; i++) {
      const freq = 2.412 + i * 0.005;
      const amp = 0.3 + 0.4 * Math.sin(t * 2 + i * 0.3) + Math.random() * 0.15;
      const phase = Math.sin(t * 1.5 + i * 0.5) * Math.PI + Math.random() * 0.3;
      frame.push({ subcarrier: i, amplitude: amp, phase: phase, frequency: freq });
    }
    return frame;
  }

  // ─── Page Renderers ─────────────────────────────────────

  const pages = {};

  // Overview
  pages.overview = function () {
    return `
      <div class="page">
        <h2 class="page-title">WiFi DensePose Explorer</h2>
        <p class="page-subtitle">Human pose estimation through walls using WiFi signals — explore, learn, and build.</p>

        <div class="grid grid-4" style="margin-bottom:24px">
          <div class="stat-card">
            <div class="stat-value">15</div>
            <div class="stat-label">Rust Crates</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">1,031+</div>
            <div class="stat-label">Tests</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">43</div>
            <div class="stat-label">ADRs</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">14</div>
            <div class="stat-label">RuvSense Modules</div>
          </div>
        </div>

        <div class="grid grid-2">
          <div class="card">
            <div class="card-title">What is WiFi DensePose?</div>
            <div class="card-body">
              AI that tracks full-body human movement <strong>through walls</strong> using just WiFi signals.
              No cameras needed — CSI (Channel State Information) from standard WiFi routers is fed through
              a neural network to produce 17-keypoint pose wireframes in real-time at 100Hz.
              <br><br>
              Based on Carnegie Mellon University research, this project implements the full pipeline:
              signal capture, phase sanitization, modality translation, and DensePose-RCNN inference.
            </div>
          </div>
          <div class="card">
            <div class="card-title">This Web App</div>
            <div class="card-body">
              This is an interactive explorer running entirely in your browser with <strong>mock data</strong> —
              no hardware or backend needed. You can:
              <ul style="margin:8px 0 0 16px;list-style:disc">
                <li>Browse all 15 Rust crates and their relationships</li>
                <li>Watch live pose detection demos with synthetic data</li>
                <li>Experiment with CSI signal visualization</li>
                <li>Try the API playground (mock responses)</li>
                <li>Learn how to build, test, and deploy</li>
              </ul>
            </div>
          </div>
        </div>

        <div class="card" style="margin-top:8px">
          <div class="card-title">How It Works</div>
          <div class="flow-container">
            <div class="flow-step"><div class="flow-step-num">Step 1</div><div class="flow-step-label">WiFi Antenna Array</div></div>
            <div class="flow-arrow">&rarr;</div>
            <div class="flow-step"><div class="flow-step-num">Step 2</div><div class="flow-step-label">CSI Extraction</div></div>
            <div class="flow-arrow">&rarr;</div>
            <div class="flow-step"><div class="flow-step-num">Step 3</div><div class="flow-step-label">Phase Sanitization</div></div>
            <div class="flow-arrow">&rarr;</div>
            <div class="flow-step"><div class="flow-step-num">Step 4</div><div class="flow-step-label">CNN Translation</div></div>
            <div class="flow-arrow">&rarr;</div>
            <div class="flow-step"><div class="flow-step-num">Step 5</div><div class="flow-step-label">DensePose RCNN</div></div>
            <div class="flow-arrow">&rarr;</div>
            <div class="flow-step"><div class="flow-step-num">Output</div><div class="flow-step-label">Pose Wireframe</div></div>
          </div>
        </div>

        <div class="grid grid-3" style="margin-top:8px">
          <div class="card">
            <div class="card-title"><span class="tag tag-green">Advantage</span> Through Walls</div>
            <div class="card-body">Works through solid barriers — no line of sight required. WiFi signals penetrate walls, furniture, and obstructions.</div>
          </div>
          <div class="card">
            <div class="card-title"><span class="tag tag-blue">Advantage</span> Privacy-Preserving</div>
            <div class="card-body">No cameras or visual recording. Only WiFi signal amplitude and phase are analyzed — no identifiable imagery.</div>
          </div>
          <div class="card">
            <div class="card-title"><span class="tag tag-purple">Advantage</span> Low Cost</div>
            <div class="card-body">Built on $30 commercial ESP32 hardware. Uses existing WiFi infrastructure — no specialized equipment needed.</div>
          </div>
        </div>
      </div>`;
  };

  // Architecture
  pages.architecture = function () {
    return `
      <div class="page">
        <h2 class="page-title">Architecture</h2>
        <p class="page-subtitle">Dual codebase — Python v1 (research) and Rust port (production). 43 Architecture Decision Records.</p>

        <div class="card">
          <div class="card-title">System Layers</div>
          <div class="card-body">
            <table class="data-table">
              <thead><tr><th>Layer</th><th>Crates / Modules</th><th>Purpose</th></tr></thead>
              <tbody>
                <tr><td><span class="tag tag-yellow">Hardware</span></td><td>hardware, vitals, wifiscan</td><td>ESP32 CSI capture, TDM protocol, channel hopping, vital signs</td></tr>
                <tr><td><span class="tag tag-blue">Core</span></td><td>core, config, db</td><td>Shared types, configuration, database adapters</td></tr>
                <tr><td><span class="tag tag-cyan">Signal</span></td><td>signal (14 RuvSense modules)</td><td>Phase alignment, coherence, tomography, gesture recognition</td></tr>
                <tr><td><span class="tag tag-purple">ML</span></td><td>nn, train, ruvector</td><td>Neural inference, training pipeline, cross-viewpoint fusion</td></tr>
                <tr><td><span class="tag tag-orange">Application</span></td><td>mat, cli, wasm</td><td>Mass casualty tool, CLI binary, browser WASM</td></tr>
                <tr><td><span class="tag tag-green">API</span></td><td>api, sensing-server</td><td>REST endpoints, WebSocket streams, sensing UI server</td></tr>
              </tbody>
            </table>
          </div>
        </div>

        <div class="card">
          <div class="card-title">RuvSense Signal Processing Modules</div>
          <div class="card-body">
            <div class="grid grid-3">
              ${RUVSENSE_MODULES.map(m => `
                <div style="padding:8px 0;border-bottom:1px solid var(--border)">
                  <div style="font-weight:600;font-size:13px;color:var(--cyan)">${m.name}.rs</div>
                  <div style="font-size:12px;color:var(--text-muted);margin-top:2px">${m.desc}</div>
                </div>
              `).join('')}
            </div>
          </div>
        </div>

        <div class="card">
          <div class="card-title">Key Architecture Decision Records</div>
          <div class="card-body">
            ${[
              { num: '014', title: 'SOTA signal processing', status: 'Accepted' },
              { num: '015', title: 'MM-Fi + Wi-Pose training datasets', status: 'Accepted' },
              { num: '016', title: 'RuVector training pipeline integration', status: 'Accepted' },
              { num: '021', title: 'ESP32 CSI-grade vital sign extraction', status: 'Accepted' },
              { num: '022', title: 'Multi-BSSID WiFi scanning', status: 'Accepted' },
              { num: '024', title: 'Contrastive CSI embedding / AETHER', status: 'Accepted' },
              { num: '027', title: 'Cross-environment domain generalization / MERIDIAN', status: 'Accepted' },
              { num: '028', title: 'ESP32 capability audit + witness verification', status: 'Accepted' },
              { num: '029', title: 'RuvSense multistatic sensing mode', status: 'Proposed' },
              { num: '031', title: 'RuView sensing-first RF mode', status: 'Proposed' },
            ].map(a => `
              <div class="adr-item">
                <span class="adr-num">ADR-${a.num}</span>
                <span class="adr-title">${a.title}</span>
                <span class="tag ${a.status === 'Accepted' ? 'tag-green' : 'tag-yellow'}">${a.status}</span>
              </div>
            `).join('')}
          </div>
        </div>
      </div>`;
  };

  // Crate Explorer
  pages.crates = function () {
    const sorted = [...CRATES].sort((a, b) => a.order - b.order);
    return `
      <div class="page">
        <h2 class="page-title">Crate Explorer</h2>
        <p class="page-subtitle">All 15 Rust workspace crates — click to expand details. Ordered by publish dependency.</p>
        <input class="search-box" id="crateSearch" placeholder="Search crates..." />
        <div id="crateList">
          ${sorted.map((c, i) => `
            <div class="card crate-card" data-crate="${c.name}" style="cursor:pointer">
              <div class="card-title">
                <span style="color:var(--text-muted);font-size:12px;min-width:24px">#${c.order}</span>
                <span class="tag tag-${c.color}">${c.tag}</span>
                ${c.name}
              </div>
              <div class="card-body">${c.desc}</div>
              <div class="crate-extra" id="extra-${i}" style="display:none;margin-top:12px;padding-top:12px;border-top:1px solid var(--border)">
                <div class="metric-row">
                  <span class="metric-label">Dependencies</span>
                  <span class="metric-value">${c.deps.length === 0 ? 'None (leaf crate)' : c.deps.join(', ')}</span>
                </div>
                <div class="metric-row">
                  <span class="metric-label">Path</span>
                  <span class="metric-value" style="font-family:monospace;font-size:12px">rust-port/wifi-densepose-rs/crates/${c.name}/</span>
                </div>
                <div class="metric-row">
                  <span class="metric-label">Publish Order</span>
                  <span class="metric-value">${c.order} of 15</span>
                </div>
                <div style="margin-top:8px">
                  <div class="code-block"><span class="code-label">Cargo.toml</span><span class="cm"># Add to your Cargo.toml</span>
[dependencies]
<span class="fn">${c.name}</span> = <span class="str">"*"</span></div>
                </div>
              </div>
            </div>
          `).join('')}
        </div>
      </div>`;
  };

  // Live Demo
  pages.demo = function () {
    return `
      <div class="page">
        <h2 class="page-title">Live Pose Detection Demo</h2>
        <p class="page-subtitle">Synthetic mock data — no hardware required. Watch real-time pose wireframes generated from simulated CSI.</p>

        <div class="btn-group" style="margin-bottom:16px">
          <button class="btn btn-primary" id="demoStart">Start Stream</button>
          <button class="btn" id="demoStop" disabled>Stop Stream</button>
          <span id="demoStatus" style="padding:8px;font-size:13px;color:var(--text-muted)">Ready</span>
        </div>

        <div class="grid grid-2">
          <div class="card">
            <div class="card-title">Pose Detection</div>
            <div class="canvas-container">
              <canvas id="poseCanvas" width="500" height="400"></canvas>
            </div>
            <div style="margin-top:12px">
              <div class="metric-row">
                <span class="metric-label">Persons Detected</span>
                <span class="metric-value" id="personCount">0</span>
              </div>
              <div class="metric-row">
                <span class="metric-label">Avg Confidence</span>
                <span class="metric-value" id="avgConf">0%</span>
              </div>
              <div class="metric-row">
                <span class="metric-label">Frame</span>
                <span class="metric-value" id="frameCount">0</span>
              </div>
            </div>
          </div>
          <div class="card">
            <div class="card-title">CSI Signal (30 subcarriers)</div>
            <div class="canvas-container">
              <canvas id="csiCanvas" width="500" height="180"></canvas>
            </div>
            <div class="card-title" style="margin-top:16px">Signal Metrics</div>
            <div class="metric-row">
              <span class="metric-label">Signal Strength</span>
              <span class="metric-value" id="sigStrength">-45 dBm</span>
            </div>
            <div class="metric-row">
              <span class="metric-label">Processing Latency</span>
              <span class="metric-value" id="latency">0 ms</span>
            </div>
            <div class="metric-row">
              <span class="metric-label">Subcarriers Active</span>
              <span class="metric-value">30 / 30</span>
            </div>
          </div>
        </div>
      </div>`;
  };

  // Signal Lab
  pages.signals = function () {
    return `
      <div class="page">
        <h2 class="page-title">Signal Lab</h2>
        <p class="page-subtitle">Interactive CSI signal exploration — adjust parameters and see how signals change in real-time.</p>

        <div class="grid grid-2">
          <div class="card">
            <div class="card-title">CSI Amplitude Heatmap</div>
            <div class="canvas-container">
              <canvas id="heatmapCanvas" width="500" height="200"></canvas>
            </div>
            <div style="margin-top:12px;font-size:13px;color:var(--text-secondary)">
              X-axis: subcarrier index (0-55) | Y-axis: time (last 50 frames) | Color: amplitude
            </div>
          </div>
          <div class="card">
            <div class="card-title">Phase Distribution</div>
            <div class="canvas-container">
              <canvas id="phaseCanvas" width="500" height="200"></canvas>
            </div>
            <div style="margin-top:12px;font-size:13px;color:var(--text-secondary)">
              Phase values across subcarriers — note the sanitization effect on raw phase noise
            </div>
          </div>
        </div>

        <div class="card">
          <div class="card-title">Signal Parameters</div>
          <div class="grid grid-3">
            <div>
              <label style="font-size:12px;color:var(--text-muted)">Subcarriers</label>
              <div class="metric-row">
                <input type="range" id="sigSubcarriers" min="16" max="114" value="56" style="flex:1">
                <span id="sigSubVal" style="min-width:30px;text-align:right;font-family:monospace">56</span>
              </div>
            </div>
            <div>
              <label style="font-size:12px;color:var(--text-muted)">Noise Level</label>
              <div class="metric-row">
                <input type="range" id="sigNoise" min="0" max="100" value="20" style="flex:1">
                <span id="sigNoiseVal" style="min-width:30px;text-align:right;font-family:monospace">20%</span>
              </div>
            </div>
            <div>
              <label style="font-size:12px;color:var(--text-muted)">Movement Speed</label>
              <div class="metric-row">
                <input type="range" id="sigSpeed" min="1" max="10" value="3" style="flex:1">
                <span id="sigSpeedVal" style="min-width:30px;text-align:right;font-family:monospace">3</span>
              </div>
            </div>
          </div>
          <div class="btn-group" style="margin-top:12px">
            <button class="btn btn-primary" id="sigStart">Start Simulation</button>
            <button class="btn" id="sigStop" disabled>Stop</button>
            <button class="btn" id="sigReset">Reset</button>
          </div>
        </div>
      </div>`;
  };

  // API Playground
  pages.api = function () {
    return `
      <div class="page">
        <h2 class="page-title">API Playground</h2>
        <p class="page-subtitle">Try API endpoints with mock responses. The real API runs on FastAPI (Python) or Axum (Rust).</p>

        <div class="grid grid-2">
          <div class="card" style="grid-row: span 2">
            <div class="card-title">Endpoints</div>
            <div class="card-body">
              ${API_ENDPOINTS.map((ep, i) => `
                <div class="adr-item" style="cursor:pointer" data-ep="${i}" onclick="window.__selectEndpoint(${i})">
                  <span class="tag ${ep.method === 'GET' ? 'tag-green' : ep.method === 'POST' ? 'tag-blue' : 'tag-purple'}" style="min-width:36px;text-align:center">${ep.method}</span>
                  <span style="font-family:monospace;font-size:12px;flex:1">${ep.path}</span>
                </div>
              `).join('')}
            </div>
          </div>
          <div>
            <div class="card">
              <div class="card-title">Request</div>
              <div class="card-body">
                <div class="metric-row">
                  <span class="metric-label">Method</span>
                  <span class="metric-value" id="apiMethod">GET</span>
                </div>
                <div class="metric-row">
                  <span class="metric-label">Path</span>
                  <span class="metric-value" id="apiPath" style="font-family:monospace;font-size:12px">/</span>
                </div>
                <div class="metric-row">
                  <span class="metric-label">Description</span>
                  <span class="metric-value" id="apiDesc" style="font-size:12px">Root endpoint</span>
                </div>
                <button class="btn btn-primary" id="apiSend" style="margin-top:12px;width:100%">Send Request</button>
              </div>
            </div>
            <div class="card">
              <div class="card-title">Response <span id="apiStatus" class="tag tag-green" style="margin-left:8px">200</span></div>
              <div class="code-block" id="apiResponse" style="max-height:300px;overflow-y:auto;white-space:pre-wrap">
<span class="cm">// Click an endpoint and press Send</span></div>
            </div>
          </div>
        </div>
      </div>`;
  };

  // Getting Started
  pages.guide = function () {
    return `
      <div class="page">
        <h2 class="page-title">Getting Started</h2>
        <p class="page-subtitle">Everything you need to run, build, and develop with WiFi DensePose.</p>

        <div class="card">
          <div class="card-title">Prerequisites</div>
          <div class="card-body">
            <table class="data-table">
              <thead><tr><th>Tool</th><th>Version</th><th>Purpose</th></tr></thead>
              <tbody>
                <tr><td>Rust</td><td>1.75+</td><td>Workspace build (15 crates)</td></tr>
                <tr><td>Python</td><td>3.9+</td><td>v1 research code, proof verification</td></tr>
                <tr><td>Node.js</td><td>18+</td><td>UI tooling, WASM builds (optional)</td></tr>
                <tr><td>Docker</td><td>24+</td><td>Containerized deployment (optional)</td></tr>
              </tbody>
            </table>
          </div>
        </div>

        <div class="card">
          <div class="card-title">Quick Start</div>
          <div class="terminal">
            <div class="terminal-bar">
              <span class="terminal-dot r"></span>
              <span class="terminal-dot y"></span>
              <span class="terminal-dot g"></span>
            </div>
            <div class="terminal-body">
<span class="prompt">$</span> <span class="cmd">git clone https://github.com/user/RuView.git && cd RuView</span>
<span class="output"></span>
<span class="cm"># Run the Rust test suite (1,031+ tests)</span>
<span class="prompt">$</span> <span class="cmd">cd rust-port/wifi-densepose-rs</span>
<span class="prompt">$</span> <span class="cmd">cargo test --workspace --no-default-features</span>
<span class="success">test result: ok. 1031 passed; 0 failed; 0 ignored</span>
<span class="output"></span>
<span class="cm"># Verify the deterministic proof</span>
<span class="prompt">$</span> <span class="cmd">cd ../.. && python v1/data/proof/verify.py</span>
<span class="success">VERDICT: PASS</span>
<span class="output"></span>
<span class="cm"># Launch this web explorer</span>
<span class="prompt">$</span> <span class="cmd">cd ui/webapp && bash serve.sh</span>
<span class="info">RuView Explorer running at http://localhost:3001</span>
<span class="output"></span>
<span class="cm"># Launch the full UI + backend</span>
<span class="prompt">$</span> <span class="cmd">cd ui && bash start-ui.sh</span>
<span class="info">WiFi DensePose UI at http://localhost:3000</span>
            </div>
          </div>
        </div>

        <div class="card">
          <div class="card-title">Project Structure</div>
          <div class="code-block"><span class="code-label">tree</span>RuView/
├── rust-port/wifi-densepose-rs/   <span class="cm"># Rust workspace (15 crates)</span>
│   ├── crates/
│   │   ├── wifi-densepose-core/   <span class="cm"># Core types & traits</span>
│   │   ├── wifi-densepose-signal/ <span class="cm"># Signal processing + RuvSense</span>
│   │   ├── wifi-densepose-nn/     <span class="cm"># Neural network inference</span>
│   │   ├── wifi-densepose-mat/    <span class="cm"># Mass Casualty Assessment</span>
│   │   ├── wifi-densepose-wasm/   <span class="cm"># Browser WASM bindings</span>
│   │   └── ...                    <span class="cm"># 10 more crates</span>
│   └── Cargo.toml
├── v1/                            <span class="cm"># Python v1 (research)</span>
│   ├── src/app.py                 <span class="cm"># FastAPI application</span>
│   ├── src/main.py                <span class="cm"># Entry point</span>
│   └── data/proof/verify.py       <span class="cm"># Deterministic proof</span>
├── ui/                            <span class="cm"># Web interfaces</span>
│   ├── webapp/                    <span class="cm"># This explorer app</span>
│   ├── index.html                 <span class="cm"># Main dashboard UI</span>
│   └── components/                <span class="cm"># UI components</span>
├── firmware/esp32-csi-node/       <span class="cm"># ESP32 C firmware</span>
├── docs/adr/                      <span class="cm"># 43 Architecture Decision Records</span>
└── docker/                        <span class="cm"># Docker configs</span></div>
        </div>

        <div class="grid grid-2">
          <div class="card">
            <div class="card-title">Building Apps with the API</div>
            <div class="code-block"><span class="code-label">Python</span><span class="kw">import</span> httpx

<span class="cm"># Get current pose detections</span>
<span class="fn">resp</span> = httpx.get(<span class="str">"http://localhost:8000/api/v1/pose/current"</span>)
data = resp.json()

<span class="kw">for</span> person <span class="kw">in</span> data[<span class="str">"persons"</span>]:
    <span class="fn">print</span>(<span class="str">f"Person {person['person_id']}: "</span>
          <span class="str">f"confidence={person['confidence']:.2f}"</span>)
    <span class="kw">for</span> kp <span class="kw">in</span> person[<span class="str">"keypoints"</span>]:
        <span class="fn">print</span>(<span class="str">f"  {kp['name']}: ({kp['x']:.1f}, {kp['y']:.1f})"</span>)</div>
          </div>
          <div class="card">
            <div class="card-title">Building Apps with Rust</div>
            <div class="code-block"><span class="code-label">Rust</span><span class="kw">use</span> wifi_densepose_core::{CsiFrame, PoseResult};
<span class="kw">use</span> wifi_densepose_signal::ruvsense::coherence::CoherenceScorer;

<span class="cm">// Process a CSI frame</span>
<span class="kw">let</span> frame = CsiFrame::from_raw(&amp;raw_bytes)?;
<span class="kw">let</span> scorer = CoherenceScorer::new(config);
<span class="kw">let</span> score = scorer.evaluate(&amp;frame);

<span class="kw">if</span> score.is_acceptable() {
    <span class="kw">let</span> pose: PoseResult = model.infer(&amp;frame)?;
    <span class="kw">for</span> kp <span class="kw">in</span> &amp;pose.keypoints {
        <span class="fn">println!</span>(<span class="str">"{}: ({:.1}, {:.1})"</span>,
            kp.name, kp.x, kp.y);
    }
}</div>
          </div>
        </div>

        <div class="card">
          <div class="card-title">WebSocket Streaming (Real-time)</div>
          <div class="code-block"><span class="code-label">JavaScript</span><span class="cm">// Connect to real-time pose stream</span>
<span class="kw">const</span> ws = <span class="kw">new</span> <span class="fn">WebSocket</span>(<span class="str">'ws://localhost:8000/api/v1/stream/pose'</span>);

ws.<span class="fn">onmessage</span> = (event) => {
  <span class="kw">const</span> data = JSON.<span class="fn">parse</span>(event.data);
  <span class="kw">if</span> (data.type === <span class="str">'pose_data'</span>) {
    <span class="kw">const</span> persons = data.data.pose.persons;
    <span class="fn">renderPoses</span>(persons);  <span class="cm">// Draw on canvas</span>
  }
};</div>
        </div>
      </div>`;
  };

  // Build & Run
  pages.build = function () {
    return `
      <div class="page">
        <h2 class="page-title">Build & Run</h2>
        <p class="page-subtitle">Commands for building, testing, and deploying the project.</p>

        <div class="tab-bar" id="buildTabs">
          <button class="tab-btn active" data-tab="rust">Rust</button>
          <button class="tab-btn" data-tab="python">Python</button>
          <button class="tab-btn" data-tab="docker">Docker</button>
          <button class="tab-btn" data-tab="verify">Verify</button>
        </div>

        <div class="tab-panel active" data-tab="rust">
          <div class="card">
            <div class="card-title">Rust Workspace</div>
            <div class="terminal">
              <div class="terminal-bar"><span class="terminal-dot r"></span><span class="terminal-dot y"></span><span class="terminal-dot g"></span></div>
              <div class="terminal-body">
<span class="cm"># Build entire workspace (15 crates)</span>
<span class="prompt">$</span> <span class="cmd">cd rust-port/wifi-densepose-rs</span>
<span class="prompt">$</span> <span class="cmd">cargo build --workspace --no-default-features</span>

<span class="cm"># Run all tests (1,031+ tests, ~2 min)</span>
<span class="prompt">$</span> <span class="cmd">cargo test --workspace --no-default-features</span>

<span class="cm"># Check a single crate (fast, no GPU needed)</span>
<span class="prompt">$</span> <span class="cmd">cargo check -p wifi-densepose-signal --no-default-features</span>

<span class="cm"># Run only signal processing tests</span>
<span class="prompt">$</span> <span class="cmd">cargo test -p wifi-densepose-signal --no-default-features</span>

<span class="cm"># Build the CLI binary</span>
<span class="prompt">$</span> <span class="cmd">cargo build -p wifi-densepose-cli --release</span>

<span class="cm"># Build WASM for browser</span>
<span class="prompt">$</span> <span class="cmd">wasm-pack build crates/wifi-densepose-wasm --target web</span>
              </div>
            </div>
          </div>
        </div>

        <div class="tab-panel" data-tab="python">
          <div class="card">
            <div class="card-title">Python v1</div>
            <div class="terminal">
              <div class="terminal-bar"><span class="terminal-dot r"></span><span class="terminal-dot y"></span><span class="terminal-dot g"></span></div>
              <div class="terminal-body">
<span class="cm"># Install dependencies</span>
<span class="prompt">$</span> <span class="cmd">pip install -r requirements.txt</span>

<span class="cm"># Run the FastAPI server</span>
<span class="prompt">$</span> <span class="cmd">cd v1 && python -m src.main</span>
<span class="info">Starting server on 0.0.0.0:8000</span>

<span class="cm"># Run Python tests</span>
<span class="prompt">$</span> <span class="cmd">cd v1 && python -m pytest tests/ -x -q</span>

<span class="cm"># Run deterministic proof verification</span>
<span class="prompt">$</span> <span class="cmd">python v1/data/proof/verify.py</span>
<span class="success">VERDICT: PASS</span>
              </div>
            </div>
          </div>
        </div>

        <div class="tab-panel" data-tab="docker">
          <div class="card">
            <div class="card-title">Docker Deployment</div>
            <div class="terminal">
              <div class="terminal-bar"><span class="terminal-dot r"></span><span class="terminal-dot y"></span><span class="terminal-dot g"></span></div>
              <div class="terminal-body">
<span class="cm"># Build Docker image</span>
<span class="prompt">$</span> <span class="cmd">docker build -t ruview .</span>

<span class="cm"># Run with port mapping</span>
<span class="prompt">$</span> <span class="cmd">docker run -p 8000:8000 -p 3000:3000 ruview</span>

<span class="cm"># Use docker-compose for full stack</span>
<span class="prompt">$</span> <span class="cmd">docker-compose up -d</span>
              </div>
            </div>
          </div>
        </div>

        <div class="tab-panel" data-tab="verify">
          <div class="card">
            <div class="card-title">Witness Verification (ADR-028)</div>
            <div class="terminal">
              <div class="terminal-bar"><span class="terminal-dot r"></span><span class="terminal-dot y"></span><span class="terminal-dot g"></span></div>
              <div class="terminal-body">
<span class="cm"># Full verification procedure</span>
<span class="prompt">$</span> <span class="cmd">cargo test --workspace --no-default-features</span>
<span class="success">test result: ok. 1031 passed; 0 failed</span>

<span class="prompt">$</span> <span class="cmd">python v1/data/proof/verify.py</span>
<span class="success">VERDICT: PASS</span>

<span class="cm"># Generate witness bundle</span>
<span class="prompt">$</span> <span class="cmd">bash scripts/generate-witness-bundle.sh</span>
<span class="info">Bundle: dist/witness-bundle-ADR028-*.tar.gz</span>

<span class="cm"># Self-verify (must be 7/7 PASS)</span>
<span class="prompt">$</span> <span class="cmd">cd dist/witness-bundle-ADR028-*/ && bash VERIFY.sh</span>
<span class="success">7/7 PASS</span>
              </div>
            </div>
          </div>
        </div>

        <div class="card" style="margin-top:16px">
          <div class="card-title">Crate Publish Order</div>
          <div class="card-body">
            <div style="display:flex;flex-wrap:wrap;gap:6px;align-items:center">
              ${[...CRATES].sort((a,b) => a.order - b.order).map((c, i) => `
                <span class="tag tag-${c.color}" style="padding:4px 8px">${c.order}. ${c.name.replace('wifi-densepose-','')}</span>
                ${i < CRATES.length - 1 ? '<span style="color:var(--text-muted)">&rarr;</span>' : ''}
              `).join('')}
            </div>
          </div>
        </div>
      </div>`;
  };

  // ─── App Controller ─────────────────────────────────────

  let currentPage = 'overview';
  let demoInterval = null;
  let signalInterval = null;
  let frameNum = 0;
  let heatmapHistory = [];

  function navigate(page) {
    // Stop running demos
    stopDemo();
    stopSignalLab();

    currentPage = page;

    // Update nav
    document.querySelectorAll('.nav-item').forEach(el => {
      el.classList.toggle('active', el.dataset.page === page);
    });

    // Render page
    const container = document.getElementById('page-container');
    if (pages[page]) {
      container.innerHTML = pages[page]();
      initPageInteractions(page);
    }

    // Close mobile sidebar
    document.getElementById('sidebar').classList.remove('open');
  }

  function initPageInteractions(page) {
    if (page === 'crates') initCratePage();
    if (page === 'demo') initDemoPage();
    if (page === 'signals') initSignalLab();
    if (page === 'api') initApiPage();
    if (page === 'build') initBuildPage();
  }

  // ── Crate page ──
  function initCratePage() {
    const search = document.getElementById('crateSearch');
    search.addEventListener('input', () => {
      const q = search.value.toLowerCase();
      document.querySelectorAll('.crate-card').forEach(el => {
        el.style.display = el.dataset.crate.includes(q) ||
          el.textContent.toLowerCase().includes(q) ? '' : 'none';
      });
    });

    document.querySelectorAll('.crate-card').forEach((el, i) => {
      el.addEventListener('click', () => {
        const extra = document.getElementById('extra-' + i);
        extra.style.display = extra.style.display === 'none' ? 'block' : 'none';
      });
    });
  }

  // ── Demo page ──
  function initDemoPage() {
    document.getElementById('demoStart').addEventListener('click', startDemo);
    document.getElementById('demoStop').addEventListener('click', stopDemo);
  }

  function startDemo() {
    if (demoInterval) return;
    document.getElementById('demoStart').disabled = true;
    document.getElementById('demoStop').disabled = false;
    document.getElementById('demoStatus').textContent = 'Streaming...';
    document.getElementById('demoStatus').style.color = 'var(--green)';
    frameNum = 0;

    demoInterval = setInterval(() => {
      frameNum++;
      const numPersons = Math.random() > 0.3 ? (Math.random() > 0.6 ? 2 : 1) : 0;
      const persons = [];
      for (let i = 0; i < numPersons; i++) {
        const cx = 120 + i * 220 + Math.sin(frameNum * 0.05 + i) * 30;
        const cy = 200 + Math.cos(frameNum * 0.03 + i) * 20;
        persons.push({ keypoints: generateKeypoints(cx, cy), confidence: 0.7 + Math.random() * 0.25 });
      }

      drawPose(persons);
      drawCSI();

      document.getElementById('personCount').textContent = numPersons;
      document.getElementById('avgConf').textContent = numPersons > 0
        ? (persons.reduce((s, p) => s + p.confidence, 0) / numPersons * 100).toFixed(1) + '%'
        : '0%';
      document.getElementById('frameCount').textContent = frameNum;
      document.getElementById('sigStrength').textContent = (-40 - Math.random() * 15).toFixed(0) + ' dBm';
      document.getElementById('latency').textContent = (8 + Math.random() * 12).toFixed(1) + ' ms';
    }, 100);
  }

  function stopDemo() {
    if (!demoInterval) return;
    clearInterval(demoInterval);
    demoInterval = null;
    const startBtn = document.getElementById('demoStart');
    const stopBtn = document.getElementById('demoStop');
    const status = document.getElementById('demoStatus');
    if (startBtn) startBtn.disabled = false;
    if (stopBtn) stopBtn.disabled = true;
    if (status) { status.textContent = 'Stopped'; status.style.color = 'var(--text-muted)'; }
  }

  function drawPose(persons) {
    const canvas = document.getElementById('poseCanvas');
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    ctx.fillStyle = '#0a0e17';
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    // Grid
    ctx.strokeStyle = 'rgba(42,53,72,0.3)';
    ctx.lineWidth = 0.5;
    for (let x = 0; x < canvas.width; x += 40) {
      ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, canvas.height); ctx.stroke();
    }
    for (let y = 0; y < canvas.height; y += 40) {
      ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(canvas.width, y); ctx.stroke();
    }

    persons.forEach((person, pi) => {
      const kps = person.keypoints;
      const hue = pi === 0 ? 210 : 160;

      // Draw skeleton
      ctx.strokeStyle = `hsla(${hue}, 80%, 60%, 0.7)`;
      ctx.lineWidth = 2;
      SKELETON_EDGES.forEach(([a, b]) => {
        ctx.beginPath();
        ctx.moveTo(kps[a].x, kps[a].y);
        ctx.lineTo(kps[b].x, kps[b].y);
        ctx.stroke();
      });

      // Draw keypoints
      kps.forEach(kp => {
        const alpha = kp.confidence;
        ctx.beginPath();
        ctx.arc(kp.x, kp.y, 4, 0, Math.PI * 2);
        ctx.fillStyle = `hsla(${hue}, 90%, 65%, ${alpha})`;
        ctx.fill();
        ctx.strokeStyle = `hsla(${hue}, 90%, 80%, ${alpha})`;
        ctx.lineWidth = 1;
        ctx.stroke();
      });

      // Confidence label
      ctx.fillStyle = `hsla(${hue}, 80%, 70%, 0.9)`;
      ctx.font = '11px monospace';
      ctx.fillText(`P${pi} ${(person.confidence * 100).toFixed(0)}%`, kps[0].x - 15, kps[0].y - 20);
    });

    // Status text
    ctx.fillStyle = 'rgba(100,116,139,0.6)';
    ctx.font = '10px monospace';
    ctx.fillText(`FRAME ${frameNum} | ${persons.length} person(s)`, 8, canvas.height - 8);
  }

  function drawCSI() {
    const canvas = document.getElementById('csiCanvas');
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    const frame = generateCSIFrame(30);

    ctx.fillStyle = '#0a0e17';
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    // Amplitude bars
    const barW = canvas.width / 30 - 2;
    frame.forEach((sc, i) => {
      const h = sc.amplitude * canvas.height * 0.8;
      const x = i * (barW + 2) + 1;
      const gradient = ctx.createLinearGradient(x, canvas.height - h, x, canvas.height);
      gradient.addColorStop(0, 'rgba(6,182,212,0.9)');
      gradient.addColorStop(1, 'rgba(59,130,246,0.3)');
      ctx.fillStyle = gradient;
      ctx.fillRect(x, canvas.height - h, barW, h);
    });

    ctx.fillStyle = 'rgba(100,116,139,0.5)';
    ctx.font = '10px monospace';
    ctx.fillText('CSI AMPLITUDE | 30 SUBCARRIERS', 8, 14);
  }

  // ── Signal Lab ──
  function initSignalLab() {
    heatmapHistory = [];
    const subSlider = document.getElementById('sigSubcarriers');
    const noiseSlider = document.getElementById('sigNoise');
    const speedSlider = document.getElementById('sigSpeed');

    subSlider.addEventListener('input', () => {
      document.getElementById('sigSubVal').textContent = subSlider.value;
    });
    noiseSlider.addEventListener('input', () => {
      document.getElementById('sigNoiseVal').textContent = noiseSlider.value + '%';
    });
    speedSlider.addEventListener('input', () => {
      document.getElementById('sigSpeedVal').textContent = speedSlider.value;
    });

    document.getElementById('sigStart').addEventListener('click', startSignalLab);
    document.getElementById('sigStop').addEventListener('click', stopSignalLab);
    document.getElementById('sigReset').addEventListener('click', () => {
      stopSignalLab();
      heatmapHistory = [];
      const hc = document.getElementById('heatmapCanvas');
      const pc = document.getElementById('phaseCanvas');
      if (hc) hc.getContext('2d').clearRect(0, 0, hc.width, hc.height);
      if (pc) pc.getContext('2d').clearRect(0, 0, pc.width, pc.height);
    });
  }

  function startSignalLab() {
    if (signalInterval) return;
    document.getElementById('sigStart').disabled = true;
    document.getElementById('sigStop').disabled = false;

    signalInterval = setInterval(() => {
      const numSub = parseInt(document.getElementById('sigSubcarriers').value);
      const noise = parseInt(document.getElementById('sigNoise').value) / 100;
      const speed = parseInt(document.getElementById('sigSpeed').value);

      const frame = [];
      const t = Date.now() / 1000 * speed;
      for (let i = 0; i < numSub; i++) {
        const amp = 0.3 + 0.4 * Math.sin(t * 0.8 + i * 0.2) + noise * (Math.random() - 0.5);
        const phase = Math.sin(t * 0.5 + i * 0.4) * Math.PI + noise * (Math.random() - 0.5) * 2;
        frame.push({ amplitude: Math.max(0, Math.min(1, amp)), phase });
      }

      // Heatmap
      heatmapHistory.push(frame.map(f => f.amplitude));
      if (heatmapHistory.length > 50) heatmapHistory.shift();
      drawHeatmap(numSub);
      drawPhase(frame, numSub);
    }, 80);
  }

  function stopSignalLab() {
    if (!signalInterval) return;
    clearInterval(signalInterval);
    signalInterval = null;
    const btn = document.getElementById('sigStart');
    const stopBtn = document.getElementById('sigStop');
    if (btn) btn.disabled = false;
    if (stopBtn) stopBtn.disabled = true;
  }

  function drawHeatmap(numSub) {
    const canvas = document.getElementById('heatmapCanvas');
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    const w = canvas.width;
    const h = canvas.height;
    ctx.fillStyle = '#0a0e17';
    ctx.fillRect(0, 0, w, h);

    const cellW = w / numSub;
    const cellH = h / 50;

    heatmapHistory.forEach((row, y) => {
      row.forEach((val, x) => {
        if (x >= numSub) return;
        const r = Math.floor(val * 200 + 30);
        const g = Math.floor(val * 80);
        const b = Math.floor(200 - val * 150);
        ctx.fillStyle = `rgb(${r},${g},${b})`;
        ctx.fillRect(x * cellW, y * cellH, cellW + 1, cellH + 1);
      });
    });
  }

  function drawPhase(frame, numSub) {
    const canvas = document.getElementById('phaseCanvas');
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    const w = canvas.width;
    const h = canvas.height;
    ctx.fillStyle = '#0a0e17';
    ctx.fillRect(0, 0, w, h);

    // Center line
    ctx.strokeStyle = 'rgba(42,53,72,0.5)';
    ctx.setLineDash([4, 4]);
    ctx.beginPath();
    ctx.moveTo(0, h / 2);
    ctx.lineTo(w, h / 2);
    ctx.stroke();
    ctx.setLineDash([]);

    // Phase line
    ctx.strokeStyle = 'rgba(168,85,247,0.8)';
    ctx.lineWidth = 2;
    ctx.beginPath();
    frame.forEach((f, i) => {
      const x = (i / (numSub - 1)) * w;
      const y = h / 2 - (f.phase / Math.PI) * (h / 2 - 10);
      if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
    });
    ctx.stroke();

    // Dots
    frame.forEach((f, i) => {
      const x = (i / (numSub - 1)) * w;
      const y = h / 2 - (f.phase / Math.PI) * (h / 2 - 10);
      ctx.beginPath();
      ctx.arc(x, y, 2, 0, Math.PI * 2);
      ctx.fillStyle = 'rgba(168,85,247,0.9)';
      ctx.fill();
    });

    ctx.fillStyle = 'rgba(100,116,139,0.5)';
    ctx.font = '10px monospace';
    ctx.fillText('+\u03C0', 4, 14);
    ctx.fillText('-\u03C0', 4, h - 4);
    ctx.fillText('PHASE DISTRIBUTION', w - 140, 14);
  }

  // ── API page ──
  let selectedEndpoint = 0;

  function initApiPage() {
    window.__selectEndpoint = function (i) {
      selectedEndpoint = i;
      const ep = API_ENDPOINTS[i];
      document.getElementById('apiMethod').textContent = ep.method;
      document.getElementById('apiPath').textContent = ep.path;
      document.getElementById('apiDesc').textContent = ep.desc;

      document.querySelectorAll('[data-ep]').forEach(el => {
        el.style.background = parseInt(el.dataset.ep) === i ? 'var(--bg-card-hover)' : '';
      });
    };

    document.getElementById('apiSend').addEventListener('click', () => {
      const ep = API_ENDPOINTS[selectedEndpoint];
      const response = generateMockApiResponse(ep);
      document.getElementById('apiResponse').textContent = JSON.stringify(response, null, 2);
      document.getElementById('apiStatus').textContent = '200';
      document.getElementById('apiStatus').className = 'tag tag-green';
    });

    window.__selectEndpoint(0);
  }

  function generateMockApiResponse(ep) {
    const responses = {
      '/': { name: 'WiFi-DensePose API', version: '1.0.0', environment: 'development', features: { pose_estimation: true, streaming: true, authentication: false } },
      '/health/health': { status: 'healthy', timestamp: new Date().toISOString(), components: { pose: { status: 'healthy' }, hardware: { status: 'healthy' }, stream: { status: 'healthy' } }, system_metrics: { cpu: { percent: 23.4 }, memory: { percent: 45.2 }, disk: { percent: 12.8 } } },
      '/health/ready': { status: 'ready', checks: { database: 'ready', hardware: 'ready', inference: 'ready' } },
      '/health/live': { status: 'alive', timestamp: new Date().toISOString() },
      '/api/v1/info': { name: 'WiFi-DensePose', version: '1.0.0', zones: ['zone1', 'zone2'], features: { pose_estimation: true, streaming: true, multi_zone: true } },
      '/api/v1/status': { services: { api: 'running', hardware: 'connected', inference: 'ready', streaming: 'active' }, streaming: { active_connections: 2, total_messages: 1847 } },
      '/api/v1/pose/current': { timestamp: new Date().toISOString(), persons: [{ person_id: 'person_0', confidence: 0.92, keypoints: generateKeypoints(250, 200).map(k => ({ x: +k.x.toFixed(1), y: +k.y.toFixed(1), confidence: +k.confidence.toFixed(2), name: k.name })), zone_id: 'zone1' }], processing_time: 11.3 },
      '/api/v1/pose/zones/summary': { zones: { zone_1: 1, zone_2: 0, zone_3: 2, zone_4: 0 } },
      '/api/v1/pose/stats': { total_detections: 8472, average_confidence: 0.87, peak_persons: 4, hours_analyzed: 24 },
      '/api/v1/stream/status': { is_active: true, connected_clients: 3, messages_sent: 4521, uptime: 3600 },
      '/api/v1/stream/start': { message: 'Streaming started', status: 'active' },
      '/api/v1/stream/stop': { message: 'Streaming stopped', status: 'inactive' },
      '/api/v1/stream/pose': { note: 'WebSocket endpoint — connect via ws://localhost:8000/api/v1/stream/pose', message_format: { type: 'pose_data', timestamp: 'ISO-8601', data: { pose: { persons: ['...'] }, confidence: 0.87, activity: 'walking' } } },
      '/api/v1/stream/events': { note: 'WebSocket endpoint — connect via ws://localhost:8000/api/v1/stream/events', event_types: ['zone_entry', 'zone_exit', 'fall_detected', 'activity_change'] },
    };
    return responses[ep.path] || { message: 'Mock response', endpoint: ep.path };
  }

  // ── Build page tabs ──
  function initBuildPage() {
    const tabBar = document.getElementById('buildTabs');
    if (!tabBar) return;
    tabBar.querySelectorAll('.tab-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        tabBar.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
        btn.classList.add('active');
        document.querySelectorAll('.tab-panel').forEach(p => {
          p.classList.toggle('active', p.dataset.tab === btn.dataset.tab);
        });
      });
    });
  }

  // ─── Init ───────────────────────────────────────────────

  function init() {
    // Nav click handlers
    document.querySelectorAll('.nav-item').forEach(el => {
      el.addEventListener('click', () => navigate(el.dataset.page));
    });

    // Mobile menu
    document.getElementById('menuToggle').addEventListener('click', () => {
      document.getElementById('sidebar').classList.toggle('open');
    });

    // Initial page
    navigate('overview');
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
