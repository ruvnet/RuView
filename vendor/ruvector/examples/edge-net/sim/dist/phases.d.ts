/**
 * Phase Transition Logic
 * Manages lifecycle phases and transition conditions
 */
import { Network } from './network.js';
import { MetricsCollector } from './metrics.js';
export interface PhaseTransitionCondition {
    minNodes: number;
    maxNodes: number;
    requiredDuration?: number;
    customCheck?: (network: Network) => boolean;
}
export declare class PhaseManager {
    private network;
    private metrics;
    private conditions;
    private lastPhase;
    constructor(network: Network, metrics: MetricsCollector);
    /**
     * Check if network should transition to next phase
     */
    checkTransition(): boolean;
    /**
     * Handle phase transition
     */
    private onTransition;
    /**
     * Log phase-specific information
     */
    private logPhaseInfo;
    /**
     * Get phase progress (0-1)
     */
    getPhaseProgress(): number;
    /**
     * Get estimated ticks to next phase
     */
    getTicksToNextPhase(): number;
}
//# sourceMappingURL=phases.d.ts.map