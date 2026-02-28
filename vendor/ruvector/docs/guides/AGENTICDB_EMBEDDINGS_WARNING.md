# AgenticDB Embedding Limitation - MUST READ

## ⚠️⚠️⚠️ CRITICAL WARNING ⚠️⚠️⚠️

**AgenticDB currently uses PLACEHOLDER HASH-BASED EMBEDDINGS, not real semantic embeddings.**

## What This Means

The current `generate_text_embedding()` function creates embeddings using a simple hash that does NOT understand semantic meaning:

### ❌ What DOESN'T Work
- Semantic similarity: "dog" and "cat" are NOT similar
- Synonyms: "happy" and "joyful" are NOT similar
- Related concepts: "car" and "automobile" are NOT similar
- Paraphrasing: "I like pizza" and "Pizza is my favorite" are NOT similar

### ✅ What "Works" (But Shouldn't)
- Character similarity: "dog" and "god" ARE similar (same letters)
- Typos: "teh" and "the" ARE similar (close characters)
- This is NOT semantic search - it's character overlap!

## Why This Exists

The placeholder embedding allows:
1. Testing the AgenticDB API structure
2. Demonstrating the API usage patterns
3. Running benchmarks on vector operations
4. Developing without external dependencies

**But it should NEVER be used for production semantic search.**

## Production Integration - Choose ONE

### Option 1: ONNX Runtime (Recommended ⭐)

**Best for**: Production deployments, cross-platform compatibility

```rust
use ort::{Session, Environment, Value, TensorRTExecutionProvider};
use tokenizers::Tokenizer;

pub struct OnnxEmbedder {
    session: Session,
    tokenizer: Tokenizer,
}

impl OnnxEmbedder {
    pub fn new(model_path: &str, tokenizer_path: &str) -> Result<Self> {
        let environment = Environment::builder()
            .with_name("embeddings")
            .with_execution_providers([TensorRTExecutionProvider::default().build()])
            .build()?;

        let session = Session::builder()?
            .with_optimization_level(ort::GraphOptimizationLevel::Level3)?
            .with_intra_threads(4)?
            .with_model_from_file(model_path)?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)?;

        Ok(Self { session, tokenizer })
    }

    pub fn generate_text_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let encoding = self.tokenizer.encode(text, true)?;
        let input_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();

        let input_ids_array = ndarray::Array2::from_shape_vec(
            (1, input_ids.len()),
            input_ids.iter().map(|&x| x as i64).collect()
        )?;

        let attention_mask_array = ndarray::Array2::from_shape_vec(
            (1, attention_mask.len()),
            attention_mask.iter().map(|&x| x as i64).collect()
        )?;

        let outputs = self.session.run(ort::inputs![
            "input_ids" => Value::from_array(self.session.allocator(), &input_ids_array)?,
            "attention_mask" => Value::from_array(self.session.allocator(), &attention_mask_array)?
        ])?;

        let embeddings: ort::OrtOwnedTensor<f32, _> = outputs["last_hidden_state"].try_extract()?;
        let embeddings = embeddings.view();

        // Mean pooling
        let embedding_vec = embeddings
            .mean_axis(ndarray::Axis(1))
            .unwrap()
            .to_vec();

        Ok(embedding_vec)
    }
}

// Replace AgenticDB's generate_text_embedding:
// 1. Add OnnxEmbedder field to AgenticDB struct
// 2. Initialize in new()
// 3. Call embedder.generate_text_embedding(text) instead of hash
```

**Models to use**:
- `all-MiniLM-L6-v2` (384 dims, fast, good quality)
- `all-mpnet-base-v2` (768 dims, higher quality)
- `gte-small` (384 dims, multilingual)

**Get ONNX models**:
```bash
python -m pip install optimum[onnxruntime]
optimum-cli export onnx --model sentence-transformers/all-MiniLM-L6-v2 all-MiniLM-L6-v2-onnx/
```

---

### Option 2: Candle (Pure Rust)

**Best for**: Native Rust deployments, no Python dependencies

