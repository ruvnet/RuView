# ADR-010: Function Calling / Tool Use in RuvLLM

**Status:** Proposed
**Date:** 2026-01-20
**Decision Makers:** Ruvector Architecture Team
**Technical Area:** LLM Capabilities / Agent Framework Integration

---

## Context and Problem Statement

RuvLLM currently provides text generation capabilities but lacks structured function calling (tool use) support, which is essential for integration with modern agent frameworks like LangChain, LlamaIndex, CrewAI, and AutoGPT. Function calling enables models to interact with external tools, APIs, and databases in a structured, type-safe manner.

### Current State

RuvLLM's generation API is limited to:
- Text-in, text-out generation
- No structured output parsing
- No tool/function definition support
- Manual prompt engineering required for tool interactions
- No support for multi-turn tool conversations

### Key Challenges

1. **Agent Framework Integration**: Popular frameworks expect OpenAI-compatible function calling APIs
2. **Structured Outputs**: Models need to generate valid JSON function calls, not freeform text
3. **Multi-Turn Conversations**: Tool results must be fed back to the model for reasoning
4. **Parallel Tool Calls**: Efficient agents need to call multiple tools simultaneously
5. **Model Format Compatibility**: Different models (Llama, Mistral, Qwen) use different tool calling formats

---

## Decision Drivers

### Functional Requirements
- **Tool Definitions**: JSON Schema-based function signatures
- **Tool Choice Control**: Auto, none, required, or specific function selection
- **Parallel Calls**: Multiple function calls in a single response
- **Result Integration**: Feeding tool outputs back to the model
- **Type Safety**: Validate function arguments against schemas

### Compatibility Requirements
- **OpenAI API Compatible**: Drop-in replacement for OpenAI function calling
- **Anthropic Tool Use**: Map to Anthropic's tool_use format
- **Framework Integration**: Direct support for LangChain, LlamaIndex, CrewAI
- **Model Agnostic**: Work across Llama 3.1+, Mistral, Qwen, custom models

### Performance Requirements
- **Constrained Generation**: Force valid JSON output via logit biasing
- **Low Latency**: <10ms overhead for tool call parsing
- **Streaming Support**: Stream tool calls as they're generated
- **Batching**: Process multiple tool calls efficiently

---

## Considered Options

### Option A: Prompt Engineering Only

Use structured prompts to request tool calls in JSON format, parse with regex/JSON parsers.

**Pros:**
- No core changes to generation logic
- Works with any model
- Simple implementation

**Cons:**
- Unreliable: models may generate invalid JSON
- No type safety guarantees
- Poor support for parallel tool calls
- Requires extensive prompt tuning per model

### Option B: Constrained Generation with Grammar

Implement constrained decoding using formal grammars (GBNF, JSON Schema) to force valid tool calls.

**Pros:**
- Guarantees valid JSON output
- Type-safe by construction
- Works across model architectures
- Best reliability for production

**Cons:**
- Complex implementation (logit masking)
- Requires grammar compiler
- Potential performance overhead

### Option C: Model-Specific Chat Templates

Leverage each model family's native tool calling format via chat templates.

**Pros:**
- Optimal for models with native tool support (Llama 3.1+, Mistral)
- Minimal overhead
- Leverages model training

**Cons:**
- Fragmented implementation across models
- No support for models without native tool calling
- Template maintenance burden

---

## Decision Outcome

**Chosen Option: Hybrid Approach - Option B (Constrained Generation) + Option C (Chat Templates)**

Implement constrained generation with grammar-based validation as the foundation, with chat template optimizations for models with native tool calling support.

### Rationale

1. **Reliability First**: Constrained generation guarantees valid outputs for critical production use cases
2. **Performance Optimization**: Chat templates optimize for models with native support (Llama 3.1+, Mistral)
3. **Universal Compatibility**: Fallback to constrained generation for any model
4. **Future-Proof**: New models can be added via chat templates without core changes

---

## Technical Specifications

### Tool Definition Schema

```rust
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Tool/function definition for function calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Function name (must be valid identifier)
    pub name: String,

    /// Human-readable description for the model
    pub description: String,

    /// JSON Schema for function parameters
    pub parameters: JsonSchema,

    /// Required parameter names
    #[serde(default)]
    pub required: Vec<String>,
}

/// JSON Schema representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchema {
    #[serde(rename = "type")]
    pub schema_type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<std::collections::HashMap<String, JsonSchema>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<JsonSchema>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
}

/// Tool choice mode for generation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    /// Model decides whether to call tools
    Auto,

    /// Model must not call any tools
    None,

    /// Model must call at least one tool
    Required,

    /// Model must call this specific function
    Specific(String),
}
```

