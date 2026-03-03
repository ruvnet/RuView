/**
 * OpenAI Realtime API Integration with MidStream
 *
 * Provides real-time audio and text streaming with OpenAI's Realtime API
 * Integrates with MidStream's temporal analysis and agentic-flow proxy
 */

import WebSocket from 'ws';
import { EventEmitter } from 'events';
import { MidStreamAgent } from './agent.js';

// ============================================================================
// Types and Interfaces
// ============================================================================

export interface RealtimeConfig {
  apiKey: string;
  model?: string;
  voice?: 'alloy' | 'echo' | 'fable' | 'onyx' | 'nova' | 'shimmer';
  temperature?: number;
  maxTokens?: number;
  agenticFlowProxy?: string;
  agenticFlowApiKey?: string;
}

export interface RealtimeMessage {
  type: string;
  [key: string]: any;
}

export interface ConversationItem {
  id: string;
  type: 'message' | 'function_call' | 'function_call_output';
  role?: 'user' | 'assistant' | 'system';
  content?: Array<{
    type: 'text' | 'audio';
    text?: string;
    audio?: string;
    transcript?: string;
  }>;
  status?: 'completed' | 'in_progress' | 'incomplete';
}

export interface SessionConfig {
  modalities: Array<'text' | 'audio'>;
  instructions?: string;
  voice?: string;
  input_audio_format?: 'pcm16' | 'g711_ulaw' | 'g711_alaw';
  output_audio_format?: 'pcm16' | 'g711_ulaw' | 'g711_alaw';
  input_audio_transcription?: {
    model: string;
  };
  turn_detection?: {
    type: 'server_vad';
    threshold?: number;
    prefix_padding_ms?: number;
    silence_duration_ms?: number;
  };
  tools?: Array<{
    type: 'function';
    name: string;
    description: string;
    parameters: object;
  }>;
  temperature?: number;
  max_response_output_tokens?: number;
}

// ============================================================================
// OpenAI Realtime Client
// ============================================================================

export class OpenAIRealtimeClient extends EventEmitter {
  private ws: WebSocket | null = null;
  private config: RealtimeConfig;
  private sessionId: string | null = null;
  private conversationItems: ConversationItem[] = [];
  private agent: MidStreamAgent;
  private isConnected: boolean = false;
  private reconnectAttempts: number = 0;
  private maxReconnectAttempts: number = 5;
  private messageQueue: RealtimeMessage[] = [];

  constructor(config: RealtimeConfig) {
    super();
    this.config = {
      model: config.model || 'gpt-4o-realtime-preview-2024-10-01',
      voice: config.voice || 'alloy',
      temperature: config.temperature || 0.8,
      maxTokens: config.maxTokens || 4096,
      ...config,
    };

    this.agent = new MidStreamAgent({
      maxHistory: 1000,
      embeddingDim: 3,
    });
  }

