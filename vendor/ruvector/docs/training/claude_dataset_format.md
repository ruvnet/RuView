# Claude Task Dataset Format Specification

## Overview

The Claude Task Fine-Tuning Dataset is designed for training RuvLTRA models to intelligently route tasks to appropriate Claude Flow agents and select optimal Claude models (Haiku/Sonnet/Opus) based on task complexity.

## Dataset Categories

### 1. Coder Tasks
**Agent:** `coder`
**Focus:** Code generation, debugging, refactoring
**Model Routing:**
- Simple: Haiku (quick fixes, simple functions)
- Moderate: Sonnet (component development, API integration)
- Complex: Opus (complex algorithms, system-level code)

**Example Tasks:**
- Implement authentication middleware
- Debug race condition in concurrent code
- Refactor monolithic service into microservices
- Write unit tests with 90% coverage

### 2. Researcher Tasks
**Agent:** `researcher`
**Focus:** Analysis, exploration, documentation
**Model Routing:**
- Simple: Haiku (basic documentation)
- Moderate: Sonnet (most research tasks)
- Complex: Sonnet (deep analysis)

**Example Tasks:**
- Analyze performance bottlenecks
- Research best practices for GraphQL
- Document API endpoints
- Compare database solutions

### 3. Security Tasks
**Agent:** `security`
**Focus:** Audit, vulnerability analysis, threat detection
**Model Routing:**
- All: Opus (security requires highest quality)

**Example Tasks:**
- Audit authentication flow for vulnerabilities
- Review cryptographic implementation
- Identify SQL injection vectors
- Ensure GDPR compliance

### 4. Architecture Tasks
**Agent:** `architecture`
**Focus:** System design, planning, architecture
**Model Routing:**
- Simple: Sonnet (basic schemas)
- Moderate: Opus (microservices, APIs)
- Complex: Opus (distributed systems)

**Example Tasks:**
- Design microservices architecture
- Plan database schema for e-commerce
- Architect caching strategy
- Design disaster recovery system

### 5. Reviewer Tasks
**Agent:** `reviewer`
**Focus:** Code review, quality assessment
**Model Routing:**
- Simple: Haiku (standards compliance)
- Moderate: Sonnet (quality review, performance)
- Complex: Sonnet (architecture review)

**Example Tasks:**
- Review pull request for best practices
- Assess code quality and maintainability
- Review error handling patterns
- Analyze scalability of design

## JSONL Format

Each line in the JSONL file represents a single training example:

```json
{
  "input": "Implement async authentication middleware in TypeScript for JWT validation",
  "context": "The middleware should verify JWT tokens from Bearer header, check expiration, and validate signature using RS256",
  "output_agent": "coder",
  "metadata": {
    "category": "Coder",
    "complexity": "Moderate",
    "domain": "Web",
    "expected_model": "sonnet",
    "quality_score": 0.87,
    "tags": ["authentication", "middleware", "jwt", "security"]
  }
}
```

## Fields Description

### Input
**Type:** String
**Description:** The task description or request from the user. This is what the model receives as input.

### Context
**Type:** String
**Description:** Additional context, requirements, constraints, or details about the task. Provides necessary background information.

### Output Agent
**Type:** String
**Enum:** `"coder"`, `"researcher"`, `"security"`, `"architecture"`, `"reviewer"`
**Description:** The expected agent that should handle this task.

### Metadata

#### Category
**Type:** TaskCategory enum
**Values:** `Coder`, `Researcher`, `Security`, `Architecture`, `Reviewer`
**Description:** Primary task category

#### Complexity
**Type:** ComplexityLevel enum
**Values:** `Simple`, `Moderate`, `Complex`
**Description:** Task complexity level determining model selection

#### Domain
**Type:** DomainType enum
**Values:** `Web`, `Systems`, `DataScience`, `Mobile`, `DevOps`, `Security`, `Database`, `Api`
**Description:** Technical domain context

#### Expected Model
**Type:** String
**Values:** `"haiku"`, `"sonnet"`, `"opus"`
**Description:** Recommended Claude model for this task based on complexity and category

**Cost Optimization:**
- Haiku: ~75% cheaper than Opus, 2-3x faster
- Sonnet: Balanced cost/quality, handles most tasks
- Opus: Highest quality, use for complex/critical tasks

#### Quality Score
**Type:** Float (0.0-1.0)
**Description:** Quality rating of this training example. Higher scores indicate more reliable examples for training.

#### Tags
**Type:** Array of strings
**Description:** Descriptive tags for filtering and analysis

## Data Augmentation

The dataset generator applies three augmentation techniques:

### 1. Paraphrasing
**Purpose:** Increase linguistic diversity
**Method:** Synonym replacement, phrase restructuring
**Example:**
- Original: "Implement a function to validate user input"
- Paraphrased: "Create a function to validate user input"

