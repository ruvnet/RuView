//! HTTP server for real-time point cloud streaming with live camera + CSI.

use crate::camera;
use crate::depth;
use crate::fusion;
use crate::pointcloud;
use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use std::sync::{Arc, Mutex};

struct AppState {
    /// Cached latest point cloud (refreshed by background task)
    latest_cloud: Mutex<pointcloud::PointCloud>,
    latest_splats: Mutex<Vec<pointcloud::GaussianSplat>>,
    frame_count: Mutex<u64>,
    use_camera: bool,
}

pub async fn serve(host: &str, port: u16, _wifi_source: Option<&str>) -> anyhow::Result<()> {
    let has_camera = camera::camera_available();
    let initial_cloud = if has_camera {
        capture_live_cloud()
    } else {
        let occ = fusion::demo_occupancy();
        let wc = fusion::occupancy_to_pointcloud(&occ);
        let dc = depth::demo_depth_cloud();
        fusion::fuse_clouds(&[&wc, &dc], 0.05)
    };
    let initial_splats = pointcloud::to_gaussian_splats(&initial_cloud);

    let state = Arc::new(AppState {
        latest_cloud: Mutex::new(initial_cloud),
        latest_splats: Mutex::new(initial_splats),
        frame_count: Mutex::new(0),
        use_camera: has_camera,
    });

    // Background: capture frames every 500ms
    if has_camera {
        let bg = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                let cloud = tokio::task::spawn_blocking(capture_live_cloud).await.unwrap_or_else(|_| {
                    let occ = fusion::demo_occupancy();
                    let dc = depth::demo_depth_cloud();
                    fusion::fuse_clouds(&[&fusion::occupancy_to_pointcloud(&occ), &dc], 0.05)
                });
                let splats = pointcloud::to_gaussian_splats(&cloud);
                *bg.latest_cloud.lock().unwrap() = cloud;
                *bg.latest_splats.lock().unwrap() = splats;
                *bg.frame_count.lock().unwrap() += 1;
            }
        });
        eprintln!("  Camera: LIVE (/dev/video0, 2 fps capture)");
    } else {
        eprintln!("  Camera: DEMO (no /dev/video0)");
    }

    let app = Router::new()
        .route("/", get(index))
        .route("/api/cloud", get(api_cloud))
        .route("/api/splats", get(api_splats))
        .route("/api/status", get(api_status))
        .route("/health", get(api_health))
        .with_state(state);

    let addr = format!("{host}:{port}");
    println!("╔══════════════════════════════════════════════╗");
    println!("║  RuView Dense Point Cloud Server             ║");
    println!("╚══════════════════════════════════════════════╝");
    println!("  HTTP:   http://{addr}");
    println!("  Viewer: http://{addr}/");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// Capture a live frame from the camera and generate a depth point cloud.
fn capture_live_cloud() -> pointcloud::PointCloud {
    let config = camera::CameraConfig::default();
    match camera::capture_frame(&config) {
        Ok(frame) => {
            match depth::estimate_depth(&frame.rgb, frame.width, frame.height) {
                Ok(depth_map) => {
                    let intrinsics = depth::CameraIntrinsics::default();
                    depth::backproject_depth(&depth_map, &intrinsics, Some(&frame.rgb), 4) // downsample 4x
                }
                Err(_) => depth::demo_depth_cloud(),
            }
        }
        Err(_) => depth::demo_depth_cloud(),
    }
}

async fn api_cloud(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let cloud = state.latest_cloud.lock().unwrap();
    let (min, max) = cloud.bounds();
    let frames = *state.frame_count.lock().unwrap();
    Json(serde_json::json!({
        "points": cloud.points.len(),
        "bounds_min": min,
        "bounds_max": max,
        "live": state.use_camera,
        "frame": frames,
        "cloud": cloud.points.iter().take(1000).collect::<Vec<_>>(),
    }))
}

async fn api_splats(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let splats = state.latest_splats.lock().unwrap();
    let frames = *state.frame_count.lock().unwrap();
    Json(serde_json::json!({
        "splats": &*splats,
        "count": splats.len(),
        "live": state.use_camera,
        "frame": frames,
        "timestamp": chrono::Utc::now().timestamp_millis(),
    }))
}

async fn api_status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let frames = *state.frame_count.lock().unwrap();
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "live": state.use_camera,
        "frames_captured": frames,
        "camera": if state.use_camera { "/dev/video0" } else { "demo" },
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
    <title>RuView Dense Point Cloud</title>
    <style>
        body { margin: 0; background: #111; color: #e8a634; font-family: monospace; }
        canvas { display: block; }
        #info { position: absolute; top: 10px; left: 10px; padding: 10px; background: rgba(0,0,0,0.8); border: 1px solid #e8a634; border-radius: 4px; }
    </style>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/three@0.128.0/examples/js/controls/OrbitControls.js"></script>
</head>
<body>
    <div id="info">
        <h3 style="margin:0 0 5px 0">RuView Point Cloud</h3>
        <div id="stats">Loading...</div>
    </div>
    <script>
        const scene = new THREE.Scene();
        scene.background = new THREE.Color(0x111111);
        const camera = new THREE.PerspectiveCamera(75, window.innerWidth/window.innerHeight, 0.1, 100);
        camera.position.set(0, 0, -3);
        camera.lookAt(0, 0, 3);

        const renderer = new THREE.WebGLRenderer({ antialias: true });
        renderer.setSize(window.innerWidth, window.innerHeight);
        document.body.appendChild(renderer.domElement);

        const controls = new THREE.OrbitControls(camera, renderer.domElement);
        controls.enableDamping = true;
        controls.target.set(0, 0, 3);

        scene.add(new THREE.GridHelper(10, 20, 0x333333, 0x222222));

        let pointsMesh = null;
        let lastFrame = -1;

        async function fetchCloud() {
            try {
                const resp = await fetch('/api/splats');
                const data = await resp.json();
                if (data.splats && data.frame !== lastFrame) {
                    lastFrame = data.frame;
                    updateSplats(data.splats);
                    const mode = data.live ? '🟢 LIVE' : '🔴 DEMO';
                    document.getElementById('stats').innerHTML =
                        `${mode}<br>Splats: ${data.count}<br>Frame: ${data.frame}`;
                }
            } catch(e) {
                document.getElementById('stats').innerHTML = 'Error: ' + e.message;
            }
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

            const material = new THREE.PointsMaterial({
                size: 0.03,
                vertexColors: true,
                sizeAttenuation: true,
            });

            pointsMesh = new THREE.Points(geometry, material);
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
