// 3D Human Body Model - WiFi DensePose Visualization
// Maps DensePose 24 body parts to 3D positions using simple geometries

export class BodyModel {
  // DensePose body part IDs (1-24)
  static PARTS = {
    TORSO_BACK: 1,
    TORSO_FRONT: 2,
    RIGHT_HAND: 3,
    LEFT_HAND: 4,
    LEFT_FOOT: 5,
    RIGHT_FOOT: 6,
    RIGHT_UPPER_LEG_BACK: 7,
    LEFT_UPPER_LEG_BACK: 8,
    RIGHT_UPPER_LEG_FRONT: 9,
    LEFT_UPPER_LEG_FRONT: 10,
    RIGHT_LOWER_LEG_BACK: 11,
    LEFT_LOWER_LEG_BACK: 12,
    RIGHT_LOWER_LEG_FRONT: 13,
    LEFT_LOWER_LEG_FRONT: 14,
    LEFT_UPPER_ARM_FRONT: 15,
    RIGHT_UPPER_ARM_FRONT: 16,
    LEFT_UPPER_ARM_BACK: 17,
    RIGHT_UPPER_ARM_BACK: 18,
    LEFT_LOWER_ARM_FRONT: 19,
    RIGHT_LOWER_ARM_FRONT: 20,
    LEFT_LOWER_ARM_BACK: 21,
    RIGHT_LOWER_ARM_BACK: 22,
    HEAD_RIGHT: 23,
    HEAD_LEFT: 24
  };

  // Skeleton connection pairs for drawing bones
  static BONE_CONNECTIONS = [
    // Spine
    ['pelvis', 'spine'],
    ['spine', 'chest'],
    ['chest', 'neck'],
    ['neck', 'head'],
    // Left arm
    ['chest', 'left_shoulder'],
    ['left_shoulder', 'left_elbow'],
    ['left_elbow', 'left_wrist'],
    // Right arm
    ['chest', 'right_shoulder'],
    ['right_shoulder', 'right_elbow'],
    ['right_elbow', 'right_wrist'],
    // Left leg
    ['pelvis', 'left_hip'],
    ['left_hip', 'left_knee'],
    ['left_knee', 'left_ankle'],
    // Right leg
    ['pelvis', 'right_hip'],
    ['right_hip', 'right_knee'],
    ['right_knee', 'right_ankle']
  ];

  constructor() {
    this.group = new THREE.Group();
    this.group.name = 'body-model';

    // Store references to body part meshes for updates
    this.joints = {};
    this.limbs = {};
    this.bones = [];
    this.partMeshes = {};

    // Current pose state
    this.confidence = 0;
    this.isVisible = false;
    this.targetPositions = {};
    this.currentPositions = {};

    // Materials
    this._materials = this._createMaterials();

    // Build the body
    this._buildBody();

    // Initial hidden state
    this.group.visible = false;
  }

  _createMaterials() {
    // Confidence-driven color: cold blue (low) -> warm orange (high)
    const jointMat = new THREE.MeshPhongMaterial({
      color: 0x00aaff,
      emissive: 0x003366,
      emissiveIntensity: 0.3,
      shininess: 60,
      transparent: true,
      opacity: 0.9
    });

    const limbMat = new THREE.MeshPhongMaterial({
      color: 0x0088dd,
      emissive: 0x002244,
      emissiveIntensity: 0.2,
      shininess: 40,
      transparent: true,
      opacity: 0.85
    });

    const headMat = new THREE.MeshPhongMaterial({
      color: 0x00ccff,
      emissive: 0x004466,
      emissiveIntensity: 0.4,
      shininess: 80,
      transparent: true,
      opacity: 0.9
    });

    const boneMat = new THREE.LineBasicMaterial({
      color: 0x00ffcc,
      transparent: true,
      opacity: 0.6,
      linewidth: 2
    });

    return { joint: jointMat, limb: limbMat, head: headMat, bone: boneMat };
  }

