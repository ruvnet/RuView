/**
 * CI/CD Test Data Generator Examples
 *
 * This module demonstrates how to use agentic-synth to generate
 * comprehensive test data for CI/CD pipelines including:
 * - Database fixtures for integration tests
 * - API mock responses
 * - User session data for E2E tests
 * - Load testing datasets
 * - Configuration variations for multi-environment testing
 *
 * @module test-data-generator
 */

import { AgenticSynth, createSynth, GenerationResult, SynthError } from '../../src/index.js';
import * as fs from 'fs/promises';
import * as path from 'path';

/**
 * Configuration for test data generation
 */
export interface TestDataConfig {
  outputDir: string;
  format: 'json' | 'csv' | 'array';
  provider?: 'gemini' | 'openrouter';
  apiKey?: string;
  seed?: string | number;
}

/**
 * Test data generator class for CI/CD pipelines
 */
export class CICDTestDataGenerator {
  private synth: AgenticSynth;
  private config: TestDataConfig;

  constructor(config: Partial<TestDataConfig> = {}) {
    this.config = {
      outputDir: config.outputDir || './test-data',
      format: config.format || 'json',
      provider: config.provider || 'gemini',
      apiKey: config.apiKey || process.env.GEMINI_API_KEY,
      seed: config.seed
    };

    // Initialize agentic-synth
    this.synth = createSynth({
      provider: this.config.provider,
      apiKey: this.config.apiKey,
      cacheStrategy: 'memory',
      cacheTTL: 3600,
      maxRetries: 3
    });
  }

