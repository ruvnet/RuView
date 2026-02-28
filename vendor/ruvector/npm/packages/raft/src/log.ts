/**
 * Raft Log Implementation
 * Manages the replicated log with persistence support
 */

import { LogEntry, LogIndex, Term } from './types.js';

/** In-memory log storage with optional persistence callback */
export class RaftLog<T = unknown> {
  private entries: LogEntry<T>[] = [];
  private persistCallback?: (entries: LogEntry<T>[]) => Promise<void>;

  constructor(options?: { onPersist?: (entries: LogEntry<T>[]) => Promise<void> }) {
    this.persistCallback = options?.onPersist;
  }

  /** Get the last log index */
  get lastIndex(): LogIndex {
    return this.entries.length > 0 ? this.entries[this.entries.length - 1].index : 0;
  }

  /** Get the last log term */
  get lastTerm(): Term {
    return this.entries.length > 0 ? this.entries[this.entries.length - 1].term : 0;
  }

  /** Get log length */
  get length(): number {
    return this.entries.length;
  }

  /** Get entry at index */
  get(index: LogIndex): LogEntry<T> | undefined {
    return this.entries.find((e) => e.index === index);
  }

  /** Get term at index */
  termAt(index: LogIndex): Term | undefined {
    if (index === 0) return 0;
    const entry = this.get(index);
    return entry?.term;
  }

  /** Append entries to log */
  async append(entries: LogEntry<T>[]): Promise<void> {
    if (entries.length === 0) return;

    // Find where to start appending (handle conflicting entries)
    for (const entry of entries) {
      const existing = this.get(entry.index);
      if (existing) {
        if (existing.term !== entry.term) {
          // Conflict: delete this and all following entries
          this.truncateFrom(entry.index);
        } else {
          // Same entry, skip
          continue;
        }
      }
      this.entries.push(entry);
    }

    // Sort by index to maintain order
    this.entries.sort((a, b) => a.index - b.index);

    if (this.persistCallback) {
      await this.persistCallback(this.entries);
    }
  }

  /** Append a single command, returning the new entry */
  async appendCommand(term: Term, command: T): Promise<LogEntry<T>> {
    const entry: LogEntry<T> = {
      term,
      index: this.lastIndex + 1,
      command,
      timestamp: Date.now(),
    };
    await this.append([entry]);
    return entry;
  }

  /** Get entries starting from index */
  getFrom(startIndex: LogIndex, maxCount?: number): LogEntry<T>[] {
    const result: LogEntry<T>[] = [];
    for (const entry of this.entries) {
      if (entry.index >= startIndex) {
        result.push(entry);
        if (maxCount && result.length >= maxCount) break;
      }
    }
    return result;
  }

  /** Get entries in range [start, end] */
  getRange(startIndex: LogIndex, endIndex: LogIndex): LogEntry<T>[] {
    return this.entries.filter((e) => e.index >= startIndex && e.index <= endIndex);
  }

  /** Truncate log from index (remove index and all following) */
  truncateFrom(index: LogIndex): void {
    this.entries = this.entries.filter((e) => e.index < index);
  }

  /** Check if log is at least as up-to-date as given term/index */
  isUpToDate(lastLogTerm: Term, lastLogIndex: LogIndex): boolean {
    if (this.lastTerm !== lastLogTerm) {
      return this.lastTerm > lastLogTerm;
    }
    return this.lastIndex >= lastLogIndex;
  }

  /** Check if log contains entry at index with matching term */
  containsEntry(index: LogIndex, term: Term): boolean {
    if (index === 0) return true;
    const entry = this.get(index);
    return entry?.term === term;
  }

  /** Get all entries */
  getAll(): LogEntry<T>[] {
    return [...this.entries];
  }

  /** Clear all entries */
  clear(): void {
    this.entries = [];
  }

  /** Load entries from storage */
  load(entries: LogEntry<T>[]): void {
    this.entries = [...entries];
    this.entries.sort((a, b) => a.index - b.index);
  }
}