  _buildBody() {
    // Default T-pose joint positions (Y-up coordinate system)
    // Heights are in meters, approximate human proportions (1.75m tall)
    const defaultJoints = {
      head:            { x: 0, y: 1.70, z: 0 },
      neck:            { x: 0, y: 1.55, z: 0 },
      chest:           { x: 0, y: 1.35, z: 0 },
      spine:           { x: 0, y: 1.10, z: 0 },
      pelvis:          { x: 0, y: 0.90, z: 0 },
      left_shoulder:   { x: -0.22, y: 1.48, z: 0 },
      right_shoulder:  { x:  0.22, y: 1.48, z: 0 },
      left_elbow:      { x: -0.45, y: 1.20, z: 0 },
      right_elbow:     { x:  0.45, y: 1.20, z: 0 },
      left_wrist:      { x: -0.55, y: 0.95, z: 0 },
      right_wrist:     { x:  0.55, y: 0.95, z: 0 },
      left_hip:        { x: -0.12, y: 0.88, z: 0 },
      right_hip:       { x:  0.12, y: 0.88, z: 0 },
      left_knee:       { x: -0.13, y: 0.50, z: 0 },
      right_knee:      { x:  0.13, y: 0.50, z: 0 },
      left_ankle:      { x: -0.13, y: 0.08, z: 0 },
      right_ankle:     { x:  0.13, y: 0.08, z: 0 }
    };

    // Create joint spheres
    const jointGeom = new THREE.SphereGeometry(0.035, 12, 12);
    const headGeom = new THREE.SphereGeometry(0.10, 16, 16);

    for (const [name, pos] of Object.entries(defaultJoints)) {
      const geom = name === 'head' ? headGeom : jointGeom;
      const mat = name === 'head' ? this._materials.head.clone() : this._materials.joint.clone();
      const mesh = new THREE.Mesh(geom, mat);
      mesh.position.set(pos.x, pos.y, pos.z);
      mesh.castShadow = true;
      mesh.name = `joint-${name}`;
      this.group.add(mesh);
      this.joints[name] = mesh;
      this.currentPositions[name] = { ...pos };
      this.targetPositions[name] = { ...pos };
    }

    // Create limb cylinders connecting joints
    const limbDefs = [
      { name: 'torso_upper', from: 'chest', to: 'neck', radius: 0.06 },
      { name: 'torso_lower', from: 'spine', to: 'chest', radius: 0.07 },
      { name: 'hip_section', from: 'pelvis', to: 'spine', radius: 0.065 },
      { name: 'left_upper_arm', from: 'left_shoulder', to: 'left_elbow', radius: 0.03 },
      { name: 'right_upper_arm', from: 'right_shoulder', to: 'right_elbow', radius: 0.03 },
      { name: 'left_forearm', from: 'left_elbow', to: 'left_wrist', radius: 0.025 },
      { name: 'right_forearm', from: 'right_elbow', to: 'right_wrist', radius: 0.025 },
      { name: 'left_thigh', from: 'left_hip', to: 'left_knee', radius: 0.04 },
      { name: 'right_thigh', from: 'right_hip', to: 'right_knee', radius: 0.04 },
      { name: 'left_shin', from: 'left_knee', to: 'left_ankle', radius: 0.03 },
      { name: 'right_shin', from: 'right_knee', to: 'right_ankle', radius: 0.03 },
      { name: 'left_clavicle', from: 'chest', to: 'left_shoulder', radius: 0.025 },
      { name: 'right_clavicle', from: 'chest', to: 'right_shoulder', radius: 0.025 },
      { name: 'left_pelvis', from: 'pelvis', to: 'left_hip', radius: 0.03 },
      { name: 'right_pelvis', from: 'pelvis', to: 'right_hip', radius: 0.03 },
      { name: 'neck_head', from: 'neck', to: 'head', radius: 0.025 }
    ];

    for (const def of limbDefs) {
      const limb = this._createLimb(def.from, def.to, def.radius);
      limb.name = `limb-${def.name}`;
      this.group.add(limb);
      this.limbs[def.name] = { mesh: limb, from: def.from, to: def.to, radius: def.radius };
    }

    // Create skeleton bone lines
    this._createBoneLines();

    // Create body part glow meshes for DensePose part activation
    this._createPartGlows();
  }

