# ADR-053: UI Design System — Dark Professional + Unity-Inspired Interface

| Field | Value |
|-------|-------|
| Status | Accepted |
| Date | 2026-03-06 |
| Deciders | ruv |
| Depends on | ADR-052 (Tauri Desktop Frontend) |

## Context

RuView Desktop (ADR-052) needs a UI design system that communicates precision and control — befitting a hardware management control plane for embedded sensing infrastructure. The interface must handle dense data (CSI heatmaps, node registries, log streams, mesh topologies) without feeling overwhelming, while remaining usable by both engineers and field operators.

Two design inspirations:

1. **Data-first professional tools** — Dense information displays where data speaks for itself. Clean typography, structured layouts, and deliberate use of color for status. The interface shows what matters and hides what doesn't. Think: network monitoring dashboards, embedded systems IDEs, infrastructure control panels.

2. **Unity Editor** — Dockable panel system, inspector/hierarchy/scene separation, property grids, dark professional theme, and dense-but-organized data display. Unity's UI is purpose-built for managing complex real-time systems — exactly what RuView needs.

The combination yields a professional control panel for WiFi sensing infrastructure. Data is organized into scannable panels with clear hierarchy. Status is communicated through consistent color coding. The layout adapts from high-level overview down to individual node details through progressive disclosure.

## Decision

### Design Principles

1. **Data is the interface** — The system reveals patterns through visualization, not through explanation. Every pixel earns its place.
2. **Precision typography** — Typography is clean and authoritative. Technical values are displayed without ambiguity. Labels are concise.
3. **Panel-based layout** — Dockable regions inspired by Unity's panel system. The operator can see the entire mesh at a glance, then drill into any node.
4. **Status through color** — Deliberate color coding: green (online), amber (degraded), red (offline/failed), blue (scanning/new). No gratuitous color.
5. **Progressive disclosure** — Dashboard shows the overview. Clicking a node reveals its details. Summary first, detail on interaction.
6. **Dual typography** — Monospace for all technical values (MAC addresses, firmware versions, CSI amplitudes). Sans-serif for labels and descriptions. The contrast signals "data vs. context."
7. **Powered by rUv** — Subtle branding: footer tagline, about dialog, splash screen.

### Color System

```css
:root {
  /* Background layers */
  --bg-base:        #0d1117;     /* App background */
  --bg-surface:     #161b22;     /* Panel backgrounds */
  --bg-elevated:    #1c2333;     /* Cards, modals, dropdowns */
  --bg-hover:       #242d3d;     /* Hover state */
  --bg-active:      #2d3748;     /* Active/selected state */

  /* Text hierarchy */
  --text-primary:   #e6edf3;     /* Headings, primary content */
  --text-secondary: #8b949e;     /* Labels, descriptions */
  --text-muted:     #484f58;     /* Disabled, hints, placeholders */

  /* Status indicators */
  --status-online:  #3fb950;     /* Node online, healthy */
  --status-warning: #d29922;     /* Degraded, needs attention */
  --status-error:   #f85149;     /* Offline, failed, critical */
  --status-info:    #58a6ff;     /* Scanning, discovering, info */

  /* Accent */
  --accent:         #7c3aed;     /* rUv purple — primary actions */
  --accent-hover:   #6d28d9;

  /* Borders */
  --border:         #30363d;
  --border-active:  #58a6ff;

  /* Data display */
  --font-mono:      'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
  --font-sans:      'Inter', -apple-system, BlinkMacSystemFont, sans-serif;
}
```

### Typography Scale

```css
/* Typographic hierarchy */
.heading-xl   { font: 600 28px/1.2 var(--font-sans); }   /* Page titles */
.heading-lg   { font: 600 20px/1.3 var(--font-sans); }   /* Section titles */
.heading-md   { font: 600 16px/1.4 var(--font-sans); }   /* Card titles */
.heading-sm   { font: 600 13px/1.4 var(--font-sans); }   /* Panel labels */
.body         { font: 400 14px/1.6 var(--font-sans); }   /* Body text */
.body-sm      { font: 400 12px/1.5 var(--font-sans); }   /* Captions */
.data         { font: 400 13px/1.4 var(--font-mono); }   /* Technical values */
.data-lg      { font: 500 18px/1.2 var(--font-mono); }   /* Key metrics */
```

