# ADR-009: Structured Output / JSON Mode for Reliable Agentic Workflows

**Status:** Proposed
**Date:** 2026-01-20
**Decision Makers:** Ruvector Architecture Team
**Technical Area:** LLM Generation / Structured Output

---

## Context and Problem Statement

RuvLLM v2.3 provides robust text generation capabilities but lacks structured output enforcement, which is critical for production agentic workflows. Modern frameworks (LangChain, CrewAI, Claude Flow, AutoGen) rely on LLMs producing valid JSON for tool use, function calling, and structured data extraction. Without JSON mode support, RuvLLM cannot reliably power these workflows.

### Current State

RuvLLM's existing `generate` interface returns unstructured text:

```rust
pub trait LlmBackend {
    fn generate(&self, prompt: &str, params: GenerateParams) -> Result<String>;
    fn generate_stream(&self, prompt: &str, params: GenerateParams) -> impl Stream<Item = String>;
}
```

Users requesting JSON output face:
- **Malformed JSON**: Models generate invalid JSON (~5-15% failure rate even with prompting)
- **No schema validation**: Output may be valid JSON but violate expected structure
- **Post-processing overhead**: Parsing, validation, and error handling must be manual
- **Retry complexity**: Applications must implement retry loops with repair attempts

### Key Challenges

1. **Agentic Framework Integration**: LangChain, CrewAI, Claude Flow require guaranteed JSON for tool/function calling
2. **Production Reliability**: 95%+ success rate needed; current prompting-based approaches achieve 85-95%
3. **Schema Enforcement**: Output must conform to JSON Schema or Pydantic models
4. **Performance**: Constrained decoding adds computational overhead to generation

### Real-World Impact

**Without JSON Mode:**
```python
# Current unreliable workflow
response = llm.generate("Extract person info as JSON: {text}")
try:
    data = json.loads(response)  # May fail
    assert "name" in data         # May fail
    assert "age" in data          # May fail
except:
    # Retry with prompt engineering, repair attempts, etc.
    pass
```

**With JSON Mode:**
```python
# Reliable workflow with schema
schema = {"type": "object", "properties": {"name": {"type": "string"}, "age": {"type": "integer"}}}
response = llm.generate_json("Extract person info: {text}", schema=schema)
# Guaranteed valid JSON conforming to schema
```

---

## Decision Drivers

### Reliability Requirements
- **99%+ valid JSON**: Eliminate malformed JSON failures
- **Schema conformance**: Guarantee output matches expected structure
- **Graceful degradation**: Repair mode for minor violations vs strict failure

### Performance Requirements
- **Minimal overhead**: <10% latency increase for JSON mode
- **Streaming compatible**: Support streaming JSON generation
- **Scalable**: Constrained decoding must work with large vocabularies (32K-128K tokens)

### Compatibility Requirements
- **Framework integration**: Compatible with LangChain, CrewAI, Claude Flow tool use
- **Schema standards**: Support JSON Schema, Pydantic models, TypeScript interfaces
- **Backward compatibility**: Existing `generate` interface unchanged

### Developer Experience
- **Simple API**: Single parameter enables JSON mode
- **Validation feedback**: Clear error messages on schema violations
- **Grammar flexibility**: Support custom grammars for domain-specific formats

---

## Considered Options

### Option A: Post-Generation Validation Only

Validate and repair JSON after generation completes.

**Pros:**
- Zero generation overhead
- Simple implementation
- Works with any model

**Cons:**
- Does not prevent invalid JSON (still 5-15% failures)
- Repair attempts may fail or produce incorrect data
- Wasted compute on failed generations
- Requires retry loops

### Option B: Constrained Decoding (Token-Level Enforcement)

Modify logits during generation to enforce JSON grammar at each token.

**Pros:**
- Guaranteed valid JSON (100% success rate)
- No retry loops needed
- Works with streaming generation
- Can enforce complex grammars

**Cons:**
- 5-10% latency overhead per token
- Implementation complexity (state machine for JSON structure)
- Requires access to model logits