  _createLimb(fromName, toName, radius) {
    const from = this.currentPositions[fromName];
    const to = this.currentPositions[toName];
    const dir = new THREE.Vector3(to.x - from.x, to.y - from.y, to.z - from.z);
    const length = dir.length();

    const geom = new THREE.CylinderGeometry(radius, radius, length, 8, 1);
    const mat = this._materials.limb.clone();
    const mesh = new THREE.Mesh(geom, mat);
    mesh.castShadow = true;

    this._positionLimb(mesh, from, to, length);
    return mesh;
  }

  _positionLimb(mesh, from, to, length) {
    const mid = {
      x: (from.x + to.x) / 2,
      y: (from.y + to.y) / 2,
      z: (from.z + to.z) / 2
    };
    mesh.position.set(mid.x, mid.y, mid.z);

    const dir = new THREE.Vector3(to.x - from.x, to.y - from.y, to.z - from.z).normalize();
    const up = new THREE.Vector3(0, 1, 0);

    if (Math.abs(dir.dot(up)) < 0.999) {
      const quat = new THREE.Quaternion();
      quat.setFromUnitVectors(up, dir);
      mesh.quaternion.copy(quat);
    }

    // Update the cylinder length
    mesh.scale.y = length / mesh.geometry.parameters.height;
  }

  _createBoneLines() {
    const boneGeom = new THREE.BufferGeometry();
    // We will update positions each frame
    const positions = new Float32Array(BodyModel.BONE_CONNECTIONS.length * 6);
    boneGeom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    const boneLine = new THREE.LineSegments(boneGeom, this._materials.bone);
    boneLine.name = 'skeleton-bones';
    this.group.add(boneLine);
    this._boneLine = boneLine;
  }

  _createPartGlows() {
    // Create subtle glow indicators for each DensePose body region
    // These light up based on which parts are being sensed
    const partRegions = {
      torso: { pos: [0, 1.2, 0], scale: [0.2, 0.3, 0.1], parts: [1, 2] },
      left_upper_arm: { pos: [-0.35, 1.35, 0], scale: [0.06, 0.15, 0.06], parts: [15, 17] },
      right_upper_arm: { pos: [0.35, 1.35, 0], scale: [0.06, 0.15, 0.06], parts: [16, 18] },
      left_lower_arm: { pos: [-0.50, 1.08, 0], scale: [0.05, 0.13, 0.05], parts: [19, 21] },
      right_lower_arm: { pos: [0.50, 1.08, 0], scale: [0.05, 0.13, 0.05], parts: [20, 22] },
      left_hand: { pos: [-0.55, 0.95, 0], scale: [0.04, 0.04, 0.03], parts: [4] },
      right_hand: { pos: [0.55, 0.95, 0], scale: [0.04, 0.04, 0.03], parts: [3] },
      left_upper_leg: { pos: [-0.13, 0.70, 0], scale: [0.07, 0.18, 0.07], parts: [8, 10] },
      right_upper_leg: { pos: [0.13, 0.70, 0], scale: [0.07, 0.18, 0.07], parts: [7, 9] },
      left_lower_leg: { pos: [-0.13, 0.30, 0], scale: [0.05, 0.18, 0.05], parts: [12, 14] },
      right_lower_leg: { pos: [0.13, 0.30, 0], scale: [0.05, 0.18, 0.05], parts: [11, 13] },
      left_foot: { pos: [-0.13, 0.05, 0.03], scale: [0.04, 0.03, 0.06], parts: [5] },
      right_foot: { pos: [0.13, 0.05, 0.03], scale: [0.04, 0.03, 0.06], parts: [6] },
      head: { pos: [0, 1.72, 0], scale: [0.09, 0.10, 0.09], parts: [23, 24] }
    };

    const glowGeom = new THREE.SphereGeometry(1, 8, 8);

    for (const [name, region] of Object.entries(partRegions)) {
      const mat = new THREE.MeshBasicMaterial({
        color: 0x00ffcc,
        transparent: true,
        opacity: 0,
        depthWrite: false
      });
      const mesh = new THREE.Mesh(glowGeom, mat);
      mesh.position.set(...region.pos);
      mesh.scale.set(...region.scale);
      mesh.name = `part-glow-${name}`;
      this.group.add(mesh);
      for (const partId of region.parts) {
        this.partMeshes[partId] = mesh;
      }
    }
  }

