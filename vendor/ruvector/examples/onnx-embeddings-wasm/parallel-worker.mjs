/**
 * Worker thread for parallel ONNX embedding generation
 */
import { parentPort, workerData } from 'worker_threads';
import { WasmEmbedder, WasmEmbedderConfig } from './pkg/ruvector_onnx_embeddings_wasm.js';

// Initialize embedder with model data passed from main thread
const { modelBytes, tokenizerJson, config } = workerData;

const embedderConfig = new WasmEmbedderConfig()
    .setMaxLength(config.maxLength)
    .setNormalize(true)
    .setPooling(0);

const embedder = WasmEmbedder.withConfig(
    new Uint8Array(modelBytes),
    tokenizerJson,
    embedderConfig
);

// Listen for texts to embed
parentPort.on('message', (message) => {
    if (message.type === 'embed') {
        const { id, texts } = message;
        try {
            const embeddings = texts.map(text => Array.from(embedder.embedOne(text)));
            parentPort.postMessage({ type: 'result', id, embeddings });
        } catch (error) {
            parentPort.postMessage({ type: 'error', id, error: error.message });
        }
    } else if (message.type === 'shutdown') {
        process.exit(0);
    }
});

// Signal ready
parentPort.postMessage({ type: 'ready' });
