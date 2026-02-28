# ADR-008: Security Architecture

## Status
Accepted

## Date
2026-01-27

## Context

RuvBot handles sensitive data including:
- User conversations and personal information
- API credentials for LLM providers
- Integration tokens (Slack, Discord)
- Vector embeddings that may encode sensitive content
- Multi-tenant data requiring strict isolation

## Decision

### Security Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                     Security Architecture                        │
├─────────────────────────────────────────────────────────────────┤
│ Layer 1: Transport Security                                     │
│   - TLS 1.3 for all connections                                 │
│   - Certificate pinning for external APIs                       │
│   - HSTS enabled by default                                     │
├─────────────────────────────────────────────────────────────────┤
│ Layer 2: Authentication                                         │
│   - JWT tokens with RS256 signing                               │
│   - OAuth 2.0 for Slack/Discord                                 │
│   - API key authentication with rate limiting                   │
│   - Session tokens with secure rotation                         │
├─────────────────────────────────────────────────────────────────┤
│ Layer 3: Authorization                                          │
│   - RBAC with claims-based permissions                          │
│   - Tenant isolation at all layers                              │
│   - Skill-level permission grants                               │
│   - Resource-based access control                               │
├─────────────────────────────────────────────────────────────────┤
│ Layer 4: Data Protection                                        │
│   - AES-256-GCM for data at rest                               │
│   - Field-level encryption for sensitive data                   │
│   - Key rotation with envelope encryption                       │
│   - Secure secret management                                    │
├─────────────────────────────────────────────────────────────────┤
│ Layer 5: Input Validation                                       │
│   - Zod schema validation for all inputs                        │
│   - SQL injection prevention (parameterized queries)            │
│   - XSS prevention (content sanitization)                       │
│   - Path traversal prevention                                   │
├─────────────────────────────────────────────────────────────────┤
│ Layer 6: WASM Sandbox                                           │
│   - Memory isolation per operation                              │
│   - Resource limits (CPU, memory)                               │
│   - No filesystem access from WASM                              │
│   - Controlled imports/exports                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Multi-Tenancy Security

```sql
-- PostgreSQL Row-Level Security
CREATE POLICY tenant_isolation ON memories
  USING (tenant_id = current_setting('app.current_tenant')::uuid);

CREATE POLICY tenant_isolation ON sessions
  USING (tenant_id = current_setting('app.current_tenant')::uuid);

CREATE POLICY tenant_isolation ON agents
  USING (tenant_id = current_setting('app.current_tenant')::uuid);
```

### Secret Management

```typescript
// Secrets are never logged or exposed
interface SecretStore {
  get(key: string): Promise<string>;
  set(key: string, value: string, options?: SecretOptions): Promise<void>;
  rotate(key: string): Promise<void>;
  delete(key: string): Promise<void>;
}

// Environment variable validation
const requiredSecrets = z.object({
  ANTHROPIC_API_KEY: z.string().startsWith('sk-ant-'),
  SLACK_BOT_TOKEN: z.string().startsWith('xoxb-').optional(),
  DATABASE_URL: z.string().url().optional(),
});
```

### API Security

1. **Rate Limiting**: Per-tenant, per-endpoint limits
2. **Request Signing**: HMAC-SHA256 for webhooks
3. **IP Allowlisting**: Optional for enterprise
4. **Audit Logging**: All security events logged

### Vulnerability Prevention

| CVE Category | Prevention |
|--------------|------------|
| Injection (SQL, NoSQL, Command) | Parameterized queries, input validation |
| XSS | Content-Security-Policy, output encoding |
| CSRF | SameSite cookies, origin validation |
| SSRF | URL allowlisting, no user-controlled URLs |
| Path Traversal | Path sanitization, chroot for file ops |
| Sensitive Data Exposure | Encryption, minimal logging |
| Broken Authentication | Secure session management |
| Security Misconfiguration | Secure defaults, hardening guide |

### Compliance Readiness

- **GDPR**: Data export, deletion, consent tracking
- **SOC 2**: Audit logging, access controls
- **HIPAA**: Encryption, access logging (with configuration)

## Consequences

### Positive
- Defense in depth provides multiple security layers
- Multi-tenancy isolation prevents data leakage
- Comprehensive input validation blocks injection attacks
- WASM sandbox limits damage from malicious code

### Negative
- Performance overhead from encryption/validation
- Complexity in secret management
- Additional testing required for security features

### Risks
- Key management complexity
- Potential for misconfiguration
- Balance between security and usability

## Security Checklist

- [ ] TLS configured for all endpoints
- [ ] API keys stored in secure vault
- [ ] Rate limiting enabled
- [ ] Audit logging configured
- [ ] Input validation on all endpoints
- [ ] SQL injection tests passing
- [ ] XSS tests passing
- [ ] CSRF protection enabled
- [ ] Security headers configured
- [ ] Dependency vulnerabilities scanned