  // Update pose from keypoints array
  // keypoints: array of {x, y, confidence} in normalized [0,1] coords
  // The mapping follows COCO 17-keypoint format:
  // 0:nose, 1:left_eye, 2:right_eye, 3:left_ear, 4:right_ear,
  // 5:left_shoulder, 6:right_shoulder, 7:left_elbow, 8:right_elbow,
  // 9:left_wrist, 10:right_wrist, 11:left_hip, 12:right_hip,
  // 13:left_knee, 14:right_knee, 15:left_ankle, 16:right_ankle
  updateFromKeypoints(keypoints, personConfidence) {
    if (!keypoints || keypoints.length < 17) return;

    this.confidence = personConfidence || 0;
    this.isVisible = this.confidence > 0.15;
    this.group.visible = this.isVisible;

    if (!this.isVisible) return;

    // Map COCO keypoints to our joint positions
    // Convert normalized [0,1] to 3D space centered at origin
    // x: left-right (normalized 0-1 maps to roughly -2 to 2 meters)
    // y: up (we compute from relative positions)
    // z: depth (we derive from some heuristics)
    const kp = keypoints;

    const mapX = (val) => (val - 0.5) * 4;
    const mapZ = (val) => (val - 0.5) * 0.5; // Slight depth from x offset

    // Helper to compute a 3D position from a COCO keypoint
    const kpPos = (idx, defaultY) => {
      const k = kp[idx];
      if (!k || k.confidence < 0.1) return null;
      return {
        x: mapX(k.x),
        y: defaultY !== undefined ? defaultY : (1.75 - k.y * 1.75),
        z: mapZ(k.x) * 0.2
      };
    };

    // Estimate vertical scale from shoulder-to-ankle distance
    const lShoulder = kp[5], lAnkle = kp[15];
    let scale = 1.0;
    if (lShoulder && lAnkle && lShoulder.confidence > 0.2 && lAnkle.confidence > 0.2) {
      const pixelHeight = Math.abs(lAnkle.y - lShoulder.y);
      if (pixelHeight > 0.05) {
        scale = 0.85 / pixelHeight; // shoulder-to-ankle is about 0.85m scaled
      }
    }

    const mapY = (val) => {
      // Map y from normalized coords (0=top, 1=bottom) to world y
      // Find the lowest point (ankles) and use that as ground reference
      const groundRef = Math.max(
        (kp[15] && kp[15].confidence > 0.2) ? kp[15].y : 0.95,
        (kp[16] && kp[16].confidence > 0.2) ? kp[16].y : 0.95
      );
      return (groundRef - val) * scale * 1.75;
    };

    // Compute mid-hip as body center
    const midHipX = this._avgCoord(kp, [11, 12], 'x');
    const midHipY = this._avgCoord(kp, [11, 12], 'y');
    const centerX = midHipX !== null ? mapX(midHipX) : 0;

    // Map all joints
    const updateJoint = (name, idx, fallbackY) => {
      const k = kp[idx];
      if (k && k.confidence > 0.1) {
        this.targetPositions[name] = {
          x: mapX(k.x) - centerX,
          y: mapY(k.y),
          z: 0
        };
      }
    };

    // Head (average of nose, eyes, ears)
    const headX = this._avgCoord(kp, [0, 1, 2, 3, 4], 'x');
    const headY = this._avgCoord(kp, [0, 1, 2, 3, 4], 'y');
    if (headX !== null && headY !== null) {
      this.targetPositions.head = { x: mapX(headX) - centerX, y: mapY(headY) + 0.08, z: 0 };
    }

    // Neck (between nose and mid-shoulder)
    const midShoulderX = this._avgCoord(kp, [5, 6], 'x');
    const midShoulderY = this._avgCoord(kp, [5, 6], 'y');
    const noseK = kp[0];
    if (midShoulderX !== null && noseK && noseK.confidence > 0.1) {
      this.targetPositions.neck = {
        x: mapX((midShoulderX + noseK.x) / 2) - centerX,
        y: mapY((midShoulderY + noseK.y) / 2),
        z: 0
      };
    }

    // Chest (mid-shoulder)
    if (midShoulderX !== null) {
      this.targetPositions.chest = {
        x: mapX(midShoulderX) - centerX,
        y: mapY(midShoulderY),
        z: 0
      };
    }

    // Spine (between chest and pelvis)
    if (midShoulderX !== null && midHipX !== null) {
      this.targetPositions.spine = {
        x: mapX((midShoulderX + midHipX) / 2) - centerX,
        y: mapY((midShoulderY + midHipY) / 2),
        z: 0
      };
    }

    // Pelvis
    if (midHipX !== null) {
      this.targetPositions.pelvis = {
        x: mapX(midHipX) - centerX,
        y: mapY(midHipY),
        z: 0
      };
    }

    // Arms and legs
    updateJoint('left_shoulder', 5);
    updateJoint('right_shoulder', 6);
    updateJoint('left_elbow', 7);
    updateJoint('right_elbow', 8);
    updateJoint('left_wrist', 9);
    updateJoint('right_wrist', 10);
    updateJoint('left_hip', 11);
    updateJoint('right_hip', 12);
    updateJoint('left_knee', 13);
    updateJoint('right_knee', 14);
    updateJoint('left_ankle', 15);
    updateJoint('right_ankle', 16);

    // Adjust all positions relative to center
    // Apply global position offset (person location in room)
    // Shift the body model to world position
    this.group.position.x = centerX;
  }

