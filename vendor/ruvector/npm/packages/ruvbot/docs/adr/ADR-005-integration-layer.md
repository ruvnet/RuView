# ADR-005: Integration Layer

**Status:** Accepted
**Date:** 2026-01-27
**Decision Makers:** RuVector Architecture Team
**Technical Area:** Integrations, External Services

---

## Context and Problem Statement

RuvBot must integrate with external systems to:

1. **Receive messages** from Slack, webhooks, and other channels
2. **Send notifications** and responses back to users
3. **Connect to AI providers** for LLM inference and embeddings
4. **Interact with external APIs** for skill execution
5. **Provide webhooks** for third-party integrations

The integration layer must be:

- **Extensible** for new integration types
- **Resilient** to external service failures
- **Secure** with proper authentication and authorization
- **Observable** with logging and metrics

---

## Decision Drivers

### Integration Requirements

| Integration | Priority | Features Required |
|-------------|----------|-------------------|
| Slack | Critical | Events, commands, blocks, threads |
| REST Webhooks | Critical | Inbound/outbound, signatures |
| Anthropic Claude | Critical | Completions, streaming |
| OpenAI | High | Completions, embeddings |
| Custom LLMs | Medium | Provider abstraction |
| External APIs | Medium | HTTP client, retries |

### Reliability Requirements

| Requirement | Target |
|-------------|--------|
| Webhook delivery success | > 99% |
| Provider failover time | < 1s |
| Message ordering | Within session |
| Duplicate detection | 100% |

---

## Decision Outcome

### Adopt Adapter Pattern with Circuit Breaker

We implement the integration layer using:

1. **Adapter Pattern**: Common interface for each integration type
2. **Circuit Breaker**: Prevent cascade failures from external services
3. **Retry with Backoff**: Handle transient failures
4. **Event-Driven**: Decouple ingestion from processing

```
+-----------------------------------------------------------------------------+
|                           INTEGRATION LAYER                                  |
+-----------------------------------------------------------------------------+

                    +---------------------------+
                    |     Integration Gateway   |
                    |   (Protocol Normalization)|
                    +-------------+-------------+
                                  |
          +-----------------------+-----------------------+
          |                       |                       |
+---------v---------+   +---------v---------+   +---------v---------+
|  Slack Adapter    |   |  Webhook Adapter  |   | Provider Adapter  |
|-------------------|   |-------------------|   |-------------------|
| - Events API      |   | - Inbound routes  |   | - LLM clients     |
| - Commands        |   | - Outbound queue  |   | - Embeddings      |
| - Interactive     |   | - Signatures      |   | - Circuit breaker |
| - OAuth           |   | - Retries         |   | - Failover        |
+-------------------+   +-------------------+   +-------------------+
          |                       |                       |
          +-----------------------+-----------------------+
                                  |
                    +-------------v-------------+
                    |      Event Normalizer     |
                    | (Unified Message Format)  |
                    +-------------+-------------+
                                  |
                    +-------------v-------------+
                    |       Core Context        |
                    +---------------------------+
```

---

## Slack Integration

### Architecture

```typescript
// Slack integration components
interface SlackIntegration {
  // Event handling
  events: SlackEventHandler;

  // Slash commands
  commands: SlackCommandHandler;

  // Interactive components (buttons, modals)
  interactive: SlackInteractiveHandler;

  // Block Kit builder
  blocks: BlockKitBuilder;

  // Web API client
  client: SlackWebClient;

  // OAuth flow
  oauth: SlackOAuthHandler;
}

// Event types we handle
type SlackEventType =
  | 'message'
  | 'app_mention'
  | 'reaction_added'
  | 'reaction_removed'
  | 'channel_created'
  | 'member_joined_channel'
  | 'file_shared'
  | 'app_home_opened';

// Normalized event structure
interface SlackIncomingEvent {
  type: SlackEventType;
  teamId: string;
  channelId: string;
  userId: string;
  text?: string;
  threadTs?: string;
  ts: string;
  raw: unknown;
}
```

### Event Handler

