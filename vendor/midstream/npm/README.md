# MidStream CLI

[![npm version](https://img.shields.io/npm/v/midstream-cli.svg)](https://www.npmjs.com/package/midstream-cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.3-blue.svg)](https://www.typescriptlang.org/)
[![WASM](https://img.shields.io/badge/WebAssembly-Enabled-blueviolet.svg)](https://webassembly.org/)
[![MCP](https://img.shields.io/badge/MCP-Compatible-green.svg)](https://modelcontextprotocol.io/)

**Real-time LLM Streaming with Lean Agentic Learning**

Created by [ruv.io](https://ruv.io) | [@ruvnet](https://github.com/ruvnet)

---

## 🌟 Introduction

**MidStream** is a cutting-edge CLI and MCP (Model Context Protocol) server that brings state-of-the-art autonomous agent capabilities to LLM streaming. Unlike traditional systems that process data after completion, MidStream analyzes, learns, and adapts **in real-time** as data flows through.

### What Makes MidStream Special?

- **Inflight Learning**: Learns from streaming data as it arrives, not after
- **Temporal Intelligence**: Detects patterns and predicts next steps in conversations
- **Meta-Learning**: Improves how it learns, creating a self-optimizing system
- **Formal Verification**: Ensures safety and correctness using temporal logic
- **Ultra-Fast**: WebAssembly-powered with <1ms latency on critical operations

## ✨ Features

### 🚀 Core Capabilities

**Temporal Analysis**
- Dynamic Time Warping (DTW) for sequence similarity
- Longest Common Subsequence (LCS) for pattern matching
- Edit distance calculation for measuring differences
- Automatic pattern detection in conversation flows

**Real-Time Scheduling**
- Earliest Deadline First (EDF) scheduling
- Rate-Monotonic scheduling
- Priority-based task execution
- Nanosecond-precision timing

**Behavior Analysis**
- Strange attractor detection
- Chaos/stability monitoring via Lyapunov exponents
- Phase space reconstruction
- Predictive behavior modeling

**Formal Verification**
- Linear Temporal Logic (LTL) verification
- Metric Temporal Logic (MTL) with time bounds
- Neural-symbolic reasoning
- Automated counterexample generation

**Meta-Learning**
- 4-level meta-learning hierarchy
- Strange loop detection
- Self-referential reasoning
- Safe self-modification with safety constraints

### 📡 Streaming Protocols

- **WebSocket**: Full-duplex real-time communication
- **SSE (Server-Sent Events)**: Unidirectional server push
- **HTTP Streaming**: Compatible with standard HTTP clients

### 🔧 MCP Integration

Full Model Context Protocol support enables:
- Seamless integration with MCP-compatible LLM tools
- Standard tool interface for conversation analysis
- Real-time pattern detection and prediction
- Temporal sequence comparison

## 🎯 Benefits

### For Developers

✅ **Drop-in Integration**: Works with existing LLM pipelines
✅ **Language Agnostic**: WASM bindings work in any JavaScript environment
✅ **Production Ready**: Comprehensive tests and benchmarks
✅ **Well Documented**: Extensive API docs and examples

### For AI Applications

🧠 **Smarter Agents**: Meta-learning enables continuous improvement
⚡ **Ultra-Responsive**: <10ms analysis latency for real-time applications
🛡️ **Safety First**: Formal verification ensures correct behavior
📊 **Deep Insights**: Temporal analysis reveals hidden patterns

### For Research

🔬 **State-of-the-Art**: Implements latest research in temporal logic and dynamical systems
📈 **Reproducible**: Comprehensive benchmarking and testing framework
🔓 **Open Source**: Full access to implementation details

## 🌐 Unique Position

MidStream is the **only** open-source solution that combines:

1. **Lean Agentic Programming**: Formal reasoning + autonomous agents
2. **Real-Time Streaming Analysis**: Process data inflight, not in batch
3. **Temporal Intelligence**: DTW, LCS, and pattern matching for sequences
4. **Dynamical Systems Theory**: Chaos detection and stability analysis
5. **Meta-Learning**: Multi-level learning hierarchy with strange loops
6. **WASM Performance**: Native speed in any JavaScript environment
7. **MCP Compatibility**: Standard protocol for LLM tool integration

### Competitive Comparison

| Feature | MidStream | Traditional Agents | LangChain | AutoGPT |
|---------|-----------|-------------------|-----------|---------|
| Real-time Learning | ✅ | ❌ | ❌ | ❌ |
| Temporal Analysis | ✅ | ❌ | ❌ | ❌ |
| Meta-Learning | ✅ | ❌ | ❌ | ❌ |
| Formal Verification | ✅ | ❌ | ❌ | ❌ |
| WASM Performance | ✅ | ❌ | ❌ | ❌ |
| MCP Support | ✅ | ❌ | ⚠️ | ❌ |
| Streaming Protocols | 3 | 0-1 | 0-1 | 0 |

## 🚀 Quick Start

### Installation

```bash
npm install -g midstream-cli
```

### CLI Usage

#### Process a Message

```bash
midstream process "Hello, how can I analyze patterns?"
```

#### Analyze a Conversation

```bash
midstream analyze examples/conversation1.json
```

#### Compare Two Sequences

```bash
midstream compare examples/sequence1.json examples/sequence2.json --algorithm dtw
```

#### Start Streaming Servers

```bash
midstream serve --ws-port 3001 --sse-port 3002
```

This starts both WebSocket and SSE servers:
- WebSocket: `ws://localhost:3001`
- SSE: `http://localhost:3002`

#### Interactive Mode

```bash
midstream interactive
```

Provides a menu-driven interface for all operations.

#### Run Benchmarks

```bash
midstream benchmark --size 100 --iterations 1000
```

### MCP Server

Start the MCP server for integration with MCP-compatible tools:

```bash
midstream mcp
```

Or use npm script:

```bash
npm run mcp
```

The MCP server provides these tools:
- `analyze_conversation` - Analyze conversation patterns
- `compare_sequences` - Compare temporal sequences
- `detect_patterns` - Find pattern occurrences
- `analyze_behavior` - Detect chaos/stability
- `meta_learn` - Perform meta-learning
- `get_status` - Get agent status
- `stream_websocket` - Start WebSocket server
- `stream_sse` - Start SSE server

## 📚 Usage Examples

### Node.js/TypeScript Integration

```typescript
import { MidStreamAgent } from 'midstream-cli';

// Create agent
const agent = new MidStreamAgent({
  maxHistory: 1000,
  embeddingDim: 3,
});

// Process streaming messages
const result = agent.processMessage("What's the weather?");

// Analyze conversation
const analysis = agent.analyzeConversation([
  "Hello",
  "What's the weather?",
  "It's sunny and 72°F",
  "Perfect, thank you!"
]);

console.log('Pattern detection:', analysis.patterns);
console.log('Meta-learning insights:', analysis.metaLearning);

// Compare sequences
const similarity = agent.compareSequences(
  ["greeting", "weather", "response"],
  ["greeting", "weather", "location", "response"],
  "dtw"
);

console.log('Sequence similarity:', similarity);

// Analyze behavior
const behaviorAnalysis = agent.analyzeBehavior([
  0.8, 0.82, 0.79, 0.81, 0.80
]);

console.log('Is stable:', behaviorAnalysis.isStable);
console.log('Is chaotic:', behaviorAnalysis.isChaotic);
```

### WebSocket Client

```typescript
import { WebSocket } from 'ws';

const ws = new WebSocket('ws://localhost:3001');

ws.on('open', () => {
  // Send a message for processing
  ws.send(JSON.stringify({
    type: 'process',
    payload: {
      message: 'Hello, MidStream!'
    }
  }));
});

ws.on('message', (data) => {
  const response = JSON.parse(data.toString());
  console.log('Received:', response);
});
```

### SSE Client

```typescript
const EventSource = require('eventsource');

const es = new EventSource('http://localhost:3002/stream');

es.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('SSE Update:', data);
};

// Send data via HTTP POST
fetch('http://localhost:3002/process', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ message: 'Hello!' })
});
```

### Browser Usage

```html
<!DOCTYPE html>
<html>
<head>
  <title>MidStream WASM Demo</title>
</head>
<body>
  <script type="module">
    import init, { MidStreamAgent } from './midstream_wasm.js';

    async function main() {
      await init();

      const agent = new MidStreamAgent({
        maxHistory: 100,
        embeddingDim: 3,
      });

      const result = agent.process_message("Hello!");
      console.log(result);
    }

    main();
  </script>
</body>
</html>
```

## 🔧 Configuration

### Agent Configuration

```typescript
const config = {
  maxHistory: 1000,        // Maximum conversation history
  embeddingDim: 3,         // Embedding dimension for attractor analysis
  schedulingPolicy: 'EDF', // EDF, RM, Priority, or FIFO
};

const agent = new MidStreamAgent(config);
```

### Server Configuration

```bash
# WebSocket server on custom port
midstream serve --ws-port 8080

# SSE server on custom port
midstream serve --sse-port 8081

# Both servers
midstream serve --ws-port 8080 --sse-port 8081
```

## 🧪 Testing

```bash
# Run all tests
npm test

# Run with coverage
npm run test:coverage

# Watch mode
npm run test:watch
```

## 📊 Benchmarks

Run performance benchmarks:

```bash
midstream benchmark --size 100 --iterations 1000
```

Expected performance (on modern hardware):
- DTW (n=100): <10ms
- LCS (n=100): <5ms
- Pattern Detection: <50ms
- Meta-Learning: <5ms per event
- WebSocket Latency: <1ms

## 🛠️ Development

### Build from Source

```bash
# Clone repository
git clone https://github.com/ruvnet/midstream
cd midstream/npm

# Install dependencies
npm install

# Build WASM bindings
npm run build:wasm

# Build TypeScript
npm run build:ts

# Run tests
npm test
```

### Project Structure

```
npm/
├── src/
│   ├── agent.ts          # Agent wrapper
│   ├── cli.ts            # CLI implementation
│   ├── mcp-server.ts     # MCP server
│   ├── streaming.ts      # WebSocket & SSE servers
│   └── __tests__/        # Test files
├── examples/             # Example data files
├── wasm/                 # Built WASM bindings
└── dist/                 # Built JavaScript
```

## 📖 API Documentation

### MidStreamAgent

**Constructor**
```typescript
new MidStreamAgent(config?: AgentConfig)
```

**Methods**
- `processMessage(message: string)` - Process single message
- `analyzeConversation(messages: string[])` - Analyze conversation
- `compareSequences(seq1, seq2, algorithm)` - Compare sequences
- `detectPattern(sequence, pattern)` - Find pattern occurrences
- `analyzeBehavior(rewards: number[])` - Analyze behavior stability
- `learn(content: string, reward: number)` - Meta-learning
- `getStatus()` - Get agent status
- `reset()` - Clear all history

### Streaming Servers

**WebSocketStreamServer**
```typescript
const server = new WebSocketStreamServer(port);
await server.start();
```

**SSEStreamServer**
```typescript
const server = new SSEStreamServer(port);
await server.start();
```

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## 📄 License

MIT License - see [LICENSE](LICENSE) file for details.

## 🔗 Links

- **GitHub**: https://github.com/ruvnet/midstream
- **npm Package**: https://www.npmjs.com/package/midstream-cli
- **Documentation**: https://docs.midstream.dev
- **Discord**: https://discord.gg/midstream
- **Created by**: [ruv.io](https://ruv.io) | [@ruvnet](https://github.com/ruvnet)

## 🙏 Acknowledgments

Built on cutting-edge research in:
- Temporal Logic (Pnueli 1977)
- Dynamical Systems Theory (Strogatz 2015)
- Strange Loops (Hofstadter 1979)
- Meta-Learning (Finn et al. 2017)
- Real-Time Scheduling (Liu & Layland 1973)

## 📈 Roadmap

- [ ] GPU acceleration for large-scale DTW
- [ ] Distributed processing support
- [ ] Advanced temporal logic operators
- [ ] QUIC protocol support
- [ ] Browser extension for LLM analysis
- [ ] Visual dashboard for real-time monitoring

## 💬 Support

- GitHub Issues: https://github.com/ruvnet/midstream/issues
- Discord Community: https://discord.gg/midstream
- Email: support@ruv.io

---

**Made with ❤️ by the MidStream Team**

*Empowering the next generation of intelligent agents*