  _avgCoord(keypoints, indices, coord) {
    let sum = 0;
    let count = 0;
    for (const idx of indices) {
      const k = keypoints[idx];
      if (k && k.confidence > 0.1) {
        sum += k[coord];
        count++;
      }
    }
    return count > 0 ? sum / count : null;
  }

  // Activate DensePose body part regions (parts: array of part IDs with confidence)
  activateParts(partConfidences) {
    // partConfidences: { partId: confidence, ... }
    for (const [partId, mesh] of Object.entries(this.partMeshes)) {
      const conf = partConfidences[partId] || 0;
      mesh.material.opacity = conf * 0.4;
      // Color temperature: blue (low) -> cyan -> green -> yellow -> orange (high)
      const hue = (1 - conf) * 0.55; // 0.55 = blue, 0 = red
      mesh.material.color.setHSL(hue, 1.0, 0.5 + conf * 0.2);
    }
  }

  // Smooth animation update - call each frame
  update(delta) {
    if (!this.isVisible) return;

    const lerpFactor = 1 - Math.pow(0.001, delta); // Smooth exponential lerp

    // Lerp joint positions
    for (const [name, joint] of Object.entries(this.joints)) {
      const target = this.targetPositions[name];
      const current = this.currentPositions[name];
      if (!target) continue;

      current.x += (target.x - current.x) * lerpFactor;
      current.y += (target.y - current.y) * lerpFactor;
      current.z += (target.z - current.z) * lerpFactor;

      joint.position.set(current.x, current.y, current.z);
    }

    // Update limb cylinders
    for (const limb of Object.values(this.limbs)) {
      const from = this.currentPositions[limb.from];
      const to = this.currentPositions[limb.to];
      if (!from || !to) continue;

      const dir = new THREE.Vector3(to.x - from.x, to.y - from.y, to.z - from.z);
      const length = dir.length();
      if (length < 0.001) continue;

      this._positionLimb(limb.mesh, from, to, length);
    }

    // Update bone lines
    this._updateBoneLines();

    // Update material colors based on confidence
    this._updateMaterialColors();
  }

  _updateBoneLines() {
    const posAttr = this._boneLine.geometry.getAttribute('position');
    const arr = posAttr.array;
    let i = 0;

    for (const [fromName, toName] of BodyModel.BONE_CONNECTIONS) {
      const from = this.currentPositions[fromName];
      const to = this.currentPositions[toName];
      if (from && to) {
        arr[i]     = from.x; arr[i + 1] = from.y; arr[i + 2] = from.z;
        arr[i + 3] = to.x;   arr[i + 4] = to.y;   arr[i + 5] = to.z;
      }
      i += 6;
    }
    posAttr.needsUpdate = true;
  }

