/**
 * MidStream Real-Time Dashboard
 *
 * Minimal console-based dashboard with WASM and streaming support
 * for real-time display of MidStream introspection capabilities.
 *
 * Created by rUv
 */

import { MidStreamAgent, AnalysisResult } from './agent.js';
import { OpenAIRealtimeClient } from './openai-realtime.js';
import chalk from 'chalk';
import * as readline from 'readline';

// ============================================================================
// Dashboard State Management
// ============================================================================

interface DashboardState {
  messageCount: number;
  totalTokens: number;
  patternsDetected: string[];
  attractorType: string;
  lyapunovExponent: number;
  isStable: boolean;
  isChaotic: boolean;
  avgReward: number;
  recentMessages: string[];
  audioStreaming: boolean;
  videoStreaming: boolean;
  lastUpdate: Date;
  fps: number;
  latency: number;
}

interface StreamMetrics {
  type: 'audio' | 'video' | 'text';
  bytesProcessed: number;
  chunksReceived: number;
  avgChunkSize: number;
  startTime: number;
  lastChunkTime: number;
}

// ============================================================================
// Dashboard Class
// ============================================================================

export class MidStreamDashboard {
  private agent: MidStreamAgent;
  private state: DashboardState;
  private streamMetrics: Map<string, StreamMetrics>;
  private updateInterval: NodeJS.Timeout | null = null;
  private startTime: number;
  private frameCount: number = 0;

  constructor() {
    this.agent = new MidStreamAgent({
      maxHistory: 1000,
      embeddingDim: 3,
    });

    this.state = {
      messageCount: 0,
      totalTokens: 0,
      patternsDetected: [],
      attractorType: 'unknown',
      lyapunovExponent: 0,
      isStable: true,
      isChaotic: false,
      avgReward: 0,
      recentMessages: [],
      audioStreaming: false,
      videoStreaming: false,
      lastUpdate: new Date(),
      fps: 0,
      latency: 0,
    };

    this.streamMetrics = new Map();
    this.startTime = Date.now();
  }

  // ==========================================================================
  // Core Dashboard Methods
  // ==========================================================================

  /**
   * Start the dashboard with real-time updates
   */
  start(refreshRate: number = 100): void {
    this.clearScreen();
    this.render();

    this.updateInterval = setInterval(() => {
      this.frameCount++;
      this.state.fps = Math.round(this.frameCount / ((Date.now() - this.startTime) / 1000));
      this.clearScreen();
      this.render();
    }, refreshRate);
  }

  /**
   * Stop the dashboard
   */
  stop(): void {
    if (this.updateInterval) {
      clearInterval(this.updateInterval);
      this.updateInterval = null;
    }
  }

  /**
   * Process a message and update dashboard state
   */
  processMessage(message: string, tokens: number = 0): void {
    const startTime = Date.now();

    // Process with agent
    this.agent.processMessage(message);

    // Update state
    this.state.messageCount++;
    this.state.totalTokens += tokens;
    this.state.recentMessages.unshift(message.substring(0, 50) + '...');
    if (this.state.recentMessages.length > 5) {
      this.state.recentMessages.pop();
    }

    // Get analysis
    const analysis = this.agent.getStatus();
    this.updateFromAnalysis(analysis);

    // Calculate latency
    this.state.latency = Date.now() - startTime;
    this.state.lastUpdate = new Date();
  }

  /**
   * Process streaming data (audio/video/text)
   */
  processStream(streamId: string, data: Buffer, type: 'audio' | 'video' | 'text'): void {
    let metrics = this.streamMetrics.get(streamId);

    if (!metrics) {
      metrics = {
        type,
        bytesProcessed: 0,
        chunksReceived: 0,
        avgChunkSize: 0,
        startTime: Date.now(),
        lastChunkTime: Date.now(),
      };
      this.streamMetrics.set(streamId, metrics);
    }

    // Update metrics
    metrics.bytesProcessed += data.length;
    metrics.chunksReceived++;
    metrics.avgChunkSize = Math.round(metrics.bytesProcessed / metrics.chunksReceived);
    metrics.lastChunkTime = Date.now();

    // Update state
    if (type === 'audio') {
      this.state.audioStreaming = true;
    } else if (type === 'video') {
      this.state.videoStreaming = true;
    }
  }

