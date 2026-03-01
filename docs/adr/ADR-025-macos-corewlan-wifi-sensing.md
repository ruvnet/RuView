# ADR-025: macOS CoreWLAN WiFi Sensing via Swift Helper Bridge

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-03-01 |
| **Deciders** | ruv |
| **Codename** | **ORCA** — OS-native Radio Channel Acquisition |
| **Relates to** | ADR-013 (Feature-Level Sensing Commodity Gear), ADR-022 (Windows WiFi Enhanced Fidelity), ADR-014 (SOTA Signal Processing), ADR-018 (ESP32 Dev Implementation) |
| **Issue** | [#56](https://github.com/ruvnet/wifi-densepose/issues/56) |
| **Build/Test Target** | Mac Mini (M2 Pro, macOS 26.3) |

---

## 1. Context

### 1.1 The Gap: macOS Is a Silent Fallback

The `--source auto` path in `sensing-server` probes for ESP32 UDP, then Windows `netsh`, then falls back to simulated mode. macOS users hit the simulation path silently — there is no macOS WiFi adapter. This is the only major desktop platform without real WiFi sensing support.

### 1.2 Platform Constraints (macOS 26.3+)

| Constraint | Detail |
|------------|--------|
| **`airport` CLI removed** | Apple removed `/System/Library/PrivateFrameworks/.../airport` in macOS 15. No CLI fallback exists. |
| **CoreWLAN is the only path** | `CWWiFiClient` (Swift/ObjC) is the supported API for WiFi scanning. Returns RSSI, channel, SSID, noise, PHY mode, security. |
| **BSSIDs redacted** | macOS privacy policy redacts MAC addresses from `CWNetwork.bssid` unless the app has Location Services + WiFi entitlement. Apps without entitlement see `nil` for BSSID. |
| **No raw CSI** | Apple does not expose CSI or per-subcarrier data. macOS WiFi sensing is RSSI-only, same tier as Windows `netsh`. |
| **Scan rate** | `CWInterface.scanForNetworks()` takes ~2-4 seconds. Effective rate: ~0.3-0.5 Hz without caching. |
| **Permissions** | Location Services prompt required for BSSID access. Without it, SSID + RSSI + channel still available. |

### 1.3 The Opportunity: Multi-AP RSSI Diversity

Same principle as ADR-022 (Windows): visible APs serve as pseudo-subcarriers. A typical indoor environment exposes 10-30+ SSIDs across 2.4 GHz and 5 GHz bands. Each AP's RSSI responds differently to human movement based on geometry, creating spatial diversity.

| Source | Effective Subcarriers | Sample Rate | Capabilities |
|--------|----------------------|-------------|-------------|
| ESP32-S3 (CSI) | 56-192 | 20 Hz | Full: pose, vitals, through-wall |
| Windows `netsh` (ADR-022) | 10-30 BSSIDs | ~2 Hz | Presence, motion, coarse breathing |
| **macOS CoreWLAN (this ADR)** | **10-30 SSIDs** | **~0.3-0.5 Hz** | **Presence, motion** |

The lower scan rate vs Windows is offset by higher signal quality — CoreWLAN returns calibrated dBm (not percentage) plus noise floor, enabling proper SNR computation.

### 1.4 Why Swift Subprocess (Not FFI)

| Approach | Complexity | Maintenance | Build | Verdict |
|----------|-----------|-------------|-------|---------|
| **Swift CLI → JSON → stdout** | Low | Independent binary, versionable | `swiftc` (ships with Xcode CLT) | **Chosen** |
| ObjC FFI via `cc` crate | Medium | Fragile header bindings, ABI churn | Requires Xcode headers | Rejected |
| `objc2` crate (Rust ObjC bridge) | High | CoreWLAN not in upstream `objc2-frameworks` | Requires manual class definitions | Rejected |
| `swift-bridge` crate | High | Young ecosystem, async bridging unsupported | Requires Swift build integration in Cargo | Rejected |

The `Command::new()` + parse JSON pattern is proven — it's exactly what `NetshBssidScanner` does for Windows. The subprocess boundary also isolates Apple framework dependencies from the Rust build graph.

### 1.5 SOTA: Platform-Adaptive WiFi Sensing

Recent work validates multi-platform RSSI-based sensing:

- **WiFind** (2024): Cross-platform WiFi fingerprinting using RSSI vectors from heterogeneous hardware. Demonstrates that normalization across scan APIs (dBm, percentage, raw) is critical for model portability.
- **WiGesture** (2025): RSSI variance-based gesture recognition achieving 89% accuracy on commodity hardware with 15+ APs. Shows that temporal RSSI variance alone carries significant motion information.
- **CrossSense** (2024): Transfer learning from CSI-rich hardware to RSSI-only devices. Pre-trained signal features transfer with 78% effectiveness, validating multi-tier hardware strategy.

---

## 2. Decision

Implement a **macOS CoreWLAN sensing adapter** as a Swift helper binary + Rust adapter pair, following the established `NetshBssidScanner` subprocess pattern from ADR-022. Real RSSI data flows through the existing 8-stage `WindowsWifiPipeline` (which operates on `BssidObservation` structs regardless of platform origin).

### 2.1 Design Principles

1. **Subprocess isolation** — Swift binary is a standalone tool, built and versioned independently of the Rust workspace.
2. **Same domain types** — macOS adapter produces `Vec<BssidObservation>`, identical to the Windows path. All downstream processing reuses as-is.
3. **SSID:channel as synthetic BSSID** — When real BSSIDs are redacted (no Location Services), `sha256(ssid + channel)[:12]` generates a stable pseudo-BSSID. Documented limitation: same-SSID same-channel APs collapse to one observation.
4. **`#[cfg(target_os = "macos")]` gating** — macOS-specific code compiles only on macOS. Windows and Linux builds are unaffected.
5. **Graceful degradation** — If the Swift helper is not found or fails, `--source auto` skips macOS WiFi and falls back to simulated mode with a clear warning.

---

## 3. Architecture

### 3.1 Component Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                     macOS WiFi Sensing Path                         │
│                                                                     │
│  ┌──────────────────────┐     ┌───────────────────────────────────┐│
│  │  Swift Helper Binary  │     │  Rust Adapter + Existing Pipeline ││
│  │  (tools/macos-wifi-   │     │                                   ││
│  │   scan/main.swift)    │     │  MacosCoreWlanScanner             ││
│  │                       │     │       │                           ││
│  │  CWWiFiClient         │JSON │       ▼                           ││
│  │  scanForNetworks()  ──┼────►│  Vec<BssidObservation>            ││
│  │  interface()          │     │       │                           ││
│  │                       │     │       ▼                           ││
│  │  Outputs:             │     │  BssidRegistry                   ││
│  │  - ssid               │     │       │                           ││
│  │  - rssi (dBm)         │     │       ▼                           ││
│  │  - noise (dBm)        │     │  WindowsWifiPipeline (reused)    ││
│  │  - channel            │     │  [8-stage signal intelligence]   ││
│  │  - band (2.4/5/6)     │     │       │                           ││
│  │  - phy_mode           │     │       ▼                           ││
│  │  - bssid (if avail)   │     │  SensingUpdate → REST/WS         ││
│  └──────────────────────┘     └───────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────┘
```

### 3.2 Swift Helper Binary

**File:** `rust-port/wifi-densepose-rs/tools/macos-wifi-scan/main.swift`

```swift
// Modes:
//   (no args)    → Full scan, output JSON array to stdout
//   --probe      → Quick availability check, output {"available": true/false}
//   --connected  → Connected network info only
//
// Output schema (scan mode):
// [
//   {
//     "ssid": "MyNetwork",
//     "rssi": -52,
//     "noise": -90,
//     "channel": 36,
//     "band": "5GHz",
//     "phy_mode": "802.11ax",
//     "bssid": "aa:bb:cc:dd:ee:ff" | null,
//     "security": "wpa2_personal"
//   }
// ]
```

**Build:**

```bash
# Requires Xcode Command Line Tools (xcode-select --install)
cd tools/macos-wifi-scan
swiftc -framework CoreWLAN -framework Foundation -O -o macos-wifi-scan main.swift
```

**Build script:** `tools/macos-wifi-scan/build.sh`

### 3.3 Rust Adapter

**File:** `crates/wifi-densepose-wifiscan/src/adapter/macos_scanner.rs`

```rust
// #[cfg(target_os = "macos")]

pub struct MacosCoreWlanScanner {
    helper_path: PathBuf,  // Resolved at construction: $PATH or sibling of server binary
}

impl MacosCoreWlanScanner {
    pub fn new() -> Result<Self, WifiScanError>  // Finds helper or errors
    pub fn probe() -> bool                        // Runs --probe, returns availability
    pub fn scan_sync(&self) -> Result<Vec<BssidObservation>, WifiScanError>
    pub fn connected_sync(&self) -> Result<Option<BssidObservation>, WifiScanError>
}
```

**Key mappings:**

| CoreWLAN field | → | BssidObservation field | Transform |
|----------------|---|----------------------|-----------|
| `rssi` (dBm) | → | `signal_dbm` | Direct (CoreWLAN gives calibrated dBm) |
| `rssi` (dBm) | → | `amplitude` | `rssi_to_amplitude()` (existing) |
| `noise` (dBm) | → | `snr` | `rssi - noise` (new field, macOS advantage) |
| `channel` | → | `channel` | Direct |
| `band` | → | `band` | `BandType::from_channel()` (existing) |
| `phy_mode` | → | `radio_type` | Map string → `RadioType` enum |
| `bssid` | → | `bssid_id` | Direct if available, else `sha256(ssid:channel)[:12]` |
| `ssid` | → | `ssid` | Direct |

### 3.4 Sensing Server Integration

**File:** `crates/wifi-densepose-sensing-server/src/main.rs`

| Function | Purpose |
|----------|---------|
| `probe_macos_wifi()` | Calls `MacosCoreWlanScanner::probe()`, returns bool |
| `macos_wifi_task()` | Async loop: scan → build `BssidObservation` vec → feed into `BssidRegistry` + `WindowsWifiPipeline` → emit `SensingUpdate`. Same structure as `windows_wifi_task()`. |

**Auto-detection order (updated):**

```
1. ESP32 UDP probe (port 5005)     → --source esp32
2. Windows netsh probe             → --source wifi (Windows)
3. macOS CoreWLAN probe  [NEW]     → --source wifi (macOS)
4. Simulated fallback              → --source simulated
```

### 3.5 Pipeline Reuse

The existing 8-stage `WindowsWifiPipeline` (ADR-022) operates entirely on `BssidObservation` / `MultiApFrame` types:

| Stage | Reusable? | Notes |
|-------|-----------|-------|
| 1. Predictive Gating | Yes | Filters static APs by temporal variance |
| 2. Attention Weighting | Yes | Weights APs by motion sensitivity |
| 3. Spatial Correlation | Yes | Cross-AP signal correlation |
| 4. Motion Estimation | Yes | RSSI variance → motion level |
| 5. Breathing Extraction | **Marginal** | 0.3 Hz scan rate is below Nyquist for breathing (0.1-0.5 Hz). May detect very slow breathing only. |
| 6. Quality Gating | Yes | Rejects low-confidence estimates |
| 7. Fingerprint Matching | Yes | Location/posture classification |
| 8. Orchestration | Yes | Fuses all stages |

**Limitation:** CoreWLAN scan rate (~0.3-0.5 Hz) is significantly slower than `netsh` (~2 Hz). Breathing extraction (stage 5) will have reduced accuracy. Motion and presence detection remain effective since they depend on variance over longer windows.

---

## 4. Files

### 4.1 New Files

| File | Purpose | Lines (est.) |
|------|---------|-------------|
| `tools/macos-wifi-scan/main.swift` | CoreWLAN scanner, JSON output | ~120 |
| `tools/macos-wifi-scan/build.sh` | Build script (`swiftc` invocation) | ~15 |
| `crates/wifi-densepose-wifiscan/src/adapter/macos_scanner.rs` | Rust adapter: spawn helper, parse JSON, produce `BssidObservation` | ~200 |

### 4.2 Modified Files

| File | Change |
|------|--------|
| `crates/wifi-densepose-wifiscan/src/adapter/mod.rs` | Add `#[cfg(target_os = "macos")] pub mod macos_scanner;` + re-export |
| `crates/wifi-densepose-wifiscan/src/lib.rs` | Add `MacosCoreWlanScanner` re-export |
| `crates/wifi-densepose-sensing-server/src/main.rs` | Add `probe_macos_wifi()`, `macos_wifi_task()`, update auto-detect + `--source wifi` dispatch |

### 4.3 No New Rust Dependencies

- `std::process::Command` — subprocess spawning (stdlib)
- `serde_json` — JSON parsing (already in workspace)
- No changes to `Cargo.toml`

---

## 5. Verification Plan

All verification on Mac Mini (M2 Pro, macOS 26.3).

### 5.1 Swift Helper

| Test | Command | Expected |
|------|---------|----------|
| Build | `cd tools/macos-wifi-scan && ./build.sh` | Produces `macos-wifi-scan` binary |
| Probe | `./macos-wifi-scan --probe` | `{"available": true}` |
| Scan | `./macos-wifi-scan` | JSON array with real SSIDs, RSSI in dBm, channels |
| Connected | `./macos-wifi-scan --connected` | Single JSON object for connected network |
| No WiFi | Disable WiFi → `./macos-wifi-scan` | `{"available": false}` or empty array |

### 5.2 Rust Adapter

| Test | Method | Expected |
|------|--------|----------|
| Unit: JSON parsing | `#[test]` with fixture JSON | Correct `BssidObservation` values |
| Unit: synthetic BSSID | `#[test]` with nil bssid input | Stable `sha256(ssid:channel)[:12]` |
| Unit: helper not found | `#[test]` with bad path | `WifiScanError::ProcessError` |
| Integration: real scan | `cargo test` on Mac Mini | Live observations from CoreWLAN |

### 5.3 End-to-End

| Step | Command | Verify |
|------|---------|--------|
| 1 | `cargo build --release` (Mac Mini) | Clean build, no warnings |
| 2 | `cargo test --workspace` | All existing tests pass + new macOS tests |
| 3 | `./target/release/sensing-server --source wifi` | Server starts, logs `source: wifi (macOS CoreWLAN)` |
| 4 | `curl http://localhost:8080/api/v1/sensing/latest` | `source: "wifi:<SSID>"`, real RSSI values |
| 5 | `curl http://localhost:8080/api/v1/vital-signs` | Motion detection responds to physical movement |
| 6 | Open UI at `http://localhost:8080` | Signal field updates with real RSSI variation |
| 7 | `--source auto` | Auto-detects macOS WiFi, does not fall back to simulated |

### 5.4 Cross-Platform Regression

| Platform | Build | Expected |
|----------|-------|----------|
| macOS (Mac Mini) | `cargo build --release` | macOS adapter compiled, works |
| Windows | `cargo build --release` | macOS adapter skipped (`#[cfg]`), Windows path unchanged |
| Linux | `cargo build --release` | macOS adapter skipped, ESP32/simulated paths unchanged |

---

## 6. Limitations

| Limitation | Impact | Mitigation |
|------------|--------|-----------|
| **BSSID redaction** | Same-SSID same-channel APs collapse to one observation | Use `sha256(ssid:channel)` as pseudo-BSSID; document edge case. Rare in practice (mesh networks). |
| **Slow scan rate** (~0.3 Hz) | Breathing extraction unreliable (below Nyquist) | Motion/presence still work. Breathing marked low-confidence. Future: cache + connected AP fast-poll hybrid. |
| **Requires Swift helper in PATH** | Extra build step for source builds | `build.sh` provided. Docker image pre-bundles it. Clear error message when missing. |
| **Location Services for BSSID** | Full BSSID requires user permission prompt | System degrades gracefully to SSID:channel pseudo-BSSID without permission. |
| **No CSI** | Cannot match ESP32 pose estimation accuracy | Expected — this is RSSI-tier sensing (presence + motion). Same limitation as Windows. |

---

## 7. Future Work

| Enhancement | Description | Depends On |
|-------------|-------------|-----------|
| **Fast-poll connected AP** | Poll connected AP's RSSI at ~10 Hz via `CWInterface.rssiValue()` (no full scan needed) | CoreWLAN `rssiValue()` performance testing |
| **Linux `iw` adapter** | Same subprocess pattern with `iw dev wlan0 scan` output | Linux machine for testing |
| **Unified `RssiPipeline` rename** | Rename `WindowsWifiPipeline` → `RssiPipeline` to reflect multi-platform use | ADR-022 update |
| **802.11bf sensing** | Apple may expose CSI via 802.11bf in future macOS | Apple framework availability |
| **Docker macOS image** | Pre-built macOS Docker image with Swift helper bundled | Docker multi-arch build |

---

## 8. References

- [Apple CoreWLAN Documentation](https://developer.apple.com/documentation/corewlan)
- [CWWiFiClient](https://developer.apple.com/documentation/corewlan/cwwificlient) — Primary WiFi interface API
- [CWNetwork](https://developer.apple.com/documentation/corewlan/cwnetwork) — Scan result type (SSID, RSSI, channel, noise)
- [macOS 15 airport removal](https://developer.apple.com/forums/thread/732431) — Apple Developer Forums
- ADR-022: Windows WiFi Enhanced Fidelity (analogous platform adapter)
- ADR-013: Feature-Level Sensing from Commodity Gear
- Issue [#56](https://github.com/ruvnet/wifi-densepose/issues/56): macOS support request