  /**
   * Connect to OpenAI Realtime API
   */
  async connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      try {
        // OpenAI Realtime API WebSocket URL
        const url = this.config.agenticFlowProxy
          ? `${this.config.agenticFlowProxy}/realtime`
          : 'wss://api.openai.com/v1/realtime';

        const headers: any = {
          'Authorization': `Bearer ${this.config.apiKey}`,
          'OpenAI-Beta': 'realtime=v1',
        };

        if (this.config.agenticFlowApiKey) {
          headers['X-Agentic-Flow-Key'] = this.config.agenticFlowApiKey;
        }

        this.ws = new WebSocket(`${url}?model=${this.config.model}`, {
          headers,
        });

        this.ws.on('open', () => {
          this.isConnected = true;
          this.reconnectAttempts = 0;
          this.emit('connected');

          // Send queued messages
          this.flushMessageQueue();

          resolve();
        });

        this.ws.on('message', (data: Buffer) => {
          this.handleMessage(JSON.parse(data.toString()));
        });

        this.ws.on('error', (error: Error) => {
          this.emit('error', error);
          reject(error);
        });

        this.ws.on('close', () => {
          this.isConnected = false;
          this.emit('disconnected');
          this.handleReconnect();
        });
      } catch (error) {
        reject(error);
      }
    });
  }

  /**
   * Disconnect from OpenAI Realtime API
   */
  disconnect(): void {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
      this.isConnected = false;
    }
  }

  /**
   * Handle incoming messages from OpenAI
   */
  private handleMessage(message: RealtimeMessage): void {
    this.emit('message', message);

    switch (message.type) {
      case 'session.created':
        this.sessionId = message.session.id;
        this.emit('session.created', message.session);
        break;

      case 'session.updated':
        this.emit('session.updated', message.session);
        break;

      case 'conversation.item.created':
        this.conversationItems.push(message.item);
        this.emit('conversation.item.created', message.item);

        // Analyze with MidStream
        if (message.item.content) {
          this.analyzeWithMidStream(message.item);
        }
        break;

      case 'conversation.item.input_audio_transcription.completed':
        this.emit('transcription.completed', message);
        break;

      case 'response.created':
        this.emit('response.created', message.response);
        break;

      case 'response.done':
        this.emit('response.done', message.response);
        break;

      case 'response.output_item.added':
        this.emit('response.output_item.added', message.item);
        break;

      case 'response.output_item.done':
        this.emit('response.output_item.done', message.item);
        break;

      case 'response.content_part.added':
        this.emit('response.content_part.added', message.part);
        break;

      case 'response.content_part.done':
        this.emit('response.content_part.done', message.part);
        break;

      case 'response.text.delta':
        this.emit('response.text.delta', message.delta);
        break;

      case 'response.text.done':
        this.emit('response.text.done', message.text);
        break;

      case 'response.audio.delta':
        this.emit('response.audio.delta', message.delta);
        break;

      case 'response.audio.done':
        this.emit('response.audio.done', message);
        break;

      case 'response.audio_transcript.delta':
        this.emit('response.audio_transcript.delta', message.delta);
        break;

      case 'response.audio_transcript.done':
        this.emit('response.audio_transcript.done', message.transcript);
        break;

      case 'response.function_call_arguments.delta':
        this.emit('response.function_call_arguments.delta', message.delta);
        break;

      case 'response.function_call_arguments.done':
        this.emit('response.function_call_arguments.done', message.arguments);
        break;

      case 'rate_limits.updated':
        this.emit('rate_limits.updated', message.rate_limits);
        break;

      case 'error':
        this.emit('error', new Error(message.error?.message || 'Unknown error'));
        break;

      default:
        this.emit('unknown_message', message);
    }
  }

  /**
   * Analyze conversation with MidStream
   */
  private analyzeWithMidStream(item: ConversationItem): void {
    if (!item.content) return;

    for (const content of item.content) {
      if (content.type === 'text' && content.text) {
        this.agent.processMessage(content.text);
      } else if (content.type === 'audio' && content.transcript) {
        this.agent.processMessage(content.transcript);
      }
    }

    // Emit analysis results
    const status = this.agent.getStatus();
    this.emit('midstream.analysis', status);
  }

  /**
   * Send a message to OpenAI
   */
  private send(message: RealtimeMessage): void {
    if (!this.isConnected || !this.ws) {
      this.messageQueue.push(message);
      return;
    }

    this.ws.send(JSON.stringify(message));
  }

  /**
   * Flush queued messages
   */
  private flushMessageQueue(): void {
    while (this.messageQueue.length > 0) {
      const message = this.messageQueue.shift();
      if (message && this.ws) {
        this.ws.send(JSON.stringify(message));
      }
    }
  }

  /**
   * Handle reconnection
   */
  private handleReconnect(): void {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      this.emit('reconnect_failed');
      return;
    }

    this.reconnectAttempts++;
    const delay = Math.min(1000 * Math.pow(2, this.reconnectAttempts), 30000);

    setTimeout(() => {
      this.emit('reconnecting', this.reconnectAttempts);
      this.connect().catch(() => {
        // Will retry in handleReconnect
      });
    }, delay);
  }

  /**
   * Update session configuration
   */
  updateSession(config: Partial<SessionConfig>): void {
    this.send({
      type: 'session.update',
      session: config,
    });
  }

  /**
   * Send text message
   */
  sendText(text: string): void {
    this.send({
      type: 'conversation.item.create',
      item: {
        type: 'message',
        role: 'user',
        content: [
          {
            type: 'text',
            text,
          },
        ],
      },
    });

    this.createResponse();
  }

  /**
   * Send audio message
   */
  sendAudio(audio: string): void {
    this.send({
      type: 'input_audio_buffer.append',
      audio,
    });
  }

  /**
   * Commit audio buffer
   */
  commitAudio(): void {
    this.send({
      type: 'input_audio_buffer.commit',
    });
  }

  /**
   * Clear audio buffer
   */
  clearAudio(): void {
    this.send({
      type: 'input_audio_buffer.clear',
    });
  }

  /**
   * Create a response
   */
  createResponse(config?: {
    modalities?: Array<'text' | 'audio'>;
    instructions?: string;
    voice?: string;
    temperature?: number;
    max_output_tokens?: number;
  }): void {
    this.send({
      type: 'response.create',
      response: config || {},
    });
  }

  /**
   * Cancel current response
   */
  cancelResponse(): void {
    this.send({
      type: 'response.cancel',
    });
  }

  /**
   * Truncate conversation
   */
  truncateConversation(itemId: string, contentIndex: number, audioEnd?: number): void {
    this.send({
      type: 'conversation.item.truncate',
      item_id: itemId,
      content_index: contentIndex,
      audio_end_ms: audioEnd,
    });
  }

  /**
   * Delete conversation item
   */
  deleteItem(itemId: string): void {
    this.send({
      type: 'conversation.item.delete',
      item_id: itemId,
    });
  }

  /**
   * Get conversation history
   */
  getConversation(): ConversationItem[] {
    return this.conversationItems;
  }

  /**
   * Get MidStream agent
   */
  getAgent(): MidStreamAgent {
    return this.agent;
  }

  /**
   * Get session ID
   */
  getSessionId(): string | null {
    return this.sessionId;
  }

  /**
   * Check if connected
   */
  isConnectedToOpenAI(): boolean {
    return this.isConnected;
  }

  /**
   * Get MidStream analysis
   */
  getMidStreamAnalysis(): any {
    const conversation = this.conversationItems
      .filter(item => item.content)
      .flatMap(item =>
        item.content!
          .filter(c => c.type === 'text' && c.text)
          .map(c => c.text!)
      );

    return this.agent.analyzeConversation(conversation);
  }
}

