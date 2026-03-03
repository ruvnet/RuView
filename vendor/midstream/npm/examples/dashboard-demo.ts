#!/usr/bin/env node
/**
 * MidStream Dashboard Demo
 *
 * Comprehensive demonstration of MidStream capabilities:
 * - Real-time dashboard with WASM support
 * - Text/Audio/Video streaming introspection
 * - Temporal pattern analysis
 * - Attractor detection
 * - Meta-learning visualization
 *
 * Created by rUv
 */

import { MidStreamDashboard } from '../src/dashboard.js';
import { RestreamClient, StreamSimulator } from '../src/restream-integration.js';
import { OpenAIRealtimeClient } from '../src/openai-realtime.js';
import chalk from 'chalk';
import * as dotenv from 'dotenv';

// Load environment variables
dotenv.config();

// ============================================================================
// Demo Configuration
// ============================================================================

interface DemoConfig {
  mode: 'text' | 'audio' | 'video' | 'all';
  simulateStream: boolean;
  useOpenAI: boolean;
  duration: number; // seconds
}

// ============================================================================
// Demo Scenarios
// ============================================================================

const DEMO_MESSAGES = [
  'Hello, I need help with my account',
  'I am having trouble logging in',
  'Can you reset my password?',
  'Thank you for your help',
  'I have another question about billing',
  'What are your pricing plans?',
  'Can I upgrade my subscription?',
  'How do I cancel my account?',
  'Is there a refund policy?',
  'Thanks for the information',
];

// ============================================================================
// Main Demo Class
// ============================================================================

class MidStreamDemo {
  private dashboard: MidStreamDashboard;
  private restreamClient: RestreamClient | null = null;
  private streamSimulator: StreamSimulator | null = null;
  private realtimeClient: OpenAIRealtimeClient | null = null;
  private config: DemoConfig;
  private messageIndex: number = 0;

  constructor(config: DemoConfig) {
    this.config = config;
    this.dashboard = new MidStreamDashboard();
  }

  // ==========================================================================
  // Demo Modes
  // ==========================================================================

  /**
   * Run text-only demo
   */
  async runTextDemo(): Promise<void> {
    console.log(chalk.bold.cyan('\n🚀 Starting Text Streaming Demo\n'));

    this.dashboard.start(100);

    // Simulate incoming messages
    const messageInterval = setInterval(() => {
      if (this.messageIndex >= DEMO_MESSAGES.length) {
        this.messageIndex = 0;
      }

      const message = DEMO_MESSAGES[this.messageIndex++];
      const tokens = Math.floor(message.split(' ').length * 1.3);

      this.dashboard.processMessage(message, tokens);
    }, 2000);

    // Run for configured duration
    await this.sleep(this.config.duration * 1000);

    clearInterval(messageInterval);
    this.dashboard.stop();
  }

  /**
   * Run audio streaming demo
   */
  async runAudioDemo(): Promise<void> {
    console.log(chalk.bold.cyan('\n🎵 Starting Audio Streaming Demo\n'));

    this.dashboard.start(100);

    if (this.config.simulateStream) {
      this.streamSimulator = new StreamSimulator(30);

      this.streamSimulator.start(
        () => {}, // Skip video frames
        (audio) => {
          this.dashboard.processStream('audio-stream-1', audio.data, 'audio');

          // Simulate transcription every few chunks
          if (Math.random() < 0.3) {
            const message = DEMO_MESSAGES[this.messageIndex++ % DEMO_MESSAGES.length];
            this.dashboard.processMessage(message, 50);
          }
        }
      );
    }

    // Run for configured duration
    await this.sleep(this.config.duration * 1000);

    if (this.streamSimulator) {
      this.streamSimulator.stop();
    }
    this.dashboard.stop();
  }

