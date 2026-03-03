#!/usr/bin/env node

/**
 * Comprehensive test runner for all test suites
 * Run with: node tests/run_all.cjs
 */

const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs').promises;

class ComprehensiveTestRunner {
    constructor() {
        this.verbose = process.argv.includes('--verbose');
        this.generateReport = process.argv.includes('--report');
        this.results = {
            timestamp: new Date().toISOString(),
            summary: {
                totalSuites: 0,
                passedSuites: 0,
                failedSuites: 0,
                totalTests: 0,
                passedTests: 0,
                failedTests: 0
            },
            suites: []
        };
    }

    async runTestSuite(name, scriptPath, description) {
        console.log(`\n🔍 Running ${name}`);
        console.log('='.repeat(50));

        const startTime = Date.now();

        return new Promise((resolve) => {
            const child = spawn('node', [scriptPath], {
                stdio: this.verbose ? 'inherit' : 'pipe',
                cwd: path.dirname(scriptPath)
            });

            let stdout = '';
            let stderr = '';

            if (!this.verbose) {
                child.stdout.on('data', (data) => {
                    stdout += data.toString();
                });

                child.stderr.on('data', (data) => {
                    stderr += data.toString();
                });
            }

            child.on('close', (code) => {
                const duration = Date.now() - startTime;
                const passed = code === 0;

                if (!this.verbose) {
                    console.log(stdout);
                    if (stderr) console.error(stderr);
                }

                console.log(`\n${passed ? '✅' : '❌'} ${name} ${passed ? 'PASSED' : 'FAILED'} (${duration}ms)`);

                const suiteResult = {
                    name,
                    description,
                    passed,
                    duration,
                    exitCode: code,
                    output: this.verbose ? null : stdout,
                    errors: this.verbose ? null : stderr
                };

                this.results.suites.push(suiteResult);
                this.results.summary.totalSuites++;

                if (passed) {
                    this.results.summary.passedSuites++;
                } else {
                    this.results.summary.failedSuites++;
                }

                // Try to extract test counts from output
                this.extractTestCounts(stdout, suiteResult);

                resolve(passed);
            });

            child.on('error', (error) => {
                console.error(`❌ Failed to run ${name}:`, error.message);
                this.results.suites.push({
                    name,
                    description,
                    passed: false,
                    duration: Date.now() - startTime,
                    error: error.message
                });
                this.results.summary.totalSuites++;
                this.results.summary.failedSuites++;
                resolve(false);
            });
        });
    }

    extractTestCounts(output, suiteResult) {
        // Try to extract test statistics from output
        const passedMatch = output.match(/✅ Passed: (\d+)/);
        const failedMatch = output.match(/❌ Failed: (\d+)/);
        const totalMatch = output.match(/📈 Total:\s+(\d+)/);

        if (passedMatch && failedMatch && totalMatch) {
            const passed = parseInt(passedMatch[1]);
            const failed = parseInt(failedMatch[1]);
            const total = parseInt(totalMatch[1]);

            suiteResult.testCounts = { passed, failed, total };

            this.results.summary.totalTests += total;
            this.results.summary.passedTests += passed;
            this.results.summary.failedTests += failed;
        }
    }

    async checkPrerequisites() {
        console.log('🔍 Checking Prerequisites');
        console.log('=========================\n');

        const checks = [
            {
                name: 'Node.js version',
                check: async () => {
                    const version = process.version;
                    const major = parseInt(version.slice(1));
                    return major >= 16;
                },
                message: 'Node.js 16+ required'
            },
            {
                name: 'NPM packages installed',
                check: async () => {
                    try {
                        await fs.access(path.join(__dirname, '../node_modules'));
                        return true;
                    } catch (error) {
                        return false;
                    }
                },
                message: 'Run "npm install" to install dependencies'
            },
            {
                name: 'Test files exist',
                check: async () => {
                    const testFiles = [
                        'unit/matrix.test.js',
                        'unit/solver.test.js',
                        'integration/cli.test.js',
                        'integration/mcp.test.js',
                        'integration/wasm.test.js',
                        'performance/benchmark.test.js'
                    ];

                    for (const file of testFiles) {
                        try {
                            await fs.access(path.join(__dirname, file));
                        } catch (error) {
                            return false;
                        }
                    }
                    return true;
                },
                message: 'Some test files are missing'
            }
        ];

        let allPassed = true;

        for (const check of checks) {
            const passed = await check.check();
            console.log(`${passed ? '✅' : '❌'} ${check.name}`);

            if (!passed) {
                console.log(`   ${check.message}`);
                allPassed = false;
            }
        }

        if (!allPassed) {
            console.log('\n⚠️  Some prerequisites failed. Tests may not run correctly.\n');
        } else {
            console.log('\n✅ All prerequisites passed.\n');
        }

        return allPassed;
    }

