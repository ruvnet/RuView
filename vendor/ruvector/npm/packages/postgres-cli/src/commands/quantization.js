"use strict";
/**
 * Quantization Commands
 * CLI commands for vector quantization operations (binary, scalar, product)
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.QuantizationCommands = void 0;
const chalk_1 = __importDefault(require("chalk"));
const ora_1 = __importDefault(require("ora"));
const cli_table3_1 = __importDefault(require("cli-table3"));
class QuantizationCommands {
    static async binaryQuantize(client, options) {
        const spinner = (0, ora_1.default)('Binary quantizing vector...').start();
        try {
            await client.connect();
            const vector = JSON.parse(options.vector);
            const result = await client.binaryQuantize(vector);
            spinner.succeed(chalk_1.default.green('Binary quantization completed'));
            console.log(chalk_1.default.bold.blue('\nBinary Quantization Result:'));
            console.log(chalk_1.default.gray('-'.repeat(50)));
            console.log(`  ${chalk_1.default.green('Original Dimension:')} ${vector.length}`);
            console.log(`  ${chalk_1.default.green('Quantized Bytes:')} ${result.length}`);
            console.log(`  ${chalk_1.default.green('Compression Ratio:')} ${(vector.length * 4 / result.length).toFixed(1)}x`);
            console.log(`  ${chalk_1.default.green('Memory Savings:')} ${((1 - result.length / (vector.length * 4)) * 100).toFixed(1)}%`);
            // Show first few bytes as hex
            const hexPreview = result.slice(0, 16).map((b) => b.toString(16).padStart(2, '0')).join(' ');
            console.log(`  ${chalk_1.default.green('Preview (hex):')} ${hexPreview}${result.length > 16 ? '...' : ''}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Binary quantization failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async scalarQuantize(client, options) {
        const spinner = (0, ora_1.default)('Scalar quantizing vector (SQ8)...').start();
        try {
            await client.connect();
            const vector = JSON.parse(options.vector);
            const result = await client.scalarQuantize(vector);
            spinner.succeed(chalk_1.default.green('Scalar quantization completed'));
            console.log(chalk_1.default.bold.blue('\nScalar Quantization (SQ8) Result:'));
            console.log(chalk_1.default.gray('-'.repeat(50)));
            console.log(`  ${chalk_1.default.green('Original Dimension:')} ${vector.length}`);
            console.log(`  ${chalk_1.default.green('Quantized Elements:')} ${result.data.length}`);
            console.log(`  ${chalk_1.default.green('Scale Factor:')} ${result.scale.toFixed(6)}`);
            console.log(`  ${chalk_1.default.green('Offset:')} ${result.offset.toFixed(6)}`);
            console.log(`  ${chalk_1.default.green('Compression Ratio:')} 4x (32-bit to 8-bit)`);
            console.log(`  ${chalk_1.default.green('Memory Savings:')} 75%`);
            // Show reconstruction formula
            console.log(chalk_1.default.bold.blue('\nReconstruction:'));
            console.log(`  ${chalk_1.default.gray('original[i] = quantized[i] * scale + offset')}`);
            // Show preview
            const preview = result.data.slice(0, 10).join(', ');
            console.log(`  ${chalk_1.default.green('Quantized Preview:')} [${preview}${result.data.length > 10 ? ', ...' : ''}]`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Scalar quantization failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async stats(client) {
        const spinner = (0, ora_1.default)('Fetching quantization statistics...').start();
        try {
            await client.connect();
            const stats = await client.quantizationStats();
            spinner.stop();
            console.log(chalk_1.default.bold.blue('\nQuantization Statistics:'));
            console.log(chalk_1.default.gray('-'.repeat(50)));
            const table = new cli_table3_1.default({
                head: [
                    chalk_1.default.cyan('Type'),
                    chalk_1.default.cyan('Bits/Dim'),
                    chalk_1.default.cyan('Compression'),
                    chalk_1.default.cyan('Accuracy Loss'),
                    chalk_1.default.cyan('Speed Boost'),
                ],
                colWidths: [15, 12, 14, 15, 14],
            });
            table.push(['Binary (BQ)', '1', '32x', '~20-30%', '~10-20x'], ['Scalar (SQ8)', '8', '4x', '~1-5%', '~2-4x'], ['Product (PQ)', 'Variable', '8-32x', '~5-15%', '~5-10x']);
            console.log(table.toString());
            console.log(chalk_1.default.bold.blue('\nMemory Usage:'));
            console.log(`  ${chalk_1.default.green('Quantization Tables:')} ${stats.quantization_tables_mb.toFixed(2)} MB`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Failed to get stats'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async compare(client, vector) {
        const spinner = (0, ora_1.default)('Comparing quantization methods...').start();
        try {
            await client.connect();
            const vec = JSON.parse(vector);
            const dim = vec.length;
            // Get all quantization results
            const binary = await client.binaryQuantize(vec);
            const scalar = await client.scalarQuantize(vec);
            spinner.stop();
            console.log(chalk_1.default.bold.blue('\nQuantization Comparison:'));
            console.log(chalk_1.default.gray('-'.repeat(60)));
            console.log(`  ${chalk_1.default.green('Original Vector:')} ${dim} dimensions, ${dim * 4} bytes`);
            const table = new cli_table3_1.default({
                head: [
                    chalk_1.default.cyan('Method'),
                    chalk_1.default.cyan('Size'),
                    chalk_1.default.cyan('Compression'),
                    chalk_1.default.cyan('Type'),
                ],
                colWidths: [18, 15, 15, 20],
            });
            table.push(['Original (f32)', `${dim * 4} bytes`, '1x', '32-bit float'], ['Binary (BQ)', `${binary.length} bytes`, `${(dim * 4 / binary.length).toFixed(1)}x`, '1-bit per dim'], ['Scalar (SQ8)', `${scalar.data.length + 8} bytes`, `${(dim * 4 / (scalar.data.length + 8)).toFixed(1)}x`, '8-bit + metadata']);
            console.log(table.toString());
            console.log(chalk_1.default.bold.blue('\nTrade-offs:'));
            console.log(`  ${chalk_1.default.yellow('Binary:')} Best compression, lowest accuracy, fastest`);
            console.log(`  ${chalk_1.default.yellow('Scalar:')} Good balance of compression and accuracy`);
            console.log(`  ${chalk_1.default.yellow('Product:')} Variable, best for specific use cases`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Comparison failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static showHelp() {
        console.log(chalk_1.default.bold.blue('\nVector Quantization:'));
        console.log(chalk_1.default.gray('-'.repeat(60)));
        console.log(`
${chalk_1.default.yellow('Overview:')}
  Quantization reduces vector storage size and speeds up search
  by representing vectors with fewer bits per dimension.

${chalk_1.default.yellow('Quantization Types:')}

  ${chalk_1.default.green('Binary Quantization (BQ)')}
  - Converts each dimension to 1 bit (sign)
  - 32x memory reduction
  - 10-20x search speedup
  - ~20-30% accuracy loss
  - Best for: Large-scale approximate search

  ${chalk_1.default.green('Scalar Quantization (SQ8)')}
  - Converts 32-bit floats to 8-bit integers
  - 4x memory reduction
  - 2-4x search speedup
  - ~1-5% accuracy loss
  - Best for: Balanced accuracy/efficiency

  ${chalk_1.default.green('Product Quantization (PQ)')}
  - Splits vector into subvectors, each quantized separately
  - 8-32x memory reduction
  - 5-10x search speedup
  - ~5-15% accuracy loss
  - Best for: Medium-scale with accuracy needs

${chalk_1.default.yellow('Commands:')}
  ${chalk_1.default.green('quantization binary')}   - Binary quantize a vector
  ${chalk_1.default.green('quantization scalar')}   - Scalar quantize (SQ8)
  ${chalk_1.default.green('quantization compare')}  - Compare all methods
  ${chalk_1.default.green('quantization stats')}    - View quantization statistics

${chalk_1.default.yellow('When to Use:')}
  - Dataset > 1M vectors: Consider BQ or PQ
  - Need < 5% accuracy loss: Use SQ8
  - Filtering important: Use BQ with re-ranking
  - Memory constrained: Use BQ or PQ
`);
    }
}
exports.QuantizationCommands = QuantizationCommands;
exports.default = QuantizationCommands;
//# sourceMappingURL=quantization.js.map