### Layout System

Three-region layout: navigation sidebar, node list, and detail inspector. Unity's docking system provides the mechanical framework.

```
+--[ Sidebar ]--+--[ Main ]-------------------------------------+
|               |                                                 |
| [Nav Items]   |  +--[ Command Bar ]---------------------------+ |
|               |  | Breadcrumb    | Actions | Search           | |
| Dashboard     |  +-------+-----------------------------------+ |
| Nodes         |  |       |                                   | |
| Flash         |  | Node  |  Detail Inspector                 | |
| OTA           |  | List  |  (selected node properties)       | |
| Edge Modules  |  |       |                                   | |
| Sensing       |  |       |  [Property Grid]                  | |
| Mesh View     |  |       |  [Status Indicators]              | |
| Settings      |  |       |  [Action Buttons]                 | |
|               |  |       |                                   | |
+-[ Status Bar ]+--+-------+-----------------------------------+ |
| rUv | 3 nodes online | Server: running | Port: 8080           |
+---------------------------------------------------------------+
```

**Panel behaviors:**
- Sidebar collapses to icon-only on narrow windows
- Node List / Inspector split is resizable via drag handle
- Inspector scrolls independently — drill into any node without losing the list
- Status Bar shows global system state at a glance (node count, server status, port)

### Component Library

#### 1. NodeCard

```
+-- NodeCard -----------------------------------------------+
|  [●] ESP32-S3 Node #2              firmware: 0.3.1       |
|  MAC: AA:BB:CC:DD:EE:FF            TDM Slot: 2/4        |
|  IP:  192.168.1.42                  Edge Tier: 1          |
|  Last seen: 3s ago                  [Flash] [OTA] [···]  |
+-----------------------------------------------------------+
```

Status dot uses `--status-online/warning/error`. Card background shifts on hover.

#### 2. FlashProgress

```
+-- Flash Progress -----------------------------------------+
|  Flashing firmware to COM3 (ESP32-S3)                     |
|                                                           |
|  Phase: Writing                                           |
|  [████████████████████░░░░░░░░░░]  67.3%                 |
|  412 KB / 612 KB  •  38.2 KB/s  •  ~5s remaining        |
+-----------------------------------------------------------+
```

Progress bar uses `--accent` fill with subtle pulse animation during active writes.

#### 3. Mesh Topology View (Three.js)

Interactive 3D visualization of the sensing network. Each node is a sphere. Edges are lines representing signal paths. The coordinator node is visually distinct (larger, outlined ring). Built with **Three.js**, consistent with the existing visualization stack in `ui/observatory/js/` and `ui/components/`.

```
+-- Mesh Topology ------------------------------------------+
|                                                           |
|         [Node 0]----[Node 1]                              |
|            |    \   /   |                                 |
|            | [Coordinator] |   Coordinator = TDM master    |
|            |    /   \   |                                 |
|         [Node 2]----[Node 3]                              |
|                                                           |
|  Drift: ±0.3ms  |  Cycle: 50ms  |  4/4 nodes online     |
+-----------------------------------------------------------+
```

**Three.js implementation details:**
- Force-directed layout computed on CPU, rendered as `THREE.Group` with `THREE.Mesh` (spheres) and `THREE.Line` (edges)
- Node spheres use `THREE.MeshPhongMaterial` with emissive color matching `--status-online/warning/error`
- Edge lines use `THREE.LineBasicMaterial` with opacity mapped to signal strength
- Coordinator node rendered with `THREE.RingGeometry` outline
- Camera: `OrbitControls` for pan/zoom/rotate, reset button returns to default view
- Follows existing patterns: `BufferGeometry` + `BufferAttribute` for dynamic updates (see `ui/observatory/js/subcarrier-manifold.js`)
- Raycasting for node click → opens detail in Inspector panel
- Real-time updates as nodes join, leave, or change status — geometry attributes updated per frame

