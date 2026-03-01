import * as THREE from 'three';

export interface GraphNode {
  id: string;
  domain: string;
  x: number;
  y: number;
  z: number;
  weight: number;
}

export interface GraphEdge {
  source: string;
  target: string;
  weight: number;
}

const DOMAIN_COLORS: Record<string, THREE.Color> = {
  transit: new THREE.Color(0x00E5FF),
  flare: new THREE.Color(0xFF4D4D),
  rotation: new THREE.Color(0x2ECC71),
  eclipse: new THREE.Color(0x9944ff),
  variability: new THREE.Color(0xFFB020),
};

const DEFAULT_COLOR = new THREE.Color(0x8B949E);

function colorForDomain(domain: string): THREE.Color {
  return DOMAIN_COLORS[domain] ?? DEFAULT_COLOR;
}

export class AtlasGraph {
  private nodesMesh: THREE.InstancedMesh | null = null;
  private edgesLine: THREE.LineSegments | null = null;
  private glowPoints: THREE.Points | null = null;
  private scene: THREE.Scene;
  private nodeMap: Map<string, number> = new Map();

  constructor(scene: THREE.Scene) {
    this.scene = scene;
  }

  setNodes(nodes: GraphNode[]): void {
    this.disposeNodes();

    // Star-like nodes using InstancedMesh with emissive material
    const geometry = new THREE.SphereGeometry(0.12, 8, 6);
    const material = new THREE.MeshStandardMaterial({
      vertexColors: false,
      emissiveIntensity: 0.8,
      roughness: 0.3,
      metalness: 0.1,
    });
    const mesh = new THREE.InstancedMesh(geometry, material, nodes.length);

    const dummy = new THREE.Object3D();
    const color = new THREE.Color();

    for (let i = 0; i < nodes.length; i++) {
      const node = nodes[i];
      this.nodeMap.set(node.id, i);

      dummy.position.set(node.x, node.y, node.z);
      const scale = 0.3 + node.weight * 0.7;
      dummy.scale.set(scale, scale, scale);
      dummy.updateMatrix();
      mesh.setMatrixAt(i, dummy.matrix);

      color.copy(colorForDomain(node.domain));
      mesh.setColorAt(i, color);
    }

    mesh.instanceMatrix.needsUpdate = true;
    if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;

    this.nodesMesh = mesh;
    this.scene.add(mesh);

    // Additive glow halo points around each node
    const glowPositions = new Float32Array(nodes.length * 3);
    const glowColors = new Float32Array(nodes.length * 3);
    const glowSizes = new Float32Array(nodes.length);

    for (let i = 0; i < nodes.length; i++) {
      const node = nodes[i];
      glowPositions[i * 3] = node.x;
      glowPositions[i * 3 + 1] = node.y;
      glowPositions[i * 3 + 2] = node.z;

      const c = colorForDomain(node.domain);
      glowColors[i * 3] = c.r;
      glowColors[i * 3 + 1] = c.g;
      glowColors[i * 3 + 2] = c.b;

      glowSizes[i] = 0.8 + node.weight * 1.5;
    }

    const glowGeo = new THREE.BufferGeometry();
    glowGeo.setAttribute('position', new THREE.Float32BufferAttribute(glowPositions, 3));
    glowGeo.setAttribute('color', new THREE.Float32BufferAttribute(glowColors, 3));

    const glowMat = new THREE.PointsMaterial({
      size: 1.2,
      vertexColors: true,
      transparent: true,
      opacity: 0.25,
      sizeAttenuation: true,
      depthWrite: false,
      blending: THREE.AdditiveBlending,
    });

    this.glowPoints = new THREE.Points(glowGeo, glowMat);
    this.scene.add(this.glowPoints);
  }

  setEdges(edges: GraphEdge[], nodes: GraphNode[]): void {
    this.disposeEdges();

    const positions: number[] = [];
    const colors: number[] = [];

    const nodeById = new Map<string, GraphNode>();
    for (const n of nodes) nodeById.set(n.id, n);

    for (const edge of edges) {
      const src = nodeById.get(edge.source);
      const tgt = nodeById.get(edge.target);
      if (!src || !tgt) continue;

      positions.push(src.x, src.y, src.z);
      positions.push(tgt.x, tgt.y, tgt.z);

      // Cyan glow edges with weight-based opacity
      const alpha = Math.max(0.05, Math.min(0.6, edge.weight * 0.5));
      colors.push(0.0, 0.9, 1.0, alpha);
      colors.push(0.0, 0.9, 1.0, alpha);
    }

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
    geometry.setAttribute('color', new THREE.Float32BufferAttribute(colors, 4));

    const material = new THREE.LineBasicMaterial({
      vertexColors: true,
      transparent: true,
      opacity: 0.6,
      depthWrite: false,
      blending: THREE.AdditiveBlending,
    });

    this.edgesLine = new THREE.LineSegments(geometry, material);
    this.scene.add(this.edgesLine);
  }

  getNodeIndex(id: string): number | undefined {
    return this.nodeMap.get(id);
  }

  /** Animate node glow pulse (0-1 range). */
  setPulse(intensity: number): void {
    if (this.glowPoints) {
      (this.glowPoints.material as THREE.PointsMaterial).opacity = 0.15 + intensity * 0.15;
    }
    if (this.nodesMesh) {
      const mat = this.nodesMesh.material as THREE.MeshStandardMaterial;
      mat.emissiveIntensity = 0.5 + intensity * 0.5;
    }
  }

  private disposeNodes(): void {
    if (this.nodesMesh) {
      this.scene.remove(this.nodesMesh);
      this.nodesMesh.geometry.dispose();
      (this.nodesMesh.material as THREE.Material).dispose();
      this.nodesMesh = null;
    }
    if (this.glowPoints) {
      this.scene.remove(this.glowPoints);
      this.glowPoints.geometry.dispose();
      (this.glowPoints.material as THREE.Material).dispose();
      this.glowPoints = null;
    }
    this.nodeMap.clear();
  }

  private disposeEdges(): void {
    if (this.edgesLine) {
      this.scene.remove(this.edgesLine);
      this.edgesLine.geometry.dispose();
      (this.edgesLine.material as THREE.Material).dispose();
      this.edgesLine = null;
    }
  }

  dispose(): void {
    this.disposeNodes();
    this.disposeEdges();
  }
}
