/**
 * FigurePool — Manages a pool of wireframe human figures for multi-person rendering.
 *
 * Extracted from main.js Observatory class. Owns the lifecycle of up to MAX_FIGURES
 * Three.js figure groups, each containing joints, bones, body segments, and aura.
 *
 * Improvements over the original inline implementation:
 * - Smooth joint interpolation (lerp toward target instead of snapping)
 * - Joint pulsation synced with breathing
 * - Natural bone thickness taper (thicker at shoulder/hip, thinner at extremities)
 * - Secondary motion with slight delay/overshoot for organic feel
 * - Pose-adaptive aura shape (wider for exercise, narrower for crouching)
 */
import * as THREE from 'three';

// 17-keypoint COCO skeleton connectivity
export const SKELETON_PAIRS = [
  [0, 1], [0, 2], [1, 3], [2, 4],
  [5, 6], [5, 7], [7, 9], [6, 8], [8, 10],
  [5, 11], [6, 12], [11, 12],
  [11, 13], [13, 15], [12, 14], [14, 16],
];

// Body segment cylinders that give volume to the wireframe
export const BODY_SEGMENT_DEFS = [
  { joints: [5, 11], radius: 0.12 },   // left torso
  { joints: [6, 12], radius: 0.12 },   // right torso
  { joints: [5, 6], radius: 0.1 },     // shoulder bar
  { joints: [11, 12], radius: 0.1 },   // hip bar
  { joints: [5, 7], radius: 0.05 },    // left upper arm
  { joints: [6, 8], radius: 0.05 },    // right upper arm
  { joints: [7, 9], radius: 0.04 },    // left forearm
  { joints: [8, 10], radius: 0.04 },   // right forearm
  { joints: [11, 13], radius: 0.07 },  // left thigh
  { joints: [12, 14], radius: 0.07 },  // right thigh
  { joints: [13, 15], radius: 0.05 },  // left shin
  { joints: [14, 16], radius: 0.05 },  // right shin
  { joints: [0, 0], radius: 0.1, isHead: true },
];

// Bone thickness multipliers — thicker at torso, thinner at extremities
const BONE_TAPER = (() => {
  const tapers = new Map();
  // Torso and shoulder/hip connections are thickest
  tapers.set('5-6', 1.4);    // shoulder bar
  tapers.set('11-12', 1.3);  // hip bar
  tapers.set('5-11', 1.3);   // left torso
  tapers.set('6-12', 1.3);   // right torso
  // Upper limbs
  tapers.set('5-7', 1.0);    // left upper arm
  tapers.set('6-8', 1.0);    // right upper arm
  tapers.set('11-13', 1.1);  // left thigh
  tapers.set('12-14', 1.1);  // right thigh
  // Lower limbs / extremities — thinnest
  tapers.set('7-9', 0.7);    // left forearm
  tapers.set('8-10', 0.7);   // right forearm
  tapers.set('13-15', 0.8);  // left shin
  tapers.set('14-16', 0.8);  // right shin
  // Head connections
  tapers.set('0-1', 0.5);
  tapers.set('0-2', 0.5);
  tapers.set('1-3', 0.4);
  tapers.set('2-4', 0.4);
  return tapers;
})();

// Secondary motion delay factors per joint — extremities lag more
const SECONDARY_DELAY = [
  0.12, // 0 nose
  0.10, // 1 left eye
  0.10, // 2 right eye
  0.08, // 3 left ear
  0.08, // 4 right ear
  0.18, // 5 left shoulder
  0.18, // 6 right shoulder
  0.14, // 7 left elbow
  0.14, // 8 right elbow
  0.10, // 9 left wrist (most lag)
  0.10, // 10 right wrist
  0.20, // 11 left hip (anchored, fast follow)
  0.20, // 12 right hip
  0.15, // 13 left knee
  0.15, // 14 right knee
  0.10, // 15 left ankle
  0.10, // 16 right ankle
];

