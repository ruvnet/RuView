# Edge-Net Browser Deployment

Deploy edge-net directly in browsers without running your own infrastructure.
Earn **rUv (Resource Utility Vouchers)** by contributing idle compute.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                   BROWSER DEPLOYMENT OPTIONS                            │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│   Option A: CDN + Public Genesis          Option B: Self-Hosted        │
│   ┌────────────────────────────┐          ┌────────────────────────┐   │
│   │  Your Website              │          │  Your Website          │   │
│   │  <script src="cdn/...">   │          │  <script src="local">  │   │
│   │         │                  │          │         │              │   │
│   │         ▼                  │          │         ▼              │   │
│   │  ┌────────────────┐       │          │  ┌────────────────┐    │   │
│   │  │ Public Genesis │◄──┐   │          │  │ Your Genesis   │    │   │
│   │  │    Nodes       │   │   │          │  │    Node        │    │   │
│   │  └────────────────┘   │   │          │  └────────────────┘    │   │
│   │                       │   │          │                         │   │
│   │  ┌────────────────┐   │   │          │  P2P via WebRTC        │   │
│   │  │ Public GUN     │◄──┘   │          │  or WebSocket          │   │
│   │  │   Relays       │       │          │                         │   │
│   │  └────────────────┘       │          └────────────────────────┘   │
│   └────────────────────────────┘                                       │
│                                                                         │
│   Best for: Quick start, testing         Best for: Control, privacy   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Option A: CDN Quick Start (Recommended)

### 1. Add Script Tag

```html
<!DOCTYPE html>
<html>
<head>
  <title>My Site</title>
</head>
<body>
  <!-- Your website content -->

  <!-- Edge-Net: Contribute compute, earn credits -->
  <script type="module">
    import { EdgeNet } from 'https://cdn.jsdelivr.net/npm/@ruvector/edge-net/dist/edge-net.min.js';

    const node = await EdgeNet.init({
      siteId: 'my-site-unique-id',
      contribution: 0.3,  // 30% CPU when idle
    });

    console.log(`Node ID: ${node.nodeId()}`);
    console.log(`Balance: ${node.creditBalance()} rUv`);
  </script>
</body>
</html>
```

### 2. NPM Installation (Alternative)

```bash
npm install @ruvector/edge-net
```

```javascript
import { EdgeNet } from '@ruvector/edge-net';

const node = await EdgeNet.init({
  siteId: 'my-site',
  contribution: 0.3,
});
```

## Configuration Options

### Basic Configuration

```javascript
const node = await EdgeNet.init({
  // Required
  siteId: 'your-unique-site-id',

  // Contribution settings
  contribution: {
    cpuLimit: 0.3,              // 0.0 - 1.0 (30% max CPU)
    memoryLimit: 256_000_000,   // 256MB max memory
    bandwidthLimit: 1_000_000,  // 1MB/s max bandwidth
    tasks: ['vectors', 'embeddings', 'encryption'],
  },

  // Idle detection
  idle: {
    minIdleTime: 5000,          // Wait 5s of idle before working
    respectBattery: true,       // Reduce when on battery
    respectDataSaver: true,     // Respect data saver mode
  },

  // UI integration
  ui: {
    showBadge: true,            // Show contribution badge
    badgePosition: 'bottom-right',
    onEarn: (credits) => {
      // Custom notification on earning
      console.log(`Earned ${credits} QDAG!`);
    },
  },
});
```

### Advanced Configuration

```javascript
const node = await EdgeNet.init({
  siteId: 'my-site',

  // Network settings
  network: {
    // Use public genesis nodes (default)
    genesis: [
      'https://us-east1-edge-net.cloudfunctions.net/genesis',
      'https://europe-west1-edge-net.cloudfunctions.net/genesis',
      'https://asia-east1-edge-net.cloudfunctions.net/genesis',
    ],

    // P2P relay servers
    relays: [
      'https://gun-manhattan.herokuapp.com/gun',
      'https://gun-us.herokuapp.com/gun',
    ],

    // WebRTC configuration
    webrtc: {
      enabled: true,
      iceServers: [
        { urls: 'stun:stun.l.google.com:19302' },
      ],
    },
  },

  // Staking for higher priority
  stake: {
    amount: 100,                // Stake 100 QDAG
    autoStake: true,            // Auto-stake earnings
  },

  // Callbacks
  onCredit: (earned, total) => console.log(`+${earned} QDAG`),
  onTask: (task) => console.log(`Processing: ${task.type}`),
  onError: (error) => console.error('Edge-Net error:', error),
  onConnect: (peers) => console.log(`Connected to ${peers} peers`),
  onDisconnect: () => console.log('Disconnected'),
});
```

## Widget Integration

### Contribution Badge

Show users their rUv contribution status:

```html
<!-- Include the badge component -->
<div id="edge-net-badge"></div>

<script type="module">
  import { EdgeNet, Badge } from '@ruvector/edge-net';

  const node = await EdgeNet.init({ siteId: 'my-site' });

  // Mount badge
  Badge.mount('#edge-net-badge', node, {
    theme: 'dark',          // 'light' | 'dark' | 'auto'
    showRuv: true,          // Show rUv balance
    showMultiplier: true,
    showUptime: true,
    minimizable: true,
  });
</script>
```

### Dashboard Widget

Full contribution dashboard:

```html
<div id="edge-net-dashboard" style="width: 400px; height: 300px;"></div>

<script type="module">
  import { EdgeNet, Dashboard } from '@ruvector/edge-net';

  const node = await EdgeNet.init({ siteId: 'my-site' });

  Dashboard.mount('#edge-net-dashboard', node, {
    showStats: true,
    showHistory: true,
    showTasks: true,
    showPeers: true,
  });
</script>
```

## User Consent Patterns

### Opt-In Modal

```html
<script type="module">
  import { EdgeNet, ConsentModal } from '@ruvector/edge-net';

  // Show consent modal before initializing
  const consent = await ConsentModal.show({
    title: 'Help power our AI features',
    message: 'Contribute idle compute cycles to earn QDAG credits.',
    benefits: [
      'Earn credits while browsing',
      'Use credits for AI features',
      'Early adopter bonus: 5.2x multiplier',
    ],
    learnMoreUrl: '/edge-net-info',
  });

  if (consent.accepted) {
    const node = await EdgeNet.init({
      siteId: 'my-site',
      contribution: consent.cpuLimit,  // User-selected limit
    });
  }
</script>
```

### Banner Opt-In

```html
<div id="edge-net-banner"></div>

<script type="module">
  import { EdgeNet, ConsentBanner } from '@ruvector/edge-net';

  ConsentBanner.show('#edge-net-banner', {
    onAccept: async (settings) => {
      const node = await EdgeNet.init({
        siteId: 'my-site',
        contribution: settings.cpuLimit,
      });
    },
    onDecline: () => {
      // User declined - respect their choice
      console.log('User declined edge-net participation');
    },
  });
</script>
```

## Task Submission

Use earned credits for compute tasks:

```javascript
// Check balance first
if (node.creditBalance() >= 5) {
  // Submit vector search task
  const result = await node.submitTask('vector_search', {
    query: new Float32Array(128).fill(0.5),
    k: 10,
  }, {
    maxRuv: 5,            // Max rUv to spend
    timeout: 30000,       // 30s timeout
    priority: 'normal',   // 'low' | 'normal' | 'high'
  });

  console.log('Results:', result.results);
  console.log('Cost:', result.cost, 'rUv');
}
```

### Available Task Types

| Type | Description | Cost |
|------|-------------|------|
| `vector_search` | k-NN search in HNSW index | ~1 rUv / 1K vectors |
| `vector_insert` | Add vectors to index | ~0.5 rUv / 100 vectors |
| `embedding` | Generate text embeddings | ~5 rUv / 100 texts |
| `semantic_match` | Task-to-agent routing | ~1 rUv / 10 queries |
| `encryption` | AES encrypt/decrypt | ~0.1 rUv / MB |
| `compression` | Adaptive quantization | ~0.2 rUv / MB |

## Framework Integration

### React

```jsx
import { useEdgeNet, Badge } from '@ruvector/edge-net/react';

function App() {
  const { node, balance, multiplier, isConnected } = useEdgeNet({
    siteId: 'my-react-app',
    contribution: 0.3,
  });

  return (
    <div>
      <h1>My App</h1>
      {isConnected && (
        <Badge balance={balance} multiplier={multiplier} />
      )}
    </div>
  );
}
```

### Vue 3

```vue
<template>
  <div>
    <h1>My App</h1>
    <EdgeNetBadge v-if="isConnected" :node="node" />
  </div>
</template>

<script setup>
import { useEdgeNet, EdgeNetBadge } from '@ruvector/edge-net/vue';

const { node, isConnected } = useEdgeNet({
  siteId: 'my-vue-app',
  contribution: 0.3,
});
</script>
```

### Next.js

```jsx
// components/EdgeNetProvider.jsx
'use client';

import { EdgeNetProvider } from '@ruvector/edge-net/react';

export default function Providers({ children }) {
  return (
    <EdgeNetProvider config={{ siteId: 'my-next-app', contribution: 0.3 }}>
      {children}
    </EdgeNetProvider>
  );
}

// app/layout.jsx
import Providers from '@/components/EdgeNetProvider';

export default function RootLayout({ children }) {
  return (
    <html>
      <body>
        <Providers>{children}</Providers>
      </body>
    </html>
  );
}
```

## Self-Hosting the WASM Bundle

If you prefer to host the WASM files yourself:

### 1. Download the Package

```bash
npm pack @ruvector/edge-net
tar -xzf ruvector-edge-net-*.tgz
cp -r package/dist/ ./public/edge-net/
```

