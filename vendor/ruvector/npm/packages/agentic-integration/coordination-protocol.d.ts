/**
 * Coordination Protocol - Inter-agent communication and consensus
 *
 * Handles:
 * - Inter-agent messaging
 * - Consensus for critical operations
 * - Event-driven coordination
 * - Pub/Sub integration
 */
import { EventEmitter } from 'events';
export interface Message {
    id: string;
    type: 'request' | 'response' | 'broadcast' | 'consensus';
    from: string;
    to?: string | string[];
    topic?: string;
    payload: any;
    timestamp: number;
    ttl: number;
    priority: number;
}
export interface ConsensusProposal {
    id: string;
    proposer: string;
    type: 'schema_change' | 'topology_change' | 'critical_operation';
    data: any;
    requiredVotes: number;
    deadline: number;
    votes: Map<string, boolean>;
    status: 'pending' | 'accepted' | 'rejected' | 'expired';
}
export interface PubSubTopic {
    name: string;
    subscribers: Set<string>;
    messageHistory: Message[];
    maxHistorySize: number;
}
export interface CoordinationProtocolConfig {
    nodeId: string;
    heartbeatInterval: number;
    messageTimeout: number;
    consensusTimeout: number;
    maxMessageQueueSize: number;
    enableClaudeFlowHooks: boolean;
    pubSubTopics: string[];
}
export declare class CoordinationProtocol extends EventEmitter {
    private config;
    private messageQueue;
    private sentMessages;
    private pendingResponses;
    private consensusProposals;
    private pubSubTopics;
    private knownNodes;
    private lastHeartbeat;
    private heartbeatTimer?;
    private messageProcessingTimer?;
    private messageCounter;
    constructor(config: CoordinationProtocolConfig);
    /**
     * Initialize coordination protocol
     */
    private initialize;
    /**
     * Send message to another node
     */
    sendMessage(to: string, type: Message['type'], payload: any, options?: {
        topic?: string;
        ttl?: number;
        priority?: number;
        expectResponse?: boolean;
    }): Promise<any>;
    /**
     * Broadcast message to all nodes
     */
    broadcastMessage(type: Message['type'], payload: any, options?: {
        topic?: string;
        ttl?: number;
        priority?: number;
    }): Promise<void>;
    /**
     * Receive and handle message
     */
    receiveMessage(message: Message): Promise<void>;
    /**
     * Handle request message
     */
    private handleRequest;
    /**
     * Send response to a request
     */
    sendResponse(requestId: string, to: string, payload: any): Promise<void>;
    /**
     * Handle response message
     */
    private handleResponse;
    /**
     * Handle broadcast message
     */
    private handleBroadcast;
    /**
     * Propose consensus for critical operation
     */
    proposeConsensus(type: ConsensusProposal['type'], data: any, requiredVotes?: number): Promise<boolean>;
    /**
     * Handle consensus message
     */
    private handleConsensusMessage;
    /**
     * Handle consensus proposal
     */
    private handleConsensusProposal;
    /**
     * Handle consensus vote
     */
    private handleConsensusVote;
    /**
     * Create pub/sub topic
     */
    createTopic(name: string, maxHistorySize?: number): void;
    /**
     * Subscribe to pub/sub topic
     */
    subscribe(topicName: string, subscriberId: string): void;
    /**
     * Unsubscribe from pub/sub topic
     */
    unsubscribe(topicName: string, subscriberId: string): void;
    /**
     * Publish message to topic
     */
    publishToTopic(topicName: string, payload: any): Promise<void>;
    /**
     * Deliver message to topic subscribers
     */
    private deliverToTopic;
    /**
     * Enqueue message for processing
     */
    private enqueueMessage;
    /**
     * Start message processing loop
     */
    private startMessageProcessing;
    /**
     * Process queued messages
     */
    private processMessages;
    /**
     * Start heartbeat mechanism
     */
    private startHeartbeat;
    /**
     * Send heartbeat to all known nodes
     */
    private sendHeartbeat;
    /**
     * Check health of known nodes
     */
    private checkNodeHealth;
    /**
     * Register a node in the network
     */
    registerNode(nodeId: string): void;
    /**
     * Unregister a node from the network
     */
    unregisterNode(nodeId: string): void;
    /**
     * Get protocol status
     */
    getStatus(): {
        nodeId: string;
        knownNodes: number;
        queuedMessages: number;
        pendingResponses: number;
        activeConsensus: number;
        topics: string[];
    };
    /**
     * Shutdown protocol gracefully
     */
    shutdown(): Promise<void>;
}
//# sourceMappingURL=coordination-protocol.d.ts.map