"use strict";
/**
 * Attention Commands
 * CLI commands for attention mechanism operations
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.AttentionCommands = void 0;
const chalk_1 = __importDefault(require("chalk"));
const ora_1 = __importDefault(require("ora"));
const cli_table3_1 = __importDefault(require("cli-table3"));
class AttentionCommands {
    static async compute(client, options) {
        const spinner = (0, ora_1.default)('Computing attention...').start();
        try {
            await client.connect();
            const query = JSON.parse(options.query);
            const keys = JSON.parse(options.keys);
            const values = JSON.parse(options.values);
            const result = await client.computeAttention(query, keys, values, options.type);
            spinner.succeed(chalk_1.default.green('Attention computed successfully'));
            console.log(chalk_1.default.bold.blue('\nAttention Output:'));
            console.log(chalk_1.default.gray('─'.repeat(40)));
            // Display output vector
            console.log(`${chalk_1.default.green('Output Vector:')} [${result.output.slice(0, 8).map(v => v.toFixed(4)).join(', ')}${result.output.length > 8 ? '...' : ''}]`);
            console.log(`${chalk_1.default.gray('Dimensions:')} ${result.output.length}`);
            // Display attention weights if available
            if (result.weights) {
                console.log(chalk_1.default.bold.blue('\nAttention Weights:'));
                const table = new cli_table3_1.default({
                    head: keys.map((_, i) => chalk_1.default.cyan(`K${i}`)),
                });
                for (let i = 0; i < Math.min(result.weights.length, 5); i++) {
                    table.push(result.weights[i].slice(0, keys.length).map(w => w.toFixed(4)));
                }
                console.log(table.toString());
            }
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to compute attention'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async listTypes(client) {
        const spinner = (0, ora_1.default)('Fetching attention types...').start();
        try {
            await client.connect();
            const types = await client.listAttentionTypes();
            spinner.stop();
            console.log(chalk_1.default.bold.blue('\nAvailable Attention Mechanisms:'));
            console.log(chalk_1.default.gray('─'.repeat(40)));
            // Group by category
            const categories = {
                'Core': ['scaled_dot_product_attention', 'multi_head_attention', 'flash_attention'],
                'Sparse': ['sparse_attention', 'local_attention', 'strided_attention', 'random_attention', 'longformer_attention'],
                'Memory': ['memory_attention', 'compressive_attention', 'memory_compressed_attention'],
                'Cross-Modal': ['cross_attention', 'cross_modal_attention', 'multimodal_attention'],
                'Efficient': ['linear_attention', 'performer_attention', 'reformer_attention', 'synthesizer_attention'],
                'Positional': ['relative_attention', 'rotary_attention', 'alibi_attention', 'rope_attention'],
                'Graph': ['graph_attention', 'gat_attention', 'sparse_graph_attention'],
                'Advanced': ['self_attention', 'causal_attention', 'bidirectional_attention', 'grouped_query_attention'],
            };
            for (const [category, items] of Object.entries(categories)) {
                const available = items.filter(t => types.includes(t));
                if (available.length > 0) {
                    console.log(`\n${chalk_1.default.yellow(category)}:`);
                    for (const item of available) {
                        console.log(`  ${chalk_1.default.green('✓')} ${item}`);
                    }
                }
            }
            // Show any types not in categories
            const categorized = Object.values(categories).flat();
            const uncategorized = types.filter(t => !categorized.includes(t));
            if (uncategorized.length > 0) {
                console.log(`\n${chalk_1.default.yellow('Other')}:`);
                for (const item of uncategorized) {
                    console.log(`  ${chalk_1.default.green('✓')} ${item}`);
                }
            }
            console.log(`\n${chalk_1.default.gray(`Total: ${types.length} attention mechanisms`)}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to list attention types'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
}
exports.AttentionCommands = AttentionCommands;
exports.default = AttentionCommands;
//# sourceMappingURL=attention.js.map