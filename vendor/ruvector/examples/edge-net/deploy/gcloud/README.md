# Edge-Net Genesis Nodes on Google Cloud

Deploy genesis relay nodes as Google Cloud Functions for global edge distribution.
Manage rUv (Resource Utility Vouchers) ledger and bootstrap the network until self-sustaining.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                   GENESIS NODE ARCHITECTURE                             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │                     GLOBAL EDGE NETWORK                          │   │
│   │                                                                  │   │
│   │     us-east1          europe-west1          asia-east1          │   │
│   │    ┌────────┐        ┌────────┐           ┌────────┐            │   │
│   │    │Genesis │        │Genesis │           │Genesis │            │   │
│   │    │Node 1  │◄──────►│Node 2  │◄─────────►│Node 3  │            │   │
│   │    └───┬────┘        └───┬────┘           └───┬────┘            │   │
│   │        │                 │                    │                  │   │
│   │        └─────────────────┼────────────────────┘                  │   │
│   │                          │                                       │   │
│   │              ┌───────────▼───────────┐                          │   │
│   │              │   Cloud Firestore     │                          │   │
│   │              │   (QDAG Ledger Sync)  │                          │   │
│   │              └───────────────────────┘                          │   │
│   │                                                                  │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│   Browser Nodes Connect to Nearest Genesis Node via Edge CDN           │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Why Google Cloud Functions?

| Feature | Benefit |
|---------|---------|
| **Global Edge** | 35+ regions, <50ms latency worldwide |
| **Auto-scaling** | 0 to millions of requests |
| **Pay-per-use** | $0 when idle, pennies under load |
| **Cold start** | <100ms with min instances |
| **WebSocket** | Via Cloud Run for persistent connections |

## Prerequisites

```bash
# Install Google Cloud SDK
curl https://sdk.cloud.google.com | bash

# Login and set project
gcloud auth login
gcloud config set project YOUR_PROJECT_ID

# Enable required APIs
gcloud services enable \
  cloudfunctions.googleapis.com \
  run.googleapis.com \
  firestore.googleapis.com \
  secretmanager.googleapis.com
```

## Deployment Steps

### 1. Create Firestore Database

```bash
# Create Firestore in Native mode (for QDAG ledger sync)
gcloud firestore databases create \
  --region=nam5 \
  --type=firestore-native
```

### 2. Store Genesis Keys

```bash
# Generate genesis keypair
node -e "
const crypto = require('crypto');
const keypair = crypto.generateKeyPairSync('ed25519');
console.log(JSON.stringify({
  public: keypair.publicKey.export({type: 'spki', format: 'der'}).toString('hex'),
  private: keypair.privateKey.export({type: 'pkcs8', format: 'der'}).toString('hex')
}));
" > genesis-keys.json

# Store in Secret Manager
gcloud secrets create edge-net-genesis-keys \
  --data-file=genesis-keys.json

# Clean up local file
rm genesis-keys.json
```

### 3. Deploy Genesis Functions

```bash
# Deploy to multiple regions
for REGION in us-east1 europe-west1 asia-east1; do
  gcloud functions deploy edge-net-genesis-$REGION \
    --gen2 \
    --runtime=nodejs20 \
    --region=$REGION \
    --source=. \
    --entry-point=genesisHandler \
    --trigger-http \
    --allow-unauthenticated \
    --memory=256MB \
    --timeout=60s \
    --min-instances=1 \
    --max-instances=100 \
    --set-env-vars=REGION=$REGION,NODE_ENV=production
done
```

### 4. Deploy WebSocket Relay (Cloud Run)

```bash
# Build and push container
gcloud builds submit \
  --tag gcr.io/YOUR_PROJECT/edge-net-relay

# Deploy to Cloud Run
gcloud run deploy edge-net-relay \
  --image gcr.io/YOUR_PROJECT/edge-net-relay \
  --platform managed \
  --region us-central1 \
  --allow-unauthenticated \
  --memory 512Mi \
  --min-instances 1 \
  --max-instances 10 \
  --concurrency 1000 \
  --timeout 3600
```

## Genesis Node Code

### index.js (Cloud Function)

