# ADR-009: Visualization and User Interface

## Status
Proposed

## Date
2026-01-15

## Context

7sense is a bioacoustics platform integrating Perch 2.0 embeddings (1536-D vectors from 5-second audio segments at 32kHz) with RuVector for HNSW-indexed vector search, GNN-enhanced retrieval, and Retrieval-Augmented Bioacoustics (RAB) interpretation. The platform needs visualization capabilities to:

1. **Explore embedding spaces** - Researchers need to understand how acoustic signatures cluster and relate
2. **Navigate neighbor graphs** - HNSW retrieval results must be interpretable with evidence attribution
3. **Analyze temporal sequences** - Call patterns, motifs, and transitions require visual trajectory analysis
4. **Present evidence packs** - RAB outputs demand transparent, citation-backed visual displays
5. **Support cross-platform deployment** - Rust-native desktop and WebAssembly browser targets

The visualization layer is critical for achieving the RAB goal of "interpretable, evidence-backed, testable" outputs rather than opaque model predictions.

## Decision

We will implement a multi-layer visualization architecture using Rust-native components with WebAssembly browser support, following these design principles:

### 1. Visualization Types

#### 1.1 Embedding Space Explorer (2D UMAP Projection)

**Purpose**: Provide navigable "acoustic cartography" where neighborhoods are meaningful.

**Implementation**:
```rust
pub struct EmbeddingExplorer {
    /// UMAP projection of 1536-D embeddings to 2D
    projection: UmapProjection,
    /// Original high-dimensional embeddings for distance calculations
    embeddings: Vec<Embedding>,
    /// Metadata for each point (segment_id, recording_id, timestamps)
    metadata: Vec<SegmentMetadata>,
    /// Current viewport bounds
    viewport: Viewport2D,
    /// Selection state
    selection: SelectionState,
}

pub struct UmapProjection {
    /// 2D coordinates for each embedding
    coordinates: Vec<[f32; 2]>,
    /// Parameters used for projection
    params: UmapParams,
    /// Precomputed kNN graph from HNSW (fed to UMAP)
    knn_graph: KnnGraph,
}

pub struct UmapParams {
    n_neighbors: usize,      // Default: 15
    min_dist: f32,           // Default: 0.1
    metric: DistanceMetric,  // Cosine for Perch embeddings
    spread: f32,             // Default: 1.0
}
```

**Features**:
- Dynamic re-projection with different parameters
- Color encoding by cluster, species, time, or metadata
- Point size encoding by confidence, SNR, or energy
- Zoom levels with progressive detail (overview -> detail)
- Lasso selection for bulk operations

#### 1.2 Neighbor Graph Viewer

**Purpose**: Visualize HNSW retrieval results and GNN-enhanced neighbor relationships.

**Implementation**:
```rust
pub struct NeighborGraphView {
    /// Graph structure from HNSW SIMILAR edges
    graph: NeighborGraph,
    /// Layout algorithm state
    layout: ForceDirectedLayout,
    /// Edge rendering options
    edge_style: EdgeStyle,
    /// Node rendering options
    node_style: NodeStyle,
}

pub struct NeighborGraph {
    /// Nodes representing CallSegments
    nodes: Vec<GraphNode>,
    /// SIMILAR edges with distance weights
    similarity_edges: Vec<SimilarityEdge>,
    /// NEXT edges for temporal sequence
    temporal_edges: Vec<TemporalEdge>,
    /// Optional co-occurrence edges
    cooccurrence_edges: Vec<CooccurrenceEdge>,
}

pub struct GraphNode {
    segment_id: SegmentId,
    embedding_id: EmbeddingId,
    position: [f32; 2],
    cluster_id: Option<ClusterId>,
    prototype_distance: Option<f32>,
}

pub struct SimilarityEdge {
    source: NodeIndex,
    target: NodeIndex,
    distance: f32,           // Cosine distance
    gnn_reranked: bool,      // Whether GNN modified this edge
    rerank_score: Option<f32>,
}
```

