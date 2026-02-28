"use strict";
/**
 * Graph Commands
 * CLI commands for graph operations and Cypher queries
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.GraphCommands = void 0;
const chalk_1 = __importDefault(require("chalk"));
const ora_1 = __importDefault(require("ora"));
const cli_table3_1 = __importDefault(require("cli-table3"));
class GraphCommands {
    static async query(client, cypher) {
        const spinner = (0, ora_1.default)('Executing Cypher query...').start();
        try {
            await client.connect();
            const results = await client.cypherQuery('default', cypher);
            spinner.stop();
            if (!results || results.length === 0) {
                console.log(chalk_1.default.yellow('Query executed successfully, no results returned'));
                return;
            }
            console.log(chalk_1.default.bold.blue(`\nQuery Results (${results.length} rows):`));
            console.log(chalk_1.default.gray('─'.repeat(60)));
            // Auto-detect columns from first result
            const firstRow = results[0];
            const columns = Object.keys(firstRow);
            const table = new cli_table3_1.default({
                head: columns.map(c => chalk_1.default.cyan(c)),
                colWidths: columns.map(() => Math.floor(60 / columns.length))
            });
            for (const row of results.slice(0, 20)) {
                const r = row;
                table.push(columns.map(c => {
                    const val = r[c];
                    if (typeof val === 'object') {
                        return JSON.stringify(val).slice(0, 20) + '...';
                    }
                    return String(val).slice(0, 20);
                }));
            }
            console.log(table.toString());
            if (results.length > 20) {
                console.log(chalk_1.default.gray(`... and ${results.length - 20} more rows`));
            }
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Query failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async createNode(client, options) {
        const spinner = (0, ora_1.default)('Creating graph node...').start();
        try {
            await client.connect();
            const labels = options.labels.split(',').map(l => l.trim());
            const properties = JSON.parse(options.properties);
            const nodeId = await client.addNode('default', labels, properties);
            spinner.succeed(chalk_1.default.green('Node created successfully'));
            console.log(chalk_1.default.bold.blue('\nNode Details:'));
            console.log(chalk_1.default.gray('─'.repeat(40)));
            console.log(`  ${chalk_1.default.green('ID:')} ${nodeId}`);
            console.log(`  ${chalk_1.default.green('Labels:')} ${labels.join(', ')}`);
            console.log(`  ${chalk_1.default.green('Properties:')}`);
            for (const [key, value] of Object.entries(properties)) {
                console.log(`    ${chalk_1.default.gray(key + ':')} ${JSON.stringify(value)}`);
            }
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to create node'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async traverse(client, options) {
        const spinner = (0, ora_1.default)(`Traversing graph from node ${options.start}...`).start();
        try {
            await client.connect();
            // Use Cypher query to find neighbors
            const cypherQuery = `MATCH (n)-[*1..${options.depth}]-(m) WHERE id(n) = ${options.start} RETURN m`;
            const results = await client.cypherQuery('default', cypherQuery);
            spinner.succeed(chalk_1.default.green('Traversal completed'));
            console.log(chalk_1.default.bold.blue('\nTraversal Results:'));
            console.log(chalk_1.default.gray('─'.repeat(50)));
            console.log(`  ${chalk_1.default.green('Algorithm:')} ${options.type.toUpperCase()}`);
            console.log(`  ${chalk_1.default.green('Max Depth:')} ${options.depth}`);
            console.log(`  ${chalk_1.default.green('Nodes Found:')} ${results.length}`);
            // Show nodes found
            if (results.length > 0) {
                console.log(chalk_1.default.bold.blue('\nFound Nodes:'));
                const nodeTable = new cli_table3_1.default({
                    head: [chalk_1.default.cyan('Node')],
                    colWidths: [60]
                });
                for (const row of results.slice(0, 10)) {
                    nodeTable.push([
                        JSON.stringify(row).slice(0, 55) + '...'
                    ]);
                }
                console.log(nodeTable.toString());
                if (results.length > 10) {
                    console.log(chalk_1.default.gray(`... and ${results.length - 10} more nodes`));
                }
            }
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Traversal failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static showSyntax() {
        console.log(chalk_1.default.bold.blue('\nCypher Query Syntax:'));
        console.log(chalk_1.default.gray('─'.repeat(60)));
        const examples = [
            { query: 'MATCH (n) RETURN n LIMIT 10', desc: 'Return first 10 nodes' },
            { query: 'MATCH (n:Person) RETURN n', desc: 'Find all Person nodes' },
            { query: 'MATCH (a)-[r]->(b) RETURN a,r,b', desc: 'Find relationships' },
            { query: "MATCH (n {name: 'Alice'}) RETURN n", desc: 'Find by property' },
            { query: 'MATCH p=(a)-[*1..3]->(b) RETURN p', desc: 'Variable-length path' },
            { query: "CREATE (n:Person {name: 'Bob'}) RETURN n", desc: 'Create a node' },
        ];
        for (const ex of examples) {
            console.log(`\n  ${chalk_1.default.yellow(ex.desc)}`);
            console.log(`  ${chalk_1.default.green('>')} ${ex.query}`);
        }
        console.log();
    }
}
exports.GraphCommands = GraphCommands;
exports.default = GraphCommands;
//# sourceMappingURL=graph.js.map