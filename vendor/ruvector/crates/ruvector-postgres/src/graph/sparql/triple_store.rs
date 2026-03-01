// RDF Triple Store with efficient indexing
//
// Provides persistent storage for RDF triples with multiple indexes
// for efficient query patterns (SPO, POS, OSP).

use super::ast::{Iri, RdfTerm};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

/// RDF Triple
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

    /// Create from string components
    pub fn from_strings(subject: &str, predicate: &str, object: &str) -> Self {
        Self {
            subject: if subject.starts_with("_:") {
                RdfTerm::BlankNode(subject[2..].to_string())
            } else {
                RdfTerm::Iri(Iri::new(subject))
            },
            predicate: Iri::new(predicate),
            object: if object.starts_with("_:") {
                RdfTerm::BlankNode(object[2..].to_string())
            } else if object.starts_with('"') {
                // Parse literal
                parse_literal_string(object)
            } else {
                RdfTerm::Iri(Iri::new(object))
            },
        }
    }
}

/// Parse a literal string like "value"@en or "value"^^xsd:type
fn parse_literal_string(s: &str) -> RdfTerm {
    let s = s.trim();
    if !s.starts_with('"') {
        return RdfTerm::literal(s);
    }

    // Find the closing quote
    let mut chars = s.chars().peekable();
    chars.next(); // Skip opening quote

    let mut value = String::new();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(escaped) = chars.next() {
                match escaped {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'r' => value.push('\r'),
                    '"' => value.push('"'),
                    '\\' => value.push('\\'),
                    _ => {
                        value.push('\\');
                        value.push(escaped);
                    }
                }
            }
        } else if c == '"' {
            break;
        } else {
            value.push(c);
        }
    }

    // Check for language tag or datatype
    let remainder: String = chars.collect();
    if remainder.starts_with('@') {
        let lang = remainder[1..].to_string();
        RdfTerm::lang_literal(value, lang)
    } else if remainder.starts_with("^^") {
        let datatype = &remainder[2..];
        let datatype = if datatype.starts_with('<') && datatype.ends_with('>') {
            &datatype[1..datatype.len() - 1]
        } else {
            datatype
        };
        RdfTerm::typed_literal(value, Iri::new(datatype))
    } else {
        RdfTerm::literal(value)
    }
}

/// Triple index type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TripleIndex {
    /// Subject-Predicate-Object (for ?p ?o given s)
    Spo,
    /// Predicate-Object-Subject (for ?s given p, o)
    Pos,
    /// Object-Subject-Predicate (for ?s ?p given o)
    Osp,
    /// Subject-Object-Predicate (for ?p given s, o)
    Sop,
    /// Predicate-Subject-Object (for ?o given p, s)
    Pso,
    /// Object-Predicate-Subject (for ?s given o, p)
    Ops,
}

/// Index key for triple lookup
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IndexKey {
    pub first: String,
    pub second: Option<String>,
}

impl IndexKey {
    pub fn single(first: impl Into<String>) -> Self {
        Self {
            first: first.into(),
            second: None,
        }
    }

    pub fn double(first: impl Into<String>, second: impl Into<String>) -> Self {
        Self {
            first: first.into(),
            second: Some(second.into()),
        }
    }
}

/// Triple store statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreStats {
    pub triple_count: u64,
    pub subject_count: usize,
    pub predicate_count: usize,
    pub object_count: usize,
    pub graph_count: usize,
}

/// RDF Triple Store
pub struct TripleStore {
    /// All triples stored by internal ID
    triples: DashMap<u64, Triple>,

    /// SPO index: subject -> predicate -> object IDs
    spo_index: DashMap<String, DashMap<String, HashSet<u64>>>,

    /// POS index: predicate -> object -> subject IDs
    pos_index: DashMap<String, DashMap<String, HashSet<u64>>>,

    /// OSP index: object -> subject -> predicate IDs
    osp_index: DashMap<String, DashMap<String, HashSet<u64>>>,

    /// Named graphs: graph IRI -> triple IDs
    graphs: DashMap<String, HashSet<u64>>,

    /// Default graph triple IDs
    default_graph: DashMap<u64, ()>,

    /// Triple ID counter
    next_id: AtomicU64,

    /// Unique subjects for statistics
    subjects: DashMap<String, ()>,

    /// Unique predicates for statistics
    predicates: DashMap<String, ()>,

    /// Unique objects for statistics
    objects: DashMap<String, ()>,
}

