//! NAPI-RS bindings for Node.js

#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use ruvector_router_core::{
    DistanceMetric as CoreDistanceMetric, SearchQuery as CoreSearchQuery, VectorDB as CoreVectorDB,
    VectorEntry as CoreVectorEntry,
};
use std::collections::HashMap;
use std::sync::Arc;

#[napi]
pub enum DistanceMetric {
    Euclidean,
    Cosine,
    DotProduct,
    Manhattan,
}

impl From<DistanceMetric> for CoreDistanceMetric {
    fn from(metric: DistanceMetric) -> Self {
        match metric {
            DistanceMetric::Euclidean => CoreDistanceMetric::Euclidean,
            DistanceMetric::Cosine => CoreDistanceMetric::Cosine,
            DistanceMetric::DotProduct => CoreDistanceMetric::DotProduct,
            DistanceMetric::Manhattan => CoreDistanceMetric::Manhattan,
        }
    }
}

#[napi(object)]
pub struct DbOptions {
    pub dimensions: u32,
    pub max_elements: Option<u32>,
    pub distance_metric: Option<DistanceMetric>,
    pub hnsw_m: Option<u32>,
    pub hnsw_ef_construction: Option<u32>,
    pub hnsw_ef_search: Option<u32>,
    pub storage_path: Option<String>,
}

#[napi]
pub struct VectorDB {
    db: Arc<CoreVectorDB>,
}

#[napi]
impl VectorDB {
    #[napi(constructor)]
    pub fn new(options: DbOptions) -> Result<Self> {
        let mut builder = CoreVectorDB::builder().dimensions(options.dimensions as usize);

        if let Some(max_elements) = options.max_elements {
            builder = builder.max_elements(max_elements as usize);
        }

        if let Some(metric) = options.distance_metric {
            builder = builder.distance_metric(metric.into());
        }

        if let Some(m) = options.hnsw_m {
            builder = builder.hnsw_m(m as usize);
        }

        if let Some(ef) = options.hnsw_ef_construction {
            builder = builder.hnsw_ef_construction(ef as usize);
        }

        if let Some(ef) = options.hnsw_ef_search {
            builder = builder.hnsw_ef_search(ef as usize);
        }

        if let Some(path) = options.storage_path {
            builder = builder.storage_path(path);
        }

        let db = builder
            .build()
            .map_err(|e| Error::from_reason(e.to_string()))?;

        Ok(Self { db: Arc::new(db) })
    }

    #[napi]
    pub fn insert(&self, id: String, vector: Float32Array) -> Result<String> {
        let vector_data: Vec<f32> = vector.to_vec();

        let core_entry = CoreVectorEntry {
            id: id.clone(),
            vector: vector_data,
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now().timestamp(),
        };

        self.db
            .insert(core_entry)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn insert_async(&self, id: String, vector: Float32Array) -> Result<String> {
        let db = self.db.clone();
        let vector_data: Vec<f32> = vector.to_vec();

        tokio::task::spawn_blocking(move || {
            let core_entry = CoreVectorEntry {
                id: id.clone(),
                vector: vector_data,
                metadata: HashMap::new(),
                timestamp: chrono::Utc::now().timestamp(),
            };

            db.insert(core_entry)
                .map_err(|e| Error::from_reason(e.to_string()))
        })
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?
    }

    #[napi]
    pub fn search(&self, query_vector: Float32Array, k: u32) -> Result<Vec<SearchResultJS>> {
        let vector_data: Vec<f32> = query_vector.to_vec();

        let core_query = CoreSearchQuery {
            vector: vector_data,
            k: k as usize,
            filters: None,
            threshold: None,
            ef_search: None,
        };

        self.db
            .search(core_query)
            .map(|results| {
                results
                    .into_iter()
                    .map(|r| SearchResultJS {
                        id: r.id,
                        score: r.score as f64,
                    })
                    .collect()
            })
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn search_async(
        &self,
        query_vector: Float32Array,
        k: u32,
    ) -> Result<Vec<SearchResultJS>> {
        let db = self.db.clone();
        let vector_data: Vec<f32> = query_vector.to_vec();

        tokio::task::spawn_blocking(move || {
            let core_query = CoreSearchQuery {
                vector: vector_data,
                k: k as usize,
                filters: None,
                threshold: None,
                ef_search: None,
            };

            db.search(core_query)
                .map(|results| {
                    results
                        .into_iter()
                        .map(|r| SearchResultJS {
                            id: r.id,
                            score: r.score as f64,
                        })
                        .collect()
                })
                .map_err(|e| Error::from_reason(e.to_string()))
        })
        .await
        .map_err(|e| Error::from_reason(e.to_string()))?
    }

    #[napi]
    pub fn delete(&self, id: String) -> Result<bool> {
        self.db
            .delete(&id)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn count(&self) -> Result<u32> {
        self.db
            .count()
            .map(|c| c as u32)
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi]
    pub fn get_all_ids(&self) -> Result<Vec<String>> {
        self.db
            .get_all_ids()
            .map_err(|e| Error::from_reason(e.to_string()))
    }
}

#[napi(object)]
pub struct SearchResultJS {
    pub id: String,
    pub score: f64,
}
