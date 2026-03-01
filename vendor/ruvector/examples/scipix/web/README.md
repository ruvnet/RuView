# Scipix WASM - WebAssembly OCR

High-performance OCR with LaTeX support for the browser, powered by WebAssembly.

## Features

- 📸 **Image OCR**: Recognize text from images
- 🧮 **LaTeX Support**: Extract mathematical formulas
- ⚡ **Web Workers**: Off-main-thread processing
- 🎯 **TypeScript**: Full type definitions
- 🚀 **Optimized**: <2MB bundle size
- 🔧 **Flexible API**: Multiple input formats

## Quick Start

### Installation

```bash
npm install ruvector-scipix-wasm
```

### Build from Source

```bash
cd examples/scipix
npm run build
```

### Basic Usage

```javascript
import { createScipix } from 'ruvector-scipix-wasm';

// Initialize
const scipix = await createScipix();

// Recognize from file
const result = await scipix.recognize(imageData);
console.log(result.text);
console.log(result.latex);
```

### Canvas Example

```javascript
import { recognizeCanvas } from 'ruvector-scipix-wasm';

const canvas = document.getElementById('myCanvas');
const result = await recognizeCanvas(canvas);
```

### Web Worker Example

```javascript
import { createWorker } from 'ruvector-scipix-wasm';

const worker = createWorker();

// Process in background
const result = await worker.recognize(imageData);

// Batch processing with progress
const results = await worker.recognizeBatch(images, {
  onProgress: ({ processed, total }) => {
    console.log(`Progress: ${processed}/${total}`);
  }
});

worker.terminate();
```

## API Reference

### `createScipix(options?)`

Create a new Scipix instance.

```typescript
const scipix = await createScipix({
  format: 'both',           // 'text' | 'latex' | 'both'
  confidenceThreshold: 0.5  // 0.0 - 1.0
});
```

### `ScipixWasm`

Main API class.

#### Methods

- `recognize(imageData: Uint8Array): Promise<OcrResult>`
- `recognizeFromCanvas(canvas: HTMLCanvasElement): Promise<OcrResult>`
- `recognizeBase64(base64: string): Promise<OcrResult>`
- `recognizeImageData(imageData: ImageData): Promise<OcrResult>`
- `recognizeBatch(images: Uint8Array[]): Promise<OcrResult[]>`
- `setFormat(format: RecognitionFormat): void`
- `setConfidenceThreshold(threshold: number): void`
- `getVersion(): string`

### Helper Functions

```javascript
import {
  recognizeFile,      // From File/Blob
  recognizeCanvas,    // From HTMLCanvasElement
  recognizeBase64,    // From base64 string
  recognizeUrl,       // From image URL
  recognizeBatch,     // Batch processing
  imageToCanvas,      // Convert image to canvas
} from 'ruvector-scipix-wasm';
```

## Types

### `OcrResult`

```typescript
interface OcrResult {
  text: string;           // Recognized text
  latex?: string;         // LaTeX (if enabled)
  confidence: number;     // 0.0 - 1.0
  metadata?: {
    width?: number;
    height?: number;
    format?: string;
  };
}
```

### `RecognitionFormat`

```typescript
type RecognitionFormat = 'text' | 'latex' | 'both';
```

## Demo

Run the interactive demo:

```bash
npm run dev
```

Open http://localhost:8080/example.html

## Performance Tips

1. **Use Web Workers** for large images or batch processing
2. **Set confidence threshold** to filter low-quality results
3. **Resize images** before processing if possible
4. **Reuse instances** instead of creating new ones
5. **Use SharedImageBuffer** for large image batches

## Browser Support

- Chrome 57+
- Firefox 52+
- Safari 11+
- Edge 16+

Requires WebAssembly support.

## Bundle Size

- WASM module: ~800KB (gzipped)
- JavaScript wrapper: ~15KB (gzipped)
- **Total: <1MB**

## License

MIT

## Credits

Built with:
- [wasm-bindgen](https://github.com/rustwasm/wasm-bindgen)
- [image-rs](https://github.com/image-rs/image)
- [ruvector](https://github.com/ruvnet/ruvector)