**Features**:
- Force-directed layout optimized for cluster visibility
- Edge thickness/opacity based on similarity strength
- Different edge colors for similarity vs temporal vs co-occurrence
- Node highlighting on hover with neighbor expansion
- GNN reranking visualization (before/after toggle)

#### 1.3 Sequence Trajectory Visualization

**Purpose**: Display temporal patterns, motifs, and call sequences.

**Implementation**:
```rust
pub struct TrajectoryView {
    /// Sequence of segments in temporal order
    sequence: Vec<SequenceSegment>,
    /// Detected motifs (repeated patterns)
    motifs: Vec<Motif>,
    /// Transition probabilities between clusters
    transitions: TransitionMatrix,
    /// Timeline display mode
    display_mode: TrajectoryDisplayMode,
}

pub struct SequenceSegment {
    segment_id: SegmentId,
    timestamp_ms: u64,
    embedding_2d: [f32; 2],    // Position in UMAP space
    cluster_id: Option<ClusterId>,
    call_type: Option<String>,
}

pub struct Motif {
    motif_id: MotifId,
    /// Segment IDs comprising this motif instance
    segments: Vec<SegmentId>,
    /// Number of occurrences in corpus
    occurrence_count: usize,
    /// Entropy rate (lower = more predictable)
    entropy_rate: f32,
    /// Representative prototype
    prototype_embedding: Vec<f32>,
}

pub enum TrajectoryDisplayMode {
    /// Timeline with embedding Y-axis
    Timeline,
    /// Path through UMAP space
    SpatialPath,
    /// Sankey diagram of cluster transitions
    TransitionFlow,
    /// Animated playback
    Animation { speed: f32 },
}
```

**Features**:
- Animated trajectory playback through embedding space
- Motif highlighting with occurrence markers
- Transition probability heatmaps
- Entropy rate visualization by time/location
- Comparison mode for multiple recordings

#### 1.4 Cluster Hierarchy Visualization

**Purpose**: Display hierarchical clustering results (dendrograms, radial trees).

**Implementation**:
```rust
pub struct ClusterHierarchyView {
    /// Hierarchical cluster structure
    hierarchy: ClusterHierarchy,
    /// Display format
    format: HierarchyFormat,
    /// Expansion state for interactive drilling
    expansion_state: HashMap<ClusterId, bool>,
}

pub struct ClusterHierarchy {
    /// Root cluster containing all data
    root: ClusterNode,
    /// Linkage method used
    linkage: LinkageMethod,
    /// Cut threshold for current cluster count
    cut_threshold: f32,
}

pub struct ClusterNode {
    cluster_id: ClusterId,
    /// Child clusters (empty for leaf nodes)
    children: Vec<ClusterNode>,
    /// Prototype embedding (centroid)
    prototype: Prototype,
    /// Member segments (for leaf nodes)
    members: Vec<SegmentId>,
    /// Merge distance in hierarchy
    merge_distance: f32,
    /// Statistics
    stats: ClusterStats,
}

pub enum HierarchyFormat {
    /// Traditional dendrogram (horizontal or vertical)
    Dendrogram { orientation: Orientation },
    /// Radial tree layout
    RadialTree,
    /// Sunburst chart
    Sunburst,
    /// Treemap
    Treemap,
    /// Icicle plot
    Icicle,
}
```

**Features**:
- Interactive drill-down into subclusters
- Prototype audio playback at each level
- Member count and purity indicators
- Cut-level slider for dynamic cluster granularity
- Export cluster assignments

#### 1.5 Spectrogram Overlay System

**Purpose**: Connect abstract embeddings to interpretable acoustic representations.

