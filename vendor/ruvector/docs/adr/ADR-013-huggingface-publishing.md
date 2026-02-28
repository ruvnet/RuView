# ADR-013: HuggingFace Model Publishing Strategy

## Status
**Accepted** - 2026-01-20

## Context

RuvLTRA models need to be distributed to users efficiently. HuggingFace Hub is the industry standard for model hosting with:
- High-speed CDN for global distribution
- Git-based versioning
- Model cards for documentation
- API for programmatic access
- Integration with major ML frameworks

## Decision

### 1. Repository Structure

All models consolidated under a single HuggingFace repository:

| Repository | Purpose | Models |
|------------|---------|--------|
| **`ruv/ruvltra`** | All RuvLTRA models | Claude Code, Small, Medium, Large |

**URL**: https://huggingface.co/ruv/ruvltra

### 2. File Naming Convention

```
ruvltra-{size}-{quant}.gguf
```

Examples:
- `ruvltra-0.5b-q4_k_m.gguf`
- `ruvltra-3b-q8_0.gguf`
- `ruvltra-claude-code-0.5b-q4_k_m.gguf`

### 3. Authentication

Support multiple environment variable names for HuggingFace token:
- `HF_TOKEN` (primary)
- `HUGGING_FACE_HUB_TOKEN` (legacy)
- `HUGGINGFACE_API_KEY` (common alternative)

### 4. Upload Workflow

```rust
// Using ModelUploader
let uploader = ModelUploader::new(get_hf_token().unwrap());
uploader.upload(
    "./model.gguf",
    "ruv/ruvltra-small",
    Some(metadata),
)?;
```

### 5. Model Card Requirements

Each repository must include:
- YAML frontmatter with tags, license, language
- Model description and capabilities
- Hardware requirements table
- Usage examples (Rust, Python, CLI)
- Benchmark results (when available)
- License information

### 6. Versioning Strategy

- Use HuggingFace's built-in Git versioning
- Tag major releases (e.g., `v1.0.0`)
- Maintain `main` branch for latest stable
- Use branches for experimental variants

## Consequences

### Positive
- **Accessibility**: Models available via standard HuggingFace APIs
- **Discoverability**: Indexed in HuggingFace model search
- **Versioning**: Full Git history for model evolution
- **CDN**: Fast global downloads via Cloudflare
- **Documentation**: Model cards provide user guidance

### Negative
- **Storage Costs**: Large models require HuggingFace Pro for private repos
- **Dependency**: Reliance on external service availability
- **Sync Complexity**: Must keep registry.rs in sync with HuggingFace

### Mitigations
- Use public repos (free unlimited storage)
- Implement fallback to direct URL downloads
- Automate registry updates via CI/CD

## Implementation

### Phase 1: Initial Publishing (Complete)
- [x] Create consolidated `ruv/ruvltra` repository
- [x] Upload Claude Code, Small, and Medium models
- [x] Upload Q4_K_M quantized models
- [x] Add comprehensive model card with badges, tutorials, architecture

### Phase 2: Enhanced Distribution
- [ ] Add Q8 quantization variants
- [ ] Add FP16 variants for fine-tuning
- [ ] Implement automated CI/CD publishing
- [ ] Add SONA weight exports

### Phase 3: Ecosystem Integration
- [ ] Add to llama.cpp model zoo
- [ ] Create Ollama modelfile
- [ ] Publish to alternative registries (ModelScope)

## References

- HuggingFace Hub Documentation: https://huggingface.co/docs/hub
- GGUF Format Specification: https://github.com/ggerganov/ggml/blob/master/docs/gguf.md
- RuvLTRA Registry: `crates/ruvllm/src/hub/registry.rs`
- Related Issue: #121
