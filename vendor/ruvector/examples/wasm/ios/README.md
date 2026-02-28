# Ruvector iOS WASM

**Privacy-Preserving On-Device AI for iOS, Safari & Modern Browsers**

A lightweight, high-performance WebAssembly vector database with machine learning capabilities optimized for Apple platforms. Run ML inference, vector search, and personalized recommendations entirely on-device without sending user data to servers.

## Key Features

| Feature | Description |
|---------|-------------|
| **Privacy-First** | All data stays on-device. No PII, coordinates, or content sent anywhere |
| **Dual Target** | Single codebase for native iOS (WasmKit) and browser (Safari/Chrome/Firefox) |
| **HNSW Index** | Hierarchical Navigable Small World graph for O(log n) similarity search |
| **Q-Learning** | Adaptive recommendation engine that learns from user behavior |
| **SIMD Acceleration** | Auto-detects and uses WASM SIMD (iOS 16.4+/Safari 16.4+/Chrome 91+) |
| **Memory Efficient** | Scalar (4x), Binary (32x), and Product (variable) quantization |
| **Self-Learning** | Health, Location, Calendar, App Usage pattern learning |
| **Tiny Footprint** | ~100KB optimized native / ~200KB browser with all features |

## Capabilities

### Vector Database
- **HNSW Index**: Fast approximate nearest neighbor search
- **Distance Metrics**: Euclidean, Cosine, Manhattan, Dot Product
- **Persistence**: Serialize/deserialize to bytes for storage
- **Capacity**: 100K+ vectors at <50ms search latency

### Machine Learning
- **Embeddings**: Hash-based text embeddings (64-512 dims)
- **Attention**: Multi-head attention for ranking
- **Q-Learning**: Adaptive recommendations with exploration/exploitation
- **Pattern Recognition**: Time-based behavioral patterns

### Privacy-Preserving Learning

| Module | What It Learns | What It NEVER Stores |
|--------|---------------|---------------------|
| Health | Activity patterns, sleep schedules | Actual health values, medical data |
| Location | Place categories, time at venues | GPS coordinates, addresses |
| Calendar | Busy times, meeting patterns | Event titles, attendees, content |
| Communication | Response patterns, quiet hours | Message content, contact names |
| App Usage | Screen time, category patterns | App names, usage details |

## Quick Start

### Browser (Safari/Chrome/Firefox)

```html
<script type="module">
import init, { VectorDatabaseJS, dot_product } from './ruvector_ios_wasm.js';

await init();

// Create vector database
const db = new VectorDatabaseJS(128, 'cosine', 'none');

// Insert vectors
const embedding = new Float32Array(128);
embedding.fill(0.5);
db.insert(1n, embedding);

// Search
const results = db.search(embedding, 10);
console.log('Nearest neighbors:', results);
</script>
```

### Native iOS (WasmKit)

```swift
import Foundation

// Load WASM module
let ruvector = RuvectorWasm.shared
try ruvector.load(from: Bundle.main.path(forResource: "ruvector", ofType: "wasm")!)

// Initialize learners
try ruvector.initIOSLearner()

// Record app usage
let session = AppUsageSession(
    category: .productivity,
    durationSeconds: 1800,
    hour: 14,
    dayOfWeek: 2,
    isActiveUse: true
)
try ruvector.learnAppSession(session)

// Get recommendations
let context = IOSContext(
    hour: 15,
    dayOfWeek: 2,
    batteryLevel: 80,
    networkType: 1,
    locationCategory: .work,
    recentAppCategory: .productivity,
    activityLevel: 5,
    healthScore: 0.8
)
let recommendations = try ruvector.getRecommendations(context)
print("Suggested: \(recommendations.suggestedAppCategory)")
```

### SwiftUI Integration

```swift
import SwiftUI

struct ContentView: View {
    @StateObject private var ruvector = RuvectorViewModel()

    var body: some View {
        VStack {
            if ruvector.isReady {
                Text("Screen Time: \(ruvector.screenTimeHours, specifier: "%.1f")h")
                Text("Focus Score: \(Int(ruvector.focusScore * 100))%")
            } else {
                ProgressView("Loading AI...")
            }
        }
        .task {
            try? await ruvector.load(from: Bundle.main.path(forResource: "ruvector", ofType: "wasm")!)
        }
    }
}
```

## Building

### Prerequisites
- Rust 1.70+ with WASM targets
- wasm-opt (optional, for size optimization)

### Native WASI Build (for WasmKit/iOS)

```bash
# Add WASI target
rustup target add wasm32-wasip1

# Build optimized native WASM
cargo build --release --target wasm32-wasip1

# Optimize size (optional)
wasm-opt -Oz -o ruvector.wasm target/wasm32-wasip1/release/ruvector_ios_wasm.wasm
```