    async generateTestReport() {
        const reportData = {
            ...this.results,
            environment: {
                nodeVersion: process.version,
                platform: process.platform,
                arch: process.arch,
                memory: Math.round(process.memoryUsage().heapUsed / 1024 / 1024) + 'MB'
            },
            recommendations: this.generateRecommendations()
        };

        const reportPath = path.join(__dirname, '../test_report.json');
        await fs.writeFile(reportPath, JSON.stringify(reportData, null, 2));

        // Generate markdown report
        const markdownReport = this.generateMarkdownReport(reportData);
        const markdownPath = path.join(__dirname, '../TEST_REPORT.md');
        await fs.writeFile(markdownPath, markdownReport);

        console.log(`\n📁 Test report saved to: ${reportPath}`);
        console.log(`📁 Markdown report saved to: ${markdownPath}`);
    }

    generateRecommendations() {
        const recommendations = [];

        // Check overall test success rate
        const successRate = this.results.summary.passedSuites / this.results.summary.totalSuites;

        if (successRate < 0.8) {
            recommendations.push({
                type: 'critical',
                message: 'Low test success rate. Address failing tests before production.',
                action: 'Review failed test suites and fix underlying issues'
            });
        }

        // Check for WASM build
        const wasmSuite = this.results.suites.find(s => s.name.includes('WASM'));
        if (wasmSuite && !wasmSuite.passed) {
            recommendations.push({
                type: 'build',
                message: 'WASM tests failed. Build the WebAssembly module.',
                action: 'Run ./scripts/build.sh after installing Rust and wasm-pack'
            });
        }

        // Check for CLI issues
        const cliSuite = this.results.suites.find(s => s.name.includes('CLI'));
        if (cliSuite && !cliSuite.passed) {
            recommendations.push({
                type: 'integration',
                message: 'CLI integration tests failed.',
                action: 'Check CLI implementation and dependencies'
            });
        }

        // Check performance
        const perfSuite = this.results.suites.find(s => s.name.includes('Performance'));
        if (perfSuite && perfSuite.duration > 30000) {
            recommendations.push({
                type: 'performance',
                message: 'Performance tests are slow.',
                action: 'Consider optimizing algorithms or test parameters'
            });
        }

        // Production readiness
        if (successRate >= 0.9) {
            recommendations.push({
                type: 'success',
                message: 'High test success rate indicates good code quality.',
                action: 'Consider additional stress testing before production deployment'
            });
        }

        return recommendations;
    }

