"use strict";
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
Object.defineProperty(exports, "__esModule", { value: true });
exports.DatasetExporter = exports.ModelImporter = exports.ModelExporter = exports.SafeTensorsReader = exports.SafeTensorsWriter = void 0;
/**
 * SafeTensors Writer
 *
 * Writes tensors in SafeTensors format for compatibility with
 * HuggingFace ecosystem.
 */
class SafeTensorsWriter {
    constructor() {
        this.tensors = new Map();
        this.metadata = {};
    }
    /**
     * Add a tensor
     */
    addTensor(name, data, shape) {
        this.tensors.set(name, { data, shape });
        return this;
    }
    /**
     * Add 2D tensor from number array
     */
    add2D(name, data) {
        const rows = data.length;
        const cols = data[0]?.length || 0;
        const flat = new Float32Array(rows * cols);
        for (let i = 0; i < rows; i++) {
            for (let j = 0; j < cols; j++) {
                flat[i * cols + j] = data[i][j];
            }
        }
        return this.addTensor(name, flat, [rows, cols]);
    }
    /**
     * Add 1D tensor from number array
     */
    add1D(name, data) {
        return this.addTensor(name, new Float32Array(data), [data.length]);
    }
    /**
     * Add metadata
     */
    addMetadata(key, value) {
        this.metadata[key] = value;
        return this;
    }
    /**
     * Build SafeTensors buffer
     */
    build() {
        // Build header
        const header = {};
        let offset = 0;
        const tensorData = [];
        for (const [name, { data, shape }] of this.tensors) {
            const bytes = new Uint8Array(data.buffer);
            const dataLength = bytes.length;
            header[name] = {
                dtype: 'F32',
                shape,
                data_offsets: [offset, offset + dataLength],
            };
            tensorData.push(bytes);
            offset += dataLength;
        }
        // Add metadata
        if (Object.keys(this.metadata).length > 0) {
            header['__metadata__'] = this.metadata;
        }
        // Encode header
        const headerJson = JSON.stringify(header);
        const headerBytes = new TextEncoder().encode(headerJson);
        // Pad header to 8-byte alignment
        const headerPadding = (8 - (headerBytes.length % 8)) % 8;
        const paddedHeaderLength = headerBytes.length + headerPadding;
        // Build final buffer
        const totalLength = 8 + paddedHeaderLength + offset;
        const buffer = new Uint8Array(totalLength);
        const view = new DataView(buffer.buffer);
        // Write header length (8 bytes, little-endian)
        view.setBigUint64(0, BigInt(paddedHeaderLength), true);
        // Write header
        buffer.set(headerBytes, 8);
        // Write tensor data
        let dataOffset = 8 + paddedHeaderLength;
        for (const data of tensorData) {
            buffer.set(data, dataOffset);
            dataOffset += data.length;
        }
        return buffer;
    }
    /**
     * Clear all tensors and metadata
     */
    clear() {
        this.tensors.clear();
        this.metadata = {};
    }
}
exports.SafeTensorsWriter = SafeTensorsWriter;
/**
 * SafeTensors Reader
 *
 * Reads tensors from SafeTensors format.
 */
