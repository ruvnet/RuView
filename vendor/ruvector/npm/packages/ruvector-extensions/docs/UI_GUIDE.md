# RuVector Graph Explorer UI Guide

## Overview

The RuVector Graph Explorer is an interactive web-based UI for visualizing and exploring vector embeddings as a force-directed graph. Built with D3.js, it provides real-time updates, similarity queries, and comprehensive graph exploration tools.

## Features

### 🎨 Visualization
- **Force-directed graph layout** - Nodes naturally cluster based on similarity
- **Interactive node dragging** - Reposition nodes by dragging
- **Zoom and pan** - Navigate large graphs with mouse/touch gestures
- **Responsive design** - Works seamlessly on desktop, tablet, and mobile

### 🔍 Search & Filter
- **Node search** - Find nodes by ID or metadata content
- **Similarity queries** - Click nodes to find similar vectors
- **Threshold filtering** - Adjust minimum similarity for connections
- **Max nodes limit** - Control graph density for performance

### 📊 Data Exploration
- **Metadata panel** - View detailed information for selected nodes
- **Statistics display** - Real-time node and edge counts
- **Color coding** - Visual categorization by metadata
- **Link weights** - Edge thickness represents similarity strength

### 💾 Export
- **PNG export** - Save visualizations as raster images
- **SVG export** - Export as scalable vector graphics
- **High quality** - Preserves graph layout and styling

### ⚡ Real-time Updates
- **WebSocket integration** - Live graph updates
- **Connection status** - Visual indicator of server connection
- **Toast notifications** - User-friendly feedback

## Quick Start

### 1. Installation

```bash
npm install ruvector-extensions
```

### 2. Basic Usage

```typescript
import { RuvectorCore } from 'ruvector-core';
import { startUIServer } from 'ruvector-extensions/ui-server';

// Initialize database
const db = new RuvectorCore({ dimension: 384 });

// Add some vectors
await db.add('doc1', embedding1, { label: 'Document 1', category: 'research' });
await db.add('doc2', embedding2, { label: 'Document 2', category: 'code' });

// Start UI server
const server = await startUIServer(db, 3000);

// Open browser at http://localhost:3000
```

### 3. Run Example

```bash
cd packages/ruvector-extensions
npm run example:ui
```

Then open your browser at `http://localhost:3000`

## UI Components

### Header
- **Title** - Application branding
- **Export buttons** - PNG and SVG export
- **Reset view** - Return to default zoom/pan
- **Connection status** - WebSocket connection indicator

### Sidebar

#### Search & Filter Section
- **Search input** - Type to filter nodes by ID or metadata
- **Clear button** - Reset search results
- **Similarity slider** - Adjust minimum similarity threshold (0-1)
- **Max nodes input** - Limit displayed nodes (10-1000)
- **Apply filters** - Refresh graph with new settings

#### Statistics Section
- **Nodes count** - Total visible nodes
- **Edges count** - Total visible connections
- **Selected node** - Currently selected node ID

#### Metadata Panel (when node selected)
- **Node details** - ID and metadata key-value pairs
- **Find similar** - Query for similar nodes
- **Close button** - Hide metadata panel

### Graph Canvas
- **Main visualization** - Force-directed graph
- **Zoom controls** - +/- buttons and fit-to-view
- **Loading overlay** - Progress indicator during operations

## Interactions

### Mouse/Touch Controls

| Action | Result |
|--------|--------|
| Click node | Select and show metadata |
| Double-click node | Find similar nodes |
| Drag node | Reposition node |
| Scroll/pinch | Zoom in/out |
| Drag background | Pan view |
| Click background | Deselect node |

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `+` | Zoom in |
| `-` | Zoom out |
| `0` | Reset view |
| `F` | Fit to view |
| `Esc` | Clear selection |

## API Endpoints

### REST API

```typescript
// Get graph data
GET /api/graph?max=100

// Search nodes
GET /api/search?q=query

// Find similar nodes
GET /api/similarity/:nodeId?threshold=0.5&limit=10

// Get node details
GET /api/nodes/:nodeId

// Add new node
POST /api/nodes
{
  "id": "node-123",
  "embedding": [0.1, 0.2, ...],
  "metadata": { "label": "Example" }
}

// Get statistics
GET /api/stats

// Health check
GET /health
```

### WebSocket Messages

#### Client → Server

```javascript
// Subscribe to updates
{
  "type": "subscribe"
}

// Request graph data
{
  "type": "request_graph",
  "maxNodes": 100
}

// Similarity query
{
  "type": "similarity_query",
  "nodeId": "node-123",
  "threshold": 0.5,
  "limit": 10
}
```

