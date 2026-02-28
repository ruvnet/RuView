// Room Environment - WiFi DensePose 3D Visualization
// Grid floor, AP/receiver markers, detection zones, confidence heatmap

export class Environment {
  constructor(scene) {
    this.scene = scene;
    this.group = new THREE.Group();
    this.group.name = 'environment';

    // Room dimensions (meters)
    this.roomWidth = 8;
    this.roomDepth = 6;
    this.roomHeight = 3;

    // AP and receiver positions
    this.accessPoints = [
      { id: 'TX1', pos: [-3.5, 2.5, -2.8], type: 'transmitter' },
      { id: 'TX2', pos: [0, 2.5, -2.8], type: 'transmitter' },
      { id: 'TX3', pos: [3.5, 2.5, -2.8], type: 'transmitter' }
    ];
    this.receivers = [
      { id: 'RX1', pos: [-3.5, 2.5, 2.8], type: 'receiver' },
      { id: 'RX2', pos: [0, 2.5, 2.8], type: 'receiver' },
      { id: 'RX3', pos: [3.5, 2.5, 2.8], type: 'receiver' }
    ];

    // Detection zones
    this.zones = [
      { id: 'zone_1', center: [-2, 0, 0], radius: 2, color: 0x0066ff, label: 'Zone 1' },
      { id: 'zone_2', center: [0, 0, 0], radius: 2, color: 0x00cc66, label: 'Zone 2' },
      { id: 'zone_3', center: [2, 0, 0], radius: 2, color: 0xff6600, label: 'Zone 3' }
    ];

    // Confidence heatmap state
    this._heatmapData = new Float32Array(20 * 15); // 20x15 grid
    this._heatmapCells = [];

    // Build everything
    this._buildFloor();
    this._buildGrid();
    this._buildWalls();
    this._buildAPMarkers();
    this._buildSignalPaths();
    this._buildDetectionZones();
    this._buildConfidenceHeatmap();

    this.scene.add(this.group);
  }

  _buildFloor() {
    // Dark reflective floor
    const floorGeom = new THREE.PlaneGeometry(this.roomWidth, this.roomDepth);
    const floorMat = new THREE.MeshPhongMaterial({
      color: 0x0a0a15,
      emissive: 0x050510,
      shininess: 60,
      specular: 0x111122,
      transparent: true,
      opacity: 0.95,
      side: THREE.DoubleSide
    });
    const floor = new THREE.Mesh(floorGeom, floorMat);
    floor.rotation.x = -Math.PI / 2;
    floor.position.y = 0;
    floor.receiveShadow = true;
    this.group.add(floor);
  }

  _buildGrid() {
    // Grid lines on the floor
    const gridGroup = new THREE.Group();
    const gridMat = new THREE.LineBasicMaterial({
      color: 0x1a1a3a,
      transparent: true,
      opacity: 0.4
    });

    const halfW = this.roomWidth / 2;
    const halfD = this.roomDepth / 2;
    const step = 0.5;

    // Lines along X
    for (let z = -halfD; z <= halfD; z += step) {
      const geom = new THREE.BufferGeometry();
      const positions = new Float32Array([-halfW, 0.005, z, halfW, 0.005, z]);
      geom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
      gridGroup.add(new THREE.Line(geom, gridMat));
    }

    // Lines along Z
    for (let x = -halfW; x <= halfW; x += step) {
      const geom = new THREE.BufferGeometry();
      const positions = new Float32Array([x, 0.005, -halfD, x, 0.005, halfD]);
      geom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
      gridGroup.add(new THREE.Line(geom, gridMat));
    }

    // Brighter center lines
    const centerMat = new THREE.LineBasicMaterial({
      color: 0x2233aa,
      transparent: true,
      opacity: 0.25
    });
    const centerX = new THREE.BufferGeometry();
    centerX.setAttribute('position', new THREE.BufferAttribute(
      new Float32Array([-halfW, 0.006, 0, halfW, 0.006, 0]), 3));
    gridGroup.add(new THREE.Line(centerX, centerMat));

    const centerZ = new THREE.BufferGeometry();
    centerZ.setAttribute('position', new THREE.BufferAttribute(
      new Float32Array([0, 0.006, -halfD, 0, 0.006, halfD]), 3));
    gridGroup.add(new THREE.Line(centerZ, centerMat));

    this.group.add(gridGroup);
  }

