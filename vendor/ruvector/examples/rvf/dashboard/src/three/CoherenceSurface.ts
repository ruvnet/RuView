import * as THREE from 'three';

export class CoherenceSurface {
  private mesh: THREE.Mesh | null = null;
  private wireframe: THREE.LineSegments | null = null;
  private contourLines: THREE.Group | null = null;
  private gridLabels: THREE.Group | null = null;
  private scene: THREE.Scene;
  private gridWidth: number;
  private gridHeight: number;

  constructor(scene: THREE.Scene, gridWidth = 64, gridHeight = 64) {
    this.scene = scene;
    this.gridWidth = gridWidth;
    this.gridHeight = gridHeight;
    this.createMesh();
    this.createGridLabels();
  }

  private createMesh(): void {
    const geometry = new THREE.PlaneGeometry(
      10, 10,
      this.gridWidth - 1,
      this.gridHeight - 1,
    );

    const vertexCount = geometry.attributes.position.count;
    const colors = new Float32Array(vertexCount * 3);
    for (let i = 0; i < vertexCount; i++) {
      colors[i * 3] = 0.0;
      colors[i * 3 + 1] = 0.3;
      colors[i * 3 + 2] = 0.5;
    }
    geometry.setAttribute('color', new THREE.BufferAttribute(colors, 3));

    const material = new THREE.MeshPhongMaterial({
      vertexColors: true,
      side: THREE.DoubleSide,
      shininess: 40,
      specular: new THREE.Color(0x112233),
      flatShading: false,
    });

    this.mesh = new THREE.Mesh(geometry, material);
    this.mesh.rotation.x = -Math.PI / 2;
    this.scene.add(this.mesh);

    // Subtle grid overlay
    const wireGeo = new THREE.WireframeGeometry(geometry);
    const wireMat = new THREE.LineBasicMaterial({
      color: 0x1C2333,
      transparent: true,
      opacity: 0.12,
    });
    this.wireframe = new THREE.LineSegments(wireGeo, wireMat);
    this.wireframe.rotation.x = -Math.PI / 2;
    this.scene.add(this.wireframe);
  }

  private createGridLabels(): void {
    this.gridLabels = new THREE.Group();

    // Base grid plane at y=0 with faint lines
    const gridHelper = new THREE.GridHelper(10, 8, 0x1C2333, 0x131A22);
    gridHelper.position.y = -0.01;
    this.gridLabels.add(gridHelper);

    // Axis lines
    const axisMat = new THREE.LineBasicMaterial({ color: 0x2A3444, transparent: true, opacity: 0.5 });
    const xAxisGeo = new THREE.BufferGeometry().setFromPoints([
      new THREE.Vector3(-5.5, 0, 5.5),
      new THREE.Vector3(5.5, 0, 5.5),
    ]);
    this.gridLabels.add(new THREE.Line(xAxisGeo, axisMat));

    const zAxisGeo = new THREE.BufferGeometry().setFromPoints([
      new THREE.Vector3(-5.5, 0, 5.5),
      new THREE.Vector3(-5.5, 0, -5.5),
    ]);
    this.gridLabels.add(new THREE.Line(zAxisGeo, axisMat));

    this.scene.add(this.gridLabels);
  }

  /** Map coherence value [0,1] to a clear multi-stop color ramp. */
  private valueToColor(v: number, color: THREE.Color): void {
    // 1.0 = deep stable blue, 0.85 = cyan, 0.75 = yellow warning, <0.7 = red critical
    if (v > 0.85) {
      // Blue -> Cyan (stable zone)
      const t = (v - 0.85) / 0.15;
      color.setRGB(0.0, 0.4 + t * 0.1, 0.6 + t * 0.4);
    } else if (v > 0.75) {
      // Cyan -> Yellow (transition)
      const t = (v - 0.75) / 0.1;
      color.setRGB(1.0 - t * 1.0, 0.7 + t * 0.2, t * 0.6);
    } else if (v > 0.65) {
      // Yellow -> Orange (warning)
      const t = (v - 0.65) / 0.1;
      color.setRGB(1.0, 0.5 + t * 0.2, t * 0.1);
    } else {
      // Orange -> Red (critical)
      const t = Math.max(0, v / 0.65);
      color.setRGB(0.9 + t * 0.1, 0.15 + t * 0.35, 0.1);
    }
  }

