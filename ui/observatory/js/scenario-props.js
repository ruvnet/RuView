/**
 * ScenarioProps — Scenario-specific room furniture and props
 *
 * Extracted from main.js. Builds and manages visibility of all physical
 * objects that appear/disappear based on the active scenario: bed, chair,
 * exercise mat, door, rubble wall, screen/TV, desks, security cameras,
 * and the alert light system.
 */
import * as THREE from 'three';

// Scenario-to-prop-name mapping
const SCENARIO_PROPS = {
  empty_room:       [],
  single_breathing: [],
  two_walking:      [],
  fall_event:       [],
  sleep_monitoring: ['bed'],
  intrusion_detect: ['door'],
  gesture_control:  ['screen'],
  crowd_occupancy:  ['desk', 'desk2'],
  search_rescue:    ['rubbleWall'],
  elderly_care:     ['chair'],
  fitness_tracking: ['exerciseMat'],
  security_patrol:  ['camera1', 'camera2'],
};

export class ScenarioProps {
  constructor(scene) {
    this._scene = scene;
    this._props = {};
    this._currentScenario = null;
    this._alertLight = null;
    this._alertIntensity = 0;

    // Animatable references
    this._screenGlow = null;
    this._camera1Group = null;
    this._camera2Group = null;
    this._cam1Cone = null;
    this._cam2Cone = null;
    this._cam1Led = null;
    this._cam2Led = null;
    this._dustParticles = null;
    this._doorSpotlight = null;
    this._alarmHousing = null;
    this._powerLed = null;

    this._build();
  }