  _buildWalls() {
    // Subtle transparent walls to define the room boundary
    const wallMat = new THREE.MeshBasicMaterial({
      color: 0x112244,
      transparent: true,
      opacity: 0.06,
      side: THREE.DoubleSide,
      depthWrite: false
    });

    const halfW = this.roomWidth / 2;
    const halfD = this.roomDepth / 2;
    const h = this.roomHeight;

    // Back wall
    const backWall = new THREE.Mesh(new THREE.PlaneGeometry(this.roomWidth, h), wallMat);
    backWall.position.set(0, h / 2, -halfD);
    this.group.add(backWall);

    // Front wall (more transparent)
    const frontMat = wallMat.clone();
    frontMat.opacity = 0.03;
    const frontWall = new THREE.Mesh(new THREE.PlaneGeometry(this.roomWidth, h), frontMat);
    frontWall.position.set(0, h / 2, halfD);
    this.group.add(frontWall);

    // Side walls
    const leftWall = new THREE.Mesh(new THREE.PlaneGeometry(this.roomDepth, h), wallMat);
    leftWall.rotation.y = Math.PI / 2;
    leftWall.position.set(-halfW, h / 2, 0);
    this.group.add(leftWall);

    const rightWall = new THREE.Mesh(new THREE.PlaneGeometry(this.roomDepth, h), wallMat);
    rightWall.rotation.y = -Math.PI / 2;
    rightWall.position.set(halfW, h / 2, 0);
    this.group.add(rightWall);

    // Wall edge lines
    const edgeMat = new THREE.LineBasicMaterial({
      color: 0x334466,
      transparent: true,
      opacity: 0.3
    });
    const edges = [
      // Floor edges
      [-halfW, 0, -halfD, halfW, 0, -halfD],
      [halfW, 0, -halfD, halfW, 0, halfD],
      [halfW, 0, halfD, -halfW, 0, halfD],
      [-halfW, 0, halfD, -halfW, 0, -halfD],
      // Ceiling edges
      [-halfW, h, -halfD, halfW, h, -halfD],
      [halfW, h, -halfD, halfW, h, halfD],
      [-halfW, h, halfD, -halfW, h, -halfD],
      // Vertical edges
      [-halfW, 0, -halfD, -halfW, h, -halfD],
      [halfW, 0, -halfD, halfW, h, -halfD],
      [-halfW, 0, halfD, -halfW, h, halfD],
      [halfW, 0, halfD, halfW, h, halfD]
    ];

    for (const e of edges) {
      const geom = new THREE.BufferGeometry();
      geom.setAttribute('position', new THREE.BufferAttribute(new Float32Array(e), 3));
      this.group.add(new THREE.Line(geom, edgeMat));
    }
  }

  _buildAPMarkers() {
    this._apMeshes = [];
    this._rxMeshes = [];

    // Transmitter markers: small pyramid/cone shape, blue
    const txGeom = new THREE.ConeGeometry(0.12, 0.25, 4);
    const txMat = new THREE.MeshPhongMaterial({
      color: 0x0088ff,
      emissive: 0x003366,
      emissiveIntensity: 0.5,
      transparent: true,
      opacity: 0.9
    });

    for (const ap of this.accessPoints) {
      const mesh = new THREE.Mesh(txGeom, txMat.clone());
      mesh.position.set(...ap.pos);
      mesh.rotation.z = Math.PI; // Point downward
      mesh.castShadow = true;
      mesh.name = `ap-${ap.id}`;
      this.group.add(mesh);
      this._apMeshes.push(mesh);

      // Small point light at each AP
      const light = new THREE.PointLight(0x0066ff, 0.3, 4);
      light.position.set(...ap.pos);
      this.group.add(light);

      // Label
      const label = this._createLabel(ap.id, 0x0088ff);
      label.position.set(ap.pos[0], ap.pos[1] + 0.3, ap.pos[2]);
      this.group.add(label);
    }

    // Receiver markers: inverted cone, green
    const rxGeom = new THREE.ConeGeometry(0.12, 0.25, 4);
    const rxMat = new THREE.MeshPhongMaterial({
      color: 0x00cc44,
      emissive: 0x004422,
      emissiveIntensity: 0.5,
      transparent: true,
      opacity: 0.9
    });

    for (const rx of this.receivers) {
      const mesh = new THREE.Mesh(rxGeom, rxMat.clone());
      mesh.position.set(...rx.pos);
      mesh.castShadow = true;
      mesh.name = `rx-${rx.id}`;
      this.group.add(mesh);
      this._rxMeshes.push(mesh);

      // Small point light
      const light = new THREE.PointLight(0x00cc44, 0.2, 3);
      light.position.set(...rx.pos);
      this.group.add(light);

      // Label
      const label = this._createLabel(rx.id, 0x00cc44);
      label.position.set(rx.pos[0], rx.pos[1] + 0.3, rx.pos[2]);
      this.group.add(label);
    }
  }

