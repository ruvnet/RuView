# WiFi-DensePose Mobile

**See through walls from your phone.** Real-time WiFi sensing, vital signs, and disaster response — in a cross-platform mobile app.

WiFi-DensePose Mobile is a React Native / Expo companion app for the [WiFi-DensePose](../../README.md) sensing platform. It connects to a WiFi sensing server over WebSocket, renders live 3D Gaussian splat visualizations of detected humans, displays breathing and heart rate in real time, and provides a full WiFi-MAT disaster triage dashboard — all from a single codebase that runs on iOS, Android, and Web.

> | Screen | What It Shows |
> |--------|---------------|
> | **Live** | 3D Gaussian splat body rendering with FPS counter, signal strength, confidence HUD |
> | **Vitals** | Breathing rate (6-30 BPM) and heart rate (40-120 BPM) arc gauges with sparkline history |
> | **Zones** | SVG floor plan with occupancy grid, zone legend, presence heatmap |
> | **MAT** | Mass casualty assessment: survivor counter, triage alerts, zone management |
> | **Settings** | Server URL, theme picker, RSSI-only toggle, alert sound control |

```bash
# Quick start — web preview in 30 seconds
cd ui/mobile
npm install
npx expo start --web
```

<!-- Screenshot placeholder: replace with actual app screenshots -->
<!-- ![WiFi-DensePose Mobile](assets/screenshots/app-overview.png) -->

---

## Features

