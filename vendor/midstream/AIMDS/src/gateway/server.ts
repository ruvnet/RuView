/**
 * AIMDS API Gateway Server
 * Production-ready Express server with AgentDB and lean-agentic integration
 */

import express, { Request, Response, NextFunction } from 'express';
import cors from 'cors';
import helmet from 'helmet';
import compression from 'compression';
import rateLimit from 'express-rate-limit';
import { AgentDBClient } from '../agentdb/client';
import { LeanAgenticVerifier } from '../lean-agentic/verifier';
import { MetricsCollector } from '../monitoring/metrics';
import { Logger } from '../utils/logger';
import {
  AIMDSRequest,
  DefenseResult,
  ThreatLevel,
  GatewayConfig,
  AgentDBConfig,
  LeanAgenticConfig,
  SecurityPolicy,
  AIMDSRequestSchema,
  ThreatIncident
} from '../types';
import { createHash } from 'crypto';

export class AIMDSGateway {
  private app: express.Application;
  private agentdb: AgentDBClient;
  private verifier: LeanAgenticVerifier;
  private metrics: MetricsCollector;
  private logger: Logger;
  private config: GatewayConfig;
  private defaultPolicy: SecurityPolicy;
  private server?: any;

  constructor(
    gatewayConfig: GatewayConfig,
    agentdbConfig: AgentDBConfig,
    verifierConfig: LeanAgenticConfig
  ) {
    this.config = gatewayConfig;
    this.logger = new Logger('AIMDSGateway');
    this.agentdb = new AgentDBClient(agentdbConfig, this.logger);
    this.verifier = new LeanAgenticVerifier(verifierConfig, this.logger);
    this.metrics = new MetricsCollector(this.logger);
    this.app = express();
    this.defaultPolicy = this.createDefaultPolicy();
  }

  /**
   * Initialize the gateway and all components
   */
  async initialize(): Promise<void> {
    try {
      this.logger.info('Initializing AIMDS Gateway...');

      // Initialize components in parallel
      await Promise.all([
        this.agentdb.initialize(),
        this.verifier.initialize(),
        this.metrics.initialize()
      ]);

      // Configure Express middleware
      this.configureMiddleware();

      // Setup routes
      this.setupRoutes();

      // Error handling
      this.setupErrorHandling();

      this.logger.info('AIMDS Gateway initialized successfully');
    } catch (error) {
      this.logger.error('Failed to initialize gateway', { error });
      throw error;
    }
  }

  /**
   * Start the gateway server
   */
  async start(): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        this.server = this.app.listen(this.config.port, this.config.host, () => {
          this.logger.info(`Gateway listening on ${this.config.host}:${this.config.port}`);
          resolve();
        });

