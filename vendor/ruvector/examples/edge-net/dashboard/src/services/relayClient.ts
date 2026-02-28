/**
 * Edge-Net Relay WebSocket Client
 *
 * Provides real-time connection to the Edge-Net relay server for:
 * - Node registration and presence
 * - Task distribution and completion
 * - Credit synchronization
 * - Time Crystal phase sync
 */

export interface RelayMessage {
  type: string;
  [key: string]: unknown;
}

export interface NetworkState {
  genesisTime: number;
  totalNodes: number;
  activeNodes: number;
  totalTasks: number;
  totalRuvDistributed: bigint;
  timeCrystalPhase: number;
}

export interface TaskAssignment {
  id: string;
  submitter: string;
  taskType: string;
  payload: Uint8Array;
  maxCredits: bigint;
  submittedAt: number;
}

export interface RelayEventHandlers {
  onConnected?: (nodeId: string, networkState: NetworkState, peers: string[]) => void;
  onDisconnected?: () => void;
  onNodeJoined?: (nodeId: string, totalNodes: number) => void;
  onNodeLeft?: (nodeId: string, totalNodes: number) => void;
  onTaskAssigned?: (task: TaskAssignment) => void;
  onTaskResult?: (taskId: string, result: unknown, processedBy: string) => void;
  onCreditEarned?: (amount: bigint, taskId: string) => void;
  onTimeCrystalSync?: (phase: number, timestamp: number, activeNodes: number) => void;
  onPeerMessage?: (from: string, payload: unknown) => void;
  onError?: (error: Error) => void;
}

const RECONNECT_DELAYS = [1000, 2000, 5000, 10000, 30000]; // Exponential backoff

class RelayClient {
  private ws: WebSocket | null = null;
  private nodeId: string | null = null;
  private relayUrl: string;
  private handlers: RelayEventHandlers = {};
  private reconnectAttempt = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  private isConnecting = false;
  private shouldReconnect = true;

  constructor(relayUrl: string = 'wss://edge-net-relay-875130704813.us-central1.run.app') {
    this.relayUrl = relayUrl;
  }

  /**
   * Set event handlers
   */
  setHandlers(handlers: RelayEventHandlers): void {
    this.handlers = { ...this.handlers, ...handlers };
  }

  /**
   * Connect to the relay server
   */
  async connect(nodeId: string): Promise<boolean> {
    if (this.ws?.readyState === WebSocket.OPEN) {
      console.log('[RelayClient] Already connected');
      return true;
    }

    if (this.isConnecting) {
      console.log('[RelayClient] Connection already in progress');
      return false;
    }

    this.nodeId = nodeId;
    this.shouldReconnect = true;
    this.isConnecting = true;

    return new Promise((resolve) => {
      try {
        console.log(`[RelayClient] Connecting to ${this.relayUrl}...`);
        this.ws = new WebSocket(this.relayUrl);

        this.ws.onopen = () => {
          console.log('[RelayClient] WebSocket connected');
          this.isConnecting = false;
          this.reconnectAttempt = 0;

          // Register with relay
          this.send({
            type: 'register',
            nodeId: this.nodeId,
            capabilities: ['compute', 'storage'],
            version: '0.1.0',
          });

          // Start heartbeat
          this.startHeartbeat();
        };

        this.ws.onmessage = (event) => {
          this.handleMessage(event.data);
        };

        this.ws.onclose = (event) => {
          console.log(`[RelayClient] WebSocket closed: ${event.code} ${event.reason}`);
          this.isConnecting = false;
          this.stopHeartbeat();
          this.handlers.onDisconnected?.();

          if (this.shouldReconnect) {
            this.scheduleReconnect();
          }
        };

        this.ws.onerror = (error) => {
          console.error('[RelayClient] WebSocket error:', error);
          this.isConnecting = false;
          this.handlers.onError?.(new Error('WebSocket connection failed'));
          resolve(false);
        };

        // Wait for welcome message to confirm connection
        const checkConnected = setInterval(() => {
          if (this.ws?.readyState === WebSocket.OPEN) {
            clearInterval(checkConnected);
            resolve(true);
          }
        }, 100);

        // Timeout after 10 seconds
        setTimeout(() => {
          clearInterval(checkConnected);
          if (this.ws?.readyState !== WebSocket.OPEN) {
            this.isConnecting = false;
            resolve(false);
          }
        }, 10000);

      } catch (error) {
        console.error('[RelayClient] Failed to create WebSocket:', error);
        this.isConnecting = false;
        resolve(false);
      }
    });
  }

  /**
   * Disconnect from the relay
   */
  disconnect(): void {
    this.shouldReconnect = false;
    this.stopHeartbeat();

    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }

    if (this.ws) {
      this.ws.close(1000, 'Client disconnect');
      this.ws = null;
    }

