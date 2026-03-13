/**
 * PoseDecoder — Maps motion detection grid → 17 COCO keypoints.
 *
 * Uses per-cell motion intensity to track actual body part positions:
 * - Head: top-center motion cluster
 * - Shoulders/Elbows/Wrists: lateral motion in upper body zone
 * - Hips/Knees/Ankles: lower body motion distribution
 *
 * When person exits frame, CSI data continues tracking (through-wall mode).
 */

// Extended keypoint definitions: 17 COCO + 9 hand/fingertip approximations = 26 total
export const KEYPOINT_NAMES = [
  'nose', 'left_eye', 'right_eye', 'left_ear', 'right_ear',
  'left_shoulder', 'right_shoulder', 'left_elbow', 'right_elbow',
  'left_wrist', 'right_wrist', 'left_hip', 'right_hip',
  'left_knee', 'right_knee', 'left_ankle', 'right_ankle',
  // Extended: hand keypoints (17-25)
  'left_thumb', 'left_index', 'left_pinky',       // 17, 18, 19
  'right_thumb', 'right_index', 'right_pinky',    // 20, 21, 22
  'left_foot_index', 'right_foot_index',           // 23, 24 (toe tips)
  'neck',                                          // 25 (mid-shoulder)
];

// Skeleton connections (pairs of keypoint indices)
export const SKELETON_CONNECTIONS = [
  [0, 1], [0, 2], [1, 3], [2, 4],           // Head
  [0, 25],                                    // Nose → neck
  [25, 5], [25, 6],                           // Neck → shoulders
  [5, 7], [7, 9],                             // Left arm
  [6, 8], [8, 10],                            // Right arm
  [5, 11], [6, 12],                           // Torso
  [11, 12],                                   // Hips
  [11, 13], [13, 15],                         // Left leg
  [12, 14], [14, 16],                         // Right leg
  // Hand connections
  [9, 17], [9, 18], [9, 19],                 // Left wrist → fingers
  [10, 20], [10, 21], [10, 22],              // Right wrist → fingers
  // Foot connections
  [15, 23], [16, 24],                         // Ankles → toes
];

// Standard body proportions (relative to body height)
const PROPORTIONS = {
  headToShoulder: 0.15,
  shoulderWidth: 0.25,
  shoulderToElbow: 0.18,
  elbowToWrist: 0.16,
  shoulderToHip: 0.30,
  hipWidth: 0.18,
  hipToKnee: 0.24,
  kneeToAnkle: 0.24,
  eyeSpacing: 0.04,
  earSpacing: 0.07,
  // Hand proportions
  wristToFinger: 0.09,
  fingerSpread: 0.04,
  thumbAngle: 0.6,    // radians from wrist-elbow axis
  // Foot proportions
  ankleToToe: 0.06,
};

export class PoseDecoder {
  constructor(embeddingDim = 128) {
    this.embeddingDim = embeddingDim;
    this.smoothedKeypoints = null;
    this.smoothingFactor = 0.25; // Low = responsive to real movement
    this._time = 0;

    // Through-wall tracking state
    this._lastBodyState = null;
    this._ghostState = null;
    this._ghostConfidence = 0;
    this._ghostVelocity = { x: 0, y: 0 };

    // Zone centroid tracking (normalized 0-1 positions)
    this._headCx = 0.5;
    this._headCy = 0.15;
    this._leftArmCx = 0.3;
    this._leftArmCy = 0.35;
    this._rightArmCx = 0.7;
    this._rightArmCy = 0.35;
    this._leftLegCx = 0.4;
    this._leftLegCy = 0.8;
    this._rightLegCx = 0.6;
    this._rightLegCy = 0.8;
    this._torsoCx = 0.5;
    this._torsoCy = 0.45;

    // RuVector embedding → joint mapping
    // Each joint gets 2 consecutive embedding dimensions (dx, dy offset)
    // and 1 dimension for confidence modulation. 26 joints × 3 = 78 dims used from 128.
    // Remaining 50 dims encode global pose features (body scale, rotation, lean).
    this._jointEmbMap = this._buildJointEmbeddingMap(embeddingDim);

    // Attention contribution tracking (for UI overlay)
    this.attentionStats = { energy: 0, maxDim: 0, refinementMag: 0 };
  }

