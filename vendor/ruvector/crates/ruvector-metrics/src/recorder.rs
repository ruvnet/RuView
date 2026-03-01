use crate::{
    COLLECTIONS_TOTAL, DELETE_REQUESTS_TOTAL, INSERT_LATENCY_SECONDS, INSERT_REQUESTS_TOTAL,
    MEMORY_USAGE_BYTES, SEARCH_LATENCY_SECONDS, SEARCH_REQUESTS_TOTAL, VECTORS_INSERTED_TOTAL,
    VECTORS_TOTAL,
};

/// Helper struct for recording metrics
pub struct MetricsRecorder;

impl MetricsRecorder {
    /// Record a search operation
    ///
    /// # Arguments
    /// * `collection` - The collection name
    /// * `latency_secs` - The latency in seconds
    /// * `success` - Whether the operation succeeded
    pub fn record_search(collection: &str, latency_secs: f64, success: bool) {
        let status = if success { "success" } else { "error" };

        SEARCH_REQUESTS_TOTAL
            .with_label_values(&[collection, status])
            .inc();

        if success {
            SEARCH_LATENCY_SECONDS
                .with_label_values(&[collection])
                .observe(latency_secs);
        }
    }

    /// Record an insert operation
    ///
    /// # Arguments
    /// * `collection` - The collection name
    /// * `latency_secs` - The latency in seconds
    /// * `count` - The number of vectors inserted
    /// * `success` - Whether the operation succeeded
    pub fn record_insert(collection: &str, latency_secs: f64, count: usize, success: bool) {
        let status = if success { "success" } else { "error" };

        INSERT_REQUESTS_TOTAL
            .with_label_values(&[collection, status])
            .inc();

        if success {
            INSERT_LATENCY_SECONDS
                .with_label_values(&[collection])
                .observe(latency_secs);

            VECTORS_INSERTED_TOTAL
                .with_label_values(&[collection])
                .inc_by(count as f64);
        }
    }

    /// Record a delete operation
    ///
    /// # Arguments
    /// * `collection` - The collection name
    /// * `success` - Whether the operation succeeded
    pub fn record_delete(collection: &str, success: bool) {
        let status = if success { "success" } else { "error" };

        DELETE_REQUESTS_TOTAL
            .with_label_values(&[collection, status])
            .inc();
    }

    /// Update the total vector count for a collection
    ///
    /// # Arguments
    /// * `collection` - The collection name
    /// * `count` - The current number of vectors
    pub fn set_vectors_count(collection: &str, count: usize) {
        VECTORS_TOTAL
            .with_label_values(&[collection])
            .set(count as f64);
    }

    /// Update the total number of collections
    ///
    /// # Arguments
    /// * `count` - The current number of collections
    pub fn set_collections_count(count: usize) {
        COLLECTIONS_TOTAL.set(count as f64);
    }

    /// Update memory usage
    ///
    /// # Arguments
    /// * `bytes` - The current memory usage in bytes
    pub fn set_memory_usage(bytes: usize) {
        MEMORY_USAGE_BYTES.set(bytes as f64);
    }

    /// Record a batch of operations
    ///
    /// # Arguments
    /// * `collection` - The collection name
    /// * `searches` - Number of search operations
    /// * `inserts` - Number of insert operations
    /// * `deletes` - Number of delete operations
    pub fn record_batch(collection: &str, searches: usize, inserts: usize, deletes: usize) {
        if searches > 0 {
            SEARCH_REQUESTS_TOTAL
                .with_label_values(&[collection, "success"])
                .inc_by(searches as f64);
        }

        if inserts > 0 {
            INSERT_REQUESTS_TOTAL
                .with_label_values(&[collection, "success"])
                .inc_by(inserts as f64);
        }

        if deletes > 0 {
            DELETE_REQUESTS_TOTAL
                .with_label_values(&[collection, "success"])
                .inc_by(deletes as f64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_search_success() {
        MetricsRecorder::record_search("test", 0.001, true);
        // Metrics are recorded, no panic
    }

    #[test]
    fn test_record_search_failure() {
        MetricsRecorder::record_search("test", 0.001, false);
        // Metrics are recorded, no panic
    }

    #[test]
    fn test_record_insert() {
        MetricsRecorder::record_insert("test", 0.002, 10, true);
        // Metrics are recorded, no panic
    }

    #[test]
    fn test_record_delete() {
        MetricsRecorder::record_delete("test", true);
        // Metrics are recorded, no panic
    }

    #[test]
    fn test_set_vectors_count() {
        MetricsRecorder::set_vectors_count("test", 1000);
        // Metrics are recorded, no panic
    }

    #[test]
    fn test_set_collections_count() {
        MetricsRecorder::set_collections_count(5);
        // Metrics are recorded, no panic
    }

    #[test]
    fn test_set_memory_usage() {
        MetricsRecorder::set_memory_usage(1024 * 1024);
        // Metrics are recorded, no panic
    }

    #[test]
    fn test_record_batch() {
        MetricsRecorder::record_batch("test", 100, 50, 10);
        // Metrics are recorded, no panic
    }
}
