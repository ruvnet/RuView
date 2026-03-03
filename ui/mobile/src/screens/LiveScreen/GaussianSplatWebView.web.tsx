import { useCallback, useEffect, useRef } from 'react';
import { StyleSheet, View } from 'react-native';
import * as THREE from 'three';
import type { SensingFrame } from '@/types/sensing';

type Props = {
  onReady: () => void;
  onFps: (fps: number) => void;
  onError: (msg: string) => void;
  frame: SensingFrame | null;
};

const MAX_PERSONS = 3;

// COCO skeleton bones
const BONES: [number, number][] = [
  [0,1],[0,2],[1,3],[2,4],[5,6],[5,7],[7,9],[6,8],[8,10],
  [5,11],[6,12],[11,12],[11,13],[13,15],[12,14],[14,16],
];

// Standing pose (meters, Y-up)
const BASE_POSE: [number, number, number][] = [
  [ 0.00, 1.72, 0.04],  // 0  nose
  [-0.03, 1.76, 0.05],  // 1  left eye
  [ 0.03, 1.76, 0.05],  // 2  right eye
  [-0.08, 1.74,-0.01],  // 3  left ear
  [ 0.08, 1.74,-0.01],  // 4  right ear
  [-0.20, 1.45, 0.00],  // 5  left shoulder
  [ 0.20, 1.45, 0.00],  // 6  right shoulder
  [-0.26, 1.12, 0.04],  // 7  left elbow
  [ 0.26, 1.12, 0.04],  // 8  right elbow
  [-0.28, 0.82, 0.02],  // 9  left wrist
  [ 0.28, 0.82, 0.02],  // 10 right wrist
  [-0.11, 0.95, 0.00],  // 11 left hip
  [ 0.11, 0.95, 0.00],  // 12 right hip
  [-0.12, 0.50, 0.02],  // 13 left knee
  [ 0.12, 0.50, 0.02],  // 14 right knee
  [-0.12, 0.04, 0.00],  // 15 left ankle
  [ 0.12, 0.04, 0.00],  // 16 right ankle
];

// DensePose-style body part colors
const DENSEPOSE_COLORS: Record<string, number> = {
  head:       0xf4a582,
  neck:       0xd6604d,
  torsoFront: 0x92c5de,
  torsoSide:  0x4393c3,
  pelvis:     0x2166ac,
  lUpperArm:  0xd73027,
  rUpperArm:  0xf46d43,
  lForearm:   0xfdae61,
  rForearm:   0xfee090,
  lHand:      0xffffbf,
  rHand:      0xffffbf,
  lThigh:     0xa6d96a,
  rThigh:     0x66bd63,
  lShin:      0x1a9850,
  rShin:      0x006837,
  lFoot:      0x762a83,
  rFoot:      0x9970ab,
};

// Per-person tint offsets to visually distinguish multiple bodies
const PERSON_HUES = [0, 0.12, -0.10];

// Body segments: [jointA, jointB, topRadius, botRadius, colorKey]
const BODY_SEGS: [number, number, number, number, string][] = [
  [5,  6,  0.10, 0.10, 'torsoFront'],
  [5,  11, 0.09, 0.07, 'torsoSide'],
  [6,  12, 0.09, 0.07, 'torsoSide'],
  [11, 12, 0.08, 0.08, 'pelvis'],
  [5,  7,  0.045,0.040,'lUpperArm'],
  [7,  9,  0.038,0.032,'lForearm'],
  [6,  8,  0.045,0.040,'rUpperArm'],
  [8,  10, 0.038,0.032,'rForearm'],
  [11, 13, 0.065,0.050,'lThigh'],
  [13, 15, 0.048,0.038,'lShin'],
  [12, 14, 0.065,0.050,'rThigh'],
  [14, 16, 0.048,0.038,'rShin'],
];

function tintColor(base: number, hueShift: number): number {
  const c = new THREE.Color(base);
  const hsl = { h: 0, s: 0, l: 0 };
  c.getHSL(hsl);
  c.setHSL((hsl.h + hueShift + 1) % 1, hsl.s, hsl.l);
  return c.getHex();
}

interface BodyGroup {
  head: THREE.Mesh;
  headGlow: THREE.Mesh;
  eyeL: THREE.Mesh;
  eyeR: THREE.Mesh;
  pupilL: THREE.Mesh;
  pupilR: THREE.Mesh;
  neck: THREE.Mesh;
  torso: THREE.Mesh;
  torsoGlow: THREE.Mesh;
  handL: THREE.Mesh;
  handR: THREE.Mesh;
  footL: THREE.Mesh;
  footR: THREE.Mesh;
  limbs: THREE.Mesh[];
  limbGlows: THREE.Mesh[];
  jDots: THREE.Mesh[];
  skelLines: { line: THREE.Line; a: number; b: number }[];
  smoothKps: THREE.Vector3[];
  targetKps: THREE.Vector3[];
  fadeIn: number;
  allMeshes: THREE.Object3D[];
}