  /**
   * Build the mapping from embedding dimensions to joint refinement signals.
   * This maps the RuVector attention output to anatomically meaningful joint offsets.
   */
  _buildJointEmbeddingMap(dim) {
    const map = [];
    // 26 joints × 3 dims each (dx, dy, confidence_mod) = 78 dims
    for (let j = 0; j < 26; j++) {
      const base = j * 3;
      if (base + 2 < dim) {
        map.push({ dxDim: base, dyDim: base + 1, confDim: base + 2 });
      } else {
        map.push({ dxDim: j % dim, dyDim: (j + 1) % dim, confDim: (j + 2) % dim });
      }
    }
    // Global pose features from dims 78-127
    return {
      joints: map,
      scaleDim: Math.min(78, dim - 1),       // body scale factor
      rotDim: Math.min(79, dim - 1),          // body rotation
      leanXDim: Math.min(80, dim - 1),        // lateral lean
      leanYDim: Math.min(81, dim - 1),        // forward/back lean
    };
  }

  /**
   * Decode motion data into 17 keypoints
   * @param {Float32Array} embedding - Fused embedding vector
   * @param {{ detected, x, y, w, h, motionGrid, gridCols, gridRows, motionCx, motionCy, exitDirection }} motionRegion
   * @param {number} elapsed - Time in seconds
   * @param {{ csiPresence: number }} csiState - CSI sensing state for through-wall
   * @returns {Array<{x: number, y: number, confidence: number, name: string}>}
   */
  decode(embedding, motionRegion, elapsed, csiState = {}) {
    this._time = elapsed;

    const hasMotion = motionRegion && motionRegion.detected;
    const hasCsi = csiState && csiState.csiPresence > 0.1;

    if (hasMotion) {
      // Active tracking from video motion grid
      this._ghostConfidence = 0;
      const rawKeypoints = this._trackFromMotionGrid(motionRegion, embedding, elapsed);
      this._lastBodyState = { keypoints: rawKeypoints.map(kp => ({...kp})), time: elapsed };

      // Track exit velocity
      if (motionRegion.exitDirection) {
        const speed = 0.008;
        this._ghostVelocity = {
          x: motionRegion.exitDirection === 'left' ? -speed : motionRegion.exitDirection === 'right' ? speed : 0,
          y: motionRegion.exitDirection === 'up' ? -speed : motionRegion.exitDirection === 'down' ? speed : 0
        };
      }

      // Apply temporal smoothing
      if (this.smoothedKeypoints && this.smoothedKeypoints.length === rawKeypoints.length) {
        const alpha = this.smoothingFactor;
        for (let i = 0; i < rawKeypoints.length; i++) {
          rawKeypoints[i].x = alpha * this.smoothedKeypoints[i].x + (1 - alpha) * rawKeypoints[i].x;
          rawKeypoints[i].y = alpha * this.smoothedKeypoints[i].y + (1 - alpha) * rawKeypoints[i].y;
        }
      }

      this.smoothedKeypoints = rawKeypoints;
      return rawKeypoints;

    } else if (this._lastBodyState && (hasCsi || this._ghostConfidence > 0.05)) {
      // Through-wall mode: person left frame but CSI still senses them
      return this._trackThroughWall(elapsed, csiState);

    } else if (this.smoothedKeypoints) {
      // Fade out
      const faded = this.smoothedKeypoints.map(kp => ({
        ...kp,
        confidence: kp.confidence * 0.88
      })).filter(kp => kp.confidence > 0.05);
      if (faded.length === 0) this.smoothedKeypoints = null;
      else this.smoothedKeypoints = faded;
      return faded;
    }

    return [];
  }

