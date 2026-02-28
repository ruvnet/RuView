/**
 * Tests for Database Persistence Module
 *
 * This test suite covers:
 * - Save and load operations
 * - Snapshot management
 * - Export/import functionality
 * - Progress callbacks
 * - Incremental saves
 * - Error handling
 * - Data integrity verification
 */

import { test } from 'node:test';
import { strictEqual, ok, deepStrictEqual } from 'node:assert';
import { promises as fs } from 'fs';
import * as path from 'path';
import { VectorDB } from 'ruvector';
import {
  DatabasePersistence,
  formatFileSize,
  formatTimestamp,
  estimateMemoryUsage,
} from '../src/persistence.js';

const TEST_DATA_DIR = './test-data';

// Cleanup helper
async function cleanup() {
  try {
    await fs.rm(TEST_DATA_DIR, { recursive: true, force: true });
  } catch (error) {
    // Ignore errors
  }
}

// Create sample database
function createSampleDB(dimension = 128, count = 100) {
  const db = new VectorDB({ dimension, metric: 'cosine' });

  for (let i = 0; i < count; i++) {
    db.insert({
      id: `doc-${i}`,
      vector: Array(dimension).fill(0).map(() => Math.random()),
      metadata: {
        index: i,
        category: i % 3 === 0 ? 'A' : i % 3 === 1 ? 'B' : 'C',
        timestamp: Date.now() - i * 1000,
      },
    });
  }

  return db;
}

// ============================================================================
// Test Suite
// ============================================================================

test('DatabasePersistence - Save and Load', async (t) => {
  await cleanup();

  const db = createSampleDB(128, 100);
  const persistence = new DatabasePersistence(db, {
    baseDir: path.join(TEST_DATA_DIR, 'save-load'),
  });

  // Save
  const savePath = await persistence.save();
  ok(savePath, 'Save should return a path');

  // Verify file exists
  const stats = await fs.stat(savePath);
  ok(stats.size > 0, 'Saved file should not be empty');

  // Load into new database
  const db2 = new VectorDB({ dimension: 128 });
  const persistence2 = new DatabasePersistence(db2, {
    baseDir: path.join(TEST_DATA_DIR, 'save-load'),
  });

  await persistence2.load({ path: savePath });

  // Verify data
  strictEqual(db2.stats().count, 100, 'Should load all vectors');

  const original = db.get('doc-50');
  const loaded = db2.get('doc-50');

  ok(original && loaded, 'Should retrieve same document');
  deepStrictEqual(loaded.metadata, original.metadata, 'Metadata should match');
});

test('DatabasePersistence - Compressed Save', async (t) => {
  await cleanup();

  const db = createSampleDB(128, 200);
  const persistence = new DatabasePersistence(db, {
    baseDir: path.join(TEST_DATA_DIR, 'compressed'),
    compression: 'gzip',
  });

  const savePath = await persistence.save({ compress: true });

  // Verify compression
  const compressedStats = await fs.stat(savePath);

  // Save uncompressed for comparison
  const persistence2 = new DatabasePersistence(db, {
    baseDir: path.join(TEST_DATA_DIR, 'uncompressed'),
    compression: 'none',
  });

  const uncompressedPath = await persistence2.save({ compress: false });
  const uncompressedStats = await fs.stat(uncompressedPath);

  ok(
    compressedStats.size < uncompressedStats.size,
    'Compressed file should be smaller'
  );
});

test('DatabasePersistence - Snapshot Management', async (t) => {
  await cleanup();

  const db = createSampleDB(64, 50);
  const persistence = new DatabasePersistence(db, {
    baseDir: path.join(TEST_DATA_DIR, 'snapshots'),
    maxSnapshots: 3,
  });

  // Create snapshots
  const snap1 = await persistence.createSnapshot('snapshot-1', {
    description: 'First snapshot',
  });

  ok(snap1.id, 'Snapshot should have ID');
  strictEqual(snap1.name, 'snapshot-1', 'Snapshot name should match');
  strictEqual(snap1.vectorCount, 50, 'Snapshot should record vector count');

  // Add more vectors
  for (let i = 50; i < 100; i++) {
    db.insert({
      id: `doc-${i}`,
      vector: Array(64).fill(0).map(() => Math.random()),
    });
  }

  const snap2 = await persistence.createSnapshot('snapshot-2');
  strictEqual(snap2.vectorCount, 100, 'Second snapshot should have more vectors');

  // List snapshots
  const snapshots = await persistence.listSnapshots();
  strictEqual(snapshots.length, 2, 'Should have 2 snapshots');

  // Restore first snapshot
  await persistence.restoreSnapshot(snap1.id);
  strictEqual(db.stats().count, 50, 'Should restore to 50 vectors');

  // Delete snapshot
  await persistence.deleteSnapshot(snap1.id);
  const remaining = await persistence.listSnapshots();
  strictEqual(remaining.length, 1, 'Should have 1 snapshot after deletion');
});