impl TripleStore {
    pub fn new() -> Self {
        Self {
            triples: DashMap::new(),
            spo_index: DashMap::new(),
            pos_index: DashMap::new(),
            osp_index: DashMap::new(),
            graphs: DashMap::new(),
            default_graph: DashMap::new(),
            next_id: AtomicU64::new(1),
            subjects: DashMap::new(),
            predicates: DashMap::new(),
            objects: DashMap::new(),
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
        self.subjects.insert(subject_key.clone(), ());
        self.predicates.insert(predicate_key.clone(), ());
        self.objects.insert(object_key.clone(), ());

        // Update SPO index
        self.spo_index
            .entry(subject_key.clone())
            .or_insert_with(DashMap::new)
            .entry(predicate_key.clone())
            .or_insert_with(HashSet::new)
            .insert(id);

        // Update POS index
        self.pos_index
            .entry(predicate_key.clone())
            .or_insert_with(DashMap::new)
            .entry(object_key.clone())
            .or_insert_with(HashSet::new)
            .insert(id);

        // Update OSP index
        self.osp_index
            .entry(object_key)
            .or_insert_with(DashMap::new)
            .entry(subject_key)
            .or_insert_with(HashSet::new)
            .insert(id);

        // Update graph membership
        if let Some(graph_iri) = graph {
            self.graphs
                .entry(graph_iri.to_string())
                .or_insert_with(HashSet::new)
                .insert(id);
        } else {
            self.default_graph.insert(id, ());
        }

        // Store the triple
        self.triples.insert(id, triple);

        id
    }

    /// Remove a triple by ID
    pub fn remove(&self, id: u64) -> Option<Triple> {
        if let Some((_, triple)) = self.triples.remove(&id) {
            let subject_key = term_to_key(&triple.subject);
            let predicate_key = triple.predicate.as_str().to_string();
            let object_key = term_to_key(&triple.object);

            // Remove from SPO index
            if let Some(pred_map) = self.spo_index.get(&subject_key) {
                if let Some(mut ids) = pred_map.get_mut(&predicate_key) {
                    ids.remove(&id);
                }
            }

            // Remove from POS index
            if let Some(obj_map) = self.pos_index.get(&predicate_key) {
                if let Some(mut ids) = obj_map.get_mut(&object_key) {
                    ids.remove(&id);
                }
            }

            // Remove from OSP index
            if let Some(subj_map) = self.osp_index.get(&object_key) {
                if let Some(mut ids) = subj_map.get_mut(&subject_key) {
                    ids.remove(&id);
                }
            }

            // Remove from graphs
            self.default_graph.remove(&id);
            for graph in self.graphs.iter() {
                if let Some(mut ids) = self.graphs.get_mut(graph.key()) {
                    ids.remove(&id);
                }
            }

            Some(triple)
        } else {
            None
        }
    }

    /// Get a triple by ID
    pub fn get(&self, id: u64) -> Option<Triple> {
        self.triples.get(&id).map(|t| t.clone())
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
            self.graphs
                .get(g)
                .map(|ids| ids.clone())
                .unwrap_or_default()
        });

