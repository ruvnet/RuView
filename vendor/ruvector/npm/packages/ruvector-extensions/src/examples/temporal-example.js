"use strict";
/**
 * Temporal Tracking Module - Usage Examples
 *
 * Demonstrates various features of the temporal tracking system
 * including version management, change tracking, time-travel queries,
 * and visualization data generation.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.basicVersionManagement = basicVersionManagement;
exports.timeTravelQueries = timeTravelQueries;
exports.versionComparison = versionComparison;
exports.versionReverting = versionReverting;
exports.visualizationData = visualizationData;
exports.auditLogging = auditLogging;
exports.storageManagement = storageManagement;
exports.backupAndRestore = backupAndRestore;
exports.eventDrivenArchitecture = eventDrivenArchitecture;
exports.runAllExamples = runAllExamples;
const temporal_js_1 = require("../temporal.js");
/**
 * Example 1: Basic Version Management
 */
async function basicVersionManagement() {
    console.log('=== Example 1: Basic Version Management ===\n');
    const tracker = new temporal_js_1.TemporalTracker();
    // Create initial schema version
    tracker.trackChange({
        type: temporal_js_1.ChangeType.ADDITION,
        path: 'nodes.User',
        before: null,
        after: {
            name: 'User',
            properties: ['id', 'name', 'email']
        },
        timestamp: Date.now()
    });
    tracker.trackChange({
        type: temporal_js_1.ChangeType.ADDITION,
        path: 'edges.FOLLOWS',
        before: null,
        after: {
            name: 'FOLLOWS',
            from: 'User',
            to: 'User'
        },
        timestamp: Date.now()
    });
    const v1 = await tracker.createVersion({
        description: 'Initial schema with User nodes and FOLLOWS edges',
        tags: ['v1.0', 'production'],
        author: 'system'
    });
    console.log('Created version:', v1.id);
    console.log('Changes:', v1.changes.length);
    console.log('Tags:', v1.tags);
    console.log();
    // Add more entities
    tracker.trackChange({
        type: temporal_js_1.ChangeType.ADDITION,
        path: 'nodes.Post',
        before: null,
        after: {
            name: 'Post',
            properties: ['id', 'title', 'content', 'authorId']
        },
        timestamp: Date.now()
    });
    tracker.trackChange({
        type: temporal_js_1.ChangeType.ADDITION,
        path: 'edges.POSTED',
        before: null,
        after: {
            name: 'POSTED',
            from: 'User',
            to: 'Post'
        },
        timestamp: Date.now()
    });
    const v2 = await tracker.createVersion({
        description: 'Added Post nodes and POSTED edges',
        tags: ['v1.1'],
        author: 'developer'
    });
    console.log('Created version:', v2.id);
    console.log('Changes:', v2.changes.length);
    console.log();
    // List all versions
    const allVersions = tracker.listVersions();
    console.log('Total versions:', allVersions.length);
    allVersions.forEach(v => {
        console.log(`- ${v.description} (${v.tags.join(', ')})`);
    });
    console.log();
}
/**
 * Example 2: Time-Travel Queries
 */
async function timeTravelQueries() {
    console.log('=== Example 2: Time-Travel Queries ===\n');
    const tracker = new temporal_js_1.TemporalTracker();
    // Create multiple versions over time
    tracker.trackChange({
        type: temporal_js_1.ChangeType.ADDITION,
        path: 'config.maxUsers',
        before: null,
        after: 100,
        timestamp: Date.now()
    });
    const v1 = await tracker.createVersion({
        description: 'Set max users to 100',
        tags: ['config-v1']
    });
    console.log(`Version 1 created at ${new Date(v1.timestamp).toISOString()}`);
    // Wait a bit and make changes
    await new Promise(resolve => setTimeout(resolve, 100));
    tracker.trackChange({
        type: temporal_js_1.ChangeType.MODIFICATION,
        path: 'config.maxUsers',
        before: 100,
        after: 500,
        timestamp: Date.now()
    });
    const v2 = await tracker.createVersion({
        description: 'Increased max users to 500',
        tags: ['config-v2']
    });
    console.log(`Version 2 created at ${new Date(v2.timestamp).toISOString()}`);
    // Query at different timestamps
    const stateAtV1 = await tracker.queryAtTimestamp(v1.timestamp);
    console.log('\nState at version 1:', JSON.stringify(stateAtV1, null, 2));
    const stateAtV2 = await tracker.queryAtTimestamp(v2.timestamp);
    console.log('\nState at version 2:', JSON.stringify(stateAtV2, null, 2));
    // Query with path filter
    const configOnly = await tracker.queryAtTimestamp({
        timestamp: v2.timestamp,
        pathPattern: /^config\./
    });
    console.log('\nFiltered state (config only):', JSON.stringify(configOnly, null, 2));
    console.log();
}
/**
 * Example 3: Version Comparison and Diffing
 */
