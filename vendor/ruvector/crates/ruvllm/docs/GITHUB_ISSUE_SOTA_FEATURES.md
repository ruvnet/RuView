# feat(ruvllm): Implement SOTA features for production agentic workflows

**Labels**: `enhancement`, `p0-critical`, `agentic`, `v2.4`, `mistral-rs`, `performance`

---

## Summary

RuvLLM v2.4 SOTA Feature Implementation - Adding the 3 critical capabilities needed for production agentic workflows: **Structured Output**, **Function Calling**, and **Prefix Caching**.

These features are essential for modern LLM applications and are currently blocking production adoption for major agent frameworks.

---

## Motivation

### Why This Matters

**Current State:**
- RuvLLM cannot reliably generate structured outputs (JSON schema enforcement)
- No native function calling support for tool-using agents
- Repeated prompts/prefixes incur full generation costs (no caching)
- Agent frameworks (LangChain, LlamaIndex, CrewAI) cannot integrate

**Impact:**
- **Blocking production adoption** for agentic workflows
- **Cost inefficiency**: 10-100x slower for RAG/chat applications vs competitors
- **Reliability gap**: JSON parsing failures break agent loops
- **Missing compatibility**: Cannot replace vLLM, llama.cpp, SGLang in existing stacks

**Competitive Gap:**
| Feature | vLLM | llama.cpp | SGLang | RuvLLM |
|---------|------|-----------|--------|--------|
| Structured Output | ✅ | ✅ | ✅ | ❌ |
| Function Calling | ✅ | ✅ | ✅ | ❌ |
| Prefix Caching | ✅ | ✅ | ✅ | ❌ |

---

## Features

### 1. Structured Output / JSON Mode (P0)

**Objective**: Guarantee valid JSON output conforming to user-provided schemas.

#### Core Capabilities
- [ ] **JSON schema validation** (JSONSchema Draft 7 support)
  - Primitive types: `string`, `number`, `boolean`, `null`
  - Complex types: `object`, `array`
  - Nested schemas with `properties`, `items`, `required`
  - Constraints: `minLength`, `maxLength`, `pattern`, `enum`, `minimum`, `maximum`

- [ ] **Constrained decoding with logit bias**
  - State machine for tracking JSON structure (open braces, quotes, commas)
  - Token masking to enforce valid next tokens
  - Rejection sampling fallback for complex schemas

- [ ] **Bracket/brace state machine**
  - Track depth of `{}` and `[]`
  - Enforce closing brackets
  - Handle escaped quotes in strings

- [ ] **JSON repair for malformed output**
  - Auto-close unclosed braces/brackets
  - Fix trailing commas
  - Escape unescaped quotes
  - Best-effort recovery mode

- [ ] **GBNF grammar support (future)**
  - llama.cpp-compatible grammar format
  - Custom domain-specific languages

- [ ] **Comprehensive tests**
  - Unit tests for all JSON types
  - Property-based testing with Hypothesis/QuickCheck
  - Adversarial inputs (deeply nested, large arrays)

- [ ] **Benchmarks vs unconstrained**
  - Measure latency overhead (<10% target)
  - Throughput impact
  - Memory usage

#### Example API
```rust
let schema = json!({
    "type": "object",
    "properties": {
        "name": {"type": "string"},
        "age": {"type": "number", "minimum": 0},
        "tags": {"type": "array", "items": {"type": "string"}}
    },
    "required": ["name"]
});

let response = llm.generate(GenerateRequest {
    prompt: "Extract person info: John is 30",
    json_schema: Some(schema),
    strict: true, // Guarantee valid JSON
    ..Default::default()
})?;

// response.text is guaranteed valid JSON matching schema
```

#### Acceptance Criteria
- [ ] **100% valid JSON** when `strict: true` enabled
- [ ] **<10% latency overhead** vs unconstrained generation
- [ ] **Schema validation passes** for nested objects/arrays (depth ≥ 5)
- [ ] **Repair mode** recovers ≥95% of malformed outputs

---

### 2. Function Calling / Tool Use (P0)

**Objective**: Enable LLMs to call external tools/functions with structured arguments.

#### Core Capabilities
- [ ] **Tool definition schema**
  - Function name, description
  - Parameters (JSON schema)
  - Return type (optional)

