# RVF Concurrency, Versioning, and Space Reclamation

## 1. Single-Writer / Multi-Reader Model

RVF uses a **single-writer, multi-reader** concurrency model. At most one process
may append segments to an RVF file at any time. Any number of readers may operate
concurrently with each other and with the writer. This model is enforced by an
advisory lock file, not by OS-level mandatory locking.

| Concern | Advisory Lock | Mandatory Lock (flock/fcntl) |
|---------|---------------|------------------------------|
| NFS compatibility | Works (lock file is a regular file) | Broken on many NFS configs |
| Crash recovery | Stale lock detectable by PID check | Kernel auto-releases, but only locally |
| Cross-language | Any language can create a file | Requires OS-specific syscalls |
| Visibility | Lock state inspectable by humans | Opaque kernel state |
| Multi-file mode | One lock covers all shards | Would need per-shard locks |

## 2. Writer Lock File

The writer lock is a file named `<basename>.rvf.lock` in the same directory as the
RVF file. For example, `data.rvf` uses `data.rvf.lock`.

### Binary Layout

```
Offset  Size  Field              Description
------  ----  -----              -----------
0x00    4     magic              0x52564C46 ("RVLF" in ASCII)
0x04    4     pid                Writer process ID (u32)
0x08    64    hostname           Null-terminated hostname (max 63 chars + null)
0x48    8     timestamp_ns       Lock acquisition time (nanosecond UNIX timestamp)
0x50    16    writer_id          Random UUID (128-bit, written as raw bytes)
0x60    4     lock_version       Lock protocol version (currently 1)
0x64    4     checksum           CRC32C of bytes 0x00-0x63
```

**Total**: 104 bytes.

### Lock Acquisition Protocol

```
1. Construct lock file content (magic, PID, hostname, timestamp, random UUID)
2. Compute CRC32C over bytes 0x00-0x63, store at 0x64
3. Attempt open("<basename>.rvf.lock", O_CREAT | O_EXCL | O_WRONLY)
4. If open succeeds:
   a. Write 104 bytes
   b. fsync
   c. Lock acquired — proceed with writes
5. If open fails (EEXIST):
   a. Read existing lock file
   b. Validate magic and checksum
   c. If invalid: delete stale lock, retry from step 3
   d. If valid: run stale lock detection (see below)
   e. If stale: delete lock, retry from step 3
   f. If not stale: lock acquisition fails — another writer is active
```

The `O_CREAT | O_EXCL` combination is atomic on POSIX filesystems, preventing
two processes from simultaneously creating the lock.

### Stale Lock Detection

A lock is considered stale when **both** of the following are true:

1. **PID is dead**: `kill(pid, 0)` returns `ESRCH` (process does not exist), OR
   the hostname does not match the current host (remote crash)
2. **Age exceeds threshold**: `now_ns - timestamp_ns > 30_000_000_000` (30 seconds)

The age check prevents a race where a PID is recycled by the OS. A lock younger
than 30 seconds is never considered stale, even if the PID appears dead, because
PID reuse on modern systems can occur within milliseconds.

If the hostname differs from the current host, the PID check is not meaningful.
In this case, only the age threshold applies. Implementations SHOULD use a longer
threshold (300 seconds) for cross-host lock recovery to account for clock skew.

### Lock Release Protocol

```
1. fsync all pending data and manifest segments
2. Verify the lock file still contains our writer_id (re-read and compare)
3. If writer_id matches: unlink("<basename>.rvf.lock")
4. If writer_id does not match: abort — another process stole the lock
```

Step 2 prevents a writer from deleting a lock that was legitimately taken over
after a stale lock recovery by another process.

If a writer crashes without releasing the lock, the lock file persists on disk.
The next writer detects the orphan via stale lock detection and reclaims it.
No data corruption occurs because the append-only segment model guarantees that
partial writes are detectable: a segment with a bad content hash or a truncated
manifest is simply ignored.

## 3. Reader-Writer Coordination

Readers and writers operate independently. The append-only architecture ensures
they never conflict.

### Reader Protocol

```
1. Open file (read-only, no lock required)
2. Read Level 0 root manifest (last 4096 bytes)
3. Parse hotset pointers and Level 1 offset
4. This manifest snapshot defines the reader's view of the file
5. All queries within this session use the snapshot
6. To see new data: re-read Level 0 (explicit refresh)
```

### Writer Protocol

```
1. Acquire lock (Section 2)
2. Read current manifest to learn segment directory state
3. Append new segments (VEC_SEG, INDEX_SEG, etc.)
4. Append new MANIFEST_SEG referencing all live segments
5. fsync
6. Release lock (Section 2)
```

### Concurrent Timeline

