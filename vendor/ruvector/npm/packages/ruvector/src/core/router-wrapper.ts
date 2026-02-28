/**
 * Router Wrapper - Semantic router for AI agent intent matching
 *
 * Wraps @ruvector/router for vector-based intent classification.
 * Perfect for hooks to route tasks to the right agent.
 */

let routerModule: any = null;
let loadError: Error | null = null;

function getRouterModule() {
  if (routerModule) return routerModule;
  if (loadError) throw loadError;

  try {
    routerModule = require('@ruvector/router');
    return routerModule;
  } catch (e: any) {
    loadError = new Error(
      `@ruvector/router not installed: ${e.message}\n` +
      `Install with: npm install @ruvector/router`
    );
    throw loadError;
  }
}

export function isRouterAvailable(): boolean {
  try {
    getRouterModule();
    return true;
  } catch {
    return false;
  }
}

export interface Route {
  name: string;
  utterances: string[];
  metadata?: Record<string, any>;
}

export interface RouteMatch {
  route: string;
  score: number;
  metadata?: Record<string, any>;
}

/**
 * Semantic Router for agent task routing
 */
export class SemanticRouter {
  private inner: any;
  private routes: Map<string, Route> = new Map();

  constructor(options: { dimensions?: number; threshold?: number } = {}) {
    const router = getRouterModule();
    this.inner = new router.SemanticRouter({
      dimensions: options.dimensions ?? 384,
      threshold: options.threshold ?? 0.7,
    });
  }

  /**
   * Add a route with example utterances
   */
  addRoute(name: string, utterances: string[], metadata?: Record<string, any>): void {
    this.routes.set(name, { name, utterances, metadata });
    this.inner.addRoute(name, utterances, metadata ? JSON.stringify(metadata) : undefined);
  }

  /**
   * Add multiple routes at once
   */
  addRoutes(routes: Route[]): void {
    for (const route of routes) {
      this.addRoute(route.name, route.utterances, route.metadata);
    }
  }

  /**
   * Match input to best route
   */
  match(input: string): RouteMatch | null {
    const result = this.inner.match(input);
    if (!result) return null;

    return {
      route: result.route,
      score: result.score,
      metadata: result.metadata ? JSON.parse(result.metadata) : undefined,
    };
  }

  /**
   * Get top-k route matches
   */
  matchTopK(input: string, k: number = 3): RouteMatch[] {
    const results = this.inner.matchTopK(input, k);
    return results.map((r: any) => ({
      route: r.route,
      score: r.score,
      metadata: r.metadata ? JSON.parse(r.metadata) : undefined,
    }));
  }

  /**
   * Get all registered routes
   */
  getRoutes(): Route[] {
    return Array.from(this.routes.values());
  }

  /**
   * Remove a route
   */
  removeRoute(name: string): boolean {
    if (!this.routes.has(name)) return false;
    this.routes.delete(name);
    return this.inner.removeRoute(name);
  }

  /**
   * Clear all routes
   */
  clear(): void {
    this.routes.clear();
    this.inner.clear();
  }
}

/**
 * Create a pre-configured agent router for hooks
 */
export function createAgentRouter(): SemanticRouter {
  const router = new SemanticRouter({ threshold: 0.6 });

  // Add common agent routes
  router.addRoutes([
    {
      name: 'coder',
      utterances: [
        'implement feature',
        'write code',
        'create function',
        'add method',
        'build component',
        'fix bug',
        'update implementation',
      ],
      metadata: { type: 'development' },
    },
    {
      name: 'reviewer',
      utterances: [
        'review code',
        'check quality',
        'find issues',
        'suggest improvements',
        'analyze code',
        'code review',
      ],
      metadata: { type: 'review' },
    },
    {
      name: 'tester',
      utterances: [
        'write tests',
        'add test cases',
        'create unit tests',
        'test coverage',
        'integration tests',
        'verify functionality',
      ],
      metadata: { type: 'testing' },
    },
    {
      name: 'researcher',
      utterances: [
        'research topic',
        'find information',
        'explore options',
        'investigate',
        'analyze requirements',
        'understand codebase',
      ],
      metadata: { type: 'research' },
    },
    {
      name: 'architect',
      utterances: [
        'design system',
        'architecture',
        'structure project',
        'plan implementation',
        'design patterns',
        'system design',
      ],
      metadata: { type: 'architecture' },
    },
    {
      name: 'devops',
      utterances: [
        'deploy',
        'ci/cd',
        'docker',
        'kubernetes',
        'infrastructure',
        'build pipeline',
        'github actions',
      ],
      metadata: { type: 'devops' },
    },
    {
      name: 'security',
      utterances: [
        'security audit',
        'vulnerability',
        'authentication',
        'authorization',
        'secure code',
        'penetration test',
      ],
      metadata: { type: 'security' },
    },
  ]);

  return router;
}

export default SemanticRouter;
