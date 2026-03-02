# ADR-034: Expo React Native Mobile Application

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2026-03-02 |
| **Deciders** | MaTriXy, rUv |
| **Codename** | **FieldView** -- Mobile Companion for WiFi-DensePose Field Deployment |
| **Relates to** | ADR-019 (Sensing-Only UI Mode), ADR-021 (Vital Sign Detection), ADR-026 (Survivor Track Lifecycle), ADR-029 (RuvSense Multistatic), ADR-031 (RuView Sensing-First RF), ADR-032 (Mesh Security) |

---

## 1. Context

### 1.1 Need for a Mobile Companion

WiFi-DensePose is a WiFi-based human pose estimation system using Channel State Information (CSI) from ESP32 mesh nodes. The existing web UI (`ui/`) serves desktop browsers but is not optimized for mobile form factors. Three deployment scenarios demand a purpose-built mobile application:

1. **Disaster response (WiFi-MAT)**: First responders deploying ESP32 mesh nodes in collapsed structures need a portable device to visualize survivor detections, breathing/heart rate vitals, and zone maps in real time. A laptop is impractical in rubble fields.
2. **Building security**: Security operators patrolling a facility need a handheld display showing occupancy by zone, movement alerts, and historical patterns. The phone in their pocket is the natural form factor.
3. **Healthcare monitoring**: Clinical staff monitoring patients via CSI-based contactless vitals need a tablet view at the bedside or nurse station, with gauges for breathing rate and heart rate that update in real time.

In all three scenarios, the mobile device does not communicate with ESP32 nodes directly. Instead, a Rust sensing server (`wifi-densepose-sensing-server`, ADR-031) aggregates ESP32 UDP streams and exposes a WebSocket API. The mobile app connects to this server over local WiFi.

### 1.2 Technology Selection Rationale