```
Time    Writer                          Reader A            Reader B
----    ------                          --------            --------
t=0     Acquires lock
t=1     Appends VEC_SEG_4                                   Opens file
t=2     Appends VEC_SEG_5               Opens file          Reads manifest M3
t=3     Appends MANIFEST_SEG M4         Reads manifest M3   Queries (sees M3)
t=4     fsync, releases lock            Queries (sees M3)   Queries (sees M3)
t=5                                     Queries (sees M3)   Refreshes -> M4
t=6                                     Refreshes -> M4     Queries (sees M4)
```

Reader A opened during the write but read manifest M3 (already stable) and never
sees partially written segments. Reader B sees M3 until explicit refresh. Neither
reader is blocked; the writer is never blocked by readers.

### Snapshot Isolation Guarantees

A reader holding a manifest snapshot is guaranteed:

1. All referenced segments are fully written and fsynced
2. Segment content hashes match (the manifest would not reference broken segments)
3. The snapshot is internally consistent (no partial epoch states)
4. The snapshot remains valid for the lifetime of the open file descriptor, even
   if the file is compacted and replaced (old inode persists until close)

## 4. Format Versioning

RVF uses explicit version fields at every structural level. The versioning rules
are designed for forward compatibility — older readers can safely process files
produced by newer writers, with graceful degradation.

### Segment Version Compatibility

The segment header `version` field (offset 0x04, currently `1`) governs
segment-level compatibility.

| Rule | Description |
|------|-------------|
| S1 | A v1 reader MUST successfully process all v1 segments |
| S2 | A v1 reader MUST skip segments with version > 1 |
| S3 | A v1 reader MUST log a warning when skipping unknown versions |
| S4 | A v1 reader MUST NOT reject a file because it contains unknown-version segments |
| S5 | A v2+ writer MUST write a root manifest readable by v1 readers (if the root manifest format allows it) |
| S6 | A v2+ writer MAY write segments with version > 1 |
| S7 | Readers MUST use `payload_length` from the segment header to skip unknown segments |

Skipping works because the segment header layout is stable: magic, version,
seg_type, and payload_length occupy fixed offsets. A reader skips unknown
segments by seeking past `64 + payload_length` bytes (header + payload).

### Unknown Segment Types

The segment type enum (offset 0x05) may be extended in future versions.

| Rule | Description |
|------|-------------|
| T1 | A reader MUST skip segment types outside the recognized range (currently 0x01-0x0C) |
| T2 | A reader MUST NOT reject a file because of unknown segment types |
| T3 | A reader MUST use the header's `payload_length` to skip the unknown segment |
| T4 | A reader SHOULD log unknown types at diagnostic/debug level |
| T5 | Types 0x00 and 0xF0-0xFF remain reserved (see spec 01, Section 3) |

### Level 1 TLV Forward Compatibility

Level 1 manifest records use tag-length-value encoding. New tags may be added
in any version.

| Rule | Description |
|------|-------------|
| L1 | A reader MUST skip TLV records with unknown tags |
| L2 | A reader MUST use the record's `length` field (4 bytes at tag offset +2) to skip |
| L3 | A writer MUST NOT change the semantics of an existing tag |
| L4 | A writer MUST NOT reuse a tag value for a different purpose |
| L5 | New tags MUST be assigned sequentially from 0x000E onward |

### Root Manifest Compatibility

The root manifest (Level 0) has the strictest compatibility requirements because
it is the entry point for all readers.

| Rule | Description |
|------|-------------|
| R1 | The magic `0x52564D30` at offset 0x000 is frozen forever |
| R2 | The layout of bytes 0x000-0x007 (magic + version + flags) is frozen forever |
| R3 | New fields may be added to reserved space at offsets 0xF00-0xFFB |
| R4 | Readers MUST ignore non-zero bytes in reserved space they do not understand |
| R5 | The root checksum at 0xFFC always covers bytes 0x000-0xFFB |
| R6 | A v2+ writer extending reserved space MUST ensure the checksum remains valid |

There is no explicit version negotiation. Compatibility is achieved through the
skip rules above. A reader processes what it understands and skips what it does
not. This avoids capability exchange, making RVF suitable for offline and
archival use cases.

## 5. Variable Dimension Support

The root manifest declares a `dimension` field (offset 0x020, u16) and each
VEC_SEG block declares its own `dim` field (block header offset 0x08, u16).
These may differ.

### Dimension Rules

| Rule | Description |
|------|-------------|
| D1 | The root manifest `dimension` is the **primary dimension** (most common in the file) |
| D2 | An RVF file MAY contain VEC_SEG blocks with dimensions different from the primary |
| D3 | Each VEC_SEG block's `dim` field is authoritative for the vectors in that block |
| D4 | The HNSW index (INDEX_SEG) covers only vectors matching the primary dimension |
| D5 | Vectors with non-primary dimensions are searchable via flat scan or a separate index |
| D6 | A PROFILE_SEG may declare multiple expected dimensions |