- [ ] **ToolChoice enum**
  - `auto`: Model decides whether to call tools
  - `none`: Never call tools (text-only)
  - `required`: Must call at least one tool
  - `specific(name)`: Force specific tool

- [ ] **Parallel tool calls**
  - Generate multiple tool calls in one response
  - Dependency-aware ordering

- [ ] **Tool result handling**
  - Inject tool results back into conversation
  - Continue generation after tool execution
  - Multi-turn tool loops

- [ ] **Model-specific formats**
  - Llama 3.1 tool format (`<|python_tag|>`)
  - Mistral tool format (function tags)
  - Qwen tool format
  - Claude tool format

- [ ] **OpenAI API compatibility layer**
  - `tools` parameter
  - `tool_choice` parameter
  - `ChatCompletionToolCall` response format

- [ ] **LangChain integration tests**
  - Works with `AgentExecutor`
  - Compatible with `StructuredTool`
  - Multi-agent workflows

#### Example API
```rust
let tools = vec![
    Tool {
        name: "get_weather".into(),
        description: "Get current weather for a location".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "location": {"type": "string"},
                "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
            },
            "required": ["location"]
        }),
    },
    Tool {
        name: "search_web".into(),
        description: "Search the web".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "query": {"type": "string"}
            },
            "required": ["query"]
        }),
    }
];

let response = llm.chat(ChatRequest {
    messages: vec![
        Message::user("What's the weather in SF and latest AI news?")
    ],
    tools: Some(tools),
    tool_choice: ToolChoice::Auto,
    ..Default::default()
})?;

// response.tool_calls contains parallel calls:
// [get_weather(location="San Francisco"), search_web(query="AI news")]
```

#### Acceptance Criteria
- [ ] **OpenAI API format compatibility** (passes OpenAI SDK tests)
- [ ] **LangChain AgentExecutor** integration works end-to-end
- [ ] **Parallel tool calls** supported (≥3 concurrent)
- [ ] **Multi-turn tool conversations** (≥5 turns)
- [ ] **Tool call success rate** ≥95% for common tools

---

### 3. Prefix Caching (P0)

**Objective**: Cache and reuse KV cache for repeated prompt prefixes (system prompts, RAG documents).

#### Core Capabilities
- [ ] **Hash-based prefix lookup**
  - SHA-256 hash of token IDs
  - Fast O(1) cache hit detection

- [ ] **Radix tree implementation**
  - Efficient storage for overlapping prefixes
  - Longest common prefix matching
  - Memory-efficient sharing

- [ ] **KV cache copy-on-write**
  - Share read-only cache entries
  - Copy only on divergence
  - Zero-copy for cache hits

- [ ] **LRU eviction policy**
  - Evict least recently used prefixes
  - Configurable cache size
  - Per-model cache isolation

- [ ] **Memory limits**
  - Hard limit on cache size (bytes)
  - Soft limit with warning
  - Graceful degradation

- [ ] **Cache hit/miss metrics**
  - Prometheus metrics
  - Hit rate tracking
  - Memory usage stats

- [ ] **Chat prefix caching**
  - System prompt caching
  - Conversation history caching
  - Automatic prefix detection

- [ ] **RAG document caching**
  - Document chunk prefixes
  - Query-independent context
  - Multi-query reuse

#### Example API
```rust
// First request - cache miss
let response1 = llm.generate(GenerateRequest {
    prompt: "System: You are a helpful assistant.\nUser: Hello",
    cache_prefix: Some(CacheConfig {
        enable: true,
        key: Some("chat-system-prompt".into()),
        ttl_seconds: Some(3600),
    }),
    ..Default::default()
})?;
// Latency: 500ms

// Second request - cache hit (reuses "System: You are..." KV cache)
let response2 = llm.generate(GenerateRequest {
    prompt: "System: You are a helpful assistant.\nUser: How are you?",
    cache_prefix: Some(CacheConfig {
        enable: true,
        key: Some("chat-system-prompt".into()),
        ttl_seconds: Some(3600),
    }),
    ..Default::default()
})?;
// Latency: 50ms (10x faster!)
```

#### Performance Targets
- [ ] **10x speedup** for repeated system prompts (cache hit)
- [ ] **<5% overhead** for cache miss
- [ ] **Memory-bounded** (configurable, default 2GB)
- [ ] **Thread-safe** for concurrent requests
- [ ] **Hit rate ≥80%** for typical chat/RAG workloads

