#!/usr/bin/env node
/**
 * WiFlow PCK Evaluation Script (ADR-079)
 *
 * Measures accuracy of WiFi-based pose estimation against ground-truth
 * camera keypoints using PCK (Percentage of Correct Keypoints) and MPJPE
 * (Mean Per-Joint Position Error) metrics.
 *
 * Usage:
 *   node scripts/eval-wiflow.js --model models/wiflow-supervised/wiflow-v1.json --data data/paired/aligned.paired.jsonl
 *   node scripts/eval-wiflow.js --baseline --data data/paired/aligned.paired.jsonl
 *   node scripts/eval-wiflow.js --model models/wiflow-supervised/wiflow-v1.json --data data/paired/aligned.paired.jsonl --verbose
 *
 * ADR: docs/adr/ADR-079
 */

'use strict';

const fs = require('fs');
const path = require('path');
const { parseArgs } = require('util');

// ---------------------------------------------------------------------------
// Resolve WiFlow model dependencies
// ---------------------------------------------------------------------------
const {
  WiFlowModel,
  COCO_KEYPOINTS,
  createRng,
} = require(path.join(__dirname, 'wiflow-model.js'));

const RUVLLM_PATH = path.resolve(__dirname, '..', 'vendor', 'ruvector', 'npm', 'packages', 'ruvllm', 'src');
const { SafeTensorsReader } = require(path.join(RUVLLM_PATH, 'export.js'));

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const NUM_KEYPOINTS = 17;
const DEFAULT_TORSO_LENGTH = 0.3; // normalized coords fallback

// Joint name aliases for display (short form)
const JOINT_NAMES = [
  'nose', 'l_eye', 'r_eye', 'l_ear', 'r_ear',
  'l_shoulder', 'r_shoulder', 'l_elbow', 'r_elbow',
  'l_wrist', 'r_wrist', 'l_hip', 'r_hip',
  'l_knee', 'r_knee', 'l_ankle', 'r_ankle',
];

// Shoulder indices: l_shoulder=5, r_shoulder=6
// Hip indices: l_hip=11, r_hip=12
const L_SHOULDER = 5;
const R_SHOULDER = 6;
const L_HIP = 11;
const R_HIP = 12;

// ---------------------------------------------------------------------------
// CLI argument parsing
// ---------------------------------------------------------------------------
const { values: args } = parseArgs({
  options: {
    model:    { type: 'string', short: 'm' },
    data:     { type: 'string', short: 'd' },
    baseline: { type: 'boolean', default: false },
    output:   { type: 'string', short: 'o' },
    verbose:  { type: 'boolean', short: 'v', default: false },
  },
  strict: true,
});

if (!args.data) {
  console.error('Usage: node scripts/eval-wiflow.js --data <paired-jsonl> [--model <path>] [--baseline] [--output <path>]');
  console.error('');
  console.error('Required:');
  console.error('  --data, -d <path>    Paired CSI + keypoint JSONL (from align-ground-truth.js)');
  console.error('');
  console.error('Options:');
  console.error('  --model, -m <path>   Path to trained model directory or JSON');
  console.error('  --baseline           Evaluate proxy-based baseline (no model)');
  console.error('  --output, -o <path>  Output eval report JSON');
  console.error('  --verbose, -v        Verbose output');
  process.exit(1);
}

