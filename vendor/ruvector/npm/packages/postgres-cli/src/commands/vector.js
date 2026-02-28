"use strict";
/**
 * Vector Commands
 * CLI commands for vector operations
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.VectorCommands = void 0;
const chalk_1 = __importDefault(require("chalk"));
const ora_1 = __importDefault(require("ora"));
const cli_table3_1 = __importDefault(require("cli-table3"));
const fs_1 = require("fs");
class VectorCommands {
    static async distance(client, options) {
        const spinner = (0, ora_1.default)('Computing vector distance...').start();
        try {
            await client.connect();
            const a = JSON.parse(options.a);
            const b = JSON.parse(options.b);
            let distance;
            let metricName;
            switch (options.metric) {
                case 'l2':
                    distance = await client.l2DistanceArr(a, b);
                    metricName = 'L2 (Euclidean)';
                    break;
                case 'ip':
                    distance = await client.innerProductArr(a, b);
                    metricName = 'Inner Product';
                    break;
                case 'cosine':
                default:
                    distance = await client.cosineDistanceArr(a, b);
                    metricName = 'Cosine';
                    break;
            }
            spinner.succeed(chalk_1.default.green('Distance computed'));
            console.log(chalk_1.default.bold.blue('\nVector Distance:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Metric:')} ${metricName}`);
            console.log(`  ${chalk_1.default.green('Distance:')} ${distance.toFixed(6)}`);
            console.log(`  ${chalk_1.default.green('Dimension:')} ${a.length}`);
            // Additional context for cosine distance
            if (options.metric === 'cosine') {
                const similarity = 1 - distance;
                console.log(`  ${chalk_1.default.green('Similarity:')} ${similarity.toFixed(6)} (1 - distance)`);
            }
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Distance computation failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async normalize(client, options) {
        const spinner = (0, ora_1.default)('Normalizing vector...').start();
        try {
            await client.connect();
            const vector = JSON.parse(options.vector);
            const normalized = await client.vectorNormalize(vector);
            spinner.succeed(chalk_1.default.green('Vector normalized'));
            console.log(chalk_1.default.bold.blue('\nNormalized Vector:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Original Dimension:')} ${vector.length}`);
            // Compute original norm for reference
            const originalNorm = Math.sqrt(vector.reduce((sum, v) => sum + v * v, 0));
            console.log(`  ${chalk_1.default.green('Original Norm:')} ${originalNorm.toFixed(6)}`);
            // Verify normalized norm is ~1
            const normalizedNorm = Math.sqrt(normalized.reduce((sum, v) => sum + v * v, 0));
            console.log(`  ${chalk_1.default.green('Normalized Norm:')} ${normalizedNorm.toFixed(6)}`);
            // Display vector (truncated if too long)
            if (normalized.length <= 10) {
                console.log(`  ${chalk_1.default.green('Result:')} [${normalized.map((v) => v.toFixed(4)).join(', ')}]`);
            }
            else {
                const first5 = normalized.slice(0, 5).map((v) => v.toFixed(4)).join(', ');
                const last3 = normalized.slice(-3).map((v) => v.toFixed(4)).join(', ');
                console.log(`  ${chalk_1.default.green('Result:')} [${first5}, ..., ${last3}]`);
            }
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Normalization failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async create(client, name, options) {
        const spinner = (0, ora_1.default)(`Creating vector table '${name}'...`).start();
        try {
            await client.connect();
            await client.createVectorTable(name, parseInt(options.dim), options.index);
            spinner.succeed(chalk_1.default.green(`Vector table '${name}' created successfully`));
            console.log(`  ${chalk_1.default.gray('Dimensions:')} ${options.dim}`);
            console.log(`  ${chalk_1.default.gray('Index Type:')} ${options.index.toUpperCase()}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to create vector table'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async insert(client, table, options) {
        const spinner = (0, ora_1.default)(`Inserting vectors into '${table}'...`).start();
        try {
            await client.connect();
            let vectors = [];
            if (options.file) {
                const content = (0, fs_1.readFileSync)(options.file, 'utf-8');
                const data = JSON.parse(content);
                vectors = Array.isArray(data) ? data : [data];
            }
            else if (options.text) {
                // For text, we'd need an embedding model
                // For now, just show a placeholder
                console.log(chalk_1.default.yellow('Note: Text embedding requires an embedding model'));
                console.log(chalk_1.default.gray('Using placeholder embedding...'));
                vectors = [{
                        vector: Array(384).fill(0).map(() => Math.random()),
                        metadata: { text: options.text }
                    }];
            }
            let inserted = 0;
            for (const item of vectors) {
                await client.insertVector(table, item.vector, item.metadata);
                inserted++;
            }
            spinner.succeed(chalk_1.default.green(`Inserted ${inserted} vector(s) into '${table}'`));
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to insert vectors'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async search(client, table, options) {
        const spinner = (0, ora_1.default)(`Searching vectors in '${table}'...`).start();
        try {
            await client.connect();
            let queryVector;
            if (options.query) {
                queryVector = JSON.parse(options.query);
            }
            else if (options.text) {
                console.log(chalk_1.default.yellow('Note: Text embedding requires an embedding model'));
                console.log(chalk_1.default.gray('Using placeholder embedding...'));
                queryVector = Array(384).fill(0).map(() => Math.random());
            }
            else {
                throw new Error('Either --query or --text is required');
            }
            const results = await client.searchVectors(table, queryVector, parseInt(options.topK), options.metric);
            spinner.stop();
            if (results.length === 0) {
                console.log(chalk_1.default.yellow('No results found'));
                return;
            }
            const resultTable = new cli_table3_1.default({
                head: [
                    chalk_1.default.cyan('ID'),
                    chalk_1.default.cyan('Distance'),
                    chalk_1.default.cyan('Metadata')
                ],
                colWidths: [10, 15, 50]
            });
            for (const result of results) {
                resultTable.push([
                    String(result.id),
                    result.distance.toFixed(6),
                    result.metadata ? JSON.stringify(result.metadata).slice(0, 45) + '...' : '-'
                ]);
            }
            console.log(chalk_1.default.bold.blue(`\nSearch Results (${results.length} matches)`));
            console.log(resultTable.toString());
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Search failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
}
exports.VectorCommands = VectorCommands;
exports.default = VectorCommands;
//# sourceMappingURL=vector.js.map