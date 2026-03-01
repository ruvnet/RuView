/**
 * Attention Commands
 * CLI commands for attention mechanism operations
 */
import type { RuVectorClient } from '../client.js';
export interface AttentionComputeOptions {
    query: string;
    keys: string;
    values: string;
    type: 'scaled_dot' | 'multi_head' | 'flash';
}
export declare class AttentionCommands {
    static compute(client: RuVectorClient, options: AttentionComputeOptions): Promise<void>;
    static listTypes(client: RuVectorClient): Promise<void>;
}
export default AttentionCommands;
//# sourceMappingURL=attention.d.ts.map