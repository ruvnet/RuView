/**
 * Tests for Temporal Tracking Module
 */

import { test } from 'node:test';
import assert from 'node:assert';
import {
  TemporalTracker,
  ChangeType,
  isChange,
  isVersion
} from '../dist/temporal.js';

test('TemporalTracker - Basic version creation', async () => {
  const tracker = new TemporalTracker();

  // Track a change
  tracker.trackChange({
    type: ChangeType.ADDITION,
    path: 'nodes.User',
    before: null,
    after: { name: 'User', properties: ['id', 'name'] },
    timestamp: Date.now()
  });

  // Create version
  const version = await tracker.createVersion({
    description: 'Initial schema',
    tags: ['v1.0']
  });

  assert.ok(version.id, 'Version should have an ID');
  assert.strictEqual(version.description, 'Initial schema');
  assert.ok(version.tags.includes('v1.0'));
  assert.strictEqual(version.changes.length, 1);
});

test('TemporalTracker - List versions', async () => {
  const tracker = new TemporalTracker();

  // Create multiple versions
  for (let i = 0; i < 3; i++) {
    tracker.trackChange({
      type: ChangeType.ADDITION,
      path: `node${i}`,
      before: null,
      after: `value${i}`,
      timestamp: Date.now()
    });

    await tracker.createVersion({
      description: `Version ${i + 1}`,
      tags: [`v${i + 1}`]
    });
  }

  const versions = tracker.listVersions();
  assert.ok(versions.length >= 3, 'Should have at least 3 versions');
});

test('TemporalTracker - Time-travel query', async () => {
  const tracker = new TemporalTracker();

  // Create initial version
  tracker.trackChange({
    type: ChangeType.ADDITION,
    path: 'config.value',
    before: null,
    after: 100,
    timestamp: Date.now()
  });

  const v1 = await tracker.createVersion({
    description: 'Version 1'
  });

  // Wait to ensure different timestamps
  await new Promise(resolve => setTimeout(resolve, 10));

  // Create second version
  tracker.trackChange({
    type: ChangeType.MODIFICATION,
    path: 'config.value',
    before: 100,
    after: 200,
    timestamp: Date.now()
  });

  const v2 = await tracker.createVersion({
    description: 'Version 2'
  });

  // Query at v1
  const stateAtV1 = await tracker.queryAtTimestamp(v1.timestamp);
  assert.strictEqual(stateAtV1.config.value, 100);

  // Query at v2
  const stateAtV2 = await tracker.queryAtTimestamp(v2.timestamp);
  assert.strictEqual(stateAtV2.config.value, 200);
});

test('TemporalTracker - Compare versions', async () => {
  const tracker = new TemporalTracker();

  // Version 1
  tracker.trackChange({
    type: ChangeType.ADDITION,
    path: 'data.field1',
    before: null,
    after: 'value1',
    timestamp: Date.now()
  });

  const v1 = await tracker.createVersion({ description: 'V1' });

  // Version 2
  tracker.trackChange({
    type: ChangeType.ADDITION,
    path: 'data.field2',
    before: null,
    after: 'value2',
    timestamp: Date.now()
  });

  tracker.trackChange({
    type: ChangeType.MODIFICATION,
    path: 'data.field1',
    before: 'value1',
    after: 'value1-modified',
    timestamp: Date.now()
  });

  const v2 = await tracker.createVersion({ description: 'V2' });

  // Compare
  const diff = await tracker.compareVersions(v1.id, v2.id);

  assert.strictEqual(diff.fromVersion, v1.id);
  assert.strictEqual(diff.toVersion, v2.id);
  assert.ok(diff.changes.length > 0);
  assert.strictEqual(diff.summary.additions, 1);
  assert.strictEqual(diff.summary.modifications, 1);
});

test('TemporalTracker - Revert version', async () => {
  const tracker = new TemporalTracker();

  // V1: Add data
  tracker.trackChange({
    type: ChangeType.ADDITION,
    path: 'test.data',
    before: null,
    after: 'original',
    timestamp: Date.now()
  });

  const v1 = await tracker.createVersion({ description: 'V1' });

  // V2: Modify data
  tracker.trackChange({
    type: ChangeType.MODIFICATION,
    path: 'test.data',
    before: 'original',
    after: 'modified',
    timestamp: Date.now()
  });

  await tracker.createVersion({ description: 'V2' });

  // Revert to V1
  const revertVersion = await tracker.revertToVersion(v1.id);

  assert.ok(revertVersion.id);
  assert.ok(revertVersion.description.includes('Revert'));

  // Check state is back to original
  const currentState = await tracker.queryAtTimestamp(Date.now());
  assert.strictEqual(currentState.test.data, 'original');
});

test('TemporalTracker - Add tags', async () => {
  const tracker = new TemporalTracker();

  tracker.trackChange({
    type: ChangeType.ADDITION,
    path: 'test',
    before: null,
    after: 'value',
    timestamp: Date.now()
  });

  const version = await tracker.createVersion({
    description: 'Test',
    tags: ['initial']
  });

  // Add more tags
  tracker.addTags(version.id, ['production', 'stable']);

  const retrieved = tracker.getVersion(version.id);
  assert.ok(retrieved.tags.includes('production'));
  assert.ok(retrieved.tags.includes('stable'));
  assert.ok(retrieved.tags.includes('initial'));
});

