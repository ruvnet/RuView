# RuVector Vanilla WebAssembly Example

Pure JavaScript WebAssembly integration without any framework dependencies.

## Features

- Zero dependencies
- Single HTML file
- Direct WASM usage
- Browser-native

## Quick Start

```bash
# Serve the directory
python -m http.server 8080
# Or use any static file server
npx serve .
```

Open http://localhost:8080 in your browser.

## Usage

```html
<!DOCTYPE html>
<html>
<head>
    <title>RuVector WASM Demo</title>
</head>
<body>
    <input type="text" id="query" placeholder="Search...">
    <button onclick="search()">Search</button>
    <div id="results"></div>

    <script type="module">
        import init, { VectorDB } from './ruvector_wasm.js';

        let db;

        async function setup() {
            await init();
            db = new VectorDB(128);

            // Add sample data
            for (let i = 0; i < 1000; i++) {
                const vector = new Float32Array(128)
                    .map(() => Math.random());
                db.insert(`doc_${i}`, vector);
            }

            console.log('Database ready with 1000 vectors');
        }

        window.search = function() {
            const query = document.getElementById('query').value;
            const queryVec = new Float32Array(128)
                .map(() => Math.random());

            const results = db.search(queryVec, 10);
            displayResults(results);
        };

        function displayResults(results) {
            const container = document.getElementById('results');
            container.innerHTML = results
                .map(r => `<div>${r.id}: ${r.score.toFixed(4)}</div>`)
                .join('');
        }

        setup();
    </script>
</body>
</html>
```

## API Reference

### Initialization

```javascript
import init, { VectorDB } from './ruvector_wasm.js';

// Initialize WASM module
await init();

// Create database (dimensions required)
const db = new VectorDB(128);
```

### Insert

```javascript
// Single insert
const vector = new Float32Array([0.1, 0.2, ...]);
db.insert('id_1', vector);

// With metadata (JSON string)
db.insert_with_metadata('id_2', vector, '{"title":"Doc"}');
```

### Search

```javascript
const queryVec = new Float32Array(128);
const results = db.search(queryVec, 10);

// Results array
results.forEach(result => {
    console.log(result.id);      // Document ID
    console.log(result.score);   // Similarity score
    console.log(result.vector);  // Original vector
});
```

### Delete

```javascript
db.delete('id_1');
```

### Statistics

```javascript
const stats = db.stats();
console.log(stats.count);      // Number of vectors
console.log(stats.dimensions); // Vector dimensions
```

## Memory Management

```javascript
// Vectors are automatically memory-managed
// For large operations, consider batching

const BATCH_SIZE = 1000;
for (let batch = 0; batch < totalVectors; batch += BATCH_SIZE) {
    const vectors = getVectorBatch(batch, BATCH_SIZE);
    vectors.forEach((v, i) => db.insert(`id_${batch + i}`, v));
}
```

## Browser Compatibility

| Browser | Min Version |
|---------|-------------|
| Chrome | 89 |
| Firefox | 89 |
| Safari | 15 |
| Edge | 89 |

## Performance

| Operation | 10K vectors | 100K vectors |
|-----------|-------------|--------------|
| Insert | ~50ms | ~500ms |
| Search (k=10) | <5ms | <10ms |
| Memory | ~5MB | ~50MB |

## Embedding Integration

```javascript
// Using Transformers.js for embeddings
import { pipeline } from '@xenova/transformers';

const embedder = await pipeline(
    'feature-extraction',
    'Xenova/all-MiniLM-L6-v2'
);

async function getEmbedding(text) {
    const output = await embedder(text, {
        pooling: 'mean',
        normalize: true
    });
    return output.data;
}

// Index document
const embedding = await getEmbedding('Document text');
db.insert('doc_1', embedding);

// Search
const queryEmbed = await getEmbedding('Search query');
const results = db.search(queryEmbed, 10);
```

## Related

- [React + WASM Example](../wasm-react/README.md)
- [Graph WASM Usage](../docs/graph_wasm_usage.html)
