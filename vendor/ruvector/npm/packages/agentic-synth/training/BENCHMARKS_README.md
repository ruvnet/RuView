# DSPy Benchmark Comparison Framework

A comprehensive benchmarking suite for comparing multiple models across quality, performance, cost, learning, and diversity metrics.

## Features

### 🎯 Core Capabilities

1. **Multi-Model Comparison**
   - Compare unlimited models side-by-side
   - Statistical significance testing
   - Pareto frontier analysis
   - Weighted scoring across dimensions

2. **Scalability Testing**
   - Test from 100 to 100,000 samples
   - Measure latency, throughput, cost at scale
   - Calculate scaling efficiency
   - Identify performance bottlenecks

3. **Cost Analysis**
   - Track total cost per run
   - Calculate cost per sample
   - Compute cost per quality point
   - Efficiency rankings

4. **Quality Convergence**
   - Measure learning rates
   - Track improvement over generations
   - Identify plateau points
   - Convergence speed analysis

5. **Diversity Analysis**
   - Unique value counting
   - Pattern variety measurement
   - Shannon entropy calculation
   - Coverage scoring

### 📊 Metrics Collected

#### Quality Metrics
- **Accuracy**: Correctness of generated data
- **Coherence**: Logical consistency and flow
- **Validity**: Adherence to schema and constraints
- **Consistency**: Uniformity across samples
- **Completeness**: Coverage of all required fields
- **Overall**: Weighted average of all quality metrics

#### Performance Metrics
- **Latency P50/P95/P99**: Response time percentiles
- **Average Latency**: Mean response time
- **Min/Max Latency**: Range of response times
- **Throughput**: Samples generated per second
- **Success Rate**: Percentage of successful generations

#### Cost Metrics
- **Total Cost**: Total expenditure for test run
- **Cost per Sample**: Average cost per generated sample
- **Cost per Quality Point**: Cost normalized by quality
- **Tokens Used**: Total tokens consumed
- **Efficiency**: Quality per unit cost

#### Learning Metrics
- **Improvement Rate**: Quality gain per generation
- **Convergence Speed**: Generations until plateau
- **Learning Curve**: Quality progression over time
- **Plateau Generation**: When learning stabilizes
- **Final Quality**: Ultimate quality achieved

#### Diversity Metrics
- **Unique Values**: Number of distinct samples
- **Pattern Variety**: Ratio of unique to total samples
- **Distribution Entropy**: Shannon entropy of data
- **Coverage Score**: Field-level diversity measure
- **Novelty Rate**: Rate of new pattern generation

## Usage

### Quick Start

```typescript
import { BenchmarkSuite } from './dspy-benchmarks.js';

const suite = new BenchmarkSuite();

// Add common models
suite.addCommonModels();

// Run comprehensive comparison
const comparison = await suite.runModelComparison(1000);

// Generate reports
await suite.generateJSONReport(comparison);
await suite.generateMarkdownReport(comparison);
```

### Custom Models

```typescript
import { BenchmarkSuite, ModelConfig } from './dspy-benchmarks.js';

const suite = new BenchmarkSuite();

// Add custom model
const customModel: ModelConfig = {
  name: 'My Custom Model',
  provider: 'openrouter',
  model: 'my-model',
  costPer1kTokens: 0.002,
  maxTokens: 8192,
  apiKey: process.env.API_KEY, // Optional
};

suite.addModel(customModel);

// Run benchmarks
const comparison = await suite.runModelComparison(1000);
```

### Running from CLI

```bash
# Full benchmark suite
npx tsx training/run-benchmarks.ts full

# Quick comparison (3 models, 500 samples)
npx tsx training/run-benchmarks.ts quick

# Scalability test only
npx tsx training/run-benchmarks.ts scalability

# Cost analysis only
npx tsx training/run-benchmarks.ts cost
```

## API Reference

### BenchmarkSuite Class

#### Constructor

```typescript
constructor(outputDir?: string)
```

Creates a new benchmark suite instance.

- `outputDir`: Optional output directory (default: `./training/results/benchmarks`)

#### Methods

##### addModel(config: ModelConfig)

Add a model to the benchmark suite.

```typescript
suite.addModel({
  name: 'GPT-4',
  provider: 'openai',
  model: 'gpt-4',
  costPer1kTokens: 0.03,
  maxTokens: 8192,
});
```

##### addCommonModels()

Add 6 pre-configured common models for quick testing:
- GPT-4
- Claude 3.5 Sonnet
- Gemini Pro
- GPT-3.5 Turbo
- Llama 3 70B
- Mixtral 8x7B

```typescript
suite.addCommonModels();
```

##### runModelComparison(sampleSize?: number): Promise<ComparisonResult>

Run comprehensive comparison across all models.

```typescript
const comparison = await suite.runModelComparison(1000);
```

**Returns**: ComparisonResult with winners, statistical significance, Pareto frontier, and recommendations.

##### runScalabilityTest(): Promise<ScalabilityResult[]>

Test scalability from 100 to 100K samples.

```typescript
const results = await suite.runScalabilityTest();
```

**Tests**: 100, 500, 1K, 5K, 10K, 50K, 100K samples

##### runCostAnalysis(): Promise<void>

Analyze cost-effectiveness across models.

```typescript
await suite.runCostAnalysis();
```

**Outputs**: Cost rankings, efficiency scores, cost/quality trade-offs

##### runQualityConvergence(generations?: number): Promise<void>

Measure learning rates and quality convergence.

```typescript
await suite.runQualityConvergence(10);
```

**Default**: 10 generations

##### runDiversityAnalysis(sampleSize?: number): Promise<void>

Analyze data diversity and variety.

```typescript
await suite.runDiversityAnalysis(5000);
```