        // Choose the best index based on bound variables
        let ids = match (subject, predicate, object) {
            // All bound - direct lookup
            (Some(s), Some(p), Some(o)) => {
                let s_key = term_to_key(s);
                let p_key = p.as_str();
                let o_key = term_to_key(o);

                self.spo_index
                    .get(&s_key)
                    .and_then(|pred_map| pred_map.get(p_key).map(|ids| ids.clone()))
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|id| {
                        self.triples
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

                self.spo_index
                    .get(&s_key)
                    .and_then(|pred_map| pred_map.get(p_key).map(|ids| ids.clone()))
                    .unwrap_or_default()
                    .into_iter()
                    .collect()
            }

            // Subject only - use SPO
            (Some(s), None, None) => {
                let s_key = term_to_key(s);

                self.spo_index
                    .get(&s_key)
                    .map(|pred_map| {
                        pred_map
                            .iter()
                            .flat_map(|entry| entry.value().clone())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            }

            // Predicate and object bound - use POS
            (None, Some(p), Some(o)) => {
                let p_key = p.as_str();
                let o_key = term_to_key(o);

                self.pos_index
                    .get(p_key)
                    .and_then(|obj_map| obj_map.get(&o_key).map(|ids| ids.clone()))
                    .unwrap_or_default()
                    .into_iter()
                    .collect()
            }

            // Predicate only - use POS
            (None, Some(p), None) => {
                let p_key = p.as_str();

                self.pos_index
                    .get(p_key)
                    .map(|obj_map| {
                        obj_map
                            .iter()
                            .flat_map(|entry| entry.value().clone())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            }

            // Object only - use OSP
            (None, None, Some(o)) => {
                let o_key = term_to_key(o);

                self.osp_index
                    .get(&o_key)
                    .map(|subj_map| {
                        subj_map
                            .iter()
                            .flat_map(|entry| entry.value().clone())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            }

            // Subject and object bound - use SPO then filter
            (Some(s), None, Some(o)) => {
                let s_key = term_to_key(s);
                let o_key = term_to_key(o);

                self.spo_index
                    .get(&s_key)
                    .map(|pred_map| {
                        pred_map
                            .iter()
                            .flat_map(|entry| entry.value().clone())
                            .filter(|id| {
                                self.triples
                                    .get(id)
                                    .map(|t| term_to_key(&t.object) == o_key)
                                    .unwrap_or(false)
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            }

            // Nothing bound - return all
            (None, None, None) => self.triples.iter().map(|entry| *entry.key()).collect(),
        };

        // Apply graph filter and collect results
        ids.into_iter()
            .filter(|id| {
                graph_filter
                    .as_ref()
                    .map(|filter| filter.contains(id))
                    .unwrap_or(true)
            })
            .filter_map(|id| self.triples.get(&id).map(|t| t.clone()))
            .collect()
    }

    /// Get all triples in the store
    pub fn all_triples(&self) -> Vec<Triple> {
        self.triples
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get triple count
    pub fn count(&self) -> usize {
        self.triples.len()
    }

    /// Check if store is empty
    pub fn is_empty(&self) -> bool {
        self.triples.is_empty()
    }

    /// Clear all triples
    pub fn clear(&self) {
        self.triples.clear();
        self.spo_index.clear();
        self.pos_index.clear();
        self.osp_index.clear();
        self.graphs.clear();
        self.default_graph.clear();
        self.subjects.clear();
        self.predicates.clear();
        self.objects.clear();
    }

    /// Clear a specific graph
    pub fn clear_graph(&self, graph: Option<&str>) {
        let ids_to_remove: Vec<u64> = if let Some(graph_iri) = graph {
            self.graphs
                .get(graph_iri)
                .map(|ids| ids.iter().copied().collect())
                .unwrap_or_default()
        } else {
            self.default_graph
                .iter()
                .map(|entry| *entry.key())
                .collect()
        };

        for id in ids_to_remove {
            self.remove(id);
        }
    }

    /// Get statistics about the store
    pub fn stats(&self) -> StoreStats {
        StoreStats {
            triple_count: self.triples.len() as u64,
            subject_count: self.subjects.len(),
            predicate_count: self.predicates.len(),
            object_count: self.objects.len(),
            graph_count: self.graphs.len() + 1, // +1 for default graph
        }
    }

    /// List all named graphs
    pub fn list_graphs(&self) -> Vec<String> {
        self.graphs
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Get triples from a specific graph
    pub fn get_graph(&self, graph: &str) -> Vec<Triple> {
        self.graphs
            .get(graph)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.triples.get(id).map(|t| t.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get triples from the default graph
    pub fn get_default_graph(&self) -> Vec<Triple> {
        self.default_graph
            .iter()
            .filter_map(|entry| self.triples.get(entry.key()).map(|t| t.clone()))
            .collect()
    }

    /// Bulk insert triples
    pub fn insert_bulk(&self, triples: impl IntoIterator<Item = Triple>) -> Vec<u64> {
        triples.into_iter().map(|t| self.insert(t)).collect()
    }

    /// Bulk insert triples into a graph
    pub fn insert_bulk_into_graph(
        &self,
        triples: impl IntoIterator<Item = Triple>,
        graph: &str,
    ) -> Vec<u64> {
        triples
            .into_iter()
            .map(|t| self.insert_into_graph(t, Some(graph)))
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
    fn test_query_by_predicate() {
        let store = TripleStore::new();

        store.insert(Triple::new(
            RdfTerm::iri("http://example.org/person/1"),
            Iri::rdf_type(),
            RdfTerm::iri("http://example.org/Person"),
        ));
        store.insert(Triple::new(
            RdfTerm::iri("http://example.org/person/2"),
            Iri::rdf_type(),
            RdfTerm::iri("http://example.org/Person"),
        ));
        store.insert(Triple::new(
            RdfTerm::iri("http://example.org/person/1"),
            Iri::rdfs_label(),
            RdfTerm::literal("Alice"),
        ));

        let results = store.query(None, Some(&Iri::rdf_type()), None);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_named_graphs() {
        let store = TripleStore::new();

        let triple = Triple::new(
            RdfTerm::iri("http://example.org/person/1"),
            Iri::rdf_type(),
            RdfTerm::iri("http://example.org/Person"),
        );

        store.insert_into_graph(triple.clone(), Some("http://example.org/graph1"));

        let graph_triples = store.get_graph("http://example.org/graph1");
        assert_eq!(graph_triples.len(), 1);

        let default_triples = store.get_default_graph();
        assert_eq!(default_triples.len(), 0);

        let graphs = store.list_graphs();
        assert!(graphs.contains(&"http://example.org/graph1".to_string()));
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

    #[test]
    fn test_remove() {
        let store = TripleStore::new();

        let id = store.insert(Triple::new(
            RdfTerm::iri("http://example.org/s"),
            Iri::new("http://example.org/p"),
            RdfTerm::literal("o"),
        ));

        assert_eq!(store.count(), 1);

        let removed = store.remove(id);
        assert!(removed.is_some());
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn test_parse_literal() {
        let simple = parse_literal_string("\"hello\"");
        assert!(matches!(simple, RdfTerm::Literal(ref l) if l.value == "hello"));

        let lang = parse_literal_string("\"hello\"@en");
        assert!(matches!(lang, RdfTerm::Literal(ref l) if l.language == Some("en".to_string())));

        let typed = parse_literal_string("\"42\"^^<http://www.w3.org/2001/XMLSchema#integer>");
        assert!(
            matches!(typed, RdfTerm::Literal(ref l) if l.datatype.as_str() == "http://www.w3.org/2001/XMLSchema#integer")
        );
    }
}
