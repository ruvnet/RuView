# MidStream Real-Time Dashboard

**Created by rUv**

Comprehensive real-time dashboard for monitoring and analyzing LLM streaming with advanced temporal pattern detection, attractor analysis, and multi-modal stream introspection.

## 🌟 Features

### Real-Time Monitoring
- **Text Streaming**: Process and analyze text messages in real-time
- **Audio Streaming**: Monitor audio streams with transcription support
- **Video Streaming**: Analyze video streams with object detection
- **Multi-Modal**: Simultaneous handling of text, audio, and video streams

### Advanced Analysis
- **Temporal Pattern Detection**: Identify patterns in conversation flows
- **Attractor Analysis**: Detect fixed points, periodic cycles, and chaotic behavior
- **Lyapunov Exponents**: Measure system stability and chaos
- **Meta-Learning**: Adaptive learning from conversation patterns
- **Behavior Classification**: Classify system behavior as stable, unstable, or chaotic

### Performance Metrics
- **Real-Time FPS**: Frames per second monitoring
- **Latency Tracking**: Message processing latency
- **Stream Metrics**: Bandwidth, bitrate, and chunk statistics
- **Token Counting**: Track LLM token usage
- **Uptime Monitoring**: System uptime and health

### Streaming Support
- **WebSocket**: Real-time bidirectional streaming
- **Server-Sent Events (SSE)**: Unidirectional event streaming
- **WebRTC**: Peer-to-peer audio/video streaming
- **RTMP**: Real-Time Messaging Protocol support
- **HLS**: HTTP Live Streaming support

## 📦 Installation

```bash
cd npm
npm install
npm run build:ts
```

## 🚀 Quick Start

### Basic Dashboard

```typescript
import { MidStreamDashboard } from 'midstream-cli';

const dashboard = new MidStreamDashboard();
dashboard.start(100); // Refresh every 100ms

// Process a message
dashboard.processMessage('Hello, world!', 5);

// Process streaming data
const audioData = Buffer.alloc(1024);
dashboard.processStream('audio-1', audioData, 'audio');
```

### Interactive Dashboard

```typescript
import { InteractiveDashboard } from 'midstream-cli';

const dashboard = new InteractiveDashboard();
dashboard.startInteractive();
```

### Run Demo

```bash
# Full demo with all features
npm run demo

# Text-only demo
npm run demo:text

# Audio streaming demo
npm run demo:audio

# Video streaming demo
npm run demo:video

# OpenAI Realtime API demo
npm run demo:openai
```

## 📊 Dashboard Components

### System Metrics Panel
```
Messages Processed: 150
Total Tokens: 2,340
FPS: 60
Latency: 12ms
Uptime: 0h 5m 23s
```

### Temporal Analysis Panel
```
Attractor Type: PERIODIC
Lyapunov Exp: -0.0234
Stability: STABLE
Chaos: ORDERED
Avg Reward: 0.847
```

### Pattern Detection Panel
```
• greeting (95%)
• question (87%)
• acknowledgment (92%)
• follow-up (78%)
• closing (88%)
```

### Streaming Status Panel
```
Audio: ● ACTIVE
Video: ● ACTIVE
Streams: 3 active
```

### Stream Metrics Panel
```
audio-stream-1 (audio): 150 chunks, 1.5 MB, 45.2 KB/s
video-stream-1 (video): 1800 frames, 180 MB, 3.2 MB/s
```

## 🎥 Restream Integration

### WebRTC Streaming

```typescript
import { RestreamClient } from 'midstream-cli';

const client = new RestreamClient({
  webrtcSignaling: 'wss://signaling.example.com',
  enableTranscription: true,
  enableObjectDetection: true,
  frameRate: 30,
  resolution: '1920x1080'
});

// Listen for frames
client.on('frame', (frame) => {
  console.log(`Frame ${frame.frameNumber}: ${frame.width}x${frame.height}`);
});

// Listen for audio
client.on('audio', (audio) => {
  console.log(`Audio chunk: ${audio.sampleRate}Hz, ${audio.channels}ch`);
});

// Listen for transcriptions
client.on('transcription', (text) => {
  console.log(`Transcription: ${text}`);
});

// Connect
await client.connectWebRTC();
```

