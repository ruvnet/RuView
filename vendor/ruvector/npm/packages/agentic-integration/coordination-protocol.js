"use strict";
/**
 * Coordination Protocol - Inter-agent communication and consensus
 *
 * Handles:
 * - Inter-agent messaging
 * - Consensus for critical operations
 * - Event-driven coordination
 * - Pub/Sub integration
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.CoordinationProtocol = void 0;
const events_1 = require("events");
const child_process_1 = require("child_process");
const util_1 = require("util");
const execAsync = (0, util_1.promisify)(child_process_1.exec);
class CoordinationProtocol extends events_1.EventEmitter {
    constructor(config) {
        super();
        this.config = config;
        this.messageQueue = [];
        this.sentMessages = new Map();
        this.pendingResponses = new Map();
        this.consensusProposals = new Map();
        this.pubSubTopics = new Map();
        this.knownNodes = new Set();
        this.lastHeartbeat = new Map();
        this.messageCounter = 0;
        this.initialize();
    }
    /**
     * Initialize coordination protocol
     */
    async initialize() {
        console.log(`[CoordinationProtocol:${this.config.nodeId}] Initializing protocol...`);
        // Initialize pub/sub topics
        for (const topicName of this.config.pubSubTopics) {
            this.createTopic(topicName);
        }
        // Start heartbeat
        this.startHeartbeat();
        // Start message processing
        this.startMessageProcessing();
        if (this.config.enableClaudeFlowHooks) {
            try {
                await execAsync(`npx claude-flow@alpha hooks pre-task --description "Initialize coordination protocol for node ${this.config.nodeId}"`);
            }
            catch (error) {
                console.warn(`[CoordinationProtocol:${this.config.nodeId}] Claude-flow hooks not available`);
            }
        }
        this.emit('protocol:initialized');
        console.log(`[CoordinationProtocol:${this.config.nodeId}] Protocol initialized`);
    }
    /**
     * Send message to another node
     */
    async sendMessage(to, type, payload, options = {}) {
        const message = {
            id: `msg-${this.config.nodeId}-${this.messageCounter++}`,
            type,
            from: this.config.nodeId,
            to,
            topic: options.topic,
            payload,
            timestamp: Date.now(),
            ttl: options.ttl || this.config.messageTimeout,
            priority: options.priority || 0,
        };
        console.log(`[CoordinationProtocol:${this.config.nodeId}] Sending ${type} message ${message.id} to ${to}`);
        // Add to queue
        this.enqueueMessage(message);
        // Track sent message
        this.sentMessages.set(message.id, message);
        // If expecting response, create promise
        if (options.expectResponse) {
            return new Promise((resolve, reject) => {
                const timeout = setTimeout(() => {
                    this.pendingResponses.delete(message.id);
                    reject(new Error(`Message ${message.id} timed out`));
                }, message.ttl);
                this.pendingResponses.set(message.id, {
                    resolve,
                    reject,
                    timeout,
                });
            });
        }
        this.emit('message:sent', message);
    }
    /**
     * Broadcast message to all nodes
     */
    async broadcastMessage(type, payload, options = {}) {
        const recipients = Array.from(this.knownNodes);
        console.log(`[CoordinationProtocol:${this.config.nodeId}] Broadcasting ${type} message to ${recipients.length} nodes`);
        for (const recipient of recipients) {
            await this.sendMessage(recipient, type, payload, {
                ...options,
                expectResponse: false,
            });
        }
        this.emit('message:broadcast', { type, recipientCount: recipients.length });
    }
    /**
     * Receive and handle message
     */
    async receiveMessage(message) {
        // Check if message is expired
        if (Date.now() - message.timestamp > message.ttl) {
            console.warn(`[CoordinationProtocol:${this.config.nodeId}] Received expired message ${message.id}`);
            return;
        }
        console.log(`[CoordinationProtocol:${this.config.nodeId}] Received ${message.type} message ${message.id} from ${message.from}`);
        // Handle different message types
        switch (message.type) {
            case 'request':
                await this.handleRequest(message);
                break;
            case 'response':
                await this.handleResponse(message);
                break;
            case 'broadcast':
                await this.handleBroadcast(message);
                break;
            case 'consensus':
                await this.handleConsensusMessage(message);
                break;
            default:
                console.warn(`[CoordinationProtocol:${this.config.nodeId}] Unknown message type: ${message.type}`);
        }
        // Update last contact time
        this.lastHeartbeat.set(message.from, Date.now());
        this.knownNodes.add(message.from);
        this.emit('message:received', message);
    }
    /**
     * Handle request message
     */
    async handleRequest(message) {
        this.emit('request:received', message);
        // Application can handle request and send response
        // Example auto-response for health checks
        if (message.payload.type === 'health_check') {
            await this.sendResponse(message.id, message.from, {
                status: 'healthy',
                timestamp: Date.now(),
            });
        }
    }
    /**
     * Send response to a request
     */
    async sendResponse(requestId, to, payload) {
        const response = {
            id: `resp-${requestId}`,
            type: 'response',
            from: this.config.nodeId,
            to,
            payload: {
                requestId,
                ...payload,
            },
            timestamp: Date.now(),
            ttl: this.config.messageTimeout,
            priority: 1,
        };
        await this.sendMessage(to, 'response', response.payload);
    }
    /**
     * Handle response message
     */
    async handleResponse(message) {
        const requestId = message.payload.requestId;
        const pending = this.pendingResponses.get(requestId);
        if (pending) {
            clearTimeout(pending.timeout);
            pending.resolve(message.payload);
            this.pendingResponses.delete(requestId);
        }
        this.emit('response:received', message);
    }
    /**
     * Handle broadcast message
     */
    async handleBroadcast(message) {
        // If message has topic, deliver to topic subscribers
        if (message.topic) {
            const topic = this.pubSubTopics.get(message.topic);
            if (topic) {
                this.deliverToTopic(message, topic);
            }
        }
        this.emit('broadcast:received', message);
    }
    /**
     * Propose consensus for critical operation
     */
    async proposeConsensus(type, data, requiredVotes = Math.floor(this.knownNodes.size / 2) + 1) {
        const proposal = {
            id: `consensus-${this.config.nodeId}-${Date.now()}`,
            proposer: this.config.nodeId,
            type,
            data,
            requiredVotes,
            deadline: Date.now() + this.config.consensusTimeout,
            votes: new Map([[this.config.nodeId, true]]), // Proposer votes yes
            status: 'pending',
        };
        this.consensusProposals.set(proposal.id, proposal);
        console.log(`[CoordinationProtocol:${this.config.nodeId}] Proposing consensus ${proposal.id} (type: ${type})`);
        // Broadcast consensus proposal
        await this.broadcastMessage('consensus', {
            action: 'propose',
            proposal: {
                id: proposal.id,
                proposer: proposal.proposer,
                type: proposal.type,
                data: proposal.data,
                requiredVotes: proposal.requiredVotes,
                deadline: proposal.deadline,
            },
        });
        // Wait for consensus
        return new Promise((resolve) => {
            const checkInterval = setInterval(() => {
                const currentProposal = this.consensusProposals.get(proposal.id);
                if (!currentProposal) {
                    clearInterval(checkInterval);
                    resolve(false);
                    return;
                }
                if (currentProposal.status === 'accepted') {
                    clearInterval(checkInterval);
                    resolve(true);
                }
                else if (currentProposal.status === 'rejected' ||
                    currentProposal.status === 'expired') {
                    clearInterval(checkInterval);
                    resolve(false);
                }
                else if (Date.now() > currentProposal.deadline) {
                    currentProposal.status = 'expired';
                    clearInterval(checkInterval);
                    resolve(false);
                }
            }, 100);
        });
    }
    /**
     * Handle consensus message
     */
    async handleConsensusMessage(message) {
        const { action, proposal, vote } = message.payload;
        switch (action) {
            case 'propose':
                // New proposal received
                await this.handleConsensusProposal(proposal, message.from);
                break;
            case 'vote':
                // Vote received for proposal
                await this.handleConsensusVote(vote.proposalId, message.from, vote.approve);
                break;
            default:
                console.warn(`[CoordinationProtocol:${this.config.nodeId}] Unknown consensus action: ${action}`);
        }
    }
    /**
     * Handle consensus proposal
     */
    async handleConsensusProposal(proposalData, from) {
        console.log(`[CoordinationProtocol:${this.config.nodeId}] Received consensus proposal ${proposalData.id} from ${from}`);
        // Store proposal
        const proposal = {
            ...proposalData,
            votes: new Map([[proposalData.proposer, true]]),
            status: 'pending',
        };
        this.consensusProposals.set(proposal.id, proposal);
        // Emit event for application to decide
        this.emit('consensus:proposed', proposal);
        // Auto-approve for demo (in production, application decides)
        const approve = true;
        // Send vote
        await this.sendMessage(proposal.proposer, 'consensus', {
            action: 'vote',
            vote: {
                proposalId: proposal.id,
                approve,
                voter: this.config.nodeId,
            },
        });
    }
    /**
     * Handle consensus vote
     */
    async handleConsensusVote(proposalId, voter, approve) {
        const proposal = this.consensusProposals.get(proposalId);
        if (!proposal || proposal.status !== 'pending') {
            return;
        }
        console.log(`[CoordinationProtocol:${this.config.nodeId}] Received ${approve ? 'approval' : 'rejection'} vote from ${voter} for proposal ${proposalId}`);
        // Record vote
        proposal.votes.set(voter, approve);
        // Count votes
        const approvals = Array.from(proposal.votes.values()).filter(v => v).length;
        const rejections = proposal.votes.size - approvals;
        // Check if consensus reached
        if (approvals >= proposal.requiredVotes) {
            proposal.status = 'accepted';
            console.log(`[CoordinationProtocol:${this.config.nodeId}] Consensus ${proposalId} accepted (${approvals}/${proposal.requiredVotes} votes)`);
            this.emit('consensus:accepted', proposal);
        }
        else if (rejections > this.knownNodes.size - proposal.requiredVotes) {
            proposal.status = 'rejected';
            console.log(`[CoordinationProtocol:${this.config.nodeId}] Consensus ${proposalId} rejected (${rejections} rejections)`);
            this.emit('consensus:rejected', proposal);
        }
    }
    /**
     * Create pub/sub topic
     */
    createTopic(name, maxHistorySize = 100) {
        if (this.pubSubTopics.has(name)) {
            console.warn(`[CoordinationProtocol:${this.config.nodeId}] Topic ${name} already exists`);
            return;
        }
        const topic = {
            name,
            subscribers: new Set(),
            messageHistory: [],
            maxHistorySize,
        };
        this.pubSubTopics.set(name, topic);
        console.log(`[CoordinationProtocol:${this.config.nodeId}] Created topic: ${name}`);
    }
    /**
     * Subscribe to pub/sub topic
     */
    subscribe(topicName, subscriberId) {
        const topic = this.pubSubTopics.get(topicName);
        if (!topic) {
            throw new Error(`Topic ${topicName} does not exist`);
        }
        topic.subscribers.add(subscriberId);
        console.log(`[CoordinationProtocol:${this.config.nodeId}] Node ${subscriberId} subscribed to topic ${topicName}`);
        this.emit('topic:subscribed', { topicName, subscriberId });
    }
    /**
     * Unsubscribe from pub/sub topic
     */
    unsubscribe(topicName, subscriberId) {
        const topic = this.pubSubTopics.get(topicName);
        if (!topic) {
            return;
        }
        topic.subscribers.delete(subscriberId);
        console.log(`[CoordinationProtocol:${this.config.nodeId}] Node ${subscriberId} unsubscribed from topic ${topicName}`);
        this.emit('topic:unsubscribed', { topicName, subscriberId });
    }
    /**
     * Publish message to topic
     */
    async publishToTopic(topicName, payload) {
        const topic = this.pubSubTopics.get(topicName);
        if (!topic) {
            throw new Error(`Topic ${topicName} does not exist`);
        }
        console.log(`[CoordinationProtocol:${this.config.nodeId}] Publishing to topic ${topicName} (${topic.subscribers.size} subscribers)`);
        // Broadcast to all subscribers
        for (const subscriber of topic.subscribers) {
            await this.sendMessage(subscriber, 'broadcast', payload, {
                topic: topicName,
            });
        }
        // Store in message history
        const message = {
            id: `topic-${topicName}-${Date.now()}`,
            type: 'broadcast',
            from: this.config.nodeId,
            topic: topicName,
            payload,
            timestamp: Date.now(),
            ttl: this.config.messageTimeout,
            priority: 0,
        };
        topic.messageHistory.push(message);
        // Trim history if needed
        if (topic.messageHistory.length > topic.maxHistorySize) {
            topic.messageHistory.shift();
        }
        this.emit('topic:published', { topicName, message });
    }
    /**
     * Deliver message to topic subscribers
     */
    deliverToTopic(message, topic) {
        // Store in history
        topic.messageHistory.push(message);
        if (topic.messageHistory.length > topic.maxHistorySize) {
            topic.messageHistory.shift();
        }
        // Emit to local subscribers
        this.emit('topic:message', {
            topicName: topic.name,
            message,
        });
    }
    /**
     * Enqueue message for processing
     */
    enqueueMessage(message) {
        if (this.messageQueue.length >= this.config.maxMessageQueueSize) {
            console.warn(`[CoordinationProtocol:${this.config.nodeId}] Message queue full, dropping lowest priority message`);
            // Remove lowest priority message
            this.messageQueue.sort((a, b) => b.priority - a.priority);
            this.messageQueue.pop();
        }
        // Insert message by priority
        let insertIndex = this.messageQueue.findIndex(m => m.priority < message.priority);
        if (insertIndex === -1) {
            this.messageQueue.push(message);
        }
        else {
            this.messageQueue.splice(insertIndex, 0, message);
        }
    }
    /**
     * Start message processing loop
     */
    startMessageProcessing() {
        this.messageProcessingTimer = setInterval(() => {
            this.processMessages();
        }, 10); // Process every 10ms
    }
    /**
     * Process queued messages
     */
    async processMessages() {
        while (this.messageQueue.length > 0) {
            const message = this.messageQueue.shift();
            // Check if message expired
            if (Date.now() - message.timestamp > message.ttl) {
                console.warn(`[CoordinationProtocol:${this.config.nodeId}] Message ${message.id} expired before processing`);
                continue;
            }
            // Simulate message transmission (replace with actual network call)
            this.emit('message:transmit', message);
        }
    }
    /**
     * Start heartbeat mechanism
     */
    startHeartbeat() {
        this.heartbeatTimer = setInterval(() => {
            this.sendHeartbeat();
            this.checkNodeHealth();
        }, this.config.heartbeatInterval);
    }
    /**
     * Send heartbeat to all known nodes
     */
    async sendHeartbeat() {
        await this.broadcastMessage('request', {
            type: 'heartbeat',
            nodeId: this.config.nodeId,
            timestamp: Date.now(),
        });
    }
    /**
     * Check health of known nodes
     */
    checkNodeHealth() {
        const now = Date.now();
        const unhealthyThreshold = this.config.heartbeatInterval * 3;
        for (const [nodeId, lastSeen] of this.lastHeartbeat.entries()) {
            if (now - lastSeen > unhealthyThreshold) {
                console.warn(`[CoordinationProtocol:${this.config.nodeId}] Node ${nodeId} appears unhealthy (last seen ${Math.floor((now - lastSeen) / 1000)}s ago)`);
                this.emit('node:unhealthy', { nodeId, lastSeen });
            }
        }
    }
    /**
     * Register a node in the network
     */
    registerNode(nodeId) {
        this.knownNodes.add(nodeId);
        this.lastHeartbeat.set(nodeId, Date.now());
        console.log(`[CoordinationProtocol:${this.config.nodeId}] Registered node: ${nodeId}`);
        this.emit('node:registered', { nodeId });
    }
    /**
     * Unregister a node from the network
     */
    unregisterNode(nodeId) {
        this.knownNodes.delete(nodeId);
        this.lastHeartbeat.delete(nodeId);
        console.log(`[CoordinationProtocol:${this.config.nodeId}] Unregistered node: ${nodeId}`);
        this.emit('node:unregistered', { nodeId });
    }
    /**
     * Get protocol status
     */
    getStatus() {
        return {
            nodeId: this.config.nodeId,
            knownNodes: this.knownNodes.size,
            queuedMessages: this.messageQueue.length,
            pendingResponses: this.pendingResponses.size,
            activeConsensus: Array.from(this.consensusProposals.values()).filter(p => p.status === 'pending').length,
            topics: Array.from(this.pubSubTopics.keys()),
        };
    }
    /**
     * Shutdown protocol gracefully
     */
    async shutdown() {
        console.log(`[CoordinationProtocol:${this.config.nodeId}] Shutting down protocol...`);
        // Stop timers
        if (this.heartbeatTimer) {
            clearInterval(this.heartbeatTimer);
        }
        if (this.messageProcessingTimer) {
            clearInterval(this.messageProcessingTimer);
        }
        // Process remaining messages
        await this.processMessages();
        // Clear pending responses
        for (const [messageId, pending] of this.pendingResponses.entries()) {
            clearTimeout(pending.timeout);
            pending.reject(new Error('Protocol shutting down'));
        }
        this.pendingResponses.clear();
        if (this.config.enableClaudeFlowHooks) {
            try {
                await execAsync(`npx claude-flow@alpha hooks post-task --task-id "protocol-${this.config.nodeId}-shutdown"`);
            }
            catch (error) {
                console.warn(`[CoordinationProtocol:${this.config.nodeId}] Error executing shutdown hooks`);
            }
        }
        this.emit('protocol:shutdown');
    }
}
exports.CoordinationProtocol = CoordinationProtocol;
//# sourceMappingURL=coordination-protocol.js.map