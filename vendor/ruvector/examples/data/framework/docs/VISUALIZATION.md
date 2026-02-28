# ASCII Graph Visualization Guide

Terminal-based graph visualization for the RuVector Discovery Framework with ANSI colors, domain clustering, coherence heatmaps, and pattern timeline displays.

## Features

### 🎨 Graph Visualization
- **ASCII art rendering** with box-drawing characters
- **Domain-based coloring** using ANSI escape codes
  - 🔵 Climate (Blue)
  - 🟢 Finance (Green)
  - 🟡 Research (Yellow)
  - 🟣 Cross-domain (Magenta)
- **Cluster structure** showing node groupings by domain
- **Cross-domain bridges** displayed as connecting lines

### 📊 Domain Matrix
- Shows connectivity strength between domains
- Diagonal elements show node count per domain
- Off-diagonal elements show cross-domain edge counts
- Color-coded by domain

### 📈 Coherence Timeline
- **ASCII sparkline** chart for temporal coherence values
- **Adaptive scaling** based on value range
- Duration display (days/hours/minutes)
- Time range labels

### 🔍 Pattern Summary
- Pattern count by type with visual bars
- Statistical significance indicators
- Top patterns ranked by confidence
- P-values and effect sizes

### 🖥️ Complete Dashboard
Combines all visualizations into a single comprehensive view.

## API Reference

### Core Functions

#### `render_graph_ascii`
```rust
pub fn render_graph_ascii(
    engine: &OptimizedDiscoveryEngine,
    width: usize,
    height: usize
) -> String
```

Renders the graph as ASCII art with colored domain nodes.

**Parameters:**
- `engine` - The discovery engine containing the graph
- `width` - Canvas width in characters (recommended: 80)
- `height` - Canvas height in characters (recommended: 20)

**Returns:** String containing the ASCII art representation

**Example:**
```rust
use ruvector_data_framework::visualization::render_graph_ascii;

let graph = render_graph_ascii(&engine, 80, 20);
println!("{}", graph);
```

---

#### `render_domain_matrix`
```rust
pub fn render_domain_matrix(
    engine: &OptimizedDiscoveryEngine
) -> String
```

Renders a domain connectivity matrix showing connections between domains.

**Returns:** Formatted matrix string with domain statistics

**Example:**
```rust
let matrix = render_domain_matrix(&engine);
println!("{}", matrix);
```

---

#### `render_coherence_timeline`
```rust
pub fn render_coherence_timeline(
    history: &[(DateTime<Utc>, f64)]
) -> String
```

Renders coherence timeline as ASCII sparkline/chart.

**Parameters:**
- `history` - Time series of (timestamp, coherence_value) pairs

**Returns:** ASCII chart with sparkline visualization

**Example:**
```rust
let timeline = render_coherence_timeline(&coherence_history);
println!("{}", timeline);
```

---

#### `render_pattern_summary`
```rust
pub fn render_pattern_summary(
    patterns: &[SignificantPattern]
) -> String
```

Renders a summary of discovered patterns with statistics.

**Parameters:**
- `patterns` - List of significant patterns to summarize

**Returns:** Formatted summary with pattern breakdown

**Example:**
```rust
let summary = render_pattern_summary(&patterns);
println!("{}", summary);
```

---

#### `render_dashboard`
```rust
pub fn render_dashboard(
    engine: &OptimizedDiscoveryEngine,
    patterns: &[SignificantPattern],
    coherence_history: &[(DateTime<Utc>, f64)]
) -> String
```

Renders a complete dashboard combining all visualizations.

**Parameters:**
- `engine` - Discovery engine with graph data
- `patterns` - Discovered patterns
- `coherence_history` - Time series of coherence values

**Returns:** Complete dashboard string

**Example:**
```rust
let dashboard = render_dashboard(&engine, &patterns, &coherence_history);
println!("{}", dashboard);
```

## Box-Drawing Characters

The module uses Unicode box-drawing characters for structure:

| Character | Unicode | Usage |
|-----------|---------|-------|
| `─` | U+2500 | Horizontal line |
| `│` | U+2502 | Vertical line |
| `┌` | U+250C | Top-left corner |
| `┐` | U+2510 | Top-right corner |
| `└` | U+2514 | Bottom-left corner |
| `┘` | U+2518 | Bottom-right corner |
| `┼` | U+253C | Cross |
| `┬` | U+252C | T-down |
| `┴` | U+2534 | T-up |
| `├` | U+251C | T-right |
| `┤` | U+2524 | T-left |

## ANSI Color Codes

Domain colors are implemented using ANSI escape sequences:

| Domain | Color | Code |
|--------|-------|------|
| Climate | Blue | `\x1b[34m` |
| Finance | Green | `\x1b[32m` |
| Research | Yellow | `\x1b[33m` |
| Cross-domain | Magenta | `\x1b[35m` |
| Reset | Default | `\x1b[0m` |
| Bright | Bold | `\x1b[1m` |
| Dim | Dimmed | `\x1b[2m` |

## Complete Example

