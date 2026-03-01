/**
 * API Endpoints - Unit Tests
 *
 * Tests for HTTP API endpoints, request validation, and response formatting
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Types for API testing
interface Request {
  method: 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH';
  path: string;
  headers: Record<string, string>;
  body?: unknown;
  query?: Record<string, string>;
  params?: Record<string, string>;
}

interface Response {
  status: number;
  headers: Record<string, string>;
  body: unknown;
}

interface Context {
  request: Request;
  response: Response;
  tenantId?: string;
  userId?: string;
  set: (key: string, value: unknown) => void;
  get: (key: string) => unknown;
}

type Middleware = (ctx: Context, next: () => Promise<void>) => Promise<void>;
type Handler = (ctx: Context) => Promise<void>;

// Mock Router for testing
class MockRouter {
  private routes: Map<string, { method: string; handler: Handler; middlewares: Middleware[] }> = new Map();
  private globalMiddlewares: Middleware[] = [];

  use(middleware: Middleware): void {
    this.globalMiddlewares.push(middleware);
  }

  get(path: string, ...handlers: (Middleware | Handler)[]): void {
    this.register('GET', path, handlers);
  }

  post(path: string, ...handlers: (Middleware | Handler)[]): void {
    this.register('POST', path, handlers);
  }

  put(path: string, ...handlers: (Middleware | Handler)[]): void {
    this.register('PUT', path, handlers);
  }

  delete(path: string, ...handlers: (Middleware | Handler)[]): void {
    this.register('DELETE', path, handlers);
  }

  patch(path: string, ...handlers: (Middleware | Handler)[]): void {
    this.register('PATCH', path, handlers);
  }

  private register(method: string, path: string, handlers: (Middleware | Handler)[]): void {
    const handler = handlers.pop() as Handler;
    const middlewares = handlers as Middleware[];
    this.routes.set(`${method}:${path}`, { method, handler, middlewares });
  }

  async handle(request: Request): Promise<Response> {
    const ctx: Context = {
      request,
      response: {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
        body: null
      },
      set: function(key, value) { (this as any)[key] = value; },
      get: function(key) { return (this as any)[key]; }
    };

    // Find matching route
    const routeKey = `${request.method}:${this.matchPath(request.path)}`;
    const route = this.routes.get(routeKey);

    if (!route) {
      ctx.response.status = 404;
      ctx.response.body = { error: 'Not Found' };
      return ctx.response;
    }

    // Extract path params
    ctx.request.params = this.extractParams(route.handler.toString(), request.path);

    try {
      // Run global middlewares
      for (const middleware of this.globalMiddlewares) {
        let nextCalled = false;
        await middleware(ctx, async () => { nextCalled = true; });
        if (!nextCalled) return ctx.response;
      }

      // Run route middlewares
      for (const middleware of route.middlewares) {
        let nextCalled = false;
        await middleware(ctx, async () => { nextCalled = true; });
        if (!nextCalled) return ctx.response;
      }

      // Run handler
      await route.handler(ctx);
    } catch (error) {
      ctx.response.status = 500;
      ctx.response.body = {
        error: error instanceof Error ? error.message : 'Internal Server Error'
      };
    }

    return ctx.response;
  }

  private matchPath(path: string): string {
    for (const key of this.routes.keys()) {
      // Split only on first colon to separate method from path
      const colonIdx = key.indexOf(':');
      const routePath = key.slice(colonIdx + 1);
      if (this.pathMatches(routePath, path)) {
        return routePath;
      }
    }
    return path;
  }

  private pathMatches(pattern: string, path: string): boolean {
    const patternParts = pattern.split('/');
    const pathParts = path.split('/');

    if (patternParts.length !== pathParts.length) return false;

    return patternParts.every((part, i) =>
      part.startsWith(':') || part === pathParts[i]
    );
  }

  private extractParams(handlerStr: string, path: string): Record<string, string> {
    // Simple extraction - in real implementation would use route pattern
    const params: Record<string, string> = {};
    const pathParts = path.split('/');

    // Extract common params like IDs
    const idMatch = path.match(/\/([^/]+)$/);
    if (idMatch) {
      params.id = idMatch[1];
    }

    return params;
  }
}

// API Services Mock
class AgentService {
  async list(tenantId: string): Promise<unknown[]> {
    return [
      { id: 'agent-1', name: 'Agent 1', type: 'coder' },
      { id: 'agent-2', name: 'Agent 2', type: 'tester' }
    ];
  }

  async get(tenantId: string, agentId: string): Promise<unknown | null> {
    if (agentId === 'agent-1') {
      return { id: 'agent-1', name: 'Agent 1', type: 'coder' };
    }
    return null;
  }

  async create(tenantId: string, data: unknown): Promise<unknown> {
    return { id: 'new-agent', ...data as object };
  }

  async update(tenantId: string, agentId: string, data: unknown): Promise<unknown | null> {
    if (agentId === 'agent-1') {
      return { id: agentId, ...data as object };
    }
    return null;
  }

  async delete(tenantId: string, agentId: string): Promise<boolean> {
    return agentId === 'agent-1';
  }
}

class SessionService {
  async list(tenantId: string): Promise<unknown[]> {
    return [
      { id: 'session-1', status: 'active' },
      { id: 'session-2', status: 'completed' }
    ];
  }

  async get(tenantId: string, sessionId: string): Promise<unknown | null> {
    if (sessionId === 'session-1') {
      return { id: 'session-1', status: 'active', messages: [] };
    }
    return null;
  }

  async create(tenantId: string, data: unknown): Promise<unknown> {
    return { id: 'new-session', status: 'active', ...data as object };
  }
}

// Middlewares
const authMiddleware: Middleware = async (ctx, next) => {
  const authHeader = ctx.request.headers['authorization'];

  if (!authHeader?.startsWith('Bearer ')) {
    ctx.response.status = 401;
    ctx.response.body = { error: 'Unauthorized' };
    return;
  }

  const token = authHeader.slice(7);
  if (token === 'invalid-token') {
    ctx.response.status = 401;
    ctx.response.body = { error: 'Invalid token' };
    return;
  }

  ctx.tenantId = 'tenant-001';
  ctx.userId = 'user-001';
  await next();
};

const validateBody = (schema: Record<string, 'string' | 'number' | 'boolean' | 'object'>): Middleware => {
  return async (ctx, next) => {
    const body = ctx.request.body as Record<string, unknown>;

    if (!body || typeof body !== 'object') {
      ctx.response.status = 400;
      ctx.response.body = { error: 'Request body is required' };
      return;
    }

    for (const [key, type] of Object.entries(schema)) {
      if (!(key in body)) {
        ctx.response.status = 400;
        ctx.response.body = { error: `Missing required field: ${key}` };
        return;
      }

      if (typeof body[key] !== type) {
        ctx.response.status = 400;
        ctx.response.body = { error: `Invalid type for ${key}: expected ${type}` };
        return;
      }
    }

    await next();
  };
};

// Tests
describe('API Router', () => {
  let router: MockRouter;

  beforeEach(() => {
    router = new MockRouter();
  });

  describe('Route Registration', () => {
    it('should register GET route', async () => {
      router.get('/test', async (ctx) => {
        ctx.response.body = { message: 'ok' };
      });

      const response = await router.handle({
        method: 'GET',
        path: '/test',
        headers: {}
      });

      expect(response.status).toBe(200);
      expect(response.body).toEqual({ message: 'ok' });
    });

    it('should register POST route', async () => {
      router.post('/test', async (ctx) => {
        ctx.response.status = 201;
        ctx.response.body = { created: true };
      });

      const response = await router.handle({
        method: 'POST',
        path: '/test',
        headers: {},
        body: { data: 'test' }
      });

      expect(response.status).toBe(201);
    });

    it('should return 404 for unregistered routes', async () => {
      const response = await router.handle({
        method: 'GET',
        path: '/unknown',
        headers: {}
      });

      expect(response.status).toBe(404);
      expect(response.body).toEqual({ error: 'Not Found' });
    });
  });

  describe('Middleware', () => {
    it('should run global middleware', async () => {
      const middlewareFn = vi.fn(async (ctx, next) => {
        ctx.set('ran', true);
        await next();
      });

      router.use(middlewareFn);
      router.get('/test', async (ctx) => {
        ctx.response.body = { ran: ctx.get('ran') };
      });

      const response = await router.handle({
        method: 'GET',
        path: '/test',
        headers: {}
      });

      expect(middlewareFn).toHaveBeenCalled();
      expect(response.body).toEqual({ ran: true });
    });

    it('should run route middleware', async () => {
      const routeMiddleware: Middleware = async (ctx, next) => {
        ctx.set('route-middleware', true);
        await next();
      };

      router.get('/test', routeMiddleware, async (ctx) => {
        ctx.response.body = { hasMiddleware: ctx.get('route-middleware') };
      });

      const response = await router.handle({
        method: 'GET',
        path: '/test',
        headers: {}
      });

      expect(response.body).toEqual({ hasMiddleware: true });
    });

    it('should stop chain when middleware does not call next', async () => {
      router.use(async (ctx, next) => {
        ctx.response.status = 403;
        ctx.response.body = { error: 'Forbidden' };
        // Not calling next()
      });

      router.get('/test', async (ctx) => {
        ctx.response.body = { message: 'should not reach' };
      });

      const response = await router.handle({
        method: 'GET',
        path: '/test',
        headers: {}
      });

      expect(response.status).toBe(403);
    });
  });

  describe('Error Handling', () => {
    it('should catch handler errors', async () => {
      router.get('/error', async () => {
        throw new Error('Handler error');
      });

      const response = await router.handle({
        method: 'GET',
        path: '/error',
        headers: {}
      });

      expect(response.status).toBe(500);
      expect(response.body).toEqual({ error: 'Handler error' });
    });
  });
});

describe('Authentication Middleware', () => {
  let router: MockRouter;

  beforeEach(() => {
    router = new MockRouter();
    router.use(authMiddleware);
  });

  it('should reject requests without auth header', async () => {
    router.get('/protected', async (ctx) => {
      ctx.response.body = { data: 'secret' };
    });

    const response = await router.handle({
      method: 'GET',
      path: '/protected',
      headers: {}
    });

    expect(response.status).toBe(401);
    expect(response.body).toEqual({ error: 'Unauthorized' });
  });

  it('should reject invalid tokens', async () => {
    router.get('/protected', async (ctx) => {
      ctx.response.body = { data: 'secret' };
    });

    const response = await router.handle({
      method: 'GET',
      path: '/protected',
      headers: { 'authorization': 'Bearer invalid-token' }
    });

    expect(response.status).toBe(401);
    expect(response.body).toEqual({ error: 'Invalid token' });
  });

  it('should allow valid tokens', async () => {
    router.get('/protected', async (ctx) => {
      ctx.response.body = {
        data: 'secret',
        tenantId: ctx.tenantId,
        userId: ctx.userId
      };
    });

    const response = await router.handle({
      method: 'GET',
      path: '/protected',
      headers: { 'authorization': 'Bearer valid-token' }
    });

    expect(response.status).toBe(200);
    expect(response.body).toEqual({
      data: 'secret',
      tenantId: 'tenant-001',
      userId: 'user-001'
    });
  });
});

describe('Validation Middleware', () => {
  let router: MockRouter;

  beforeEach(() => {
    router = new MockRouter();
  });

  it('should reject missing body', async () => {
    router.post('/create', validateBody({ name: 'string' }), async (ctx) => {
      ctx.response.body = { created: true };
    });

    const response = await router.handle({
      method: 'POST',
      path: '/create',
      headers: {}
    });

    expect(response.status).toBe(400);
    expect(response.body).toEqual({ error: 'Request body is required' });
  });

  it('should reject missing required fields', async () => {
    router.post('/create', validateBody({ name: 'string', type: 'string' }), async (ctx) => {
      ctx.response.body = { created: true };
    });

    const response = await router.handle({
      method: 'POST',
      path: '/create',
      headers: {},
      body: { name: 'Test' }
    });

    expect(response.status).toBe(400);
    expect(response.body).toEqual({ error: 'Missing required field: type' });
  });

  it('should reject invalid field types', async () => {
    router.post('/create', validateBody({ count: 'number' }), async (ctx) => {
      ctx.response.body = { created: true };
    });

    const response = await router.handle({
      method: 'POST',
      path: '/create',
      headers: {},
      body: { count: 'not-a-number' }
    });

    expect(response.status).toBe(400);
    expect(response.body).toEqual({ error: 'Invalid type for count: expected number' });
  });

  it('should pass valid body', async () => {
    router.post('/create', validateBody({ name: 'string', count: 'number' }), async (ctx) => {
      ctx.response.body = { created: true };
    });

    const response = await router.handle({
      method: 'POST',
      path: '/create',
      headers: {},
      body: { name: 'Test', count: 5 }
    });

    expect(response.status).toBe(200);
    expect(response.body).toEqual({ created: true });
  });
});

describe('Agent API Endpoints', () => {
  let router: MockRouter;
  let agentService: AgentService;

  beforeEach(() => {
    router = new MockRouter();
    agentService = new AgentService();
    router.use(authMiddleware);

    // Register routes
    router.get('/agents', async (ctx) => {
      const agents = await agentService.list(ctx.tenantId!);
      ctx.response.body = { agents };
    });

    router.get('/agents/:id', async (ctx) => {
      const agent = await agentService.get(ctx.tenantId!, ctx.request.params!.id);
      if (!agent) {
        ctx.response.status = 404;
        ctx.response.body = { error: 'Agent not found' };
        return;
      }
      ctx.response.body = { agent };
    });

    router.post('/agents', validateBody({ name: 'string', type: 'string' }), async (ctx) => {
      const agent = await agentService.create(ctx.tenantId!, ctx.request.body);
      ctx.response.status = 201;
      ctx.response.body = { agent };
    });

    router.delete('/agents/:id', async (ctx) => {
      const deleted = await agentService.delete(ctx.tenantId!, ctx.request.params!.id);
      if (!deleted) {
        ctx.response.status = 404;
        ctx.response.body = { error: 'Agent not found' };
        return;
      }
      ctx.response.status = 204;
      ctx.response.body = null;
    });
  });

  it('should list agents', async () => {
    const response = await router.handle({
      method: 'GET',
      path: '/agents',
      headers: { 'authorization': 'Bearer valid-token' }
    });

    expect(response.status).toBe(200);
    expect(response.body).toHaveProperty('agents');
    expect((response.body as any).agents).toHaveLength(2);
  });

  it('should get agent by ID', async () => {
    const response = await router.handle({
      method: 'GET',
      path: '/agents/agent-1',
      headers: { 'authorization': 'Bearer valid-token' }
    });

    expect(response.status).toBe(200);
    expect((response.body as any).agent.id).toBe('agent-1');
  });

  it('should return 404 for non-existent agent', async () => {
    const response = await router.handle({
      method: 'GET',
      path: '/agents/non-existent',
      headers: { 'authorization': 'Bearer valid-token' }
    });

    expect(response.status).toBe(404);
  });

  it('should create agent', async () => {
    const response = await router.handle({
      method: 'POST',
      path: '/agents',
      headers: { 'authorization': 'Bearer valid-token' },
      body: { name: 'New Agent', type: 'coder' }
    });

    expect(response.status).toBe(201);
    expect((response.body as any).agent.name).toBe('New Agent');
  });

  it('should delete agent', async () => {
    const response = await router.handle({
      method: 'DELETE',
      path: '/agents/agent-1',
      headers: { 'authorization': 'Bearer valid-token' }
    });

    expect(response.status).toBe(204);
  });
});

describe('Session API Endpoints', () => {
  let router: MockRouter;
  let sessionService: SessionService;

  beforeEach(() => {
    router = new MockRouter();
    sessionService = new SessionService();
    router.use(authMiddleware);

    router.get('/sessions', async (ctx) => {
      const sessions = await sessionService.list(ctx.tenantId!);
      ctx.response.body = { sessions };
    });

    router.get('/sessions/:id', async (ctx) => {
      const session = await sessionService.get(ctx.tenantId!, ctx.request.params!.id);
      if (!session) {
        ctx.response.status = 404;
        ctx.response.body = { error: 'Session not found' };
        return;
      }
      ctx.response.body = { session };
    });

    router.post('/sessions', async (ctx) => {
      const session = await sessionService.create(ctx.tenantId!, ctx.request.body);
      ctx.response.status = 201;
      ctx.response.body = { session };
    });
  });

  it('should list sessions', async () => {
    const response = await router.handle({
      method: 'GET',
      path: '/sessions',
      headers: { 'authorization': 'Bearer valid-token' }
    });

    expect(response.status).toBe(200);
    expect((response.body as any).sessions).toHaveLength(2);
  });

  it('should get session by ID', async () => {
    const response = await router.handle({
      method: 'GET',
      path: '/sessions/session-1',
      headers: { 'authorization': 'Bearer valid-token' }
    });

    expect(response.status).toBe(200);
    expect((response.body as any).session.id).toBe('session-1');
  });

  it('should create session', async () => {
    const response = await router.handle({
      method: 'POST',
      path: '/sessions',
      headers: { 'authorization': 'Bearer valid-token' },
      body: { channelId: 'C12345' }
    });

    expect(response.status).toBe(201);
    expect((response.body as any).session.status).toBe('active');
  });
});