### Tool Call Request and Response

```rust
/// Request with tool calling support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    /// User message/prompt
    pub messages: Vec<ChatMessage>,

    /// Available tools/functions
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,

    /// Tool choice mode
    #[serde(default)]
    pub tool_choice: ToolChoice,

    /// Enable parallel tool calls (default: true)
    #[serde(default = "default_true")]
    pub parallel_tool_calls: bool,

    /// Standard generation parameters
    #[serde(flatten)]
    pub params: GenerateParams,
}

/// Tool call in model response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this tool call
    pub id: String,

    /// Type (always "function" for now)
    #[serde(rename = "type")]
    pub call_type: String,

    /// Function call details
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// Function name (must match a tool definition)
    pub name: String,

    /// JSON-encoded function arguments
    pub arguments: serde_json::Value,
}

/// Chat message with tool call support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role: system, user, assistant, tool
    pub role: String,

    /// Text content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Tool calls (for assistant messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,

    /// Tool call ID (for tool result messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

fn default_true() -> bool { true }
```

### Chat Template Integration

Different models require different formatting for tool calling:

```rust
/// Chat template for tool calling
pub trait ToolCallingTemplate {
    /// Format messages with tool definitions
    fn format_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        tool_choice: &ToolChoice,
    ) -> Result<String>;

    /// Parse tool calls from model output
    fn parse_tool_calls(&self, output: &str) -> Result<Vec<ToolCall>>;

    /// Check if model has native tool calling support
    fn has_native_support(&self) -> bool;
}

/// Llama 3.1+ tool calling format
pub struct Llama31ToolTemplate;

impl ToolCallingTemplate for Llama31ToolTemplate {
    fn format_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        tool_choice: &ToolChoice,
    ) -> Result<String> {
        // Llama 3.1 uses special <|python_tag|> tokens for tools
        let mut prompt = String::new();

        // Add tool definitions
        prompt.push_str("<|start_header_id|>system<|end_header_id|>\n\n");
        prompt.push_str("Available tools:\n");
        for tool in tools {
            prompt.push_str(&format!(
                "<|python_tag|>{}<|eom_id|>\n",
                serde_json::to_string_pretty(tool)?
            ));
        }

        // Add conversation history
        for msg in messages {
            prompt.push_str(&format!(
                "<|start_header_id|>{}<|end_header_id|>\n\n{}<|eom_id|>\n",
                msg.role,
                msg.content.as_deref().unwrap_or("")
            ));
        }

        // Start assistant response
        prompt.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");

        Ok(prompt)
    }

    fn parse_tool_calls(&self, output: &str) -> Result<Vec<ToolCall>> {
        // Parse <|python_tag|>{"name": "...", "arguments": {...}}<|eom_id|>
        // Implementation details omitted for brevity
        todo!("Parse Llama 3.1 tool call format")
    }

    fn has_native_support(&self) -> bool { true }
}

/// Mistral tool calling format
pub struct MistralToolTemplate;

impl ToolCallingTemplate for MistralToolTemplate {
    fn format_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        tool_choice: &ToolChoice,
    ) -> Result<String> {
        // Mistral uses [AVAILABLE_TOOLS] and [/AVAILABLE_TOOLS] markers
        let mut prompt = String::new();

        prompt.push_str("[AVAILABLE_TOOLS]\n");
        prompt.push_str(&serde_json::to_string(tools)?);
        prompt.push_str("\n[/AVAILABLE_TOOLS]\n\n");

        // Add conversation
        for msg in messages {
            prompt.push_str(&format!("[INST] {} [/INST]\n", msg.content.as_deref().unwrap_or("")));
        }

        Ok(prompt)
    }

    fn parse_tool_calls(&self, output: &str) -> Result<Vec<ToolCall>> {
        // Parse [TOOL_CALLS] ... [/TOOL_CALLS]
        todo!("Parse Mistral tool call format")
    }

    fn has_native_support(&self) -> bool { true }
}

/// Qwen tool calling format
pub struct QwenToolTemplate;

/// Generic XML-based format for models without native support
pub struct GenericXmlToolTemplate;

impl ToolCallingTemplate for GenericXmlToolTemplate {
    fn format_with_tools(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        tool_choice: &ToolChoice,
    ) -> Result<String> {
        // Generic format using XML tags
        let mut prompt = String::from(
            "You have access to the following tools. To use a tool, respond with:\n\
             <tool_call>\n\
             <name>function_name</name>\n\
             <arguments>{\"arg1\": \"value1\"}</arguments>\n\
             </tool_call>\n\n"
        );

        prompt.push_str("Available tools:\n");
        for tool in tools {
            prompt.push_str(&format!("- {}: {}\n", tool.name, tool.description));
            prompt.push_str(&format!("  Parameters: {}\n",
                serde_json::to_string(&tool.parameters)?));
        }
        prompt.push_str("\n");

        // Add conversation
        for msg in messages {
            prompt.push_str(&format!("{}: {}\n", msg.role, msg.content.as_deref().unwrap_or("")));
        }

        Ok(prompt)
    }

    fn parse_tool_calls(&self, output: &str) -> Result<Vec<ToolCall>> {
        // Parse <tool_call>...</tool_call> blocks
        use regex::Regex;

        let re = Regex::new(
            r"<tool_call>\s*<name>([^<]+)</name>\s*<arguments>([^<]+)</arguments>\s*</tool_call>"
        )?;

        let mut calls = Vec::new();
        for cap in re.captures_iter(output) {
            calls.push(ToolCall {
                id: uuid::Uuid::new_v4().to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: cap[1].to_string(),
                    arguments: serde_json::from_str(&cap[2])?,
                },
            });
        }

        Ok(calls)
    }

    fn has_native_support(&self) -> bool { false }
}
```

