# ruv-neural-decoder

Cognitive state classification and BCI decoding from neural topology embeddings.

## Overview

`ruv-neural-decoder` classifies cognitive states from brain graph embeddings and
topology metrics. It provides multiple decoding strategies -- KNN classification
from labeled exemplars, threshold-based rule systems, temporal transition detection,
and clinical biomarker scoring -- plus an ensemble pipeline that combines all
strategies for robust real-time brain-computer interface (BCI) output.

## Features

- **KNN decoder** (`knn_decoder`): K-nearest neighbor classification using stored
  labeled embeddings from `ruv-neural-memory`; supports configurable k and distance
  metrics
- **Threshold decoder** (`threshold_decoder`): Rule-based classification from
  topology metric ranges (mincut value, modularity, efficiency, Fiedler value)
  with configurable `TopologyThreshold` bounds per cognitive state
- **Transition decoder** (`transition_decoder`): Detects cognitive state transitions
  from temporal topology dynamics; outputs `StateTransition` events matching
  known `TransitionPattern` templates
- **Clinical scorer** (`clinical`): `ClinicalScorer` for biomarker detection via
  deviation from healthy baseline distributions; flags abnormal topology patterns
- **Ensemble pipeline** (`pipeline`): `DecoderPipeline` combining all decoder
  strategies with confidence-weighted voting; produces `DecoderOutput` with
  classified state, confidence score, and contributing decoder votes

## Usage

```rust
use ruv_neural_decoder::{
    KnnDecoder, ThresholdDecoder, TopologyThreshold,
    TransitionDecoder, ClinicalScorer, DecoderPipeline, DecoderOutput,
};
use ruv_neural_core::topology::{CognitiveState, TopologyMetrics};

// Threshold-based decoding from topology metrics
let mut decoder = ThresholdDecoder::new();
decoder.add_threshold(TopologyThreshold {
    state: CognitiveState::Focused,
    min_modularity: 0.3,
    max_modularity: 0.5,
    min_efficiency: 0.6,
    ..Default::default()
});
let state = decoder.decode(&metrics);

// KNN-based decoding from embeddings
let mut knn = KnnDecoder::new(5); // k=5
knn.add_exemplar(embedding, CognitiveState::Rest);
let predicted = knn.classify(&query_embedding);

// Transition detection from temporal sequences
let mut transition_decoder = TransitionDecoder::new();
if let Some(transition) = transition_decoder.check(&current_metrics) {
    println!("Transition: {:?} -> {:?}", transition.from, transition.to);
}

// Full ensemble pipeline
let mut pipeline = DecoderPipeline::new();
let output: DecoderOutput = pipeline.decode(&metrics, &embedding);
println!("State: {:?}, confidence: {:.2}", output.state, output.confidence);
```

## API Reference

| Module               | Key Types                                                  |
|----------------------|------------------------------------------------------------|
| `knn_decoder`        | `KnnDecoder`                                               |
| `threshold_decoder`  | `ThresholdDecoder`, `TopologyThreshold`                    |
| `transition_decoder` | `TransitionDecoder`, `StateTransition`, `TransitionPattern`|
| `clinical`           | `ClinicalScorer`                                           |
| `pipeline`           | `DecoderPipeline`, `DecoderOutput`                         |

## Feature Flags

| Feature | Default | Description                      |
|---------|---------|----------------------------------|
| `std`   | Yes     | Standard library support         |
| `wasm`  | No      | WASM-compatible decoding         |

## Integration

Depends on `ruv-neural-core` for `CognitiveState`, `TopologyMetrics`, and
`NeuralEmbedding` types. Consumes embeddings from `ruv-neural-embed` and
topology results from `ruv-neural-mincut`. The KNN decoder can query stored
exemplars from `ruv-neural-memory`.

## License

MIT OR Apache-2.0
