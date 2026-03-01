import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';

/**
 * Interactive 3D Dyson sphere visualization with galactic context.
 *
 * Renders:
 * - Deep-field starfield + galactic plane
 * - Central star (emissive sphere, color from spectral type)
 * - Partial Dyson swarm shell (coverage_fraction controls opacity mask)
 * - IR waste heat glow halo
 * - Orbiting collector panels as instanced quads
 *
 * Interaction:
 * - OrbitControls: drag to rotate, scroll to zoom, right-drag to pan
 * - Speed control via setSpeed()
 * - Reset view via resetCamera()
 */

export interface DysonParams {
  coverageFraction: number;
  warmTempK: number;
  spectralType: string;
  w3Excess: number;
  w4Excess: number;
  label: string;
}

const SPECTRAL_COLORS: Record<string, number> = {
  O: 0x9bb0ff, B: 0xaabfff, A: 0xcad7ff, F: 0xf8f7ff,
  G: 0xfff4ea, K: 0xffd2a1, M: 0xffb56c, L: 0xff8833,
};

function starColor(spectralType: string): number {
  const letter = spectralType.charAt(0).toUpperCase();
  return SPECTRAL_COLORS[letter] ?? 0xffd2a1;
}

function warmColor(tempK: number): THREE.Color {
  const t = Math.max(0, Math.min(1, (tempK - 100) / 400));
  return new THREE.Color().setHSL(0.02 + t * 0.06, 0.9, 0.3 + t * 0.2);
}

function seededRandom(seed: number): () => number {
  let s = seed;
  return () => {
    s = (s * 16807 + 0) % 2147483647;
    return (s - 1) / 2147483646;
  };
}

export class DysonSphere3D {
  private scene: THREE.Scene;
  private camera: THREE.PerspectiveCamera;
  private renderer: THREE.WebGLRenderer;
  private controls: OrbitControls;
  private starMesh: THREE.Mesh | null = null;
  private shellMesh: THREE.Mesh | null = null;
  private glowMesh: THREE.Mesh | null = null;
  private panelInstances: THREE.InstancedMesh | null = null;
  private animId = 0;
  private time = 0;
  private speedMultiplier = 1;
  private autoRotate = true;
  private defaultCamPos = new THREE.Vector3(0, 1.5, 4);
  private bgGroup: THREE.Group | null = null;

  constructor(private container: HTMLElement) {
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x020408);

    const w = container.clientWidth || 400;
    const h = container.clientHeight || 300;

    this.camera = new THREE.PerspectiveCamera(50, w / h, 0.01, 2000);
    this.camera.position.set(0, 1.5, 4);
    this.camera.lookAt(0, 0, 0);

    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    this.renderer.setSize(w, h);
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    container.appendChild(this.renderer.domElement);

    // OrbitControls
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.08;
    this.controls.minDistance = 1;
    this.controls.maxDistance = 400;
    this.controls.enablePan = true;
    this.controls.zoomSpeed = 1.2;
    this.controls.rotateSpeed = 0.8;
    this.controls.addEventListener('start', () => { this.autoRotate = false; });

