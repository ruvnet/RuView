// Real-time CSI Signal Visualization - WiFi DensePose
// Amplitude heatmap, Phase plot, Doppler spectrum, Motion energy

export class SignalVisualization {
  constructor(scene) {
    this.scene = scene;
    this.group = new THREE.Group();
    this.group.name = 'signal-visualization';
    this.group.position.set(-5.5, 0, -3);

    // Configuration
    this.config = {
      subcarriers: 30,
      timeSlots: 40,
      heatmapWidth: 3.0,
      heatmapHeight: 1.5,
      phaseWidth: 3.0,
      phaseHeight: 1.0,
      dopplerBars: 16,
      dopplerWidth: 2.0,
      dopplerHeight: 1.0
    };

    // Data buffers
    this.amplitudeHistory = [];
    this.phaseData = new Float32Array(this.config.subcarriers);
    this.dopplerData = new Float32Array(this.config.dopplerBars);
    this.motionEnergy = 0;
    this.targetMotionEnergy = 0;

    // Initialize for timeSlots rows of subcarrier data
    for (let i = 0; i < this.config.timeSlots; i++) {
      this.amplitudeHistory.push(new Float32Array(this.config.subcarriers));
    }

    // Build visualizations
    this._buildAmplitudeHeatmap();
    this._buildPhasePlot();
    this._buildDopplerSpectrum();
    this._buildMotionIndicator();
    this._buildLabels();

    this.scene.add(this.group);
  }

  _buildAmplitudeHeatmap() {
    // Create a grid of colored cells for CSI amplitude across subcarriers over time
    const { subcarriers, timeSlots, heatmapWidth, heatmapHeight } = this.config;
    const cellW = heatmapWidth / subcarriers;
    const cellH = heatmapHeight / timeSlots;

    this._heatmapCells = [];
    this._heatmapGroup = new THREE.Group();
    this._heatmapGroup.position.set(0, 3.5, 0);

    const cellGeom = new THREE.PlaneGeometry(cellW * 0.9, cellH * 0.9);

    for (let t = 0; t < timeSlots; t++) {
      const row = [];
      for (let s = 0; s < subcarriers; s++) {
        const mat = new THREE.MeshBasicMaterial({
          color: 0x000022,
          transparent: true,
          opacity: 0.85,
          side: THREE.DoubleSide
        });
        const cell = new THREE.Mesh(cellGeom, mat);
        cell.position.set(
          s * cellW - heatmapWidth / 2 + cellW / 2,
          t * cellH,
          0
        );
        this._heatmapGroup.add(cell);
        row.push(cell);
      }
      this._heatmapCells.push(row);
    }

    // Border frame
    const frameGeom = new THREE.EdgesGeometry(
      new THREE.PlaneGeometry(heatmapWidth + 0.1, heatmapHeight + 0.1)
    );
    const frameMat = new THREE.LineBasicMaterial({ color: 0x335577, opacity: 0.5, transparent: true });
    const frame = new THREE.LineSegments(frameGeom, frameMat);
    frame.position.set(0, heatmapHeight / 2, -0.01);
    this._heatmapGroup.add(frame);

    this.group.add(this._heatmapGroup);
  }

  _buildPhasePlot() {
    // Line chart showing phase across subcarriers in 3D space
    const { subcarriers, phaseWidth, phaseHeight } = this.config;

    this._phaseGroup = new THREE.Group();
    this._phaseGroup.position.set(0, 2.0, 0);

    // Create the phase line
    const positions = new Float32Array(subcarriers * 3);
    for (let i = 0; i < subcarriers; i++) {
      positions[i * 3] = (i / (subcarriers - 1)) * phaseWidth - phaseWidth / 2;
      positions[i * 3 + 1] = 0;
      positions[i * 3 + 2] = 0;
    }

    const phaseGeom = new THREE.BufferGeometry();
    phaseGeom.setAttribute('position', new THREE.BufferAttribute(positions, 3));

    const phaseMat = new THREE.LineBasicMaterial({
      color: 0x00ff88,
      transparent: true,
      opacity: 0.8,
      linewidth: 2
    });

    this._phaseLine = new THREE.Line(phaseGeom, phaseMat);
    this._phaseGroup.add(this._phaseLine);

    // Phase reference line (zero line)
    const refPositions = new Float32Array(6);
    refPositions[0] = -phaseWidth / 2; refPositions[1] = 0; refPositions[2] = 0;
    refPositions[3] = phaseWidth / 2;  refPositions[4] = 0; refPositions[5] = 0;
    const refGeom = new THREE.BufferGeometry();
    refGeom.setAttribute('position', new THREE.BufferAttribute(refPositions, 3));
    const refMat = new THREE.LineBasicMaterial({ color: 0x224433, opacity: 0.3, transparent: true });
    this._phaseGroup.add(new THREE.LineSegments(refGeom, refMat));

    // Vertical axis lines
    const axisPositions = new Float32Array(12);
    // Left axis
    axisPositions[0] = -phaseWidth / 2; axisPositions[1] = -phaseHeight / 2; axisPositions[2] = 0;
    axisPositions[3] = -phaseWidth / 2; axisPositions[4] = phaseHeight / 2;  axisPositions[5] = 0;
    // Right axis
    axisPositions[6] = phaseWidth / 2;  axisPositions[7] = -phaseHeight / 2; axisPositions[8] = 0;
    axisPositions[9] = phaseWidth / 2;  axisPositions[10] = phaseHeight / 2; axisPositions[11] = 0;
    const axisGeom = new THREE.BufferGeometry();
    axisGeom.setAttribute('position', new THREE.BufferAttribute(axisPositions, 3));
    this._phaseGroup.add(new THREE.LineSegments(axisGeom, refMat));

    this.group.add(this._phaseGroup);
  }

