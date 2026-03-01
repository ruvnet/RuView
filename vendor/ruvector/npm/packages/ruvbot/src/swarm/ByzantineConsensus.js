"use strict";
/**
 * ByzantineConsensus - Byzantine Fault Tolerant Consensus
 *
 * Implements PBFT-style consensus for distributed decision making.
 * Tolerates f faulty nodes where f < n/3.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.ByzantineConsensus = void 0;
exports.createByzantineConsensus = createByzantineConsensus;
const uuid_1 = require("uuid");
const events_1 = require("events");
// ============================================================================
// ByzantineConsensus Implementation
// ============================================================================
class ByzantineConsensus extends events_1.EventEmitter {
    constructor(config = {}) {
        super();
        this.replicas = new Map();
        this.proposals = new Map();
        this.votes = new Map();
        this.decided = new Map();
        this.leaderId = null;
        this.viewNumber = 0;
        this.config = {
            replicas: config.replicas ?? 5,
            timeout: config.timeout ?? 30000,
            retries: config.retries ?? 3,
            requireSignatures: config.requireSignatures ?? false,
        };
        this.replicaId = (0, uuid_1.v4)();
        // Validate Byzantine tolerance requirement
        const maxFaulty = Math.floor((this.config.replicas - 1) / 3);
        if (maxFaulty < 1 && this.config.replicas > 1) {
            console.warn('ByzantineConsensus: Minimum 4 replicas recommended for fault tolerance');
        }
    }
    /**
     * Get maximum tolerable faulty nodes
     */
    get maxFaulty() {
        return Math.floor((this.config.replicas - 1) / 3);
    }
    /**
     * Get quorum size required for consensus
     */
    get quorumSize() {
        return Math.ceil((2 * this.config.replicas) / 3);
    }
    /**
     * Initialize replicas
     */
    initializeReplicas(replicaIds) {
        this.replicas.clear();
        for (let i = 0; i < replicaIds.length; i++) {
            this.replicas.set(replicaIds[i], {
                id: replicaIds[i],
                isLeader: i === 0,
                lastActivity: new Date(),
                status: 'active',
            });
            if (i === 0) {
                this.leaderId = replicaIds[i];
            }
        }
    }
    /**
     * Propose a value for consensus
     */
    async propose(value, proposerId) {
        const proposal = {
            id: (0, uuid_1.v4)(),
            value,
            proposerId: proposerId ?? this.replicaId,
            timestamp: new Date(),
        };
        this.proposals.set(proposal.id, proposal);
        this.votes.set(proposal.id, []);
        this.emit('proposal:created', proposal);
        try {
            // Phase 1: Pre-prepare
            await this.prePrepare(proposal);
            // Phase 2: Prepare
            await this.prepare(proposal);
            // Phase 3: Commit
            const result = await this.commit(proposal);
            return result;
        }
        catch (error) {
            const failedResult = {
                proposalId: proposal.id,
                value,
                phase: 'failed',
                votes: this.votes.get(proposal.id) ?? [],
                decided: false,
                timestamp: new Date(),
            };
            this.emit('consensus:failed', { proposal, error });
            return failedResult;
        }
    }
    /**
     * Vote on a proposal (called by replicas)
     */
    vote(proposalId, voterId, phase, accept) {
        const vote = {
            proposalId,
            voterId,
            phase,
            accept,
        };
        let proposalVotes = this.votes.get(proposalId);
        if (!proposalVotes) {
            proposalVotes = [];
            this.votes.set(proposalId, proposalVotes);
        }
        proposalVotes.push(vote);
        // Update replica activity
        const replica = this.replicas.get(voterId);
        if (replica) {
            replica.lastActivity = new Date();
        }
        this.emit('vote:received', vote);
    }
    /**
     * Get consensus result for a proposal
     */
    getResult(proposalId) {
        return this.decided.get(proposalId);
    }
    /**
     * Get all decided proposals
     */
    getDecided() {
        return Array.from(this.decided.values());
    }
    /**
     * Get replica status
     */
    getReplicaStatus() {
        return Array.from(this.replicas.values());
    }
    /**
     * Mark a replica as faulty
     */
    markFaulty(replicaId) {
        const replica = this.replicas.get(replicaId);
        if (replica) {
            replica.status = 'faulty';
            this.emit('replica:faulty', replica);
            // If leader is faulty, trigger view change
            if (replica.isLeader) {
                this.viewChange();
            }
        }
    }
    /**
     * Get consensus statistics
     */
    getStats() {
        let activeReplicas = 0;
        let faultyReplicas = 0;
        for (const replica of this.replicas.values()) {
            if (replica.status === 'active')
                activeReplicas++;
            if (replica.status === 'faulty')
                faultyReplicas++;
        }
        return {
            totalReplicas: this.replicas.size,
            activeReplicas,
            faultyReplicas,
            maxFaulty: this.maxFaulty,
            quorumSize: this.quorumSize,
            totalProposals: this.proposals.size,
            decidedProposals: this.decided.size,
            viewNumber: this.viewNumber,
            leaderId: this.leaderId,
        };
    }
    // ==========================================================================
    // Private Methods - PBFT Phases
    // ==========================================================================
    async prePrepare(proposal) {
        this.emit('phase:pre-prepare', proposal);
        // Leader broadcasts pre-prepare
        // In a real implementation, this would be sent to all replicas
        // Here we simulate immediate local acceptance
        // Wait for pre-prepare acceptance (simulated)
        await this.waitForPhase(proposal.id, 'pre-prepare', 1);
    }
    async prepare(proposal) {
        this.emit('phase:prepare', proposal);
        // Broadcast prepare message to all replicas
        // Replicas validate and send prepare messages
        // Simulate self-vote
        this.vote(proposal.id, this.replicaId, 'prepare', true);
        // Wait for quorum of prepare messages
        await this.waitForPhase(proposal.id, 'prepare', this.quorumSize);
    }
    async commit(proposal) {
        this.emit('phase:commit', proposal);
        // Broadcast commit message
        // Simulate self-vote
        this.vote(proposal.id, this.replicaId, 'commit', true);
        // Wait for quorum of commit messages
        await this.waitForPhase(proposal.id, 'commit', this.quorumSize);
        // Decision reached
        const result = {
            proposalId: proposal.id,
            value: proposal.value,
            phase: 'decided',
            votes: this.votes.get(proposal.id) ?? [],
            decided: true,
            timestamp: new Date(),
        };
        this.decided.set(proposal.id, result);
        this.emit('consensus:decided', result);
        return result;
    }
    async waitForPhase(proposalId, phase, required) {
        return new Promise((resolve, reject) => {
            const timeout = setTimeout(() => {
                reject(new Error(`Timeout waiting for ${phase} phase`));
            }, this.config.timeout);
            const checkVotes = () => {
                const votes = this.votes.get(proposalId) ?? [];
                const phaseVotes = votes.filter(v => v.phase === phase && v.accept);
                if (phaseVotes.length >= required) {
                    clearTimeout(timeout);
                    resolve();
                    return;
                }
                // Check again
                setTimeout(checkVotes, 100);
            };
            checkVotes();
        });
    }
    viewChange() {
        this.viewNumber++;
        // Select new leader (round-robin)
        const replicaIds = Array.from(this.replicas.keys());
        const activeReplicas = replicaIds.filter(id => {
            const replica = this.replicas.get(id);
            return replica && replica.status === 'active';
        });
        if (activeReplicas.length === 0) {
            this.emit('consensus:no-quorum');
            return;
        }
        const newLeaderIndex = this.viewNumber % activeReplicas.length;
        const newLeaderId = activeReplicas[newLeaderIndex];
        // Update leader status
        for (const [id, replica] of this.replicas) {
            replica.isLeader = id === newLeaderId;
        }
        this.leaderId = newLeaderId;
        this.emit('view:changed', { viewNumber: this.viewNumber, leaderId: newLeaderId });
    }
}
exports.ByzantineConsensus = ByzantineConsensus;
// ============================================================================
// Factory Function
// ============================================================================
function createByzantineConsensus(config) {
    return new ByzantineConsensus(config);
}
exports.default = ByzantineConsensus;
//# sourceMappingURL=ByzantineConsensus.js.map