// Overshoot factors — extremities overshoot more for organic feel
const OVERSHOOT = [
  0.02, // 0 nose
  0.01, // 1 left eye
  0.01, // 2 right eye
  0.01, // 3 left ear
  0.01, // 4 right ear
  0.03, // 5 left shoulder
  0.03, // 6 right shoulder
  0.05, // 7 left elbow
  0.05, // 8 right elbow
  0.08, // 9 left wrist
  0.08, // 10 right wrist
  0.02, // 11 left hip
  0.02, // 12 right hip
  0.04, // 13 left knee
  0.04, // 14 right knee
  0.06, // 15 left ankle
  0.06, // 16 right ankle
];

const MAX_FIGURES = 4;

// Reusable vectors to avoid per-frame allocation
const _vecFrom = new THREE.Vector3();
const _vecTo = new THREE.Vector3();
const _vecTarget = new THREE.Vector3();

export class FigurePool {
  /**
   * @param {THREE.Scene} scene - The Three.js scene to add figures to
   * @param {object} settings - Shared settings object (boneThick, jointSize, glow, etc.)
   * @param {object} poseSystem - PoseSystem instance with generateKeypoints(person, elapsed, breathPulse)
   */
  constructor(scene, settings, poseSystem) {
    this._scene = scene;
    this._settings = settings;
    this._poseSystem = poseSystem;
    this._figures = [];
    this._maxFigures = MAX_FIGURES;
    this._build();
  }

  /** @returns {Array} The array of figure objects */
  get figures() { return this._figures; }

  // ---- Construction ----

  _build() {
    for (let f = 0; f < this._maxFigures; f++) {
      this._figures.push(this._createFigure());
    }
  }

  _createFigure() {
    const group = new THREE.Group();
    this._scene.add(group);
    const wireColor = new THREE.Color(this._settings.wireColor);
    const jointColor = new THREE.Color(this._settings.jointColor);

    // Joints (17 COCO keypoints)
    const joints = [];
    for (let i = 0; i < 17; i++) {
      const isNose = i === 0;
      const size = isNose ? this._settings.jointSize * 0.7 : this._settings.jointSize;
      const geo = new THREE.SphereGeometry(size, 12, 12);
      const mat = new THREE.MeshStandardMaterial({
        color: isNose ? wireColor : jointColor,
        emissive: isNose ? wireColor : jointColor,
        emissiveIntensity: 0.35,
        transparent: true, opacity: 0,
        roughness: 0.3, metalness: 0.2,
      });
      const sphere = new THREE.Mesh(geo, mat);
      sphere.castShadow = true;
      group.add(sphere);
      joints.push(sphere);

      // Halo glow on key joints
      if ([5, 6, 9, 10, 11, 12, 15, 16].includes(i)) {
        const haloGeo = new THREE.SphereGeometry(size * 1.3, 8, 8);
        const haloMat = new THREE.MeshBasicMaterial({
          color: jointColor,
          transparent: true, opacity: 0,
          blending: THREE.AdditiveBlending,
          depthWrite: false,
        });
        const halo = new THREE.Mesh(haloGeo, haloMat);
        sphere.add(halo);
        sphere._halo = halo;
        sphere._haloMat = haloMat;

        const glow = new THREE.PointLight(jointColor, 0, 0.8);
        sphere.add(glow);
        sphere._glow = glow;
      }
    }

    // Bones — tapered thickness
    const bones = [];
    for (const [a, b] of SKELETON_PAIRS) {
      const taperKey = `${Math.min(a, b)}-${Math.max(a, b)}`;
      const taper = BONE_TAPER.get(taperKey) || 1.0;
      const thick = this._settings.boneThick * taper;
      // Top radius thicker than bottom for natural taper along bone length
      const topRadius = thick;
      const botRadius = thick * 0.65;
      const geo = new THREE.CylinderGeometry(topRadius, botRadius, 1, 8, 1);
      geo.translate(0, 0.5, 0);
      geo.rotateX(Math.PI / 2);
      const mat = new THREE.MeshStandardMaterial({
        color: wireColor, emissive: wireColor, emissiveIntensity: 0.3,
        transparent: true, opacity: 0, roughness: 0.4, metalness: 0.1,
      });
      const mesh = new THREE.Mesh(geo, mat);
      mesh.castShadow = true;
      group.add(mesh);
      bones.push({ mesh, a, b, taper });
    }

    // Body segments (volume cylinders and head sphere)
    const bodySegments = [];
    for (const seg of BODY_SEGMENT_DEFS) {
      const geo = seg.isHead
        ? new THREE.SphereGeometry(seg.radius, 12, 12)
        : new THREE.CylinderGeometry(seg.radius, seg.radius * 0.85, 1, 8, 1);
      if (!seg.isHead) {
        geo.translate(0, 0.5, 0);
        geo.rotateX(Math.PI / 2);
      }
      const mat = new THREE.MeshStandardMaterial({
        color: wireColor, emissive: wireColor, emissiveIntensity: 0.12,
        transparent: true, opacity: 0, roughness: 0.5, metalness: 0.1,
        side: THREE.DoubleSide,
      });
      const mesh = new THREE.Mesh(geo, mat);
      group.add(mesh);
      bodySegments.push({ mesh, mat, a: seg.joints[0], b: seg.joints[1], isHead: seg.isHead });
    }

    // Aura cylinder
    const auraGeo = new THREE.CylinderGeometry(0.4, 0.3, 1.7, 16, 1, true);
    const auraMat = new THREE.MeshBasicMaterial({
      color: wireColor, transparent: true, opacity: 0,
      side: THREE.DoubleSide, blending: THREE.AdditiveBlending, depthWrite: false,
    });
    const aura = new THREE.Mesh(auraGeo, auraMat);
    aura.position.y = 1;
    group.add(aura);

    // Per-figure point light
    const personLight = new THREE.PointLight(wireColor, 0, 6);
    personLight.position.y = 1;
    group.add(personLight);

    // Interpolation state: previous positions for smooth lerp and secondary motion
    const prevPositions = [];
    const velocities = [];
    for (let i = 0; i < 17; i++) {
      prevPositions.push(new THREE.Vector3(0, 0, 0));
      velocities.push(new THREE.Vector3(0, 0, 0));
    }

    return {
      group, joints, bones, bodySegments, aura, auraMat, personLight,
      visible: false,
      prevPositions,
      velocities,
      _initialized: false,
      _lastPose: null,
    };
  }

