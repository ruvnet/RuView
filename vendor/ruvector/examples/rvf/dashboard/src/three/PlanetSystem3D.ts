import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';

/**
 * Interactive 3D exoplanet system visualization with galactic context.
 *
 * Renders:
 * - Deep-field starfield (4000 background stars)
 * - Milky Way galactic plane disc
 * - Distant nebula patches
 * - Central host star (color from effective temperature)
 * - Planet on animated orbital path (size from radius_earth)
 * - Habitable zone annulus (green band)
 * - Orbit ellipse line
 * - AU scale labels
 *
 * Interaction:
 * - OrbitControls: drag to rotate, scroll to zoom, right-drag to pan
 * - Speed control via setSpeed()
 * - Reset view via resetCamera()
 */

export interface PlanetSystemParams {
  label: string;
  radiusEarth: number;
  semiMajorAxisAU: number;
  eqTempK: number;
  stellarTempK: number;
  stellarRadiusSolar: number;
  periodDays: number;
  hzMember: boolean;
  esiScore: number;
  transitDepth: number;
}

function starColorFromTemp(teff: number): number {
  if (teff > 7500) return 0xaabfff;
  if (teff > 6000) return 0xf8f7ff;
  if (teff > 5200) return 0xfff4ea;
  if (teff > 3700) return 0xffd2a1;
  return 0xffb56c;
}

function planetColor(eqTempK: number): number {
  if (eqTempK < 200) return 0x4488cc;
  if (eqTempK < 260) return 0x44aa77;
  if (eqTempK < 320) return 0x55bb55;
  if (eqTempK < 500) return 0xddaa44;
  return 0xff6644;
}

/** Deterministic pseudo-random from seed. */
function seededRandom(seed: number): () => number {
  let s = seed;
  return () => {
    s = (s * 16807 + 0) % 2147483647;
    return (s - 1) / 2147483646;
  };
}

export class PlanetSystem3D {
  private scene: THREE.Scene;
  private camera: THREE.PerspectiveCamera;
  private renderer: THREE.WebGLRenderer;
  private controls: OrbitControls;
  private starMesh: THREE.Mesh | null = null;
  private planetMesh: THREE.Mesh | null = null;
  private orbitLine: THREE.Line | null = null;
  private hzInnerRing: THREE.Mesh | null = null;
  private animId = 0;
  private time = 0;
  private orbitPoints: THREE.Vector3[] = [];
  private orbitSpeed = 0.003;
  private orbitAngle = 0;
  private speedMultiplier = 1;
  private autoRotate = true;
  private defaultCamPos = new THREE.Vector3(0, 3, 6);
  private bgGroup: THREE.Group | null = null;
  private labelSprites: THREE.Sprite[] = [];
  private currentParams: PlanetSystemParams | null = null;

  constructor(private container: HTMLElement) {
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x020408);

    const w = container.clientWidth || 400;
    const h = container.clientHeight || 300;

    this.camera = new THREE.PerspectiveCamera(50, w / h, 0.01, 2000);
    this.camera.position.set(0, 3, 6);
    this.camera.lookAt(0, 0, 0);

    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    this.renderer.setSize(w, h);
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    container.appendChild(this.renderer.domElement);

    // OrbitControls for mouse interaction
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.08;
    this.controls.minDistance = 1;
    this.controls.maxDistance = 500;
    this.controls.enablePan = true;
    this.controls.autoRotate = false; // We handle auto-rotate ourselves
    this.controls.zoomSpeed = 1.2;
    this.controls.rotateSpeed = 0.8;

    // Stop auto-rotation when user interacts
    this.controls.addEventListener('start', () => { this.autoRotate = false; });

    this.scene.add(new THREE.AmbientLight(0x222244, 0.4));

    // Build immutable background (stars, galaxy, nebulae)
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

  // ── Background: starfield, galaxy, nebulae ──

