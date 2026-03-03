/**
 * Tests for OpenAI Realtime API Integration
 */

import { OpenAIRealtimeClient, AgenticFlowProxyClient, createDefaultSessionConfig, audioToBase64, base64ToAudio } from '../openai-realtime';
import WebSocket from 'ws';

// Mock WebSocket
jest.mock('ws');

describe('OpenAI Realtime Client', () => {
  let client: OpenAIRealtimeClient;
  let mockWs: any;

  beforeEach(() => {
    mockWs = {
      on: jest.fn(),
      send: jest.fn(),
      close: jest.fn(),
    };

    (WebSocket as any).mockImplementation(() => mockWs);

    client = new OpenAIRealtimeClient({
      apiKey: 'test-api-key',
      model: 'gpt-4o-realtime-preview-2024-10-01',
    });
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  describe('Connection', () => {
    it('should create client with config', () => {
      expect(client).toBeDefined();
      expect(client.isConnectedToOpenAI()).toBe(false);
    });

    it('should connect to OpenAI', async () => {
      const connectPromise = client.connect();

      // Simulate connection
      const openHandler = mockWs.on.mock.calls.find((call: any) => call[0] === 'open')[1];
      openHandler();

      await connectPromise;

      expect(client.isConnectedToOpenAI()).toBe(true);
    });

    it('should handle connection errors', async () => {
      const error = new Error('Connection failed');

      // Add error listener to prevent unhandled error
      const errorListener = jest.fn();
      client.on('error', errorListener);

      const connectPromise = client.connect();

      // Simulate error
      const errorHandler = mockWs.on.mock.calls.find((call: any) => call[0] === 'error')[1];
      const closeHandler = mockWs.on.mock.calls.find((call: any) => call[0] === 'close')[1];

      errorHandler(error);
      closeHandler();

      await expect(connectPromise).rejects.toThrow('Connection failed');
      expect(errorListener).toHaveBeenCalledWith(error);
    });

    it('should disconnect gracefully', async () => {
      // First connect the client
      const connectPromise = client.connect();
      const openHandler = mockWs.on.mock.calls.find((call: any) => call[0] === 'open')[1];
      openHandler();
      await connectPromise;

      // Then disconnect
      client.disconnect();
      expect(mockWs.close).toHaveBeenCalled();
    });
  });

  describe('Message Handling', () => {
    beforeEach(async () => {
      const connectPromise = client.connect();
      const openHandler = mockWs.on.mock.calls.find((call: any) => call[0] === 'open')[1];
      openHandler();
      await connectPromise;
    });

    it('should handle session.created message', () => {
      const messageHandler = mockWs.on.mock.calls.find((call: any) => call[0] === 'message')[1];

      const sessionCreatedHandler = jest.fn();
      client.on('session.created', sessionCreatedHandler);

      const message = JSON.stringify({
        type: 'session.created',
        session: { id: 'sess_123' },
      });

      messageHandler(Buffer.from(message));

      expect(sessionCreatedHandler).toHaveBeenCalledWith({ id: 'sess_123' });
      expect(client.getSessionId()).toBe('sess_123');
    });

    it('should handle text delta messages', () => {
      const messageHandler = mockWs.on.mock.calls.find((call: any) => call[0] === 'message')[1];

      const deltaHandler = jest.fn();
      client.on('response.text.delta', deltaHandler);

      const message = JSON.stringify({
        type: 'response.text.delta',
        delta: 'Hello ',
      });

      messageHandler(Buffer.from(message));

      expect(deltaHandler).toHaveBeenCalledWith('Hello ');
    });

    it('should handle audio delta messages', () => {
      const messageHandler = mockWs.on.mock.calls.find((call: any) => call[0] === 'message')[1];

      const audioHandler = jest.fn();
      client.on('response.audio.delta', audioHandler);

      const message = JSON.stringify({
        type: 'response.audio.delta',
        delta: 'base64_audio_chunk',
      });

      messageHandler(Buffer.from(message));

      expect(audioHandler).toHaveBeenCalledWith('base64_audio_chunk');
    });

    it('should handle error messages', () => {
      const messageHandler = mockWs.on.mock.calls.find((call: any) => call[0] === 'message')[1];

      const errorHandler = jest.fn();
      client.on('error', errorHandler);

      const message = JSON.stringify({
        type: 'error',
        error: { message: 'API Error' },
      });

      messageHandler(Buffer.from(message));

      expect(errorHandler).toHaveBeenCalled();
    });
  });

  describe('Sending Messages', () => {
    beforeEach(async () => {
      const connectPromise = client.connect();
      const openHandler = mockWs.on.mock.calls.find((call: any) => call[0] === 'open')[1];
      openHandler();
      await connectPromise;
    });

    it('should send text message', () => {
      client.sendText('Hello, world!');

      expect(mockWs.send).toHaveBeenCalled();
      const sentMessage = JSON.parse(mockWs.send.mock.calls[0][0]);

      expect(sentMessage.type).toBe('conversation.item.create');
      expect(sentMessage.item.content[0].text).toBe('Hello, world!');
    });

    it('should send audio', () => {
      client.sendAudio('base64_audio_data');

      expect(mockWs.send).toHaveBeenCalled();
      const sentMessage = JSON.parse(mockWs.send.mock.calls[0][0]);

      expect(sentMessage.type).toBe('input_audio_buffer.append');
      expect(sentMessage.audio).toBe('base64_audio_data');
    });

    it('should commit audio buffer', () => {
      client.commitAudio();

      expect(mockWs.send).toHaveBeenCalled();
      const sentMessage = JSON.parse(mockWs.send.mock.calls[0][0]);

      expect(sentMessage.type).toBe('input_audio_buffer.commit');
    });

    it('should clear audio buffer', () => {
      client.clearAudio();

      expect(mockWs.send).toHaveBeenCalled();
      const sentMessage = JSON.parse(mockWs.send.mock.calls[0][0]);

      expect(sentMessage.type).toBe('input_audio_buffer.clear');
    });

    it('should create response', () => {
      client.createResponse({ modalities: ['text'] });

      expect(mockWs.send).toHaveBeenCalled();
      const sentMessage = JSON.parse(mockWs.send.mock.calls[0][0]);

      expect(sentMessage.type).toBe('response.create');
      expect(sentMessage.response.modalities).toEqual(['text']);
    });

    it('should cancel response', () => {
      client.cancelResponse();

      expect(mockWs.send).toHaveBeenCalled();
      const sentMessage = JSON.parse(mockWs.send.mock.calls[0][0]);

      expect(sentMessage.type).toBe('response.cancel');
    });
  });

  describe('Session Management', () => {
    beforeEach(async () => {
      const connectPromise = client.connect();
      const openHandler = mockWs.on.mock.calls.find((call: any) => call[0] === 'open')[1];
      openHandler();
      await connectPromise;
    });

    it('should update session configuration', () => {
      const config = {
        modalities: ['text', 'audio'],
        voice: 'alloy',
      };

      client.updateSession(config as any);

      expect(mockWs.send).toHaveBeenCalled();
      const sentMessage = JSON.parse(mockWs.send.mock.calls[0][0]);

      expect(sentMessage.type).toBe('session.update');
      expect(sentMessage.session).toEqual(config);
    });

    it('should delete conversation item', () => {
      client.deleteItem('item_123');

      expect(mockWs.send).toHaveBeenCalled();
      const sentMessage = JSON.parse(mockWs.send.mock.calls[0][0]);

      expect(sentMessage.type).toBe('conversation.item.delete');
      expect(sentMessage.item_id).toBe('item_123');
    });

    it('should truncate conversation', () => {
      client.truncateConversation('item_123', 0, 1000);

      expect(mockWs.send).toHaveBeenCalled();
      const sentMessage = JSON.parse(mockWs.send.mock.calls[0][0]);

      expect(sentMessage.type).toBe('conversation.item.truncate');
      expect(sentMessage.item_id).toBe('item_123');
      expect(sentMessage.audio_end_ms).toBe(1000);
    });
  });

  describe('MidStream Integration', () => {
    beforeEach(async () => {
      const connectPromise = client.connect();
      const openHandler = mockWs.on.mock.calls.find((call: any) => call[0] === 'open')[1];
      openHandler();
      await connectPromise;
    });

    it('should integrate with MidStream agent', () => {
      const agent = client.getAgent();
      expect(agent).toBeDefined();
    });

    it('should analyze conversation with MidStream', () => {
      const messageHandler = mockWs.on.mock.calls.find((call: any) => call[0] === 'message')[1];

      // Simulate conversation items
      const message1 = JSON.stringify({
        type: 'conversation.item.created',
        item: {
          id: 'item_1',
          type: 'message',
          role: 'user',
          content: [{ type: 'text', text: 'Hello' }],
        },
      });

      const message2 = JSON.stringify({
        type: 'conversation.item.created',
        item: {
          id: 'item_2',
          type: 'message',
          role: 'assistant',
          content: [{ type: 'text', text: 'Hi there!' }],
        },
      });

      messageHandler(Buffer.from(message1));
      messageHandler(Buffer.from(message2));

      const analysis = client.getMidStreamAnalysis();

      expect(analysis).toBeDefined();
      expect(analysis.messageCount).toBeGreaterThan(0);
    });

    it('should emit MidStream analysis events', () => {
      const messageHandler = mockWs.on.mock.calls.find((call: any) => call[0] === 'message')[1];
      const analysisHandler = jest.fn();

      client.on('midstream.analysis', analysisHandler);

      const message = JSON.stringify({
        type: 'conversation.item.created',
        item: {
          id: 'item_1',
          type: 'message',
          content: [{ type: 'text', text: 'Test message' }],
        },
      });

      messageHandler(Buffer.from(message));

      expect(analysisHandler).toHaveBeenCalled();
    });
  });

  describe('Conversation Management', () => {
    beforeEach(async () => {
      const connectPromise = client.connect();
      const openHandler = mockWs.on.mock.calls.find((call: any) => call[0] === 'open')[1];
      openHandler();
      await connectPromise;
    });

    it('should track conversation items', () => {
      const messageHandler = mockWs.on.mock.calls.find((call: any) => call[0] === 'message')[1];

      const message = JSON.stringify({
        type: 'conversation.item.created',
        item: {
          id: 'item_1',
          type: 'message',
          content: [{ type: 'text', text: 'Test' }],
        },
      });

      messageHandler(Buffer.from(message));

      const conversation = client.getConversation();
      expect(conversation.length).toBe(1);
      expect(conversation[0].id).toBe('item_1');
    });
  });
});

describe('AgenticFlowProxyClient', () => {
  let proxyClient: AgenticFlowProxyClient;

  beforeEach(() => {
    proxyClient = new AgenticFlowProxyClient({
      baseUrl: 'https://test-proxy.com',
      apiKey: 'test-key',
      openAiApiKey: 'openai-key',
    });
  });

  it('should create proxy client', () => {
    expect(proxyClient).toBeDefined();
  });

  it('should create realtime session', async () => {
    const mockWs = {
      on: jest.fn(),
      send: jest.fn(),
      close: jest.fn(),
    };

    (WebSocket as any).mockImplementation(() => mockWs);

    const sessionPromise = proxyClient.createRealtimeSession({
      apiKey: 'openai-key',
    });

    // Wait a bit for the WebSocket to be created
    await new Promise(resolve => setTimeout(resolve, 10));

    // Simulate connection
    const openHandler = mockWs.on.mock.calls.find((call: any) => call[0] === 'open')[1];
    if (openHandler) {
      openHandler();
    }

    const client = await sessionPromise;

    expect(client).toBeDefined();
    expect(proxyClient.getRealtimeClient()).toBe(client);
  });
});

describe('Helper Functions', () => {
  it('should convert audio to base64', () => {
    const buffer = Buffer.from('test audio data');
    const base64 = audioToBase64(buffer);

    expect(typeof base64).toBe('string');
    expect(base64.length).toBeGreaterThan(0);
  });

  it('should convert base64 to audio', () => {
    const base64 = Buffer.from('test audio data').toString('base64');
    const buffer = base64ToAudio(base64);

    expect(Buffer.isBuffer(buffer)).toBe(true);
    expect(buffer.toString()).toBe('test audio data');
  });

  it('should create default session config', () => {
    const config = createDefaultSessionConfig();

    expect(config.modalities).toEqual(['text', 'audio']);
    expect(config.voice).toBe('alloy');
    expect(config.temperature).toBe(0.8);
    expect(config.turn_detection).toBeDefined();
  });
});
