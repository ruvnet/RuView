"use strict";
/**
 * Example usage of the Database Persistence module
 *
 * This example demonstrates all major features:
 * - Basic save/load operations
 * - Snapshot management
 * - Export/import
 * - Progress callbacks
 * - Auto-save configuration
 * - Incremental saves
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.example1_BasicSaveLoad = example1_BasicSaveLoad;
exports.example2_SnapshotManagement = example2_SnapshotManagement;
exports.example3_ExportImport = example3_ExportImport;
exports.example4_AutoSaveIncremental = example4_AutoSaveIncremental;
exports.example5_AdvancedProgress = example5_AdvancedProgress;
const ruvector_1 = require("ruvector");
const persistence_js_1 = require("../persistence.js");
// ============================================================================
// Example 1: Basic Save and Load
// ============================================================================
async function example1_BasicSaveLoad() {
    console.log('\n=== Example 1: Basic Save and Load ===\n');
    // Create a vector database
    const db = new ruvector_1.VectorDB({
        dimension: 384,
        metric: 'cosine',
    });
    // Add some sample vectors
    console.log('Adding sample vectors...');
    for (let i = 0; i < 1000; i++) {
        db.insert({
            id: `doc-${i}`,
            vector: Array(384).fill(0).map(() => Math.random()),
            metadata: {
                category: i % 3 === 0 ? 'A' : i % 3 === 1 ? 'B' : 'C',
                timestamp: Date.now() - i * 1000,
            },
        });
    }
    console.log(`Added ${db.stats().count} vectors`);
    // Create persistence manager
    const persistence = new persistence_js_1.DatabasePersistence(db, {
        baseDir: './data/example1',
        format: 'json',
        compression: 'gzip',
    });
    // Save database with progress tracking
    console.log('\nSaving database...');
    const savePath = await persistence.save({
        onProgress: (progress) => {
            console.log(`  [${progress.percentage}%] ${progress.message}`);
        },
    });
    console.log(`Saved to: ${savePath}`);
    // Create a new database and load the saved data
    const db2 = new ruvector_1.VectorDB({ dimension: 384 });
    const persistence2 = new persistence_js_1.DatabasePersistence(db2, {
        baseDir: './data/example1',
    });
    console.log('\nLoading database...');
    await persistence2.load({
        path: savePath,
        verifyChecksum: true,
        onProgress: (progress) => {
            console.log(`  [${progress.percentage}%] ${progress.message}`);
        },
    });
    console.log(`Loaded ${db2.stats().count} vectors`);
    // Verify data integrity
    const original = db.get('doc-500');
    const loaded = db2.get('doc-500');
    console.log('\nData integrity check:');
    console.log('  Original metadata:', original?.metadata);
    console.log('  Loaded metadata:  ', loaded?.metadata);
    console.log('  Match:', JSON.stringify(original) === JSON.stringify(loaded) ? '✓' : '✗');
}
// ============================================================================
// Example 2: Snapshot Management
// ============================================================================
async function example2_SnapshotManagement() {
    console.log('\n=== Example 2: Snapshot Management ===\n');
    const db = new ruvector_1.VectorDB({ dimension: 128 });
    const persistence = new persistence_js_1.DatabasePersistence(db, {
        baseDir: './data/example2',
        format: 'binary',
        compression: 'gzip',
        maxSnapshots: 5,
    });
    // Create initial data
    console.log('Creating initial dataset...');
    for (let i = 0; i < 500; i++) {
        db.insert({
            id: `v${i}`,
            vector: Array(128).fill(0).map(() => Math.random()),
        });
    }
    // Create snapshot before major changes
    console.log('\nCreating snapshot "before-update"...');
    const snapshot1 = await persistence.createSnapshot('before-update', {
        description: 'Baseline before adding new vectors',
        user: 'admin',
    });
    console.log(`Snapshot created: ${snapshot1.id}`);
    console.log(`  Name: ${snapshot1.name}`);
    console.log(`  Vectors: ${snapshot1.vectorCount}`);
    console.log(`  Size: ${(0, persistence_js_1.formatFileSize)(snapshot1.fileSize)}`);
    console.log(`  Created: ${(0, persistence_js_1.formatTimestamp)(snapshot1.timestamp)}`);
    // Make changes
    console.log('\nAdding more vectors...');
    for (let i = 500; i < 1000; i++) {
        db.insert({
            id: `v${i}`,
            vector: Array(128).fill(0).map(() => Math.random()),
        });
    }
    // Create another snapshot
    console.log('\nCreating snapshot "after-update"...');
    const snapshot2 = await persistence.createSnapshot('after-update');
    console.log(`Snapshot created: ${snapshot2.id} (${snapshot2.vectorCount} vectors)`);
    // List all snapshots
    console.log('\nAll snapshots:');
    const snapshots = await persistence.listSnapshots();
    for (const snapshot of snapshots) {
        console.log(`  ${snapshot.name}: ${snapshot.vectorCount} vectors, ${(0, persistence_js_1.formatFileSize)(snapshot.fileSize)}`);
    }
    // Restore from first snapshot
    console.log('\nRestoring from "before-update" snapshot...');
    await persistence.restoreSnapshot(snapshot1.id, {
        verifyChecksum: true,
        onProgress: (p) => console.log(`  [${p.percentage}%] ${p.message}`),
    });
    console.log(`After restore: ${db.stats().count} vectors`);
    // Delete a snapshot
    console.log('\nDeleting snapshot...');
    await persistence.deleteSnapshot(snapshot2.id);
    console.log('Snapshot deleted');
}
// ============================================================================
// Example 3: Export and Import
// ============================================================================
async function example3_ExportImport() {
    console.log('\n=== Example 3: Export and Import ===\n');
    // Create source database
    const sourceDb = new ruvector_1.VectorDB({ dimension: 256 });
    console.log('Creating source database...');
    for (let i = 0; i < 2000; i++) {
        sourceDb.insert({
            id: `item-${i}`,
            vector: Array(256).fill(0).map(() => Math.random()),
            metadata: {
                type: 'product',
                price: Math.random() * 100,
                rating: Math.floor(Math.random() * 5) + 1,
            },
        });
    }
    const sourcePersistence = new persistence_js_1.DatabasePersistence(sourceDb, {
        baseDir: './data/example3/source',
    });
    // Export to different formats
    console.log('\nExporting to JSON...');
    await sourcePersistence.export({
        path: './data/example3/export/database.json',
        format: 'json',
        compress: false,
        includeIndex: false,
        onProgress: (p) => console.log(`  [${p.percentage}%] ${p.message}`),
    });
    console.log('\nExporting to compressed binary...');
    await sourcePersistence.export({
        path: './data/example3/export/database.bin.gz',
        format: 'binary',
        compress: true,
        includeIndex: true,
    });
    // Import into new database
    const targetDb = new ruvector_1.VectorDB({ dimension: 256 });
    const targetPersistence = new persistence_js_1.DatabasePersistence(targetDb, {
        baseDir: './data/example3/target',
    });
    console.log('\nImporting from compressed binary...');
    await targetPersistence.import({
        path: './data/example3/export/database.bin.gz',
        format: 'binary',
        clear: true,
        verifyChecksum: true,
        onProgress: (p) => console.log(`  [${p.percentage}%] ${p.message}`),
    });
    console.log(`\nImport complete: ${targetDb.stats().count} vectors`);
    // Test a search to verify data integrity
    const sampleVector = sourceDb.get('item-100');
    if (sampleVector) {
        const results = targetDb.search({
            vector: sampleVector.vector,
            k: 1,
        });
        console.log('\nData integrity verification:');
        console.log('  Search for item-100:', results[0]?.id === 'item-100' ? '✓' : '✗');
        console.log('  Similarity score:', results[0]?.score.toFixed(4));
    }
}
// ============================================================================
// Example 4: Auto-Save and Incremental Saves
// ============================================================================
async function example4_AutoSaveIncremental() {
    console.log('\n=== Example 4: Auto-Save and Incremental Saves ===\n');
    const db = new ruvector_1.VectorDB({ dimension: 64 });
    const persistence = new persistence_js_1.DatabasePersistence(db, {
        baseDir: './data/example4',
        format: 'json',
        compression: 'none',
        incremental: true,
        autoSaveInterval: 5000, // Auto-save every 5 seconds
        maxSnapshots: 3,
    });
    console.log('Auto-save enabled (every 5 seconds)');
    console.log('Incremental saves enabled');
    // Add initial batch
    console.log('\nAdding initial batch (500 vectors)...');
    for (let i = 0; i < 500; i++) {
        db.insert({
            id: `vec-${i}`,
            vector: Array(64).fill(0).map(() => Math.random()),
        });
    }
    // Manual incremental save
    console.log('\nPerforming initial save...');
    await persistence.save();
    // Simulate ongoing operations
    console.log('\nAdding more vectors...');
    for (let i = 500; i < 600; i++) {
        db.insert({
            id: `vec-${i}`,
            vector: Array(64).fill(0).map(() => Math.random()),
        });
    }
    // Incremental save (only saves changes)
    console.log('\nPerforming incremental save...');
    const incrementalPath = await persistence.saveIncremental();
    if (incrementalPath) {
        console.log(`Incremental save completed: ${incrementalPath}`);
    }
    else {
        console.log('No changes detected (skip)');
    }
    // Wait for auto-save to trigger
    console.log('\nWaiting for auto-save (5 seconds)...');
    await new Promise(resolve => setTimeout(resolve, 6000));
    // Cleanup
    console.log('\nShutting down (final save)...');
    await persistence.shutdown();
    console.log('Shutdown complete');
}
// ============================================================================
// Example 5: Advanced Progress Tracking
// ============================================================================
async function example5_AdvancedProgress() {
    console.log('\n=== Example 5: Advanced Progress Tracking ===\n');
    const db = new ruvector_1.VectorDB({ dimension: 512 });
    // Create large dataset
    console.log('Creating large dataset (5000 vectors)...');
    const startTime = Date.now();
    for (let i = 0; i < 5000; i++) {
        db.insert({
            id: `large-${i}`,
            vector: Array(512).fill(0).map(() => Math.random()),
            metadata: {
                batch: Math.floor(i / 100),
                index: i,
            },
        });
    }
    console.log(`Dataset created in ${Date.now() - startTime}ms`);
    const persistence = new persistence_js_1.DatabasePersistence(db, {
        baseDir: './data/example5',
        format: 'binary',
        compression: 'gzip',
        batchSize: 500, // Process in batches of 500
    });
    // Custom progress handler with detailed stats
    let lastUpdate = Date.now();
    const progressHandler = (progress) => {
        const now = Date.now();
        const elapsed = now - lastUpdate;
        if (elapsed > 100) { // Update max every 100ms
            const bar = '█'.repeat(Math.floor(progress.percentage / 2)) +
                '░'.repeat(50 - Math.floor(progress.percentage / 2));
            process.stdout.write(`\r  [${bar}] ${progress.percentage}% - ${progress.message}`.padEnd(100));
            lastUpdate = now;
        }
    };
    // Save with detailed progress
    console.log('\nSaving with progress tracking:');
    const saveStart = Date.now();
    await persistence.save({
        compress: true,
        onProgress: progressHandler,
    });
    console.log(`\n\nSave completed in ${Date.now() - saveStart}ms`);
    // Load with progress
    const db2 = new ruvector_1.VectorDB({ dimension: 512 });
    const persistence2 = new persistence_js_1.DatabasePersistence(db2, {
        baseDir: './data/example5',
    });
    console.log('\nLoading with progress tracking:');
    const loadStart = Date.now();
    await persistence2.load({
        path: './data/example5/database.bin.gz',
        verifyChecksum: true,
        onProgress: progressHandler,
    });
    console.log(`\n\nLoad completed in ${Date.now() - loadStart}ms`);
    console.log(`Loaded ${db2.stats().count} vectors`);
}
// ============================================================================
// Run All Examples
// ============================================================================
async function runAllExamples() {
    try {
        await example1_BasicSaveLoad();
        await example2_SnapshotManagement();
        await example3_ExportImport();
        await example4_AutoSaveIncremental();
        await example5_AdvancedProgress();
        console.log('\n\n✓ All examples completed successfully!\n');
    }
    catch (error) {
        console.error('\n✗ Error running examples:', error);
        process.exit(1);
    }
}
// Run examples if executed directly
if (import.meta.url === `file://${process.argv[1]}`) {
    runAllExamples();
}
//# sourceMappingURL=persistence-example.js.map