  // ---- helper: positioned box with shadow ----
  _box(x, y, z, w, h, d, mat) {
    const m = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), mat);
    m.position.set(x, y, z);
    m.castShadow = true;
    m.receiveShadow = true;
    return m;
  }

  // ---- helper: positioned cylinder with shadow ----
  _cyl(x, y, z, rTop, rBot, h, segs, mat) {
    const m = new THREE.Mesh(new THREE.CylinderGeometry(rTop, rBot, h, segs), mat);
    m.position.set(x, y, z);
    m.castShadow = true;
    m.receiveShadow = true;
    return m;
  }

  // ========================================
  //  BUILD ALL PROPS
  // ========================================

  _build() {
    const darkMat  = new THREE.MeshStandardMaterial({ color: 0x6b5840, roughness: 0.6, emissive: 0x1a1408, emissiveIntensity: 0.25 });
    const metalMat = new THREE.MeshStandardMaterial({ color: 0x808088, roughness: 0.3, metalness: 0.7, emissive: 0x1a1a20, emissiveIntensity: 0.2 });
    const accentMat = new THREE.MeshStandardMaterial({ color: 0x606070, roughness: 0.4, metalness: 0.4, emissive: 0x101018, emissiveIntensity: 0.15 });

    this._buildBed(darkMat);
    this._buildChair(darkMat, accentMat);
    this._buildExerciseMat();
    this._buildDoor();
    this._buildRubbleWall();
    this._buildScreen(metalMat);
    this._buildDesks(darkMat, metalMat, accentMat);
    this._buildCameras(metalMat);
    this._buildAlertSystem();
  }

  // ---- BED (sleep monitoring) ----
  _buildBed(darkMat) {
    const bedGroup = new THREE.Group();

    // Bed frame with legs
    const frameMat = new THREE.MeshStandardMaterial({ color: 0x7a6448, roughness: 0.55, metalness: 0.25, emissive: 0x181008, emissiveIntensity: 0.25 });
    const bedFrame = new THREE.Mesh(new THREE.BoxGeometry(2.2, 0.12, 1.2), frameMat);
    bedFrame.position.set(3.5, 0.32, -3.5);
    bedFrame.castShadow = true;
    bedGroup.add(bedFrame);

    // Frame legs (4 short posts)
    for (const [lx, lz] of [[2.5, -4.0], [4.5, -4.0], [2.5, -3.0], [4.5, -3.0]]) {
      bedGroup.add(this._cyl(lx, 0.13, lz, 0.04, 0.04, 0.26, 6, frameMat));
    }

    // Headboard — tall panel at head of bed
    const headboardMat = new THREE.MeshStandardMaterial({ color: 0x6a5440, roughness: 0.65, emissive: 0x140e08, emissiveIntensity: 0.2 });
    const headboard = new THREE.Mesh(new THREE.BoxGeometry(0.08, 0.7, 1.2), headboardMat);
    headboard.position.set(2.38, 0.65, -3.5);
    headboard.castShadow = true;
    bedGroup.add(headboard);

    // Mattress
    const mattressMat = new THREE.MeshStandardMaterial({ color: 0x484860, roughness: 0.75, emissive: 0x0c0c1a, emissiveIntensity: 0.2 });
    const mattress = new THREE.Mesh(new THREE.BoxGeometry(2.0, 0.15, 1.1), mattressMat);
    mattress.position.set(3.5, 0.455, -3.5);
    mattress.castShadow = true;
    bedGroup.add(mattress);

    // Wrinkled sheet — wave-displaced plane
    const sheetGeo = new THREE.PlaneGeometry(1.4, 1.0, 20, 20);
    const posAttr = sheetGeo.getAttribute('position');
    for (let i = 0; i < posAttr.count; i++) {
      const px = posAttr.getX(i);
      const py = posAttr.getY(i);
      posAttr.setZ(i, Math.sin(px * 4) * 0.015 + Math.cos(py * 5) * 0.01 + Math.sin(px * py * 3) * 0.008);
    }
    posAttr.needsUpdate = true;
    sheetGeo.computeVertexNormals();
    const sheetMat = new THREE.MeshStandardMaterial({
      color: 0x506880, roughness: 0.75, side: THREE.DoubleSide, emissive: 0x0c1018, emissiveIntensity: 0.2,
    });
    const sheet = new THREE.Mesh(sheetGeo, sheetMat);
    sheet.rotation.x = -Math.PI / 2;
    sheet.position.set(3.7, 0.54, -3.5);
    sheet.castShadow = true;
    bedGroup.add(sheet);

    // Pillow — soft shape using scaled sphere
    const pillowGeo = new THREE.SphereGeometry(0.18, 12, 8);
    pillowGeo.scale(1, 0.35, 1.4);
    const pillowMat = new THREE.MeshStandardMaterial({ color: 0x706868, roughness: 0.7, emissive: 0x141010, emissiveIntensity: 0.2 });
    const pillow = new THREE.Mesh(pillowGeo, pillowMat);
    pillow.position.set(2.65, 0.52, -3.5);
    pillow.castShadow = true;
    bedGroup.add(pillow);

    // Bedside lamp — small cylinder + sphere shade on a tiny table
    const lampBaseMat = new THREE.MeshStandardMaterial({ color: 0x686870, roughness: 0.3, metalness: 0.7, emissive: 0x101018, emissiveIntensity: 0.15 });
    // Nightstand
    bedGroup.add(this._box(2.15, 0.25, -3.5, 0.35, 0.5, 0.35, darkMat));
    // Lamp base
    bedGroup.add(this._cyl(2.15, 0.55, -3.5, 0.04, 0.05, 0.1, 8, lampBaseMat));
    // Lamp stem
    bedGroup.add(this._cyl(2.15, 0.68, -3.5, 0.015, 0.015, 0.2, 6, lampBaseMat));
    // Lamp shade (emissive warm glow)
    const shadeMat = new THREE.MeshStandardMaterial({
      color: 0x705830, emissive: 0x604018, emissiveIntensity: 1.0, roughness: 0.6,
      side: THREE.DoubleSide, transparent: true, opacity: 0.9,
    });
    const shade = new THREE.Mesh(new THREE.ConeGeometry(0.08, 0.1, 8, 1, true), shadeMat);
    shade.position.set(2.15, 0.78, -3.5);
    shade.rotation.x = Math.PI;
    bedGroup.add(shade);

    // Warm lamp light
    const lampLight = new THREE.PointLight(0xffcc88, 2.0, 6, 1.2);
    lampLight.position.set(2.15, 0.78, -3.5);
    bedGroup.add(lampLight);

    this._props.bed = bedGroup;
    bedGroup.visible = false;
    this._scene.add(bedGroup);
  }

  // ---- CHAIR (elderly care) ----
  _buildChair(darkMat, accentMat) {
    const chairGroup = new THREE.Group();
    chairGroup.position.set(1, 0, -1.5);

    const cushionMat = new THREE.MeshStandardMaterial({ color: 0x5a5078, roughness: 0.7, emissive: 0x10101a, emissiveIntensity: 0.2 });

    // Seat
    chairGroup.add(this._box(0, 0.45, 0, 0.5, 0.04, 0.45, darkMat));
    // Seat cushion — slightly puffy
    const cushionGeo = new THREE.BoxGeometry(0.46, 0.06, 0.42);
    // Gentle puff on top vertices
    const cPos = cushionGeo.getAttribute('position');
    for (let i = 0; i < cPos.count; i++) {
      if (cPos.getY(i) > 0) {
        const dx = cPos.getX(i) / 0.23;
        const dz = cPos.getZ(i) / 0.21;
        cPos.setY(i, cPos.getY(i) + 0.015 * (1 - dx * dx) * (1 - dz * dz));
      }
    }
    cPos.needsUpdate = true;
    cushionGeo.computeVertexNormals();
    const cushion = new THREE.Mesh(cushionGeo, cushionMat);
    cushion.position.set(0, 0.50, 0);
    cushion.castShadow = true;
    chairGroup.add(cushion);

    // Back
    chairGroup.add(this._box(0, 0.72, -0.22, 0.5, 0.5, 0.04, darkMat));
    // Legs
    for (const [lx, lz] of [[-0.22, -0.2], [0.22, -0.2], [-0.22, 0.2], [0.22, 0.2]]) {
      chairGroup.add(this._box(lx, 0.22, lz, 0.04, 0.44, 0.04, darkMat));
    }
    // Armrests
    chairGroup.add(this._box(-0.28, 0.6, 0, 0.04, 0.04, 0.4, accentMat));
    chairGroup.add(this._box(0.28, 0.6, 0, 0.04, 0.04, 0.4, accentMat));
    // Armrest supports
    chairGroup.add(this._box(-0.28, 0.52, -0.18, 0.04, 0.12, 0.04, accentMat));
    chairGroup.add(this._box(0.28, 0.52, -0.18, 0.04, 0.12, 0.04, accentMat));

    // Small side table
    const tableMat = new THREE.MeshStandardMaterial({ color: 0x685840, roughness: 0.55, emissive: 0x14100a, emissiveIntensity: 0.2 });
    chairGroup.add(this._box(0.65, 0.3, 0, 0.35, 0.03, 0.35, tableMat));
    // Table legs
    for (const [tx, tz] of [[0.5, -0.14], [0.8, -0.14], [0.5, 0.14], [0.8, 0.14]]) {
      chairGroup.add(this._cyl(tx, 0.15, tz, 0.015, 0.015, 0.28, 6, tableMat));
    }

    this._props.chair = chairGroup;
    chairGroup.visible = false;
    this._scene.add(chairGroup);
  }

  // ---- EXERCISE MAT (fitness tracking) ----
  _buildExerciseMat() {
    const matGroup = new THREE.Group();
    const matMat = new THREE.MeshStandardMaterial({ color: 0x408858, roughness: 0.75, emissive: 0x0c2010, emissiveIntensity: 0.25 });

    // Mat body
    const exerciseMat = new THREE.Mesh(new THREE.BoxGeometry(1.8, 0.015, 0.8), matMat);
    exerciseMat.position.set(0, 0.008, 0);
    exerciseMat.receiveShadow = true;
    matGroup.add(exerciseMat);

    // Boundary lines on the mat (thin strips)
    const lineMat = new THREE.MeshStandardMaterial({ color: 0x50a068, roughness: 0.7, emissive: 0x102818, emissiveIntensity: 0.3 });
    // Longitudinal borders
    matGroup.add(this._box(0, 0.017, -0.37, 1.7, 0.003, 0.02, lineMat));
    matGroup.add(this._box(0, 0.017, 0.37, 1.7, 0.003, 0.02, lineMat));
    // Cross lines (exercise area markers)
    for (const xOff of [-0.6, 0, 0.6]) {
      matGroup.add(this._box(xOff, 0.017, 0, 0.02, 0.003, 0.74, lineMat));
    }

    // Water bottle (cylinder body + hemisphere cap)
    const bottleMat = new THREE.MeshStandardMaterial({ color: 0x4878a8, roughness: 0.2, metalness: 0.7, emissive: 0x0c1828, emissiveIntensity: 0.25 });
    const bottleBody = new THREE.Mesh(new THREE.CylinderGeometry(0.035, 0.035, 0.18, 10), bottleMat);
    bottleBody.position.set(1.1, 0.09, 0.25);
    bottleBody.castShadow = true;
    matGroup.add(bottleBody);
    const bottleCap = new THREE.Mesh(new THREE.SphereGeometry(0.035, 8, 6, 0, Math.PI * 2, 0, Math.PI / 2), bottleMat);
    bottleCap.position.set(1.1, 0.18, 0.25);
    matGroup.add(bottleCap);
    // Bottle neck
    const neckMat = new THREE.MeshStandardMaterial({ color: 0x587088, roughness: 0.3, metalness: 0.6, emissive: 0x0c1420, emissiveIntensity: 0.2 });
    matGroup.add(this._cyl(1.1, 0.21, 0.25, 0.018, 0.025, 0.04, 8, neckMat));

    // Small towel (flat draped box)
    const towelMat = new THREE.MeshStandardMaterial({ color: 0x686890, roughness: 0.75, emissive: 0x101020, emissiveIntensity: 0.2 });
    const towel = this._box(1.1, 0.01, -0.25, 0.3, 0.008, 0.15, towelMat);
    towel.rotation.y = 0.15;
    matGroup.add(towel);

    this._props.exerciseMat = matGroup;
    matGroup.visible = false;
    this._scene.add(matGroup);
  }

  // ---- DOOR (intrusion detection) ----
  _buildDoor() {
    const doorGroup = new THREE.Group();
    doorGroup.position.set(-5.5, 0, -1);
    const doorMat = new THREE.MeshStandardMaterial({ color: 0x7a6040, roughness: 0.5, emissive: 0x18140a, emissiveIntensity: 0.25 });
    const hingeMat = new THREE.MeshStandardMaterial({ color: 0x909098, roughness: 0.2, metalness: 0.85, emissive: 0x181820, emissiveIntensity: 0.15 });

    // Left jamb
    doorGroup.add(this._box(-0.45, 1.1, 0, 0.08, 2.2, 0.15, doorMat));
    // Right jamb
    doorGroup.add(this._box(0.45, 1.1, 0, 0.08, 2.2, 0.15, doorMat));
    // Top
    doorGroup.add(this._box(0, 2.2, 0, 0.98, 0.08, 0.15, doorMat));
    // Door panel (partially open)
    const doorPanel = new THREE.Mesh(new THREE.BoxGeometry(0.85, 2.1, 0.04), doorMat);
    doorPanel.position.set(0.2, 1.05, -0.2);
    doorPanel.rotation.y = -0.7;
    doorPanel.castShadow = true;
    doorGroup.add(doorPanel);

    // Door handle (torus)
    const handleMat = new THREE.MeshStandardMaterial({ color: 0xaaaaB0, roughness: 0.1, metalness: 0.9, emissive: 0x1a1a20, emissiveIntensity: 0.2 });
    const handle = new THREE.Mesh(new THREE.TorusGeometry(0.035, 0.008, 6, 12), handleMat);
    // Position on the door panel (relative to panel pivot)
    handle.position.set(0.48, 1.05, -0.22);
    handle.rotation.y = -0.7;
    handle.rotation.x = Math.PI / 2;
    doorGroup.add(handle);

    // Hinge details — small cylinders at jamb
    for (const hy of [0.4, 1.1, 1.8]) {
      const hinge = new THREE.Mesh(new THREE.CylinderGeometry(0.015, 0.015, 0.06, 6), hingeMat);
      hinge.position.set(-0.42, hy, 0.06);
      doorGroup.add(hinge);
    }

    // Light spill through the gap — spotlight from outside
    const doorSpot = new THREE.SpotLight(0x88aacc, 3.0, 10, Math.PI / 4, 0.3, 0.6);
    doorSpot.position.set(-0.8, 1.2, -0.5);
    doorSpot.target.position.set(0.5, 0, 0.5);
    doorGroup.add(doorSpot);
    doorGroup.add(doorSpot.target);
    this._doorSpotlight = doorSpot;

    // Window next to door — simple frame with translucent pane
    const windowFrame = new THREE.MeshStandardMaterial({ color: 0x686878, roughness: 0.35, metalness: 0.6, emissive: 0x101018, emissiveIntensity: 0.15 });
    // Frame
    doorGroup.add(this._box(1.2, 1.5, 0, 0.04, 0.8, 0.06, windowFrame));
    doorGroup.add(this._box(1.2, 1.5, 0, 0.6, 0.04, 0.06, windowFrame));
    doorGroup.add(this._box(0.92, 1.5, 0, 0.04, 0.8, 0.06, windowFrame));
    doorGroup.add(this._box(1.48, 1.5, 0, 0.04, 0.8, 0.06, windowFrame));
    doorGroup.add(this._box(1.2, 1.1, 0, 0.6, 0.04, 0.06, windowFrame));
    doorGroup.add(this._box(1.2, 1.9, 0, 0.6, 0.04, 0.06, windowFrame));
    // Glass pane
    const glassMat = new THREE.MeshStandardMaterial({
      color: 0x305880, transparent: true, opacity: 0.4, roughness: 0.05, metalness: 0.3, emissive: 0x0c1830, emissiveIntensity: 0.35,
    });
    const glass = new THREE.Mesh(new THREE.BoxGeometry(0.52, 0.72, 0.01), glassMat);
    glass.position.set(1.2, 1.5, 0);
    doorGroup.add(glass);

    this._props.door = doorGroup;
    doorGroup.visible = false;
    this._scene.add(doorGroup);
  }

  // ---- RUBBLE WALL (search & rescue) ----
  _buildRubbleWall() {
    const rubbleGroup = new THREE.Group();
    const rubbleMat = new THREE.MeshStandardMaterial({ color: 0x807868, roughness: 0.75, emissive: 0x181610, emissiveIntensity: 0.25 });
    const rebarMat = new THREE.MeshStandardMaterial({ color: 0x8a7858, roughness: 0.4, metalness: 0.7, emissive: 0x1a1408, emissiveIntensity: 0.2 });

    // Broken wall — main slab
    rubbleGroup.add(this._box(2, 1, 0, 0.4, 2, 3, rubbleMat));

    // Wall crack lines (thin dark boxes embedded in wall surface)
    const crackMat = new THREE.MeshStandardMaterial({ color: 0x403828, roughness: 0.9 });
    const cracks = [
      [1.82, 1.4, -0.3, 0.01, 0.6, 0.02, 0.3],
      [1.82, 0.8, 0.5, 0.01, 0.5, 0.02, -0.2],
      [1.82, 1.6, 0.8, 0.01, 0.4, 0.02, 0.15],
      [1.82, 0.5, -0.7, 0.01, 0.35, 0.02, -0.25],
    ];
    for (const [cx, cy, cz, cw, ch, cd, rot] of cracks) {
      const crack = this._box(cx, cy, cz, cw, ch, cd, crackMat);
      crack.rotation.z = rot;
      rubbleGroup.add(crack);
    }

    // Rebar — thin metal cylinders protruding from the wall
    for (const [rx, ry, rz, rLen, rRot] of [
      [1.6, 1.7, -0.4, 0.8, 0.3],
      [1.5, 1.2, 0.6, 0.6, -0.2],
      [1.7, 0.9, -0.8, 0.5, 0.5],
      [1.55, 1.5, 1.0, 0.7, -0.4],
    ]) {
      const rebar = new THREE.Mesh(new THREE.CylinderGeometry(0.012, 0.012, rLen, 6), rebarMat);
      rebar.position.set(rx, ry, rz);
      rebar.rotation.z = Math.PI / 2 + rRot;
      rebar.rotation.y = rRot * 0.5;
      rebar.castShadow = true;
      rubbleGroup.add(rebar);
    }

    // Rubble pieces — more varied with random rotations
    const rubbleColors = [0x807868, 0x706860, 0x908878, 0x686058];
    for (let i = 0; i < 10; i++) {
      const s = 0.12 + Math.random() * 0.3;
      const rMat = new THREE.MeshStandardMaterial({
        color: rubbleColors[i % rubbleColors.length], roughness: 0.7 + Math.random() * 0.15,
        emissive: 0x141210, emissiveIntensity: 0.2,
      });
      const piece = this._box(
        1.3 + Math.random() * 1.4, s / 2, -1.5 + Math.random() * 3,
        s, s * (0.4 + Math.random() * 0.5), s * (0.6 + Math.random() * 0.4), rMat
      );
      piece.rotation.x = (Math.random() - 0.5) * 0.6;
      piece.rotation.y = (Math.random() - 0.5) * 1.2;
      piece.rotation.z = (Math.random() - 0.5) * 0.4;
      rubbleGroup.add(piece);
    }

    // Dust particles near rubble
    const dustCount = 60;
    const dustGeo = new THREE.BufferGeometry();
    const dustPositions = new Float32Array(dustCount * 3);
    for (let i = 0; i < dustCount; i++) {
      dustPositions[i * 3]     = 1.0 + Math.random() * 2.0;
      dustPositions[i * 3 + 1] = Math.random() * 2.5;
      dustPositions[i * 3 + 2] = -1.5 + Math.random() * 3.0;
    }
    dustGeo.setAttribute('position', new THREE.BufferAttribute(dustPositions, 3));
    const dustMaterial = new THREE.PointsMaterial({
      color: 0xaa9988, size: 0.03, transparent: true, opacity: 0.5,
      blending: THREE.AdditiveBlending, depthWrite: false,
    });
    this._dustParticles = new THREE.Points(dustGeo, dustMaterial);
    rubbleGroup.add(this._dustParticles);

    this._props.rubbleWall = rubbleGroup;
    rubbleGroup.visible = false;
    this._scene.add(rubbleGroup);
  }

  // ---- SCREEN / TV (gesture control) ----
  _buildScreen(metalMat) {
    const screenGroup = new THREE.Group();
    const screenFrame = new THREE.MeshStandardMaterial({ color: 0x484850, roughness: 0.2, metalness: 0.7, emissive: 0x0c0c14, emissiveIntensity: 0.15 });

    // Frame
    screenGroup.add(this._box(0, 1.5, -4.7, 1.8, 1.1, 0.06, screenFrame));
    // Screen surface (emissive, color shifts in update())
    const screenSurfMat = new THREE.MeshStandardMaterial({
      color: 0x1a3868, emissive: 0x1a3868, emissiveIntensity: 1.2, roughness: 0.1,
    });
    const screenSurf = new THREE.Mesh(new THREE.BoxGeometry(1.6, 0.9, 0.02), screenSurfMat);
    screenSurf.position.set(0, 1.5, -4.66);
    screenGroup.add(screenSurf);
    this._screenGlow = screenSurfMat;

    // Stand / mount — neck + base
    screenGroup.add(this._box(0, 0.88, -4.7, 0.08, 0.16, 0.08, screenFrame));
    screenGroup.add(this._box(0, 0.78, -4.7, 0.4, 0.03, 0.2, metalMat));

    // Power LED indicator
    const ledMat = new THREE.MeshStandardMaterial({
      color: 0x00ff40, emissive: 0x00ff40, emissiveIntensity: 1.0,
    });
    const powerLed = new THREE.Mesh(new THREE.SphereGeometry(0.012, 6, 4), ledMat);
    powerLed.position.set(0.82, 0.96, -4.66);
    screenGroup.add(powerLed);
    this._powerLed = ledMat;

    // Subtle screen glow (point light)
    const screenLight = new THREE.PointLight(0x4080e0, 1.5, 6);
    screenLight.position.set(0, 1.5, -4.5);
    screenGroup.add(screenLight);

    // Media console below the screen
    const consoleMat = new THREE.MeshStandardMaterial({ color: 0x484858, roughness: 0.45, metalness: 0.5, emissive: 0x0c0c14, emissiveIntensity: 0.15 });
    screenGroup.add(this._box(0, 0.55, -4.7, 1.2, 0.35, 0.35, consoleMat));
    // Console shelf divider
    screenGroup.add(this._box(0, 0.55, -4.54, 1.1, 0.02, 0.01, metalMat));

    this._props.screen = screenGroup;
    screenGroup.visible = false;
    this._scene.add(screenGroup);
  }

  // ---- DESKS (crowd / office) ----
  _buildDesks(darkMat, metalMat, accentMat) {
    // Desk 1 (left)
    const deskGroup = new THREE.Group();
    deskGroup.add(this._box(-2, 0.38, -1, 1.2, 0.04, 0.6, darkMat));
    for (const [lx, lz] of [[-2.55, -1.25], [-1.45, -1.25], [-2.55, -0.75], [-1.45, -0.75]]) {
      deskGroup.add(this._box(lx, 0.19, lz, 0.04, 0.38, 0.04, darkMat));
    }
    // Monitor on desk 1
    const monitorMat = new THREE.MeshStandardMaterial({ color: 0x484850, roughness: 0.2, metalness: 0.7, emissive: 0x0c0c14, emissiveIntensity: 0.15 });
    const monScreenMat = new THREE.MeshStandardMaterial({
      color: 0x183858, emissive: 0x183858, emissiveIntensity: 1.0, roughness: 0.1,
    });
    deskGroup.add(this._box(-2, 0.62, -1.15, 0.5, 0.35, 0.03, monitorMat));
    deskGroup.add(this._box(-2, 0.62, -1.13, 0.44, 0.29, 0.01, monScreenMat));
    deskGroup.add(this._box(-2, 0.42, -1.1, 0.06, 0.04, 0.06, metalMat)); // stand neck
    deskGroup.add(this._box(-2, 0.40, -1.05, 0.18, 0.01, 0.12, metalMat)); // stand base
    // Keyboard outline
    deskGroup.add(this._box(-2, 0.405, -0.85, 0.35, 0.008, 0.12, accentMat));
    // Office chair at desk 1
    this._buildOfficeChair(deskGroup, -2, -0.55, darkMat, metalMat);

    // Monitor glow light
    const monLight = new THREE.PointLight(0x4080e0, 1.2, 4);
    monLight.position.set(-2, 0.7, -1.0);
    deskGroup.add(monLight);

    this._props.desk = deskGroup;
    deskGroup.visible = false;
    this._scene.add(deskGroup);

    // Desk 2 (right)
    const desk2Group = new THREE.Group();
    desk2Group.add(this._box(2, 0.38, 1, 1.0, 0.04, 0.6, darkMat));
    for (const [lx, lz] of [[1.45, 0.75], [2.55, 0.75], [1.45, 1.25], [2.55, 1.25]]) {
      desk2Group.add(this._box(lx, 0.19, lz, 0.04, 0.38, 0.04, darkMat));
    }
    // Monitor on desk 2
    desk2Group.add(this._box(2, 0.62, 1.15, 0.5, 0.35, 0.03, monitorMat));
    desk2Group.add(this._box(2, 0.62, 1.17, 0.44, 0.29, 0.01, monScreenMat));
    desk2Group.add(this._box(2, 0.42, 1.1, 0.06, 0.04, 0.06, metalMat));
    desk2Group.add(this._box(2, 0.40, 1.05, 0.18, 0.01, 0.12, metalMat));
    // Keyboard
    desk2Group.add(this._box(2, 0.405, 0.85, 0.35, 0.008, 0.12, accentMat));
    // Office chair at desk 2
    this._buildOfficeChair(desk2Group, 2, 0.55, darkMat, metalMat);

    // Water cooler / plant between desks area
    const plantMat = new THREE.MeshStandardMaterial({ color: 0x2a7838, roughness: 0.7, emissive: 0x0c2810, emissiveIntensity: 0.3 });
    const potMat = new THREE.MeshStandardMaterial({ color: 0x706858, roughness: 0.6, emissive: 0x14120c, emissiveIntensity: 0.15 });
    desk2Group.add(this._cyl(3.2, 0.15, 0, 0.12, 0.1, 0.3, 8, potMat));
    // Foliage — cluster of small spheres
    for (const [fx, fy, fz] of [[3.2, 0.45, 0], [3.15, 0.4, 0.06], [3.25, 0.42, -0.05]]) {
      const leaf = new THREE.Mesh(new THREE.SphereGeometry(0.08, 6, 5), plantMat);
      leaf.position.set(fx, fy, fz);
      desk2Group.add(leaf);
    }

    // Monitor glow light
    const monLight2 = new THREE.PointLight(0x4080e0, 1.2, 4);
    monLight2.position.set(2, 0.7, 1.0);
    desk2Group.add(monLight2);

    this._props.desk2 = desk2Group;
    desk2Group.visible = false;
    this._scene.add(desk2Group);
  }

  // Helper: small office chair
  _buildOfficeChair(parent, x, z, darkMat, metalMat) {
    // Seat
    parent.add(this._box(x, 0.38, z, 0.35, 0.03, 0.35, darkMat));
    // Backrest
    parent.add(this._box(x, 0.55, z - 0.16, 0.32, 0.3, 0.03, darkMat));
    // Central post
    parent.add(this._cyl(x, 0.22, z, 0.025, 0.025, 0.28, 6, metalMat));
    // Base star (5 legs)
    for (let i = 0; i < 5; i++) {
      const angle = (i / 5) * Math.PI * 2;
      const legLen = 0.16;
      const leg = this._box(
        x + Math.cos(angle) * legLen * 0.5, 0.04, z + Math.sin(angle) * legLen * 0.5,
        legLen, 0.015, 0.025, metalMat
      );
      leg.rotation.y = -angle;
      parent.add(leg);
    }
  }

  // ---- SECURITY CAMERAS (patrol) ----
  _buildCameras(metalMat) {
    const camData = [
      ['camera1', [5, 3.5, -4.5]],
      ['camera2', [-5, 3.5, 4.5]],
    ];

    for (const [name, pos] of camData) {
      const camGroup = new THREE.Group();
      camGroup.position.set(...pos);

      // Camera body
      camGroup.add(this._box(0, 0, 0, 0.15, 0.1, 0.2, metalMat));

      // Lens
      const lens = new THREE.Mesh(new THREE.CylinderGeometry(0.04, 0.04, 0.08, 8), metalMat);
      lens.rotation.x = Math.PI / 2;
      lens.position.z = 0.14;
      camGroup.add(lens);

      // Bracket / mount arm
      camGroup.add(this._box(0, 0.1, -0.08, 0.04, 0.2, 0.04, metalMat));

      // Rotating motor housing (visible joint)
      const motorMat = new THREE.MeshStandardMaterial({ color: 0x686870, roughness: 0.35, metalness: 0.8, emissive: 0x141418, emissiveIntensity: 0.15 });
      const motor = new THREE.Mesh(new THREE.CylinderGeometry(0.03, 0.03, 0.04, 8), motorMat);
      motor.position.set(0, 0.05, -0.08);
      camGroup.add(motor);

      // FOV cone (semi-transparent)
      const coneMat = new THREE.MeshStandardMaterial({
        color: 0xff3040, transparent: true, opacity: 0.15,
        side: THREE.DoubleSide, depthWrite: false,
        emissive: 0xff2020, emissiveIntensity: 0.3,
      });
      const cone = new THREE.Mesh(new THREE.ConeGeometry(1.5, 3, 16, 1, true), coneMat);
      cone.rotation.x = Math.PI / 2;
      cone.position.z = 1.7;
      camGroup.add(cone);

      // Status LED (blinks in update)
      const ledMat = new THREE.MeshStandardMaterial({
        color: 0xff2020, emissive: 0xff2020, emissiveIntensity: 1.0,
      });
      const led = new THREE.Mesh(new THREE.SphereGeometry(0.015, 6, 4), ledMat);
      led.position.set(0.08, 0.04, 0.08);
      camGroup.add(led);

      this._props[name] = camGroup;
      camGroup.visible = false;
      this._scene.add(camGroup);

      // Store references for animation
      if (name === 'camera1') {
        this._camera1Group = camGroup;
        this._cam1Cone = cone;
        this._cam1Led = ledMat;
      } else {
        this._camera2Group = camGroup;
        this._cam2Cone = cone;
        this._cam2Led = ledMat;
      }
    }
  }

  // ---- ALERT SYSTEM ----
  _buildAlertSystem() {
    // Main alert point light
    this._alertLight = new THREE.PointLight(0xff3040, 0, 10);
    this._alertLight.position.set(0, 3.5, 0);
    this._scene.add(this._alertLight);

    // Ceiling-mounted alarm housing
    const housingMat = new THREE.MeshStandardMaterial({ color: 0x686878, roughness: 0.35, metalness: 0.6, emissive: 0x101018, emissiveIntensity: 0.15 });
    const housing = new THREE.Group();
    // Base plate
    housing.add(this._box(0, 3.95, 0, 0.2, 0.02, 0.2, housingMat));
    // Housing body
    housing.add(this._cyl(0, 3.85, 0, 0.08, 0.1, 0.16, 8, housingMat));
    // Alarm lens (red when active, dark when inactive)
    const lensMat = new THREE.MeshStandardMaterial({
      color: 0x330808, emissive: 0x000000, emissiveIntensity: 0, roughness: 0.2,
      transparent: true, opacity: 0.8,
    });
    const alarmLens = new THREE.Mesh(new THREE.SphereGeometry(0.06, 10, 8, 0, Math.PI * 2, 0, Math.PI / 2), lensMat);
    alarmLens.position.set(0, 3.76, 0);
    alarmLens.rotation.x = Math.PI;
    housing.add(alarmLens);

    this._alarmHousing = housing;
    this._alarmLensMat = lensMat;
    this._scene.add(housing);
  }

  // ========================================
  //  UPDATE (called every frame)
  // ========================================

  update(data, currentScenario) {
    const scenario = data?.scenario || currentScenario;
    const elapsed = Date.now() * 0.001;

    // Switch visible props when scenario changes
    if (scenario !== this._currentScenario) {
      this._currentScenario = scenario;
      for (const prop of Object.values(this._props)) prop.visible = false;
      const propsToShow = SCENARIO_PROPS[scenario] || [];
      for (const name of propsToShow) {
        if (this._props[name]) this._props[name].visible = true;
      }
    }

    // --- Alert light (fall / intrusion) ---
    const cls = data?.classification || {};
    if (cls.fall_detected || cls.intrusion) {
      this._alertIntensity = Math.min(2, this._alertIntensity + 0.1);
    } else {
      this._alertIntensity = Math.max(0, this._alertIntensity - 0.05);
    }
    // Sawtooth pattern for urgency instead of smooth sine
    const alertPhase = (elapsed * 3) % 1.0;
    const sawtooth = alertPhase < 0.5 ? alertPhase * 2 : 2 - alertPhase * 2;
    this._alertLight.intensity = this._alertIntensity * sawtooth;

    // Alarm housing lens glow tracks alert
    if (this._alarmLensMat) {
      const alertFrac = Math.min(this._alertIntensity / 2, 1);
      this._alarmLensMat.emissive.setHex(alertFrac > 0.05 ? 0xff2020 : 0x000000);
      this._alarmLensMat.emissiveIntensity = alertFrac * sawtooth;
    }

    // Subtle ambient color shift during alerts
    if (this._alertIntensity > 0.1 && this._alertLight) {
      const r = 0.08 + 0.04 * sawtooth * this._alertIntensity;
      const g = 0.05 - 0.02 * this._alertIntensity;
      const b = 0.10 - 0.04 * this._alertIntensity;
      // Shift the alert light color slightly over time
      this._alertLight.color.setRGB(
        Math.max(0, Math.min(1, 1.0)),
        Math.max(0, Math.min(1, 0.15 - 0.1 * sawtooth)),
        Math.max(0, Math.min(1, 0.2 - 0.15 * sawtooth))
      );
    } else if (this._alertLight) {
      this._alertLight.color.setHex(0xff3040);
    }

    // --- Camera rotation animation ---
    if (this._camera1Group && this._camera1Group.visible) {
      this._camera1Group.rotation.y = Math.sin(elapsed * 0.4) * 0.5;
    }
    if (this._camera2Group && this._camera2Group.visible) {
      this._camera2Group.rotation.y = Math.sin(elapsed * 0.4 + Math.PI) * 0.5;
    }

    // Camera LED blink
    if (this._cam1Led && this._camera1Group?.visible) {
      this._cam1Led.emissiveIntensity = (Math.sin(elapsed * 4) > 0.3) ? 1.0 : 0.1;
    }
    if (this._cam2Led && this._camera2Group?.visible) {
      this._cam2Led.emissiveIntensity = (Math.sin(elapsed * 4 + 1) > 0.3) ? 1.0 : 0.1;
    }

    // --- Screen glow color shift ---
    if (this._screenGlow && this._props.screen?.visible) {
      const hue = (elapsed * 0.03) % 1;
      const r = 0.10 + 0.06 * Math.sin(hue * Math.PI * 2);
      const g = 0.16 + 0.08 * Math.sin(hue * Math.PI * 2 + 2.1);
      const b = 0.28 + 0.12 * Math.sin(hue * Math.PI * 2 + 4.2);
      this._screenGlow.emissive.setRGB(r, g, b);
    }

    // Power LED gentle pulse
    if (this._powerLed && this._props.screen?.visible) {
      this._powerLed.emissiveIntensity = 0.5 + 0.5 * Math.sin(elapsed * 2);
    }

    // --- Dust particle drift near rubble ---
    if (this._dustParticles && this._props.rubbleWall?.visible) {
      const dPos = this._dustParticles.geometry.getAttribute('position');
      for (let i = 0; i < dPos.count; i++) {
        let y = dPos.getY(i) + 0.002 * Math.sin(elapsed + i);
        if (y > 2.5) y = 0;
        dPos.setY(i, y);
        dPos.setX(i, dPos.getX(i) + Math.sin(elapsed * 0.5 + i * 0.3) * 0.0005);
      }
      dPos.needsUpdate = true;
    }
  }
}
