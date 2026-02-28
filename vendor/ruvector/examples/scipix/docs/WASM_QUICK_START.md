# WebAssembly Quick Start Guide

## Build WASM Module

```bash
cd examples/scipix

# Install wasm-pack (if not already installed)
cargo install wasm-pack

# Build for web (production)
wasm-pack build --target web --out-dir web/pkg --release -- --features wasm

# Build for development
wasm-pack build --target web --out-dir web/pkg --dev -- --features wasm
```

## Run Demo

```bash
cd web
npm install
npm run serve
```

Open http://localhost:8080/example.html

## Basic Usage

### Initialize

```javascript
import { createScipix } from './web/index.js';

const scipix = await createScipix({
  format: 'both',              // 'text' | 'latex' | 'both'
  confidenceThreshold: 0.5     // 0.0 - 1.0
});
```

### From File Input

```javascript
const input = document.querySelector('input[type="file"]');
const file = input.files[0];

const result = await scipix.recognize(
  new Uint8Array(await file.arrayBuffer())
);

console.log('Text:', result.text);
console.log('LaTeX:', result.latex);
console.log('Confidence:', result.confidence);
```

### From Canvas

```javascript
const canvas = document.getElementById('myCanvas');
const result = await scipix.recognizeFromCanvas(canvas);
```

### From Base64

```javascript
const base64 = 'data:image/png;base64,iVBORw0KG...';
const result = await scipix.recognizeBase64(base64);
```

### With Web Worker

```javascript
import { createWorker } from './web/index.js';

const worker = createWorker();

// Single image
const result = await worker.recognize(imageData);

// Batch with progress
const results = await worker.recognizeBatch(images, {
  onProgress: ({ processed, total }) => {
    console.log(`Progress: ${processed}/${total}`);
  }
});

worker.terminate();
```

## Integration Examples

### React

```jsx
import { useEffect, useState } from 'react';
import { createScipix } from 'ruvector-scipix-wasm';

function OcrComponent() {
  const [scipix, setScipix] = useState(null);
  const [result, setResult] = useState(null);

  useEffect(() => {
    createScipix().then(setScipix);
  }, []);

  const handleFile = async (e) => {
    const file = e.target.files[0];
    const data = new Uint8Array(await file.arrayBuffer());
    const res = await scipix.recognize(data);
    setResult(res);
  };

  return (
    <div>
      <input type="file" onChange={handleFile} />
      {result && (
        <div>
          <p>Text: {result.text}</p>
          <p>LaTeX: {result.latex}</p>
          <p>Confidence: {(result.confidence * 100).toFixed(1)}%</p>
        </div>
      )}
    </div>
  );
}
```

### Vue

```vue
<template>
  <div>
    <input type="file" @change="handleFile" />
    <div v-if="result">
      <p>Text: {{ result.text }}</p>
      <p>LaTeX: {{ result.latex }}</p>
      <p>Confidence: {{ (result.confidence * 100).toFixed(1) }}%</p>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue';
import { createScipix } from 'ruvector-scipix-wasm';

const scipix = ref(null);
const result = ref(null);

onMounted(async () => {
  scipix.value = await createScipix();
});

const handleFile = async (e) => {
  const file = e.target.files[0];
  const data = new Uint8Array(await file.arrayBuffer());
  result.value = await scipix.value.recognize(data);
};
</script>
```

### Svelte

```svelte
<script>
  import { onMount } from 'svelte';
  import { createScipix } from 'ruvector-scipix-wasm';

  let scipix;
  let result;

  onMount(async () => {
    scipix = await createScipix();
  });

  async function handleFile(e) {
    const file = e.target.files[0];
    const data = new Uint8Array(await file.arrayBuffer());
    result = await scipix.recognize(data);
  }
</script>

<input type="file" on:change={handleFile} />

{#if result}
  <div>
    <p>Text: {result.text}</p>
    <p>LaTeX: {result.latex}</p>
    <p>Confidence: {(result.confidence * 100).toFixed(1)}%</p>
  </div>
{/if}
```

## Build Configuration

### Webpack

```javascript
// webpack.config.js
module.exports = {
  experiments: {
    asyncWebAssembly: true,
  },
  module: {
    rules: [
      {
        test: /\.wasm$/,
        type: 'webassembly/async',
      },
    ],
  },
};
```

### Vite

```javascript
// vite.config.js
export default {
  optimizeDeps: {
    exclude: ['ruvector-scipix-wasm']
  }
};
```

## Browser Compatibility

Minimum required versions:
- Chrome 57+
- Firefox 52+
- Safari 11+
- Edge 16+

Required features:
- WebAssembly
- ES6 Modules
- Async/Await
- (Optional) Web Workers

## Performance Tips

1. **Preload WASM**: Initialize early in your app lifecycle
2. **Reuse instances**: Don't create new instances for each operation
3. **Use workers**: For images larger than 1MB
4. **Batch operations**: Group similar processing tasks
5. **Set threshold**: Filter low-confidence results

## Troubleshooting

### CORS Errors

If loading from CDN, ensure CORS headers are set:
```
Access-Control-Allow-Origin: *
```

### Memory Issues

For large batches, process in chunks:
```javascript
const chunkSize = 10;
for (let i = 0; i < images.length; i += chunkSize) {
  const chunk = images.slice(i, i + chunkSize);
  const results = await worker.recognizeBatch(chunk);
  // Process results
}
```

### Initialization Fails

Check that WASM file is accessible:
```javascript
try {
  const scipix = await createScipix();
} catch (error) {
  console.error('Failed to initialize:', error);
  // Fallback to server-side processing
}
```

## Next Steps

- Read [WASM Architecture](./WASM_ARCHITECTURE.md)
- Check [API Reference](../web/README.md)
- View [Example Demo](../web/example.html)
- See [TypeScript Definitions](../web/types.ts)
