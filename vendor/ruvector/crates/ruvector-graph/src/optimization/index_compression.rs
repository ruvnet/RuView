//! Compressed index structures for massive space savings
//!
//! This module provides:
//! - Roaring bitmaps for label indexes
//! - Delta encoding for sorted ID lists
//! - Dictionary encoding for string properties

use parking_lot::RwLock;
use roaring::RoaringBitmap;
use std::collections::HashMap;
use std::sync::Arc;

/// Compressed index using multiple encoding strategies
pub struct CompressedIndex {
    /// Bitmap indexes for labels
    label_indexes: Arc<RwLock<HashMap<String, RoaringBitmap>>>,
    /// Delta-encoded sorted ID lists
    sorted_indexes: Arc<RwLock<HashMap<String, DeltaEncodedList>>>,
    /// Dictionary encoding for string properties
    string_dict: Arc<RwLock<StringDictionary>>,
}

impl CompressedIndex {
    pub fn new() -> Self {
        Self {
            label_indexes: Arc::new(RwLock::new(HashMap::new())),
            sorted_indexes: Arc::new(RwLock::new(HashMap::new())),
            string_dict: Arc::new(RwLock::new(StringDictionary::new())),
        }
    }

    /// Add node to label index
    pub fn add_to_label_index(&self, label: &str, node_id: u64) {
        let mut indexes = self.label_indexes.write();
        indexes
            .entry(label.to_string())
            .or_insert_with(RoaringBitmap::new)
            .insert(node_id as u32);
    }

    /// Get all nodes with a specific label
    pub fn get_nodes_by_label(&self, label: &str) -> Vec<u64> {
        self.label_indexes
            .read()
            .get(label)
            .map(|bitmap| bitmap.iter().map(|id| id as u64).collect())
            .unwrap_or_default()
    }

    /// Check if node has label (fast bitmap lookup)
    pub fn has_label(&self, label: &str, node_id: u64) -> bool {
        self.label_indexes
            .read()
            .get(label)
            .map(|bitmap| bitmap.contains(node_id as u32))
            .unwrap_or(false)
    }

    /// Count nodes with label
    pub fn count_label(&self, label: &str) -> u64 {
        self.label_indexes
            .read()
            .get(label)
            .map(|bitmap| bitmap.len())
            .unwrap_or(0)
    }

    /// Intersect multiple labels (efficient bitmap AND)
    pub fn intersect_labels(&self, labels: &[&str]) -> Vec<u64> {
        let indexes = self.label_indexes.read();

        if labels.is_empty() {
            return Vec::new();
        }

        let mut result = indexes
            .get(labels[0])
            .cloned()
            .unwrap_or_else(RoaringBitmap::new);

        for &label in &labels[1..] {
            if let Some(bitmap) = indexes.get(label) {
                result &= bitmap;
            } else {
                return Vec::new();
            }
        }

        result.iter().map(|id| id as u64).collect()
    }

    /// Union multiple labels (efficient bitmap OR)
    pub fn union_labels(&self, labels: &[&str]) -> Vec<u64> {
        let indexes = self.label_indexes.read();
        let mut result = RoaringBitmap::new();

        for &label in labels {
            if let Some(bitmap) = indexes.get(label) {
                result |= bitmap;
            }
        }

        result.iter().map(|id| id as u64).collect()
    }

    /// Encode string using dictionary
    pub fn encode_string(&self, s: &str) -> u32 {
        self.string_dict.write().encode(s)
    }

    /// Decode string from dictionary
    pub fn decode_string(&self, id: u32) -> Option<String> {
        self.string_dict.read().decode(id)
    }
}

impl Default for CompressedIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Roaring bitmap index for efficient set operations
pub struct RoaringBitmapIndex {
    bitmap: RoaringBitmap,
}

impl RoaringBitmapIndex {
    pub fn new() -> Self {
        Self {
            bitmap: RoaringBitmap::new(),
        }
    }

    pub fn insert(&mut self, id: u64) {
        self.bitmap.insert(id as u32);
    }

    pub fn contains(&self, id: u64) -> bool {
        self.bitmap.contains(id as u32)
    }

    pub fn remove(&mut self, id: u64) {
        self.bitmap.remove(id as u32);
    }

    pub fn len(&self) -> u64 {
        self.bitmap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bitmap.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = u64> + '_ {
        self.bitmap.iter().map(|id| id as u64)
    }

    /// Intersect with another bitmap
    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            bitmap: &self.bitmap & &other.bitmap,
        }
    }

    /// Union with another bitmap
    pub fn union(&self, other: &Self) -> Self {
        Self {
            bitmap: &self.bitmap | &other.bitmap,
        }
    }

    /// Serialize to bytes
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.bitmap
            .serialize_into(&mut bytes)
            .expect("Failed to serialize bitmap");
        bytes
    }

    /// Deserialize from bytes
    pub fn deserialize(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let bitmap = RoaringBitmap::deserialize_from(bytes)?;
        Ok(Self { bitmap })
    }
}

impl Default for RoaringBitmapIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Delta encoding for sorted ID lists
/// Stores differences between consecutive IDs for better compression
pub struct DeltaEncodedList {
    /// Base value (first ID)
    base: u64,
    /// Delta values
    deltas: Vec<u32>,
}

impl DeltaEncodedList {
    pub fn new() -> Self {
        Self {
            base: 0,
            deltas: Vec::new(),
        }
    }

