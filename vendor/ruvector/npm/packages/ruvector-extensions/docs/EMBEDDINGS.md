# Embeddings Integration Module

Comprehensive embeddings integration for ruvector-extensions, supporting multiple providers with a unified interface.

## Features

✨ **Multi-Provider Support**
- OpenAI (text-embedding-3-small, text-embedding-3-large, ada-002)
- Cohere (embed-english-v3.0, embed-multilingual-v3.0)
- Anthropic/Voyage (voyage-2)
- HuggingFace (local models via transformers.js)

⚡ **Automatic Batch Processing**
- Intelligent batching based on provider limits
- Automatic retry logic with exponential backoff
- Progress tracking for large datasets

🔒 **Type-Safe & Production-Ready**
- Full TypeScript support
- Comprehensive error handling
- JSDoc documentation
- Configurable retry strategies

## Installation

```bash
npm install ruvector-extensions

# Install provider SDKs (optional - based on what you use)
npm install openai              # For OpenAI
npm install cohere-ai           # For Cohere
npm install @anthropic-ai/sdk   # For Anthropic
npm install @xenova/transformers # For local HuggingFace models
```

## Quick Start

### OpenAI Embeddings

```typescript
import { OpenAIEmbeddings } from 'ruvector-extensions';

const openai = new OpenAIEmbeddings({
  apiKey: process.env.OPENAI_API_KEY,
  model: 'text-embedding-3-small', // 1536 dimensions
});

// Embed single text
const embedding = await openai.embedText('Hello, world!');

// Embed multiple texts (automatic batching)
const result = await openai.embedTexts([
  'Machine learning is fascinating',
  'Deep learning uses neural networks',
  'Natural language processing is important',
]);

console.log('Embeddings:', result.embeddings.length);
console.log('Tokens used:', result.totalTokens);
```

### Custom Dimensions (OpenAI)

```typescript
const openai = new OpenAIEmbeddings({
  apiKey: process.env.OPENAI_API_KEY,
  model: 'text-embedding-3-large',
  dimensions: 1024, // Reduce from 3072 to 1024
});

const embedding = await openai.embedText('Custom dimension embedding');
console.log('Dimension:', embedding.length); // 1024
```

### Cohere Embeddings

```typescript
import { CohereEmbeddings } from 'ruvector-extensions';

// For document storage
const documentEmbedder = new CohereEmbeddings({
  apiKey: process.env.COHERE_API_KEY,
  model: 'embed-english-v3.0',
  inputType: 'search_document',
});

// For search queries
const queryEmbedder = new CohereEmbeddings({
  apiKey: process.env.COHERE_API_KEY,
  model: 'embed-english-v3.0',
  inputType: 'search_query',
});

const docs = await documentEmbedder.embedTexts([
  'The Eiffel Tower is in Paris',
  'The Statue of Liberty is in New York',
]);

const query = await queryEmbedder.embedText('famous landmarks in France');
```

### Anthropic/Voyage Embeddings

```typescript
import { AnthropicEmbeddings } from 'ruvector-extensions';

const anthropic = new AnthropicEmbeddings({
  apiKey: process.env.VOYAGE_API_KEY,
  model: 'voyage-2',
  inputType: 'document',
});

const result = await anthropic.embedTexts([
  'Anthropic develops Claude AI',
  'Voyage AI provides embedding models',
]);
```

### Local HuggingFace Embeddings

```typescript
import { HuggingFaceEmbeddings } from 'ruvector-extensions';

// No API key needed - runs locally!
const hf = new HuggingFaceEmbeddings({
  model: 'Xenova/all-MiniLM-L6-v2',
  normalize: true,
  batchSize: 32,
});

const result = await hf.embedTexts([
  'Local embeddings are fast',
  'No API calls required',
  'Privacy-friendly solution',
]);
```

## VectorDB Integration

### Insert Documents

```typescript
import { VectorDB } from 'ruvector';
import { OpenAIEmbeddings, embedAndInsert } from 'ruvector-extensions';

const openai = new OpenAIEmbeddings({
  apiKey: process.env.OPENAI_API_KEY,
});

const db = new VectorDB({ dimension: openai.getDimension() });

const documents = [
  {
    id: 'doc1',
    text: 'Machine learning enables computers to learn from data',
    metadata: { category: 'AI', author: 'John Doe' },
  },
  {
    id: 'doc2',
    text: 'Deep learning uses neural networks',
    metadata: { category: 'AI', author: 'Jane Smith' },
  },
];

const ids = await embedAndInsert(db, openai, documents, {
  overwrite: true,
  onProgress: (current, total) => {
    console.log(`Progress: ${current}/${total}`);
  },
});

console.log('Inserted IDs:', ids);
```

### Search Documents

```typescript
import { embedAndSearch } from 'ruvector-extensions';

const results = await embedAndSearch(
  db,
  openai,
  'What is deep learning?',
  {
    topK: 5,
    threshold: 0.7,
    filter: { category: 'AI' },
  }
);

console.log('Search results:', results);
```

## Advanced Features

### Custom Retry Configuration

```typescript
const openai = new OpenAIEmbeddings({
  apiKey: process.env.OPENAI_API_KEY,
  retryConfig: {
    maxRetries: 5,
    initialDelay: 2000,      // 2 seconds
    maxDelay: 30000,         // 30 seconds
    backoffMultiplier: 2,    // Exponential backoff
  },
});
```

### Batch Processing Large Datasets

```typescript
// Automatically handles batching based on provider limits
const largeDataset = Array.from({ length: 10000 }, (_, i) =>
  `Document ${i}: Sample text for embedding`
);

const result = await openai.embedTexts(largeDataset);
console.log(`Processed ${result.embeddings.length} documents`);
console.log(`Total tokens: ${result.totalTokens}`);
```

