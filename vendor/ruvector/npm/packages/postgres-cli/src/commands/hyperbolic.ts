/**
 * Hyperbolic Geometry Commands
 * CLI commands for hyperbolic embedding operations (Poincare ball, Lorentz model)
 *
 * NOTE: These functions require the hyperbolic geometry module to be enabled
 * in the RuVector PostgreSQL extension. Currently in development.
 */

import chalk from 'chalk';
import ora from 'ora';
import Table from 'cli-table3';
import type { RuVectorClient } from '../client.js';

const HYPERBOLIC_REQUIRES_EXTENSION_MSG = `
${chalk.yellow('Hyperbolic geometry requires the RuVector PostgreSQL extension.')}

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

${chalk.gray('See: https://github.com/ruvnet/ruvector for setup instructions.')}
`;

function checkHyperbolicAvailable(): boolean {
  // Hyperbolic geometry functions are now implemented in the PostgreSQL extension
  // The functions are available in ruvector--0.1.0.sql
  return true;
}

export interface PoincareDistanceOptions {
  a: string;
  b: string;
  curvature?: string;
}

export interface LorentzDistanceOptions {
  a: string;
  b: string;
  curvature?: string;
}

export interface MobiusAddOptions {
  a: string;
  b: string;
  curvature?: string;
}

export interface ExpMapOptions {
  base: string;
  tangent: string;
  curvature?: string;
}

export interface LogMapOptions {
  base: string;
  target: string;
  curvature?: string;
}

export interface ConvertOptions {
  vector: string;
  curvature?: string;
}

