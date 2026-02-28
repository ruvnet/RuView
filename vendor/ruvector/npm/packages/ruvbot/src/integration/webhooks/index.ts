/**
 * Webhook Integration - Inbound and Outbound
 */

export interface WebhookManager {
  inbound: InboundWebhooks;
  outbound: OutboundWebhooks;
}

export interface InboundWebhooks {
  register(path: string, handler: WebhookHandler): void;
  verify(request: WebhookRequest, signature: string): boolean;
}

export type WebhookHandler = (payload: WebhookPayload) => Promise<WebhookResponse>;

export interface WebhookPayload {
  body: unknown;
  headers: Record<string, string>;
  query: Record<string, string>;
}

export interface WebhookResponse {
  status?: number;
  body?: unknown;
}

export interface WebhookRequest {
  body: string;
  headers: Record<string, string>;
}

export interface OutboundWebhooks {
  configure(endpoint: WebhookEndpoint): Promise<string>;
  dispatch(webhookId: string, payload: unknown): Promise<DispatchResult>;
  retry(dispatchId: string): Promise<DispatchResult>;
}

export interface WebhookEndpoint {
  url: string;
  secret: string;
  events: string[];
  headers?: Record<string, string>;
}

export interface DispatchResult {
  success: boolean;
  statusCode?: number;
  latencyMs?: number;
  error?: string;
}
