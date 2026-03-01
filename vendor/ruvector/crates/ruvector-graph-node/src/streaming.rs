//! Streaming query results using AsyncIterator pattern

use crate::types::*;
use futures::stream::Stream;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Streaming query result iterator
#[napi]
pub struct QueryResultStream {
    inner: Pin<Box<dyn Stream<Item = JsQueryResult> + Send>>,
}

impl QueryResultStream {
    /// Create a new query result stream
    pub fn new(stream: Pin<Box<dyn Stream<Item = JsQueryResult> + Send>>) -> Self {
        Self { inner: stream }
    }
}

#[napi]
impl QueryResultStream {
    /// Get the next result from the stream
    ///
    /// # Example
    /// ```javascript
    /// const stream = await db.queryStream('MATCH (n) RETURN n');
    /// while (true) {
    ///   const result = await stream.next();
    ///   if (!result) break;
    ///   console.log(result);
    /// }
    /// ```
    #[napi]
    pub fn next(&mut self) -> Result<Option<JsQueryResult>> {
        // This would poll the stream in a real implementation
        Ok(None)
    }
}

/// Streaming hyperedge result iterator
#[napi]
pub struct HyperedgeStream {
    results: Vec<JsHyperedgeResult>,
    index: usize,
}

impl HyperedgeStream {
    /// Create a new hyperedge stream
    pub fn new(results: Vec<JsHyperedgeResult>) -> Self {
        Self { results, index: 0 }
    }
}

#[napi]
impl HyperedgeStream {
    /// Get the next hyperedge result
    ///
    /// # Example
    /// ```javascript
    /// const stream = await db.searchHyperedgesStream(query);
    /// for await (const result of stream) {
    ///   console.log(result);
    /// }
    /// ```
    #[napi]
    pub fn next(&mut self) -> Result<Option<JsHyperedgeResult>> {
        if self.index < self.results.len() {
            let result = self.results[self.index].clone();
            self.index += 1;
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    /// Collect all remaining results
    #[napi]
    pub fn collect(&mut self) -> Vec<JsHyperedgeResult> {
        let remaining = self.results[self.index..].to_vec();
        self.index = self.results.len();
        remaining
    }
}

/// Node stream iterator
#[napi]
pub struct NodeStream {
    nodes: Vec<JsNode>,
    index: usize,
}

impl NodeStream {
    /// Create a new node stream
    pub fn new(nodes: Vec<JsNode>) -> Self {
        Self { nodes, index: 0 }
    }
}

#[napi]
impl NodeStream {
    /// Get the next node
    #[napi]
    pub fn next(&mut self) -> Result<Option<JsNode>> {
        if self.index < self.nodes.len() {
            let node = self.nodes[self.index].clone();
            self.index += 1;
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }

    /// Collect all remaining nodes
    #[napi]
    pub fn collect(&mut self) -> Vec<JsNode> {
        let remaining = self.nodes[self.index..].to_vec();
        self.index = self.nodes.len();
        remaining
    }
}