  _buildDopplerSpectrum() {
    // Bar chart for Doppler frequency spectrum
    const { dopplerBars, dopplerWidth, dopplerHeight } = this.config;
    const barWidth = (dopplerWidth / dopplerBars) * 0.8;
    const gap = (dopplerWidth / dopplerBars) * 0.2;

    this._dopplerGroup = new THREE.Group();
    this._dopplerGroup.position.set(0, 0.8, 0);
    this._dopplerBars = [];

    const barGeom = new THREE.BoxGeometry(barWidth, 1, 0.05);

    for (let i = 0; i < dopplerBars; i++) {
      const mat = new THREE.MeshBasicMaterial({
        color: 0x0044aa,
        transparent: true,
        opacity: 0.75
      });
      const bar = new THREE.Mesh(barGeom, mat);
      const x = (i / (dopplerBars - 1)) * dopplerWidth - dopplerWidth / 2;
      bar.position.set(x, 0, 0);
      bar.scale.y = 0.01; // Start flat
      this._dopplerGroup.add(bar);
      this._dopplerBars.push(bar);
    }

    // Base line
    const basePositions = new Float32Array(6);
    basePositions[0] = -dopplerWidth / 2 - 0.1; basePositions[1] = 0; basePositions[2] = 0;
    basePositions[3] = dopplerWidth / 2 + 0.1;  basePositions[4] = 0; basePositions[5] = 0;
    const baseGeom = new THREE.BufferGeometry();
    baseGeom.setAttribute('position', new THREE.BufferAttribute(basePositions, 3));
    const baseMat = new THREE.LineBasicMaterial({ color: 0x335577, opacity: 0.5, transparent: true });
    this._dopplerGroup.add(new THREE.LineSegments(baseGeom, baseMat));

    this.group.add(this._dopplerGroup);
  }

  _buildMotionIndicator() {
    // Pulsating sphere that grows/brightens with motion energy
    this._motionGroup = new THREE.Group();
    this._motionGroup.position.set(2.0, 1.5, 0);

    // Outer glow ring
    const ringGeom = new THREE.RingGeometry(0.25, 0.3, 32);
    const ringMat = new THREE.MeshBasicMaterial({
      color: 0x00ff44,
      transparent: true,
      opacity: 0.3,
      side: THREE.DoubleSide
    });
    this._motionRing = new THREE.Mesh(ringGeom, ringMat);
    this._motionGroup.add(this._motionRing);

    // Inner core
    const coreGeom = new THREE.SphereGeometry(0.15, 16, 16);
    const coreMat = new THREE.MeshBasicMaterial({
      color: 0x004422,
      transparent: true,
      opacity: 0.6
    });
    this._motionCore = new THREE.Mesh(coreGeom, coreMat);
    this._motionGroup.add(this._motionCore);

    // Surrounding pulse rings
    this._pulseRings = [];
    for (let i = 0; i < 3; i++) {
      const pulseGeom = new THREE.RingGeometry(0.3, 0.32, 32);
      const pulseMat = new THREE.MeshBasicMaterial({
        color: 0x00ff88,
        transparent: true,
        opacity: 0,
        side: THREE.DoubleSide
      });
      const ring = new THREE.Mesh(pulseGeom, pulseMat);
      ring.userData.phase = (i / 3) * Math.PI * 2;
      this._motionGroup.add(ring);
      this._pulseRings.push(ring);
    }

    this.group.add(this._motionGroup);
  }