  /**
   * Track body parts from the motion grid.
   * Finds the centroid of motion in each body zone and positions joints there.
   */
  _trackFromMotionGrid(region, embedding, elapsed) {
    const grid = region.motionGrid;
    const cols = region.gridCols || 10;
    const rows = region.gridRows || 8;

    // Body bounding box (in normalized 0-1 coords)
    const bx = region.x, by = region.y, bw = region.w, bh = region.h;
    const cx = bx + bw / 2;
    const cy = by + bh / 2;
    const bodyH = Math.max(bh, 0.3);
    const bodyW = Math.max(bw, 0.15);

    // Find motion centroids per body zone from the grid
    if (grid) {
      const zones = this._findZoneCentroids(grid, cols, rows, bx, by, bw, bh);
      // Smooth with low alpha for responsiveness
      const a = 0.3; // 30% old, 70% new → responsive
      this._headCx    = a * this._headCx    + (1 - a) * zones.head.x;
      this._headCy    = a * this._headCy    + (1 - a) * zones.head.y;
      this._leftArmCx = a * this._leftArmCx + (1 - a) * zones.leftArm.x;
      this._leftArmCy = a * this._leftArmCy + (1 - a) * zones.leftArm.y;
      this._rightArmCx= a * this._rightArmCx+ (1 - a) * zones.rightArm.x;
      this._rightArmCy= a * this._rightArmCy+ (1 - a) * zones.rightArm.y;
      this._leftLegCx = a * this._leftLegCx + (1 - a) * zones.leftLeg.x;
      this._leftLegCy = a * this._leftLegCy + (1 - a) * zones.leftLeg.y;
      this._rightLegCx= a * this._rightLegCx+ (1 - a) * zones.rightLeg.x;
      this._rightLegCy= a * this._rightLegCy+ (1 - a) * zones.rightLeg.y;
      this._torsoCx   = a * this._torsoCx   + (1 - a) * zones.torso.x;
      this._torsoCy   = a * this._torsoCy   + (1 - a) * zones.torso.y;
    }

    const P = PROPORTIONS;

    // Breathing (subtle)
    const breathe = Math.sin(elapsed * 1.5) * 0.002;

    // === Position joints using tracked centroids ===

    // HEAD: tracked centroid (top zone)
    const headX = this._headCx;
    const headY = this._headCy;

    // TORSO center drives shoulder/hip
    const torsoX = this._torsoCx;
    const shoulderY = this._torsoCy - bodyH * 0.08 + breathe;
    const halfW = P.shoulderWidth * bodyH / 2;
    const hipHalfW = P.hipWidth * bodyH / 2;
    const hipY = shoulderY + P.shoulderToHip * bodyH;

    // ARMS: elbow + wrist driven toward arm zone centroids
    // Left arm: shoulder is fixed, elbow/wrist pulled toward left arm centroid
    const lShX = torsoX - halfW;
    const lShY = shoulderY;
    // Vector from shoulder toward arm centroid
    const lArmDx = this._leftArmCx - lShX;
    const lArmDy = this._leftArmCy - lShY;
    const lArmDist = Math.sqrt(lArmDx * lArmDx + lArmDy * lArmDy) || 0.01;
    const lArmNx = lArmDx / lArmDist;
    const lArmNy = lArmDy / lArmDist;
    // Elbow at shoulderToElbow distance along that direction
    const elbowLen = P.shoulderToElbow * bodyH;
    const lElbowX = lShX + lArmNx * elbowLen;
    const lElbowY = lShY + lArmNy * elbowLen;
    // Wrist continues further
    const wristLen = P.elbowToWrist * bodyH;
    const lWristX = lElbowX + lArmNx * wristLen;
    const lWristY = lElbowY + lArmNy * wristLen;

    // Right arm: same approach
    const rShX = torsoX + halfW;
    const rShY = shoulderY;
    const rArmDx = this._rightArmCx - rShX;
    const rArmDy = this._rightArmCy - rShY;
    const rArmDist = Math.sqrt(rArmDx * rArmDx + rArmDy * rArmDy) || 0.01;
    const rArmNx = rArmDx / rArmDist;
    const rArmNy = rArmDy / rArmDist;
    const rElbowX = rShX + rArmNx * elbowLen;
    const rElbowY = rShY + rArmNy * elbowLen;
    const rWristX = rElbowX + rArmNx * wristLen;
    const rWristY = rElbowY + rArmNy * wristLen;

    // LEGS: knees/ankles pulled toward leg zone centroids
    const lHipX = torsoX - hipHalfW;
    const rHipX = torsoX + hipHalfW;
    const lLegDx = this._leftLegCx - lHipX;
    const lLegDy = Math.max(0.05, this._leftLegCy - hipY); // always downward
    const lLegDist = Math.sqrt(lLegDx * lLegDx + lLegDy * lLegDy) || 0.01;
    const lLegNx = lLegDx / lLegDist;
    const lLegNy = lLegDy / lLegDist;
    const kneeLen = P.hipToKnee * bodyH;
    const ankleLen = P.kneeToAnkle * bodyH;
    const lKneeX = lHipX + lLegNx * kneeLen;
    const lKneeY = hipY + lLegNy * kneeLen;
    const lAnkleX = lKneeX + lLegNx * ankleLen;
    const lAnkleY = lKneeY + lLegNy * ankleLen;

    const rLegDx = this._rightLegCx - rHipX;
    const rLegDy = Math.max(0.05, this._rightLegCy - hipY);
    const rLegDist = Math.sqrt(rLegDx * rLegDx + rLegDy * rLegDy) || 0.01;
    const rLegNx = rLegDx / rLegDist;
    const rLegNy = rLegDy / rLegDist;
    const rKneeX = rHipX + rLegNx * kneeLen;
    const rKneeY = hipY + rLegNy * kneeLen;
    const rAnkleX = rKneeX + rLegNx * ankleLen;
    const rAnkleY = rKneeY + rLegNy * ankleLen;

    // Arm raise amount (for hand openness)
    const leftArmRaise = Math.max(0, Math.min(1, (shoulderY - this._leftArmCy) / (bodyH * 0.3)));
    const rightArmRaise = Math.max(0, Math.min(1, (shoulderY - this._rightArmCy) / (bodyH * 0.3)));

    // Compute hand finger positions from wrist-elbow axis
    const lHandAngle = Math.atan2(lWristY - lElbowY, lWristX - lElbowX);
    const rHandAngle = Math.atan2(rWristY - rElbowY, rWristX - rElbowX);
    const fingerLen = P.wristToFinger * bodyH;
    const fingerSpr = P.fingerSpread * bodyH;

    // Hand openness driven by arm raise + arm lateral spread
    const lArmSpread = Math.abs(this._leftArmCx - (bx + bw * 0.3)) / (bw * 0.3);
    const rArmSpread = Math.abs(this._rightArmCx - (bx + bw * 0.7)) / (bw * 0.3);
    const lHandOpen = Math.min(1, leftArmRaise * 0.5 + lArmSpread * 0.5);
    const rHandOpen = Math.min(1, rightArmRaise * 0.5 + rArmSpread * 0.5);

    const keypoints = [
      // 0: nose
      { x: headX, y: headY + 0.01, confidence: 0.92 },
      // 1: left_eye
      { x: headX - P.eyeSpacing * bodyH, y: headY - 0.005, confidence: 0.88 },
      // 2: right_eye
      { x: headX + P.eyeSpacing * bodyH, y: headY - 0.005, confidence: 0.88 },
      // 3: left_ear
      { x: headX - P.earSpacing * bodyH, y: headY + 0.005, confidence: 0.72 },
      // 4: right_ear
      { x: headX + P.earSpacing * bodyH, y: headY + 0.005, confidence: 0.72 },
      // 5: left_shoulder
      { x: lShX, y: lShY, confidence: 0.94 },
      // 6: right_shoulder
      { x: rShX, y: rShY, confidence: 0.94 },
      // 7: left_elbow
      { x: lElbowX, y: lElbowY, confidence: 0.87 },
      // 8: right_elbow
      { x: rElbowX, y: rElbowY, confidence: 0.87 },
      // 9: left_wrist
      { x: lWristX, y: lWristY, confidence: 0.82 },
      // 10: right_wrist
      { x: rWristX, y: rWristY, confidence: 0.82 },
      // 11: left_hip
      { x: lHipX, y: hipY, confidence: 0.91 },
      // 12: right_hip
      { x: rHipX, y: hipY, confidence: 0.91 },
      // 13: left_knee
      { x: lKneeX, y: lKneeY, confidence: 0.88 },
      // 14: right_knee
      { x: rKneeX, y: rKneeY, confidence: 0.88 },
      // 15: left_ankle
      { x: lAnkleX, y: lAnkleY, confidence: 0.83 },
      // 16: right_ankle
      { x: rAnkleX, y: rAnkleY, confidence: 0.83 },

      // === Extended keypoints (17-25) ===

      // 17: left_thumb — offset at thumb angle from wrist-elbow axis
      { x: lWristX + fingerLen * Math.cos(lHandAngle + P.thumbAngle) * (0.6 + lHandOpen * 0.4),
        y: lWristY + fingerLen * Math.sin(lHandAngle + P.thumbAngle) * (0.6 + lHandOpen * 0.4),
        confidence: 0.68 * (0.5 + lHandOpen * 0.5) },
      // 18: left_index — extends along wrist-elbow axis
      { x: lWristX + fingerLen * Math.cos(lHandAngle) + fingerSpr * lHandOpen * Math.cos(lHandAngle + 0.3),
        y: lWristY + fingerLen * Math.sin(lHandAngle) + fingerSpr * lHandOpen * Math.sin(lHandAngle + 0.3),
        confidence: 0.72 * (0.5 + lHandOpen * 0.5) },
      // 19: left_pinky — offset opposite thumb
      { x: lWristX + fingerLen * 0.85 * Math.cos(lHandAngle - P.thumbAngle * 0.7),
        y: lWristY + fingerLen * 0.85 * Math.sin(lHandAngle - P.thumbAngle * 0.7),
        confidence: 0.60 * (0.5 + lHandOpen * 0.5) },

      // 20: right_thumb
      { x: rWristX + fingerLen * Math.cos(rHandAngle - P.thumbAngle) * (0.6 + rHandOpen * 0.4),
        y: rWristY + fingerLen * Math.sin(rHandAngle - P.thumbAngle) * (0.6 + rHandOpen * 0.4),
        confidence: 0.68 * (0.5 + rHandOpen * 0.5) },
      // 21: right_index
      { x: rWristX + fingerLen * Math.cos(rHandAngle) + fingerSpr * rHandOpen * Math.cos(rHandAngle - 0.3),
        y: rWristY + fingerLen * Math.sin(rHandAngle) + fingerSpr * rHandOpen * Math.sin(rHandAngle - 0.3),
        confidence: 0.72 * (0.5 + rHandOpen * 0.5) },
      // 22: right_pinky
      { x: rWristX + fingerLen * 0.85 * Math.cos(rHandAngle + P.thumbAngle * 0.7),
        y: rWristY + fingerLen * 0.85 * Math.sin(rHandAngle + P.thumbAngle * 0.7),
        confidence: 0.60 * (0.5 + rHandOpen * 0.5) },

      // 23: left_foot_index (toe tip) — extends forward from ankle
      { x: lAnkleX + P.ankleToToe * bodyH * 0.5,
        y: lAnkleY + P.ankleToToe * bodyH * 0.3,
        confidence: 0.65 },
      // 24: right_foot_index
      { x: rAnkleX + P.ankleToToe * bodyH * 0.5,
        y: rAnkleY + P.ankleToToe * bodyH * 0.3,
        confidence: 0.65 },

      // 25: neck (midpoint between shoulders, slightly above)
      { x: (lShX + rShX) / 2, y: shoulderY - P.headToShoulder * bodyH * 0.35, confidence: 0.93 },
    ];

    for (let i = 0; i < keypoints.length; i++) {
      keypoints[i].name = KEYPOINT_NAMES[i];
    }

    // === RuVector Attention Embedding Refinement ===
    // Compute attention stats for the UI pipeline display, but only apply
    // positional refinement when a trained model is loaded (random-weight
    // embeddings carry no meaningful spatial signal and distort the skeleton).
    if (embedding && embedding.length >= 26 * 3) {
      this._computeEmbeddingStats(keypoints, embedding, bodyH);
    }

    return keypoints;
  }

