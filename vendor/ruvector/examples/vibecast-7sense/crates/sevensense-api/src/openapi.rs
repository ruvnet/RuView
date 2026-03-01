//! OpenAPI/Swagger documentation generation.
//!
//! This module generates OpenAPI 3.0 documentation for the REST API
//! and serves a Swagger UI at `/docs`.

use axum::{routing::get, Json, Router};
use utoipa::{
    openapi::{
        security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
        OpenApi as OpenApiDoc,
    },
    Modify, OpenApi,
};
use utoipa_swagger_ui::SwaggerUi;

use crate::error::ErrorResponse;
use crate::rest::handlers::*;
use crate::AppContext;
use crate::HealthResponse;

/// OpenAPI documentation struct.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "7sense Bioacoustic Analysis API",
        version = "1.0.0",
        description = "REST API for bioacoustic recording analysis, similarity search, and cluster discovery.",
        contact(
            name = "7sense Team",
            url = "https://github.com/vibecast/vibecast"
        ),
        license(
            name = "MIT OR Apache-2.0",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    servers(
        (url = "/api/v1", description = "API v1")
    ),
    paths(
        upload_recording,
        get_recording,
        get_neighbors,
        list_clusters,
        get_cluster,
        assign_cluster_label,
        get_evidence_pack,
        generate_evidence_pack,
        search,
        health_check,
    ),
    components(
        schemas(
            Recording,
            UploadResponse,
            Neighbor,
            NeighborParams,
            Cluster,
            SpeciesCount,
            EvidencePack,
            SegmentSummary,
            NeighborEvidence,
            FeatureContribution,
            AcousticFeature,
            EvidenceVisualizations,
            SearchQuery,
            SearchResults,
            SearchQueryEcho,
            SearchResult,
            AssignLabelRequest,
            GenerateEvidenceRequest,
            ErrorResponse,
            HealthResponse,
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "recordings", description = "Recording upload and management"),
        (name = "segments", description = "Segment analysis and similarity search"),
        (name = "clusters", description = "Cluster discovery and labeling"),
        (name = "evidence", description = "Evidence packs for interpretability"),
        (name = "search", description = "Semantic search"),
        (name = "system", description = "System health and status"),
    )
)]
pub struct ApiDoc;

/// Security scheme addon.
struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut OpenApiDoc) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some("API key authentication"))
                        .build(),
                ),
            );
        }
    }
}

/// Create the OpenAPI documentation router.
#[must_use]
pub fn create_router() -> Router<AppContext> {
    Router::new()
        // Raw OpenAPI JSON
        .route("/openapi.json", get(openapi_json))
        // Swagger UI - merge directly
        .merge(SwaggerUi::new("/docs/swagger-ui")
            .url("/docs/openapi.json", ApiDoc::openapi()))
}

/// Get raw OpenAPI JSON.
#[allow(clippy::unused_async)]
async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_generation() {
        let doc = ApiDoc::openapi();
        assert_eq!(doc.info.title, "7sense Bioacoustic Analysis API");
        assert!(!doc.paths.paths.is_empty());
    }

    #[test]
    fn test_openapi_has_required_paths() {
        let doc = ApiDoc::openapi();
        let paths: Vec<&str> = doc.paths.paths.keys().map(std::string::String::as_str).collect();

        assert!(paths.contains(&"/recordings"));
        assert!(paths.contains(&"/segments/{id}/neighbors"));
        assert!(paths.contains(&"/clusters"));
        assert!(paths.contains(&"/evidence/{id}"));
        assert!(paths.contains(&"/search"));
        assert!(paths.contains(&"/health"));
    }
}