```typescript
// Slack event processing
class SlackEventHandler {
  private eventQueue: Queue<SlackIncomingEvent>;
  private deduplicator: EventDeduplicator;

  constructor(
    private config: SlackConfig,
    private sessionManager: SessionManager,
    private agent: Agent
  ) {
    this.eventQueue = new Queue('slack-events');
    this.deduplicator = new EventDeduplicator({
      ttl: 300000, // 5 minutes
      keyFn: (e) => `${e.teamId}:${e.channelId}:${e.ts}`,
    });
  }

  // Express middleware for Slack events
  middleware(): RequestHandler {
    return async (req, res) => {
      // Verify Slack signature
      if (!this.verifySignature(req)) {
        return res.status(401).send('Invalid signature');
      }

      const body = req.body;

      // Handle URL verification challenge
      if (body.type === 'url_verification') {
        return res.json({ challenge: body.challenge });
      }

      // Acknowledge immediately (Slack 3s timeout)
      res.status(200).send();

      // Process event asynchronously
      await this.handleEvent(body.event);
    };
  }

  private async handleEvent(rawEvent: unknown): Promise<void> {
    const event = this.normalizeEvent(rawEvent);

    // Deduplicate (Slack may retry)
    if (await this.deduplicator.isDuplicate(event)) {
      this.logger.debug('Duplicate event ignored', { event });
      return;
    }

    // Filter events we care about
    if (!this.shouldProcess(event)) {
      return;
    }

    // Map to tenant context
    const tenant = await this.resolveTenant(event.teamId);
    if (!tenant) {
      this.logger.warn('Unknown Slack team', { teamId: event.teamId });
      return;
    }

    // Enqueue for processing
    await this.eventQueue.add('process', {
      event,
      tenant,
      receivedAt: Date.now(),
    });
  }

  private shouldProcess(event: SlackIncomingEvent): boolean {
    // Skip bot messages
    if (event.raw?.bot_id) return false;

    // Only process certain event types
    return ['message', 'app_mention'].includes(event.type);
  }

  private verifySignature(req: Request): boolean {
    const timestamp = req.headers['x-slack-request-timestamp'] as string;
    const signature = req.headers['x-slack-signature'] as string;

    // Prevent replay attacks (5 minute window)
    const now = Math.floor(Date.now() / 1000);
    if (Math.abs(now - parseInt(timestamp)) > 300) {
      return false;
    }

    const baseString = `v0:${timestamp}:${req.rawBody}`;
    const expectedSignature = `v0=${crypto
      .createHmac('sha256', this.config.signingSecret)
      .update(baseString)
      .digest('hex')}`;

    return crypto.timingSafeEqual(
      Buffer.from(signature),
      Buffer.from(expectedSignature)
    );
  }
}
```

### Slash Commands

```typescript
// Slash command handling
class SlackCommandHandler {
  private commands: Map<string, CommandDefinition> = new Map();

  register(command: CommandDefinition): void {
    this.commands.set(command.name, command);
  }

  middleware(): RequestHandler {
    return async (req, res) => {
      if (!this.verifySignature(req)) {
        return res.status(401).send('Invalid signature');
      }

      const { command, text, user_id, channel_id, team_id, response_url } = req.body;

      const commandDef = this.commands.get(command);
      if (!commandDef) {
        return res.json({
          response_type: 'ephemeral',
          text: `Unknown command: ${command}`,
        });
      }

      // Parse arguments
      const args = this.parseArgs(text, commandDef.argSchema);

      // Acknowledge with loading state
      res.json({
        response_type: 'ephemeral',
        text: 'Processing...',
      });

      try {
        // Execute command
        const result = await commandDef.handler({
          args,
          userId: user_id,
          channelId: channel_id,
          teamId: team_id,
        });

        // Send actual response
        await this.sendResponse(response_url, {
          response_type: result.public ? 'in_channel' : 'ephemeral',
          blocks: result.blocks,
          text: result.text,
        });
      } catch (error) {
        await this.sendResponse(response_url, {
          response_type: 'ephemeral',
          text: `Error: ${(error as Error).message}`,
        });
      }
    };
  }

  private parseArgs(text: string, schema: ArgSchema): Record<string, unknown> {
    const args: Record<string, unknown> = {};
    const parts = text.trim().split(/\s+/);

    for (const [name, def] of Object.entries(schema)) {
      if (def.positional !== undefined) {
        args[name] = parts[def.positional];
      } else if (def.flag) {
        const flagIndex = parts.indexOf(`--${name}`);
        if (flagIndex !== -1) {
          args[name] = parts[flagIndex + 1] ?? true;
        }
      }
    }

    return args;
  }
}

// Command definition
interface CommandDefinition {
  name: string;
  description: string;
  argSchema: ArgSchema;
  handler: (ctx: CommandContext) => Promise<CommandResult>;
}

// Example command
const askCommand: CommandDefinition = {
  name: '/ask',
  description: 'Ask RuvBot a question',
  argSchema: {
    question: { positional: 0, required: true },
    context: { flag: true },
  },
  handler: async (ctx) => {
    const session = await sessionManager.getOrCreate(ctx.userId, ctx.channelId);
    const response = await agent.process(session, ctx.args.question as string);

    return {
      public: false,
      text: response.content,
      blocks: formatResponseBlocks(response),
    };
  },
};
```

