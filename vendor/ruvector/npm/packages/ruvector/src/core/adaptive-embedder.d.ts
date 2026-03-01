/**
 * AdaptiveEmbedder - Micro-LoRA Style Optimization for ONNX Embeddings
 *
 * Applies continual learning techniques to frozen ONNX embeddings:
 *
 * 1. MICRO-LORA ADAPTERS
 *    - Low-rank projection layers (rank 2-8) on top of frozen embeddings
 *    - Domain-specific fine-tuning with minimal parameters
 *    - ~0.1% of base model parameters
 *
 * 2. CONTRASTIVE LEARNING
 *    - Files edited together → embeddings closer
 *    - Semantic clustering from trajectories
 *    - Online learning from user behavior
 *
 * 3. EWC++ (Elastic Weight Consolidation)
 *    - Prevents catastrophic forgetting
 *    - Consolidates important adaptations
 *    - Fisher information regularization
 *
 * 4. MEMORY-AUGMENTED RETRIEVAL
 *    - Episodic memory for context-aware embeddings
 *    - Attention over past similar embeddings
 *    - Domain prototype learning
 *
 * Architecture:
 *   ONNX(text) → [frozen 384d] → LoRA_A → LoRA_B → [adapted 384d]
 *                                 (384×r)   (r×384)
 */
export interface AdaptiveConfig {
    /** LoRA rank (lower = fewer params, higher = more expressive) */
    loraRank?: number;
    /** Learning rate for online updates */
    learningRate?: number;
    /** EWC regularization strength */
    ewcLambda?: number;
    /** Number of domain prototypes to maintain */
    numPrototypes?: number;
    /** Enable contrastive learning from co-edits */
    contrastiveLearning?: boolean;
    /** Temperature for contrastive loss */
    contrastiveTemp?: number;
    /** Memory capacity for episodic retrieval */
    memoryCapacity?: number;
}
export interface LoRAWeights {
    A: number[][];
    B: number[][];
    bias?: number[];
}
export interface DomainPrototype {
    domain: string;
    centroid: number[];
    count: number;
    variance: number;
}
export interface AdaptiveStats {
    baseModel: string;
    dimension: number;
    loraRank: number;
    loraParams: number;
    adaptations: number;
    prototypes: number;
    memorySize: number;
    ewcConsolidations: number;
    contrastiveUpdates: number;
}
export declare class AdaptiveEmbedder {
    private config;
    private lora;
    private prototypes;
    private episodic;
    private onnxReady;
    private dimension;
    private adaptationCount;
    private ewcCount;
    private contrastiveCount;
    private coEditBuffer;
    constructor(config?: AdaptiveConfig);
    /**
     * Initialize ONNX backend
     */
    init(): Promise<void>;
    /**
     * Generate adaptive embedding
     * Pipeline: ONNX → LoRA → Prototype Adjustment → Episodic Augmentation
     */
    embed(text: string, options?: {
        domain?: string;
        useEpisodic?: boolean;
        storeInMemory?: boolean;
    }): Promise<number[]>;
    /**
     * Batch embed with adaptation
     */
    embedBatch(texts: string[], options?: {
        domain?: string;
    }): Promise<number[][]>;
    /**
     * Learn from co-edit pattern (contrastive learning)
     * Files edited together should have similar embeddings
     */
    learnCoEdit(file1: string, content1: string, file2: string, content2: string): Promise<number>;
    /**
     * Process co-edit batch with contrastive loss
     */
    private processCoEditBatch;
    /**
     * Learn from trajectory outcome (reinforcement-like)
     */
    learnFromOutcome(context: string, action: string, success: boolean, quality?: number): Promise<void>;
    /**
     * EWC consolidation - prevent forgetting important adaptations
     * OPTIMIZED: Works with Float32Array episodic entries
     */
    consolidate(): Promise<void>;
    /**
     * Fallback hash embedding
     */
    private hashEmbed;
    private normalize;
    /**
     * Get statistics
     */
    getStats(): AdaptiveStats;
    /**
     * Export learned weights
     */
    export(): {
        lora: LoRAWeights;
        prototypes: DomainPrototype[];
        stats: AdaptiveStats;
    };
    /**
     * Import learned weights
     */
    import(data: {
        lora?: LoRAWeights;
        prototypes?: DomainPrototype[];
    }): void;
    /**
     * Reset adaptations
     */
    reset(): void;
    /**
     * Get LoRA cache statistics
     */
    getCacheStats(): {
        size: number;
        maxSize: number;
    };
}
export declare function getAdaptiveEmbedder(config?: AdaptiveConfig): AdaptiveEmbedder;
export declare function initAdaptiveEmbedder(config?: AdaptiveConfig): Promise<AdaptiveEmbedder>;
export default AdaptiveEmbedder;
//# sourceMappingURL=adaptive-embedder.d.ts.map