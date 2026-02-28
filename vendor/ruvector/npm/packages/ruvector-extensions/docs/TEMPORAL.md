# Temporal Tracking Module

Complete version control and time-travel capabilities for RUVector database evolution.

## Overview

The Temporal Tracking module provides enterprise-grade version management for your vector database, enabling:

- **Version Control**: Create snapshots of database state over time
- **Change Tracking**: Track all modifications with full audit trail
- **Time-Travel Queries**: Query database at any point in history
- **Diff Generation**: Compare versions to see what changed
- **Revert Capability**: Safely rollback to previous states
- **Visualization Data**: Generate timeline and change frequency data
- **Delta Encoding**: Efficient storage using incremental changes
- **Event System**: React to changes with event listeners

## Installation

```bash
npm install ruvector-extensions
```

## Quick Start

```typescript
import { TemporalTracker, ChangeType } from 'ruvector-extensions';

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
const version = await tracker.createVersion({
  description: 'Initial schema',
  tags: ['v1.0', 'production']
});

// Query past state
const pastState = await tracker.queryAtTimestamp(version.timestamp);

// Compare versions
const diff = await tracker.compareVersions(v1.id, v2.id);
```

## Core Concepts

### Change Types

Four types of changes are tracked:

```typescript
enum ChangeType {
  ADDITION = 'addition',       // New entity added
  DELETION = 'deletion',       // Entity removed
  MODIFICATION = 'modification', // Entity changed
  METADATA = 'metadata'        // Metadata updated
}
```

### Path System

Changes are organized by path (dot-notation):

```typescript
'nodes.User'              // User node type
'edges.FOLLOWS'           // FOLLOWS edge type
'config.maxUsers'         // Configuration value
'schema.version'          // Schema version
'nodes.User.properties'   // Nested property
```

### Delta Encoding

Only differences between versions are stored:

```
Baseline (v0): {}
  ↓ + Change 1: Add User node
V1: { nodes: { User: {...} } }
  ↓ + Change 2: Add Post node
V2: { nodes: { User: {...}, Post: {...} } }
```

## API Reference

### TemporalTracker Class

#### Constructor

```typescript
const tracker = new TemporalTracker();
```

Creates a new tracker with a baseline version.

#### trackChange(change: Change): void

Track a change to be included in the next version.

```typescript
tracker.trackChange({
  type: ChangeType.ADDITION,
  path: 'nodes.User',
  before: null,
  after: { name: 'User', properties: ['id', 'name'] },
  timestamp: Date.now(),
  metadata: { author: 'system' } // optional
});
```

**Parameters:**
- `type`: Type of change (ADDITION, DELETION, MODIFICATION, METADATA)
- `path`: Dot-notation path to the changed entity
- `before`: Previous value (null for additions)
- `after`: New value (null for deletions)
- `timestamp`: When the change occurred
- `metadata`: Optional metadata about the change

**Events:** Emits `changeTracked` event

#### createVersion(options: CreateVersionOptions): Promise<Version>

Create a new version with all pending changes.

```typescript
const version = await tracker.createVersion({
  description: 'Added user authentication',
  tags: ['v2.0', 'production'],
  author: 'developer@example.com',
  metadata: { ticket: 'FEAT-123' }
});
```

**Parameters:**
- `description`: Human-readable description (required)
- `tags`: Array of tags for categorization
- `author`: Who created this version
- `metadata`: Additional custom metadata

**Returns:** Version object with ID, timestamp, changes, checksum

**Events:** Emits `versionCreated` event

#### listVersions(tags?: string[]): Version[]

List all versions, optionally filtered by tags.

```typescript
// All versions
const allVersions = tracker.listVersions();

// Only production versions
const prodVersions = tracker.listVersions(['production']);

// Multiple tags (OR logic)
const tagged = tracker.listVersions(['v1.0', 'v2.0']);
```

**Returns:** Array of versions, sorted newest first

#### getVersion(versionId: string): Version | null

Get a specific version by ID.

```typescript
const version = tracker.getVersion('version-id-here');
if (version) {
  console.log(version.description);
  console.log(version.changes.length);
}
```

#### compareVersions(fromId, toId): Promise<VersionDiff>

Generate a diff between two versions.

