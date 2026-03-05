/**
 * Module C — "Presence Cartography"
 * InstancedMesh 20x4x20 voxel heatmap with person lights
 */
import * as THREE from 'three';

const GRID_X = 20;
const GRID_Y = 4;
const GRID_Z = 20;
const TOTAL_VOXELS = GRID_X * GRID_Y * GRID_Z;
const VOXEL_SIZE = 0.22;

export class PresenceCartography {
  constructor(scene, panelGroup) {
    this.group = new THREE.Group();
    if (panelGroup) panelGroup.add(this.group);
    else scene.add(this.group);

    // Instanced cubes
    const cubeGeo = new THREE.BoxGeometry(VOXEL_SIZE, VOXEL_SIZE, VOXEL_SIZE);
    const cubeMat = new THREE.MeshBasicMaterial({
      color: 0xffffff,
      transparent: true,
      opacity: 1,
      blending: THREE.AdditiveBlending,
      depthWrite: false,
    });

    this._mesh = new THREE.InstancedMesh(cubeGeo, cubeMat, TOTAL_VOXELS);
    this._mesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);

    // Color attribute
    this._colors = new Float32Array(TOTAL_VOXELS * 3);
    this._mesh.instanceColor = new THREE.InstancedBufferAttribute(this._colors, 3);

    // Initialize positions
    const dummy = new THREE.Object3D();
    const halfX = (GRID_X * VOXEL_SIZE * 1.1) / 2;
    const halfZ = (GRID_Z * VOXEL_SIZE * 1.1) / 2;

    for (let y = 0; y < GRID_Y; y++) {
      for (let z = 0; z < GRID_Z; z++) {
        for (let x = 0; x < GRID_X; x++) {
          const idx = y * GRID_Z * GRID_X + z * GRID_X + x;
          dummy.position.set(
            x * VOXEL_SIZE * 1.1 - halfX,
            y * VOXEL_SIZE * 1.1,
            z * VOXEL_SIZE * 1.1 - halfZ
          );
          dummy.scale.set(0.01, 0.01, 0.01); // start invisible
          dummy.updateMatrix();
          this._mesh.setMatrixAt(idx, dummy.matrix);

          this._colors[idx * 3] = 0;
          this._colors[idx * 3 + 1] = 0.2;
          this._colors[idx * 3 + 2] = 0.4;
        }
      }
    }
    this._mesh.instanceMatrix.needsUpdate = true;
    this._mesh.instanceColor.needsUpdate = true;
    this.group.add(this._mesh);

    // Room wireframe
    const roomW = GRID_X * VOXEL_SIZE * 1.1;
    const roomH = GRID_Y * VOXEL_SIZE * 1.1;
    const roomD = GRID_Z * VOXEL_SIZE * 1.1;
    const boxGeo = new THREE.BoxGeometry(roomW, roomH, roomD);
    const edges = new THREE.EdgesGeometry(boxGeo);
    const lineMat = new THREE.LineBasicMaterial({
      color: 0x00d4ff,
      transparent: true,
      opacity: 0.15,
    });
    const wireframe = new THREE.LineSegments(edges, lineMat);
    wireframe.position.y = roomH / 2;
    this.group.add(wireframe);

    // Person lights (up to 4)
    this._personLights = [];
    for (let i = 0; i < 4; i++) {
      const light = new THREE.PointLight(0xff8800, 0, 3);
      this.group.add(light);
      this._personLights.push(light);
    }

    this._dummy = new THREE.Object3D();
    this._halfX = halfX;
    this._halfZ = halfZ;
  }

  update(dt, elapsed, data) {
    const field = data?.signal_field?.values;
    const persons = data?.persons || [];

    const dummy = this._dummy;

    if (field && field.length >= GRID_X * GRID_Z) {
      for (let y = 0; y < GRID_Y; y++) {
        for (let z = 0; z < GRID_Z; z++) {
          for (let x = 0; x < GRID_X; x++) {
            const idx = y * GRID_Z * GRID_X + z * GRID_X + x;
            const fieldIdx = z * GRID_X + x;
            const val = field[fieldIdx] || 0;

            // Extrude vertically: layer 0 = full val, higher layers diminish
            const layerFactor = Math.max(0, 1 - y / GRID_Y);
            const v = val * layerFactor;

            // Scale voxel by value
            const s = v > 0.05 ? 0.3 + v * 0.7 : 0.01;
            dummy.position.set(
              x * VOXEL_SIZE * 1.1 - this._halfX,
              y * VOXEL_SIZE * 1.1,
              z * VOXEL_SIZE * 1.1 - this._halfZ
            );
            dummy.scale.set(s, s, s);
            dummy.updateMatrix();
            this._mesh.setMatrixAt(idx, dummy.matrix);

            // Color: blue(low) -> cyan(mid) -> amber(high)
            let r, g, b;
            if (v < 0.3) {
              const t = v / 0.3;
              r = 0.02;
              g = 0.06 + t * 0.6;
              b = 0.2 + t * 0.6;
            } else if (v < 0.6) {
              const t = (v - 0.3) / 0.3;
              r = t * 0.8;
              g = 0.66 + t * 0.2;
              b = 0.8 - t * 0.5;
            } else {
              const t = (v - 0.6) / 0.4;
              r = 0.8 + t * 0.2;
              g = 0.86 - t * 0.5;
              b = 0.3 - t * 0.3;
            }
            this._colors[idx * 3] = r;
            this._colors[idx * 3 + 1] = g;
            this._colors[idx * 3 + 2] = b;
          }
        }
      }
      this._mesh.instanceMatrix.needsUpdate = true;
      this._mesh.instanceColor.needsUpdate = true;
    }

    // Person lights
    for (let i = 0; i < this._personLights.length; i++) {
      const light = this._personLights[i];
      if (i < persons.length) {
        const p = persons[i].position || [0, 0, 0];
        light.position.set(p[0] * 2, 1.5, p[2] * 2);
        light.intensity = 1.5 + Math.sin(elapsed * 3 + i) * 0.5;
        light.color.setHex(0xff8800);
      } else {
        light.intensity = 0;
      }
    }
  }

  /** Reduce voxel count for performance */
  setQuality(level) {
    // For now just toggle visibility of upper layers
    // level 0 = show only ground, 2 = show all
    this._mesh.count = level === 0
      ? GRID_X * GRID_Z
      : level === 1
        ? GRID_X * GRID_Z * 2
        : TOTAL_VOXELS;
  }

  dispose() {
    this._mesh.geometry.dispose();
    this._mesh.material.dispose();
  }
}