        this.server.on('error', reject);
      } catch (error) {
        reject(error);
      }
    });
  }

  /**
   * Process incoming security request
   * Fast path: Vector search + pattern matching (<10ms)
   * Deep path if needed: Behavioral + LTL verification (<520ms)
   */
  async processRequest(req: AIMDSRequest): Promise<DefenseResult> {
    const startTime = Date.now();
    const requestId = req.id;

    try {
      this.logger.debug('Processing request', { requestId, type: req.action.type });

      // Step 1: Generate embedding for request (fast)
      const embedding = await this.generateEmbedding(req);
      const embedTime = Date.now();

      // Step 2: Fast path - Vector search with HNSW (<2ms target)
      const vectorSearchStart = Date.now();
      const matches = await this.agentdb.vectorSearch(embedding, {
        k: 10,
        threshold: 0.75,
        diversityFactor: 0.3
      });
      const vectorSearchTime = Date.now() - vectorSearchStart;

      // Calculate threat level from matches
      const threatLevel = this.calculateThreatLevel(matches);
      const confidence = this.calculateConfidence(matches);

      // Step 3: Quick decision for low-risk requests
      if (threatLevel <= ThreatLevel.LOW && confidence >= 0.9) {
        const result: DefenseResult = {
          allowed: true,
          confidence,
          latencyMs: Date.now() - startTime,
          threatLevel,
          matches,
          metadata: {
            vectorSearchTime,
            verificationTime: 0,
            totalTime: Date.now() - startTime,
            pathTaken: 'fast'
          }
        };

        this.metrics.recordDetection(result.latencyMs, result);
        await this.storeIncident(req, result, embedding);

        return result;
      }

      // Step 4: Deep path - Formal verification for high-risk requests
      const verificationStart = Date.now();
      const action = this.requestToAction(req);
      const verificationResult = await this.verifier.verifyPolicy(
        action,
        this.defaultPolicy
      );
      const verificationTime = Date.now() - verificationStart;

      // Step 5: Make final decision
      const allowed = verificationResult.valid && threatLevel < ThreatLevel.CRITICAL;

      const result: DefenseResult = {
        allowed,
        confidence: verificationResult.valid ? Math.min(confidence, 0.95) : 0,
        latencyMs: Date.now() - startTime,
        threatLevel,
        matches,
        verificationProof: verificationResult.proof,
        metadata: {
          vectorSearchTime,
          verificationTime,
          totalTime: Date.now() - startTime,
          pathTaken: 'deep'
        }
      };

      this.metrics.recordDetection(result.latencyMs, result);
      await this.storeIncident(req, result, embedding);

      this.logger.debug('Request processed', {
        requestId,
        allowed,
        latency: result.latencyMs,
        path: result.metadata.pathTaken
      });

      return result;
    } catch (error) {
      this.logger.error('Request processing failed', { error, requestId });

      // Fail closed - deny on error
      return {
        allowed: false,
        confidence: 0,
        latencyMs: Date.now() - startTime,
        threatLevel: ThreatLevel.CRITICAL,
        matches: [],
        metadata: {
          vectorSearchTime: 0,
          verificationTime: 0,
          totalTime: Date.now() - startTime,
          pathTaken: 'fast'
        }
      };
    }
  }

  /**
   * Graceful shutdown
   */
  async shutdown(): Promise<void> {
    this.logger.info('Shutting down gateway...');

    return new Promise((resolve) => {
      // Stop accepting new connections
      if (this.server) {
        this.server.close(async () => {
          // Shutdown components
          await Promise.all([
            this.agentdb.shutdown(),
            this.verifier.shutdown(),
            this.metrics.shutdown()
          ]);

          this.logger.info('Gateway shutdown complete');
          resolve();
        });

        // Force close after timeout
        setTimeout(() => {
          this.logger.warn('Forcing shutdown after timeout');
          resolve();
        }, this.config.timeouts.shutdown);
      } else {
        resolve();
      }
    });
  }

  // ============================================================================
  // Private Methods - Express Configuration
  // ============================================================================

  private configureMiddleware(): void {
    // Security headers
    this.app.use(helmet());

    // CORS
    if (this.config.enableCors) {
      this.app.use(cors());
    }

    // Compression
    if (this.config.enableCompression) {
      this.app.use(compression());
    }

    // Rate limiting
    const limiter = rateLimit({
      windowMs: this.config.rateLimit.windowMs,
      max: this.config.rateLimit.max,
      message: 'Too many requests from this IP'
    });
    this.app.use('/api/', limiter);

    // Body parsing
    this.app.use(express.json({ limit: '1mb' }));
    this.app.use(express.urlencoded({ extended: true, limit: '1mb' }));

    // Request timeout
    this.app.use((req: Request, res: Response, next: NextFunction) => {
      req.setTimeout(this.config.timeouts.request);
      next();
    });

    // Request logging
    this.app.use((req: Request, res: Response, next: NextFunction) => {
      const start = Date.now();
      res.on('finish', () => {
        this.logger.debug('Request completed', {
          method: req.method,
          path: req.path,
          status: res.statusCode,
          latency: Date.now() - start
        });
      });
      next();
    });
  }

  private setupRoutes(): void {
    // Health check
    this.app.get('/health', async (req: Request, res: Response) => {
      try {
        const [agentdbStats, verifierStats] = await Promise.all([
          this.agentdb.getStats(),
          this.verifier.getCacheStats()
        ]);

        res.json({
          status: 'healthy',
          timestamp: Date.now(),
          components: {
            gateway: { status: 'up' },
            agentdb: { status: 'up', ...agentdbStats },
            verifier: { status: 'up', ...verifierStats }
          }
        });
      } catch (error) {
        res.status(503).json({
          status: 'unhealthy',
          error: error instanceof Error ? error.message : 'Unknown error'
        });
      }
    });

    // Metrics endpoint
    this.app.get('/metrics', async (req: Request, res: Response) => {
      const metrics = await this.metrics.exportPrometheus();
      res.set('Content-Type', 'text/plain');
      res.send(metrics);
    });

    // Main defense endpoint
    this.app.post('/api/v1/defend', async (req: Request, res: Response) => {
      try {
        // Validate request
        const validatedReq = AIMDSRequestSchema.parse({
          ...req.body,
          id: req.body.id || this.generateRequestId(),
          timestamp: req.body.timestamp || Date.now(),
          source: {
            ...req.body.source,
            ip: req.body.source?.ip || req.ip,
            headers: req.body.source?.headers || req.headers
          }
        });

        // Process request
        const result = await this.processRequest(validatedReq);

        // Return result
        res.status(result.allowed ? 200 : 403).json({
          requestId: validatedReq.id,
          allowed: result.allowed,
          confidence: result.confidence,
          threatLevel: ThreatLevel[result.threatLevel],
          latency: result.latencyMs,
          metadata: result.metadata,
          proof: result.verificationProof?.id
        });
      } catch (error) {
        this.logger.error('Defense endpoint error', { error });
        res.status(400).json({
          error: error instanceof Error ? error.message : 'Invalid request'
        });
      }
    });

    // Batch defense endpoint
    this.app.post('/api/v1/defend/batch', async (req: Request, res: Response) => {
      try {
        const requests: AIMDSRequest[] = req.body.requests || [];

        if (requests.length === 0 || requests.length > 100) {
          return res.status(400).json({
            error: 'Batch size must be between 1 and 100'
          });
        }

        // Process in parallel
        const results = await Promise.all(
          requests.map(r => this.processRequest(r))
        );

        res.json({ results });
      } catch (error) {
        res.status(400).json({
          error: error instanceof Error ? error.message : 'Invalid request'
        });
      }
    });

    // Stats endpoint
    this.app.get('/api/v1/stats', async (req: Request, res: Response) => {
      const snapshot = await this.metrics.getSnapshot();
      res.json(snapshot);
    });
  }

  private setupErrorHandling(): void {
    // 404 handler
    this.app.use((req: Request, res: Response) => {
      res.status(404).json({ error: 'Not found' });
    });

    // Global error handler
    this.app.use((err: Error, req: Request, res: Response, next: NextFunction) => {
      this.logger.error('Unhandled error', { error: err });
      res.status(500).json({
        error: 'Internal server error',
        message: process.env.NODE_ENV === 'development' ? err.message : undefined
      });
    });
  }

  // ============================================================================
  // Private Methods - Request Processing
  // ============================================================================

  private async generateEmbedding(req: AIMDSRequest): Promise<number[]> {
    // Simple embedding generation (use proper embedding model in production)
    const text = JSON.stringify({
      type: req.action.type,
      resource: req.action.resource,
      method: req.action.method,
      ip: req.source.ip
    });

    // Hash-based embedding for demo (use BERT/etc in production)
    const hash = createHash('sha256').update(text).digest();
    const embedding = new Array(384);

    for (let i = 0; i < 384; i++) {
      embedding[i] = hash[i % hash.length] / 255;
    }

    return embedding;
  }

  private calculateThreatLevel(matches: any[]): ThreatLevel {
    if (matches.length === 0) return ThreatLevel.NONE;

    const maxThreat = Math.max(...matches.map(m => m.threatLevel));
    return maxThreat;
  }

  private calculateConfidence(matches: any[]): number {
    if (matches.length === 0) return 1.0;

    const avgSimilarity = matches.reduce((sum, m) => sum + m.similarity, 0) / matches.length;
    return avgSimilarity;
  }

  private requestToAction(req: AIMDSRequest): any {
    return {
      type: req.action.type,
      resource: req.action.resource,
      parameters: req.action.payload || {},
      context: {
        timestamp: req.timestamp,
        metadata: req.context
      }
    };
  }

  private async storeIncident(
    req: AIMDSRequest,
    result: DefenseResult,
    embedding: number[]
  ): Promise<void> {
    const incident: ThreatIncident = {
      id: req.id,
      timestamp: req.timestamp,
      request: req,
      result,
      embedding
    };

    await this.agentdb.storeIncident(incident);
  }

  private generateRequestId(): string {
    return `req_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
  }

  private createDefaultPolicy(): SecurityPolicy {
    return {
      id: 'default',
      name: 'Default Security Policy',
      rules: [
        {
          id: 'deny_critical',
          condition: 'threatLevel >= 4',
          action: 'deny',
          priority: 100
        },
        {
          id: 'verify_high',
          condition: 'threatLevel >= 3',
          action: 'verify',
          priority: 90
        },
        {
          id: 'allow_low',
          condition: 'threatLevel <= 1',
          action: 'allow',
          priority: 10
        }
      ],
      constraints: [
        {
          type: 'temporal',
          expression: 'timestamp > now() - 5min',
          severity: 'error'
        },
        {
          type: 'behavioral',
          expression: 'request_rate < 1000/min',
          severity: 'warning'
        }
      ]
    };
  }
}
