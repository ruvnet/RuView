# Quality Experience (QX) Analysis: WiFi-DensePose

**Report ID**: QX-2026-005
**Date**: 2026-04-05
**Scope**: Full-stack quality experience across API, CLI, Mobile, DX, and Hardware
**QX Score**: 71/100 (C+)

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Overall QX Scores](#2-overall-qx-scores)
3. [User Journey Analysis by Persona](#3-user-journey-analysis-by-persona)
4. [API Experience Analysis](#4-api-experience-analysis)
5. [CLI Experience Analysis](#5-cli-experience-analysis)
6. [Mobile App UX Analysis](#6-mobile-app-ux-analysis)
7. [Developer Experience (DX) Analysis](#7-developer-experience-dx-analysis)
8. [Hardware Integration UX Analysis](#8-hardware-integration-ux-analysis)
9. [Cross-Cutting Quality Concerns](#9-cross-cutting-quality-concerns)
10. [Oracle Problems Detected](#10-oracle-problems-detected)
11. [Prioritized Recommendations](#11-prioritized-recommendations)
12. [Heuristic Scoring Summary](#12-heuristic-scoring-summary)

---

## 1. Executive Summary

The WiFi-DensePose system demonstrates strong architectural foundations with a well-structured FastAPI backend, a mature React Native mobile app, and a comprehensive CLI. However, the quality experience is uneven across touchpoints, with several gaps that impact different user personas in distinct ways.

### Key Findings

**Strengths:**
- Comprehensive error handling middleware with structured error responses, request IDs, and environment-aware detail levels (`archive/v1/src/middleware/error_handler.py`)
- Robust WebSocket reconnection with exponential backoff and automatic simulation fallback in the mobile app (`ui/mobile/src/services/ws.service.ts`)
- Well-designed health check architecture with component-level status, readiness probes, and liveness endpoints (`archive/v1/src/api/routers/health.py`)
- Strong input validation on API models with Pydantic, including range constraints and clear field descriptions (`archive/v1/src/api/routers/pose.py`)
- Persistent settings with AsyncStorage in the mobile app, surviving app restarts (`ui/mobile/src/stores/settingsStore.ts`)
- Server URL validation with test-before-save workflow in mobile settings (`ui/mobile/src/screens/SettingsScreen/ServerUrlInput.tsx`)

**Critical Issues:**
- API documentation is disabled in production (`docs_url=None`, `redoc_url=None` when `is_production=True`), leaving production API consumers without discoverability (in `archive/v1/src/api/main.py` line 146-148)
- No user-facing progress indicator during calibration -- the calibration endpoint returns an estimated duration but there is no polling endpoint progress beyond percentage (`archive/v1/src/api/routers/pose.py` lines 320-361)
- Rate limit responses lack a human-readable `Retry-After` message body; the client receives a bare `"Rate limit exceeded"` string with retry information only in HTTP headers (`archive/v1/src/middleware/rate_limit.py` line 323)
- CLI `status` command uses emoji/Unicode characters that break in terminals without UTF-8 support (`archive/v1/src/commands/status.py` lines 360-474)
- Mobile app `MainTabs.tsx` passes an inline arrow function as the `component` prop to `Tab.Screen` (line 130), causing unnecessary re-renders on every parent render cycle

**Top 3 Recommendations:**
1. Add a separate production API documentation URL (e.g., `/api-docs`) with authentication, rather than removing docs entirely
2. Implement a WebSocket-based calibration progress stream or add a polling endpoint that returns step-by-step progress
3. Add a `--no-emoji` CLI flag or auto-detect terminal capabilities to avoid broken status output

---

## 2. Overall QX Scores

| Dimension | Score | Grade | Assessment |
|-----------|-------|-------|------------|
| **Overall QX** | 71/100 | C+ | Functional but inconsistent across touchpoints |
| **API Experience** | 78/100 | B- | Well-structured endpoints, good error model, weak discoverability |
| **CLI Experience** | 65/100 | D+ | Adequate commands, poor terminal compatibility, limited help |
| **Mobile UX** | 80/100 | B | Strong connection handling, good fallbacks, minor render issues |
| **Developer Experience** | 68/100 | D+ | Steep learning curve, complex build, limited onboarding docs |
| **Hardware UX** | 62/100 | D | Complex provisioning, limited error recovery guidance |
| **Accessibility** | 45/100 | F | No ARIA consideration in mobile, no high-contrast support |
| **Trust & Reliability** | 76/100 | B- | Good health checks, rate limiting, auth framework in place |
| **Cross-Codebase Consistency** | 70/100 | C | Different error formats between API/CLI, naming inconsistencies |

---

## 3. User Journey Analysis by Persona

### 3.1 Developer Persona

**Journey**: Clone repo -> Set up environment -> Build -> Run tests -> Develop -> Submit PR

| Step | Success Rate | Pain Level | Bottleneck |
|------|-------------|------------|------------|
| Clone & orient | Moderate | MEDIUM | Multiple codebases (Python v1, Rust, firmware, mobile) with no single entry point guide |
| Environment setup | Low | HIGH | Requires Python + Rust toolchain + Node.js + ESP-IDF for full development |
| Build Python API | Moderate | MEDIUM | Dependency management not containerized for easy onboarding |
| Run Rust tests | High | LOW | `cargo test --workspace --no-default-features` works reliably (1,031+ tests) |
| Run Python tests | Moderate | MEDIUM | Requires database setup, Redis optional but affects behavior |
| Contribute to mobile | Moderate | MEDIUM | Expo/React Native setup is standard but undocumented within this repo |

**Key Findings:**
- `CLAUDE.md` is comprehensive for AI agents but not optimized for human developers; it mixes agent configuration with build instructions
- No `CONTRIBUTING.md` file exists
- Build commands are scattered: Python uses `pip`, Rust uses `cargo`, mobile uses `npm`, firmware uses ESP-IDF
- Test commands differ between `npm test`, `cargo test`, and `python -m pytest` with no unified runner
- The pre-merge checklist in `CLAUDE.md` has 12 items, which is thorough but creates friction for external contributors

### 3.2 Operator Persona

**Journey**: Install -> Configure -> Start server -> Monitor -> Troubleshoot

| Step | Success Rate | Pain Level | Bottleneck |
|------|-------------|------------|------------|
| Install | Low | HIGH | No single installation script or Docker Compose for the full stack |
| Configure | Moderate | MEDIUM | Config file path must be specified; no `--init` to generate default config |
| Start server | Moderate | MEDIUM | `wifi-densepose start` works but database must be initialized first |
| Monitor status | High | LOW | `wifi-densepose status --detailed` provides comprehensive output |
| Stop server | High | LOW | Both graceful and force-stop options available |
| Troubleshoot | Low | HIGH | Error messages reference internal exceptions; no runbook or FAQ |

**Key Findings:**
- The CLI offers `start`, `stop`, `status`, `db init/migrate/rollback`, `config show/validate/failsafe`, `tasks run/status`, and `version` -- a reasonable command set
- However, there is no `wifi-densepose init` command to scaffold a working configuration from scratch
- The `config validate` command checks database, Redis, and directory availability -- good for operators
- The `config failsafe` command showing SQLite fallback status is a strong resilience feature
- Missing: log rotation configuration, log level adjustment at runtime, and a `wifi-densepose doctor` self-diagnosis command

### 3.3 End-User Persona (Mobile App User)

**Journey**: Open app -> Connect to server -> View live data -> Check vitals -> Manage zones -> Configure settings

| Step | Success Rate | Pain Level | Bottleneck |
|------|-------------|------------|------------|
| Open app | High | LOW | Clean initial load with loading spinners |
| Connect to server | Moderate | MEDIUM | Default URL is `localhost:3000` which will not work on physical devices |
| View live data | High | LOW | Simulation fallback ensures something is always displayed |
| Check vitals | High | LOW | Gauges, sparklines, and classification render smoothly |
| Manage zones | Moderate | LOW | Heatmap visualization is functional |
| Configure settings | High | LOW | Server URL validation, test connection, save workflow is solid |

**Key Findings:**
- The default `serverUrl` in `settingsStore.ts` is `http://localhost:3000`, which will fail on a physical device where the server runs on a different machine; a first-run setup wizard would improve this
- Connection state management is well-implemented with three visible states: `LIVE STREAM`, `SIMULATED DATA`, and `DISCONNECTED` via `ConnectionBanner.tsx`
- The simulation fallback (`generateSimulatedData()`) activates automatically when WebSocket connection fails, ensuring the app never shows a blank screen
- The MAT (Mass Casualty Assessment Tool) screen seeds a training scenario on first load, which may confuse users who expect a clean state
- `ErrorBoundary` provides crash recovery with a "Retry" button, but the error message is the raw JavaScript error (`error.message`) without user-friendly context

---

## 4. API Experience Analysis

### 4.1 Endpoint Structure (Score: 82/100)

The API follows RESTful conventions with clear resource paths:

```
GET  /health/health       - System health
GET  /health/ready        - Readiness probe
GET  /health/live         - Liveness probe
GET  /health/metrics      - System metrics (auth required for detailed)
GET  /health/version      - Version info

GET  /api/v1/pose/current - Current pose estimation
POST /api/v1/pose/analyze - Custom analysis (auth required)
GET  /api/v1/pose/zones/{zone_id}/occupancy - Zone occupancy
GET  /api/v1/pose/zones/summary - All zones summary
POST /api/v1/pose/historical - Historical data (auth required)
GET  /api/v1/pose/activities - Recent activities
POST /api/v1/pose/calibrate - Start calibration (auth required)
GET  /api/v1/pose/calibration/status - Calibration status
GET  /api/v1/pose/stats - Statistics

WS   /api/v1/stream/pose  - Real-time pose stream
WS   /api/v1/stream/events - Event stream
```

**Issues Found:**
- `GET /health/health` is redundant path nesting; the health router is mounted at `/health` prefix, making the full path `/health/health`. This should be `/health` (root of the health router) or the prefix should be `/` for the health router
- `POST /api/v1/pose/historical` uses POST for a read operation. While this is common for complex queries, it violates REST conventions. A `GET` with query parameters or a `POST /api/v1/pose/query` would be clearer
- The root endpoint (`GET /`) exposes feature flags (`authentication`, `rate_limiting`) which could leak security posture information

### 4.2 Error Handling (Score: 85/100)

The `ErrorHandler` class in `archive/v1/src/middleware/error_handler.py` is well-designed:

**Strengths:**
- Structured error responses with consistent format: `{ "error": { "code": "...", "message": "...", "timestamp": "...", "request_id": "..." } }`
- Request ID tracking via `X-Request-ID` header for debugging
- Environment-aware: tracebacks included in development, hidden in production
- Specialized handlers for HTTP, validation, Pydantic, database, and external service errors
- Custom exception classes (`BusinessLogicError`, `ResourceNotFoundError`, `ConflictError`, `ServiceUnavailableError`) with domain context

**Issues Found:**
- The `ErrorHandlingMiddleware` class exists but is commented out (line 432-434 in `error_handler.py`), meaning errors are handled by `setup_error_handling()` exception handlers instead. The middleware class and the exception handlers use different `ErrorHandler` instances, creating potential inconsistency if one is changed without the other
- The `_is_database_error()` check uses string matching on module names (line 355-373), which is fragile. `"ConnectionError"` will match `aiohttp.ConnectionError` (an external service error), not just database connection errors
- Error responses do not include a `documentation_url` field that could guide users to relevant docs

### 4.3 Rate Limiting UX (Score: 72/100)

**Strengths:**
- Dual algorithm support: sliding window counter and token bucket
- Per-endpoint rate limiting with per-user differentiation
- Standard `X-RateLimit-*` headers on all responses
- `Retry-After` header on 429 responses
- Health/docs/metrics paths exempted from rate limiting
- Configurable presets for development, production, API, and strict modes

**Issues Found:**
- The 429 response body is `"Rate limit exceeded"` (a plain string). No structured error response with the `ErrorResponse` format is used. The rate limit middleware raises `HTTPException` directly rather than using `CustomHTTPException` or `ErrorResponse`
- No information about which rate limit bucket was exhausted (per-IP vs per-user vs per-endpoint)
- No rate limit dashboard or endpoint to check current rate limit status without making a request
- The `RateLimitConfig` presets (development, production, api, strict) are defined but there is no CLI command or API endpoint to switch between them

### 4.4 WebSocket Experience (Score: 80/100)

**Strengths:**
- Connection confirmation message with client ID and configuration on connect
- Structured message protocol with `type` field (`ping`, `update_config`, `get_status`)
- Invalid JSON is handled gracefully with an error message back to client
- Stale connection cleanup every 60 seconds with 5-minute timeout
- Zone-based and stream-type-based filtering for broadcasts
- Client-side config updates without reconnection via `update_config` message

**Issues Found:**
- Authentication is checked _after_ `websocket.accept()` (line 80-93 in `stream.py`), meaning unauthenticated clients briefly hold a connection before being closed. This wastes resources and leaks the existence of the endpoint
- The `handle_websocket_message` function handles unknown message types with an error, but does not suggest valid message types: `"Unknown message type: foo"` should list valid options
- No heartbeat/keepalive mechanism initiated from the server. The client must send ping messages. If the client does not ping, the connection will be considered stale after 5 minutes even if data is flowing
- Close codes are not documented for clients to handle reconnection logic

### 4.5 API Documentation & Discoverability (Score: 58/100)

**Issues Found:**
- Swagger UI (`/docs`) and ReDoc (`/redoc`) are **disabled in production** (line 146-148 of `main.py`): `docs_url=settings.docs_url if not settings.is_production else None`
- No alternative documentation hosting for production environments
- The `GET /` root endpoint and `GET /api/v1/info` endpoint provide feature information but no link to documentation
- Pydantic models have good `Field(description=...)` annotations, which would generate useful OpenAPI docs -- but only visible in development
- No API changelog or versioning documentation beyond the `version` field

---

## 5. CLI Experience Analysis

### 5.1 Command Structure (Score: 70/100)

The CLI uses Click with a nested group structure:

```
wifi-densepose [--config FILE] [--verbose] [--debug]
  start   [--host] [--port] [--workers] [--reload] [--daemon]
  stop    [--force] [--timeout]
  status  [--format text|json] [--detailed]
  db
    init      [--url]
    migrate   [--revision]
    rollback  [--steps]
  tasks
    run       [--task cleanup|monitoring|backup]
    status
  config
    show
    validate
    failsafe  [--format text|json]
  version
```

**Strengths:**
- Logical grouping of commands (server, db, tasks, config)
- Global options `--config`, `--verbose`, `--debug` available on all commands
- `--daemon` mode with PID file management and stale PID detection
- JSON output format option on `status` and `failsafe` for scripting

**Issues Found:**
- No shell completion support (Click supports it but it is not configured)
- No `init` or `setup` command to generate a default configuration file
- No `logs` command to tail or search server logs
- The `tasks status` subcommand shadows the parent `status` command in Click's namespace (line 347-348 in `cli.py` defines `def status(ctx):` under the `tasks` group), which works but creates confusion
- No `--quiet` option for scripting (opposite of `--verbose`)
- Error output goes through `logger.error()` which depends on logging configuration; if logging is misconfigured, errors are silently lost

### 5.2 Error Messages (Score: 60/100)

**Issues Found:**
- Errors from `start` command show the raw exception: `"Failed to start server: {e}"` where `{e}` is the Python exception string
- No suggestion for common failure scenarios. For example, if the database connection fails during `start`, the error is `"Database connection failed: [psycopg2 error]"` with no guidance like "Check your DATABASE_URL setting" or "Run 'wifi-densepose db init' first"
- The `config validate` command outputs check-style messages (`"X Database connection: FAILED - {e}"`) which is helpful, but the X and checkmark characters use Unicode that may not render in all terminals
- The `stop` command handles "Server is not running" gracefully, which is good
- Missing: error codes that users could search for in documentation

### 5.3 Help Text (Score: 65/100)

**Strengths:**
- Each command has a one-line description
- Options have help text and defaults documented

**Issues Found:**
- No examples in help text. The argparse `epilog` pattern used in `provision.py` is good practice but is not used in the Click CLI
- No `--help` examples showing common workflows like "Start a development server", "Deploy to production", or "Initialize a fresh installation"
- Command descriptions are terse: `"Start the WiFi-DensePose API server"` does not mention prerequisites

### 5.4 Configuration Workflow (Score: 68/100)

**Strengths:**
- `config show` displays the full configuration without secrets
- `config validate` checks database, Redis, and directory access
- `config failsafe` shows SQLite fallback and Redis degradation status
- Settings can be loaded from a file via `--config` flag

**Issues Found:**
- No `config init` to generate a template configuration file
- No `config set KEY VALUE` to modify individual settings
- No environment variable listing showing which variables affect configuration
- The `config show` output dumps JSON but does not annotate which values are defaults vs user-configured

---

## 6. Mobile App UX Analysis

### 6.1 Screen Flow Architecture (Score: 82/100)

The app uses a bottom tab navigator with five screens:

```
Live (wifi icon) -> Vitals (heart) -> Zones (grid) -> MAT (shield) -> Settings (gear)
```

**Strengths:**
- Lazy loading of all screens with `React.lazy` and suspense fallbacks showing loading indicator with screen name
- Fallback placeholder screens for any screen that fails to load: `"{label} screen not implemented yet"` with a "Placeholder shell" subtitle
- MAT screen badge showing alert count in the tab bar
- Icon mapping is clear and semantically appropriate

**Issues Found:**
- `MainTabs.tsx` line 130: `component={() => <Suspended component={component} />}` creates a new function reference on every render. This should be refactored to a stable component reference to prevent unnecessary tab re-renders
- No deep linking support for navigating directly to a screen from a notification or external URL
- No screen transition animations configured; the default tab switch is abrupt
- Tab labels use `fontFamily: 'Courier New'` which may not be available on all devices, with no fallback font specified

### 6.2 Connection Handling (Score: 88/100)

The WebSocket connection strategy in `ws.service.ts` is well-designed:

**Strengths:**
- Exponential backoff reconnection: delays of 1s, 2s, 4s, 8s, 16s
- Maximum 10 reconnection attempts before falling back to simulation
- Simulation mode provides continuous data display even when disconnected
- Connection status propagated to all screens via Zustand store
- Clean disconnect with close code 1000
- Auto-connect on app mount via `usePoseStream` hook
- URL validation before attempting connection

**Issues Found:**
- When reconnecting, the simulation timer starts immediately during the backoff delay, which means the user briefly sees "SIMULATED DATA" then "LIVE STREAM" then potentially "SIMULATED DATA" again if the reconnect fails. This creates a flickering experience
- No user notification when switching between live and simulated modes beyond the banner color change
- The WebSocket URL construction in `buildWsUrl()` hardcodes the path `/ws/sensing`, but the API server expects `/api/v1/stream/pose`. This path mismatch (`WS_PATH = '/api/v1/stream/pose'` in `constants/websocket.ts` vs `/ws/sensing` in `ws.service.ts`) is a potential connection failure point
- No explicit ping/pong keepalive from the client; relies on the WebSocket protocol's built-in mechanism

### 6.3 Loading & Error States (Score: 78/100)

**Strengths:**
- `LoadingSpinner` component with smooth rotation animation using `react-native-reanimated`
- `ErrorBoundary` wraps the LiveScreen with crash recovery
- LiveScreen shows a dedicated error state with "Live visualization failed", the error message, and a "Retry" button
- Retry increments a `viewerKey` to force component remount
- `ConnectionBanner` provides three distinct visual states with semantic colors (green/amber/red)

**Issues Found:**
- The `ErrorBoundary` shows `error.message` directly, which may be a technical JavaScript error string like `"Cannot read property 'x' of undefined"`. A user-friendly message mapping would improve the experience
- No timeout handling on loading states. If the GaussianSplat WebView never fires `onReady`, the loading spinner displays indefinitely
- The VitalsScreen shows `N/A` for features when no data is available, but the gauges (`BreathingGauge`, `HeartRateGauge`) behavior at zero/null values is not guarded in the screen code
- No skeleton loading states; screens jump from blank to fully rendered

### 6.4 State Management (Score: 85/100)

**Strengths:**
- Zustand stores are well-structured with clear separation: `poseStore` (real-time data), `settingsStore` (configuration), `matStore` (MAT data)
- `settingsStore` uses `persist` middleware with AsyncStorage for cross-session persistence
- `poseStore` uses a `RingBuffer` for RSSI history, capping at 60 entries to prevent memory growth
- Clean `reset()` method on `poseStore` to clear all state

**Issues Found:**
- `poseStore` is not persisted, so all historical data is lost on app restart. For a monitoring application, this is a significant gap
- The `handleFrame` method updates 6 state properties atomically in one `set()` call, which is correct, but the `rssiHistory` is computed from a module-level `RingBuffer` that exists outside the store, creating a potential synchronization issue during hot reload
- No state migration strategy for `settingsStore` -- if the schema changes between app versions, persisted state may cause errors

### 6.5 Server Configuration UX (Score: 82/100)

The `ServerUrlInput` component in the Settings screen provides:

**Strengths:**
- Real-time URL validation with `validateServerUrl()` showing error messages inline
- "Test Connection" button that measures and displays response latency
- Visual feedback: border turns red on invalid URL, test result shows checkmark/X with timing
- "Save" button separated from "Test" to allow testing before committing

**Issues Found:**
- Default server URL `http://localhost:3000` will never work on a physical device. The first-run experience should prompt for the server address or attempt auto-discovery via mDNS/Bonjour
- No QR code scanner to configure server URL (common in IoT companion apps)
- Test result is ephemeral -- it disappears when navigating away and returning
- No validation of port range or IP address format beyond URL syntax
- Save does not confirm success to the user; the connection simply restarts silently

---

## 7. Developer Experience (DX) Analysis

### 7.1 Build Process (Score: 65/100)

**Issues Found:**
- Four separate build systems: Python (`pip`/`poetry`), Rust (`cargo`), Node.js (`npm`), and ESP-IDF for firmware
- No unified `Makefile`, `Taskfile`, or `just` file to abstract build commands
- `CLAUDE.md` lists build commands but they are mixed with AI agent configuration
- Docker support is mentioned in the pre-merge checklist but no `docker-compose.yml` for local development was found
- The Rust workspace has 15 crates with a specific publishing order -- this dependency chain is documented but not automated

### 7.2 Testing Experience (Score: 72/100)

**Strengths:**
- Rust workspace has 1,031+ tests with a single command: `cargo test --workspace --no-default-features`
- Deterministic proof verification via `python archive/v1/data/proof/verify.py` with SHA-256 hash checking
- Mobile app has comprehensive test coverage with tests for components, hooks, screens, services, stores, and utilities
- Witness bundle verification with `VERIFY.sh` providing 7/7 pass/fail attestation

**Issues Found:**
- No unified test runner across codebases
- Python test command (`python -m pytest tests/ -x -q`) requires proper environment setup first
- Mobile tests require additional setup (`jest`, React Native testing libraries)
- No integration test suite that tests the full stack (API + WebSocket + Mobile)
- No test coverage reporting configured for the Python codebase

### 7.3 Documentation Quality (Score: 62/100)

**Strengths:**
- 43 Architecture Decision Records (ADRs) in `docs/adr/`
- Domain-Driven Design documentation in `docs/ddd/`
- Comprehensive hardware audit in ADR-028 with witness bundle
- User guide at `docs/user-guide.md`

**Issues Found:**
- No quickstart guide for first-time contributors
- `CLAUDE.md` is 500+ lines but is primarily an AI agent configuration file, not a developer guide
- No API reference documentation beyond the auto-generated Swagger (which is disabled in production)
- No architecture diagram showing how the Python API, Rust core, mobile app, and ESP32 firmware interact
- Missing: changelog is referenced in the pre-merge checklist but its location is not specified

### 7.4 Error Messages for Developers (Score: 70/100)

**Strengths:**
- FastAPI validation errors return field-level details with type, message, and location
- Rust crate errors use typed error types (`wifi-densepose-core`)
- Middleware error handler includes traceback in development mode

**Issues Found:**
- Python API errors in handlers use f-string formatting with raw exception messages: `f"Pose estimation failed: {str(e)}"`. These are user-facing but contain internal details
- No error code catalog or error reference documentation
- Startup validation errors print checkmarks but do not provide remediation steps

### 7.5 Configuration Management (Score: 68/100)

**Strengths:**
- Pydantic `Settings` class with environment variable support
- Configuration file loading via `--config` CLI flag
- Database failsafe with SQLite fallback
- Redis optional with graceful degradation

**Issues Found:**
- No `.env.example` or `.env.template` file to guide environment variable setup
- No configuration schema documentation beyond code inspection
- Sensitive settings (database URL, JWT secret) are validated but error messages do not specify which environment variables to set
- The `config show` command redacts secrets but does not explain where secrets should be configured

---

## 8. Hardware Integration UX Analysis

### 8.1 ESP32 Provisioning Flow (Score: 65/100)

The `provision.py` script in `firmware/esp32-csi-node/` handles WiFi credential and mesh configuration:

**Strengths:**
- Clear `--help` text with usage examples in the argparse epilog
- Parameter validation: TDM slot/total must be specified together, channel ranges validated, MAC format validated
- `--dry-run` option to generate binary without flashing
- Fallback CSV generation when NVS binary generation fails, with manual flash instructions
- Password masked in output: `"WiFi Password: ****"`
- Multiple NVS generator discovery methods (Python module, ESP-IDF bundled script)

**Issues Found:**
- No auto-detection of serial port. The `--port` is required, but users may not know which port their ESP32 is on. A `--port auto` option using `serial.tools.list_ports` would help
- No verification step after flashing to confirm the provisioned values were written correctly
- Error when `esptool` or `nvs_partition_gen` is not installed is a raw Python exception. A friendlier message like `"Required tool 'esptool' not found. Install with: pip install esptool"` would be better
- The script name is `provision.py` but it is invoked as `python firmware/esp32-csi-node/provision.py`, which is a long path. A CLI subcommand like `wifi-densepose hw provision` would integrate better
- 22 command-line arguments is overwhelming; grouped parameter presets (e.g., `--profile basic`, `--profile mesh`, `--profile edge`) would simplify common use cases
- No interactive mode for guided provisioning

### 8.2 Serial Monitoring (Score: 55/100)

**Issues Found:**
- Serial monitoring is done via `python -m serial.tools.miniterm COM7 115200`, which is a raw tool with no structured log parsing
- No custom monitoring tool that parses ESP32 output, highlights errors, or shows CSI data visualization
- No documentation on what serial output to expect during normal operation vs error conditions
- Baud rate (115200) must be known; no auto-baud detection

### 8.3 Firmware Update Process (Score: 60/100)

**Issues Found:**
- Firmware flashing uses `idf.py flash` which requires the full ESP-IDF toolchain
- No OTA (Over-The-Air) update workflow documented for field deployments
- The `ota_data_initial.bin` is listed in the release process but OTA update instructions are not provided
- No firmware version reporting from the device to verify the update was successful
- 8MB and 4MB builds require different `sdkconfig.defaults` files with manual copying

---

## 9. Cross-Cutting Quality Concerns

### 9.1 Error Handling Quality Across Touchpoints (Score: 73/100)

| Touchpoint | Error Format | User Guidance | Recovery Path |
|------------|-------------|---------------|---------------|
| API REST | Structured JSON with code, message, request_id | No documentation links | Retry logic needed by client |
| API WebSocket | JSON `{ type: "error", message: "..." }` | Lists valid message types: No | Reconnect |
| CLI | Logger output to stderr | No remediation suggestions | Exit code 1 |
| Mobile | `ErrorBoundary` with retry, `ConnectionBanner` | Raw error messages | Retry button, reconnect |
| Provisioning | Python exceptions | Fallback CSV on failure | Manual flash instructions |

**Key Gap**: Error message styles differ between API (structured JSON) and CLI (logger strings). A unified error taxonomy would improve consistency.

### 9.2 Feedback Loops (Score: 72/100)

| Action | Feedback Mechanism | Timeliness | Quality |
|--------|-------------------|------------|---------|
| API request | HTTP status + response body | Immediate | Good |
| WebSocket connect | `connection_established` message | Immediate | Good |
| CLI start | Log messages to stdout | Real-time | Adequate |
| CLI stop | "Server stopped gracefully" | After completion | Good |
| Calibration start | Returns `calibration_id` and `estimated_duration_minutes` | Immediate | Incomplete (no progress stream) |
| Mobile connect | Banner color change | ~1s delay | Good |
| Firmware flash | `print()` statements | Real-time | Adequate |
| Settings save | No confirmation | Silent | Poor |

### 9.3 Recovery Paths (Score: 68/100)

| Failure Scenario | Recovery Path | Automated? | Documentation |
|-----------------|---------------|------------|---------------|
| Database connection fails | SQLite failsafe fallback | Yes | `config failsafe` command |
| Redis unavailable | Continues without Redis, logs warning | Yes | Mentioned in startup output |
| WebSocket disconnects | Exponential backoff reconnection, simulation fallback | Yes | Not documented |
| Stale PID file | Detected and cleaned up on `start`/`stop` | Yes | Not documented |
| API server crash | No automatic restart | No | No systemd/supervisor config |
| Mobile app crash | `ErrorBoundary` with retry | Partial | Not documented |
| Firmware flash fails | Fallback CSV with manual instructions | Partial | Inline help |
| Calibration fails | No documented recovery | No | Not documented |

### 9.4 Accessibility (Score: 45/100)

**Issues Found:**
- Mobile app uses hardcoded hex colors throughout (e.g., `'#0F141E'`, `'#0F6B2A'`, `'#8A1E2A'`) with no high-contrast mode support
- No `accessibilityLabel` or `accessibilityRole` props on interactive components in the mobile app
- `ConnectionBanner` relies on color alone to distinguish states (green/amber/red). The text labels (`LIVE STREAM`, `SIMULATED DATA`, `DISCONNECTED`) help, but there is no screen reader announcement on state change
- CLI status output uses emoji (checkmarks, X marks, weather symbols) as semantic indicators with no text-only fallback
- API documentation (when available) has no known accessibility testing
- No ARIA landmarks or roles in the sensing server web UI (if any)
- Font sizes are fixed in the mobile theme with no dynamic type/accessibility sizing support

---

## 10. Oracle Problems Detected

### Oracle Problem 1 (HIGH): Production API Documentation vs Security

**Type**: User Need vs Business Need Conflict

- **User Need**: API consumers need documentation to discover and integrate with endpoints
- **Business Need**: Hiding Swagger/ReDoc in production reduces attack surface
- **Conflict**: Disabling docs entirely (`docs_url=None` when `is_production=True`) leaves production API consumers without any discoverability mechanism

**Failure Modes:**
1. Developers working against production endpoints cannot discover available APIs
2. Third-party integrators have no self-service documentation
3. Internal teams must maintain separate documentation that can drift from the actual API

**Resolution Options:**
| Option | User Score | Security Score | Recommendation |
|--------|-----------|---------------|----------------|
| Keep docs disabled | 20 | 95 | Current state |
| Auth-gated docs endpoint | 85 | 80 | Recommended |
| Separate docs site from OpenAPI spec export | 90 | 90 | Best but more effort |
| Rate-limited docs with no auth | 70 | 60 | Compromise |

### Oracle Problem 2 (MEDIUM): Simulation Fallback vs Data Integrity

**Type**: User Experience vs Data Accuracy Conflict

- **User Need**: The app should always show something; blank screens feel broken
- **Business Need**: Users should know when they are seeing real vs simulated data
- **Conflict**: Automatic simulation fallback means users may not realize they lost their real data feed

**Failure Modes:**
1. Operator monitors "activity" that is actually simulated, missing real events
2. MAT (Mass Casualty Assessment) screen shows simulated survivor data during a real incident
3. Vitals screen displays simulated breathing/heart rate data, creating false confidence

**Resolution Options:**
| Option | UX Score | Safety Score | Recommendation |
|--------|---------|-------------|----------------|
| Current: auto-simulate with banner | 80 | 50 | Risky for safety-critical screens |
| Disable simulation on MAT/Vitals screens | 60 | 85 | Recommended |
| Prominent modal overlay for simulated mode | 70 | 80 | Good compromise |
| Require user confirmation to enter simulation | 55 | 90 | Safest |

### Oracle Problem 3 (MEDIUM): WebSocket Path Mismatch

**Type**: Missing Information / Implementation Inconsistency

- **Evidence**: The mobile app's `ws.service.ts` constructs the WebSocket URL as `/ws/sensing` (line 104), while `constants/websocket.ts` defines `WS_PATH = '/api/v1/stream/pose'`. The API server serves WebSocket on `/api/v1/stream/pose` (stream router). These paths do not match.
- **Impact**: The actual connection behavior depends on which path the sensing server uses (the lightweight Axum server may use `/ws/sensing`), but the inconsistency creates confusion and potential silent connection failures
- **Resolution**: Align the WebSocket paths across the mobile app and server, or make the path configurable

---

## 11. Prioritized Recommendations

### Priority 1 -- Critical (address before next release)

| # | Recommendation | Effort | Impact | Persona |
|---|---------------|--------|--------|---------|
| 1.1 | Add auth-gated API documentation endpoint for production | Low | High | Developer, Operator |
| 1.2 | Resolve WebSocket path mismatch between `ws.service.ts` and `constants/websocket.ts` | Low | High | End-User |
| 1.3 | Disable automatic simulation fallback on MAT screen (safety-critical) | Low | High | End-User, Operator |
| 1.4 | Fix `MainTabs.tsx` inline arrow function causing unnecessary re-renders (line 130) | Low | Medium | End-User |
| 1.5 | Include structured error body in 429 rate limit responses using `ErrorResponse` format | Low | Medium | Developer |

### Priority 2 -- High (next sprint)

| # | Recommendation | Effort | Impact | Persona |
|---|---------------|--------|--------|---------|
| 2.1 | Add `wifi-densepose init` command to scaffold default configuration | Medium | High | Operator |
| 2.2 | Change default mobile `serverUrl` from `localhost:3000` to empty string with first-run setup prompt | Medium | High | End-User |
| 2.3 | Add terminal capability detection to CLI for emoji/unicode fallback | Medium | Medium | Operator |
| 2.4 | Add calibration progress WebSocket stream or polling endpoint with step-by-step updates | Medium | Medium | Operator, Developer |
| 2.5 | Create a `CONTRIBUTING.md` with quickstart for each codebase | Medium | High | Developer |
| 2.6 | Map `ErrorBoundary` error messages to user-friendly strings | Low | Medium | End-User |
| 2.7 | Add loading timeout to LiveScreen WebView initialization | Low | Medium | End-User |

### Priority 3 -- Medium (next quarter)

| # | Recommendation | Effort | Impact | Persona |
|---|---------------|--------|--------|---------|
| 3.1 | Create unified `Makefile` or `Taskfile` for cross-codebase builds and tests | High | High | Developer |
| 3.2 | Add `--port auto` to provisioning script with serial port auto-detection | Medium | Medium | Operator |
| 3.3 | Add accessibility labels to mobile app interactive components | Medium | Medium | End-User |
| 3.4 | Create architecture diagram showing component interactions | Medium | High | Developer |
| 3.5 | Add `.env.example` file documenting all environment variables | Low | Medium | Developer, Operator |
| 3.6 | Implement `wifi-densepose doctor` for self-diagnosis | High | Medium | Operator |
| 3.7 | Add `wifi-densepose logs` command with filtering and formatting | Medium | Medium | Operator |
| 3.8 | Persist `poseStore` RSSI history for post-restart analysis | Medium | Low | End-User |
| 3.9 | Add provisioning parameter presets (`--profile basic/mesh/edge`) | Medium | Medium | Operator |
| 3.10 | Authenticate WebSocket before `websocket.accept()` | Low | Low | Developer |

---

## 12. Heuristic Scoring Summary

### Problem Analysis (H1)

| Heuristic | Score | Finding |
|-----------|-------|---------|
| H1.1: Understand the Problem | 75/100 | The system addresses WiFi-based pose estimation well but the quality experience varies significantly across touchpoints. The core problem (sensing and display) is well-solved; the surrounding experience (setup, configuration, debugging) needs work. |
| H1.2: Identify Stakeholders | 70/100 | Three personas (developer, operator, end-user) are implicitly served but not explicitly designed for. The mobile app targets end-users well; the CLI targets operators adequately; developer experience is the weakest. |
| H1.3: Define Quality Criteria | 65/100 | Health checks define "healthy/degraded/unhealthy" but no SLA or quality thresholds are documented. Rate limits are configurable but default values are not justified. |
| H1.4: Map Failure Modes | 72/100 | Database failsafe, Redis degradation, and WebSocket reconnection cover major failure modes. Missing: calibration failure recovery, firmware flash failure recovery, mobile app state corruption. |

### User Needs (H2)

| Heuristic | Score | Finding |
|-----------|-------|---------|
| H2.1: Task Completion | 78/100 | Core tasks (view live data, check vitals, manage zones) are completable. Setup tasks (install, configure, provision) have friction. |
| H2.2: Error Recovery | 68/100 | Some automated recovery (database failsafe, WebSocket reconnect). Missing recovery paths for calibration failure and firmware issues. |
| H2.3: Learning Curve | 60/100 | Steep onboarding across four codebases. No quickstart guide. Mobile app is the most intuitive touchpoint. |
| H2.4: Feedback Clarity | 72/100 | API provides structured feedback. CLI provides log-style feedback. Mobile provides visual feedback. Calibration progress is the biggest gap. |
| H2.5: Consistency | 70/100 | Error formats differ between API (JSON) and CLI (logger). Mobile is internally consistent. Naming conventions mostly aligned. |

### Business Needs (H3)

| Heuristic | Score | Finding |
|-----------|-------|---------|
| H3.1: Reliability | 76/100 | Health checks, failsafes, and reconnection strategies demonstrate reliability focus. No documented SLAs or uptime targets. |
| H3.2: Security Posture | 72/100 | Authentication framework exists but JWT validation is not implemented. Rate limiting is configurable. Production docs are hidden. Secrets redacted in config output. |
| H3.3: Scalability | 68/100 | Multi-worker support, WebSocket connection management, per-endpoint rate limiting. No load testing results or capacity planning documented. |
| H3.4: Maintainability | 74/100 | Well-separated crates, clear module boundaries, typed interfaces. Pre-merge checklist ensures documentation updates. ADR process is mature. |

### Balance (H4)

| Heuristic | Score | Finding |
|-----------|-------|---------|
| H4.1: UX vs Security | 65/100 | Production API docs disabled for security, but no alternative provided. Authentication errors are informative without leaking implementation details. |
| H4.2: Simplicity vs Capability | 68/100 | Provisioning script has 22 parameters. CLI has good grouping but missing convenience features. API has comprehensive endpoints. |
| H4.3: Consistency vs Flexibility | 72/100 | Error handling is structured but not uniform across touchpoints. Settings are flexible (env vars + config file + CLI flags). |

### Impact (H5)

| Heuristic | Score | Finding |
|-----------|-------|---------|
| H5.1: Visible Impact (GUI/UX) | 76/100 | Mobile app provides clear visual states. CLI status output is detailed. API responses are informative. |
| H5.2: Invisible Impact (Performance) | 70/100 | `cpu_percent(interval=1)` in health check blocks for 1 second per request. Rate limiting uses async locks correctly. RingBuffer prevents memory growth. |
| H5.3: Safety Impact | 62/100 | MAT screen auto-simulation is a safety concern. Simulated vitals data could mislead operators. No data provenance indicator beyond the connection banner. |
| H5.4: Data Integrity | 72/100 | Pydantic validation on all inputs. Zone ID existence checks. Time range validation on historical queries. Deterministic proof verification for core pipeline. |

### Creativity (H6)

| Heuristic | Score | Finding |
|-----------|-------|---------|
| H6.1: Novel Testing Approaches | 68/100 | Witness bundle verification is creative. Deterministic proof with SHA-256 is strong. No mutation testing or property-based testing. |
| H6.2: Alternative Perspectives | 65/100 | The simulation fallback is creative but creates oracle problems. Database failsafe is a pragmatic solution. |
| H6.3: Cross-Domain Insights | 70/100 | WiFi CSI for pose estimation is inherently cross-domain (RF + computer vision + IoT). The mobile app's GaussianSplat visualization is innovative. |

---

## Methodology

This Quality Experience analysis was performed by examining source code across all touchpoints of the WiFi-DensePose system. Files analyzed include:

**API Layer (9 files):**
- `archive/v1/src/api/main.py` -- FastAPI application setup, middleware configuration, exception handlers
- `archive/v1/src/api/routers/health.py` -- Health check endpoints
- `archive/v1/src/api/routers/pose.py` -- Pose estimation endpoints
- `archive/v1/src/api/routers/stream.py` -- WebSocket streaming endpoints
- `archive/v1/src/api/websocket/connection_manager.py` -- WebSocket connection lifecycle
- `archive/v1/src/api/dependencies.py` -- Dependency injection, authentication, authorization
- `archive/v1/src/middleware/error_handler.py` -- Error handling middleware
- `archive/v1/src/middleware/rate_limit.py` -- Rate limiting middleware

**CLI Layer (4 files):**
- `archive/v1/src/cli.py` -- Click CLI entry point
- `archive/v1/src/commands/start.py` -- Server start command
- `archive/v1/src/commands/stop.py` -- Server stop command
- `archive/v1/src/commands/status.py` -- Server status command

**Mobile Layer (15 files):**
- `ui/mobile/src/screens/LiveScreen/index.tsx` -- Live visualization screen
- `ui/mobile/src/screens/VitalsScreen/index.tsx` -- Vitals monitoring screen
- `ui/mobile/src/screens/ZonesScreen/index.tsx` -- Zone occupancy screen
- `ui/mobile/src/screens/MATScreen/index.tsx` -- Mass casualty assessment screen
- `ui/mobile/src/screens/SettingsScreen/index.tsx` -- Settings screen
- `ui/mobile/src/screens/SettingsScreen/ServerUrlInput.tsx` -- Server URL configuration
- `ui/mobile/src/navigation/MainTabs.tsx` -- Tab navigation
- `ui/mobile/src/components/ErrorBoundary.tsx` -- Error boundary
- `ui/mobile/src/components/ConnectionBanner.tsx` -- Connection status banner
- `ui/mobile/src/components/LoadingSpinner.tsx` -- Loading indicator
- `ui/mobile/src/services/ws.service.ts` -- WebSocket service
- `ui/mobile/src/services/api.service.ts` -- HTTP API service
- `ui/mobile/src/stores/poseStore.ts` -- Real-time data store
- `ui/mobile/src/stores/settingsStore.ts` -- Persisted settings store
- `ui/mobile/src/utils/urlValidator.ts` -- URL validation
- `ui/mobile/src/hooks/usePoseStream.ts` -- Pose data stream hook
- `ui/mobile/src/constants/websocket.ts` -- WebSocket constants

**Hardware Layer (1 file):**
- `firmware/esp32-csi-node/provision.py` -- ESP32 provisioning script

The analysis applied 23 QX heuristics across 6 categories (Problem Analysis, User Needs, Business Needs, Balance, Impact, Creativity) and identified 3 oracle problems where quality criteria conflict across stakeholders.
