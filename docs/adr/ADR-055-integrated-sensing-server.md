# ADR-055: Integrated Sensing Server in Desktop App

## Status
Accepted

## Context
The RuView Desktop application (ADR-054) requires the WiFi sensing server to provide real-time CSI data, activity detection, and vital signs monitoring. Currently, the sensing server is a separate binary (`wifi-densepose-sensing-server`) that must be installed separately and found in the system PATH.

This creates several problems:
1. **Distribution complexity**: Users must install two binaries
2. **Path issues**: Binary may not be in PATH, causing "No such file or directory" errors
3. **Version mismatch**: Server and desktop app versions may diverge
4. **Poor UX**: Error messages about missing binaries confuse users

## Decision
Bundle the sensing server binary inside the desktop application and provide intelligent binary discovery with clear fallback paths.

### Binary Discovery Order
The desktop app searches for the sensing server in this order:
1. **Custom path** from user settings (`server_path`)
2. **Bundled resources** (`Contents/Resources/bin/` on macOS)
3. **Next to executable** (same directory as the app binary)
4. **System PATH** (legacy fallback)

### Implementation
```rust
fn find_server_binary(app: &AppHandle, custom_path: Option<&str>) -> Result<String, String> {
    // 1. Custom path from settings
    if let Some(path) = custom_path {
        if std::path::Path::new(path).exists() {
            return Ok(path.to_string());
        }
    }

    // 2. Bundled in resources
    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled = resource_dir.join("bin").join(DEFAULT_SERVER_BIN);
        if bundled.exists() {
            return Ok(bundled.to_string_lossy().to_string());
        }
    }

    // 3. Next to executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let sibling = exe_dir.join(DEFAULT_SERVER_BIN);
            if sibling.exists() {
                return Ok(sibling.to_string_lossy().to_string());
            }
        }
    }

    // 4. System PATH
    // ... which lookup ...

    Err("Sensing server binary not found")
}
```

### Bundle Configuration
In `tauri.conf.json`:
```json
{
  "bundle": {
    "resources": [
      {
        "src": "../../target/release/wifi-densepose-sensing-server",
        "target": "bin/wifi-densepose-sensing-server"
      }
    ]
  }
}
```

## Consequences

### Positive
- **Single package distribution**: Users download one DMG/MSI/EXE
- **Version alignment**: Server and UI always match
- **Better UX**: No PATH configuration required
- **Offline capable**: Works without network access to download server

### Negative
- **Larger bundle size**: ~10-15MB additional for server binary
- **Build complexity**: Must build server before bundling desktop
- **Platform-specific**: Need separate server binaries per platform

### Neutral
- CI/CD workflow updated to build server before desktop
- GitHub Actions builds all platforms (macOS arm64/x64, Windows x64)

## WebSocket Integration
The Sensing page connects to the bundled server's WebSocket endpoint:
- `ws://127.0.0.1:{ws_port}/ws/sensing` - Real-time CSI data stream
- `ws://127.0.0.1:{ws_port}/ws/pose` - Pose estimation stream

Message format:
```typescript
interface WsSensingUpdate {
  type: string;
  timestamp: number;
  source: string;
  tick: number;
  nodes: WsNodeInfo[];
  classification: { motion_level: string; presence: boolean; confidence: number };
  vital_signs?: { breathing_rate_hz?: number; heart_rate_bpm?: number };
}
```

## Security Considerations
- Server binary signed with same certificate as desktop app
- Communication over localhost only (127.0.0.1)
- No external network access by default
- Process spawned as child of desktop app (inherits permissions)

## Related ADRs
- ADR-054: Desktop Full Implementation
- ADR-053: UI Design System
- ADR-052: Tauri Desktop Frontend