| | Feature | Details |
|---|---------|---------|
| **3D Live View** | Gaussian splat rendering | Three.js via WebView (native) or iframe (web), real-time pose overlay |
| **Vital Signs** | Breathing + heart rate | Arc gauge components with sparkline 60-sample history, confidence indicators |
| **Disaster Response** | WiFi-MAT dashboard | Survivor detection, START triage classification, priority alerts, zone scan tracking |
| **Floor Plan** | SVG occupancy grid | Zone-level presence visualization, color-coded density, interactive legend |
| **Cross-Platform** | iOS, Android, Web | Expo SDK 55, React Native 0.83, single codebase with platform-specific modules |
| **Offline Capable** | Automatic simulation fallback | When the sensing server is unreachable, generates synthetic data so the UI stays functional |
| **RSSI Mode** | No CSI hardware needed | Toggle RSSI-only scanning for coarse presence detection on consumer WiFi devices |
| **Dark Theme** | Cyan accent (#32B8C6) | Dark-first design system with consistent color tokens, spacing scale, and monospace typography |
| **Persistent State** | Zustand + AsyncStorage | Settings, connection preferences, and theme survive app restarts |
| **Platform WiFi** | Native RSSI scanning | Android: `react-native-wifi-reborn`, iOS: stub (requires entitlement), Web: synthetic values |

---

## Prerequisites

| Requirement | Version | Notes |
|-------------|---------|-------|
| Node.js | 18+ | LTS recommended |
| npm | 9+ | Ships with Node.js 18+ |
| Expo CLI | Latest | Installed automatically via `npx` |
| iOS Simulator | Xcode 15+ | macOS only; optional for iOS development |
| Android Emulator | API 33+ | Android Studio; optional for Android development |
| WiFi-DensePose Server | Any | Optional — app falls back to simulated data without a server |

---

## Quick Start

### Web (fastest)

```bash
cd ui/mobile
npm install
npx expo start --web
```

Open `http://localhost:8081` in your browser. The app starts in simulation mode with synthetic pose and vital sign data.

### Android

```bash
cd ui/mobile
npm install
npx expo start --android
```

Requires Android Studio with an emulator running, or a physical device with Expo Go installed.

### iOS

```bash
cd ui/mobile
npm install
npx expo start --ios
```

Requires Xcode with a simulator, or a physical device with Expo Go. RSSI scanning on iOS requires the `com.apple.developer.networking.wifi-info` entitlement.

---

## Connecting to a Sensing Server

The app connects to the WiFi-DensePose sensing server over WebSocket for live data. Configure the server URL in the **Settings** tab.

| Server Location | URL | Notes |
|----------------|-----|-------|
| Local dev server | `http://localhost:3000` | Default; sensing WS auto-connects on port 3001 |
| Docker container | `http://host.docker.internal:3000` | From emulator connecting to host Docker |
| ESP32 mesh | `http://<esp32-ip>:3000` | Direct connection to ESP32 aggregator |
| Remote server | `https://your-server.example.com` | TLS supported; WebSocket upgrades to `wss://` |

When the server is unreachable, the app automatically falls back to **simulation mode** after exhausting reconnect attempts (exponential backoff). A yellow `SIM` badge appears in the connection banner. Reconnection resumes automatically when the server becomes available.

---

<details>
<summary><strong>Architecture</strong></summary>

### Directory Structure

```
ui/mobile/
  App.tsx                          Root component (providers, navigation, services)
  app.config.ts                    Expo configuration
  index.ts                         Entry point
  src/
    components/
      ConnectionBanner.tsx         Server status banner (connected/simulated/disconnected)
      ErrorBoundary.tsx            Crash boundary with fallback UI
      GaugeArc.tsx                 SVG arc gauge for vital sign display
      HudOverlay.tsx               Heads-up display overlay
      LoadingSpinner.tsx           Themed loading indicator
      ModeBadge.tsx                LIVE / SIM / RSSI mode indicator
      OccupancyGrid.tsx            Grid-based occupancy visualization
      SignalBar.tsx                RSSI signal strength bars
      SparklineChart.tsx           Mini sparkline for metric history
      StatusDot.tsx                Connection status indicator dot
      ThemedText.tsx               Text component with theme presets
      ThemedView.tsx               View component with theme background
    constants/
      api.ts                       REST API path constants
      simulation.ts                Simulation tick interval, defaults
      websocket.ts                 WS path, reconnect delays, max attempts
    hooks/
      usePoseStream.ts             Subscribe to live or simulated sensing frames
      useRssiScanner.ts            Platform RSSI scanning hook
      useServerReachability.ts     HTTP health check polling
      useTheme.ts                  Dark/light/system theme resolution
      useWebViewBridge.ts          WebView message bridge for Gaussian viewer
    navigation/
      MainTabs.tsx                 Bottom tab navigator (5 tabs with lazy loading)
      RootNavigator.tsx            Root stack navigator
      types.ts                     Navigation param list types
    screens/
      LiveScreen/
        index.tsx                  3D Gaussian splat view with HUD overlay
        GaussianSplatWebView.tsx   Native WebView renderer (Three.js)
        GaussianSplatWebView.web.tsx  Web iframe renderer
        LiveHUD.tsx                FPS, RSSI, confidence, person count overlay
        useGaussianBridge.ts       WebView message protocol
      VitalsScreen/
        index.tsx                  Breathing + heart rate dashboard
        BreathingGauge.tsx         Arc gauge for breathing BPM
        HeartRateGauge.tsx         Arc gauge for heart rate BPM
        MetricCard.tsx             Vital sign metric card with sparkline
      ZonesScreen/
        index.tsx                  Floor plan occupancy view
        FloorPlanSvg.tsx           SVG floor plan renderer
        useOccupancyGrid.ts        Grid computation from sensing frames
        ZoneLegend.tsx             Color-coded zone legend
      MATScreen/
        index.tsx                  Mass casualty assessment dashboard
        AlertCard.tsx              Single triage alert card
        AlertList.tsx              Scrollable alert list with priority sorting
        MatWebView.tsx             MAT visualization WebView
        SurvivorCounter.tsx        Survivor count by triage status
        useMatBridge.ts            MAT WebView message protocol
      SettingsScreen/
        index.tsx                  App settings panel
        ServerUrlInput.tsx         Server URL text input with validation
        RssiToggle.tsx             RSSI-only mode switch
        ThemePicker.tsx            Dark / light / system theme selector
    services/
      ws.service.ts               WebSocket client with auto-reconnect + simulation fallback
      api.service.ts              REST client (Axios) with retry logic
      rssi.service.ts             Platform-agnostic RSSI scanner interface
      rssi.service.android.ts     Android: react-native-wifi-reborn integration
      rssi.service.ios.ts         iOS: stub (requires entitlement)
      rssi.service.web.ts         Web: synthetic RSSI values
      simulation.service.ts       Generates synthetic SensingFrame data
    stores/
      poseStore.ts                Pose frames, connection status, frame history (Zustand)
      matStore.ts                 MAT survivors, zones, alerts, disaster events (Zustand)
      settingsStore.ts            Server URL, theme, RSSI toggle (Zustand + persist)
    theme/
      colors.ts                   Color tokens (bg, surface, accent, danger, etc.)
      spacing.ts                  4px-based spacing scale
      typography.ts               Font families and size presets
      ThemeContext.tsx             React context provider for theme
      index.ts                    Theme barrel export
    types/
      sensing.ts                  SensingFrame, SensingNode, VitalsData, Classification
      mat.ts                      Survivor, Alert, ScanZone, TriageStatus, DisasterType
      api.ts                      PoseStatus, ZoneConfig, HistoricalFrames, ApiError
      navigation.ts               Navigation param lists
    utils/
      colorMap.ts                 Value-to-color mapping for heatmaps
      formatters.ts               Number and date formatting utilities
      ringBuffer.ts               Fixed-size circular buffer for frame history
      urlValidator.ts             Server URL validation
  e2e/                            Maestro end-to-end test specs
  assets/                         App icons and images
```

### Data Flow

```
WiFi Sensing Server (Rust/Axum)
       |
       | WebSocket (ws://host:3001/ws/sensing)
       v
  ws.service.ts -----> [auto-reconnect with exponential backoff]
       |                       |
       | SensingFrame          | (server unreachable)
       v                       v
  poseStore.ts          simulation.service.ts
       |                       |
       | Zustand state         | synthetic SensingFrame
       v                       v
  usePoseStream.ts  <----------+
       |
       +---> LiveScreen (3D Gaussian splat + HUD)
       +---> VitalsScreen (breathing + heart rate gauges)
       +---> ZonesScreen (floor plan occupancy grid)

  api.service.ts -----> REST API (GET /api/pose/status, /zones, /frames)
       |
       v
  matStore.ts -----> MATScreen (survivor counter, alerts, zones)

  rssi.service.ts -----> Platform WiFi scan (Android / iOS / Web)
       |
       v
  useRssiScanner.ts -----> LiveScreen HUD (signal bars)
```

</details>

---

<details>
<summary><strong>Screens</strong></summary>

### Live

The primary visualization screen. Renders a 3D Gaussian splat representation of detected humans using Three.js. On native platforms, the renderer runs inside a WebView; on web, it uses an iframe. A heads-up display overlays connection status, FPS, RSSI signal strength, detection confidence, and person count. Supports three modes: **LIVE** (connected to server), **SIM** (simulation fallback), and **RSSI** (RSSI-only scanning).

### Vitals

Displays real-time breathing rate and heart rate extracted from CSI signal processing. Each vital sign is shown as an animated arc gauge (`GaugeArc` component) with the current BPM value, a 60-sample sparkline history (`SparklineChart`), and a confidence percentage. Normal ranges: breathing 6-30 BPM, heart rate 40-120 BPM.

### Zones

A floor plan view that maps WiFi sensing coverage to physical space. Uses SVG rendering (`react-native-svg`) to draw zones with color-coded occupancy density. The `useOccupancyGrid` hook computes grid cell values from incoming sensing frames. A legend shows the color scale from empty to high-density zones.

### MAT

Mass Casualty Assessment Tool for disaster response. Displays a survivor counter grouped by START triage classification (Immediate / Delayed / Minor / Deceased), a scrollable alert list sorted by priority, and zone scan progress. Each alert card shows the survivor location, recommended action, and triage color. The MAT tab badge shows the active alert count.

### Settings

Configuration panel with four controls:
- **Server URL** — text input with URL validation; changes trigger WebSocket reconnect
- **Theme** — dark / light / system picker
- **RSSI Scanning** — toggle for platform-native WiFi RSSI scanning
- **Alert Sound** — toggle for MAT alert audio notifications

All settings persist across app restarts via Zustand with AsyncStorage.

</details>

---

<details>
<summary><strong>API Integration</strong></summary>

### WebSocket Protocol

The app connects to the sensing server's WebSocket endpoint for real-time data streaming.

**Endpoint:** `ws://<host>:3001/ws/sensing`

**Frame format** (`SensingFrame`):

```typescript
interface SensingFrame {
  type?: string;
  timestamp?: number;
  source?: string;           // "live" | "simulated"
  tick?: number;
  nodes: SensingNode[];      // Per-node RSSI, position, amplitude
  features: FeatureSet;      // mean_rssi, variance, motion_band_power, etc.
  classification: Classification; // motion_level, presence, confidence
  signal_field: SignalField;  // 3D voxel grid values
  vital_signs?: VitalsData;  // breathing_bpm, hr_proxy_bpm, confidence
}
```

The WebSocket service (`ws.service.ts`) handles:
- Automatic reconnection with exponential backoff (1s, 2s, 4s, 8s, 16s)
- Fallback to simulation after max reconnect attempts
- Protocol upgrade (`http:` to `ws:`, `https:` to `wss:`)
- Port mapping (HTTP 3000 maps to WS 3001)

### REST API

The REST client (`api.service.ts`) provides:

| Method | Path | Returns |
|--------|------|---------|
| `GET` | `/api/pose/status` | `PoseStatus` — server health and capabilities |
| `GET` | `/api/pose/zones` | `ZoneConfig[]` — configured sensing zones |
| `GET` | `/api/pose/frames?limit=N` | `HistoricalFrames` — recent frame history |

All requests use Axios with a 5-second timeout and automatic retry (2 attempts).

</details>

---

## Testing

### Unit Tests

```bash
cd ui/mobile
npm test
```

Runs the Jest test suite via `jest-expo`. Tests cover:

| Category | Files | What Is Tested |
|----------|-------|----------------|
| Components | 7 | `ConnectionBanner`, `GaugeArc`, `HudOverlay`, `OccupancyGrid`, `SignalBar`, `SparklineChart`, `StatusDot` |
| Screens | 5 | `LiveScreen`, `VitalsScreen`, `ZonesScreen`, `MATScreen`, `SettingsScreen` |
| Services | 4 | `ws.service`, `api.service`, `rssi.service`, `simulation.service` |
| Stores | 3 | `poseStore`, `matStore`, `settingsStore` |
| Hooks | 3 | `usePoseStream`, `useRssiScanner`, `useServerReachability` |
| Utils | 3 | `colorMap`, `ringBuffer`, `urlValidator` |

### End-to-End Tests (Maestro)

```bash
# Install Maestro CLI
curl -Ls https://get.maestro.mobile.dev | bash

# Run all e2e specs
maestro test e2e/
```

Maestro YAML specs cover each screen:

| Spec | What It Verifies |
|------|-----------------|
| `live_screen.yaml` | 3D viewer loads, HUD elements visible, mode badge displays |
| `vitals_screen.yaml` | Breathing and heart rate gauges render with values |
| `zones_screen.yaml` | Floor plan SVG renders, zone legend visible |
| `mat_screen.yaml` | Survivor counter displays, alert list populates |
| `settings_screen.yaml` | URL input editable, theme picker works, toggles respond |
| `offline_fallback.yaml` | App transitions to SIM mode when server unreachable |

---

## Tech Stack

| Layer | Technology | Version |
|-------|-----------|---------|
| Framework | Expo | 55 |
| UI | React Native | 0.83 |
| Language | TypeScript | 5.9 |
| Navigation | React Navigation | 7.x |
| State | Zustand | 5.x |
| HTTP | Axios | 1.x |
| SVG | react-native-svg | 15.x |
| WebView | react-native-webview | 13.x |
| WiFi | react-native-wifi-reborn | 4.x |
| Charts | Victory Native | 41.x |
| Animations | react-native-reanimated | 4.x |
| Testing | Jest + jest-expo | 30.x |
| E2E | Maestro | Latest |

---

## Contributing

1. Fork the repository
2. Create a feature branch from `main`
3. Make changes in the `ui/mobile/` directory
4. Run `npm test` and verify all tests pass
5. Run `npx expo start --web` to verify the app renders correctly
6. Submit a pull request

Follow the project's existing patterns:
- Components go in `src/components/`
- Screen-specific components go in `src/screens/<ScreenName>/`
- Platform-specific files use the `.android.ts` / `.ios.ts` / `.web.ts` suffix convention
- All state management uses Zustand stores in `src/stores/`
- All types go in `src/types/`

---

## Credits

Mobile app by [@MaTriXy](https://github.com/MaTriXy) — original scaffold, screen architecture, and cross-platform service layer.

Built on the [WiFi-DensePose](../../README.md) sensing platform.

---

## License

[MIT](../../LICENSE)