### Block Kit Builder

```typescript
// Fluent Block Kit builder
class BlockKitBuilder {
  private blocks: Block[] = [];

  section(text: string): this {
    this.blocks.push({
      type: 'section',
      text: { type: 'mrkdwn', text },
    });
    return this;
  }

  divider(): this {
    this.blocks.push({ type: 'divider' });
    return this;
  }

  context(...elements: string[]): this {
    this.blocks.push({
      type: 'context',
      elements: elements.map(e => ({ type: 'mrkdwn', text: e })),
    });
    return this;
  }

  actions(actionId: string, buttons: Button[]): this {
    this.blocks.push({
      type: 'actions',
      block_id: actionId,
      elements: buttons.map(b => ({
        type: 'button',
        text: { type: 'plain_text', text: b.text },
        action_id: b.actionId,
        value: b.value,
        style: b.style,
      })),
    });
    return this;
  }

  input(label: string, actionId: string, options: InputOptions): this {
    this.blocks.push({
      type: 'input',
      label: { type: 'plain_text', text: label },
      element: {
        type: options.multiline ? 'plain_text_input' : 'plain_text_input',
        action_id: actionId,
        multiline: options.multiline,
        placeholder: options.placeholder
          ? { type: 'plain_text', text: options.placeholder }
          : undefined,
      },
    });
    return this;
  }

  build(): Block[] {
    return this.blocks;
  }
}

// Usage example
const responseBlocks = new BlockKitBuilder()
  .section('Here is what I found:')
  .divider()
  .section(responseText)
  .context(`Generated in ${latencyMs}ms`)
  .actions('feedback', [
    { text: 'Helpful', actionId: 'feedback_positive', value: responseId, style: 'primary' },
    { text: 'Not helpful', actionId: 'feedback_negative', value: responseId },
  ])
  .build();
```

---

## Webhook Integration

### Inbound Webhooks

```typescript
// Inbound webhook configuration
interface WebhookEndpoint {
  id: string;
  path: string;  // e.g., "/webhooks/github"
  method: 'POST' | 'PUT';
  secretKey?: string;
  signatureHeader?: string;
  signatureAlgorithm?: 'hmac-sha256' | 'hmac-sha1';
  handler: WebhookHandler;
  rateLimit?: RateLimitConfig;
}

class InboundWebhookRouter {
  private endpoints: Map<string, WebhookEndpoint> = new Map();

  register(endpoint: WebhookEndpoint): void {
    this.endpoints.set(endpoint.path, endpoint);
  }

  middleware(): RequestHandler {
    return async (req, res, next) => {
      const endpoint = this.endpoints.get(req.path);
      if (!endpoint) {
        return next();
      }

      // Rate limiting
      if (endpoint.rateLimit) {
        const allowed = await this.rateLimiter.check(
          `webhook:${endpoint.id}:${req.ip}`,
          endpoint.rateLimit
        );
        if (!allowed) {
          return res.status(429).json({ error: 'Rate limit exceeded' });
        }
      }

      // Signature verification
      if (endpoint.secretKey) {
        if (!this.verifySignature(req, endpoint)) {
          return res.status(401).json({ error: 'Invalid signature' });
        }
      }

      try {
        const result = await endpoint.handler({
          body: req.body,
          headers: req.headers,
          query: req.query,
        });

        res.status(result.status ?? 200).json(result.body ?? { ok: true });
      } catch (error) {
        this.logger.error('Webhook handler error', { error, endpoint: endpoint.id });
        res.status(500).json({ error: 'Internal error' });
      }
    };
  }

  private verifySignature(req: Request, endpoint: WebhookEndpoint): boolean {
    const signatureHeader = endpoint.signatureHeader ?? 'x-signature';
    const providedSignature = req.headers[signatureHeader.toLowerCase()] as string;

    if (!providedSignature) return false;

    const algorithm = endpoint.signatureAlgorithm ?? 'hmac-sha256';
    const expectedSignature = crypto
      .createHmac(algorithm.replace('hmac-', ''), endpoint.secretKey!)
      .update(req.rawBody)
      .digest('hex');

    // Handle various signature formats
    const normalizedProvided = providedSignature
      .replace(/^sha256=/, '')
      .replace(/^sha1=/, '');

    return crypto.timingSafeEqual(
      Buffer.from(normalizedProvided),
      Buffer.from(expectedSignature)
    );
  }
}
```