### Constrained Generation Engine

For guaranteed valid JSON output, implement constrained decoding:

```rust
use serde_json::Value as JsonValue;

/// Constrained generation for tool calls
pub struct ConstrainedToolGenerator {
    /// JSON Schema grammar compiler
    grammar_compiler: GrammarCompiler,

    /// Logit processor for constraint enforcement
    logit_processor: LogitProcessor,
}

impl ConstrainedToolGenerator {
    /// Generate tool calls with grammar constraints
    pub fn generate_tool_calls(
        &self,
        model: &LlmBackend,
        prompt: &str,
        tools: &[ToolDefinition],
        params: GenerateParams,
    ) -> Result<Vec<ToolCall>> {
        // Compile JSON Schema to GBNF grammar
        let grammar = self.compile_tool_grammar(tools)?;

        // Generate with logit masking to enforce grammar
        let output = model.generate_constrained(prompt, &grammar, params)?;

        // Parse guaranteed-valid JSON
        let calls: Vec<ToolCall> = serde_json::from_str(&output)?;

        Ok(calls)
    }

    /// Compile JSON Schema into GBNF grammar
    fn compile_tool_grammar(&self, tools: &[ToolDefinition]) -> Result<Grammar> {
        // Build grammar that only allows valid tool calls
        // Example: tool_call ::= "{" ws "\"name\"" ws ":" ws name ws "," ws "\"arguments\"" ws ":" ws arguments ws "}"
        // name ::= "\"tool1\"" | "\"tool2\"" | ...
        // arguments ::= { schema-specific grammar }

        self.grammar_compiler.compile_tool_schema(tools)
    }
}

/// GBNF (GGML BNF) grammar for constrained generation
#[derive(Debug, Clone)]
pub struct Grammar {
    /// Grammar rules in GBNF format
    pub rules: String,
}

/// Logit processor for grammar enforcement
pub struct LogitProcessor {
    /// Current parse state
    state: ParseState,
}

impl LogitProcessor {
    /// Mask logits to only allow valid next tokens
    pub fn process_logits(
        &mut self,
        logits: &mut [f32],
        grammar: &Grammar,
        tokenizer: &Tokenizer,
    ) -> Result<()> {
        // Get valid next tokens from grammar state
        let valid_tokens = self.state.get_valid_next_tokens(grammar)?;

        // Mask out invalid tokens (set logit to -inf)
        for (token_id, logit) in logits.iter_mut().enumerate() {
            if !valid_tokens.contains(&(token_id as u32)) {
                *logit = f32::NEG_INFINITY;
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
struct ParseState {
    /// Current position in grammar
    position: usize,

    /// Parse stack for nested structures
    stack: Vec<String>,
}
```

### Multi-Turn Tool Conversations

Support iterative tool use:

