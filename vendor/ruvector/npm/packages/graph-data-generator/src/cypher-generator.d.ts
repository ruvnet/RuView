/**
 * Cypher statement generator for Neo4j
 */
import { GraphData, CypherStatement, CypherBatch } from './types.js';
export declare class CypherGenerator {
    /**
     * Generate Cypher statements from graph data
     */
    generate(data: GraphData): CypherBatch;
    /**
     * Generate CREATE statement for a node
     */
    private generateNodeStatement;
    /**
     * Generate CREATE statement for an edge
     */
    private generateEdgeStatement;
    /**
     * Generate MERGE statements (upsert)
     */
    generateMergeStatements(data: GraphData): CypherBatch;
    /**
     * Generate MERGE statement for a node
     */
    private generateNodeMergeStatement;
    /**
     * Generate MERGE statement for an edge
     */
    private generateEdgeMergeStatement;
    /**
     * Generate index creation statements
     */
    generateIndexStatements(data: GraphData): CypherStatement[];
    /**
     * Generate constraint creation statements
     */
    generateConstraintStatements(data: GraphData): CypherStatement[];
    /**
     * Generate complete setup script
     */
    generateSetupScript(data: GraphData, options?: {
        useConstraints?: boolean;
        useIndexes?: boolean;
        useMerge?: boolean;
    }): string;
    /**
     * Format a statement for output
     */
    private formatStatement;
    /**
     * Escape label names for Cypher
     */
    private escapeLabel;
    /**
     * Generate batch insert with transactions
     */
    generateBatchInsert(data: GraphData, batchSize?: number): CypherStatement[];
}
/**
 * Create a Cypher generator
 */
export declare function createCypherGenerator(): CypherGenerator;
//# sourceMappingURL=cypher-generator.d.ts.map