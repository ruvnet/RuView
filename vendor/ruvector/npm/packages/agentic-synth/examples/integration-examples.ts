/**
 * Real-World Integration Examples for Agentic-Synth
 *
 * This file demonstrates practical integrations with popular
 * frameworks and tools for various use cases.
 */

import { AgenticSynth } from '../dist/index.js';
import type { GenerationResult } from '../src/types.js';

// ============================================================================
// Example 1: Express.js API Endpoint
// ============================================================================

/**
 * Express.js REST API for synthetic data generation
 */
export async function expressAPIExample() {
  // Normally you'd: import express from 'express';
  console.log('\n📡 Example 1: Express.js API Integration\n');

  const synth = new AgenticSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
    cacheStrategy: 'memory',
    cacheTTL: 3600,
  });

  // Simulated endpoint handler
  const generateEndpoint = async (req: any, res: any) => {
    try {
      const { type, options } = req.body;

      // Generate data
      const result = await synth.generate(type, options);

      // Return with metadata
      res.json({
        success: true,
        data: result.data,
        metadata: {
          count: result.data.length,
          cached: result.metadata.cached,
          generationTime: result.metadata.generationTime,
          provider: result.metadata.provider,
        },
      });
    } catch (error: any) {
      res.status(500).json({
        success: false,
        error: error.message,
      });
    }
  };

  // Example request
  const mockRequest = {
    body: {
      type: 'structured',
      options: {
        count: 10,
        schema: {
          id: 'UUID',
          name: 'full name',
          email: 'valid email',
        },
      },
    },
  };

  const mockResponse = {
    json: (data: any) => console.log('Response:', JSON.stringify(data, null, 2)),
    status: (code: number) => ({
      json: (data: any) => console.log(`Status ${code}:`, JSON.stringify(data, null, 2)),
    }),
  };

  await generateEndpoint(mockRequest, mockResponse);
}

// ============================================================================
// Example 2: Prisma Database Seeding
// ============================================================================

/**
 * Database seeding with Prisma ORM
 */
export async function prismaSeedingExample() {
  console.log('\n🗄️  Example 2: Prisma Database Seeding\n');

  const synth = new AgenticSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
    cacheStrategy: 'memory',
  });

  // Simulate Prisma client
  const prisma = {
    user: {
      createMany: async (data: any) => {
        console.log(`Created ${data.data.length} users`);
        return { count: data.data.length };
      },
    },
    post: {
      createMany: async (data: any) => {
        console.log(`Created ${data.data.length} posts`);
        return { count: data.data.length };
      },
    },
  };

  // Generate users
  const users = await synth.generateStructured({
    count: 50,
    schema: {
      email: 'unique valid email',
      name: 'full name',
      password: 'bcrypt hash',
      bio: 'short bio (1-2 sentences) or null',
      avatar: 'profile image URL or null',
      createdAt: 'ISO timestamp (last 2 years)',
    },
  });

  // Seed database
  await prisma.user.createMany({
    data: users.data,
    skipDuplicates: true,
  });

  // Generate posts for users
  const posts = await synth.generateStructured({
    count: 200,
    schema: {
      title: 'blog post title',
      content: 'blog post content (3-5 paragraphs)',
      published: 'boolean (80% true)',
      authorId: 'UUID (from users)',
      createdAt: 'ISO timestamp (last year)',
      tags: ['array of 2-5 topic tags'],
    },
  });

  await prisma.post.createMany({
    data: posts.data,
  });

  console.log('\n✅ Database seeded successfully');
}

// ============================================================================
// Example 3: Jest Testing Fixtures
// ============================================================================

/**
 * Generate test fixtures for Jest tests
 */
export async function jestFixturesExample() {
  console.log('\n🧪 Example 3: Jest Testing Fixtures\n');

  const synth = new AgenticSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
    cacheStrategy: 'memory',
    cacheTTL: 7200, // Cache test data for 2 hours
  });

  // Generate consistent test data
  const testUsers = await synth.generateStructured({
    count: 5,
    schema: {
      id: 'UUID',
      email: 'valid email',
      name: 'full name',
      role: 'admin | user | moderator',
      active: 'boolean',
    },
  });

  // Use in tests
  console.log('Test Fixtures Generated:');
  console.log('- Admin user:', testUsers.data.find((u: any) => u.role === 'admin'));
  console.log('- Regular user:', testUsers.data.find((u: any) => u.role === 'user'));
  console.log('- Inactive user:', testUsers.data.find((u: any) => !u.active));

  // Example test usage
  const exampleTest = () => {
    // describe('User Authentication', () => {
    //   it('should allow admin users to access admin panel', () => {
    //     const admin = testUsers.data.find(u => u.role === 'admin');
    //     expect(canAccessAdminPanel(admin)).toBe(true);
    //   });
    // });
  };

  console.log('\n✅ Test fixtures ready for use in Jest');
}

