/**
 * Raft Log Implementation
 * Manages the replicated log with persistence support
 */
import { LogEntry, LogIndex, Term } from './types.js';
/** In-memory log storage with optional persistence callback */
export declare class RaftLog<T = unknown> {
    private entries;
    private persistCallback?;
    constructor(options?: {
        onPersist?: (entries: LogEntry<T>[]) => Promise<void>;
    });
    /** Get the last log index */
    get lastIndex(): LogIndex;
    /** Get the last log term */
    get lastTerm(): Term;
    /** Get log length */
    get length(): number;
    /** Get entry at index */
    get(index: LogIndex): LogEntry<T> | undefined;
    /** Get term at index */
    termAt(index: LogIndex): Term | undefined;
    /** Append entries to log */
    append(entries: LogEntry<T>[]): Promise<void>;
    /** Append a single command, returning the new entry */
    appendCommand(term: Term, command: T): Promise<LogEntry<T>>;
    /** Get entries starting from index */
    getFrom(startIndex: LogIndex, maxCount?: number): LogEntry<T>[];
    /** Get entries in range [start, end] */
    getRange(startIndex: LogIndex, endIndex: LogIndex): LogEntry<T>[];
    /** Truncate log from index (remove index and all following) */
    truncateFrom(index: LogIndex): void;
    /** Check if log is at least as up-to-date as given term/index */
    isUpToDate(lastLogTerm: Term, lastLogIndex: LogIndex): boolean;
    /** Check if log contains entry at index with matching term */
    containsEntry(index: LogIndex, term: Term): boolean;
    /** Get all entries */
    getAll(): LogEntry<T>[];
    /** Clear all entries */
    clear(): void;
    /** Load entries from storage */
    load(entries: LogEntry<T>[]): void;
}
//# sourceMappingURL=log.d.ts.map