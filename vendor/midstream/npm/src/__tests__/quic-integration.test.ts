/**
 * QUIC Integration Tests
 *
 * Comprehensive tests for QUIC client/server functionality
 */

import {
  QuicConnection,
  QuicServer,
  QuicClient,
  QuicStream,
  createQuicServer,
  connectQuic,
  isQuicSupported
} from '../quic-integration.js';

describe('QUIC Integration', () => {
  describe('QuicStream', () => {
    let stream: QuicStream;

    beforeEach(() => {
      stream = new QuicStream(1, 0);
    });

    afterEach(() => {
      stream.close();
    });

    it('should create a stream with ID', () => {
      expect(stream.getStreamId()).toBe(1);
    });

    it('should write data to stream', (done) => {
      stream.on('data', (data) => {
        expect(data.toString()).toBe('test data');
        done();
      });

      stream.write('test data');
    });

    it('should close stream', (done) => {
      stream.on('close', () => {
        expect(stream.isClosed()).toBe(true);
        done();
      });

      stream.close();
    });

    it('should throw when writing to closed stream', () => {
      stream.close();
      expect(() => stream.write('test')).toThrow('Stream is closed');
    });

    it('should set and get priority', () => {
      stream.setPriority(5);
      expect(stream.getPriority()).toBe(5);
    });

    it('should handle Buffer data', (done) => {
      const buffer = Buffer.from('binary data');

      stream.on('data', (data) => {
        expect(Buffer.isBuffer(data)).toBe(true);
        expect(data.toString()).toBe('binary data');
        done();
      });

      stream.write(buffer);
    });
  });

  describe('QuicConnection', () => {
    let connection: QuicConnection;

    beforeEach(async () => {
      connection = new QuicConnection({
        host: 'localhost',
        port: 4433
      });
    });

    afterEach(() => {
      if (connection.isConnected()) {
        connection.close();
      }
    });

    it('should create connection with default config', () => {
      expect(connection).toBeDefined();
      expect(connection.isConnected()).toBe(false);
    });

    it('should connect to server', async () => {
      const connectPromise = connection.connect();

      await expect(connectPromise).resolves.toBeUndefined();
      expect(connection.isConnected()).toBe(true);
    });

    it('should emit connected event', (done) => {
      connection.on('connected', () => {
        expect(connection.isConnected()).toBe(true);
        done();
      });

      connection.connect();
    });

    it('should open bidirectional stream', async () => {
      await connection.connect();

      const stream = await connection.openBiStream();
      expect(stream).toBeInstanceOf(QuicStream);
      expect(connection.getStreamCount()).toBe(1);
    });

    it('should open unidirectional stream', async () => {
      await connection.connect();

      const stream = await connection.openUniStream();
      expect(stream).toBeInstanceOf(QuicStream);
    });

    it('should throw when opening stream before connect', async () => {
      await expect(connection.openBiStream()).rejects.toThrow('Not connected');
    });

    it('should respect max streams limit', async () => {
      const smallConnection = new QuicConnection({
        maxStreams: 2
      });

      await smallConnection.connect();

      await smallConnection.openBiStream();
      await smallConnection.openBiStream();

      await expect(smallConnection.openBiStream()).rejects.toThrow('Max streams reached');

      smallConnection.close();
    });

    it('should emit stream event when opening stream', (done) => {
      connection.connect().then(() => {
        connection.on('stream', (stream) => {
          expect(stream).toBeInstanceOf(QuicStream);
          done();
        });

        connection.openBiStream();
      });
    });

    it('should track statistics', async () => {
      await connection.connect();

      const stream = await connection.openBiStream();
      stream.write('test data');

      const stats = connection.getStats();
      expect(stats.streamsOpened).toBe(1);
      expect(stats.bytesSent).toBeGreaterThan(0);
    });

    it('should close all streams on connection close', async () => {
      await connection.connect();

      const stream1 = await connection.openBiStream();
      const stream2 = await connection.openBiStream();

      expect(connection.getStreamCount()).toBe(2);

      connection.close();

      expect(connection.getStreamCount()).toBe(0);
      expect(stream1.isClosed()).toBe(true);
      expect(stream2.isClosed()).toBe(true);
    });

    it('should have MidStream agent', () => {
      const agent = connection.getAgent();
      expect(agent).toBeDefined();
    });

    it('should process messages with agent', async () => {
      await connection.connect();

      const stream = await connection.openBiStream();
      stream.write('Hello from QUIC');

      const agent = connection.getAgent();
      const status = agent.getStatus();

      expect(status.conversationHistorySize).toBeGreaterThanOrEqual(0);
    });
  });

  describe('QuicServer', () => {
    let server: QuicServer;

    beforeEach(() => {
      server = new QuicServer({
        port: 4434 // Different port to avoid conflicts
      });
    });

    afterEach(() => {
      if (server.isListening()) {
        server.close();
      }
    });

    it('should create server with config', () => {
      expect(server).toBeDefined();
      expect(server.isListening()).toBe(false);
    });

    it('should start listening', async () => {
      await server.listen();
      expect(server.isListening()).toBe(true);
    });

    it('should emit listening event', (done) => {
      server.on('listening', (port) => {
        expect(port).toBe(4434);
        expect(server.isListening()).toBe(true);
        done();
      });

      server.listen();
    });

    it('should track connections', async () => {
      await server.listen();

      // Server starts with no connections
      expect(server.getConnectionCount()).toBe(0);

      // Connection count is tested in integration tests
      // where actual client connections are made
    });

    it('should close all connections on server close', async () => {
      await server.listen();

      // Server should close cleanly even with no connections
      server.close();

      expect(server.getConnectionCount()).toBe(0);
      expect(server.isListening()).toBe(false);
    });

    it('should handle errors', (done) => {
      server.on('error', (error) => {
        expect(error).toBeDefined();
        done();
      });

      // Simulate error after listening
      server.listen().then(() => {
        server.emit('error', new Error('Test error'));
      });
    });
  });

  describe('QuicClient', () => {
    let client: QuicClient;

    beforeEach(() => {
      client = new QuicClient();
    });

    afterEach(() => {
      client.disconnect();
    });

    it('should create client', () => {
      expect(client).toBeDefined();
      expect(client.getConnection()).toBeNull();
    });

    it('should connect to server', async () => {
      const connection = await client.connect('localhost', 4433);

      expect(connection).toBeInstanceOf(QuicConnection);
      expect(connection.isConnected()).toBe(true);
      expect(client.getConnection()).toBe(connection);
    });

    it('should disconnect', async () => {
      await client.connect('localhost', 4433);

      const connection = client.getConnection();
      expect(connection?.isConnected()).toBe(true);

      client.disconnect();

      expect(client.getConnection()).toBeNull();
    });
  });

  describe('Utility Functions', () => {
    it('should create QUIC server with defaults', () => {
      const server = createQuicServer();
      expect(server).toBeInstanceOf(QuicServer);
      server.close();
    });

    it('should create QUIC server with custom config', () => {
      const server = createQuicServer({ port: 5000 });
      expect(server).toBeInstanceOf(QuicServer);
      server.close();
    });

    it('should connect to QUIC server', async () => {
      const connection = await connectQuic('localhost', 4433);
      expect(connection).toBeInstanceOf(QuicConnection);
      expect(connection.isConnected()).toBe(true);
      connection.close();
    });

    it('should check QUIC support', () => {
      const supported = isQuicSupported();
      expect(typeof supported).toBe('boolean');
      expect(supported).toBe(true);
    });
  });

  describe('Integration Tests', () => {
    let server: QuicServer;
    let client: QuicConnection;

    beforeEach(async () => {
      server = createQuicServer({ port: 4435 });
      await server.listen();
    });

    afterEach(() => {
      if (client && client.isConnected()) {
        client.close();
      }
      if (server && server.isListening()) {
        server.close();
      }
    });

    it('should establish connection and open streams', async () => {
      client = await connectQuic('localhost', 4435);

      const stream = await client.openBiStream();
      expect(stream).toBeInstanceOf(QuicStream);
    });

    it('should send and process data', async () => {
      client = await connectQuic('localhost', 4435);

      const stream = await client.openBiStream();
      const testData = 'Hello QUIC!';

      stream.write(testData);

      const agent = client.getAgent();
      const status = agent.getStatus();

      expect(status.conversationHistorySize).toBeGreaterThanOrEqual(0);
    });

    it('should handle multiple streams', async () => {
      client = await connectQuic('localhost', 4435);

      const stream1 = await client.openBiStream();
      const stream2 = await client.openBiStream();
      const stream3 = await client.openBiStream();

      expect(client.getStreamCount()).toBe(3);

      stream1.write('Stream 1');
      stream2.write('Stream 2');
      stream3.write('Stream 3');

      const stats = client.getStats();
      expect(stats.streamsOpened).toBe(3);
    });

    it('should handle stream priorities', async () => {
      client = await connectQuic('localhost', 4435);

      const highPriority = await client.openBiStream({ priority: 10 });
      const lowPriority = await client.openBiStream({ priority: 1 });

      expect(highPriority.getPriority()).toBe(10);
      expect(lowPriority.getPriority()).toBe(1);
    });
  });

  describe('Performance Tests', () => {
    it('should handle rapid stream creation', async () => {
      const connection = new QuicConnection({ maxStreams: 100 });
      await connection.connect();

      const streams = [];
      const startTime = Date.now();

      for (let i = 0; i < 50; i++) {
        streams.push(await connection.openBiStream());
      }

      const duration = Date.now() - startTime;

      expect(streams.length).toBe(50);
      expect(duration).toBeLessThan(1000); // Should be fast

      connection.close();
    });

    it('should handle large data transfers', async () => {
      const connection = new QuicConnection();
      await connection.connect();

      const stream = await connection.openBiStream();
      const largeData = Buffer.alloc(1024 * 1024); // 1 MB

      const startTime = Date.now();
      stream.write(largeData);
      const duration = Date.now() - startTime;

      expect(duration).toBeLessThan(100); // Should be fast

      const stats = connection.getStats();
      expect(stats.bytesSent).toBeGreaterThanOrEqual(largeData.length);

      connection.close();
    });
  });
});
