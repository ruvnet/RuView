//! Service and RPC implementation

use crate::error::{Error, Result};
use crate::message::Message;
use parking_lot::RwLock;
use std::sync::Arc;
use tracing::debug;

/// Service request handler
pub type ServiceHandler<Req, Res> =
    Arc<dyn Fn(Req) -> Result<Res> + Send + Sync + 'static>;

/// Queryable service (RPC)
pub struct Queryable<Req: Message, Res: Message> {
    name: String,
    handler: ServiceHandler<Req, Res>,
    stats: Arc<RwLock<ServiceStats>>,
}

#[derive(Debug, Default)]
struct ServiceStats {
    pub requests_handled: u64,
    pub errors: u64,
}

impl<Req: Message, Res: Message> Queryable<Req, Res> {
    /// Create a new queryable service
    pub fn new<F>(name: impl Into<String>, handler: F) -> Self
    where
        F: Fn(Req) -> Result<Res> + Send + Sync + 'static,
    {
        let name = name.into();
        debug!("Creating queryable service: {}", name);

        Self {
            name,
            handler: Arc::new(handler),
            stats: Arc::new(RwLock::new(ServiceStats::default())),
        }
    }

    /// Handle a request
    pub async fn handle(&self, request: Req) -> Result<Res> {
        let result = (self.handler)(request);

        let mut stats = self.stats.write();
        stats.requests_handled += 1;
        if result.is_err() {
            stats.errors += 1;
        }

        result
    }

    /// Get service name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get statistics
    pub fn stats(&self) -> (u64, u64) {
        let stats = self.stats.read();
        (stats.requests_handled, stats.errors)
    }
}

/// Service client
pub struct Service<Req: Message, Res: Message> {
    name: String,
    _phantom: std::marker::PhantomData<(Req, Res)>,
}

impl<Req: Message, Res: Message> Service<Req, Res> {
    /// Create a new service client
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        debug!("Creating service client: {}", name);

        Self {
            name,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Call the service
    pub async fn call(&self, _request: Req) -> Result<Res> {
        // In real implementation, this would call via Zenoh
        Err(Error::Other(anyhow::anyhow!("Service call not implemented")))
    }

    /// Get service name
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::RobotState;

    #[tokio::test]
    async fn test_queryable() {
        let queryable = Queryable::new("compute", |req: RobotState| {
            Ok(RobotState {
                position: req.position,
                velocity: [1.0, 2.0, 3.0],
                timestamp: req.timestamp + 1,
            })
        });

        let request = RobotState::default();
        let response = queryable.handle(request).await.unwrap();

        assert_eq!(response.velocity, [1.0, 2.0, 3.0]);

        let (handled, errors) = queryable.stats();
        assert_eq!(handled, 1);
        assert_eq!(errors, 0);
    }

    #[test]
    fn test_service_client() {
        let service = Service::<RobotState, RobotState>::new("compute");
        assert_eq!(service.name(), "compute");
    }
}
