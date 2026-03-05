/**
 * Holographic Panel — Reusable frame with border shader, scan line, title
 */
import * as THREE from 'three';

const BORDER_VERTEX = `
varying vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

const BORDER_FRAGMENT = `
uniform float uTime;
uniform vec3 uColor;
varying vec2 vUv;

void main() {
  // Thin border
  float bx = step(vUv.x, 0.015) + step(1.0 - 0.015, vUv.x);
  float by = step(vUv.y, 0.02) + step(1.0 - 0.02, vUv.y);
  float border = clamp(bx + by, 0.0, 1.0);

  // Scan line moving upward
  float scan = smoothstep(0.0, 0.02, abs(vUv.y - fract(uTime * 0.15))) ;
  scan = 1.0 - (1.0 - scan) * 0.4;

  // Corner accents
  float corner = 0.0;
  float cx = min(vUv.x, 1.0 - vUv.x);
  float cy = min(vUv.y, 1.0 - vUv.y);
  if (cx < 0.06 && cy < 0.08) corner = 0.6;

  // Subtle fill
  float fill = 0.03 + corner * 0.05;

  float alpha = max(border * 0.7, fill) * scan;
  gl_FragColor = vec4(uColor, alpha);
}
`;

export class HolographicPanel {
  /**
   * @param {Object} opts
   * @param {number[]} opts.position - [x, y, z]
   * @param {number} opts.width
   * @param {number} opts.height
   * @param {string} opts.title
   * @param {number} [opts.color=0x00d4ff]
   */
  constructor(opts) {
    this.group = new THREE.Group();
    this.group.position.set(...opts.position);

    const color = new THREE.Color(opts.color || 0x00d4ff);

    // Border plane
    this._uniforms = {
      uTime: { value: 0 },
      uColor: { value: color },
    };

    const borderGeo = new THREE.PlaneGeometry(opts.width, opts.height);
    const borderMat = new THREE.ShaderMaterial({
      vertexShader: BORDER_VERTEX,
      fragmentShader: BORDER_FRAGMENT,
      uniforms: this._uniforms,
      transparent: true,
      side: THREE.DoubleSide,
      depthWrite: false,
      blending: THREE.AdditiveBlending,
    });
    this._border = new THREE.Mesh(borderGeo, borderMat);
    this.group.add(this._border);

    // Title sprite
    if (opts.title) {
      const canvas = document.createElement('canvas');
      canvas.width = 512;
      canvas.height = 64;
      const ctx = canvas.getContext('2d');
      ctx.fillStyle = 'transparent';
      ctx.fillRect(0, 0, 512, 64);
      ctx.font = '600 28px "Courier New", monospace';
      ctx.fillStyle = `#${color.getHexString()}`;
      ctx.textAlign = 'center';
      ctx.fillText(opts.title.toUpperCase(), 256, 42);

      const tex = new THREE.CanvasTexture(canvas);
      const spriteMat = new THREE.SpriteMaterial({
        map: tex,
        transparent: true,
        blending: THREE.AdditiveBlending,
        depthWrite: false,
      });
      const sprite = new THREE.Sprite(spriteMat);
      sprite.scale.set(opts.width * 0.8, opts.width * 0.1, 1);
      sprite.position.y = opts.height / 2 + 0.3;
      this.group.add(sprite);
      this._titleSprite = sprite;
      this._titleTex = tex;
    }
  }

  update(dt, elapsed) {
    this._uniforms.uTime.value = elapsed;
  }

  /** Make panel face camera */
  lookAt(cameraPos) {
    this.group.lookAt(cameraPos);
  }

  dispose() {
    this._border.geometry.dispose();
    this._border.material.dispose();
    if (this._titleTex) this._titleTex.dispose();
    if (this._titleSprite) this._titleSprite.material.dispose();
  }
}
