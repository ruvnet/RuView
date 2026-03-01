import * as THREE from 'three';

export class OrbitPreview {
  private line: THREE.Line | null = null;
  private starMesh: THREE.Mesh | null = null;
  private starGlow: THREE.Sprite | null = null;
  private planetMesh: THREE.Mesh | null = null;
  private hzRing: THREE.Line | null = null;
  private gridHelper: THREE.GridHelper | null = null;
  private scene: THREE.Scene;
  private orbitPoints: THREE.Vector3[] = [];
  private orbitAngle = 0;
  private orbitSpeed = 0.005;
  private paramOverlay: HTMLElement | null = null;
  private parentEl: HTMLElement | null = null;

  constructor(scene: THREE.Scene) {
    this.scene = scene;
    this.addStar();
    this.addGrid();
  }

  private addStar(): void {
    // Solid sphere
    const geo = new THREE.SphereGeometry(0.18, 24, 16);
    const mat = new THREE.MeshBasicMaterial({ color: 0xffdd44 });
    this.starMesh = new THREE.Mesh(geo, mat);
    this.scene.add(this.starMesh);

    // Glow sprite
    const canvas = document.createElement('canvas');
    canvas.width = 64;
    canvas.height = 64;
    const ctx = canvas.getContext('2d');
    if (ctx) {
      const grad = ctx.createRadialGradient(32, 32, 2, 32, 32, 32);
      grad.addColorStop(0, 'rgba(255,221,68,0.6)');
      grad.addColorStop(0.4, 'rgba(255,200,50,0.15)');
      grad.addColorStop(1, 'rgba(255,200,50,0)');
      ctx.fillStyle = grad;
      ctx.fillRect(0, 0, 64, 64);
    }
    const tex = new THREE.CanvasTexture(canvas);
    const spriteMat = new THREE.SpriteMaterial({ map: tex, transparent: true, blending: THREE.AdditiveBlending });
    this.starGlow = new THREE.Sprite(spriteMat);
    this.starGlow.scale.set(1.2, 1.2, 1);
    this.scene.add(this.starGlow);
  }

  private addGrid(): void {
    this.gridHelper = new THREE.GridHelper(8, 8, 0x1C2333, 0x151B23);
    this.gridHelper.position.y = -0.5;
    this.scene.add(this.gridHelper);
  }

