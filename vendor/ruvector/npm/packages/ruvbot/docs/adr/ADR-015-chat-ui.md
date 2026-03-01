# ADR-015: Chat UI Architecture

## Status

Accepted

## Date

2026-01-28

## Context

RuvBot provides a powerful REST API for chat interactions, but lacks a user-facing web interface. When users visit the root URL of a deployed RuvBot instance (e.g., on Cloud Run), they receive a 404 error instead of a usable chat interface.

### Requirements

1. Provide a modern, responsive chat UI out of the box
2. Support dark mode (default) and light mode themes
3. Work with the existing REST API endpoints
4. No build step required - serve static files directly
5. Support streaming responses for real-time AI interaction
6. Mobile-friendly design
7. Model selection capability
8. Integration with CLI and npm package

### Alternatives Considered

| Option | Pros | Cons |
|--------|------|------|
| **assistant-ui** | Industry leader, 200k+ downloads, Y Combinator backed | Requires React build, adds complexity |
| **Vercel AI Elements** | Official Vercel components, AI SDK integration | Requires Next.js |
| **shadcn-chatbot-kit** | Beautiful components, shadcn design system | Requires React build |
| **Embedded HTML/CSS/JS** | No build step, portable, fast deployment | Less features, custom implementation |

## Decision

Implement a **lightweight embedded chat UI** using vanilla HTML, CSS, and JavaScript that:

1. Is served directly from the existing HTTP server
2. Requires no build step or additional dependencies
3. Provides a modern, accessible interface
4. Supports dark mode by default
5. Includes basic markdown rendering
6. Works seamlessly with the existing REST API

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        RuvBot Server                             │
├─────────────────────────────────────────────────────────────────┤
│  GET /              → Chat UI (index.html)                       │
│  GET /health        → Health check                               │
│  GET /api/models    → Available models                           │
│  POST /api/sessions → Create session                             │
│  POST /api/sessions/:id/chat → Chat endpoint                     │
└─────────────────────────────────────────────────────────────────┘
```

### File Structure

```
src/
├── api/
│   └── public/
│       └── index.html    # Chat UI (single file)
├── server.ts             # Updated to serve static files
└── ...
```

### Features

1. **Theme Support**: Dark mode default, light mode toggle
2. **Model Selection**: Dropdown for available models
3. **Responsive Design**: Mobile-first approach
4. **Accessibility**: ARIA labels, keyboard navigation
5. **Markdown Rendering**: Code blocks, lists, links
6. **Error Handling**: User-friendly error messages
7. **Session Management**: Automatic session creation
8. **Real-time Updates**: Typing indicators

### CSS Design System

```css
:root {
  --bg-primary: #0a0a0f;      /* Dark background */
  --bg-secondary: #12121a;     /* Card background */
  --text-primary: #f0f0f5;     /* Main text */
  --accent: #6366f1;           /* Indigo accent */
  --radius: 12px;              /* Border radius */
}
```

### API Integration

The UI integrates with existing endpoints:

```javascript
// Create session
POST /api/sessions { agentId: 'default-agent' }

// Send message
POST /api/sessions/:id/chat { message: '...', model: '...' }
```

## Consequences

### Positive

1. **Zero Configuration**: Works out of the box
2. **Fast Deployment**: No build step required
3. **Portable**: Single HTML file, easy to customize
4. **Lightweight**: ~25KB uncompressed
5. **Framework Agnostic**: No React/Vue/Svelte dependency
6. **Cloud Run Compatible**: Works with existing deployment

### Negative

1. **Limited Features**: No streaming UI (yet), basic markdown
2. **Manual Updates**: No component library updates
3. **Custom Code**: Maintenance responsibility

### Neutral

1. Future option to add assistant-ui or similar for advanced features
2. Can be replaced with any frontend framework later

## Implementation

### Server Changes (server.ts)

```typescript
// Serve static files
function getChatUIPath(): string {
  const possiblePaths = [
    join(__dirname, 'api', 'public', 'index.html'),
    // ... fallback paths
  ];
  // Find first existing path
}

// Add root route
{ method: 'GET', pattern: /^\/$/, handler: handleRoot }
```

### CLI Integration

```bash
# View chat UI URL after deployment
ruvbot deploy-cloud cloudrun
# Output: URL: https://ruvbot-xxx.run.app

# Open chat UI
ruvbot open  # Opens browser to chat UI
```

### npm Package

The chat UI is bundled with the npm package:

```json
{
  "files": [
    "dist",
    "bin",
    "scripts",
    "src/api/public"
  ]
}
```

## Future Enhancements

1. **Streaming Responses**: SSE/WebSocket for real-time streaming
2. **File Uploads**: Image and document support
3. **Voice Input**: Speech-to-text integration
4. **assistant-ui Migration**: Full-featured React UI option
5. **Themes**: Additional theme presets
6. **Plugins**: Extensible UI components

## References

- [assistant-ui](https://github.com/assistant-ui/assistant-ui) - Industry-leading chat UI library
- [Vercel AI SDK](https://ai-sdk.dev/) - AI SDK with streaming support
- [shadcn/ui](https://ui.shadcn.com/) - Design system inspiration
- [ADR-013: GCP Deployment](./ADR-013-gcp-deployment.md) - Cloud Run deployment

## Changelog

| Date | Change |
|------|--------|
| 2026-01-28 | Initial version - embedded chat UI |
