/**
 * RuVector WASM Unified Types
 * Core type definitions shared across all modules
 */

// ============================================================================
// Core Types
// ============================================================================

/** Tensor representation for neural computations */
export interface Tensor {
  data: Float32Array;
  shape: number[];
  dtype: 'float32' | 'float16' | 'int32' | 'uint8';
}

/** Result wrapper for fallible operations */
export interface Result<T, E = Error> {
  ok: boolean;
  value?: T;
  error?: E;
}

/** Async result for operations that may be pending */
export interface AsyncResult<T> extends Result<T> {
  pending: boolean;
  progress?: number;
}

// ============================================================================
// Attention Types
// ============================================================================

/** Configuration for multi-head attention */
export interface MultiHeadConfig {
  numHeads: number;
  headDim: number;
  dropout?: number;
  useBias?: boolean;
  scaleFactor?: number;
}

/** Result from Mixture of Experts attention */
export interface MoEResult {
  output: Float32Array;
  routerLogits: Float32Array;
  expertUsage: Float32Array;
  loadBalanceLoss: number;
}

/** Result from Mamba state-space model */
export interface MambaResult {
  output: Float32Array;
  newState: Float32Array;
  deltaTime: number;
}

/** Attention scores with metadata */
export interface AttentionScores {
  scores: Float32Array;
  weights: Float32Array;
  metadata: AttentionMetadata;
}

/** Metadata for attention computation */
export interface AttentionMetadata {
  mechanism: string;
  computeTimeMs: number;
  memoryUsageBytes: number;
  sparsityRatio?: number;
}

// ============================================================================
// DAG Types
// ============================================================================

/** Node in a query DAG */
export interface QueryNode {
  id: string;
  embedding: Float32Array;
  nodeType: 'query' | 'key' | 'value' | 'gate' | 'aggregate';
  metadata?: Record<string, unknown>;
}

/** Edge in a query DAG */
export interface QueryEdge {
  source: string;
  target: string;
  weight: number;
  edgeType: 'attention' | 'dependency' | 'gate' | 'skip';
}

/** Directed Acyclic Graph for query processing */
export interface QueryDag {
  nodes: QueryNode[];
  edges: QueryEdge[];
  rootIds: string[];
  leafIds: string[];
}

/** Gating packet for mincut operations */
export interface GatePacket {
  gateValues: Float32Array;
  threshold: number;
  mode: 'hard' | 'soft' | 'stochastic';
}

// ============================================================================
// Learning Types
// ============================================================================

/** Enhanced embedding with SONA pre-query processing */
export interface EnhancedEmbedding {
  original: Float32Array;
  enhanced: Float32Array;
  contextVector: Float32Array;
  confidence: number;
}

/** Learning trajectory for reinforcement */
export interface LearningTrajectory {
  states: Float32Array[];
  actions: number[];
  rewards: number[];
  dones: boolean[];
}

/** Micro-LoRA adaptation config */
export interface MicroLoraConfig {
  rank: number;
  alpha: number;
  dropout?: number;
  targetModules: string[];
}

/** BTSP (Behavioral Timescale Synaptic Plasticity) config */
export interface BtspConfig {
  learningRate: number;
  eligibilityDecay: number;
  rewardWindow: number;
}

// ============================================================================
// Nervous System Types
// ============================================================================

/** Synapse connection between neurons */
export interface Synapse {
  presynapticId: string;
  postsynapticId: string;
  weight: number;
  delay: number;
  plasticity: PlasticityRule;
}

/** Plasticity rule for synapse adaptation */
export interface PlasticityRule {
  type: 'stdp' | 'btsp' | 'hebbian' | 'oja' | 'bcm';
  params: Record<string, number>;
}

/** Neuron in the nervous system */
export interface Neuron {
  id: string;
  potential: number;
  threshold: number;
  refractory: number;
  neuronType: 'excitatory' | 'inhibitory' | 'modulatory';
}

/** Nervous system state snapshot */
export interface NervousState {
  neurons: Map<string, Neuron>;
  synapses: Synapse[];
  globalModulation: number;
  timestamp: number;
}

/** Signal propagation result */
export interface PropagationResult {
  activatedNeurons: string[];
  spikeTimings: Map<string, number>;
  totalActivity: number;
}

// ============================================================================
// Economy Types
// ============================================================================

/** Credit account state */
export interface CreditAccount {
  balance: number;
  stakedAmount: number;
  contributionMultiplier: number;
  lastUpdate: number;
}

/** Transaction record */
export interface Transaction {
  id: string;
  type: 'deposit' | 'withdraw' | 'stake' | 'unstake' | 'reward' | 'penalty';
  amount: number;
  timestamp: number;
  metadata?: Record<string, unknown>;
}

/** Staking position */
export interface StakingPosition {
  amount: number;
  lockDuration: number;
  startTime: number;
  expectedReward: number;
}

/** Economy metrics */
export interface EconomyMetrics {
  totalSupply: number;
  totalStaked: number;
  circulatingSupply: number;
  averageMultiplier: number;
}

// ============================================================================
// Exotic Types
// ============================================================================

/** Quantum-inspired state */
export interface QuantumState {
  amplitudes: Float32Array;
  phases: Float32Array;
  entanglementMap: Map<number, number[]>;
}

/** Hyperbolic embedding */
export interface HyperbolicPoint {
  coordinates: Float32Array;
  curvature: number;
  manifold: 'poincare' | 'lorentz' | 'klein';
}

/** Topological feature */
export interface TopologicalFeature {
  dimension: number;
  persistence: number;
  birthTime: number;
  deathTime: number;
}

/** Exotic computation result */
export interface ExoticResult<T> {
  value: T;
  computationType: 'quantum' | 'hyperbolic' | 'topological' | 'fractal';
  fidelity: number;
  resourceUsage: ResourceUsage;
}

/** Resource usage metrics */
export interface ResourceUsage {
  cpuTimeMs: number;
  memoryBytes: number;
  wasmCycles?: number;
}

// ============================================================================
// Event Types
// ============================================================================

/** Event emitted by the system */
export interface SystemEvent {
  type: string;
  timestamp: number;
  source: string;
  payload: unknown;
}

/** Event listener callback */
export type EventCallback<T = unknown> = (event: SystemEvent & { payload: T }) => void;

/** Subscription handle */
export interface Subscription {
  unsubscribe(): void;
  readonly active: boolean;
}

// ============================================================================
// Configuration Types
// ============================================================================

/** Global configuration */
export interface UnifiedConfig {
  wasmPath?: string;
  enableSimd?: boolean;
  enableThreads?: boolean;
  memoryLimit?: number;
  logLevel?: 'debug' | 'info' | 'warn' | 'error';
}

/** Module-specific configuration */
export interface ModuleConfig {
  attention?: AttentionConfig;
  learning?: LearningConfig;
  nervous?: NervousConfig;
  economy?: EconomyConfig;
  exotic?: ExoticConfig;
}

export interface AttentionConfig {
  defaultMechanism?: string;
  cacheSize?: number;
  precisionMode?: 'fp32' | 'fp16' | 'mixed';
}

export interface LearningConfig {
  defaultLearningRate?: number;
  batchSize?: number;
  enableGradientCheckpointing?: boolean;
}

export interface NervousConfig {
  maxNeurons?: number;
  simulationDt?: number;
  enablePlasticity?: boolean;
}

export interface EconomyConfig {
  initialBalance?: number;
  stakingEnabled?: boolean;
  rewardRate?: number;
}

export interface ExoticConfig {
  quantumSimulationDepth?: number;
  hyperbolicPrecision?: number;
  topologicalMaxDimension?: number;
}