  _buildLabels() {
    // Create text labels using canvas textures
    const labels = [
      { text: 'CSI AMPLITUDE', pos: [0, 5.2, 0], parent: this._heatmapGroup },
      { text: 'PHASE', pos: [0, 0.7, 0], parent: this._phaseGroup },
      { text: 'DOPPLER SPECTRUM', pos: [0, 0.8, 0], parent: this._dopplerGroup },
      { text: 'MOTION', pos: [0, 0.55, 0], parent: this._motionGroup }
    ];

    for (const label of labels) {
      const sprite = this._createTextSprite(label.text, {
        fontSize: 14,
        color: '#5588aa',
        bgColor: 'transparent'
      });
      sprite.position.set(...label.pos);
      sprite.scale.set(1.2, 0.3, 1);
      if (label.parent) {
        label.parent.add(sprite);
      } else {
        this.group.add(sprite);
      }
    }
  }

  _createTextSprite(text, opts = {}) {
    const canvas = document.createElement('canvas');
    const ctx = canvas.getContext('2d');
    canvas.width = 256;
    canvas.height = 64;

    if (opts.bgColor && opts.bgColor !== 'transparent') {
      ctx.fillStyle = opts.bgColor;
      ctx.fillRect(0, 0, canvas.width, canvas.height);
    }

    ctx.font = `${opts.fontSize || 14}px monospace`;
    ctx.fillStyle = opts.color || '#88aacc';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(text, canvas.width / 2, canvas.height / 2);

    const texture = new THREE.CanvasTexture(canvas);
    texture.needsUpdate = true;

    const mat = new THREE.SpriteMaterial({
      map: texture,
      transparent: true,
      depthWrite: false
    });
    return new THREE.Sprite(mat);
  }

  // Feed new CSI data
  // data: { amplitude: Float32Array(30), phase: Float32Array(30), doppler: Float32Array(16), motionEnergy: number }
  updateSignalData(data) {
    if (!data) return;

    // Amplitude: shift history and add new row
    if (data.amplitude) {
      this.amplitudeHistory.shift();
      this.amplitudeHistory.push(new Float32Array(data.amplitude));
    }

    // Phase
    if (data.phase) {
      this.phaseData = new Float32Array(data.phase);
    }

    // Doppler
    if (data.doppler) {
      for (let i = 0; i < Math.min(data.doppler.length, this.config.dopplerBars); i++) {
        this.dopplerData[i] = data.doppler[i];
      }
    }

    // Motion energy
    if (data.motionEnergy !== undefined) {
      this.targetMotionEnergy = Math.max(0, Math.min(1, data.motionEnergy));
    }
  }

  // Call each frame
  update(delta, elapsed) {
    this._updateHeatmap();
    this._updatePhasePlot();
    this._updateDoppler(delta);
    this._updateMotionIndicator(delta, elapsed);
  }

  _updateHeatmap() {
    const { subcarriers, timeSlots } = this.config;
    for (let t = 0; t < timeSlots; t++) {
      const row = this.amplitudeHistory[t];
      for (let s = 0; s < subcarriers; s++) {
        const cell = this._heatmapCells[t][s];
        const val = row[s] || 0;
        // Color: dark blue (0) -> cyan (0.5) -> yellow (0.8) -> red (1.0)
        cell.material.color.setHSL(
          0.6 - val * 0.6,  // hue: 0.6 (blue) -> 0 (red)
          0.9,               // saturation
          0.1 + val * 0.5    // lightness: dim to bright
        );
      }
    }
  }

  _updatePhasePlot() {
    const posAttr = this._phaseLine.geometry.getAttribute('position');
    const arr = posAttr.array;
    const { subcarriers, phaseWidth, phaseHeight } = this.config;

    for (let i = 0; i < subcarriers; i++) {
      const x = (i / (subcarriers - 1)) * phaseWidth - phaseWidth / 2;
      // Phase is in radians, normalize to [-1, 1] range then scale to height
      const phase = this.phaseData[i] || 0;
      const y = (phase / Math.PI) * (phaseHeight / 2);
      arr[i * 3] = x;
      arr[i * 3 + 1] = y;
      arr[i * 3 + 2] = 0;
    }
    posAttr.needsUpdate = true;

    // Color based on phase variance (more variance = more activity = greener/brighter)
    let variance = 0;
    let mean = 0;
    for (let i = 0; i < subcarriers; i++) mean += this.phaseData[i] || 0;
    mean /= subcarriers;
    for (let i = 0; i < subcarriers; i++) {
      const diff = (this.phaseData[i] || 0) - mean;
      variance += diff * diff;
    }
    variance /= subcarriers;
    const activity = Math.min(1, variance / 2);
    this._phaseLine.material.color.setHSL(0.3 - activity * 0.15, 1.0, 0.35 + activity * 0.3);
  }