```typescript
const diff = await tracker.compareVersions(v1.id, v2.id);

console.log('Summary:', diff.summary);
// { additions: 5, deletions: 2, modifications: 3 }

diff.changes.forEach(change => {
  console.log(`${change.type} at ${change.path}`);
  if (change.type === ChangeType.MODIFICATION) {
    console.log(`  Before: ${change.before}`);
    console.log(`  After: ${change.after}`);
  }
});
```

**Returns:** VersionDiff with:
- `fromVersion`: Source version ID
- `toVersion`: Target version ID
- `changes`: Array of changes
- `summary`: Count of additions/deletions/modifications

#### revertToVersion(versionId: string): Promise<Version>

Revert to a previous version (creates new version with inverse changes).

```typescript
// Revert to v1 state
const revertVersion = await tracker.revertToVersion(v1.id);

console.log('Created revert version:', revertVersion.id);
console.log('Description:', revertVersion.description);
// "Revert to version: {original description}"
```

**Important:** This creates a NEW version with inverse changes, preserving history.

**Events:** Emits `versionReverted` event

#### queryAtTimestamp(timestamp | options): Promise<any>

Perform a time-travel query to get database state at a specific point.

```typescript
// Query at specific timestamp
const yesterday = Date.now() - 86400000;
const pastState = await tracker.queryAtTimestamp(yesterday);

// Query at specific version
const stateAtV1 = await tracker.queryAtTimestamp({
  versionId: v1.id
});

// Query with filters
const userNodesOnly = await tracker.queryAtTimestamp({
  timestamp: Date.now(),
  pathPattern: /^nodes\.User/,  // Only User nodes
  includeMetadata: true
});
```

**Options:**
- `timestamp`: Unix timestamp
- `versionId`: Specific version to query
- `pathPattern`: RegExp to filter paths
- `includeMetadata`: Include metadata in results

**Returns:** Reconstructed state object

#### addTags(versionId: string, tags: string[]): void

Add tags to an existing version.

```typescript
tracker.addTags(version.id, ['stable', 'tested', 'production']);
```

Tags are useful for:
- Release marking (`v1.0`, `v2.0`)
- Environment (`production`, `staging`)
- Status (`stable`, `experimental`)
- Features (`auth-enabled`, `new-ui`)

#### getVisualizationData(): VisualizationData

Get data for visualizing change history.

```typescript
const vizData = tracker.getVisualizationData();

// Timeline of all versions
vizData.timeline.forEach(item => {
  console.log(`${new Date(item.timestamp).toISOString()}`);
  console.log(`  ${item.description}`);
  console.log(`  Changes: ${item.changeCount}`);
});

// Change frequency over time
vizData.changeFrequency.forEach(({ timestamp, count, type }) => {
  console.log(`${timestamp}: ${count} ${type} changes`);
});

// Most frequently changed paths
vizData.hotspots.forEach(({ path, changeCount }) => {
  console.log(`${path}: ${changeCount} changes`);
});

// Version graph (for D3.js, vis.js, etc.)
const graph = vizData.versionGraph;
// graph.nodes: [{ id, label, timestamp }]
// graph.edges: [{ from, to }]
```

**Returns:** VisualizationData with:
- `timeline`: Chronological version list
- `changeFrequency`: Changes over time
- `hotspots`: Most modified paths
- `versionGraph`: Parent-child relationships

#### getAuditLog(limit?: number): AuditLogEntry[]

Get audit trail of all operations.

```typescript
const recentLogs = tracker.getAuditLog(50);

recentLogs.forEach(entry => {
  console.log(`[${entry.operation}] ${entry.status}`);
  console.log(`  By: ${entry.actor || 'system'}`);
  console.log(`  Details:`, entry.details);
  if (entry.error) {
    console.log(`  Error: ${entry.error}`);
  }
});
```

**Returns:** Array of audit entries, newest first

#### pruneVersions(keepCount, preserveTags?): void

Delete old versions to save space.

```typescript
// Keep last 10 versions + tagged ones
tracker.pruneVersions(10, ['baseline', 'production', 'stable']);
```

**Parameters:**
- `keepCount`: Number of recent versions to keep
- `preserveTags`: Tags to always preserve

**Safety:** Never deletes versions with dependencies

#### exportBackup(): BackupData

Export all data for backup.

```typescript
const backup = tracker.exportBackup();

// Save to file
import { writeFileSync } from 'fs';
writeFileSync('backup.json', JSON.stringify(backup));

console.log(`Backed up ${backup.versions.length} versions`);
console.log(`Exported at: ${new Date(backup.exportedAt).toISOString()}`);
```

