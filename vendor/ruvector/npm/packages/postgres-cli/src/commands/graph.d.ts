/**
 * Graph Commands
 * CLI commands for graph operations and Cypher queries
 */
import type { RuVectorClient } from '../client.js';
export interface CreateNodeOptions {
    labels: string;
    properties: string;
}
export interface TraverseOptions {
    start: string;
    depth: string;
    type: 'bfs' | 'dfs';
}
export declare class GraphCommands {
    static query(client: RuVectorClient, cypher: string): Promise<void>;
    static createNode(client: RuVectorClient, options: CreateNodeOptions): Promise<void>;
    static traverse(client: RuVectorClient, options: TraverseOptions): Promise<void>;
    static showSyntax(): void;
}
export default GraphCommands;
//# sourceMappingURL=graph.d.ts.map