### Dimension Catalog (Level 1 Record)

A new Level 1 TLV record (tag `0x0010`, DIMENSION_CATALOG) enables readers to
discover all dimensions present without scanning every VEC_SEG.

Record layout:

```
Offset  Size  Field                Description
------  ----  -----                -----------
0x00    2     entry_count          Number of dimension entries
0x02    2     reserved             Must be zero
```

Followed by `entry_count` entries of:

```
Offset  Size  Field                Description
------  ----  -----                -----------
0x00    2     dimension            Vector dimensionality
0x02    1     dtype                Data type enum for these vectors
0x03    1     flags                0x01 = primary, 0x02 = has_index
0x04    4     vector_count         Number of vectors with this dimension
0x08    8     index_seg_offset     Offset to dedicated index (0 if none)
```

**Entry size**: 16 bytes.

Example for an RVDNA profile file:

```
DIMENSION_CATALOG:
  entry_count: 3
  [0] dim=64,   dtype=f16, flags=0x01 (primary, has_index), count=10000000, index=0x1A00000
  [1] dim=384,  dtype=f16, flags=0x02 (has_index),          count=500000,   index=0x3F00000
  [2] dim=4096, dtype=f32, flags=0x00 (flat scan only),     count=10000,    index=0
```

## 6. Space Reclamation

Over time, tombstoned segments and superseded manifests accumulate dead space.
RVF provides three reclamation strategies, each suited to different operating
conditions.

### Strategy 1: Hole-Punching

On Linux filesystems that support `fallocate(2)` with `FALLOC_FL_PUNCH_HOLE`
(ext4, XFS, btrfs), tombstoned segment ranges can be released back to the
filesystem without rewriting the file.

```
Before:  [VEC_1 live] [VEC_2 dead] [VEC_3 dead] [VEC_4 live] [MANIFEST]
After:   [VEC_1 live] [  hole   ] [  hole   ] [VEC_4 live] [MANIFEST]
```

File size is unchanged but disk blocks are freed. No data movement occurs — each
punch is O(1). Reader mmap still works (holes read as zeros, but the manifest
never references them). Hole-punching is performed only on segments marked as
TOMBSTONE in the current manifest's COMPACTION_STATE record.

### Strategy 2: Copy-Compact

Copy-compact rewrites the file, including only live segments. This is the
universal strategy that works on all filesystems.

```
Protocol:
1. Acquire writer lock
2. Read current manifest to enumerate live segments
3. Create temporary file: <basename>.rvf.compact.tmp
4. Write live segments sequentially to temporary file
5. Write new MANIFEST_SEG with updated offsets
6. fsync temporary file
7. Atomic rename: <basename>.rvf.compact.tmp -> <basename>.rvf
8. Release writer lock
```

The atomic rename (step 7) ensures readers either see the old file or the new
file, never a partial state. Readers that opened the old file before the rename
continue operating on the old inode via their open file descriptor. The old
inode is freed when the last reader closes its descriptor.

### Strategy 3: Shard Rewrite (Multi-File Mode)

In multi-file mode, individual shard files can be rewritten independently:

```
Protocol:
1. Acquire writer lock
2. Read shard reference from Level 1 SHARD_REFS record
3. Write new shard: <basename>.rvf.cold.<N>.compact.tmp
4. fsync new shard
5. Update main file manifest with new shard reference
6. fsync main file
7. Atomic rename new shard over old shard
8. Release writer lock
```

The old shard is safe to delete after all readers close their descriptors.
Implementations MAY defer deletion using a grace period (default: 60 seconds).

## 7. Space Reclamation Triggers

Reclamation is not performed on every write. Implementations SHOULD evaluate
triggers after each manifest write and act when thresholds are exceeded.

| Trigger | Threshold | Action |
|---------|-----------|--------|
| Dead space ratio | > 50% of file size | Copy-compact |
| Dead space absolute | > 1 GB | Hole-punch if supported, else copy-compact |
| Tombstone count | > 10,000 JOURNAL_SEG tombstone entries | Consolidate journal segments |
| Time since last compaction | > 7 days | Evaluate dead space ratio, compact if > 25% |

### Dead Space Calculation

Dead space is computed from the manifest's COMPACTION_STATE record:

```
dead_bytes = sum(payload_length + 64) for each tombstoned segment
total_bytes = file_size
dead_ratio = dead_bytes / total_bytes
```

The `+ 64` accounts for the segment header.

### Trigger Evaluation Protocol

