// RDF Triple Store with efficient in-memory indexing for WASM
//
// Provides in-memory storage for RDF triples with multiple indexes
// for efficient query patterns (SPO, POS, OSP).

use super::ast::{Iri, RdfTerm};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

/// RDF Triple
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Triple {
    pub subject: RdfTerm,
    pub predicate: Iri,
    pub object: RdfTerm,
}

impl Triple {
    pub fn new(subject: RdfTerm, predicate: Iri, object: RdfTerm) -> Self {
        Self {
            subject,
            predicate,
            object,
        }
    }
}

/// Triple store statistics
#[derive(Debug, Clone)]
pub struct StoreStats {
    pub triple_count: u64,
    pub subject_count: usize,
    pub predicate_count: usize,
    pub object_count: usize,
    pub graph_count: usize,
}

/// RDF Triple Store (WASM-compatible, thread-safe via RwLock)
pub struct TripleStore {
    /// All triples stored by internal ID
    triples: RwLock<HashMap<u64, Triple>>,

    /// SPO index: subject -> predicate -> object IDs
    spo_index: RwLock<HashMap<String, HashMap<String, HashSet<u64>>>>,

    /// POS index: predicate -> object -> subject IDs
    pos_index: RwLock<HashMap<String, HashMap<String, HashSet<u64>>>>,

    /// OSP index: object -> subject -> predicate IDs
    osp_index: RwLock<HashMap<String, HashMap<String, HashSet<u64>>>>,

    /// Named graphs: graph IRI -> triple IDs
    graphs: RwLock<HashMap<String, HashSet<u64>>>,

    /// Default graph triple IDs
    default_graph: RwLock<HashSet<u64>>,

    /// Triple ID counter
    next_id: AtomicU64,

    /// Unique subjects for statistics
    subjects: RwLock<HashSet<String>>,

    /// Unique predicates for statistics
    predicates: RwLock<HashSet<String>>,

    /// Unique objects for statistics
    objects: RwLock<HashSet<String>>,
}

impl TripleStore {
    pub fn new() -> Self {
        Self {
            triples: RwLock::new(HashMap::new()),
            spo_index: RwLock::new(HashMap::new()),
            pos_index: RwLock::new(HashMap::new()),
            osp_index: RwLock::new(HashMap::new()),
            graphs: RwLock::new(HashMap::new()),
            default_graph: RwLock::new(HashSet::new()),
            next_id: AtomicU64::new(1),
            subjects: RwLock::new(HashSet::new()),
            predicates: RwLock::new(HashSet::new()),
            objects: RwLock::new(HashSet::new()),
        }
    }

    /// Insert a triple into the default graph
    pub fn insert(&self, triple: Triple) -> u64 {
        self.insert_into_graph(triple, None)
    }

    /// Insert a triple into a specific graph
    pub fn insert_into_graph(&self, triple: Triple, graph: Option<&str>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        // Get string representations for indexing
        let subject_key = term_to_key(&triple.subject);
        let predicate_key = triple.predicate.as_str().to_string();
        let object_key = term_to_key(&triple.object);

        // Update statistics
        {
            let mut subjects = self.subjects.write().unwrap();
            subjects.insert(subject_key.clone());
        }
        {
            let mut predicates = self.predicates.write().unwrap();
            predicates.insert(predicate_key.clone());
        }
        {
            let mut objects = self.objects.write().unwrap();
            objects.insert(object_key.clone());
        }

        // Update SPO index
        {
            let mut spo_index = self.spo_index.write().unwrap();
            spo_index
                .entry(subject_key.clone())
                .or_insert_with(HashMap::new)
                .entry(predicate_key.clone())
                .or_insert_with(HashSet::new)
                .insert(id);
        }

        // Update POS index
        {
            let mut pos_index = self.pos_index.write().unwrap();
            pos_index
                .entry(predicate_key.clone())
                .or_insert_with(HashMap::new)
                .entry(object_key.clone())
                .or_insert_with(HashSet::new)
                .insert(id);
        }

        // Update OSP index
        {
            let mut osp_index = self.osp_index.write().unwrap();
            osp_index
                .entry(object_key)
                .or_insert_with(HashMap::new)
                .entry(subject_key)
                .or_insert_with(HashSet::new)
                .insert(id);
        }

        // Update graph membership
        if let Some(graph_iri) = graph {
            let mut graphs = self.graphs.write().unwrap();
            graphs
                .entry(graph_iri.to_string())
                .or_insert_with(HashSet::new)
                .insert(id);
        } else {
            let mut default_graph = self.default_graph.write().unwrap();
            default_graph.insert(id);
        }

        // Store the triple
        {
            let mut triples = self.triples.write().unwrap();
            triples.insert(id, triple);
        }

        id
    }