**Implementation**:
```rust
pub struct SpectrogramOverlay {
    /// Cache of rendered spectrograms
    cache: SpectrogramCache,
    /// Rendering parameters
    params: SpectrogramParams,
    /// Overlay display mode
    mode: OverlayMode,
}

pub struct SpectrogramCache {
    /// LRU cache of rendered images
    cache: LruCache<SegmentId, SpectrogramImage>,
    /// Maximum cache size in bytes
    max_size_bytes: usize,
    /// Thumbnail vs full resolution
    resolution_levels: Vec<ResolutionLevel>,
}

pub struct SpectrogramParams {
    /// FFT window size (default: 2048 for 32kHz)
    n_fft: usize,
    /// Hop length (default: 512)
    hop_length: usize,
    /// Mel bins (default: 128, matching Perch input)
    n_mels: usize,
    /// Frequency range (default: 60-16000 Hz)
    fmin: f32,
    fmax: f32,
    /// Color map
    colormap: Colormap,
    /// dB range for normalization
    db_range: (f32, f32),
}

pub enum OverlayMode {
    /// Show on hover
    Hover { delay_ms: u32 },
    /// Show on click
    Click,
    /// Always show for selected points
    Selected,
    /// Grid view of multiple spectrograms
    Grid { columns: usize },
    /// Side panel comparison
    Comparison,
}
```

**Features**:
- Progressive loading (thumbnail -> full resolution)
- Synchronized hover across all views
- Comparison panels for neighbor spectrograms
- Annotation overlay (detected features, time markers)
- Export and download individual spectrograms

### 2. Technology Stack

#### 2.1 Rust Core Components

| Crate | Purpose | Version |
|-------|---------|---------|
| `umap-rs` | 2D/3D projection from precomputed kNN | Latest |
| `plotly` | Rust bindings for chart generation | Latest |
| `image` | Spectrogram image generation | 0.24+ |
| `petgraph` | Graph data structures | 0.6+ |
| `serde` | Serialization for WASM boundary | 1.0+ |
| `wasm-bindgen` | JavaScript interop | 0.2+ |

```toml
# Cargo.toml visualization dependencies
[dependencies]
umap-rs = "0.2"
plotly = { version = "0.8", features = ["wasm"] }
image = "0.24"
petgraph = "0.6"
serde = { version = "1.0", features = ["derive"] }
wasm-bindgen = "0.2"
js-sys = "0.3"
web-sys = { version = "0.3", features = [
    "Document", "Element", "HtmlElement",
    "HtmlCanvasElement", "CanvasRenderingContext2d",
    "AudioContext", "AudioBuffer", "AudioBufferSourceNode"
]}

[target.'cfg(target_arch = "wasm32")'.dependencies]
getrandom = { version = "0.2", features = ["js"] }
```

#### 2.2 WebAssembly Browser Support

**Architecture**:
```
+-------------------+     +------------------+     +------------------+
|   Rust Core       |     |  WASM Bridge     |     |  Web Frontend    |
|   (sevensense-viz)  |---->|  (wasm-bindgen)  |---->|  (TypeScript)    |
+-------------------+     +------------------+     +------------------+
        |                         |                        |
   - UMAP projection         - Serialization          - Plotly.js
   - Graph algorithms        - Memory mgmt            - D3.js (graphs)
   - Audio processing        - Async handling         - Web Audio API
   - Spectrogram gen         - Event binding          - Canvas/WebGL
```

**WASM Module Exports**:
```rust
#[wasm_bindgen]
pub struct 7senseViz {
    explorer: EmbeddingExplorer,
    graph_view: NeighborGraphView,
    trajectory_view: TrajectoryView,
    spectrogram_cache: SpectrogramCache,
}

#[wasm_bindgen]
impl 7senseViz {
    #[wasm_bindgen(constructor)]
    pub fn new(config: JsValue) -> Result<7senseViz, JsValue>;

    /// Load embeddings from ArrayBuffer
    pub fn load_embeddings(&mut self, data: &[u8]) -> Result<(), JsValue>;

    /// Compute UMAP projection
    pub fn compute_projection(&mut self, params: JsValue) -> Result<JsValue, JsValue>;

    /// Get points in viewport for rendering
    pub fn get_visible_points(&self, viewport: JsValue) -> JsValue;

    /// Get spectrogram for segment
    pub fn get_spectrogram(&mut self, segment_id: &str) -> Result<Vec<u8>, JsValue>;

    /// Query neighbors for a point
    pub fn get_neighbors(&self, segment_id: &str, k: usize) -> JsValue;

    /// Get trajectory for time range
    pub fn get_trajectory(&self, start_ms: u64, end_ms: u64) -> JsValue;
}
```

