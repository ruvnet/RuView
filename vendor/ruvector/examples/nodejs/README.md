# RuVector Node.js Examples

JavaScript/TypeScript examples for integrating RuVector with Node.js applications.

## Examples

| File | Description |
|------|-------------|
| `basic_usage.js` | Getting started with the JS SDK |
| `semantic_search.js` | Semantic search implementation |

## Quick Start

```bash
npm install ruvector
node basic_usage.js
node semantic_search.js
```

## Basic Usage

```javascript
const { VectorDB } = require('ruvector');

async function main() {
    // Initialize database
    const db = new VectorDB({
        dimensions: 128,
        storagePath: './my_vectors.db'
    });
    await db.initialize();

    // Insert vectors
    await db.insert({
        id: 'doc_001',
        vector: new Float32Array(128).fill(0.1),
        metadata: { title: 'Document 1' }
    });

    // Search
    const results = await db.search({
        vector: new Float32Array(128).fill(0.1),
        topK: 10
    });

    console.log('Results:', results);
}

main().catch(console.error);
```

## Semantic Search

```javascript
const { VectorDB } = require('ruvector');
const { encode } = require('your-embedding-model');

async function semanticSearch() {
    const db = new VectorDB({ dimensions: 384 });
    await db.initialize();

    // Index documents
    const documents = [
        'Machine learning is a subset of AI',
        'Neural networks power modern AI',
        'Deep learning uses multiple layers'
    ];

    for (const doc of documents) {
        const embedding = await encode(doc);
        await db.insert({
            id: doc.slice(0, 20),
            vector: embedding,
            metadata: { text: doc }
        });
    }

    // Search by meaning
    const query = 'How does artificial intelligence work?';
    const queryVec = await encode(query);

    const results = await db.search({
        vector: queryVec,
        topK: 5
    });

    results.forEach(r => {
        console.log(`${r.score.toFixed(3)}: ${r.metadata.text}`);
    });
}
```

## Batch Operations

```javascript
// Batch insert for efficiency
const entries = documents.map((doc, i) => ({
    id: `doc_${i}`,
    vector: embeddings[i],
    metadata: { text: doc }
}));

await db.insertBatch(entries);

// Batch search
const queries = ['query1', 'query2', 'query3'];
const queryVectors = await Promise.all(queries.map(encode));

const batchResults = await db.searchBatch(
    queryVectors.map(v => ({ vector: v, topK: 5 }))
);
```

## Filtering

```javascript
// Metadata filtering
const results = await db.search({
    vector: queryVec,
    topK: 10,
    filter: {
        category: { $eq: 'technology' },
        date: { $gte: '2024-01-01' }
    }
});
```

## TypeScript

```typescript
import { VectorDB, VectorEntry, SearchResult } from 'ruvector';

interface DocMetadata {
    title: string;
    author: string;
    date: string;
}

const db = new VectorDB<DocMetadata>({
    dimensions: 384
});

const entry: VectorEntry<DocMetadata> = {
    id: 'doc_001',
    vector: new Float32Array(384),
    metadata: {
        title: 'TypeScript Guide',
        author: 'Dev Team',
        date: '2024-01-01'
    }
};

await db.insert(entry);
```

## Express.js Integration

```javascript
const express = require('express');
const { VectorDB } = require('ruvector');

const app = express();
const db = new VectorDB({ dimensions: 384 });

app.post('/search', express.json(), async (req, res) => {
    const { query, topK = 10 } = req.body;
    const queryVec = await encode(query);

    const results = await db.search({
        vector: queryVec,
        topK
    });

    res.json(results);
});

app.listen(3000);
```

## Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `dimensions` | number | required | Vector dimensions |
| `storagePath` | string | `:memory:` | Database file path |
| `metric` | string | `cosine` | Distance metric |
| `indexType` | string | `hnsw` | Index algorithm |

## Error Handling

```javascript
try {
    await db.insert(entry);
} catch (error) {
    if (error.code === 'DIMENSION_MISMATCH') {
        console.error('Vector dimension mismatch');
    } else if (error.code === 'DUPLICATE_ID') {
        console.error('ID already exists');
    } else {
        throw error;
    }
}
```

## Performance Tips

1. Use batch operations for bulk inserts
2. Keep vector dimensions consistent
3. Use appropriate index for query patterns
4. Consider in-memory mode for speed