### 2. Complexity Variations
**Purpose:** Create training examples at different complexity levels
**Method:** Vary complexity while keeping core task same
**Example:**
- Simple: "Add error handling to API endpoint"
- Moderate: "Implement comprehensive error handling with retry logic"
- Complex: "Design fault-tolerant error handling with circuit breakers"

### 3. Domain Transfer
**Purpose:** Generalize across technical domains
**Method:** Apply same task pattern to different domains
**Example:**
- Web: "Optimize React component rendering"
- Mobile: "Optimize Flutter widget rendering"
- Systems: "Optimize kernel thread scheduling"

## Dataset Statistics

Typical generated dataset (100 base examples per category + augmentation):

```
Total Examples: ~1,500 (500 base + 1,000 augmented)

By Category:
- Coder:        ~300 (20%)
- Researcher:   ~300 (20%)
- Security:     ~300 (20%)
- Architecture: ~300 (20%)
- Reviewer:     ~300 (20%)

By Complexity:
- Simple:    ~500 (33%)
- Moderate:  ~600 (40%)
- Complex:   ~400 (27%)

By Model:
- Haiku:  ~400 (27%) - Cost-effective for simple tasks
- Sonnet: ~700 (47%) - Balanced for most tasks
- Opus:   ~400 (27%) - High-quality for complex/security
```

## Training Splits

Recommended split ratios:
- **Training:** 70% (~1,050 examples)
- **Validation:** 15% (~225 examples)
- **Test:** 15% (~225 examples)

Stratified sampling ensures balanced representation across categories and complexity levels.

## Quality Assurance

Each training example includes a quality score (0.0-1.0) based on:

1. **Template Quality** (0.8-0.96)
   - Seed templates: Hand-crafted, highest quality
   - Paraphrased: Slightly lower due to automated generation

2. **Category Appropriateness**
   - Security tasks: Higher scores (0.90-0.96)
   - Code generation: Good scores (0.83-0.90)

3. **Complexity Alignment**
   - Well-defined complexity: Higher scores
   - Ambiguous complexity: Lower scores

## Usage in Fine-Tuning

### For Task Routing
Train model to predict `output_agent` given `input` and `context`.

```python
# Pseudo-code
def train_task_router(dataset):
    for example in dataset:
        x = embed(example.input + example.context)
        y = encode_agent(example.output_agent)
        model.train(x, y)
```

### For Model Selection
Train model to predict `expected_model` given task characteristics.

```python
# Pseudo-code
def train_model_selector(dataset):
    for example in dataset:
        features = extract_features(example.input, example.context)
        complexity = encode_complexity(example.metadata.complexity)
        category = encode_category(example.metadata.category)
        x = [features, complexity, category]
        y = encode_model(example.metadata.expected_model)
        model.train(x, y)
```

## Export Formats

### JSONL (Recommended)
- One example per line
- Memory-efficient streaming
- Standard for LLM fine-tuning
- File: `claude_training_full.jsonl`

### JSON
- Full array of examples
- Human-readable
- Good for inspection
- File: `claude_training_full.json`

### Parquet (Planned)
- Columnar format
- Highly compressed
- Fast for analytics
- Integration with Arrow/Polars

## Example Generation Code

```rust
use ruvllm::training::{DatasetGenerator, DatasetConfig};

// Configure dataset
let config = DatasetConfig {
    examples_per_category: 100,
    enable_augmentation: true,
    ..Default::default()
};

// Generate dataset
let mut generator = DatasetGenerator::new(config);
let dataset = generator.generate();

// Export to JSONL
dataset.export_jsonl("training.jsonl")?;

// Split for training
let (train, val, test) = dataset.split(0.7, 0.15, 0.15, 42);
```

## Integration with RuvLTRA

The dataset is designed for fine-tuning RuvLTRA models with:

1. **Task Embedding Layer**
   - Input: Task description + context
   - Output: 768-dim semantic embedding

2. **Agent Classification Head**
   - Input: Task embedding
   - Output: 5-way classification (5 agent types)

3. **Model Selection Head**
   - Input: Task embedding + complexity features
   - Output: 3-way classification (Haiku/Sonnet/Opus)

4. **Quality Prediction Head**
   - Input: Task embedding
   - Output: Quality score (0-1)

## Versioning

**Current Version:** 1.0.0
**Format Version:** 1.0
**Last Updated:** 2024-01

## License

Training data follows the same license as RuvLTRA (MIT/Apache-2.0).

## References

- Claude Flow Documentation: https://github.com/ruvnet/claude-flow
- RuvLTRA Architecture: `../crates/ruvllm/README.md`
- SONA Learning: `../crates/sona/README.md`
