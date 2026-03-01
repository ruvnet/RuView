"use strict";
/**
 * Router Wrapper - Semantic router for AI agent intent matching
 *
 * Wraps @ruvector/router for vector-based intent classification.
 * Perfect for hooks to route tasks to the right agent.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.SemanticRouter = void 0;
exports.isRouterAvailable = isRouterAvailable;
exports.createAgentRouter = createAgentRouter;
let routerModule = null;
let loadError = null;
function getRouterModule() {
    if (routerModule)
        return routerModule;
    if (loadError)
        throw loadError;
    try {
        routerModule = require('@ruvector/router');
        return routerModule;
    }
    catch (e) {
        loadError = new Error(`@ruvector/router not installed: ${e.message}\n` +
            `Install with: npm install @ruvector/router`);
        throw loadError;
    }
}
function isRouterAvailable() {
    try {
        getRouterModule();
        return true;
    }
    catch {
        return false;
    }
}
/**
 * Semantic Router for agent task routing
 */
class SemanticRouter {
    constructor(options = {}) {
        this.routes = new Map();
        const router = getRouterModule();
        this.inner = new router.SemanticRouter({
            dimensions: options.dimensions ?? 384,
            threshold: options.threshold ?? 0.7,
        });
    }
    /**
     * Add a route with example utterances
     */
    addRoute(name, utterances, metadata) {
        this.routes.set(name, { name, utterances, metadata });
        this.inner.addRoute(name, utterances, metadata ? JSON.stringify(metadata) : undefined);
    }
    /**
     * Add multiple routes at once
     */
    addRoutes(routes) {
        for (const route of routes) {
            this.addRoute(route.name, route.utterances, route.metadata);
        }
    }
    /**
     * Match input to best route
     */
    match(input) {
        const result = this.inner.match(input);
        if (!result)
            return null;
        return {
            route: result.route,
            score: result.score,
            metadata: result.metadata ? JSON.parse(result.metadata) : undefined,
        };
    }
    /**
     * Get top-k route matches
     */
    matchTopK(input, k = 3) {
        const results = this.inner.matchTopK(input, k);
        return results.map((r) => ({
            route: r.route,
            score: r.score,
            metadata: r.metadata ? JSON.parse(r.metadata) : undefined,
        }));
    }
    /**
     * Get all registered routes
     */
    getRoutes() {
        return Array.from(this.routes.values());
    }
    /**
     * Remove a route
     */
    removeRoute(name) {
        if (!this.routes.has(name))
            return false;
        this.routes.delete(name);
        return this.inner.removeRoute(name);
    }
    /**
     * Clear all routes
     */
    clear() {
        this.routes.clear();
        this.inner.clear();
    }
}
exports.SemanticRouter = SemanticRouter;
/**
 * Create a pre-configured agent router for hooks
 */
function createAgentRouter() {
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
exports.default = SemanticRouter;
//# sourceMappingURL=router-wrapper.js.map