#!/usr/bin/env node
/**
 * Edge-Net CLI Integration Test
 *
 * Tests:
 * 1. Identity persistence in ~/.ruvector/identities/
 * 2. Contribution history tracking
 * 3. Network join/leave operations
 * 4. QDAG ledger storage
 */

import { spawn } from 'child_process';
import { existsSync, readFileSync } from 'fs';
import { join } from 'path';
import { homedir } from 'os';

const CLI_PATH = '/workspaces/ruvector/examples/edge-net/pkg/cli.js';
const JOIN_PATH = '/workspaces/ruvector/examples/edge-net/pkg/join.js';
const IDENTITY_DIR = join(homedir(), '.ruvector', 'identities');
const CONTRIB_DIR = join(homedir(), '.ruvector', 'contributions');
const NETWORK_DIR = join(homedir(), '.ruvector', 'networks');

console.log('🧪 Edge-Net CLI Integration Test\n');
console.log('═'.repeat(60));

const results = {
    passed: 0,
    failed: 0,
    tests: []
};

function test(name, condition, details = '') {
    const passed = condition;
    results.tests.push({ name, passed, details });

    if (passed) {
        results.passed++;
        console.log(`✅ PASS: ${name}`);
        if (details) console.log(`   ${details}`);
    } else {
        results.failed++;
        console.log(`❌ FAIL: ${name}`);
        if (details) console.log(`   ${details}`);
    }
}

async function runCommand(cmd, args = []) {
    return new Promise((resolve, reject) => {
        const proc = spawn('node', [cmd, ...args], {
            cwd: '/workspaces/ruvector/examples/edge-net/pkg'
        });

        let stdout = '';
        let stderr = '';

        proc.stdout.on('data', (data) => {
            stdout += data.toString();
        });

        proc.stderr.on('data', (data) => {
            stderr += data.toString();
        });

        proc.on('close', (code) => {
            resolve({ code, stdout, stderr });
        });

        proc.on('error', reject);

        // Timeout after 10 seconds
        setTimeout(() => {
            proc.kill();
            reject(new Error('Command timeout'));
        }, 10000);
    });
}