  /**
   * Run video streaming demo
   */
  async runVideoDemo(): Promise<void> {
    console.log(chalk.bold.cyan('\n📹 Starting Video Streaming Demo\n'));

    this.dashboard.start(100);

    if (this.config.simulateStream) {
      this.streamSimulator = new StreamSimulator(30);

      this.streamSimulator.start(
        (frame) => {
          this.dashboard.processStream('video-stream-1', frame.data, 'video');

          // Simulate object detection every 30 frames
          if (frame.frameNumber % 30 === 0) {
            const message = `Detected objects in frame ${frame.frameNumber}`;
            this.dashboard.processMessage(message, 20);
          }
        },
        (audio) => {
          this.dashboard.processStream('audio-stream-1', audio.data, 'audio');
        }
      );
    }

    // Run for configured duration
    await this.sleep(this.config.duration * 1000);

    if (this.streamSimulator) {
      this.streamSimulator.stop();
    }
    this.dashboard.stop();
  }

  /**
   * Run comprehensive demo with all features
   */
  async runFullDemo(): Promise<void> {
    console.log(chalk.bold.cyan('\n🌟 Starting Comprehensive MidStream Demo\n'));
    console.log(chalk.gray('Demonstrating all capabilities:\n'));
    console.log(chalk.yellow('  • Real-time text processing'));
    console.log(chalk.yellow('  • Audio stream introspection'));
    console.log(chalk.yellow('  • Video stream analysis'));
    console.log(chalk.yellow('  • Temporal pattern detection'));
    console.log(chalk.yellow('  • Attractor analysis'));
    console.log(chalk.yellow('  • Meta-learning'));
    console.log(chalk.gray('\nPress Ctrl+C to exit\n'));

    await this.sleep(2000);

    this.dashboard.start(100);

    // Start stream simulator
    if (this.config.simulateStream) {
      this.streamSimulator = new StreamSimulator(30);

      this.streamSimulator.start(
        (frame) => {
          this.dashboard.processStream('video-stream-1', frame.data, 'video');

          // Simulate detections and analysis
          if (frame.frameNumber % 30 === 0) {
            const message = `Frame ${frame.frameNumber}: detected 2 objects`;
            this.dashboard.processMessage(message, 15);
          }
        },
        (audio) => {
          this.dashboard.processStream('audio-stream-1', audio.data, 'audio');
        }
      );
    }

    // Simulate text messages
    const messageInterval = setInterval(() => {
      if (this.messageIndex >= DEMO_MESSAGES.length) {
        this.messageIndex = 0;
      }

      const message = DEMO_MESSAGES[this.messageIndex++];
      const tokens = Math.floor(message.split(' ').length * 1.3);

      this.dashboard.processMessage(message, tokens);
    }, 3000);

    // Initialize Restream client if configured
    if (this.config.simulateStream) {
      this.restreamClient = new RestreamClient({
        frameRate: 30,
        resolution: '1920x1080',
        enableTranscription: true,
        enableObjectDetection: true,
      });

      this.restreamClient.on('frame', (frame) => {
        // Process frame
      });

      this.restreamClient.on('audio', (audio) => {
        // Process audio
      });

      this.restreamClient.on('transcription', (text) => {
        this.dashboard.processMessage(`Transcription: ${text}`, 20);
      });
    }

    // Run for configured duration
    await this.sleep(this.config.duration * 1000);

    clearInterval(messageInterval);
    if (this.streamSimulator) {
      this.streamSimulator.stop();
    }
    if (this.restreamClient) {
      this.restreamClient.disconnect();
    }
    this.dashboard.stop();
  }

  /**
   * Run OpenAI Realtime demo
   */
  async runOpenAIDemo(): Promise<void> {
    if (!process.env.OPENAI_API_KEY) {
      console.log(chalk.red('❌ OPENAI_API_KEY not found in environment'));
      return;
    }

    console.log(chalk.bold.cyan('\n🤖 Starting OpenAI Realtime Demo\n'));

    this.dashboard.start(100);

    this.realtimeClient = new OpenAIRealtimeClient({
      apiKey: process.env.OPENAI_API_KEY,
      model: process.env.OPENAI_REALTIME_MODEL || 'gpt-4o-realtime-preview-2024-10-01',
      voice: 'alloy',
    });

    this.realtimeClient.on('response.text.delta', (delta) => {
      this.dashboard.processMessage(delta, delta.length);
    });

    this.realtimeClient.on('response.audio.delta', (delta) => {
      this.dashboard.processStream('openai-audio', Buffer.from(delta, 'base64'), 'audio');
    });

    try {
      await this.realtimeClient.connect();
      console.log(chalk.green('✓ Connected to OpenAI Realtime API'));

      // Send test messages
      const messages = [
        'Hello, can you help me understand patterns in conversations?',
        'What are the key characteristics of chaotic systems?',
        'Explain temporal attractors in simple terms.',
      ];

      for (const message of messages) {
        await this.sleep(5000);
        this.realtimeClient.sendText(message);
        this.dashboard.processMessage(`User: ${message}`, message.split(' ').length);
      }

      // Run for configured duration
      await this.sleep(this.config.duration * 1000);

      this.realtimeClient.disconnect();
    } catch (error) {
      console.error(chalk.red('Error:', error));
    }

    this.dashboard.stop();
  }

