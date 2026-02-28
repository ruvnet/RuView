"use strict";
/**
 * GNN Commands
 * CLI commands for Graph Neural Network operations
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.GnnCommands = void 0;
const chalk_1 = __importDefault(require("chalk"));
const ora_1 = __importDefault(require("ora"));
const cli_table3_1 = __importDefault(require("cli-table3"));
const fs_1 = require("fs");
class GnnCommands {
    static async create(client, name, options) {
        const spinner = (0, ora_1.default)(`Creating GNN layer '${name}'...`).start();
        try {
            await client.connect();
            await client.createGnnLayer(name, options.type, parseInt(options.inputDim), parseInt(options.outputDim));
            spinner.succeed(chalk_1.default.green(`GNN layer '${name}' created successfully`));
            console.log(chalk_1.default.bold.blue('\nLayer Configuration:'));
            console.log(chalk_1.default.gray('─'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Type:')} ${options.type.toUpperCase()}`);
            console.log(`  ${chalk_1.default.green('Input Dimensions:')} ${options.inputDim}`);
            console.log(`  ${chalk_1.default.green('Output Dimensions:')} ${options.outputDim}`);
            // Type-specific info
            const typeInfo = {
                gcn: 'Graph Convolutional Network - Spectral graph convolutions',
                graphsage: 'GraphSAGE - Inductive learning with neighborhood sampling',
                gat: 'Graph Attention Network - Attention-based message passing',
                gin: 'Graph Isomorphism Network - WL-test expressive power'
            };
            console.log(`\n  ${chalk_1.default.gray(typeInfo[options.type])}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to create GNN layer'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async forward(client, layer, options) {
        const spinner = (0, ora_1.default)(`Running forward pass through '${layer}'...`).start();
        try {
            await client.connect();
            // Load features and edges from files
            const featuresContent = (0, fs_1.readFileSync)(options.features, 'utf-8');
            const edgesContent = (0, fs_1.readFileSync)(options.edges, 'utf-8');
            const features = JSON.parse(featuresContent);
            const edges = JSON.parse(edgesContent);
            // Extract src and dst from edges
            const src = edges.map(e => e[0]);
            const dst = edges.map(e => e[1]);
            const outDim = features[0]?.length || 64;
            const result = await client.gnnForward(layer, features, src, dst, outDim);
            spinner.succeed(chalk_1.default.green('Forward pass completed successfully'));
            console.log(chalk_1.default.bold.blue('\nGNN Output:'));
            console.log(chalk_1.default.gray('─'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Nodes:')} ${result.length}`);
            console.log(`  ${chalk_1.default.green('Embedding Dim:')} ${result[0]?.length || 0}`);
            // Show sample embeddings
            console.log(chalk_1.default.bold.blue('\nSample Node Embeddings:'));
            const table = new cli_table3_1.default({
                head: [
                    chalk_1.default.cyan('Node'),
                    chalk_1.default.cyan('Embedding (first 8 dims)')
                ],
                colWidths: [8, 60]
            });
            for (let i = 0; i < Math.min(5, result.length); i++) {
                const emb = result[i];
                table.push([
                    `${i}`,
                    `[${emb.slice(0, 8).map((v) => v.toFixed(4)).join(', ')}${emb.length > 8 ? '...' : ''}]`
                ]);
            }
            console.log(table.toString());
            if (result.length > 5) {
                console.log(chalk_1.default.gray(`  ... and ${result.length - 5} more nodes`));
            }
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Forward pass failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async listTypes() {
        console.log(chalk_1.default.bold.blue('\nAvailable GNN Layer Types:'));
        console.log(chalk_1.default.gray('─'.repeat(50)));
        const types = [
            {
                name: 'GCN',
                desc: 'Graph Convolutional Network',
                details: 'Spectral graph convolutions using Chebyshev polynomials'
            },
            {
                name: 'GraphSAGE',
                desc: 'Sample and Aggregate',
                details: 'Inductive learning with neighborhood sampling and aggregation'
            },
            {
                name: 'GAT',
                desc: 'Graph Attention Network',
                details: 'Attention-weighted message passing between nodes'
            },
            {
                name: 'GIN',
                desc: 'Graph Isomorphism Network',
                details: 'Provably as powerful as WL-test for graph isomorphism'
            }
        ];
        for (const type of types) {
            console.log(`\n  ${chalk_1.default.yellow(type.name)} - ${type.desc}`);
            console.log(`    ${chalk_1.default.gray(type.details)}`);
        }
        console.log();
    }
}
exports.GnnCommands = GnnCommands;
exports.default = GnnCommands;
//# sourceMappingURL=gnn.js.map