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
import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

export interface Message {
  id: string;
  type: 'request' | 'response' | 'broadcast' | 'consensus';
  from: string;
  to?: string | string[]; // Single recipient or multiple for broadcast
  topic?: string;
  payload: any;
  timestamp: number;
  ttl: number; // Time to live in milliseconds
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

export class CoordinationProtocol extends EventEmitter {
  private messageQueue: Message[] = [];
  private sentMessages: Map<string, Message> = new Map();
  private pendingResponses: Map<string, {
    resolve: (value: any) => void;
    reject: (error: Error) => void;
    timeout: NodeJS.Timeout;
  }> = new Map();
  private consensusProposals: Map<string, ConsensusProposal> = new Map();
  private pubSubTopics: Map<string, PubSubTopic> = new Map();
  private knownNodes: Set<string> = new Set();
  private lastHeartbeat: Map<string, number> = new Map();
  private heartbeatTimer?: NodeJS.Timeout;
  private messageProcessingTimer?: NodeJS.Timeout;
  private messageCounter = 0;

  constructor(private config: CoordinationProtocolConfig) {
    super();
    this.initialize();
  }

  /**
   * Initialize coordination protocol
   */
  private async initialize(): Promise<void> {
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
        await execAsync(
          `npx claude-flow@alpha hooks pre-task --description "Initialize coordination protocol for node ${this.config.nodeId}"`
        );
      } catch (error) {
        console.warn(`[CoordinationProtocol:${this.config.nodeId}] Claude-flow hooks not available`);
      }
    }

    this.emit('protocol:initialized');