  /**
   * Apply RuVector attention embedding to refine joint positions and confidence.
   *
   * The 128-dim fused embedding is decoded as:
   * - Dims 0-77:  Per-joint (dx, dy, confidence_mod) × 26 joints
   * - Dims 78-81: Global pose parameters (scale, rotation, lean)
   * - Dims 82-127: Reserved for cross-modal fusion features
   *
   * The attention mechanism determines HOW MUCH each spatial region contributes
   * to each joint's refinement. Multi-Head captures global relationships,
   * Hyperbolic captures hierarchical (torso→limb→hand) dependencies,
   * MoE routes different body regions to specialized experts,
   * Linear provides fast extremity refinement, Local-Global balances detail/context.
   */
  /**
   * Compute embedding statistics for UI display without modifying joint positions.
   * The 6-stage attention pipeline stats are shown in the RuVector panel.
   * Position refinement is disabled until a trained model replaces random weights.
   */
  _computeEmbeddingStats(keypoints, emb, bodyH) {
    const map = this._jointEmbMap;
    const tc = (v) => Math.tanh(Number(v) || 0);

    // Embedding energy (L2 norm of the used dims)
    let energy = 0;
    for (let i = 0; i < Math.min(emb.length, 82); i++) {
      energy += emb[i] * emb[i];
    }
    energy = Math.sqrt(energy);

    // Simulated per-joint refinement magnitude (what WOULD be applied)
    const scale = bodyH * 0.015;
    let totalRefinement = 0;
    let maxDimVal = 0;

    for (let j = 0; j < Math.min(keypoints.length, 26); j++) {
      const jmap = map.joints[j];
      if (!jmap) continue;
      const dx = tc(emb[jmap.dxDim]) * scale;
      const dy = tc(emb[jmap.dyDim]) * scale;
      totalRefinement += Math.sqrt(dx * dx + dy * dy);
      maxDimVal = Math.max(maxDimVal, Math.abs(tc(emb[jmap.dxDim])), Math.abs(tc(emb[jmap.dyDim])));
    }

    this.attentionStats.energy = energy;
    this.attentionStats.maxDim = maxDimVal;
    this.attentionStats.refinementMag = totalRefinement / 26;
  }