  /**
   * Generate database fixtures for integration tests
   *
   * Creates realistic database records with proper relationships
   * and constraints for testing database operations.
   *
   * @example
   * ```typescript
   * const generator = new CICDTestDataGenerator();
   * const fixtures = await generator.generateDatabaseFixtures({
   *   users: 50,
   *   posts: 200,
   *   comments: 500
   * });
   * ```
   */
  async generateDatabaseFixtures(options: {
    users?: number;
    posts?: number;
    comments?: number;
    orders?: number;
    products?: number;
  } = {}): Promise<Record<string, GenerationResult>> {
    const {
      users = 10,
      posts = 50,
      comments = 100,
      orders = 25,
      products = 30
    } = options;

    console.log('Generating database fixtures...');

    try {
      // Generate users with realistic data
      const usersSchema = {
        id: { type: 'uuid', required: true },
        username: { type: 'string', required: true, pattern: '^[a-z0-9_]{3,20}$' },
        email: { type: 'email', required: true },
        firstName: { type: 'string', required: true },
        lastName: { type: 'string', required: true },
        passwordHash: { type: 'string', required: true },
        role: { type: 'enum', values: ['admin', 'user', 'moderator'], required: true },
        isActive: { type: 'boolean', required: true },
        emailVerified: { type: 'boolean', required: true },
        createdAt: { type: 'timestamp', required: true },
        lastLoginAt: { type: 'timestamp', required: false },
        profile: {
          type: 'object',
          properties: {
            bio: { type: 'string' },
            avatar: { type: 'url' },
            timezone: { type: 'string' },
            language: { type: 'string' }
          }
        }
      };

      // Generate posts with foreign key relationships
      const postsSchema = {
        id: { type: 'uuid', required: true },
        userId: { type: 'uuid', required: true }, // Foreign key to users
        title: { type: 'string', required: true, minLength: 10, maxLength: 200 },
        content: { type: 'text', required: true, minLength: 100 },
        slug: { type: 'string', required: true },
        status: { type: 'enum', values: ['draft', 'published', 'archived'], required: true },
        publishedAt: { type: 'timestamp', required: false },
        viewCount: { type: 'integer', min: 0, max: 1000000, required: true },
        tags: { type: 'array', items: { type: 'string' } },
        createdAt: { type: 'timestamp', required: true },
        updatedAt: { type: 'timestamp', required: true }
      };

      // Generate comments with nested relationships
      const commentsSchema = {
        id: { type: 'uuid', required: true },
        postId: { type: 'uuid', required: true }, // Foreign key to posts
        userId: { type: 'uuid', required: true }, // Foreign key to users
        parentId: { type: 'uuid', required: false }, // Self-referencing for nested comments
        content: { type: 'text', required: true, minLength: 10, maxLength: 1000 },
        isEdited: { type: 'boolean', required: true },
        isDeleted: { type: 'boolean', required: true },
        upvotes: { type: 'integer', min: 0, required: true },
        downvotes: { type: 'integer', min: 0, required: true },
        createdAt: { type: 'timestamp', required: true },
        updatedAt: { type: 'timestamp', required: true }
      };

      // Generate products for e-commerce tests
      const productsSchema = {
        id: { type: 'uuid', required: true },
        sku: { type: 'string', required: true, pattern: '^[A-Z0-9-]{8,15}$' },
        name: { type: 'string', required: true },
        description: { type: 'text', required: true },
        price: { type: 'decimal', min: 0.01, max: 10000, required: true },
        currency: { type: 'string', required: true, default: 'USD' },
        stockQuantity: { type: 'integer', min: 0, max: 10000, required: true },
        category: { type: 'string', required: true },
        brand: { type: 'string', required: false },
        weight: { type: 'decimal', min: 0, required: false },
        dimensions: {
          type: 'object',
          properties: {
            length: { type: 'decimal' },
            width: { type: 'decimal' },
            height: { type: 'decimal' },
            unit: { type: 'string', default: 'cm' }
          }
        },
        images: { type: 'array', items: { type: 'url' } },
        isActive: { type: 'boolean', required: true },
        createdAt: { type: 'timestamp', required: true }
      };

      // Generate orders with complex relationships
      const ordersSchema = {
        id: { type: 'uuid', required: true },
        userId: { type: 'uuid', required: true },
        orderNumber: { type: 'string', required: true, pattern: '^ORD-[0-9]{10}$' },
        status: { type: 'enum', values: ['pending', 'processing', 'shipped', 'delivered', 'cancelled'], required: true },
        subtotal: { type: 'decimal', min: 0, required: true },
        tax: { type: 'decimal', min: 0, required: true },
        shipping: { type: 'decimal', min: 0, required: true },
        total: { type: 'decimal', min: 0, required: true },
        currency: { type: 'string', required: true, default: 'USD' },
        paymentMethod: { type: 'enum', values: ['credit_card', 'paypal', 'bank_transfer'], required: true },
        paymentStatus: { type: 'enum', values: ['pending', 'completed', 'failed', 'refunded'], required: true },
        shippingAddress: {
          type: 'object',
          properties: {
            street: { type: 'string' },
            city: { type: 'string' },
            state: { type: 'string' },
            postalCode: { type: 'string' },
            country: { type: 'string' }
          }
        },
        items: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              productId: { type: 'uuid' },
              quantity: { type: 'integer', min: 1 },
              price: { type: 'decimal' }
            }
          }
        },
        createdAt: { type: 'timestamp', required: true },
        updatedAt: { type: 'timestamp', required: true }
      };

      // Generate all fixtures in parallel
      const [usersResult, postsResult, commentsResult, productsResult, ordersResult] =
        await Promise.all([
          this.synth.generateStructured({ count: users, schema: usersSchema, seed: this.config.seed }),
          this.synth.generateStructured({ count: posts, schema: postsSchema, seed: this.config.seed }),
          this.synth.generateStructured({ count: comments, schema: commentsSchema, seed: this.config.seed }),
          this.synth.generateStructured({ count: products, schema: productsSchema, seed: this.config.seed }),
          this.synth.generateStructured({ count: orders, schema: ordersSchema, seed: this.config.seed })
        ]);

      // Save to files
      await this.saveToFile('users', usersResult);
      await this.saveToFile('posts', postsResult);
      await this.saveToFile('comments', commentsResult);
      await this.saveToFile('products', productsResult);
      await this.saveToFile('orders', ordersResult);

      console.log('✅ Database fixtures generated successfully');
      console.log(`   Users: ${usersResult.metadata.count}`);
      console.log(`   Posts: ${postsResult.metadata.count}`);
      console.log(`   Comments: ${commentsResult.metadata.count}`);
      console.log(`   Products: ${productsResult.metadata.count}`);
      console.log(`   Orders: ${ordersResult.metadata.count}`);

      return {
        users: usersResult,
        posts: postsResult,
        comments: commentsResult,
        products: productsResult,
        orders: ordersResult
      };
    } catch (error) {
      console.error('❌ Failed to generate database fixtures:', error);
      throw new SynthError('Database fixture generation failed', 'FIXTURE_ERROR', error);
    }
  }

  /**
   * Generate API mock responses for testing
   *
   * Creates realistic API responses with various status codes,
   * headers, and payloads for comprehensive API testing.
   */
  async generateAPIMockResponses(options: {
    endpoints?: string[];
    responsesPerEndpoint?: number;
    includeErrors?: boolean;
  } = {}): Promise<GenerationResult> {
    const {
      endpoints = ['/api/users', '/api/posts', '/api/products', '/api/orders'],
      responsesPerEndpoint = 5,
      includeErrors = true
    } = options;

    console.log('Generating API mock responses...');

    try {
      const mockResponseSchema = {
        endpoint: { type: 'string', required: true },
        method: { type: 'enum', values: ['GET', 'POST', 'PUT', 'PATCH', 'DELETE'], required: true },
        statusCode: { type: 'integer', required: true },
        statusText: { type: 'string', required: true },
        headers: {
          type: 'object',
          properties: {
            'Content-Type': { type: 'string' },
            'X-Request-Id': { type: 'uuid' },
            'X-RateLimit-Limit': { type: 'integer' },
            'X-RateLimit-Remaining': { type: 'integer' },
            'Cache-Control': { type: 'string' }
          }
        },
        body: { type: 'object', required: true },
        latency: { type: 'integer', min: 10, max: 5000, required: true },
        timestamp: { type: 'timestamp', required: true }
      };

      const totalResponses = endpoints.length * responsesPerEndpoint;
      const result = await this.synth.generateStructured({
        count: totalResponses,
        schema: mockResponseSchema,
        seed: this.config.seed
      });

      await this.saveToFile('api-mocks', result);

      console.log('✅ API mock responses generated successfully');
      console.log(`   Total responses: ${result.metadata.count}`);
      console.log(`   Endpoints: ${endpoints.length}`);

      return result;
    } catch (error) {
      console.error('❌ Failed to generate API mocks:', error);
      throw new SynthError('API mock generation failed', 'MOCK_ERROR', error);
    }
  }

  /**
   * Generate user session data for E2E tests
   *
   * Creates realistic user sessions with cookies, tokens,
   * and session state for end-to-end testing.
   */
  async generateUserSessions(options: {
    sessionCount?: number;
    includeAnonymous?: boolean;
  } = {}): Promise<GenerationResult> {
    const {
      sessionCount = 20,
      includeAnonymous = true
    } = options;

    console.log('Generating user session data...');

    try {
      const sessionSchema = {
        sessionId: { type: 'uuid', required: true },
        userId: { type: 'uuid', required: false }, // Null for anonymous sessions
        isAuthenticated: { type: 'boolean', required: true },
        username: { type: 'string', required: false },
        email: { type: 'email', required: false },
        token: { type: 'string', required: false }, // JWT token
        refreshToken: { type: 'string', required: false },
        tokenExpiry: { type: 'timestamp', required: false },
        cookies: {
          type: 'object',
          properties: {
            sessionId: { type: 'string' },
            csrfToken: { type: 'string' },
            preferences: { type: 'string' }
          }
        },
        userAgent: { type: 'string', required: true },
        ipAddress: { type: 'string', required: true },
        location: {
          type: 'object',
          properties: {
            country: { type: 'string' },
            city: { type: 'string' },
            timezone: { type: 'string' }
          }
        },
        permissions: { type: 'array', items: { type: 'string' } },
        createdAt: { type: 'timestamp', required: true },
        lastActivityAt: { type: 'timestamp', required: true },
        expiresAt: { type: 'timestamp', required: true }
      };

      const result = await this.synth.generateStructured({
        count: sessionCount,
        schema: sessionSchema,
        seed: this.config.seed
      });

      await this.saveToFile('user-sessions', result);

      console.log('✅ User session data generated successfully');
      console.log(`   Sessions: ${result.metadata.count}`);

      return result;
    } catch (error) {
      console.error('❌ Failed to generate user sessions:', error);
      throw new SynthError('Session generation failed', 'SESSION_ERROR', error);
    }
  }

  /**
   * Generate load testing datasets
   *
   * Creates large-scale datasets for load and performance testing
   * with configurable data patterns and distributions.
   */
  async generateLoadTestData(options: {
    requestCount?: number;
    concurrent?: number;
    duration?: number; // in minutes
  } = {}): Promise<GenerationResult> {
    const {
      requestCount = 10000,
      concurrent = 100,
      duration = 10
    } = options;

    console.log('Generating load test data...');

    try {
      const loadTestSchema = {
        requestId: { type: 'uuid', required: true },
        endpoint: { type: 'string', required: true },
        method: { type: 'enum', values: ['GET', 'POST', 'PUT', 'DELETE'], required: true },
        payload: { type: 'object', required: false },
        headers: {
          type: 'object',
          properties: {
            'Authorization': { type: 'string' },
            'Content-Type': { type: 'string' },
            'User-Agent': { type: 'string' }
          }
        },
        timestamp: { type: 'timestamp', required: true },
        priority: { type: 'enum', values: ['low', 'medium', 'high', 'critical'], required: true },
        expectedStatusCode: { type: 'integer', required: true },
        timeout: { type: 'integer', min: 1000, max: 30000, required: true }
      };

      // Generate in batches for better performance
      const batchSize = 1000;
      const batches = Math.ceil(requestCount / batchSize);
      const batchOptions = Array.from({ length: batches }, () => ({
        count: batchSize,
        schema: loadTestSchema,
        seed: this.config.seed
      }));

      const results = await this.synth.generateBatch('structured', batchOptions, concurrent);

      // Combine all results
      const combinedData = results.flatMap(r => r.data);
      const combinedResult: GenerationResult = {
        data: combinedData,
        metadata: {
          count: combinedData.length,
          generatedAt: new Date(),
          provider: results[0].metadata.provider,
          model: results[0].metadata.model,
          cached: false,
          duration: results.reduce((sum, r) => sum + r.metadata.duration, 0)
        }
      };

      await this.saveToFile('load-test-data', combinedResult);

      console.log('✅ Load test data generated successfully');
      console.log(`   Requests: ${combinedResult.metadata.count}`);
      console.log(`   Duration: ${combinedResult.metadata.duration}ms`);

      return combinedResult;
    } catch (error) {
      console.error('❌ Failed to generate load test data:', error);
      throw new SynthError('Load test data generation failed', 'LOAD_TEST_ERROR', error);
    }
  }

  /**
   * Generate configuration variations for multi-environment testing
   *
   * Creates configuration files for different environments
   * (dev, staging, production) with realistic values.
   */
  async generateEnvironmentConfigs(options: {
    environments?: string[];
    includeSecrets?: boolean;
  } = {}): Promise<Record<string, GenerationResult>> {
    const {
      environments = ['development', 'staging', 'production'],
      includeSecrets = false
    } = options;

    console.log('Generating environment configurations...');

    try {
      const configSchema = {
        environment: { type: 'string', required: true },
        app: {
          type: 'object',
          properties: {
            name: { type: 'string' },
            version: { type: 'string', pattern: '^\\d+\\.\\d+\\.\\d+$' },
            port: { type: 'integer', min: 3000, max: 9999 },
            host: { type: 'string' },
            logLevel: { type: 'enum', values: ['debug', 'info', 'warn', 'error'] }
          }
        },
        database: {
          type: 'object',
          properties: {
            host: { type: 'string' },
            port: { type: 'integer' },
            name: { type: 'string' },
            username: { type: 'string' },
            password: { type: 'string', required: includeSecrets },
            ssl: { type: 'boolean' },
            poolSize: { type: 'integer', min: 5, max: 100 },
            timeout: { type: 'integer' }
          }
        },
        redis: {
          type: 'object',
          properties: {
            host: { type: 'string' },
            port: { type: 'integer' },
            password: { type: 'string', required: includeSecrets },
            db: { type: 'integer', min: 0, max: 15 }
          }
        },
        api: {
          type: 'object',
          properties: {
            baseUrl: { type: 'url' },
            timeout: { type: 'integer' },
            retries: { type: 'integer', min: 0, max: 5 },
            rateLimit: {
              type: 'object',
              properties: {
                maxRequests: { type: 'integer' },
                windowMs: { type: 'integer' }
              }
            }
          }
        },
        features: {
          type: 'object',
          properties: {
            authentication: { type: 'boolean' },
            caching: { type: 'boolean' },
            monitoring: { type: 'boolean' },
            analytics: { type: 'boolean' }
          }
        }
      };

      const results: Record<string, GenerationResult> = {};

      for (const env of environments) {
        const result = await this.synth.generateStructured({
          count: 1,
          schema: { ...configSchema, environment: { type: 'string', default: env } },
          seed: `${this.config.seed}-${env}`
        });

        results[env] = result;
        await this.saveToFile(`config-${env}`, result);
      }

      console.log('✅ Environment configurations generated successfully');
      console.log(`   Environments: ${environments.join(', ')}`);

      return results;
    } catch (error) {
      console.error('❌ Failed to generate environment configs:', error);
      throw new SynthError('Config generation failed', 'CONFIG_ERROR', error);
    }
  }

  /**
   * Generate all test data at once
   *
   * Convenience method to generate all types of test data
   * in a single operation.
   */
  async generateAll(options: {
    users?: number;
    posts?: number;
    comments?: number;
    orders?: number;
    products?: number;
    apiMocks?: number;
    sessions?: number;
    loadTestRequests?: number;
  } = {}): Promise<void> {
    console.log('🚀 Generating all test data...\n');

    const startTime = Date.now();

    try {
      await Promise.all([
        this.generateDatabaseFixtures({
          users: options.users,
          posts: options.posts,
          comments: options.comments,
          orders: options.orders,
          products: options.products
        }),
        this.generateAPIMockResponses({
          responsesPerEndpoint: options.apiMocks || 5
        }),
        this.generateUserSessions({
          sessionCount: options.sessions || 20
        }),
        this.generateEnvironmentConfigs()
      ]);

      // Load test data generation is CPU-intensive, run separately
      if (options.loadTestRequests && options.loadTestRequests > 0) {
        await this.generateLoadTestData({
          requestCount: options.loadTestRequests
        });
      }

      const duration = Date.now() - startTime;

      console.log(`\n✅ All test data generated successfully in ${duration}ms`);
      console.log(`📁 Output directory: ${path.resolve(this.config.outputDir)}`);
    } catch (error) {
      console.error('\n❌ Failed to generate test data:', error);
      throw error;
    }
  }

  /**
   * Save generation result to file
   */
  private async saveToFile(name: string, result: GenerationResult): Promise<void> {
    try {
      // Ensure output directory exists
      await fs.mkdir(this.config.outputDir, { recursive: true });

      const filename = `${name}.${this.config.format}`;
      const filepath = path.join(this.config.outputDir, filename);

      let content: string;

      if (this.config.format === 'json') {
        content = JSON.stringify(result.data, null, 2);
      } else if (this.config.format === 'csv') {
        // Simple CSV conversion (you might want to use a library for production)
        if (result.data.length === 0) {
          content = '';
        } else {
          const headers = Object.keys(result.data[0]);
          const rows = result.data.map((item: any) =>
            headers.map(header => JSON.stringify(item[header] ?? '')).join(',')
          );
          content = [headers.join(','), ...rows].join('\n');
        }
      } else {
        content = JSON.stringify(result.data, null, 2);
      }

      await fs.writeFile(filepath, content, 'utf-8');

      // Also save metadata
      const metadataPath = path.join(this.config.outputDir, `${name}.metadata.json`);
      await fs.writeFile(
        metadataPath,
        JSON.stringify(result.metadata, null, 2),
        'utf-8'
      );
    } catch (error) {
      console.error(`Failed to save ${name}:`, error);
      throw error;
    }
  }
}