```
1. After writing a new MANIFEST_SEG, compute dead_bytes and dead_ratio
2. If dead_ratio > 0.50: schedule copy-compact
3. Else if dead_bytes > 1 GB:
   a. If fallocate supported: hole-punch tombstoned ranges
   b. Else: schedule copy-compact
4. If tombstone_count > 10,000: consolidate JOURNAL_SEGs
5. If days_since_last_compact > 7 AND dead_ratio > 0.25: schedule copy-compact
```

Scheduled compactions MAY be deferred to a background process or low-activity
period.

## 8. Multi-Process Compaction

Compaction is a write operation and requires the writer lock. Only one process
may compact at a time.

### Background Compaction Process

A dedicated compaction process can run alongside the application:

```
1. Attempt writer lock acquisition
2. If lock acquired:
   a. Read current manifest
   b. Evaluate reclamation triggers
   c. If compaction needed:
      i.   Write WITNESS_SEG with compaction_state = STARTED
      ii.  Perform compaction (copy-compact or hole-punch)
      iii. Write WITNESS_SEG with compaction_state = COMPLETED
      iv.  Write new MANIFEST_SEG
   d. Release lock
3. If lock not acquired: sleep and retry
```

### Crash Safety

Compaction is crash-safe by construction. Copy-compact does not rename until
fsynced — a crash before rename leaves the original file untouched and the
temporary file is cleaned up on next startup. Hole-punch `fallocate` calls are
individually atomic; a crash mid-sequence leaves the manifest consistent because
it references only live segments. Shard rewrite follows the same atomic rename
pattern as copy-compact.

### Compaction Progress and Resumability

For long-running compactions, the writer records progress in WITNESS_SEG segments:

```
WITNESS_SEG compaction payload:
  Offset  Size  Field                Description
  ------  ----  -----                -----------
  0x00    4     state                0=STARTED, 1=IN_PROGRESS, 2=COMPLETED, 3=ABORTED
  0x04    8     source_manifest_id   Segment ID of manifest being compacted
  0x0C    8     last_copied_seg_id   Last segment ID successfully written to new file
  0x14    8     bytes_written        Total bytes written to new file so far
  0x1C    8     bytes_remaining      Estimated bytes remaining
  0x24    16    temp_file_hash       Hash of temporary file at last checkpoint
```

If a compaction process crashes and restarts, it can:

1. Find the latest WITNESS_SEG with `state = IN_PROGRESS`
2. Verify the temporary file exists and matches `temp_file_hash`
3. Resume from `last_copied_seg_id + 1`
4. If verification fails, delete the temporary file and restart compaction

## 9. Crash Recovery Summary

RVF recovers from crashes at any point without external tooling.

| Crash Point | State After Recovery | Action Required |
|-------------|---------------------|-----------------|
| Segment append (before manifest) | Orphan segment at tail | None — manifest does not reference it |
| Manifest write | Partial manifest at tail | Scan backward to previous valid manifest |
| Lock acquisition | Lock file may or may not exist | Stale lock detection resolves it |
| Lock release | Lock file persists | Stale lock detection resolves it |
| Copy-compact (before rename) | Temporary file on disk | Delete `*.compact.tmp` on startup |
| Copy-compact (during rename) | Atomic — old or new | No action needed |
| Hole-punch | Partial holes punched | No action — manifest is consistent |
| Shard rewrite | Temporary shard on disk | Delete `*.compact.tmp` on startup |

### Startup Recovery Protocol

On startup, before acquiring a write lock, a writer SHOULD:

```
1. Delete any <basename>.rvf.compact.tmp files (orphaned compaction)
2. Delete any <basename>.rvf.cold.*.compact.tmp files (orphaned shard compaction)
3. Validate the lock file (if present) for staleness
4. Open the RVF file and locate the latest valid manifest
5. If the tail contains a partial segment (magic present, bad hash):
   a. Log a warning with the partial segment's offset and type
   b. The partial segment is outside the manifest — it is harmless
   c. The next append will overwrite it (or it will be compacted away)
```

## 10. Invariants

The following invariants extend those in spec 01 (Section 7):

1. At most one writer lock exists per RVF file at any time
2. A lock file with valid magic and checksum represents an active or stale lock
3. Readers never require a lock, regardless of operation
4. A manifest snapshot is immutable for the lifetime of a reader session
5. Compaction never modifies live segments — it creates new ones
6. Hole-punched regions are never referenced by any manifest
7. The root manifest magic and first 8 bytes are frozen across all versions
8. Unknown segment versions and types are skipped, never rejected
9. Unknown TLV tags in Level 1 are skipped, never rejected
10. Each VEC_SEG block's `dim` field is authoritative for that block's vectors