```javascript
const functions = require('@google-cloud/functions-framework');
const { Firestore } = require('@google-cloud/firestore');
const { SecretManagerServiceClient } = require('@google-cloud/secret-manager');

const firestore = new Firestore();
const secrets = new SecretManagerServiceClient();

// Genesis node state
let genesisKeys = null;
let ledgerState = null;

// Initialize genesis node
async function init() {
  if (genesisKeys) return;

  // Load genesis keys from Secret Manager
  const [version] = await secrets.accessSecretVersion({
    name: 'projects/YOUR_PROJECT/secrets/edge-net-genesis-keys/versions/latest',
  });
  genesisKeys = JSON.parse(version.payload.data.toString());

  // Load or create genesis ledger
  const genesisDoc = await firestore.collection('edge-net').doc('genesis').get();
  if (!genesisDoc.exists) {
    // Create genesis transaction
    ledgerState = await createGenesisLedger();
    await firestore.collection('edge-net').doc('genesis').set(ledgerState);
  } else {
    ledgerState = genesisDoc.data();
  }
}

// Create genesis ledger with initial supply
async function createGenesisLedger() {
  const crypto = require('crypto');

  const genesis = {
    id: crypto.randomBytes(32).toString('hex'),
    type: 'genesis',
    amount: 1_000_000_000_000_000, // 1 billion rUv (Resource Utility Vouchers)
    recipient: genesisKeys.public,
    timestamp: Date.now(),
    transactions: [],
    tips: [],
    totalSupply: 1_000_000_000_000_000,
    networkCompute: 0,
    nodeCount: 0,
    // Genesis sunset thresholds
    sunsetPhase: 0, // 0=active, 1=transition, 2=read-only, 3=retired
    sunsetThresholds: {
      stopNewConnections: 10_000,
      readOnlyMode: 50_000,
      safeRetirement: 100_000,
    },
  };

  return genesis;
}

// Main handler
functions.http('genesisHandler', async (req, res) => {
  // CORS
  res.set('Access-Control-Allow-Origin', '*');
  res.set('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
  res.set('Access-Control-Allow-Headers', 'Content-Type');

  if (req.method === 'OPTIONS') {
    return res.status(204).send('');
  }

  await init();

  const { action, data } = req.body || {};

  try {
    switch (action) {
      case 'status':
        return res.json({
          nodeId: `genesis-${process.env.REGION}`,
          region: process.env.REGION,
          ledger: {
            totalSupply: ledgerState.totalSupply,
            networkCompute: ledgerState.networkCompute,
            nodeCount: ledgerState.nodeCount,
            tipCount: ledgerState.tips.length,
          },
          multiplier: calculateMultiplier(ledgerState.networkCompute),
          currency: 'rUv', // Resource Utility Vouchers
          sunsetStatus: getSunsetStatus(ledgerState),
        });

      case 'register':
        return await handleRegister(data, res);

      case 'submitTransaction':
        return await handleTransaction(data, res);

      case 'getTips':
        return res.json({ tips: ledgerState.tips.slice(-10) });

      case 'sync':
        return await handleSync(data, res);

      default:
        return res.status(400).json({ error: 'Unknown action' });
    }
  } catch (error) {
    console.error('Error:', error);
    return res.status(500).json({ error: error.message });
  }
});

// Handle node registration
async function handleRegister(data, res) {
  const { nodeId, pubkey, stake } = data;

  // Validate registration
  if (!nodeId || !pubkey) {
    return res.status(400).json({ error: 'Missing nodeId or pubkey' });
  }

  // Store node in Firestore
  await firestore.collection('edge-net').doc('nodes').collection(nodeId).set({
    pubkey,
    stake: stake || 0,
    registeredAt: Date.now(),
    region: process.env.REGION,
    reputation: 0.5,
  });

  ledgerState.nodeCount++;

  return res.json({
    success: true,
    nodeId,
    multiplier: calculateMultiplier(ledgerState.networkCompute),
  });
}

// Handle QDAG transaction
async function handleTransaction(data, res) {
  const { transaction, signature } = data;

  // Validate transaction
  if (!validateTransaction(transaction, signature)) {
    return res.status(400).json({ error: 'Invalid transaction' });
  }

  // Apply to ledger
  await applyTransaction(transaction);

  // Store in Firestore
  await firestore.collection('edge-net').doc('transactions')
    .collection(transaction.id).set(transaction);

  // Update tips
  ledgerState.tips = ledgerState.tips.filter(
    tip => !transaction.validates.includes(tip)
  );
  ledgerState.tips.push(transaction.id);

  // Sync to other genesis nodes
  await syncToOtherNodes(transaction);

  return res.json({
    success: true,
    txId: transaction.id,
    newBalance: await getBalance(transaction.sender),
  });
}

// Handle ledger sync from other genesis nodes
async function handleSync(data, res) {
  const { transactions, fromNode } = data;

  let imported = 0;
  for (const tx of transactions) {
    if (!ledgerState.transactions.find(t => t.id === tx.id)) {
      if (validateTransaction(tx, tx.signature)) {
        await applyTransaction(tx);
        imported++;
      }
    }
  }

  return res.json({ imported, total: ledgerState.transactions.length });
}

// Validate transaction signature and structure
function validateTransaction(tx, signature) {
  // TODO: Implement full Ed25519 verification
  return tx && tx.id && tx.sender && tx.recipient && tx.amount >= 0;
}

// Apply transaction to ledger state
async function applyTransaction(tx) {
  ledgerState.transactions.push(tx);

  // Update network compute for reward calculation
  if (tx.type === 'compute_reward') {
    ledgerState.networkCompute += tx.computeHours || 0;
  }

  // Persist to Firestore
  await firestore.collection('edge-net').doc('genesis').update({
    transactions: ledgerState.transactions,
    tips: ledgerState.tips,
    networkCompute: ledgerState.networkCompute,
  });
}

// Calculate contribution curve multiplier
function calculateMultiplier(networkCompute) {
  const MAX_BONUS = 10.0;
  const DECAY_CONSTANT = 1_000_000;
  return 1 + (MAX_BONUS - 1) * Math.exp(-networkCompute / DECAY_CONSTANT);
}

// Get genesis sunset status
function getSunsetStatus(ledger) {
  const thresholds = ledger.sunsetThresholds || {
    stopNewConnections: 10_000,
    readOnlyMode: 50_000,
    safeRetirement: 100_000,
  };

  let phase = 0;
  let phaseName = 'active';

  if (ledger.nodeCount >= thresholds.safeRetirement) {
    phase = 3;
    phaseName = 'retired';
  } else if (ledger.nodeCount >= thresholds.readOnlyMode) {
    phase = 2;
    phaseName = 'read_only';
  } else if (ledger.nodeCount >= thresholds.stopNewConnections) {
    phase = 1;
    phaseName = 'transition';
  }

  return {
    phase,
    phaseName,
    nodeCount: ledger.nodeCount,
    nextThreshold: phase === 0 ? thresholds.stopNewConnections :
                   phase === 1 ? thresholds.readOnlyMode :
                   phase === 2 ? thresholds.safeRetirement : 0,
    canRetire: phase >= 3,
    message: phase >= 3 ?
      'Network is self-sustaining. Genesis nodes can be safely retired.' :
      `${((ledger.nodeCount / thresholds.safeRetirement) * 100).toFixed(1)}% to self-sustaining`
  };
}

// Get balance for a node
async function getBalance(nodeId) {
  let balance = 0;
  for (const tx of ledgerState.transactions) {
    if (tx.recipient === nodeId) balance += tx.amount;
    if (tx.sender === nodeId) balance -= tx.amount;
  }
  return balance;
}

// Sync transaction to other genesis nodes
async function syncToOtherNodes(transaction) {
  const regions = ['us-east1', 'europe-west1', 'asia-east1'];
  const currentRegion = process.env.REGION;

  for (const region of regions) {
    if (region === currentRegion) continue;

    try {
      const url = `https://${region}-YOUR_PROJECT.cloudfunctions.net/edge-net-genesis-${region}`;
      await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          action: 'sync',
          data: {
            transactions: [transaction],
            fromNode: `genesis-${currentRegion}`,
          },
        }),
      });
    } catch (error) {
      console.error(`Failed to sync to ${region}:`, error.message);
    }
  }
}
```

### package.json

```json
{
  "name": "edge-net-genesis",
  "version": "1.0.0",
  "main": "index.js",
  "engines": {
    "node": ">=20"
  },
  "dependencies": {
    "@google-cloud/functions-framework": "^3.0.0",
    "@google-cloud/firestore": "^7.0.0",
    "@google-cloud/secret-manager": "^5.0.0"
  }
}
```

## WebSocket Relay (Cloud Run)

### Dockerfile

```dockerfile
FROM node:20-slim