    console.log(`[CoordinationProtocol:${this.config.nodeId}] Protocol initialized`);
  }

  /**
   * Send message to another node
   */
  async sendMessage(
    to: string,
    type: Message['type'],
    payload: any,
    options: {
      topic?: string;
      ttl?: number;
      priority?: number;
      expectResponse?: boolean;
    } = {}
  ): Promise<any> {
    const message: Message = {
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

    console.log(
      `[CoordinationProtocol:${this.config.nodeId}] Sending ${type} message ${message.id} to ${to}`
    );

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
  async broadcastMessage(
    type: Message['type'],
    payload: any,
    options: {
      topic?: string;
      ttl?: number;
      priority?: number;
    } = {}
  ): Promise<void> {
    const recipients = Array.from(this.knownNodes);

    console.log(
      `[CoordinationProtocol:${this.config.nodeId}] Broadcasting ${type} message to ${recipients.length} nodes`
    );

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
  async receiveMessage(message: Message): Promise<void> {
    // Check if message is expired
    if (Date.now() - message.timestamp > message.ttl) {
      console.warn(
        `[CoordinationProtocol:${this.config.nodeId}] Received expired message ${message.id}`
      );
      return;
    }

    console.log(
      `[CoordinationProtocol:${this.config.nodeId}] Received ${message.type} message ${message.id} from ${message.from}`
    );

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
        console.warn(
          `[CoordinationProtocol:${this.config.nodeId}] Unknown message type: ${message.type}`
        );
    }

    // Update last contact time
    this.lastHeartbeat.set(message.from, Date.now());
    this.knownNodes.add(message.from);

    this.emit('message:received', message);
  }

  /**
   * Handle request message
   */
  private async handleRequest(message: Message): Promise<void> {
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
  async sendResponse(requestId: string, to: string, payload: any): Promise<void> {
    const response: Message = {
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
  private async handleResponse(message: Message): Promise<void> {
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
  private async handleBroadcast(message: Message): Promise<void> {
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
  async proposeConsensus(
    type: ConsensusProposal['type'],
    data: any,
    requiredVotes: number = Math.floor(this.knownNodes.size / 2) + 1
  ): Promise<boolean> {
    const proposal: ConsensusProposal = {
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

    console.log(
      `[CoordinationProtocol:${this.config.nodeId}] Proposing consensus ${proposal.id} (type: ${type})`
    );

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
        } else if (
          currentProposal.status === 'rejected' ||
          currentProposal.status === 'expired'
        ) {
          clearInterval(checkInterval);
          resolve(false);
        } else if (Date.now() > currentProposal.deadline) {
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
  private async handleConsensusMessage(message: Message): Promise<void> {
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
        console.warn(
          `[CoordinationProtocol:${this.config.nodeId}] Unknown consensus action: ${action}`
        );
    }
  }

  /**
   * Handle consensus proposal
   */
  private async handleConsensusProposal(proposalData: any, from: string): Promise<void> {
    console.log(
      `[CoordinationProtocol:${this.config.nodeId}] Received consensus proposal ${proposalData.id} from ${from}`
    );

    // Store proposal
    const proposal: ConsensusProposal = {
      ...proposalData,
      votes: new Map([[proposalData.proposer, true]]),
      status: 'pending' as const,
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
  private async handleConsensusVote(
    proposalId: string,
    voter: string,
    approve: boolean
  ): Promise<void> {
    const proposal = this.consensusProposals.get(proposalId);

    if (!proposal || proposal.status !== 'pending') {
      return;
    }

    console.log(
      `[CoordinationProtocol:${this.config.nodeId}] Received ${approve ? 'approval' : 'rejection'} vote from ${voter} for proposal ${proposalId}`
    );

    // Record vote
    proposal.votes.set(voter, approve);

    // Count votes
    const approvals = Array.from(proposal.votes.values()).filter(v => v).length;
    const rejections = proposal.votes.size - approvals;

    // Check if consensus reached
    if (approvals >= proposal.requiredVotes) {
      proposal.status = 'accepted';
      console.log(
        `[CoordinationProtocol:${this.config.nodeId}] Consensus ${proposalId} accepted (${approvals}/${proposal.requiredVotes} votes)`
      );
      this.emit('consensus:accepted', proposal);
    } else if (rejections > this.knownNodes.size - proposal.requiredVotes) {
      proposal.status = 'rejected';
      console.log(
        `[CoordinationProtocol:${this.config.nodeId}] Consensus ${proposalId} rejected (${rejections} rejections)`
      );
      this.emit('consensus:rejected', proposal);
    }
  }

  /**
   * Create pub/sub topic
   */
  createTopic(name: string, maxHistorySize: number = 100): void {
    if (this.pubSubTopics.has(name)) {
      console.warn(`[CoordinationProtocol:${this.config.nodeId}] Topic ${name} already exists`);
      return;
    }

    const topic: PubSubTopic = {
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
  subscribe(topicName: string, subscriberId: string): void {
    const topic = this.pubSubTopics.get(topicName);

    if (!topic) {
      throw new Error(`Topic ${topicName} does not exist`);
    }

    topic.subscribers.add(subscriberId);

    console.log(
      `[CoordinationProtocol:${this.config.nodeId}] Node ${subscriberId} subscribed to topic ${topicName}`
    );

    this.emit('topic:subscribed', { topicName, subscriberId });
  }

  /**
   * Unsubscribe from pub/sub topic
   */
  unsubscribe(topicName: string, subscriberId: string): void {
    const topic = this.pubSubTopics.get(topicName);

    if (!topic) {
      return;
    }

    topic.subscribers.delete(subscriberId);

    console.log(
      `[CoordinationProtocol:${this.config.nodeId}] Node ${subscriberId} unsubscribed from topic ${topicName}`
    );

    this.emit('topic:unsubscribed', { topicName, subscriberId });
  }

  /**
   * Publish message to topic
   */
  async publishToTopic(topicName: string, payload: any): Promise<void> {
    const topic = this.pubSubTopics.get(topicName);

    if (!topic) {
      throw new Error(`Topic ${topicName} does not exist`);
    }

    console.log(
      `[CoordinationProtocol:${this.config.nodeId}] Publishing to topic ${topicName} (${topic.subscribers.size} subscribers)`
    );

    // Broadcast to all subscribers
    for (const subscriber of topic.subscribers) {
      await this.sendMessage(subscriber, 'broadcast', payload, {
        topic: topicName,
      });
    }

    // Store in message history
    const message: Message = {
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
  private deliverToTopic(message: Message, topic: PubSubTopic): void {
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
  private enqueueMessage(message: Message): void {
    if (this.messageQueue.length >= this.config.maxMessageQueueSize) {
      console.warn(
        `[CoordinationProtocol:${this.config.nodeId}] Message queue full, dropping lowest priority message`
      );

      // Remove lowest priority message
      this.messageQueue.sort((a, b) => b.priority - a.priority);
      this.messageQueue.pop();
    }

    // Insert message by priority
    let insertIndex = this.messageQueue.findIndex(m => m.priority < message.priority);
    if (insertIndex === -1) {
      this.messageQueue.push(message);
    } else {
      this.messageQueue.splice(insertIndex, 0, message);
    }
  }

  /**
   * Start message processing loop
   */
  private startMessageProcessing(): void {
    this.messageProcessingTimer = setInterval(() => {
      this.processMessages();
    }, 10); // Process every 10ms
  }

  /**
   * Process queued messages
   */
  private async processMessages(): Promise<void> {
    while (this.messageQueue.length > 0) {
      const message = this.messageQueue.shift()!;

      // Check if message expired
      if (Date.now() - message.timestamp > message.ttl) {
        console.warn(
          `[CoordinationProtocol:${this.config.nodeId}] Message ${message.id} expired before processing`
        );
        continue;
      }

      // Simulate message transmission (replace with actual network call)
      this.emit('message:transmit', message);
    }
  }

  /**
   * Start heartbeat mechanism
   */
  private startHeartbeat(): void {
    this.heartbeatTimer = setInterval(() => {
      this.sendHeartbeat();
      this.checkNodeHealth();
    }, this.config.heartbeatInterval);
  }

  /**
   * Send heartbeat to all known nodes
   */
  private async sendHeartbeat(): Promise<void> {
    await this.broadcastMessage('request', {
      type: 'heartbeat',
      nodeId: this.config.nodeId,
      timestamp: Date.now(),
    });
  }

  /**
   * Check health of known nodes
   */
  private checkNodeHealth(): void {
    const now = Date.now();
    const unhealthyThreshold = this.config.heartbeatInterval * 3;

    for (const [nodeId, lastSeen] of this.lastHeartbeat.entries()) {
      if (now - lastSeen > unhealthyThreshold) {
        console.warn(
          `[CoordinationProtocol:${this.config.nodeId}] Node ${nodeId} appears unhealthy (last seen ${Math.floor((now - lastSeen) / 1000)}s ago)`
        );

        this.emit('node:unhealthy', { nodeId, lastSeen });
      }
    }
  }

  /**
   * Register a node in the network
   */
  registerNode(nodeId: string): void {
    this.knownNodes.add(nodeId);
    this.lastHeartbeat.set(nodeId, Date.now());

    console.log(`[CoordinationProtocol:${this.config.nodeId}] Registered node: ${nodeId}`);

    this.emit('node:registered', { nodeId });
  }

  /**
   * Unregister a node from the network
   */
  unregisterNode(nodeId: string): void {
    this.knownNodes.delete(nodeId);
    this.lastHeartbeat.delete(nodeId);

    console.log(`[CoordinationProtocol:${this.config.nodeId}] Unregistered node: ${nodeId}`);

    this.emit('node:unregistered', { nodeId });
  }

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
  } {
    return {
      nodeId: this.config.nodeId,
      knownNodes: this.knownNodes.size,
      queuedMessages: this.messageQueue.length,
      pendingResponses: this.pendingResponses.size,
      activeConsensus: Array.from(this.consensusProposals.values()).filter(
        p => p.status === 'pending'
      ).length,
      topics: Array.from(this.pubSubTopics.keys()),
    };
  }

  /**
   * Shutdown protocol gracefully
   */
  async shutdown(): Promise<void> {
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
        await execAsync(
          `npx claude-flow@alpha hooks post-task --task-id "protocol-${this.config.nodeId}-shutdown"`
        );
      } catch (error) {
        console.warn(`[CoordinationProtocol:${this.config.nodeId}] Error executing shutdown hooks`);
      }
    }

    this.emit('protocol:shutdown');
  }
}