### Option C: Fine-Tuned JSON Models

Train separate model checkpoints optimized for JSON output.

**Pros:**
- Best performance (native JSON understanding)
- No generation overhead
- Highest quality output

**Cons:**
- Requires training infrastructure
- Multiple model variants to maintain
- Does not generalize to custom schemas
- High storage/deployment cost

---

## Decision Outcome

**Chosen Option: Option B - Constrained Decoding with Optional Post-Validation**

Implement token-level constrained decoding as the primary JSON mode, with optional post-generation validation for models without logit access. This provides guaranteed JSON validity with acceptable performance overhead.

### Rationale

1. **Reliability first**: Agentic workflows require 99%+ success rates; only constrained decoding guarantees this
2. **Framework compatibility**: LangChain, CrewAI, Claude Flow expect reliable JSON mode
3. **Streaming support**: Constrained decoding works with streaming generation
4. **Graceful fallback**: Post-validation mode for models/backends without logit access
5. **Industry standard**: Matches llama.cpp (GBNF), Outlines, guidance library approaches

---

## Technical Specifications

### API Design

```rust
/// JSON Mode configuration for structured output
#[derive(Debug, Clone)]
pub struct JsonModeConfig {
    /// Optional JSON Schema for validation
    pub schema: Option<JsonSchema>,

    /// Strict mode: fail on invalid JSON (vs repair attempts)
    pub strict: bool,

    /// Repair mode: attempt to fix malformed JSON
    pub repair: bool,

    /// Grammar file for custom structured formats (GBNF-compatible)
    pub grammar: Option<String>,

    /// Enable constrained decoding (vs post-validation only)
    pub constrained_decoding: bool,
}

impl Default for JsonModeConfig {
    fn default() -> Self {
        Self {
            schema: None,
            strict: true,
            repair: false,
            grammar: None,
            constrained_decoding: true,
        }
    }
}

/// Extended generation parameters with JSON mode
#[derive(Debug, Clone)]
pub struct GenerateParams {
    // Existing fields
    pub max_tokens: usize,
    pub temperature: f32,
    pub top_p: f32,

    // New JSON mode
    pub json_mode: Option<JsonModeConfig>,
}

/// LLM Backend trait with JSON mode support
pub trait LlmBackend {
    /// Existing text generation
    fn generate(&self, prompt: &str, params: GenerateParams) -> Result<String>;

    /// JSON-structured generation (convenience wrapper)
    fn generate_json(
        &self,
        prompt: &str,
        schema: Option<JsonSchema>,
        params: GenerateParams
    ) -> Result<serde_json::Value> {
        let mut json_params = params.clone();
        json_params.json_mode = Some(JsonModeConfig {
            schema,
            ..Default::default()
        });

        let output = self.generate(prompt, json_params)?;
        serde_json::from_str(&output)
            .map_err(|e| Error::msg(format!("Invalid JSON output: {}", e)))
    }

    /// Streaming generation with JSON mode
    fn generate_stream(
        &self,
        prompt: &str,
        params: GenerateParams
    ) -> impl Stream<Item = Result<String>>;
}
```

### JSON Schema Support

```rust
use schemars::schema::RootSchema;
use serde_json::Value;

/// JSON Schema for validation
#[derive(Debug, Clone)]
pub struct JsonSchema {
    /// JSON Schema specification (Draft 7 or 2020-12)
    pub schema: RootSchema,
}

impl JsonSchema {
    /// Create from JSON Schema string
    pub fn from_str(schema_json: &str) -> Result<Self> {
        let schema: RootSchema = serde_json::from_str(schema_json)?;
        Ok(Self { schema })
    }

    /// Create from Pydantic-style Rust struct
    pub fn from_type<T: schemars::JsonSchema>() -> Self {
        let schema = schemars::schema_for!(T);
        Self { schema }
    }

    /// Validate JSON value against schema
    pub fn validate(&self, value: &Value) -> Result<()> {
        let validator = jsonschema::validator_for(&serde_json::to_value(&self.schema)?)?;
        validator.validate(value)
            .map_err(|e| Error::msg(format!("Schema validation failed: {}", e)))
    }
}
```