  setOrbit(
    semiMajorAxis: number,
    eccentricity: number,
    inclination: number,
    parentElement?: HTMLElement,
  ): void {
    this.disposeLine();
    this.disposePlanet();
    this.disposeHzRing();
    this.disposeOverlay();

    const segments = 128;
    this.orbitPoints = [];
    const a = semiMajorAxis;
    const e = Math.min(Math.max(eccentricity, 0), 0.99);
    const incRad = (inclination * Math.PI) / 180;

    for (let i = 0; i <= segments; i++) {
      const theta = (i / segments) * Math.PI * 2;
      const r = (a * (1 - e * e)) / (1 + e * Math.cos(theta));
      const x = r * Math.cos(theta);
      const z = r * Math.sin(theta) * Math.cos(incRad);
      const y = r * Math.sin(theta) * Math.sin(incRad);
      this.orbitPoints.push(new THREE.Vector3(x, y, z));
    }

    // Orbit path
    const geometry = new THREE.BufferGeometry().setFromPoints(this.orbitPoints);
    const material = new THREE.LineBasicMaterial({
      color: 0x4488ff,
      transparent: true,
      opacity: 0.7,
    });
    this.line = new THREE.Line(geometry, material);
    this.scene.add(this.line);

    // Planet dot
    const planetGeo = new THREE.SphereGeometry(0.08, 12, 8);
    const planetMat = new THREE.MeshStandardMaterial({ color: 0x4488ff, emissive: 0x2244aa, emissiveIntensity: 0.3 });
    this.planetMesh = new THREE.Mesh(planetGeo, planetMat);
    this.planetMesh.position.copy(this.orbitPoints[0]);
    this.scene.add(this.planetMesh);

    // Habitable zone ring (0.95-1.37 AU scaled)
    const hzInner = 0.95 * (a / 1.5);
    const hzOuter = 1.37 * (a / 1.5);
    const hzMid = (hzInner + hzOuter) / 2;
    const hzPts: THREE.Vector3[] = [];
    for (let i = 0; i <= 64; i++) {
      const theta = (i / 64) * Math.PI * 2;
      hzPts.push(new THREE.Vector3(hzMid * Math.cos(theta), -0.48, hzMid * Math.sin(theta)));
    }
    const hzGeo = new THREE.BufferGeometry().setFromPoints(hzPts);
    const hzMat = new THREE.LineBasicMaterial({ color: 0x2ECC71, transparent: true, opacity: 0.25 });
    this.hzRing = new THREE.Line(hzGeo, hzMat);
    this.scene.add(this.hzRing);

    // Orbit speed based on period (faster for shorter periods)
    this.orbitSpeed = 0.003 + (1 / (a * 10)) * 0.02;
    this.orbitAngle = 0;

    // Param overlay
    if (parentElement) {
      this.parentEl = parentElement;
      this.paramOverlay = document.createElement('div');
      this.paramOverlay.style.cssText =
        'position:absolute;bottom:8px;left:8px;' +
        'background:rgba(11,15,20,0.85);border:1px solid var(--border);border-radius:4px;' +
        'padding:6px 10px;font-family:var(--font-mono);font-size:10px;color:var(--text-secondary);' +
        'line-height:1.6;z-index:10;pointer-events:none';
      this.paramOverlay.innerHTML =
        `<div style="color:var(--text-primary);font-weight:600;margin-bottom:2px">Orbit Parameters</div>` +
        `<div>Semi-major: <span style="color:var(--accent)">${a.toFixed(2)} AU</span></div>` +
        `<div>Eccentricity: <span style="color:var(--accent)">${e.toFixed(3)}</span></div>` +
        `<div>Inclination: <span style="color:var(--accent)">${inclination.toFixed(1)}&deg;</span></div>` +
        `<div style="margin-top:4px;color:#2ECC71;font-size:9px">&#9679; Habitable zone</div>`;
      parentElement.appendChild(this.paramOverlay);
    }
  }

  /** Call each frame to animate the planet along the orbit. */
  tick(): void {
    if (!this.planetMesh || this.orbitPoints.length < 2) return;
    this.orbitAngle = (this.orbitAngle + this.orbitSpeed) % 1;
    const idx = Math.floor(this.orbitAngle * (this.orbitPoints.length - 1));
    this.planetMesh.position.copy(this.orbitPoints[idx]);
  }

  private disposeLine(): void {
    if (this.line) {
      this.scene.remove(this.line);
      this.line.geometry.dispose();
      (this.line.material as THREE.Material).dispose();
      this.line = null;
    }
  }

  private disposePlanet(): void {
    if (this.planetMesh) {
      this.scene.remove(this.planetMesh);
      this.planetMesh.geometry.dispose();
      (this.planetMesh.material as THREE.Material).dispose();
      this.planetMesh = null;
    }
  }

  private disposeHzRing(): void {
    if (this.hzRing) {
      this.scene.remove(this.hzRing);
      this.hzRing.geometry.dispose();
      (this.hzRing.material as THREE.Material).dispose();
      this.hzRing = null;
    }
  }

  private disposeOverlay(): void {
    if (this.paramOverlay && this.parentEl) {
      this.parentEl.removeChild(this.paramOverlay);
      this.paramOverlay = null;
      this.parentEl = null;
    }
  }

  dispose(): void {
    this.disposeLine();
    this.disposePlanet();
    this.disposeHzRing();
    this.disposeOverlay();

    if (this.starMesh) {
      this.scene.remove(this.starMesh);
      this.starMesh.geometry.dispose();
      (this.starMesh.material as THREE.Material).dispose();
      this.starMesh = null;
    }
    if (this.starGlow) {
      this.scene.remove(this.starGlow);
      this.starGlow.material.map?.dispose();
      this.starGlow.material.dispose();
      this.starGlow = null;
    }
    if (this.gridHelper) {
      this.scene.remove(this.gridHelper);
      this.gridHelper.geometry.dispose();
      (this.gridHelper.material as THREE.Material).dispose();
      this.gridHelper = null;
    }
  }
}