  // ---- Per-frame update ----

  /**
   * Update all figures based on current data frame.
   * @param {object} data - Current sensing data with persons[], vital_signs, classification
   * @param {number} elapsed - Elapsed time in seconds
   */
  update(data, elapsed) {
    const persons = data?.persons || [];
    const vs = data?.vital_signs || {};
    const isPresent = data?.classification?.presence || false;
    const breathBpm = vs.breathing_rate_bpm || 0;
    const breathPulse = breathBpm > 0
      ? Math.sin(elapsed * Math.PI * 2 * (breathBpm / 60)) * 0.012
      : 0;

    for (let f = 0; f < this._figures.length; f++) {
      const fig = this._figures[f];
      if (f < persons.length && isPresent) {
        const p = persons[f];
        const kps = this._poseSystem.generateKeypoints(p, elapsed, breathPulse);
        this.applyKeypoints(fig, kps, breathPulse, p.position || [0, 0, 0], elapsed, p.pose);
        fig.visible = true;
      } else {
        if (fig.visible) {
          this.hide(fig);
          fig.visible = false;
        }
      }
    }
  }

  /**
   * Apply keypoints to a figure with smooth interpolation, pulsation, and secondary motion.
   * @param {object} fig - Figure object from the pool
   * @param {Array} kps - 17-element array of [x,y,z] keypoint positions
   * @param {number} breathPulse - Current breathing pulse value
   * @param {Array} pos - Person world position [x,y,z]
   * @param {number} elapsed - Elapsed time for pulsation effects
   * @param {string} pose - Current pose name for aura adaptation
   */
  applyKeypoints(fig, kps, breathPulse, pos, elapsed = 0, pose = 'standing') {
    const lerpFactor = fig._initialized ? 0.18 : 1.0;

    // Joints with smooth interpolation and secondary motion
    for (let i = 0; i < 17 && i < kps.length; i++) {
      const j = fig.joints[i];
      _vecTarget.set(kps[i][0], kps[i][1], kps[i][2]);

      if (fig._initialized) {
        // Compute velocity for overshoot
        const prev = fig.prevPositions[i];
        const vel = fig.velocities[i];

        // Smooth lerp with per-joint delay
        const delay = SECONDARY_DELAY[i];
        const jointLerp = lerpFactor + delay;
        j.position.lerp(_vecTarget, Math.min(jointLerp, 0.95));

        // Apply subtle overshoot based on velocity change
        const overshoot = OVERSHOOT[i];
        vel.subVectors(j.position, prev).multiplyScalar(overshoot);
        j.position.add(vel);

        prev.copy(j.position);
      } else {
        // First frame: snap to position
        j.position.copy(_vecTarget);
        fig.prevPositions[i].copy(_vecTarget);
        fig.velocities[i].set(0, 0, 0);
      }

      j.material.opacity = 0.95;

      // Joint pulsation synced with breathing
      const pulseFactor = 1.0 + Math.abs(breathPulse) * 8.0;
      j.material.emissiveIntensity = 0.35 * pulseFactor;

      const baseScale = this._settings.jointSize / 0.04;
      // Subtle size pulsation on breathing
      const pulseScale = baseScale * (1.0 + Math.abs(breathPulse) * 3.0);
      j.scale.setScalar(pulseScale);

      if (j._haloMat) {
        j._haloMat.opacity = 0.04 * this._settings.glow * pulseFactor;
      }
      if (j._glow) {
        j._glow.intensity = this._settings.glow * 0.12 * pulseFactor;
      }
    }

    fig._initialized = true;

    // Bones with tapered thickness
    for (const bone of fig.bones) {
      const pA = kps[bone.a], pB = kps[bone.b];
      if (pA && pB) {
        _vecFrom.set(pA[0], pA[1], pA[2]);
        _vecTo.set(pB[0], pB[1], pB[2]);
        const len = _vecFrom.distanceTo(_vecTo);

        // Use interpolated joint positions for smooth bone movement
        if (fig._initialized) {
          const jA = fig.joints[bone.a];
          const jB = fig.joints[bone.b];
          bone.mesh.position.copy(jA.position);
          bone.mesh.scale.set(1, 1, jA.position.distanceTo(jB.position));
          bone.mesh.lookAt(jB.position);
        } else {
          bone.mesh.position.copy(_vecFrom);
          bone.mesh.scale.set(1, 1, len);
          bone.mesh.lookAt(_vecTo);
        }

        bone.mesh.material.opacity = 0.85;
        bone.mesh.material.emissiveIntensity = 0.3 + Math.abs(breathPulse) * 2.0;
      }
    }

    // Body segments
    for (const seg of fig.bodySegments) {
      if (seg.isHead) {
        const headJoint = fig.joints[seg.a];
        seg.mesh.position.set(headJoint.position.x, headJoint.position.y + 0.05, headJoint.position.z);
        seg.mat.opacity = 0.15;
      } else {
        const jA = fig.joints[seg.a];
        const jB = fig.joints[seg.b];
        if (jA && jB) {
          const len = jA.position.distanceTo(jB.position);
          seg.mesh.position.copy(jA.position);
          seg.mesh.scale.set(1, 1, len);
          seg.mesh.lookAt(jB.position);
          seg.mat.opacity = 0.12;
        }
      }
      seg.mat.emissiveIntensity = 0.1 + Math.abs(breathPulse) * 0.4;
    }

    // Aura — adapt shape to pose
    const hipY = (fig.joints[11].position.y + fig.joints[12].position.y) / 2;
    const cx = (fig.joints[11].position.x + fig.joints[12].position.x) / 2;
    const cz = (fig.joints[11].position.z + fig.joints[12].position.z) / 2;
    fig.aura.position.set(cx, hipY, cz);
    fig.auraMat.opacity = this._settings.aura + Math.abs(breathPulse) * 0.8;

    // Pose-adaptive aura: compute from actual keypoint spread
    const auraShape = this._computeAuraShape(fig, pose, breathPulse);
    fig.aura.scale.set(auraShape.scaleX, auraShape.scaleY, auraShape.scaleZ);

    // Person light
    fig.personLight.position.set(pos[0], 1.2, pos[2]);
    fig.personLight.intensity = this._settings.glow * 0.4;

    fig._lastPose = pose;
  }