    this.scene.add(new THREE.AmbientLight(0x222244, 0.3));
    this.buildBackground();
  }

  // ── Public controls ──

  setSpeed(multiplier: number): void {
    this.speedMultiplier = multiplier;
  }

  resetCamera(): void {
    this.autoRotate = true;
    this.camera.position.copy(this.defaultCamPos);
    this.camera.lookAt(0, 0, 0);
    this.controls.target.set(0, 0, 0);
    this.controls.update();
  }

  toggleAutoRotate(): void {
    this.autoRotate = !this.autoRotate;
  }

  getAutoRotate(): boolean {
    return this.autoRotate;
  }

  // ── Background ──

  private buildBackground(): void {
    this.bgGroup = new THREE.Group();
    const rand = seededRandom(77);

    // Starfield
    const starCount = 4000;
    const positions = new Float32Array(starCount * 3);
    const colors = new Float32Array(starCount * 3);
    const tints = [
      new THREE.Color(0xffffff), new THREE.Color(0xaaccff),
      new THREE.Color(0xfff4ea), new THREE.Color(0xffd2a1),
      new THREE.Color(0xffb56c), new THREE.Color(0xccddff),
    ];

    for (let i = 0; i < starCount; i++) {
      const theta = rand() * Math.PI * 2;
      const phi = Math.acos(2 * rand() - 1);
      const r = 300 + rand() * 500;
      positions[i * 3] = r * Math.sin(phi) * Math.cos(theta);
      positions[i * 3 + 1] = r * Math.sin(phi) * Math.sin(theta);
      positions[i * 3 + 2] = r * Math.cos(phi);
      const tint = tints[Math.floor(rand() * tints.length)];
      const b = 0.4 + rand() * 0.6;
      colors[i * 3] = tint.r * b;
      colors[i * 3 + 1] = tint.g * b;
      colors[i * 3 + 2] = tint.b * b;
    }

    const sGeo = new THREE.BufferGeometry();
    sGeo.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    sGeo.setAttribute('color', new THREE.BufferAttribute(colors, 3));
    this.bgGroup.add(new THREE.Points(sGeo, new THREE.PointsMaterial({
      size: 1.5, vertexColors: true, transparent: true, opacity: 0.9,
      sizeAttenuation: true, depthWrite: false,
    })));

    // Galactic plane
    const galCount = 5000;
    const gp = new Float32Array(galCount * 3);
    const gc = new Float32Array(galCount * 3);
    for (let i = 0; i < galCount; i++) {
      const a = rand() * Math.PI * 2;
      const d = Math.pow(rand(), 0.5) * 500;
      const h = (rand() - 0.5) * (12 + d * 0.02);
      gp[i * 3] = d * Math.cos(a);
      gp[i * 3 + 1] = h;
      gp[i * 3 + 2] = d * Math.sin(a);
      const cp = 1 - Math.min(1, d / 500);
      gc[i * 3] = 0.5 + cp * 0.35;
      gc[i * 3 + 1] = 0.5 + cp * 0.25;
      gc[i * 3 + 2] = 0.6 + rand() * 0.1;
    }
    const gGeo = new THREE.BufferGeometry();
    gGeo.setAttribute('position', new THREE.BufferAttribute(gp, 3));
    gGeo.setAttribute('color', new THREE.BufferAttribute(gc, 3));
    const gal = new THREE.Points(gGeo, new THREE.PointsMaterial({
      size: 0.8, vertexColors: true, transparent: true, opacity: 0.2,
      sizeAttenuation: true, depthWrite: false,
    }));
    gal.rotation.x = Math.PI * 0.35;
    gal.rotation.z = Math.PI * 0.15;
    gal.position.set(0, 80, -180);
    this.bgGroup.add(gal);

    // Nebulae
    const nebColors = [0x3344aa, 0xaa3355, 0x2288aa, 0x8844aa];
    for (let i = 0; i < 6; i++) {
      const canvas = document.createElement('canvas');
      canvas.width = 128; canvas.height = 128;
      const ctx = canvas.getContext('2d')!;
      const grad = ctx.createRadialGradient(64, 64, 4, 64, 64, 64);
      const col = new THREE.Color(nebColors[i % nebColors.length]);
      grad.addColorStop(0, `rgba(${Math.floor(col.r * 255)},${Math.floor(col.g * 255)},${Math.floor(col.b * 255)},0.25)`);
      grad.addColorStop(0.4, `rgba(${Math.floor(col.r * 255)},${Math.floor(col.g * 255)},${Math.floor(col.b * 255)},0.06)`);
      grad.addColorStop(1, 'rgba(0,0,0,0)');
      ctx.fillStyle = grad;
      ctx.fillRect(0, 0, 128, 128);
      const tex = new THREE.CanvasTexture(canvas);
      const mat = new THREE.SpriteMaterial({ map: tex, transparent: true, blending: THREE.AdditiveBlending, depthWrite: false });
      const sp = new THREE.Sprite(mat);
      const t2 = rand() * Math.PI * 2;
      const p2 = (rand() - 0.5) * Math.PI * 0.5;
      const r2 = 200 + rand() * 350;
      sp.position.set(r2 * Math.cos(p2) * Math.cos(t2), r2 * Math.sin(p2), r2 * Math.cos(p2) * Math.sin(t2));
      sp.scale.setScalar(50 + rand() * 100);
      this.bgGroup.add(sp);
    }

    this.scene.add(this.bgGroup);
  }

  update(params: DysonParams): void {
    this.clearSystem();

    const sc = starColor(params.spectralType);

    // ── Central Star ──
    const starGeo = new THREE.SphereGeometry(0.5, 32, 32);
    const starMat = new THREE.MeshBasicMaterial({ color: sc });
    this.starMesh = new THREE.Mesh(starGeo, starMat);
    this.scene.add(this.starMesh);

    const starLight = new THREE.PointLight(sc, 2, 20);
    starLight.position.set(0, 0, 0);
    this.scene.add(starLight);

    // ── Dyson Shell ──
    const shellRadius = 1.5;
    const shellGeo = new THREE.SphereGeometry(shellRadius, 64, 64);
    const wc = warmColor(params.warmTempK);

    const positions = shellGeo.attributes.position;
    const vertColors = new Float32Array(positions.count * 4);
    const coverage = params.coverageFraction;

    for (let i = 0; i < positions.count; i++) {
      const x = positions.getX(i);
      const y = positions.getY(i);
      const z = positions.getZ(i);
      const theta = Math.atan2(Math.sqrt(x * x + z * z), y);
      const phi = Math.atan2(z, x);
      const pattern =
        0.5 + 0.2 * Math.sin(theta * 5 + phi * 3) +
        0.15 * Math.sin(theta * 8 - phi * 5) +
        0.15 * Math.cos(phi * 7 + theta * 2);
      const visible = pattern < coverage;
      const alpha = visible ? 0.6 + coverage * 0.3 : 0.02;
      vertColors[i * 4] = visible ? wc.r : 0.05;
      vertColors[i * 4 + 1] = visible ? wc.g : 0.05;
      vertColors[i * 4 + 2] = visible ? wc.b : 0.05;
      vertColors[i * 4 + 3] = alpha;
    }
    shellGeo.setAttribute('color', new THREE.BufferAttribute(vertColors, 4));

    const shellMat = new THREE.MeshBasicMaterial({
      vertexColors: true, transparent: true, opacity: 0.7,
      side: THREE.DoubleSide, depthWrite: false,
    });
    this.shellMesh = new THREE.Mesh(shellGeo, shellMat);
    this.scene.add(this.shellMesh);

    // Wireframe overlay
    const wireGeo = new THREE.SphereGeometry(shellRadius + 0.01, 24, 24);
    const wireMat = new THREE.MeshBasicMaterial({ color: wc, transparent: true, opacity: 0.08, wireframe: true });
    this.scene.add(new THREE.Mesh(wireGeo, wireMat));

    // ── IR Glow ──
    const glowRadius = shellRadius + 0.3 + params.w4Excess * 0.1;
    const glowGeo = new THREE.SphereGeometry(glowRadius, 32, 32);
    const glowMat = new THREE.MeshBasicMaterial({
      color: wc, transparent: true, opacity: 0.04 + coverage * 0.06,
      side: THREE.BackSide, depthWrite: false,
    });
    this.glowMesh = new THREE.Mesh(glowGeo, glowMat);
    this.scene.add(this.glowMesh);

    // ── Collector Panels ──
    const panelCount = Math.floor(coverage * 400);
    if (panelCount > 0) {
      const panelGeo = new THREE.PlaneGeometry(0.06, 0.06);
      const panelMat = new THREE.MeshBasicMaterial({ color: wc, transparent: true, opacity: 0.9, side: THREE.DoubleSide });
      this.panelInstances = new THREE.InstancedMesh(panelGeo, panelMat, panelCount);

      const dummy = new THREE.Object3D();
      for (let i = 0; i < panelCount; i++) {
        const t = i / panelCount;
        const incl = Math.acos(1 - 2 * t);
        const azim = Math.PI * (1 + Math.sqrt(5)) * i;
        const r = shellRadius + 0.02 + Math.random() * 0.05;
        dummy.position.set(
          r * Math.sin(incl) * Math.cos(azim),
          r * Math.cos(incl),
          r * Math.sin(incl) * Math.sin(azim),
        );
        dummy.lookAt(0, 0, 0);
        dummy.updateMatrix();
        this.panelInstances.setMatrixAt(i, dummy.matrix);
      }
      this.panelInstances.instanceMatrix.needsUpdate = true;
      this.scene.add(this.panelInstances);
    }

    this.defaultCamPos.set(2.5, 1.5, 3.5);
    this.camera.position.copy(this.defaultCamPos);
    this.controls.target.set(0, 0, 0);
    this.controls.update();
    this.autoRotate = true;

    this.animate();
  }

  private clearSystem(): void {
    cancelAnimationFrame(this.animId);
    const toRemove: THREE.Object3D[] = [];
    this.scene.traverse((obj) => {
      if (obj !== this.scene && obj !== this.bgGroup && obj.parent === this.scene && !(obj instanceof THREE.AmbientLight)) {
        toRemove.push(obj);
      }
    });
    for (const obj of toRemove) {
      this.scene.remove(obj);
      if ((obj as THREE.Mesh).geometry) (obj as THREE.Mesh).geometry.dispose();
    }
    let hasAmbient = false;
    this.scene.traverse((o) => { if (o instanceof THREE.AmbientLight) hasAmbient = true; });
    if (!hasAmbient) this.scene.add(new THREE.AmbientLight(0x222244, 0.3));

    this.starMesh = null;
    this.shellMesh = null;
    this.glowMesh = null;
    this.panelInstances = null;
  }

  private animate = (): void => {
    this.animId = requestAnimationFrame(this.animate);
    this.time += 0.005 * this.speedMultiplier;

    if (this.shellMesh) this.shellMesh.rotation.y = this.time * 0.3;
    if (this.panelInstances) this.panelInstances.rotation.y = this.time * 0.3;

    // Auto-rotate camera
    if (this.autoRotate) {
      const camR = 4;
      this.camera.position.x = camR * Math.sin(this.time * 0.15);
      this.camera.position.z = camR * Math.cos(this.time * 0.15);
      this.camera.position.y = 1.2 + 0.3 * Math.sin(this.time * 0.1);
      this.controls.target.set(0, 0, 0);
    }

    if (this.starMesh) {
      const scale = 1 + 0.03 * Math.sin(this.time * 3);
      this.starMesh.scale.setScalar(scale);
    }

    if (this.glowMesh) {
      const mat = this.glowMesh.material as THREE.MeshBasicMaterial;
      mat.opacity = 0.04 + 0.02 * Math.sin(this.time * 2);
    }

    this.controls.update();
    this.renderer.render(this.scene, this.camera);
  };

  resize(): void {
    const w = this.container.clientWidth || 400;
    const h = this.container.clientHeight || 300;
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(w, h);
  }

  destroy(): void {
    cancelAnimationFrame(this.animId);
    this.controls.dispose();
    this.clearSystem();
    if (this.bgGroup) {
      this.scene.remove(this.bgGroup);
      this.bgGroup.traverse((obj) => {
        if ((obj as THREE.Mesh).geometry) (obj as THREE.Mesh).geometry.dispose();
      });
      this.bgGroup = null;
    }
    this.renderer.dispose();
    if (this.renderer.domElement.parentElement) {
      this.renderer.domElement.remove();
    }
  }
}
