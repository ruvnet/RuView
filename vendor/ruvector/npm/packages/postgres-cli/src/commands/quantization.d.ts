/**
 * Quantization Commands
 * CLI commands for vector quantization operations (binary, scalar, product)
 */
import type { RuVectorClient } from '../client.js';
export interface BinaryQuantizeOptions {
    vector: string;
}
export interface ScalarQuantizeOptions {
    vector: string;
}
export interface QuantizedSearchOptions {
    table: string;
    query: string;
    topK?: string;
    quantType?: 'binary' | 'scalar';
}
export declare class QuantizationCommands {
    static binaryQuantize(client: RuVectorClient, options: BinaryQuantizeOptions): Promise<void>;
    static scalarQuantize(client: RuVectorClient, options: ScalarQuantizeOptions): Promise<void>;
    static stats(client: RuVectorClient): Promise<void>;
    static compare(client: RuVectorClient, vector: string): Promise<void>;
    static showHelp(): void;
}
export default QuantizationCommands;
//# sourceMappingURL=quantization.d.ts.map