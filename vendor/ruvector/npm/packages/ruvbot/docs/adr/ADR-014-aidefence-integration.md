# ADR-014: AIDefence Integration for Adversarial Protection

## Status
Accepted

## Date
2026-01-27

## Context

RuvBot requires robust protection against adversarial attacks including:
- Prompt injection (OWASP #1 LLM vulnerability)
- Jailbreak attempts
- PII leakage
- Malicious code injection
- Data exfiltration

The `aidefence` package provides production-ready adversarial defense with <10ms detection latency.

## Decision

Integrate `aidefence@2.1.1` into RuvBot as a core security layer.

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         RuvBot Security Layer                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  User Input ────┐                                                           │
│                 ▼                                                           │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                    AIDefenceGuard                                     │   │
│  ├──────────────────────────────────────────────────────────────────────┤   │
│  │  Layer 1: Pattern Detection (<5ms)                                   │   │
│  │    └─ 50+ injection signatures                                       │   │
│  │    └─ Jailbreak patterns (DAN, bypass, etc.)                        │   │
│  │    └─ Custom patterns (configurable)                                │   │
│  ├──────────────────────────────────────────────────────────────────────┤   │
│  │  Layer 2: PII Detection (<5ms)                                       │   │
│  │    └─ Email, phone, SSN, credit card                                │   │
│  │    └─ API keys and tokens                                           │   │
│  │    └─ IP addresses                                                   │   │
│  ├──────────────────────────────────────────────────────────────────────┤   │
│  │  Layer 3: Sanitization (<1ms)                                        │   │
│  │    └─ Control character removal                                      │   │
│  │    └─ Unicode homoglyph normalization                               │   │
│  │    └─ PII masking                                                    │   │
│  ├──────────────────────────────────────────────────────────────────────┤   │
│  │  Layer 4: Behavioral Analysis (<100ms) [Optional]                    │   │
│  │    └─ User behavior baseline                                        │   │
│  │    └─ Anomaly detection                                             │   │
│  │    └─ Deviation scoring                                             │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                 │                                                           │
│                 ▼                                                           │
│          ┌──────────┐                                                       │
│          │  Safe?   │────No───► Block / Sanitize                           │
│          └────┬─────┘                                                       │
│               │ Yes                                                         │
│               ▼                                                             │
│          LLM Provider                                                       │
│               │                                                             │
│               ▼                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │                Response Validation                                    │   │
│  │    └─ PII leak detection                                             │   │
│  │    └─ Injection echo detection                                       │   │
│  │    └─ Malicious code detection                                       │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
│                 │                                                           │
│                 ▼                                                           │
│          Safe Response ────► User                                           │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Threat Types Detected

| Threat Type | Severity | Detection Method | Response |
|-------------|----------|------------------|----------|
| Prompt Injection | High | Pattern matching | Block/Sanitize |
| Jailbreak | Critical | Signature detection | Block |
| PII Exposure | Medium-Critical | Regex patterns | Mask |
| Malicious Code | High | AST-like patterns | Block |
| Data Exfiltration | High | URL/webhook detection | Block |
| Control Characters | Medium | Unicode analysis | Remove |
| Encoding Attacks | Medium | Homoglyph detection | Normalize |
| Anomalous Behavior | Medium | Baseline deviation | Alert |

### Performance Targets

| Operation | Target | Achieved |
|-----------|--------|----------|
| Pattern Detection | <10ms | ~5ms |
| PII Detection | <10ms | ~3ms |
| Sanitization | <5ms | ~1ms |
| Full Analysis | <20ms | ~10ms |
| Response Validation | <15ms | ~8ms |

### Usage

```typescript
import { createAIDefenceGuard, createAIDefenceMiddleware } from '@ruvector/ruvbot';

// Simple usage
const guard = createAIDefenceGuard({
  detectPromptInjection: true,
  detectJailbreak: true,
  detectPII: true,
  blockThreshold: 'medium',
});

const result = await guard.analyze(userInput, {
  userId: 'user-123',
  sessionId: 'session-456',
});

if (!result.safe) {
  console.log('Threats detected:', result.threats);
  // Use sanitized input or block
  const safeInput = result.sanitizedInput;
}

// Middleware usage
const middleware = createAIDefenceMiddleware({
  blockThreshold: 'medium',
  enableAuditLog: true,
});

// Validate input before LLM
const { allowed, sanitizedInput } = await middleware.validateInput(userInput);

if (allowed) {
  const response = await llm.complete(sanitizedInput);

  // Validate response before returning
  const { allowed: responseAllowed } = await middleware.validateOutput(response, userInput);

  if (responseAllowed) {
    return response;
  }
}
```

### Configuration Options

```typescript
interface AIDefenceConfig {
  // Detection toggles
  detectPromptInjection: boolean;  // Default: true
  detectJailbreak: boolean;        // Default: true
  detectPII: boolean;              // Default: true

  // Advanced features
  enableBehavioralAnalysis: boolean;  // Default: false
  enablePolicyVerification: boolean;  // Default: false

  // Threshold: 'none' | 'low' | 'medium' | 'high' | 'critical'
  blockThreshold: ThreatLevel;     // Default: 'medium'

  // Custom patterns (regex strings)
  customPatterns?: string[];

  // Allowed domains for URL validation
  allowedDomains?: string[];

  // Max input length (chars)
  maxInputLength: number;          // Default: 100000

  // Audit logging
  enableAuditLog: boolean;         // Default: true
}
```

### Preset Configurations

```typescript
// Strict mode (production)
const strictConfig = createStrictConfig();
// - All detection enabled
// - Behavioral analysis enabled
// - Block threshold: 'low'

// Permissive mode (development)
const permissiveConfig = createPermissiveConfig();
// - Core detection only
// - Block threshold: 'critical'
// - Audit logging disabled
```

## Consequences

### Positive
- Sub-10ms detection latency
- 50+ built-in injection patterns
- PII protection out of the box
- Configurable security levels
- Audit logging for compliance
- Response validation
- Unicode/homoglyph protection

### Negative
- Additional dependency (aidefence)
- Small latency overhead (~10ms per request)
- False positives possible with strict settings

### Trade-offs
- Strict mode may block legitimate queries
- Behavioral analysis adds latency (~100ms)
- PII masking may alter valid content

## Integration with Existing Security

AIDefence integrates with RuvBot's 6-layer security architecture:

```
Layer 1: Transport (TLS 1.3)
Layer 2: Authentication (JWT)
Layer 3: Authorization (RBAC)
Layer 4: Data Protection (Encryption)
Layer 5: Input Validation (AIDefence) ◄── NEW
Layer 6: WASM Sandbox
```

## Dependencies

```json
{
  "aidefence": "^2.1.1"
}
```

The aidefence package includes:
- agentdb (vector storage)
- lean-agentic (formal verification)
- zod (schema validation)
- winston (logging)
- helmet (HTTP security headers)

## References

- [aidefence on npm](https://www.npmjs.com/package/aidefence)
- [OWASP LLM Top 10](https://owasp.org/www-project-top-10-for-large-language-model-applications/)
- [Prompt Injection Guide](https://www.lakera.ai/blog/guide-to-prompt-injection)
- [AIMDS Documentation](https://ruv.io/aimds)
