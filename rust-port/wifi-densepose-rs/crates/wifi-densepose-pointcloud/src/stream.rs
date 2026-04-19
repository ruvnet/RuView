//! HTTP server — live camera + ESP32 CSI + fusion → real-time point cloud.

use crate::camera;
use crate::depth;
use crate::fusion;
use crate::pointcloud;
use crate::serial_csi;
use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use std::sync::{Arc, Mutex};

struct AppState {
    latest_cloud: Mutex<pointcloud::PointCloud>,
    latest_splats: Mutex<Vec<pointcloud::GaussianSplat>>,
    frame_count: Mutex<u64>,
    use_camera: bool,
    csi_state: Option<Arc<Mutex<serial_csi::CsiState>>>,
}

pub async fn serve(host: &str, port: u16, _wifi_source: Option<&str>) -> anyhow::Result<()> {
    let has_camera = camera::camera_available();

    // CSI serial readers — only start if explicitly requested via env var
    // (serial reader needs proper baud rate config to avoid reconnect loop)
    let csi_state = if std::env::var("RUVIEW_CSI").is_ok() {
        let mut csi_ports = Vec::new();
        for p in &["/dev/ttyACM0", "/dev/ttyUSB0"] {
            if std::path::Path::new(p).exists() { csi_ports.push(*p); }
        }
        if !csi_ports.is_empty() {
            eprintln!("  CSI ports: {:?}", csi_ports);
            Some(serial_csi::start_serial_readers(&csi_ports))
        } else { None }
    } else {
        eprintln!("  CSI: disabled (set RUVIEW_CSI=1 to enable)");
        None
    };

    let initial_cloud = if has_camera {
        capture_live_cloud(csi_state.as_ref())
    } else {
        demo_cloud()
    };
    let initial_splats = pointcloud::to_gaussian_splats(&initial_cloud);

    let state = Arc::new(AppState {
        latest_cloud: Mutex::new(initial_cloud),
        latest_splats: Mutex::new(initial_splats),
        frame_count: Mutex::new(0),
        use_camera: has_camera,
        csi_state: csi_state.clone(),
    });

    // Background: capture + fuse every 500ms
    let bg = state.clone();
    let bg_csi = csi_state.clone();
    let bg_cam = has_camera;
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let csi_clone = bg_csi.clone();
            let cloud = if bg_cam {
                tokio::task::spawn_blocking(move || capture_live_cloud(csi_clone.as_ref()))
                    .await.unwrap_or_else(|_| demo_cloud())
            } else {
                demo_cloud()
            };
            let splats = pointcloud::to_gaussian_splats(&cloud);
            *bg.latest_cloud.lock().unwrap() = cloud;
            *bg.latest_splats.lock().unwrap() = splats;
            *bg.frame_count.lock().unwrap() += 1;
        }
    });

    if has_camera { eprintln!("  Camera: LIVE (/dev/video0)"); }
    else { eprintln!("  Camera: DEMO"); }

    let app = Router::new()
        .route("/", get(index))
        .route("/api/cloud", get(api_cloud))
        .route("/api/splats", get(api_splats))
        .route("/api/status", get(api_status))
        .route("/health", get(api_health))
        .with_state(state);

    let addr = format!("{host}:{port}");
    println!("╔══════════════════════════════════════════════╗");
    println!("║  RuView Dense Point Cloud — ALL SENSORS      ║");
    println!("╚══════════════════════════════════════════════╝");
    println!("  Viewer: http://{addr}/");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn capture_live_cloud(csi: Option<&Arc<Mutex<serial_csi::CsiState>>>) -> pointcloud::PointCloud {
    // 1. Camera → depth → dense points
    let cam_cloud = {
        let config = camera::CameraConfig::default();
        match camera::capture_frame(&config) {
            Ok(frame) => {
                match depth::estimate_depth(&frame.rgb, frame.width, frame.height) {
                    Ok(dm) => {
                        let intr = depth::CameraIntrinsics::default();
                        depth::backproject_depth(&dm, &intr, Some(&frame.rgb), 2)
                    }
                    Err(_) => depth::demo_depth_cloud(),
                }
            }
            Err(_) => depth::demo_depth_cloud(),
        }
    };

    // 2. CSI → motion + presence → modify point cloud
    let mut clouds: Vec<&pointcloud::PointCloud> = vec![&cam_cloud];

    let csi_cloud;
    if let Some(csi_state) = csi {
        let (motion, distance, frames) = serial_csi::get_csi_influence(csi_state);
        if frames > 0 {
            // Create CSI-informed occupancy around detected presence
            let mut occ = fusion::OccupancyVolume {
                densities: vec![0.0; 8 * 8 * 4],
                nx: 8, ny: 8, nz: 4,
                bounds: [
                    -distance as f64, -distance as f64, 0.0,
                    distance as f64, distance as f64, 2.5,
                ],
                occupied_count: 0,
            };

            // Place high density where CSI indicates presence
            let cx: usize = 4; let cy: usize = 4;
            let radius: usize = (motion * 3.0).max(1.0) as usize;
            for iz in 0..4 {
                for iy in (cy.saturating_sub(radius))..=(cy + radius).min(7) {
                    for ix in (cx.saturating_sub(radius))..=(cx + radius).min(7) {
                        let idx = iz * 64 + iy * 8 + ix;
                        let dx = (ix as f32 - cx as f32).abs() / radius as f32;
                        let dy = (iy as f32 - cy as f32).abs() / radius as f32;
                        let r2 = dx * dx + dy * dy;
                        if r2 < 1.0 {
                            occ.densities[idx] = (1.0 - r2 as f64) * (0.5 + motion as f64 * 0.5);
                        }
                    }
                }
            }
            occ.occupied_count = occ.densities.iter().filter(|&&d| d > 0.3).count();

            csi_cloud = fusion::occupancy_to_pointcloud(&occ);
            clouds.push(&csi_cloud);
        }
    }

    // 3. Fuse camera + CSI
    if clouds.len() > 1 {
        fusion::fuse_clouds(&clouds, 0.04)
    } else {
        cam_cloud
    }
}