```rust
/// Multi-turn conversation with tool calls
pub struct ToolConversation {
    /// Conversation history
    messages: Vec<ChatMessage>,

    /// Available tools
    tools: Vec<ToolDefinition>,

    /// Backend for generation
    backend: Box<dyn LlmBackend>,
}

impl ToolConversation {
    /// Add user message and generate response (may include tool calls)
    pub fn send_message(&mut self, content: &str) -> Result<ConversationTurn> {
        // Add user message
        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: None,
        });

        // Generate response with tool calls
        let request = ToolCallRequest {
            messages: self.messages.clone(),
            tools: self.tools.clone(),
            tool_choice: ToolChoice::Auto,
            parallel_tool_calls: true,
            params: GenerateParams::default(),
        };

        let response = self.backend.generate_with_tools(request)?;

        // Add assistant response to history
        self.messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: response.content.clone(),
            tool_calls: response.tool_calls.clone(),
            tool_call_id: None,
        });

        Ok(ConversationTurn {
            content: response.content,
            tool_calls: response.tool_calls,
        })
    }

    /// Submit tool results and continue conversation
    pub fn submit_tool_results(&mut self, results: Vec<ToolResult>) -> Result<ConversationTurn> {
        // Add tool result messages
        for result in results {
            self.messages.push(ChatMessage {
                role: "tool".to_string(),
                content: Some(result.output),
                tool_calls: None,
                tool_call_id: Some(result.tool_call_id),
            });
        }

        // Generate next response
        self.send_message("")
    }
}

#[derive(Debug, Clone)]
pub struct ConversationTurn {
    /// Text content
    pub content: Option<String>,

    /// Tool calls (if any)
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    /// Tool call ID this result corresponds to
    pub tool_call_id: String,

    /// Tool output (JSON or text)
    pub output: String,
}
```

---

## Implementation Plan

### Phase 1: Core Infrastructure (Week 1-2)

1. **Define Tool Schema Types**
   - Implement `ToolDefinition`, `ToolCall`, `ToolChoice` types
   - Add JSON Schema validation
   - Create builder APIs for ergonomic tool definitions

2. **Chat Template Integration**
   - Implement `ToolCallingTemplate` trait
   - Add Llama 3.1, Mistral, Qwen templates
   - Create generic XML fallback template

3. **Request/Response API**
   - Extend `LlmBackend` with `generate_with_tools` method
   - Add tool call parsing logic
   - Implement OpenAI-compatible API surface

**Deliverables:**
```rust
// User-facing API
let tools = vec![
    ToolDefinition::new("get_weather")
        .description("Get current weather for a location")
        .parameter("location", JsonSchema::string())
        .parameter("units", JsonSchema::enum_values(&["celsius", "fahrenheit"]))
        .required(&["location"])
];

let request = ToolCallRequest {
    messages: vec![
        ChatMessage::user("What's the weather in San Francisco?")
    ],
    tools,
    tool_choice: ToolChoice::Auto,
    parallel_tool_calls: true,
    params: GenerateParams::default(),
};

let response = backend.generate_with_tools(request)?;
for call in response.tool_calls.unwrap_or_default() {
    println!("Tool: {}, Args: {}", call.function.name, call.function.arguments);
}
```

### Phase 2: Constrained Generation (Week 3-4)

1. **Grammar Compiler**
   - Implement JSON Schema to GBNF compiler
   - Support nested objects, arrays, enums
   - Add grammar caching for performance

2. **Logit Processor**
   - Implement parse state machine
   - Add logit masking for valid tokens
   - Optimize for streaming generation

3. **Integration**
   - Wire constrained generation to `LlmBackend`
   - Add fallback logic (native template → constrained generation)
   - Benchmark performance impact

**Deliverables:**
```rust
// Constrained generation ensures valid JSON
let generator = ConstrainedToolGenerator::new();
let calls = generator.generate_tool_calls(
    &backend,
    &prompt,
    &tools,
    params,
)?;

// Guaranteed to parse successfully
assert!(calls.iter().all(|c| tools.iter().any(|t| t.name == c.function.name)));
```

### Phase 3: Multi-Turn Conversations (Week 5-6)

1. **Conversation Manager**
   - Implement `ToolConversation` for stateful interactions
   - Add automatic tool result integration
   - Support parallel tool call orchestration

2. **Agent Framework Integration**
   - LangChain adapter
   - LlamaIndex integration
   - CrewAI support

3. **Examples and Documentation**
   - Multi-turn conversation examples
   - Agent framework integration guides
   - Performance tuning documentation