```rust
use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config as BertConfig};

pub struct CandleEmbedder {
    model: BertModel,
    tokenizer: tokenizers::Tokenizer,
    device: Device,
}

impl CandleEmbedder {
    pub fn new(model_path: &str, tokenizer_path: &str) -> Result<Self> {
        let device = Device::cuda_if_available(0)?;

        let config = BertConfig::default();
        let vb = VarBuilder::from_pth(model_path, candle_core::DType::F32, &device)?;
        let model = BertModel::load(vb, &config)?;

        let tokenizer = tokenizers::Tokenizer::from_file(tokenizer_path)?;

        Ok(Self { model, tokenizer, device })
    }

    pub fn generate_text_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let encoding = self.tokenizer.encode(text, true)?;

        let input_ids = Tensor::new(
            encoding.get_ids(),
            &self.device
        )?.unsqueeze(0)?;

        let token_type_ids = Tensor::zeros(
            (1, encoding.get_ids().len()),
            candle_core::DType::U32,
            &self.device
        )?;

        let embeddings = self.model.forward(&input_ids, &token_type_ids)?;

        // Mean pooling
        let embedding_vec = embeddings
            .mean(1)?
            .to_vec1::<f32>()?;

        Ok(embedding_vec)
    }
}
```

**Dependencies**:
```toml
[dependencies]
candle-core = "0.3"
candle-nn = "0.3"
candle-transformers = "0.3"
```

---

### Option 3: API-based (OpenAI, Cohere, Anthropic)

**Best for**: Quick prototyping, cloud deployments

#### OpenAI

```rust
use reqwest;
use serde_json::json;

pub struct OpenAIEmbedder {
    client: reqwest::Client,
    api_key: String,
}

impl OpenAIEmbedder {
    pub fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
        }
    }

    pub async fn generate_text_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let response = self.client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&json!({
                "model": "text-embedding-3-small",
                "input": text,
            }))
            .send()
            .await?;

        let json: serde_json::Value = response.json().await?;
        let embeddings = json["data"][0]["embedding"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap() as f32)
            .collect();

        Ok(embeddings)
    }
}
```

**Costs** (as of 2024):
- `text-embedding-3-small`: $0.02 / 1M tokens
- `text-embedding-3-large`: $0.13 / 1M tokens

#### Cohere

```rust
pub async fn generate_text_embedding(&self, text: &str) -> Result<Vec<f32>> {
    let response = self.client
        .post("https://api.cohere.ai/v1/embed")
        .header("Authorization", format!("Bearer {}", self.api_key))
        .json(&json!({
            "model": "embed-english-v3.0",
            "texts": [text],
            "input_type": "search_query",
        }))
        .send()
        .await?;

    let json: serde_json::Value = response.json().await?;
    let embeddings = json["embeddings"][0]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap() as f32)
        .collect();

    Ok(embeddings)
}
```

**Costs**: $0.10 / 1M tokens

---

### Option 4: Python Bindings (sentence-transformers)

**Best for**: Leveraging existing Python ML ecosystem

```rust
use pyo3::prelude::*;
use pyo3::types::PyModule;
use numpy::PyArray1;

pub struct PythonEmbedder {
    model: Py<PyAny>,
}

impl PythonEmbedder {
    pub fn new(model_name: &str) -> Result<Self> {
        Python::with_gil(|py| {
            let sentence_transformers = PyModule::import(py, "sentence_transformers")?;
            let model = sentence_transformers
                .getattr("SentenceTransformer")?
                .call1((model_name,))?;

            Ok(Self {
                model: model.into(),
            })
        })
    }

    pub fn generate_text_embedding(&self, text: &str) -> Result<Vec<f32>> {
        Python::with_gil(|py| {
            let embeddings = self.model
                .call_method1(py, "encode", (text,))?
                .extract::<&PyArray1<f32>>(py)?;

            Ok(embeddings.to_vec()?)
        })
    }
}
```

**Dependencies**:
```toml
[dependencies]
pyo3 = { version = "0.20", features = ["extension-module"] }
numpy = "0.20"
```

**Python setup**:
```bash
pip install sentence-transformers
```

---

## Integration Steps

### 1. Choose Your Approach