### Browser Build (wasm-bindgen)

```bash
# Add browser target
rustup target add wasm32-unknown-unknown

# Build with browser feature
cargo build --release --target wasm32-unknown-unknown --features browser

# Generate JS bindings
wasm-bindgen target/wasm32-unknown-unknown/release/ruvector_ios_wasm.wasm \
  --out-dir pkg --target web
```

### Build Options

| Feature | Flag | Description |
|---------|------|-------------|
| browser | `--features browser` | wasm-bindgen JS bindings |
| simd | `--features simd` | WASM SIMD acceleration |
| full | `--features full` | All features |

## Benchmarks

Tested on Apple M2 (native) and Safari 17 (browser):

### Vector Operations (128 dims, 10K iterations)

| Operation | Native | Browser | Ops/sec |
|-----------|--------|---------|---------|
| Dot Product | 0.8ms | 1.2ms | 8M+ |
| L2 Distance | 0.9ms | 1.4ms | 7M+ |
| Cosine Similarity | 1.1ms | 1.6ms | 6M+ |

### HNSW Index (64 dims)

| Operation | 1K vectors | 10K vectors | 100K vectors |
|-----------|-----------|-------------|--------------|
| Insert | 2.3ms | 45ms | 890ms |
| Search (k=10) | 0.05ms | 0.3ms | 2.1ms |
| Search QPS | 20,000 | 3,300 | 476 |

### Memory Usage

| Vectors | No Quant | Scalar (4x) | Binary (32x) |
|---------|----------|-------------|--------------|
| 1,000 | 512 KB | 128 KB | 16 KB |
| 10,000 | 5.1 MB | 1.3 MB | 160 KB |
| 100,000 | 51 MB | 13 MB | 1.6 MB |

### Binary Size

| Configuration | Size |
|--------------|------|
| Native WASI (optimized) | 103 KB |
| Native WASI (debug) | 141 KB |
| Browser (full features) | 357 KB |
| Browser + gzip | ~120 KB |

## Comparison

### vs. Other WASM Vector DBs

| Feature | Ruvector iOS | HNSWLIB-WASM | Vectra.js |
|---------|-------------|--------------|-----------|
| Native iOS (WasmKit) | Yes | No | No |
| Safari Support | Yes | Partial | Yes |
| Quantization | 3 modes | None | Scalar |
| ML Integration | Q-Learning, Attention | None | None |
| Privacy Learning | 5 modules | None | None |
| Binary Size | 103KB | 450KB | 280KB |
| SIMD | Auto-detect | Manual | No |

### vs. Native Swift Solutions

| Aspect | Ruvector iOS WASM | Native Swift |
|--------|-------------------|--------------|
| Development | Single Rust codebase | Swift only |
| Cross-platform | iOS + Safari + Chrome | iOS only |
| Performance | 90-95% native | 100% |
| Binary Size | +100KB | Varies |
| Updates | Hot-loadable | App Store |

## Tutorials

### 1. Building a Recommendation Engine

```javascript
import init, { RecommendationEngineJS } from './ruvector_ios_wasm.js';

await init();

// Create engine with 64-dim embeddings
const engine = new RecommendationEngineJS(64, 10000);

// Add items (products, articles, etc.)
const productEmbedding = new Float32Array(64);
productEmbedding.set([0.1, 0.2, 0.3, /* ... */]);
engine.add_item(123n, productEmbedding);

// Record user interactions
engine.record_interaction(1n, 123n, 1.0);  // User 1 clicked item 123

// Get personalized recommendations
const recs = engine.recommend(1n, 10);
for (const rec of recs) {
    console.log(`Item ${rec.item_id}: score ${rec.score.toFixed(3)}`);
}
```

### 2. Privacy-Preserving Health Insights

```javascript
import init, { HealthLearnerJS, HealthMetrics } from './ruvector_ios_wasm.js';

await init();

const health = new HealthLearnerJS();

// Learn from HealthKit data (values normalized to 0-9 buckets)
health.learn_event({
    metric: HealthMetrics.STEPS,
    value_bucket: 7,  // High activity (buckets hide actual step count)
    hour: 8,
    day_of_week: 1
});

// Predict typical activity level
const predictedBucket = health.predict(HealthMetrics.STEPS, 8, 1);
console.log(`Usually active at 8am Monday: bucket ${predictedBucket}`);

// Get overall activity score
console.log(`Activity score: ${(health.activity_score() * 100).toFixed(0)}%`);
```

### 3. Smart Focus Time Suggestions