/**
 * Example usage in CI/CD pipeline
 */
async function cicdExample() {
  // Initialize generator
  const generator = new CICDTestDataGenerator({
    outputDir: './test-fixtures',
    format: 'json',
    provider: 'gemini',
    seed: process.env.CI_COMMIT_SHA || 'default-seed' // Use commit SHA for reproducibility
  });

  // Generate all test data
  await generator.generateAll({
    users: 50,
    posts: 200,
    comments: 500,
    orders: 100,
    products: 75,
    apiMocks: 10,
    sessions: 30,
    loadTestRequests: 5000
  });

  console.log('Test data ready for CI/CD pipeline');
}

/**
 * GitHub Actions example
 */
async function githubActionsExample() {
  const generator = new CICDTestDataGenerator({
    outputDir: process.env.GITHUB_WORKSPACE + '/test-data',
    seed: process.env.GITHUB_SHA
  });

  await generator.generateDatabaseFixtures();
  await generator.generateAPIMockResponses();
}

/**
 * GitLab CI example
 */
async function gitlabCIExample() {
  const generator = new CICDTestDataGenerator({
    outputDir: process.env.CI_PROJECT_DIR + '/test-data',
    seed: process.env.CI_COMMIT_SHORT_SHA
  });

  await generator.generateAll();
}

// Export for use in CI/CD scripts
export {
  cicdExample,
  githubActionsExample,
  gitlabCIExample
};

// Run if called directly
if (import.meta.url === `file://${process.argv[1]}`) {
  cicdExample().catch(console.error);
}