// ============================================================================
// Example 4: TensorFlow.js Training Data
// ============================================================================

/**
 * Generate training data for TensorFlow.js models
 */
export async function tensorflowTrainingExample() {
  console.log('\n🤖 Example 4: TensorFlow.js Training Data\n');

  const synth = new AgenticSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
    cacheStrategy: 'memory',
  });

  // Generate classification dataset
  const trainingData = await synth.generateStructured({
    count: 1000,
    schema: {
      // Features
      feature1: 'number (0-100)',
      feature2: 'number (0-100)',
      feature3: 'number (0-100)',
      feature4: 'number (0-100)',
      // Label
      label: 'number (0 or 1, based on features: 1 if feature1 + feature2 > 100)',
    },
    constraints: [
      'Features should have normal distribution',
      'Labels should be balanced (50/50 split)',
      'Include some edge cases near decision boundary',
    ],
  });

  // Convert to TensorFlow format
  const features = trainingData.data.map((d: any) => [
    d.feature1,
    d.feature2,
    d.feature3,
    d.feature4,
  ]);

  const labels = trainingData.data.map((d: any) => d.label);

  console.log('Training Data Statistics:');
  console.log(`- Total samples: ${features.length}`);
  console.log(`- Positive samples: ${labels.filter((l: number) => l === 1).length}`);
  console.log(`- Negative samples: ${labels.filter((l: number) => l === 0).length}`);
  console.log(`- Feature shape: [${features.length}, ${features[0].length}]`);

  // In real usage:
  // const xs = tf.tensor2d(features);
  // const ys = tf.tensor2d(labels, [labels.length, 1]);
  // await model.fit(xs, ys, { epochs: 100 });

  console.log('\n✅ Training data ready for TensorFlow.js');
}

// ============================================================================
// Example 5: GraphQL Mocking
// ============================================================================

/**
 * Generate mock data for GraphQL resolvers
 */
export async function graphqlMockingExample() {
  console.log('\n🔷 Example 5: GraphQL Schema Mocking\n');

  const synth = new AgenticSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
    cacheStrategy: 'memory',
  });

  // Mock User type
  const mockUsers = await synth.generateStructured({
    count: 10,
    schema: {
      id: 'UUID',
      username: 'unique username',
      email: 'valid email',
      avatar: 'profile image URL',
      bio: 'short bio or null',
      followers: 'number (0-10000)',
      following: 'number (0-1000)',
    },
  });

  // Mock Post type with relationships
  const mockPosts = await synth.generateStructured({
    count: 30,
    schema: {
      id: 'UUID',
      title: 'post title',
      content: 'post content (2-3 paragraphs)',
      authorId: 'UUID (from users)',
      likes: 'number (0-1000)',
      comments: 'number (0-100)',
      createdAt: 'ISO timestamp',
      tags: ['array of 2-5 tags'],
    },
  });

  // Example resolver implementation
  const resolvers = {
    Query: {
      users: () => mockUsers.data,
      posts: () => mockPosts.data,
      user: (_: any, { id }: any) => mockUsers.data.find((u: any) => u.id === id),
    },
    User: {
      posts: (user: any) => mockPosts.data.filter((p: any) => p.authorId === user.id),
    },
  };

  console.log('GraphQL Mocks Generated:');
  console.log(`- Users: ${mockUsers.data.length}`);
  console.log(`- Posts: ${mockPosts.data.length}`);
  console.log('\n✅ GraphQL resolvers ready with mock data');
}

// ============================================================================
// Example 6: Redis Caching Integration
// ============================================================================

/**
 * Cache generated data in Redis for distributed systems
 */