#### Acceptance Criteria
- [ ] **Speedup**: ≥10x for 1024-token prefix reuse
- [ ] **Memory**: Bounded by config, no OOM
- [ ] **Correctness**: Identical outputs for cached vs uncached
- [ ] **Concurrency**: No race conditions (stress tested)
- [ ] **Metrics**: Prometheus metrics exported

---

## Technical Design

### Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                     RuvLLM v2.4                         │
├─────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌──────────────┐  ┌────────────┐ │
│  │ Structured      │  │ Function     │  │ Prefix     │ │
│  │ Output Engine   │  │ Calling      │  │ Cache      │ │
│  │                 │  │ Router       │  │ Manager    │ │
│  │ - JSON Schema   │  │ - Tool Defs  │  │ - Radix    │ │
│  │ - Logit Bias    │  │ - ToolChoice │  │   Tree     │ │
│  │ - State Machine │  │ - Multi-call │  │ - LRU      │ │
│  └────────┬────────┘  └──────┬───────┘  └─────┬──────┘ │
│           │                  │                 │        │
│           └──────────────────┼─────────────────┘        │
│                              │                          │
│                    ┌─────────▼──────────┐               │
│                    │  mistral-rs Core   │               │
│                    │  - Model Loading   │               │
│                    │  - Token Sampling  │               │
│                    │  - KV Cache        │               │
│                    └────────────────────┘               │
└─────────────────────────────────────────────────────────┘
```

### Reference ADRs

- **ADR-009**: Structured Output Implementation
  - Constrained decoding algorithm
  - JSON schema validation approach
  - Performance optimization strategies

- **ADR-010**: Function Calling Architecture
  - Tool definition format
  - Multi-model compatibility layer
  - Parallel execution model

- **ADR-011**: Prefix Caching Design
  - Radix tree structure
  - Eviction policies
  - Memory management

- **ADR-008**: mistral-rs Integration
  - Dependency structure
  - API surface
  - Migration path

---

## Implementation Plan

### Phase 1: Foundation (Weeks 1-2)
**Focus**: Structured Output basics + Function Calling definitions

- [ ] Week 1: JSON schema parser and validator
  - Implement schema types (object, array, string, number, boolean, null)
  - Unit tests for all types
  - Property-based tests

- [ ] Week 2: Constrained decoding MVP
  - Logit bias implementation
  - Simple state machine (braces, brackets)
  - Integration with mistral-rs sampler
  - Basic function calling types (Tool, ToolChoice enums)

**Deliverable**: JSON mode works for simple schemas, tool definitions parsed

---

### Phase 2: Core Logic (Weeks 3-4)
**Focus**: Constrained decoding + Tool generation

- [ ] Week 3: Advanced constrained decoding
  - Nested schema support
  - String pattern matching
  - Enum constraints
  - JSON repair mode

- [ ] Week 4: Tool call generation
  - Llama 3.1 format support
  - Mistral format support
  - Parallel tool calls
  - OpenAI API compatibility layer

**Deliverable**: Complex JSON schemas work, tool calls generated in OpenAI format

---

### Phase 3: Caching + Polish (Weeks 5-6)
**Focus**: Prefix Caching + Integration tests

- [ ] Week 5: Prefix caching implementation
  - Radix tree structure
  - Hash-based lookup
  - LRU eviction
  - Thread-safety (RwLock)

- [ ] Week 6: Integration + benchmarks
  - LangChain integration tests
  - RAG workflow tests
  - Performance benchmarks
  - Documentation
  - Example applications

**Deliverable**: All 3 features production-ready, benchmarked, documented

---

## Testing Strategy

### Unit Tests
- JSON schema validation (all types, nested, constraints)
- Logit bias correctness
- Tool definition parsing
- Prefix cache hit/miss logic
- Radix tree operations

### Integration Tests
- LangChain AgentExecutor with tools
- LlamaIndex ReAct agent
- CrewAI multi-agent workflows
- OpenAI SDK compatibility tests

### Benchmarks
- Structured output latency vs unconstrained
- Tool calling accuracy (% correct tool selections)
- Prefix cache speedup (1x, 10x, 100x reuse)
- Memory usage under load

### Stress Tests
- 1000 concurrent requests with caching
- Deeply nested JSON schemas (depth 20)
- Large tool libraries (100+ tools)
- Multi-turn tool conversations (50+ turns)

---

## Success Metrics

### Structured Output
- [ ] **Validity**: 100% valid JSON when `strict: true`
- [ ] **Overhead**: <10% latency vs unconstrained
- [ ] **Schema compliance**: 100% for depth ≤10 schemas
- [ ] **Repair rate**: ≥95% successful repairs

### Function Calling
- [ ] **Compatibility**: Passes OpenAI SDK test suite
- [ ] **LangChain**: Works with AgentExecutor (5+ examples)
- [ ] **Accuracy**: ≥95% correct tool selection (benchmark dataset)
- [ ] **Parallel calls**: Supports ≥5 concurrent tools

### Prefix Caching
- [ ] **Speedup**: 10x for 1024-token prefix, 100x for 4096-token
- [ ] **Hit rate**: ≥80% for chat workloads
- [ ] **Memory**: Bounded, no OOM under stress
- [ ] **Correctness**: 100% identical outputs (cached vs uncached)

---

## Dependencies

### Upstream
- **mistral-rs v0.4.x** (ADR-008)
  - KV cache access for prefix caching
  - Token sampling hooks for logit bias
  - Model loading infrastructure

### Downstream
- **Enables**: Agentic workflow support
- **Enables**: LangChain/LlamaIndex/CrewAI integration
- **Blocks**: v2.4 release
- **Blocks**: Production adoption by agent frameworks

---

## Related Issues

- Depends on: #XXX (mistral-rs integration ADR-008)
- Enables: #XXX (Agentic workflow support)
- Enables: #XXX (LangChain integration)
- Blocks: #XXX (v2.4 release milestone)

---

## Documentation Requirements

- [ ] API reference docs (rustdoc)
- [ ] User guides for each feature
  - "How to use JSON mode"
  - "How to define tools"
  - "How to enable prefix caching"
- [ ] Migration guide from v2.3
- [ ] Example applications
  - Structured extraction (NER, info extraction)
  - Multi-tool agent (ReAct loop)
  - RAG with caching (chatbot)
- [ ] Performance tuning guide

---

## Open Questions

1. **JSON Schema**: Full Draft 7 or subset? (Propose: Core subset + extensions)
2. **Tool formats**: Support all models or Llama 3.1+ only? (Propose: Llama 3.1+ with adapters)
3. **Cache eviction**: LRU vs LFU vs TTL-based? (Propose: LRU + TTL)
4. **Memory limit**: Default cache size? (Propose: 2GB default, configurable)
5. **Breaking changes**: Any API changes needed? (Propose: Additive only, no breaks)

---

## Future Enhancements (Post-v2.4)

- **Structured Output**:
  - GBNF grammar support (custom DSLs)
  - Regex-constrained strings
  - Speculative decoding for constrained generation

- **Function Calling**:
  - Async/streaming tool execution
  - Tool result validation
  - Tool dependency graphs

- **Prefix Caching**:
  - Cross-request caching (shared cache pool)
  - Disk-backed cache (persist across restarts)
  - Distributed caching (Redis/memcached)

---

## Timeline Summary

| Phase | Duration | Focus | Deliverable |
|-------|----------|-------|-------------|
| 1 | Weeks 1-2 | Structured Output + Tool Definitions | JSON mode MVP, tool parsing |
| 2 | Weeks 3-4 | Constrained Decoding + Tool Generation | Complex schemas, tool calls |
| 3 | Weeks 5-6 | Prefix Caching + Integration | Production-ready, benchmarked |

**Total**: 6 weeks to production-ready v2.4

---

## Getting Involved

### For Contributors
- Pick a task from the checkboxes above
- Comment on this issue to claim a feature
- Follow the implementation plan phases
- Submit PRs with tests + benchmarks

### For Reviewers
- Focus on correctness (JSON validity, cache correctness)
- Performance regression checks (<10% overhead target)
- API design feedback (before Week 3)

### For Testers
- Test with real-world agent workflows
- Report edge cases and failure modes
- Benchmark on your hardware/models

---

**Let's close the gap with vLLM/llama.cpp and make RuvLLM the best choice for production agentic workflows!** 🚀
