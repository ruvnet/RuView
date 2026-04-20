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
        #info { position: absolute; top: 10px; left: 10px; padding: 12px; background: rgba(0,0,0,0.85); border: 1px solid #e8a634; border-radius: 6px; min-width: 240px; font-size: 13px; line-height: 1.5; }
        .live { color: #4f4; } .demo { color: #f44; }
        .section { margin-top: 6px; padding-top: 6px; border-top: 1px solid #333; }
        .label { color: #888; }
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
        var scene = new THREE.Scene();
        scene.background = new THREE.Color(0x0a0a0a);
        var camera = new THREE.PerspectiveCamera(75, window.innerWidth/window.innerHeight, 0.1, 100);
        camera.position.set(0, 2, -4);
        camera.lookAt(0, 0, 2);

        var renderer = new THREE.WebGLRenderer({ antialias: true });
        renderer.setSize(window.innerWidth, window.innerHeight);
        document.body.appendChild(renderer.domElement);

        var controls = new THREE.OrbitControls(camera, renderer.domElement);
        controls.enableDamping = true;
        controls.target.set(0, 0, 2);

        var pointsMesh = null;
        var lastFrame = -1;
        var skeletonGroup = null;
        var prevTimestamp = 0;
        var frameRateVal = 0;

        // COCO skeleton connections: pairs of keypoint indices
        // 0=nose 1=leftEye 2=rightEye 3=leftEar 4=rightEar
        // 5=leftShoulder 6=rightShoulder 7=leftElbow 8=rightElbow
        // 9=leftWrist 10=rightWrist 11=leftHip 12=rightHip
        // 13=leftKnee 14=rightKnee 15=leftAnkle 16=rightAnkle
        var COCO_BONES = [
            [0,1],[0,2],[1,3],[2,4],
            [5,6],[5,7],[7,9],[6,8],[8,10],
            [5,11],[6,12],[11,12],
            [11,13],[13,15],[12,14],[14,16]
        ];

        function clearSkeleton() {
            if (skeletonGroup) {
                scene.remove(skeletonGroup);
                skeletonGroup.traverse(function(obj) {
                    if (obj.geometry) obj.geometry.dispose();
                    if (obj.material) obj.material.dispose();
                });
                skeletonGroup = null;
            }
        }

        function drawSkeleton(keypoints) {
            clearSkeleton();
            if (!keypoints || keypoints.length < 17) return;
            skeletonGroup = new THREE.Group();

            // Map keypoints from [0,1] to scene coords
            // x: [-2, 2], y: [2, -2] (flip y), z: fixed at 2
            var sphereGeo = new THREE.SphereGeometry(0.04, 8, 8);
            var sphereMat = new THREE.MeshBasicMaterial({ color: 0xffff00 });
            var positions3D = [];
            var i, kp, sx, sy;
            for (i = 0; i < 17; i++) {
                kp = keypoints[i];
                if (!kp) { positions3D.push(null); continue; }
                sx = (kp[0] - 0.5) * 4;
                sy = (0.5 - kp[1]) * 4;
                positions3D.push([sx, sy, 2]);
                var sphere = new THREE.Mesh(sphereGeo, sphereMat);
                sphere.position.set(sx, sy, 2);
                skeletonGroup.add(sphere);
            }

            // Draw bones as white lines
            var lineMat = new THREE.LineBasicMaterial({ color: 0xffffff, linewidth: 2 });
            var b, a, bIdx;
            for (b = 0; b < COCO_BONES.length; b++) {
                a = COCO_BONES[b][0];
                bIdx = COCO_BONES[b][1];
                if (!positions3D[a] || !positions3D[bIdx]) continue;
                var lineGeo = new THREE.BufferGeometry();
                var verts = new Float32Array([
                    positions3D[a][0], positions3D[a][1], positions3D[a][2],
                    positions3D[bIdx][0], positions3D[bIdx][1], positions3D[bIdx][2]
                ]);
                lineGeo.setAttribute("position", new THREE.BufferAttribute(verts, 3));
                var line = new THREE.Line(lineGeo, lineMat);
                skeletonGroup.add(line);
            }

            scene.add(skeletonGroup);
        }

        async function fetchCloud() {
            try {
                var resp = await fetch("/api/splats");
                var data = await resp.json();
                if (data.splats && data.frame !== lastFrame) {
                    // Compute CSI frame rate
                    var now = Date.now();
                    if (prevTimestamp > 0) {
                        var dt = (now - prevTimestamp) / 1000.0;
                        if (dt > 0) frameRateVal = (1.0 / dt).toFixed(1);
                    }
                    prevTimestamp = now;
                    lastFrame = data.frame;
                    updateSplats(data.splats);

                    // Draw skeleton if available
                    var pipe = data.pipeline;
                    if (pipe && pipe.skeleton && pipe.skeleton.keypoints) {
                        drawSkeleton(pipe.skeleton.keypoints);
                    } else {
                        clearSkeleton();
                    }

                    // Build info panel
                    var mode = data.live
                        ? '<span class="live">&#9679; LIVE</span>'
                        : '<span class="demo">&#9679; DEMO</span>';
                    var html = mode + " Camera + CSI<br>"
                        + "Splats: " + data.count + "<br>"
                        + "Frame: " + data.frame;

                    // CSI frame rate
                    html += '<div class="section">'
                        + '<span class="label">CSI Rate:</span> '
                        + frameRateVal + " fps</div>";

                    // Skeleton confidence
                    if (pipe && pipe.skeleton && pipe.skeleton.confidence !== undefined) {
                        var conf = (pipe.skeleton.confidence * 100).toFixed(0);
                        html += '<div class="section">'
                            + '<span class="label">Skeleton:</span> '
                            + conf + "% confidence</div>";
                    }

                    // Weather data
                    if (pipe && pipe.weather) {
                        var w = pipe.weather;
                        html += '<div class="section">'
                            + '<span class="label">Weather:</span> ';
                        if (w.temperature !== undefined) {
                            html += w.temperature + "&deg;C";
                        }
                        if (w.conditions) {
                            html += " " + w.conditions;
                        }
                        html += "</div>";
                    }

                    // Building count from geo
                    if (pipe && pipe.geo && pipe.geo.building_count !== undefined) {
                        html += '<div class="section">'
                            + '<span class="label">Buildings:</span> '
                            + pipe.geo.building_count + "</div>";
                    }

                    // Vitals
                    if (pipe && pipe.vitals) {
                        var v = pipe.vitals;
                        html += '<div class="section">'
                            + '<span class="label">Vitals:</span> ';
                        if (v.breathing_rate !== undefined) {
                            html += "BR " + v.breathing_rate + "/min";
                        }
                        if (v.motion_score !== undefined) {
                            html += " Motion " + (v.motion_score * 100).toFixed(0) + "%";
                        }
                        html += "</div>";
                    }

                    document.getElementById("stats").innerHTML = html;
                }
            } catch(e) {}
        }
        fetchCloud();
        setInterval(fetchCloud, 500);

        function updateSplats(splats) {
            if (pointsMesh) scene.remove(pointsMesh);
            var geometry = new THREE.BufferGeometry();
            var positions = new Float32Array(splats.length * 3);
            var colors = new Float32Array(splats.length * 3);
            var i, s;
            for (i = 0; i < splats.length; i++) {
                s = splats[i];
                positions[i*3] = s.center[0];
                positions[i*3+1] = -s.center[1];
                positions[i*3+2] = s.center[2];
                colors[i*3] = s.color[0];
                colors[i*3+1] = s.color[1];
                colors[i*3+2] = s.color[2];
            }
            geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
            geometry.setAttribute("color", new THREE.BufferAttribute(colors, 3));
            pointsMesh = new THREE.Points(geometry, new THREE.PointsMaterial({
                size: 0.02, vertexColors: true, sizeAttenuation: true
            }));
            scene.add(pointsMesh);
        }

        function animate() {
            requestAnimationFrame(animate);
            controls.update();
            renderer.render(scene, camera);
        }
        animate();
        window.addEventListener("resize", function() {
            camera.aspect = window.innerWidth / window.innerHeight;
            camera.updateProjectionMatrix();
            renderer.setSize(window.innerWidth, window.innerHeight);
        });
    </script>
</body>
</html>"#.to_string())
}
