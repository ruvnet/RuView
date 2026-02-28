pub mod compute;
pub mod benchmark;
pub mod convert;
pub mod serve;
pub mod repl;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputData {
    pub query: Vec<Vec<f32>>,
    pub keys: Vec<Vec<f32>>,
    pub values: Vec<Vec<f32>>,
    pub dim: usize,
}

impl InputData {
    pub fn keys_refs(&self) -> Vec<&[f32]> {
        self.keys.iter().map(|k| k.as_slice()).collect()
    }

    pub fn values_refs(&self) -> Vec<&[f32]> {
        self.values.iter().map(|v| v.as_slice()).collect()
    }
}

pub fn load_input(path: &std::path::Path) -> anyhow::Result<InputData> {
    let content = std::fs::read(path)?;

    // Try to parse as JSON first
    if let Ok(data) = serde_json::from_slice::<InputData>(&content) {
        return Ok(data);
    }

    // Try MessagePack
    if let Ok(data) = rmp_serde::from_slice::<InputData>(&content) {
        return Ok(data);
    }

    // Try bincode
    if let Ok(data) = bincode::deserialize::<InputData>(&content) {
        return Ok(data);
    }

    Err(anyhow::anyhow!("Failed to parse input file"))
}

pub fn save_output(path: &std::path::Path, data: &[Vec<f32>], format: &str) -> anyhow::Result<()> {
    match format {
        "json" => {
            let json = serde_json::to_string_pretty(data)?;
            std::fs::write(path, json)?;
        }
        "msgpack" => {
            let bytes = rmp_serde::to_vec(data)?;
            std::fs::write(path, bytes)?;
        }
        "binary" => {
            let bytes = bincode::serialize(data)?;
            std::fs::write(path, bytes)?;
        }
        _ => return Err(anyhow::anyhow!("Unsupported format: {}", format)),
    }

    Ok(())
}