export async function redisCachingExample() {
  console.log('\n💾 Example 6: Redis Caching Integration\n');

  const synth = new AgenticSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
    cacheStrategy: 'memory', // In-memory for local caching
  });

  // Simulate Redis client
  const redis = {
    get: async (key: string) => {
      console.log(`Redis GET: ${key}`);
      return null; // Simulate cache miss
    },
    set: async (key: string, value: string, ex: number) => {
      console.log(`Redis SET: ${key} (TTL: ${ex}s)`);
      return 'OK';
    },
  };

  // Helper function with Redis caching
  async function getCachedData(cacheKey: string, generator: () => Promise<any>) {
    // Check Redis first
    const cached = await redis.get(cacheKey);
    if (cached) {
      console.log('✅ Cache hit (Redis)');
      return JSON.parse(cached);
    }

    console.log('❌ Cache miss - generating...');
    const result = await generator();

    // Store in Redis
    await redis.set(cacheKey, JSON.stringify(result), 3600);

    return result;
  }

  // Usage
  const data = await getCachedData('users:sample:100', async () => {
    return await synth.generateStructured({
      count: 100,
      schema: {
        id: 'UUID',
        name: 'full name',
        email: 'valid email',
      },
    });
  });

  console.log(`\nGenerated ${data.data.length} records (cached in Redis)`);
  console.log('✅ Redis caching integration complete');
}

// ============================================================================
// Example 7: Kafka Event Stream
// ============================================================================

/**
 * Generate and publish events to Kafka
 */
export async function kafkaStreamingExample() {
  console.log('\n📨 Example 7: Kafka Event Streaming\n');

  const synth = new AgenticSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
    cacheStrategy: 'memory',
    streaming: true, // Enable streaming
  });

  // Simulate Kafka producer
  const kafka = {
    send: async (topic: string, messages: any[]) => {
      console.log(`Kafka SEND to ${topic}: ${messages.length} messages`);
      return { success: true };
    },
  };

  console.log('Streaming events to Kafka...\n');

  // Stream events and publish to Kafka in batches
  const batchSize = 100;
  let batch: any[] = [];
  let totalSent = 0;

  for await (const event of synth.generateStream('events', {
    count: 1000,
    eventTypes: ['user_login', 'page_view', 'purchase', 'logout'],
    schema: {
      event_id: 'UUID',
      event_type: 'one of eventTypes',
      user_id: 'UUID',
      timestamp: 'ISO timestamp',
      metadata: {
        ip_address: 'IPv4 address',
        user_agent: 'browser user agent',
        session_id: 'UUID',
      },
    },
  })) {
    batch.push(event);

    // Send batch when full
    if (batch.length >= batchSize) {
      await kafka.send('user-events', batch);
      totalSent += batch.length;
      console.log(`Sent ${totalSent} events...`);
      batch = [];
    }
  }

  // Send remaining
  if (batch.length > 0) {
    await kafka.send('user-events', batch);
    totalSent += batch.length;
  }

  console.log(`\n✅ Streamed ${totalSent} events to Kafka`);
}

// ============================================================================
// Example 8: Elasticsearch Bulk Indexing
// ============================================================================

/**
 * Generate and bulk index documents in Elasticsearch
 */
export async function elasticsearchIndexingExample() {
  console.log('\n🔍 Example 8: Elasticsearch Bulk Indexing\n');

  const synth = new AgenticSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
    cacheStrategy: 'memory',
  });

  // Simulate Elasticsearch client
  const elasticsearch = {
    bulk: async (body: any) => {
      const operations = body.length / 2; // Each doc has 2 lines (action + doc)
      console.log(`Elasticsearch BULK: ${operations} operations`);
      return { errors: false, items: [] };
    },
  };

  // Generate documents
  const documents = await synth.generateStructured({
    count: 1000,
    schema: {
      id: 'UUID',
      title: 'article title',
      content: 'article content (3-5 paragraphs)',
      author: 'author name',
      category: 'technology | business | science | health | sports',
      tags: ['array of 3-7 tags'],
      publishedAt: 'ISO timestamp',
      views: 'number (0-100000)',
      likes: 'number (0-10000)',
    },
  });

  // Prepare bulk request
  const bulkBody: any[] = [];
  for (const doc of documents.data) {
    bulkBody.push({ index: { _index: 'articles', _id: doc.id } });
    bulkBody.push(doc);
  }

  // Execute bulk indexing
  const result = await elasticsearch.bulk(bulkBody);

  console.log(`\n✅ Indexed ${documents.data.length} documents in Elasticsearch`);
  console.log(`   Success: ${!result.errors}`);
}

// ============================================================================
// Example 9: Next.js API Route
// ============================================================================