fn demo_cloud() -> pointcloud::PointCloud {
    let occ = fusion::demo_occupancy();
    let wc = fusion::occupancy_to_pointcloud(&occ);
    let dc = depth::demo_depth_cloud();
    fusion::fuse_clouds(&[&wc, &dc], 0.05)
}

async fn api_cloud(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let cloud = state.latest_cloud.lock().unwrap();
    let (min, max) = cloud.bounds();
    let frames = *state.frame_count.lock().unwrap();
    let csi_info = state.csi_state.as_ref().map(|c| serial_csi::get_csi_influence(c));
    Json(serde_json::json!({
        "points": cloud.points.len(),
        "bounds_min": min, "bounds_max": max,
        "live": state.use_camera,
        "frame": frames,
        "csi": csi_info.map(|(m,d,f)| serde_json::json!({"motion":m,"distance_m":d,"frames":f})),
        "cloud": cloud.points.iter().take(1000).collect::<Vec<_>>(),
    }))
}

async fn api_splats(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let splats = state.latest_splats.lock().unwrap();
    let frames = *state.frame_count.lock().unwrap();
    let csi_info = state.csi_state.as_ref().map(|c| serial_csi::get_csi_influence(c));
    Json(serde_json::json!({
        "splats": &*splats,
        "count": splats.len(),
        "live": state.use_camera,
        "frame": frames,
        "csi": csi_info.map(|(m,d,f)| serde_json::json!({"motion":m,"distance_m":d,"frames":f})),
        "timestamp": chrono::Utc::now().timestamp_millis(),
    }))
}

async fn api_status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let frames = *state.frame_count.lock().unwrap();
    let csi_info = state.csi_state.as_ref().map(|c| serial_csi::get_csi_influence(c));
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "live": state.use_camera,
        "camera": if state.use_camera { "/dev/video0" } else { "demo" },
        "csi_ports": if state.csi_state.is_some() { "active" } else { "none" },
        "csi": csi_info.map(|(m,d,f)| serde_json::json!({"motion":m,"distance_m":d,"frames":f})),
        "frames_captured": frames,
        "fps": 2,
    }))
}