### Error Handling

```typescript
try {
  const result = await openai.embedTexts(['Test text']);
  console.log('Success!');
} catch (error) {
  if (error.retryable) {
    console.log('Temporary error - can retry');
  } else {
    console.log('Permanent error - fix required');
  }
  console.error('Error:', error.message);
}
```

### Progress Tracking

```typescript
const progressBar = (current: number, total: number) => {
  const percentage = Math.round((current / total) * 100);
  console.log(`[${percentage}%] ${current}/${total}`);
};

await embedAndInsert(db, openai, documents, {
  onProgress: progressBar,
});
```

## Provider Comparison

| Provider | Dimension | Max Batch | API Required | Local |
|----------|-----------|-----------|--------------|-------|
| OpenAI text-embedding-3-small | 1536 | 2048 | ✅ | ❌ |
| OpenAI text-embedding-3-large | 3072 (configurable) | 2048 | ✅ | ❌ |
| Cohere embed-v3.0 | 1024 | 96 | ✅ | ❌ |
| Anthropic/Voyage | 1024 | 128 | ✅ | ❌ |
| HuggingFace (local) | 384 (model-dependent) | Configurable | ❌ | ✅ |

## API Reference

### `EmbeddingProvider` (Abstract Base Class)

```typescript
abstract class EmbeddingProvider {
  // Get maximum batch size
  abstract getMaxBatchSize(): number;

  // Get embedding dimension
  abstract getDimension(): number;

  // Embed single text
  async embedText(text: string): Promise<number[]>;

  // Embed multiple texts
  abstract embedTexts(texts: string[]): Promise<BatchEmbeddingResult>;
}
```

### `OpenAIEmbeddingsConfig`

```typescript
interface OpenAIEmbeddingsConfig {
  apiKey: string;
  model?: string; // Default: 'text-embedding-3-small'
  dimensions?: number; // Only for text-embedding-3-* models
  organization?: string;
  baseURL?: string;
  retryConfig?: Partial<RetryConfig>;
}
```

### `CohereEmbeddingsConfig`

```typescript
interface CohereEmbeddingsConfig {
  apiKey: string;
  model?: string; // Default: 'embed-english-v3.0'
  inputType?: 'search_document' | 'search_query' | 'classification' | 'clustering';
  truncate?: 'NONE' | 'START' | 'END';
  retryConfig?: Partial<RetryConfig>;
}
```

### `AnthropicEmbeddingsConfig`

```typescript
interface AnthropicEmbeddingsConfig {
  apiKey: string; // Voyage API key
  model?: string; // Default: 'voyage-2'
  inputType?: 'document' | 'query';
  retryConfig?: Partial<RetryConfig>;
}
```

### `HuggingFaceEmbeddingsConfig`

```typescript
interface HuggingFaceEmbeddingsConfig {
  model?: string; // Default: 'Xenova/all-MiniLM-L6-v2'
  device?: 'cpu' | 'cuda';
  normalize?: boolean; // Default: true
  batchSize?: number; // Default: 32
  retryConfig?: Partial<RetryConfig>;
}
```

### `embedAndInsert`

```typescript
async function embedAndInsert(
  db: VectorDB,
  provider: EmbeddingProvider,
  documents: DocumentToEmbed[],
  options?: {
    overwrite?: boolean;
    onProgress?: (current: number, total: number) => void;
  }
): Promise<string[]>;
```

### `embedAndSearch`

```typescript
async function embedAndSearch(
  db: VectorDB,
  provider: EmbeddingProvider,
  query: string,
  options?: {
    topK?: number;
    threshold?: number;
    filter?: Record<string, unknown>;
  }
): Promise<any[]>;
```

## Best Practices

1. **Choose the Right Provider**
   - OpenAI: Best general-purpose, flexible dimensions
   - Cohere: Optimized for search, separate document/query embeddings
   - Anthropic/Voyage: High quality, good for semantic search
   - HuggingFace: Privacy-focused, no API costs, offline support

2. **Batch Processing**
   - Let the library handle batching automatically
   - Use progress callbacks for large datasets
   - Consider memory usage for very large datasets

3. **Error Handling**
   - Configure retry logic for production environments
   - Handle rate limits gracefully
   - Log errors with context for debugging

4. **Performance**
   - Use custom dimensions (OpenAI) to reduce storage
   - Cache embeddings when possible
   - Consider local models for high-volume use cases

5. **Security**
   - Store API keys in environment variables
   - Never commit API keys to version control
   - Use key rotation for production systems

## Examples

See [src/examples/embeddings-example.ts](../src/examples/embeddings-example.ts) for comprehensive examples including:

- Basic usage for all providers
- Batch processing
- Error handling
- VectorDB integration
- Progress tracking
- Provider comparison

## Troubleshooting

### "Module not found" errors

Make sure you've installed the required provider SDK:

```bash
npm install openai        # For OpenAI
npm install cohere-ai     # For Cohere
npm install @xenova/transformers  # For HuggingFace
```

### Rate limit errors

Configure retry logic with longer delays:

```typescript
const provider = new OpenAIEmbeddings({
  apiKey: '...',
  retryConfig: {
    maxRetries: 5,
    initialDelay: 5000,
    maxDelay: 60000,
  },
});
```

### Dimension mismatches

Ensure VectorDB dimension matches provider dimension:

```typescript
const db = new VectorDB({
  dimension: provider.getDimension()
});
```

## License

MIT © ruv.io Team

## Support

- GitHub Issues: https://github.com/ruvnet/ruvector/issues
- Documentation: https://github.com/ruvnet/ruvector
- Email: info@ruv.io
