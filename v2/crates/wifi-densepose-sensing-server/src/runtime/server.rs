//! Server orchestration: startup, background tasks, listener binding, and graceful shutdown.
//!
//! Complies with ELMAS.md: <= 350 lines per file, early returns, clean abstraction.

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use wifi_densepose_aether::graph_transformer;
use wifi_densepose_sensing_server::cli::Args;
use wifi_densepose_sensing_server::discovery;
use wifi_densepose_sensing_server::host_validation::HostAllowlist;
use wifi_densepose_sensing_server::ingest::{plan_source, probe_esp32, probe_wifi};
use wifi_densepose_sensing_server::rufield_surface::{self, FieldState};
use wifi_densepose_sensing_server::rvf_container::{RvfBuilder, VitalSignConfig};
use wifi_densepose_sensing_server::{bearer_auth, http_bind, udp_bind, udp_tee};
use crate::mediatek_activity;

use super::eviction::node_states_eviction_task;
use super::ingress::udp_receiver_task;
use super::router::{build_http_router, build_ws_router};
use super::simulation::simulated_data_task;
use super::state_init::init_server_state;
use super::ticker::broadcast_tick_task;
use super::wifi::wifi_task;
use crate::SharedState;

/// Run the full Sensing Server lifecycle.
pub async fn run_server(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    info!("WiFi-DensePose Sensing Server (Rust + Axum + RuVector)");
    info!("  HTTP:      http://localhost:{}", args.http_port);
    info!("  WebSocket: ws://localhost:{}/ws/sensing", args.ws_port);
    info!("  UDP:       {}:{} (ESP32 CSI)", args.udp_bind, args.udp_port);
    info!("  UI path:   {}", args.ui_path.display());
    info!("  Source:    {}", args.source);

    let normalized = if args.source == "simulate" { "simulated" } else { args.source.as_str() };
    let plan = if normalized == "auto" {
        info!("Auto-detecting data source (UDP :{} bound either way)...", args.udp_port);
        let esp32 = probe_esp32(args.udp_port).await;
        let wifi = if esp32 { false } else { probe_wifi().await };
        if esp32 {
            info!("  ESP32 CSI detected on UDP :{}", args.udp_port);
        } else if wifi {
            info!("  WiFi detected ({})", std::env::consts::OS);
        } else {
            warn!(
                "No real CSI source at boot — serving SIMULATED data while UDP :{} stays bound.",
                args.udp_port
            );
        }
        plan_source("auto", esp32, wifi)
    } else {
        plan_source(normalized, false, false)
    }?;

    let source = plan.initial_source.as_str();
    info!(
        "Data source: {source} (udp_receiver={}, simulator={}, wifi={})",
        plan.bind_udp, plan.run_simulator, plan.run_wifi
    );

    let field_surface: FieldState = Arc::new(RwLock::new(rufield_surface::FieldSurface::from_env()));
    let state = init_server_state(&args, source, &field_surface);

    if plan.bind_udp {
        spawn_udp_and_eviction_tasks(&args, &state).await?;
    }
    if plan.run_wifi {
        tokio::spawn(wifi_task(state.clone(), args.tick_ms));
    }
    if plan.run_simulator {
        tokio::spawn(simulated_data_task(state.clone(), args.tick_ms));
    }
    if let Some(activity_state_path) = args.activity_state.clone() {
        tokio::spawn(mediatek_activity::activity_state_persist_task(
            state.clone(),
            activity_state_path,
        ));
    }

    let bind_ip: std::net::IpAddr = args.bind_addr.parse()?;
    let bearer_auth_state = bearer_auth::AuthState::from_env()?;
    if bearer_auth_state.is_enabled() {
        info!("API auth: ON for /api/v1/*");
    } else {
        info!("API auth: OFF — /api/v1/* is unauthenticated.");
    }

    let allow_unauth = http_bind::allow_unauthenticated_opt_in(
        std::env::var(http_bind::ALLOW_UNAUTHENTICATED_ENV).ok().as_deref(),
    );
    http_bind::decide_http_bind(bind_ip, bearer_auth_state.is_enabled(), allow_unauth)?;

    let host_allowlist = if args.disable_host_validation {
        warn!("Host-header validation DISABLED");
        HostAllowlist::disabled()
    } else {
        let label = args.installation_id.as_deref().unwrap_or("installation");
        let disc_host = discovery::discovery_hostname(label);
        HostAllowlist::from_cli_and_env(args.allowed_hosts.iter().cloned().chain(std::iter::once(disc_host)))
    };

    let ws_app = build_ws_router(
        state.clone(),
        field_surface.clone(),
        args.privacy_mode,
        bearer_auth_state.clone(),
        host_allowlist.clone(),
    );
    let ws_addr = SocketAddr::from((bind_ip, args.ws_port));
    let ws_listener = tokio::net::TcpListener::bind(ws_addr).await?;
    info!("WebSocket server listening on {ws_addr}");
    tokio::spawn(async move {
        let _ = axum::serve(ws_listener, ws_app).await;
    });

    let edge_registry = if args.no_edge_registry || args.edge_registry_url.is_empty() {
        None
    } else {
        Some(Arc::new(wifi_densepose_sensing_server::edge_registry::EdgeRegistry::new(
            args.edge_registry_url.clone(),
            std::time::Duration::from_secs(args.edge_registry_ttl_secs),
        )))
    };

    let http_app = build_http_router(
        state.clone(),
        field_surface,
        &args.ui_path,
        edge_registry,
        args.privacy_mode,
        bearer_auth_state,
        host_allowlist,
    );
    let http_addr = SocketAddr::from((bind_ip, args.http_port));
    let http_listener = tokio::net::TcpListener::bind(http_addr).await?;
    info!("HTTP server listening on {http_addr}");
    info!("Open http://localhost:{}/ui/index.html in your browser", args.http_port);

    let shutdown_state = state.clone();
    let server = axum::serve(http_listener, http_app).with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
        info!("Shutdown signal received");
    });
    server.await?;

    handle_shutdown_save(&shutdown_state).await;
    info!("Server shut down cleanly");
    Ok(())
}

