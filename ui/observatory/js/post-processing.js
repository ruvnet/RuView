/**
 * Post-Processing — Subtle bloom for green glow wireframe,
 * warm vignette, minimal grain. Foundation-style.
 */
import * as THREE from 'three';
import { EffectComposer } from 'three/addons/postprocessing/EffectComposer.js';
import { RenderPass } from 'three/addons/postprocessing/RenderPass.js';
import { UnrealBloomPass } from 'three/addons/postprocessing/UnrealBloomPass.js';
import { ShaderPass } from 'three/addons/postprocessing/ShaderPass.js';

const VignetteShader = {
  uniforms: {
    tDiffuse: { value: null },
    uTime: { value: 0 },
    uVignetteStrength: { value: 0.5 },
    uChromaticStrength: { value: 0.0015 },
    uGrainStrength: { value: 0.03 },
    uWarmth: { value: 0.08 },
  },
  vertexShader: `
    varying vec2 vUv;
    void main() {
      vUv = uv;
      gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
    }
  `,
  fragmentShader: `
    uniform sampler2D tDiffuse;
    uniform float uTime;
    uniform float uVignetteStrength;
    uniform float uChromaticStrength;
    uniform float uGrainStrength;
    uniform float uWarmth;
    varying vec2 vUv;

    float rand(vec2 co) {
      return fract(sin(dot(co, vec2(12.9898, 78.233))) * 43758.5453);
    }

    void main() {
      vec2 uv = vUv;
      vec2 center = uv - 0.5;
      float dist = length(center);

      // Subtle chromatic aberration at edges only
      vec2 offset = center * dist * uChromaticStrength;
      float r = texture2D(tDiffuse, uv + offset).r;
      float g = texture2D(tDiffuse, uv).g;
      float b = texture2D(tDiffuse, uv - offset * 0.5).b;
      vec3 color = vec3(r, g, b);

      // Warm vignette
      float vignette = 1.0 - dist * dist * uVignetteStrength * 1.8;
      color *= vignette;

      // Very subtle warm shift in shadows
      float luma = dot(color, vec3(0.299, 0.587, 0.114));
      color.r += (1.0 - luma) * uWarmth * 0.5;
      color.g += (1.0 - luma) * uWarmth * 0.2;

      // Minimal grain
      float grain = (rand(uv * uTime * 0.01) - 0.5) * uGrainStrength;
      color += grain;

      gl_FragColor = vec4(color, 1.0);
    }
  `,
};

export class PostProcessing {
  constructor(renderer, scene, camera) {
    const size = renderer.getSize(new THREE.Vector2());

    this.composer = new EffectComposer(renderer);
    this.composer.addPass(new RenderPass(scene, camera));

    // Bloom — tuned for green wireframe glow
    this._bloomPass = new UnrealBloomPass(
      new THREE.Vector2(size.x, size.y),
      0.08,  // strength — subtle glow, overridden by settings
      0.2,   // radius
      0.6    // threshold
    );
    this.composer.addPass(this._bloomPass);

    // Vignette + warmth
    this._vignettePass = new ShaderPass(VignetteShader);
    this.composer.addPass(this._vignettePass);

    this._bloomEnabled = true;
  }

  update(elapsed) {
    this._vignettePass.uniforms.uTime.value = elapsed;
  }

  render() {
    this.composer.render();
  }

  resize(width, height) {
    this.composer.setSize(width, height);
    this._bloomPass.resolution.set(width, height);
  }

  setQuality(level) {
    if (level === 0) {
      this._bloomPass.strength = 0;
      this._vignettePass.uniforms.uChromaticStrength.value = 0;
      this._vignettePass.uniforms.uGrainStrength.value = 0;
    } else if (level === 1) {
      this._bloomPass.strength = 0.6;
      this._vignettePass.uniforms.uChromaticStrength.value = 0.001;
      this._vignettePass.uniforms.uGrainStrength.value = 0.02;
    } else {
      this._bloomPass.strength = 1.0;
      this._vignettePass.uniforms.uChromaticStrength.value = 0.0015;
      this._vignettePass.uniforms.uGrainStrength.value = 0.03;
    }
  }

  dispose() {
    this.composer.dispose();
  }
}
