/**
 * Module A — "The Subcarrier Manifold"
 * 3D scrolling surface: 64 subcarriers x 60 time slots
 */
import * as THREE from 'three';

const MANIFOLD_VERTEX = `
attribute float aHeight;
attribute float aAge; // 0 = newest, 1 = oldest
varying float vHeight;
varying float vAge;
void main() {
  vec3 pos = position;
  pos.y += aHeight * 2.0;
  vHeight = aHeight;
  vAge = aAge;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(pos, 1.0);
}
`;

const MANIFOLD_FRAGMENT = `
uniform float uTime;
varying float vHeight;
varying float vAge;
void main() {
  // Color map: low=deep blue, mid=cyan, high=amber
  vec3 lo = vec3(0.02, 0.06, 0.2);
  vec3 mid = vec3(0.0, 0.83, 1.0);
  vec3 hi = vec3(1.0, 0.53, 0.0);

  float h = clamp(vHeight, 0.0, 1.0);
  vec3 col = h < 0.5
    ? mix(lo, mid, h * 2.0)
    : mix(mid, hi, (h - 0.5) * 2.0);

  // Fade older rows
  float alpha = 0.3 + 0.7 * (1.0 - vAge);
  gl_FragColor = vec4(col, alpha);
}
`;

const SUBS = 64;
const TIME_SLOTS = 60;

export class SubcarrierManifold {
  constructor(scene, panelGroup) {
    this.group = new THREE.Group();
    if (panelGroup) panelGroup.add(this.group);
    else scene.add(this.group);

    this._history = []; // ring buffer of Float32Array[64]
    for (let i = 0; i < TIME_SLOTS; i++) {
      this._history.push(new Float32Array(SUBS));
    }
    this._head = 0;

    // Build surface geometry
    const geo = new THREE.PlaneGeometry(8, 5, SUBS - 1, TIME_SLOTS - 1);
    const vertCount = SUBS * TIME_SLOTS;

    this._heights = new Float32Array(vertCount);
    this._ages = new Float32Array(vertCount);
    for (let t = 0; t < TIME_SLOTS; t++) {
      for (let s = 0; s < SUBS; s++) {
        this._ages[t * SUBS + s] = t / TIME_SLOTS;
      }
    }

    geo.setAttribute('aHeight', new THREE.BufferAttribute(this._heights, 1));
    geo.setAttribute('aAge', new THREE.BufferAttribute(this._ages, 1));

    // Solid surface
    const mat = new THREE.ShaderMaterial({
      vertexShader: MANIFOLD_VERTEX,
      fragmentShader: MANIFOLD_FRAGMENT,
      uniforms: { uTime: { value: 0 } },
      transparent: true,
      side: THREE.DoubleSide,
      depthWrite: false,
      blending: THREE.AdditiveBlending,
    });
    this._mesh = new THREE.Mesh(geo, mat);
    this._mesh.rotation.x = -Math.PI * 0.35;
    this.group.add(this._mesh);

    // Wireframe overlay
    const wireGeo = geo.clone();
    wireGeo.setAttribute('aHeight', new THREE.BufferAttribute(this._heights, 1));
    wireGeo.setAttribute('aAge', new THREE.BufferAttribute(this._ages, 1));
    const wireMat = new THREE.ShaderMaterial({
      vertexShader: MANIFOLD_VERTEX,
      fragmentShader: `
        varying float vHeight;
        varying float vAge;
        void main() {
          float alpha = 0.15 * (1.0 - vAge);
          gl_FragColor = vec4(0.0, 0.83, 1.0, alpha);
        }
      `,
      uniforms: { uTime: { value: 0 } },
      transparent: true,
      wireframe: true,
      depthWrite: false,
      blending: THREE.AdditiveBlending,
    });
    this._wire = new THREE.Mesh(wireGeo, wireMat);
    this._wire.rotation.x = -Math.PI * 0.35;
    this.group.add(this._wire);

    this._frameAccum = 0;
    this._pushInterval = 1 / 15; // push ~15 rows/sec
  }

  update(dt, elapsed, data) {
    this._mesh.material.uniforms.uTime.value = elapsed;

    // Push new amplitude data at regular intervals
    this._frameAccum += dt;
    if (this._frameAccum >= this._pushInterval && data) {
      this._frameAccum = 0;

      const amp = data.nodes?.[0]?.amplitude;
      const row = new Float32Array(SUBS);
      if (amp && amp.length > 0) {
        for (let i = 0; i < SUBS; i++) {
          row[i] = amp[i % amp.length] || 0;
        }
      }

      this._history[this._head] = row;
      this._head = (this._head + 1) % TIME_SLOTS;

      this._rebuildHeights();
    }
  }

  _rebuildHeights() {
    for (let t = 0; t < TIME_SLOTS; t++) {
      const histIdx = (this._head + t) % TIME_SLOTS;
      const row = this._history[histIdx];
      for (let s = 0; s < SUBS; s++) {
        const idx = t * SUBS + s;
        this._heights[idx] = row[s];
        this._ages[idx] = t / TIME_SLOTS;
      }
    }

    const geo = this._mesh.geometry;
    geo.attributes.aHeight.needsUpdate = true;
    geo.attributes.aAge.needsUpdate = true;

    const wGeo = this._wire.geometry;
    wGeo.attributes.aHeight.needsUpdate = true;
    wGeo.attributes.aAge.needsUpdate = true;
  }

  dispose() {
    this._mesh.geometry.dispose();
    this._mesh.material.dispose();
    this._wire.geometry.dispose();
    this._wire.material.dispose();
  }
}