**Deliverables:**
```rust
// Multi-turn conversation with tool use
let mut conv = ToolConversation::new(backend, tools);

let turn1 = conv.send_message("Book a flight to NYC")?;
// Model calls search_flights(destination="NYC")

let results = vec![ToolResult {
    tool_call_id: turn1.tool_calls[0].id.clone(),
    output: r#"{"flights": [{"price": 250, "time": "10am"}]}"#.to_string(),
}];

let turn2 = conv.submit_tool_results(results)?;
// Model responds with flight options
```

---

## Compatibility Matrix

### API Compatibility

| API Style | RuvLLM Support | Notes |
|-----------|----------------|-------|
| OpenAI Function Calling | ✅ Full | Drop-in replacement for `functions` and `tools` parameters |
| Anthropic Tool Use | ✅ Full | Map `tool_use` blocks to OpenAI format |
| LangChain Tools | ✅ Full | Direct integration via `BaseTool` adapter |
| LlamaIndex Tools | ✅ Full | Implement `BaseToolSpec` interface |
| CrewAI Tools | ✅ Full | Compatible with `Tool` decorator |

### Model Support

| Model Family | Native Support | Template | Constrained Fallback |
|--------------|----------------|----------|----------------------|
| Llama 3.1+ | ✅ Yes | Llama31ToolTemplate | ✅ |
| Llama 3.0 and earlier | ❌ No | GenericXmlToolTemplate | ✅ |
| Mistral 7B+ | ✅ Yes | MistralToolTemplate | ✅ |
| Qwen 2.5+ | ✅ Yes | QwenToolTemplate | ✅ |
| CodeLlama | ❌ No | GenericXmlToolTemplate | ✅ |
| Custom Models | ❌ No | GenericXmlToolTemplate | ✅ |

### Framework Integration

```rust
// LangChain integration example
use langchain_rs::{Tool, ToolInput, ToolOutput};

struct RuvLlmTool {
    definition: ToolDefinition,
    executor: Box<dyn Fn(JsonValue) -> Result<String>>,
}

impl Tool for RuvLlmTool {
    fn name(&self) -> &str {
        &self.definition.name
    }

    fn description(&self) -> &str {
        &self.definition.description
    }

    fn run(&self, input: ToolInput) -> Result<ToolOutput> {
        let args = serde_json::to_value(input)?;
        let output = (self.executor)(args)?;
        Ok(ToolOutput::Text(output))
    }
}
```

---

## Performance Characteristics

### Latency Overhead

| Component | Latency | Notes |
|-----------|---------|-------|
| Tool schema compilation | <1ms | Cached after first use |
| Grammar compilation | 5-10ms | Cached per tool set |
| Logit processing (per token) | <0.1ms | Minimal impact on generation |
| JSON parsing | <1ms | Standard serde_json |
| **Total overhead** | **<10ms** | Amortized across conversation |

### Memory Overhead

| Component | Memory | Notes |
|-----------|--------|-------|
| Tool definitions | ~1KB per tool | Scales with number of tools |
| Grammar cache | ~10KB per tool set | One-time cost |
| Parse state | ~1KB per request | Freed after generation |
| **Total overhead** | **~10KB + 1KB/tool** | Negligible for typical use |

### Throughput Comparison

| Method | Tools/sec | Reliability | Use Case |
|--------|-----------|-------------|----------|
| Prompt engineering only | 1000+ | 70-80% | Development/testing |
| Chat template (native) | 800-1000 | 90-95% | Production (supported models) |
| Constrained generation | 200-500 | 99.9%+ | Production (all models), critical systems |

---

## Consequences

### Positive Consequences

1. **Agent Framework Integration**: Direct compatibility with LangChain, LlamaIndex, CrewAI enables rich agent ecosystems
2. **Type Safety**: JSON Schema validation prevents invalid tool calls at generation time
3. **Reliability**: Constrained generation guarantees valid outputs for production systems
4. **OpenAI Compatibility**: Drop-in replacement for OpenAI API reduces migration friction
5. **Multi-Modal Agents**: Foundation for RAG, web search, database access, API integration
6. **Parallel Execution**: Multiple tool calls enable efficient multi-step reasoning

### Negative Consequences

1. **Complexity**: Grammar compilation and constrained generation add implementation complexity
2. **Performance Impact**: Logit processing adds 5-10% latency for constrained generation
3. **Model Requirements**: Best performance requires models with native tool calling support
4. **Testing Burden**: Must validate across multiple model families and templates

