//! Agentic Robotics Node.js Bindings
//!
//! NAPI bindings for Node.js/TypeScript integration with agentic-robotics-core

#![deny(clippy::all)]

use agentic_robotics_core::{Publisher, Subscriber};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Node for creating publishers and subscribers
#[napi]
pub struct AgenticNode {
    name: String,
    publishers: Arc<RwLock<HashMap<String, Arc<Publisher<JsonValue>>>>>,
    subscribers: Arc<RwLock<HashMap<String, Arc<Subscriber<JsonValue>>>>>,
}

#[napi]
impl AgenticNode {
    /// Create a new node
    #[napi(constructor)]
    pub fn new(name: String) -> Result<Self> {
        Ok(Self {
            name,
            publishers: Arc::new(RwLock::new(HashMap::new())),
            subscribers: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Get node name
    #[napi]
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    /// Create a publisher for a topic
    #[napi]
    pub async fn create_publisher(&self, topic: String) -> Result<AgenticPublisher> {
        // Use JSON format for serde_json::Value to avoid CDR serialization issues
        let publisher = Arc::new(Publisher::<JsonValue>::with_format(
            topic.clone(),
            agentic_robotics_core::serialization::Format::Json,
        ));

        let mut publishers = self.publishers.write().await;
        publishers.insert(topic.clone(), publisher.clone());

        Ok(AgenticPublisher {
            topic,
            inner: publisher,
        })
    }

    /// Create a subscriber for a topic
    #[napi]
    pub async fn create_subscriber(&self, topic: String) -> Result<AgenticSubscriber> {
        let subscriber = Arc::new(Subscriber::<JsonValue>::new(topic.clone()));

        let mut subscribers = self.subscribers.write().await;
        subscribers.insert(topic.clone(), subscriber.clone());

        Ok(AgenticSubscriber {
            topic,
            inner: subscriber,
        })
    }

    /// Get library version
    #[napi]
    pub fn get_version() -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// List all active publishers
    #[napi]
    pub async fn list_publishers(&self) -> Vec<String> {
        let publishers = self.publishers.read().await;
        publishers.keys().cloned().collect()
    }

    /// List all active subscribers
    #[napi]
    pub async fn list_subscribers(&self) -> Vec<String> {
        let subscribers = self.subscribers.read().await;
        subscribers.keys().cloned().collect()
    }
}

/// Publisher for sending messages to a topic
#[napi]
pub struct AgenticPublisher {
    topic: String,
    inner: Arc<Publisher<JsonValue>>,
}

#[napi]
impl AgenticPublisher {
    /// Publish a message (JSON string or object)
    #[napi]
    pub async fn publish(&self, data: String) -> Result<()> {
        let value: JsonValue = serde_json::from_str(&data)
            .map_err(|e| Error::from_reason(format!("Invalid JSON: {}", e)))?;

        self.inner
            .publish(&value)
            .await
            .map_err(|e| Error::from_reason(format!("Publish failed: {}", e)))?;

        Ok(())
    }

    /// Get topic name
    #[napi]
    pub fn get_topic(&self) -> String {
        self.topic.clone()
    }

    /// Get publisher statistics (messages sent, bytes sent)
    #[napi]
    pub fn get_stats(&self) -> PublisherStats {
        let (messages, bytes) = self.inner.stats();
        PublisherStats {
            messages: messages as i64,
            bytes: bytes as i64,
        }
    }
}

/// Publisher statistics
#[napi(object)]
pub struct PublisherStats {
    pub messages: i64,
    pub bytes: i64,
}

/// Subscriber for receiving messages from a topic
#[napi]
pub struct AgenticSubscriber {
    topic: String,
    inner: Arc<Subscriber<JsonValue>>,
}

#[napi]
impl AgenticSubscriber {
    /// Get topic name
    #[napi]
    pub fn get_topic(&self) -> String {
        self.topic.clone()
    }

    /// Try to receive a message immediately (non-blocking)
    #[napi]
    pub async fn try_recv(&self) -> Result<Option<String>> {
        match self.inner.try_recv() {
            Ok(Some(msg)) => {
                let json_str = serde_json::to_string(&msg)
                    .map_err(|e| Error::from_reason(format!("Serialization failed: {}", e)))?;
                Ok(Some(json_str))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(Error::from_reason(format!("Receive failed: {}", e))),
        }
    }

    /// Receive a message (blocking until message arrives)
    #[napi]
    pub async fn recv(&self) -> Result<String> {
        let msg = self
            .inner
            .recv_async()
            .await
            .map_err(|e| Error::from_reason(format!("Receive failed: {}", e)))?;

        let json_str = serde_json::to_string(&msg)
            .map_err(|e| Error::from_reason(format!("Serialization failed: {}", e)))?;

        Ok(json_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_node_creation() {
        let node = AgenticNode::new("test_node".to_string()).unwrap();
        assert_eq!(node.get_name(), "test_node");
    }

    #[tokio::test]
    async fn test_create_publisher() {
        let node = AgenticNode::new("test_node".to_string()).unwrap();
        let publisher = node.create_publisher("/test".to_string()).await.unwrap();
        assert_eq!(publisher.get_topic(), "/test");
    }

    #[tokio::test]
    async fn test_publish() {
        let node = AgenticNode::new("test_node".to_string()).unwrap();
        let publisher = node.create_publisher("/test".to_string()).await.unwrap();

        let result = publisher.publish(r#"{"message": "hello"}"#.to_string()).await;
        assert!(result.is_ok());

        let stats = publisher.get_stats();
        assert_eq!(stats.messages, 1);
    }

    #[tokio::test]
    async fn test_create_subscriber() {
        let node = AgenticNode::new("test_node".to_string()).unwrap();
        let subscriber = node.create_subscriber("/test".to_string()).await.unwrap();
        assert_eq!(subscriber.get_topic(), "/test");
    }

    #[tokio::test]
    async fn test_list_publishers() {
        let node = AgenticNode::new("test_node".to_string()).unwrap();
        node.create_publisher("/test1".to_string()).await.unwrap();
        node.create_publisher("/test2".to_string()).await.unwrap();

        let publishers = node.list_publishers().await;
        assert_eq!(publishers.len(), 2);
        assert!(publishers.contains(&"/test1".to_string()));
        assert!(publishers.contains(&"/test2".to_string()));
    }
}
