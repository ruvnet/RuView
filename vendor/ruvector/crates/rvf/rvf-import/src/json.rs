//! JSON importer for RVF stores.
//!
//! Supports two JSON layouts:
//!
//! 1. **Array of objects** (the common case):
//!    ```json
//!    [
//!      {"id": 1, "vector": [0.1, 0.2, ...], "metadata": {"key": "value"}},
//!      {"id": 2, "vector": [0.3, 0.4, ...]}
//!    ]
//!    ```
//!
//! 2. **HNSW dump format**:
//!    ```json
//!    {
//!      "vectors": [
//!        {"id": 1, "vector": [0.1, 0.2, ...]},
//!        ...
//!      ],
//!      "graph": { ... }
//!    }
//!    ```
//!
//! The `graph` field in HNSW dumps is ignored — only vector data is imported.

use crate::VectorRecord;
use rvf_runtime::{MetadataEntry, MetadataValue};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

/// A single vector entry as it appears in JSON.
#[derive(Deserialize)]
struct JsonVectorEntry {
    id: u64,
    vector: Vec<f32>,
    #[serde(default)]
    metadata: Option<HashMap<String, serde_json::Value>>,
}

/// HNSW dump envelope.
#[derive(Deserialize)]
struct HnswDump {
    vectors: Vec<JsonVectorEntry>,
    // `graph` is intentionally ignored during import.
}

/// Intermediate deserialization target that handles both layouts.
#[derive(Deserialize)]
#[serde(untagged)]
enum JsonInput {
    Array(Vec<JsonVectorEntry>),
    HnswDump(HnswDump),
}

fn convert_metadata(map: &HashMap<String, serde_json::Value>) -> Vec<MetadataEntry> {
    let mut entries = Vec::new();
    for (i, (_key, value)) in map.iter().enumerate() {
        let field_id = i as u16;
        match value {
            serde_json::Value::Number(n) => {
                if let Some(u) = n.as_u64() {
                    entries.push(MetadataEntry {
                        field_id,
                        value: MetadataValue::U64(u),
                    });
                } else if let Some(i) = n.as_i64() {
                    entries.push(MetadataEntry {
                        field_id,
                        value: MetadataValue::I64(i),
                    });
                } else if let Some(f) = n.as_f64() {
                    entries.push(MetadataEntry {
                        field_id,
                        value: MetadataValue::F64(f),
                    });
                }
            }
            serde_json::Value::String(s) => {
                entries.push(MetadataEntry {
                    field_id,
                    value: MetadataValue::String(s.clone()),
                });
            }
            _ => {
                // Arrays, objects, bools, null — store as JSON string
                entries.push(MetadataEntry {
                    field_id,
                    value: MetadataValue::String(value.to_string()),
                });
            }
        }
    }
    entries
}

fn entries_to_records(entries: Vec<JsonVectorEntry>) -> Vec<VectorRecord> {
    entries
        .into_iter()
        .map(|e| {
            let metadata = e
                .metadata
                .as_ref()
                .map(convert_metadata)
                .unwrap_or_default();
            VectorRecord {
                id: e.id,
                vector: e.vector,
                metadata,
            }
        })
        .collect()
}

/// Parse JSON from a reader. Handles both array-of-objects and HNSW dump formats.
pub fn parse_json<R: Read>(reader: R) -> Result<Vec<VectorRecord>, String> {
    let input: JsonInput =
        serde_json::from_reader(reader).map_err(|e| format!("JSON parse error: {e}"))?;

    let entries = match input {
        JsonInput::Array(arr) => arr,
        JsonInput::HnswDump(dump) => dump.vectors,
    };

    Ok(entries_to_records(entries))
}

/// Parse JSON from a file path.
pub fn parse_json_file(path: &Path) -> Result<Vec<VectorRecord>, String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let reader = std::io::BufReader::new(file);
    parse_json(reader)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_array_format() {
        let json = r#"[
            {"id": 1, "vector": [0.1, 0.2, 0.3]},
            {"id": 2, "vector": [0.4, 0.5, 0.6], "metadata": {"category": "test", "score": 42}}
        ]"#;

        let records = parse_json(json.as_bytes()).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].id, 1);
        assert_eq!(records[0].vector, vec![0.1, 0.2, 0.3]);
        assert!(records[0].metadata.is_empty());

        assert_eq!(records[1].id, 2);
        assert_eq!(records[1].vector, vec![0.4, 0.5, 0.6]);
        assert_eq!(records[1].metadata.len(), 2);
    }

    #[test]
    fn parse_hnsw_dump_format() {
        let json = r#"{
            "vectors": [
                {"id": 10, "vector": [1.0, 2.0]},
                {"id": 20, "vector": [3.0, 4.0]}
            ],
            "graph": {"layers": 3, "nodes": []}
        }"#;

        let records = parse_json(json.as_bytes()).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].id, 10);
        assert_eq!(records[1].id, 20);
    }

    #[test]
    fn parse_empty_array() {
        let json = "[]";
        let records = parse_json(json.as_bytes()).unwrap();
        assert!(records.is_empty());
    }

    #[test]
    fn parse_invalid_json() {
        let json = "not json at all";
        let result = parse_json(json.as_bytes());
        assert!(result.is_err());
    }

    #[test]
    fn metadata_types() {
        let json = r#"[
            {"id": 1, "vector": [0.1], "metadata": {
                "name": "hello",
                "count": 99,
                "neg": -5,
                "score": 3.14
            }}
        ]"#;

        let records = parse_json(json.as_bytes()).unwrap();
        assert_eq!(records[0].metadata.len(), 4);
    }
}