#### 2.3 Web Rendering Layer

**Plotly.js** for primary charts:
- Scatter plots (embedding explorer)
- Heatmaps (transition matrices)
- Line charts (trajectories)
- 3D scatter (optional high-dimensional view)

**D3.js** for graph visualizations:
- Force-directed neighbor graphs
- Dendrograms and radial trees
- Sankey diagrams for transitions
- Custom interactive overlays

**Canvas/WebGL** for performance:
- Large point cloud rendering (>100k points)
- Real-time animation
- Spectrogram display

### 3. Interaction Patterns

#### 3.1 Click to Hear Audio

**Implementation**:
```typescript
interface AudioPlayer {
    audioContext: AudioContext;
    currentSource: AudioBufferSourceNode | null;

    // Play segment audio
    playSegment(segmentId: string): Promise<void>;

    // Play with context (before/after segments)
    playWithContext(segmentId: string, contextMs: number): Promise<void>;

    // Stop current playback
    stop(): void;

    // Loop playback
    setLoop(enabled: boolean): void;
}

// Click handler
async function onPointClick(point: EmbeddingPoint) {
    // Highlight point in all views
    highlightPoint(point.segmentId);

    // Load and play audio
    await audioPlayer.playSegment(point.segmentId);

    // Show spectrogram in detail panel
    showSpectrogramDetail(point.segmentId);

    // Update evidence pack display
    updateEvidencePack(point.segmentId);
}
```

#### 3.2 Hover for Spectrogram

**Implementation**:
```typescript
interface HoverHandler {
    hoverDelay: number;  // ms before showing (default: 200)
    currentHover: string | null;
    hoverTimeout: number | null;

    onMouseEnter(point: EmbeddingPoint): void;
    onMouseLeave(): void;
}

function onMouseEnter(point: EmbeddingPoint) {
    // Clear any pending hide
    if (this.hoverTimeout) clearTimeout(this.hoverTimeout);

    // Delay before showing
    this.hoverTimeout = setTimeout(async () => {
        // Get spectrogram from WASM module (cached)
        const spectrogram = await sevensenseViz.get_spectrogram(point.segmentId);

        // Show tooltip with spectrogram
        showSpectrogramTooltip({
            x: point.screenX,
            y: point.screenY,
            spectrogram: spectrogram,
            metadata: point.metadata
        });

        this.currentHover = point.segmentId;
    }, this.hoverDelay);
}
```

#### 3.3 Select to See Neighbors

**Implementation**:
```typescript
interface SelectionHandler {
    selectedPoints: Set<string>;
    selectionMode: 'single' | 'multi' | 'lasso';

    onSelect(points: EmbeddingPoint[]): void;
    clearSelection(): void;
}

async function onSelect(points: EmbeddingPoint[]) {
    // Update selection state
    this.selectedPoints = new Set(points.map(p => p.segmentId));

    // Get neighbors for all selected points
    const neighbors = await Promise.all(
        points.map(p => sevensenseViz.get_neighbors(p.segmentId, 10))
    );

    // Highlight neighbors in embedding view
    highlightNeighbors(neighbors.flat());

    // Update neighbor graph view
    updateNeighborGraph(points, neighbors);

    // Show comparison grid
    showComparisonGrid(points);

    // Build evidence pack for selection
    buildEvidencePack(points, neighbors);
}
```

#### 3.4 Filter by Metadata

