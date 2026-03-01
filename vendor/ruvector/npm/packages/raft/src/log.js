"use strict";
/**
 * Raft Log Implementation
 * Manages the replicated log with persistence support
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.RaftLog = void 0;
/** In-memory log storage with optional persistence callback */
class RaftLog {
    constructor(options) {
        this.entries = [];
        this.persistCallback = options?.onPersist;
    }
    /** Get the last log index */
    get lastIndex() {
        return this.entries.length > 0 ? this.entries[this.entries.length - 1].index : 0;
    }
    /** Get the last log term */
    get lastTerm() {
        return this.entries.length > 0 ? this.entries[this.entries.length - 1].term : 0;
    }
    /** Get log length */
    get length() {
        return this.entries.length;
    }
    /** Get entry at index */
    get(index) {
        return this.entries.find((e) => e.index === index);
    }
    /** Get term at index */
    termAt(index) {
        if (index === 0)
            return 0;
        const entry = this.get(index);
        return entry?.term;
    }
    /** Append entries to log */
    async append(entries) {
        if (entries.length === 0)
            return;
        // Find where to start appending (handle conflicting entries)
        for (const entry of entries) {
            const existing = this.get(entry.index);
            if (existing) {
                if (existing.term !== entry.term) {
                    // Conflict: delete this and all following entries
                    this.truncateFrom(entry.index);
                }
                else {
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
    async appendCommand(term, command) {
        const entry = {
            term,
            index: this.lastIndex + 1,
            command,
            timestamp: Date.now(),
        };
        await this.append([entry]);
        return entry;
    }
    /** Get entries starting from index */
    getFrom(startIndex, maxCount) {
        const result = [];
        for (const entry of this.entries) {
            if (entry.index >= startIndex) {
                result.push(entry);
                if (maxCount && result.length >= maxCount)
                    break;
            }
        }
        return result;
    }
    /** Get entries in range [start, end] */
    getRange(startIndex, endIndex) {
        return this.entries.filter((e) => e.index >= startIndex && e.index <= endIndex);
    }
    /** Truncate log from index (remove index and all following) */
    truncateFrom(index) {
        this.entries = this.entries.filter((e) => e.index < index);
    }
    /** Check if log is at least as up-to-date as given term/index */
    isUpToDate(lastLogTerm, lastLogIndex) {
        if (this.lastTerm !== lastLogTerm) {
            return this.lastTerm > lastLogTerm;
        }
        return this.lastIndex >= lastLogIndex;
    }
    /** Check if log contains entry at index with matching term */
    containsEntry(index, term) {
        if (index === 0)
            return true;
        const entry = this.get(index);
        return entry?.term === term;
    }
    /** Get all entries */
    getAll() {
        return [...this.entries];
    }
    /** Clear all entries */
    clear() {
        this.entries = [];
    }
    /** Load entries from storage */
    load(entries) {
        this.entries = [...entries];
        this.entries.sort((a, b) => a.index - b.index);
    }
}
exports.RaftLog = RaftLog;
//# sourceMappingURL=log.js.map