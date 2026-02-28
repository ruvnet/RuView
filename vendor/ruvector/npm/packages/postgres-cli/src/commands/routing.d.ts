/**
 * Routing/Agent Commands
 * CLI commands for Tiny Dancer agent routing and management
 */
import type { RuVectorClient } from '../client.js';
export interface RegisterAgentOptions {
    name: string;
    type: string;
    capabilities: string;
    cost: string;
    latency: string;
    quality: string;
}
export interface RegisterAgentFullOptions {
    config: string;
}
export interface UpdateMetricsOptions {
    name: string;
    latency: string;
    success: boolean;
    quality?: string;
}
export interface RouteOptions {
    embedding: string;
    optimizeFor?: string;
    constraints?: string;
}
export interface FindAgentsOptions {
    capability: string;
    limit?: string;
}
export declare class RoutingCommands {
    static registerAgent(client: RuVectorClient, options: RegisterAgentOptions): Promise<void>;
    static registerAgentFull(client: RuVectorClient, options: RegisterAgentFullOptions): Promise<void>;
    static updateMetrics(client: RuVectorClient, options: UpdateMetricsOptions): Promise<void>;
    static removeAgent(client: RuVectorClient, name: string): Promise<void>;
    static setActive(client: RuVectorClient, name: string, active: boolean): Promise<void>;
    static route(client: RuVectorClient, options: RouteOptions): Promise<void>;
    static listAgents(client: RuVectorClient): Promise<void>;
    static getAgent(client: RuVectorClient, name: string): Promise<void>;
    static findByCapability(client: RuVectorClient, options: FindAgentsOptions): Promise<void>;
    static stats(client: RuVectorClient): Promise<void>;
    static clearAgents(client: RuVectorClient): Promise<void>;
    static showHelp(): void;
}
export default RoutingCommands;
//# sourceMappingURL=routing.d.ts.map