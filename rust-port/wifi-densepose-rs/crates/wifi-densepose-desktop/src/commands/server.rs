use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tauri::State;

use crate::state::AppState;

/// Default path to the sensing server binary (relative to resources).
const DEFAULT_SERVER_BIN: &str = "wifi-densepose-sensing-server";

/// Start the sensing server as a managed child process.
///
/// The server binary is looked up in the following order:
/// 1. Settings `server_path` if set
/// 2. Bundled resource path
/// 3. System PATH
#[tauri::command]
pub async fn start_server(
    config: ServerConfig,
    state: State<'_, AppState>,
) -> Result<ServerStartResult, String> {
    // Check if already running
    {
        let srv = state.server.lock().map_err(|e| e.to_string())?;
        if srv.running {
            return Err("Server is already running".into());
        }
    }

    // Determine server binary path
    let server_path = config.server_path
        .clone()
        .unwrap_or_else(|| DEFAULT_SERVER_BIN.to_string());

    // Build command with configuration
    let mut cmd = Command::new(&server_path);

    if let Some(port) = config.http_port {
        cmd.args(["--http-port", &port.to_string()]);
    }
    if let Some(port) = config.ws_port {
        cmd.args(["--ws-port", &port.to_string()]);
    }
    if let Some(port) = config.udp_port {
        cmd.args(["--udp-port", &port.to_string()]);
    }
    if let Some(ref bind_addr) = config.bind_address {
        cmd.args(["--bind", bind_addr]);
    }
    if let Some(ref log_level) = config.log_level {
        cmd.args(["--log-level", log_level]);
    }

    // Redirect stdout/stderr to pipes for monitoring
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Spawn the child process
    let child = cmd.spawn()
        .map_err(|e| format!("Failed to start server: {}. Is '{}' installed?", e, server_path))?;

    let pid = child.id();

    // Store the child process in state
    {
        let mut srv = state.server.lock().map_err(|e| e.to_string())?;
        srv.running = true;
        srv.pid = Some(pid);
        srv.http_port = config.http_port;
        srv.ws_port = config.ws_port;
        srv.udp_port = config.udp_port;
        srv.child = Some(child);
    }

    tracing::info!("Started sensing server with PID {}", pid);

    Ok(ServerStartResult {
        pid,
        http_port: config.http_port,
        ws_port: config.ws_port,
        udp_port: config.udp_port,
    })
}

/// Stop the managed sensing server process.
///
/// First attempts graceful termination (SIGTERM), then SIGKILL after timeout.
#[tauri::command]
pub async fn stop_server(state: State<'_, AppState>) -> Result<(), String> {
    // Extract child process ID and take ownership of child for killing
    // This releases the lock before any await points
    let child_id = {
        let srv = state.server.lock().map_err(|e| e.to_string())?;
        if !srv.running {
            return Err("Server is not running".into());
        }
        srv.pid
    };

    let child_id = match child_id {
        Some(id) => id,
        None => return Err("No server process found".into()),
    };

    // First try graceful termination
    #[cfg(unix)]
    {
        unsafe {
            libc::kill(child_id as i32, libc::SIGTERM);
        }
    }

    // Wait briefly for graceful shutdown (async operation - no lock held)
    let wait_result: Result<Result<bool, _>, _> = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking({
            move || {
                std::thread::sleep(std::time::Duration::from_millis(100));
                // Check if process is still alive
                let mut sys = System::new();
                let pid = Pid::from_u32(child_id);
                sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
                sys.process(pid).is_some()
            }
        })
    ).await;

    // Force kill if still running - re-acquire lock
    let still_running = match wait_result {
        Ok(Ok(running)) => running,
        _ => true,
    };

    {
        let mut srv = state.server.lock().map_err(|e| e.to_string())?;

        if still_running {
            if let Some(ref mut child) = srv.child {
                let _ = child.kill();
                let _ = child.wait();
            }
        }

        // Clear state
        srv.running = false;
        srv.pid = None;
        srv.http_port = None;
        srv.ws_port = None;
        srv.udp_port = None;
        srv.child = None;
    }

    tracing::info!("Stopped sensing server");

    Ok(())
}

