/**
 * Module E — "Statistical Convergence Engine"
 * RSSI waveform, person orbs, classification, fall alert, metric bars
 */
import * as THREE from 'three';

const WAVEFORM_POINTS = 120;

export class ConvergenceEngine {
  constructor(scene, panelGroup) {
    this.group = new THREE.Group();
    if (panelGroup) panelGroup.add(this.group);
    else scene.add(this.group);

    // --- RSSI Waveform (scrolling line) ---
    this._rssiHistory = new Float32Array(WAVEFORM_POINTS);
    const waveGeo = new THREE.BufferGeometry();
    this._wavePositions = new Float32Array(WAVEFORM_POINTS * 3);
    for (let i = 0; i < WAVEFORM_POINTS; i++) {
      this._wavePositions[i * 3] = (i / WAVEFORM_POINTS) * 6 - 3; // x: -3 to 3
      this._wavePositions[i * 3 + 1] = 0;
      this._wavePositions[i * 3 + 2] = 0;
    }
    waveGeo.setAttribute('position', new THREE.BufferAttribute(this._wavePositions, 3));
    const waveMat = new THREE.LineBasicMaterial({
      color: 0x00d4ff,
      transparent: true,
      opacity: 0.8,
      blending: THREE.AdditiveBlending,
    });
    this._waveform = new THREE.Line(waveGeo, waveMat);
    this._waveform.position.y = 1.5;
    this.group.add(this._waveform);

    // Waveform glow (thicker, dimmer duplicate)
    const glowMat = new THREE.LineBasicMaterial({
      color: 0x00d4ff,
      transparent: true,
      opacity: 0.2,
      linewidth: 2,
      blending: THREE.AdditiveBlending,
    });
    this._waveGlow = new THREE.Line(waveGeo.clone(), glowMat);
    this._waveGlow.position.y = 1.5;
    this._waveGlow.scale.set(1, 1.3, 1);
    this.group.add(this._waveGlow);

    // --- Person orbs (up to 4) ---
    this._personOrbs = [];
    for (let i = 0; i < 4; i++) {
      const orbGeo = new THREE.SphereGeometry(0.2, 16, 16);
      const orbMat = new THREE.MeshBasicMaterial({
        color: 0xff8800,
        transparent: true,
        opacity: 0,
        blending: THREE.AdditiveBlending,
      });
      const orb = new THREE.Mesh(orbGeo, orbMat);
      orb.position.set(-2 + i * 1.2, -0.5, 0);
      this.group.add(orb);

      const light = new THREE.PointLight(0xff8800, 0, 3);
      orb.add(light);

      this._personOrbs.push({ mesh: orb, light, mat: orbMat });
    }

    // --- Classification text sprite ---
    this._classCanvas = document.createElement('canvas');
    this._classCanvas.width = 256;
    this._classCanvas.height = 48;
    this._classCtx = this._classCanvas.getContext('2d');
    this._classTex = new THREE.CanvasTexture(this._classCanvas);
    const classMat = new THREE.SpriteMaterial({
      map: this._classTex,
      transparent: true,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
    });
    this._classSprite = new THREE.Sprite(classMat);
    this._classSprite.scale.set(3, 0.6, 1);
    this._classSprite.position.y = 0.3;
    this.group.add(this._classSprite);

    // --- Fall alert ring ---
    const alertGeo = new THREE.TorusGeometry(2.5, 0.05, 8, 48);
    this._alertMat = new THREE.MeshBasicMaterial({
      color: 0xff2244,
      transparent: true,
      opacity: 0,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
    });
    this._alertRing = new THREE.Mesh(alertGeo, this._alertMat);
    this._alertRing.rotation.x = Math.PI / 2;
    this._alertRing.position.y = -1;
    this.group.add(this._alertRing);

    // --- Metric bars (3: frame rate, confidence, variance) ---
    this._metricBars = [];
    const barLabels = ['CONF', 'VAR', 'SPEC'];
    for (let i = 0; i < 3; i++) {
      const barGeo = new THREE.PlaneGeometry(0.15, 1.5);
      const barMat = new THREE.MeshBasicMaterial({
        color: [0x00d4ff, 0x8844ff, 0xff8800][i],
        transparent: true,
        opacity: 0.5,
        blending: THREE.AdditiveBlending,
        depthWrite: false,
        side: THREE.DoubleSide,
      });
      const bar = new THREE.Mesh(barGeo, barMat);
      bar.position.set(2 + i * 0.4, -1.2, 0);
      this.group.add(bar);
      this._metricBars.push({ mesh: bar, mat: barMat });
    }

    this._rssiHead = 0;
    this._lastClassification = '';
  }