  _buildSignalPaths() {
    // Dashed lines from each TX to each RX showing WiFi signal paths
    this._signalLines = [];
    const lineMat = new THREE.LineDashedMaterial({
      color: 0x1133aa,
      transparent: true,
      opacity: 0.15,
      dashSize: 0.15,
      gapSize: 0.1,
      linewidth: 1
    });

    for (const tx of this.accessPoints) {
      for (const rx of this.receivers) {
        const geom = new THREE.BufferGeometry();
        const positions = new Float32Array([...tx.pos, ...rx.pos]);
        geom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
        const line = new THREE.Line(geom, lineMat.clone());
        line.computeLineDistances();
        this.group.add(line);
        this._signalLines.push(line);
      }
    }
  }

  _buildDetectionZones() {
    this._zoneMeshes = {};

    for (const zone of this.zones) {
      const zoneGroup = new THREE.Group();
      zoneGroup.name = `zone-${zone.id}`;

      // Zone circle on floor
      const circleGeom = new THREE.RingGeometry(zone.radius * 0.95, zone.radius, 48);
      const circleMat = new THREE.MeshBasicMaterial({
        color: zone.color,
        transparent: true,
        opacity: 0.12,
        side: THREE.DoubleSide,
        depthWrite: false
      });
      const circle = new THREE.Mesh(circleGeom, circleMat);
      circle.rotation.x = -Math.PI / 2;
      circle.position.set(zone.center[0], 0.01, zone.center[2]);
      zoneGroup.add(circle);

      // Zone fill
      const fillGeom = new THREE.CircleGeometry(zone.radius * 0.95, 48);
      const fillMat = new THREE.MeshBasicMaterial({
        color: zone.color,
        transparent: true,
        opacity: 0.04,
        side: THREE.DoubleSide,
        depthWrite: false
      });
      const fill = new THREE.Mesh(fillGeom, fillMat);
      fill.rotation.x = -Math.PI / 2;
      fill.position.set(zone.center[0], 0.008, zone.center[2]);
      zoneGroup.add(fill);

      // Zone label
      const label = this._createLabel(zone.label, zone.color);
      label.position.set(zone.center[0], 0.15, zone.center[2] + zone.radius + 0.2);
      label.scale.set(1.0, 0.25, 1);
      zoneGroup.add(label);

      this.group.add(zoneGroup);
      this._zoneMeshes[zone.id] = { group: zoneGroup, circle, fill, circleMat, fillMat };
    }
  }

  _buildConfidenceHeatmap() {
    // Ground-level heatmap showing detection confidence across the room
    const cols = 20;
    const rows = 15;
    const cellW = this.roomWidth / cols;
    const cellD = this.roomDepth / rows;
    const cellGeom = new THREE.PlaneGeometry(cellW * 0.95, cellD * 0.95);

    this._heatmapGroup = new THREE.Group();
    this._heatmapGroup.position.y = 0.003;

    for (let r = 0; r < rows; r++) {
      const rowCells = [];
      for (let c = 0; c < cols; c++) {
        const mat = new THREE.MeshBasicMaterial({
          color: 0x000000,
          transparent: true,
          opacity: 0,
          side: THREE.DoubleSide,
          depthWrite: false
        });
        const cell = new THREE.Mesh(cellGeom, mat);
        cell.rotation.x = -Math.PI / 2;
        cell.position.set(
          (c + 0.5) * cellW - this.roomWidth / 2,
          0,
          (r + 0.5) * cellD - this.roomDepth / 2
        );
        this._heatmapGroup.add(cell);
        rowCells.push(cell);
      }
      this._heatmapCells.push(rowCells);
    }

    this.group.add(this._heatmapGroup);
  }