class SafeTensorsReader {
    constructor(buffer) {
        this.header = {};
        this.dataOffset = 0;
        this.buffer = buffer;
        this.parseHeader();
    }
    /**
     * Get tensor names
     */
    getTensorNames() {
        return Object.keys(this.header).filter(k => k !== '__metadata__');
    }
    /**
     * Get tensor by name
     */
    getTensor(name) {
        const entry = this.header[name];
        if (!entry || typeof entry === 'object' && 'dtype' in entry === false) {
            return null;
        }
        const tensorHeader = entry;
        const [start, end] = tensorHeader.data_offsets;
        const bytes = this.buffer.slice(this.dataOffset + start, this.dataOffset + end);
        return {
            data: new Float32Array(bytes.buffer, bytes.byteOffset, bytes.length / 4),
            shape: tensorHeader.shape,
        };
    }
    /**
     * Get tensor as 2D array
     */
    getTensor2D(name) {
        const tensor = this.getTensor(name);
        if (!tensor || tensor.shape.length !== 2)
            return null;
        const [rows, cols] = tensor.shape;
        const result = [];
        for (let i = 0; i < rows; i++) {
            const row = [];
            for (let j = 0; j < cols; j++) {
                row.push(tensor.data[i * cols + j]);
            }
            result.push(row);
        }
        return result;
    }
    /**
     * Get tensor as 1D array
     */
    getTensor1D(name) {
        const tensor = this.getTensor(name);
        if (!tensor)
            return null;
        return Array.from(tensor.data);
    }
    /**
     * Get metadata
     */
    getMetadata() {
        const meta = this.header['__metadata__'];
        if (!meta || typeof meta !== 'object')
            return {};
        return meta;
    }
    parseHeader() {
        const view = new DataView(this.buffer.buffer, this.buffer.byteOffset);
        const headerLength = Number(view.getBigUint64(0, true));
        const headerBytes = this.buffer.slice(8, 8 + headerLength);
        const headerJson = new TextDecoder().decode(headerBytes);
        this.header = JSON.parse(headerJson.replace(/\0+$/, '')); // Remove padding nulls
        this.dataOffset = 8 + headerLength;
    }
}
exports.SafeTensorsReader = SafeTensorsReader;
/**
 * Model Exporter
 *
 * Unified export interface for SONA models.
 */
class ModelExporter {
    /**
     * Export to SafeTensors format
     */
    toSafeTensors(model) {
        const writer = new SafeTensorsWriter();
        // Add metadata
        writer.addMetadata('name', model.metadata.name);
        writer.addMetadata('version', model.metadata.version);
        writer.addMetadata('architecture', model.metadata.architecture);
        if (model.metadata.training) {
            writer.addMetadata('training_steps', String(model.metadata.training.steps));
            writer.addMetadata('training_loss', String(model.metadata.training.loss));
        }
        // Add LoRA weights
        if (model.loraWeights) {
            writer.add2D('lora.A', model.loraWeights.loraA);
            writer.add2D('lora.B', model.loraWeights.loraB);
            writer.add1D('lora.scaling', [model.loraWeights.scaling]);
        }
        // Add patterns as embeddings
        if (model.patterns && model.patterns.length > 0) {
            const embeddings = model.patterns.map(p => p.embedding);
            writer.add2D('patterns.embeddings', embeddings);
            const successRates = model.patterns.map(p => p.successRate);
            writer.add1D('patterns.success_rates', successRates);
        }
        // Add raw tensors
        if (model.tensors) {
            for (const [name, data] of model.tensors) {
                writer.addTensor(name, data, [data.length]);
            }
        }
        return writer.build();
    }
    /**
     * Export to JSON format
     */
    toJSON(model) {
        return JSON.stringify({
            metadata: model.metadata,
            loraConfig: model.loraConfig,
            loraWeights: model.loraWeights,
            patterns: model.patterns,
            ewcStats: model.ewcStats,
        }, null, 2);
    }
    /**
     * Export to compact binary format
     */
    toBinary(model) {
        const json = this.toJSON(model);
        const jsonBytes = new TextEncoder().encode(json);
        // Simple format: [4-byte length][json bytes]
        const buffer = new Uint8Array(4 + jsonBytes.length);
        const view = new DataView(buffer.buffer);
        view.setUint32(0, jsonBytes.length, true);
        buffer.set(jsonBytes, 4);
        return buffer;
    }
    /**
     * Export for HuggingFace Hub compatibility
     */
    toHuggingFace(model) {
        const safetensors = this.toSafeTensors(model);
        const config = JSON.stringify({
            model_type: 'sona-lora',
            ...model.metadata,
            lora_config: model.loraConfig,
        }, null, 2);
        const readme = `---
license: mit
tags:
- sona
- lora
- ruvector
---

# ${model.metadata.name}

${model.metadata.architecture} model trained with SONA adaptive learning.

## Usage

\`\`\`typescript
import { LoraAdapter, SafeTensorsReader } from '@ruvector/ruvllm';

const reader = new SafeTensorsReader(buffer);
const adapter = new LoraAdapter();
adapter.setWeights({
  loraA: reader.getTensor2D('lora.A'),
  loraB: reader.getTensor2D('lora.B'),
  scaling: reader.getTensor1D('lora.scaling')[0],
});
\`\`\`

## Training Info

- Steps: ${model.metadata.training?.steps || 'N/A'}
- Final Loss: ${model.metadata.training?.loss || 'N/A'}
`;
        return { safetensors, config, readme };
    }
}
exports.ModelExporter = ModelExporter;
/**
 * Model Importer
 *
 * Import models from various formats.
 */
