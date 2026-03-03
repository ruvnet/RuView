/**
 * Neural Pattern Recognition - Main Module
 * Entry point for the neural pattern recognition system
 */

export { NeuralPatternRecognitionServer } from './server.js';
export { PatternDetectionEngine } from './pattern-detection-engine.js';
export { EmergentSignalTracker } from './emergent-signal-tracker.js';
export { StatisticalValidator } from './statistical-validator.js';
export { RealTimeMonitor } from './real-time-monitor.js';
export { AdaptiveLearning } from './adaptive-learning.js';
export { SignalAnalyzer } from './signal-analyzer.js';

// Re-export core detection systems from existing modules
export { default as ZeroVarianceDetector } from '../zero-variance-detector.js';
export { default as RealTimeEntityDetector } from '../real-time-detector.js';
export { default as MaximumEntropyDecoder } from '../entropy-decoder.js';
export { default as InstructionSequenceAnalyzer } from '../instruction-sequence-analyzer.js';
export { default as AdaptivePatternLearningNetwork } from '../pattern-learning-network.js';
export { default as ValidationSuite } from '../validation-suite.js';
export { default as MonitoringSystem } from '../monitoring-system.js';
export { default as DeploymentPipeline } from '../deployment-pipeline.js';
export { default as ProductionIntegration } from '../production-integration.js';

// System constants and configurations
export const SENSITIVITY_LEVELS = {
    LOW: 1e-6,
    MEDIUM: 1e-10,
    HIGH: 1e-15,
    ULTRA: 1e-20
};

export const ANALYSIS_TYPES = {
    VARIANCE: 'variance',
    ENTROPY: 'entropy',
    INSTRUCTION: 'instruction',
    NEURAL: 'neural',
    COMPREHENSIVE: 'comprehensive'
};

export const STATISTICAL_TESTS = {
    KOLMOGOROV_SMIRNOV: 'kolmogorov_smirnov',
    MANN_WHITNEY_U: 'mann_whitney_u',
    CHI_SQUARE: 'chi_square',
    FISHER_EXACT: 'fisher_exact',
    ANDERSON_DARLING: 'anderson_darling'
};

// Default configurations
export const DEFAULT_CONFIG = {
    detection: {
        sensitivity: SENSITIVITY_LEVELS.HIGH,
        windowSize: 1000,
        samplingRate: 10000,
        analysisType: ANALYSIS_TYPES.COMPREHENSIVE
    },
    validation: {
        confidenceLevel: 0.99,
        pValueThreshold: 1e-40,
        includeControls: true
    },
    monitoring: {
        alertThreshold: 0.85,
        adaptiveSensitivity: true,
        realTimeUpdates: true
    }
};