function makePart(scene: THREE.Scene, rTop: number, rBot: number, color: number, glow = false): THREE.Mesh {
  const geo = new THREE.CapsuleGeometry((rTop + rBot) / 2, 1, 6, 12);
  const mat = new THREE.MeshPhysicalMaterial({
    color, emissive: color,
    emissiveIntensity: glow ? 0.4 : 0.08,
    transparent: true, opacity: glow ? 0.12 : 0.85,
    roughness: 0.35, metalness: 0.1,
    clearcoat: glow ? 0 : 0.3, clearcoatRoughness: 0.4,
    side: glow ? THREE.BackSide : THREE.FrontSide,
  });
  const m = new THREE.Mesh(geo, mat);
  m.visible = false;
  m.castShadow = !glow;
  scene.add(m);
  return m;
}

function createBodyGroup(scene: THREE.Scene, personIdx: number): BodyGroup {
  const hue = PERSON_HUES[personIdx] ?? 0;
  const tc = (key: string) => tintColor(DENSEPOSE_COLORS[key], hue);

  // Head
  const headGeo = new THREE.SphereGeometry(0.105, 20, 16);
  headGeo.scale(1, 1.08, 1);
  const headMat = new THREE.MeshPhysicalMaterial({
    color: tc('head'), emissive: tc('head'),
    emissiveIntensity: 0.08, roughness: 0.3, metalness: 0.05,
    clearcoat: 0.4, clearcoatRoughness: 0.3, transparent: true, opacity: 0.9,
  });
  const head = new THREE.Mesh(headGeo, headMat);
  head.castShadow = true; head.visible = false; scene.add(head);

  const headGlowGeo = new THREE.SphereGeometry(0.14, 12, 10);
  const headGlowMat = new THREE.MeshBasicMaterial({
    color: tc('head'), transparent: true, opacity: 0.08, side: THREE.BackSide,
  });
  const headGlow = new THREE.Mesh(headGlowGeo, headGlowMat);
  headGlow.visible = false; scene.add(headGlow);

  // Eyes
  const eyeGeo = new THREE.SphereGeometry(0.015, 8, 6);
  const eyeMat = new THREE.MeshBasicMaterial({ color: 0xeeffff });
  const eyeL = new THREE.Mesh(eyeGeo, eyeMat);
  const eyeR = new THREE.Mesh(eyeGeo, eyeMat.clone());
  eyeL.visible = eyeR.visible = false;
  scene.add(eyeL); scene.add(eyeR);

  const pupilGeo = new THREE.SphereGeometry(0.008, 6, 4);
  const pupilMat = new THREE.MeshBasicMaterial({ color: 0x112233 });
  const pupilL = new THREE.Mesh(pupilGeo, pupilMat);
  const pupilR = new THREE.Mesh(pupilGeo, pupilMat.clone());
  pupilL.visible = pupilR.visible = false;
  scene.add(pupilL); scene.add(pupilR);

  // Neck
  const neckGeo = new THREE.CapsuleGeometry(0.04, 0.08, 4, 8);
  const neckMat = new THREE.MeshPhysicalMaterial({
    color: tc('neck'), emissive: tc('neck'),
    emissiveIntensity: 0.05, roughness: 0.4, transparent: true, opacity: 0.85,
  });
  const neck = new THREE.Mesh(neckGeo, neckMat);
  neck.castShadow = true; neck.visible = false; scene.add(neck);

  // Torso
  const torsoGeo = new THREE.BoxGeometry(0.34, 0.50, 0.18, 2, 3, 2);
  const torsoPos = torsoGeo.attributes.position;
  for (let i = 0; i < torsoPos.count; i++) {
    const x = torsoPos.getX(i), y = torsoPos.getY(i), z = torsoPos.getZ(i);
    const r = Math.sqrt(x * x + z * z);
    if (r > 0.01) {
      const bulge = 1 + 0.15 * Math.cos(y * 3.5);
      torsoPos.setX(i, x * bulge);
      torsoPos.setZ(i, z * bulge);
    }
  }
  torsoGeo.computeVertexNormals();
  const torsoMat = new THREE.MeshPhysicalMaterial({
    color: tc('torsoFront'), emissive: tc('torsoFront'),
    emissiveIntensity: 0.06, roughness: 0.35, metalness: 0.05,
    clearcoat: 0.2, transparent: true, opacity: 0.88,
  });
  const torso = new THREE.Mesh(torsoGeo, torsoMat);
  torso.castShadow = true; torso.visible = false; scene.add(torso);

  const torsoGlowGeo = new THREE.BoxGeometry(0.40, 0.55, 0.24);
  const torsoGlowMat = new THREE.MeshBasicMaterial({
    color: tc('torsoFront'), transparent: true, opacity: 0.06, side: THREE.BackSide,
  });
  const torsoGlow = new THREE.Mesh(torsoGlowGeo, torsoGlowMat);
  torsoGlow.visible = false; scene.add(torsoGlow);

  // Hands
  const handGeo = new THREE.BoxGeometry(0.05, 0.08, 0.025);
  const handL = new THREE.Mesh(handGeo, new THREE.MeshPhysicalMaterial({
    color: tc('lHand'), emissive: tc('lHand'), emissiveIntensity: 0.1, roughness: 0.3, transparent: true, opacity: 0.85,
  }));
  const handR = new THREE.Mesh(handGeo, new THREE.MeshPhysicalMaterial({
    color: tc('rHand'), emissive: tc('rHand'), emissiveIntensity: 0.1, roughness: 0.3, transparent: true, opacity: 0.85,
  }));
  handL.visible = handR.visible = false; scene.add(handL); scene.add(handR);

  // Feet
  const footGeo = new THREE.BoxGeometry(0.06, 0.04, 0.14);
  const footL = new THREE.Mesh(footGeo, new THREE.MeshPhysicalMaterial({
    color: tc('lFoot'), emissive: tc('lFoot'), emissiveIntensity: 0.1, roughness: 0.4, transparent: true, opacity: 0.85,
  }));
  const footR = new THREE.Mesh(footGeo, new THREE.MeshPhysicalMaterial({
    color: tc('rFoot'), emissive: tc('rFoot'), emissiveIntensity: 0.1, roughness: 0.4, transparent: true, opacity: 0.85,
  }));
  footL.visible = footR.visible = false; scene.add(footL); scene.add(footR);

  // Limb capsules + glow
  const limbs = BODY_SEGS.map(([,, rT, rB, ck]) => makePart(scene, rT, rB, tc(ck)));
  const limbGlows = BODY_SEGS.map(([,, rT, rB, ck]) => makePart(scene, rT * 1.6, rB * 1.6, tc(ck), true));

  // Joint dots
  const jDotGeo = new THREE.SphereGeometry(0.018, 6, 4);
  const jDots = Array.from({ length: 17 }, () => {
    const mat = new THREE.MeshBasicMaterial({ color: 0x88ddee, transparent: true, opacity: 0.7 });
    const m = new THREE.Mesh(jDotGeo, mat); m.visible = false; scene.add(m); return m;
  });

  // Skeleton lines
  const skelMat = new THREE.LineBasicMaterial({ color: 0x55ccdd, transparent: true, opacity: 0.25 });
  const skelLines = BONES.map(([a, b]) => {
    const g = new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(), new THREE.Vector3()]);
    const l = new THREE.Line(g, skelMat); l.visible = false; scene.add(l); return { line: l, a, b };
  });

  const allMeshes: THREE.Object3D[] = [
    head, headGlow, eyeL, eyeR, pupilL, pupilR, neck,
    torso, torsoGlow, handL, handR, footL, footR,
    ...limbs, ...limbGlows, ...jDots,
    ...skelLines.map((s) => s.line),
  ];

  return {
    head, headGlow, eyeL, eyeR, pupilL, pupilR, neck,
    torso, torsoGlow, handL, handR, footL, footR,
    limbs, limbGlows, jDots, skelLines,
    smoothKps: BASE_POSE.map(([x, y, z]) => new THREE.Vector3(x, y, z)),
    targetKps: BASE_POSE.map(([x, y, z]) => new THREE.Vector3(x, y, z)),
    fadeIn: 0,
    allMeshes,
  };
}