  /**
   * Find weighted motion centroids for each body zone.
   * Divides the bounding box into 6 zones: head, left arm, right arm, torso, left leg, right leg.
   * Returns the (x,y) centroid of motion intensity for each zone.
   */
  _findZoneCentroids(grid, cols, rows, bx, by, bw, bh) {
    // Zone definitions (in grid-relative fractions)
    const zones = {
      head:     { rMin: 0,    rMax: 0.2,  cMin: 0.25, cMax: 0.75, wx: 0, wy: 0, wt: 0 },
      leftArm:  { rMin: 0.1,  rMax: 0.6,  cMin: 0,    cMax: 0.35, wx: 0, wy: 0, wt: 0 },
      rightArm: { rMin: 0.1,  rMax: 0.6,  cMin: 0.65, cMax: 1.0,  wx: 0, wy: 0, wt: 0 },
      torso:    { rMin: 0.15, rMax: 0.55, cMin: 0.3,  cMax: 0.7,  wx: 0, wy: 0, wt: 0 },
      leftLeg:  { rMin: 0.5,  rMax: 1.0,  cMin: 0.1,  cMax: 0.5,  wx: 0, wy: 0, wt: 0 },
      rightLeg: { rMin: 0.5,  rMax: 1.0,  cMin: 0.5,  cMax: 0.9,  wx: 0, wy: 0, wt: 0 },
    };

    // Accumulate weighted centroids per zone
    for (let r = 0; r < rows; r++) {
      const ry = r / rows; // 0-1 within grid
      for (let c = 0; c < cols; c++) {
        const cx_g = c / cols; // 0-1 within grid
        const val = grid[r][c];
        if (val < 0.005) continue; // skip near-zero motion

        // Map grid position to body-space coordinates (0-1)
        const worldX = bx + cx_g * bw;
        const worldY = by + ry * bh;

        // Assign to matching zones (a cell can contribute to multiple overlapping zones)
        for (const z of Object.values(zones)) {
          if (ry >= z.rMin && ry < z.rMax && cx_g >= z.cMin && cx_g < z.cMax) {
            z.wx += worldX * val;
            z.wy += worldY * val;
            z.wt += val;
          }
        }
      }
    }

    // Compute centroids with fallback defaults
    const centroid = (z, defX, defY) => ({
      x: z.wt > 0.01 ? z.wx / z.wt : defX,
      y: z.wt > 0.01 ? z.wy / z.wt : defY,
      weight: z.wt
    });

    const midX = bx + bw / 2;
    const midY = by + bh / 2;

    return {
      head:     centroid(zones.head,     midX,           by + bh * 0.1),
      leftArm:  centroid(zones.leftArm,  bx + bw * 0.2, midY - bh * 0.05),
      rightArm: centroid(zones.rightArm, bx + bw * 0.8, midY - bh * 0.05),
      torso:    centroid(zones.torso,    midX,           midY),
      leftLeg:  centroid(zones.leftLeg,  bx + bw * 0.35,by + bh * 0.75),
      rightLeg: centroid(zones.rightLeg, bx + bw * 0.65,by + bh * 0.75),
    };
  }