  private buildBackground(): void {
    this.bgGroup = new THREE.Group();

    // ── Starfield: 4000 background stars ──
    const starCount = 4000;
    const positions = new Float32Array(starCount * 3);
    const colors = new Float32Array(starCount * 3);
    const sizes = new Float32Array(starCount);
    const rand = seededRandom(42);

    const starTints = [
      new THREE.Color(0xffffff), // white
      new THREE.Color(0xaaccff), // blue-white
      new THREE.Color(0xfff4ea), // yellow-white
      new THREE.Color(0xffd2a1), // orange
      new THREE.Color(0xffb56c), // red-orange
      new THREE.Color(0xccddff), // pale blue
    ];

    for (let i = 0; i < starCount; i++) {
      // Distribute on a large sphere shell (300-800 units away)
      const theta = rand() * Math.PI * 2;
      const phi = Math.acos(2 * rand() - 1);
      const r = 300 + rand() * 500;
      positions[i * 3] = r * Math.sin(phi) * Math.cos(theta);
      positions[i * 3 + 1] = r * Math.sin(phi) * Math.sin(theta);
      positions[i * 3 + 2] = r * Math.cos(phi);

      const tint = starTints[Math.floor(rand() * starTints.length)];
      const brightness = 0.4 + rand() * 0.6;
      colors[i * 3] = tint.r * brightness;
      colors[i * 3 + 1] = tint.g * brightness;
      colors[i * 3 + 2] = tint.b * brightness;

      sizes[i] = 0.5 + rand() * 2.0;
    }

    const starGeo = new THREE.BufferGeometry();
    starGeo.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    starGeo.setAttribute('color', new THREE.BufferAttribute(colors, 3));
    starGeo.setAttribute('size', new THREE.BufferAttribute(sizes, 1));

    const starMat = new THREE.PointsMaterial({
      size: 1.5,
      vertexColors: true,
      transparent: true,
      opacity: 0.9,
      sizeAttenuation: true,
      depthWrite: false,
    });
    this.bgGroup.add(new THREE.Points(starGeo, starMat));

    // ── Milky Way galactic plane ──
    // A large tilted disc with dense star concentration
    const galaxyCount = 6000;
    const galPos = new Float32Array(galaxyCount * 3);
    const galCol = new Float32Array(galaxyCount * 3);
    for (let i = 0; i < galaxyCount; i++) {
      // Flat disc distribution, concentrated toward center
      const angle = rand() * Math.PI * 2;
      const dist = Math.pow(rand(), 0.5) * 600; // More concentrated near center
      const height = (rand() - 0.5) * (15 + dist * 0.02); // Thin disc, thicker outward

      galPos[i * 3] = dist * Math.cos(angle);
      galPos[i * 3 + 1] = height;
      galPos[i * 3 + 2] = dist * Math.sin(angle);

      // Milky Way is blueish-white with warm core
      const coreProx = 1 - Math.min(1, dist / 600);
      const r2 = rand();
      galCol[i * 3] = 0.5 + coreProx * 0.4 + r2 * 0.1;
      galCol[i * 3 + 1] = 0.5 + coreProx * 0.3 + r2 * 0.1;
      galCol[i * 3 + 2] = 0.6 + r2 * 0.15;
    }

    const galGeo = new THREE.BufferGeometry();
    galGeo.setAttribute('position', new THREE.BufferAttribute(galPos, 3));
    galGeo.setAttribute('color', new THREE.BufferAttribute(galCol, 3));
    const galMat = new THREE.PointsMaterial({
      size: 0.8,
      vertexColors: true,
      transparent: true,
      opacity: 0.25,
      sizeAttenuation: true,
      depthWrite: false,
    });
    const galaxy = new THREE.Points(galGeo, galMat);
    // Tilt the galactic plane ~60 degrees (we see the Milky Way at an angle)
    galaxy.rotation.x = Math.PI * 0.35;
    galaxy.rotation.z = Math.PI * 0.15;
    galaxy.position.set(0, 100, -200);
    this.bgGroup.add(galaxy);

    // ── Galactic core glow ──
    const coreGlowGeo = new THREE.SphereGeometry(40, 16, 16);
    const coreGlowMat = new THREE.MeshBasicMaterial({
      color: 0xeeddcc,
      transparent: true,
      opacity: 0.04,
      side: THREE.BackSide,
      depthWrite: false,
    });
    const coreGlow = new THREE.Mesh(coreGlowGeo, coreGlowMat);
    coreGlow.position.copy(galaxy.position);
    this.bgGroup.add(coreGlow);

    // ── Nebula patches (colored sprite billboards) ──
    const nebulaColors = [0x3344aa, 0xaa3355, 0x2288aa, 0x8844aa, 0x44aa66];
    for (let i = 0; i < 8; i++) {
      const canvas = document.createElement('canvas');
      canvas.width = 128;
      canvas.height = 128;
      const ctx = canvas.getContext('2d')!;
      const grad = ctx.createRadialGradient(64, 64, 4, 64, 64, 64);
      const col = new THREE.Color(nebulaColors[i % nebulaColors.length]);
      grad.addColorStop(0, `rgba(${Math.floor(col.r * 255)},${Math.floor(col.g * 255)},${Math.floor(col.b * 255)},0.3)`);
      grad.addColorStop(0.4, `rgba(${Math.floor(col.r * 255)},${Math.floor(col.g * 255)},${Math.floor(col.b * 255)},0.08)`);
      grad.addColorStop(1, 'rgba(0,0,0,0)');
      ctx.fillStyle = grad;
      ctx.fillRect(0, 0, 128, 128);

      const tex = new THREE.CanvasTexture(canvas);
      const spriteMat = new THREE.SpriteMaterial({
        map: tex,
        transparent: true,
        blending: THREE.AdditiveBlending,
        depthWrite: false,
      });
      const sprite = new THREE.Sprite(spriteMat);
      const theta2 = rand() * Math.PI * 2;
      const phi2 = (rand() - 0.5) * Math.PI * 0.6;
      const r2 = 200 + rand() * 400;
      sprite.position.set(
        r2 * Math.cos(phi2) * Math.cos(theta2),
        r2 * Math.sin(phi2),
        r2 * Math.cos(phi2) * Math.sin(theta2),
      );
      sprite.scale.setScalar(60 + rand() * 120);
      this.bgGroup.add(sprite);
    }

    this.scene.add(this.bgGroup);
  }

