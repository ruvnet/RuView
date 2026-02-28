# sevensense-interpretation

[![Crate](https://img.shields.io/badge/crates.io-sevensense--interpretation-orange.svg)](https://crates.io/crates/sevensense-interpretation)
[![Docs](https://img.shields.io/badge/docs-sevensense--interpretation-blue.svg)](https://docs.rs/sevensense-interpretation)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

> Evidence-based interpretation and explanation generation for bioacoustic AI.

**sevensense-interpretation** generates human-readable explanations for AI predictions. Using the RAB (Reasoning, Accountability, Believability) framework, it produces "evidence packs" that document why a species was identified, what features contributed to the decision, and how confident the system is—essential for scientific credibility and regulatory compliance.

## Features

- **RAB Evidence Packs**: Structured explanation documents
- **Confidence Scoring**: Multi-factor confidence with breakdowns
- **Feature Attribution**: Which acoustic features drove predictions
- **Uncertainty Quantification**: Epistemic vs. aleatoric uncertainty
- **Natural Language**: Human-readable narratives
- **Audit Trails**: Complete decision provenance

## Use Cases

| Use Case | Description | Key Functions |
|----------|-------------|---------------|
| Evidence Generation | Create explanation packs | `EvidencePack::generate()` |
| Confidence Scoring | Multi-factor confidence | `ConfidenceScorer::score()` |
| Feature Attribution | Explain which features matter | `attribute_features()` |
| Narrative Generation | Human-readable explanations | `generate_narrative()` |
| Audit Export | Compliance documentation | `export_audit_trail()` |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
sevensense-interpretation = "0.1"
```

## Quick Start

```rust
use sevensense_interpretation::{EvidenceGenerator, EvidenceConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create evidence generator
    let generator = EvidenceGenerator::new(EvidenceConfig::default());

    // Generate evidence pack for a prediction
    let evidence = generator.generate(
        &query_embedding,
        &prediction,
        &neighbors,
        &cluster_info,
    )?;

    println!("Confidence: {:.1}%", evidence.confidence * 100.0);
    println!("Reasoning: {}", evidence.narrative);
    println!("Key features: {:?}", evidence.top_features);

    Ok(())
}
```

---

<details>
<summary><b>Tutorial: Generating Evidence Packs</b></summary>

### Basic Evidence Generation

```rust
use sevensense_interpretation::{EvidenceGenerator, EvidenceConfig, Prediction};

let config = EvidenceConfig {
    include_neighbors: true,
    include_features: true,
    include_uncertainty: true,
    narrative_style: NarrativeStyle::Scientific,
};

let generator = EvidenceGenerator::new(config);

// Prediction to explain
let prediction = Prediction {
    species_id: "Turdus merula".into(),
    confidence: 0.94,
    embedding: query_embedding.clone(),
};

// Generate evidence
let evidence = generator.generate(
    &prediction,
    &neighbors,      // Similar examples from index
    &cluster_info,   // Clustering context
)?;

println!("{}", evidence.to_json()?);
```

### Evidence Pack Structure

```rust
// The EvidencePack contains:
println!("=== Evidence Pack ===");
println!("Prediction: {}", evidence.prediction.species_id);
println!("Overall Confidence: {:.1}%", evidence.overall_confidence * 100.0);

println!("\nConfidence Breakdown:");
println!("  Neighbor Agreement: {:.1}%", evidence.breakdown.neighbor_agreement * 100.0);
println!("  Cluster Membership: {:.1}%", evidence.breakdown.cluster_membership * 100.0);
println!("  Embedding Quality: {:.1}%", evidence.breakdown.embedding_quality * 100.0);

println!("\nSupporting Evidence:");
for (i, neighbor) in evidence.neighbors.iter().take(3).enumerate() {
    println!("  {}. {} (similarity: {:.3})",
        i + 1, neighbor.species_id, neighbor.similarity);
}

println!("\nNarrative:");
println!("{}", evidence.narrative);
```

</details>

<details>
<summary><b>Tutorial: Confidence Scoring</b></summary>

### Multi-Factor Confidence

```rust
use sevensense_interpretation::{ConfidenceScorer, ConfidenceConfig};

let config = ConfidenceConfig {
    neighbor_weight: 0.4,      // Weight for neighbor agreement
    cluster_weight: 0.3,       // Weight for cluster membership
    quality_weight: 0.3,       // Weight for embedding quality
};

let scorer = ConfidenceScorer::new(config);

let score = scorer.score(
    &prediction,
    &neighbors,
    &cluster_info,
)?;

println!("Overall: {:.3}", score.overall);
println!("Components:");
println!("  Neighbor Agreement: {:.3}", score.neighbor_agreement);
println!("  Cluster Membership: {:.3}", score.cluster_membership);
println!("  Embedding Quality: {:.3}", score.embedding_quality);
```

### Confidence Calibration

```rust
use sevensense_interpretation::{ConfidenceCalibrator, CalibrationData};

// Calibrate confidence scores using validation data
let calibrator = ConfidenceCalibrator::train(&validation_predictions)?;

// Apply calibration
let raw_confidence = 0.85;
let calibrated = calibrator.calibrate(raw_confidence);

println!("Raw: {:.2}, Calibrated: {:.2}", raw_confidence, calibrated);

// Calibration diagnostics
let diagnostics = calibrator.diagnostics();
println!("ECE (Expected Calibration Error): {:.4}", diagnostics.ece);
println!("MCE (Maximum Calibration Error): {:.4}", diagnostics.mce);
```

### Uncertainty Decomposition

```rust
use sevensense_interpretation::{UncertaintyEstimator, UncertaintyType};

let estimator = UncertaintyEstimator::new();

let uncertainty = estimator.estimate(&prediction, &neighbors)?;

println!("Total Uncertainty: {:.3}", uncertainty.total);
println!("  Epistemic (model uncertainty): {:.3}", uncertainty.epistemic);
println!("  Aleatoric (data uncertainty): {:.3}", uncertainty.aleatoric);

// Interpretation
if uncertainty.epistemic > uncertainty.aleatoric {
    println!("High epistemic uncertainty: model needs more training data");
} else {
    println!("High aleatoric uncertainty: inherently ambiguous input");
}
```

</details>

<details>
<summary><b>Tutorial: Feature Attribution</b></summary>

### Identifying Important Features

```rust
use sevensense_interpretation::{FeatureAttributor, AttributionMethod};

let attributor = FeatureAttributor::new(AttributionMethod::Gradient);

// Get feature importance scores
let attributions = attributor.attribute(
    &model,
    &query_embedding,
    &prediction,
)?;

println!("Top 10 most important embedding dimensions:");
let mut sorted: Vec<_> = attributions.iter().enumerate().collect();
sorted.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());

for (dim, importance) in sorted.iter().take(10) {
    println!("  Dimension {}: {:.4}", dim, importance);
}
```

### Acoustic Feature Mapping

```rust
use sevensense_interpretation::{AcousticFeatureMapper, AcousticFeature};

let mapper = AcousticFeatureMapper::new();

// Map embedding dimensions to acoustic features
let acoustic_attributions = mapper.map_to_acoustic(&attributions)?;

println!("Important acoustic features:");
for (feature, importance) in acoustic_attributions.iter().take(5) {
    println!("  {:?}: {:.3}", feature, importance);
}
// Output example:
//   Frequency Range (2-4 kHz): 0.342
//   Temporal Modulation: 0.287
//   Harmonic Structure: 0.156
```

### Contrastive Explanations

```rust
use sevensense_interpretation::ContrastiveExplainer;

let explainer = ContrastiveExplainer::new();

// Why species A and not species B?
let explanation = explainer.explain(
    &query_embedding,
    "Turdus merula",    // Predicted
    "Turdus philomelos", // Alternative
)?;

println!("Why {} and not {}?", explanation.predicted, explanation.contrast);
println!("Key differences:");
for diff in &explanation.differences {
    println!("  {}: {:.3} vs {:.3}",
        diff.feature, diff.predicted_value, diff.contrast_value);
}
```

</details>

<details>
<summary><b>Tutorial: Narrative Generation</b></summary>

### Scientific Narratives

```rust
use sevensense_interpretation::{NarrativeGenerator, NarrativeStyle};

let generator = NarrativeGenerator::new(NarrativeStyle::Scientific);

let narrative = generator.generate(&evidence)?;

println!("{}", narrative);
// Output:
// "The audio segment was classified as Turdus merula (Eurasian Blackbird)
// with 94.2% confidence. This classification is supported by high similarity
// (>0.90) to 8 confirmed Turdus merula recordings in the reference database.
// The embedding falls within the core region of the Turdus merula cluster
// (silhouette score: 0.87). Key discriminating features include the
// characteristic frequency range (2.1-4.3 kHz) and the presence of
// melodic phrases with harmonic structure typical of the species."
```

### Conversational Narratives

```rust
let generator = NarrativeGenerator::new(NarrativeStyle::Conversational);

let narrative = generator.generate(&evidence)?;

println!("{}", narrative);
// Output:
// "This sounds like a Eurasian Blackbird! I'm 94% confident because
// it matches several confirmed blackbird recordings in our database.
// The distinctive melodic whistling in the 2-4 kHz range is a classic
// blackbird signature."
```

### Template-Based Narratives

```rust
use sevensense_interpretation::{NarrativeTemplate, TemplateEngine};

let template = NarrativeTemplate::new(
    "Species: {{species_name}} ({{confidence}}% confidence). \
     Based on {{neighbor_count}} similar recordings. \
     {{#if low_confidence}}Note: Confidence is below threshold.{{/if}}"
);

let engine = TemplateEngine::new();
let narrative = engine.render(&template, &evidence)?;
```

</details>

<details>
<summary><b>Tutorial: Audit Trails</b></summary>

### Creating Audit Records

```rust
use sevensense_interpretation::{AuditTrail, AuditRecord};

let mut audit = AuditTrail::new();

// Record prediction event
audit.record(AuditRecord::Prediction {
    timestamp: Utc::now(),
    input_hash: hash(&audio_data),
    prediction: prediction.clone(),
    confidence: 0.94,
    model_version: "perch-2.0".into(),
});

// Record evidence generation
audit.record(AuditRecord::Evidence {
    timestamp: Utc::now(),
    prediction_id: prediction.id,
    evidence_pack: evidence.clone(),
});
```

### Exporting for Compliance

```rust
use sevensense_interpretation::{AuditExporter, ExportFormat};

let exporter = AuditExporter::new();

// Export to JSON
let json = exporter.export(&audit, ExportFormat::Json)?;
std::fs::write("audit_trail.json", json)?;

// Export to CSV (for spreadsheet analysis)
let csv = exporter.export(&audit, ExportFormat::Csv)?;
std::fs::write("audit_trail.csv", csv)?;

// Export to PDF report
let pdf = exporter.export(&audit, ExportFormat::Pdf)?;
std::fs::write("audit_report.pdf", pdf)?;
```

### Provenance Tracking

```rust
use sevensense_interpretation::ProvenanceTracker;

let tracker = ProvenanceTracker::new();

// Track data lineage
tracker.record_input("recording_001.wav", &audio_metadata)?;
tracker.record_processing("segmentation", &segment_config)?;
tracker.record_processing("embedding", &embedding_config)?;
tracker.record_prediction(&prediction)?;

// Generate provenance graph
let graph = tracker.to_graph()?;
println!("{}", graph.to_dot());  // GraphViz format
```

</details>

---

## Configuration

### EvidenceConfig Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `include_neighbors` | true | Include similar examples |
| `include_features` | true | Include feature attribution |
| `include_uncertainty` | true | Include uncertainty estimates |
| `narrative_style` | Scientific | Narrative style |
| `max_neighbors` | 10 | Max neighbors to include |

### ConfidenceConfig Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `neighbor_weight` | 0.4 | Neighbor agreement weight |
| `cluster_weight` | 0.3 | Cluster membership weight |
| `quality_weight` | 0.3 | Embedding quality weight |

## RAB Framework

| Component | Description | Implementation |
|-----------|-------------|----------------|
| **R**easoning | Why was this prediction made? | Feature attribution, neighbors |
| **A**ccountability | Who/what is responsible? | Audit trails, model versions |
| **B**elievability | How trustworthy is this? | Confidence, uncertainty |

## Links

- **Homepage**: [ruv.io](https://ruv.io)
- **Repository**: [github.com/ruvnet/ruvector](https://github.com/ruvnet/ruvector)
- **Crates.io**: [crates.io/crates/sevensense-interpretation](https://crates.io/crates/sevensense-interpretation)
- **Documentation**: [docs.rs/sevensense-interpretation](https://docs.rs/sevensense-interpretation)

## License

MIT License - see [LICENSE](../../LICENSE) for details.

---

*Part of the [7sense Bioacoustic Intelligence Platform](https://ruv.io) by rUv*
