//! DAG Generator for testing

use ruvector_dag::dag::{QueryDag, OperatorNode, OperatorType};
use rand::Rng;

#[derive(Debug, Clone, Copy)]
pub enum DagComplexity {
    Simple,      // 3-5 nodes, linear
    Medium,      // 10-20 nodes, some branches
    Complex,     // 50-100 nodes, many branches
    VectorQuery, // Typical vector search pattern
}

pub struct DagGenerator {
    rng: rand::rngs::ThreadRng,
}

impl DagGenerator {
    pub fn new() -> Self {
        Self {
            rng: rand::thread_rng(),
        }
    }

    pub fn generate(&mut self, complexity: DagComplexity) -> QueryDag {
        match complexity {
            DagComplexity::Simple => self.generate_simple(),
            DagComplexity::Medium => self.generate_medium(),
            DagComplexity::Complex => self.generate_complex(),
            DagComplexity::VectorQuery => self.generate_vector_query(),
        }
    }

    fn generate_simple(&mut self) -> QueryDag {
        let mut dag = QueryDag::new();

        // Simple: Scan -> Filter -> Result
        let scan = dag.add_node(OperatorNode::seq_scan(0, "users"));
        let filter = dag.add_node(OperatorNode::filter(1, "id > 0"));
        let result = dag.add_node(OperatorNode::new(2, OperatorType::Result));

        dag.add_edge(scan, filter).unwrap();
        dag.add_edge(filter, result).unwrap();

        dag
    }

    fn generate_medium(&mut self) -> QueryDag {
        let mut dag = QueryDag::new();
        let mut id = 0;

        // Two table join with aggregation
        let scan1 = dag.add_node(OperatorNode::seq_scan(id, "orders")); id += 1;
        let scan2 = dag.add_node(OperatorNode::seq_scan(id, "products")); id += 1;

        let join = dag.add_node(OperatorNode::hash_join(id, "product_id")); id += 1;
        dag.add_edge(scan1, join).unwrap();
        dag.add_edge(scan2, join).unwrap();

        let filter = dag.add_node(OperatorNode::filter(id, "amount > 100")); id += 1;
        dag.add_edge(join, filter).unwrap();

        let agg = dag.add_node(OperatorNode::new(id, OperatorType::Aggregate {
            functions: vec!["SUM(amount)".to_string()],
        })); id += 1;
        dag.add_edge(filter, agg).unwrap();

        let sort = dag.add_node(OperatorNode::sort(id, vec!["total".to_string()])); id += 1;
        dag.add_edge(agg, sort).unwrap();

        let limit = dag.add_node(OperatorNode::limit(id, 10)); id += 1;
        dag.add_edge(sort, limit).unwrap();

        let result = dag.add_node(OperatorNode::new(id, OperatorType::Result));
        dag.add_edge(limit, result).unwrap();

        dag
    }

    fn generate_complex(&mut self) -> QueryDag {
        let mut dag = QueryDag::new();
        let node_count = self.rng.gen_range(50..100);

        // Generate nodes
        for i in 0..node_count {
            let op_type = self.random_operator_type(i);
            let mut node = OperatorNode::new(i, op_type);
            node.estimated_cost = self.rng.gen_range(1.0..1000.0);
            node.estimated_rows = self.rng.gen_range(1.0..100000.0);
            dag.add_node(node);
        }

        // Generate edges (ensuring DAG property)
        for i in 1..node_count {
            let parent_count = self.rng.gen_range(1..=2.min(i));
            for _ in 0..parent_count {
                let parent = self.rng.gen_range(0..i);
                let _ = dag.add_edge(parent, i);
            }
        }

        dag
    }

    fn generate_vector_query(&mut self) -> QueryDag {
        let mut dag = QueryDag::new();
        let mut id = 0;

        // Vector search with join to metadata
        let hnsw = dag.add_node(OperatorNode::hnsw_scan(id, "vectors_idx", 64)); id += 1;
        let meta_scan = dag.add_node(OperatorNode::seq_scan(id, "metadata")); id += 1;

        let join = dag.add_node(OperatorNode::new(id, OperatorType::NestedLoopJoin)); id += 1;
        dag.add_edge(hnsw, join).unwrap();
        dag.add_edge(meta_scan, join).unwrap();

        let rerank = dag.add_node(OperatorNode::new(id, OperatorType::Rerank {
            model: "cross-encoder".to_string(),
        })); id += 1;
        dag.add_edge(join, rerank).unwrap();

        let limit = dag.add_node(OperatorNode::limit(id, 10)); id += 1;
        dag.add_edge(rerank, limit).unwrap();

        let result = dag.add_node(OperatorNode::new(id, OperatorType::Result));
        dag.add_edge(limit, result).unwrap();

        dag
    }

    fn random_operator_type(&mut self, id: usize) -> OperatorType {
        match self.rng.gen_range(0..10) {
            0 => OperatorType::SeqScan { table: format!("table_{}", id) },
            1 => OperatorType::IndexScan {
                index: format!("idx_{}", id),
                table: format!("table_{}", id)
            },
            2 => OperatorType::HnswScan {
                index: format!("hnsw_{}", id),
                ef_search: 64
            },
            3 => OperatorType::HashJoin {
                hash_key: "id".to_string()
            },
            4 => OperatorType::Filter {
                predicate: "x > 0".to_string()
            },
            5 => OperatorType::Sort {
                keys: vec!["col1".to_string()],
                descending: vec![false]
            },
            6 => OperatorType::Limit { count: 100 },
            7 => OperatorType::Aggregate {
                functions: vec!["COUNT(*)".to_string()]
            },
            8 => OperatorType::Project {
                columns: vec!["a".to_string(), "b".to_string()]
            },
            _ => OperatorType::Result,
        }
    }
}

impl Default for DagGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a batch of DAGs
pub fn generate_dag_batch(count: usize, complexity: DagComplexity) -> Vec<QueryDag> {
    let mut gen = DagGenerator::new();
    (0..count).map(|_| gen.generate(complexity)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_simple() {
        let mut gen = DagGenerator::new();
        let dag = gen.generate(DagComplexity::Simple);
        assert_eq!(dag.nodes.len(), 3);
        assert_eq!(dag.edges.len(), 2);
    }

    #[test]
    fn test_generate_medium() {
        let mut gen = DagGenerator::new();
        let dag = gen.generate(DagComplexity::Medium);
        assert!(dag.nodes.len() >= 5);
        assert!(dag.nodes.len() <= 20);
    }

    #[test]
    fn test_generate_vector_query() {
        let mut gen = DagGenerator::new();
        let dag = gen.generate(DagComplexity::VectorQuery);

        // Should have HNSW scan node
        let has_hnsw = dag.nodes.iter().any(|n| matches!(n.op_type, OperatorType::HnswScan { .. }));
        assert!(has_hnsw);
    }

    #[test]
    fn test_generate_batch() {
        let dags = generate_dag_batch(10, DagComplexity::Simple);
        assert_eq!(dags.len(), 10);
    }
}