  setValues(values: number[]): void {
    if (!this.mesh) return;

    const geometry = this.mesh.geometry;
    const colorAttr = geometry.attributes.color;
    const posAttr = geometry.attributes.position;
    const count = Math.min(values.length, colorAttr.count);

    const color = new THREE.Color();

    for (let i = 0; i < count; i++) {
      const v = Math.max(0, Math.min(1, values[i]));
      this.valueToColor(v, color);
      colorAttr.setXYZ(i, color.r, color.g, color.b);

      // Elevation: higher coherence = flat, lower = raised (shows "pressure")
      const elevation = (1 - v) * 2.5;
      posAttr.setZ(i, elevation);
    }

    colorAttr.needsUpdate = true;
    posAttr.needsUpdate = true;
    geometry.computeVertexNormals();

    this.updateWireframe(geometry);
    this.updateContours(values);
  }

  private updateWireframe(geometry: THREE.PlaneGeometry): void {
    if (this.wireframe) {
      this.scene.remove(this.wireframe);
      this.wireframe.geometry.dispose();
      (this.wireframe.material as THREE.Material).dispose();
    }

    const wireGeo = new THREE.WireframeGeometry(geometry);
    const wireMat = new THREE.LineBasicMaterial({
      color: 0x1C2333,
      transparent: true,
      opacity: 0.12,
    });
    this.wireframe = new THREE.LineSegments(wireGeo, wireMat);
    this.wireframe.rotation.x = -Math.PI / 2;
    this.scene.add(this.wireframe);
  }

  /** Draw contour rings at threshold boundaries (0.8 warning, 0.7 critical). */
  private updateContours(values: number[]): void {
    if (this.contourLines) {
      this.scene.remove(this.contourLines);
      this.contourLines.traverse((obj) => {
        if (obj instanceof THREE.Line) {
          obj.geometry.dispose();
          (obj.material as THREE.Material).dispose();
        }
      });
    }

    this.contourLines = new THREE.Group();
    const gw = this.gridWidth;
    const gh = this.gridHeight;
    const halfW = 5;

    const thresholds = [
      { level: 0.80, color: 0xFFB020, opacity: 0.6 },  // warning
      { level: 0.70, color: 0xFF4D4D, opacity: 0.7 },  // critical
    ];

    for (const thresh of thresholds) {
      const points: THREE.Vector3[] = [];

      for (let y = 0; y < gh - 1; y++) {
        for (let x = 0; x < gw - 1; x++) {
          const v00 = values[y * gw + x] ?? 1;
          const v10 = values[y * gw + x + 1] ?? 1;
          const v01 = values[(y + 1) * gw + x] ?? 1;

          // Horizontal edge crossing
          if ((v00 - thresh.level) * (v10 - thresh.level) < 0) {
            const t = (thresh.level - v00) / (v10 - v00);
            const wx = -halfW + ((x + t) / (gw - 1)) * halfW * 2;
            const wz = -halfW + (y / (gh - 1)) * halfW * 2;
            const elev = (1 - thresh.level) * 2.5;
            points.push(new THREE.Vector3(wx, elev + 0.02, wz));
          }

          // Vertical edge crossing
          if ((v00 - thresh.level) * (v01 - thresh.level) < 0) {
            const t = (thresh.level - v00) / (v01 - v00);
            const wx = -halfW + (x / (gw - 1)) * halfW * 2;
            const wz = -halfW + ((y + t) / (gh - 1)) * halfW * 2;
            const elev = (1 - thresh.level) * 2.5;
            points.push(new THREE.Vector3(wx, elev + 0.02, wz));
          }
        }
      }

      if (points.length > 1) {
        const geo = new THREE.BufferGeometry().setFromPoints(points);
        const mat = new THREE.PointsMaterial({
          color: thresh.color,
          size: 0.08,
          transparent: true,
          opacity: thresh.opacity,
          depthWrite: false,
        });
        this.contourLines.add(new THREE.Points(geo, mat));
      }
    }

    this.scene.add(this.contourLines);
  }

  dispose(): void {
    if (this.mesh) {
      this.scene.remove(this.mesh);
      this.mesh.geometry.dispose();
      (this.mesh.material as THREE.Material).dispose();
      this.mesh = null;
    }
    if (this.wireframe) {
      this.scene.remove(this.wireframe);
      this.wireframe.geometry.dispose();
      (this.wireframe.material as THREE.Material).dispose();
      this.wireframe = null;
    }
    if (this.contourLines) {
      this.scene.remove(this.contourLines);
      this.contourLines.traverse((obj) => {
        if (obj instanceof THREE.Line || obj instanceof THREE.Points) {
          obj.geometry.dispose();
          (obj.material as THREE.Material).dispose();
        }
      });
      this.contourLines = null;
    }
    if (this.gridLabels) {
      this.scene.remove(this.gridLabels);
      this.gridLabels = null;
    }
  }
}