/// Get sensing server status including resource usage.
#[tauri::command]
pub async fn server_status(state: State<'_, AppState>) -> Result<ServerStatusResponse, String> {
    let srv = state.server.lock().map_err(|e| e.to_string())?;

    if !srv.running || srv.pid.is_none() {
        return Ok(ServerStatusResponse {
            running: false,
            pid: None,
            http_port: None,
            ws_port: None,
            udp_port: None,
            memory_mb: None,
            cpu_percent: None,
            uptime_secs: None,
        });
    }

    let pid = srv.pid.unwrap();
    let mut sys = System::new();
    let sysinfo_pid = Pid::from_u32(pid);
    sys.refresh_processes(ProcessesToUpdate::Some(&[sysinfo_pid]), true);

    let (memory_mb, cpu_percent) = sys.process(sysinfo_pid)
        .map(|proc| {
            let mem = proc.memory() as f64 / 1024.0 / 1024.0;
            let cpu = proc.cpu_usage();
            (Some(mem), Some(cpu))
        })
        .unwrap_or((None, None));

    // Calculate uptime if we have start time
    let uptime_secs = srv.start_time.map(|start| {
        std::time::Instant::now().duration_since(start).as_secs()
    });

    Ok(ServerStatusResponse {
        running: srv.running,
        pid: Some(pid),
        http_port: srv.http_port,
        ws_port: srv.ws_port,
        udp_port: srv.udp_port,
        memory_mb,
        cpu_percent,
        uptime_secs,
    })
}

/// Restart the sensing server with the same or new configuration.
#[tauri::command]
pub async fn restart_server(
    config: Option<ServerConfig>,
    state: State<'_, AppState>,
) -> Result<ServerStartResult, String> {
    // Get current config if no new config provided
    let restart_config = if let Some(cfg) = config {
        cfg
    } else {
        let srv = state.server.lock().map_err(|e| e.to_string())?;
        ServerConfig {
            http_port: srv.http_port,
            ws_port: srv.ws_port,
            udp_port: srv.udp_port,
            log_level: None,
            bind_address: None,
            server_path: None,
        }
    };

    // Stop existing server
    let _ = stop_server(state.clone()).await;

    // Brief delay to ensure port is released
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Start with new config
    start_server(restart_config, state).await
}

/// Get server logs (last N lines from stdout/stderr).
#[tauri::command]
pub async fn server_logs(
    _lines: Option<usize>,
    state: State<'_, AppState>,
) -> Result<ServerLogsResponse, String> {
    let _srv = state.server.lock().map_err(|e| e.to_string())?;

    // For now, return empty logs - full implementation would capture stdout/stderr
    // to ring buffer during process lifetime
    Ok(ServerLogsResponse {
        stdout: Vec::new(),
        stderr: Vec::new(),
        truncated: false,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub http_port: Option<u16>,
    pub ws_port: Option<u16>,
    pub udp_port: Option<u16>,
    pub log_level: Option<String>,
    pub bind_address: Option<String>,
    pub server_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerStartResult {
    pub pid: u32,
    pub http_port: Option<u16>,
    pub ws_port: Option<u16>,
    pub udp_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerStatusResponse {
    pub running: bool,
    pub pid: Option<u32>,
    pub http_port: Option<u16>,
    pub ws_port: Option<u16>,
    pub udp_port: Option<u16>,
    pub memory_mb: Option<f64>,
    pub cpu_percent: Option<f32>,
    pub uptime_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerLogsResponse {
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
    pub truncated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig {
            http_port: Some(8080),
            ws_port: Some(8765),
            udp_port: Some(5005),
            log_level: None,
            bind_address: None,
            server_path: None,
        };

        assert_eq!(config.http_port, Some(8080));
        assert_eq!(config.ws_port, Some(8765));
    }
}