  /**
   * Update dashboard from analysis results
   */
  private updateFromAnalysis(analysis: AnalysisResult): void {
    this.state.patternsDetected = analysis.patterns
      .slice(0, 5)
      .map((p: any) => `${p.type || 'pattern'} (${p.confidence || 0}%)`);

    if (analysis.temporalAnalysis) {
      this.state.attractorType = analysis.temporalAnalysis.attractorType || 'unknown';
      this.state.lyapunovExponent = analysis.temporalAnalysis.lyapunovExponent || 0;
      this.state.isStable = analysis.temporalAnalysis.isStable || false;
      this.state.isChaotic = analysis.temporalAnalysis.isChaotic || false;
    }

    if (analysis.metaLearning) {
      this.state.avgReward = analysis.metaLearning.avgReward || 0;
    }
  }

  // ==========================================================================
  // Rendering Methods
  // ==========================================================================

  /**
   * Clear the console screen
   */
  private clearScreen(): void {
    process.stdout.write('\x1b[2J\x1b[0f');
  }

  /**
   * Render the dashboard
   */
  private render(): void {
    const width = process.stdout.columns || 80;
    const separator = '─'.repeat(width);

    console.log(chalk.bold.cyan('╔' + '═'.repeat(width - 2) + '╗'));
    console.log(
      chalk.bold.cyan('║') +
        this.centerText('MidStream Real-Time Dashboard', width - 2) +
        chalk.bold.cyan('║')
    );
    console.log(
      chalk.bold.cyan('║') +
        this.centerText('Created by rUv', width - 2) +
        chalk.bold.cyan('║')
    );
    console.log(chalk.bold.cyan('╚' + '═'.repeat(width - 2) + '╝'));

    // System Metrics
    this.renderSection('System Metrics', [
      `Messages Processed: ${chalk.green(this.state.messageCount)}`,
      `Total Tokens: ${chalk.green(this.state.totalTokens)}`,
      `FPS: ${chalk.yellow(this.state.fps)}`,
      `Latency: ${chalk.yellow(this.state.latency + 'ms')}`,
      `Uptime: ${chalk.cyan(this.formatUptime())}`,
    ]);

    // Temporal Analysis
    this.renderSection('Temporal Analysis', [
      `Attractor Type: ${this.colorizeAttractor(this.state.attractorType)}`,
      `Lyapunov Exp: ${this.colorizeLyapunov(this.state.lyapunovExponent)}`,
      `Stability: ${this.state.isStable ? chalk.green('STABLE') : chalk.red('UNSTABLE')}`,
      `Chaos: ${this.state.isChaotic ? chalk.red('CHAOTIC') : chalk.green('ORDERED')}`,
      `Avg Reward: ${chalk.cyan(this.state.avgReward.toFixed(3))}`,
    ]);

    // Pattern Detection
    this.renderSection(
      'Pattern Detection',
      this.state.patternsDetected.length > 0
        ? this.state.patternsDetected.map((p) => chalk.magenta('• ' + p))
        : [chalk.gray('No patterns detected yet')]
    );

    // Streaming Status
    this.renderStreamingStatus();

    // Recent Messages
    this.renderSection(
      'Recent Messages',
      this.state.recentMessages.length > 0
        ? this.state.recentMessages.map((m) => chalk.gray('• ' + m))
        : [chalk.gray('No messages yet')]
    );

    // Stream Metrics
    this.renderStreamMetrics();

    // Footer
    console.log(chalk.gray(separator));
    console.log(
      chalk.gray(
        `Last Update: ${this.state.lastUpdate.toLocaleTimeString()} | Press Ctrl+C to exit`
      )
    );
  }

