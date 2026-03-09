use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

/// Start the sensing server as a managed child process.
#[tauri::command]
pub async fn start_server(
    config: ServerConfig,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _ = config;
    let mut srv = state.server.lock().map_err(|e| e.to_string())?;
    srv.running = true;
    srv.pid = Some(0); // Stub PID
    Ok(())
}

/// Stop the managed sensing server process.
#[tauri::command]
pub async fn stop_server(state: State<'_, AppState>) -> Result<(), String> {
    let mut srv = state.server.lock().map_err(|e| e.to_string())?;
    srv.running = false;
    srv.pid = None;
    Ok(())
}

/// Get sensing server status.
#[tauri::command]
pub async fn server_status(state: State<'_, AppState>) -> Result<ServerStatusResponse, String> {
    let srv = state.server.lock().map_err(|e| e.to_string())?;
    Ok(ServerStatusResponse {
        running: srv.running,
        pid: srv.pid,
        http_port: None,
        ws_port: None,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub http_port: Option<u16>,
    pub ws_port: Option<u16>,
    pub udp_port: Option<u16>,
    pub log_level: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerStatusResponse {
    pub running: bool,
    pub pid: Option<u32>,
    pub http_port: Option<u16>,
    pub ws_port: Option<u16>,
}
