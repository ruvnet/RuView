# ADR-010: Multi-Channel Integration

## Status
Accepted (Partially Implemented)

## Date
2026-01-27

## Context

Clawdbot supports multiple messaging channels:
- Slack, Discord, Telegram, Signal, WhatsApp, Line, iMessage
- Web, CLI, API interfaces

RuvBot must match and exceed with:
- All Clawdbot channels
- Multi-tenant channel isolation
- Unified message handling

## Decision

### Channel Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    RuvBot Channel Layer                          │
├─────────────────────────────────────────────────────────────────┤
│  Channel Adapters                                                │
│    ├─ SlackAdapter       : @slack/bolt        [IMPLEMENTED]     │
│    ├─ DiscordAdapter     : discord.js         [IMPLEMENTED]     │
│    ├─ TelegramAdapter    : telegraf           [IMPLEMENTED]     │
│    ├─ SignalAdapter      : signal-client      [PLANNED]         │
│    ├─ WhatsAppAdapter    : baileys            [PLANNED]         │
│    ├─ LineAdapter        : @line/bot-sdk      [PLANNED]         │
│    ├─ WebAdapter         : WebSocket + REST   [PLANNED]         │
│    └─ CLIAdapter         : readline + terminal [PLANNED]        │
├─────────────────────────────────────────────────────────────────┤
│  Message Normalization                                           │
│    └─ Unified Message format                                    │
│    └─ Attachment handling                                       │
│    └─ Thread/reply context                                      │
├─────────────────────────────────────────────────────────────────┤
│  Multi-Tenant Isolation                                          │
│    └─ Channel credentials per tenant                            │
│    └─ Namespace isolation                                       │
│    └─ Rate limiting per tenant                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Implementation

Located in `/npm/packages/ruvbot/src/channels/`:
- `ChannelRegistry.ts` - Central registry and routing
- `adapters/BaseAdapter.ts` - Abstract base class
- `adapters/SlackAdapter.ts` - Slack integration
- `adapters/DiscordAdapter.ts` - Discord integration
- `adapters/TelegramAdapter.ts` - Telegram integration

### Unified Message Interface

```typescript
interface UnifiedMessage {
  id: string;
  channelId: string;
  channelType: ChannelType;
  tenantId: string;
  userId: string;
  username?: string;
  content: string;
  attachments?: Attachment[];
  threadId?: string;
  replyTo?: string;
  timestamp: Date;
  metadata: Record<string, unknown>;
}

interface Attachment {
  id: string;
  type: 'image' | 'file' | 'audio' | 'video' | 'link';
  url?: string;
  data?: Buffer;
  mimeType?: string;
  filename?: string;
  size?: number;
}

type ChannelType =
  | 'slack' | 'discord' | 'telegram'
  | 'signal' | 'whatsapp' | 'line'
  | 'imessage' | 'web' | 'api' | 'cli';
```

### BaseAdapter Abstract Class

```typescript
abstract class BaseAdapter {
  type: ChannelType;
  tenantId: string;
  enabled: boolean;

  // Lifecycle
  abstract connect(): Promise<void>;
  abstract disconnect(): Promise<void>;

  // Messaging
  abstract send(channelId: string, content: string, options?: SendOptions): Promise<string>;
  abstract reply(message: UnifiedMessage, content: string, options?: SendOptions): Promise<string>;

  // Event handling
  onMessage(handler: MessageHandler): void;
  offMessage(handler: MessageHandler): void;
  getStatus(): AdapterStatus;
}
```

### Channel Registry

```typescript
interface ChannelRegistry {
  // Registration
  register(adapter: BaseAdapter): void;
  unregister(type: ChannelType, tenantId: string): boolean;

  // Lookup
  get(type: ChannelType, tenantId: string): BaseAdapter | undefined;
  getByType(type: ChannelType): BaseAdapter[];
  getByTenant(tenantId: string): BaseAdapter[];
  getAll(): BaseAdapter[];

  // Lifecycle
  start(): Promise<void>;
  stop(): Promise<void>;

  // Messaging
  onMessage(handler: MessageHandler): void;
  offMessage(handler: MessageHandler): void;
  broadcast(message: string, channelIds: string[], filter?: ChannelFilter): Promise<Map<string, string>>;

  // Statistics
  getStats(): RegistryStats;
}

interface ChannelRegistryConfig {
  defaultRateLimit?: {
    requests: number;
    windowMs: number;
  };
}
```

### Adapter Configuration

```typescript
interface AdapterConfig {
  type: ChannelType;
  tenantId: string;
  credentials: ChannelCredentials;
  enabled?: boolean;
  rateLimit?: {
    requests: number;
    windowMs: number;
  };
}

interface ChannelCredentials {
  token?: string;
  apiKey?: string;
  webhookUrl?: string;
  clientId?: string;
  clientSecret?: string;
  botId?: string;
  [key: string]: unknown;
}
```

### Usage Example

```typescript
import { ChannelRegistry, SlackAdapter, DiscordAdapter } from './channels';

// Create registry with rate limiting
const registry = new ChannelRegistry({
  defaultRateLimit: { requests: 100, windowMs: 60000 }
});

// Register adapters
registry.register(new SlackAdapter({
  type: 'slack',
  tenantId: 'tenant-1',
  credentials: { token: process.env.SLACK_TOKEN }
}));

registry.register(new DiscordAdapter({
  type: 'discord',
  tenantId: 'tenant-1',
  credentials: { token: process.env.DISCORD_TOKEN }
}));

// Handle messages
registry.onMessage(async (message) => {
  console.log(`[${message.channelType}] ${message.userId}: ${message.content}`);
});

// Start all adapters
await registry.start();
```

## Implementation Status

| Adapter | Status | Library | Notes |
|---------|--------|---------|-------|
| Slack | Implemented | @slack/bolt | Full support |
| Discord | Implemented | discord.js | Full support |
| Telegram | Implemented | telegraf | Full support |
| Signal | Planned | signal-client | Requires native deps |
| WhatsApp | Planned | baileys | Unofficial API |
| Line | Planned | @line/bot-sdk | - |
| Web | Planned | WebSocket | Custom implementation |
| CLI | Planned | readline | For testing |

## Consequences

### Positive
- Unified message handling across all channels
- Multi-tenant channel isolation with per-tenant indexing
- Easy to add new channels via BaseAdapter
- Built-in rate limiting per adapter

### Negative
- Complexity of maintaining multiple integrations
- Different channel capabilities (some don't support threads)
- Only 3 of 8+ channels currently implemented

### RuvBot Advantages over Clawdbot
- Multi-tenant channel credentials with isolation
- Channel-specific rate limiting
- Cross-channel message routing via broadcast
- Adapter status tracking and statistics