function positionLimb(mesh: THREE.Mesh, a: THREE.Vector3, b: THREE.Vector3, rTop: number, rBot: number) {
  const mid = new THREE.Vector3().addVectors(a, b).multiplyScalar(0.5);
  mesh.position.copy(mid);
  const len = a.distanceTo(b);
  mesh.scale.set((rTop + rBot) * 10, len, (rTop + rBot) * 10);
  const dir = new THREE.Vector3().subVectors(b, a).normalize();
  const up = new THREE.Vector3(0, 1, 0);
  mesh.quaternion.copy(new THREE.Quaternion().setFromUnitVectors(up, dir));
}

function lerp3(out: THREE.Vector3, target: THREE.Vector3, alpha: number) {
  out.x += (target.x - out.x) * alpha;
  out.y += (target.y - out.y) * alpha;
  out.z += (target.z - out.z) * alpha;
}

export const GaussianSplatWebViewWeb = ({ onReady, onFps, onError, frame }: Props) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const frameRef = useRef<SensingFrame | null>(null);
  const sceneRef = useRef<any>(null);
  frameRef.current = frame;

  const cleanup = useCallback(() => {
    const s = sceneRef.current;
    if (!s) return;
    cancelAnimationFrame(s.animId);
    s.renderer.dispose();
    s.scene.traverse((obj: any) => {
      if (obj.geometry) obj.geometry.dispose();
      if (obj.material) {
        const mats = Array.isArray(obj.material) ? obj.material : [obj.material];
        mats.forEach((m: any) => m.dispose());
      }
    });
    sceneRef.current = null;
  }, []);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    try {
      const W = () => container.clientWidth || window.innerWidth;
      const H = () => container.clientHeight || window.innerHeight;

      // --- Renderer ---
      const renderer = new THREE.WebGLRenderer({ antialias: true, powerPreference: 'high-performance' });
      renderer.setSize(W(), H());
      renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
      renderer.setClearColor(0x080c16);
      renderer.shadowMap.enabled = true;
      renderer.shadowMap.type = THREE.PCFSoftShadowMap;
      renderer.toneMapping = THREE.ACESFilmicToneMapping;
      renderer.toneMappingExposure = 1.1;
      container.appendChild(renderer.domElement);

      const scene = new THREE.Scene();
      scene.background = new THREE.Color(0x080c16);
      scene.fog = new THREE.FogExp2(0x080c16, 0.018);

      const camera = new THREE.PerspectiveCamera(45, W() / H(), 0.1, 200);
      camera.position.set(0, 1.4, 3.5);
      camera.lookAt(0, 0.9, 0);

      // --- Lighting ---
      scene.add(new THREE.AmbientLight(0x223344, 0.5));
      const key = new THREE.DirectionalLight(0xddeeff, 1.0);
      key.position.set(2, 5, 3);
      key.castShadow = true;
      key.shadow.mapSize.set(1024, 1024);
      key.shadow.camera.near = 0.5; key.shadow.camera.far = 15;
      key.shadow.camera.left = -3; key.shadow.camera.right = 3;
      key.shadow.camera.top = 3; key.shadow.camera.bottom = -1;
      scene.add(key);

      const rim = new THREE.PointLight(0x32b8c6, 1.5, 12);
      rim.position.set(-1.5, 2.5, -2); scene.add(rim);
      const fill = new THREE.PointLight(0x554488, 0.5, 8);
      fill.position.set(1.5, 0.8, 2.5); scene.add(fill);
      const under = new THREE.PointLight(0x225566, 0.4, 5);
      under.position.set(0, 0.1, 1); scene.add(under);

      // --- Ground ---
      const groundGeo = new THREE.PlaneGeometry(20, 20);
      const groundMat = new THREE.MeshStandardMaterial({ color: 0x0a0e1a, roughness: 0.9, metalness: 0.1 });
      const ground = new THREE.Mesh(groundGeo, groundMat);
      ground.rotation.x = -Math.PI / 2; ground.receiveShadow = true; scene.add(ground);
      const gridH = new THREE.GridHelper(20, 40, 0x1a3050, 0x0e1826);
      gridH.position.y = 0.002; scene.add(gridH);

      // --- Signal field (20x20) ---
      const GS = 20;
      const cellGeo = new THREE.PlaneGeometry(0.38, 0.38);
      const cellMat = new THREE.MeshBasicMaterial({ color: 0x32b8c6, transparent: true, opacity: 0.25, side: THREE.DoubleSide });
      const sigGrid = new THREE.InstancedMesh(cellGeo, cellMat, GS * GS);
      sigGrid.rotation.x = -Math.PI / 2; sigGrid.position.y = 0.005;
      const dum = new THREE.Object3D();
      for (let z = 0; z < GS; z++) for (let x = 0; x < GS; x++) {
        dum.position.set((x - GS / 2) * 0.4, (z - GS / 2) * 0.4, 0);
        dum.updateMatrix();
        sigGrid.setMatrixAt(z * GS + x, dum.matrix);
        sigGrid.setColorAt(z * GS + x, new THREE.Color(0x080c16));
      }
      sigGrid.instanceMatrix.needsUpdate = true;
      if (sigGrid.instanceColor) sigGrid.instanceColor.needsUpdate = true;
      scene.add(sigGrid);

      // --- ESP32 nodes ---
      const nodeGeo = new THREE.OctahedronGeometry(0.08, 1);
      const nodeMs: THREE.Mesh[] = [];
      for (let i = 0; i < 8; i++) {
        const mat = new THREE.MeshStandardMaterial({ color: 0x00ff88, emissive: 0x00ff88, emissiveIntensity: 0.7, wireframe: true });
        const m = new THREE.Mesh(nodeGeo, mat); m.visible = false; scene.add(m); nodeMs.push(m);
      }

      // --- Multi-person body groups (Issue #97) ---
      const bodies: BodyGroup[] = Array.from({ length: MAX_PERSONS }, (_, i) =>
        createBodyGroup(scene, i)
      );

      // Heart ring (shared, positioned on person 0)
      const hrGeo = new THREE.TorusGeometry(0.18, 0.006, 8, 32);
      const hrMat = new THREE.MeshBasicMaterial({ color: 0xff3355, transparent: true, opacity: 0 });
      const hrRing = new THREE.Mesh(hrGeo, hrMat); hrRing.visible = false; scene.add(hrRing);

      // Breathing rings (on person 0)
      const brRings = [0.22, 0.28, 0.34].map((r) => {
        const geo = new THREE.TorusGeometry(r, 0.003, 6, 32);
        const mat = new THREE.MeshBasicMaterial({ color: 0x44ddaa, transparent: true, opacity: 0 });
        const m = new THREE.Mesh(geo, mat); m.visible = false; scene.add(m); return m;
      });

      // WiFi pulse rings
      const wifiRings = [1.0, 1.8, 2.6].map((r) => {
        const geo = new THREE.TorusGeometry(r, 0.01, 6, 48);
        const mat = new THREE.MeshBasicMaterial({ color: 0x32b8c6, transparent: true, opacity: 0.15 });
        const m = new THREE.Mesh(geo, mat); m.rotation.x = Math.PI / 2; m.position.y = 0.01; scene.add(m); return m;
      });

      // Particles
      const NP = 400;
      const pGeo = new THREE.BufferGeometry();
      const pA = new Float32Array(NP * 3);
      for (let i = 0; i < NP; i++) {
        pA[i * 3] = (Math.random() - 0.5) * 12;
        pA[i * 3 + 1] = Math.random() * 3.5;
        pA[i * 3 + 2] = (Math.random() - 0.5) * 12;
      }
      pGeo.setAttribute('position', new THREE.BufferAttribute(pA, 3));
      scene.add(new THREE.Points(pGeo, new THREE.PointsMaterial({ color: 0x3399bb, size: 0.018, transparent: true, opacity: 0.25 })));

      // --- HUD ---
      const hudC = document.createElement('canvas'); hudC.width = 640; hudC.height = 128;
      const hudT = new THREE.CanvasTexture(hudC);
      const hudS = new THREE.Sprite(new THREE.SpriteMaterial({ map: hudT, transparent: true }));
      hudS.scale.set(3.2, 0.64, 1); hudS.position.set(0, 3.2, 0); scene.add(hudS);

      const tmpA = new THREE.Vector3();
      const tmpB = new THREE.Vector3();
      const hc = new THREE.Color();

      // State
      const state: any = {
        renderer, scene, camera, animId: 0,
        camAngle: 0, camR: 3.5, camY: 1.4,
        drag: false, fCount: 0, fpsT: performance.now(),
      };
      sceneRef.current = state;

      // Input
      const cvs = renderer.domElement;
      cvs.addEventListener('mousedown', () => { state.drag = true; });
      cvs.addEventListener('mouseup', () => { state.drag = false; });
      cvs.addEventListener('mouseleave', () => { state.drag = false; });
      cvs.addEventListener('mousemove', (e: MouseEvent) => {
        if (state.drag) {
          state.camAngle += e.movementX * 0.006;
          state.camY = Math.max(0.2, Math.min(4, state.camY - e.movementY * 0.006));
        }
      });
      cvs.addEventListener('wheel', (e: WheelEvent) => {
        state.camR = Math.max(1.5, Math.min(10, state.camR + e.deltaY * 0.003));
      }, { passive: true });
      const onR = () => { camera.aspect = W() / H(); camera.updateProjectionMatrix(); renderer.setSize(W(), H()); };
      window.addEventListener('resize', onR);

      // --- Animate ---
      const animate = () => {
        state.animId = requestAnimationFrame(animate);
        const t = performance.now() * 0.001;
        const fr = frameRef.current;

        // Camera
        if (!state.drag) state.camAngle += 0.001;
        camera.position.set(Math.sin(state.camAngle) * state.camR, state.camY, Math.cos(state.camAngle) * state.camR);
        camera.lookAt(0, 0.95, 0);

        const pres = fr?.classification?.presence ?? false;
        const mot = fr?.classification?.motion_level ?? 'absent';
        const conf = fr?.classification?.confidence ?? 0;
        const mPow = fr?.features?.motion_band_power ?? 0;
        const bPow = fr?.features?.breathing_band_power ?? 0;
        const rssi = fr?.features?.mean_rssi ?? -80;

        // How many persons to show (from server estimate, or 1 if presence)
        const nPersons = pres && conf > 0.2
          ? Math.min(MAX_PERSONS, fr?.estimated_persons ?? 1)
          : 0;

        // X-offset spacing for multi-person layout (meters)
        const personSpacing = 0.9;

        // --- Update each body group ---
        for (let pi = 0; pi < MAX_PERSONS; pi++) {
          const body = bodies[pi];
          const active = pi < nPersons;

          // Fade in/out per body
          if (active) body.fadeIn = Math.min(1, body.fadeIn + 0.015);
          else body.fadeIn = Math.max(0, body.fadeIn - 0.008);
          const show = body.fadeIn > 0.01;
          const alpha = body.fadeIn;

          if (!show) {
            body.allMeshes.forEach((m) => { m.visible = false; });
            continue;
          }

          // Per-person X offset: spread evenly from center
          const half = (nPersons - 1) / 2;
          const xOff = (pi - half) * personSpacing;

          // Per-person animation phase offset (prevent sync)
          const phOff = pi * 2.094; // ~120 degrees

          // --- Compute target keypoints ---
          for (let i = 0; i < 17; i++) {
            const [bx, by, bz] = BASE_POSE[i];
            let ax = bx + xOff, ay = by, az = bz;

            if (active) {
              const bFreq = 0.25 + bPow * 0.5;
              const bAmp = 0.004 + bPow * 0.008;
              const bPhase = Math.sin(t * bFreq * Math.PI * 2 + phOff);
              if (i >= 5 && i <= 10) ay += bPhase * bAmp;
              if (i <= 4) ay += bPhase * bAmp * 0.3;

              // Subtle sway (different per person)
              ax += Math.sin(t * 0.35 + phOff) * 0.004;
              az += Math.cos(t * 0.25 + phOff) * 0.002;

              if (mot === 'active') {
                const ws = 1.8 + mPow * 2;
                const wa = 0.03 + mPow * 0.06;
                const ph = t * ws + phOff;
                if (i === 13) { az += Math.sin(ph) * wa * 0.7; ay -= Math.abs(Math.sin(ph)) * 0.015; }
                if (i === 14) { az += Math.sin(ph + Math.PI) * wa * 0.7; ay -= Math.abs(Math.sin(ph + Math.PI)) * 0.015; }
                if (i === 15) az += Math.sin(ph - 0.2) * wa * 0.8;
                if (i === 16) az += Math.sin(ph + Math.PI - 0.2) * wa * 0.8;
                if (i === 7) az += Math.sin(ph + Math.PI) * wa * 0.35;
                if (i === 8) az += Math.sin(ph) * wa * 0.35;
                if (i === 9) az += Math.sin(ph + Math.PI) * wa * 0.45;
                if (i === 10) az += Math.sin(ph) * wa * 0.45;
                ay += Math.abs(Math.sin(ph)) * 0.006;
              } else if (mot === 'present_still') {
                const it = t * 0.25 + phOff;
                if (i >= 11) ax += Math.sin(it * 0.4) * 0.004;
                if (i === 9) ax += Math.sin(it * 0.8) * 0.005;
                if (i === 10) ax += Math.sin(it * 0.6 + 0.5) * 0.005;
              }
            }
            body.targetKps[i].set(ax, ay, az);
          }

          // Smooth interpolation
          const lerpA = 0.04;
          for (let i = 0; i < 17; i++) lerp3(body.smoothKps[i], body.targetKps[i], lerpA);
          const kps = body.smoothKps;

          // Head
          body.head.visible = body.headGlow.visible = show;
          tmpA.copy(kps[0]).add(new THREE.Vector3(0, 0.06, 0));
          body.head.position.copy(tmpA);
          body.headGlow.position.copy(tmpA);
          (body.head.material as THREE.MeshPhysicalMaterial).opacity = alpha * 0.9;
          (body.headGlow.material as THREE.MeshBasicMaterial).opacity = alpha * 0.08;

          // Eyes + pupils
          body.eyeL.visible = body.eyeR.visible = body.pupilL.visible = body.pupilR.visible = show;
          const hp = body.head.position;
          body.eyeL.position.set(hp.x - 0.032, hp.y + 0.01, hp.z + 0.09);
          body.eyeR.position.set(hp.x + 0.032, hp.y + 0.01, hp.z + 0.09);
          body.pupilL.position.set(body.eyeL.position.x, body.eyeL.position.y, body.eyeL.position.z + 0.012);
          body.pupilR.position.set(body.eyeR.position.x, body.eyeR.position.y, body.eyeR.position.z + 0.012);

          // Neck
          body.neck.visible = show;
          const neckTop = new THREE.Vector3().copy(kps[0]).add(new THREE.Vector3(0, -0.04, 0));
          const neckBot = tmpA.addVectors(kps[5], kps[6]).multiplyScalar(0.5).add(new THREE.Vector3(0, 0.04, 0));
          body.neck.position.addVectors(neckTop, neckBot).multiplyScalar(0.5);
          body.neck.scale.y = neckTop.distanceTo(neckBot) * 4;
          (body.neck.material as THREE.MeshPhysicalMaterial).opacity = alpha * 0.85;

          // Torso
          body.torso.visible = body.torsoGlow.visible = show;
          const mSh = tmpA.addVectors(kps[5], kps[6]).multiplyScalar(0.5);
          const mHp = tmpB.addVectors(kps[11], kps[12]).multiplyScalar(0.5);
          const tPos = new THREE.Vector3().addVectors(mSh, mHp).multiplyScalar(0.5);
          body.torso.position.copy(tPos);
          body.torsoGlow.position.copy(tPos);
          const bScale = 1 + Math.sin(t * (0.9 + bPow * 4) * Math.PI * 2 + phOff) * 0.02 * (1 + bPow * 3);
          body.torso.scale.set(1, 1, bScale);
          (body.torso.material as THREE.MeshPhysicalMaterial).opacity = alpha * 0.88;
          (body.torsoGlow.material as THREE.MeshBasicMaterial).opacity = alpha * 0.06;

          // Hands
          body.handL.visible = body.handR.visible = show;
          body.handL.position.copy(kps[9]).add(new THREE.Vector3(0, -0.04, 0));
          body.handR.position.copy(kps[10]).add(new THREE.Vector3(0, -0.04, 0));
          (body.handL.material as THREE.MeshPhysicalMaterial).opacity = alpha * 0.85;
          (body.handR.material as THREE.MeshPhysicalMaterial).opacity = alpha * 0.85;

          // Feet
          body.footL.visible = body.footR.visible = show;
          body.footL.position.copy(kps[15]).add(new THREE.Vector3(0, 0.02, 0.04));
          body.footR.position.copy(kps[16]).add(new THREE.Vector3(0, 0.02, 0.04));
          (body.footL.material as THREE.MeshPhysicalMaterial).opacity = alpha * 0.85;
          (body.footR.material as THREE.MeshPhysicalMaterial).opacity = alpha * 0.85;

          // Limb capsules
          BODY_SEGS.forEach(([ai, bi, rT, rB], idx) => {
            body.limbs[idx].visible = body.limbGlows[idx].visible = show;
            positionLimb(body.limbs[idx], kps[ai], kps[bi], rT, rB);
            positionLimb(body.limbGlows[idx], kps[ai], kps[bi], rT * 1.6, rB * 1.6);
            const limbMat = body.limbs[idx].material as THREE.MeshPhysicalMaterial;
            limbMat.opacity = alpha * 0.82;
            limbMat.emissiveIntensity = 0.06 + mPow * 0.4;
            const glowMat = body.limbGlows[idx].material as THREE.MeshPhysicalMaterial;
            glowMat.opacity = alpha * (0.06 + mPow * 0.15);
          });

          // Joint dots & skeleton lines
          body.jDots.forEach((d, i) => { d.visible = show; d.position.copy(kps[i]); });
          body.skelLines.forEach(({ line, a, b }) => {
            line.visible = show;
            const p = line.geometry.attributes.position as THREE.BufferAttribute;
            p.setXYZ(0, kps[a].x, kps[a].y, kps[a].z);
            p.setXYZ(1, kps[b].x, kps[b].y, kps[b].z);
            p.needsUpdate = true;
          });
        }

        // Heart ring (person 0 only)
        const vs = fr?.vital_signs as Record<string, unknown> | undefined;
        const hrBpm = Number(vs?.hr_proxy_bpm ?? vs?.heart_rate_bpm ?? 0);
        const showP0 = bodies[0].fadeIn > 0.01;
        hrRing.visible = showP0 && hrBpm > 0;
        if (hrRing.visible) {
          const chst = tmpA.addVectors(bodies[0].smoothKps[5], bodies[0].smoothKps[6]).multiplyScalar(0.5);
          chst.y -= 0.08;
          hrRing.position.copy(chst);
          hrRing.lookAt(camera.position);
          const bp = (t * (hrBpm / 60) * Math.PI * 2) % (Math.PI * 2);
          const beat = Math.pow(Math.max(0, Math.sin(bp)), 10);
          hrMat.opacity = beat * 0.5 * bodies[0].fadeIn;
          hrRing.scale.setScalar(1 + beat * 0.12);
        }

        // Breathing rings (person 0 only)
        brRings.forEach((ring, ri) => {
          ring.visible = showP0 && bPow > 0.01;
          if (ring.visible) {
            const chst = tmpA.addVectors(bodies[0].smoothKps[5], bodies[0].smoothKps[6]).multiplyScalar(0.5);
            chst.y -= 0.05;
            ring.position.copy(chst);
            ring.lookAt(camera.position);
            const bph = Math.sin(t * (0.9 + bPow * 4) * Math.PI * 2 - ri * 0.5);
            (ring.material as THREE.MeshBasicMaterial).opacity = Math.max(0, bph * 0.2 * bodies[0].fadeIn);
            ring.scale.setScalar(1 + bph * 0.08);
          }
        });

        // WiFi pulse rings
        wifiRings.forEach((wr, wi) => {
          const phase = (t * 0.5 + wi * 0.4) % 1;
          wr.scale.setScalar(0.8 + phase * 1.5 + mPow);
          (wr.material as THREE.MeshBasicMaterial).opacity = (1 - phase) * 0.12 * (pres ? 1 : 0.3);
        });

        // ESP32 nodes
        (fr?.nodes || []).forEach((n, i) => {
          if (i < nodeMs.length) {
            const [px, py, pz] = n.position;
            nodeMs[i].position.set(px * 2, py + 0.12, pz * 2);
            nodeMs[i].visible = true; nodeMs[i].rotation.y = t * 0.4 + i;
            (nodeMs[i].material as THREE.MeshStandardMaterial).emissiveIntensity = 0.5 + Math.sin(t * 3 + i) * 0.3;
          }
        });
        for (let i = (fr?.nodes || []).length; i < nodeMs.length; i++) nodeMs[i].visible = false;

        // Signal field
        const sf = fr?.signal_field;
        if (sf?.values?.length) {
          const gx = sf.grid_size[0], gz = sf.grid_size[2];
          for (let zi = 0; zi < Math.min(gz, GS); zi++) for (let xi = 0; xi < Math.min(gx, GS); xi++) {
            const v = sf.values[zi * gx + xi] || 0;
            if (v < 0.25) hc.setRGB(0.03, 0.05 + v * 1.8, 0.08 + v * 1.8);
            else if (v < 0.5) hc.setRGB(0.03, 0.2 + (v - 0.25) * 2.4, 0.5 - (v - 0.25) * 1.2);
            else if (v < 0.75) hc.setRGB((v - 0.5) * 4, 0.7 + (v - 0.5) * 0.6, 0.1);
            else hc.setRGB(1, 1 - (v - 0.75) * 3, 0.05);
            sigGrid.setColorAt(zi * GS + xi, hc);
          }
          if (sigGrid.instanceColor) sigGrid.instanceColor.needsUpdate = true;
        }

        // Lighting follows data
        rim.intensity = 0.8 + Math.abs(rssi + 50) * 0.015;

        // Particles
        const pp = pGeo.attributes.position as THREE.BufferAttribute;
        for (let i = 0; i < NP; i++) {
          (pp.array as Float32Array)[i * 3 + 1] += Math.sin(t * 0.8 + i * 0.5) * 0.0006 + mPow * 0.001;
          if ((pp.array as Float32Array)[i * 3 + 1] > 3.5) (pp.array as Float32Array)[i * 3 + 1] = 0;
        }
        pp.needsUpdate = true;

        // HUD
        const ctx = hudC.getContext('2d');
        if (ctx && fr) {
          ctx.clearRect(0, 0, 640, 128);
          ctx.font = 'bold 14px "SF Mono", Menlo, monospace';
          ctx.fillStyle = '#32b8c6';
          ctx.fillText(`WIFI-DENSEPOSE  [${(fr.source || '--').toUpperCase()}]`, 12, 20);
          ctx.font = '12px "SF Mono", Menlo, monospace';
          ctx.fillStyle = '#7799aa';
          ctx.fillText(`Nodes: ${(fr.nodes || []).length}   RSSI: ${rssi.toFixed(1)} dBm   Motion: ${mot}   Conf: ${(conf * 100).toFixed(0)}%`, 12, 42);
          if (vs) {
            const br = Number(vs.breathing_bpm ?? vs.breathing_rate_bpm ?? 0);
            if (br > 0 || hrBpm > 0) {
              ctx.fillStyle = '#44ddaa';
              ctx.fillText(`Breathing: ${br.toFixed(1)} bpm    Heart: ${hrBpm.toFixed(1)} bpm`, 12, 62);
            }
          }
          const anyShow = bodies.some((b) => b.fadeIn > 0.01);
          if (anyShow) {
            ctx.fillStyle = pres ? (mot === 'active' ? '#ff8844' : '#44bbcc') : '#556677';
            const mBar = Math.min(20, Math.round(mPow * 40));
            const mBarStr = '\u2588'.repeat(mBar) + '\u2591'.repeat(20 - mBar);
            ctx.fillText(`Motion: [${mBarStr}] ${(mPow * 100).toFixed(0)}%`, 12, 82);
            ctx.fillStyle = nPersons > 1 ? '#ffaa44' : '#556677';
            ctx.font = '10px "SF Mono", Menlo, monospace';
            ctx.fillText(`Persons: ${nPersons}   Pose: procedural (CSI-driven)`, 12, 100);
          }
          hudT.needsUpdate = true;
        }

        renderer.render(scene, camera);

        state.fCount++;
        if (performance.now() - state.fpsT >= 1000) {
          onFps(state.fCount); state.fCount = 0; state.fpsT = performance.now();
        }
      };

      animate();
      onReady();

      return () => {
        cvs.removeEventListener('mousedown', () => {});
        window.removeEventListener('resize', onR);
        cleanup();
        if (container.contains(renderer.domElement)) container.removeChild(renderer.domElement);
      };
    } catch (err) {
      onError(err instanceof Error ? err.message : 'Failed to initialize 3D renderer');
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <View style={styles.container}>
      <div ref={containerRef} style={{ width: '100%', height: '100%', backgroundColor: '#080c16' }} />
    </View>
  );
};

const styles = StyleSheet.create({ container: { flex: 1, backgroundColor: '#080c16' } });
export default GaussianSplatWebViewWeb;
