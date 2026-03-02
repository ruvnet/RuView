import { useCallback, useEffect, useRef } from 'react';
import { StyleSheet, View } from 'react-native';
import * as THREE from 'three';
import type { SensingFrame } from '@/types/sensing';

type GaussianSplatWebViewWebProps = {
  onReady: () => void;
  onFps: (fps: number) => void;
  onError: (msg: string) => void;
  frame: SensingFrame | null;
};

const BONES: [number, number][] = [
  [0,1],[0,2],[1,3],[2,4],[5,6],[5,7],[7,9],[6,8],[8,10],
  [5,11],[6,12],[11,12],[11,13],[13,15],[12,14],[14,16],
];

export const GaussianSplatWebViewWeb = ({ onReady, onFps, onError, frame }: GaussianSplatWebViewWebProps) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const sceneRef = useRef<{
    renderer: THREE.WebGLRenderer;
    scene: THREE.Scene;
    camera: THREE.PerspectiveCamera;
    joints: THREE.Mesh[];
    boneLines: { line: THREE.Line; a: number; b: number }[];
    ring: THREE.Mesh;
    particleGeo: THREE.BufferGeometry;
    pointLight: THREE.PointLight;
    animId: number;
    cameraAngle: number;
    cameraRadius: number;
    cameraY: number;
    isDragging: boolean;
    frameCount: number;
    lastFpsTime: number;
  } | null>(null);
  const frameRef = useRef<SensingFrame | null>(null);

  // Keep frame ref current without re-running effect
  frameRef.current = frame;

  const cleanup = useCallback(() => {
    const s = sceneRef.current;
    if (!s) return;
    cancelAnimationFrame(s.animId);
    s.renderer.dispose();
    s.scene.traverse((obj) => {
      if (obj instanceof THREE.Mesh) {
        obj.geometry.dispose();
        if (Array.isArray(obj.material)) obj.material.forEach((m) => m.dispose());
        else obj.material.dispose();
      }
    });
    sceneRef.current = null;
  }, []);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    try {
      const W = () => container.clientWidth || window.innerWidth;
      const H = () => container.clientHeight || window.innerHeight;

      // Renderer
      const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
      renderer.setSize(W(), H());
      renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
      renderer.setClearColor(0x0a0e1a);
      container.appendChild(renderer.domElement);

      // Scene
      const scene = new THREE.Scene();
      scene.background = new THREE.Color(0x0a0e1a);
      scene.fog = new THREE.FogExp2(0x0a0e1a, 0.008);

      // Camera
      const camera = new THREE.PerspectiveCamera(60, W() / H(), 0.1, 500);
      camera.position.set(0, 2, 6);
      camera.lookAt(0, 1, 0);

      // Grid
      const grid = new THREE.GridHelper(20, 40, 0x1a3a4a, 0x0d1f2a);
      scene.add(grid);

      // Lights
      scene.add(new THREE.AmbientLight(0x32b8c6, 0.3));
      const pointLight = new THREE.PointLight(0x32b8c6, 1.5, 20);
      pointLight.position.set(0, 4, 0);
      scene.add(pointLight);

      // Skeleton joints (17 COCO keypoints)
      const jointGeo = new THREE.SphereGeometry(0.06, 8, 8);
      const joints: THREE.Mesh[] = [];
      for (let i = 0; i < 17; i++) {
        const mat = new THREE.MeshStandardMaterial({
          color: 0x32b8c6,
          emissive: 0x32b8c6,
          emissiveIntensity: 0.6,
        });
        const m = new THREE.Mesh(jointGeo, mat);
        m.visible = false;
        scene.add(m);
        joints.push(m);
      }

      // Bone lines
      const boneMat = new THREE.LineBasicMaterial({
        color: 0x32b8c6,
        transparent: true,
        opacity: 0.7,
      });
      const boneLines = BONES.map(([a, b]) => {
        const g = new THREE.BufferGeometry().setFromPoints([
          new THREE.Vector3(),
          new THREE.Vector3(),
        ]);
        const l = new THREE.Line(g, boneMat);
        l.visible = false;
        scene.add(l);
        return { line: l, a, b };
      });

      // Particle field
      const N = 500;
      const particleGeo = new THREE.BufferGeometry();
      const pPos = new Float32Array(N * 3);
      for (let i = 0; i < N; i++) {
        pPos[i * 3] = (Math.random() - 0.5) * 16;
        pPos[i * 3 + 1] = Math.random() * 4;
        pPos[i * 3 + 2] = (Math.random() - 0.5) * 16;
      }
      particleGeo.setAttribute('position', new THREE.BufferAttribute(pPos, 3));
      const pMat = new THREE.PointsMaterial({
        color: 0x32b8c6,
        size: 0.04,
        transparent: true,
        opacity: 0.4,
      });
      scene.add(new THREE.Points(particleGeo, pMat));

      // Signal ring
      const ringGeo = new THREE.TorusGeometry(2, 0.02, 8, 64);
      const ringMat = new THREE.MeshBasicMaterial({
        color: 0x32b8c6,
        transparent: true,
        opacity: 0.3,
      });
      const ring = new THREE.Mesh(ringGeo, ringMat);
      ring.rotation.x = Math.PI / 2;
      ring.position.y = 0.01;
      scene.add(ring);

      // State
      const state = {
        renderer,
        scene,
        camera,
        joints,
        boneLines,
        ring,
        particleGeo,
        pointLight,
        animId: 0,
        cameraAngle: 0,
        cameraRadius: 6,
        cameraY: 2,
        isDragging: false,
        frameCount: 0,
        lastFpsTime: performance.now(),
      };
      sceneRef.current = state;

      // Mouse interaction
      const canvas = renderer.domElement;
      const onMouseDown = () => { state.isDragging = true; };
      const onMouseUp = () => { state.isDragging = false; };
      const onMouseMove = (e: MouseEvent) => {
        if (state.isDragging) {
          state.cameraAngle += e.movementX * 0.01;
          state.cameraY = Math.max(0.5, Math.min(5, state.cameraY - e.movementY * 0.01));
        }
      };
      const onWheel = (e: WheelEvent) => {
        state.cameraRadius = Math.max(2, Math.min(15, state.cameraRadius + e.deltaY * 0.005));
      };
      canvas.addEventListener('mousedown', onMouseDown);
      canvas.addEventListener('mouseup', onMouseUp);
      canvas.addEventListener('mousemove', onMouseMove);
      canvas.addEventListener('wheel', onWheel, { passive: true });

      // Resize
      const onResize = () => {
        camera.aspect = W() / H();
        camera.updateProjectionMatrix();
        renderer.setSize(W(), H());
      };
      window.addEventListener('resize', onResize);

      // Animation loop
      const animate = () => {
        state.animId = requestAnimationFrame(animate);
        const t = performance.now() * 0.001;

        // Camera orbit
        if (!state.isDragging) state.cameraAngle += 0.002;
        camera.position.set(
          Math.sin(state.cameraAngle) * state.cameraRadius,
          state.cameraY,
          Math.cos(state.cameraAngle) * state.cameraRadius,
        );
        camera.lookAt(0, 1, 0);

        // Animate ring
        ring.material.opacity = 0.15 + Math.sin(t * 2) * 0.1;
        const scale = 1 + Math.sin(t) * 0.1;
        ring.scale.set(scale, scale, 1);

        // Animate particles
        const pp = particleGeo.attributes.position as THREE.BufferAttribute;
        for (let i = 0; i < N; i++) {
          (pp.array as Float32Array)[i * 3 + 1] += Math.sin(t + i) * 0.001;
        }
        pp.needsUpdate = true;

        // Update skeleton from frame data
        const currentFrame = frameRef.current;
        if (currentFrame) {
          const persons = (currentFrame as any).persons || [];
          if (persons.length > 0) {
            const kps = persons[0].keypoints || [];
            kps.forEach((kp: any, i: number) => {
              if (i < 17 && joints[i]) {
                joints[i].position.set(
                  (kp.x - 0.5) * 4,
                  (1 - kp.y) * 3,
                  (kp.z || 0) * 2,
                );
                joints[i].visible = kp.confidence > 0.3;
                (joints[i].material as THREE.MeshStandardMaterial).emissiveIntensity =
                  0.3 + kp.confidence * 0.7;
              }
            });
            boneLines.forEach(({ line, a, b }) => {
              if (joints[a].visible && joints[b].visible) {
                const pos = line.geometry.attributes.position as THREE.BufferAttribute;
                pos.setXYZ(0, joints[a].position.x, joints[a].position.y, joints[a].position.z);
                pos.setXYZ(1, joints[b].position.x, joints[b].position.y, joints[b].position.z);
                pos.needsUpdate = true;
                line.visible = true;
              } else {
                line.visible = false;
              }
            });
          } else {
            joints.forEach((j) => { j.visible = false; });
            boneLines.forEach((bl) => { bl.line.visible = false; });
          }

          // Adjust light from RSSI
          const features = (currentFrame as any).features;
          if (features) {
            const rssi = features.mean_rssi || -70;
            pointLight.intensity = 1 + Math.abs(rssi + 50) * 0.02;
          }
        }

        renderer.render(scene, camera);

        // FPS counter
        state.frameCount++;
        if (performance.now() - state.lastFpsTime >= 1000) {
          onFps(state.frameCount);
          state.frameCount = 0;
          state.lastFpsTime = performance.now();
        }
      };

      animate();
      onReady();

      return () => {
        canvas.removeEventListener('mousedown', onMouseDown);
        canvas.removeEventListener('mouseup', onMouseUp);
        canvas.removeEventListener('mousemove', onMouseMove);
        canvas.removeEventListener('wheel', onWheel);
        window.removeEventListener('resize', onResize);
        cleanup();
        if (container.contains(renderer.domElement)) {
          container.removeChild(renderer.domElement);
        }
      };
    } catch (err) {
      onError(err instanceof Error ? err.message : 'Failed to initialize 3D renderer');
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <View style={styles.container}>
      <div
        ref={containerRef}
        style={{ width: '100%', height: '100%', backgroundColor: '#0a0e1a' }}
      />
    </View>
  );
};

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#0a0e1a',
  },
});

export default GaussianSplatWebViewWeb;