// ============================================================================
// Agentic Flow Proxy Client
// ============================================================================

export class AgenticFlowProxyClient {
  private baseUrl: string;
  private apiKey: string;
  private realtimeClient: OpenAIRealtimeClient | null = null;

  constructor(config: { baseUrl: string; apiKey: string; openAiApiKey: string }) {
    this.baseUrl = config.baseUrl;
    this.apiKey = config.apiKey;
  }

  /**
   * Create a realtime session through agentic-flow proxy
   */
  async createRealtimeSession(config: RealtimeConfig): Promise<OpenAIRealtimeClient> {
    const client = new OpenAIRealtimeClient({
      ...config,
      agenticFlowProxy: this.baseUrl,
      agenticFlowApiKey: this.apiKey,
    });

    await client.connect();
    this.realtimeClient = client;

    return client;
  }

  /**
   * Execute agentic workflow
   */
  async executeWorkflow(workflowId: string, inputs: any): Promise<any> {
    const response = await fetch(`${this.baseUrl}/workflows/${workflowId}/execute`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${this.apiKey}`,
      },
      body: JSON.stringify({ inputs }),
    });

    if (!response.ok) {
      throw new Error(`Workflow execution failed: ${response.statusText}`);
    }

    return response.json();
  }

  /**
   * Get current realtime client
   */
  getRealtimeClient(): OpenAIRealtimeClient | null {
    return this.realtimeClient;
  }
}

// ============================================================================
// Helper Functions
// ============================================================================

/**
 * Convert audio buffer to base64
 */
export function audioToBase64(buffer: Buffer): string {
  return buffer.toString('base64');
}

/**
 * Convert base64 to audio buffer
 */
export function base64ToAudio(base64: string): Buffer {
  return Buffer.from(base64, 'base64');
}

/**
 * Create default session config
 */
export function createDefaultSessionConfig(): SessionConfig {
  return {
    modalities: ['text', 'audio'],
    instructions: 'You are a helpful AI assistant integrated with MidStream for real-time conversation analysis.',
    voice: 'alloy',
    input_audio_format: 'pcm16',
    output_audio_format: 'pcm16',
    input_audio_transcription: {
      model: 'whisper-1',
    },
    turn_detection: {
      type: 'server_vad',
      threshold: 0.5,
      prefix_padding_ms: 300,
      silence_duration_ms: 200,
    },
    temperature: 0.8,
    max_response_output_tokens: 4096,
  };
}
