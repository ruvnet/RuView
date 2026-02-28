"use strict";
/**
 * Raft Node Implementation
 * Core Raft consensus algorithm implementation
 */
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.RaftNode = void 0;
const eventemitter3_1 = __importDefault(require("eventemitter3"));
const types_js_1 = require("./types.js");
const state_js_1 = require("./state.js");
/** Default configuration values */
const DEFAULT_CONFIG = {
    electionTimeout: [150, 300],
    heartbeatInterval: 50,
    maxEntriesPerRequest: 100,
};
/** Raft consensus node */
class RaftNode extends eventemitter3_1.default {
    constructor(config) {
        super();
        this.nodeState = types_js_1.NodeState.Follower;
        this.leaderId = null;
        this.transport = null;
        this.stateMachine = null;
        this.electionTimer = null;
        this.heartbeatTimer = null;
        this.running = false;
        this.config = { ...DEFAULT_CONFIG, ...config };
        this.state = new state_js_1.RaftState(config.nodeId, config.peers);
    }
    /** Get node ID */
    get nodeId() {
        return this.config.nodeId;
    }
    /** Get current state */
    get currentState() {
        return this.nodeState;
    }
    /** Get current term */
    get currentTerm() {
        return this.state.currentTerm;
    }
    /** Get current leader ID */
    get leader() {
        return this.leaderId;
    }
    /** Check if this node is the leader */
    get isLeader() {
        return this.nodeState === types_js_1.NodeState.Leader;
    }
    /** Get commit index */
    get commitIndex() {
        return this.state.commitIndex;
    }
    /** Set transport for RPC communication */
    setTransport(transport) {
        this.transport = transport;
    }
    /** Set state machine for applying commands */
    setStateMachine(stateMachine) {
        this.stateMachine = stateMachine;
    }
    /** Start the Raft node */
    start() {
        if (this.running)
            return;
        this.running = true;
        this.resetElectionTimer();
    }
    /** Stop the Raft node */
    stop() {
        this.running = false;
        this.clearTimers();
    }
    /** Propose a command to be replicated (only works if leader) */
    async propose(command) {
        if (this.nodeState !== types_js_1.NodeState.Leader) {
            throw types_js_1.RaftError.notLeader();
        }
        const entry = await this.state.log.appendCommand(this.state.currentTerm, command);
        this.emit(types_js_1.RaftEvent.LogAppended, entry);
        // Immediately replicate to followers
        await this.replicateToFollowers();
        return entry;
    }
    /** Handle RequestVote RPC from a candidate */
    async handleRequestVote(request) {
        // If request term is higher, update term and become follower
        if (request.term > this.state.currentTerm) {
            await this.state.setTerm(request.term);
            this.transitionTo(types_js_1.NodeState.Follower);
        }
        // Deny vote if request term is less than current term
        if (request.term < this.state.currentTerm) {
            return { term: this.state.currentTerm, voteGranted: false };
        }
        // Check if we can vote for this candidate
        const canVote = (this.state.votedFor === null || this.state.votedFor === request.candidateId) &&
            this.state.log.isUpToDate(request.lastLogTerm, request.lastLogIndex);
        if (canVote) {
            await this.state.vote(request.term, request.candidateId);
            this.resetElectionTimer();
            this.emit(types_js_1.RaftEvent.VoteGranted, { candidateId: request.candidateId, term: request.term });
            return { term: this.state.currentTerm, voteGranted: true };
        }
        return { term: this.state.currentTerm, voteGranted: false };
    }
    /** Handle AppendEntries RPC from leader */
    async handleAppendEntries(request) {
        // If request term is higher, update term
        if (request.term > this.state.currentTerm) {
            await this.state.setTerm(request.term);
            this.transitionTo(types_js_1.NodeState.Follower);
        }
        // Reject if term is less than current term
        if (request.term < this.state.currentTerm) {
            return { term: this.state.currentTerm, success: false };
        }
        // Valid leader - reset election timer
        this.leaderId = request.leaderId;
        this.resetElectionTimer();
        // If not follower, become follower
        if (this.nodeState !== types_js_1.NodeState.Follower) {
            this.transitionTo(types_js_1.NodeState.Follower);
        }
        this.emit(types_js_1.RaftEvent.Heartbeat, { leaderId: request.leaderId, term: request.term });
        // Check if log contains entry at prevLogIndex with prevLogTerm
        if (request.prevLogIndex > 0 && !this.state.log.containsEntry(request.prevLogIndex, request.prevLogTerm)) {
            return { term: this.state.currentTerm, success: false };
        }
        // Append entries
        if (request.entries.length > 0) {
            await this.state.log.append(request.entries);
        }
        // Update commit index
        if (request.leaderCommit > this.state.commitIndex) {
            this.state.setCommitIndex(Math.min(request.leaderCommit, this.state.log.lastIndex));
            await this.applyCommitted();
        }
        return {
            term: this.state.currentTerm,
            success: true,
            matchIndex: this.state.log.lastIndex,
        };
    }
    /** Load persistent state */
    loadState(state) {
        this.state.loadPersistentState(state);
    }
    /** Get current persistent state */
    getState() {
        return this.state.getPersistentState();
    }
    // Private methods
    transitionTo(newState) {
        const previousState = this.nodeState;
        if (previousState === newState)
            return;
        this.nodeState = newState;
        this.clearTimers();
        if (newState === types_js_1.NodeState.Leader) {
            this.state.initLeaderState();
            this.leaderId = this.config.nodeId;
            this.startHeartbeat();
            this.emit(types_js_1.RaftEvent.LeaderElected, {
                leaderId: this.config.nodeId,
                term: this.state.currentTerm,
            });
        }
        else {
            this.state.clearLeaderState();
            if (newState === types_js_1.NodeState.Follower) {
                this.leaderId = null;
                this.resetElectionTimer();
            }
        }
        this.emit(types_js_1.RaftEvent.StateChange, {
            previousState,
            newState,
            term: this.state.currentTerm,
        });
    }
    getRandomElectionTimeout() {
        const [min, max] = this.config.electionTimeout;
        return min + Math.random() * (max - min);
    }
    resetElectionTimer() {
        if (this.electionTimer) {
            clearTimeout(this.electionTimer);
        }
        if (!this.running)
            return;
        this.electionTimer = setTimeout(() => {
            this.startElection();
        }, this.getRandomElectionTimeout());
    }
    clearTimers() {
        if (this.electionTimer) {
            clearTimeout(this.electionTimer);
            this.electionTimer = null;
        }
        if (this.heartbeatTimer) {
            clearInterval(this.heartbeatTimer);
            this.heartbeatTimer = null;
        }
    }
    async startElection() {
        if (!this.running)
            return;
        // Increment term and become candidate
        await this.state.setTerm(this.state.currentTerm + 1);
        await this.state.vote(this.state.currentTerm, this.config.nodeId);
        this.transitionTo(types_js_1.NodeState.Candidate);
        this.emit(types_js_1.RaftEvent.VoteRequested, {
            term: this.state.currentTerm,
            candidateId: this.config.nodeId,
        });
        // Start with 1 vote (self)
        let votesReceived = 1;
        const majority = Math.floor((this.config.peers.length + 1) / 2) + 1;
        // Request votes from all peers
        if (!this.transport) {
            this.resetElectionTimer();
            return;
        }
        const votePromises = this.config.peers.map(async (peerId) => {
            try {
                const response = await this.transport.requestVote(peerId, {
                    term: this.state.currentTerm,
                    candidateId: this.config.nodeId,
                    lastLogIndex: this.state.log.lastIndex,
                    lastLogTerm: this.state.log.lastTerm,
                });
                // If response term is higher, become follower
                if (response.term > this.state.currentTerm) {
                    await this.state.setTerm(response.term);
                    this.transitionTo(types_js_1.NodeState.Follower);
                    return;
                }
                if (response.voteGranted && this.nodeState === types_js_1.NodeState.Candidate) {
                    votesReceived++;
                    if (votesReceived >= majority) {
                        this.transitionTo(types_js_1.NodeState.Leader);
                    }
                }
            }
            catch {
                // Peer unavailable, continue
            }
        });
        await Promise.allSettled(votePromises);
        // If still candidate, restart election timer
        if (this.nodeState === types_js_1.NodeState.Candidate) {
            this.resetElectionTimer();
        }
    }
    startHeartbeat() {
        if (this.heartbeatTimer) {
            clearInterval(this.heartbeatTimer);
        }
        // Send immediate heartbeat
        this.replicateToFollowers();
        // Start periodic heartbeat
        this.heartbeatTimer = setInterval(() => {
            if (this.nodeState === types_js_1.NodeState.Leader) {
                this.replicateToFollowers();
            }
        }, this.config.heartbeatInterval);
    }
    async replicateToFollowers() {
        if (!this.transport || this.nodeState !== types_js_1.NodeState.Leader)
            return;
        const replicationPromises = this.config.peers.map(async (peerId) => {
            await this.replicateToPeer(peerId);
        });
        await Promise.allSettled(replicationPromises);
        // Update commit index if majority have replicated
        if (this.state.updateCommitIndex()) {
            this.emit(types_js_1.RaftEvent.LogCommitted, {
                index: this.state.commitIndex,
                term: this.state.currentTerm,
            });
            await this.applyCommitted();
        }
    }
    async replicateToPeer(peerId) {
        if (!this.transport || this.nodeState !== types_js_1.NodeState.Leader)
            return;
        const nextIndex = this.state.getNextIndex(peerId);
        const prevLogIndex = nextIndex - 1;
        const prevLogTerm = this.state.log.termAt(prevLogIndex) ?? 0;
        const entries = this.state.log.getFrom(nextIndex, this.config.maxEntriesPerRequest);
        try {
            const response = await this.transport.appendEntries(peerId, {
                term: this.state.currentTerm,
                leaderId: this.config.nodeId,
                prevLogIndex,
                prevLogTerm,
                entries,
                leaderCommit: this.state.commitIndex,
            });
            if (response.term > this.state.currentTerm) {
                await this.state.setTerm(response.term);
                this.transitionTo(types_js_1.NodeState.Follower);
                return;
            }
            if (response.success) {
                if (response.matchIndex !== undefined) {
                    this.state.setNextIndex(peerId, response.matchIndex + 1);
                    this.state.setMatchIndex(peerId, response.matchIndex);
                }
                else if (entries.length > 0) {
                    const lastEntry = entries[entries.length - 1];
                    this.state.setNextIndex(peerId, lastEntry.index + 1);
                    this.state.setMatchIndex(peerId, lastEntry.index);
                }
            }
            else {
                // Decrement nextIndex and retry
                this.state.setNextIndex(peerId, nextIndex - 1);
            }
        }
        catch {
            // Peer unavailable, will retry on next heartbeat
        }
    }
    async applyCommitted() {
        while (this.state.lastApplied < this.state.commitIndex) {
            const nextIndex = this.state.lastApplied + 1;
            const entry = this.state.log.get(nextIndex);
            if (entry && this.stateMachine) {
                try {
                    await this.stateMachine.apply(entry.command);
                    this.state.setLastApplied(nextIndex);
                    this.emit(types_js_1.RaftEvent.LogApplied, entry);
                }
                catch (error) {
                    this.emit(types_js_1.RaftEvent.Error, error);
                    break;
                }
            }
            else {
                this.state.setLastApplied(nextIndex);
            }
        }
    }
}
exports.RaftNode = RaftNode;
//# sourceMappingURL=node.js.map