async function versionComparison() {
    console.log('=== Example 3: Version Comparison and Diffing ===\n');
    const tracker = new temporal_js_1.TemporalTracker();
    // Create initial state
    tracker.trackChange({
        type: temporal_js_1.ChangeType.ADDITION,
        path: 'schema.version',
        before: null,
        after: '1.0.0',
        timestamp: Date.now()
    });
    tracker.trackChange({
        type: temporal_js_1.ChangeType.ADDITION,
        path: 'schema.entities.User',
        before: null,
        after: { fields: ['id', 'name'] },
        timestamp: Date.now()
    });
    const v1 = await tracker.createVersion({
        description: 'Initial schema',
        tags: ['schema-v1']
    });
    // Make multiple changes
    tracker.trackChange({
        type: temporal_js_1.ChangeType.MODIFICATION,
        path: 'schema.version',
        before: '1.0.0',
        after: '2.0.0',
        timestamp: Date.now()
    });
    tracker.trackChange({
        type: temporal_js_1.ChangeType.MODIFICATION,
        path: 'schema.entities.User',
        before: { fields: ['id', 'name'] },
        after: { fields: ['id', 'name', 'email', 'createdAt'] },
        timestamp: Date.now()
    });
    tracker.trackChange({
        type: temporal_js_1.ChangeType.ADDITION,
        path: 'schema.entities.Post',
        before: null,
        after: { fields: ['id', 'title', 'content'] },
        timestamp: Date.now()
    });
    const v2 = await tracker.createVersion({
        description: 'Schema v2 with enhanced User and new Post',
        tags: ['schema-v2']
    });
    // Compare versions
    const diff = await tracker.compareVersions(v1.id, v2.id);
    console.log('Diff from v1 to v2:');
    console.log('Summary:', JSON.stringify(diff.summary, null, 2));
    console.log('\nChanges:');
    diff.changes.forEach(change => {
        console.log(`- ${change.type}: ${change.path}`);
        if (change.before !== null)
            console.log(`  Before: ${JSON.stringify(change.before)}`);
        if (change.after !== null)
            console.log(`  After: ${JSON.stringify(change.after)}`);
    });
    console.log();
}
/**
 * Example 4: Version Reverting
 */
async function versionReverting() {
    console.log('=== Example 4: Version Reverting ===\n');
    const tracker = new temporal_js_1.TemporalTracker();
    // Create progression of versions
    tracker.trackChange({
        type: temporal_js_1.ChangeType.ADDITION,
        path: 'feature.experimentalMode',
        before: null,
        after: false,
        timestamp: Date.now()
    });
    const v1 = await tracker.createVersion({
        description: 'Initial stable version',
        tags: ['stable', 'v1.0']
    });
    console.log('v1 created:', v1.description);
    // Enable experimental feature
    tracker.trackChange({
        type: temporal_js_1.ChangeType.MODIFICATION,
        path: 'feature.experimentalMode',
        before: false,
        after: true,
        timestamp: Date.now()
    });
    tracker.trackChange({
        type: temporal_js_1.ChangeType.ADDITION,
        path: 'feature.betaFeatures',
        before: null,
        after: ['feature1', 'feature2'],
        timestamp: Date.now()
    });
    const v2 = await tracker.createVersion({
        description: 'Experimental features enabled',
        tags: ['experimental', 'v2.0']
    });
    console.log('v2 created:', v2.description);
    // Current state
    const currentState = await tracker.queryAtTimestamp(Date.now());
    console.log('\nCurrent state:', JSON.stringify(currentState, null, 2));
    // Revert to stable version
    const revertVersion = await tracker.revertToVersion(v1.id);
    console.log('\nReverted to v1, created new version:', revertVersion.id);
    console.log('Revert description:', revertVersion.description);
    // Check state after revert
    const revertedState = await tracker.queryAtTimestamp(Date.now());
    console.log('\nState after revert:', JSON.stringify(revertedState, null, 2));
    console.log();
}
/**
 * Example 5: Visualization Data
 */