#### 4. PropertyGrid (Unity Inspector-style)

```
+-- Node Inspector -----------------------------------------+
|  General                                            [▼]  |
|    MAC Address      AA:BB:CC:DD:EE:FF                    |
|    IP Address       192.168.1.42                         |
|    Firmware         0.3.1                                |
|    Chip             ESP32-S3                             |
|  TDM Configuration                                 [▼]  |
|    Slot Index       2                                    |
|    Total Nodes      4                                    |
|    Cycle Period     50 ms                                |
|    Sync Drift       +0.12 ms                             |
|  WASM Modules                                      [▼]  |
|    [0] activity_detect  running    12.4 KB    83 us/f    |
|    [1] vital_monitor    stopped     8.1 KB     — us/f   |
+-----------------------------------------------------------+
```

Collapsible sections with alternating row backgrounds for scanability.

#### 5. StatusBadge

```
[● Online]    [◐ Degraded]    [○ Offline]    [↻ Updating]
```

Small inline badges with status dot, label, and optional tooltip.

#### 6. LogViewer

```
+-- Server Log (auto-scroll) -----------[ Clear ] [ ⏸ ]---+
| 19:42:01.234 INFO  sensing-server  HTTP on 127.0.0.1:8080|
| 19:42:01.235 INFO  sensing-server  WS on 127.0.0.1:8765  |
| 19:42:01.890 INFO  udp_receiver    CSI frame from .42    |
| 19:42:02.003 WARN  vital_signs     Low signal quality    |
+-----------------------------------------------------------+
```

Monospace, color-coded by log level (INFO=text, WARN=amber, ERROR=red). Virtual scrolling for performance.

### Spacing and Grid

```css
/* 4px base grid */
--space-1: 4px;    /* Tight spacing (within components) */
--space-2: 8px;    /* Component internal padding */
--space-3: 12px;   /* Between related elements */
--space-4: 16px;   /* Card padding, section gaps */
--space-5: 24px;   /* Between sections */
--space-6: 32px;   /* Page-level spacing */
--space-8: 48px;   /* Major section breaks */

/* Panel dimensions */
--sidebar-width: 220px;
--sidebar-collapsed: 52px;
--statusbar-height: 28px;
--toolbar-height: 44px;
```

### Animations

Minimal and purposeful:
- Panel collapse/expand: 200ms ease-out
- Node card health transition: 300ms (color fade, not flash)
- Progress bar fill: smooth 60fps CSS transition
- Mesh graph: Three.js render loop at 60fps, force simulation on requestAnimationFrame
- No loading spinners — use skeleton placeholders instead

### Branding

- **Splash screen**: rUv logo + "RuView Desktop" + version, 1.5s duration
- **Status bar**: "Powered by rUv" in `--text-muted`, left-aligned
- **About dialog**: rUv logo, version, license, links to GitHub and docs
- **App icon**: Stylized WiFi signal + human silhouette in rUv purple (#7c3aed)

## Consequences

### Positive

- Professional, data-dense UI suitable for hardware management
- Consistent design language across all 7 pages
- Dual typography (mono + sans-serif) ensures readability at all information densities
- Unity-inspired panels feel natural to engineers familiar with IDE/editor tools
- Dark theme reduces eye strain for extended monitoring sessions

### Negative

- Custom design system means no off-the-shelf component library (shadcn/ui partially usable)
- Dockable panels add complexity to the layout system
- Dark-only theme may not suit all users (could add light mode later)

### Neutral

- The design system is CSS-only with React components — no heavy UI framework dependency
- Component library can be extracted as a separate package if other rUv projects need it

## References

- ADR-052: Tauri Desktop Frontend
- Unity Editor UI Guidelines: https://docs.unity3d.com/Manual/UIE-USS.html
- Three.js (existing project dependency): `ui/observatory/js/`, `ui/components/`
- Inter font: https://rsms.me/inter/
- JetBrains Mono: https://www.jetbrains.com/lp/mono/
