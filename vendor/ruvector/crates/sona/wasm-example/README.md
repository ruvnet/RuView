# SONA WASM Example

Interactive browser demo of the Self-Optimizing Neural Architecture (SONA).

## Quick Start

1. Build the WASM module (if not already built):
```bash
cd ..
wasm-pack build --target web --features wasm
cp -r pkg wasm-example/
```

2. Serve the example:
```bash
cd wasm-example
python3 -m http.server 8080
```

3. Open in browser:
```
http://localhost:8080
```

## Features

- **Real-time Learning**: Record trajectories and see instant updates
- **LoRA Visualization**: Watch transformation in real-time
- **Statistics Dashboard**: Monitor patterns, quality, and performance
- **Interactive Controls**: Adjust configuration and run experiments

## Files

- `index.html` - Demo page with UI
- `index.js` - JavaScript logic using WASM bindings
- `package.json` - NPM configuration
- `pkg/` - Generated WASM package
  - `sona.js` - JavaScript bindings
  - `sona_bg.wasm` - WebAssembly binary
  - `sona.d.ts` - TypeScript definitions

## Usage Example

```javascript
import init, { WasmSonaEngine } from './pkg/sona.js';

async function main() {
  await init();
  
  const engine = new WasmSonaEngine(256);
  const trajectoryId = engine.start_trajectory(new Float32Array(256).fill(0.1));
  engine.record_step(trajectoryId, 42, 0.8, 1000);
  engine.end_trajectory(trajectoryId, 0.85);
  
  const output = engine.apply_lora(new Float32Array(256).fill(1.0));
  console.log('Transformed output:', output);
}

main();
```

## Performance

- WASM file size: ~1.5MB (release build)
- Initialization: < 100ms
- Per-trajectory overhead: < 1ms
- LoRA application: < 0.1ms (256-dim)

## Browser Support

- Chrome/Edge 91+
- Firefox 89+
- Safari 14.1+

## License

MIT OR Apache-2.0