    /// Encode a sorted list of IDs
    pub fn encode(ids: &[u64]) -> Self {
        if ids.is_empty() {
            return Self::new();
        }

        let base = ids[0];
        let deltas = ids
            .windows(2)
            .map(|pair| (pair[1] - pair[0]) as u32)
            .collect();

        Self { base, deltas }
    }

    /// Decode to original ID list
    pub fn decode(&self) -> Vec<u64> {
        if self.deltas.is_empty() {
            if self.base == 0 {
                return Vec::new();
            }
            return vec![self.base];
        }

        let mut result = Vec::with_capacity(self.deltas.len() + 1);
        result.push(self.base);

        let mut current = self.base;
        for &delta in &self.deltas {
            current += delta as u64;
            result.push(current);
        }

        result
    }

    /// Get compression ratio
    pub fn compression_ratio(&self) -> f64 {
        let original_size = (self.deltas.len() + 1) * 8; // u64s
        let compressed_size = 8 + self.deltas.len() * 4; // base + u32 deltas
        original_size as f64 / compressed_size as f64
    }
}

impl Default for DeltaEncodedList {
    fn default() -> Self {
        Self::new()
    }
}

/// Delta encoder utility
pub struct DeltaEncoder;

impl DeltaEncoder {
    /// Encode sorted u64 slice to delta-encoded format
    pub fn encode(values: &[u64]) -> Vec<u8> {
        if values.is_empty() {
            return Vec::new();
        }

        let mut result = Vec::new();

        // Write base value
        result.extend_from_slice(&values[0].to_le_bytes());

        // Write deltas
        for window in values.windows(2) {
            let delta = (window[1] - window[0]) as u32;
            result.extend_from_slice(&delta.to_le_bytes());
        }

        result
    }

    /// Decode delta-encoded format back to u64 values
    pub fn decode(bytes: &[u8]) -> Vec<u64> {
        if bytes.len() < 8 {
            return Vec::new();
        }

        let mut result = Vec::new();

        // Read base value
        let base = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        result.push(base);

        // Read deltas
        let mut current = base;
        for chunk in bytes[8..].chunks(4) {
            if chunk.len() == 4 {
                let delta = u32::from_le_bytes(chunk.try_into().unwrap());
                current += delta as u64;
                result.push(current);
            }
        }

        result
    }
}

/// String dictionary for deduplication and compression
struct StringDictionary {
    /// String to ID mapping
    string_to_id: HashMap<String, u32>,
    /// ID to string mapping
    id_to_string: HashMap<u32, String>,
    /// Next available ID
    next_id: u32,
}

impl StringDictionary {
    fn new() -> Self {
        Self {
            string_to_id: HashMap::new(),
            id_to_string: HashMap::new(),
            next_id: 0,
        }
    }

    fn encode(&mut self, s: &str) -> u32 {
        if let Some(&id) = self.string_to_id.get(s) {
            return id;
        }

        let id = self.next_id;
        self.next_id += 1;

        self.string_to_id.insert(s.to_string(), id);
        self.id_to_string.insert(id, s.to_string());

        id
    }

    fn decode(&self, id: u32) -> Option<String> {
        self.id_to_string.get(&id).cloned()
    }

    fn len(&self) -> usize {
        self.string_to_id.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compressed_index() {
        let index = CompressedIndex::new();

        index.add_to_label_index("Person", 1);
        index.add_to_label_index("Person", 2);
        index.add_to_label_index("Person", 3);
        index.add_to_label_index("Employee", 2);
        index.add_to_label_index("Employee", 3);

        let persons = index.get_nodes_by_label("Person");
        assert_eq!(persons.len(), 3);

        let intersection = index.intersect_labels(&["Person", "Employee"]);
        assert_eq!(intersection.len(), 2);

        let union = index.union_labels(&["Person", "Employee"]);
        assert_eq!(union.len(), 3);
    }

    #[test]
    fn test_roaring_bitmap() {
        let mut bitmap = RoaringBitmapIndex::new();

        bitmap.insert(1);
        bitmap.insert(100);
        bitmap.insert(1000);

        assert!(bitmap.contains(1));
        assert!(bitmap.contains(100));
        assert!(!bitmap.contains(50));

        assert_eq!(bitmap.len(), 3);
    }

    #[test]
    fn test_delta_encoding() {
        let ids = vec![100, 102, 105, 110, 120];
        let encoded = DeltaEncodedList::encode(&ids);
        let decoded = encoded.decode();

        assert_eq!(ids, decoded);
        assert!(encoded.compression_ratio() > 1.0);
    }

    #[test]
    fn test_delta_encoder() {
        let values = vec![1000, 1005, 1010, 1020, 1030];
        let encoded = DeltaEncoder::encode(&values);
        let decoded = DeltaEncoder::decode(&encoded);

        assert_eq!(values, decoded);

        // Encoded size should be smaller
        assert!(encoded.len() < values.len() * 8);
    }

    #[test]
    fn test_string_dictionary() {
        let index = CompressedIndex::new();

        let id1 = index.encode_string("hello");
        let id2 = index.encode_string("world");
        let id3 = index.encode_string("hello"); // Duplicate

        assert_eq!(id1, id3); // Same string gets same ID
        assert_ne!(id1, id2);

        assert_eq!(index.decode_string(id1), Some("hello".to_string()));
        assert_eq!(index.decode_string(id2), Some("world".to_string()));
    }
}