**Default**: 5000 samples

##### generateJSONReport(comparison: ComparisonResult): Promise<void>

Generate comprehensive JSON report.

```typescript
await suite.generateJSONReport(comparison);
```

**Output**: `benchmark-comparison.json`

##### generateMarkdownReport(comparison: ComparisonResult): Promise<void>

Generate human-readable Markdown report.

```typescript
await suite.generateMarkdownReport(comparison);
```

**Output**: `BENCHMARK_REPORT.md`

## Output Files

### JSON Reports

#### benchmark-comparison.json
Complete benchmark results including:
- Metadata and timestamps
- Comparison results
- All model results
- Statistical summaries

#### scalability-results.json
Scalability test results including:
- Latencies at each scale
- Throughput measurements
- Cost progression
- Scaling efficiency

#### convergence-data.json
Learning convergence data including:
- Quality curves
- Improvement rates
- Plateau generations

### Markdown Reports

#### BENCHMARK_REPORT.md
Comprehensive human-readable report including:
- Executive summary
- Detailed results per model
- Comparative tables
- Pareto frontier analysis
- Use case recommendations
- Statistical significance
- Methodology explanation
- Conclusions

## Use Case Recommendations

The benchmark suite automatically recommends models for different scenarios:

### High-Quality, Low-Volume (Research)
Best for research, high-stakes decisions, and scenarios where quality is paramount.

**Optimizes for**: Maximum quality, learning capability

### High-Volume, Low-Latency (Production)
Best for production systems requiring high throughput and low latency.

**Optimizes for**: Throughput, low latency, success rate

### Cost-Optimized (Batch Processing)
Best for batch processing, large-scale data generation, and cost-sensitive applications.

**Optimizes for**: Lowest cost per sample, efficiency

### Balanced (General Purpose)
Best for general-purpose applications requiring a good balance of quality, performance, and cost.

**Optimizes for**: Weighted score across all metrics

## Statistical Analysis

### T-Test for Significance

The suite performs t-tests to determine if quality differences between models are statistically significant:

- **p < 0.01**: Highly significant difference
- **p < 0.05**: Significant difference
- **p ≥ 0.05**: No significant difference

### Pareto Frontier

Identifies models with optimal quality/cost trade-offs. A model is on the Pareto frontier if no other model is better in both quality AND cost.

## Mock Data Generation

The framework includes a sophisticated mock data generator for demonstration purposes:

- **Realistic Latencies**: Based on actual model characteristics
- **Learning Simulation**: Quality improves over generations
- **Quality Differentiation**: Different models have different base qualities
- **Schema Support**: Handles various field types (UUID, email, name, numbers, etc.)

## Example Output

```
🔬 Running Model Comparison (1000 samples)
======================================================================

Testing GPT-4...
  Quality: 0.872
  Latency P95: 1589ms
  Cost/Sample: $0.004500
  Diversity: 0.843

Testing Claude 3.5 Sonnet...
  Quality: 0.891
  Latency P95: 1267ms
  Cost/Sample: $0.002250
  Diversity: 0.867

...

✅ All benchmarks completed!

📊 Key Findings:
   Overall Winner: Claude 3.5 Sonnet
   Best Quality: Claude 3.5 Sonnet
   Best Performance: Mixtral 8x7B
   Most Cost-Effective: Gemini Pro
   Pareto Frontier: Claude 3.5 Sonnet, Gemini Pro, Mixtral 8x7B

💡 Recommendations by Use Case:
   high-quality-low-volume: Claude 3.5 Sonnet
   high-volume-low-latency: Mixtral 8x7B
   cost-optimized: Gemini Pro
   balanced: Claude 3.5 Sonnet
   research: Claude 3.5 Sonnet
   production: Claude 3.5 Sonnet
```

## Advanced Features

### Custom Weighting

You can modify the overall winner calculation by adjusting weights in the `compareResults()` method:

```typescript
const score =
  quality * 0.3 +           // 30% quality
  performance * 0.2 +       // 20% performance
  (1/cost) * 0.2 +         // 20% cost
  learning * 0.15 +        // 15% learning
  diversity * 0.15;        // 15% diversity
```

### Statistical Utilities

The `StatisticalAnalyzer` class provides utilities for:
- Mean and standard deviation
- Percentile calculation
- T-test for significance
- Shannon entropy
- Distribution analysis

### Extensibility

Easily extend the framework:

1. **Add new metrics**: Extend metric interfaces
2. **Add new models**: Implement `ModelConfig`
3. **Add new tests**: Add methods to `BenchmarkSuite`
4. **Custom analysis**: Use `StatisticalAnalyzer` utilities

## Performance Considerations

- **Mock Mode**: Runs without API calls for testing
- **Parallel Testing**: Could be extended for concurrent model testing
- **Caching**: Results are cached to disk
- **Memory Efficient**: Processes samples in batches

## Limitations

- Mock data generator simulates behavior (no actual API calls)
- Quality metrics are approximations based on model characteristics
- Statistical tests use simplified distributions
- Assumes consistent model behavior

## Future Enhancements

- [ ] Real API integration with actual model calls
- [ ] Parallel model testing for faster benchmarks
- [ ] More sophisticated quality assessment
- [ ] Interactive visualization dashboard
- [ ] A/B testing framework
- [ ] Confidence interval calculation
- [ ] Cost prediction modeling
- [ ] Automated model selection

## License

MIT

## Contributing

Contributions welcome! Please ensure:
- TypeScript type safety
- Comprehensive documentation
- Test coverage
- Performance optimization

## Support

For issues or questions:
- GitHub Issues: https://github.com/ruvnet/ruvector/issues
- Documentation: See main project README