**Returns:**
- `versions`: All version objects
- `auditLog`: Complete audit trail
- `currentState`: Current database state
- `exportedAt`: Export timestamp

#### importBackup(backup: BackupData): void

Import data from backup.

```typescript
import { readFileSync } from 'fs';

const backup = JSON.parse(readFileSync('backup.json', 'utf8'));
tracker.importBackup(backup);

console.log('Backup restored successfully');
```

**Warning:** Clears all existing data before import

#### getStorageStats(): StorageStats

Get storage statistics.

```typescript
const stats = tracker.getStorageStats();

console.log(`Versions: ${stats.versionCount}`);
console.log(`Changes: ${stats.totalChanges}`);
console.log(`Audit entries: ${stats.auditLogSize}`);
console.log(`Estimated size: ${(stats.estimatedSizeBytes / 1024).toFixed(2)} KB`);
console.log(`Date range: ${new Date(stats.oldestVersion).toISOString()} to ${new Date(stats.newestVersion).toISOString()}`);
```

## Event System

The tracker is an EventEmitter with the following events:

### versionCreated

Emitted when a new version is created.

```typescript
tracker.on('versionCreated', (version: Version) => {
  console.log(`New version: ${version.id}`);
  console.log(`Changes: ${version.changes.length}`);

  // Send notification
  notificationService.send(`Version ${version.description} created`);
});
```

### versionReverted

Emitted when reverting to a previous version.

```typescript
tracker.on('versionReverted', (fromVersion: string, toVersion: string) => {
  console.log(`Reverted from ${fromVersion} to ${toVersion}`);

  // Log critical event
  logger.warn('Database reverted', { fromVersion, toVersion });
});
```

### changeTracked

Emitted when a change is tracked.

```typescript
tracker.on('changeTracked', (change: Change) => {
  console.log(`Change: ${change.type} at ${change.path}`);

  // Real-time monitoring
  monitoringService.trackChange(change);
});
```

### auditLogged

Emitted when an audit entry is created.

```typescript
tracker.on('auditLogged', (entry: AuditLogEntry) => {
  console.log(`Audit: ${entry.operation} - ${entry.status}`);

  // Send to external audit system
  auditSystem.log(entry);
});
```

### error

Emitted on errors.

```typescript
tracker.on('error', (error: Error) => {
  console.error('Tracker error:', error);

  // Error handling
  errorService.report(error);
});
```

## Usage Patterns

### Pattern 1: Continuous Development

Track changes as you develop, create versions at milestones.

```typescript
// Development loop
function updateSchema(changes) {
  changes.forEach(change => tracker.trackChange(change));

  if (readyForRelease) {
    await tracker.createVersion({
      description: 'Release v2.1',
      tags: ['v2.1', 'production']
    });
  }
}
```

### Pattern 2: Rollback Safety

Keep production-tagged versions for easy rollback.

```typescript
// Before risky change
const safePoint = await tracker.createVersion({
  description: 'Safe point before migration',
  tags: ['production', 'safe-point']
});

try {
  // Risky operation
  performMigration();
} catch (error) {
  // Rollback on failure
  await tracker.revertToVersion(safePoint.id);
  console.log('Rolled back to safe state');
}
```

### Pattern 3: Change Analysis

Analyze what changed between releases.

```typescript
const prodVersions = tracker.listVersions(['production']);
const [current, previous] = prodVersions; // Newest first

const diff = await tracker.compareVersions(previous.id, current.id);

console.log('Changes in this release:');
console.log(`  Added: ${diff.summary.additions}`);
console.log(`  Modified: ${diff.summary.modifications}`);
console.log(`  Deleted: ${diff.summary.deletions}`);

// Generate changelog
const changelog = diff.changes.map(c =>
  `- ${c.type} ${c.path}`
).join('\n');
```

### Pattern 4: Audit Compliance

Maintain complete audit trail for compliance.

```typescript
// Track all changes with metadata
tracker.trackChange({
  type: ChangeType.MODIFICATION,
  path: 'sensitive.data',
  before: oldValue,
  after: newValue,
  timestamp: Date.now(),
  metadata: {
    user: currentUser.id,
    reason: 'GDPR request',
    ticket: 'LEGAL-456'
  }
});

// Export audit log monthly
const log = tracker.getAuditLog();
const monthlyLog = log.filter(e =>
  e.timestamp >= startOfMonth && e.timestamp < endOfMonth
);

saveAuditReport('audit-2024-01.json', monthlyLog);
```