async function main() {
    console.log('\n📋 Test 1: CLI Info Command');
    console.log('─'.repeat(60));

    try {
        const { stdout, code } = await runCommand(CLI_PATH, ['info']);
        test('CLI info command executes', code === 0);
        test('CLI info shows package name', stdout.includes('@ruvector/edge-net'));
        test('CLI info shows WASM modules', stdout.includes('WASM MODULES'));
        test('CLI info shows capabilities', stdout.includes('Ed25519'));
    } catch (error) {
        test('CLI info command executes', false, error.message);
    }

    console.log('\n📋 Test 2: Identity Persistence');
    console.log('─'.repeat(60));

    test('Identity directory exists', existsSync(IDENTITY_DIR), IDENTITY_DIR);

    const { readdirSync } = await import('fs');
    const identityFiles = existsSync(IDENTITY_DIR) ?
        readdirSync(IDENTITY_DIR).filter(f => f.endsWith('.identity')) : [];

    test('Identity file created', identityFiles.length > 0,
        `Found ${identityFiles.length} identity file(s)`);

    if (existsSync(join(IDENTITY_DIR, 'edge-contributor.meta.json'))) {
        const metaContent = readFileSync(
            join(IDENTITY_DIR, 'edge-contributor.meta.json'),
            'utf8'
        );
        const meta = JSON.parse(metaContent);

        test('Identity metadata has version', meta.version === 1);
        test('Identity has short ID (Pi-Key)', meta.shortId?.startsWith('π:'));
        test('Identity has public key', meta.publicKey?.length === 64);
        test('Identity has genesis fingerprint', meta.genesisFingerprint?.length > 0);
        test('Identity tracks creation date', Boolean(meta.createdAt));
        test('Identity tracks last used', Boolean(meta.lastUsed));
        test('Identity tracks total sessions', typeof meta.totalSessions === 'number');
    }

    console.log('\n📋 Test 3: Contribution History Tracking');
    console.log('─'.repeat(60));

    test('Contribution directory exists', existsSync(CONTRIB_DIR), CONTRIB_DIR);

    if (existsSync(join(CONTRIB_DIR, 'edge-contributor.history.json'))) {
        const historyContent = readFileSync(
            join(CONTRIB_DIR, 'edge-contributor.history.json'),
            'utf8'
        );
        const history = JSON.parse(historyContent);

        test('History tracks site ID', Boolean(history.siteId));
        test('History tracks short ID', Boolean(history.shortId));
        test('History has sessions array', Array.isArray(history.sessions));
        test('History has contributions array', Array.isArray(history.contributions));
        test('History has milestones array', Array.isArray(history.milestones));

        if (history.sessions.length > 0) {
            const session = history.sessions[0];
            test('Session has timestamp', Boolean(session.started));
            test('Session has type', Boolean(session.type));
        }

        if (history.milestones.length > 0) {
            const milestone = history.milestones[0];
            test('Milestone has type', Boolean(milestone.type));
            test('Milestone has timestamp', Boolean(milestone.timestamp));
        }
    }

    console.log('\n📋 Test 4: Network Join/List Operations');
    console.log('─'.repeat(60));

    try {
        const { stdout: statusOut, code: statusCode } =
            await runCommand(CLI_PATH, ['join', '--status']);

        test('Join status command executes', statusCode === 0);
        test('Status shows identity', statusOut.includes('π:'));
        test('Status shows contributor status', statusOut.includes('CONTRIBUTOR STATUS'));
        test('Status shows network metrics', statusOut.includes('NETWORK METRICS'));
    } catch (error) {
        test('Join status command executes', false, error.message);
    }

    try {
        const { stdout: listOut, code: listCode } =
            await runCommand(CLI_PATH, ['join', '--list']);

        test('Join list command executes', listCode === 0);
        test('List shows identities', listOut.includes('STORED IDENTITIES'));
        test('List shows identity count', listOut.includes('Found'));
        test('List shows storage path', listOut.includes('.ruvector/identities'));
    } catch (error) {
        test('Join list command executes', false, error.message);
    }

    console.log('\n📋 Test 5: Network Discovery');
    console.log('─'.repeat(60));

    try {
        const { stdout: networksOut, code: networksCode } =
            await runCommand(JOIN_PATH, ['--networks']);

        test('Networks list command executes', networksCode === 0);
        test('Shows known networks', networksOut.includes('KNOWN NETWORKS'));
        test('Shows mainnet', networksOut.includes('mainnet'));
        test('Shows testnet', networksOut.includes('testnet'));
    } catch (error) {
        test('Networks list command executes', false, error.message);
    }

    console.log('\n📋 Test 6: QDAG and Ledger Storage');
    console.log('─'.repeat(60));

    test('Network directory exists', existsSync(NETWORK_DIR), NETWORK_DIR);

    // Test QDAG module loading
    try {
        const { QDAG } = await import('/workspaces/ruvector/examples/edge-net/pkg/qdag.js');
        const qdag = new QDAG('test-site');

        test('QDAG instantiates', Boolean(qdag));
        test('QDAG has genesis transaction', qdag.transactions.size >= 1);
        test('QDAG has site ID', qdag.siteId === 'test-site');
    } catch (error) {
        test('QDAG module loads', false, error.message);
    }

    // Test Ledger module loading
    try {
        const { Ledger } = await import('/workspaces/ruvector/examples/edge-net/pkg/ledger.js');
        const ledger = new Ledger({ nodeId: 'test-node' });

        test('Ledger instantiates', Boolean(ledger));
        test('Ledger has node ID', ledger.nodeId === 'test-node');
        test('Ledger has credit method', typeof ledger.credit === 'function');
        test('Ledger has debit method', typeof ledger.debit === 'function');
        test('Ledger has balance method', typeof ledger.balance === 'function');
        test('Ledger has save method', typeof ledger.save === 'function');

        // Test credit operation
        const tx = ledger.credit(100, 'test credit');
        test('Ledger can credit', Boolean(tx));
        test('Ledger tracks balance', ledger.balance() === 100);
        test('Ledger creates transaction', Boolean(tx.id));
    } catch (error) {
        test('Ledger module loads', false, error.message);
    }

    console.log('\n📋 Test 7: Contribution History Command');
    console.log('─'.repeat(60));

    try {
        const { stdout: historyOut, code: historyCode } =
            await runCommand(JOIN_PATH, ['--history']);

        test('History command executes', historyCode === 0);
        test('History shows contribution data', historyOut.includes('CONTRIBUTION HISTORY'));
        test('History shows milestones', historyOut.includes('Milestones'));
        test('History shows sessions', historyOut.includes('Recent Sessions'));
    } catch (error) {
        test('History command executes', false, error.message);
    }

    // Print summary
    console.log('\n' + '═'.repeat(60));
    console.log('📊 Test Summary');
    console.log('═'.repeat(60));
    console.log(`✅ Passed: ${results.passed}`);
    console.log(`❌ Failed: ${results.failed}`);
    console.log(`📈 Success Rate: ${((results.passed / (results.passed + results.failed)) * 100).toFixed(1)}%`);

    if (results.failed > 0) {
        console.log('\n❌ Failed Tests:');
        results.tests.filter(t => !t.passed).forEach(t => {
            console.log(`   • ${t.name}`);
            if (t.details) console.log(`     ${t.details}`);
        });
    }

    console.log('\n' + '═'.repeat(60));

    process.exit(results.failed > 0 ? 1 : 0);
}

main().catch(error => {
    console.error('Test suite error:', error);
    process.exit(1);
});
