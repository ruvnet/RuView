/**
 * ByzantineConsensus - Byzantine Fault Tolerant Consensus
 *
 * Implements PBFT-style consensus for distributed decision making.
 * Tolerates f faulty nodes where f < n/3.
 */
import { EventEmitter } from 'events';
export type ConsensusPhase = 'pre-prepare' | 'prepare' | 'commit' | 'decided' | 'failed';
export interface ConsensusConfig {
    replicas: number;
    timeout: number;
    retries: number;
    requireSignatures: boolean;
}
export interface Proposal<T = unknown> {
    id: string;
    value: T;
    proposerId: string;
    timestamp: Date;
}
export interface Vote {
    proposalId: string;
    voterId: string;
    phase: ConsensusPhase;
    accept: boolean;
    signature?: string;
}
export interface ConsensusResult<T = unknown> {
    proposalId: string;
    value: T;
    phase: ConsensusPhase;
    votes: Vote[];
    decided: boolean;
    timestamp: Date;
}
export interface ReplicaInfo {
    id: string;
    isLeader: boolean;
    lastActivity: Date;
    status: 'active' | 'suspected' | 'faulty';
}
export declare class ByzantineConsensus<T = unknown> extends EventEmitter {
    private readonly config;
    private readonly replicaId;
    private replicas;
    private proposals;
    private votes;
    private decided;
    private leaderId;
    private viewNumber;
    constructor(config?: Partial<ConsensusConfig>);
    /**
     * Get maximum tolerable faulty nodes
     */
    get maxFaulty(): number;
    /**
     * Get quorum size required for consensus
     */
    get quorumSize(): number;
    /**
     * Initialize replicas
     */
    initializeReplicas(replicaIds: string[]): void;
    /**
     * Propose a value for consensus
     */
    propose(value: T, proposerId?: string): Promise<ConsensusResult<T>>;
    /**
     * Vote on a proposal (called by replicas)
     */
    vote(proposalId: string, voterId: string, phase: ConsensusPhase, accept: boolean): void;
    /**
     * Get consensus result for a proposal
     */
    getResult(proposalId: string): ConsensusResult<T> | undefined;
    /**
     * Get all decided proposals
     */
    getDecided(): ConsensusResult<T>[];
    /**
     * Get replica status
     */
    getReplicaStatus(): ReplicaInfo[];
    /**
     * Mark a replica as faulty
     */
    markFaulty(replicaId: string): void;
    /**
     * Get consensus statistics
     */
    getStats(): {
        totalReplicas: number;
        activeReplicas: number;
        faultyReplicas: number;
        maxFaulty: number;
        quorumSize: number;
        totalProposals: number;
        decidedProposals: number;
        viewNumber: number;
        leaderId: string | null;
    };
    private prePrepare;
    private prepare;
    private commit;
    private waitForPhase;
    private viewChange;
}
export declare function createByzantineConsensus<T = unknown>(config?: Partial<ConsensusConfig>): ByzantineConsensus<T>;
export default ByzantineConsensus;
//# sourceMappingURL=ByzantineConsensus.d.ts.map