### Neutral Consequences

1. **Template Maintenance**: Each new model family may require new chat template
2. **Schema Limitations**: Complex schemas (recursive types, unions) may be challenging to constrain
3. **Backward Compatibility**: Existing text generation API unchanged, tool calling is additive

### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Invalid JSON output | Constrained generation with grammar enforcement |
| Template incompatibility | Generic XML fallback for unsupported models |
| Performance regression | Benchmark suite, caching, optional constrained mode |
| Schema complexity | Comprehensive test suite with edge cases |
| Framework API changes | Version pinning, adapter pattern for isolation |

---

## Alternatives Considered

### Text Parsing Only (Rejected)

Use prompt engineering with regex/JSON parsing.

- **Rejected**: Unreliable for production; 20-30% failure rate for complex schemas
- **Consideration**: Useful for prototyping and development

### Python Backend (vLLM, Outlines) (Rejected)

Integrate vLLM or Outlines Python libraries via FFI.

- **Rejected**: Cross-language complexity, deployment burden, latency overhead
- **Consideration**: Reference implementation for grammar compilation logic

### Custom DSL for Tool Definitions (Rejected)

Create a Rust macro-based DSL for tool definitions.

- **Rejected**: JSON Schema is industry standard, better tooling support
- **Consideration**: Could add as syntactic sugar on top of JSON Schema

---

## Related Decisions

- **ADR-002**: RuvLLM Integration with Ruvector (foundation for tool-enhanced RAG)
- **ADR-008**: mistral-rs Integration (backend for high-performance tool calling)
- **ADR-009**: Streaming Architecture (streaming tool calls in progress)

---

## References

1. **OpenAI Function Calling**: https://platform.openai.com/docs/guides/function-calling
   - Industry-standard API for tool use
   - `functions` parameter (deprecated) and `tools` parameter
   - Parallel tool calls and tool choice modes

2. **Anthropic Tool Use**: https://docs.anthropic.com/claude/docs/tool-use
   - Alternative API design with `tool_use` blocks
   - Computer use (bash, editor) as specialized tools
   - Multi-step tool orchestration patterns

3. **LangChain Tool Documentation**: https://python.langchain.com/docs/modules/agents/tools/
   - Agent framework integration patterns
   - `BaseTool` interface and tool decorators
   - Tool result schemas

4. **LlamaIndex Tools**: https://docs.llamaindex.ai/en/stable/module_guides/deploying/agents/tools/
   - `BaseToolSpec` interface
   - Function tools and query engine tools

5. **Constrained Decoding**:
   - GBNF (GGML BNF) grammar: https://github.com/ggerganov/llama.cpp/blob/master/grammars/README.md
   - Outlines (Python): https://github.com/outlines-dev/outlines
   - Guidance (Microsoft): https://github.com/guidance-ai/guidance

6. **Model-Specific Tool Formats**:
   - Llama 3.1 tool use: https://www.llama.com/docs/model-cards-and-prompt-formats/llama3_1
   - Mistral function calling: https://docs.mistral.ai/capabilities/function_calling/
   - Qwen tools: https://qwen.readthedocs.io/en/latest/framework/function_call.html

---

## Implementation Status

| Component | Status | Notes |
|-----------|--------|-------|
| Tool schema types | Pending | Define `ToolDefinition`, `ToolCall`, `ToolChoice` |
| JSON Schema validation | Pending | Integrate `schemars` crate |
| Chat templates | Pending | Llama 3.1, Mistral, Qwen, Generic XML |
| Request/Response API | Pending | `generate_with_tools` method on `LlmBackend` |
| Grammar compiler | Pending | JSON Schema → GBNF compiler |
| Logit processor | Pending | Parse state machine and masking logic |
| Constrained generation | Pending | Integration with backend |
| Multi-turn conversations | Pending | `ToolConversation` manager |
| LangChain integration | Pending | `BaseTool` adapter |
| LlamaIndex integration | Pending | `BaseToolSpec` implementation |
| CrewAI support | Pending | Tool decorator compatibility |
| OpenAI API compatibility | Pending | `/v1/chat/completions` endpoint |
| Anthropic format mapping | Pending | `tool_use` block conversion |
| Streaming tool calls | Pending | Stream partial JSON as generated |
| Parallel tool execution | Pending | Concurrent tool call orchestration |
| Documentation | Pending | API docs, examples, integration guides |

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-20 | Ruvector Architecture Team | Initial proposal |
