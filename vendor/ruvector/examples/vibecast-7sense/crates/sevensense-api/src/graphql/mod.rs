//! GraphQL API module for 7sense.
//!
//! This module provides a flexible GraphQL API with:
//! - Query operations for recordings, segments, clusters, and evidence
//! - Mutations for ingestion and labeling
//! - Subscriptions for real-time processing updates
//!
//! ## Schema
//!
//! The schema is defined using `async-graphql` with automatic type generation.
//! Access the GraphQL playground at `/graphql` when enabled.

pub mod schema;
pub mod types;

use async_graphql::Schema;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};

use crate::AppContext;
use schema::{MutationRoot, QueryRoot, SubscriptionRoot};

/// GraphQL schema type alias.
pub type ApiSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

/// Build the GraphQL schema with the application context.
#[must_use]
pub fn build_schema(ctx: AppContext) -> ApiSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(ctx)
        .finish()
}

/// Create the GraphQL router.
#[must_use]
pub fn create_router(ctx: AppContext) -> Router<AppContext> {
    let schema = build_schema(ctx.clone());

    Router::new()
        .route("/", get(graphql_playground).post(graphql_handler))
        .with_state(schema)
}

/// GraphQL request handler.
async fn graphql_handler(State(schema): State<ApiSchema>, req: GraphQLRequest) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

/// GraphQL Playground HTML page.
#[allow(clippy::unused_async)]
async fn graphql_playground() -> impl IntoResponse {
    Html(PLAYGROUND_HTML)
}

const PLAYGROUND_HTML: &str = r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>7sense GraphQL Playground</title>
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/graphql-playground-react@1.7.26/build/static/css/index.css" />
    <script src="https://cdn.jsdelivr.net/npm/graphql-playground-react@1.7.26/build/static/js/middleware.js"></script>
</head>
<body>
    <div id="root"></div>
    <script>
        window.addEventListener('load', function (event) {
            GraphQLPlayground.init(document.getElementById('root'), {
                endpoint: '/graphql',
                settings: {
                    'editor.theme': 'dark',
                    'editor.fontSize': 14,
                    'request.credentials': 'include',
                },
                tabs: [
                    {
                        name: 'Example Queries',
                        endpoint: '/graphql',
                        query: `# 7sense GraphQL API

# Get all clusters
query ListClusters {
  clusters {
    id
    label
    size
    density
  }
}

# Find similar segments
query FindNeighbors($segmentId: UUID!, $k: Int) {
  neighbors(segmentId: $segmentId, k: $k) {
    segmentId
    similarity
    startTime
    endTime
  }
}
`
                    }
                ]
            });
        });
    </script>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    // Schema tests would go here with mock context
}
