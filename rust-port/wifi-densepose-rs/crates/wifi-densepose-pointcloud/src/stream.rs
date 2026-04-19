//! WebSocket + HTTP server for real-time point cloud streaming.

use crate::depth;
use crate::fusion;
use crate::pointcloud;
use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use std::sync::Arc;

struct AppState {
    wifi_source: Option<String>,
}

pub async fn serve(host: &str, port: u16, wifi_source: Option<&str>) -> anyhow::Result<()> {
    let state = Arc::new(AppState {
        wifi_source: wifi_source.map(|s| s.to_string()),
    });

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
    println!("  HTTP:      http://{addr}");
    println!("  WebSocket: ws://{addr}/ws");
    println!("  API:       http://{addr}/api/cloud");
    println!("  Viewer:    http://{addr}/");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn api_cloud() -> Json<serde_json::Value> {
    let occupancy = fusion::demo_occupancy();
    let wifi_cloud = fusion::occupancy_to_pointcloud(&occupancy);
    let depth_cloud = depth::demo_depth_cloud();
    let fused = fusion::fuse_clouds(&[&wifi_cloud, &depth_cloud], 0.05);
    let (min, max) = fused.bounds();

    Json(serde_json::json!({
        "points": fused.points.len(),
        "bounds_min": min,
        "bounds_max": max,
        "sources": ["camera_depth", "wifi_occupancy"],
        "cloud": fused.points.iter().take(1000).collect::<Vec<_>>(),
    }))
}

async fn api_splats() -> Json<serde_json::Value> {
    let occupancy = fusion::demo_occupancy();
    let wifi_cloud = fusion::occupancy_to_pointcloud(&occupancy);
    let depth_cloud = depth::demo_depth_cloud();
    let fused = fusion::fuse_clouds(&[&wifi_cloud, &depth_cloud], 0.05);
    let splats = pointcloud::to_gaussian_splats(&fused);

    Json(serde_json::json!({
        "splats": splats,
        "count": splats.len(),
        "timestamp": chrono::Utc::now().timestamp_millis(),
    }))
}

async fn api_status() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "pipeline": "camera_depth + wifi_occupancy → fused → gaussian_splats",
        "fps": 10,
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
        #info { position: absolute; top: 10px; left: 10px; padding: 10px; background: rgba(0,0,0,0.7); border: 1px solid #e8a634; }
    </style>
    <script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/three@0.128.0/examples/js/controls/OrbitControls.js"></script>
</head>
<body>
    <div id="info">
        <h3>RuView Dense Point Cloud</h3>
        <div id="stats">Connecting...</div>
    </div>
    <script>
        const scene = new THREE.Scene();
        scene.background = new THREE.Color(0x111111);
        const camera = new THREE.PerspectiveCamera(75, window.innerWidth/window.innerHeight, 0.1, 100);
        camera.position.set(3, 3, 3);

        const renderer = new THREE.WebGLRenderer({ antialias: true });
        renderer.setSize(window.innerWidth, window.innerHeight);
        document.body.appendChild(renderer.domElement);

        const controls = new THREE.OrbitControls(camera, renderer.domElement);
        controls.enableDamping = true;

        // Grid
        scene.add(new THREE.GridHelper(10, 20, 0x333333, 0x222222));
        scene.add(new THREE.AxesHelper(2));

        let pointsMesh = null;

        // Poll API for updates (no WebSocket needed)
        async function fetchCloud() {
            try {
                const resp = await fetch('/api/splats');
                const data = await resp.json();
                if (data.splats) {
                    updateSplats(data.splats);
                    document.getElementById('stats').innerHTML =
                        `Splats: ${data.count}<br>Timestamp: ${new Date(data.timestamp).toLocaleTimeString()}`;
                }
            } catch(e) {
                document.getElementById('stats').innerHTML = 'Error: ' + e.message;
            }
        }
        fetchCloud();
        setInterval(fetchCloud, 1000); // refresh every second
        document.getElementById('stats').innerHTML = 'Loading...';

        function updateSplats(splats) {
            if (pointsMesh) scene.remove(pointsMesh);

            const geometry = new THREE.BufferGeometry();
            const positions = new Float32Array(splats.length * 3);
            const colors = new Float32Array(splats.length * 3);
            const sizes = new Float32Array(splats.length);

            splats.forEach((s, i) => {
                positions[i*3] = s.center[0];
                positions[i*3+1] = s.center[2]; // swap Y/Z for Three.js
                positions[i*3+2] = s.center[1];
                colors[i*3] = s.color[0];
                colors[i*3+1] = s.color[1];
                colors[i*3+2] = s.color[2];
                sizes[i] = (s.scale[0] + s.scale[1] + s.scale[2]) * 50;
            });

            geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
            geometry.setAttribute('color', new THREE.BufferAttribute(colors, 3));
            geometry.setAttribute('size', new THREE.BufferAttribute(sizes, 1));

            const material = new THREE.PointsMaterial({
                size: 0.05,
                vertexColors: true,
                sizeAttenuation: true,
                transparent: true,
                opacity: 0.8,
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