  /**
   * Compute pose-adaptive aura shape based on actual keypoint spread.
   * Wider for exercise/spread poses, narrower for crouching/compact poses.
   */
  _computeAuraShape(fig, pose, breathPulse) {
    // Measure horizontal spread from shoulders and hips
    const lShoulder = fig.joints[5].position;
    const rShoulder = fig.joints[6].position;
    const lHip = fig.joints[11].position;
    const rHip = fig.joints[12].position;
    const nose = fig.joints[0].position;
    const lAnkle = fig.joints[15].position;
    const rAnkle = fig.joints[16].position;

    // Horizontal spread (X-Z plane)
    const shoulderWidth = Math.sqrt(
      (rShoulder.x - lShoulder.x) ** 2 +
      (rShoulder.z - lShoulder.z) ** 2
    );
    const ankleWidth = Math.sqrt(
      (rAnkle.x - lAnkle.x) ** 2 +
      (rAnkle.z - lAnkle.z) ** 2
    );
    const maxWidth = Math.max(shoulderWidth, ankleWidth);

    // Vertical extent
    const headY = nose.y;
    const footY = Math.min(lAnkle.y, rAnkle.y);
    const height = headY - footY;

    // Normalize to base aura dimensions
    const baseWidth = 0.44; // default shoulder width
    const baseHeight = 1.7; // default standing height

    const widthRatio = Math.max(0.6, Math.min(2.0, maxWidth / baseWidth));
    const heightRatio = Math.max(0.4, Math.min(1.3, height / baseHeight));

    // Breathing modulation
    const breathMod = 1 + breathPulse * 2;

    return {
      scaleX: widthRatio * breathMod,
      scaleY: heightRatio * breathMod,
      scaleZ: widthRatio * breathMod,
    };
  }

