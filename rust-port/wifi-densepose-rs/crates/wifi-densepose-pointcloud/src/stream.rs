//! HTTP server — live camera + ESP32 CSI + fusion → real-time point cloud.

use crate::brain_bridge;
use crate::camera;
use crate::csi_pipeline;
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
    latest_cloud: Mutex<pointcloud::PointCloud>,
    latest_splats: Mutex<Vec<pointcloud::GaussianSplat>>,
    latest_pipeline: Mutex<Option<csi_pipeline::PipelineOutput>>,
    frame_count: Mutex<u64>,
    use_camera: bool,
    csi_pipeline: Option<Arc<Mutex<csi_pipeline::CsiPipelineState>>>,
}

pub async fn serve(host: &str, port: u16, _wifi_source: Option<&str>) -> anyhow::Result<()> {
    let has_camera = camera::camera_available();

    // Start CSI pipeline — listens for UDP CSI data from ESP32 nodes
    let csi_pipeline_state = csi_pipeline::start_pipeline("0.0.0.0:3333");
    eprintln!("  CSI pipeline: UDP port 3333 (ADR-018 binary frames)");

    let initial_cloud = if has_camera {
        capture_camera_cloud()
    } else {
        demo_cloud()
    };
    let initial_splats = pointcloud::to_gaussian_splats(&initial_cloud);

    let state = Arc::new(AppState {
        latest_cloud: Mutex::new(initial_cloud),
        latest_splats: Mutex::new(initial_splats),
        latest_pipeline: Mutex::new(None),
        frame_count: Mutex::new(0),
        use_camera: has_camera,
        csi_pipeline: Some(csi_pipeline_state.clone()),
    });

    // Background: capture + fuse every 500ms (motion-adaptive)
    let bg = state.clone();
    let bg_csi = Some(csi_pipeline_state.clone());
    let bg_cam = has_camera;
    tokio::spawn(async move {
        let mut skip_depth = false;
        loop {
            // Motion-adaptive: check CSI motion score
            let pipeline_out = bg_csi.as_ref().map(|c| csi_pipeline::get_pipeline_output(c));
            if let Some(ref out) = pipeline_out {
                // Only run expensive depth when motion detected or every 5th frame
                let frame_num = *bg.frame_count.lock().unwrap();
                skip_depth = !out.motion_detected && frame_num % 5 != 0;
            }
            let pipeline_clone = pipeline_out.clone();
            *bg.latest_pipeline.lock().unwrap() = pipeline_out;
            let pipeline_out = pipeline_clone;

            let interval = if skip_depth { 1000 } else { 500 }; // slower when no motion
            tokio::time::sleep(std::time::Duration::from_millis(interval)).await;

            let cloud = if bg_cam && !skip_depth {
                tokio::task::spawn_blocking(capture_camera_cloud)
                    .await.unwrap_or_else(|_| demo_cloud())
            } else {
                // Reuse previous cloud when no motion
                bg.latest_cloud.lock().unwrap().clone()
            };
            let splats = pointcloud::to_gaussian_splats(&cloud);
            *bg.latest_cloud.lock().unwrap() = cloud;
            *bg.latest_splats.lock().unwrap() = splats;
            let frame_num = {
                let mut fc = bg.frame_count.lock().unwrap();
                *fc += 1;
                *fc
            };

            // Brain sync — sparse, every 120 frames (~60 seconds)
            if frame_num % 120 == 0 {
                if let Some(ref out) = pipeline_out {
                    brain_bridge::sync_to_brain(out, frame_num).await;
                }
            }
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

fn capture_camera_cloud() -> pointcloud::PointCloud {
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
    let pipeline = state.latest_pipeline.lock().unwrap();
    Json(serde_json::json!({
        "points": cloud.points.len(),
        "bounds_min": min, "bounds_max": max,
        "live": state.use_camera,
        "frame": frames,
        "pipeline": &*pipeline,
        "cloud": cloud.points.iter().take(1000).collect::<Vec<_>>(),
    }))
}

async fn api_splats(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let splats = state.latest_splats.lock().unwrap();
    let frames = *state.frame_count.lock().unwrap();
    let pipeline = state.latest_pipeline.lock().unwrap();
    Json(serde_json::json!({
        "splats": &*splats,
        "count": splats.len(),
        "live": state.use_camera,
        "frame": frames,
        "pipeline": &*pipeline,
        "timestamp": chrono::Utc::now().timestamp_millis(),
    }))
}

async fn api_status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let frames = *state.frame_count.lock().unwrap();
    let pipeline = state.latest_pipeline.lock().unwrap();
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "live": state.use_camera,
        "camera": if state.use_camera { "/dev/video0" } else { "demo" },
        "csi_pipeline": "active (UDP:3333)",
        "pipeline": &*pipeline,
        "frames_captured": frames,
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