### RTMP Streaming

```typescript
const client = new RestreamClient({
  rtmpUrl: 'rtmp://live.example.com/live',
  streamKey: 'your-stream-key',
  enableTranscription: true
});

await client.connectRTMP();
```

### HLS Streaming

```typescript
const client = new RestreamClient({
  enableTranscription: true
});

await client.connectHLS('https://example.com/stream.m3u8');
```

### Stream Analysis

```typescript
// Get real-time analysis
const analysis = client.getAnalysis();

console.log(`
  Frames: ${analysis.frameCount}
  Audio Chunks: ${analysis.audioChunks}
  FPS: ${analysis.fps}
  Bitrate: ${analysis.bitrate} Kbps
  Patterns: ${analysis.patterns.length}
`);
```

## 🤖 OpenAI Realtime Integration

```typescript
import { MidStreamDashboard } from 'midstream-cli';
import { OpenAIRealtimeClient } from 'midstream-cli';

const dashboard = new MidStreamDashboard();
dashboard.start();

const client = new OpenAIRealtimeClient({
  apiKey: process.env.OPENAI_API_KEY,
  model: 'gpt-4o-realtime-preview-2024-10-01',
  voice: 'alloy'
});

// Connect dashboard to OpenAI events
client.on('response.text.delta', (delta) => {
  dashboard.processMessage(delta, delta.length);
});

client.on('response.audio.delta', (delta) => {
  const audio = Buffer.from(delta, 'base64');
  dashboard.processStream('openai-audio', audio, 'audio');
});

await client.connect();
client.sendText('Analyze this conversation...');
```

## 🧪 Testing with Stream Simulator

```typescript
import { StreamSimulator } from 'midstream-cli';

const simulator = new StreamSimulator(30); // 30 FPS

simulator.start(
  (frame) => {
    // Process video frame
    dashboard.processStream('video', frame.data, 'video');
  },
  (audio) => {
    // Process audio chunk
    dashboard.processStream('audio', audio.data, 'audio');
  }
);

// Run for 60 seconds
setTimeout(() => simulator.stop(), 60000);
```

## 🔧 Configuration

### Dashboard Options

```typescript
const dashboard = new MidStreamDashboard();

// Custom agent configuration
const agent = dashboard.getAgent();
// Agent is pre-configured with:
// - maxHistory: 1000
// - embeddingDim: 3
// - schedulingPolicy: 'EDF'
```

### Refresh Rate

```typescript
// Fast refresh (100ms) - smooth but CPU intensive
dashboard.start(100);

// Medium refresh (500ms) - balanced
dashboard.start(500);

// Slow refresh (1000ms) - low CPU usage
dashboard.start(1000);
```

## 📈 Advanced Usage

### Custom Pattern Analysis

```typescript
const agent = dashboard.getAgent();

// Detect custom pattern
const pattern = ['greeting', 'question', 'answer'];
const positions = agent.detectPattern(conversation, pattern);

console.log(`Pattern found at positions: ${positions}`);
```

### Sequence Comparison

```typescript
// Compare two conversation sequences
const similarity = agent.compareSequences(
  sequence1,
  sequence2,
  'dtw' // Dynamic Time Warping
);

console.log(`Similarity: ${(similarity * 100).toFixed(1)}%`);
```

### Behavior Analysis

```typescript
// Analyze system behavior
const rewards = [0.8, 0.85, 0.83, 0.87, 0.84];
const analysis = agent.analyzeBehavior(rewards);

console.log(`
  Attractor: ${analysis.attractorType}
  Lyapunov: ${analysis.lyapunovExponent}
  Stable: ${analysis.isStable}
  Chaotic: ${analysis.isChaotic}
`);
```

## 🎨 Customization

### Color Themes

The dashboard uses chalk for colorful console output:
- **Cyan**: Headers and titles
- **Green**: Success states and positive metrics
- **Yellow**: Warnings and neutral metrics
- **Red**: Errors and negative states
- **Magenta**: Patterns and detections
- **Gray**: Secondary information

### Custom Metrics

