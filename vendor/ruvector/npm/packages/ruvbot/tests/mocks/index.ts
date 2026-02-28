/**
 * Mock Module Index
 *
 * Central exports for all RuvBot test mocks
 */

// WASM Mocks
export {
  MockWasmVectorIndex,
  MockWasmEmbedder,
  MockWasmRouter,
  mockWasmLoader,
  createMockRuVectorBindings,
  resetWasmMocks,
  type WasmVectorIndex,
  type WasmEmbedder,
  type WasmRouter,
  type SearchResult,
  type RouteResult
} from './wasm.mock';

// PostgreSQL Mocks
export {
  MockPool,
  createMockPool,
  mockPoolFactory,
  queryBuilderHelpers,
  type QueryResult,
  type PoolClient,
  type PoolConfig
} from './postgres.mock';

// Slack Mocks
export {
  MockSlackWebClient,
  MockSlackEventsHandler,
  MockSlackBoltApp,
  createMockSlackClient,
  createMockSlackApp,
  type SlackMessage,
  type SlackResponse,
  type SlackUser,
  type SlackChannel
} from './slack.mock';