  _updateDoppler(delta) {
    for (let i = 0; i < this._dopplerBars.length; i++) {
      const bar = this._dopplerBars[i];
      const target = this.dopplerData[i] || 0;
      // Smooth bar height
      const currentH = bar.scale.y;
      bar.scale.y += (target * this.config.dopplerHeight - currentH) * Math.min(1, delta * 8);
      bar.scale.y = Math.max(0.01, bar.scale.y);

      // Position bar bottom at y=0
      bar.position.y = bar.scale.y / 2;

      // Color: blue (low) -> purple (mid) -> magenta (high)
      const val = target;
      bar.material.color.setHSL(
        0.7 - val * 0.3, // blue to magenta
        0.8,
        0.25 + val * 0.35
      );
    }
  }

  _updateMotionIndicator(delta, elapsed) {
    // Smooth motion energy
    this.motionEnergy += (this.targetMotionEnergy - this.motionEnergy) * Math.min(1, delta * 5);

    const energy = this.motionEnergy;

    // Core: grows and brightens with motion
    const coreScale = 0.8 + energy * 0.7;
    this._motionCore.scale.setScalar(coreScale);
    this._motionCore.material.color.setHSL(
      0.3 - energy * 0.2,  // green -> yellow-green
      1.0,
      0.15 + energy * 0.4
    );
    this._motionCore.material.opacity = 0.4 + energy * 0.5;

    // Ring
    this._motionRing.material.opacity = 0.15 + energy * 0.5;
    this._motionRing.material.color.setHSL(0.3 - energy * 0.15, 1.0, 0.4 + energy * 0.3);

    // Pulse rings
    for (const ring of this._pulseRings) {
      const phase = ring.userData.phase + elapsed * (1 + energy * 3);
      const t = (Math.sin(phase) + 1) / 2;
      const scale = 1 + t * energy * 2;
      ring.scale.setScalar(scale);
      ring.material.opacity = (1 - t) * energy * 0.4;
    }
  }

  // Generate synthetic demo signal data
  static generateDemoData(elapsed) {
    const subcarriers = 30;
    const dopplerBars = 16;

    // Amplitude: sinusoidal pattern with noise simulating human movement
    const amplitude = new Float32Array(subcarriers);
    for (let i = 0; i < subcarriers; i++) {
      const baseFreq = Math.sin(elapsed * 2 + i * 0.3) * 0.3;
      const bodyEffect = Math.sin(elapsed * 0.8 + i * 0.15) * 0.25;
      const noise = (Math.random() - 0.5) * 0.1;
      amplitude[i] = Math.max(0, Math.min(1, 0.4 + baseFreq + bodyEffect + noise));
    }

    // Phase: linear with perturbations from movement
    const phase = new Float32Array(subcarriers);
    for (let i = 0; i < subcarriers; i++) {
      const linearPhase = (i / subcarriers) * Math.PI * 2;
      const bodyPhase = Math.sin(elapsed * 1.5 + i * 0.2) * 0.8;
      phase[i] = linearPhase + bodyPhase;
    }

    // Doppler: spectral peaks from movement velocity
    const doppler = new Float32Array(dopplerBars);
    const centerBin = dopplerBars / 2 + Math.sin(elapsed * 0.7) * 3;
    for (let i = 0; i < dopplerBars; i++) {
      const dist = Math.abs(i - centerBin);
      doppler[i] = Math.max(0, Math.exp(-dist * dist * 0.15) * (0.6 + Math.sin(elapsed * 1.2) * 0.3));
      doppler[i] += (Math.random() - 0.5) * 0.05;
      doppler[i] = Math.max(0, Math.min(1, doppler[i]));
    }

    // Motion energy: pulsating
    const motionEnergy = (Math.sin(elapsed * 0.5) + 1) / 2 * 0.7 + 0.15;

    return { amplitude, phase, doppler, motionEnergy };
  }

  getGroup() {
    return this.group;
  }

  dispose() {
    this.group.traverse((child) => {
      if (child.geometry) child.geometry.dispose();
      if (child.material) {
        if (child.material.map) child.material.map.dispose();
        child.material.dispose();
      }
    });
    this.scene.remove(this.group);
  }
}