```typescript
// Get current state
const state = dashboard.getState();

// Modify or extend as needed
console.log(`
  Messages: ${state.messageCount}
  Patterns: ${state.patternsDetected.length}
  Attractor: ${state.attractorType}
`);
```

## 🔐 Security Considerations

### API Keys
Always use environment variables for API keys:

```bash
# .env file
OPENAI_API_KEY=sk-...
AGENTIC_FLOW_API_KEY=...
```

### Stream Authentication
When using WebRTC or RTMP, ensure proper authentication:

```typescript
const client = new RestreamClient({
  rtmpUrl: 'rtmps://secure.example.com/live', // Use RTMPS
  streamKey: process.env.STREAM_KEY,
  apiKey: process.env.API_KEY
});
```

### Rate Limiting
Implement rate limiting for API calls:

```typescript
// Limit message processing rate
let lastProcess = 0;
const minInterval = 100; // ms

function processWithRateLimit(message: string) {
  const now = Date.now();
  if (now - lastProcess >= minInterval) {
    dashboard.processMessage(message, message.length);
    lastProcess = now;
  }
}
```

## 📊 Performance Optimization

### Buffer Management
The dashboard automatically manages buffers:
- Recent messages: Last 5 messages
- Frame buffer: Last 100 frames
- Audio buffer: Last 100 chunks

### Memory Usage
Monitor and optimize memory usage:

```typescript
// Periodic cleanup
setInterval(() => {
  if (global.gc) {
    global.gc();
  }
}, 60000);
```

### CPU Optimization
Adjust refresh rate based on CPU usage:

```typescript
// Start with fast refresh
dashboard.start(100);

// Reduce if CPU is high
if (cpuUsage > 80) {
  dashboard.stop();
  dashboard.start(500);
}
```

## 🐛 Troubleshooting

### Dashboard Not Updating
- Check refresh rate is appropriate
- Verify messages are being processed
- Check console for errors

### Stream Not Connecting
- Verify URL and credentials
- Check network connectivity
- Review firewall settings

### High CPU Usage
- Increase refresh interval
- Reduce stream resolution
- Disable unnecessary features

### Memory Leaks
- Check buffer sizes
- Verify event listeners are cleaned up
- Monitor with `process.memoryUsage()`

## 📚 API Reference

### MidStreamDashboard

#### Constructor
```typescript
new MidStreamDashboard()
```

#### Methods
- `start(refreshRate: number): void` - Start dashboard
- `stop(): void` - Stop dashboard
- `processMessage(message: string, tokens?: number): void` - Process text message
- `processStream(streamId: string, data: Buffer, type: 'audio' | 'video' | 'text'): void` - Process stream data
- `getAgent(): MidStreamAgent` - Get underlying agent
- `getState(): DashboardState` - Get current state

### RestreamClient

#### Constructor
```typescript
new RestreamClient(config: RestreamConfig)
```

#### Methods
- `connectRTMP(): Promise<void>` - Connect to RTMP stream
- `connectWebRTC(): Promise<void>` - Connect to WebRTC stream
- `connectHLS(url: string): Promise<void>` - Connect to HLS stream
- `disconnect(): void` - Disconnect from stream
- `getAnalysis(): StreamAnalysis` - Get stream analysis
- `getStats()` - Get stream statistics

#### Events
- `connected` - Stream connected
- `disconnected` - Stream disconnected
- `frame` - Video frame received
- `audio` - Audio chunk received
- `transcription` - Audio transcribed
- `objects_detected` - Objects detected in frame
- `error` - Error occurred

### StreamSimulator

#### Constructor
```typescript
new StreamSimulator(frameRate: number)
```

#### Methods
- `start(onFrame, onAudio?): void` - Start simulation
- `stop(): void` - Stop simulation
- `getFrameNumber(): number` - Get current frame number

## 🤝 Contributing

Contributions are welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## 📄 License

MIT License - see LICENSE file for details

## 👨‍💻 Author

**Created by rUv**

For questions or support, please open an issue on GitHub.

## 🙏 Acknowledgments

- OpenAI for Realtime API
- WebRTC community
- Node.js community
- All contributors

---

**MidStream Dashboard** - Real-time introspection for the AI age
