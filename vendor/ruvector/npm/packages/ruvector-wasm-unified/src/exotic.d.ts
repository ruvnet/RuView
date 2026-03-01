/**
 * RuVector WASM Unified - Exotic Computation Engine
 *
 * Provides advanced computation paradigms including:
 * - Quantum-inspired algorithms (superposition, entanglement, interference)
 * - Hyperbolic geometry operations (Poincare, Lorentz, Klein models)
 * - Topological data analysis (persistent homology, Betti numbers)
 * - Fractal and chaos-based computation
 * - Non-Euclidean neural operations
 */
import type { QuantumState, HyperbolicPoint, TopologicalFeature, ExoticConfig } from './types';
/**
 * Core exotic computation engine for advanced algorithmic paradigms
 */
export interface ExoticEngine {
    /**
     * Initialize quantum-inspired state
     * @param numQubits Number of qubits to simulate
     * @returns Initial quantum state
     */
    quantumInit(numQubits: number): QuantumState;
    /**
     * Apply Hadamard gate for superposition
     * @param state Current quantum state
     * @param qubit Target qubit index
     * @returns New quantum state
     */
    quantumHadamard(state: QuantumState, qubit: number): QuantumState;
    /**
     * Apply CNOT gate for entanglement
     * @param state Current quantum state
     * @param control Control qubit
     * @param target Target qubit
     * @returns Entangled quantum state
     */
    quantumCnot(state: QuantumState, control: number, target: number): QuantumState;
    /**
     * Apply phase rotation gate
     * @param state Current quantum state
     * @param qubit Target qubit
     * @param phase Phase angle in radians
     * @returns Rotated quantum state
     */
    quantumPhase(state: QuantumState, qubit: number, phase: number): QuantumState;
    /**
     * Measure quantum state
     * @param state Quantum state to measure
     * @param qubits Qubits to measure (empty = all)
     * @returns Measurement result and collapsed state
     */
    quantumMeasure(state: QuantumState, qubits?: number[]): QuantumMeasurement;
    /**
     * Quantum amplitude amplification (Grover-like)
     * @param state Initial state
     * @param oracle Oracle function marking solutions
     * @param iterations Number of amplification iterations
     * @returns Amplified state
     */
    quantumAmplify(state: QuantumState, oracle: (amplitudes: Float32Array) => Float32Array, iterations?: number): QuantumState;
    /**
     * Variational quantum eigensolver simulation
     * @param hamiltonian Hamiltonian matrix
     * @param ansatz Variational ansatz circuit
     * @param optimizer Optimizer type
     * @returns Ground state energy estimate
     */
    quantumVqe(hamiltonian: Float32Array, ansatz?: QuantumCircuit, optimizer?: 'cobyla' | 'spsa' | 'adam'): VqeResult;
    /**
     * Create point in hyperbolic space
     * @param coordinates Euclidean coordinates
     * @param manifold Target manifold model
     * @param curvature Negative curvature value
     * @returns Hyperbolic point
     */
    hyperbolicPoint(coordinates: Float32Array, manifold?: 'poincare' | 'lorentz' | 'klein', curvature?: number): HyperbolicPoint;
    /**
     * Compute hyperbolic distance
     * @param p1 First point
     * @param p2 Second point
     * @returns Hyperbolic distance
     */
    hyperbolicDistance(p1: HyperbolicPoint, p2: HyperbolicPoint): number;
    /**
     * Mobius addition in Poincare ball
     * @param x First point
     * @param y Second point
     * @param c Curvature parameter
     * @returns Sum in hyperbolic space
     */
    mobiusAdd(x: HyperbolicPoint, y: HyperbolicPoint, c?: number): HyperbolicPoint;
    /**
     * Mobius matrix-vector multiplication
     * @param M Matrix
     * @param x Point
     * @param c Curvature parameter
     * @returns Transformed point
     */
    mobiusMatvec(M: Float32Array, x: HyperbolicPoint, c?: number): HyperbolicPoint;
    /**
     * Exponential map (tangent space to hyperbolic)
     * @param v Tangent vector
     * @param base Base point
     * @returns Point in hyperbolic space
     */
    hyperbolicExp(v: Float32Array, base?: HyperbolicPoint): HyperbolicPoint;
    /**
     * Logarithmic map (hyperbolic to tangent space)
     * @param y Target point
     * @param base Base point
     * @returns Tangent vector
     */
    hyperbolicLog(y: HyperbolicPoint, base?: HyperbolicPoint): Float32Array;
    /**
     * Parallel transport in hyperbolic space
     * @param v Tangent vector
     * @param from Source point
     * @param to Target point
     * @returns Transported vector
     */
    hyperbolicTransport(v: Float32Array, from: HyperbolicPoint, to: HyperbolicPoint): Float32Array;
    /**
     * Hyperbolic centroid (Frechet mean)
     * @param points Points to average
     * @param weights Optional weights
     * @returns Centroid in hyperbolic space
     */
    hyperbolicCentroid(points: HyperbolicPoint[], weights?: Float32Array): HyperbolicPoint;
    /**
     * Compute persistent homology
     * @param data Point cloud data
     * @param maxDimension Maximum homology dimension
     * @param threshold Filtration threshold
     * @returns Topological features
     */
    persistentHomology(data: Float32Array[], maxDimension?: number, threshold?: number): TopologicalFeature[];
    /**
     * Compute Betti numbers
     * @param features Topological features from persistent homology
     * @param threshold Persistence threshold
     * @returns Betti numbers by dimension
     */
    bettiNumbers(features: TopologicalFeature[], threshold?: number): number[];
    /**
     * Generate persistence diagram
     * @param features Topological features
     * @returns Birth-death pairs for visualization
     */
    persistenceDiagram(features: TopologicalFeature[]): PersistencePair[];
    /**
     * Compute bottleneck distance between persistence diagrams
     * @param diagram1 First persistence diagram
     * @param diagram2 Second persistence diagram
     * @returns Bottleneck distance
     */
    bottleneckDistance(diagram1: PersistencePair[], diagram2: PersistencePair[]): number;
    /**
     * Compute Wasserstein distance between persistence diagrams
     * @param diagram1 First persistence diagram
     * @param diagram2 Second persistence diagram
     * @param p Order of Wasserstein distance
     * @returns Wasserstein distance
     */
    wassersteinDistance(diagram1: PersistencePair[], diagram2: PersistencePair[], p?: number): number;
    /**
     * Mapper algorithm for topological visualization
     * @param data Input data
     * @param lens Lens function
     * @param numBins Number of bins per dimension
     * @param overlap Overlap between bins
     * @returns Mapper graph
     */
    mapper(data: Float32Array[], lens?: (point: Float32Array) => number[], numBins?: number, overlap?: number): MapperGraph;
    /**
     * Compute fractal dimension (box-counting)
     * @param data Point cloud or image data
     * @returns Estimated fractal dimension
     */
    fractalDimension(data: Float32Array[]): number;
    /**
     * Generate Mandelbrot/Julia set embedding
     * @param c Julia set constant (undefined for Mandelbrot)
     * @param resolution Grid resolution
     * @param maxIterations Maximum iterations
     * @returns Escape time embedding
     */
    fractalEmbedding(c?: {
        re: number;
        im: number;
    }, resolution?: number, maxIterations?: number): Float32Array;
    /**
     * Compute Lyapunov exponents for chaotic dynamics
     * @param trajectory Time series trajectory
     * @param embeddingDim Embedding dimension
     * @param delay Time delay
     * @returns Lyapunov exponents
     */
    lyapunovExponents(trajectory: Float32Array, embeddingDim?: number, delay?: number): Float32Array;
    /**
     * Recurrence plot analysis
     * @param trajectory Time series
     * @param threshold Recurrence threshold
     * @returns Recurrence plot matrix
     */
    recurrencePlot(trajectory: Float32Array, threshold?: number): Uint8Array;
    /**
     * Hyperbolic neural network layer forward pass
     * @param input Input in hyperbolic space
     * @param weights Weight matrix
     * @param bias Bias vector
     * @returns Output in hyperbolic space
     */
    hyperbolicLayer(input: HyperbolicPoint[], weights: Float32Array, bias?: Float32Array): HyperbolicPoint[];
    /**
     * Spherical neural network layer (on n-sphere)
     * @param input Input on sphere
     * @param weights Weight matrix
     * @returns Output on sphere
     */
    sphericalLayer(input: Float32Array[], weights: Float32Array): Float32Array[];
    /**
     * Mixed-curvature neural network
     * @param input Input embeddings
     * @param curvatures Curvature for each dimension
     * @param weights Weight matrices
     * @returns Output in product manifold
     */
    productManifoldLayer(input: Float32Array[], curvatures: Float32Array, weights: Float32Array[]): Float32Array[];
    /**
     * Get exotic computation statistics
     * @returns Resource usage and statistics
     */
    getStats(): ExoticStats;
    /**
     * Configure exotic engine
     * @param config Configuration options
     */
    configure(config: Partial<ExoticConfig>): void;
}
/** Quantum measurement result */
export interface QuantumMeasurement {
    bitstring: number[];
    probability: number;
    collapsedState: QuantumState;
}
/** Quantum circuit representation */
export interface QuantumCircuit {
    numQubits: number;
    gates: QuantumGate[];
    parameters?: Float32Array;
}
/** Quantum gate */
export interface QuantumGate {
    type: 'H' | 'X' | 'Y' | 'Z' | 'CNOT' | 'RX' | 'RY' | 'RZ' | 'CZ' | 'SWAP';
    targets: number[];
    parameter?: number;
}
/** VQE result */
export interface VqeResult {
    energy: number;
    optimalParameters: Float32Array;
    iterations: number;
    converged: boolean;
}
/** Persistence pair for diagrams */
export interface PersistencePair {
    birth: number;
    death: number;
    dimension: number;
}
/** Mapper graph structure */
export interface MapperGraph {
    nodes: MapperNode[];
    edges: MapperEdge[];
}
/** Mapper node */
export interface MapperNode {
    id: string;
    members: number[];
    centroid: Float32Array;
}
/** Mapper edge */
export interface MapperEdge {
    source: string;
    target: string;
    weight: number;
}
/** Exotic engine statistics */
export interface ExoticStats {
    quantumOperations: number;
    hyperbolicOperations: number;
    topologicalOperations: number;
    totalComputeTimeMs: number;
    peakMemoryBytes: number;
}
/**
 * Create an exotic computation engine instance
 * @param config Optional configuration
 * @returns Initialized exotic engine
 */
export declare function createExoticEngine(config?: ExoticConfig): ExoticEngine;
/**
 * Create quantum circuit builder
 * @param numQubits Number of qubits
 * @returns Circuit builder
 */
export declare function createCircuitBuilder(numQubits: number): {
    h: (qubit: number) => void;
    x: (qubit: number) => void;
    cnot: (control: number, target: number) => void;
    rx: (qubit: number, angle: number) => void;
    ry: (qubit: number, angle: number) => void;
    rz: (qubit: number, angle: number) => void;
    build: () => QuantumCircuit;
};
/**
 * Utility: Project from Euclidean to Poincare ball
 * @param x Euclidean coordinates
 * @param c Curvature parameter
 */
export declare function projectToPoincare(x: Float32Array, c?: number): Float32Array;
/**
 * Utility: Project from Poincare to Lorentz model
 * @param x Poincare coordinates
 * @param c Curvature parameter
 */
export declare function poincareToLorentz(x: Float32Array, c?: number): Float32Array;
//# sourceMappingURL=exotic.d.ts.map