  _updateMaterialColors() {
    // Confidence drives color temperature
    // Low confidence = cool blue, high = warm cyan/green
    const conf = this.confidence;
    const hue = 0.55 - conf * 0.25; // blue -> cyan -> green
    const saturation = 0.8;
    const lightness = 0.35 + conf * 0.2;

    for (const joint of Object.values(this.joints)) {
      if (joint.name !== 'joint-head') {
        joint.material.color.setHSL(hue, saturation, lightness);
        joint.material.emissive.setHSL(hue, saturation, lightness * 0.3);
        joint.material.opacity = 0.5 + conf * 0.5;
      }
    }

    for (const limb of Object.values(this.limbs)) {
      limb.mesh.material.color.setHSL(hue, saturation * 0.9, lightness * 0.9);
      limb.mesh.material.emissive.setHSL(hue, saturation * 0.9, lightness * 0.2);
      limb.mesh.material.opacity = 0.4 + conf * 0.5;
    }

    // Head
    const headJoint = this.joints.head;
    if (headJoint) {
      headJoint.material.color.setHSL(hue - 0.05, saturation, lightness + 0.1);
      headJoint.material.emissive.setHSL(hue - 0.05, saturation, lightness * 0.4);
      headJoint.material.opacity = 0.6 + conf * 0.4;
    }

    // Bone line color
    this._materials.bone.color.setHSL(hue + 0.1, 1.0, 0.5 + conf * 0.2);
    this._materials.bone.opacity = 0.3 + conf * 0.4;
  }

  // Set the world position of this body model (for multi-person scenes)
  setWorldPosition(x, y, z) {
    this.group.position.set(x, y || 0, z || 0);
  }

  getGroup() {
    return this.group;
  }

  dispose() {
    this.group.traverse((child) => {
      if (child.geometry) child.geometry.dispose();
      if (child.material) {
        if (Array.isArray(child.material)) {
          child.material.forEach(m => m.dispose());
        } else {
          child.material.dispose();
        }
      }
    });
  }
}


// Manager for multiple body models (multi-person tracking)
export class BodyModelManager {
  constructor(scene) {
    this.scene = scene;
    this.models = new Map(); // personId -> BodyModel
    this.maxModels = 6;
    this.inactiveTimeout = 3000; // ms before removing inactive model
    this.lastSeen = new Map(); // personId -> timestamp
  }

  // Update with new pose data for potentially multiple persons
  update(personsData, delta) {
    const now = Date.now();

    if (personsData && personsData.length > 0) {
      for (let i = 0; i < Math.min(personsData.length, this.maxModels); i++) {
        const person = personsData[i];
        const personId = person.id || `person_${i}`;

        // Get or create model
        let model = this.models.get(personId);
        if (!model) {
          model = new BodyModel();
          this.models.set(personId, model);
          this.scene.add(model.getGroup());
        }

        // Update the model
        if (person.keypoints) {
          model.updateFromKeypoints(person.keypoints, person.confidence);
        }

        // Activate DensePose parts if available
        if (person.body_parts) {
          model.activateParts(person.body_parts);
        }

        this.lastSeen.set(personId, now);
      }
    }

    // Animate all models
    for (const model of this.models.values()) {
      model.update(delta);
    }

    // Remove stale models
    for (const [id, lastTime] of this.lastSeen.entries()) {
      if (now - lastTime > this.inactiveTimeout) {
        const model = this.models.get(id);
        if (model) {
          this.scene.remove(model.getGroup());
          model.dispose();
          this.models.delete(id);
          this.lastSeen.delete(id);
        }
      }
    }
  }

  getActiveCount() {
    return this.models.size;
  }

  getAverageConfidence() {
    if (this.models.size === 0) return 0;
    let sum = 0;
    for (const model of this.models.values()) {
      sum += model.confidence;
    }
    return sum / this.models.size;
  }

  dispose() {
    for (const model of this.models.values()) {
      this.scene.remove(model.getGroup());
      model.dispose();
    }
    this.models.clear();
    this.lastSeen.clear();
  }
}
