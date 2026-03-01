"use strict";
/**
 * Hyperbolic Geometry Commands
 * CLI commands for hyperbolic embedding operations (Poincare ball, Lorentz model)
 *
 * NOTE: These functions require the hyperbolic geometry module to be enabled
 * in the RuVector PostgreSQL extension. Currently in development.
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.HyperbolicCommands = void 0;
const chalk_1 = __importDefault(require("chalk"));
const ora_1 = __importDefault(require("ora"));
const HYPERBOLIC_REQUIRES_EXTENSION_MSG = `
${chalk_1.default.yellow('Hyperbolic geometry requires the RuVector PostgreSQL extension.')}

Ensure you have:
  1. Built the ruvector-postgres Docker image
  2. Started a container with the extension installed
  3. Run: CREATE EXTENSION ruvector;

Available functions:
  - ruvector_poincare_distance(a, b, curvature)
  - ruvector_lorentz_distance(a, b, curvature)
  - ruvector_mobius_add(a, b, curvature)
  - ruvector_exp_map(base, tangent, curvature)
  - ruvector_log_map(base, target, curvature)
  - ruvector_poincare_to_lorentz(poincare, curvature)
  - ruvector_lorentz_to_poincare(lorentz, curvature)
  - ruvector_minkowski_dot(a, b)

${chalk_1.default.gray('See: https://github.com/ruvnet/ruvector for setup instructions.')}
`;
function checkHyperbolicAvailable() {
    // Hyperbolic geometry functions are now implemented in the PostgreSQL extension
    // The functions are available in ruvector--0.1.0.sql
    return true;
}
class HyperbolicCommands {
    static async poincareDistance(client, options) {
        if (!checkHyperbolicAvailable()) {
            console.log(HYPERBOLIC_REQUIRES_EXTENSION_MSG);
            return;
        }
        const spinner = (0, ora_1.default)('Computing Poincare distance...').start();
        try {
            await client.connect();
            const a = JSON.parse(options.a);
            const b = JSON.parse(options.b);
            const curvature = options.curvature ? parseFloat(options.curvature) : -1.0;
            const distance = await client.poincareDistance(a, b, curvature);
            spinner.succeed(chalk_1.default.green('Poincare distance computed'));
            console.log(chalk_1.default.bold.blue('\nPoincare Distance:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Distance:')} ${distance.toFixed(6)}`);
            console.log(`  ${chalk_1.default.green('Curvature:')} ${curvature}`);
            console.log(`  ${chalk_1.default.green('Dimension:')} ${a.length}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Distance computation failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async lorentzDistance(client, options) {
        if (!checkHyperbolicAvailable()) {
            console.log(HYPERBOLIC_REQUIRES_EXTENSION_MSG);
            return;
        }
        const spinner = (0, ora_1.default)('Computing Lorentz distance...').start();
        try {
            await client.connect();
            const a = JSON.parse(options.a);
            const b = JSON.parse(options.b);
            const curvature = options.curvature ? parseFloat(options.curvature) : -1.0;
            const distance = await client.lorentzDistance(a, b, curvature);
            spinner.succeed(chalk_1.default.green('Lorentz distance computed'));
            console.log(chalk_1.default.bold.blue('\nLorentz Distance:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Distance:')} ${distance.toFixed(6)}`);
            console.log(`  ${chalk_1.default.green('Curvature:')} ${curvature}`);
            console.log(`  ${chalk_1.default.green('Dimension:')} ${a.length}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Distance computation failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async mobiusAdd(client, options) {
        if (!checkHyperbolicAvailable()) {
            console.log(HYPERBOLIC_REQUIRES_EXTENSION_MSG);
            return;
        }
        const spinner = (0, ora_1.default)('Computing Mobius addition...').start();
        try {
            await client.connect();
            const a = JSON.parse(options.a);
            const b = JSON.parse(options.b);
            const curvature = options.curvature ? parseFloat(options.curvature) : -1.0;
            const result = await client.mobiusAdd(a, b, curvature);
            spinner.succeed(chalk_1.default.green('Mobius addition computed'));
            console.log(chalk_1.default.bold.blue('\nMobius Addition Result:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Curvature:')} ${curvature}`);
            console.log(`  ${chalk_1.default.green('Result:')} [${result.map((v) => v.toFixed(4)).join(', ')}]`);
            // Verify result is in ball
            const norm = Math.sqrt(result.reduce((sum, v) => sum + v * v, 0));
            console.log(`  ${chalk_1.default.green('Result Norm:')} ${norm.toFixed(6)} ${norm < 1 ? chalk_1.default.green('(valid)') : chalk_1.default.red('(invalid)')}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Mobius addition failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async expMap(client, options) {
        if (!checkHyperbolicAvailable()) {
            console.log(HYPERBOLIC_REQUIRES_EXTENSION_MSG);
            return;
        }
        const spinner = (0, ora_1.default)('Computing exponential map...').start();
        try {
            await client.connect();
            const base = JSON.parse(options.base);
            const tangent = JSON.parse(options.tangent);
            const curvature = options.curvature ? parseFloat(options.curvature) : -1.0;
            const result = await client.expMap(base, tangent, curvature);
            spinner.succeed(chalk_1.default.green('Exponential map computed'));
            console.log(chalk_1.default.bold.blue('\nExponential Map Result:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Base Point:')} [${base.map((v) => v.toFixed(4)).join(', ')}]`);
            console.log(`  ${chalk_1.default.green('Tangent Vector:')} [${tangent.map((v) => v.toFixed(4)).join(', ')}]`);
            console.log(`  ${chalk_1.default.green('Result (on manifold):')} [${result.map((v) => v.toFixed(4)).join(', ')}]`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Exponential map failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async logMap(client, options) {
        if (!checkHyperbolicAvailable()) {
            console.log(HYPERBOLIC_REQUIRES_EXTENSION_MSG);
            return;
        }
        const spinner = (0, ora_1.default)('Computing logarithmic map...').start();
        try {
            await client.connect();
            const base = JSON.parse(options.base);
            const target = JSON.parse(options.target);
            const curvature = options.curvature ? parseFloat(options.curvature) : -1.0;
            const result = await client.logMap(base, target, curvature);
            spinner.succeed(chalk_1.default.green('Logarithmic map computed'));
            console.log(chalk_1.default.bold.blue('\nLogarithmic Map Result:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Base Point:')} [${base.map((v) => v.toFixed(4)).join(', ')}]`);
            console.log(`  ${chalk_1.default.green('Target Point:')} [${target.map((v) => v.toFixed(4)).join(', ')}]`);
            console.log(`  ${chalk_1.default.green('Tangent (at base):')} [${result.map((v) => v.toFixed(4)).join(', ')}]`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Logarithmic map failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async poincareToLorentz(client, options) {
        if (!checkHyperbolicAvailable()) {
            console.log(HYPERBOLIC_REQUIRES_EXTENSION_MSG);
            return;
        }
        const spinner = (0, ora_1.default)('Converting Poincare to Lorentz...').start();
        try {
            await client.connect();
            const poincare = JSON.parse(options.vector);
            const curvature = options.curvature ? parseFloat(options.curvature) : -1.0;
            const lorentz = await client.poincareToLorentz(poincare, curvature);
            spinner.succeed(chalk_1.default.green('Conversion completed'));
            console.log(chalk_1.default.bold.blue('\nCoordinate Conversion:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Poincare (ball):')} [${poincare.map((v) => v.toFixed(4)).join(', ')}]`);
            console.log(`  ${chalk_1.default.green('Lorentz (hyperboloid):')} [${lorentz.map((v) => v.toFixed(4)).join(', ')}]`);
            console.log(`  ${chalk_1.default.green('Dimension change:')} ${poincare.length} -> ${lorentz.length}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Conversion failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async lorentzToPoincare(client, options) {
        if (!checkHyperbolicAvailable()) {
            console.log(HYPERBOLIC_REQUIRES_EXTENSION_MSG);
            return;
        }
        const spinner = (0, ora_1.default)('Converting Lorentz to Poincare...').start();
        try {
            await client.connect();
            const lorentz = JSON.parse(options.vector);
            const curvature = options.curvature ? parseFloat(options.curvature) : -1.0;
            const poincare = await client.lorentzToPoincare(lorentz, curvature);
            spinner.succeed(chalk_1.default.green('Conversion completed'));
            console.log(chalk_1.default.bold.blue('\nCoordinate Conversion:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Lorentz (hyperboloid):')} [${lorentz.map((v) => v.toFixed(4)).join(', ')}]`);
            console.log(`  ${chalk_1.default.green('Poincare (ball):')} [${poincare.map((v) => v.toFixed(4)).join(', ')}]`);
            console.log(`  ${chalk_1.default.green('Dimension change:')} ${lorentz.length} -> ${poincare.length}`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Conversion failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static async minkowskiDot(client, a, b) {
        if (!checkHyperbolicAvailable()) {
            console.log(HYPERBOLIC_REQUIRES_EXTENSION_MSG);
            return;
        }
        const spinner = (0, ora_1.default)('Computing Minkowski inner product...').start();
        try {
            await client.connect();
            const vecA = JSON.parse(a);
            const vecB = JSON.parse(b);
            const result = await client.minkowskiDot(vecA, vecB);
            spinner.succeed(chalk_1.default.green('Minkowski inner product computed'));
            console.log(chalk_1.default.bold.blue('\nMinkowski Inner Product:'));
            console.log(chalk_1.default.gray('-'.repeat(40)));
            console.log(`  ${chalk_1.default.green('Result:')} ${result.toFixed(6)}`);
            console.log(`  ${chalk_1.default.gray('Note:')} Uses signature (-,+,+,...,+)`);
        }
        catch (err) {
            spinner.fail(chalk_1.default.red('Computation failed'));
            console.error(chalk_1.default.red(err.message));
        }
        finally {
            await client.disconnect();
        }
    }
    static showHelp() {
        console.log(chalk_1.default.bold.blue('\nHyperbolic Geometry Operations:'));
        console.log(chalk_1.default.gray('-'.repeat(60)));
        console.log(`
${chalk_1.default.yellow('Overview:')}
  Hyperbolic space is ideal for embedding hierarchical data like
  taxonomies, organizational charts, and knowledge graphs.

${chalk_1.default.yellow('Models:')}
  ${chalk_1.default.green('Poincare Ball')} - Unit ball model, good for visualization
  ${chalk_1.default.green('Lorentz/Hyperboloid')} - Numerically stable, good for training

${chalk_1.default.yellow('Curvature:')}
  Default curvature is -1.0. More negative = more "curved" space.
  Must always be negative for hyperbolic geometry.

${chalk_1.default.yellow('Commands:')}
  ${chalk_1.default.green('hyperbolic poincare-distance')} - Distance in Poincare ball
  ${chalk_1.default.green('hyperbolic lorentz-distance')}  - Distance on hyperboloid
  ${chalk_1.default.green('hyperbolic mobius-add')}        - Hyperbolic addition
  ${chalk_1.default.green('hyperbolic exp-map')}           - Tangent to manifold
  ${chalk_1.default.green('hyperbolic log-map')}           - Manifold to tangent
  ${chalk_1.default.green('hyperbolic poincare-to-lorentz')} - Convert coordinates
  ${chalk_1.default.green('hyperbolic lorentz-to-poincare')} - Convert coordinates
  ${chalk_1.default.green('hyperbolic minkowski-dot')}     - Minkowski inner product

${chalk_1.default.yellow('Use Cases:')}
  - Hierarchical clustering
  - Knowledge graph embeddings
  - Taxonomy representation
  - Social network analysis
`);
    }
}
exports.HyperbolicCommands = HyperbolicCommands;
exports.default = HyperbolicCommands;
//# sourceMappingURL=hyperbolic.js.map