/**
 * Module D — "The Phase Constellation"
 * I/Q star map with constellation lines and rotating temporal view
 */
import * as THREE from 'three';

const NUM_SUBCARRIERS = 64;

export class PhaseConstellation {
  constructor(scene, panelGroup) {
    this.group = new THREE.Group();
    if (panelGroup) panelGroup.add(this.group);
    else scene.add(this.group);

    // Star points (current frame)
    const starGeo = new THREE.BufferGeometry();
    this._positions = new Float32Array(NUM_SUBCARRIERS * 3);
    this._colors = new Float32Array(NUM_SUBCARRIERS * 3);
    this._sizes = new Float32Array(NUM_SUBCARRIERS);

    starGeo.setAttribute('position', new THREE.BufferAttribute(this._positions, 3));
    starGeo.setAttribute('color', new THREE.BufferAttribute(this._colors, 3));
    starGeo.setAttribute('size', new THREE.BufferAttribute(this._sizes, 1));

    const starMat = new THREE.PointsMaterial({
      size: 0.12,
      vertexColors: true,
      transparent: true,
      opacity: 0.9,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
      sizeAttenuation: true,
    });
    this._stars = new THREE.Points(starGeo, starMat);
    this.group.add(this._stars);

    // Ghost layer (previous frame)
    const ghostGeo = new THREE.BufferGeometry();
    this._ghostPos = new Float32Array(NUM_SUBCARRIERS * 3);
    ghostGeo.setAttribute('position', new THREE.BufferAttribute(this._ghostPos, 3));

    const ghostMat = new THREE.PointsMaterial({
      color: 0x00d4ff,
      size: 0.06,
      transparent: true,
      opacity: 0.2,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
      sizeAttenuation: true,
    });
    this._ghosts = new THREE.Points(ghostGeo, ghostMat);
    this.group.add(this._ghosts);

    // Constellation lines (connecting adjacent subcarriers)
    const lineGeo = new THREE.BufferGeometry();
    this._linePos = new Float32Array(NUM_SUBCARRIERS * 2 * 3); // pairs
    lineGeo.setAttribute('position', new THREE.BufferAttribute(this._linePos, 3));

    const lineMat = new THREE.LineBasicMaterial({
      color: 0x00d4ff,
      transparent: true,
      opacity: 0.15,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
    });
    this._lines = new THREE.LineSegments(lineGeo, lineMat);
    this.group.add(this._lines);

    // Axes
    this._addAxes();

    this._prevIQ = null;
  }

  _addAxes() {
    const axesMat = new THREE.LineBasicMaterial({
      color: 0x00d4ff,
      transparent: true,
      opacity: 0.1,
    });

    // I axis
    const iGeo = new THREE.BufferGeometry().setFromPoints([
      new THREE.Vector3(-2.5, 0, 0),
      new THREE.Vector3(2.5, 0, 0),
    ]);
    this.group.add(new THREE.Line(iGeo, axesMat));

    // Q axis
    const qGeo = new THREE.BufferGeometry().setFromPoints([
      new THREE.Vector3(0, -2.5, 0),
      new THREE.Vector3(0, 2.5, 0),
    ]);
    this.group.add(new THREE.Line(qGeo, axesMat));
  }

  update(dt, elapsed, data) {
    const iq = data?._observatory?.subcarrier_iq;
    const variance = data?._observatory?.per_subcarrier_variance;
    const amplitude = data?.nodes?.[0]?.amplitude;

    // Slow Y rotation for temporal evolution
    this.group.rotation.y = elapsed * 0.05;

    if (!iq || iq.length < NUM_SUBCARRIERS) return;

    // Copy current to ghost
    this._ghostPos.set(this._positions);
    this._ghosts.geometry.attributes.position.needsUpdate = true;

    // Update current positions from I/Q
    for (let s = 0; s < NUM_SUBCARRIERS; s++) {
      const i3 = s * 3;
      const iVal = (iq[s]?.i || 0) * 4; // scale for visibility
      const qVal = (iq[s]?.q || 0) * 4;

      this._positions[i3] = iVal;
      this._positions[i3 + 1] = qVal;
      this._positions[i3 + 2] = 0;

      // Size from amplitude
      const amp = amplitude ? (amplitude[s % amplitude.length] || 0.1) : 0.1;
      this._sizes[s] = 0.06 + amp * 0.15;

      // Color from variance: blue(low) -> amber(high)
      const v = variance ? Math.min(1, (variance[s] || 0) * 2) : 0;
      this._colors[i3] = v * 1.0;              // R
      this._colors[i3 + 1] = 0.5 + v * 0.3;   // G
      this._colors[i3 + 2] = 1.0 - v * 0.7;   // B
    }

    this._stars.geometry.attributes.position.needsUpdate = true;
    this._stars.geometry.attributes.color.needsUpdate = true;
    this._stars.geometry.attributes.size.needsUpdate = true;

    // Update constellation lines
    for (let s = 0; s < NUM_SUBCARRIERS - 1; s++) {
      const li = s * 6;
      const i3a = s * 3;
      const i3b = (s + 1) * 3;

      this._linePos[li] = this._positions[i3a];
      this._linePos[li + 1] = this._positions[i3a + 1];
      this._linePos[li + 2] = this._positions[i3a + 2];
      this._linePos[li + 3] = this._positions[i3b];
      this._linePos[li + 4] = this._positions[i3b + 1];
      this._linePos[li + 5] = this._positions[i3b + 2];
    }
    // Last pair: wrap around
    const lastLi = (NUM_SUBCARRIERS - 1) * 6;
    const lastI3 = (NUM_SUBCARRIERS - 1) * 3;
    this._linePos[lastLi] = this._positions[lastI3];
    this._linePos[lastLi + 1] = this._positions[lastI3 + 1];
    this._linePos[lastLi + 2] = this._positions[lastI3 + 2];
    this._linePos[lastLi + 3] = this._positions[0];
    this._linePos[lastLi + 4] = this._positions[1];
    this._linePos[lastLi + 5] = this._positions[2];

    this._lines.geometry.attributes.position.needsUpdate = true;
  }

  dispose() {
    this._stars.geometry.dispose();
    this._stars.material.dispose();
    this._ghosts.geometry.dispose();
    this._ghosts.material.dispose();
    this._lines.geometry.dispose();
    this._lines.material.dispose();
  }
}