| Requirement | Decision | Rationale |
|-------------|----------|-----------|
| Cross-platform (iOS + Android + Web) | Expo SDK 55 + React Native 0.83 | Single codebase, managed workflow, OTA updates |
| Real-time streaming | WebSocket (ws://host:3001/ws/sensing) | Sub-100ms latency from CSI capture to mobile display |
| 3D visualization | Three.js Gaussian splat via WebView | Reuses existing `ui/` Three.js splat renderer; avoids native OpenGL binding |
| State management | Zustand | Minimal boilerplate, React-concurrent safe, selector-based re-renders |
| Persistence | AsyncStorage | Built into Expo, sufficient for settings and small cached state |
| Navigation | react-navigation v7 (bottom tabs) | Standard React Native navigation; 5-tab layout fits mobile ergonomics |
| WiFi RSSI scanning | Platform-specific (Android: react-native-wifi-reborn, iOS: CoreWLAN stub, Web: synthetic) | No cross-platform WiFi scanning API exists; platform modules are required |
| E2E testing | Maestro YAML specs | Declarative, no Detox native build dependency, runs on CI |
| Design system | Dark theme (#0D1117 bg, #32B8C6 accent) | Matches existing `ui/` sensing dashboard aesthetic; reduces eye strain in field conditions |

### 1.3 Relationship to Existing UI

The desktop web UI (`ui/`) and the mobile app share no code at the component level, but they consume the same backend APIs:

- **WebSocket**: `ws://host:3001/ws/sensing` -- streaming SensingFrame JSON
- **REST**: `http://host:3000/api/v1/...` -- configuration, history, health

The mobile app's Three.js Gaussian splat viewer (LiveScreen) loads the same splat HTML bundle used by the desktop UI, rendered inside a WebView (native) or iframe (web).

---

## 2. Decision

Build an Expo React Native mobile application at `ui/mobile/` that provides five primary screens for field operators, connected to the Rust sensing server via WebSocket streaming. The app automatically falls back to simulated data when the sensing server is unreachable, enabling demos and offline testing.

### 2.1 Screen Architecture

```
+---------------------------------------------------------------+
|                    MainTabs (Bottom Tab Navigator)             |
+---------------------------------------------------------------+
|                                                                 |
|  +----------+  +----------+  +----------+  +--------+  +-----+ |
|  |   Live   |  |  Vitals  |  |  Zones   |  |  MAT   |  | Cog | |
|  | (3D splat|  |(breathing|  |(floor    |  |(disaster|  |(set-| |
|  |  + HUD)  |  | + heart) |  | plan SVG)|  |response)|  |tings| |
|  +----------+  +----------+  +----------+  +--------+  +-----+ |
|                                                                 |
+---------------------------------------------------------------+
|  ConnectionBanner (Connected / Simulated / Disconnected)       |
+---------------------------------------------------------------+
```

**Screen responsibilities:**

| Screen | Primary View | Data Source | Key Components |
|--------|-------------|-------------|----------------|
| **Live** | 3D Gaussian splat with 17 COCO keypoints + HUD overlay | `poseStore.latestFrame` | `GaussianSplatWebView`, `LiveHUD`, `HudOverlay` |
| **Vitals** | Breathing BPM gauge, heart rate BPM gauge, sparkline history | `poseStore.latestFrame.vital_signs` | `BreathingGauge`, `HeartRateGauge`, `MetricCard`, `SparklineChart` |
| **Zones** | Floor plan SVG with occupancy heat overlay, zone legend | `poseStore.latestFrame.persons` | `FloorPlanSvg`, `OccupancyGrid`, `ZoneLegend` |
| **MAT** | Survivor counter, zone map WebView, alert list | `matStore.survivors`, `matStore.alerts` | `SurvivorCounter`, `MatWebView`, `AlertList`, `AlertCard` |
| **Settings** | Server URL input, theme picker, RSSI toggle | `settingsStore` | `ServerUrlInput`, `ThemePicker`, `RssiToggle` |

### 2.2 State Architecture

Three Zustand stores separate concerns and prevent unnecessary re-renders:

```
+------------------------------------------------------------+
|                     Zustand Stores                          |
+------------------------------------------------------------+
|                                                              |
|  poseStore                                                   |
|  +--------------------------------------------------------+ |
|  | connectionStatus: 'connected' | 'simulated' | 'error'  | |
|  | latestFrame: SensingFrame | null                        | |
|  | frameHistory: RingBuffer<SensingFrame>                  | |
|  | features: FeatureVector | null                          | |
|  | persons: Person[]                                       | |
|  | vitalSigns: VitalSigns | null                           | |
|  +--------------------------------------------------------+ |
|                                                              |
|  matStore                                                    |
|  +--------------------------------------------------------+ |
|  | survivors: Survivor[]                                   | |
|  | alerts: MatAlert[]                                      | |
|  | events: MatEvent[]                                      | |
|  | zoneMap: ZoneMap | null                                  | |
|  +--------------------------------------------------------+ |
|                                                              |
|  settingsStore  (persisted via AsyncStorage)                 |
|  +--------------------------------------------------------+ |
|  | serverUrl: string  (default: 'http://localhost:3000')   | |
|  | wsUrl: string      (default: 'ws://localhost:3001')     | |
|  | theme: 'dark' | 'light'                                 | |
|  | rssiEnabled: boolean                                    | |
|  | simulationMode: boolean                                 | |
|  +--------------------------------------------------------+ |
|                                                              |
+------------------------------------------------------------+
```

### 2.3 Service Layer

Four services encapsulate external communication and data generation:

| Service | File | Responsibility |
|---------|------|----------------|
| `ws.service` | `src/services/ws.service.ts` | WebSocket connection lifecycle, reconnection with exponential backoff, SensingFrame parsing, dispatches to `poseStore` |
| `api.service` | `src/services/api.service.ts` | REST calls to sensing server (health check, configuration, history endpoints) |
| `rssi.service` | `src/services/rssi.service.ts` (+ platform variants) | Platform-specific WiFi RSSI scanning. Android uses `react-native-wifi-reborn`, iOS provides a CoreWLAN stub, Web generates synthetic RSSI values |
| `simulation.service` | `src/services/simulation.service.ts` | Generates synthetic SensingFrame data when the real server is unreachable. Produces realistic amplitude, phase, vital signs, and person data on a configurable tick interval |

**Platform-specific RSSI service files:**

| File | Platform | Implementation |
|------|----------|----------------|
| `rssi.service.android.ts` | Android | `react-native-wifi-reborn` native module, requires `ACCESS_FINE_LOCATION` permission |
| `rssi.service.ios.ts` | iOS | CoreWLAN stub (returns empty scan results; Apple restricts WiFi scanning to system apps) |
| `rssi.service.web.ts` | Web | Synthetic RSSI values generated from noise model |
| `rssi.service.ts` | Default | Re-exports platform-appropriate module via React Native file resolution |

### 2.4 Data Flow

```
ESP32 Mesh Nodes
      |
      | UDP CSI frames (ADR-029 TDM protocol)
      v
+---------------------------+
| Rust Sensing Server       |
| (wifi-densepose-sensing-  |
|  server, ADR-031)         |
|                           |
| Aggregates ESP32 streams  |
| Runs RuvSense pipeline    |
| Exposes WS + REST APIs    |
+---------------------------+
      |                    |
      | WebSocket          | REST
      | ws://host:3001     | http://host:3000
      | /ws/sensing        | /api/v1/...
      v                    v
+---------------------------+
| Expo Mobile App           |
|                           |
| ws.service                |
|   -> poseStore            |
|   -> matStore             |
|                           |
| Screens subscribe to      |
| stores via Zustand        |
| selectors                 |
+---------------------------+
```

**Connection lifecycle:**

1. App boots. `settingsStore` loads persisted server URL from AsyncStorage.
2. `ws.service` opens WebSocket to `wsUrl/ws/sensing`.
3. On each message, `ws.service` parses the `SensingFrame` JSON and dispatches to `poseStore`.
4. If the WebSocket fails, `ws.service` retries with exponential backoff (1s, 2s, 4s, 8s, 16s max).
5. After `MAX_RECONNECT_ATTEMPTS` (5) consecutive failures, `ws.service` switches to `simulation.service`, which generates synthetic frames at 10 Hz.
6. `poseStore.connectionStatus` transitions: `connected` -> `error` -> `simulated`.
7. `ConnectionBanner` component reflects the current status on all screens.
8. If the server becomes reachable again, `ws.service` reconnects and resumes live data.

### 2.5 SensingFrame JSON Schema

The WebSocket stream delivers JSON frames matching the Rust `SensingFrame` struct:

```typescript
interface SensingFrame {
  timestamp: number;           // Unix epoch ms
  amplitude: number[];         // Per-subcarrier amplitude (52 or 114 values)
  phase: number[];             // Per-subcarrier phase (radians)
  features: {
    mean_amplitude: number;
    std_amplitude: number;
    phase_slope: number;
    doppler_shift: number;
    delay_spread: number;
  };
  classification: string;      // "empty" | "single_person" | "multi_person" | "motion"
  confidence: number;          // 0.0 - 1.0
  persons: Array<{
    id: number;
    keypoints: Array<[number, number, number]>;  // 17 COCO keypoints [x, y, confidence]
    bbox: [number, number, number, number];       // [x, y, width, height]
    track_id: number;
  }>;
  vital_signs?: {
    breathing_rate_bpm: number;
    heart_rate_bpm: number;
    breathing_confidence: number;
    heart_confidence: number;
  };
  rssi?: number;
  node_id?: number;
}
```

### 2.6 Three.js Gaussian Splat Rendering

The LiveScreen uses a WebView (native) or iframe (web) to render a Three.js Gaussian splat scene. This avoids native OpenGL bindings while reusing the existing splat renderer from the desktop UI.

**Native path (iOS/Android):**
- `GaussianSplatWebView.tsx` renders a `<WebView>` loading a bundled HTML page.
- The HTML page initializes a Three.js scene with Gaussian splat shaders.
- Communication between React Native and the WebView uses `postMessage` / `onMessage` bridge.
- `useGaussianBridge.ts` hook manages the bridge, sending skeleton keypoint updates as JSON.

**Web path:**
- `GaussianSplatWebView.web.tsx` (platform-specific file) renders an `<iframe>` with the same HTML bundle.
- Communication uses `window.postMessage` with origin checks.

### 2.7 Design System

| Token | Value | Usage |
|-------|-------|-------|
| `colors.background` | `#0D1117` | Primary background (dark theme) |
| `colors.surface` | `#161B22` | Card/panel backgrounds |
| `colors.border` | `#30363D` | Borders, dividers |
| `colors.accent` | `#32B8C6` | Primary accent, active tab, gauge fill |
| `colors.danger` | `#F85149` | Alerts, errors, critical vitals |
| `colors.warning` | `#D29922` | Warnings, degraded state |
| `colors.success` | `#3FB950` | Connected status, normal vitals |
| `colors.text` | `#E6EDF3` | Primary text |
| `colors.textSecondary` | `#8B949E` | Secondary/muted text |
| `typography.mono` | `Courier New` | Monospace for data values, HUD |
| `spacing.xs` | `4` | Tight spacing |
| `spacing.sm` | `8` | Small spacing |
| `spacing.md` | `16` | Medium spacing |
| `spacing.lg` | `24` | Large spacing |
| `spacing.xl` | `32` | Extra-large spacing |

The dark theme is the default and primary design target, optimized for field conditions (low ambient light, glare reduction). A light theme variant is available via the Settings screen.

### 2.8 ESP32 Integration Model

The mobile app does not communicate with ESP32 nodes directly. The architecture is:

```
ESP32 Node A ---\
ESP32 Node B ----+---> Sensing Server (Raspberry Pi / Laptop) <---> Mobile App
ESP32 Node C ---/         (local WiFi)                            (local WiFi)
```

- **Field deployment**: The sensing server runs on a Raspberry Pi 4 or operator laptop. All devices (ESP32 nodes, server, mobile app) connect to the same local WiFi network or a portable router.
- **Server URL**: Configurable in Settings screen. Default: `http://localhost:3000` (server) and `ws://localhost:3001/ws/sensing` (WebSocket). In field use, the operator sets this to the server's LAN IP (e.g., `http://192.168.1.100:3000`).
- **No BLE/direct connection**: ESP32 nodes use UDP broadcast for CSI frames (ADR-029). The mobile app has no UDP listener; it consumes the server's processed output.

---

## 3. Directory Structure

```
ui/mobile/
|-- App.tsx                              # Root component, ThemeProvider + NavigationContainer
|-- app.config.ts                        # Expo config (SDK 55, app name, icons, splash)
|-- app.json                             # Expo static config
|-- babel.config.js                      # Babel config (expo-router preset)
|-- eas.json                             # EAS Build profiles (dev, preview, production)
|-- index.ts                             # Entry point (registerRootComponent)
|-- jest.config.js                       # Jest config for unit tests
|-- jest.setup.ts                        # Jest setup (mock AsyncStorage, react-native modules)
|-- metro.config.js                      # Metro bundler config
|-- package.json                         # Dependencies and scripts
|-- tsconfig.json                        # TypeScript config (strict mode)
|
|-- assets/
|   |-- android-icon-background.png      # Android adaptive icon background
|   |-- android-icon-foreground.png      # Android adaptive icon foreground
|   |-- android-icon-monochrome.png      # Android monochrome icon
|   |-- favicon.png                      # Web favicon
|   |-- icon.png                         # App icon (1024x1024)
|   |-- splash-icon.png                  # Splash screen icon
|
|-- e2e/                                 # Maestro E2E test specs
|   |-- live_screen.yaml                 # LiveScreen: splat renders, HUD shows data
|   |-- vitals_screen.yaml              # VitalsScreen: gauges animate, sparklines update
|   |-- zones_screen.yaml              # ZonesScreen: floor plan renders, legend visible
|   |-- mat_screen.yaml                 # MATScreen: survivor count, alerts list
|   |-- settings_screen.yaml            # SettingsScreen: URL input, theme toggle
|   |-- offline_fallback.yaml           # Simulated mode activates on server disconnect
|
|-- src/
|   |-- components/                      # Shared UI components (12 components)
|   |   |-- ConnectionBanner.tsx         # Status banner: Connected/Simulated/Disconnected
|   |   |-- ErrorBoundary.tsx            # React error boundary with fallback UI
|   |   |-- GaugeArc.tsx                 # SVG arc gauge (used by vitals)
|   |   |-- HudOverlay.tsx              # Translucent HUD overlay for LiveScreen
|   |   |-- LoadingSpinner.tsx           # Animated loading indicator
|   |   |-- ModeBadge.tsx               # Badge showing current mode (Live/Sim)
|   |   |-- OccupancyGrid.tsx           # Grid overlay for zone occupancy
|   |   |-- SignalBar.tsx               # WiFi signal strength bar
|   |   |-- SparklineChart.tsx          # Inline sparkline chart (SVG)
|   |   |-- StatusDot.tsx              # Colored status dot indicator
|   |   |-- ThemedText.tsx             # Text component with theme support
|   |   |-- ThemedView.tsx             # View component with theme support
|   |
|   |-- constants/                       # App-wide constants
|   |   |-- api.ts                       # REST API endpoint paths, timeouts
|   |   |-- simulation.ts               # Simulation tick rate, data ranges
|   |   |-- websocket.ts                # WS reconnect config, max attempts
|   |
|   |-- hooks/                           # Custom React hooks (5 hooks)
|   |   |-- usePoseStream.ts            # Subscribe to poseStore, manage WS lifecycle
|   |   |-- useRssiScanner.ts           # Platform RSSI scanning with permission handling
|   |   |-- useServerReachability.ts    # Periodic health check, reachability state
|   |   |-- useTheme.ts                # Theme context consumer
|   |   |-- useWebViewBridge.ts         # WebView <-> RN message bridge
|   |
|   |-- navigation/                      # React Navigation setup
|   |   |-- MainTabs.tsx                # Bottom tab navigator (5 tabs)
|   |   |-- RootNavigator.tsx           # Root stack (splash -> MainTabs)
|   |   |-- types.ts                    # Navigation type definitions
|   |
|   |-- screens/                         # Screen modules (5 screens)
|   |   |-- LiveScreen/
|   |   |   |-- index.tsx               # LiveScreen container
|   |   |   |-- GaussianSplatWebView.tsx       # Native: WebView 3D splat
|   |   |   |-- GaussianSplatWebView.web.tsx   # Web: iframe 3D splat
|   |   |   |-- LiveHUD.tsx             # Heads-up display overlay
|   |   |   |-- useGaussianBridge.ts    # Bridge hook for splat WebView
|   |   |
|   |   |-- VitalsScreen/
|   |   |   |-- index.tsx               # VitalsScreen container
|   |   |   |-- BreathingGauge.tsx      # Breathing rate arc gauge
|   |   |   |-- HeartRateGauge.tsx      # Heart rate arc gauge
|   |   |   |-- MetricCard.tsx          # Metric display card
|   |   |
|   |   |-- ZonesScreen/
|   |   |   |-- index.tsx               # ZonesScreen container
|   |   |   |-- FloorPlanSvg.tsx        # SVG floor plan with occupancy overlay
|   |   |   |-- useOccupancyGrid.ts     # Occupancy grid computation hook
|   |   |   |-- ZoneLegend.tsx          # Zone color legend
|   |   |
|   |   |-- MATScreen/
|   |   |   |-- index.tsx               # MATScreen container
|   |   |   |-- SurvivorCounter.tsx     # Large survivor count display
|   |   |   |-- MatWebView.tsx          # WebView for MAT zone map
|   |   |   |-- AlertList.tsx           # Scrollable alert list
|   |   |   |-- AlertCard.tsx           # Individual alert card
|   |   |   |-- useMatBridge.ts         # Bridge hook for MAT WebView
|   |   |
|   |   |-- SettingsScreen/
|   |       |-- index.tsx               # SettingsScreen container
|   |       |-- ServerUrlInput.tsx      # Server URL text input with validation
|   |       |-- ThemePicker.tsx         # Dark/light theme toggle
|   |       |-- RssiToggle.tsx          # RSSI scanning enable/disable
|   |
|   |-- services/                        # External communication services (4 services)
|   |   |-- ws.service.ts               # WebSocket client with reconnection
|   |   |-- api.service.ts              # REST API client (fetch-based)
|   |   |-- rssi.service.ts             # Default RSSI service (platform re-export)
|   |   |-- rssi.service.android.ts     # Android RSSI via react-native-wifi-reborn
|   |   |-- rssi.service.ios.ts         # iOS CoreWLAN stub
|   |   |-- rssi.service.web.ts         # Web synthetic RSSI
|   |   |-- simulation.service.ts       # Synthetic SensingFrame generator
|   |
|   |-- stores/                          # Zustand state stores (3 stores)
|   |   |-- poseStore.ts                # Connection state, frames, features, persons
|   |   |-- matStore.ts                 # Survivors, alerts, events, zone map
|   |   |-- settingsStore.ts            # Server URL, theme, RSSI toggle (persisted)
|   |
|   |-- theme/                           # Design system tokens
|   |   |-- index.ts                    # Theme re-exports
|   |   |-- colors.ts                   # Color palette (dark + light)
|   |   |-- spacing.ts                  # Spacing scale
|   |   |-- typography.ts              # Font families and sizes
|   |   |-- ThemeContext.tsx            # React context for theme
|   |
|   |-- types/                           # TypeScript type definitions
|   |   |-- api.ts                      # REST API response types
|   |   |-- html.d.ts                   # HTML asset module declaration
|   |   |-- mat.ts                      # MAT domain types (Survivor, Alert, Event)
|   |   |-- navigation.ts              # Navigation param list types
|   |   |-- react-native-wifi-reborn.d.ts  # Type stubs for wifi-reborn
|   |   |-- sensing.ts                  # SensingFrame, Person, VitalSigns types
|   |
|   |-- utils/                           # Utility functions
|   |   |-- colorMap.ts                 # Value-to-color mapping for gauges
|   |   |-- formatters.ts              # Number/date formatting helpers
|   |   |-- ringBuffer.ts             # Fixed-size ring buffer for frame history
|   |   |-- urlValidator.ts           # Server URL validation
|   |
|   |-- __tests__/                       # Unit tests (mirroring src/ structure)
|       |-- test-utils.tsx              # Test utilities, render helpers, mocks
|       |-- components/                 # Component unit tests (7 test files)
|       |-- hooks/                      # Hook unit tests (3 test files)
|       |-- screens/                    # Screen unit tests (5 test files)
|       |-- services/                   # Service unit tests (4 test files)
|       |-- stores/                     # Store unit tests (3 test files)
|       |-- utils/                      # Utility unit tests (3 test files)
```

**File count summary:**

| Category | Files |
|----------|-------|
| Source (components, screens, services, stores, hooks, utils, types, theme, navigation) | 63 `.ts`/`.tsx` files |
| Unit tests | 25 test files |
| E2E tests (Maestro) | 6 YAML specs |
| Config (babel, metro, jest, tsconfig, eas, app) | 7 config files |
| Assets | 6 image files |
| **Total** | **107 files** |

---

## 4. Implementation Plan (File-Level)

### 4.1 Phase 1: Core Infrastructure

| File | Purpose | Priority |
|------|---------|----------|
| `App.tsx` | Root component with ThemeProvider and NavigationContainer | P0 |
| `index.ts` | Expo entry point | P0 |
| `app.config.ts` | Expo SDK 55 configuration | P0 |
| `src/theme/colors.ts` | Dark and light color palettes | P0 |
| `src/theme/spacing.ts` | Spacing scale | P0 |
| `src/theme/typography.ts` | Font definitions | P0 |
| `src/theme/ThemeContext.tsx` | React context provider for theme | P0 |
| `src/navigation/MainTabs.tsx` | Bottom tab navigator with 5 tabs | P0 |
| `src/navigation/RootNavigator.tsx` | Root stack navigator | P0 |
| `src/types/sensing.ts` | SensingFrame, Person, VitalSigns type definitions | P0 |

### 4.2 Phase 2: State and Services

| File | Purpose | Priority |
|------|---------|----------|
| `src/stores/poseStore.ts` | Zustand store for connection state, frames, persons | P0 |
| `src/stores/matStore.ts` | Zustand store for MAT survivors, alerts, events | P0 |
| `src/stores/settingsStore.ts` | Zustand store with AsyncStorage persistence | P0 |
| `src/services/ws.service.ts` | WebSocket client with reconnection and dispatch | P0 |
| `src/services/api.service.ts` | REST API client | P1 |
| `src/services/simulation.service.ts` | Synthetic SensingFrame generator for fallback | P0 |
| `src/services/rssi.service.ts` | Platform RSSI re-export | P1 |
| `src/services/rssi.service.android.ts` | Android react-native-wifi-reborn integration | P1 |
| `src/services/rssi.service.ios.ts` | iOS CoreWLAN stub | P2 |
| `src/services/rssi.service.web.ts` | Web synthetic RSSI | P1 |
| `src/utils/ringBuffer.ts` | Fixed-size ring buffer for frame history | P0 |
| `src/utils/urlValidator.ts` | Server URL validation | P1 |

### 4.3 Phase 3: Shared Components

| File | Purpose | Priority |
|------|---------|----------|
| `src/components/ConnectionBanner.tsx` | Status banner across all screens | P0 |
| `src/components/GaugeArc.tsx` | SVG arc gauge for vitals | P0 |
| `src/components/SparklineChart.tsx` | Inline sparkline for history | P0 |
| `src/components/OccupancyGrid.tsx` | Grid overlay for zones | P1 |
| `src/components/StatusDot.tsx` | Colored status indicator | P1 |
| `src/components/SignalBar.tsx` | WiFi signal strength display | P1 |
| `src/components/ModeBadge.tsx` | Live/Sim mode badge | P1 |
| `src/components/ErrorBoundary.tsx` | React error boundary | P0 |
| `src/components/LoadingSpinner.tsx` | Loading state indicator | P1 |
| `src/components/ThemedText.tsx` | Themed text component | P0 |
| `src/components/ThemedView.tsx` | Themed view component | P0 |
| `src/components/HudOverlay.tsx` | Translucent HUD for Live screen | P1 |

### 4.4 Phase 4: Screens

| File | Purpose | Priority |
|------|---------|----------|
| `src/screens/LiveScreen/index.tsx` | Live 3D splat + HUD | P0 |
| `src/screens/LiveScreen/GaussianSplatWebView.tsx` | Native WebView for splat | P0 |
| `src/screens/LiveScreen/GaussianSplatWebView.web.tsx` | Web iframe for splat | P1 |
| `src/screens/LiveScreen/LiveHUD.tsx` | HUD overlay with metrics | P1 |
| `src/screens/LiveScreen/useGaussianBridge.ts` | WebView bridge hook | P0 |
| `src/screens/VitalsScreen/index.tsx` | Vitals gauges and sparklines | P0 |
| `src/screens/VitalsScreen/BreathingGauge.tsx` | Breathing rate gauge | P0 |
| `src/screens/VitalsScreen/HeartRateGauge.tsx` | Heart rate gauge | P0 |
| `src/screens/VitalsScreen/MetricCard.tsx` | Vitals metric card | P1 |
| `src/screens/ZonesScreen/index.tsx` | Floor plan with occupancy | P1 |
| `src/screens/ZonesScreen/FloorPlanSvg.tsx` | SVG floor plan renderer | P1 |
| `src/screens/ZonesScreen/useOccupancyGrid.ts` | Occupancy computation | P1 |
| `src/screens/ZonesScreen/ZoneLegend.tsx` | Zone legend | P2 |
| `src/screens/MATScreen/index.tsx` | MAT dashboard | P1 |
| `src/screens/MATScreen/SurvivorCounter.tsx` | Survivor count display | P1 |
| `src/screens/MATScreen/MatWebView.tsx` | MAT zone map WebView | P1 |
| `src/screens/MATScreen/AlertList.tsx` | Alert list | P1 |
| `src/screens/MATScreen/AlertCard.tsx` | Alert card | P2 |
| `src/screens/MATScreen/useMatBridge.ts` | MAT WebView bridge | P1 |
| `src/screens/SettingsScreen/index.tsx` | Settings form | P0 |
| `src/screens/SettingsScreen/ServerUrlInput.tsx` | Server URL input | P0 |
| `src/screens/SettingsScreen/ThemePicker.tsx` | Theme toggle | P2 |
| `src/screens/SettingsScreen/RssiToggle.tsx` | RSSI toggle | P2 |

### 4.5 Phase 5: Testing

| File | Purpose | Priority |
|------|---------|----------|
| `src/__tests__/stores/poseStore.test.ts` | Store state transitions, frame processing | P0 |
| `src/__tests__/stores/matStore.test.ts` | MAT store state management | P1 |
| `src/__tests__/stores/settingsStore.test.ts` | Persistence, defaults | P1 |
| `src/__tests__/services/ws.service.test.ts` | WS connection, reconnection, fallback | P0 |
| `src/__tests__/services/simulation.service.test.ts` | Synthetic frame generation | P1 |
| `src/__tests__/services/api.service.test.ts` | REST client mocking | P1 |
| `src/__tests__/services/rssi.service.test.ts` | Platform RSSI mocking | P2 |
| `src/__tests__/components/*.test.tsx` | Component render tests (7 files) | P1 |
| `src/__tests__/hooks/*.test.ts` | Hook behavior tests (3 files) | P1 |
| `src/__tests__/screens/*.test.tsx` | Screen integration tests (5 files) | P1 |
| `src/__tests__/utils/*.test.ts` | Utility function tests (3 files) | P1 |
| `e2e/*.yaml` | Maestro E2E specs (6 files) | P2 |

---

## 5. Acceptance Criteria

### 5.1 Build and Platform Support

| ID | Criterion | Test Method |
|----|-----------|-------------|
| B-1 | App builds successfully with `npx expo start` for iOS, Android, and Web | CI build matrix: `expo start --ios`, `--android`, `--web` |
| B-2 | App runs on iOS Simulator (iPhone 15 Pro, iOS 17+) | Manual verification on Simulator |
| B-3 | App runs on Android Emulator (API 34+) | Manual verification on Emulator |
| B-4 | App runs in web browser (Chrome 120+, Safari 17+, Firefox 120+) | Manual verification in browsers |
| B-5 | TypeScript compiles with zero errors in strict mode | `npx tsc --noEmit` in CI |

### 5.2 WebSocket and Data Streaming

| ID | Criterion | Test Method |
|----|-----------|-------------|
| W-1 | WebSocket connects to sensing server and receives SensingFrame JSON | Integration test: start server, verify `poseStore.connectionStatus === 'connected'` |
| W-2 | `poseStore.latestFrame` updates within 100ms of WebSocket message receipt | Unit test: mock WS, measure dispatch latency |
| W-3 | WebSocket reconnects with exponential backoff after connection loss | Unit test: simulate WS close, verify retry intervals (1s, 2s, 4s, 8s, 16s) |
| W-4 | Automatic fallback to simulated data within 5 seconds of connection failure | Unit test: fail WS 5 times, verify `connectionStatus === 'simulated'` within 5s |
| W-5 | App recovers gracefully from sensing server restart (reconnects without crash) | Integration test: kill server, restart, verify reconnection and `connectionStatus === 'connected'` |

### 5.3 Screen Rendering

| ID | Criterion | Test Method |
|----|-----------|-------------|
| S-1 | All 5 screens render correctly with live data from sensing server | Integration test: connect to server, navigate all tabs, verify content |
| S-2 | All 5 screens render correctly with simulated data | Unit test: set `connectionStatus = 'simulated'`, verify all screens render |
| S-3 | Vital signs gauges animate smoothly (breathing BPM, heart rate BPM) | Visual inspection: gauges update at frame rate without jank |
| S-4 | 3D Gaussian splat viewer shows skeleton with 17 COCO keypoints | Integration test: verify WebView loads, bridge sends keypoints, splat renders |
| S-5 | Floor plan SVG updates with occupancy data when persons are detected | Unit test: inject 3 persons into poseStore, verify 3 markers on FloorPlanSvg |
| S-6 | MAT dashboard shows survivor count, zone map, and alert list | Unit test: inject matStore data, verify SurvivorCounter and AlertList render |
| S-7 | Connection banner shows correct status text and color for all 3 states | Unit test: cycle through `connected`/`simulated`/`error`, verify banner text and color |

### 5.4 Persistence and Settings

| ID | Criterion | Test Method |
|----|-----------|-------------|
| P-1 | Settings persist across app restarts (server URL, theme, RSSI toggle) | Integration test: set values, kill app, restart, verify values restored |
| P-2 | Default server URL is `http://localhost:3000` when no persisted value exists | Unit test: clear AsyncStorage, verify default |
| P-3 | Server URL input validates format before saving | Unit test: submit `not-a-url`, verify rejection; submit `http://192.168.1.1:3000`, verify acceptance |

### 5.5 Navigation and UX

| ID | Criterion | Test Method |
|----|-----------|-------------|
| N-1 | Bottom tab navigation works with correct icons for all 5 tabs | E2E: Maestro navigates all tabs, verifies active state |
| N-2 | Dark theme renders correctly on all platforms (background #0D1117, accent #32B8C6) | Visual inspection on iOS, Android, Web |
| N-3 | No infinite render loops or memory leaks in stores | Unit test: mount all screens, process 1000 frames, verify no memory growth beyond ring buffer size |
| N-4 | ErrorBoundary catches and displays fallback UI for component errors | Unit test: throw in child component, verify fallback renders |

### 5.6 Platform-Specific Features

| ID | Criterion | Test Method |
|----|-----------|-------------|
| R-1 | RSSI scanning works on Android with react-native-wifi-reborn | Manual test on Android device with location permission granted |
| R-2 | iOS RSSI service returns empty results without crashing | Unit test: call `scanNetworks()` on iOS, verify empty array returned |
| R-3 | Web RSSI service generates synthetic RSSI values | Unit test: call `scanNetworks()` on web, verify synthetic data returned |

### 5.7 Testing

| ID | Criterion | Test Method |
|----|-----------|-------------|
| T-1 | All unit tests pass (`npm test` exits 0) | CI: `cd ui/mobile && npm test` |
| T-2 | E2E Maestro tests pass for all 5 screens | CI: `maestro test e2e/` |
| T-3 | E2E offline fallback test passes (simulated mode activates on disconnect) | CI: `maestro test e2e/offline_fallback.yaml` |
| T-4 | No TypeScript type errors | CI: `npx tsc --noEmit` |

---

## 6. Consequences

### 6.1 Positive

- **Single codebase for three platforms**: Expo SDK 55 with React Native 0.83 builds iOS, Android, and Web from the same TypeScript source, reducing development and maintenance cost by approximately 60% compared to separate native apps.
- **Instant field deployment**: Operators can install the app via Expo Go (development) or EAS Build (production) and connect to a local sensing server within minutes. No server-side mobile infrastructure required.
- **Sub-100ms display latency**: WebSocket streaming from the Rust sensing server to the mobile app introduces less than 100ms additional latency beyond the CSI processing pipeline, providing near-real-time visualization.
- **Offline-capable demos**: The simulation service generates realistic synthetic SensingFrame data, enabling demonstrations to stakeholders and testing without ESP32 hardware or a running sensing server.
- **Operator-friendly UX**: Five purpose-built screens cover the primary use cases (live view, vitals, zones, MAT, settings) with a bottom-tab navigation pattern familiar to mobile users.
- **Testable architecture**: Zustand stores with selector-based subscriptions, service-layer abstraction, and Maestro E2E specs provide a comprehensive testing strategy from unit to integration to end-to-end.
- **Reuses existing infrastructure**: The app consumes the same WebSocket and REST APIs as the desktop UI, requiring no backend changes. The Three.js splat renderer is reused via WebView.

### 6.2 Negative

- **WebView-based 3D rendering has lower performance than native OpenGL**: The Gaussian splat viewer runs inside a WebView (native) or iframe (web), adding a JavaScript-to-native bridge hop and limiting frame rate to approximately 30 FPS on mid-range devices. Native OpenGL or Metal/Vulkan rendering would achieve 60 FPS but requires platform-specific code.
- **react-native-wifi-reborn requires native module linking for Android RSSI**: This breaks the pure Expo managed workflow for Android builds. EAS Build with a custom development client is required. iOS RSSI scanning is not possible at all due to Apple restrictions.
- **Expo managed workflow limits some native module access**: Certain native APIs (background location, Bluetooth LE, raw WiFi frames) are not available without ejecting to a bare workflow. This constrains future features like Bluetooth mesh fallback.
- **WebView bridge latency**: Communication between React Native and the Three.js WebView via `postMessage` adds 5-15ms per message, reducing effective update rate for the 3D splat view. This is acceptable for 10-20 Hz sensing frame rates but would become a bottleneck at higher rates.
- **AsyncStorage has no encryption**: Settings (including server URL) are stored in plaintext AsyncStorage. For security-sensitive deployments, expo-secure-store should replace AsyncStorage for credential storage.

### 6.3 Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Expo SDK 55 breaking changes in future updates | Medium | Build failures, API deprecations | Pin SDK version in `app.config.ts`; test upgrades in preview branch |
| WebView memory pressure on low-end Android devices | Medium | OOM crash during Three.js splat rendering | Implement splat LOD (level of detail) fallback; monitor WebView memory via `onContentProcessDidTerminate` |
| react-native-wifi-reborn unmaintained or incompatible with RN 0.83 | Low | Android RSSI scanning broken | Fork and patch if needed; RSSI scanning is a secondary feature |
| Sensing server WebSocket protocol changes | Medium | Frame parsing errors, broken display | Version the WebSocket protocol; add `protocol_version` field to SensingFrame |
| Battery drain from continuous WebSocket connection on mobile | Medium | Poor user experience in extended field use | Implement configurable update rate throttling in settings; pause WS when app is backgrounded |
| Three.js Gaussian splat HTML bundle size exceeds WebView limits | Low | Slow initial load, white screen | Lazy-load splat bundle; show placeholder skeleton during load; cache bundle in AsyncStorage |

---

## 7. Future Work

### 7.1 Offline Model Inference

Run a quantized ONNX pose estimation model directly on the mobile device using `onnxruntime-react-native`. This would allow the app to process raw CSI data (received via a local UDP relay or Bluetooth) without a sensing server, enabling fully disconnected field operation.

**Prerequisites:** Export the trained WiFi-DensePose model (ADR-023) to ONNX format; quantize to INT8 for mobile; benchmark inference latency on iPhone 15 and Pixel 8.

### 7.2 Push Notifications for MAT Alerts

Integrate Firebase Cloud Messaging (Android) and APNs (iOS) to deliver push notifications when the sensing server detects new survivors or critical vital sign alerts. This allows operators to be alerted even when the app is backgrounded.

**Prerequisites:** Add a push notification endpoint to the Rust sensing server; implement Expo Notifications integration in the mobile app.

### 7.3 Apple Watch Companion

Build a watchOS companion app using Expo's experimental watch support or a native SwiftUI module. The watch would display a minimal vitals view (breathing rate, heart rate, alert count) on the operator's wrist, with haptic feedback for critical MAT alerts.

**Prerequisites:** Evaluate Expo watch support maturity; define minimal watch screen set; implement WatchConnectivity bridge.

### 7.4 Bluetooth Mesh Fallback

When WiFi is unavailable (collapsed building, power outage), use Bluetooth Low Energy (BLE) mesh to relay aggregated CSI summaries from ESP32 nodes to the mobile device. This requires ejecting from Expo managed workflow to bare workflow for BLE native module access.

**Prerequisites:** Implement BLE GATT service on ESP32 firmware (ADR-018); integrate `react-native-ble-plx` in bare Expo workflow; define BLE CSI summary protocol (compressed, lower bandwidth than WiFi).

### 7.5 Multi-Server Dashboard

Support connecting to multiple sensing servers simultaneously (e.g., one per floor or building wing). The app would aggregate data from all servers into a unified zone map and MAT dashboard with per-server status indicators.

**Prerequisites:** Extend `settingsStore` to support server list; modify `ws.service` to manage multiple WebSocket connections; merge `poseStore` frames from multiple sources with server-id tags.

---

## 8. Related ADRs

| ADR | Relationship |
|-----|-------------|
| ADR-019 (Sensing-Only UI Mode) | **Extended**: The mobile app is the field-optimized evolution of the sensing-only UI mode, adding native mobile capabilities (push, RSSI, offline) |
| ADR-021 (Vital Sign Detection) | **Consumed**: VitalsScreen displays breathing_rate_bpm and heart_rate_bpm extracted by the ADR-021 pipeline |
| ADR-026 (Survivor Track Lifecycle) | **Consumed**: MATScreen displays survivor tracks with lifecycle states (detected, confirmed, rescued, lost) from ADR-026 |
| ADR-029 (RuvSense Multistatic) | **Consumed**: The sensing server aggregates ESP32 TDM frames (ADR-029) and streams processed results to the mobile app |
| ADR-031 (RuView Sensing-First RF) | **Consumed**: The WebSocket and REST APIs exposed by `wifi-densepose-sensing-server` (ADR-031) are the mobile app's data source |
| ADR-032 (Mesh Security) | **Consumed**: Authenticated CSI frames (ADR-032) ensure the mobile app displays trustworthy data, not spoofed sensor readings |

---

## 9. References

1. Expo SDK 55 Documentation. https://docs.expo.dev/
2. React Native 0.83 Release Notes. https://reactnative.dev/
3. Zustand v5. https://github.com/pmndrs/zustand
4. React Navigation v7. https://reactnavigation.org/
5. Maestro Mobile Testing Framework. https://maestro.mobile.dev/
6. react-native-wifi-reborn. https://github.com/JuanSeBestworker/react-native-wifi-reborn
7. Three.js Gaussian Splatting. https://github.com/mrdoob/three.js
8. AsyncStorage. https://react-native-async-storage.github.io/async-storage/
9. Geng, J. et al. (2023). "DensePose From WiFi." arXiv:2301.00250.
10. ADR-019 through ADR-032 (internal).
