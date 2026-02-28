/**
 * Report Generation
 * Generates comprehensive JSON reports of simulation results
 */
import { writeFileSync } from 'fs';
export class ReportGenerator {
    network;
    metrics;
    startTime;
    constructor(network, metrics) {
        this.network = network;
        this.metrics = metrics;
        this.startTime = Date.now();
    }
    /**
     * Generate comprehensive simulation report
     */
    generateReport() {
        const endTime = Date.now();
        const stats = this.network.getStats();
        const allMetrics = this.metrics.getAllMetrics();
        const overallSuccess = this.metrics.getOverallSuccess();
        // Organize metrics by phase
        const phaseMetrics = {};
        allMetrics.forEach(m => {
            phaseMetrics[m.phase] = m;
        });
        // Get top performing nodes
        const topPerformers = this.getTopPerformers(10);
        // Collect validation issues
        const validation = this.collectValidation(allMetrics);
        const report = {
            metadata: {
                timestamp: new Date().toISOString(),
                simulationVersion: '1.0.0',
                duration: endTime - this.startTime,
                totalTicks: this.network.currentTick,
            },
            configuration: {
                genesisNodeCount: this.network.config.genesisNodeCount,
                targetNodeCount: this.network.config.targetNodeCount,
                nodesPerTick: this.network.config.nodesPerTick,
                taskGenerationRate: this.network.config.taskGenerationRate,
                baseTaskReward: this.network.config.baseTaskReward,
            },
            summary: {
                phasesCompleted: allMetrics.length,
                totalPassed: overallSuccess.passed,
                phasesPassed: overallSuccess.totalPassed,
                phasesTotal: overallSuccess.totalPhases,
                finalNodeCount: stats.nodeCount,
                finalPhase: this.network.currentPhase,
            },
            phases: phaseMetrics,
            finalState: {
                nodeCount: stats.nodeCount,
                genesisNodes: stats.genesisNodes,
                economy: stats.economy,
                network: stats.network,
                topPerformers,
            },
            validation,
        };
        return report;
    }
    /**
     * Get top performing nodes
     */
    getTopPerformers(count) {
        const cells = Array.from(this.network.cells.values());
        return cells
            .sort((a, b) => {
            const scoreA = a.metrics.energyEarned - a.metrics.energySpent;
            const scoreB = b.metrics.energyEarned - b.metrics.energySpent;
            return scoreB - scoreA;
        })
            .slice(0, count)
            .map(cell => ({
            id: cell.id.substring(0, 8),
            type: cell.type,
            netEnergy: cell.metrics.energyEarned - cell.metrics.energySpent,
            tasksCompleted: cell.metrics.tasksCompleted,
            successRate: (cell.metrics.successRate * 100).toFixed(1) + '%',
            connections: cell.connectedCells.size,
            fitnessScore: cell.getFitnessScore().toFixed(3),
        }));
    }
    /**
     * Collect all validation issues
     */
    collectValidation(allMetrics) {
        const criticalIssues = [];
        const warnings = [];
        const successes = [];
        allMetrics.forEach(metrics => {
            if (!metrics.validation.passed) {
                criticalIssues.push(`${metrics.phase.toUpperCase()} phase failed validation`);
            }
            metrics.validation.reasons.forEach(reason => {
                if (reason.startsWith('✓')) {
                    successes.push(`${metrics.phase}: ${reason}`);
                }
                else if (reason.includes('too low') || reason.includes('insufficient')) {
                    warnings.push(`${metrics.phase}: ${reason}`);
                }
                else {
                    criticalIssues.push(`${metrics.phase}: ${reason}`);
                }
            });
        });
        return {
            overallPassed: criticalIssues.length === 0,
            criticalIssues,
            warnings,
            successes,
        };
    }
    /**
     * Save report to file
     */
    saveReport(filepath) {
        const report = this.generateReport();
        writeFileSync(filepath, JSON.stringify(report, null, 2), 'utf-8');
        console.log(`\n📄 Report saved to: ${filepath}`);
    }
    /**
     * Print summary to console
     */
    printSummary() {
        const report = this.generateReport();
        console.log('\n╔════════════════════════════════════════════════════════════╗');
        console.log('║         EDGE-NET LIFECYCLE SIMULATION REPORT              ║');
        console.log('╚════════════════════════════════════════════════════════════╝\n');
        console.log('📊 SUMMARY:');
        console.log(`   Duration: ${(report.metadata.duration / 1000).toFixed(2)}s`);
        console.log(`   Total Ticks: ${report.metadata.totalTicks.toLocaleString()}`);
        console.log(`   Final Nodes: ${report.summary.finalNodeCount.toLocaleString()}`);
        console.log(`   Final Phase: ${report.summary.finalPhase.toUpperCase()}`);
        console.log(`   Phases Passed: ${report.summary.phasesPassed}/${report.summary.phasesTotal}`);
        console.log(`   Overall Result: ${report.summary.totalPassed ? '✅ PASSED' : '❌ FAILED'}\n`);
        console.log('📈 PHASE RESULTS:');
        Object.entries(report.phases).forEach(([phase, metrics]) => {
            const icon = metrics.validation.passed ? '✅' : '❌';
            console.log(`   ${icon} ${phase.toUpperCase()}:`);
            console.log(`      Nodes: ${metrics.nodeCount.start.toLocaleString()} → ${metrics.nodeCount.end.toLocaleString()}`);
            console.log(`      Energy: ${metrics.energy.netEnergy.toFixed(2)} rUv (${metrics.energy.sustainability.toFixed(2)}x sustainable)`);
            console.log(`      Tasks: ${metrics.network.tasksCompleted.toLocaleString()} completed`);
            console.log(`      Success Rate: ${(metrics.network.avgSuccessRate * 100).toFixed(1)}%`);
        });
        console.log('\n🏆 TOP PERFORMERS:');
        report.finalState.topPerformers.slice(0, 5).forEach((node, i) => {
            console.log(`   ${i + 1}. ${node.id} (${node.type})`);
            console.log(`      Net Energy: ${node.netEnergy.toFixed(2)} rUv | Tasks: ${node.tasksCompleted} | Success: ${node.successRate}`);
        });
        if (report.validation.criticalIssues.length > 0) {
            console.log('\n🚨 CRITICAL ISSUES:');
            report.validation.criticalIssues.forEach(issue => {
                console.log(`   ❌ ${issue}`);
            });
        }
        if (report.validation.warnings.length > 0) {
            console.log('\n⚠️  WARNINGS:');
            report.validation.warnings.slice(0, 5).forEach(warning => {
                console.log(`   ⚠️  ${warning}`);
            });
            if (report.validation.warnings.length > 5) {
                console.log(`   ... and ${report.validation.warnings.length - 5} more warnings`);
            }
        }
        console.log('\n✅ SUCCESSES:');
        report.validation.successes.slice(0, 10).forEach(success => {
            console.log(`   ${success}`);
        });
        console.log('\n╚════════════════════════════════════════════════════════════╝\n');
    }
}
//# sourceMappingURL=report.js.map