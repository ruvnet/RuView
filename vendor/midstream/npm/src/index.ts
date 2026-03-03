/**
 * MidStream - Real-time LLM Streaming with Lean Agentic Learning
 *
 * Main exports for npm package
 */

export { MidStreamAgent } from './agent.js';
export { WebSocketStreamServer, SSEStreamServer, HTTPStreamingClient } from './streaming.js';
export { MidStreamMCPServer } from './mcp-server.js';
export {
  OpenAIRealtimeClient,
  AgenticFlowProxyClient,
  createDefaultSessionConfig,
  audioToBase64,
  base64ToAudio
} from './openai-realtime.js';
export { MidStreamDashboard, InteractiveDashboard } from './dashboard.js';
export {
  RestreamClient,
  WebRTCSignalingServer,
  StreamSimulator
} from './restream-integration.js';
export {
  QuicConnection,
  QuicServer,
  QuicClient,
  QuicStream,
  createQuicServer,
  connectQuic,
  isQuicSupported
} from './quic-integration.js';

// Re-export types
export type {
  AgentConfig,
  AnalysisResult,
  BehaviorAnalysis,
} from './agent.js';

export type {
  RealtimeConfig,
  SessionConfig,
  ConversationItem,
  RealtimeMessage,
} from './openai-realtime.js';

export type {
  DashboardState,
  StreamMetrics
} from './dashboard.js';

export type {
  RestreamConfig,
  StreamFrame,
  AudioChunk,
  StreamAnalysis,
  DetectedObject
} from './restream-integration.js';

export type {
  QuicConfig,
  QuicStreamConfig,
  QuicConnectionStats
} from './quic-integration.js';
