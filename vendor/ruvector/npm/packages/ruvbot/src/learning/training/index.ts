/**
 * Training Module - Trajectory-based learning with LoRA and EWC
 */

export interface TrainingPipeline {
  trajectoryCollector: TrajectoryCollector;
  loraTrainer: LoRATrainer;
  ewcConsolidator: EWCConsolidator;
}

export interface TrajectoryCollector {
  startSession(sessionId: string): void;
  recordTurn(sessionId: string, turn: TurnSnapshot): Promise<void>;
  recordSkillExecution(sessionId: string, execution: SkillExecution): void;
  endSession(sessionId: string): Promise<Trajectory>;
  label(trajectoryId: string, verdict: Verdict, reason?: string): Promise<void>;
}

export interface Trajectory {
  id: string;
  sessionId: string;
  tenantId: string;
  turns: TurnSnapshot[];
  skillExecutions: SkillExecution[];
  startTime: Date;
  endTime: Date;
  verdict?: Verdict;
  verdictReason?: string;
  embeddingId?: string;
}

export interface TurnSnapshot {
  turnId: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  contentEmbedding: Float32Array;
  tokenCount: number;
  latencyMs: number;
  timestamp: Date;
}

export interface SkillExecution {
  skillId: string;
  params: Record<string, unknown>;
  success: boolean;
  latencyMs: number;
}

export type Verdict = 'positive' | 'negative' | 'neutral';

export interface LoRATrainer {
  train(trajectories: LabeledTrajectory[], config: LoRAConfig): Promise<LoRAWeights>;
  merge(baseModel: ModelWeights, lora: LoRAWeights): Promise<ModelWeights>;
  evaluate(testSet: LabeledTrajectory[]): Promise<EvaluationMetrics>;
}

export interface LabeledTrajectory extends Trajectory {
  verdict: Verdict;
  verdictConfidence?: number;
}

export interface LoRAConfig {
  rank: number;
  alpha: number;
  epochs: number;
  batchSize: number;
  learningRate: number;
}

export interface LoRAWeights {
  rank: number;
  alpha: number;
  layerAdapters: LayerAdapter[];
}

export interface LayerAdapter {
  layerIndex: number;
  A: Float32Array;
  B: Float32Array;
}

export interface ModelWeights {
  layers: Float32Array[];
  hiddenSize: number;
  parameterCount: number;
}

export interface EvaluationMetrics {
  accuracy: number;
  precision: number;
  recall: number;
  f1Score: number;
}

export interface EWCConsolidator {
  computeFisher(model: ModelWeights, trajectories: Trajectory[]): Promise<FisherMatrix>;
  consolidate(
    oldWeights: ModelWeights,
    newWeights: ModelWeights,
    fisher: FisherMatrix,
    lambda: number
  ): Promise<ConsolidationResult>;
}

export interface FisherMatrix {
  diagonal: Float32Array;
  parameterSnapshot: ModelWeights;
}

export interface ConsolidationResult {
  weights: ModelWeights;
  fisher: FisherMatrix;
  oldKnowledgeRetention: number;
  newKnowledgeGain: number;
}