if (!args.model && !args.baseline) {
  console.error('Error: Must specify either --model <path> or --baseline');
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------

/**
 * Load paired JSONL samples.
 * Each line: { csi: [...], csi_shape: [S, T], kp: [[x,y],...], conf: 0.xx, ... }
 */
function loadPairedData(filePath) {
  const content = fs.readFileSync(filePath, 'utf-8');
  const samples = [];
  for (const line of content.split('\n')) {
    if (!line.trim()) continue;
    try {
      const s = JSON.parse(line);
      if (!s.kp || !Array.isArray(s.kp)) continue;
      if (!s.csi && !s.csi_shape) continue;
      samples.push(s);
    } catch (e) {
      // skip malformed lines
    }
  }
  return samples;
}

// ---------------------------------------------------------------------------
// Model loading
// ---------------------------------------------------------------------------

/**
 * Load WiFlow model from a directory or JSON file.
 * Tries: model.safetensors, then config.json for architecture config.
 * Returns { model, name }.
 */
function loadModel(modelPath) {
  const stat = fs.statSync(modelPath);
  let modelDir;

  if (stat.isDirectory()) {
    modelDir = modelPath;
  } else {
    // Assume JSON file in a model directory
    modelDir = path.dirname(modelPath);
  }

  // Load architecture config if available
  let config = {};
  const configPath = path.join(modelDir, 'config.json');
  if (fs.existsSync(configPath)) {
    try {
      const raw = JSON.parse(fs.readFileSync(configPath, 'utf-8'));
      if (raw.custom) {
        config.inputChannels = raw.custom.inputChannels || 128;
        config.timeSteps = raw.custom.timeSteps || 20;
        config.numKeypoints = raw.custom.numKeypoints || 17;
        config.numHeads = raw.custom.numHeads || 8;
        config.seed = raw.custom.seed || 42;
      }
    } catch (e) {
      // use defaults
    }
  }

  // Load training-metrics.json for additional config
  const metricsPath = path.join(modelDir, 'training-metrics.json');
  if (fs.existsSync(metricsPath)) {
    try {
      const metrics = JSON.parse(fs.readFileSync(metricsPath, 'utf-8'));
      if (metrics.model && metrics.model.architecture === 'wiflow') {
        // metrics available for report
      }
    } catch (e) {
      // ignore
    }
  }

  // Create model with config
  const model = new WiFlowModel(config);
  model.setTraining(false); // eval mode

  // Load weights from SafeTensors
  const safetensorsPath = path.join(modelDir, 'model.safetensors');
  if (fs.existsSync(safetensorsPath)) {
    const buffer = new Uint8Array(fs.readFileSync(safetensorsPath));
    const reader = new SafeTensorsReader(buffer);
    const tensorNames = reader.getTensorNames();

    // Build tensor map for fromTensorMap
    const tensorMap = new Map();
    for (const name of tensorNames) {
      const tensor = reader.getTensor(name);
      if (tensor) {
        tensorMap.set(name, tensor.data);
      }
    }

    model.fromTensorMap(tensorMap);
    if (args.verbose) {
      console.log(`Loaded ${tensorNames.length} tensors from ${safetensorsPath}`);
      console.log(`Model params: ${model.numParams().toLocaleString()}`);
    }
  } else {
    console.warn(`WARN: No model.safetensors found in ${modelDir}, using random weights`);
  }

  // Derive model name
  const name = path.basename(modelDir);
  return { model, name };
}

// ---------------------------------------------------------------------------
// Baseline proxy pose generation (ADR-072 Phase 2 heuristic)
// ---------------------------------------------------------------------------

/**
 * Generate a proxy standing skeleton from CSI features.
 * If presence detected (amplitude energy > threshold), place a standing
 * person at center with standard COCO proportions, perturbed by motion energy.
 */
function generateBaselinePose(sample) {
  const rng = createRng(42);

  // Estimate presence from CSI amplitude energy
  const csi = sample.csi;
  let energy = 0;
  if (Array.isArray(csi)) {
    for (let i = 0; i < csi.length; i++) {
      energy += csi[i] * csi[i];
    }
    energy = Math.sqrt(energy / csi.length);
  }

  // Estimate motion energy (variance across subcarriers)
  let motionEnergy = 0;
  if (Array.isArray(csi) && sample.csi_shape) {
    const [S, T] = sample.csi_shape;
    if (T > 1) {
      for (let s = 0; s < S; s++) {
        let sum = 0;
        let sumSq = 0;
        for (let t = 0; t < T; t++) {
          const v = csi[s * T + t] || 0;
          sum += v;
          sumSq += v * v;
        }
        const mean = sum / T;
        motionEnergy += (sumSq / T) - (mean * mean);
      }
      motionEnergy = Math.sqrt(Math.max(0, motionEnergy / S));
    }
  }

  // Normalized presence heuristic
  const presence = Math.min(1, energy / 10);

  if (presence < 0.3) {
    // No person detected: return zero pose
    return new Float32Array(NUM_KEYPOINTS * 2);
  }

  // Standing skeleton at center (0.5, 0.5) with standard proportions
  // Coordinates are [x, y] in normalized [0, 1] space
  // y=0 is top, y=1 is bottom (image convention)
  const cx = 0.5;
  const headY = 0.2;
  const shoulderY = 0.32;
  const elbowY = 0.45;
  const wristY = 0.55;
  const hipY = 0.55;
  const kneeY = 0.72;
  const ankleY = 0.88;
  const shoulderW = 0.08;
  const hipW = 0.06;
  const armSpread = 0.12;

  // Standard standing pose keypoints [x, y]
  const skeleton = [
    [cx, headY],                              // 0: nose
    [cx - 0.02, headY - 0.02],               // 1: l_eye
    [cx + 0.02, headY - 0.02],               // 2: r_eye
    [cx - 0.04, headY],                       // 3: l_ear
    [cx + 0.04, headY],                       // 4: r_ear
    [cx - shoulderW, shoulderY],              // 5: l_shoulder
    [cx + shoulderW, shoulderY],              // 6: r_shoulder
    [cx - armSpread, elbowY],                 // 7: l_elbow
    [cx + armSpread, elbowY],                 // 8: r_elbow
    [cx - armSpread - 0.02, wristY],          // 9: l_wrist
    [cx + armSpread + 0.02, wristY],          // 10: r_wrist
    [cx - hipW, hipY],                        // 11: l_hip
    [cx + hipW, hipY],                        // 12: r_hip
    [cx - hipW, kneeY],                       // 13: l_knee
    [cx + hipW, kneeY],                       // 14: r_knee
    [cx - hipW, ankleY],                      // 15: l_ankle
    [cx + hipW, ankleY],                      // 16: r_ankle
  ];

  // Perturb limbs by motion energy
  const perturbScale = Math.min(motionEnergy * 0.1, 0.05);
  const result = new Float32Array(NUM_KEYPOINTS * 2);
  for (let k = 0; k < NUM_KEYPOINTS; k++) {
    const px = (rng() - 0.5) * 2 * perturbScale;
    const py = (rng() - 0.5) * 2 * perturbScale;
    result[k * 2] = Math.max(0, Math.min(1, skeleton[k][0] + px));
    result[k * 2 + 1] = Math.max(0, Math.min(1, skeleton[k][1] + py));
  }
  return result;
}

// ---------------------------------------------------------------------------
// Metric computation
// ---------------------------------------------------------------------------

/** Euclidean distance between two 2D points */
function dist2d(x1, y1, x2, y2) {
  const dx = x1 - x2;
  const dy = y1 - y2;
  return Math.sqrt(dx * dx + dy * dy);
}

/**
 * Compute torso length from ground-truth keypoints.
 * Torso = distance(mid_shoulder, mid_hip).
 * Returns DEFAULT_TORSO_LENGTH if shoulders or hips not visible.
 */
function computeTorsoLength(kp) {
  if (!kp || kp.length < 13) return DEFAULT_TORSO_LENGTH;

  const lsX = kp[L_SHOULDER][0];
  const lsY = kp[L_SHOULDER][1];
  const rsX = kp[R_SHOULDER][0];
  const rsY = kp[R_SHOULDER][1];
  const lhX = kp[L_HIP][0];
  const lhY = kp[L_HIP][1];
  const rhX = kp[R_HIP][0];
  const rhY = kp[R_HIP][1];

  // Check if joints are at origin (not visible)
  const shoulderVisible = (lsX !== 0 || lsY !== 0) && (rsX !== 0 || rsY !== 0);
  const hipVisible = (lhX !== 0 || lhY !== 0) && (rhX !== 0 || rhY !== 0);

  if (!shoulderVisible || !hipVisible) return DEFAULT_TORSO_LENGTH;

  const midShoulderX = (lsX + rsX) / 2;
  const midShoulderY = (lsY + rsY) / 2;
  const midHipX = (lhX + rhX) / 2;
  const midHipY = (lhY + rhY) / 2;

  const torso = dist2d(midShoulderX, midShoulderY, midHipX, midHipY);
  return torso > 0.01 ? torso : DEFAULT_TORSO_LENGTH;
}

/**
 * Evaluate predictions against ground truth.
 *
 * @param {Array<{pred: Float32Array, gt: number[][], conf: number}>} results
 * @returns {object} Evaluation report
 */
function computeMetrics(results) {
  const n = results.length;
  if (n === 0) {
    return {
      n_samples: 0,
      pck_10: 0, pck_20: 0, pck_50: 0,
      mpjpe: 0,
      per_joint_pck20: {},
      per_joint_mpjpe: {},
      conf_weighted_pck20: 0,
      conf_weighted_mpjpe: 0,
    };
  }

  // Accumulators
  const pckCounts = { 10: 0, 20: 0, 50: 0 };
  let totalJoints = 0;
  let totalMPJPE = 0;

  const perJointPck20 = new Float64Array(NUM_KEYPOINTS);
  const perJointMPJPE = new Float64Array(NUM_KEYPOINTS);
  const perJointCount = new Float64Array(NUM_KEYPOINTS);

  // Confidence-weighted accumulators
  let confWeightedPck20Num = 0;
  let confWeightedPck20Den = 0;
  let confWeightedMpjpeNum = 0;
  let confWeightedMpjpeDen = 0;

  for (const { pred, gt, conf } of results) {
    const torso = computeTorsoLength(gt);
    const w = Math.max(conf, 1e-6);

    for (let k = 0; k < NUM_KEYPOINTS; k++) {
      if (k >= gt.length) continue;

      const gtX = gt[k][0];
      const gtY = gt[k][1];
      const predX = pred[k * 2];
      const predY = pred[k * 2 + 1];

      const d = dist2d(predX, predY, gtX, gtY);

      totalJoints++;
      totalMPJPE += d;

      perJointMPJPE[k] += d;
      perJointCount[k] += 1;

      // PCK at different thresholds
      if (d < 0.10 * torso) pckCounts[10]++;
      if (d < 0.20 * torso) {
        pckCounts[20]++;
        perJointPck20[k]++;
        confWeightedPck20Num += w;
      }
      if (d < 0.50 * torso) pckCounts[50]++;

      confWeightedPck20Den += w;
      confWeightedMpjpeNum += d * w;
      confWeightedMpjpeDen += w;
    }
  }

  // Aggregate metrics
  const pck10 = totalJoints > 0 ? pckCounts[10] / totalJoints : 0;
  const pck20 = totalJoints > 0 ? pckCounts[20] / totalJoints : 0;
  const pck50 = totalJoints > 0 ? pckCounts[50] / totalJoints : 0;
  const mpjpe = totalJoints > 0 ? totalMPJPE / totalJoints : 0;

  // Per-joint breakdown
  const perJointPck20Map = {};
  const perJointMpjpeMap = {};
  for (let k = 0; k < NUM_KEYPOINTS; k++) {
    const name = JOINT_NAMES[k];
    perJointPck20Map[name] = perJointCount[k] > 0 ? perJointPck20[k] / perJointCount[k] : 0;
    perJointMpjpeMap[name] = perJointCount[k] > 0 ? perJointMPJPE[k] / perJointCount[k] : 0;
  }

  // Confidence-weighted
  const confPck20 = confWeightedPck20Den > 0 ? confWeightedPck20Num / confWeightedPck20Den : 0;
  const confMpjpe = confWeightedMpjpeDen > 0 ? confWeightedMpjpeNum / confWeightedMpjpeDen : 0;

  return {
    n_samples: n,
    pck_10: pck10,
    pck_20: pck20,
    pck_50: pck50,
    mpjpe,
    per_joint_pck20: perJointPck20Map,
    per_joint_mpjpe: perJointMpjpeMap,
    conf_weighted_pck20: confPck20,
    conf_weighted_mpjpe: confMpjpe,
  };
}

// ---------------------------------------------------------------------------
// Inference
// ---------------------------------------------------------------------------

/**
 * Run model inference on a single paired sample.
 * @param {WiFlowModel} model
 * @param {object} sample - { csi, csi_shape, kp, conf }
 * @returns {Float32Array} - [17*2] predicted keypoints
 */
function runModelInference(model, sample) {
  const csi = sample.csi;
  const shape = sample.csi_shape;
  const S = shape ? shape[0] : 128;
  const T = shape ? shape[1] : 20;

  // Prepare input as Float32Array [S, T]
  let input;
  if (csi instanceof Float32Array) {
    input = csi;
  } else if (Array.isArray(csi)) {
    input = new Float32Array(csi);
  } else {
    input = new Float32Array(S * T);
  }

  // Ensure correct size (pad or truncate)
  const expectedLen = model.inputChannels * model.timeSteps;
  if (input.length !== expectedLen) {
    const resized = new Float32Array(expectedLen);
    const copyLen = Math.min(input.length, expectedLen);
    resized.set(input.subarray(0, copyLen));
    input = resized;
  }

  return model.forward(input);
}

// ---------------------------------------------------------------------------
// Formatted output
// ---------------------------------------------------------------------------

function formatPercent(v) {
  return (v * 100).toFixed(1) + '%';
}

function formatFloat(v, decimals) {
  decimals = decimals || 4;
  return v.toFixed(decimals);
}

function printReport(report) {
  console.log('');
  console.log('WiFlow Evaluation Report (ADR-079)');
  console.log('===================================');
  console.log(`Model:    ${report.model}`);
  console.log(`Samples:  ${report.n_samples.toLocaleString()}`);
  console.log(`PCK@10:   ${formatPercent(report.pck_10)}`);
  console.log(`PCK@20:   ${formatPercent(report.pck_20)}`);
  console.log(`PCK@50:   ${formatPercent(report.pck_50)}`);
  console.log(`MPJPE:    ${formatFloat(report.mpjpe)}`);
  console.log('');
  console.log('Per-Joint PCK@20:');

  const maxNameLen = Math.max(...JOINT_NAMES.map(n => n.length));
  for (const name of JOINT_NAMES) {
    const pck = report.per_joint_pck20[name] || 0;
    const pad = ' '.repeat(maxNameLen - name.length + 2);
    console.log(`  ${name}${pad}${formatPercent(pck)}`);
  }

  console.log('');
  console.log('Per-Joint MPJPE:');
  for (const name of JOINT_NAMES) {
    const mpjpe = report.per_joint_mpjpe[name] || 0;
    const pad = ' '.repeat(maxNameLen - name.length + 2);
    console.log(`  ${name}${pad}${formatFloat(mpjpe)}`);
  }

  console.log('');
  console.log('Confidence-Weighted:');
  console.log(`  PCK@20: ${formatPercent(report.conf_weighted_pck20)}`);
  console.log(`  MPJPE:  ${formatFloat(report.conf_weighted_mpjpe)}`);
  console.log('');
  console.log(`Inference: ${report.inference_latency_ms.toFixed(2)}ms/sample`);
  console.log('');
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function main() {
  // Load paired data
  if (args.verbose) console.log(`Loading paired data from ${args.data}...`);
  const samples = loadPairedData(args.data);
  if (samples.length === 0) {
    console.error('Error: No valid paired samples found in', args.data);
    process.exit(1);
  }
  if (args.verbose) console.log(`Loaded ${samples.length} paired samples`);

  let modelName;
  let model = null;

  if (args.baseline) {
    modelName = 'baseline-proxy';
    if (args.verbose) console.log('Running baseline proxy evaluation (ADR-072 Phase 2 heuristic)');
  } else {
    const loaded = loadModel(args.model);
    model = loaded.model;
    modelName = loaded.name;
    if (args.verbose) console.log(`Running model evaluation: ${modelName}`);
  }

  // Run inference and collect results
  const results = [];
  const startTime = process.hrtime.bigint();

  for (const sample of samples) {
    let pred;
    if (args.baseline) {
      pred = generateBaselinePose(sample);
    } else {
      pred = runModelInference(model, sample);
    }

    results.push({
      pred,
      gt: sample.kp,
      conf: sample.conf || 0,
    });
  }

  const endTime = process.hrtime.bigint();
  const totalMs = Number(endTime - startTime) / 1e6;
  const latencyMs = totalMs / samples.length;

  // Compute metrics
  const metrics = computeMetrics(results);

  // Build report
  const report = {
    model: modelName,
    n_samples: metrics.n_samples,
    pck_10: Math.round(metrics.pck_10 * 10000) / 10000,
    pck_20: Math.round(metrics.pck_20 * 10000) / 10000,
    pck_50: Math.round(metrics.pck_50 * 10000) / 10000,
    mpjpe: Math.round(metrics.mpjpe * 100000) / 100000,
    per_joint_pck20: {},
    per_joint_mpjpe: {},
    conf_weighted_pck20: Math.round(metrics.conf_weighted_pck20 * 10000) / 10000,
    conf_weighted_mpjpe: Math.round(metrics.conf_weighted_mpjpe * 100000) / 100000,
    inference_latency_ms: Math.round(latencyMs * 100) / 100,
    timestamp: new Date().toISOString(),
  };

  // Round per-joint metrics
  for (const name of JOINT_NAMES) {
    report.per_joint_pck20[name] = Math.round((metrics.per_joint_pck20[name] || 0) * 10000) / 10000;
    report.per_joint_mpjpe[name] = Math.round((metrics.per_joint_mpjpe[name] || 0) * 100000) / 100000;
  }

  // Print formatted report
  printReport(report);

  // Write output JSON
  const outputPath = args.output ||
    (args.model
      ? path.join(path.dirname(
          fs.statSync(args.model).isDirectory() ? path.join(args.model, '.') : args.model
        ), 'eval-report.json')
      : 'models/wiflow-supervised/eval-report.json');

  const outputDir = path.dirname(outputPath);
  if (!fs.existsSync(outputDir)) {
    fs.mkdirSync(outputDir, { recursive: true });
  }

  fs.writeFileSync(outputPath, JSON.stringify(report, null, 2) + '\n');
  console.log(`Report saved to ${outputPath}`);
}

main();