async fn spawn_udp_and_eviction_tasks(
    args: &Args,
    state: &SharedState,
) -> Result<(), Box<dyn std::error::Error>> {
    let udp_bind_ip: std::net::IpAddr = args.udp_bind.parse()?;
    let udp_allowlist = Arc::new(udp_bind::UdpSourceAllowlist::parse(args.udp_allow.iter())?);
    let decision = udp_bind::decide_udp_bind(udp_bind_ip, udp_allowlist.is_active(), args.udp_insecure_lan)?;
    info!(
        "UDP data plane security: {}",
        udp_bind::startup_summary(decision, udp_bind_ip, args.udp_port, &udp_allowlist)
    );

    let udp_tee = match udp_tee::parse_targets(args.udp_tee.iter(), args.udp_port) {
        Ok(targets) if targets.is_empty() => None,
        Ok(targets) => Some(Arc::new(udp_tee::UdpTee::bind(targets)?)),
        Err(e) => return Err(e.into()),
    };

    tokio::spawn(udp_receiver_task(
        state.clone(),
        udp_bind_ip,
        args.udp_port,
        udp_allowlist,
        udp_tee,
    ));
    tokio::spawn(broadcast_tick_task(state.clone(), args.tick_ms));
    tokio::spawn(node_states_eviction_task(state.clone()));
    Ok(())
}

async fn handle_shutdown_save(state: &SharedState) {
    let s = state.read().await;
    let Some(ref save_path) = s.save_rvf_path else { return; };
    info!("Saving RVF container to {}", save_path.display());

    let mut builder = RvfBuilder::new();
    builder.add_manifest(
        "wifi-densepose-sensing",
        env!("CARGO_PKG_VERSION"),
        "WiFi DensePose sensing model state",
    );
    builder.add_metadata(&serde_json::json!({
        "source": s.effective_source(),
        "total_ticks": s.tick,
        "total_detections": s.total_detections,
        "uptime_secs": s.start_time.elapsed().as_secs(),
    }));
    builder.add_vital_config(&VitalSignConfig::default());

    let weights: Vec<f32> = if s.model_loaded {
        let tf = graph_transformer::CsiToPoseTransformer::new(Default::default());
        tf.flatten_weights()
    } else {
        Vec::new()
    };
    builder.add_weights(&weights);
    match builder.write_to_file(save_path) {
        Ok(()) => info!("  RVF saved ({} weight params)", weights.len()),
        Err(e) => error!("  Failed to save RVF: {e}"),
    }
}