### Constrained Decoding Implementation

```rust
/// Token-level JSON constraint enforcer
pub struct JsonConstraintDecoder {
    /// Current state in JSON grammar (object, array, key, value, etc.)
    state: JsonState,

    /// Stack of open structures (brackets, braces)
    structure_stack: Vec<StructureType>,

    /// Expected schema at current position
    schema_context: Option<SchemaNode>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum JsonState {
    Start,
    ObjectStart,
    ObjectKey,
    ObjectColon,
    ObjectValue,
    ArrayStart,
    ArrayValue,
    String,
    Number,
    Boolean,
    Null,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum StructureType {
    Object,
    Array,
}

impl JsonConstraintDecoder {
    /// Apply logit bias based on current state
    pub fn apply_constraints(&mut self, logits: &mut [f32], vocab: &Vocabulary) -> Result<()> {
        match self.state {
            JsonState::Start => {
                // Only allow '{' or '['
                self.mask_except(logits, vocab, &["{", "["])?;
            }
            JsonState::ObjectStart => {
                // Allow '"' for key or '}' for empty object
                self.mask_except(logits, vocab, &["\"", "}"])?;
            }
            JsonState::ObjectKey => {
                // Must be string token (continue string or close with ")
                self.allow_string_tokens(logits, vocab)?;
            }
            JsonState::ObjectColon => {
                // Must be ':'
                self.mask_except(logits, vocab, &[":"])?;
            }
            JsonState::ObjectValue => {
                // Allow any valid JSON value start
                self.allow_value_start(logits, vocab)?;
            }
            JsonState::ArrayValue => {
                // Allow any valid JSON value start or ']' to close
                self.allow_value_start(logits, vocab)?;
                self.allow_token(logits, vocab, "]")?;
            }
            // ... other states
            _ => {}
        }

        Ok(())
    }

    /// Update state based on generated token
    pub fn update_state(&mut self, token: &str) -> Result<()> {
        match (self.state, token) {
            (JsonState::Start, "{") => {
                self.structure_stack.push(StructureType::Object);
                self.state = JsonState::ObjectStart;
            }
            (JsonState::Start, "[") => {
                self.structure_stack.push(StructureType::Array);
                self.state = JsonState::ArrayStart;
            }
            (JsonState::ObjectStart, "\"") => {
                self.state = JsonState::ObjectKey;
            }
            (JsonState::ObjectKey, "\"") => {
                self.state = JsonState::ObjectColon;
            }
            // ... state transitions
            _ => return Err(Error::msg("Invalid JSON token sequence"))
        }
        Ok(())
    }

    /// Check if generation is complete
    pub fn is_complete(&self) -> bool {
        self.state == JsonState::End && self.structure_stack.is_empty()
    }

    fn mask_except(&self, logits: &mut [f32], vocab: &Vocabulary, allowed: &[&str]) -> Result<()> {
        // Set all logits to -inf except allowed tokens
        logits.iter_mut().for_each(|l| *l = f32::NEG_INFINITY);
        for token in allowed {
            if let Some(id) = vocab.token_to_id(token) {
                logits[id] = 0.0; // Reset to neutral
            }
        }
        Ok(())
    }
}
```

### Schema-Aware Constraints

```rust
impl JsonConstraintDecoder {
    /// Apply schema constraints at current position
    fn apply_schema_constraints(&mut self, logits: &mut [f32], vocab: &Vocabulary) -> Result<()> {
        if let Some(schema) = &self.schema_context {
            match schema {
                SchemaNode::String => {
                    // Only allow string tokens
                    self.allow_string_tokens(logits, vocab)?;
                }
                SchemaNode::Integer => {
                    // Only allow numeric tokens (no decimal point)
                    self.allow_integer_tokens(logits, vocab)?;
                }
                SchemaNode::Boolean => {
                    // Only allow 'true' or 'false'
                    self.mask_except(logits, vocab, &["true", "false"])?;
                }
                SchemaNode::Enum(values) => {
                    // Only allow tokens from enum values
                    let allowed: Vec<&str> = values.iter().map(|s| s.as_str()).collect();
                    self.mask_except(logits, vocab, &allowed)?;
                }
                SchemaNode::Object(props) => {
                    // Only allow property names from schema
                    let allowed: Vec<&str> = props.keys().map(|s| s.as_str()).collect();
                    self.allow_tokens(logits, vocab, &allowed)?;
                }
                // ... other schema types
            }
        }
        Ok(())
    }
}
```

