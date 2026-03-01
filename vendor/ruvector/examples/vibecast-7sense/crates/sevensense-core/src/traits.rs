//! Common traits for the 7sense platform.
//!
//! This module defines cross-cutting traits used by multiple bounded contexts.

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

/// Marker trait for domain entities.
pub trait Entity: Debug + Clone + Send + Sync {
    /// The type of the entity's identifier.
    type Id: Debug + Clone + Eq + std::hash::Hash + Send + Sync;

    /// Returns the entity's identifier.
    fn id(&self) -> &Self::Id;
}

/// Marker trait for value objects.
pub trait ValueObject: Debug + Clone + PartialEq + Send + Sync {}

/// Trait for objects that can be serialized to/from JSON.
pub trait JsonSerializable: Serialize + DeserializeOwned {
    /// Serializes the object to a JSON string.
    fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Serializes the object to a pretty-printed JSON string.
    fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserializes an object from a JSON string.
    fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Blanket implementation for all serializable types.
impl<T: Serialize + DeserializeOwned> JsonSerializable for T {}

/// Trait for domain events.
pub trait DomainEvent: Debug + Clone + Send + Sync + Serialize {
    /// Returns the event's unique identifier.
    fn event_id(&self) -> &str;

    /// Returns the timestamp when the event occurred.
    fn occurred_at(&self) -> chrono::DateTime<chrono::Utc>;

    /// Returns the event type name for routing.
    fn event_type(&self) -> &'static str;
}

/// Trait for domain event handlers.
#[async_trait]
pub trait EventHandler<E: DomainEvent>: Send + Sync {
    /// Handles a domain event.
    async fn handle(&self, event: &E) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Trait for unit of work pattern.
#[async_trait]
pub trait UnitOfWork: Send + Sync {
    /// Commits all pending changes.
    async fn commit(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Rolls back all pending changes.
    async fn rollback(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Trait for paginated queries.
pub trait Paginated {
    /// Returns the current page number (0-indexed).
    fn page(&self) -> usize;

    /// Returns the page size.
    fn page_size(&self) -> usize;

    /// Returns the offset for database queries.
    fn offset(&self) -> usize {
        self.page() * self.page_size()
    }
}

/// A page of results.
#[derive(Debug, Clone, Serialize)]
pub struct Page<T> {
    /// The items in this page.
    pub items: Vec<T>,
    /// Current page number (0-indexed).
    pub page: usize,
    /// Page size.
    pub page_size: usize,
    /// Total number of items across all pages.
    pub total_items: usize,
}

impl<T> Page<T> {
    /// Creates a new page of results.
    #[must_use]
    pub fn new(items: Vec<T>, page: usize, page_size: usize, total_items: usize) -> Self {
        Self {
            items,
            page,
            page_size,
            total_items,
        }
    }

    /// Returns the total number of pages.
    #[must_use]
    pub fn total_pages(&self) -> usize {
        if self.page_size == 0 {
            0
        } else {
            (self.total_items + self.page_size - 1) / self.page_size
        }
    }

    /// Returns true if there is a next page.
    #[must_use]
    pub fn has_next(&self) -> bool {
        self.page + 1 < self.total_pages()
    }

    /// Returns true if there is a previous page.
    #[must_use]
    pub fn has_previous(&self) -> bool {
        self.page > 0
    }

    /// Maps the items to a different type.
    pub fn map<U, F: FnMut(T) -> U>(self, f: F) -> Page<U> {
        Page {
            items: self.items.into_iter().map(f).collect(),
            page: self.page,
            page_size: self.page_size,
            total_items: self.total_items,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_calculations() {
        let page: Page<i32> = Page::new(vec![1, 2, 3], 0, 3, 10);
        assert_eq!(page.total_pages(), 4);
        assert!(page.has_next());
        assert!(!page.has_previous());
    }

    #[test]
    fn test_page_map() {
        let page = Page::new(vec![1, 2, 3], 0, 3, 3);
        let mapped = page.map(|x| x * 2);
        assert_eq!(mapped.items, vec![2, 4, 6]);
    }
}
