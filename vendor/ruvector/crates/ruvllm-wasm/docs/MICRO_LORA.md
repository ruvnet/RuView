# MicroLoRA - Browser-Compatible Lightweight LoRA Adaptation

MicroLoRA provides ultra-lightweight LoRA (Low-Rank Adaptation) for real-time adaptation of language models directly in web browsers.

## Features

- **Tiny Memory Footprint**: Rank 1-4 adapters use <50KB per adapter
- **Pure WASM**: No threading, no file I/O, fully browser-compatible
- **Real-time Adaptation**: Update weights based on user feedback with <1ms latency
- **Serialization**: JSON-based persistence for localStorage/IndexedDB
- **TypeScript-Friendly**: Full type definitions with getter/setter patterns

## Architecture

```
┌─────────────────┐
│  Base LLM       │
│  (frozen)       │
└────────┬────────┘
         │
         ├──────────┐
         │          │
┌────────▼────────┐ │
│  Input          │ │
│  (768-dim)      │ │
└────────┬────────┘ │
         │          │
         ▼          │
┌─────────────────┐ │
│  LoRA A         │ │  Down projection
│  (768 x 2)      │ │  (in_features x rank)
└────────┬────────┘ │
         │          │
         ▼          │
┌─────────────────┐ │
│  Intermediate   │ │
│  (2-dim)        │ │
└────────┬────────┘ │
         │          │
         ▼          │
┌─────────────────┐ │
│  LoRA B         │ │  Up projection
│  (2 x 768)      │ │  (rank x out_features)
└────────┬────────┘ │
         │          │
         ▼          │
┌─────────────────┐ │
│  LoRA Output    │ │  Scaled by (alpha / rank)
│  (768-dim)      │ │
└────────┬────────┘ │
         │          │
         └──────────┤
                    │
         ┌──────────▼───────┐
         │  Final Output    │
         │  (base + LoRA)   │
         └──────────────────┘
```

## Quick Start

### Basic Usage

```javascript
import init, { MicroLoraWasm, MicroLoraConfigWasm, AdaptFeedbackWasm } from 'ruvllm-wasm';

// Initialize WASM
await init();

// Create adapter config
const config = new MicroLoraConfigWasm();
config.rank = 2;              // Rank 1-4 (2 recommended for browser)
config.alpha = 4.0;           // Scaling factor
config.inFeatures = 768;      // Match your model's hidden size
config.outFeatures = 768;

// Create the adapter
const lora = new MicroLoraWasm(config);

// Apply LoRA to hidden states
const hiddenState = new Float32Array(768);
const output = lora.apply(hiddenState);
```

### Real-time Adaptation

```javascript
// User provides feedback on model output
const feedback = new AdaptFeedbackWasm(0.8); // Quality score [0.0, 1.0]
feedback.learningRate = 0.01;

// Adapt weights based on feedback
lora.adapt(hiddenState, feedback);

// Apply updates (can batch multiple adapt calls)
lora.applyUpdates(0.01);

// Get statistics
const stats = lora.stats();
console.log(`Average quality: ${stats.avgQuality}`);
console.log(`Samples seen: ${stats.samplesSeen}`);
```

### Persistence

```javascript
// Save to localStorage
const json = lora.toJson();
localStorage.setItem('lora-state', json);

// Restore from localStorage
const saved = localStorage.getItem('lora-state');
const restored = MicroLoraWasm.fromJson(saved);
```

## API Reference

### MicroLoraConfigWasm

Configuration for the LoRA adapter.

**Properties:**
- `rank: number` - LoRA rank (1-4, clamped). Default: 2
- `alpha: number` - Scaling factor. Default: 4.0
- `inFeatures: number` - Input dimension. Default: 768
- `outFeatures: number` - Output dimension. Default: 768

**Methods:**
- `memoryBytes(): number` - Calculate memory footprint in bytes
- `computeScaling(): number` - Get computed scaling (alpha / rank)

### MicroLoraWasm

The main LoRA adapter.

**Constructor:**
- `new MicroLoraWasm(config: MicroLoraConfigWasm)`

**Methods:**
- `apply(input: Float32Array): Float32Array` - Apply LoRA transformation
- `adapt(input: Float32Array, feedback: AdaptFeedbackWasm): void` - Accumulate gradients
- `applyUpdates(learningRate: number): void` - Apply accumulated gradients
- `reset(): void` - Reset to initial state
- `stats(): MicroLoraStatsWasm` - Get adapter statistics
- `toJson(): string` - Serialize to JSON
- `fromJson(json: string): MicroLoraWasm` - Deserialize from JSON (static)
- `pendingUpdates(): number` - Get number of pending gradient updates
- `getConfig(): MicroLoraConfigWasm` - Get current configuration

### AdaptFeedbackWasm

Feedback for weight adaptation.

**Constructor:**
- `new AdaptFeedbackWasm(quality: number)` - Quality score [0.0, 1.0]

**Properties:**
- `quality: number` - Quality/reward signal [0.0, 1.0]
- `learningRate: number` - Learning rate. Default: 0.01

### MicroLoraStatsWasm

Adapter statistics.

**Properties:**
- `samplesSeen: number` - Total samples seen
- `avgQuality: number` - Average quality score
- `memoryBytes: number` - Memory usage in bytes
- `paramCount: number` - Total parameter count

**Methods:**
- `toJson(): string` - Convert to JSON string

