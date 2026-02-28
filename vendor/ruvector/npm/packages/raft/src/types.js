"use strict";
/**
 * Raft Consensus Types
 * Based on the Raft paper specification
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.RaftEvent = exports.RaftErrorCode = exports.RaftError = exports.NodeState = void 0;
/** Possible states of a Raft node */
var NodeState;
(function (NodeState) {
    NodeState["Follower"] = "follower";
    NodeState["Candidate"] = "candidate";
    NodeState["Leader"] = "leader";
})(NodeState || (exports.NodeState = NodeState = {}));
/** Raft error types */
class RaftError extends Error {
    constructor(message, code) {
        super(message);
        this.code = code;
        this.name = 'RaftError';
    }
    static notLeader() {
        return new RaftError('Node is not the leader', RaftErrorCode.NotLeader);
    }
    static noLeader() {
        return new RaftError('No leader available', RaftErrorCode.NoLeader);
    }
    static electionTimeout() {
        return new RaftError('Election timeout', RaftErrorCode.ElectionTimeout);
    }
    static logInconsistency() {
        return new RaftError('Log inconsistency detected', RaftErrorCode.LogInconsistency);
    }
}
exports.RaftError = RaftError;
var RaftErrorCode;
(function (RaftErrorCode) {
    RaftErrorCode["NotLeader"] = "NOT_LEADER";
    RaftErrorCode["NoLeader"] = "NO_LEADER";
    RaftErrorCode["InvalidTerm"] = "INVALID_TERM";
    RaftErrorCode["InvalidLogIndex"] = "INVALID_LOG_INDEX";
    RaftErrorCode["ElectionTimeout"] = "ELECTION_TIMEOUT";
    RaftErrorCode["LogInconsistency"] = "LOG_INCONSISTENCY";
    RaftErrorCode["SnapshotFailed"] = "SNAPSHOT_FAILED";
    RaftErrorCode["ConfigError"] = "CONFIG_ERROR";
    RaftErrorCode["Internal"] = "INTERNAL";
})(RaftErrorCode || (exports.RaftErrorCode = RaftErrorCode = {}));
/** Event types emitted by RaftNode */
var RaftEvent;
(function (RaftEvent) {
    RaftEvent["StateChange"] = "stateChange";
    RaftEvent["LeaderElected"] = "leaderElected";
    RaftEvent["LogAppended"] = "logAppended";
    RaftEvent["LogCommitted"] = "logCommitted";
    RaftEvent["LogApplied"] = "logApplied";
    RaftEvent["VoteRequested"] = "voteRequested";
    RaftEvent["VoteGranted"] = "voteGranted";
    RaftEvent["Heartbeat"] = "heartbeat";
    RaftEvent["Error"] = "error";
})(RaftEvent || (exports.RaftEvent = RaftEvent = {}));
//# sourceMappingURL=types.js.map