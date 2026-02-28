"use strict";
/**
 * RuvLLM CLI Types
 * Types for CLI configuration and inference options
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.QuantizationType = exports.AccelerationBackend = exports.ModelFormat = void 0;
/** Supported model formats */
var ModelFormat;
(function (ModelFormat) {
    ModelFormat["GGUF"] = "gguf";
    ModelFormat["SafeTensors"] = "safetensors";
    ModelFormat["ONNX"] = "onnx";
})(ModelFormat || (exports.ModelFormat = ModelFormat = {}));
/** Hardware acceleration backends */
var AccelerationBackend;
(function (AccelerationBackend) {
    /** Apple Metal (macOS) */
    AccelerationBackend["Metal"] = "metal";
    /** NVIDIA CUDA */
    AccelerationBackend["CUDA"] = "cuda";
    /** CPU only */
    AccelerationBackend["CPU"] = "cpu";
    /** Apple Neural Engine */
    AccelerationBackend["ANE"] = "ane";
    /** Vulkan (cross-platform GPU) */
    AccelerationBackend["Vulkan"] = "vulkan";
})(AccelerationBackend || (exports.AccelerationBackend = AccelerationBackend = {}));
/** Quantization levels */
var QuantizationType;
(function (QuantizationType) {
    QuantizationType["F32"] = "f32";
    QuantizationType["F16"] = "f16";
    QuantizationType["Q8_0"] = "q8_0";
    QuantizationType["Q4_K_M"] = "q4_k_m";
    QuantizationType["Q4_K_S"] = "q4_k_s";
    QuantizationType["Q5_K_M"] = "q5_k_m";
    QuantizationType["Q5_K_S"] = "q5_k_s";
    QuantizationType["Q6_K"] = "q6_k";
    QuantizationType["Q2_K"] = "q2_k";
    QuantizationType["Q3_K_M"] = "q3_k_m";
})(QuantizationType || (exports.QuantizationType = QuantizationType = {}));
//# sourceMappingURL=types.js.map