  _createLabel(text, color) {
    const canvas = document.createElement('canvas');
    const ctx = canvas.getContext('2d');
    canvas.width = 128;
    canvas.height = 32;

    ctx.font = 'bold 14px monospace';
    ctx.fillStyle = '#' + new THREE.Color(color).getHexString();
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(text, canvas.width / 2, canvas.height / 2);

    const texture = new THREE.CanvasTexture(canvas);
    const mat = new THREE.SpriteMaterial({
      map: texture,
      transparent: true,
      depthWrite: false
    });
    return new THREE.Sprite(mat);
  }

  // Update zone occupancy display
  // zoneOccupancy: { zone_1: count, zone_2: count, ... }
  updateZoneOccupancy(zoneOccupancy) {
    if (!zoneOccupancy) return;

    for (const [zoneId, meshes] of Object.entries(this._zoneMeshes)) {
      const count = zoneOccupancy[zoneId] || 0;
      const isOccupied = count > 0;

      // Brighten occupied zones
      meshes.circleMat.opacity = isOccupied ? 0.25 : 0.08;
      meshes.fillMat.opacity = isOccupied ? 0.10 : 0.03;
    }
  }

  // Update confidence heatmap from detection data
  // confidenceMap: 2D array or flat array of confidence values [0,1]
  updateConfidenceHeatmap(confidenceMap) {
    if (!confidenceMap) return;
    const rows = this._heatmapCells.length;
    const cols = this._heatmapCells[0]?.length || 0;

    for (let r = 0; r < rows; r++) {
      for (let c = 0; c < cols; c++) {
        const idx = r * cols + c;
        const val = Array.isArray(confidenceMap)
          ? (Array.isArray(confidenceMap[r]) ? confidenceMap[r][c] : confidenceMap[idx])
          : (confidenceMap[idx] || 0);

        const cell = this._heatmapCells[r][c];
        if (val > 0.01) {
          // Color temperature: blue (low) -> green (mid) -> red (high)
          cell.material.color.setHSL(0.6 - val * 0.6, 1.0, 0.3 + val * 0.3);
          cell.material.opacity = val * 0.3;
        } else {
          cell.material.opacity = 0;
        }
      }
    }
  }

  // Generate a demo confidence heatmap centered on given positions
  static generateDemoHeatmap(personPositions, cols, rows, roomWidth, roomDepth) {
    const map = new Float32Array(cols * rows);
    const cellW = roomWidth / cols;
    const cellD = roomDepth / rows;

    for (const pos of (personPositions || [])) {
      for (let r = 0; r < rows; r++) {
        for (let c = 0; c < cols; c++) {
          const cx = (c + 0.5) * cellW - roomWidth / 2;
          const cz = (r + 0.5) * cellD - roomDepth / 2;
          const dx = cx - (pos.x || 0);
          const dz = cz - (pos.z || 0);
          const dist = Math.sqrt(dx * dx + dz * dz);
          const conf = Math.exp(-dist * dist * 0.5) * (pos.confidence || 0.8);
          map[r * cols + c] = Math.max(map[r * cols + c], conf);
        }
      }
    }
    return map;
  }

  // Animate AP and RX markers (subtle pulse)
  update(delta, elapsed) {
    // Pulse AP markers
    for (const mesh of this._apMeshes) {
      const pulse = 0.9 + Math.sin(elapsed * 2) * 0.1;
      mesh.scale.setScalar(pulse);
      mesh.material.emissiveIntensity = 0.3 + Math.sin(elapsed * 3) * 0.15;
    }

    // Pulse RX markers
    for (const mesh of this._rxMeshes) {
      const pulse = 0.9 + Math.sin(elapsed * 2 + Math.PI) * 0.1;
      mesh.scale.setScalar(pulse);
      mesh.material.emissiveIntensity = 0.3 + Math.sin(elapsed * 3 + Math.PI) * 0.15;
    }

    // Animate signal paths subtly
    for (const line of this._signalLines) {
      line.material.opacity = 0.08 + Math.sin(elapsed * 1.5) * 0.05;
    }
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