WORKDIR /app

COPY package*.json ./
RUN npm ci --only=production

COPY . .

EXPOSE 8080

CMD ["node", "relay.js"]
```

### relay.js

```javascript
const WebSocket = require('ws');
const http = require('http');

const server = http.createServer((req, res) => {
  res.writeHead(200, { 'Content-Type': 'text/plain' });
  res.end('Edge-Net Relay\n');
});

const wss = new WebSocket.Server({ server });

// Connected nodes
const nodes = new Map();

// Handle WebSocket connections
wss.on('connection', (ws, req) => {
  const nodeId = req.headers['x-node-id'] || `anon-${Date.now()}`;
  nodes.set(nodeId, ws);

  console.log(`Node connected: ${nodeId}`);

  ws.on('message', (data) => {
    try {
      const message = JSON.parse(data);
      handleMessage(nodeId, message, ws);
    } catch (error) {
      console.error('Invalid message:', error);
    }
  });

  ws.on('close', () => {
    nodes.delete(nodeId);
    console.log(`Node disconnected: ${nodeId}`);
  });

  // Send welcome message
  ws.send(JSON.stringify({
    type: 'welcome',
    nodeId,
    peers: nodes.size,
  }));
});

// Handle incoming messages
function handleMessage(fromId, message, ws) {
  switch (message.type) {
    case 'broadcast':
      // Broadcast to all other nodes
      for (const [id, peer] of nodes) {
        if (id !== fromId && peer.readyState === WebSocket.OPEN) {
          peer.send(JSON.stringify({
            type: 'message',
            from: fromId,
            data: message.data,
          }));
        }
      }
      break;

    case 'direct':
      // Send to specific node
      const target = nodes.get(message.to);
      if (target && target.readyState === WebSocket.OPEN) {
        target.send(JSON.stringify({
          type: 'message',
          from: fromId,
          data: message.data,
        }));
      }
      break;

    case 'peers':
      // Return list of connected peers
      ws.send(JSON.stringify({
        type: 'peers',
        peers: Array.from(nodes.keys()).filter(id => id !== fromId),
      }));
      break;

    default:
      console.warn('Unknown message type:', message.type);
  }
}

