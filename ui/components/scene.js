// Three.js Scene Setup - WiFi DensePose 3D Visualization
// Camera, lights, renderer, OrbitControls

export class Scene {
  constructor(container) {
    this.container = typeof container === 'string'
      ? document.getElementById(container)
      : container;

    if (!this.container) {
      throw new Error('Scene container element not found');
    }

    this.scene = null;
    this.camera = null;
    this.renderer = null;
    this.controls = null;
    this.clock = null;
    this.animationId = null;
    this.updateCallbacks = [];
    this.isRunning = false;

    this._init();
  }

  _init() {
    const width = this.container.clientWidth || 960;
    const height = this.container.clientHeight || 640;

    // Scene
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x0a0a1a);
    this.scene.fog = new THREE.FogExp2(0x0a0a1a, 0.008);

    // Camera - positioned to see the room from a 3/4 angle
    this.camera = new THREE.PerspectiveCamera(55, width / height, 0.1, 500);
    this.camera.position.set(8, 7, 10);
    this.camera.lookAt(0, 1.5, 0);

    // Renderer
    this.renderer = new THREE.WebGLRenderer({
      antialias: true,
      alpha: false,
      powerPreference: 'high-performance'
    });
    this.renderer.setSize(width, height);
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.shadowMap.enabled = true;
    this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.0;
    this.container.appendChild(this.renderer.domElement);

    // OrbitControls
    this.controls = new THREE.OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.08;
    this.controls.minDistance = 3;
    this.controls.maxDistance = 30;
    this.controls.maxPolarAngle = Math.PI * 0.85;
    this.controls.target.set(0, 1.2, 0);
    this.controls.update();

    // Lights
    this._setupLights();

    // Clock for animation delta
    this.clock = new THREE.Clock();

    // Handle resize
    this._resizeObserver = new ResizeObserver(() => this._onResize());
    this._resizeObserver.observe(this.container);
    window.addEventListener('resize', () => this._onResize());
  }

  _setupLights() {
    // Ambient light - subtle blue tint for tech feel
    const ambient = new THREE.AmbientLight(0x223355, 0.4);
    this.scene.add(ambient);

    // Hemisphere light - sky/ground gradient
    const hemi = new THREE.HemisphereLight(0x4488cc, 0x112233, 0.5);
    hemi.position.set(0, 20, 0);
    this.scene.add(hemi);

    // Key light - warm directional light from above-right
    const keyLight = new THREE.DirectionalLight(0xffeedd, 0.8);
    keyLight.position.set(5, 10, 5);
    keyLight.castShadow = true;
    keyLight.shadow.mapSize.width = 1024;
    keyLight.shadow.mapSize.height = 1024;
    keyLight.shadow.camera.near = 0.5;
    keyLight.shadow.camera.far = 30;
    keyLight.shadow.camera.left = -10;
    keyLight.shadow.camera.right = 10;
    keyLight.shadow.camera.top = 10;
    keyLight.shadow.camera.bottom = -10;
    this.scene.add(keyLight);

    // Fill light - cool from left
    const fillLight = new THREE.DirectionalLight(0x88aaff, 0.3);
    fillLight.position.set(-5, 6, -3);
    this.scene.add(fillLight);

    // Point light under the body for a soft uplight glow
    const uplight = new THREE.PointLight(0x0066ff, 0.4, 8);
    uplight.position.set(0, 0.1, 0);
    this.scene.add(uplight);
  }

  // Register a callback that runs each frame with (deltaTime, elapsedTime)
  onUpdate(callback) {
    this.updateCallbacks.push(callback);
    return () => {
      const idx = this.updateCallbacks.indexOf(callback);
      if (idx !== -1) this.updateCallbacks.splice(idx, 1);
    };
  }

  start() {
    if (this.isRunning) return;
    this.isRunning = true;
    this.clock.start();
    this._animate();
  }

  stop() {
    this.isRunning = false;
    if (this.animationId !== null) {
      cancelAnimationFrame(this.animationId);
      this.animationId = null;
    }
  }

  _animate() {
    if (!this.isRunning) return;
    this.animationId = requestAnimationFrame(() => this._animate());

    const delta = this.clock.getDelta();
    const elapsed = this.clock.getElapsedTime();

    // Run registered update callbacks
    for (const cb of this.updateCallbacks) {
      cb(delta, elapsed);
    }

    this.controls.update();
    this.renderer.render(this.scene, this.camera);
  }

  _onResize() {
    const width = this.container.clientWidth;
    const height = this.container.clientHeight;
    if (width === 0 || height === 0) return;

    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(width, height);
  }

  // Add an object to the scene
  add(object) {
    this.scene.add(object);
  }

  // Remove an object from the scene
  remove(object) {
    this.scene.remove(object);
  }

  // Get the Three.js scene, camera, renderer for external access
  getScene() { return this.scene; }
  getCamera() { return this.camera; }
  getRenderer() { return this.renderer; }

  // Reset camera to default position
  resetCamera() {
    this.camera.position.set(8, 7, 10);
    this.controls.target.set(0, 1.2, 0);
    this.controls.update();
  }

  dispose() {
    this.stop();
    if (this._resizeObserver) {
      this._resizeObserver.disconnect();
    }
    window.removeEventListener('resize', this._onResize);
    this.controls.dispose();
    this.renderer.dispose();
    if (this.renderer.domElement.parentNode) {
      this.renderer.domElement.parentNode.removeChild(this.renderer.domElement);
    }
    this.updateCallbacks = [];
  }
}
