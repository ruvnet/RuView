# API Reference

Complete API documentation for Agentic-Synth.

## Table of Contents

- [SynthEngine](#synthengine)
- [Schema](#schema)
- [Generators](#generators)
- [Templates](#templates)
- [Quality Metrics](#quality-metrics)
- [Integrations](#integrations)
- [Types](#types)

---

## SynthEngine

The main entry point for synthetic data generation.

### Constructor

```typescript
new SynthEngine(config: SynthEngineConfig)
```

#### Parameters

```typescript
interface SynthEngineConfig {
  // LLM Provider Configuration
  provider?: 'openai' | 'anthropic' | 'cohere' | 'custom';
  model?: string;
  apiKey?: string;
  temperature?: number;           // 0.0 - 1.0
  maxTokens?: number;

  // Vector Database Configuration
  vectorDB?: 'ruvector' | 'agenticdb' | VectorDBInstance;
  embeddingModel?: string;
  embeddingDimensions?: number;

  // Generation Configuration
  batchSize?: number;             // Default: 100
  maxWorkers?: number;            // Default: 4
  streaming?: boolean;            // Default: false
  cacheEnabled?: boolean;         // Default: true

  // Quality Configuration
  minQuality?: number;            // 0.0 - 1.0, default: 0.85
  validationEnabled?: boolean;    // Default: true
  retryOnLowQuality?: boolean;   // Default: true
}
```

#### Example

```typescript
import { SynthEngine } from 'agentic-synth';

const synth = new SynthEngine({
  provider: 'openai',
  model: 'gpt-4',
  temperature: 0.8,
  vectorDB: 'ruvector',
  batchSize: 1000,
  streaming: true,
});
```

### Methods

#### generate()

Generate synthetic data based on a schema.

```typescript
async generate<T>(options: GenerateOptions): Promise<GeneratedData<T>>
```

**Parameters:**

```typescript
interface GenerateOptions {
  schema: Schema;
  count: number;
  streaming?: boolean;
  progressCallback?: (progress: Progress) => void;
  abortSignal?: AbortSignal;
}

interface Progress {
  current: number;
  total: number;
  rate: number;          // Items per second
  estimatedTimeRemaining: number;  // Seconds
}
```

**Returns:**

```typescript
interface GeneratedData<T> {
  data: T[];
  metadata: {
    count: number;
    schema: Schema;
    quality: QualityMetrics;
    duration: number;
  };

  // Methods
  export(options: ExportOptions): Promise<void>;
  filter(predicate: (item: T) => boolean): GeneratedData<T>;
  map<U>(mapper: (item: T) => U): GeneratedData<U>;
  toJSON(): string;
  toCSV(): string;
  toParquet(): Buffer;
}
```

**Example:**

```typescript
const result = await synth.generate({
  schema: customerSupportSchema,
  count: 1000,
  streaming: true,
  progressCallback: (progress) => {
    console.log(`Progress: ${progress.current}/${progress.total}`);
  },
});

console.log('Quality:', result.metadata.quality);
await result.export({ format: 'jsonl', outputPath: './data.jsonl' });
```

#### generateStream()

Generate data as an async iterator for real-time processing.

```typescript
async *generateStream<T>(options: GenerateOptions): AsyncIterator<T>
```

**Example:**

```typescript
for await (const item of synth.generateStream({ schema, count: 10000 })) {
  // Process item in real-time
  await processItem(item);
}
```

#### generateAndInsert()

Generate and directly insert into vector database.

```typescript
async generateAndInsert(options: GenerateAndInsertOptions): Promise<InsertResult>
```

**Parameters:**

```typescript
interface GenerateAndInsertOptions extends GenerateOptions {
  collection: string;
  batchSize?: number;
  includeEmbeddings?: boolean;
}

interface InsertResult {
  inserted: number;
  failed: number;
  duration: number;
  errors: Error[];
}
```

**Example:**

```typescript
const result = await synth.generateAndInsert({
  schema: productSchema,
  count: 10000,
  collection: 'products',
  batchSize: 1000,
  includeEmbeddings: true,
});

console.log(`Inserted ${result.inserted} items`);
```

---

## Schema

Schema definition system for structured data generation.

### Schema.define()

Define a custom schema.

```typescript
Schema.define(definition: SchemaDefinition): Schema
```

**Parameters:**

```typescript
interface SchemaDefinition {
  name: string;
  description?: string;
  type: 'object' | 'array' | 'conversation' | 'embedding';

  // For object types
  properties?: Record<string, PropertyDefinition>;
  required?: string[];

  // For array types
  items?: SchemaDefinition;
  minItems?: number;
  maxItems?: number;

  // For conversation types
  personas?: PersonaDefinition[];
  turns?: { min: number; max: number };

  // Additional constraints
  constraints?: Constraint[];
  distribution?: DistributionSpec;
}

interface PropertyDefinition {
  type: 'string' | 'number' | 'boolean' | 'date' | 'email' | 'url' | 'embedding';
  description?: string;
  format?: string;
  enum?: any[];
  minimum?: number;
  maximum?: number;
  pattern?: string;
  default?: any;
}

interface PersonaDefinition {
  name: string;
  traits: string[];
  temperature?: number;
  examples?: string[];
}
```

**Example:**

```typescript
const userSchema = Schema.define({
  name: 'User',
  type: 'object',
  properties: {
    id: { type: 'string', format: 'uuid' },
    name: { type: 'string' },
    email: { type: 'email' },
    age: { type: 'number', minimum: 18, maximum: 100 },
    role: { type: 'string', enum: ['admin', 'user', 'guest'] },
    createdAt: { type: 'date' },
    bio: { type: 'string' },
    embedding: { type: 'embedding', dimensions: 384 },
  },
  required: ['id', 'name', 'email'],
});
```

### Pre-defined Schemas

#### Schema.conversation()

```typescript
Schema.conversation(options: ConversationOptions): Schema
```

```typescript
interface ConversationOptions {
  domain: string;
  personas: string[] | PersonaDefinition[];
  topics?: string[];
  turns: { min: number; max: number };
  includeEmbeddings?: boolean;
}
```

**Example:**

```typescript
const supportSchema = Schema.conversation({
  domain: 'customer-support',
  personas: [
    { name: 'customer', traits: ['frustrated', 'confused'] },
    { name: 'agent', traits: ['helpful', 'professional'] },
  ],
  topics: ['billing', 'technical', 'shipping'],
  turns: { min: 4, max: 12 },
});
```

#### Schema.embedding()

```typescript
Schema.embedding(options: EmbeddingOptions): Schema
```

```typescript
interface EmbeddingOptions {
  dimensions: number;
  domain: string;
  clusters?: number;
  distribution?: 'gaussian' | 'uniform' | 'clustered';
}
```

---

## Generators

Specialized generators for common use cases.

### RAGDataGenerator

Generate question-answer pairs for RAG systems.

```typescript
class RAGDataGenerator {
  static async create(options: RAGOptions): Promise<GeneratedData<RAGPair>>
}
```

**Parameters:**

```typescript
interface RAGOptions {
  domain: string;
  sources?: string[];           // File paths or URLs
  questionsPerSource?: number;
  includeNegatives?: boolean;   // For contrastive learning
  difficulty?: 'easy' | 'medium' | 'hard' | 'mixed';
}

interface RAGPair {
  question: string;
  answer: string;
  context: string;
  embedding?: number[];
  metadata: {
    source: string;
    difficulty: string;
    type: 'positive' | 'negative';
  };
}
```

**Example:**

```typescript
const ragData = await RAGDataGenerator.create({
  domain: 'technical-documentation',
  sources: ['./docs/**/*.md'],
  questionsPerSource: 10,
  includeNegatives: true,
  difficulty: 'mixed',
});
```

### AgentMemoryGenerator

Generate agent interaction histories.

```typescript
class AgentMemoryGenerator {
  static async synthesize(options: MemoryOptions): Promise<GeneratedData<Memory>>
}
```

**Parameters:**

```typescript
interface MemoryOptions {
  agentType: string;
  interactions: number;
  userPersonas?: string[];
  taskDistribution?: Record<string, number>;
  includeEmbeddings?: boolean;
}

interface Memory {
  id: string;
  timestamp: Date;
  userInput: string;
  agentResponse: string;
  taskType: string;
  persona: string;
  embedding?: number[];
  metadata: Record<string, any>;
}
```

### EdgeCaseGenerator

Generate edge cases for testing.

```typescript
class EdgeCaseGenerator {
  static async create(options: EdgeCaseOptions): Promise<GeneratedData<any>>
}
```

**Parameters:**

```typescript
interface EdgeCaseOptions {
  schema: Schema;
  categories: EdgeCaseCategory[];
  coverage?: 'minimal' | 'standard' | 'exhaustive';
}

type EdgeCaseCategory =
  | 'boundary-values'
  | 'null-handling'
  | 'type-mismatches'
  | 'malicious-input'
  | 'unicode-edge-cases'
  | 'race-conditions'
  | 'overflow'
  | 'underflow';
```

### EmbeddingDatasetGenerator

Generate vector embeddings datasets.

```typescript
class EmbeddingDatasetGenerator {
  static async create(options: EmbeddingDatasetOptions): Promise<GeneratedData<EmbeddingItem>>
}
```

**Parameters:**

```typescript
interface EmbeddingDatasetOptions {
  domain: string;
  clusters: number;
  itemsPerCluster: number;
  vectorDim: number;
  distribution?: 'gaussian' | 'uniform' | 'clustered';
}

interface EmbeddingItem {
  id: string;
  text: string;
  embedding: number[];
  cluster: number;
  metadata: Record<string, any>;
}
```

---

## Templates

Pre-built templates for common domains.

### Templates.customerSupport

```typescript
Templates.customerSupport.generate(count: number): Promise<GeneratedData<Conversation>>
```

### Templates.codeReviews

```typescript
Templates.codeReviews.generate(count: number): Promise<GeneratedData<Review>>
```

### Templates.ecommerce

```typescript
Templates.ecommerce.generate(count: number): Promise<GeneratedData<Product>>
```

### Templates.medicalQA

```typescript
Templates.medicalQA.generate(count: number): Promise<GeneratedData<MedicalQA>>
```

### Templates.legalContracts

```typescript
Templates.legalContracts.generate(count: number): Promise<GeneratedData<Contract>>
```

**Example:**

```typescript
import { Templates } from 'agentic-synth';

const products = await Templates.ecommerce.generate(10000);
await products.export({ format: 'parquet', outputPath: './products.parquet' });
```

---

## Quality Metrics

Evaluate synthetic data quality.

### QualityMetrics.evaluate()

```typescript
QualityMetrics.evaluate(data: any[], options: EvaluationOptions): Promise<QualityReport>
```

**Parameters:**

```typescript
interface EvaluationOptions {
  realism?: boolean;      // Human-like quality
  diversity?: boolean;    // Unique examples ratio
  coverage?: boolean;     // Schema satisfaction
  coherence?: boolean;    // Logical consistency
  bias?: boolean;         // Detect biases
}

interface QualityReport {
  realism: number;        // 0-1
  diversity: number;      // 0-1
  coverage: number;       // 0-1
  coherence: number;      // 0-1
  bias: {
    gender: number;
    age: number;
    ethnicity: number;
    [key: string]: number;
  };
  overall: number;        // Weighted average
}
```

**Example:**

```typescript
const metrics = await QualityMetrics.evaluate(syntheticData, {
  realism: true,
  diversity: true,
  coverage: true,
  bias: true,
});

if (metrics.overall < 0.85) {
  console.warn('Low quality data detected');
}
```

---

## Integrations

### RuvectorAdapter

```typescript
class RuvectorAdapter {
  constructor(synthEngine: SynthEngine, vectorDB: VectorDB)

  async generateAndInsert(options: GenerateOptions): Promise<InsertResult>
  async augmentCollection(collection: string, count: number): Promise<void>
}
```

### AgenticDBAdapter

```typescript
class AgenticDBAdapter {
  constructor(synthEngine: SynthEngine)

  async generateMemory(options: MemoryOptions): Promise<Memory[]>
  async generateSkills(count: number): Promise<Skill[]>
}
```

### LangChainAdapter

```typescript
class LangChainAdapter {
  constructor(synthEngine: SynthEngine)

  async generateDocuments(options: GenerateOptions): Promise<Document[]>
  async createVectorStore(options: VectorStoreOptions): Promise<VectorStore>
}
```

---

## Types

### Core Types

```typescript
// Schema types
type Schema = { /* ... */ };
type PropertyDefinition = { /* ... */ };
type SchemaDefinition = { /* ... */ };

// Generation types
type GenerateOptions = { /* ... */ };
type GeneratedData<T> = { /* ... */ };
type Progress = { /* ... */ };

// Quality types
type QualityMetrics = { /* ... */ };
type QualityReport = { /* ... */ };

// Export types
type ExportFormat = 'json' | 'jsonl' | 'csv' | 'parquet' | 'sql';
type ExportOptions = {
  format: ExportFormat;
  outputPath: string;
  includeVectors?: boolean;
  compress?: boolean;
};
```

---

## CLI Reference

### Commands

```bash
# Generate data
agentic-synth generate --schema <schema> --count <n> --output <file>

# Augment existing data
agentic-synth augment --input <file> --variations <n> --output <file>

# Validate quality
agentic-synth validate --input <file> --metrics <metrics>

# Export/convert
agentic-synth export --input <file> --format <format> --output <file>

# List templates
agentic-synth templates list

# Generate from template
agentic-synth templates use <name> --count <n> --output <file>
```

### Options

```bash
--schema <file>           # Schema file (YAML/JSON)
--count <number>          # Number of examples
--output <path>           # Output file path
--format <format>         # json|jsonl|csv|parquet
--embeddings              # Include vector embeddings
--quality <threshold>     # Minimum quality (0-1)
--streaming               # Enable streaming mode
--workers <number>        # Number of parallel workers
--verbose                 # Detailed logging
```

---

## Error Handling

```typescript
import { SynthError, ValidationError, GenerationError } from 'agentic-synth';

try {
  const data = await synth.generate({ schema, count });
} catch (error) {
  if (error instanceof ValidationError) {
    console.error('Schema validation failed:', error.issues);
  } else if (error instanceof GenerationError) {
    console.error('Generation failed:', error.message);
  } else if (error instanceof SynthError) {
    console.error('Synth error:', error.message);
  }
}
```

---

## Best Practices

1. **Start Small**: Test with 100 examples before scaling to millions
2. **Validate Schemas**: Use TypeScript types for compile-time safety
3. **Monitor Quality**: Always evaluate quality metrics
4. **Use Streaming**: For large datasets (>10K), enable streaming
5. **Cache Results**: Enable caching for repeated generations
6. **Tune Temperature**: Lower (0.5-0.7) for consistency, higher (0.8-1.0) for diversity
7. **Batch Operations**: Use batching for vector DB insertions
8. **Handle Errors**: Implement retry logic for API failures

---

## Examples

See [EXAMPLES.md](./EXAMPLES.md) for comprehensive usage examples.

## Support

- GitHub Issues: https://github.com/ruvnet/ruvector/issues
- Discord: https://discord.gg/ruvnet
- Email: support@ruv.io