**Implementation**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterState {
    /// Species/taxon filter
    species: Option<Vec<String>>,
    /// Geographic bounding box
    location: Option<BoundingBox>,
    /// Time range
    time_range: Option<TimeRange>,
    /// Recording IDs
    recording_ids: Option<Vec<RecordingId>>,
    /// Cluster membership
    clusters: Option<Vec<ClusterId>>,
    /// Quality thresholds
    min_snr: Option<f32>,
    min_confidence: Option<f32>,
    /// Custom metadata filters
    custom: HashMap<String, FilterValue>,
}

impl FilterState {
    pub fn apply(&self, points: &[EmbeddingPoint]) -> Vec<&EmbeddingPoint> {
        points.iter().filter(|p| {
            self.matches_species(p) &&
            self.matches_location(p) &&
            self.matches_time(p) &&
            self.matches_recording(p) &&
            self.matches_cluster(p) &&
            self.matches_quality(p) &&
            self.matches_custom(p)
        }).collect()
    }
}
```

**Filter UI Components**:
- Species autocomplete (searchable dropdown)
- Map-based location filter (draw bounding box)
- Date/time range slider
- Cluster membership checkboxes
- Quality threshold sliders
- Saved filter presets

### 4. Evidence Pack Display for RAB

**Purpose**: Present RAB outputs with full attribution and transparency.

**Implementation**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidencePack {
    /// Query segment that prompted this pack
    query_segment: SegmentId,

    /// Top-k nearest neighbors with distances
    neighbors: Vec<NeighborEvidence>,

    /// Cluster prototypes (exemplar calls)
    cluster_exemplars: Vec<ExemplarEvidence>,

    /// Model predictions if available
    predictions: Option<Vec<Prediction>>,

    /// Sequence context
    sequence_context: SequenceContext,

    /// Signal quality metrics
    quality: SignalQuality,

    /// Generated interpretation with citations
    interpretation: Interpretation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeighborEvidence {
    segment_id: SegmentId,
    distance: f32,
    rank: usize,
    /// Spectrogram thumbnail URL/data
    spectrogram_thumb: String,
    /// Audio URL for playback
    audio_url: String,
    /// Known label if available
    label: Option<String>,
    /// Metadata
    metadata: SegmentMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interpretation {
    /// Natural language summary
    summary: String,
    /// Individual claims with evidence
    claims: Vec<EvidencedClaim>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidencedClaim {
    /// The claim being made
    statement: String,
    /// Evidence supporting this claim
    evidence: Vec<EvidenceRef>,
    /// Confidence level
    confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvidenceRef {
    Neighbor { rank: usize, segment_id: SegmentId },
    Exemplar { cluster_id: ClusterId, segment_id: SegmentId },
    SequencePattern { pattern_id: String },
    SignalQuality { metric: String, value: f32 },
}
```

**Display Components**:

```
+------------------------------------------------------------------+
|  EVIDENCE PACK: Segment #12847                                    |
+------------------------------------------------------------------+
|                                                                   |
|  QUERY SEGMENT                   | SIGNAL QUALITY                 |
|  +---------------------------+   | SNR: 24.3 dB      [====----]  |
|  | [Spectrogram Display]     |   | Energy: 0.82      [=======--]  |
|  | [Play Button]             |   | Clipping: None    [=========]  |
|  +---------------------------+   | Overlap: Low      [========-]  |
|                                                                   |
+------------------------------------------------------------------+
|  TOP NEIGHBORS (k=5)                                              |
|  +-------+-------+-------+-------+-------+                        |
|  | #1    | #2    | #3    | #4    | #5    |                        |
|  | d=.12 | d=.15 | d=.18 | d=.21 | d=.23 |                        |
|  |[spec] |[spec] |[spec] |[spec] |[spec] |                        |
|  | [>]   | [>]   | [>]   | [>]   | [>]   |  (click to play)      |
|  +-------+-------+-------+-------+-------+                        |
+------------------------------------------------------------------+
|  CLUSTER EXEMPLARS                                                |
|  Assigned: Cluster 7 (alarm call type)                            |
|  +---------------------------+                                    |
|  | Prototype [spec] [>]     | Distance to centroid: 0.14         |
|  +---------------------------+                                    |
|  Other nearby: Cluster 3 (0.31), Cluster 12 (0.38)               |
+------------------------------------------------------------------+
|  SEQUENCE CONTEXT                                                 |
|  Previous: [seg#12846] -> [THIS] -> Next: [seg#12848]            |
|  Motif: Part of "alarm-response" pattern (23 occurrences)         |
+------------------------------------------------------------------+
|  INTERPRETATION                                                   |
|                                                                   |
|  "This call sits in the same neighborhood as known alarm          |
|   exemplars [1,2,3] and appears in similar sequence positions     |
|   during disturbance periods [4]."                                |
|                                                                   |
|  Citations:                                                       |
|  [1] Neighbor #1 (d=0.12): labeled alarm call, recording R-2341  |
|  [2] Neighbor #2 (d=0.15): labeled alarm call, recording R-1892  |
|  [3] Cluster 7 prototype: alarm call type                         |
|  [4] Motif "alarm-response": 78% association with disturbance    |
+------------------------------------------------------------------+
```

