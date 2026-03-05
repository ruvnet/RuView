/**
 * Module B — "Vital Signs Oracle"
 * Breathing/HR as orbital torus rings with beat markers + trail particles
 */
import * as THREE from 'three';

export class VitalsOracle {
  constructor(scene, panelGroup) {
    this.group = new THREE.Group();
    if (panelGroup) panelGroup.add(this.group);
    else scene.add(this.group);

    // Outer torus — breathing (violet)
    const breathGeo = new THREE.TorusGeometry(1.8, 0.06, 16, 64);
    this._breathMat = new THREE.MeshBasicMaterial({
      color: 0x8844ff,
      transparent: true,
      opacity: 0.7,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
    });
    this._breathRing = new THREE.Mesh(breathGeo, this._breathMat);
    this._breathRing.rotation.x = Math.PI * 0.4;
    this.group.add(this._breathRing);

    // Inner torus — heart rate (crimson)
    const hrGeo = new THREE.TorusGeometry(1.2, 0.04, 16, 64);
    this._hrMat = new THREE.MeshBasicMaterial({
      color: 0xff2244,
      transparent: true,
      opacity: 0.6,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
    });
    this._hrRing = new THREE.Mesh(hrGeo, this._hrMat);
    this._hrRing.rotation.x = Math.PI * 0.5;
    this._hrRing.rotation.z = Math.PI * 0.15;
    this.group.add(this._hrRing);

    // Center orb
    const orbGeo = new THREE.SphereGeometry(0.35, 24, 24);
    this._orbMat = new THREE.MeshBasicMaterial({
      color: 0x00d4ff,
      transparent: true,
      opacity: 0.5,
      blending: THREE.AdditiveBlending,
    });
    this._orb = new THREE.Mesh(orbGeo, this._orbMat);
    this.group.add(this._orb);

    // Bloom point light
    this._light = new THREE.PointLight(0x00d4ff, 1.5, 8);
    this.group.add(this._light);

    // Trail particles along breathing ring
    const trailCount = 120;
    const trailGeo = new THREE.BufferGeometry();
    const trailPos = new Float32Array(trailCount * 3);
    const trailSizes = new Float32Array(trailCount);
    for (let i = 0; i < trailCount; i++) {
      const angle = (i / trailCount) * Math.PI * 2;
      trailPos[i * 3] = Math.cos(angle) * 1.8;
      trailPos[i * 3 + 1] = 0;
      trailPos[i * 3 + 2] = Math.sin(angle) * 1.8;
      trailSizes[i] = 3;
    }
    trailGeo.setAttribute('position', new THREE.BufferAttribute(trailPos, 3));
    trailGeo.setAttribute('size', new THREE.BufferAttribute(trailSizes, 1));

    const trailMat = new THREE.PointsMaterial({
      color: 0x8844ff,
      size: 0.08,
      transparent: true,
      opacity: 0.4,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
      sizeAttenuation: true,
    });
    this._trails = new THREE.Points(trailGeo, trailMat);
    this._trails.rotation.x = Math.PI * 0.4;
    this.group.add(this._trails);

    // Beat flash sprites
    this._beatFlash = this._createBeatSprite(0xff2244);
    this.group.add(this._beatFlash);
    this._beatTimer = 0;
    this._lastBeatTime = 0;

    // State
    this._breathBpm = 0;
    this._hrBpm = 0;
    this._breathConf = 0;
    this._hrConf = 0;
  }

  _createBeatSprite(color) {
    const canvas = document.createElement('canvas');
    canvas.width = 64;
    canvas.height = 64;
    const ctx = canvas.getContext('2d');
    const gradient = ctx.createRadialGradient(32, 32, 0, 32, 32, 32);
    gradient.addColorStop(0, `rgba(255, 34, 68, 1)`);
    gradient.addColorStop(0.3, `rgba(255, 34, 68, 0.5)`);
    gradient.addColorStop(1, `rgba(255, 34, 68, 0)`);
    ctx.fillStyle = gradient;
    ctx.fillRect(0, 0, 64, 64);

    const tex = new THREE.CanvasTexture(canvas);
    const mat = new THREE.SpriteMaterial({
      map: tex,
      transparent: true,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
    });
    const sprite = new THREE.Sprite(mat);
    sprite.scale.set(0, 0, 0);
    return sprite;
  }

  update(dt, elapsed, data) {
    const vs = data?.vital_signs || {};
    this._breathBpm = vs.breathing_rate_bpm || 0;
    this._hrBpm = vs.heart_rate_bpm || 0;
    this._breathConf = vs.breathing_confidence || 0;
    this._hrConf = vs.heart_rate_confidence || 0;

    // Breathing ring pulsation
    const breathFreq = this._breathBpm / 60;
    const breathPulse = breathFreq > 0 ? Math.sin(elapsed * Math.PI * 2 * breathFreq) : 0;
    const breathScale = 1.0 + breathPulse * 0.08 * this._breathConf;
    this._breathRing.scale.set(breathScale, breathScale, 1);
    this._breathMat.opacity = 0.3 + this._breathConf * 0.5;

    // HR ring pulsation (faster)
    const hrFreq = this._hrBpm / 60;
    const hrPulse = hrFreq > 0 ? Math.sin(elapsed * Math.PI * 2 * hrFreq) : 0;
    const hrScale = 1.0 + hrPulse * 0.06 * this._hrConf;
    this._hrRing.scale.set(hrScale, hrScale, 1);
    this._hrMat.opacity = 0.2 + this._hrConf * 0.5;

    // Slow rotation
    this._breathRing.rotation.z = elapsed * 0.1;
    this._hrRing.rotation.z = -elapsed * 0.15;
    this._trails.rotation.z = elapsed * 0.1;

    // Center orb pulse
    const orbPulse = 1.0 + breathPulse * 0.1;
    this._orb.scale.set(orbPulse, orbPulse, orbPulse);
    this._light.intensity = 0.8 + Math.abs(breathPulse) * 1.0;

    // Beat flash on HR cycle
    if (hrFreq > 0) {
      this._beatTimer += dt;
      const beatInterval = 1 / hrFreq;
      if (this._beatTimer >= beatInterval) {
        this._beatTimer -= beatInterval;
        this._lastBeatTime = elapsed;
      }
      const beatAge = elapsed - this._lastBeatTime;
      const flashSize = Math.max(0, 1.2 - beatAge * 4) * this._hrConf;
      this._beatFlash.scale.set(flashSize, flashSize, 1);
    } else {
      this._beatFlash.scale.set(0, 0, 0);
    }

    // Update trail particle sizes based on breathing
    const sizes = this._trails.geometry.attributes.size;
    if (sizes) {
      for (let i = 0; i < sizes.count; i++) {
        const phase = (i / sizes.count) * Math.PI * 2 + elapsed * breathFreq * Math.PI * 2;
        sizes.array[i] = 0.04 + Math.abs(Math.sin(phase)) * 0.06 * this._breathConf;
      }
      sizes.needsUpdate = true;
    }
  }

  dispose() {
    this._breathRing.geometry.dispose();
    this._breathMat.dispose();
    this._hrRing.geometry.dispose();
    this._hrMat.dispose();
    this._orb.geometry.dispose();
    this._orbMat.dispose();
    this._trails.geometry.dispose();
    this._trails.material.dispose();
  }
}
