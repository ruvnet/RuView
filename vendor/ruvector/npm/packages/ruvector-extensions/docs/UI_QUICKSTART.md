# 🚀 Quick Start Guide - RuVector Graph Explorer

## 5-Minute Setup

### Prerequisites
- Node.js 18+
- npm or yarn

### Installation

```bash
# Install the package
npm install ruvector-extensions

# Install peer dependencies for UI server
npm install express ws

# Install dev dependencies for TypeScript
npm install -D tsx @types/express @types/ws
```

### Minimal Example

Create a file `graph-ui.ts`:

```typescript
import { RuvectorCore } from 'ruvector-core';
import { startUIServer } from 'ruvector-extensions';

async function main() {
    // 1. Create database
    const db = new RuvectorCore({ dimension: 384 });

    // 2. Add sample data
    const sampleEmbedding = Array(384).fill(0).map(() => Math.random());
    await db.add('sample-1', sampleEmbedding, {
        label: 'My First Node',
        category: 'example'
    });

    // 3. Start UI server
    await startUIServer(db, 3000);

    console.log('🌐 Open http://localhost:3000 in your browser!');
}

main();
```

Run it:

```bash
npx tsx graph-ui.ts
```

Open your browser at **http://localhost:3000**

## What You'll See

1. **Interactive Graph** - A force-directed visualization of your vectors
2. **Search Bar** - Filter nodes by ID or metadata
3. **Metadata Panel** - Click any node to see details
4. **Controls** - Zoom, pan, export, and more

## Next Steps

### Add More Data

```typescript
// Generate 50 sample nodes
for (let i = 0; i < 50; i++) {
    const embedding = Array(384).fill(0).map(() => Math.random());
    await db.add(`node-${i}`, embedding, {
        label: `Node ${i}`,
        category: ['research', 'code', 'docs'][i % 3]
    });
}
```

### Find Similar Nodes

1. Click any node in the graph
2. Click "Find Similar Nodes" button
3. Watch similar nodes highlight

### Customize Colors

Edit `src/ui/app.js`:

```javascript
getNodeColor(node) {
    const colors = {
        'research': '#667eea',
        'code': '#f093fb',
        'docs': '#4caf50'
    };
    return colors[node.metadata?.category] || '#667eea';
}
```

### Export Visualization

Click the **PNG** or **SVG** button in the header to save your graph.

## Common Tasks

### Real-time Updates

```typescript
// Add nodes dynamically
setInterval(async () => {
    const embedding = Array(384).fill(0).map(() => Math.random());
    await db.add(`dynamic-${Date.now()}`, embedding, {
        label: 'Real-time Node',
        timestamp: Date.now()
    });

    // Notify UI
    server.notifyGraphUpdate();
}, 5000);
```

### Search Nodes

Type in the search box to filter by:
- Node ID
- Metadata values
- Labels

### Adjust Similarity

Use the **Min Similarity** slider to control which connections are shown:
- 0.0 = Show all connections
- 0.5 = Medium similarity (default)
- 0.8 = High similarity only

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `+` | Zoom in |
| `-` | Zoom out |
| `0` | Reset view |
| `F` | Fit to view |

## Mobile Support

The UI works great on mobile devices:
- Pinch to zoom
- Drag to pan
- Tap to select nodes
- Swipe to navigate

## API Examples

### REST API

```bash
# Get graph data
curl http://localhost:3000/api/graph

# Search nodes
curl http://localhost:3000/api/search?q=research

# Find similar
curl http://localhost:3000/api/similarity/node-1?threshold=0.5

# Get stats
curl http://localhost:3000/api/stats
```

### WebSocket

```javascript
const ws = new WebSocket('ws://localhost:3000');

ws.onmessage = (event) => {
    const data = JSON.parse(event.data);
    console.log('Received:', data);
};

// Subscribe to updates
ws.send(JSON.stringify({ type: 'subscribe' }));
```

## Troubleshooting

### Port Already in Use
```bash
# Use a different port
await startUIServer(db, 3001);
```

### Graph Not Loading
```bash
# Check database has data
curl http://localhost:3000/api/stats
```

### WebSocket Disconnected
- Check browser console for errors
- Verify firewall allows WebSocket connections
- Look for red status indicator in header

## Full Example

See the complete example:
```bash
npm run example:ui
```

## Next: Read the Full Guide

📚 [Complete UI Guide](./UI_GUIDE.md)

📖 [API Reference](./API.md)

🎨 [Customization Guide](./CUSTOMIZATION.md)

---

Need help? Open an issue: https://github.com/ruvnet/ruvector/issues
