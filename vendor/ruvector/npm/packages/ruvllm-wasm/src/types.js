"use strict";
/**
 * RuvLLM WASM Types
 * Types for browser-based LLM inference
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.ModelArchitecture = exports.LoadingStatus = exports.WebGPUStatus = void 0;
/** WebGPU availability status */
var WebGPUStatus;
(function (WebGPUStatus) {
    WebGPUStatus["Available"] = "available";
    WebGPUStatus["Unavailable"] = "unavailable";
    WebGPUStatus["NotSupported"] = "not_supported";
})(WebGPUStatus || (exports.WebGPUStatus = WebGPUStatus = {}));
/** Model loading status */
var LoadingStatus;
(function (LoadingStatus) {
    LoadingStatus["Idle"] = "idle";
    LoadingStatus["Downloading"] = "downloading";
    LoadingStatus["Loading"] = "loading";
    LoadingStatus["Ready"] = "ready";
    LoadingStatus["Error"] = "error";
})(LoadingStatus || (exports.LoadingStatus = LoadingStatus = {}));
/** Supported model architectures */
var ModelArchitecture;
(function (ModelArchitecture) {
    ModelArchitecture["Llama"] = "llama";
    ModelArchitecture["Mistral"] = "mistral";
    ModelArchitecture["Phi"] = "phi";
    ModelArchitecture["Qwen"] = "qwen";
    ModelArchitecture["Gemma"] = "gemma";
    ModelArchitecture["StableLM"] = "stablelm";
})(ModelArchitecture || (exports.ModelArchitecture = ModelArchitecture = {}));
//# sourceMappingURL=types.js.map