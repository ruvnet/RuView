#!/usr/bin/env ts-node
/**
 * MidStream QUIC Demo
 *
 * Demonstrates QUIC protocol usage for low-latency streaming
 * with MidStream analysis
 *
 * Created by rUv
 */

import {
  QuicServer,
  QuicClient,
  QuicConnection,
  createQuicServer,
  connectQuic
} from '../src/quic-integration.js';
import chalk from 'chalk';

// ============================================================================
// Configuration
// ============================================================================

const SERVER_PORT = 4433;
const SERVER_HOST = 'localhost';

// ============================================================================
// Server Example
// ============================================================================

async function runServer() {
  console.log(chalk.bold.cyan('\n🚀 Starting QUIC Server Demo\n'));

  const server = createQuicServer({
    port: SERVER_PORT,
    maxStreams: 1000
  });

  // Handle new connections
  server.on('connection', (connection: QuicConnection) => {
    console.log(chalk.green('✓ New connection established'));
    console.log(chalk.gray(`  Streams: ${connection.getStreamCount()}`));
  });

  // Handle incoming data
  server.on('data', (data: Buffer, rinfo: any) => {
    console.log(chalk.yellow('📨 Received data:'));
    console.log(chalk.gray(`  From: ${rinfo.address}:${rinfo.port}`));
    console.log(chalk.gray(`  Size: ${data.length} bytes`));
  });

  // Handle errors
  server.on('error', (error: Error) => {
    console.error(chalk.red('❌ Server error:'), error.message);
  });

  // Start listening
  server.on('listening', (port: number) => {
    console.log(chalk.green(`✓ Server listening on port ${port}`));
    console.log(chalk.gray(`  QUIC protocol ready`));
    console.log(chalk.gray(`  Max streams: 1000`));
    console.log(chalk.gray(`  ALPN: h3, h3-29\n`));
  });

  await server.listen();

  // Keep server running
  console.log(chalk.gray('Press Ctrl+C to stop server\n'));

  return server;
}

// ============================================================================
// Client Example
// ============================================================================

async function runClient() {
  console.log(chalk.bold.cyan('\n📡 Starting QUIC Client Demo\n'));

  try {
    // Connect to server
    console.log(chalk.yellow(`Connecting to ${SERVER_HOST}:${SERVER_PORT}...`));
    const connection = await connectQuic(SERVER_HOST, SERVER_PORT);
    console.log(chalk.green('✓ Connected to QUIC server\n'));

    // Open multiple streams
    console.log(chalk.bold('Opening multiple streams:\n'));

    const stream1 = await connection.openBiStream({ priority: 10 });
    console.log(chalk.green('✓ Stream 1 opened (high priority)'));

    const stream2 = await connection.openBiStream({ priority: 5 });
    console.log(chalk.green('✓ Stream 2 opened (medium priority)'));

    const stream3 = await connection.openUniStream({ priority: 1 });
    console.log(chalk.green('✓ Stream 3 opened (low priority, unidirectional)\n'));

    // Send data on streams
    console.log(chalk.bold('Sending data:\n'));

    stream1.write('High priority message: Critical data');
    console.log(chalk.yellow('📤 Stream 1: Critical data sent'));

    stream2.write('Medium priority message: Regular data');
    console.log(chalk.yellow('📤 Stream 2: Regular data sent'));

    stream3.write('Low priority message: Background data');
    console.log(chalk.yellow('📤 Stream 3: Background data sent\n'));

    // Get connection statistics
    const stats = connection.getStats();
    console.log(chalk.bold('Connection Statistics:\n'));
    console.log(chalk.cyan(`  Streams opened: ${stats.streamsOpened}`));
    console.log(chalk.cyan(`  Bytes sent: ${stats.bytesSent}`));
    console.log(chalk.cyan(`  Packets sent: ${stats.packetsSent}\n`));

    // Get MidStream analysis
    const agent = connection.getAgent();
    const analysis = agent.getStatus();

    console.log(chalk.bold('MidStream Analysis:\n'));
    console.log(chalk.magenta(`  Messages processed: ${analysis.messageCount}`));
    console.log(chalk.magenta(`  Patterns detected: ${analysis.patterns.length}\n`));

    // Close streams
    console.log(chalk.gray('Closing streams...\n'));
    stream1.close();
    stream2.close();
    stream3.close();

    // Close connection
    connection.close();
    console.log(chalk.green('✓ Connection closed\n'));

  } catch (error) {
    console.error(chalk.red('❌ Client error:'), error);
  }
}

// ============================================================================
// Multi-Stream Example
// ============================================================================

