# Temporal Tracking - Quick Start Guide

Get started with temporal tracking in 5 minutes!

## Installation

```bash
npm install ruvector-extensions
```

## Basic Usage

```typescript
import { TemporalTracker, ChangeType } from 'ruvector-extensions';

// Create tracker
const tracker = new TemporalTracker();

// Track a change
tracker.trackChange({
  type: ChangeType.ADDITION,
  path: 'nodes.User',
  before: null,
  after: { name: 'User', properties: ['id', 'name', 'email'] },
  timestamp: Date.now()
});

// Create version
const v1 = await tracker.createVersion({
  description: 'Initial user schema',
  tags: ['v1.0']
});

console.log('Created version:', v1.id);
```

## Common Operations

### 1. Track Multiple Changes

```typescript
// Add User node
tracker.trackChange({
  type: ChangeType.ADDITION,
  path: 'nodes.User',
  before: null,
  after: { name: 'User', properties: ['id', 'name'] },
  timestamp: Date.now()
});

// Add FOLLOWS edge
tracker.trackChange({
  type: ChangeType.ADDITION,
  path: 'edges.FOLLOWS',
  before: null,
  after: { from: 'User', to: 'User' },
  timestamp: Date.now()
});

// Create version with both changes
const version = await tracker.createVersion({
  description: 'Social graph schema',
  tags: ['v1.0', 'production']
});
```

### 2. Time-Travel Queries

```typescript
// Query state at specific time
const yesterday = Date.now() - 86400000;
const pastState = await tracker.queryAtTimestamp(yesterday);

console.log('Database state 24h ago:', pastState);

// Query state at specific version
const stateAtV1 = await tracker.queryAtTimestamp({
  versionId: v1.id
});
```

### 3. Compare Versions

```typescript
const diff = await tracker.compareVersions(v1.id, v2.id);

console.log('Changes between versions:');
console.log(`Added: ${diff.summary.additions}`);
console.log(`Modified: ${diff.summary.modifications}`);
console.log(`Deleted: ${diff.summary.deletions}`);

diff.changes.forEach(change => {
  console.log(`${change.type}: ${change.path}`);
});
```

### 4. Revert to Previous Version

```typescript
// Something went wrong, revert!
const revertVersion = await tracker.revertToVersion(v1.id);

console.log('Reverted to:', v1.description);
console.log('Created revert version:', revertVersion.id);
```

### 5. List Versions

```typescript
// All versions
const allVersions = tracker.listVersions();

// Production versions only
const prodVersions = tracker.listVersions(['production']);

allVersions.forEach(v => {
  console.log(`${v.description} - ${v.tags.join(', ')}`);
});
```

## Change Types

### Addition
```typescript
tracker.trackChange({
  type: ChangeType.ADDITION,
  path: 'nodes.NewType',
  before: null,  // Was nothing
  after: { ... }, // Now exists
  timestamp: Date.now()
});
```

### Modification
```typescript
tracker.trackChange({
  type: ChangeType.MODIFICATION,
  path: 'config.maxUsers',
  before: 100,    // Was 100
  after: 500,     // Now 500
  timestamp: Date.now()
});
```

### Deletion
```typescript
tracker.trackChange({
  type: ChangeType.DELETION,
  path: 'deprecated.feature',
  before: { ... }, // Was this
  after: null,     // Now gone
  timestamp: Date.now()
});
```

## Event Listeners

```typescript
// Listen for version creation
tracker.on('versionCreated', (version) => {
  console.log(`New version: ${version.description}`);
  notifyTeam(`Version ${version.description} deployed`);
});

// Listen for reverts
tracker.on('versionReverted', (from, to) => {
  console.log(`⚠️ Database reverted!`);
  alertOps(`Reverted from ${from} to ${to}`);
});

// Listen for changes
tracker.on('changeTracked', (change) => {
  console.log(`Change: ${change.type} at ${change.path}`);
});
```

## Backup & Restore

```typescript
// Export backup
const backup = tracker.exportBackup();
saveToFile('backup.json', JSON.stringify(backup));

// Restore backup
const backup = JSON.parse(readFromFile('backup.json'));
tracker.importBackup(backup);
```