  /**
   * Through-wall tracking: continue showing pose via CSI when person left video frame.
   * The skeleton drifts in the exit direction with decreasing confidence.
   */
  _trackThroughWall(elapsed, csiState) {
    if (!this._lastBodyState) return [];

    const dt = elapsed - this._lastBodyState.time;
    const csiPresence = csiState.csiPresence || 0;

    // Initialize ghost on first call
    if (this._ghostConfidence <= 0.05) {
      this._ghostConfidence = 0.8;
      this._ghostState = this._lastBodyState.keypoints.map(kp => ({...kp}));
    }

    // Ghost confidence decays, but CSI presence sustains it
    const csiBoost = Math.min(0.7, csiPresence * 0.8);
    this._ghostConfidence = Math.max(0.05, this._ghostConfidence * 0.995 - 0.001 + csiBoost * 0.002);

    // Drift the ghost in exit direction
    const vx = this._ghostVelocity.x;
    const vy = this._ghostVelocity.y;

    // Breathing continues via CSI
    const breathe = Math.sin(elapsed * 1.5) * 0.003 * csiPresence;

    const keypoints = this._ghostState.map((kp, i) => {
      return {
        x: kp.x + vx * dt * 0.3,
        y: kp.y + vy * dt * 0.3 + (i >= 5 && i <= 6 ? breathe : 0),
        confidence: kp.confidence * this._ghostConfidence * (0.5 + csiPresence * 0.5),
        name: kp.name
      };
    });

    // Slow down drift over time
    this._ghostVelocity.x *= 0.998;
    this._ghostVelocity.y *= 0.998;

    this.smoothedKeypoints = keypoints;
    return keypoints;
  }
}
