# 🚀 Agentic-Synth Deployment Guide

**Version**: 0.1.0
**Last Updated**: 2025-11-22

---

## Table of Contents

1. [Pre-Deployment Checklist](#1-pre-deployment-checklist)
2. [Environment Configuration](#2-environment-configuration)
3. [Deployment Platforms](#3-deployment-platforms)
4. [Production Best Practices](#4-production-best-practices)
5. [Monitoring & Logging](#5-monitoring--logging)
6. [Scaling Strategies](#6-scaling-strategies)
7. [Security Considerations](#7-security-considerations)
8. [Troubleshooting](#8-troubleshooting)

---

## 1. Pre-Deployment Checklist

### ✅ Code Readiness

- [ ] All tests passing (run `npm test`)
- [ ] Build succeeds (run `npm run build`)
- [ ] No ESLint errors (run `npm run lint`)
- [ ] TypeScript compiles (run `npm run typecheck`)
- [ ] Dependencies audited (run `npm audit`)
- [ ] Environment variables documented
- [ ] Error handling implemented
- [ ] Logging configured
- [ ] Performance benchmarks met

### ✅ Configuration

- [ ] API keys secured (not in source code)
- [ ] Cache strategy configured
- [ ] Retry logic enabled
- [ ] Rate limiting implemented
- [ ] Timeout values set appropriately
- [ ] Health check endpoint created
- [ ] Metrics collection enabled

### ✅ Documentation

- [ ] README.md up to date
- [ ] API documentation complete
- [ ] Environment variables listed
- [ ] Deployment instructions written
- [ ] Troubleshooting guide available

---

## 2. Environment Configuration

### 2.1 Environment Variables

Create a `.env` file (or configure in platform):

```bash
# API Configuration
SYNTH_PROVIDER=gemini
SYNTH_API_KEY=your-api-key-here
SYNTH_MODEL=gemini-2.0-flash-exp

# Optional: OpenRouter fallback
OPENROUTER_API_KEY=your-openrouter-key

# Cache Configuration
CACHE_STRATEGY=memory
CACHE_TTL=3600
MAX_CACHE_SIZE=10000

# Performance
MAX_RETRIES=3
REQUEST_TIMEOUT=30000
ENABLE_STREAMING=true

# Optional Integrations
ENABLE_AUTOMATION=false
ENABLE_VECTOR_DB=false
RUVECTOR_URL=http://localhost:3000

# Monitoring
LOG_LEVEL=info
ENABLE_METRICS=true
```

### 2.2 Configuration Validation

```typescript
// config/validate.ts
import { z } from 'zod';

const EnvSchema = z.object({
  SYNTH_PROVIDER: z.enum(['gemini', 'openrouter']),
  SYNTH_API_KEY: z.string().min(10),
  SYNTH_MODEL: z.string().optional(),
  CACHE_TTL: z.string().transform(Number).pipe(z.number().positive()),
  MAX_CACHE_SIZE: z.string().transform(Number).pipe(z.number().positive()),
  MAX_RETRIES: z.string().transform(Number).pipe(z.number().min(0).max(10)),
  REQUEST_TIMEOUT: z.string().transform(Number).pipe(z.number().positive()),
});

export function validateEnv() {
  try {
    return EnvSchema.parse(process.env);
  } catch (error) {
    console.error('❌ Environment validation failed:', error);
    process.exit(1);
  }
}
```

---

## 3. Deployment Platforms

### 3.1 Node.js Server (Express/Fastify)

**Installation:**

```bash
npm install @ruvector/agentic-synth express dotenv
```

**Server Setup:**

```typescript
// server.ts
import express from 'express';
import { AgenticSynth } from '@ruvector/agentic-synth';
import dotenv from 'dotenv';

dotenv.config();

const app = express();
app.use(express.json());

// Initialize synth
const synth = new AgenticSynth({
  provider: process.env.SYNTH_PROVIDER as 'gemini',
  apiKey: process.env.SYNTH_API_KEY!,
  cacheStrategy: 'memory',
  cacheTTL: parseInt(process.env.CACHE_TTL || '3600'),
  maxCacheSize: parseInt(process.env.MAX_CACHE_SIZE || '10000'),
});

// Health check
app.get('/health', async (req, res) => {
  try {
    const stats = synth.cache.getStats();
    res.json({
      status: 'healthy',
      timestamp: new Date().toISOString(),
      cache: {
        size: stats.size,
        hitRate: (stats.hitRate * 100).toFixed(2) + '%'
      }
    });
  } catch (error) {
    res.status(503).json({ status: 'unhealthy', error: error.message });
  }
});

// Generate endpoint
app.post('/generate/:type', async (req, res) => {
  try {
    const { type } = req.params;
    const options = req.body;

    const result = await synth.generate(type as any, options);
    res.json(result);
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

const PORT = process.env.PORT || 3000;
app.listen(PORT, () => {
  console.log(`✅ Server running on port ${PORT}`);
});
```

**Start:**

```bash
npm run build
node dist/server.js
```

### 3.2 AWS Lambda (Serverless)

**Installation:**

```bash
npm install @ruvector/agentic-synth aws-lambda
```

**Lambda Handler:**

```typescript
// lambda/handler.ts
import { APIGatewayProxyEvent, APIGatewayProxyResult } from 'aws-lambda';
import { AgenticSynth } from '@ruvector/agentic-synth';

// Initialize outside handler for reuse (Lambda warm starts)
const synth = new AgenticSynth({
  provider: process.env.SYNTH_PROVIDER as 'gemini',
  apiKey: process.env.SYNTH_API_KEY!,
  cacheStrategy: 'memory',
  cacheTTL: 3600,
});

export const handler = async (
  event: APIGatewayProxyEvent
): Promise<APIGatewayProxyResult> => {
  try {
    const { type, ...options } = JSON.parse(event.body || '{}');

    const result = await synth.generate(type, options);

    return {
      statusCode: 200,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(result),
    };
  } catch (error) {
    return {
      statusCode: 500,
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ error: error.message }),
    };
  }
};
```

**Deployment (Serverless Framework):**

```yaml
# serverless.yml
service: agentic-synth-api

provider:
  name: aws
  runtime: nodejs20.x
  region: us-east-1
  environment:
    SYNTH_PROVIDER: ${env:SYNTH_PROVIDER}
    SYNTH_API_KEY: ${env:SYNTH_API_KEY}
    CACHE_TTL: 3600

functions:
  generate:
    handler: dist/lambda/handler.handler
    events:
      - http:
          path: generate
          method: post
    timeout: 30
    memorySize: 1024
```

```bash
serverless deploy
```

### 3.3 Docker Container

**Dockerfile:**

```dockerfile
FROM node:20-alpine

WORKDIR /app

# Copy package files
COPY package*.json ./
RUN npm ci --production

# Copy source and build
COPY . .
RUN npm run build

# Expose port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD node -e "require('http').get('http://localhost:3000/health', (r) => {process.exit(r.statusCode === 200 ? 0 : 1)})"

# Start server
CMD ["node", "dist/server.js"]
```

**Build & Run:**

```bash
docker build -t agentic-synth .
docker run -p 3000:3000 \
  -e SYNTH_PROVIDER=gemini \
  -e SYNTH_API_KEY=your-key \
  -e CACHE_TTL=3600 \
  agentic-synth
```

**Docker Compose:**

```yaml
version: '3.8'

services:
  agentic-synth:
    build: .
    ports:
      - "3000:3000"
    environment:
      - SYNTH_PROVIDER=gemini
      - SYNTH_API_KEY=${SYNTH_API_KEY}
      - CACHE_TTL=3600
      - MAX_CACHE_SIZE=10000
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/health"]
      interval: 30s
      timeout: 3s
      retries: 3
```

```bash
docker-compose up -d
```

### 3.4 Vercel (Edge Functions)

**Installation:**

```bash
npm install @ruvector/agentic-synth
```

**API Route:**

```typescript
// api/generate.ts
import type { VercelRequest, VercelResponse } from '@vercel/node';
import { AgenticSynth } from '@ruvector/agentic-synth';

const synth = new AgenticSynth({
  provider: process.env.SYNTH_PROVIDER as 'gemini',
  apiKey: process.env.SYNTH_API_KEY!,
  cacheStrategy: 'memory',
  cacheTTL: 3600,
});

export default async function handler(
  req: VercelRequest,
  res: VercelResponse
) {
  if (req.method !== 'POST') {
    return res.status(405).json({ error: 'Method not allowed' });
  }

  try {
    const { type, ...options } = req.body;
    const result = await synth.generate(type, options);
    res.status(200).json(result);
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
}
```

**Deploy:**

```bash
vercel deploy --prod
```

---

## 4. Production Best Practices

### 4.1 Error Handling

```typescript
import { AgenticSynth, APIError, ValidationError } from '@ruvector/agentic-synth';

app.post('/generate', async (req, res) => {
  try {
    const result = await synth.generate(req.body.type, req.body.options);
    res.json(result);
  } catch (error) {
    if (error instanceof ValidationError) {
      return res.status(400).json({
        error: 'Validation failed',
        details: error.validationErrors
      });
    }

    if (error instanceof APIError) {
      console.error('API Error:', {
        provider: error.provider,
        status: error.statusCode,
        message: error.message
      });

      return res.status(502).json({
        error: 'External API error',
        message: error.message
      });
    }

    // Unknown error
    console.error('Unexpected error:', error);
    res.status(500).json({ error: 'Internal server error' });
  }
});
```

### 4.2 Request Validation

```typescript
import { z } from 'zod';

const GenerateRequestSchema = z.object({
  type: z.enum(['time-series', 'events', 'structured']),
  options: z.object({
    count: z.number().positive().max(10000),
    schema: z.record(z.any()),
    constraints: z.array(z.string()).optional(),
  }),
});

app.post('/generate', async (req, res) => {
  try {
    const validated = GenerateRequestSchema.parse(req.body);
    const result = await synth.generate(validated.type, validated.options);
    res.json(result);
  } catch (error) {
    if (error instanceof z.ZodError) {
      return res.status(400).json({
        error: 'Invalid request',
        details: error.errors
      });
    }
    // ... other error handling
  }
});
```

### 4.3 Rate Limiting

```typescript
import rateLimit from 'express-rate-limit';

const limiter = rateLimit({
  windowMs: 60 * 1000, // 1 minute
  max: 60, // 60 requests per minute
  message: 'Too many requests, please try again later',
  standardHeaders: true,
  legacyHeaders: false,
});

app.use('/generate', limiter);
```

### 4.4 Caching Strategy

```typescript
// Use cache for repeated requests
const synth = new AgenticSynth({
  provider: 'gemini',
  apiKey: process.env.SYNTH_API_KEY!,
  cacheStrategy: 'memory',
  cacheTTL: 3600, // 1 hour
  maxCacheSize: 10000,
});

// Monitor cache performance
setInterval(() => {
  const stats = synth.cache.getStats();
  console.log('Cache Stats:', {
    size: stats.size,
    hitRate: (stats.hitRate * 100).toFixed(2) + '%',
    utilization: ((stats.size / 10000) * 100).toFixed(2) + '%'
  });
}, 60000); // Every minute
```

---

## 5. Monitoring & Logging

### 5.1 Structured Logging

```typescript
import winston from 'winston';

const logger = winston.createLogger({
  level: process.env.LOG_LEVEL || 'info',
  format: winston.format.json(),
  transports: [
    new winston.transports.File({ filename: 'error.log', level: 'error' }),
    new winston.transports.File({ filename: 'combined.log' }),
  ],
});

if (process.env.NODE_ENV !== 'production') {
  logger.add(new winston.transports.Console({
    format: winston.format.simple(),
  }));
}

// Log all requests
app.use((req, res, next) => {
  logger.info('Request', {
    method: req.method,
    path: req.path,
    timestamp: new Date().toISOString()
  });
  next();
});

// Log generation events
app.post('/generate', async (req, res) => {
  const start = Date.now();
  try {
    const result = await synth.generate(req.body.type, req.body.options);

    logger.info('Generation success', {
      type: req.body.type,
      count: req.body.options.count,
      duration: Date.now() - start,
      cached: result.metadata.cached,
      generationTime: result.metadata.generationTime
    });

    res.json(result);
  } catch (error) {
    logger.error('Generation failed', {
      type: req.body.type,
      error: error.message,
      duration: Date.now() - start
    });
    throw error;
  }
});
```

### 5.2 Metrics Collection

```typescript
import { Counter, Histogram, register } from 'prom-client';

// Define metrics
const requestCounter = new Counter({
  name: 'synth_requests_total',
  help: 'Total number of generation requests',
  labelNames: ['type', 'status']
});

const requestDuration = new Histogram({
  name: 'synth_request_duration_seconds',
  help: 'Duration of generation requests',
  labelNames: ['type']
});

const cacheHitRate = new Histogram({
  name: 'synth_cache_hit_rate',
  help: 'Cache hit rate percentage'
});

// Expose metrics endpoint
app.get('/metrics', async (req, res) => {
  res.set('Content-Type', register.contentType);
  res.end(await register.metrics());
});

// Track metrics
app.post('/generate', async (req, res) => {
  const end = requestDuration.startTimer({ type: req.body.type });

  try {
    const result = await synth.generate(req.body.type, req.body.options);

    requestCounter.inc({ type: req.body.type, status: 'success' });
    cacheHitRate.observe(result.metadata.cached ? 100 : 0);

    res.json(result);
  } catch (error) {
    requestCounter.inc({ type: req.body.type, status: 'error' });
    throw error;
  } finally {
    end();
  }
});
```

---

## 6. Scaling Strategies

### 6.1 Horizontal Scaling

**Load Balancer (Nginx):**

```nginx
upstream agentic_synth {
    least_conn;
    server synth1:3000 weight=1;
    server synth2:3000 weight=1;
    server synth3:3000 weight=1;
}

server {
    listen 80;

    location / {
        proxy_pass http://agentic_synth;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }

    location /health {
        proxy_pass http://agentic_synth/health;
        proxy_connect_timeout 2s;
        proxy_send_timeout 2s;
        proxy_read_timeout 2s;
    }
}
```

### 6.2 Kubernetes Deployment

```yaml
# deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: agentic-synth
spec:
  replicas: 3
  selector:
    matchLabels:
      app: agentic-synth
  template:
    metadata:
      labels:
        app: agentic-synth
    spec:
      containers:
      - name: agentic-synth
        image: agentic-synth:latest
        ports:
        - containerPort: 3000
        env:
        - name: SYNTH_PROVIDER
          value: "gemini"
        - name: SYNTH_API_KEY
          valueFrom:
            secretKeyRef:
              name: synth-secrets
              key: api-key
        - name: CACHE_TTL
          value: "3600"
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "1Gi"
            cpu: "1000m"
        livenessProbe:
          httpGet:
            path: /health
            port: 3000
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: 3000
          initialDelaySeconds: 5
          periodSeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: agentic-synth-service
spec:
  selector:
    app: agentic-synth
  ports:
  - protocol: TCP
    port: 80
    targetPort: 3000
  type: LoadBalancer
---
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: agentic-synth-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: agentic-synth
  minReplicas: 3
  maxReplicas: 10
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
```

---

## 7. Security Considerations

### 7.1 API Key Management

```typescript
// ✅ Good: Environment variables
const synth = new AgenticSynth({
  provider: 'gemini',
  apiKey: process.env.SYNTH_API_KEY!
});

// ❌ Bad: Hardcoded
const synth = new AgenticSynth({
  provider: 'gemini',
  apiKey: 'AIza...' // NEVER DO THIS
});
```

### 7.2 Input Validation

```typescript
const MAX_GENERATION_COUNT = 10000;
const MAX_SCHEMA_DEPTH = 5;

function validateOptions(options: any) {
  if (options.count > MAX_GENERATION_COUNT) {
    throw new Error(`Count exceeds maximum (${MAX_GENERATION_COUNT})`);
  }

  if (getSchemaDepth(options.schema) > MAX_SCHEMA_DEPTH) {
    throw new Error(`Schema depth exceeds maximum (${MAX_SCHEMA_DEPTH})`);
  }
}
```

### 7.3 HTTPS Only

```typescript
// Redirect HTTP to HTTPS
app.use((req, res, next) => {
  if (req.header('x-forwarded-proto') !== 'https' && process.env.NODE_ENV === 'production') {
    res.redirect(`https://${req.header('host')}${req.url}`);
  } else {
    next();
  }
});
```

---

## 8. Troubleshooting

### Common Issues

**Issue: High Memory Usage**
- Solution: Reduce `maxCacheSize` or enable streaming for large datasets

**Issue: Slow Response Times**
- Solution: Enable caching, use faster model, increase `cacheTTL`

**Issue: Rate Limiting (429)**
- Solution: Implement exponential backoff, add rate limiter

**Issue: API Connection Failures**
- Solution: Verify API key, check network connectivity, implement retry logic

---

**Last Updated**: 2025-11-22
**Package Version**: 0.1.0
**Status**: Production Ready ✅