export class HyperbolicCommands {
  static async poincareDistance(
    client: RuVectorClient,
    options: PoincareDistanceOptions
  ): Promise<void> {
    if (!checkHyperbolicAvailable()) {
      console.log(HYPERBOLIC_REQUIRES_EXTENSION_MSG);
      return;
    }

    const spinner = ora('Computing Poincare distance...').start();

    try {
      await client.connect();

      const a = JSON.parse(options.a);
      const b = JSON.parse(options.b);
      const curvature = options.curvature ? parseFloat(options.curvature) : -1.0;

      const distance = await client.poincareDistance(a, b, curvature);

      spinner.succeed(chalk.green('Poincare distance computed'));

      console.log(chalk.bold.blue('\nPoincare Distance:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Distance:')} ${distance.toFixed(6)}`);
      console.log(`  ${chalk.green('Curvature:')} ${curvature}`);
      console.log(`  ${chalk.green('Dimension:')} ${a.length}`);
    } catch (err) {
      spinner.fail(chalk.red('Distance computation failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async lorentzDistance(
    client: RuVectorClient,
    options: LorentzDistanceOptions
  ): Promise<void> {
    if (!checkHyperbolicAvailable()) {
      console.log(HYPERBOLIC_REQUIRES_EXTENSION_MSG);
      return;
    }

    const spinner = ora('Computing Lorentz distance...').start();

    try {
      await client.connect();

      const a = JSON.parse(options.a);
      const b = JSON.parse(options.b);
      const curvature = options.curvature ? parseFloat(options.curvature) : -1.0;

      const distance = await client.lorentzDistance(a, b, curvature);

      spinner.succeed(chalk.green('Lorentz distance computed'));

      console.log(chalk.bold.blue('\nLorentz Distance:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Distance:')} ${distance.toFixed(6)}`);
      console.log(`  ${chalk.green('Curvature:')} ${curvature}`);
      console.log(`  ${chalk.green('Dimension:')} ${a.length}`);
    } catch (err) {
      spinner.fail(chalk.red('Distance computation failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async mobiusAdd(
    client: RuVectorClient,
    options: MobiusAddOptions
  ): Promise<void> {
    if (!checkHyperbolicAvailable()) {
      console.log(HYPERBOLIC_REQUIRES_EXTENSION_MSG);
      return;
    }

    const spinner = ora('Computing Mobius addition...').start();

    try {
      await client.connect();

      const a = JSON.parse(options.a);
      const b = JSON.parse(options.b);
      const curvature = options.curvature ? parseFloat(options.curvature) : -1.0;

      const result = await client.mobiusAdd(a, b, curvature);

      spinner.succeed(chalk.green('Mobius addition computed'));

      console.log(chalk.bold.blue('\nMobius Addition Result:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Curvature:')} ${curvature}`);
      console.log(`  ${chalk.green('Result:')} [${result.map((v: number) => v.toFixed(4)).join(', ')}]`);

      // Verify result is in ball
      const norm = Math.sqrt(result.reduce((sum: number, v: number) => sum + v * v, 0));
      console.log(`  ${chalk.green('Result Norm:')} ${norm.toFixed(6)} ${norm < 1 ? chalk.green('(valid)') : chalk.red('(invalid)')}`);
    } catch (err) {
      spinner.fail(chalk.red('Mobius addition failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async expMap(
    client: RuVectorClient,
    options: ExpMapOptions
  ): Promise<void> {
    if (!checkHyperbolicAvailable()) {
      console.log(HYPERBOLIC_REQUIRES_EXTENSION_MSG);
      return;
    }

    const spinner = ora('Computing exponential map...').start();

    try {
      await client.connect();

      const base = JSON.parse(options.base);
      const tangent = JSON.parse(options.tangent);
      const curvature = options.curvature ? parseFloat(options.curvature) : -1.0;

      const result = await client.expMap(base, tangent, curvature);

      spinner.succeed(chalk.green('Exponential map computed'));

      console.log(chalk.bold.blue('\nExponential Map Result:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Base Point:')} [${base.map((v: number) => v.toFixed(4)).join(', ')}]`);
      console.log(`  ${chalk.green('Tangent Vector:')} [${tangent.map((v: number) => v.toFixed(4)).join(', ')}]`);
      console.log(`  ${chalk.green('Result (on manifold):')} [${result.map((v: number) => v.toFixed(4)).join(', ')}]`);
    } catch (err) {
      spinner.fail(chalk.red('Exponential map failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async logMap(
    client: RuVectorClient,
    options: LogMapOptions
  ): Promise<void> {
    if (!checkHyperbolicAvailable()) {
      console.log(HYPERBOLIC_REQUIRES_EXTENSION_MSG);
      return;
    }

    const spinner = ora('Computing logarithmic map...').start();

    try {
      await client.connect();

      const base = JSON.parse(options.base);
      const target = JSON.parse(options.target);
      const curvature = options.curvature ? parseFloat(options.curvature) : -1.0;

      const result = await client.logMap(base, target, curvature);

      spinner.succeed(chalk.green('Logarithmic map computed'));

      console.log(chalk.bold.blue('\nLogarithmic Map Result:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Base Point:')} [${base.map((v: number) => v.toFixed(4)).join(', ')}]`);
      console.log(`  ${chalk.green('Target Point:')} [${target.map((v: number) => v.toFixed(4)).join(', ')}]`);
      console.log(`  ${chalk.green('Tangent (at base):')} [${result.map((v: number) => v.toFixed(4)).join(', ')}]`);
    } catch (err) {
      spinner.fail(chalk.red('Logarithmic map failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async poincareToLorentz(
    client: RuVectorClient,
    options: ConvertOptions
  ): Promise<void> {
    if (!checkHyperbolicAvailable()) {
      console.log(HYPERBOLIC_REQUIRES_EXTENSION_MSG);
      return;
    }

    const spinner = ora('Converting Poincare to Lorentz...').start();

    try {
      await client.connect();

      const poincare = JSON.parse(options.vector);
      const curvature = options.curvature ? parseFloat(options.curvature) : -1.0;

      const lorentz = await client.poincareToLorentz(poincare, curvature);

      spinner.succeed(chalk.green('Conversion completed'));

      console.log(chalk.bold.blue('\nCoordinate Conversion:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Poincare (ball):')} [${poincare.map((v: number) => v.toFixed(4)).join(', ')}]`);
      console.log(`  ${chalk.green('Lorentz (hyperboloid):')} [${lorentz.map((v: number) => v.toFixed(4)).join(', ')}]`);
      console.log(`  ${chalk.green('Dimension change:')} ${poincare.length} -> ${lorentz.length}`);
    } catch (err) {
      spinner.fail(chalk.red('Conversion failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async lorentzToPoincare(
    client: RuVectorClient,
    options: ConvertOptions
  ): Promise<void> {
    if (!checkHyperbolicAvailable()) {
      console.log(HYPERBOLIC_REQUIRES_EXTENSION_MSG);
      return;
    }

    const spinner = ora('Converting Lorentz to Poincare...').start();

    try {
      await client.connect();

      const lorentz = JSON.parse(options.vector);
      const curvature = options.curvature ? parseFloat(options.curvature) : -1.0;

      const poincare = await client.lorentzToPoincare(lorentz, curvature);

      spinner.succeed(chalk.green('Conversion completed'));

      console.log(chalk.bold.blue('\nCoordinate Conversion:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Lorentz (hyperboloid):')} [${lorentz.map((v: number) => v.toFixed(4)).join(', ')}]`);
      console.log(`  ${chalk.green('Poincare (ball):')} [${poincare.map((v: number) => v.toFixed(4)).join(', ')}]`);
      console.log(`  ${chalk.green('Dimension change:')} ${lorentz.length} -> ${poincare.length}`);
    } catch (err) {
      spinner.fail(chalk.red('Conversion failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static async minkowskiDot(
    client: RuVectorClient,
    a: string,
    b: string
  ): Promise<void> {
    if (!checkHyperbolicAvailable()) {
      console.log(HYPERBOLIC_REQUIRES_EXTENSION_MSG);
      return;
    }

    const spinner = ora('Computing Minkowski inner product...').start();

    try {
      await client.connect();

      const vecA = JSON.parse(a);
      const vecB = JSON.parse(b);

      const result = await client.minkowskiDot(vecA, vecB);

      spinner.succeed(chalk.green('Minkowski inner product computed'));

      console.log(chalk.bold.blue('\nMinkowski Inner Product:'));
      console.log(chalk.gray('-'.repeat(40)));
      console.log(`  ${chalk.green('Result:')} ${result.toFixed(6)}`);
      console.log(`  ${chalk.gray('Note:')} Uses signature (-,+,+,...,+)`);
    } catch (err) {
      spinner.fail(chalk.red('Computation failed'));
      console.error(chalk.red((err as Error).message));
    } finally {
      await client.disconnect();
    }
  }

  static showHelp(): void {
    console.log(chalk.bold.blue('\nHyperbolic Geometry Operations:'));
    console.log(chalk.gray('-'.repeat(60)));

    console.log(`
${chalk.yellow('Overview:')}
  Hyperbolic space is ideal for embedding hierarchical data like
  taxonomies, organizational charts, and knowledge graphs.

${chalk.yellow('Models:')}
  ${chalk.green('Poincare Ball')} - Unit ball model, good for visualization
  ${chalk.green('Lorentz/Hyperboloid')} - Numerically stable, good for training

${chalk.yellow('Curvature:')}
  Default curvature is -1.0. More negative = more "curved" space.
  Must always be negative for hyperbolic geometry.

${chalk.yellow('Commands:')}
  ${chalk.green('hyperbolic poincare-distance')} - Distance in Poincare ball
  ${chalk.green('hyperbolic lorentz-distance')}  - Distance on hyperboloid
  ${chalk.green('hyperbolic mobius-add')}        - Hyperbolic addition
  ${chalk.green('hyperbolic exp-map')}           - Tangent to manifold
  ${chalk.green('hyperbolic log-map')}           - Manifold to tangent
  ${chalk.green('hyperbolic poincare-to-lorentz')} - Convert coordinates
  ${chalk.green('hyperbolic lorentz-to-poincare')} - Convert coordinates
  ${chalk.green('hyperbolic minkowski-dot')}     - Minkowski inner product

${chalk.yellow('Use Cases:')}
  - Hierarchical clustering
  - Knowledge graph embeddings
  - Taxonomy representation
  - Social network analysis
`);
  }
}

export default HyperbolicCommands;
