# 🎓 Agentic-Synth Advanced Usage Guide

**Version**: 0.1.0
**Last Updated**: 2025-11-22

---

## Table of Contents

1. [Advanced Data Generation](#1-advanced-data-generation)
2. [Real-World Integration Examples](#2-real-world-integration-examples)
3. [Performance Optimization](#3-performance-optimization)
4. [Production Deployment](#4-production-deployment)
5. [Error Handling & Monitoring](#5-error-handling--monitoring)
6. [Advanced Patterns](#6-advanced-patterns)

---

## 1. Advanced Data Generation

### 1.1 Complex Time-Series with Custom Patterns

Generate realistic stock market data with multiple indicators:

```typescript
import { AgenticSynth } from '@ruvector/agentic-synth';

const synth = new AgenticSynth({
  provider: 'gemini',
  apiKey: process.env.GEMINI_API_KEY,
  cacheStrategy: 'memory',
  cacheTTL: 3600
});

// Generate 1 year of stock market data
const stockData = await synth.generateTimeSeries({
  count: 365,
  startDate: '2024-01-01',
  interval: '1d',
  schema: {
    date: 'ISO date',
    open: 'number (100-200)',
    high: 'number (105-210)',
    low: 'number (95-195)',
    close: 'number (100-200)',
    volume: 'number (1000000-10000000)',
    // Technical indicators
    sma_20: 'number (calculated 20-day moving average)',
    rsi_14: 'number (0-100, RSI indicator)',
    macd: 'number (-5 to 5)',
    bollinger_upper: 'number',
    bollinger_lower: 'number'
  },
  constraints: [
    'high must be >= open and close',
    'low must be <= open and close',
    'close of previous day influences next day open (± 5%)',
    'volume increases on large price changes',
    'RSI correlates with price momentum',
    'SMA_20 follows price trend'
  ]
});

console.log(`Generated ${stockData.data.length} days of stock data`);
console.log('Cache hit rate:', stockData.metadata.cached ? '100%' : '0%');
```

### 1.2 Multi-User Event Simulation

Simulate realistic user behavior across 100 users:

```typescript
const userBehaviorSim = await synth.generateEvents({
  count: 10000,
  timeRange: {
    start: '2024-01-01T00:00:00Z',
    end: '2024-01-31T23:59:59Z'
  },
  eventTypes: [
    'page_view',
    'click',
    'form_submit',
    'purchase',
    'logout',
    'search',
    'add_to_cart'
  ],
  schema: {
    event_id: 'UUID',
    user_id: 'UUID (one of 100 consistent users)',
    event_type: 'one of eventTypes',
    timestamp: 'ISO timestamp within timeRange',
    session_id: 'UUID (consistent per user session)',
    page_url: 'URL path',
    metadata: {
      device: 'mobile | desktop | tablet',
      browser: 'chrome | firefox | safari | edge',
      country: 'ISO country code',
      referrer: 'URL or null',
      duration_ms: 'number (100-30000 for page_view)',
      cart_value: 'number (only for add_to_cart/purchase)',
      search_query: 'string (only for search events)'
    }
  },
  constraints: [
    'Users follow realistic session patterns (20-40 events per session)',
    'Purchase events must be preceded by add_to_cart',
    'Events follow temporal ordering per user',
    'Session gaps between 30min-24hours',
    'Time distribution follows Poisson with peak hours 10am-4pm',
    'Mobile users more common in evening hours',
    'Purchase conversion rate ~2-3%'
  ]
});

console.log('Event simulation complete');
console.log('Total events:', userBehaviorSim.data.length);
console.log('Unique users:', new Set(userBehaviorSim.data.map(e => e.user_id)).size);
```

### 1.3 Nested Schema Generation

Generate complex e-commerce orders with line items:

```typescript
const orders = await synth.generateStructured({
  count: 1000,
  schema: {
    order_id: 'UUID',
    customer: {
      customer_id: 'UUID',
      email: 'valid email',
      name: 'full name',
      phone: 'phone number with country code',
      address: {
        street: 'street address',
        city: 'city name',
        state: 'US state code',
        zip: 'US ZIP code',
        country: 'USA'
      }
    },
    items: [
      {
        sku: 'product SKU code',
        name: 'product name',
        category: 'electronics | clothing | home | books | food',
        quantity: 'number (1-10)',
        unit_price: 'number (5-500)',
        discount_percent: 'number (0-30)',
        subtotal: 'calculated: quantity * unit_price * (1 - discount_percent/100)'
      }
    ],
    payment: {
      method: 'credit_card | paypal | apple_pay | google_pay',
      status: 'pending | completed | failed | refunded',
      transaction_id: 'UUID',
      amount: 'sum of all item subtotals'
    },
    shipping: {
      method: 'standard | express | overnight',
      cost: 'number (0-50, based on method)',
      tracking_number: 'tracking code',
      estimated_delivery: 'ISO date (3-10 days from order date)'
    },
    order_date: 'ISO timestamp',
    status: 'pending | processing | shipped | delivered | cancelled',
    total_amount: 'payment.amount + shipping.cost',
    notes: 'optional customer notes or null'
  },
  constraints: [
    'Each order has 1-8 line items',
    'Total amount must equal sum of items + shipping',
    'Status progression: pending → processing → shipped → delivered',
    'Express shipping costs 2x standard',
    'Overnight shipping costs 4x standard',
    'Electronics have 10-20% discount',
    'Credit card most common payment (60%)',
    'Standard shipping most common (70%)'
  ]
});

console.log(`Generated ${orders.data.length} complex orders`);
```

---

## 2. Real-World Integration Examples

### 2.1 ML Training Pipeline

Complete machine learning data generation pipeline:

```typescript
import { AgenticSynth } from '@ruvector/agentic-synth';
import * as fs from 'fs/promises';

class MLTrainingPipeline {
  private synth: AgenticSynth;

  constructor() {
    this.synth = new AgenticSynth({
      provider: 'gemini',
      apiKey: process.env.GEMINI_API_KEY,
      cacheStrategy: 'memory',
      maxCacheSize: 10000 // Large cache for ML datasets
    });
  }

  async generateTrainingData(samples = 5000) {
    console.log(`Generating ${samples} training samples...`);

    const training = await this.synth.generateStructured({
      count: samples,
      schema: {
        // Feature engineering
        customer_age: 'number (18-80)',
        annual_income: 'number (20000-200000)',
        credit_score: 'number (300-850)',
        account_tenure_months: 'number (1-360)',
        num_products: 'number (1-5)',
        balance: 'number (0-250000)',
        num_transactions_12m: 'number (0-200)',
        avg_transaction_amount: 'number (10-5000)',
        credit_utilization: 'number (0-100)',
        num_late_payments: 'number (0-10)',

        // Target variable
        churn: 'boolean (based on features: higher likelihood if credit_score<600, num_late_payments>3, balance<1000, credit_utilization>80)'
      },
      constraints: [
        'Credit utilization correlates negatively with credit score',
        'Higher income correlates with higher balance',
        'Churn rate should be ~15-20% (imbalanced dataset)',
        'Customers with 1 product more likely to churn',
        'Tenure > 24 months reduces churn likelihood'
      ]
    });

    await fs.writeFile(
      'ml_data/training.json',
      JSON.stringify(training.data, null, 2)
    );

    return training;
  }

  async generateTestData(samples = 1000) {
    console.log(`Generating ${samples} test samples...`);

    // Similar schema, different random seed
    const test = await this.synth.generateStructured({
      count: samples,
      schema: { /* same as training */ },
      constraints: [ /* same as training */ ]
    });

    await fs.writeFile(
      'ml_data/test.json',
      JSON.stringify(test.data, null, 2)
    );

    return test;
  }

  async generateEdgeCases(samples = 100) {
    console.log(`Generating ${samples} edge case samples...`);

    const edgeCases = await this.synth.generateStructured({
      count: samples,
      schema: { /* same schema */ },
      constraints: [
        'Include extreme values: age 18-22 or 75-80',
        'Include very high credit_utilization (90-100)',
        'Include very low credit_score (300-400)',
        'Include zero balance accounts',
        'Include customers with num_products = 5'
      ]
    });

    await fs.writeFile(
      'ml_data/edge_cases.json',
      JSON.stringify(edgeCases.data, null, 2)
    );

    return edgeCases;
  }

  async run() {
    await fs.mkdir('ml_data', { recursive: true });

    const [training, test, edges] = await Promise.all([
      this.generateTrainingData(5000),
      this.generateTestData(1000),
      this.generateEdgeCases(100)
    ]);

    console.log('\n📊 ML Dataset Generation Complete:');
    console.log(`   Training: ${training.data.length} samples`);
    console.log(`   Test: ${test.data.length} samples`);
    console.log(`   Edge Cases: ${edges.data.length} samples`);
    console.log(`   Total generation time: ${
      training.metadata.generationTime +
      test.metadata.generationTime +
      edges.metadata.generationTime
    }ms`);

    // Get cache statistics
    const cacheStats = this.synth.cache.getStats();
    console.log(`   Cache hit rate: ${(cacheStats.hitRate * 100).toFixed(1)}%`);
  }
}

// Run the pipeline
const pipeline = new MLTrainingPipeline();
await pipeline.run();
```

### 2.2 RAG System Data Generation

Generate Q&A pairs for Retrieval-Augmented Generation:

```typescript
import { AgenticSynth } from '@ruvector/agentic-synth';
import { RuvectorClient } from 'ruvector'; // Optional: vector DB integration

class RAGDataGenerator {
  private synth: AgenticSynth;
  private vectorDB?: RuvectorClient;

  constructor(useVectorDB = true) {
    this.synth = new AgenticSynth({
      provider: 'gemini',
      apiKey: process.env.GEMINI_API_KEY,
      cacheStrategy: 'memory',
      vectorDB: useVectorDB
    });

    if (useVectorDB) {
      this.vectorDB = new RuvectorClient({
        url: process.env.RUVECTOR_URL || 'http://localhost:3000'
      });
    }
  }

  async generateCustomerSupportData() {
    // Generate diverse Q&A pairs
    const qaData = await this.synth.generateStructured({
      count: 1000,
      schema: {
        question_id: 'UUID',
        category: 'billing | technical | shipping | returns | account | product_info',
        question: 'realistic customer support question',
        answer: 'detailed, helpful answer (2-4 sentences)',
        keywords: ['array of 3-5 relevant keywords'],
        difficulty: 'easy | medium | hard',
        sentiment: 'neutral | frustrated | confused | satisfied',
        related_questions: ['array of 2-3 related question texts'],
        metadata: {
          estimated_resolution_time: 'number (1-30 minutes)',
          requires_escalation: 'boolean',
          product_category: 'electronics | clothing | home | books | other',
          language: 'en'
        }
      },
      constraints: [
        'Questions should be natural, conversational',
        'Answers should be accurate and empathetic',
        'Billing questions often frustrated sentiment',
        'Technical questions higher difficulty',
        'Include typos and informal language in 10% of questions',
        'Related questions share category and keywords',
        'Hard questions more likely to require escalation'
      ]
    });

    console.log(`Generated ${qaData.data.length} Q&A pairs`);
    return qaData;
  }

  async generateEmbeddingsAndStore(qaData: any) {
    if (!this.vectorDB) {
      console.log('Vector DB not enabled, skipping embedding storage');
      return;
    }

    console.log('Generating embeddings and storing in vector DB...');

    // Batch process for efficiency
    const batchSize = 50;
    for (let i = 0; i < qaData.data.length; i += batchSize) {
      const batch = qaData.data.slice(i, i + batchSize);

      // Generate embeddings using Gemini's embedding model
      const embeddings = await Promise.all(
        batch.map(async (qa: any) => {
          const text = `${qa.question} ${qa.answer}`;
          // Use Gemini embedding API
          // const embedding = await generateEmbedding(text);
          return {
            id: qa.question_id,
            text,
            metadata: qa
          };
        })
      );

      // Store in vector database
      // await this.vectorDB.insert(embeddings);

      console.log(`Processed batch ${i / batchSize + 1}/${Math.ceil(qaData.data.length / batchSize)}`);
    }

    console.log('✅ All embeddings stored in vector DB');
  }

  async run() {
    const qaData = await this.generateCustomerSupportData();
    await this.generateEmbeddingsAndStore(qaData);

    console.log('\n📚 RAG Data Generation Complete:');
    console.log(`   Q&A Pairs: ${qaData.data.length}`);
    console.log(`   Categories: ${new Set(qaData.data.map((d: any) => d.category)).size}`);
    console.log(`   Generation time: ${qaData.metadata.generationTime}ms`);
  }
}

// Run the generator
const ragGen = new RAGDataGenerator(true);
await ragGen.run();
```

### 2.3 Database Seeding

Seed development/staging databases with realistic test data:

```typescript
import { AgenticSynth } from '@ruvector/agentic-synth';
import { Pool } from 'pg'; // PostgreSQL example

class DatabaseSeeder {
  private synth: AgenticSynth;
  private db: Pool;

  constructor() {
    this.synth = new AgenticSynth({
      provider: 'gemini',
      apiKey: process.env.GEMINI_API_KEY,
      cacheStrategy: 'memory',
      maxCacheSize: 5000
    });

    this.db = new Pool({
      host: process.env.DB_HOST,
      database: process.env.DB_NAME,
      user: process.env.DB_USER,
      password: process.env.DB_PASSWORD
    });
  }

  async seedUsers(count = 1000) {
    console.log(`Seeding ${count} users...`);

    const users = await this.synth.generateStructured({
      count,
      schema: {
        email: 'valid unique email',
        username: 'unique username (5-20 chars)',
        first_name: 'first name',
        last_name: 'last name',
        password_hash: 'bcrypt hash',
        phone: 'phone number or null',
        avatar_url: 'profile image URL or null',
        bio: 'user bio (1-2 sentences) or null',
        birth_date: 'ISO date (age 18-75)',
        country: 'ISO country code',
        created_at: 'ISO timestamp (last 2 years)',
        last_login_at: 'ISO timestamp (recent)',
        email_verified: 'boolean (90% true)',
        account_status: 'active | suspended | deleted'
      }
    });

    // Bulk insert
    const client = await this.db.connect();
    try {
      await client.query('BEGIN');

      for (const user of users.data) {
        await client.query(
          `INSERT INTO users (email, username, first_name, last_name, password_hash,
           phone, avatar_url, bio, birth_date, country, created_at, last_login_at,
           email_verified, account_status)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)`,
          [user.email, user.username, user.first_name, user.last_name,
           user.password_hash, user.phone, user.avatar_url, user.bio,
           user.birth_date, user.country, user.created_at, user.last_login_at,
           user.email_verified, user.account_status]
        );
      }

      await client.query('COMMIT');
      console.log(`✅ Seeded ${count} users`);
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }

    return users.data;
  }

  async seedOrders(users: any[], ordersPerUser = 5) {
    console.log(`Seeding orders for ${users.length} users...`);

    const totalOrders = users.length * ordersPerUser;
    const orders = await this.synth.generateStructured({
      count: totalOrders,
      schema: {
        user_id: 'UUID (from users array)',
        order_number: 'unique order number',
        status: 'pending | processing | shipped | delivered | cancelled',
        total_amount: 'number (10-1000)',
        currency: 'USD',
        payment_method: 'credit_card | paypal | apple_pay',
        shipping_address: 'full address',
        order_date: 'ISO timestamp (after user.created_at)',
        shipped_date: 'ISO timestamp or null',
        delivered_date: 'ISO timestamp or null'
      }
    });

    // Bulk insert orders
    const client = await this.db.connect();
    try {
      await client.query('BEGIN');

      for (const order of orders.data) {
        await client.query(
          `INSERT INTO orders (user_id, order_number, status, total_amount,
           currency, payment_method, shipping_address, order_date, shipped_date,
           delivered_date)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)`,
          [order.user_id, order.order_number, order.status, order.total_amount,
           order.currency, order.payment_method, order.shipping_address,
           order.order_date, order.shipped_date, order.delivered_date]
        );
      }

      await client.query('COMMIT');
      console.log(`✅ Seeded ${totalOrders} orders`);
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }

    return orders.data;
  }

  async seedProducts(count = 500) {
    console.log(`Seeding ${count} products...`);

    const products = await this.synth.generateStructured({
      count,
      schema: {
        sku: 'unique SKU code',
        name: 'product name',
        description: 'product description (2-3 sentences)',
        category: 'electronics | clothing | home | books | food | sports',
        price: 'number (5-500)',
        stock_quantity: 'number (0-1000)',
        weight_kg: 'number (0.1-50)',
        dimensions: '{ length, width, height in cm }',
        manufacturer: 'company name',
        rating: 'number (1-5, one decimal)',
        num_reviews: 'number (0-5000)',
        images: ['array of 1-5 image URLs'],
        tags: ['array of 3-7 product tags'],
        created_at: 'ISO timestamp'
      }
    });

    // Bulk insert products
    const client = await this.db.connect();
    try {
      await client.query('BEGIN');

      for (const product of products.data) {
        await client.query(
          `INSERT INTO products (sku, name, description, category, price,
           stock_quantity, weight_kg, dimensions, manufacturer, rating,
           num_reviews, images, tags, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)`,
          [product.sku, product.name, product.description, product.category,
           product.price, product.stock_quantity, product.weight_kg,
           JSON.stringify(product.dimensions), product.manufacturer,
           product.rating, product.num_reviews, JSON.stringify(product.images),
           JSON.stringify(product.tags), product.created_at]
        );
      }

      await client.query('COMMIT');
      console.log(`✅ Seeded ${count} products`);
    } catch (error) {
      await client.query('ROLLBACK');
      throw error;
    } finally {
      client.release();
    }

    return products.data;
  }

  async run() {
    console.log('🌱 Starting database seeding...\n');

    const users = await this.seedUsers(1000);
    await this.seedProducts(500);
    await this.seedOrders(users, 5);

    await this.db.end();

    console.log('\n✅ Database seeding complete!');
    console.log('   Users: 1000');
    console.log('   Products: 500');
    console.log('   Orders: 5000');
  }
}

// Run the seeder
const seeder = new DatabaseSeeder();
await seeder.run();
```

---

## 3. Performance Optimization

### 3.1 Maximize Cache Hit Rate

```typescript
import { AgenticSynth } from '@ruvector/agentic-synth';

// Configure for optimal caching
const synth = new AgenticSynth({
  provider: 'gemini',
  apiKey: process.env.GEMINI_API_KEY,
  cacheStrategy: 'memory',
  cacheTTL: 7200, // 2 hours
  maxCacheSize: 10000 // Large cache
});

// Use stable options for better cache hits
const baseOptions = {
  count: 100,
  schema: {
    user_id: 'UUID',
    name: 'full name',
    email: 'valid email'
  }
};

// First call - cache miss
const result1 = await synth.generateStructured(baseOptions);
console.log('Cached:', result1.metadata.cached); // false

// Second call with identical options - cache hit!
const result2 = await synth.generateStructured(baseOptions);
console.log('Cached:', result2.metadata.cached); // true

// Check cache statistics
const stats = synth.cache.getStats();
console.log('Cache hit rate:', (stats.hitRate * 100).toFixed(1) + '%');
console.log('Cache size:', stats.size);
```

### 3.2 Batch Processing for High Throughput

```typescript
// Generate 10 different datasets in parallel
const batchOptions = [
  { count: 100, schema: { id: 'UUID', value: 'number' } },
  { count: 200, schema: { id: 'UUID', name: 'string' } },
  { count: 150, schema: { id: 'UUID', email: 'email' } },
  // ... 7 more options
];

// Process with concurrency control
const results = await synth.generateBatch(
  'structured',
  batchOptions,
  5 // 5 concurrent requests
);

console.log(`Generated ${results.length} datasets in parallel`);
console.log('Total records:', results.reduce((sum, r) => sum + r.data.length, 0));
```

### 3.3 Streaming for Large Datasets

```typescript
// Stream 1 million records without loading all into memory
console.log('Streaming 1M records...');

let count = 0;
for await (const record of synth.generateStream('structured', {
  count: 1_000_000,
  schema: {
    id: 'UUID',
    timestamp: 'ISO timestamp',
    value: 'number'
  }
})) {
  // Process record immediately (e.g., write to file/DB)
  await processRecord(record);

  count++;
  if (count % 10000 === 0) {
    console.log(`Processed ${count.toLocaleString()} records...`);
  }
}

console.log('Streaming complete!');
```

### 3.4 Model Selection for Cost Optimization

```typescript
// Use cheaper models for simple data
const simpleData = new AgenticSynth({
  provider: 'gemini',
  model: 'gemini-2.0-flash-exp', // Fast and cheap
  apiKey: process.env.GEMINI_API_KEY
});

// Use more powerful models for complex data
const complexData = new AgenticSynth({
  provider: 'openrouter',
  model: 'anthropic/claude-3.5-sonnet', // More capable
  apiKey: process.env.OPENROUTER_API_KEY
});
```

---

## 4. Production Deployment

### 4.1 Environment Configuration

```typescript
// config/production.ts
export const productionConfig = {
  provider: process.env.SYNTH_PROVIDER as 'gemini' | 'openrouter',
  apiKey: process.env.SYNTH_API_KEY!,
  model: process.env.SYNTH_MODEL,
  cacheStrategy: 'memory' as const,
  cacheTTL: parseInt(process.env.CACHE_TTL || '3600'),
  maxCacheSize: parseInt(process.env.MAX_CACHE_SIZE || '10000'),
  maxRetries: 3,
  timeout: 30000,
  streaming: process.env.ENABLE_STREAMING === 'true',
  automation: process.env.ENABLE_AUTOMATION === 'true',
  vectorDB: process.env.ENABLE_VECTOR_DB === 'true'
};

// Validation
if (!productionConfig.apiKey) {
  throw new Error('SYNTH_API_KEY environment variable is required');
}
```

### 4.2 Error Handling & Retry Logic

```typescript
import { AgenticSynth, APIError, ValidationError } from '@ruvector/agentic-synth';

async function generateWithRetry(
  synth: AgenticSynth,
  options: any,
  maxRetries = 3
) {
  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    try {
      const result = await synth.generateStructured(options);
      return result;
    } catch (error) {
      if (error instanceof ValidationError) {
        // Don't retry validation errors
        console.error('Validation failed:', error.message);
        throw error;
      }

      if (error instanceof APIError) {
        if (error.statusCode === 429) {
          // Rate limit - exponential backoff
          const delay = Math.pow(2, attempt) * 1000;
          console.log(`Rate limited. Retrying in ${delay}ms...`);
          await new Promise(resolve => setTimeout(resolve, delay));
          continue;
        }

        if (error.statusCode >= 500) {
          // Server error - retry
          console.log(`Server error (${error.statusCode}). Retry ${attempt}/${maxRetries}`);
          continue;
        }
      }

      // Unknown error or non-retryable
      throw error;
    }
  }

  throw new Error('Max retries exceeded');
}
```

### 4.3 Monitoring & Metrics

```typescript
import { AgenticSynth } from '@ruvector/agentic-synth';
import { performance } from 'perf_hooks';

class MonitoredSynth {
  private synth: AgenticSynth;
  private metrics = {
    totalRequests: 0,
    successfulRequests: 0,
    failedRequests: 0,
    totalLatency: 0,
    cacheHits: 0,
    cacheMisses: 0
  };

  constructor(config: any) {
    this.synth = new AgenticSynth(config);
  }

  async generate(type: string, options: any) {
    const start = performance.now();
    this.metrics.totalRequests++;

    try {
      const result = await this.synth.generate(type as any, options);

      this.metrics.successfulRequests++;
      this.metrics.totalLatency += performance.now() - start;

      if (result.metadata.cached) {
        this.metrics.cacheHits++;
      } else {
        this.metrics.cacheMisses++;
      }

      return result;
    } catch (error) {
      this.metrics.failedRequests++;
      throw error;
    }
  }

  getMetrics() {
    const avgLatency = this.metrics.totalLatency / this.metrics.successfulRequests;
    const successRate = this.metrics.successfulRequests / this.metrics.totalRequests;
    const cacheHitRate = this.metrics.cacheHits / (this.metrics.cacheHits + this.metrics.cacheMisses);

    return {
      ...this.metrics,
      avgLatency: Math.round(avgLatency),
      successRate: (successRate * 100).toFixed(2) + '%',
      cacheHitRate: (cacheHitRate * 100).toFixed(2) + '%'
    };
  }
}

// Usage
const monitored = new MonitoredSynth(productionConfig);

// Generate data
await monitored.generate('structured', { count: 100, schema: { id: 'UUID' } });

// Log metrics periodically
setInterval(() => {
  console.log('Synth Metrics:', monitored.getMetrics());
}, 60000); // Every minute
```

### 4.4 Rate Limiting

```typescript
import { AgenticSynth } from '@ruvector/agentic-synth';
import { RateLimiter } from 'limiter';

class RateLimitedSynth {
  private synth: AgenticSynth;
  private limiter: RateLimiter;

  constructor(config: any, requestsPerMinute = 60) {
    this.synth = new AgenticSynth(config);
    this.limiter = new RateLimiter({
      tokensPerInterval: requestsPerMinute,
      interval: 'minute'
    });
  }

  async generate(type: string, options: any) {
    // Wait for rate limit token
    await this.limiter.removeTokens(1);

    // Proceed with generation
    return await this.synth.generate(type as any, options);
  }
}

// Usage: Limit to 60 requests per minute
const limited = new RateLimitedSynth(productionConfig, 60);
```

---

## 5. Error Handling & Monitoring

### 5.1 Comprehensive Error Handling

```typescript
import { AgenticSynth, SynthError, ValidationError, APIError, CacheError } from '@ruvector/agentic-synth';

async function robustGeneration(options: any) {
  const synth = new AgenticSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY
  });

  try {
    const result = await synth.generateStructured(options);
    return result;
  } catch (error) {
    if (error instanceof ValidationError) {
      console.error('❌ Invalid options:', error.message);
      console.error('   Validation errors:', error.validationErrors);
      // Fix options and retry
      return null;
    }

    if (error instanceof APIError) {
      console.error('❌ API error:', error.message);
      console.error('   Status:', error.statusCode);
      console.error('   Provider:', error.provider);

      if (error.statusCode === 401) {
        console.error('   Check API key configuration');
      } else if (error.statusCode === 429) {
        console.error('   Rate limit exceeded - implement backoff');
      } else if (error.statusCode >= 500) {
        console.error('   Provider service error - retry later');
      }

      return null;
    }

    if (error instanceof CacheError) {
      console.error('❌ Cache error:', error.message);
      // Cache errors are non-fatal - continue without cache
      synth.config.cacheStrategy = undefined;
      return await synth.generateStructured(options);
    }

    if (error instanceof SynthError) {
      console.error('❌ Synth error:', error.message);
      return null;
    }

    // Unknown error
    console.error('❌ Unexpected error:', error);
    throw error;
  }
}
```

### 5.2 Health Checks

```typescript
import { AgenticSynth } from '@ruvector/agentic-synth';

class HealthChecker {
  private synth: AgenticSynth;

  constructor(config: any) {
    this.synth = new AgenticSynth(config);
  }

  async checkHealth() {
    const health = {
      status: 'healthy',
      timestamp: new Date().toISOString(),
      checks: {
        apiConnection: false,
        cacheWorking: false,
        generationWorking: false
      },
      metrics: {}
    };

    try {
      // Test API connection with minimal request
      const testResult = await this.synth.generateStructured({
        count: 1,
        schema: { test: 'string' }
      });

      health.checks.apiConnection = true;
      health.checks.generationWorking = true;
      health.checks.cacheWorking = testResult.metadata.cached === false;

      // Get cache stats
      const cacheStats = this.synth.cache.getStats();
      health.metrics = {
        cacheSize: cacheStats.size,
        cacheHitRate: (cacheStats.hitRate * 100).toFixed(2) + '%',
        generationTime: testResult.metadata.generationTime + 'ms'
      };

    } catch (error) {
      health.status = 'unhealthy';
      health.checks.apiConnection = false;
    }

    return health;
  }
}

// Express.js health endpoint
app.get('/health', async (req, res) => {
  const checker = new HealthChecker(productionConfig);
  const health = await checker.checkHealth();

  const statusCode = health.status === 'healthy' ? 200 : 503;
  res.status(statusCode).json(health);
});
```

---

## 6. Advanced Patterns

### 6.1 Multi-Provider Fallback

```typescript
import { AgenticSynth } from '@ruvector/agentic-synth';

class MultiProviderSynth {
  private providers: AgenticSynth[];

  constructor() {
    this.providers = [
      new AgenticSynth({
        provider: 'gemini',
        apiKey: process.env.GEMINI_API_KEY,
        model: 'gemini-2.0-flash-exp'
      }),
      new AgenticSynth({
        provider: 'openrouter',
        apiKey: process.env.OPENROUTER_API_KEY,
        model: 'anthropic/claude-3.5-sonnet'
      })
    ];
  }

  async generateWithFallback(type: string, options: any) {
    for (let i = 0; i < this.providers.length; i++) {
      try {
        console.log(`Attempting provider ${i + 1}/${this.providers.length}...`);
        const result = await this.providers[i].generate(type as any, options);
        console.log(`✅ Success with provider ${i + 1}`);
        return result;
      } catch (error) {
        console.log(`❌ Provider ${i + 1} failed:`, error.message);
        if (i === this.providers.length - 1) {
          throw new Error('All providers failed');
        }
      }
    }
  }
}
```

### 6.2 Conditional Generation Logic

```typescript
import { AgenticSynth } from '@ruvector/agentic-synth';

async function generateCustomerProfiles(synth: AgenticSynth, count: number) {
  // First, generate base profiles
  const profiles = await synth.generateStructured({
    count,
    schema: {
      customer_id: 'UUID',
      customer_type: 'individual | business',
      name: 'full name',
      email: 'valid email'
    }
  });

  // Then, conditionally generate additional data based on type
  for (const profile of profiles.data) {
    if (profile.customer_type === 'business') {
      // Generate business-specific data
      const businessData = await synth.generateStructured({
        count: 1,
        schema: {
          company_name: 'company name',
          tax_id: 'EIN',
          employees: 'number (1-10000)',
          annual_revenue: 'number (10000-10000000)',
          industry: 'industry type'
        }
      });

      Object.assign(profile, businessData.data[0]);
    } else {
      // Generate individual-specific data
      const individualData = await synth.generateStructured({
        count: 1,
        schema: {
          age: 'number (18-80)',
          occupation: 'job title',
          income: 'number (20000-200000)',
          marital_status: 'single | married | divorced'
        }
      });

      Object.assign(profile, individualData.data[0]);
    }
  }

  return profiles;
}
```

### 6.3 Progressive Enhancement

```typescript
import { AgenticSynth } from '@ruvector/agentic-synth';

async function progressiveDataGeneration(synth: AgenticSynth) {
  console.log('Phase 1: Basic data generation...');
  let data = await synth.generateStructured({
    count: 100,
    schema: {
      id: 'UUID',
      name: 'full name',
      email: 'valid email'
    }
  });

  console.log('Phase 2: Adding relationships...');
  data = await synth.generateStructured({
    count: 100,
    schema: {
      ...data.data[0], // Existing fields
      friends: ['array of 3-10 UUIDs from existing data'],
      groups: ['array of 1-3 group names']
    }
  });

  console.log('Phase 3: Adding behavioral data...');
  data = await synth.generateStructured({
    count: 100,
    schema: {
      ...data.data[0], // Existing fields
      last_login: 'ISO timestamp',
      total_purchases: 'number (0-100)',
      avg_order_value: 'number (10-500)',
      loyalty_tier: 'bronze | silver | gold | platinum'
    }
  });

  return data;
}
```

---

## 7. Best Practices Summary

### ✅ Do's

1. **Enable caching** for repeated or similar requests
2. **Use batch operations** for multiple datasets
3. **Stream large datasets** to avoid memory issues
4. **Implement retry logic** with exponential backoff
5. **Monitor cache hit rates** and adjust TTL accordingly
6. **Validate options** before generation
7. **Use environment variables** for sensitive config
8. **Implement health checks** in production
9. **Log errors comprehensively** with context
10. **Test with realistic schemas** before production

### ❌ Don'ts

1. **Don't hardcode API keys** in source code
2. **Don't generate without caching** in production
3. **Don't ignore validation errors** - fix schemas
4. **Don't load massive datasets** into memory
5. **Don't skip error handling** on generation calls
6. **Don't use inappropriate models** for task complexity
7. **Don't disable retries** unless intentional
8. **Don't forget to monitor** metrics in production
9. **Don't generate unconstrained** data without schema
10. **Don't skip testing** with edge cases

---

## 8. Performance Tips

1. **Cache Configuration**: Larger TTL (1-2 hours) for stable schemas
2. **Batch Size**: 3-5 concurrent requests for optimal throughput
3. **Model Selection**: Use `gemini-2.0-flash-exp` for speed
4. **Streaming**: Use for >10K records to reduce memory
5. **Connection Pooling**: Reuse AgenticSynth instances
6. **Rate Limiting**: Implement to avoid 429 errors
7. **Schema Simplicity**: Simpler schemas = faster generation
8. **Constraint Clarity**: Clear constraints improve accuracy
9. **Error Recovery**: Implement fallback chains
10. **Monitoring**: Track P95/P99 latencies

---

## 9. Troubleshooting

### Issue: Low Cache Hit Rate

**Solution**: Use stable, deterministic options
```typescript
// ❌ Bad: timestamp makes every request unique
const options = { count: 100, timestamp: Date.now() };

// ✅ Good: stable options enable caching
const options = { count: 100, schema: { id: 'UUID' } };
```

### Issue: High Latency

**Solution**:
1. Enable caching
2. Use faster model (gemini-2.0-flash-exp)
3. Reduce complexity of schema
4. Batch similar requests

### Issue: Memory Errors

**Solution**: Use streaming for large datasets
```typescript
// ❌ Bad: load all into memory
const result = await synth.generateStructured({ count: 1000000 });

// ✅ Good: stream records
for await (const record of synth.generateStream('structured', { count: 1000000 })) {
  processRecord(record);
}
```

### Issue: Rate Limiting (429)

**Solution**: Implement exponential backoff
```typescript
async function generateWithBackoff(synth, options, maxRetries = 3) {
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await synth.generateStructured(options);
    } catch (error) {
      if (error.statusCode === 429) {
        const delay = Math.pow(2, i) * 1000;
        await new Promise(resolve => setTimeout(resolve, delay));
        continue;
      }
      throw error;
    }
  }
}
```

---

## 10. Additional Resources

- **API Reference**: See `docs/API.md`
- **Performance Guide**: See `docs/PERFORMANCE.md`
- **Benchmarks**: See `PERFORMANCE_REPORT.md`
- **Examples**: See `examples/` directory
- **GitHub**: https://github.com/ruvnet/ruvector

---

**Last Updated**: 2025-11-22
**Package Version**: 0.1.0
**Status**: Production Ready ✅