test('TemporalTracker - Visualization data', async () => {
  const tracker = new TemporalTracker();

  // Create multiple versions
  for (let i = 0; i < 3; i++) {
    tracker.trackChange({
      type: ChangeType.ADDITION,
      path: `node${i}`,
      before: null,
      after: `value${i}`,
      timestamp: Date.now()
    });

    await tracker.createVersion({ description: `V${i}` });
  }

  const vizData = tracker.getVisualizationData();

  assert.ok(vizData.timeline.length >= 3);
  assert.ok(Array.isArray(vizData.changeFrequency));
  assert.ok(Array.isArray(vizData.hotspots));
  assert.ok(vizData.versionGraph.nodes.length >= 3);
  assert.ok(Array.isArray(vizData.versionGraph.edges));
});

test('TemporalTracker - Audit log', async () => {
  const tracker = new TemporalTracker();

  tracker.trackChange({
    type: ChangeType.ADDITION,
    path: 'test',
    before: null,
    after: 'value',
    timestamp: Date.now()
  });

  await tracker.createVersion({ description: 'Test version' });

  const auditLog = tracker.getAuditLog(10);
  assert.ok(auditLog.length > 0);

  const createEntry = auditLog.find(e => e.operation === 'create');
  assert.ok(createEntry);
  assert.strictEqual(createEntry.status, 'success');
});

test('TemporalTracker - Storage stats', async () => {
  const tracker = new TemporalTracker();

  tracker.trackChange({
    type: ChangeType.ADDITION,
    path: 'test',
    before: null,
    after: 'value',
    timestamp: Date.now()
  });

  await tracker.createVersion({ description: 'Test' });

  const stats = tracker.getStorageStats();
  assert.ok(stats.versionCount > 0);
  assert.ok(stats.totalChanges > 0);
  assert.ok(stats.estimatedSizeBytes > 0);
  assert.ok(stats.oldestVersion >= 0); // Baseline is at timestamp 0
  assert.ok(stats.newestVersion > 0);
});

test('TemporalTracker - Prune versions', async () => {
  const tracker = new TemporalTracker();

  // Create many versions
  for (let i = 0; i < 10; i++) {
    tracker.trackChange({
      type: ChangeType.ADDITION,
      path: `node${i}`,
      before: null,
      after: `value${i}`,
      timestamp: Date.now()
    });

    await tracker.createVersion({
      description: `V${i}`,
      tags: i < 2 ? ['important'] : []
    });
  }

  const beforePrune = tracker.listVersions().length;

  // Prune, keeping only last 3 versions + important ones
  tracker.pruneVersions(3, ['baseline', 'important']);

  const afterPrune = tracker.listVersions().length;

  // Should have pruned some versions
  assert.ok(afterPrune < beforePrune);

  // Important versions should still exist
  const importantVersions = tracker.listVersions(['important']);
  assert.ok(importantVersions.length >= 2);
});

test('TemporalTracker - Backup and restore', async () => {
  const tracker1 = new TemporalTracker();

  // Create data
  tracker1.trackChange({
    type: ChangeType.ADDITION,
    path: 'important.data',
    before: null,
    after: { value: 42 },
    timestamp: Date.now()
  });

  await tracker1.createVersion({
    description: 'Important version',
    tags: ['backup-test']
  });

  // Export backup
  const backup = tracker1.exportBackup();
  assert.ok(backup.versions.length > 0);
  assert.ok(backup.exportedAt > 0);

  // Import to new tracker
  const tracker2 = new TemporalTracker();
  tracker2.importBackup(backup);

  // Verify data
  const versions = tracker2.listVersions(['backup-test']);
  assert.ok(versions.length > 0);

  const state = await tracker2.queryAtTimestamp(Date.now());
  assert.deepStrictEqual(state.important.data, { value: 42 });
});

test('TemporalTracker - Event emission', async (t) => {
  const tracker = new TemporalTracker();
  let versionCreatedEmitted = false;
  let changeTrackedEmitted = false;

  tracker.on('versionCreated', () => {
    versionCreatedEmitted = true;
  });

  tracker.on('changeTracked', () => {
    changeTrackedEmitted = true;
  });

  tracker.trackChange({
    type: ChangeType.ADDITION,
    path: 'test',
    before: null,
    after: 'value',
    timestamp: Date.now()
  });

  await tracker.createVersion({ description: 'Test' });

  assert.ok(changeTrackedEmitted, 'changeTracked event should be emitted');
  assert.ok(versionCreatedEmitted, 'versionCreated event should be emitted');
});

test('Type guards - isChange', () => {
  const validChange = {
    type: ChangeType.ADDITION,
    path: 'test.path',
    before: null,
    after: 'value',
    timestamp: Date.now()
  };

  const invalidChange = {
    type: 'invalid',
    path: 123,
    timestamp: 'not-a-number'
  };

  assert.ok(isChange(validChange));
  assert.ok(!isChange(invalidChange));
});

test('Type guards - isVersion', () => {
  const validVersion = {
    id: 'test-id',
    parentId: null,
    timestamp: Date.now(),
    description: 'Test',
    changes: [],
    tags: [],
    checksum: 'abc123',
    metadata: {}
  };

  const invalidVersion = {
    id: 123,
    timestamp: 'invalid',
    changes: 'not-an-array',
    tags: null
  };

  assert.ok(isVersion(validVersion));
  assert.ok(!isVersion(invalidVersion));
});
