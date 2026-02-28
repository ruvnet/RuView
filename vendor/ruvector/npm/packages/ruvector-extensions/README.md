# ruvector-extensions

Advanced persistence and extension features for the [ruvector](https://github.com/ruvnet/ruvector) vector database.

## Features

- 💾 **Multiple Persistence Formats**: JSON, Binary (MessagePack), SQLite
- 📸 **Snapshot Management**: Create, list, restore, and delete database snapshots
- 🔄 **Incremental Saves**: Save only changed data for efficiency
- 📤 **Export/Import**: Flexible data portability
- 🗜️ **Compression Support**: Gzip and Brotli compression for smaller files
- 📊 **Progress Tracking**: Real-time progress callbacks for large operations
- ⚡ **Auto-Save**: Configurable automatic saves
- 🔒 **Data Integrity**: Built-in checksum verification

## Installation

```bash
npm install ruvector-extensions ruvector
```

## Quick Start

```typescript
import { VectorDB } from 'ruvector';
import { DatabasePersistence } from 'ruvector-extensions';

// Create a vector database
const db = new VectorDB({ dimension: 384 });

// Add vectors
db.insert({
  id: 'doc1',
  vector: [0.1, 0.2, ...], // 384-dimensional vector
  metadata: { title: 'My Document' }
});

// Create persistence manager
const persistence = new DatabasePersistence(db, {
  baseDir: './data',
  format: 'json',
  compression: 'gzip',
  autoSaveInterval: 60000, // Auto-save every minute
});

// Save database
await persistence.save({
  onProgress: (p) => console.log(`${p.percentage}% - ${p.message}`)
});

// Create snapshot
const snapshot = await persistence.createSnapshot('backup-v1');

// Later: restore from snapshot
await persistence.restoreSnapshot(snapshot.id);
```

## API Documentation

### DatabasePersistence

Main class for managing database persistence.

#### Constructor

```typescript
new DatabasePersistence(db: VectorDB, options: PersistenceOptions)
```

**Options:**
- `baseDir` (string): Base directory for persistence files
- `format` (string): Default format - 'json', 'binary', or 'sqlite'
- `compression` (string): Compression type - 'none', 'gzip', or 'brotli'
- `incremental` (boolean): Enable incremental saves
- `autoSaveInterval` (number): Auto-save interval in ms (0 = disabled)
- `maxSnapshots` (number): Maximum snapshots to keep
- `batchSize` (number): Batch size for large operations

#### Save Operations

**save(options?): Promise&lt;string&gt;**

Save the entire database to disk.

```typescript
await persistence.save({
  path: './backup.json.gz',
  format: 'json',
  compress: true,
  onProgress: (p) => console.log(p.message)
});
```

**saveIncremental(options?): Promise&lt;string | null&gt;**

Save only changed data (returns null if no changes).

```typescript
const path = await persistence.saveIncremental();
if (path) {
  console.log('Changes saved to:', path);
}
```

**load(options): Promise&lt;void&gt;**

Load database from disk.

```typescript
await persistence.load({
  path: './backup.json.gz',
  verifyChecksum: true,
  onProgress: (p) => console.log(p.message)
});
```

#### Snapshot Management

**createSnapshot(name, metadata?): Promise&lt;SnapshotMetadata&gt;**

Create a named snapshot of the current database state.

```typescript
const snapshot = await persistence.createSnapshot('pre-migration', {
  version: '2.0',
  user: 'admin'
});

console.log(`Created snapshot ${snapshot.id}`);
console.log(`Size: ${formatFileSize(snapshot.fileSize)}`);
```

**listSnapshots(): Promise&lt;SnapshotMetadata[]&gt;**

List all available snapshots (sorted newest first).

```typescript
const snapshots = await persistence.listSnapshots();
for (const snap of snapshots) {
  console.log(`${snap.name}: ${snap.vectorCount} vectors`);
}
```

**restoreSnapshot(id, options?): Promise&lt;void&gt;**

Restore database from a snapshot.

```typescript
await persistence.restoreSnapshot(snapshot.id, {
  verifyChecksum: true,
  onProgress: (p) => console.log(p.message)
});
```

**deleteSnapshot(id): Promise&lt;void&gt;**

Delete a snapshot.

```typescript
await persistence.deleteSnapshot(oldSnapshotId);
```

#### Export/Import

**export(options): Promise&lt;void&gt;**

Export database to a file.

```typescript
await persistence.export({
  path: './export/database.json',
  format: 'json',
  compress: true,
  includeIndex: false,
  onProgress: (p) => console.log(p.message)
});
```

**import(options): Promise&lt;void&gt;**

Import database from a file.

```typescript
await persistence.import({
  path: './export/database.json',
  clear: true, // Clear existing data first
  verifyChecksum: true,
  onProgress: (p) => console.log(p.message)
});
```

#### Auto-Save

**startAutoSave(): void**

Start automatic saves at configured interval.

```typescript
persistence.startAutoSave();
```

**stopAutoSave(): void**

Stop automatic saves.

```typescript
persistence.stopAutoSave();
```

**shutdown(): Promise&lt;void&gt;**

Cleanup and perform final save.

```typescript
await persistence.shutdown();
```

### Utility Functions

**formatFileSize(bytes): string**

Format bytes as human-readable size.

```typescript
console.log(formatFileSize(1536000)); // "1.46 MB"
```

**formatTimestamp(timestamp): string**

Format Unix timestamp as ISO string.

```typescript
console.log(formatTimestamp(Date.now())); // "2024-01-15T10:30:00.000Z"
```

**estimateMemoryUsage(state): number**

Estimate memory usage of a database state.

```typescript
const usage = estimateMemoryUsage(state);
console.log(`Estimated: ${formatFileSize(usage)}`);
```

## Examples

### Example 1: Basic Persistence

```typescript
import { VectorDB } from 'ruvector';
import { DatabasePersistence } from 'ruvector-extensions';

const db = new VectorDB({ dimension: 128 });

// Add data
for (let i = 0; i < 1000; i++) {
  db.insert({
    id: `doc-${i}`,
    vector: Array(128).fill(0).map(() => Math.random())
  });
}

// Save
const persistence = new DatabasePersistence(db, {
  baseDir: './data'
});

await persistence.save();
console.log('Database saved!');
```

### Example 2: Snapshot Workflow

```typescript
// Create initial snapshot
const v1 = await persistence.createSnapshot('version-1.0');

// Make changes
db.insert({ id: 'new-doc', vector: [...] });

// Create new snapshot
const v2 = await persistence.createSnapshot('version-1.1');

// Rollback to v1 if needed
await persistence.restoreSnapshot(v1.id);
```

### Example 3: Export/Import

```typescript
// Export to JSON
await persistence.export({
  path: './backup.json',
  format: 'json',
  compress: false
});

// Import into new database
const db2 = new VectorDB({ dimension: 128 });
const p2 = new DatabasePersistence(db2, { baseDir: './data2' });

await p2.import({
  path: './backup.json',
  verifyChecksum: true
});
```

### Example 4: Progress Tracking

```typescript
await persistence.save({
  onProgress: (progress) => {
    console.log(`[${progress.percentage}%] ${progress.message}`);
    console.log(`${progress.current}/${progress.total} items`);
  }
});
```

### Example 5: Auto-Save

```typescript
const persistence = new DatabasePersistence(db, {
  baseDir: './data',
  autoSaveInterval: 300000, // Save every 5 minutes
  incremental: true
});

// Auto-save runs automatically
// Stop when done
await persistence.shutdown();
```

## TypeScript Support

Full TypeScript definitions are included:

```typescript
import type {
  PersistenceOptions,
  SnapshotMetadata,
  DatabaseState,
  ProgressCallback,
  ExportOptions,
  ImportOptions
} from 'ruvector-extensions';
```

## Performance Tips

1. **Use Binary Format**: Faster than JSON for large databases
2. **Enable Compression**: Reduces storage size by 70-90%
3. **Incremental Saves**: Much faster for small changes
4. **Batch Size**: Adjust `batchSize` for optimal performance
5. **Auto-Save**: Use reasonable intervals (5-10 minutes)

## Error Handling

All async methods may throw errors:

```typescript
try {
  await persistence.save();
} catch (error) {
  if (error.code === 'ENOSPC') {
    console.error('Not enough disk space');
  } else if (error.message.includes('checksum')) {
    console.error('Data corruption detected');
  } else {
    console.error('Save failed:', error.message);
  }
}
```

## License

MIT - See [LICENSE](LICENSE) for details

## Contributing

Contributions welcome! Please see the main [ruvector repository](https://github.com/ruvnet/ruvector) for contribution guidelines.

## Support

- Documentation: https://github.com/ruvnet/ruvector
- Issues: https://github.com/ruvnet/ruvector/issues
- Discord: [Join our community](https://discord.gg/ruvector)