### 5. Responsive Design Considerations

#### 5.1 Viewport Breakpoints

| Breakpoint | Width | Layout Adjustments |
|------------|-------|-------------------|
| Desktop XL | >1400px | Full 3-panel layout, all views visible |
| Desktop | 1200-1400px | 2-panel with tabbed secondary |
| Laptop | 992-1200px | Stacked panels, reduced graph size |
| Tablet | 768-992px | Single panel with navigation |
| Mobile | <768px | Essential features only, touch-optimized |

#### 5.2 Component Adaptation

**Embedding Explorer**:
- Desktop: Full scatter with legend and controls
- Tablet: Scatter with collapsible controls
- Mobile: Pinch-zoom scatter, bottom sheet for details

**Neighbor Graph**:
- Desktop: Force-directed with full interactivity
- Tablet: Simplified layout, tap for expansion
- Mobile: List view alternative with thumbnails

**Evidence Pack**:
- Desktop: Full multi-column layout
- Tablet: Two-column with scroll
- Mobile: Vertical card stack, swipe navigation

#### 5.3 Performance Optimization

```typescript
interface ResponsivePerformance {
    // Reduce point count on smaller screens
    maxPointsVisible: {
        desktop: 50000,
        tablet: 20000,
        mobile: 5000
    };

    // Adjust spectrogram resolution
    spectrogramResolution: {
        desktop: 'full',    // 500x128
        tablet: 'medium',   // 250x64
        mobile: 'thumbnail' // 125x32
    };

    // Debounce/throttle timings
    hoverDelay: {
        desktop: 150,
        tablet: 300,
        mobile: 500  // disabled, use tap
    };
}
```

### 6. Accessibility Requirements

#### 6.1 WCAG 2.1 AA Compliance

**Perceivable**:
- Color-blind safe palettes (Viridis, Cividis)
- Minimum contrast ratio 4.5:1 for text
- Text alternatives for all spectrograms
- Audio descriptions for playback

**Operable**:
- Full keyboard navigation (Tab, Arrow, Enter, Escape)
- Skip links for main content areas
- No seizure-triggering animations
- Touch targets minimum 44x44px

**Understandable**:
- Consistent navigation across views
- Error messages with clear guidance
- Help documentation and tooltips
- Language attributes on all content

**Robust**:
- Semantic HTML structure
- ARIA labels and roles
- Screen reader compatibility (NVDA, VoiceOver)
- Progressive enhancement

#### 6.2 Implementation