async function visualizationData() {
    console.log('=== Example 5: Visualization Data ===\n');
    const tracker = new temporal_js_1.TemporalTracker();
    // Create several versions with various changes
    for (let i = 0; i < 5; i++) {
        const changeCount = Math.floor(Math.random() * 5) + 1;
        for (let j = 0; j < changeCount; j++) {
            tracker.trackChange({
                type: [temporal_js_1.ChangeType.ADDITION, temporal_js_1.ChangeType.MODIFICATION, temporal_js_1.ChangeType.DELETION][j % 3],
                path: `data.entity${i}.field${j}`,
                before: j > 0 ? `value${j - 1}` : null,
                after: j < changeCount - 1 ? `value${j}` : null,
                timestamp: Date.now()
            });
        }
        await tracker.createVersion({
            description: `Version ${i + 1} with ${changeCount} changes`,
            tags: [`v${i + 1}`],
            author: `developer${(i % 3) + 1}`
        });
        await new Promise(resolve => setTimeout(resolve, 50));
    }
    // Get visualization data
    const vizData = tracker.getVisualizationData();
    console.log('Timeline:');
    vizData.timeline.forEach(item => {
        console.log(`- ${new Date(item.timestamp).toISOString()}: ${item.description}`);
        console.log(`  Changes: ${item.changeCount}, Tags: ${item.tags.join(', ')}`);
    });
    console.log('\nTop Hotspots:');
    vizData.hotspots.slice(0, 5).forEach(hotspot => {
        console.log(`- ${hotspot.path}: ${hotspot.changeCount} changes`);
    });
    console.log('\nVersion Graph:');
    console.log('Nodes:', vizData.versionGraph.nodes.length);
    console.log('Edges:', vizData.versionGraph.edges.length);
    console.log();
}
/**
 * Example 6: Audit Logging
 */
async function auditLogging() {
    console.log('=== Example 6: Audit Logging ===\n');
    const tracker = new temporal_js_1.TemporalTracker();
    // Listen to audit events
    tracker.on('auditLogged', (entry) => {
        console.log(`[AUDIT] ${entry.operation} - ${entry.status} at ${new Date(entry.timestamp).toISOString()}`);
    });
    // Perform various operations
    tracker.trackChange({
        type: temporal_js_1.ChangeType.ADDITION,
        path: 'test.data',
        before: null,
        after: 'value',
        timestamp: Date.now()
    });
    await tracker.createVersion({
        description: 'Test version',
        tags: ['test']
    });
    // Get audit log
    const auditLog = tracker.getAuditLog(10);
    console.log('\nRecent Audit Entries:');
    auditLog.forEach(entry => {
        console.log(`- ${entry.operation}: ${entry.status}`);
        console.log(`  Details:`, JSON.stringify(entry.details, null, 2));
    });
    console.log();
}
/**
 * Example 7: Storage Management
 */