  // ==========================================================================
  // Helpers
  // ==========================================================================

  /**
   * Sleep for specified milliseconds
   */
  private sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
}

// ============================================================================
// Main Entry Point
// ============================================================================

async function main() {
  const args = process.argv.slice(2);

  let config: DemoConfig = {
    mode: 'all',
    simulateStream: true,
    useOpenAI: false,
    duration: 60, // 1 minute default
  };

  // Parse command line arguments
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];

    if (arg === '--mode' && args[i + 1]) {
      config.mode = args[++i] as any;
    } else if (arg === '--duration' && args[i + 1]) {
      config.duration = parseInt(args[++i]);
    } else if (arg === '--no-simulate') {
      config.simulateStream = false;
    } else if (arg === '--openai') {
      config.useOpenAI = true;
    } else if (arg === '--help' || arg === '-h') {
      printHelp();
      return;
    }
  }

  // Print banner
  printBanner();

  // Create and run demo
  const demo = new MidStreamDemo(config);

  try {
    switch (config.mode) {
      case 'text':
        await demo.runTextDemo();
        break;
      case 'audio':
        await demo.runAudioDemo();
        break;
      case 'video':
        await demo.runVideoDemo();
        break;
      case 'all':
        if (config.useOpenAI) {
          await demo.runOpenAIDemo();
        } else {
          await demo.runFullDemo();
        }
        break;
      default:
        console.log(chalk.red(`Unknown mode: ${config.mode}`));
        printHelp();
    }

    console.log(chalk.green('\n✓ Demo completed successfully\n'));
  } catch (error) {
    console.error(chalk.red('\n❌ Demo failed:'), error);
    process.exit(1);
  }
}

// ============================================================================
// Helper Functions
// ============================================================================

function printBanner() {
  console.log(chalk.bold.cyan('\n╔═══════════════════════════════════════════════════════════╗'));
  console.log(chalk.bold.cyan('║                                                           ║'));
  console.log(chalk.bold.cyan('║              MidStream Dashboard Demo                     ║'));
  console.log(chalk.bold.cyan('║                                                           ║'));
  console.log(chalk.bold.cyan('║         Real-time LLM Streaming Analysis                  ║'));
  console.log(chalk.bold.cyan('║         with Lean Agentic Learning                        ║'));
  console.log(chalk.bold.cyan('║                                                           ║'));
  console.log(chalk.bold.cyan('║                  Created by rUv                           ║'));
  console.log(chalk.bold.cyan('║                                                           ║'));
  console.log(chalk.bold.cyan('╚═══════════════════════════════════════════════════════════╝\n'));
}

function printHelp() {
  console.log(chalk.bold('\nUsage:') + ' npm run demo [options]\n');
  console.log(chalk.bold('Options:'));
  console.log('  --mode <mode>       Demo mode: text, audio, video, all (default: all)');
  console.log('  --duration <secs>   Duration in seconds (default: 60)');
  console.log('  --no-simulate       Disable stream simulation');
  console.log('  --openai            Use OpenAI Realtime API');
  console.log('  --help, -h          Show this help message\n');
  console.log(chalk.bold('Examples:'));
  console.log('  npm run demo --mode text --duration 30');
  console.log('  npm run demo --mode all --openai');
  console.log('  npm run demo --mode video --duration 120\n');
}

// Run the demo
if (require.main === module) {
  main().catch((error) => {
    console.error(chalk.red('Fatal error:'), error);
    process.exit(1);
  });
}

export { MidStreamDemo };