### 2. Configure Your Web Server

```nginx
# nginx configuration
location /edge-net/ {
    add_header Cross-Origin-Opener-Policy same-origin;
    add_header Cross-Origin-Embedder-Policy require-corp;

    # WASM MIME type
    types {
        application/wasm wasm;
    }
}
```

### 3. Use Local Path

```html
<script type="module">
  import { EdgeNet } from '/edge-net/edge-net.min.js';

  const node = await EdgeNet.init({
    siteId: 'my-site',
    wasmPath: '/edge-net/',  // Path to WASM files
  });
</script>
```

## Option B: Self-Hosted Genesis Node

For full control, run your own genesis node:

### Using Docker

```bash
# Pull the edge-net genesis image
docker pull ruvector/edge-net-genesis:latest

# Run genesis node
docker run -d \
  --name edge-net-genesis \
  -p 8080:8080 \
  -e NODE_ENV=production \
  -e GENESIS_KEYS_PATH=/keys/genesis.json \
  -v ./keys:/keys:ro \
  ruvector/edge-net-genesis:latest
```

### Connect Browsers to Your Genesis

```javascript
const node = await EdgeNet.init({
  siteId: 'my-site',
  network: {
    genesis: ['https://your-genesis.example.com'],
    relays: ['wss://your-relay.example.com'],
  },
});
```

See [../gcloud/README.md](../gcloud/README.md) for Google Cloud Functions deployment.

## Privacy & Compliance

### GDPR Compliance

```javascript
// Check for prior consent
const hasConsent = localStorage.getItem('edge-net-consent') === 'true';

if (hasConsent) {
  const node = await EdgeNet.init({ siteId: 'my-site' });
} else {
  // Show consent UI
  showConsentDialog();
}

// Handle "forget me" requests
async function handleForgetMe() {
  const node = await EdgeNet.getNode();
  if (node) {
    await node.deleteAllData();
    await node.disconnect();
  }
  localStorage.removeItem('edge-net-consent');
}
```

### Data Collected

| Data | Purpose | Retention |
|------|---------|-----------|
| Node ID | Identity | Until user clears |
| Task results | Verification | 24 hours |
| rUv balance | Economics | Permanent (on-chain) |
| IP address | Rate limiting | Not stored |
| Browser fingerprint | Sybil prevention | Hashed, 7 days |

### No Personal Data

Edge-net does NOT collect:
- Names or emails
- Browsing history
- Cookie contents
- Form inputs
- Screen recordings

## Performance Impact

| Scenario | CPU Impact | Memory | Network |
|----------|------------|--------|---------|
| Idle (no tasks) | 0% | ~10MB | 0 |
| Light tasks | 5-10% | ~50MB | ~1KB/s |
| Active contribution | 10-30% | ~100MB | ~10KB/s |
| Heavy workload | 30% (capped) | ~256MB | ~50KB/s |

### Optimization Tips

```javascript
const node = await EdgeNet.init({
  siteId: 'my-site',

  contribution: {
    cpuLimit: 0.2,              // Lower CPU for sensitive sites
    memoryLimit: 128_000_000,   // Lower memory footprint
  },

  idle: {
    minIdleTime: 10000,         // Wait longer before starting
    checkInterval: 5000,        // Check less frequently
  },

  // Pause during critical interactions
  pauseDuringInteraction: true,
});

// Manually pause during important operations
node.pause();
await performCriticalOperation();
node.resume();
```

## Monitoring & Analytics

### Built-in Stats

```javascript
const stats = node.getStats();
console.log({
  uptime: stats.uptimeHours,
  tasksCompleted: stats.tasksCompleted,
  creditsEarned: stats.creditsEarned,
  reputation: stats.reputation,
  peers: stats.connectedPeers,
});
```

### Integration with Analytics

```javascript
// Send to your analytics
const node = await EdgeNet.init({
  siteId: 'my-site',
  onCredit: (earned, total) => {
    gtag('event', 'edge_net_credit', {
      earned,
      total,
      multiplier: node.getMultiplier(),
    });
  },
});
```

## Troubleshooting

### Common Issues

**WASM fails to load**
```
Error: Failed to load WASM module
```
Solution: Ensure CORS headers allow WASM loading from CDN.

**SharedArrayBuffer not available**
```
Error: SharedArrayBuffer is not defined
```
Solution: Add required COOP/COEP headers:
```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

**WebWorkers blocked**
```
Error: Worker constructor blocked
```
Solution: Ensure your CSP allows worker-src.

### Debug Mode

```javascript
const node = await EdgeNet.init({
  siteId: 'my-site',
  debug: true,  // Enable verbose logging
});
```

## Support

- Documentation: https://github.com/ruvnet/ruvector
- Issues: https://github.com/ruvnet/ruvector/issues
- Discord: https://discord.gg/ruvector