  update(dt, elapsed, data) {
    const features = data?.features || {};
    const classification = data?.classification || {};
    const persons = data?.persons || [];
    const estPersons = data?.estimated_persons || 0;

    // --- Update RSSI waveform ---
    const rssi = features.mean_rssi || -50;
    this._rssiHistory[this._rssiHead] = rssi;
    this._rssiHead = (this._rssiHead + 1) % WAVEFORM_POINTS;

    for (let i = 0; i < WAVEFORM_POINTS; i++) {
      const histIdx = (this._rssiHead + i) % WAVEFORM_POINTS;
      const val = this._rssiHistory[histIdx];
      // Normalize RSSI (-80 to -20 range) to -1.5 to 1.5
      this._wavePositions[i * 3 + 1] = ((val + 50) / 30) * 1.5;
    }
    this._waveform.geometry.attributes.position.needsUpdate = true;

    // Copy to glow
    const glowPos = this._waveGlow.geometry.attributes.position;
    glowPos.array.set(this._wavePositions);
    glowPos.needsUpdate = true;

    // --- Person orbs ---
    for (let i = 0; i < this._personOrbs.length; i++) {
      const { mesh, light, mat } = this._personOrbs[i];
      if (i < estPersons) {
        mat.opacity = 0.7;
        light.intensity = 1.0 + Math.sin(elapsed * 3 + i * 1.5) * 0.5;
        const pulse = 1.0 + Math.sin(elapsed * 2 + i) * 0.15;
        mesh.scale.set(pulse, pulse, pulse);
      } else {
        mat.opacity = 0.05;
        light.intensity = 0;
        mesh.scale.set(0.5, 0.5, 0.5);
      }
    }

    // --- Classification text ---
    const motionLevel = classification.motion_level || 'absent';
    const label = motionLevel.toUpperCase().replace('_', ' ');
    if (label !== this._lastClassification) {
      this._lastClassification = label;
      const ctx = this._classCtx;
      ctx.clearRect(0, 0, 256, 48);
      ctx.font = '600 24px "Courier New", monospace';
      ctx.textAlign = 'center';

      if (motionLevel === 'active') ctx.fillStyle = '#ff8800';
      else if (motionLevel.includes('present')) ctx.fillStyle = '#00d4ff';
      else ctx.fillStyle = '#445566';

      ctx.fillText(label, 128, 32);
      this._classTex.needsUpdate = true;
    }

    // --- Fall alert ---
    const fallDetected = classification.fall_detected || false;
    if (fallDetected) {
      this._alertMat.opacity = 0.3 + Math.abs(Math.sin(elapsed * 6)) * 0.5;
      const scale = 1.0 + Math.sin(elapsed * 4) * 0.1;
      this._alertRing.scale.set(scale, scale, 1);
    } else {
      this._alertMat.opacity = 0;
    }

    // --- Metric bars ---
    const confidence = classification.confidence || 0;
    const variance = Math.min(1, (features.variance || 0) / 5);
    const spectral = Math.min(1, (features.spectral_power || 0) / 0.5);
    const values = [confidence, variance, spectral];

    for (let i = 0; i < 3; i++) {
      const bar = this._metricBars[i];
      const v = values[i];
      bar.mesh.scale.y = Math.max(0.05, v);
      bar.mesh.position.y = -1.2 + v * 0.75;
      bar.mat.opacity = 0.3 + v * 0.4;
    }
  }

  dispose() {
    this._waveform.geometry.dispose();
    this._waveform.material.dispose();
    this._waveGlow.geometry.dispose();
    this._waveGlow.material.dispose();
    this._alertRing.geometry.dispose();
    this._alertMat.dispose();
    this._classTex.dispose();
    for (const { mesh, mat } of this._personOrbs) {
      mesh.geometry.dispose();
      mat.dispose();
    }
    for (const { mesh, mat } of this._metricBars) {
      mesh.geometry.dispose();
      mat.dispose();
    }
  }
}