  update(params: PlanetSystemParams): void {
    this.clearSystem();
    this.currentParams = params;

    const sc = starColorFromTemp(params.stellarTempK);
    const pc = planetColor(params.eqTempK);
    const orbitRadius = Math.max(0.8, Math.min(4.0, params.semiMajorAxisAU * 2.5));

    // ── Host Star ──
    const starVisualRadius = 0.25 + params.stellarRadiusSolar * 0.2;
    const starGeo = new THREE.SphereGeometry(starVisualRadius, 32, 32);
    const starMat = new THREE.MeshBasicMaterial({ color: sc });
    this.starMesh = new THREE.Mesh(starGeo, starMat);
    this.scene.add(this.starMesh);

    const starLight = new THREE.PointLight(sc, 2.5, 30);
    starLight.position.set(0, 0, 0);
    this.scene.add(starLight);

    // Star corona
    const glowGeo = new THREE.SphereGeometry(starVisualRadius * 2.5, 24, 24);
    const glowMat = new THREE.MeshBasicMaterial({
      color: sc,
      transparent: true,
      opacity: 0.05,
      side: THREE.BackSide,
      depthWrite: false,
    });
    this.scene.add(new THREE.Mesh(glowGeo, glowMat));

    // ── Habitable Zone ──
    if (params.hzMember) {
      const hzInner = orbitRadius * 0.75;
      const hzOuter = orbitRadius * 1.35;
      const hzGeo = new THREE.RingGeometry(hzInner, hzOuter, 64);
      const hzMat = new THREE.MeshBasicMaterial({
        color: 0x2ecc71,
        transparent: true,
        opacity: 0.07,
        side: THREE.DoubleSide,
        depthWrite: false,
      });
      this.hzInnerRing = new THREE.Mesh(hzGeo, hzMat);
      this.hzInnerRing.rotation.x = -Math.PI / 2;
      this.hzInnerRing.position.y = -0.01;
      this.scene.add(this.hzInnerRing);

      const makeHzCircle = (r: number, color: number, opacity: number) => {
        const pts: THREE.Vector3[] = [];
        for (let i = 0; i <= 128; i++) {
          const th = (i / 128) * Math.PI * 2;
          pts.push(new THREE.Vector3(r * Math.cos(th), 0, r * Math.sin(th)));
        }
        const geo = new THREE.BufferGeometry().setFromPoints(pts);
        const mat = new THREE.LineBasicMaterial({ color, transparent: true, opacity });
        this.scene.add(new THREE.Line(geo, mat));
      };
      makeHzCircle(hzInner, 0x2ecc71, 0.25);
      makeHzCircle(hzOuter, 0x2ecc71, 0.12);
    }

    // ── Orbit Path ──
    this.orbitPoints = [];
    const segments = 256;
    for (let i = 0; i <= segments; i++) {
      const theta = (i / segments) * Math.PI * 2;
      this.orbitPoints.push(new THREE.Vector3(
        orbitRadius * Math.cos(theta), 0, orbitRadius * Math.sin(theta),
      ));
    }
    const orbitGeo = new THREE.BufferGeometry().setFromPoints(this.orbitPoints);
    const orbitMat = new THREE.LineBasicMaterial({ color: pc, transparent: true, opacity: 0.5 });
    this.orbitLine = new THREE.Line(orbitGeo, orbitMat);
    this.scene.add(this.orbitLine);

    // Reference rings
    const makeRefRing = (r: number) => {
      const pts: THREE.Vector3[] = [];
      for (let i = 0; i <= 128; i++) {
        const th = (i / 128) * Math.PI * 2;
        pts.push(new THREE.Vector3(r * Math.cos(th), 0, r * Math.sin(th)));
      }
      const geo = new THREE.BufferGeometry().setFromPoints(pts);
      const mat = new THREE.LineBasicMaterial({ color: 0x1c2333, transparent: true, opacity: 0.3 });
      this.scene.add(new THREE.Line(geo, mat));
    };
    makeRefRing(0.5 * 2.5);
    makeRefRing(1.5 * 2.5);

    // AU scale labels
    this.addScaleLabel('0.5 AU', 0.5 * 2.5 + 0.2, 0.3, 0);
    this.addScaleLabel('1.0 AU', 1.0 * 2.5 + 0.2, 0.3, 0);
    this.addScaleLabel('1.5 AU', 1.5 * 2.5 + 0.2, 0.3, 0);
    if (params.hzMember) {
      this.addScaleLabel('HZ', orbitRadius * 1.05, 0.5, 0, '#2ecc71');
    }

    // ── Planet ──
    const planetVisualRadius = Math.max(0.06, Math.min(0.2, params.radiusEarth * 0.1));
    const planetGeo = new THREE.SphereGeometry(planetVisualRadius, 24, 24);
    const planetMat = new THREE.MeshStandardMaterial({
      color: pc,
      emissive: pc,
      emissiveIntensity: 0.2,
      roughness: 0.7,
      metalness: 0.1,
    });
    this.planetMesh = new THREE.Mesh(planetGeo, planetMat);
    this.planetMesh.position.copy(this.orbitPoints[0]);
    this.scene.add(this.planetMesh);

    // Atmosphere halo for habitable candidates
    if (params.hzMember && params.eqTempK > 180 && params.eqTempK < 350) {
      const atmoGeo = new THREE.SphereGeometry(planetVisualRadius * 1.2, 24, 24);
      const atmoMat = new THREE.MeshBasicMaterial({
        color: 0x66ccff,
        transparent: true,
        opacity: 0.12,
        side: THREE.BackSide,
        depthWrite: false,
      });
      this.planetMesh.add(new THREE.Mesh(atmoGeo, atmoMat));
    }

    // Planet label
    this.addScaleLabel(
      params.label,
      this.orbitPoints[0].x,
      this.orbitPoints[0].y + planetVisualRadius + 0.15,
      this.orbitPoints[0].z,
      '#00e5ff',
    );

    // ── Grid ──
    const gridHelper = new THREE.GridHelper(12, 12, 0x151b23, 0x0d1117);
    gridHelper.position.y = -0.3;
    this.scene.add(gridHelper);

    // Speed and camera
    this.orbitSpeed = 0.002 + (1 / Math.max(params.periodDays, 10)) * 0.8;
    this.orbitAngle = 0;

    const camDist = orbitRadius * 1.8 + 2;
    this.defaultCamPos.set(camDist * 0.6, camDist * 0.45, camDist * 0.7);
    this.camera.position.copy(this.defaultCamPos);
    this.controls.target.set(0, 0, 0);
    this.controls.update();
    this.autoRotate = true;

    this.animate();
  }