class ModelImporter {
    /**
     * Import from SafeTensors format
     */
    fromSafeTensors(buffer) {
        const reader = new SafeTensorsReader(buffer);
        const metadata = reader.getMetadata();
        const result = {
            metadata: {
                name: metadata.name || 'unknown',
                version: metadata.version || '1.0.0',
                architecture: metadata.architecture || 'sona-lora',
                training: metadata.training_steps ? {
                    steps: parseInt(metadata.training_steps),
                    loss: parseFloat(metadata.training_loss || '0'),
                    learningRate: 0,
                } : undefined,
            },
        };
        // Load LoRA weights
        const loraA = reader.getTensor2D('lora.A');
        const loraB = reader.getTensor2D('lora.B');
        const loraScaling = reader.getTensor1D('lora.scaling');
        if (loraA && loraB && loraScaling) {
            result.loraWeights = {
                loraA,
                loraB,
                scaling: loraScaling[0],
            };
        }
        // Load patterns
        const patternEmbeddings = reader.getTensor2D('patterns.embeddings');
        const patternRates = reader.getTensor1D('patterns.success_rates');
        if (patternEmbeddings && patternRates) {
            result.patterns = patternEmbeddings.map((embedding, i) => ({
                id: `imported-${i}`,
                type: 'query_response',
                embedding,
                successRate: patternRates[i] || 0,
                useCount: 0,
                lastUsed: new Date(),
            }));
        }
        return result;
    }
    /**
     * Import from JSON format
     */
    fromJSON(json) {
        return JSON.parse(json);
    }
    /**
     * Import from binary format
     */
    fromBinary(buffer) {
        const view = new DataView(buffer.buffer, buffer.byteOffset);
        const length = view.getUint32(0, true);
        const jsonBytes = buffer.slice(4, 4 + length);
        const json = new TextDecoder().decode(jsonBytes);
        return this.fromJSON(json);
    }
}
exports.ModelImporter = ModelImporter;
/**
 * Dataset Exporter
 *
 * Export training data in various formats.
 */
class DatasetExporter {
    /**
     * Export to JSONL format (one JSON per line)
     */
    toJSONL(data) {
        return data
            .map(item => JSON.stringify({
            input: item.input,
            output: item.output,
            quality: item.quality,
        }))
            .join('\n');
    }
    /**
     * Export to CSV format
     */
    toCSV(data) {
        const header = 'quality,input,output';
        const rows = data.map(item => `${item.quality},"${item.input.join(',')}","${item.output.join(',')}"`);
        return [header, ...rows].join('\n');
    }
    /**
     * Export patterns for pre-training
     */
    toPretrain(patterns) {
        return patterns
            .filter(p => p.successRate >= 0.7)
            .map(p => JSON.stringify({
            embedding: p.embedding,
            type: p.type,
            quality: p.successRate,
        }))
            .join('\n');
    }
}
exports.DatasetExporter = DatasetExporter;
//# sourceMappingURL=export.js.map