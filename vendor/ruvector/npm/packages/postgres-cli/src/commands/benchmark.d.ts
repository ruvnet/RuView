/**
 * Benchmark Commands
 * CLI commands for performance benchmarking
 */
import type { RuVectorClient } from '../client.js';
export interface BenchmarkRunOptions {
    type: 'vector' | 'attention' | 'gnn' | 'all';
    size: string;
    dim: string;
}
export interface BenchmarkReportOptions {
    format: 'json' | 'table' | 'markdown';
}
export declare class BenchmarkCommands {
    static run(client: RuVectorClient, options: BenchmarkRunOptions): Promise<void>;
    static report(client: RuVectorClient, options: BenchmarkReportOptions): Promise<void>;
    static showInfo(): void;
}
export default BenchmarkCommands;
//# sourceMappingURL=benchmark.d.ts.map