  /**
   * Render a section with title and content
   */
  private renderSection(title: string, lines: string[]): void {
    const width = process.stdout.columns || 80;
    console.log('\n' + chalk.bold.white(title));
    console.log(chalk.gray('─'.repeat(width)));
    lines.forEach((line) => console.log(line));
  }

  /**
   * Render streaming status
   */
  private renderStreamingStatus(): void {
    const audioStatus = this.state.audioStreaming
      ? chalk.green('● ACTIVE')
      : chalk.gray('○ INACTIVE');
    const videoStatus = this.state.videoStreaming
      ? chalk.green('● ACTIVE')
      : chalk.gray('○ INACTIVE');

    this.renderSection('Streaming Status', [
      `Audio: ${audioStatus}`,
      `Video: ${videoStatus}`,
      `Streams: ${chalk.cyan(this.streamMetrics.size)} active`,
    ]);
  }

  /**
   * Render stream metrics
   */
  private renderStreamMetrics(): void {
    if (this.streamMetrics.size === 0) {
      return;
    }

    const metrics: string[] = [];
    this.streamMetrics.forEach((metric, streamId) => {
      const duration = (Date.now() - metric.startTime) / 1000;
      const rate = (metric.bytesProcessed / duration / 1024).toFixed(2);

      metrics.push(
        chalk.cyan(`${streamId} (${metric.type}):`) +
          ` ${chalk.yellow(metric.chunksReceived)} chunks, ` +
          `${chalk.yellow(this.formatBytes(metric.bytesProcessed))}, ` +
          `${chalk.yellow(rate)} KB/s`
      );
    });

    this.renderSection('Stream Metrics', metrics);
  }

  // ==========================================================================
  // Helper Methods
  // ==========================================================================

  /**
   * Center text within a given width
   */
  private centerText(text: string, width: number): string {
    const padding = Math.max(0, width - text.length);
    const leftPad = Math.floor(padding / 2);
    const rightPad = padding - leftPad;
    return ' '.repeat(leftPad) + text + ' '.repeat(rightPad);
  }

  /**
   * Format uptime
   */
  private formatUptime(): string {
    const seconds = Math.floor((Date.now() - this.startTime) / 1000);
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    return `${hours}h ${minutes}m ${secs}s`;
  }

  /**
   * Format bytes
   */
  private formatBytes(bytes: number): string {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(2) + ' KB';
    return (bytes / (1024 * 1024)).toFixed(2) + ' MB';
  }

  /**
   * Colorize attractor type
   */
  private colorizeAttractor(type: string): string {
    const colors: Record<string, any> = {
      fixed: chalk.green,
      periodic: chalk.blue,
      chaotic: chalk.red,
      unknown: chalk.gray,
    };
    const color = colors[type] || chalk.white;
    return color(type.toUpperCase());
  }

  /**
   * Colorize Lyapunov exponent
   */
  private colorizeLyapunov(value: number): string {
    const str = value.toFixed(4);
    if (value < 0) return chalk.green(str);
    if (value > 0) return chalk.red(str);
    return chalk.yellow(str);
  }

  /**
   * Get current agent
   */
  getAgent(): MidStreamAgent {
    return this.agent;
  }

  /**
   * Get current state
   */
  getState(): DashboardState {
    return { ...this.state };
  }
}

// ============================================================================
// Interactive Dashboard
// ============================================================================

export class InteractiveDashboard extends MidStreamDashboard {
  private rl: readline.Interface | null = null;

  /**
   * Start interactive mode with user input
   */
  startInteractive(): void {
    this.start(100);

    this.rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout,
    });

    // Setup readline to capture input without blocking display
    process.stdin.on('keypress', (str, key) => {
      if (key.ctrl && key.name === 'c') {
        this.stop();
        if (this.rl) {
          this.rl.close();
        }
        process.exit(0);
      }
    });
  }

  /**
   * Stop interactive mode
   */
  stopInteractive(): void {
    this.stop();
    if (this.rl) {
      this.rl.close();
      this.rl = null;
    }
  }
}

// ============================================================================
// Exports
// ============================================================================

export { DashboardState, StreamMetrics };