### Outbound Webhooks

```typescript
// Outbound webhook delivery
class OutboundWebhookDispatcher {
  constructor(
    private queue: Queue<WebhookDelivery>,
    private storage: WebhookStorage,
    private http: HttpClient
  ) {}

  async dispatch(
    webhookId: string,
    event: WebhookEvent,
    options?: DispatchOptions
  ): Promise<string> {
    const webhook = await this.storage.findById(webhookId);
    if (!webhook || !webhook.isEnabled) {
      throw new Error(`Webhook ${webhookId} not found or disabled`);
    }

    const deliveryId = crypto.randomUUID();
    const payload = this.buildPayload(event, webhook);
    const signature = this.sign(payload, webhook.secret);

    // Queue for delivery
    await this.queue.add(
      'deliver',
      {
        deliveryId,
        webhookId,
        url: webhook.url,
        payload,
        signature,
        headers: webhook.headers,
      },
      {
        attempts: 10,
        backoff: { type: 'exponential', delay: 1000 },
        removeOnComplete: 100,
        removeOnFail: 1000,
      }
    );

    return deliveryId;
  }

  private buildPayload(event: WebhookEvent, webhook: Webhook): string {
    return JSON.stringify({
      id: crypto.randomUUID(),
      type: event.type,
      timestamp: new Date().toISOString(),
      data: event.data,
      webhook_id: webhook.id,
    });
  }

  private sign(payload: string, secret: string): string {
    const timestamp = Math.floor(Date.now() / 1000);
    const signaturePayload = `${timestamp}.${payload}`;
    const signature = crypto
      .createHmac('sha256', secret)
      .update(signaturePayload)
      .digest('hex');
    return `t=${timestamp},v1=${signature}`;
  }
}

// Webhook event types
type WebhookEventType =
  | 'session.created'
  | 'session.ended'
  | 'message.received'
  | 'message.sent'
  | 'memory.created'
  | 'skill.executed'
  | 'error.occurred';

interface WebhookEvent {
  type: WebhookEventType;
  data: Record<string, unknown>;
}
```

---

## LLM Provider Integration

### Provider Abstraction

```typescript
// Unified LLM provider interface
interface LLMProvider {
  // Basic completion
  complete(
    messages: Message[],
    options: CompletionOptions
  ): Promise<Completion>;

  // Streaming completion
  stream(
    messages: Message[],
    options: StreamOptions
  ): AsyncGenerator<Token, Completion, void>;

  // Token counting
  countTokens(text: string): Promise<number>;

  // Model info
  getModel(): ModelInfo;

  // Health check
  isHealthy(): Promise<boolean>;
}

interface CompletionOptions {
  maxTokens?: number;
  temperature?: number;
  topP?: number;
  stopSequences?: string[];
  tools?: Tool[];
}

interface Completion {
  content: string;
  finishReason: 'stop' | 'length' | 'tool_use';
  usage: {
    inputTokens: number;
    outputTokens: number;
  };
  toolCalls?: ToolCall[];
}
```

### Anthropic Claude Provider

```typescript
// Claude provider implementation
class ClaudeProvider implements LLMProvider {
  private client: AnthropicClient;
  private circuitBreaker: CircuitBreaker;

  constructor(config: ClaudeConfig) {
    this.client = new Anthropic({
      apiKey: config.apiKey,
      baseURL: config.baseURL,
    });

    this.circuitBreaker = new CircuitBreaker({
      failureThreshold: 5,
      resetTimeout: 30000,
    });
  }

  async complete(
    messages: Message[],
    options: CompletionOptions
  ): Promise<Completion> {
    return this.circuitBreaker.execute(async () => {
      const response = await this.client.messages.create({
        model: 'claude-sonnet-4-20250514',
        max_tokens: options.maxTokens ?? 1024,
        temperature: options.temperature ?? 0.7,
        messages: this.formatMessages(messages),
        tools: options.tools?.map(this.formatTool),
      });

      return this.parseResponse(response);
    });
  }

  async *stream(
    messages: Message[],
    options: StreamOptions
  ): AsyncGenerator<Token, Completion, void> {
    const stream = await this.client.messages.stream({
      model: 'claude-sonnet-4-20250514',
      max_tokens: options.maxTokens ?? 1024,
      temperature: options.temperature ?? 0.7,
      messages: this.formatMessages(messages),
    });

    let fullContent = '';
    let inputTokens = 0;
    let outputTokens = 0;

    for await (const event of stream) {
      if (event.type === 'content_block_delta') {
        const text = event.delta.text;
        fullContent += text;
        yield { type: 'text', text };
      } else if (event.type === 'message_delta') {
        outputTokens = event.usage?.output_tokens ?? 0;
      } else if (event.type === 'message_start') {
        inputTokens = event.message.usage?.input_tokens ?? 0;
      }
    }

    return {
      content: fullContent,
      finishReason: 'stop',
      usage: { inputTokens, outputTokens },
    };
  }

  private formatMessages(messages: Message[]): AnthropicMessage[] {
    return messages.map(m => ({
      role: m.role === 'user' ? 'user' : 'assistant',
      content: m.content,
    }));
  }
}
```

