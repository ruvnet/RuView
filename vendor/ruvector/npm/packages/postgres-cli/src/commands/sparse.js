"use strict";
/**
 * Sparse Vector Commands
 * CLI commands for sparse vector operations including BM25, sparsification, and distance calculations
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.SparseCommands = void 0;
const chalk_1 = __importDefault(require("chalk"));
const ora_1 = __importDefault(require("ora"));
class SparseCommands {
    static async create(client, options) {
        const spinner = (0, ora_1.default)('Creating sparse vector...').start();
        try {
            await client.connect();
            const indices = JSON.parse(options.indices);
            const values = JSON.parse(options.values);
            const dim = parseInt(options.dim);
            const result = await client.createSparseVector(indices, values, dim);
            spinner.succeed(chalk_1.default.green('Sparse vector created successfully'));
            console.log(chalk_1.default.bold.blue('\nSparse Vector Details:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Indices:')} ${indices.length}`);
            console.log(`  ${chalk_1.default.green('Non-zero elements:')} ${values.length}`);
            console.log(`  ${chalk_1.default.green('Dimension:')} ${dim}`);
            console.log(`  ${chalk_1.default.green('Sparsity:')} ${((1 - values.length / dim) * 100).toFixed(2)}%`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to create sparse vector'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async distance(client, options) {
        const spinner = (0, ora_1.default)(`Computing sparse ${options.metric} distance...`).start();
        try {
            await client.connect();
            const result = await client.sparseDistance(options.a, options.b, options.metric);
            spinner.succeed(chalk_1.default.green(`Sparse ${options.metric} distance computed`));
            console.log(chalk_1.default.bold.blue('\nDistance Result:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Metric:')} ${options.metric}`);
            console.log(`  ${chalk_1.default.green('Distance:')} ${result.toFixed(6)}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Distance computation failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async bm25(client, options) {
        const spinner = (0, ora_1.default)('Computing BM25 score...').start();
        try {
            await client.connect();
            const k1 = options.k1 ? parseFloat(options.k1) : 1.2;
            const b = options.b ? parseFloat(options.b) : 0.75;
            const score = await client.sparseBM25(options.query, options.doc, parseFloat(options.docLen), parseFloat(options.avgDocLen), k1, b);
            spinner.succeed(chalk_1.default.green('BM25 score computed'));
            console.log(chalk_1.default.bold.blue('\nBM25 Result:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Score:')} ${score.toFixed(6)}`);
            console.log(`  ${chalk_1.default.green('k1:')} ${k1}`);
            console.log(`  ${chalk_1.default.green('b:')} ${b}`);
            console.log(`  ${chalk_1.default.green('Document Length:')} ${options.docLen}`);
            console.log(`  ${chalk_1.default.green('Avg Doc Length:')} ${options.avgDocLen}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('BM25 computation failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async topK(client, options) {
        const spinner = (0, ora_1.default)('Computing top-k sparse elements...').start();
        try {
            await client.connect();
            const result = await client.sparseTopK(options.sparse, parseInt(options.k));
            spinner.succeed(chalk_1.default.green('Top-k elements computed'));
            console.log(chalk_1.default.bold.blue('\nTop-K Result:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Original NNZ:')} ${result.originalNnz}`);
            console.log(`  ${chalk_1.default.green('After Top-K:')} ${result.newNnz}`);
            console.log(`  ${chalk_1.default.green('Sparse Vector:')} ${result.vector}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Top-k computation failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async prune(client, options) {
        const spinner = (0, ora_1.default)('Pruning sparse vector...').start();
        try {
            await client.connect();
            const result = await client.sparsePrune(options.sparse, parseFloat(options.threshold));
            spinner.succeed(chalk_1.default.green('Sparse vector pruned'));
            console.log(chalk_1.default.bold.blue('\nPrune Result:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Threshold:')} ${options.threshold}`);
            console.log(`  ${chalk_1.default.green('Original NNZ:')} ${result.originalNnz ?? 'N/A'}`);
            console.log(`  ${chalk_1.default.green('After Pruning:')} ${result.newNnz ?? 'N/A'}`);
            console.log(`  ${chalk_1.default.green('Elements Removed:')} ${(result.originalNnz ?? 0) - (result.newNnz ?? 0)}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Pruning failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async denseToSparse(client, options) {
        const spinner = (0, ora_1.default)('Converting dense to sparse...').start();
        try {
            await client.connect();
            const dense = JSON.parse(options.dense);
            const result = await client.denseToSparse(dense);
            spinner.succeed(chalk_1.default.green('Conversion completed'));
            console.log(chalk_1.default.bold.blue('\nConversion Result:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Dense Dimension:')} ${dense.length}`);
            console.log(`  ${chalk_1.default.green('Non-zero Elements:')} ${result.nnz}`);
            console.log(`  ${chalk_1.default.green('Sparsity:')} ${((1 - result.nnz / dense.length) * 100).toFixed(2)}%`);
            console.log(`  ${chalk_1.default.green('Sparse Vector:')} ${result.vector}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Conversion failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async sparseToDense(client, sparse) {
        const spinner = (0, ora_1.default)('Converting sparse to dense...').start();
        try {
            await client.connect();
            const result = await client.sparseToDense(sparse);
            spinner.succeed(chalk_1.default.green('Conversion completed'));
            console.log(chalk_1.default.bold.blue('\nConversion Result:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Dense Dimension:')} ${result.length}`);
            console.log(`  ${chalk_1.default.green('Non-zero Elements:')} ${result.filter((v) => v !== 0).length}`);
            // Show first 10 elements
            const preview = result.slice(0, 10).map((v) => v.toFixed(4)).join(', ');
            console.log(`  ${chalk_1.default.green('Preview:')} [${preview}${result.length > 10 ? ', ...' : ''}]`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Conversion failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async info(client, sparse) {
        const spinner = (0, ora_1.default)('Getting sparse vector info...').start();
        try {
            await client.connect();
            const info = await client.sparseInfo(sparse);
            spinner.stop();
            console.log(chalk_1.default.bold.blue('\nSparse Vector Info:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Dimension:')} ${info.dim}`);
            console.log(`  ${chalk_1.default.green('Non-zero Elements (NNZ):')} ${info.nnz}`);
            console.log(`  ${chalk_1.default.green('Sparsity:')} ${info.sparsity.toFixed(2)}%`);
            console.log(`  ${chalk_1.default.green('L2 Norm:')} ${info.norm.toFixed(6)}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to get info'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static showHelp() {
        console.log(chalk_1.default.bold.blue('\nSparse Vector Operations:'));
        console.log(chalk_1.default.gray('-'.repeat(60)));
        console.log(`
${chalk_1.default.yellow('Format:')}
  Sparse vectors use the format: '{index:value, index:value, ...}'
  Example: '{0:0.5, 10:0.3, 100:0.8}'

${chalk_1.default.yellow('Distance Metrics:')}
  ${chalk_1.default.green('dot')}       - Dot product (inner product)
  ${chalk_1.default.green('cosine')}    - Cosine similarity
  ${chalk_1.default.green('euclidean')} - L2 distance
  ${chalk_1.default.green('manhattan')} - L1 distance

${chalk_1.default.yellow('BM25 Scoring:')}
  Used for text search relevance ranking.
  Parameters:
    ${chalk_1.default.green('k1')} - Term frequency saturation (default: 1.2)
    ${chalk_1.default.green('b')}  - Length normalization (default: 0.75)

${chalk_1.default.yellow('Commands:')}
  ${chalk_1.default.green('sparse create')}         - Create sparse vector from indices/values
  ${chalk_1.default.green('sparse distance')}       - Compute distance between sparse vectors
  ${chalk_1.default.green('sparse bm25')}           - Compute BM25 relevance score
  ${chalk_1.default.green('sparse top-k')}          - Keep only top-k elements by value
  ${chalk_1.default.green('sparse prune')}          - Remove elements below threshold
  ${chalk_1.default.green('sparse dense-to-sparse')} - Convert dense to sparse
  ${chalk_1.default.green('sparse sparse-to-dense')} - Convert sparse to dense
  ${chalk_1.default.green('sparse info')}           - Get sparse vector statistics
`);
    }
}
exports.SparseCommands = SparseCommands;
exports.default = SparseCommands;
//# sourceMappingURL=sparse.js.map