Pick one of the 4 options above based on your requirements:
- **ONNX**: Best balance of performance and compatibility ⭐
- **Candle**: Pure Rust, no external runtime
- **API**: Fastest to prototype, pay per use
- **Python**: Maximum flexibility with ML libraries

### 2. Update AgenticDB Struct

```rust
pub struct AgenticDB {
    vector_db: Arc<VectorDB>,
    db: Arc<Database>,
    dimensions: usize,
    embedder: Arc<dyn Embedder>, // Add this
}
```

### 3. Create Embedder Trait

```rust
pub trait Embedder: Send + Sync {
    fn generate_text_embedding(&self, text: &str) -> Result<Vec<f32>>;
}

// Implement for each option:
impl Embedder for OnnxEmbedder { /* ... */ }
impl Embedder for CandleEmbedder { /* ... */ }
impl Embedder for OpenAIEmbedder { /* ... */ }
impl Embedder for PythonEmbedder { /* ... */ }
```

### 4. Update Constructor

```rust
impl AgenticDB {
    pub fn new(options: DbOptions, embedder: Arc<dyn Embedder>) -> Result<Self> {
        // ... existing code ...
        Ok(Self {
            vector_db,
            db,
            dimensions: options.dimensions,
            embedder, // Use provided embedder
        })
    }
}
```

### 5. Replace Hash Implementation

```rust
fn generate_text_embedding(&self, text: &str) -> Result<Vec<f32>> {
    self.embedder.generate_text_embedding(text)
}
```

### 6. Update Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct MockEmbedder;
    impl Embedder for MockEmbedder {
        fn generate_text_embedding(&self, text: &str) -> Result<Vec<f32>> {
            // Use hash for tests only
            // ... hash implementation ...
        }
    }

    fn create_test_db() -> Result<AgenticDB> {
        let embedder = Arc::new(MockEmbedder);
        AgenticDB::new(options, embedder)
    }
}
```

---

## Verification

After integration, verify semantic search works:

```rust
#[test]
fn test_semantic_similarity() {
    let db = create_db_with_real_embeddings()?;

    // These should be similar with real embeddings
    let skill1 = db.create_skill(
        "Dog Handler".to_string(),
        "Take care of dogs".to_string(),
        HashMap::new(),
        vec![],
    )?;

    let skill2 = db.create_skill(
        "Cat Handler".to_string(),
        "Take care of cats".to_string(),
        HashMap::new(),
        vec![],
    )?;

    // Search with semantic query
    let results = db.search_skills("pet care", 5)?;

    // Both should be found because "pet care" is semantically similar
    // to both "take care of dogs" and "take care of cats"
    assert!(results.len() >= 2);

    // With hash embeddings, this would likely fail!
}
```

---

## Performance Considerations

| Method | Latency | Cost | Offline | Quality |
|--------|---------|------|---------|---------|
| **ONNX** | ~5-20ms | Free | ✅ | ⭐⭐⭐⭐ |
| **Candle** | ~10-30ms | Free | ✅ | ⭐⭐⭐⭐ |
| **OpenAI API** | ~100-300ms | $0.02/1M tokens | ❌ | ⭐⭐⭐⭐⭐ |
| **Cohere API** | ~100-300ms | $0.10/1M tokens | ❌ | ⭐⭐⭐⭐ |
| **Python** | ~5-20ms | Free | ✅ | ⭐⭐⭐⭐ |
| **Hash (current)** | ~0.1ms | Free | ✅ | ❌ |

---

## Feature Flag (Future)

We plan to add a compile-time check:

```rust
#[cfg(not(feature = "real-embeddings"))]
compile_error!(
    "AgenticDB requires 'real-embeddings' feature for production use. \
     Current placeholder embeddings do NOT provide semantic search. \
     Enable with: cargo build --features real-embeddings"
);
```

---

## Conclusion

**DO NOT use the current AgenticDB implementation for semantic search in production.**

The placeholder embeddings are ONLY suitable for:
- API structure testing
- Performance benchmarking (vector operations)
- Development without external dependencies

For any real semantic search use case, integrate one of the four real embedding options above.

**See `/examples/onnx-embeddings` for a complete ONNX integration example.**