  /**
   * Hide a figure by fading all materials to invisible.
   * @param {object} fig - Figure object to hide
   */
  hide(fig) {
    for (const j of fig.joints) {
      j.material.opacity = 0;
      if (j._haloMat) j._haloMat.opacity = 0;
      if (j._glow) j._glow.intensity = 0;
    }
    for (const b of fig.bones) b.mesh.material.opacity = 0;
    for (const seg of fig.bodySegments) seg.mat.opacity = 0;
    fig.auraMat.opacity = 0;
    fig.personLight.intensity = 0;
    fig._initialized = false;
  }

  /**
   * Apply wire and joint colors to all figures in the pool.
   * @param {THREE.Color} wireColor
   * @param {THREE.Color} jointColor
   */
  applyColors(wireColor, jointColor) {
    for (const fig of this._figures) {
      for (let i = 0; i < fig.joints.length; i++) {
        const j = fig.joints[i];
        if (i === 0) {
          j.material.color.copy(wireColor);
          j.material.emissive.copy(wireColor);
        } else {
          j.material.color.copy(jointColor);
          j.material.emissive.copy(jointColor);
        }
        if (j._haloMat) j._haloMat.color.copy(jointColor);
        if (j._glow) j._glow.color.copy(jointColor);
      }
      for (const b of fig.bones) {
        b.mesh.material.color.copy(wireColor);
        b.mesh.material.emissive.copy(wireColor);
      }
      for (const seg of fig.bodySegments) {
        seg.mat.color.copy(wireColor);
        seg.mat.emissive.copy(wireColor);
      }
      fig.auraMat.color.copy(wireColor);
      fig.personLight.color.copy(wireColor);
    }
  }
}