### Grammar-Based Generation (GBNF Support)

```rust
/// GBNF (llama.cpp) compatible grammar
#[derive(Debug, Clone)]
pub struct Grammar {
    /// Grammar rules in GBNF format
    rules: HashMap<String, GrammarRule>,
    /// Start rule name
    start: String,
}

#[derive(Debug, Clone)]
enum GrammarRule {
    /// Terminal: exact string match
    Terminal(String),
    /// Reference to another rule
    Reference(String),
    /// Sequence: rules in order
    Sequence(Vec<GrammarRule>),
    /// Choice: one of multiple rules
    Choice(Vec<GrammarRule>),
    /// Optional: zero or one
    Optional(Box<GrammarRule>),
    /// Repeat: zero or more
    Repeat(Box<GrammarRule>),
}

impl Grammar {
    /// Parse GBNF grammar string
    pub fn from_gbnf(grammar_str: &str) -> Result<Self> {
        // Parse GBNF format (similar to llama.cpp)
        // Example:
        // root ::= object
        // object ::= "{" ws members ws "}"
        // members ::= pair (ws "," ws pair)*
        // pair ::= string ws ":" ws value
        // ...
        todo!("GBNF parser implementation")
    }

    /// Create JSON grammar
    pub fn json() -> Self {
        // Built-in JSON grammar
        todo!("Built-in JSON grammar")
    }

    /// Apply grammar constraints to logits
    pub fn apply_constraints(
        &self,
        current_state: &GrammarState,
        logits: &mut [f32],
        vocab: &Vocabulary
    ) -> Result<()> {
        // Determine valid next tokens based on grammar state
        let valid_tokens = self.get_valid_tokens(current_state)?;

        // Mask logits for invalid tokens
        logits.iter_mut().for_each(|l| *l = f32::NEG_INFINITY);
        for token in valid_tokens {
            if let Some(id) = vocab.token_to_id(&token) {
                logits[id] = 0.0;
            }
        }

        Ok(())
    }
}
```

### Post-Validation Mode (Fallback)

```rust
/// JSON repair and validation (for backends without logit access)
pub struct JsonValidator {
    schema: Option<JsonSchema>,
    strict: bool,
    repair: bool,
}

impl JsonValidator {
    /// Validate and optionally repair JSON output
    pub fn validate(&self, output: &str) -> Result<String> {
        // Attempt to parse JSON
        match serde_json::from_str::<Value>(output) {
            Ok(value) => {
                // Valid JSON, check schema
                if let Some(schema) = &self.schema {
                    schema.validate(&value)?;
                }
                Ok(output.to_string())
            }
            Err(e) if self.repair => {
                // Attempt repair
                self.repair_json(output)
            }
            Err(e) if self.strict => {
                Err(Error::msg(format!("Invalid JSON: {}", e)))
            }
            Err(_) => {
                // Non-strict mode: return as-is with warning
                Ok(output.to_string())
            }
        }
    }

    fn repair_json(&self, output: &str) -> Result<String> {
        // Common repairs:
        // 1. Add missing closing braces/brackets
        // 2. Fix trailing commas
        // 3. Escape unescaped quotes
        // 4. Remove markdown code fences

        let mut repaired = output.to_string();

        // Remove markdown code fences
        repaired = repaired
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
            .to_string();

        // Count open/close braces and brackets
        let open_braces = repaired.matches('{').count();
        let close_braces = repaired.matches('}').count();
        let open_brackets = repaired.matches('[').count();
        let close_brackets = repaired.matches(']').count();

        // Add missing closing characters
        for _ in close_braces..open_braces {
            repaired.push('}');
        }
        for _ in close_brackets..open_brackets {
            repaired.push(']');
        }

        // Validate repaired JSON
        serde_json::from_str::<Value>(&repaired)
            .map(|_| repaired)
            .map_err(|e| Error::msg(format!("Repair failed: {}", e)))
    }
}
```

