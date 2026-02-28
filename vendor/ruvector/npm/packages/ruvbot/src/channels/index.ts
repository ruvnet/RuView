/**
 * Channels module exports
 *
 * Multi-channel messaging support with unified interface.
 */

// Base adapter and types
export {
  BaseAdapter,
  type ChannelType,
  type Attachment,
  type UnifiedMessage,
  type SendOptions,
  type ChannelCredentials,
  type AdapterConfig,
  type AdapterStatus,
  type MessageHandler,
} from './adapters/BaseAdapter.js';

// Channel adapters
export { SlackAdapter, createSlackAdapter, type SlackCredentials } from './adapters/SlackAdapter.js';
export { DiscordAdapter, createDiscordAdapter, type DiscordCredentials } from './adapters/DiscordAdapter.js';
export { TelegramAdapter, createTelegramAdapter, type TelegramCredentials } from './adapters/TelegramAdapter.js';

// Channel registry
export {
  ChannelRegistry,
  createChannelRegistry,
  type ChannelFilter,
  type ChannelRegistryConfig,
  type AdapterFactory,
} from './ChannelRegistry.js';
