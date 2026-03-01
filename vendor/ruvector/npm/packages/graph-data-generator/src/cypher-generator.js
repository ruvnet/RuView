"use strict";
/**
 * Cypher statement generator for Neo4j
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.CypherGenerator = void 0;
exports.createCypherGenerator = createCypherGenerator;
class CypherGenerator {
    /**
     * Generate Cypher statements from graph data
     */
    generate(data) {
        const statements = [];
        // Generate node creation statements
        for (const node of data.nodes) {
            statements.push(this.generateNodeStatement(node));
        }
        // Generate relationship creation statements
        for (const edge of data.edges) {
            statements.push(this.generateEdgeStatement(edge));
        }
        // Collect metadata
        const labels = new Set();
        const relationshipTypes = new Set();
        data.nodes.forEach(node => node.labels.forEach(label => labels.add(label)));
        data.edges.forEach(edge => relationshipTypes.add(edge.type));
        return {
            statements,
            metadata: {
                total_nodes: data.nodes.length,
                total_relationships: data.edges.length,
                labels: Array.from(labels),
                relationship_types: Array.from(relationshipTypes)
            }
        };
    }
    /**
     * Generate CREATE statement for a node
     */
    generateNodeStatement(node) {
        const labels = node.labels.map(l => `:${this.escapeLabel(l)}`).join('');
        const propsVar = 'props';
        return {
            query: `CREATE (n${labels} $${propsVar})`,
            parameters: {
                [propsVar]: {
                    id: node.id,
                    ...node.properties,
                    ...(node.embedding ? { embedding: node.embedding } : {})
                }
            }
        };
    }
    /**
     * Generate CREATE statement for an edge
     */
    generateEdgeStatement(edge) {
        const type = this.escapeLabel(edge.type);
        const propsVar = 'props';
        return {
            query: `
        MATCH (source { id: $sourceId })
        MATCH (target { id: $targetId })
        CREATE (source)-[r:${type} $${propsVar}]->(target)
      `.trim(),
            parameters: {
                sourceId: edge.source,
                targetId: edge.target,
                [propsVar]: {
                    id: edge.id,
                    ...edge.properties,
                    ...(edge.embedding ? { embedding: edge.embedding } : {})
                }
            }
        };
    }
    /**
     * Generate MERGE statements (upsert)
     */
    generateMergeStatements(data) {
        const statements = [];
        // Generate node merge statements
        for (const node of data.nodes) {
            statements.push(this.generateNodeMergeStatement(node));
        }
        // Generate relationship merge statements
        for (const edge of data.edges) {
            statements.push(this.generateEdgeMergeStatement(edge));
        }
        const labels = new Set();
        const relationshipTypes = new Set();
        data.nodes.forEach(node => node.labels.forEach(label => labels.add(label)));
        data.edges.forEach(edge => relationshipTypes.add(edge.type));
        return {
            statements,
            metadata: {
                total_nodes: data.nodes.length,
                total_relationships: data.edges.length,
                labels: Array.from(labels),
                relationship_types: Array.from(relationshipTypes)
            }
        };
    }
    /**
     * Generate MERGE statement for a node
     */
    generateNodeMergeStatement(node) {
        const primaryLabel = node.labels[0];
        const additionalLabels = node.labels.slice(1).map(l => `:${this.escapeLabel(l)}`).join('');
        const propsVar = 'props';
        return {
            query: `
        MERGE (n:${this.escapeLabel(primaryLabel)} { id: $id })
        SET n${additionalLabels}
        SET n += $${propsVar}
      `.trim(),
            parameters: {
                id: node.id,
                [propsVar]: {
                    ...node.properties,
                    ...(node.embedding ? { embedding: node.embedding } : {})
                }
            }
        };
    }
    /**
     * Generate MERGE statement for an edge
     */
    generateEdgeMergeStatement(edge) {
        const type = this.escapeLabel(edge.type);
        const propsVar = 'props';
        return {
            query: `
        MATCH (source { id: $sourceId })
        MATCH (target { id: $targetId })
        MERGE (source)-[r:${type} { id: $id }]->(target)
        SET r += $${propsVar}
      `.trim(),
            parameters: {
                sourceId: edge.source,
                targetId: edge.target,
                id: edge.id,
                [propsVar]: {
                    ...edge.properties,
                    ...(edge.embedding ? { embedding: edge.embedding } : {})
                }
            }
        };
    }
    /**
     * Generate index creation statements
     */
    generateIndexStatements(data) {
        const statements = [];
        const labels = new Set();
        data.nodes.forEach(node => node.labels.forEach(label => labels.add(label)));
        // Create index on id for each label
        for (const label of labels) {
            statements.push({
                query: `CREATE INDEX IF NOT EXISTS FOR (n:${this.escapeLabel(label)}) ON (n.id)`
            });
        }
        // Create vector indexes if embeddings are present
        const hasEmbeddings = data.nodes.some(node => node.embedding);
        if (hasEmbeddings) {
            for (const label of labels) {
                statements.push({
                    query: `
            CREATE VECTOR INDEX IF NOT EXISTS ${this.escapeLabel(label)}_embedding
            FOR (n:${this.escapeLabel(label)})
            ON (n.embedding)
            OPTIONS {
              indexConfig: {
                \`vector.dimensions\`: ${data.nodes.find(n => n.embedding)?.embedding?.length || 1536},
                \`vector.similarity_function\`: 'cosine'
              }
            }
          `.trim()
                });
            }
        }
        return statements;
    }
    /**
     * Generate constraint creation statements
     */
    generateConstraintStatements(data) {
        const statements = [];
        const labels = new Set();
        data.nodes.forEach(node => node.labels.forEach(label => labels.add(label)));
        // Create unique constraint on id for each label
        for (const label of labels) {
            statements.push({
                query: `CREATE CONSTRAINT IF NOT EXISTS FOR (n:${this.escapeLabel(label)}) REQUIRE n.id IS UNIQUE`
            });
        }
        return statements;
    }
    /**
     * Generate complete setup script
     */
    generateSetupScript(data, options) {
        const statements = [];
        // Add constraints
        if (options?.useConstraints !== false) {
            statements.push('// Create constraints');
            this.generateConstraintStatements(data).forEach(stmt => {
                statements.push(this.formatStatement(stmt) + ';');
            });
            statements.push('');
        }
        // Add indexes
        if (options?.useIndexes !== false) {
            statements.push('// Create indexes');
            this.generateIndexStatements(data).forEach(stmt => {
                statements.push(this.formatStatement(stmt) + ';');
            });
            statements.push('');
        }
        // Add data
        statements.push('// Create data');
        const batch = options?.useMerge
            ? this.generateMergeStatements(data)
            : this.generate(data);
        batch.statements.forEach(stmt => {
            statements.push(this.formatStatement(stmt) + ';');
        });
        return statements.join('\n');
    }
    /**
     * Format a statement for output
     */
    formatStatement(stmt) {
        if (!stmt.parameters || Object.keys(stmt.parameters).length === 0) {
            return stmt.query;
        }
        let formatted = stmt.query;
        for (const [key, value] of Object.entries(stmt.parameters)) {
            const jsonValue = JSON.stringify(value);
            formatted = formatted.replace(new RegExp(`\\$${key}\\b`, 'g'), jsonValue);
        }
        return formatted;
    }
    /**
     * Escape label names for Cypher
     */
    escapeLabel(label) {
        // Remove special characters and use backticks if needed
        if (/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(label)) {
            return label;
        }
        return `\`${label.replace(/`/g, '``')}\``;
    }
    /**
     * Generate batch insert with transactions
     */
    generateBatchInsert(data, batchSize = 1000) {
        const statements = [];
        // Batch nodes
        for (let i = 0; i < data.nodes.length; i += batchSize) {
            const batch = data.nodes.slice(i, i + batchSize);
            statements.push({
                query: `
          UNWIND $nodes AS node
          CREATE (n)
          SET n = node.properties
          SET n.id = node.id
          WITH n, node.labels AS labels
          CALL apoc.create.addLabels(n, labels) YIELD node AS labeled
          RETURN count(labeled)
        `.trim(),
                parameters: {
                    nodes: batch.map(node => ({
                        id: node.id,
                        labels: node.labels,
                        properties: node.properties
                    }))
                }
            });
        }
        // Batch edges
        for (let i = 0; i < data.edges.length; i += batchSize) {
            const batch = data.edges.slice(i, i + batchSize);
            statements.push({
                query: `
          UNWIND $edges AS edge
          MATCH (source { id: edge.source })
          MATCH (target { id: edge.target })
          CALL apoc.create.relationship(source, edge.type, edge.properties, target) YIELD rel
          RETURN count(rel)
        `.trim(),
                parameters: {
                    edges: batch.map(edge => ({
                        source: edge.source,
                        target: edge.target,
                        type: edge.type,
                        properties: edge.properties
                    }))
                }
            });
        }
        return statements;
    }
}
exports.CypherGenerator = CypherGenerator;
/**
 * Create a Cypher generator
 */
function createCypherGenerator() {
    return new CypherGenerator();
}
//# sourceMappingURL=cypher-generator.js.map