### Provider Registry with Failover

```typescript
// Multi-provider registry with automatic failover
class ProviderRegistry {
  private providers: Map<string, LLMProvider> = new Map();
  private primary: string;
  private fallbacks: string[];

  constructor(config: ProviderRegistryConfig) {
    this.primary = config.primary;
    this.fallbacks = config.fallbacks;
  }

  register(name: string, provider: LLMProvider): void {
    this.providers.set(name, provider);
  }

  async complete(
    messages: Message[],
    options: CompletionOptions
  ): Promise<Completion> {
    const providerOrder = [this.primary, ...this.fallbacks];

    for (const providerName of providerOrder) {
      const provider = this.providers.get(providerName);
      if (!provider) continue;

      try {
        // Check health before using
        if (await provider.isHealthy()) {
          const result = await provider.complete(messages, options);
          this.metrics.increment('provider.success', { provider: providerName });
          return result;
        }
      } catch (error) {
        this.logger.warn(`Provider ${providerName} failed`, { error });
        this.metrics.increment('provider.failure', { provider: providerName });
      }
    }

    throw new Error('All LLM providers unavailable');
  }

  async *stream(
    messages: Message[],
    options: StreamOptions
  ): AsyncGenerator<Token, Completion, void> {
    const provider = this.providers.get(this.primary);
    if (!provider) {
      throw new Error(`Primary provider ${this.primary} not found`);
    }

    // Streaming doesn't support automatic failover (would be disruptive)
    yield* provider.stream(messages, options);
  }
}
```

---

## Circuit Breaker

```typescript
// Circuit breaker for external service protection
class CircuitBreaker {
  private state: 'closed' | 'open' | 'half-open' = 'closed';
  private failures = 0;
  private lastFailureTime = 0;
  private successesSinceHalfOpen = 0;

  constructor(private config: CircuitBreakerConfig) {}

  async execute<T>(fn: () => Promise<T>): Promise<T> {
    if (this.state === 'open') {
      if (Date.now() - this.lastFailureTime > this.config.resetTimeout) {
        this.state = 'half-open';
        this.successesSinceHalfOpen = 0;
      } else {
        throw new CircuitBreakerOpenError();
      }
    }

    try {
      const result = await fn();
      this.onSuccess();
      return result;
    } catch (error) {
      this.onFailure();
      throw error;
    }
  }

  private onSuccess(): void {
    if (this.state === 'half-open') {
      this.successesSinceHalfOpen++;
      if (this.successesSinceHalfOpen >= this.config.successThreshold) {
        this.state = 'closed';
        this.failures = 0;
      }
    } else {
      this.failures = 0;
    }
  }

  private onFailure(): void {
    this.failures++;
    this.lastFailureTime = Date.now();

    if (this.failures >= this.config.failureThreshold) {
      this.state = 'open';
    }
  }

  getState(): CircuitBreakerState {
    return {
      state: this.state,
      failures: this.failures,
      lastFailureTime: this.lastFailureTime,
    };
  }
}

interface CircuitBreakerConfig {
  failureThreshold: number;   // Failures before opening
  successThreshold: number;   // Successes in half-open to close
  resetTimeout: number;       // ms before trying half-open
}
```

---

## Consequences

### Benefits

1. **Unified Interface**: All integrations exposed through consistent APIs
2. **Resilience**: Circuit breakers and retries prevent cascade failures
3. **Extensibility**: Easy to add new providers and integrations
4. **Observability**: Comprehensive metrics and logging
5. **Security**: Proper signature verification and authentication

### Trade-offs

| Benefit | Trade-off |
|---------|-----------|
| Abstraction | Some provider-specific features hidden |
| Circuit breaker | Delayed recovery after incidents |
| Retry logic | Potential duplicate processing |
| Async processing | Eventually consistent state |

---

## Related Decisions

- **ADR-001**: Architecture Overview
- **ADR-004**: Background Workers (webhook delivery)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-27 | RuVector Architecture Team | Initial version |
