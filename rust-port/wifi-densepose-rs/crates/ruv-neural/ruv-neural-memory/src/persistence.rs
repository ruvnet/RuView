//! File-based persistence for neural memory stores.
//!
//! Supports two formats:
//! - **Bincode**: Fast binary serialization for local storage.
//! - **RVF**: RuVector File format for interoperability with the RuVector ecosystem.

use ruv_neural_core::embedding::NeuralEmbedding;
use ruv_neural_core::error::{Result, RuvNeuralError};
use ruv_neural_core::rvf::{RvfDataType, RvfFile, RvfHeader};

use serde::{Deserialize, Serialize};

use crate::store::NeuralMemoryStore;

/// Serializable representation of the store for bincode persistence.
#[derive(Serialize, Deserialize)]
struct StoreSnapshot {
    embeddings: Vec<NeuralEmbedding>,
    capacity: usize,
}

/// Save a memory store to disk using bincode serialization.
pub fn save_store(store: &NeuralMemoryStore, path: &str) -> Result<()> {
    let snapshot = StoreSnapshot {
        embeddings: store.embeddings_iter().cloned().collect(),
        capacity: store.capacity(),
    };

    let bytes = bincode::serialize(&snapshot)
        .map_err(|e| RuvNeuralError::Serialization(format!("bincode encode: {}", e)))?;

    std::fs::write(path, bytes)
        .map_err(|e| RuvNeuralError::Serialization(format!("write file: {}", e)))?;

    Ok(())
}

/// Load a memory store from a bincode file on disk.
pub fn load_store(path: &str) -> Result<NeuralMemoryStore> {
    let bytes = std::fs::read(path)
        .map_err(|e| RuvNeuralError::Serialization(format!("read file: {}", e)))?;

    let snapshot: StoreSnapshot = bincode::deserialize(&bytes)
        .map_err(|e| RuvNeuralError::Serialization(format!("bincode decode: {}", e)))?;

    let mut store = NeuralMemoryStore::new(snapshot.capacity);
    for emb in snapshot.embeddings {
        store.store(emb)?;
    }

    Ok(store)
}

/// Save a memory store in RVF (RuVector File) format.
pub fn save_rvf(store: &NeuralMemoryStore, path: &str) -> Result<()> {
    let embeddings: Vec<NeuralEmbedding> = store.embeddings_iter().cloned().collect();
    let embedding_dim = embeddings.first().map(|e| e.dimension as u32).unwrap_or(0);

    let mut rvf = RvfFile::new(RvfDataType::NeuralEmbedding);
    rvf.header = RvfHeader::new(
        RvfDataType::NeuralEmbedding,
        embeddings.len() as u64,
        embedding_dim,
    );

    // Store metadata as JSON
    let metadata = serde_json::json!({
        "format": "ruv-neural-memory",
        "version": "0.1.0",
        "num_embeddings": embeddings.len(),
        "embedding_dim": embedding_dim,
        "capacity": store.capacity(),
    });
    rvf.metadata = metadata;

    // Serialize embeddings as the binary payload
    let data = bincode::serialize(&embeddings)
        .map_err(|e| RuvNeuralError::Serialization(format!("bincode encode: {}", e)))?;
    rvf.data = data;

    let mut file = std::fs::File::create(path)
        .map_err(|e| RuvNeuralError::Serialization(format!("create file: {}", e)))?;

    rvf.write_to(&mut file)?;
    Ok(())
}

/// Load a memory store from an RVF file.
pub fn load_rvf(path: &str) -> Result<NeuralMemoryStore> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| RuvNeuralError::Serialization(format!("open file: {}", e)))?;

    let rvf = RvfFile::read_from(&mut file)?;

    // Verify data type
    if rvf.header.data_type != RvfDataType::NeuralEmbedding {
        return Err(RuvNeuralError::Serialization(format!(
            "Expected NeuralEmbedding data type, got {:?}",
            rvf.header.data_type
        )));
    }

    // Extract capacity from metadata
    let capacity = rvf
        .metadata
        .get("capacity")
        .and_then(|v| v.as_u64())
        .unwrap_or(10000) as usize;

    // Deserialize embeddings from binary payload
    let embeddings: Vec<NeuralEmbedding> = bincode::deserialize(&rvf.data)
        .map_err(|e| RuvNeuralError::Serialization(format!("bincode decode: {}", e)))?;

    let mut store = NeuralMemoryStore::new(capacity);
    for emb in embeddings {
        store.store(emb)?;
    }

    Ok(store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;
    use ruv_neural_core::embedding::EmbeddingMetadata;
    use ruv_neural_core::topology::CognitiveState;

    fn make_embedding(vector: Vec<f64>, timestamp: f64) -> NeuralEmbedding {
        NeuralEmbedding::new(
            vector,
            timestamp,
            EmbeddingMetadata {
                subject_id: Some("subj1".to_string()),
                session_id: None,
                cognitive_state: Some(CognitiveState::Focused),
                source_atlas: Atlas::Schaefer100,
                embedding_method: "spectral".to_string(),
            },
        )
        .unwrap()
    }

    #[test]
    fn bincode_round_trip() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_memory_store.bin");
        let path_str = path.to_str().unwrap();

        let mut store = NeuralMemoryStore::new(100);
        store.store(make_embedding(vec![1.0, 2.0, 3.0], 1.0)).unwrap();
        store.store(make_embedding(vec![4.0, 5.0, 6.0], 2.0)).unwrap();

        save_store(&store, path_str).unwrap();
        let loaded = load_store(path_str).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.get(0).unwrap().vector, vec![1.0, 2.0, 3.0]);
        assert_eq!(loaded.get(1).unwrap().vector, vec![4.0, 5.0, 6.0]);

        // Cleanup
        let _ = std::fs::remove_file(path_str);
    }

    #[test]
    fn rvf_round_trip() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_memory_store.rvf");
        let path_str = path.to_str().unwrap();

        let mut store = NeuralMemoryStore::new(50);
        store.store(make_embedding(vec![10.0, 20.0], 0.5)).unwrap();
        store.store(make_embedding(vec![30.0, 40.0], 1.5)).unwrap();
        store.store(make_embedding(vec![50.0, 60.0], 2.5)).unwrap();

        save_rvf(&store, path_str).unwrap();
        let loaded = load_rvf(path_str).unwrap();

        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded.get(0).unwrap().vector, vec![10.0, 20.0]);
        assert_eq!(loaded.get(2).unwrap().vector, vec![50.0, 60.0]);
        assert_eq!(loaded.capacity(), 50);

        // Cleanup
        let _ = std::fs::remove_file(path_str);
    }
}