---

## Implementation Plan

### Phase 1: Basic JSON Validation (Week 1)
**Effort:** 2-3 days

1. Implement `JsonModeConfig` and `JsonSchema` types
2. Add `json_mode` field to `GenerateParams`
3. Implement post-generation validation with `JsonValidator`
4. Add `generate_json` convenience method
5. Tests for validation and repair

**Deliverables:**
- Post-validation JSON mode working with all backends
- Schema validation with JSON Schema Draft 7
- Basic repair for common issues

### Phase 2: Constrained Decoding (Week 2-3)
**Effort:** 5-7 days

1. Implement `JsonConstraintDecoder` state machine
2. Integrate with Candle backend logit processing
3. Add schema-aware constraints
4. Streaming support for JSON mode
5. Benchmark performance overhead

**Deliverables:**
- Constrained decoding for Candle backend
- 99%+ valid JSON success rate
- <10% latency overhead
- Streaming JSON generation

### Phase 3: Grammar Support (Week 4-5)
**Effort:** 7-10 days

1. Implement GBNF grammar parser
2. Build grammar state machine
3. Create built-in grammars (JSON, JSONL, CSV, XML)
4. Custom grammar API
5. Grammar compilation and optimization

**Deliverables:**
- GBNF-compatible grammar system
- Built-in grammars for common formats
- Custom grammar support

### Phase 4: Integration & Optimization (Week 6)
**Effort:** 3-5 days

1. Integrate with mistral-rs backend (ADR-008)
2. Framework adapters (LangChain, CrewAI)
3. Performance optimization (caching valid tokens)
4. Documentation and examples

**Deliverables:**
- Framework integration examples
- Optimized constraint checking
- Comprehensive documentation

---

## Performance Impact

### Latency Overhead

| Mode | Overhead | Notes |
|------|----------|-------|
| No JSON mode | 0% | Baseline |
| Post-validation only | <1% | Validation after generation |
| Constrained decoding | 5-10% | Per-token logit masking |
| Grammar-based | 8-12% | Complex grammar state machine |

### Memory Overhead

| Component | Memory | Notes |
|-----------|--------|-------|
| JSON state machine | ~1KB | Negligible |
| Schema tree | 10-100KB | Depends on schema complexity |
| Grammar rules | 50-500KB | GBNF grammar compilation |
| Valid token cache | 100-500KB | Per-state valid token sets |

### Reliability Improvement

| Method | Valid JSON Rate | Schema Conformance |
|--------|-----------------|-------------------|
| Prompt engineering only | 85-95% | 70-85% |
| Post-validation + repair | 95-98% | 85-95% |
| Constrained decoding | 99.9%+ | 99%+ |

---

## Consequences

### Positive Consequences

1. **Production reliability**: 99%+ success rate enables reliable agentic workflows
2. **Framework compatibility**: Direct integration with LangChain, CrewAI, Claude Flow
3. **Developer experience**: Simple API eliminates retry loops and error handling
4. **Streaming support**: JSON mode works with streaming generation
5. **Future extensibility**: Grammar support enables custom structured formats

### Negative Consequences

1. **Performance overhead**: 5-10% latency increase for constrained decoding
2. **Implementation complexity**: State machine and grammar parsing add code complexity
3. **Backend limitations**: Not all backends support logit access (fallback to post-validation)
4. **Token vocabulary dependency**: Constraint effectiveness depends on tokenizer granularity

### Neutral Consequences

1. **Optional feature**: JSON mode is opt-in via `GenerateParams`
2. **Graceful degradation**: Falls back to post-validation for unsupported backends
3. **Schema flexibility**: Supports JSON Schema, Pydantic, and custom grammars

### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| High latency overhead | Cache valid token sets per state; optimize state transitions |
| Complex grammar bugs | Extensive test suite with fuzzing; start with simple JSON grammar |
| Tokenizer edge cases | Handle subword tokens; fallback to character-level constraints |
| Schema complexity | Limit schema depth; provide performance warnings for complex schemas |

---

## Alternatives Considered

### Prompt Engineering Only

- **Rejected**: 85-95% success rate insufficient for production
- **Consideration**: Still useful as complementary technique

### Model-Specific JSON Modes

- **Rejected**: Requires separate models; doesn't generalize to custom schemas
- **Consideration**: Could offer as optimization for common cases

### External Validation Services

- **Rejected**: Adds network latency; doesn't prevent generation failures
- **Consideration**: Could integrate as async validation for auditing

---

## Related Decisions

- **ADR-001**: Ruvector Core Architecture (HNSW, Graph Store)
- **ADR-002**: RuvLLM Integration with Ruvector
- **ADR-007**: Security Review & Technical Debt
- **ADR-008**: mistral-rs Integration for Production-Scale LLM Serving

---

## Compliance and Standards

### JSON Schema Standards
- JSON Schema Draft 7 (primary support)
- JSON Schema 2020-12 (future)
- Pydantic model compatibility

### Grammar Standards
- GBNF (llama.cpp) compatibility
- EBNF subset for custom grammars
- Regex-based constraints (limited support)

### Framework Compatibility
- LangChain StructuredOutputParser
- CrewAI tool schemas
- Claude Flow structured outputs
- AutoGen function calling

### Testing Requirements
- Unit tests for state machine transitions
- Integration tests with sample schemas
- Fuzzing for grammar parser
- Benchmark suite for performance
- Framework integration tests

### Documentation Requirements
- JSON mode API guide
- Schema definition tutorial
- Grammar syntax reference
- Framework integration examples
- Performance optimization guide

---

## References

1. **llama.cpp GBNF**: https://github.com/ggerganov/llama.cpp/blob/master/grammars/README.md
2. **Outlines Library**: https://github.com/outlines-dev/outlines - Structured text generation
3. **Guidance Library**: https://github.com/guidance-ai/guidance - Constrained generation
4. **JSON Schema**: https://json-schema.org/specification
5. **LangChain StructuredOutput**: https://python.langchain.com/docs/modules/model_io/output_parsers/structured
6. **OpenAI JSON Mode**: https://platform.openai.com/docs/guides/structured-outputs
7. **Anthropic Tool Use**: https://docs.anthropic.com/en/docs/build-with-claude/tool-use

---

## Implementation Status

| Component | Status | Effort | Notes |
|-----------|--------|--------|-------|
| JsonModeConfig types | Pending | 0.5 days | Basic config structures |
| JsonSchema validation | Pending | 1 day | JSON Schema Draft 7 support |
| Post-validation mode | Pending | 1 day | Fallback for all backends |
| JSON repair | Pending | 1 day | Common malformation fixes |
| JsonConstraintDecoder | Pending | 3 days | State machine for JSON grammar |
| Schema-aware constraints | Pending | 2 days | Schema-driven logit masking |
| Streaming JSON | Pending | 2 days | Stream-compatible constraints |
| GBNF parser | Pending | 5 days | Grammar definition language |
| Grammar state machine | Pending | 3 days | Generic grammar constraints |
| Built-in grammars | Pending | 2 days | JSON, JSONL, CSV, XML |
| Candle integration | Pending | 2 days | Wire to Candle backend |
| mistral-rs integration | Pending | 2 days | Wire to mistral-rs backend |
| Framework adapters | Pending | 3 days | LangChain, CrewAI examples |
| Performance optimization | Pending | 2 days | Token caching, fast paths |
| Documentation | Pending | 3 days | API guide, examples, tutorials |

**Total Effort:** ~30-35 days (1 developer)
**Phased Delivery:** 4-6 weeks

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-20 | Ruvector Architecture Team | Initial proposal |