## Memory Footprint

Memory usage for different configurations:

| Config | Memory | Parameters |
|--------|--------|------------|
| Rank 1, 768×768 | 6KB | 1,536 |
| Rank 2, 768×768 | 12KB | 3,072 |
| Rank 4, 768×768 | 24KB | 6,144 |
| Rank 2, 512×512 | 8KB | 2,048 |

Formula: `(in_features × rank + rank × out_features) × 4 bytes`

## Use Cases

### 1. Personalized Chat Interface

```javascript
// Adapt based on user thumbs up/down
async function handleUserFeedback(hiddenStates, wasHelpful) {
    const feedback = new AdaptFeedbackWasm(wasHelpful ? 0.9 : 0.3);
    lora.adapt(hiddenStates, feedback);

    // Apply after every 5 interactions
    if (interactionCount % 5 === 0) {
        lora.applyUpdates(0.02);

        // Persist to localStorage
        localStorage.setItem('chat-lora', lora.toJson());
    }
}
```

### 2. Domain-Specific Fine-tuning

```javascript
// Adapt to technical domain over time
const conversations = [
    { input: codeHelpQuery, quality: 0.85 },
    { input: technicalExplanation, quality: 0.92 },
    // ...
];

for (const conv of conversations) {
    const feedback = new AdaptFeedbackWasm(conv.quality);
    lora.adapt(conv.input, feedback);
}

lora.applyUpdates(0.01);
```

### 3. Multi-User Adapters

```javascript
// Store separate adapters per user
function getUserLora(userId) {
    const key = `lora-${userId}`;
    const saved = localStorage.getItem(key);

    if (saved) {
        return MicroLoraWasm.fromJson(saved);
    }

    const config = new MicroLoraConfigWasm();
    return new MicroLoraWasm(config);
}

function saveUserLora(userId, lora) {
    localStorage.setItem(`lora-${userId}`, lora.toJson());
}
```

## Performance Tips

### 1. Batch Gradient Updates

```javascript
// ❌ Bad: Update after every sample
for (const sample of samples) {
    lora.adapt(sample.input, sample.feedback);
    lora.applyUpdates(0.01); // Expensive!
}

// ✅ Good: Batch updates
for (const sample of samples) {
    lora.adapt(sample.input, sample.feedback);
}
lora.applyUpdates(0.01); // Once at the end
```

### 2. Choose Optimal Rank

- **Rank 1**: Fastest, minimal memory (~6KB), good for simple adaptations
- **Rank 2**: Best balance, recommended for most use cases (~12KB)
- **Rank 4**: More expressive, use when quality matters more than size (~24KB)

### 3. Learning Rate Guidelines

- Start with `0.01` for general use
- Increase to `0.02-0.05` for faster adaptation
- Decrease to `0.001-0.005` for fine-grained control
- Use adaptive rates based on quality variance

```javascript
const variance = computeQualityVariance(recentSamples);
const adaptiveLR = 0.01 * (1 + variance);
lora.applyUpdates(adaptiveLR);
```

## Comparison with Full LoRA

| Feature | MicroLoRA | Standard LoRA |
|---------|-----------|---------------|
| Memory | 6-24KB | 50-500KB |
| Rank | 1-4 | 8-64 |
| Adaptation | Real-time (<1ms) | Batch (>100ms) |
| Threading | None | Multi-threaded |
| Platform | Browser only | Any |
| Gradients | Simplified | Full backprop |

## Browser Compatibility

Requires:
- WebAssembly support
- Float32Array support
- localStorage for persistence (optional)

Tested on:
- Chrome 90+
- Firefox 88+
- Safari 14+
- Edge 90+

## Advanced: Integration with Base Model

```javascript
async function generateWithLoRA(prompt, lora) {
    // 1. Get base model output and hidden states
    const { output, hiddenStates } = await baseModel.generate(prompt);

    // 2. Apply LoRA transformation to hidden states
    const loraOutput = lora.apply(hiddenStates);

    // 3. Combine (additive)
    const finalHidden = hiddenStates.map((h, i) => h + loraOutput[i]);

    // 4. Project to tokens
    const tokens = await baseModel.projectToTokens(finalHidden);

    return tokens;
}
```

## Troubleshooting

### High Memory Usage

```javascript
// Check actual memory usage
const stats = lora.stats();
console.log(`Memory: ${stats.memoryBytes} bytes`);

// If too high, reduce rank
config.rank = 1; // Instead of 2 or 4
```

### Slow Adaptation

```javascript
// Increase learning rate
feedback.learningRate = 0.05; // Instead of 0.01

// Or apply updates more frequently
if (sampleCount % 3 === 0) { // Instead of % 10
    lora.applyUpdates(0.02);
}
```

### Quality Not Improving

```javascript
// Check if feedback is balanced
const stats = lora.stats();
if (stats.avgQuality < 0.4 || stats.avgQuality > 0.9) {
    console.warn('Feedback may be too one-sided');
}

// Add quality normalization
const normalizedQuality = (rawQuality - minQuality) / (maxQuality - minQuality);
feedback.quality = normalizedQuality;
```

## Examples

See `examples/micro_lora_example.ts` for complete working examples including:
- Basic usage
- Online learning loop
- Serialization/deserialization
- Browser storage integration
- Multi-user scenarios

## License

MIT License - see LICENSE file for details
