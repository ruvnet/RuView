# RuVector React + WebAssembly Example

Modern React application with RuVector running entirely in the browser via WebAssembly.

## Features

- Client-side vector database
- Real-time similarity search
- Zero server dependencies
- Full React integration

## Quick Start

```bash
npm install
npm run dev
```

Open http://localhost:5173 in your browser.

## Project Structure

```
wasm-react/
├── index.html       # Entry HTML
├── main.jsx         # React entry point
├── App.jsx          # Main application
├── package.json     # Dependencies
└── vite.config.js   # Vite configuration
```

## Usage

```jsx
import React, { useState, useEffect } from 'react';
import init, { VectorDB } from 'ruvector-wasm';

function App() {
    const [db, setDb] = useState(null);
    const [results, setResults] = useState([]);

    useEffect(() => {
        async function setup() {
            await init();
            const vectorDb = new VectorDB(128);
            setDb(vectorDb);
        }
        setup();
    }, []);

    const handleSearch = async (query) => {
        if (!db) return;

        const queryVector = await getEmbedding(query);
        const searchResults = db.search(queryVector, 10);
        setResults(searchResults);
    };

    return (
        <div>
            <SearchInput onSearch={handleSearch} />
            <ResultsList results={results} />
        </div>
    );
}
```

## Hooks

### useVectorDB

```jsx
function useVectorDB(dimensions) {
    const [db, setDb] = useState(null);
    const [ready, setReady] = useState(false);

    useEffect(() => {
        let mounted = true;

        async function initialize() {
            await init();
            if (mounted) {
                setDb(new VectorDB(dimensions));
                setReady(true);
            }
        }

        initialize();
        return () => { mounted = false; };
    }, [dimensions]);

    return { db, ready };
}
```

### useSemanticSearch

```jsx
function useSemanticSearch(db, embedding) {
    const [results, setResults] = useState([]);
    const [loading, setLoading] = useState(false);

    useEffect(() => {
        if (!db || !embedding) return;

        setLoading(true);
        const searchResults = db.search(embedding, 10);
        setResults(searchResults);
        setLoading(false);
    }, [db, embedding]);

    return { results, loading };
}
```

## Performance

- **Initial Load**: ~500KB WASM bundle (gzipped)
- **Memory**: ~50MB for 100K vectors (128d)
- **Search Latency**: <10ms for 100K vectors

## Configuration

```javascript
// vite.config.js
export default {
    plugins: [],
    optimizeDeps: {
        exclude: ['ruvector-wasm']
    },
    build: {
        target: 'esnext'
    }
};
```

## Browser Support

- Chrome 89+
- Firefox 89+
- Safari 15+
- Edge 89+

## Dependencies

```json
{
    "dependencies": {
        "react": "^18.2.0",
        "react-dom": "^18.2.0",
        "ruvector-wasm": "^0.1.0"
    },
    "devDependencies": {
        "@vitejs/plugin-react": "^4.0.0",
        "vite": "^5.0.0"
    }
}
```

## Deployment

```bash
npm run build
# Deploy dist/ to any static hosting
```

Works with:
- Vercel
- Netlify
- GitHub Pages
- Cloudflare Pages
- Any CDN

## Related

- [WASM Vanilla Example](../wasm-vanilla/README.md)
- [Graph WASM Usage](../docs/graph_wasm_usage.html)