async function storageManagement() {
    console.log('=== Example 7: Storage Management ===\n');
    const tracker = new temporal_js_1.TemporalTracker();
    // Create multiple versions
    for (let i = 0; i < 10; i++) {
        tracker.trackChange({
            type: temporal_js_1.ChangeType.ADDITION,
            path: `data.item${i}`,
            before: null,
            after: `value${i}`,
            timestamp: Date.now()
        });
        await tracker.createVersion({
            description: `Version ${i + 1}`,
            tags: i < 3 ? ['important'] : []
        });
        await new Promise(resolve => setTimeout(resolve, 10));
    }
    // Get storage stats before pruning
    const statsBefore = tracker.getStorageStats();
    console.log('Storage stats before pruning:');
    console.log(`- Versions: ${statsBefore.versionCount}`);
    console.log(`- Total changes: ${statsBefore.totalChanges}`);
    console.log(`- Estimated size: ${(statsBefore.estimatedSizeBytes / 1024).toFixed(2)} KB`);
    // Prune old versions, keeping last 5 and preserving tagged ones
    tracker.pruneVersions(5, ['baseline', 'important']);
    // Get storage stats after pruning
    const statsAfter = tracker.getStorageStats();
    console.log('\nStorage stats after pruning:');
    console.log(`- Versions: ${statsAfter.versionCount}`);
    console.log(`- Total changes: ${statsAfter.totalChanges}`);
    console.log(`- Estimated size: ${(statsAfter.estimatedSizeBytes / 1024).toFixed(2)} KB`);
    console.log(`- Space saved: ${((statsBefore.estimatedSizeBytes - statsAfter.estimatedSizeBytes) / 1024).toFixed(2)} KB`);
    console.log();
}
/**
 * Example 8: Backup and Restore
 */
async function backupAndRestore() {
    console.log('=== Example 8: Backup and Restore ===\n');
    const tracker1 = new temporal_js_1.TemporalTracker();
    // Create some versions
    tracker1.trackChange({
        type: temporal_js_1.ChangeType.ADDITION,
        path: 'important.data',
        before: null,
        after: { critical: true, value: 42 },
        timestamp: Date.now()
    });
    await tracker1.createVersion({
        description: 'Important data version',
        tags: ['production', 'critical']
    });
    // Export backup
    const backup = tracker1.exportBackup();
    console.log('Backup created:');
    console.log(`- Versions: ${backup.versions.length}`);
    console.log(`- Audit entries: ${backup.auditLog.length}`);
    console.log(`- Exported at: ${new Date(backup.exportedAt).toISOString()}`);
    // Create new tracker and import
    const tracker2 = new temporal_js_1.TemporalTracker();
    tracker2.importBackup(backup);
    console.log('\nBackup restored to new tracker:');
    const restoredVersions = tracker2.listVersions();
    console.log(`- Restored versions: ${restoredVersions.length}`);
    restoredVersions.forEach(v => {
        console.log(`  - ${v.description} (${v.tags.join(', ')})`);
    });
    // Verify data integrity
    const originalState = await tracker1.queryAtTimestamp(Date.now());
    const restoredState = await tracker2.queryAtTimestamp(Date.now());
    console.log('\nData integrity check:');
    console.log(`- States match: ${JSON.stringify(originalState) === JSON.stringify(restoredState)}`);
    console.log();
}
/**
 * Example 9: Event-Driven Architecture
 */
async function eventDrivenArchitecture() {
    console.log('=== Example 9: Event-Driven Architecture ===\n');
    const tracker = new temporal_js_1.TemporalTracker();
    // Set up event listeners
    tracker.on('versionCreated', (version) => {
        console.log(`✓ Version created: ${version.description}`);
        console.log(`  ID: ${version.id}, Changes: ${version.changes.length}`);
    });
    tracker.on('changeTracked', (change) => {
        console.log(`→ Change tracked: ${change.type} at ${change.path}`);
    });
    tracker.on('versionReverted', (fromVersion, toVersion) => {
        console.log(`⟲ Reverted from ${fromVersion} to ${toVersion}`);
    });
    // Perform operations that trigger events
    console.log('Tracking changes...');
    tracker.trackChange({
        type: temporal_js_1.ChangeType.ADDITION,
        path: 'events.example',
        before: null,
        after: 'test',
        timestamp: Date.now()
    });
    console.log('\nCreating version...');
    await tracker.createVersion({
        description: 'Event demo version',
        tags: ['demo']
    });
    console.log();
}
/**
 * Run all examples
 */
async function runAllExamples() {
    try {
        await basicVersionManagement();
        await timeTravelQueries();
        await versionComparison();
        await versionReverting();
        await visualizationData();
        await auditLogging();
        await storageManagement();
        await backupAndRestore();
        await eventDrivenArchitecture();
        console.log('✓ All examples completed successfully!');
    }
    catch (error) {
        console.error('Error running examples:', error);
        throw error;
    }
}
// Run if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
    runAllExamples().catch(console.error);
}
//# sourceMappingURL=temporal-example.js.map