# ADR-054: RuView Desktop Full Implementation

## Status
**Accepted** — Implementation in progress

## Context

RuView Desktop v0.3.0 shipped with a complete React/TypeScript frontend but stub-only Rust backend commands. Users report:
- Settings cannot be saved (#206) ✅ Fixed in PR #209
- Flash firmware does nothing
- OTA updates are non-functional
- Node discovery returns hardcoded data
- Server start/stop is cosmetic only

This ADR defines the complete implementation plan to make all desktop features production-ready with proper security, optimization, and error handling.

## Decision

Implement all 14 Tauri commands with full functionality, security hardening, and performance optimization.

---

## 1. Command Implementation Matrix

| Module | Command | Current | Target | Priority | Security |
|--------|---------|---------|--------|----------|----------|
| **Settings** | `get_settings` | ✅ Done | ✅ Done | P0 | File permissions |
| | `save_settings` | ✅ Done | ✅ Done | P0 | Input validation |
| **Discovery** | `discover_nodes` | Stub | Full mDNS + UDP | P1 | Network boundary |
| | `list_serial_ports` | Stub | Real enumeration | P1 | USB device access |
| **Flash** | `flash_firmware` | Stub | espflash integration | P1 | Binary validation |
| | `flash_progress` | Stub | Event streaming | P1 | Progress channel |
| **OTA** | `ota_update` | Stub | HTTP multipart + PSK | P1 | TLS + PSK auth |
| | `batch_ota_update` | Stub | Parallel with backoff | P2 | Rate limiting |
| **WASM** | `wasm_list` | Stub | HTTP GET /api/wasm | P2 | Response validation |
| | `wasm_upload` | Stub | HTTP POST multipart | P2 | Size limits, signing |
| | `wasm_control` | Stub | HTTP POST commands | P2 | Action whitelist |
| **Server** | `start_server` | Partial | Child process spawn | P1 | Port validation |
| | `stop_server` | Partial | Graceful shutdown | P1 | PID verification |
| | `server_status` | Partial | Health check | P1 | Timeout handling |
| **Provision** | `provision_node` | Stub | NVS binary write | P2 | Serial validation |
| | `read_nvs` | Stub | NVS binary read | P2 | Parse validation |

---

## 2. Implementation Details

### 2.1 Discovery Module

**Dependencies:**
```toml
mdns-sd = "0.11"
serialport = "4.6"
tokio = { version = "1", features = ["net", "time"] }
```

**discover_nodes Implementation:**
```rust
pub async fn discover_nodes(timeout_ms: Option<u64>) -> Result<Vec<DiscoveredNode>, String> {
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(3000));
    let mut nodes = Vec::new();

    // 1. mDNS discovery (_ruview._tcp.local)
    let mdns = ServiceDaemon::new()?;
    let receiver = mdns.browse("_ruview._tcp.local.")?;

    // 2. UDP broadcast probe (port 5005)
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.set_broadcast(true)?;
    socket.send_to(b"RUVIEW_DISCOVER", "255.255.255.255:5005").await?;

    // 3. Collect responses with timeout
    tokio::select! {
        _ = collect_mdns(&receiver, &mut nodes) => {},
        _ = collect_udp(&socket, &mut nodes) => {},
        _ = tokio::time::sleep(timeout) => {},
    }

    Ok(nodes)
}
```

**list_serial_ports Implementation:**
```rust
pub async fn list_serial_ports() -> Result<Vec<SerialPortInfo>, String> {
    let ports = serialport::available_ports()
        .map_err(|e| format!("Failed to enumerate ports: {}", e))?;

    Ok(ports.into_iter().map(|p| SerialPortInfo {
        name: p.port_name,
        vid: extract_vid(&p.port_type),
        pid: extract_pid(&p.port_type),
        manufacturer: extract_manufacturer(&p.port_type),
        chip: detect_esp_chip(&p.port_type),
    }).collect())
}
```

### 2.2 Flash Module

**Dependencies:**
```toml
espflash = "4.0"
tokio = { version = "1", features = ["sync"] }
```

**flash_firmware Implementation:**
```rust
pub async fn flash_firmware(
    port: String,
    firmware_path: String,
    chip: Option<String>,
    baud: Option<u32>,
    app: AppHandle,
) -> Result<FlashResult, String> {
    // 1. Validate firmware binary
    let firmware = std::fs::read(&firmware_path)
        .map_err(|e| format!("Cannot read firmware: {}", e))?;
    validate_esp_binary(&firmware)?;

    // 2. Open serial connection
    let serial = serialport::new(&port, baud.unwrap_or(460800))
        .timeout(Duration::from_secs(30))
        .open()
        .map_err(|e| format!("Cannot open {}: {}", port, e))?;

    // 3. Connect to ESP bootloader
    let mut flasher = Flasher::connect(serial, None, None)?;

    // 4. Flash with progress callback
    let start = Instant::now();
    flasher.write_bin_to_flash(
        0x0,
        &firmware,
        Some(&mut |current, total| {
            let _ = app.emit("flash_progress", FlashProgress {
                phase: "writing".into(),
                progress_pct: (current as f32 / total as f32) * 100.0,
                bytes_written: current as u64,
                bytes_total: total as u64,
            });
        }),
    )?;

    Ok(FlashResult {
        success: true,
        message: "Flash complete".into(),
        duration_secs: start.elapsed().as_secs_f64(),
    })
}
```

### 2.3 OTA Module

**Dependencies:**
```toml
reqwest = { version = "0.12", features = ["multipart", "rustls-tls"] }
sha2 = "0.10"
```

**ota_update Implementation:**
```rust
pub async fn ota_update(
    node_ip: String,
    firmware_path: String,
    psk: Option<String>,
) -> Result<OtaResult, String> {
    // 1. Validate IP format
    let ip: IpAddr = node_ip.parse()
        .map_err(|_| "Invalid IP address")?;

    // 2. Read and hash firmware
    let firmware = tokio::fs::read(&firmware_path).await
        .map_err(|e| format!("Cannot read firmware: {}", e))?;
    let hash = Sha256::digest(&firmware);

    // 3. Build multipart request
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;

    let form = multipart::Form::new()
        .part("firmware", multipart::Part::bytes(firmware)
            .file_name("firmware.bin")
            .mime_str("application/octet-stream")?);

    // 4. Send with PSK auth header
    let mut req = client.post(format!("http://{}:8032/ota", ip))
        .multipart(form);

    if let Some(key) = psk {
        req = req.header("X-OTA-PSK", key);
    }

    let resp = req.send().await
        .map_err(|e| format!("OTA request failed: {}", e))?;

    if resp.status().is_success() {
        Ok(OtaResult {
            success: true,
            node_ip: node_ip.clone(),
            message: "OTA update initiated".into(),
        })
    } else {
        Err(format!("OTA failed: {}", resp.status()))
    }
}
```

**batch_ota_update Implementation:**
```rust
pub async fn batch_ota_update(
    node_ips: Vec<String>,
    firmware_path: String,
    psk: Option<String>,
    strategy: Option<String>,
) -> Result<Vec<OtaResult>, String> {
    let firmware = Arc::new(tokio::fs::read(&firmware_path).await?);
    let psk = Arc::new(psk);

    let strategy = strategy.unwrap_or("sequential".into());

    match strategy.as_str() {
        "parallel" => {
            // All at once (max 4 concurrent)
            let semaphore = Arc::new(Semaphore::new(4));
            let handles: Vec<_> = node_ips.into_iter().map(|ip| {
                let fw = firmware.clone();
                let key = psk.clone();
                let sem = semaphore.clone();
                tokio::spawn(async move {
                    let _permit = sem.acquire().await;
                    ota_single(&ip, &fw, key.as_ref().as_ref()).await
                })
            }).collect();

            let results = futures::future::join_all(handles).await;
            Ok(results.into_iter().filter_map(|r| r.ok()).collect())
        }
        "tdm_safe" => {
            // One per TDM slot group with delays
            let mut results = Vec::new();
            for ip in node_ips {
                results.push(ota_single(&ip, &firmware, psk.as_ref().as_ref()).await);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            Ok(results)
        }
        _ => {
            // Sequential (default)
            let mut results = Vec::new();
            for ip in node_ips {
                results.push(ota_single(&ip, &firmware, psk.as_ref().as_ref()).await);
            }
            Ok(results)
        }
    }
}
```

### 2.4 Server Module

**Dependencies:**
```toml
tokio = { version = "1", features = ["process"] }
sysinfo = "0.32"
```

**start_server Implementation:**
```rust
pub async fn start_server(
    config: ServerConfig,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // 1. Check if already running
    {
        let srv = state.server.lock().map_err(|e| e.to_string())?;
        if srv.running {
            return Err("Server already running".into());
        }
    }

    // 2. Validate ports
    validate_port(config.http_port.unwrap_or(8080))?;
    validate_port(config.ws_port.unwrap_or(8765))?;

    // 3. Spawn sensing server as child process
    let child = Command::new("wifi-densepose-sensing-server")
        .args([
            "--http-port", &config.http_port.unwrap_or(8080).to_string(),
            "--ws-port", &config.ws_port.unwrap_or(8765).to_string(),
            "--udp-port", &config.udp_port.unwrap_or(5005).to_string(),
        ])
        .spawn()
        .map_err(|e| format!("Failed to start server: {}", e))?;

    // 4. Update state
    let mut srv = state.server.lock().map_err(|e| e.to_string())?;
    srv.running = true;
    srv.pid = Some(child.id());
    srv.child = Some(child);

    Ok(())
}
```

**stop_server Implementation:**
```rust
pub async fn stop_server(state: State<'_, AppState>) -> Result<(), String> {
    let mut srv = state.server.lock().map_err(|e| e.to_string())?;

    if let Some(mut child) = srv.child.take() {
        // Graceful shutdown via SIGTERM
        #[cfg(unix)]
        {
            use nix::sys::signal::{kill, Signal};
            use nix::unistd::Pid;
            let _ = kill(Pid::from_raw(child.id() as i32), Signal::SIGTERM);
        }

        // Wait up to 5s, then force kill
        tokio::select! {
            _ = child.wait() => {},
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                let _ = child.kill();
            }
        }
    }

    srv.running = false;
    srv.pid = None;

    Ok(())
}
```

### 2.5 WASM Module

**Dependencies:**
```toml
reqwest = { version = "0.12", features = ["json", "multipart"] }
```

**wasm_list Implementation:**
```rust
pub async fn wasm_list(node_ip: String) -> Result<Vec<WasmModuleInfo>, String> {
    let client = reqwest::Client::new();
    let resp = client.get(format!("http://{}:8080/api/wasm", node_ip))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Node returned {}", resp.status()));
    }

    let modules: Vec<WasmModuleInfo> = resp.json().await
        .map_err(|e| format!("Invalid response: {}", e))?;

    Ok(modules)
}
```

**wasm_upload Implementation:**
```rust
pub async fn wasm_upload(
    node_ip: String,
    wasm_path: String,
) -> Result<WasmUploadResult, String> {
    // 1. Validate WASM binary
    let wasm = tokio::fs::read(&wasm_path).await
        .map_err(|e| format!("Cannot read WASM: {}", e))?;

    if wasm.len() > 256 * 1024 {
        return Err("WASM module exceeds 256KB limit".into());
    }

    if &wasm[0..4] != b"\0asm" {
        return Err("Invalid WASM magic bytes".into());
    }

    // 2. Upload to node
    let client = reqwest::Client::new();
    let form = multipart::Form::new()
        .part("module", multipart::Part::bytes(wasm)
            .file_name(Path::new(&wasm_path).file_name().unwrap().to_string_lossy())
            .mime_str("application/wasm")?);

    let resp = client.post(format!("http://{}:8080/api/wasm", node_ip))
        .multipart(form)
        .timeout(Duration::from_secs(30))
        .send()
        .await?;

    if resp.status().is_success() {
        let result: WasmUploadResult = resp.json().await?;
        Ok(result)
    } else {
        Err(format!("Upload failed: {}", resp.status()))
    }
}
```

### 2.6 Provision Module

**Dependencies:**
```toml
nvs-partition-tool = "0.1"  # Or implement NVS binary format
serialport = "4.6"
```

**provision_node Implementation:**
```rust
pub async fn provision_node(
    port: String,
    config: ProvisioningConfig,
) -> Result<ProvisionResult, String> {
    // 1. Validate config
    config.validate()?;

    // 2. Build NVS binary blob
    let nvs_blob = build_nvs_blob(&config)?;

    // 3. Open serial port
    let mut serial = serialport::new(&port, 115200)
        .timeout(Duration::from_secs(10))
        .open()
        .map_err(|e| format!("Cannot open {}: {}", port, e))?;

    // 4. Enter bootloader mode
    enter_bootloader(&mut serial)?;

    // 5. Write NVS partition (offset 0x9000, size 0x6000)
    write_partition(&mut serial, 0x9000, &nvs_blob)?;

    // 6. Reset device
    reset_device(&mut serial)?;

    Ok(ProvisionResult {
        success: true,
        message: "Provisioning complete".into(),
    })
}
```

---

## 3. Security Hardening

### 3.1 Input Validation

```rust
// All string inputs sanitized
fn validate_ip(ip: &str) -> Result<IpAddr, String> {
    ip.parse::<IpAddr>().map_err(|_| "Invalid IP address".into())
}

fn validate_port(port: u16) -> Result<(), String> {
    if port < 1024 && port != 0 {
        return Err("Privileged ports (1-1023) not allowed".into());
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path);
    if path.components().any(|c| c == std::path::Component::ParentDir) {
        return Err("Path traversal detected".into());
    }
    Ok(path)
}
```

### 3.2 Network Security

```rust
// OTA PSK validation
fn validate_psk(psk: &str) -> Result<(), String> {
    if psk.len() < 16 {
        return Err("PSK must be at least 16 characters".into());
    }
    if !psk.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("PSK contains invalid characters".into());
    }
    Ok(())
}

// Rate limiting for network operations
struct RateLimiter {
    last_request: Instant,
    min_interval: Duration,
}

impl RateLimiter {
    fn check(&mut self) -> Result<(), String> {
        if self.last_request.elapsed() < self.min_interval {
            return Err("Rate limit exceeded".into());
        }
        self.last_request = Instant::now();
        Ok(())
    }
}
```

### 3.3 Binary Validation

```rust
fn validate_esp_binary(data: &[u8]) -> Result<(), String> {
    // Check ESP binary magic (0xE9 at offset 0)
    if data.is_empty() || data[0] != 0xE9 {
        return Err("Invalid ESP firmware magic byte".into());
    }

    // Check minimum size (header + some code)
    if data.len() < 256 {
        return Err("Firmware too small".into());
    }

    // Check maximum size (4MB flash)
    if data.len() > 4 * 1024 * 1024 {
        return Err("Firmware exceeds flash size".into());
    }

    Ok(())
}
```

---

## 4. Performance Optimization

### 4.1 Async Everything

All I/O operations are async with proper timeouts:

```rust
// Timeout wrapper
async fn with_timeout<T, F: Future<Output = Result<T, String>>>(
    future: F,
    duration: Duration,
) -> Result<T, String> {
    tokio::time::timeout(duration, future)
        .await
        .map_err(|_| "Operation timed out".into())?
}
```

### 4.2 Connection Pooling

```rust
// Reusable HTTP client
lazy_static! {
    static ref HTTP_CLIENT: reqwest::Client = reqwest::Client::builder()
        .pool_max_idle_per_host(5)
        .pool_idle_timeout(Duration::from_secs(30))
        .build()
        .unwrap();
}
```

### 4.3 Streaming Progress

Flash and OTA operations stream progress via Tauri events:

```rust
// Real-time progress updates
app.emit("flash_progress", FlashProgress { ... })?;
app.emit("ota_progress", OtaProgress { ... })?;
```

---

## 5. Testing Strategy

### 5.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_validate_ip() {
        assert!(validate_ip("192.168.1.1").is_ok());
        assert!(validate_ip("invalid").is_err());
    }

    #[test]
    fn test_validate_esp_binary() {
        let valid = vec![0xE9; 1024];
        assert!(validate_esp_binary(&valid).is_ok());

        let invalid = vec![0x00; 1024];
        assert!(validate_esp_binary(&invalid).is_err());
    }
}
```

### 5.2 Integration Tests

```rust
#[tokio::test]
async fn test_discover_nodes_timeout() {
    let result = discover_nodes(Some(100)).await;
    assert!(result.is_ok());
    // Should return empty or cached results within timeout
}
```

### 5.3 Mock Testing

```rust
// Mock serial port for flash tests
struct MockSerial {
    responses: VecDeque<Vec<u8>>,
}