#### Server → Client

```javascript
// Connection established
{
  "type": "connected",
  "message": "Connected to RuVector UI Server"
}

// Graph data update
{
  "type": "graph_data",
  "payload": {
    "nodes": [...],
    "links": [...]
  }
}

// Node added
{
  "type": "node_added",
  "payload": { "id": "node-123", "metadata": {...} }
}

// Similarity results
{
  "type": "similarity_result",
  "payload": {
    "nodeId": "node-123",
    "similar": [...]
  }
}
```

## Customization

### Node Colors

Edit `app.js` to customize node colors:

```javascript
getNodeColor(node) {
    if (node.metadata && node.metadata.category) {
        const colors = {
            'research': '#667eea',
            'code': '#f093fb',
            'documentation': '#4caf50',
            'test': '#ff9800'
        };
        return colors[node.metadata.category] || '#667eea';
    }
    return '#667eea';
}
```

### Styling

Edit `styles.css` to customize appearance:

```css
:root {
    --primary-color: #667eea;
    --secondary-color: #764ba2;
    --accent-color: #f093fb;
    /* ... more variables ... */
}
```

### Force Layout

Adjust force simulation parameters in `app.js`:

```javascript
this.simulation = d3.forceSimulation()
    .force('link', d3.forceLink().distance(100))
    .force('charge', d3.forceManyBody().strength(-300))
    .force('center', d3.forceCenter(width / 2, height / 2))
    .force('collision', d3.forceCollide().radius(30));
```

## Performance Optimization

### For Large Graphs (1000+ nodes)

1. **Limit visible nodes**
   ```javascript
   const maxNodes = 500; // Reduce from default 1000
   ```

2. **Reduce force iterations**
   ```javascript
   this.simulation.alpha(0.5).alphaDecay(0.05);
   ```

3. **Disable labels for small nodes**
   ```javascript
   label.style('display', d => this.zoom.scale() > 1.5 ? 'block' : 'none');
   ```

4. **Use clustering**
   - Group similar nodes before rendering
   - Show clusters as single nodes
   - Expand on demand

### Mobile Optimization

The UI is already optimized for mobile:
- Touch gestures for zoom/pan
- Responsive sidebar layout
- Simplified controls on small screens
- Efficient rendering with requestAnimationFrame

## Troubleshooting

### Graph not loading
- Check browser console for errors
- Verify database has vectors: `GET /api/stats`
- Ensure WebSocket connection: look for green dot in header

### Slow performance
- Reduce max nodes in sidebar
- Clear search/filters
- Restart simulation with fewer iterations
- Check network tab for slow API calls

### WebSocket disconnections
- Check firewall/proxy settings
- Verify port 3000 is accessible
- Look for server errors in terminal

### Export not working
- Ensure browser allows downloads
- Try different export format (PNG vs SVG)
- Check browser compatibility (Chrome/Firefox recommended)

## Browser Support

| Browser | Version | Support |
|---------|---------|---------|
| Chrome | 90+ | ✅ Full |
| Firefox | 88+ | ✅ Full |
| Safari | 14+ | ✅ Full |
| Edge | 90+ | ✅ Full |
| Mobile Safari | 14+ | ✅ Full |
| Chrome Mobile | 90+ | ✅ Full |

## Advanced Usage

### Custom Server Configuration

```typescript
import express from 'express';
import { UIServer } from 'ruvector-extensions/ui-server';

const app = express();
const server = new UIServer(db, 3000);

// Add custom middleware
app.use('/api/custom', customRouter);

// Start with custom configuration
await server.start();
```

### Real-time Notifications

```typescript
// Notify clients of graph updates
server.notifyGraphUpdate();

// Broadcast custom message
server.broadcast({
    type: 'custom_event',
    payload: { message: 'Hello!' }
});
```

### Integration with Existing Apps

```typescript
// Use as middleware
app.use('/graph', server.app);

// Or mount on custom route
const uiRouter = express.Router();
uiRouter.use(server.app);
app.use('/visualize', uiRouter);
```

## License

MIT License - see LICENSE file for details

## Contributing

Contributions welcome! Please see CONTRIBUTING.md for guidelines.

## Support

- 📖 Documentation: https://github.com/ruvnet/ruvector
- 🐛 Issues: https://github.com/ruvnet/ruvector/issues
- 💬 Discussions: https://github.com/ruvnet/ruvector/discussions
