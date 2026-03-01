//! Publisher implementation

use crate::error::Result;
use crate::message::Message;
use crate::serialization::{Format, Serializer};
use parking_lot::RwLock;
use std::sync::Arc;

/// Publisher for sending messages
pub struct Publisher<T: Message> {
    topic: String,
    serializer: Serializer,
    _phantom: std::marker::PhantomData<T>,
    stats: Arc<RwLock<PublisherStats>>,
}

#[derive(Debug, Default)]
struct PublisherStats {
    pub messages_sent: u64,
    pub bytes_sent: u64,
}

impl<T: Message> Publisher<T> {
    /// Create a new publisher
    pub fn new(topic: impl Into<String>) -> Self {
        Self::with_format(topic, Format::Cdr)
    }

    /// Create a new publisher with specific format
    pub fn with_format(topic: impl Into<String>, format: Format) -> Self {
        let topic = topic.into();

        Self {
            topic,
            serializer: Serializer::new(format),
            _phantom: std::marker::PhantomData,
            stats: Arc::new(RwLock::new(PublisherStats::default())),
        }
    }

    /// Publish a message
    pub async fn publish(&self, msg: &T) -> Result<()> {
        let bytes = self.serializer.serialize(msg)?;

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.messages_sent += 1;
            stats.bytes_sent += bytes.len() as u64;
        }

        // In real implementation, this would send via Zenoh
        Ok(())
    }

    /// Get topic name
    pub fn topic(&self) -> &str {
        &self.topic
    }

    /// Get statistics
    pub fn stats(&self) -> (u64, u64) {
        let stats = self.stats.read();
        (stats.messages_sent, stats.bytes_sent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::RobotState;

    #[tokio::test]
    async fn test_publisher() {
        let publisher = Publisher::<RobotState>::new("robot/state");
        let msg = RobotState::default();

        let result = publisher.publish(&msg).await;
        assert!(result.is_ok());

        let (count, bytes) = publisher.stats();
        assert_eq!(count, 1);
        assert!(bytes > 0);
    }
}