impl Read for MockSerial { ... }
impl Write for MockSerial { ... }
```

---

## 6. Dependencies Update

**Cargo.toml additions:**
```toml
[dependencies]
# Discovery
mdns-sd = "0.11"
serialport = "4.6"

# HTTP client
reqwest = { version = "0.12", features = ["json", "multipart", "rustls-tls"] }

# Crypto
sha2 = "0.10"

# Process management
sysinfo = "0.32"

# Async
tokio = { version = "1", features = ["full"] }
futures = "0.3"

# Flash
espflash = "4.0"
```

---

## 7. Implementation Timeline

| Week | Deliverable |
|------|-------------|
| 1 | Discovery + Serial ports (real enumeration) |
| 1 | Server start/stop (child process management) |
| 2 | Flash firmware (espflash integration) |
| 2 | OTA update (HTTP multipart) |
| 3 | Batch OTA (parallel + sequential strategies) |
| 3 | WASM management (list/upload/control) |
| 4 | Provision NVS (binary format) |
| 4 | Security audit + E2E testing |

---

## 8. Rollout Plan

1. **v0.3.1** — Settings fix + Discovery + Server
2. **v0.4.0** — Flash + OTA (single node)
3. **v0.5.0** — Batch OTA + WASM + Provision
4. **v1.0.0** — Full E2E tested, security audited

---

## Consequences

### Positive
- Desktop app becomes fully functional
- Real device management capabilities
- Production-ready security posture
- Async performance throughout

### Negative
- Additional dependencies increase binary size
- espflash adds ~2MB to binary
- Hardware required for full testing

### Neutral
- Feature parity with browser-based UI
- Same API contract as sensing server

---

## References

- [Tauri v2 Commands](https://v2.tauri.app/develop/commands/)
- [espflash Documentation](https://github.com/esp-rs/espflash)
- [ESP32 OTA Protocol](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/system/ota.html)
- [mDNS-SD Rust](https://docs.rs/mdns-sd/)