```typescript
// Accessible scatter plot point
interface AccessiblePoint {
    // Visual representation
    x: number;
    y: number;
    color: string;
    size: number;

    // Accessibility attributes
    ariaLabel: string;      // "Call segment 12847, cluster 7, alarm type"
    ariaDescribedBy: string; // Reference to description element
    tabIndex: number;       // Keyboard navigation order
    role: 'button';         // Interactive element role

    // Screen reader text (not visual)
    srDescription: string;  // Full context for screen readers
}

// Keyboard navigation
interface KeyboardNav {
    // Arrow keys: move between points (nearest in direction)
    ArrowUp: () => selectNearestPoint('up');
    ArrowDown: () => selectNearestPoint('down');
    ArrowLeft: () => selectNearestPoint('left');
    ArrowRight: () => selectNearestPoint('right');

    // Enter: play audio and show details
    Enter: () => activateSelectedPoint();

    // Space: toggle selection
    Space: () => togglePointSelection();

    // Escape: clear selection, close panels
    Escape: () => clearAndClose();

    // Tab: move to next interactive region
    Tab: () => nextRegion();
}

// Audio alternatives
interface AudioAccessibility {
    // Sonification of data patterns
    sonifyCluster(clusterId: string): AudioBuffer;
    sonifyTrajectory(trajectory: Trajectory): AudioBuffer;

    // Audio descriptions
    describeEvidencePack(pack: EvidencePack): string;
    describeNeighbors(neighbors: NeighborEvidence[]): string;
}
```

#### 6.3 Color Accessibility

```rust
/// Color-blind safe palettes
pub enum AccessibleColorPalette {
    /// Viridis (perceptually uniform, color-blind safe)
    Viridis,
    /// Cividis (optimized for deuteranopia)
    Cividis,
    /// Plasma (high contrast, color-blind safe)
    Plasma,
    /// Inferno (black-body, color-blind safe)
    Inferno,
    /// Custom high-contrast for categorical data
    HighContrast,
}

impl AccessibleColorPalette {
    pub fn get_colors(&self, n: usize) -> Vec<Color> {
        match self {
            Self::Viridis => viridis_colors(n),
            Self::Cividis => cividis_colors(n),
            Self::Plasma => plasma_colors(n),
            Self::Inferno => inferno_colors(n),
            Self::HighContrast => high_contrast_categorical(n),
        }
    }

    /// Ensure minimum contrast between adjacent colors
    pub fn validate_contrast(&self, colors: &[Color]) -> bool {
        for i in 0..colors.len() - 1 {
            if contrast_ratio(colors[i], colors[i + 1]) < 3.0 {
                return false;
            }
        }
        true
    }
}
```

## Consequences

### Positive

1. **Rust-Native Performance**: UMAP and graph computations run at native speed, with WASM enabling browser deployment without Python dependencies.

2. **Evidence-Based Interpretation**: The RAB evidence pack display ensures all claims are traceable to specific acoustic examples, maintaining scientific integrity.

3. **Cross-Platform Consistency**: Single codebase (Rust) compiles to both desktop and WASM, reducing maintenance burden.

4. **Accessibility First**: WCAG 2.1 AA compliance ensures the platform is usable by researchers with diverse abilities.

5. **Progressive Enhancement**: Core functionality works without JavaScript, with enhanced interactivity layered on top.

### Negative

1. **WASM Bundle Size**: The visualization module will add ~2-5MB to browser downloads (mitigated by lazy loading and code splitting).

2. **Browser Compatibility**: WebAssembly + Web Audio requires modern browsers (Chrome 57+, Firefox 52+, Safari 11+).

3. **Touch Optimization**: Some visualizations (dense scatter plots, force-directed graphs) are challenging on mobile devices.

4. **Learning Curve**: Researchers familiar with Python visualization (matplotlib, seaborn) may need adjustment time.

### Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| UMAP projection quality | Misleading clusters | Validate with known labels, provide multiple projection parameters |
| Audio playback latency | Poor user experience | Preload audio chunks, use streaming where possible |
| Large dataset performance | UI freezing | Virtual scrolling, progressive loading, Web Workers |
| Spectrogram memory usage | Browser crashes | LRU cache with size limits, resolution tiers |

## Implementation Phases

