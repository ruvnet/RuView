/**
 * API module exports
 *
 * Provides REST and GraphQL endpoints.
 */

// Placeholder exports - to be implemented
export const API_MODULE_VERSION = '0.1.0';

export interface APIServerOptions {
  port: number;
  host?: string;
  cors?: boolean;
  rateLimit?: {
    max: number;
    timeWindow: number;
  };
  auth?: {
    enabled: boolean;
    type: 'bearer' | 'basic' | 'apikey';
    secret?: string;
  };
}

export interface APIRoute {
  method: 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH';
  path: string;
  handler: (request: unknown, reply: unknown) => Promise<unknown>;
  schema?: Record<string, unknown>;
}