### Pattern 5: Time-Travel Debugging

Debug issues by examining past states.

```typescript
// Find when bug was introduced
const versions = tracker.listVersions();

for (const version of versions) {
  const state = await tracker.queryAtTimestamp(version.timestamp);

  if (hasBug(state)) {
    console.log(`Bug present in version: ${version.description}`);
  } else {
    console.log(`Bug not present in version: ${version.description}`);

    // Compare with next version to find the change
    const nextVersion = versions[versions.indexOf(version) - 1];
    if (nextVersion) {
      const diff = await tracker.compareVersions(version.id, nextVersion.id);
      console.log('Changes that introduced bug:', diff.changes);
    }
    break;
  }
}
```

## Best Practices

### 1. Meaningful Descriptions

```typescript
// ❌ Bad
await tracker.createVersion({ description: 'Update' });

// ✅ Good
await tracker.createVersion({
  description: 'Add email verification to user registration',
  tags: ['feature', 'auth'],
  metadata: { ticket: 'FEAT-123' }
});
```

### 2. Consistent Tagging

```typescript
// Establish tagging convention
const TAGS = {
  PRODUCTION: 'production',
  STAGING: 'staging',
  FEATURE: 'feature',
  BUGFIX: 'bugfix',
  HOTFIX: 'hotfix'
};

await tracker.createVersion({
  description: 'Fix critical auth bug',
  tags: [TAGS.HOTFIX, TAGS.PRODUCTION, 'v2.1.1']
});
```

### 3. Regular Pruning

```typescript
// Prune monthly
setInterval(() => {
  tracker.pruneVersions(
    50, // Keep last 50 versions
    ['production', 'baseline', 'hotfix'] // Preserve important ones
  );
}, 30 * 24 * 60 * 60 * 1000); // 30 days
```

### 4. Backup Before Major Changes

```typescript
async function majorMigration() {
  // Backup first
  const backup = tracker.exportBackup();
  await saveBackup('pre-migration.json', backup);

  // Create checkpoint
  const checkpoint = await tracker.createVersion({
    description: 'Pre-migration checkpoint',
    tags: ['checkpoint', 'migration']
  });

  // Perform migration
  try {
    await performMigration();
  } catch (error) {
    await tracker.revertToVersion(checkpoint.id);
    throw error;
  }
}
```

### 5. Use Events for Integration

```typescript
// Integrate with monitoring
tracker.on('versionCreated', async (version) => {
  await metrics.increment('versions.created');
  await metrics.gauge('versions.total', tracker.listVersions().length);
});

// Integrate with notifications
tracker.on('versionReverted', async (from, to) => {
  await slack.send(`⚠️ Database reverted from ${from} to ${to}`);
});
```

## Performance Considerations

### Memory Usage

- **In-Memory Storage**: All versions kept in memory
- **Recommendation**: Prune old versions regularly
- **Large Databases**: Consider periodic export/import

### Query Performance

- **Time Complexity**: O(n) where n = version chain length
- **Optimization**: Keep version chains short with pruning
- **Path Filtering**: O(1) lookup with path index

### Storage Size

- **Delta Encoding**: ~70-90% smaller than full snapshots
- **Compression**: Use `exportBackup()` with external compression
- **Estimate**: ~100 bytes per change on average

## TypeScript Support

Full TypeScript definitions included:

```typescript
import type {
  TemporalTracker,
  Change,
  ChangeType,
  Version,
  VersionDiff,
  AuditLogEntry,
  CreateVersionOptions,
  QueryOptions,
  VisualizationData
} from 'ruvector-extensions';
```

## Examples

See `/src/examples/temporal-example.ts` for comprehensive examples covering:
- Basic version management
- Time-travel queries
- Version comparison
- Reverting
- Visualization data
- Audit logging
- Storage management
- Backup/restore
- Event-driven architecture

Run examples:
```bash
npm run build
node dist/examples/temporal-example.js
```

## License

MIT

## Support

- Issues: https://github.com/ruvnet/ruvector/issues
- Documentation: https://github.com/ruvnet/ruvector