### Phase 1: Core Visualization (Weeks 1-3)
- Embedding explorer with UMAP projection
- Basic spectrogram display
- Click-to-play audio integration

### Phase 2: Graph Views (Weeks 4-5)
- Neighbor graph visualization
- Force-directed layout
- GNN reranking toggle

### Phase 3: Sequence Analysis (Weeks 6-7)
- Trajectory visualization
- Motif detection display
- Transition matrix heatmaps

### Phase 4: RAB Integration (Weeks 8-9)
- Evidence pack component
- Citation display
- Interpretation panel

### Phase 5: Polish and Accessibility (Weeks 10-12)
- Responsive design implementation
- WCAG 2.1 AA audit and fixes
- Performance optimization
- Documentation and user guides

## References

- [umap-rs Documentation](https://docs.rs/umap-rs)
- [Plotly Rust Bindings](https://crates.io/crates/plotly)
- [WCAG 2.1 Guidelines](https://www.w3.org/WAI/WCAG21/quickref/)
- [Perch 2.0 Paper](https://arxiv.org/abs/2508.04665)
- [RuVector Repository](https://github.com/ruvnet/ruvector)
- [Web Audio API](https://developer.mozilla.org/en-US/docs/Web/API/Web_Audio_API)

## Appendix A: File Structure

```
sevensense/
├── crates/
│   └── sevensense-viz/
│       ├── Cargo.toml
│       ├── src/
│       │   ├── lib.rs
│       │   ├── explorer/
│       │   │   ├── mod.rs
│       │   │   ├── umap.rs
│       │   │   └── viewport.rs
│       │   ├── graph/
│       │   │   ├── mod.rs
│       │   │   ├── layout.rs
│       │   │   └── render.rs
│       │   ├── trajectory/
│       │   │   ├── mod.rs
│       │   │   ├── motifs.rs
│       │   │   └── transitions.rs
│       │   ├── spectrogram/
│       │   │   ├── mod.rs
│       │   │   ├── cache.rs
│       │   │   └── render.rs
│       │   ├── evidence/
│       │   │   ├── mod.rs
│       │   │   └── pack.rs
│       │   ├── wasm/
│       │   │   ├── mod.rs
│       │   │   └── bindings.rs
│       │   └── accessibility/
│       │       ├── mod.rs
│       │       ├── colors.rs
│       │       └── keyboard.rs
│       └── tests/
├── web/
│   └── sevensense-ui/
│       ├── package.json
│       ├── src/
│       │   ├── components/
│       │   │   ├── EmbeddingExplorer.tsx
│       │   │   ├── NeighborGraph.tsx
│       │   │   ├── TrajectoryView.tsx
│       │   │   ├── EvidencePack.tsx
│       │   │   └── FilterPanel.tsx
│       │   ├── hooks/
│       │   │   ├── useVibecastWasm.ts
│       │   │   ├── useAudioPlayer.ts
│       │   │   └── useSelection.ts
│       │   └── utils/
│       │       ├── wasm-loader.ts
│       │       └── accessibility.ts
│       └── public/
└── docs/
    └── adr/
        └── ADR-009-visualization-ui.md
```

## Appendix B: Color Palette Specifications

```rust
/// Viridis palette (256 colors)
pub const VIRIDIS: [(u8, u8, u8); 256] = [
    (68, 1, 84),    // 0 - dark purple
    // ... intermediate values
    (253, 231, 37), // 255 - bright yellow
];

/// Categorical palette for clusters (max 12 distinct)
pub const CATEGORICAL_12: [(u8, u8, u8); 12] = [
    (31, 119, 180),   // blue
    (255, 127, 14),   // orange
    (44, 160, 44),    // green
    (214, 39, 40),    // red
    (148, 103, 189),  // purple
    (140, 86, 75),    // brown
    (227, 119, 194),  // pink
    (127, 127, 127),  // gray
    (188, 189, 34),   // olive
    (23, 190, 207),   // cyan
    (31, 180, 119),   // teal
    (255, 187, 120),  // light orange
];
```
