//! REST API module for 7sense.
//!
//! This module provides RESTful endpoints for:
//! - Recording upload and management
//! - Segment similarity search
//! - Cluster discovery and labeling
//! - Evidence pack retrieval
//!
//! ## API Versioning
//!
//! All endpoints are versioned under `/api/v1/`. Breaking changes will
//! result in a new API version (e.g., `/api/v2/`).
//!
//! ## Authentication
//!
//! If `SEVENSENSE_API_KEY` is set, all requests must include an
//! `Authorization: Bearer <api_key>` header.

pub mod handlers;
pub mod middleware;
pub mod routes;

pub use handlers::*;
pub use routes::create_router;