```rust
use chrono::{Duration, Utc};
use ruvector_data_framework::optimized::{OptimizedConfig, OptimizedDiscoveryEngine};
use ruvector_data_framework::ruvector_native::{Domain, SemanticVector};
use ruvector_data_framework::visualization::render_dashboard;
use std::collections::HashMap;

fn main() {
    // Create engine
    let config = OptimizedConfig::default();
    let mut engine = OptimizedDiscoveryEngine::new(config);

    // Add vectors
    let now = Utc::now();
    for i in 0..10 {
        let vector = SemanticVector {
            id: format!("climate_{}", i),
            embedding: vec![0.5 + i as f32 * 0.05; 128],
            domain: Domain::Climate,
            timestamp: now,
            metadata: HashMap::new(),
        };
        engine.add_vector(vector);
    }

    // Compute coherence over time
    let mut coherence_history = Vec::new();
    let mut all_patterns = Vec::new();

    for step in 0..5 {
        let timestamp = now + Duration::hours(step);
        let coherence = engine.compute_coherence();
        coherence_history.push((timestamp, coherence.mincut_value));

        let patterns = engine.detect_patterns_with_significance();
        all_patterns.extend(patterns);
    }

    // Display dashboard
    let dashboard = render_dashboard(&engine, &all_patterns, &coherence_history);
    println!("{}", dashboard);
}
```

## Terminal Compatibility

The visualization module uses ANSI escape codes and Unicode box-drawing characters. For best results:

### ✅ Recommended Terminals
- **Linux**: GNOME Terminal, Konsole, Alacritty, Kitty
- **macOS**: Terminal.app, iTerm2
- **Windows**: Windows Terminal, ConEmu
- **Cross-platform**: Alacritty, Kitty

### ⚠️ Limited Support
- **Windows CMD**: No ANSI color support (use Windows Terminal instead)
- **Old terminals**: May not support Unicode box-drawing

### 🔧 Environment Variables
```bash
# Ensure Unicode support
export LANG=en_US.UTF-8
export LC_ALL=en_US.UTF-8

# Force color output
export FORCE_COLOR=1
```

## Performance Considerations

### Memory
- Graph rendering: O(width × height) for canvas
- Timeline rendering: O(history length)
- Pattern summary: O(pattern count)

### Time Complexity
- Graph layout: O(nodes + edges)
- Timeline chart: O(history samples)
- Pattern summary: O(patterns × log(patterns)) for sorting

### Optimization Tips
1. **Limit canvas size** - Use 80×20 for standard terminals
2. **Sample large datasets** - Timeline auto-samples if > 60 points
3. **Filter patterns** - Only display top N patterns for large lists

## Testing

Run the visualization tests:
```bash
# Run all visualization tests
cargo test --lib visualization

# Run specific test
cargo test --lib test_render_graph_ascii

# Run visualization demo
cargo run --example visualization_demo
```

## Integration with Discovery Pipeline

```rust
use ruvector_data_framework::{DiscoveryPipeline, PipelineConfig};
use ruvector_data_framework::visualization::render_dashboard;

// Create pipeline
let config = PipelineConfig::default();
let mut pipeline = DiscoveryPipeline::new(config);

// Run discovery
let patterns = pipeline.run(data_source).await?;

// Build coherence history from engine
let coherence_history = pipeline.coherence.signals()
    .iter()
    .map(|s| (s.window.start, s.min_cut_value))
    .collect();

// Visualize results
let dashboard = render_dashboard(
    &pipeline.discovery_engine,
    &patterns,
    &coherence_history
);

println!("{}", dashboard);
```

## Customization

### Custom Color Schemes
Modify the color constants in `visualization.rs`:

```rust
const COLOR_CLIMATE: &str = "\x1b[34m";  // Change to your preference
const COLOR_FINANCE: &str = "\x1b[32m";
const COLOR_RESEARCH: &str = "\x1b[33m";
```

### Custom Characters
Replace box-drawing characters:

```rust
const BOX_H: char = '-';  // Use ASCII alternative
const BOX_V: char = '|';
const BOX_TL: char = '+';
```

### Layout Customization
Modify domain positions in `render_graph_ascii`:

```rust
let domain_regions = [
    (Domain::Climate, 10, 2),          // Top-left
    (Domain::Finance, mid_x + 10, 2),  // Top-right
    (Domain::Research, 10, mid_y + 2), // Bottom-left
];
```

## Troubleshooting

### Colors not displaying
```bash
# Check terminal color support
echo -e "\x1b[34mBlue\x1b[0m"

# Enable color in cargo output
cargo run --color=always
```

### Box characters appear as question marks
```bash
# Verify UTF-8 encoding
locale  # Should show UTF-8

# Set UTF-8 locale
export LANG=en_US.UTF-8
```

### Layout issues
- Ensure terminal width ≥ 80 characters
- Use monospace font (recommended: Cascadia Code, Fira Code)
- Adjust canvas size parameters

## Future Enhancements

Planned features for future versions:

- [ ] Interactive terminal UI with cursive/tui-rs
- [ ] Real-time streaming updates
- [ ] Export to SVG/PNG
- [ ] 3D graph visualization (ASCII isometric)
- [ ] Animated transitions between states
- [ ] Custom color themes
- [ ] Responsive layout for different terminal sizes
- [ ] Mouse interaction support

## See Also

- [Optimized Discovery Engine](../src/optimized.rs)
- [Pattern Detection](../src/discovery.rs)
- [Coherence Computation](../src/coherence.rs)
- [Cross-Domain Discovery Example](../examples/cross_domain_discovery.rs)

## License

Part of the RuVector Discovery Framework. See main repository for license information.