  private addScaleLabel(text: string, x: number, y: number, z: number, color = '#556677'): void {
    const canvas = document.createElement('canvas');
    canvas.width = 128;
    canvas.height = 32;
    const ctx = canvas.getContext('2d')!;
    ctx.font = '14px monospace';
    ctx.fillStyle = color;
    ctx.textAlign = 'center';
    ctx.fillText(text, 64, 20);

    const tex = new THREE.CanvasTexture(canvas);
    tex.minFilter = THREE.LinearFilter;
    const mat = new THREE.SpriteMaterial({
      map: tex,
      transparent: true,
      depthWrite: false,
      depthTest: false,
    });
    const sprite = new THREE.Sprite(mat);
    sprite.position.set(x, y, z);
    sprite.scale.set(1.2, 0.3, 1);
    this.scene.add(sprite);
    this.labelSprites.push(sprite);
  }

  /** Remove system objects but keep background. */
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
    // Re-add ambient if missing
    let hasAmbient = false;
    this.scene.traverse((o) => { if (o instanceof THREE.AmbientLight) hasAmbient = true; });
    if (!hasAmbient) this.scene.add(new THREE.AmbientLight(0x222244, 0.4));

    this.starMesh = null;
    this.planetMesh = null;
    this.orbitLine = null;
    this.hzInnerRing = null;
    this.labelSprites = [];
  }

  private animate = (): void => {
    this.animId = requestAnimationFrame(this.animate);
    this.time += 0.005 * this.speedMultiplier;

    // Planet orbit
    if (this.planetMesh && this.orbitPoints.length > 1) {
      this.orbitAngle = (this.orbitAngle + this.orbitSpeed * this.speedMultiplier) % 1;
      const idx = Math.floor(this.orbitAngle * (this.orbitPoints.length - 1));
      this.planetMesh.position.copy(this.orbitPoints[idx]);
      this.planetMesh.rotation.y += 0.01 * this.speedMultiplier;
    }

    // Star pulse
    if (this.starMesh) {
      const scale = 1 + 0.02 * Math.sin(this.time * 3);
      this.starMesh.scale.setScalar(scale);
    }

    // Auto-rotate camera (only if user hasn't grabbed controls)
    if (this.autoRotate) {
      const camDist = this.camera.position.length();
      this.camera.position.x = camDist * 0.7 * Math.sin(this.time * 0.1);
      this.camera.position.z = camDist * 0.7 * Math.cos(this.time * 0.1);
      this.camera.position.y = camDist * 0.35 + 0.5 * Math.sin(this.time * 0.07);
      this.controls.target.set(0, 0, 0);
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
    // Also clear background
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
