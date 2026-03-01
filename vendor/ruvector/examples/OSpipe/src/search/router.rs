//! Query routing to the optimal search backend.

/// The search backend to route a query to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryRoute {
    /// Pure vector HNSW semantic search.
    Semantic,
    /// Full-text keyword search (FTS5-style).
    Keyword,
    /// Graph-based relationship query.
    Graph,
    /// Time-based delta replay query.
    Temporal,
    /// Combined semantic + keyword search.
    Hybrid,
}

/// Routes incoming queries to the optimal search backend based on
/// query content heuristics.
pub struct QueryRouter;

impl QueryRouter {
    /// Create a new query router.
    pub fn new() -> Self {
        Self
    }

    /// Determine the best search route for the given query string.
    ///
    /// Routing heuristics:
    /// - Temporal keywords ("yesterday", "last week", etc.) -> Temporal
    /// - Graph keywords ("related to", "connected", etc.) -> Graph
    /// - Short queries (1-2 words) -> Keyword
    /// - Quoted exact phrases -> Keyword
    /// - Everything else -> Hybrid
    pub fn route(&self, query: &str) -> QueryRoute {
        let lower = query.to_lowercase();
        let word_count = lower.split_whitespace().count();

        // Temporal patterns
        let temporal_keywords = [
            "yesterday",
            "last week",
            "last month",
            "today",
            "this morning",
            "this afternoon",
            "hours ago",
            "minutes ago",
            "days ago",
            "between",
            "before",
            "after",
        ];
        if temporal_keywords.iter().any(|kw| lower.contains(kw)) {
            return QueryRoute::Temporal;
        }

        // Graph patterns
        let graph_keywords = [
            "related to",
            "connected to",
            "linked with",
            "associated with",
            "relationship between",
        ];
        if graph_keywords.iter().any(|kw| lower.contains(kw)) {
            return QueryRoute::Graph;
        }

        // Exact phrase (quoted)
        if query.starts_with('"') && query.ends_with('"') {
            return QueryRoute::Keyword;
        }

        // Very short queries are better served by keyword
        if word_count <= 2 {
            return QueryRoute::Keyword;
        }

        // Default: hybrid combines the best of both
        QueryRoute::Hybrid
    }
}

impl Default for QueryRouter {
    fn default() -> Self {
        Self::new()
    }
}