## Storage Management

```typescript
// Get storage stats
const stats = tracker.getStorageStats();
console.log(`Versions: ${stats.versionCount}`);
console.log(`Size: ${(stats.estimatedSizeBytes / 1024).toFixed(2)} KB`);

// Prune old versions (keep last 10 + important ones)
tracker.pruneVersions(10, ['production', 'baseline']);
```

## Visualization

```typescript
const vizData = tracker.getVisualizationData();

// Timeline
vizData.timeline.forEach(item => {
  console.log(`${item.timestamp}: ${item.description}`);
});

// Hotspots (most changed paths)
vizData.hotspots.forEach(({ path, changeCount }) => {
  console.log(`${path}: ${changeCount} changes`);
});

// Use with D3.js
const graph = vizData.versionGraph;
d3Graph.nodes(graph.nodes).links(graph.edges);
```

## Best Practices

### 1. Use Meaningful Descriptions

```typescript
// ❌ Bad
await tracker.createVersion({ description: 'Update' });

// ✅ Good
await tracker.createVersion({
  description: 'Add email verification to user registration',
  tags: ['feature', 'auth'],
  author: 'developer@company.com'
});
```

### 2. Tag Your Versions

```typescript
// Development
await tracker.createVersion({
  description: 'Work in progress',
  tags: ['dev', 'unstable']
});

// Production
await tracker.createVersion({
  description: 'Stable release v2.0',
  tags: ['production', 'stable', 'v2.0']
});
```

### 3. Create Checkpoints

```typescript
// Before risky operation
const checkpoint = await tracker.createVersion({
  description: 'Pre-migration checkpoint',
  tags: ['checkpoint', 'safe-point']
});

try {
  performRiskyMigration();
} catch (error) {
  await tracker.revertToVersion(checkpoint.id);
}
```

### 4. Prune Regularly

```typescript
// Keep last 50 versions + important ones
setInterval(() => {
  tracker.pruneVersions(50, ['production', 'checkpoint']);
}, 7 * 24 * 60 * 60 * 1000); // Weekly
```

## Complete Example

```typescript
import { TemporalTracker, ChangeType } from 'ruvector-extensions';

async function main() {
  const tracker = new TemporalTracker();

  // Listen for events
  tracker.on('versionCreated', (v) => {
    console.log(`✓ Version ${v.description} created`);
  });

  // Initial schema
  tracker.trackChange({
    type: ChangeType.ADDITION,
    path: 'nodes.User',
    before: null,
    after: { name: 'User', properties: ['id', 'name'] },
    timestamp: Date.now()
  });

  const v1 = await tracker.createVersion({
    description: 'Initial schema',
    tags: ['v1.0']
  });

  // Enhance schema
  tracker.trackChange({
    type: ChangeType.MODIFICATION,
    path: 'nodes.User.properties',
    before: ['id', 'name'],
    after: ['id', 'name', 'email', 'createdAt'],
    timestamp: Date.now()
  });

  const v2 = await tracker.createVersion({
    description: 'Enhanced user fields',
    tags: ['v1.1']
  });

  // Compare changes
  const diff = await tracker.compareVersions(v1.id, v2.id);
  console.log('Changes:', diff.summary);

  // Time-travel
  const stateAtV1 = await tracker.queryAtTimestamp(v1.timestamp);
  console.log('State at v1:', stateAtV1);

  // If needed, revert
  if (somethingWentWrong) {
    await tracker.revertToVersion(v1.id);
  }

  // Backup
  const backup = tracker.exportBackup();
  console.log(`Backed up ${backup.versions.length} versions`);
}

main().catch(console.error);
```

## Next Steps

- Read the [full API documentation](./TEMPORAL.md)
- See [complete examples](../src/examples/temporal-example.ts)
- Check [implementation details](./TEMPORAL_SUMMARY.md)

## Support

- Documentation: https://github.com/ruvnet/ruvector
- Issues: https://github.com/ruvnet/ruvector/issues

---

Happy tracking! 🚀
