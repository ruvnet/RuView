/**
 * Export/Serialization for SONA Models
 *
 * Support for SafeTensors, JSON, and other export formats.
 *
 * @example
 * ```typescript
 * import { ModelExporter, SafeTensorsWriter } from '@ruvector/ruvllm';
 *
 * // Export model to SafeTensors format
 * const exporter = new ModelExporter();
 * const buffer = exporter.toSafeTensors({
 *   weights: loraAdapter.getWeights(),
 *   config: loraAdapter.getConfig(),
 * });
 *
 * // Save to file
 * fs.writeFileSync('model.safetensors', buffer);
 * ```
 */
import { LoRAConfig, LearnedPattern, EwcStats, Embedding, ModelMetadata } from './types';
import { LoraWeights } from './lora';
/**
 * Exportable model data
 */
export interface ExportableModel {
    /** Model metadata */
    metadata: ModelMetadata;
    /** LoRA weights (if applicable) */
    loraWeights?: LoraWeights;
    /** LoRA config */
    loraConfig?: LoRAConfig;
    /** Learned patterns */
    patterns?: LearnedPattern[];
    /** EWC statistics */
    ewcStats?: EwcStats;
    /** Raw tensors */
    tensors?: Map<string, Float32Array>;
}
/**
 * SafeTensors Writer
 *
 * Writes tensors in SafeTensors format for compatibility with
 * HuggingFace ecosystem.
 */
export declare class SafeTensorsWriter {
    private tensors;
    private metadata;
    /**
     * Add a tensor
     */
    addTensor(name: string, data: Float32Array, shape: number[]): this;
    /**
     * Add 2D tensor from number array
     */
    add2D(name: string, data: number[][]): this;
    /**
     * Add 1D tensor from number array
     */
    add1D(name: string, data: number[]): this;
    /**
     * Add metadata
     */
    addMetadata(key: string, value: string): this;
    /**
     * Build SafeTensors buffer
     */
    build(): Uint8Array;
    /**
     * Clear all tensors and metadata
     */
    clear(): void;
}
/**
 * SafeTensors Reader
 *
 * Reads tensors from SafeTensors format.
 */
export declare class SafeTensorsReader {
    private buffer;
    private header;
    private dataOffset;
    constructor(buffer: Uint8Array);
    /**
     * Get tensor names
     */
    getTensorNames(): string[];
    /**
     * Get tensor by name
     */
    getTensor(name: string): {
        data: Float32Array;
        shape: number[];
    } | null;
    /**
     * Get tensor as 2D array
     */
    getTensor2D(name: string): number[][] | null;
    /**
     * Get tensor as 1D array
     */
    getTensor1D(name: string): number[] | null;
    /**
     * Get metadata
     */
    getMetadata(): Record<string, string>;
    private parseHeader;
}
/**
 * Model Exporter
 *
 * Unified export interface for SONA models.
 */
export declare class ModelExporter {
    /**
     * Export to SafeTensors format
     */
    toSafeTensors(model: ExportableModel): Uint8Array;
    /**
     * Export to JSON format
     */
    toJSON(model: ExportableModel): string;
    /**
     * Export to compact binary format
     */
    toBinary(model: ExportableModel): Uint8Array;
    /**
     * Export for HuggingFace Hub compatibility
     */
    toHuggingFace(model: ExportableModel): {
        safetensors: Uint8Array;
        config: string;
        readme: string;
    };
}
/**
 * Model Importer
 *
 * Import models from various formats.
 */
export declare class ModelImporter {
    /**
     * Import from SafeTensors format
     */
    fromSafeTensors(buffer: Uint8Array): Partial<ExportableModel>;
    /**
     * Import from JSON format
     */
    fromJSON(json: string): Partial<ExportableModel>;
    /**
     * Import from binary format
     */
    fromBinary(buffer: Uint8Array): Partial<ExportableModel>;
}
/**
 * Dataset Exporter
 *
 * Export training data in various formats.
 */
export declare class DatasetExporter {
    /**
     * Export to JSONL format (one JSON per line)
     */
    toJSONL(data: Array<{
        input: Embedding;
        output: Embedding;
        quality: number;
    }>): string;
    /**
     * Export to CSV format
     */
    toCSV(data: Array<{
        input: Embedding;
        output: Embedding;
        quality: number;
    }>): string;
    /**
     * Export patterns for pre-training
     */
    toPretrain(patterns: LearnedPattern[]): string;
}
//# sourceMappingURL=export.d.ts.map