/**
 * Next.js API route for data generation
 */
export async function nextjsAPIRouteExample() {
  console.log('\n⚡ Example 9: Next.js API Route\n');

  const synth = new AgenticSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
    cacheStrategy: 'memory',
    cacheTTL: 3600,
  });

  // Simulated Next.js API handler
  const handler = async (req: any, res: any) => {
    if (req.method !== 'POST') {
      return res.status(405).json({ error: 'Method not allowed' });
    }

    try {
      const { type, options } = req.body;

      const startTime = Date.now();
      const result = await synth.generate(type, options);
      const duration = Date.now() - startTime;

      res.status(200).json({
        data: result.data,
        meta: {
          count: result.data.length,
          cached: result.metadata.cached,
          duration,
          provider: result.metadata.provider,
        },
      });
    } catch (error: any) {
      res.status(500).json({ error: error.message });
    }
  };

  // Example request
  const mockReq = {
    method: 'POST',
    body: {
      type: 'structured',
      options: {
        count: 20,
        schema: {
          id: 'UUID',
          product: 'product name',
          price: 'number (10-1000)',
        },
      },
    },
  };

  const mockRes = {
    status: (code: number) => ({
      json: (data: any) => console.log(`Status ${code}:`, JSON.stringify(data, null, 2)),
    }),
  };

  await handler(mockReq, mockRes);
  console.log('\n✅ Next.js API route ready');
}

// ============================================================================
// Example 10: Supabase Integration
// ============================================================================

/**
 * Supabase database seeding and real-time subscriptions
 */
export async function supabaseIntegrationExample() {
  console.log('\n🔥 Example 10: Supabase Integration\n');

  const synth = new AgenticSynth({
    provider: 'gemini',
    apiKey: process.env.GEMINI_API_KEY || 'demo-key',
    cacheStrategy: 'memory',
  });

  // Simulate Supabase client
  const supabase = {
    from: (table: string) => ({
      insert: async (data: any[]) => {
        console.log(`Supabase INSERT into ${table}: ${data.length} rows`);
        return { data, error: null };
      },
      select: async () => {
        return { data: [], error: null };
      },
    }),
  };

  // Generate user profiles
  const profiles = await synth.generateStructured({
    count: 50,
    schema: {
      id: 'UUID',
      username: 'unique username',
      full_name: 'full name',
      avatar_url: 'profile image URL or null',
      bio: 'short bio or null',
      website: 'URL or null',
      created_at: 'ISO timestamp',
    },
  });

  // Insert into Supabase
  const { error } = await supabase.from('profiles').insert(profiles.data);

  if (!error) {
    console.log(`✅ Inserted ${profiles.data.length} profiles into Supabase`);
  }

  // Generate posts
  const posts = await synth.generateStructured({
    count: 200,
    schema: {
      id: 'UUID',
      user_id: 'UUID (from profiles)',
      title: 'post title',
      content: 'post content',
      published: 'boolean',
      created_at: 'ISO timestamp',
    },
  });

  await supabase.from('posts').insert(posts.data);
  console.log(`✅ Inserted ${posts.data.length} posts into Supabase`);
}

// ============================================================================
// Run All Examples
// ============================================================================

export async function runAllExamples() {
  console.log('🚀 Running All Integration Examples\n');
  console.log('='.repeat(60));

  const examples = [
    { name: 'Express.js API', fn: expressAPIExample },
    { name: 'Prisma Seeding', fn: prismaSeedingExample },
    { name: 'Jest Fixtures', fn: jestFixturesExample },
    { name: 'TensorFlow.js', fn: tensorflowTrainingExample },
    { name: 'GraphQL Mocking', fn: graphqlMockingExample },
    { name: 'Redis Caching', fn: redisCachingExample },
    { name: 'Kafka Streaming', fn: kafkaStreamingExample },
    { name: 'Elasticsearch', fn: elasticsearchIndexingExample },
    { name: 'Next.js API', fn: nextjsAPIRouteExample },
    { name: 'Supabase', fn: supabaseIntegrationExample },
  ];

  for (const example of examples) {
    try {
      await example.fn();
      console.log('='.repeat(60));
    } catch (error: any) {
      console.error(`❌ Error in ${example.name}:`, error.message);
    }
  }

  console.log('\n✅ All integration examples completed!\n');
}

// Run if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
  runAllExamples().catch(console.error);
}
