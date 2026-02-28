//! Subscriber implementation

use crate::error::{Error, Result};
use crate::message::Message;
use crossbeam::channel::{self, Receiver, Sender};
use std::sync::Arc;
use tracing::debug;

/// Subscriber for receiving messages
pub struct Subscriber<T: Message> {
    topic: String,
    receiver: Receiver<T>,
    _sender: Arc<Sender<T>>, // Keep sender alive
}

impl<T: Message> Subscriber<T> {
    /// Create a new subscriber
    pub fn new(topic: impl Into<String>) -> Self {
        let topic = topic.into();
        debug!("Creating subscriber for topic: {}", topic);

        let (sender, receiver) = channel::unbounded();

        Self {
            topic,
            receiver,
            _sender: Arc::new(sender),
        }
    }

    /// Receive a message (blocking)
    pub fn recv(&self) -> Result<T> {
        self.receiver
            .recv()
            .map_err(|e| Error::Other(e.into()))
    }

    /// Try to receive a message (non-blocking)
    pub fn try_recv(&self) -> Result<Option<T>> {
        match self.receiver.try_recv() {
            Ok(msg) => Ok(Some(msg)),
            Err(crossbeam::channel::TryRecvError::Empty) => Ok(None),
            Err(e) => Err(Error::Other(e.into())),
        }
    }

    /// Receive a message asynchronously
    pub async fn recv_async(&self) -> Result<T> {
        let receiver = self.receiver.clone();
        tokio::task::spawn_blocking(move || {
            receiver.recv()
        })
        .await
        .map_err(|e| Error::Other(e.into()))?
        .map_err(|e| Error::Other(e.into()))
    }

    /// Get topic name
    pub fn topic(&self) -> &str {
        &self.topic
    }
}

impl<T: Message> Clone for Subscriber<T> {
    fn clone(&self) -> Self {
        Self {
            topic: self.topic.clone(),
            receiver: self.receiver.clone(),
            _sender: self._sender.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::RobotState;

    #[test]
    fn test_subscriber_creation() {
        let subscriber = Subscriber::<RobotState>::new("robot/state");
        assert_eq!(subscriber.topic(), "robot/state");
    }

    #[test]
    fn test_subscriber_try_recv() {
        let subscriber = Subscriber::<RobotState>::new("robot/state");
        let result = subscriber.try_recv().unwrap();
        assert!(result.is_none());
    }
}