const PORT = process.env.PORT || 8080;
server.listen(PORT, () => {
  console.log(`Edge-Net Relay listening on port ${PORT}`);
});
```

## Monitoring

### Cloud Monitoring Dashboard

```bash
# Create dashboard
gcloud monitoring dashboards create \
  --config-from-file=dashboard.json
```

### dashboard.json

```json
{
  "displayName": "Edge-Net Genesis Nodes",
  "mosaicLayout": {
    "columns": 12,
    "tiles": [
      {
        "width": 6,
        "height": 4,
        "widget": {
          "title": "Request Count by Region",
          "xyChart": {
            "dataSets": [{
              "timeSeriesQuery": {
                "timeSeriesFilter": {
                  "filter": "resource.type=\"cloud_function\" AND metric.type=\"cloudfunctions.googleapis.com/function/execution_count\""
                }
              }
            }]
          }
        }
      },
      {
        "xPos": 6,
        "width": 6,
        "height": 4,
        "widget": {
          "title": "Execution Latency",
          "xyChart": {
            "dataSets": [{
              "timeSeriesQuery": {
                "timeSeriesFilter": {
                  "filter": "resource.type=\"cloud_function\" AND metric.type=\"cloudfunctions.googleapis.com/function/execution_times\""
                }
              }
            }]
          }
        }
      }
    ]
  }
}
```

## Cost Estimate

| Component | Monthly Cost (Low Traffic) | Monthly Cost (High Traffic) |
|-----------|---------------------------|----------------------------|
| Cloud Functions (3 regions) | $5 | $50 |
| Cloud Run (WebSocket) | $10 | $100 |
| Firestore | $1 | $25 |
| Secret Manager | $0.06 | $0.06 |
| **Total** | **~$16** | **~$175** |

## Security Checklist

- [ ] Enable Cloud Armor for DDoS protection
- [ ] Configure VPC Service Controls
- [ ] Set up Cloud Audit Logs
- [ ] Enable Binary Authorization
- [ ] Configure IAM least privilege
- [ ] Enable Secret Manager rotation
- [ ] Set up alerting policies

## Next Steps

1. Deploy to all regions
2. Initialize genesis ledger
3. Configure DNS with global load balancer
4. Set up monitoring and alerting
5. Run load tests
6. Enable Cloud CDN for static assets