    console.log('[RelayClient] Disconnected');
  }

  /**
   * Check if connected
   */
  isConnected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  /**
   * Get current node ID
   */
  getNodeId(): string | null {
    return this.nodeId;
  }

  /**
   * Submit a task to the network
   */
  submitTask(taskType: string, payload: Uint8Array, maxCredits: bigint): void {
    this.send({
      type: 'task_submit',
      task: {
        taskType,
        payload: Array.from(payload), // Convert to array for JSON
        maxCredits: maxCredits.toString(),
      },
    });
  }

  /**
   * Report task completion
   */
  completeTask(taskId: string, submitterId: string, result: unknown, reward: bigint): void {
    this.send({
      type: 'task_complete',
      taskId,
      submitterId,
      result,
      reward: reward.toString(),
    });
  }

  /**
   * Send a message to a specific peer
   */
  sendToPeer(targetId: string, payload: unknown): void {
    this.send({
      type: 'peer_message',
      targetId,
      payload,
    });
  }

  /**
   * Broadcast a message to all peers
   */
  broadcast(payload: unknown): void {
    this.send({
      type: 'broadcast',
      payload,
    });
  }

  // ============================================================================
  // Private Methods
  // ============================================================================

  private send(message: RelayMessage): void {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(message));
    } else {
      console.warn('[RelayClient] Cannot send - not connected');
    }
  }

  private handleMessage(data: string): void {
    try {
      const message = JSON.parse(data) as RelayMessage;

      switch (message.type) {
        case 'welcome':
          console.log('[RelayClient] Registered with relay:', message.nodeId);
          this.handlers.onConnected?.(
            message.nodeId as string,
            {
              genesisTime: (message.networkState as NetworkState)?.genesisTime || Date.now(),
              totalNodes: (message.networkState as NetworkState)?.totalNodes || 0,
              activeNodes: (message.networkState as NetworkState)?.activeNodes || 0,
              totalTasks: (message.networkState as NetworkState)?.totalTasks || 0,
              totalRuvDistributed: BigInt((message.networkState as NetworkState)?.totalRuvDistributed?.toString() || '0'),
              timeCrystalPhase: (message.networkState as NetworkState)?.timeCrystalPhase || 0,
            },
            (message.peers as string[]) || []
          );
          break;

        case 'node_joined':
          console.log('[RelayClient] Node joined:', message.nodeId);
          this.handlers.onNodeJoined?.(
            message.nodeId as string,
            message.totalNodes as number
          );
          break;

        case 'node_left':
          console.log('[RelayClient] Node left:', message.nodeId);
          this.handlers.onNodeLeft?.(
            message.nodeId as string,
            message.totalNodes as number
          );
          break;

        case 'task_assignment':
          console.log('[RelayClient] Task assigned:', (message.task as TaskAssignment)?.id);
          const task = message.task as Record<string, unknown>;
          this.handlers.onTaskAssigned?.({
            id: task.id as string,
            submitter: task.submitter as string,
            taskType: task.taskType as string,
            payload: new Uint8Array(task.payload as number[]),
            maxCredits: BigInt(task.maxCredits as string || '0'),
            submittedAt: task.submittedAt as number,
          });
          break;

        case 'task_accepted':
          console.log('[RelayClient] Task accepted:', message.taskId);
          break;

        case 'task_result':
          console.log('[RelayClient] Task result:', message.taskId);
          this.handlers.onTaskResult?.(
            message.taskId as string,
            message.result,
            message.processedBy as string
          );
          break;

        case 'credit_earned':
          console.log('[RelayClient] Credit earned:', message.amount);
          this.handlers.onCreditEarned?.(
            BigInt(message.amount as string || '0'),
            message.taskId as string
          );
          break;

        case 'time_crystal_sync':
          this.handlers.onTimeCrystalSync?.(
            message.phase as number,
            message.timestamp as number,
            message.activeNodes as number
          );
          break;

        case 'peer_message':
          this.handlers.onPeerMessage?.(
            message.from as string,
            message.payload
          );
          break;

        case 'heartbeat_ack':
          // Heartbeat acknowledged
          break;

        case 'error':
          console.error('[RelayClient] Relay error:', message.message);
          this.handlers.onError?.(new Error(message.message as string));
          break;

        case 'relay_shutdown':
          console.warn('[RelayClient] Relay is shutting down');
          this.shouldReconnect = true; // Will reconnect when relay comes back
          break;

        default:
          console.log('[RelayClient] Unknown message type:', message.type);
      }
    } catch (error) {
      console.error('[RelayClient] Failed to parse message:', error);
    }
  }

  private startHeartbeat(): void {
    this.stopHeartbeat();
    this.heartbeatTimer = setInterval(() => {
      this.send({ type: 'heartbeat' });
    }, 15000); // Every 15 seconds
  }

  private stopHeartbeat(): void {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer) return;

    const delay = RECONNECT_DELAYS[Math.min(this.reconnectAttempt, RECONNECT_DELAYS.length - 1)];
    console.log(`[RelayClient] Reconnecting in ${delay}ms (attempt ${this.reconnectAttempt + 1})`);

    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.reconnectAttempt++;
      if (this.nodeId) {
        this.connect(this.nodeId);
      }
    }, delay);
  }
}

// Export singleton instance
export const relayClient = new RelayClient();

// Export class for testing
export { RelayClient };
