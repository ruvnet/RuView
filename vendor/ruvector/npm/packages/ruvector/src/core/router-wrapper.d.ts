/**
 * Router Wrapper - Semantic router for AI agent intent matching
 *
 * Wraps @ruvector/router for vector-based intent classification.
 * Perfect for hooks to route tasks to the right agent.
 */
export declare function isRouterAvailable(): boolean;
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
export declare class SemanticRouter {
    private inner;
    private routes;
    constructor(options?: {
        dimensions?: number;
        threshold?: number;
    });
    /**
     * Add a route with example utterances
     */
    addRoute(name: string, utterances: string[], metadata?: Record<string, any>): void;
    /**
     * Add multiple routes at once
     */
    addRoutes(routes: Route[]): void;
    /**
     * Match input to best route
     */
    match(input: string): RouteMatch | null;
    /**
     * Get top-k route matches
     */
    matchTopK(input: string, k?: number): RouteMatch[];
    /**
     * Get all registered routes
     */
    getRoutes(): Route[];
    /**
     * Remove a route
     */
    removeRoute(name: string): boolean;
    /**
     * Clear all routes
     */
    clear(): void;
}
/**
 * Create a pre-configured agent router for hooks
 */
export declare function createAgentRouter(): SemanticRouter;
export default SemanticRouter;
//# sourceMappingURL=router-wrapper.d.ts.map