    /// Get a triple by ID
    pub fn get(&self, id: u64) -> Option<Triple> {
        let triples = self.triples.read().unwrap();
        triples.get(&id).cloned()
    }

    /// Query triples matching a pattern (None means any value)
    pub fn query(
        &self,
        subject: Option<&RdfTerm>,
        predicate: Option<&Iri>,
        object: Option<&RdfTerm>,
    ) -> Vec<Triple> {
        self.query_with_graph(subject, predicate, object, None)
    }

    /// Query triples matching a pattern in a specific graph
    pub fn query_with_graph(
        &self,
        subject: Option<&RdfTerm>,
        predicate: Option<&Iri>,
        object: Option<&RdfTerm>,
        graph: Option<&str>,
    ) -> Vec<Triple> {
        // Filter by graph if specified
        let graph_filter: Option<HashSet<u64>> = graph.map(|g| {
            let graphs = self.graphs.read().unwrap();
            graphs.get(g).cloned().unwrap_or_default()
        });

        let spo_index = self.spo_index.read().unwrap();
        let pos_index = self.pos_index.read().unwrap();
        let osp_index = self.osp_index.read().unwrap();
        let triples = self.triples.read().unwrap();

        // Choose the best index based on bound variables
        let ids = match (subject, predicate, object) {
            // All bound - direct lookup
            (Some(s), Some(p), Some(o)) => {
                let s_key = term_to_key(s);
                let p_key = p.as_str();
                let o_key = term_to_key(o);

                spo_index
                    .get(&s_key)
                    .and_then(|pred_map| pred_map.get(p_key))
                    .map(|ids| ids.iter().copied().collect::<Vec<_>>())
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|id| {
                        triples
                            .get(id)
                            .map(|t| term_to_key(&t.object) == o_key)
                            .unwrap_or(false)
                    })
                    .collect::<Vec<_>>()
            }

            // Subject and predicate bound - use SPO
            (Some(s), Some(p), None) => {
                let s_key = term_to_key(s);
                let p_key = p.as_str();

                spo_index
                    .get(&s_key)
                    .and_then(|pred_map| pred_map.get(p_key))
                    .map(|ids| ids.iter().copied().collect())
                    .unwrap_or_default()
            }

            // Subject only - use SPO
            (Some(s), None, None) => {
                let s_key = term_to_key(s);

                spo_index
                    .get(&s_key)
                    .map(|pred_map| {
                        pred_map
                            .values()
                            .flat_map(|ids| ids.iter().copied())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            }

            // Predicate and object bound - use POS
            (None, Some(p), Some(o)) => {
                let p_key = p.as_str();
                let o_key = term_to_key(o);

                pos_index
                    .get(p_key)
                    .and_then(|obj_map| obj_map.get(&o_key))
                    .map(|ids| ids.iter().copied().collect())
                    .unwrap_or_default()
            }

            // Predicate only - use POS
            (None, Some(p), None) => {
                let p_key = p.as_str();

                pos_index
                    .get(p_key)
                    .map(|obj_map| {
                        obj_map
                            .values()
                            .flat_map(|ids| ids.iter().copied())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            }

            // Object only - use OSP
            (None, None, Some(o)) => {
                let o_key = term_to_key(o);

                osp_index
                    .get(&o_key)
                    .map(|subj_map| {
                        subj_map
                            .values()
                            .flat_map(|ids| ids.iter().copied())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            }

            // Subject and object bound - use SPO then filter
            (Some(s), None, Some(o)) => {
                let s_key = term_to_key(s);
                let o_key = term_to_key(o);

                spo_index
                    .get(&s_key)
                    .map(|pred_map| {
                        pred_map
                            .values()
                            .flat_map(|ids| ids.iter().copied())
                            .filter(|id| {
                                triples
                                    .get(id)
                                    .map(|t| term_to_key(&t.object) == o_key)
                                    .unwrap_or(false)
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            }

            // Nothing bound - return all
            (None, None, None) => triples.keys().copied().collect(),
        };

        // Apply graph filter and collect results
        ids.into_iter()
            .filter(|id| {
                graph_filter
                    .as_ref()
                    .map(|filter| filter.contains(id))
                    .unwrap_or(true)
            })
            .filter_map(|id| triples.get(&id).cloned())
            .collect()
    }

    /// Get all triples in the store
    pub fn all_triples(&self) -> Vec<Triple> {
        let triples = self.triples.read().unwrap();
        triples.values().cloned().collect()
    }

    /// Get triple count
    pub fn count(&self) -> usize {
        let triples = self.triples.read().unwrap();
        triples.len()
    }

    /// Check if store is empty
    pub fn is_empty(&self) -> bool {
        let triples = self.triples.read().unwrap();
        triples.is_empty()
    }

    /// Clear all triples
    pub fn clear(&self) {
        self.triples.write().unwrap().clear();
        self.spo_index.write().unwrap().clear();
        self.pos_index.write().unwrap().clear();
        self.osp_index.write().unwrap().clear();
        self.graphs.write().unwrap().clear();
        self.default_graph.write().unwrap().clear();
        self.subjects.write().unwrap().clear();
        self.predicates.write().unwrap().clear();
        self.objects.write().unwrap().clear();
    }

    /// Clear a specific graph
    pub fn clear_graph(&self, graph: Option<&str>) {
        let ids_to_remove: Vec<u64> = if let Some(graph_iri) = graph {
            let graphs = self.graphs.read().unwrap();
            graphs
                .get(graph_iri)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect()
        } else {
            let default_graph = self.default_graph.read().unwrap();
            default_graph.iter().copied().collect()
        };

        for id in ids_to_remove {
            self.remove(id);
        }
    }

    /// Remove a triple by ID
    pub fn remove(&self, id: u64) -> Option<Triple> {
        let triple = {
            let mut triples = self.triples.write().unwrap();
            triples.remove(&id)
        }?;

        let subject_key = term_to_key(&triple.subject);
        let predicate_key = triple.predicate.as_str().to_string();
        let object_key = term_to_key(&triple.object);

        // Remove from SPO index
        {
            let mut spo_index = self.spo_index.write().unwrap();
            if let Some(pred_map) = spo_index.get_mut(&subject_key) {
                if let Some(ids) = pred_map.get_mut(&predicate_key) {
                    ids.remove(&id);
                }
            }
        }

        // Remove from POS index
        {
            let mut pos_index = self.pos_index.write().unwrap();
            if let Some(obj_map) = pos_index.get_mut(&predicate_key) {
                if let Some(ids) = obj_map.get_mut(&object_key) {
                    ids.remove(&id);
                }
            }
        }

        // Remove from OSP index
        {
            let mut osp_index = self.osp_index.write().unwrap();
            if let Some(subj_map) = osp_index.get_mut(&object_key) {
                if let Some(ids) = subj_map.get_mut(&subject_key) {
                    ids.remove(&id);
                }
            }
        }

        // Remove from graphs
        {
            let mut default_graph = self.default_graph.write().unwrap();
            default_graph.remove(&id);
        }
        {
            let mut graphs = self.graphs.write().unwrap();
            for (_, ids) in graphs.iter_mut() {
                ids.remove(&id);
            }
        }

        Some(triple)
    }

    /// Get statistics about the store
    pub fn stats(&self) -> StoreStats {
        let triples = self.triples.read().unwrap();
        let subjects = self.subjects.read().unwrap();
        let predicates = self.predicates.read().unwrap();
        let objects = self.objects.read().unwrap();
        let graphs = self.graphs.read().unwrap();

        StoreStats {
            triple_count: triples.len() as u64,
            subject_count: subjects.len(),
            predicate_count: predicates.len(),
            object_count: objects.len(),
            graph_count: graphs.len() + 1, // +1 for default graph
        }
    }

    /// List all named graphs
    pub fn list_graphs(&self) -> Vec<String> {
        let graphs = self.graphs.read().unwrap();
        graphs.keys().cloned().collect()
    }

    /// Get triples from a specific graph
    pub fn get_graph(&self, graph: &str) -> Vec<Triple> {
        let graphs = self.graphs.read().unwrap();
        let triples = self.triples.read().unwrap();

        graphs
            .get(graph)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| triples.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get triples from the default graph
    pub fn get_default_graph(&self) -> Vec<Triple> {
        let default_graph = self.default_graph.read().unwrap();
        let triples = self.triples.read().unwrap();

        default_graph
            .iter()
            .filter_map(|id| triples.get(id).cloned())
            .collect()
    }
}

impl Default for TripleStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert an RDF term to a string key for indexing
fn term_to_key(term: &RdfTerm) -> String {
    match term {
        RdfTerm::Iri(iri) => format!("<{}>", iri.as_str()),
        RdfTerm::Literal(lit) => {
            if let Some(ref lang) = lit.language {
                format!("\"{}\"@{}", lit.value, lang)
            } else if lit.datatype.as_str() != "http://www.w3.org/2001/XMLSchema#string" {
                format!("\"{}\"^^<{}>", lit.value, lit.datatype.as_str())
            } else {
                format!("\"{}\"", lit.value)
            }
        }
        RdfTerm::BlankNode(id) => format!("_:{}", id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_query() {
        let store = TripleStore::new();

        let triple = Triple::new(
            RdfTerm::iri("http://example.org/person/1"),
            Iri::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
            RdfTerm::iri("http://example.org/Person"),
        );

        let id = store.insert(triple.clone());
        assert!(id > 0);

        let retrieved = store.get(id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), triple);
    }

    #[test]
    fn test_query_by_subject() {
        let store = TripleStore::new();

        let subject = RdfTerm::iri("http://example.org/person/1");
        store.insert(Triple::new(
            subject.clone(),
            Iri::rdf_type(),
            RdfTerm::iri("http://example.org/Person"),
        ));
        store.insert(Triple::new(
            subject.clone(),
            Iri::rdfs_label(),
            RdfTerm::literal("Alice"),
        ));
        store.insert(Triple::new(
            RdfTerm::iri("http://example.org/person/2"),
            Iri::rdf_type(),
            RdfTerm::iri("http://example.org/Person"),
        ));

        let results = store.query(Some(&subject), None, None);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_statistics() {
        let store = TripleStore::new();

        store.insert(Triple::new(
            RdfTerm::iri("http://example.org/s1"),
            Iri::new("http://example.org/p1"),
            RdfTerm::literal("o1"),
        ));
        store.insert(Triple::new(
            RdfTerm::iri("http://example.org/s2"),
            Iri::new("http://example.org/p1"),
            RdfTerm::literal("o2"),
        ));

        let stats = store.stats();
        assert_eq!(stats.triple_count, 2);
        assert_eq!(stats.subject_count, 2);
        assert_eq!(stats.predicate_count, 1);
        assert_eq!(stats.object_count, 2);
    }
}