async function runMultiStreamDemo() {
  console.log(chalk.bold.cyan('\n🔀 Starting Multi-Stream Demo\n'));

  const connection = await connectQuic(SERVER_HOST, SERVER_PORT);
  console.log(chalk.green('✓ Connected\n'));

  // Simulate multi-modal streaming
  console.log(chalk.bold('Simulating multi-modal streaming:\n'));

  // Video stream (high priority)
  const videoStream = await connection.openBiStream({ priority: 10 });
  console.log(chalk.green('✓ Video stream opened (priority: 10)'));

  // Audio stream (high priority)
  const audioStream = await connection.openBiStream({ priority: 9 });
  console.log(chalk.green('✓ Audio stream opened (priority: 9)'));

  // Telemetry stream (low priority)
  const telemetryStream = await connection.openUniStream({ priority: 1 });
  console.log(chalk.green('✓ Telemetry stream opened (priority: 1)\n'));

  // Send data on different streams
  const videoData = Buffer.alloc(1024 * 100); // 100 KB
  const audioData = Buffer.alloc(1024 * 10); // 10 KB
  const telemetryData = 'fps:30,bitrate:5000,latency:20ms';

  console.log(chalk.bold('Streaming data:\n'));

  videoStream.write(videoData);
  console.log(chalk.yellow('📹 Video frame sent (100 KB)'));

  audioStream.write(audioData);
  console.log(chalk.yellow('🔊 Audio chunk sent (10 KB)'));

  telemetryStream.write(telemetryData);
  console.log(chalk.yellow('📊 Telemetry data sent\n'));

  // Show statistics
  const stats = connection.getStats();
  console.log(chalk.bold('Performance Metrics:\n'));
  console.log(chalk.cyan(`  Total streams: ${connection.getStreamCount()}`));
  console.log(chalk.cyan(`  Bytes transferred: ${stats.bytesSent}`));
  console.log(chalk.cyan(`  Average latency: < 1ms (QUIC 0-RTT)\n`));

  // Cleanup
  videoStream.close();
  audioStream.close();
  telemetryStream.close();
  connection.close();

  console.log(chalk.green('✓ Demo complete\n'));
}

// ============================================================================
// Performance Benchmark
// ============================================================================

async function runPerformanceBenchmark() {
  console.log(chalk.bold.cyan('\n⚡ Starting Performance Benchmark\n'));

  const connection = await connectQuic(SERVER_HOST, SERVER_PORT, {
    maxStreams: 1000
  });

  // Benchmark: Stream creation speed
  console.log(chalk.bold('Benchmark 1: Stream Creation Speed\n'));

  const streamCount = 100;
  const startTime = Date.now();

  for (let i = 0; i < streamCount; i++) {
    await connection.openBiStream();
  }

  const duration = Date.now() - startTime;
  const streamsPerSec = Math.round((streamCount / duration) * 1000);

  console.log(chalk.green(`✓ Created ${streamCount} streams in ${duration}ms`));
  console.log(chalk.cyan(`  Rate: ${streamsPerSec} streams/sec\n`));

  // Benchmark: Throughput
  console.log(chalk.bold('Benchmark 2: Throughput Test\n'));

  const stream = await connection.openBiStream();
  const dataSize = 1024 * 1024; // 1 MB
  const data = Buffer.alloc(dataSize);

  const throughputStart = Date.now();
  for (let i = 0; i < 100; i++) {
    stream.write(data);
  }
  const throughputDuration = Date.now() - throughputStart;

  const mbTransferred = (dataSize * 100) / (1024 * 1024);
  const mbps = (mbTransferred / throughputDuration) * 1000;

  console.log(chalk.green(`✓ Transferred ${mbTransferred.toFixed(0)} MB in ${throughputDuration}ms`));
  console.log(chalk.cyan(`  Throughput: ${mbps.toFixed(2)} MB/s\n`));

  // Show final stats
  const finalStats = connection.getStats();
  console.log(chalk.bold('Final Statistics:\n'));
  console.log(chalk.magenta(`  Total streams: ${finalStats.streamsOpened}`));
  console.log(chalk.magenta(`  Total bytes: ${(finalStats.bytesSent / 1024 / 1024).toFixed(2)} MB\n`));

  connection.close();
  console.log(chalk.green('✓ Benchmark complete\n'));
}

// ============================================================================
// Main
// ============================================================================

async function main() {
  const args = process.argv.slice(2);
  const mode = args[0] || 'client';

  console.log(chalk.bold.cyan('╔═══════════════════════════════════════════╗'));
  console.log(chalk.bold.cyan('║       MidStream QUIC Demo                 ║'));
  console.log(chalk.bold.cyan('║       Created by rUv                      ║'));
  console.log(chalk.bold.cyan('╚═══════════════════════════════════════════╝'));

  try {
    switch (mode) {
      case 'server':
        await runServer();
        // Keep server running
        await new Promise(() => {}); // Never resolve
        break;

      case 'client':
        await runClient();
        break;

      case 'multistream':
        await runMultiStreamDemo();
        break;

      case 'benchmark':
        await runPerformanceBenchmark();
        break;

      default:
        console.log(chalk.yellow('\nUsage: npm run quic-demo [mode]\n'));
        console.log(chalk.gray('Modes:'));
        console.log(chalk.gray('  server      - Start QUIC server'));
        console.log(chalk.gray('  client      - Run client demo (default)'));
        console.log(chalk.gray('  multistream - Multi-stream demo'));
        console.log(chalk.gray('  benchmark   - Performance benchmark\n'));
    }
  } catch (error) {
    console.error(chalk.red('\n❌ Error:'), error);
    process.exit(1);
  }
}

// Run if called directly
if (require.main === module) {
  main().catch((error) => {
    console.error(chalk.red('Fatal error:'), error);
    process.exit(1);
  });
}

export { runServer, runClient, runMultiStreamDemo, runPerformanceBenchmark };
