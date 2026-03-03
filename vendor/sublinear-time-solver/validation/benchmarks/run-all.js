import chalk from 'chalk';
import { runBenchmarks as runPsycho } from './psycho-symbolic-bench.js';
import { runTraditionalBenchmarks } from './traditional-bench.js';
import { PerformanceVerifier } from './verify-claims.js';

async function runAllBenchmarks() {
    console.log(chalk.cyan.bold('\n╔══════════════════════════════════════════════════════╗'));
    console.log(chalk.cyan.bold('║   PSYCHO-SYMBOLIC REASONER PERFORMANCE VALIDATION   ║'));
    console.log(chalk.cyan.bold('╚══════════════════════════════════════════════════════╝\n'));

    console.log(chalk.yellow('This validation suite provides verifiable proof of performance claims.\n'));

    try {
        console.log(chalk.blue.bold('Step 1: Benchmarking Psycho-Symbolic Reasoner\n'));
        const psychoResults = await runPsycho();

        console.log(chalk.blue.bold('\nStep 2: Simulating Traditional Systems Performance\n'));
        const traditionalResults = await runTraditionalBenchmarks();

        console.log(chalk.blue.bold('\nStep 3: Verifying Performance Claims\n'));
        const verifier = new PerformanceVerifier();
        const verificationReport = await verifier.generateVerificationReport();

        console.log(chalk.green.bold('\n✓ All benchmarks completed successfully!'));
        console.log(chalk.gray('\nResults saved in validation/results/ directory'));

    } catch (error) {
        console.error(chalk.red('\n✗ Benchmark failed:'), error);
        process.exit(1);
    }
}

if (import.meta.url === `file://${process.argv[1]}`) {
    runAllBenchmarks();
}

export { runAllBenchmarks };