```javascript
import init, { CalendarLearnerJS, CalendarEventTypes } from './ruvector_ios_wasm.js';

await init();

const calendar = new CalendarLearnerJS();

// Learn from calendar events (no titles stored)
calendar.learn_event({
    event_type: CalendarEventTypes.MEETING,
    start_hour: 10,
    duration_minutes: 60,
    day_of_week: 1,
    is_recurring: true,
    has_attendees: true
});

// Find best focus time blocks
const focusTimes = calendar.suggest_focus_times(2); // 2-hour blocks
for (const slot of focusTimes) {
    const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
    console.log(`${days[slot.day]} ${slot.start_hour}:00 - Score: ${slot.score.toFixed(2)}`);
}

// Check if specific time is likely busy
const busy = calendar.busy_probability(14, 2);
console.log(`Tuesday 2pm busy probability: ${(busy * 100).toFixed(0)}%`);
```

### 4. Digital Wellbeing Dashboard

```javascript
import init, { AppUsageLearnerJS, AppCategories } from './ruvector_ios_wasm.js';

await init();

const usage = new AppUsageLearnerJS();

// Track app sessions (category only, not app names)
usage.learn_session({
    category: AppCategories.SOCIAL,
    duration_seconds: 1800,
    hour: 20,
    day_of_week: 5,
    is_active_use: true
});

// Get screen time summary
const summary = usage.screen_time_summary();
console.log(`Total: ${summary.total_minutes.toFixed(0)} min`);
console.log(`Top category: ${summary.top_category}`);

// Get wellbeing insights
const insights = usage.wellbeing_insights();
for (const insight of insights) {
    console.log(`[${insight.category}] ${insight.message} (score: ${insight.score})`);
}
```

### 5. Context-Aware App Launcher

```swift
// Swift example for native iOS
let context = IOSContext(
    hour: 7,
    dayOfWeek: 1,  // Monday morning
    batteryLevel: 100,
    networkType: 1,  // WiFi
    locationCategory: .home,
    recentAppCategory: .utilities,
    activityLevel: 3,
    healthScore: 0.7
)

let recommendations = try ruvector.getRecommendations(context)

// Show suggested apps based on context
switch recommendations.suggestedAppCategory {
case .productivity:
    showWidget("Work Focus")
case .health:
    showWidget("Morning Workout")
case .news:
    showWidget("Morning Brief")
default:
    break
}

// Determine notification priority
if recommendations.optimalNotificationTime {
    enableNotifications()
} else {
    enableFocusMode()
}
```

### 6. Semantic Search

```javascript
import init, { VectorDatabaseJS, dot_product } from './ruvector_ios_wasm.js';

await init();

// Create database with cosine similarity
const db = new VectorDatabaseJS(384, 'cosine', 'scalar');

// In production: use a real embedding model
async function embed(text) {
    // Placeholder - use transformers.js, TensorFlow.js, or remote API
    return new Float32Array(384).fill(0.1);
}

// Index documents
const docs = [
    { id: 1, text: "Machine learning fundamentals" },
    { id: 2, text: "iOS development with Swift" },
    { id: 3, text: "Web performance optimization" },
];

for (const doc of docs) {
    const embedding = await embed(doc.text);
    db.insert(BigInt(doc.id), embedding);
}

// Search
const query = await embed("How to build iOS apps");
const results = db.search(query, 3);

for (const result of results) {
    console.log(`Doc ${result.id}: similarity ${(1 - result.distance).toFixed(3)}`);
}
```

## API Reference

See [TypeScript Definitions](./types/ruvector-ios.d.ts) for complete API documentation.

### Core Classes
- `VectorDatabaseJS` - Main vector database with HNSW
- `HnswIndexJS` - Low-level HNSW index
- `RecommendationEngineJS` - Q-learning recommendation engine

### Quantization
- `ScalarQuantizedJS` - 8-bit quantization (4x compression)
- `BinaryQuantizedJS` - 1-bit quantization (32x compression)
- `ProductQuantizedJS` - Sub-vector clustering

### Learning Modules
- `HealthLearnerJS` - Health/fitness patterns
- `LocationLearnerJS` - Location category patterns
- `CommLearnerJS` - Communication patterns
- `CalendarLearnerJS` - Calendar/schedule patterns
- `AppUsageLearnerJS` - App usage/screen time
- `iOSLearnerJS` - Unified learner with all modules

## Platform Support

| Platform | Version | SIMD | Notes |
|----------|---------|------|-------|
| iOS (WasmKit) | 15.0+ | Yes | Native performance |
| Safari | 16.4+ | Yes | Full WASM support |
| Chrome | 91+ | Yes | Best SIMD support |
| Firefox | 89+ | Yes | Full support |
| Edge | 91+ | Yes | Chromium-based |
| Node.js | 16+ | Yes | Server-side option |

## License

MIT License - See [LICENSE](../../../LICENSE) for details.

## Contributing

Contributions welcome! See [CONTRIBUTING.md](../../../CONTRIBUTING.md) for guidelines.
