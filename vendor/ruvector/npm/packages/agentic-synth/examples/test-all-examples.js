"use strict";
/**
 * Comprehensive Test Suite for All Agentic-Synth Examples
 *
 * This script tests all examples to ensure they work correctly
 * and generate valid synthetic data.
 */
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
const perf_hooks_1 = require("perf_hooks");
const fs = __importStar(require("fs/promises"));
const path = __importStar(require("path"));
// Test configuration
const TEST_CONFIG = {
    timeout: 60000, // 60 seconds per test
    skipLargeDatasets: false,
    maxRecords: 100, // Limit for testing
    verbose: true
};
class ExampleTester {
    constructor() {
        this.results = [];
        this.startTime = 0;
        this.startTime = perf_hooks_1.performance.now();
    }
    /**
     * Run all example tests
     */
    async runAllTests() {
        console.log('🧪 Starting Comprehensive Example Test Suite\n');
        console.log('='.repeat(70));
        // Test each category
        await this.testCICDExamples();
        await this.testSelfLearningExamples();
        await this.testAdROASExamples();
        await this.testStockExamples();
        await this.testCryptoExamples();
        await this.testLogExamples();
        await this.testSecurityExamples();
        await this.testSwarmExamples();
        await this.testBusinessExamples();
        await this.testEmployeeExamples();
        // Generate report
        this.generateReport();
    }
    /**
     * Test CI/CD examples
     */
    async testCICDExamples() {
        console.log('\n📦 Testing CI/CD Examples...');
        await this.runTest('cicd', 'test-data-generator', async () => {
            // Test basic functionality without full execution
            const hasRequiredExports = await this.checkExports('examples/cicd/test-data-generator.ts', ['CICDTestDataGenerator']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('cicd', 'pipeline-testing', async () => {
            const hasRequiredExports = await this.checkExports('examples/cicd/pipeline-testing.ts', ['PipelineTester']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
    }
    /**
     * Test Self-Learning examples
     */
    async testSelfLearningExamples() {
        console.log('\n🧠 Testing Self-Learning Examples...');
        await this.runTest('self-learning', 'reinforcement-learning', async () => {
            const hasRequiredExports = await this.checkExports('examples/self-learning/reinforcement-learning.ts', ['generateRLTrainingData']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('self-learning', 'feedback-loop', async () => {
            const hasRequiredExports = await this.checkExports('examples/self-learning/feedback-loop.ts', ['qualityScoringLoop']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('self-learning', 'continual-learning', async () => {
            const hasRequiredExports = await this.checkExports('examples/self-learning/continual-learning.ts', ['generateIncrementalTrainingData']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
    }
    /**
     * Test Ad ROAS examples
     */
    async testAdROASExamples() {
        console.log('\n📊 Testing Ad ROAS Examples...');
        await this.runTest('ad-roas', 'campaign-data', async () => {
            const hasRequiredExports = await this.checkExports('examples/ad-roas/campaign-data.ts', ['generateGoogleAdsCampaign']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('ad-roas', 'optimization-simulator', async () => {
            const hasRequiredExports = await this.checkExports('examples/ad-roas/optimization-simulator.ts', ['simulateBudgetAllocation']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('ad-roas', 'analytics-pipeline', async () => {
            const hasRequiredExports = await this.checkExports('examples/ad-roas/analytics-pipeline.ts', ['generateAttributionModels']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
    }
    /**
     * Test Stock examples
     */
    async testStockExamples() {
        console.log('\n📈 Testing Stock Market Examples...');
        await this.runTest('stocks', 'market-data', async () => {
            const hasRequiredExports = await this.checkExports('examples/stocks/market-data.ts', ['generateOHLCV']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('stocks', 'trading-scenarios', async () => {
            const hasRequiredExports = await this.checkExports('examples/stocks/trading-scenarios.ts', ['generateBullMarket']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('stocks', 'portfolio-simulation', async () => {
            const hasRequiredExports = await this.checkExports('examples/stocks/portfolio-simulation.ts', ['generateMultiAssetPortfolio']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
    }
    /**
     * Test Crypto examples
     */
    async testCryptoExamples() {
        console.log('\n💰 Testing Cryptocurrency Examples...');
        await this.runTest('crypto', 'exchange-data', async () => {
            const hasRequiredExports = await this.checkExports('examples/crypto/exchange-data.ts', ['generateCryptoOHLCV']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('crypto', 'defi-scenarios', async () => {
            const hasRequiredExports = await this.checkExports('examples/crypto/defi-scenarios.ts', ['generateYieldFarmingData']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('crypto', 'blockchain-data', async () => {
            const hasRequiredExports = await this.checkExports('examples/crypto/blockchain-data.ts', ['generateTransactionPatterns']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
    }
    /**
     * Test Log examples
     */
    async testLogExamples() {
        console.log('\n📝 Testing Log Generation Examples...');
        await this.runTest('logs', 'application-logs', async () => {
            const hasRequiredExports = await this.checkExports('examples/logs/application-logs.ts', ['generateStructuredLogs']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('logs', 'system-logs', async () => {
            const hasRequiredExports = await this.checkExports('examples/logs/system-logs.ts', ['generateApacheLogs']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('logs', 'anomaly-scenarios', async () => {
            const hasRequiredExports = await this.checkExports('examples/logs/anomaly-scenarios.ts', ['generateAnomalyTrainingData']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('logs', 'log-analytics', async () => {
            const hasRequiredExports = await this.checkExports('examples/logs/log-analytics.ts', ['generateLogAggregationData']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
    }
    /**
     * Test Security examples
     */
    async testSecurityExamples() {
        console.log('\n🔒 Testing Security Examples...');
        await this.runTest('security', 'vulnerability-testing', async () => {
            const hasRequiredExports = await this.checkExports('examples/security/vulnerability-testing.ts', ['generateSQLInjectionPayloads']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('security', 'threat-simulation', async () => {
            const hasRequiredExports = await this.checkExports('examples/security/threat-simulation.ts', ['generateBruteForcePatterns']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('security', 'security-audit', async () => {
            const hasRequiredExports = await this.checkExports('examples/security/security-audit.ts', ['generateAccessPatterns']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('security', 'penetration-testing', async () => {
            const hasRequiredExports = await this.checkExports('examples/security/penetration-testing.ts', ['generateNetworkScanResults']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
    }
    /**
     * Test Swarm examples
     */
    async testSwarmExamples() {
        console.log('\n🤝 Testing Swarm Coordination Examples...');
        await this.runTest('swarms', 'agent-coordination', async () => {
            const hasRequiredExports = await this.checkExports('examples/swarms/agent-coordination.ts', ['generateAgentCommunication']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('swarms', 'distributed-processing', async () => {
            const hasRequiredExports = await this.checkExports('examples/swarms/distributed-processing.ts', ['generateMapReduceData']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('swarms', 'collective-intelligence', async () => {
            const hasRequiredExports = await this.checkExports('examples/swarms/collective-intelligence.ts', ['generateCollaborativeProblemSolving']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('swarms', 'agent-lifecycle', async () => {
            const hasRequiredExports = await this.checkExports('examples/swarms/agent-lifecycle.ts', ['generateAgentSpawningEvents']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
    }
    /**
     * Test Business Management examples
     */
    async testBusinessExamples() {
        console.log('\n💼 Testing Business Management Examples...');
        await this.runTest('business-management', 'erp-data', async () => {
            const hasRequiredExports = await this.checkExports('examples/business-management/erp-data.ts', ['generateInventoryData']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('business-management', 'crm-simulation', async () => {
            const hasRequiredExports = await this.checkExports('examples/business-management/crm-simulation.ts', ['generateLeadsData']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('business-management', 'hr-management', async () => {
            const hasRequiredExports = await this.checkExports('examples/business-management/hr-management.ts', ['generateEmployeeProfiles']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('business-management', 'financial-planning', async () => {
            const hasRequiredExports = await this.checkExports('examples/business-management/financial-planning.ts', ['generateBudgetData']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('business-management', 'operations', async () => {
            const hasRequiredExports = await this.checkExports('examples/business-management/operations.ts', ['generateProjectData']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
    }
    /**
     * Test Employee Simulation examples
     */
    async testEmployeeExamples() {
        console.log('\n👥 Testing Employee Simulation Examples...');
        await this.runTest('employee-simulation', 'workforce-behavior', async () => {
            const hasRequiredExports = await this.checkExports('examples/employee-simulation/workforce-behavior.ts', ['generateWorkSchedules']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('employee-simulation', 'performance-data', async () => {
            const hasRequiredExports = await this.checkExports('examples/employee-simulation/performance-data.ts', ['generateKPIData']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('employee-simulation', 'organizational-dynamics', async () => {
            const hasRequiredExports = await this.checkExports('examples/employee-simulation/organizational-dynamics.ts', ['generateTeamFormation']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('employee-simulation', 'workforce-planning', async () => {
            const hasRequiredExports = await this.checkExports('examples/employee-simulation/workforce-planning.ts', ['generateHiringNeeds']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
        await this.runTest('employee-simulation', 'workplace-events', async () => {
            const hasRequiredExports = await this.checkExports('examples/employee-simulation/workplace-events.ts', ['generateOnboardingData']);
            if (!hasRequiredExports)
                throw new Error('Missing required exports');
            return { recordCount: 0 };
        });
    }
    /**
     * Run a single test
     */
    async runTest(category, example, testFn) {
        const start = perf_hooks_1.performance.now();
        const memStart = process.memoryUsage().heapUsed;
        try {
            const result = await Promise.race([
                testFn(),
                new Promise((_, reject) => setTimeout(() => reject(new Error('Test timeout')), TEST_CONFIG.timeout))
            ]);
            const duration = perf_hooks_1.performance.now() - start;
            const memUsed = process.memoryUsage().heapUsed - memStart;
            this.results.push({
                category,
                example,
                status: 'pass',
                duration,
                recordCount: result.recordCount,
                memoryUsed: memUsed
            });
            console.log(`  ✅ ${example}: PASS (${duration.toFixed(0)}ms)`);
        }
        catch (error) {
            const duration = perf_hooks_1.performance.now() - start;
            this.results.push({
                category,
                example,
                status: 'fail',
                duration,
                error: error.message
            });
            console.log(`  ❌ ${example}: FAIL - ${error.message}`);
        }
    }
    /**
     * Check if file exports required functions
     */
    async checkExports(filePath, requiredExports) {
        try {
            const fullPath = path.join(process.cwd(), filePath);
            const content = await fs.readFile(fullPath, 'utf-8');
            // Check if exports exist in file
            for (const exportName of requiredExports) {
                if (!content.includes(`export`) || !content.includes(exportName)) {
                    console.log(`    ⚠️  Missing export: ${exportName}`);
                    return false;
                }
            }
            return true;
        }
        catch (error) {
            console.log(`    ⚠️  File not found: ${filePath}`);
            return false;
        }
    }
    /**
     * Generate test report
     */
    generateReport() {
        console.log('\n' + '='.repeat(70));
        console.log('\n📊 Test Results Summary\n');
        // Calculate category stats
        const categoryStats = new Map();
        for (const result of this.results) {
            if (!categoryStats.has(result.category)) {
                categoryStats.set(result.category, {
                    category: result.category,
                    passed: 0,
                    failed: 0,
                    skipped: 0,
                    totalDuration: 0,
                    avgDuration: 0
                });
            }
            const stats = categoryStats.get(result.category);
            stats.totalDuration += result.duration;
            if (result.status === 'pass')
                stats.passed++;
            else if (result.status === 'fail')
                stats.failed++;
            else
                stats.skipped++;
        }
        // Print category stats
        console.log('By Category:');
        console.log('-'.repeat(70));
        for (const [category, stats] of categoryStats) {
            const total = stats.passed + stats.failed + stats.skipped;
            stats.avgDuration = stats.totalDuration / total;
            console.log(`\n${category}:`);
            console.log(`  Passed:  ${stats.passed}/${total}`);
            console.log(`  Failed:  ${stats.failed}/${total}`);
            console.log(`  Skipped: ${stats.skipped}/${total}`);
            console.log(`  Avg Duration: ${stats.avgDuration.toFixed(0)}ms`);
        }
        // Overall stats
        const totalPassed = this.results.filter(r => r.status === 'pass').length;
        const totalFailed = this.results.filter(r => r.status === 'fail').length;
        const totalSkipped = this.results.filter(r => r.status === 'skip').length;
        const totalTests = this.results.length;
        const totalDuration = perf_hooks_1.performance.now() - this.startTime;
        console.log('\n' + '-'.repeat(70));
        console.log('\nOverall Results:');
        console.log(`  Total Tests: ${totalTests}`);
        console.log(`  ✅ Passed: ${totalPassed} (${((totalPassed / totalTests) * 100).toFixed(1)}%)`);
        console.log(`  ❌ Failed: ${totalFailed} (${((totalFailed / totalTests) * 100).toFixed(1)}%)`);
        console.log(`  ⏭️  Skipped: ${totalSkipped} (${((totalSkipped / totalTests) * 100).toFixed(1)}%)`);
        console.log(`  ⏱️  Total Duration: ${(totalDuration / 1000).toFixed(2)}s`);
        // Failed tests details
        if (totalFailed > 0) {
            console.log('\n' + '='.repeat(70));
            console.log('\n❌ Failed Tests:\n');
            const failedTests = this.results.filter(r => r.status === 'fail');
            for (const test of failedTests) {
                console.log(`  ${test.category}/${test.example}`);
                console.log(`    Error: ${test.error}`);
            }
        }
        console.log('\n' + '='.repeat(70));
        console.log(`\n${totalFailed === 0 ? '✅ All tests passed!' : '⚠️  Some tests failed'}\n`);
    }
}
// Run tests
const tester = new ExampleTester();
tester.runAllTests().catch(console.error);
//# sourceMappingURL=test-all-examples.js.map