    generateMarkdownReport(data) {
        return `# Sublinear Time Solver - Test Report

**Generated:** ${data.timestamp}

## Summary

| Metric | Value |
|--------|-------|
| Test Suites | ${data.summary.totalSuites} |
| Passed Suites | ${data.summary.passedSuites} |
| Failed Suites | ${data.summary.failedSuites} |
| Success Rate | ${((data.summary.passedSuites / data.summary.totalSuites) * 100).toFixed(1)}% |
| Total Tests | ${data.summary.totalTests || 'N/A'} |
| Passed Tests | ${data.summary.passedTests || 'N/A'} |
| Failed Tests | ${data.summary.failedTests || 'N/A'} |

## Environment

- **Node.js:** ${data.environment.nodeVersion}
- **Platform:** ${data.environment.platform}
- **Architecture:** ${data.environment.arch}
- **Memory Usage:** ${data.environment.memory}

## Test Suite Results

${data.suites.map(suite => `
### ${suite.name}

- **Status:** ${suite.passed ? '✅ PASSED' : '❌ FAILED'}
- **Duration:** ${suite.duration}ms
- **Description:** ${suite.description}
${suite.testCounts ? `- **Tests:** ${suite.testCounts.passed}/${suite.testCounts.total} passed` : ''}
${suite.error ? `- **Error:** ${suite.error}` : ''}
`).join('')}

## Recommendations

${data.recommendations.map(rec => `
### ${rec.type.toUpperCase()}: ${rec.message}

**Action:** ${rec.action}
`).join('')}

## Production Readiness Assessment

${data.summary.passedSuites === data.summary.totalSuites
    ? '🟢 **READY** - All test suites passed. System is ready for production deployment.'
    : data.summary.passedSuites / data.summary.totalSuites >= 0.8
    ? '🟡 **NEEDS ATTENTION** - Most tests passed but some issues need addressing.'
    : '🔴 **NOT READY** - Significant test failures. Address issues before deployment.'
}

---

*Report generated by the Sublinear Time Solver Test Suite*
`;
    }

    async run() {
        console.log('🧪 Sublinear Time Solver - Comprehensive Test Suite');
        console.log('====================================================');

        // Check prerequisites
        const prereqsPassed = await this.checkPrerequisites();

        // Define test suites to run
        const testSuites = [
            {
                name: 'Unit Tests - Matrix',
                script: 'unit/matrix.test.cjs',
                description: 'Tests for Matrix class and basic operations'
            },
            {
                name: 'Unit Tests - Solver',
                script: 'unit/solver.test.cjs',
                description: 'Tests for SublinearSolver class and algorithms'
            },
            {
                name: 'Integration Tests - CLI',
                script: 'integration/cli.test.cjs',
                description: 'Tests for command-line interface functionality'
            },
            {
                name: 'Integration Tests - MCP Protocol',
                script: 'integration/mcp.test.cjs',
                description: 'Tests for Model Context Protocol compliance'
            },
            {
                name: 'Integration Tests - WASM Interface',
                script: 'integration/wasm.test.cjs',
                description: 'Tests for WebAssembly integration and performance'
            },
            {
                name: 'Performance Tests - Benchmarks',
                script: 'performance/benchmark.test.cjs',
                description: 'Algorithm validation and performance benchmarks'
            }
        ];

        const startTime = Date.now();
        let allPassed = true;

        // Run each test suite
        for (const suite of testSuites) {
            const scriptPath = path.join(__dirname, suite.script);
            const passed = await this.runTestSuite(suite.name, scriptPath, suite.description);
            if (!passed) allPassed = false;
        }

        const totalDuration = Date.now() - startTime;

        // Print final summary
        console.log('\n' + '='.repeat(60));
        console.log('📊 FINAL TEST SUMMARY');
        console.log('='.repeat(60));
        console.log(`Total Duration: ${(totalDuration / 1000).toFixed(1)}s`);
        console.log(`Test Suites: ${this.results.summary.passedSuites}/${this.results.summary.totalSuites} passed`);

        if (this.results.summary.totalTests > 0) {
            console.log(`Individual Tests: ${this.results.summary.passedTests}/${this.results.summary.totalTests} passed`);
        }

        const successRate = (this.results.summary.passedSuites / this.results.summary.totalSuites) * 100;
        console.log(`Success Rate: ${successRate.toFixed(1)}%`);

        // Production readiness
        if (allPassed) {
            console.log('\n🎉 ALL TESTS PASSED! System is ready for production.');
        } else if (successRate >= 80) {
            console.log('\n⚠️  Most tests passed, but some issues need attention.');
        } else {
            console.log('\n❌ Significant test failures. Address issues before deployment.');
        }

        // Generate report if requested
        if (this.generateReport) {
            await this.generateTestReport();
        }

        return allPassed;
    }
}

// Run the comprehensive test suite
if (require.main === module) {
    const runner = new ComprehensiveTestRunner();

    runner.run().then(success => {
        process.exit(success ? 0 : 1);
    }).catch(error => {
        console.error('Test runner failed:', error);
        process.exit(1);
    });
}

module.exports = { ComprehensiveTestRunner };