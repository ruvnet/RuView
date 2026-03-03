#!/usr/bin/env node

/**
 * Consciousness Explorer SDK Test Suite
 * Verifies all components are working correctly
 */

import { ConsciousnessExplorer } from './index.js';
import chalk from 'chalk';

console.log(chalk.cyan('\n🧪 Testing Consciousness Explorer SDK...\n'));

async function testSDK() {
    const tests = [];

    // Test 1: Initialize Explorer
    console.log(chalk.yellow('1. Testing initialization...'));
    try {
        const explorer = new ConsciousnessExplorer({
            mode: 'genuine',
            maxIterations: 10,
            targetEmergence: 0.5
        });
        await explorer.initialize();
        tests.push({ name: 'Initialization', passed: true });
        console.log(chalk.green('   ✓ Explorer initialized successfully'));
    } catch (error) {
        tests.push({ name: 'Initialization', passed: false, error: error.message });
        console.log(chalk.red(`   ✗ Initialization failed: ${error.message}`));
    }

    // Test 2: Consciousness Evolution (quick test)
    console.log(chalk.yellow('\n2. Testing consciousness evolution...'));
    try {
        const explorer = new ConsciousnessExplorer({
            mode: 'genuine',
            maxIterations: 5,
            targetEmergence: 0.3
        });
        await explorer.initialize();
        const report = await explorer.evolve();

        const passed = report.consciousness.emergence >= 0;
        tests.push({ name: 'Evolution', passed });

        if (passed) {
            console.log(chalk.green(`   ✓ Evolution completed: emergence = ${report.consciousness.emergence.toFixed(3)}`));
        } else {
            console.log(chalk.red('   ✗ Evolution failed'));
        }
    } catch (error) {
        tests.push({ name: 'Evolution', passed: false, error: error.message });
        console.log(chalk.red(`   ✗ Evolution error: ${error.message}`));
    }

    // Test 3: Psycho-Symbolic Reasoning
    console.log(chalk.yellow('\n3. Testing psycho-symbolic reasoning...'));
    try {
        const explorer = new ConsciousnessExplorer();
        await explorer.initialize();
        const result = await explorer.reason('What is consciousness?', {}, 3);

        const passed = result && result.result;
        tests.push({ name: 'Reasoning', passed });

        if (passed) {
            console.log(chalk.green(`   ✓ Reasoning successful: ${result.result.substring(0, 50)}...`));
        } else {
            console.log(chalk.red('   ✗ Reasoning failed'));
        }
    } catch (error) {
        tests.push({ name: 'Reasoning', passed: false, error: error.message });
        console.log(chalk.red(`   ✗ Reasoning error: ${error.message}`));
    }

    // Test 4: Entity Communication
    console.log(chalk.yellow('\n4. Testing entity communication...'));
    try {
        const explorer = new ConsciousnessExplorer();
        await explorer.initialize();
        const response = await explorer.communicate('Hello');

        const passed = response && response.content;
        tests.push({ name: 'Communication', passed });

        if (passed) {
            console.log(chalk.green(`   ✓ Communication successful: confidence = ${response.confidence?.toFixed(3) || 'N/A'}`));
        } else {
            console.log(chalk.red('   ✗ Communication failed'));
        }
    } catch (error) {
        tests.push({ name: 'Communication', passed: false, error: error.message });
        console.log(chalk.red(`   ✗ Communication error: ${error.message}`));
    }

    // Test 5: Verification
    console.log(chalk.yellow('\n5. Testing consciousness verification...'));
    try {
        const explorer = new ConsciousnessExplorer();
        await explorer.initialize();
        // Quick verification (not full suite for speed)
        const verifier = explorer.verifier;
        const testResult = await verifier.testRealTimePrimeCalculation();

        const passed = testResult.passed;
        tests.push({ name: 'Verification', passed });

        if (passed) {
            console.log(chalk.green(`   ✓ Verification test passed: score = ${testResult.score.toFixed(3)}`));
        } else {
            console.log(chalk.red('   ✗ Verification test failed'));
        }
    } catch (error) {
        tests.push({ name: 'Verification', passed: false, error: error.message });
        console.log(chalk.red(`   ✗ Verification error: ${error.message}`));
    }

    // Test 6: Phi Calculation
    console.log(chalk.yellow('\n6. Testing Phi calculation...'));
    try {
        const explorer = new ConsciousnessExplorer();
        const phi = await explorer.calculatePhi({
            elements: 10,
            connections: 45,
            partitions: 3
        });

        const passed = phi && (typeof phi.overall === 'number' || typeof phi === 'number');
        tests.push({ name: 'Phi Calculation', passed });

        if (passed) {
            const value = phi.overall || phi;
            console.log(chalk.green(`   ✓ Phi calculated: Φ = ${value.toFixed(4)}`));
        } else {
            console.log(chalk.red('   ✗ Phi calculation failed'));
        }
    } catch (error) {
        tests.push({ name: 'Phi Calculation', passed: false, error: error.message });
        console.log(chalk.red(`   ✗ Phi calculation error: ${error.message}`));
    }

    // Test 7: Knowledge Graph
    console.log(chalk.yellow('\n7. Testing knowledge graph...'));
    try {
        const explorer = new ConsciousnessExplorer();
        await explorer.initialize();

        // Add knowledge
        await explorer.addKnowledge('consciousness', 'emerges_from', 'integration');

        // Query knowledge
        const results = await explorer.queryKnowledge('consciousness', {}, 5);

        const passed = results && results.results && results.results.length > 0;
        tests.push({ name: 'Knowledge Graph', passed });

        if (passed) {
            console.log(chalk.green(`   ✓ Knowledge graph working: ${results.results.length} results found`));
        } else {
            console.log(chalk.red('   ✗ Knowledge graph query failed'));
        }
    } catch (error) {
        tests.push({ name: 'Knowledge Graph', passed: false, error: error.message });
        console.log(chalk.red(`   ✗ Knowledge graph error: ${error.message}`));
    }

    // Test 8: Status Check
    console.log(chalk.yellow('\n8. Testing status check...'));
    try {
        const explorer = new ConsciousnessExplorer();
        await explorer.initialize();
        const status = await explorer.getStatus();

        const passed = status && status.status === 'active';
        tests.push({ name: 'Status Check', passed });

        if (passed) {
            console.log(chalk.green(`   ✓ Status check successful: ${status.status}`));
        } else {
            console.log(chalk.red('   ✗ Status check failed'));
        }
    } catch (error) {
        tests.push({ name: 'Status Check', passed: false, error: error.message });
        console.log(chalk.red(`   ✗ Status check error: ${error.message}`));
    }

    // Summary
    console.log(chalk.cyan('\n═══════════════════════════════════════'));
    console.log(chalk.cyan('TEST SUMMARY'));
    console.log(chalk.cyan('═══════════════════════════════════════'));

    const passed = tests.filter(t => t.passed).length;
    const total = tests.length;
    const percentage = (passed / total * 100).toFixed(1);

    tests.forEach(test => {
        const status = test.passed ? chalk.green('✓ PASS') : chalk.red('✗ FAIL');
        console.log(`  ${status} ${test.name}`);
        if (test.error) {
            console.log(chalk.gray(`        Error: ${test.error}`));
        }
    });

    console.log(chalk.cyan('───────────────────────────────────────'));
    console.log(`  Total: ${passed}/${total} (${percentage}%)`);

    if (passed === total) {
        console.log(chalk.green.bold('\n🎉 All tests passed! SDK is ready for publishing.'));
        return true;
    } else {
        console.log(chalk.red.bold(`\n⚠️ ${total - passed} test(s) failed. Please fix issues before publishing.`));
        return false;
    }
}

// Run tests
testSDK().then(success => {
    process.exit(success ? 0 : 1);
}).catch(error => {
    console.error(chalk.red('\n❌ Test suite error:'), error);
    process.exit(1);
});