async fn api_health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

async fn index() -> Html<String> {
    Html(r#"<!DOCTYPE html>
<html>
<head>
    <title>RuView — Camera + WiFi CSI Point Cloud</title>
    <style>
        body { margin: 0; background: #0a0a0a; color: #e8a634; font-family: monospace; }
        canvas { display: block; }
        #info { position: absolute; top: 10px; left: 10px; padding: 12px; background: rgba(0,0,0,0.85); border: 1px solid #e8a634; border-radius: 6px; min-width: 200px; }
        .live { color: #4f4; } .demo { color: #f44; }
    </style>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/three@0.128.0/examples/js/controls/OrbitControls.js"></script>
</head>
<body>
    <div id="info">
        <h3 style="margin:0 0 8px 0">RuView Point Cloud</h3>
        <div id="stats">Loading...</div>
    </div>
    <script>
        const scene = new THREE.Scene();
        scene.background = new THREE.Color(0x0a0a0a);
        const camera = new THREE.PerspectiveCamera(75, window.innerWidth/window.innerHeight, 0.1, 100);
        camera.position.set(0, 0, -2);
        camera.lookAt(0, 0, 3);

        const renderer = new THREE.WebGLRenderer({ antialias: true });
        renderer.setSize(window.innerWidth, window.innerHeight);
        document.body.appendChild(renderer.domElement);

        const controls = new THREE.OrbitControls(camera, renderer.domElement);
        controls.enableDamping = true;
        controls.target.set(0, 0, 3);

        let pointsMesh = null;
        let lastFrame = -1;

        async function fetchCloud() {
            try {
                const resp = await fetch('/api/splats');
                const data = await resp.json();
                if (data.splats && data.frame !== lastFrame) {
                    lastFrame = data.frame;
                    updateSplats(data.splats);
                    const mode = data.live ? '<span class="live">● LIVE</span>' : '<span class="demo">● DEMO</span>';
                    let csiInfo = '';
                    if (data.csi) {
                        const m = (data.csi.motion * 100).toFixed(0);
                        csiInfo = `<br>CSI: ${data.csi.frames} frames, motion ${m}%<br>Distance: ${data.csi.distance_m.toFixed(1)}m`;
                    }
                    document.getElementById('stats').innerHTML =
                        `${mode} Camera + CSI<br>Splats: ${data.count}<br>Frame: ${data.frame}${csiInfo}`;
                }
            } catch(e) {}
        }
        fetchCloud();
        setInterval(fetchCloud, 500);

        function updateSplats(splats) {
            if (pointsMesh) scene.remove(pointsMesh);
            const geometry = new THREE.BufferGeometry();
            const positions = new Float32Array(splats.length * 3);
            const colors = new Float32Array(splats.length * 3);
            splats.forEach((s, i) => {
                positions[i*3] = s.center[0];
                positions[i*3+1] = -s.center[1];
                positions[i*3+2] = s.center[2];
                colors[i*3] = s.color[0];
                colors[i*3+1] = s.color[1];
                colors[i*3+2] = s.color[2];
            });
            geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
            geometry.setAttribute('color', new THREE.BufferAttribute(colors, 3));
            pointsMesh = new THREE.Points(geometry, new THREE.PointsMaterial({
                size: 0.025, vertexColors: true, sizeAttenuation: true,
            }));
            scene.add(pointsMesh);
        }

        function animate() {
            requestAnimationFrame(animate);
            controls.update();
            renderer.render(scene, camera);
        }
        animate();
        window.addEventListener('resize', () => {
            camera.aspect = window.innerWidth / window.innerHeight;
            camera.updateProjectionMatrix();
            renderer.setSize(window.innerWidth, window.innerHeight);
        });
    </script>
</body>
</html>"#.to_string())
}