test('DatabasePersistence - Export and Import', async (t) => {
  await cleanup();

  const db = createSampleDB(256, 150);
  const persistence = new DatabasePersistence(db, {
    baseDir: path.join(TEST_DATA_DIR, 'export'),
  });

  const exportPath = path.join(TEST_DATA_DIR, 'export', 'database-export.json');

  // Export
  await persistence.export({
    path: exportPath,
    format: 'json',
    compress: false,
  });

  // Verify export file
  const exportStats = await fs.stat(exportPath);
  ok(exportStats.size > 0, 'Export file should exist');

  // Import into new database
  const db2 = new VectorDB({ dimension: 256 });
  const persistence2 = new DatabasePersistence(db2, {
    baseDir: path.join(TEST_DATA_DIR, 'import'),
  });

  await persistence2.import({
    path: exportPath,
    clear: true,
    verifyChecksum: true,
  });

  strictEqual(db2.stats().count, 150, 'Should import all vectors');
});

test('DatabasePersistence - Progress Callbacks', async (t) => {
  await cleanup();

  const db = createSampleDB(128, 300);
  const persistence = new DatabasePersistence(db, {
    baseDir: path.join(TEST_DATA_DIR, 'progress'),
  });

  let progressCalls = 0;
  let lastPercentage = 0;

  await persistence.save({
    onProgress: (progress) => {
      progressCalls++;
      ok(progress.percentage >= 0 && progress.percentage <= 100, 'Percentage should be 0-100');
      ok(progress.percentage >= lastPercentage, 'Percentage should increase');
      ok(progress.message, 'Should have progress message');
      lastPercentage = progress.percentage;
    },
  });

  ok(progressCalls > 0, 'Should call progress callback');
  strictEqual(lastPercentage, 100, 'Should reach 100%');
});

test('DatabasePersistence - Checksum Verification', async (t) => {
  await cleanup();

  const db = createSampleDB(128, 100);
  const persistence = new DatabasePersistence(db, {
    baseDir: path.join(TEST_DATA_DIR, 'checksum'),
  });

  const savePath = await persistence.save();

  // Load with checksum verification
  const db2 = new VectorDB({ dimension: 128 });
  const persistence2 = new DatabasePersistence(db2, {
    baseDir: path.join(TEST_DATA_DIR, 'checksum'),
  });

  // Should succeed with valid checksum
  await persistence2.load({
    path: savePath,
    verifyChecksum: true,
  });

  strictEqual(db2.stats().count, 100, 'Should load successfully');

  // Corrupt the file
  const data = await fs.readFile(savePath, 'utf-8');
  const corrupted = data.replace('"doc-50"', '"doc-XX"');
  await fs.writeFile(savePath, corrupted);

  // Should fail with corrupted file
  const db3 = new VectorDB({ dimension: 128 });
  const persistence3 = new DatabasePersistence(db3, {
    baseDir: path.join(TEST_DATA_DIR, 'checksum'),
  });

  let errorThrown = false;
  try {
    await persistence3.load({
      path: savePath,
      verifyChecksum: true,
    });
  } catch (error) {
    errorThrown = true;
    ok(error.message.includes('checksum'), 'Should mention checksum in error');
  }

  ok(errorThrown, 'Should throw error for corrupted file');
});

test('Utility Functions', async (t) => {
  // Test formatFileSize
  strictEqual(formatFileSize(0), '0.00 B');
  strictEqual(formatFileSize(1024), '1.00 KB');
  strictEqual(formatFileSize(1024 * 1024), '1.00 MB');
  strictEqual(formatFileSize(1536 * 1024), '1.50 MB');

  // Test formatTimestamp
  const timestamp = new Date('2024-01-15T10:30:00.000Z').getTime();
  ok(formatTimestamp(timestamp).includes('2024-01-15'));

  // Test estimateMemoryUsage
  const state = {
    version: '1.0.0',
    options: { dimension: 128, metric: 'cosine' as const },
    stats: { count: 100, dimension: 128, metric: 'cosine' },
    vectors: Array(100).fill(null).map((_, i) => ({
      id: `doc-${i}`,
      vector: Array(128).fill(0),
      metadata: { index: i },
    })),
    timestamp: Date.now(),
  };

  const usage = estimateMemoryUsage(state);
  ok(usage > 0, 'Should estimate positive memory usage');
});

test('DatabasePersistence - Snapshot Cleanup', async (t) => {
  await cleanup();

  const db = createSampleDB(64, 50);
  const persistence = new DatabasePersistence(db, {
    baseDir: path.join(TEST_DATA_DIR, 'cleanup'),
    maxSnapshots: 2,
  });

  // Create 4 snapshots
  await persistence.createSnapshot('snap-1');
  await persistence.createSnapshot('snap-2');
  await persistence.createSnapshot('snap-3');
  await persistence.createSnapshot('snap-4');

  // Should only keep 2 most recent
  const snapshots = await persistence.listSnapshots();
  strictEqual(snapshots.length, 2, 'Should auto-cleanup old snapshots');
  strictEqual(snapshots[0].name, 'snap-4', 'Should keep newest');
  strictEqual(snapshots[1].name, 'snap-3', 'Should keep second newest');
});

// Cleanup after all tests
test.after(async () => {
  await cleanup();
});
