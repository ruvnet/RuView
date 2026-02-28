//! DAG traversal algorithms and iterators

use std::collections::{HashSet, VecDeque};

use super::query_dag::{DagError, QueryDag};

/// Iterator for topological order traversal (dependencies first)
pub struct TopologicalIterator<'a> {
    #[allow(dead_code)]
    dag: &'a QueryDag,
    sorted: Vec<usize>,
    index: usize,
}

impl<'a> TopologicalIterator<'a> {
    pub(crate) fn new(dag: &'a QueryDag) -> Result<Self, DagError> {
        let sorted = dag.topological_sort()?;
        Ok(Self {
            dag,
            sorted,
            index: 0,
        })
    }
}

impl<'a> Iterator for TopologicalIterator<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.sorted.len() {
            let id = self.sorted[self.index];
            self.index += 1;
            Some(id)
        } else {
            None
        }
    }
}

/// Iterator for depth-first search traversal
pub struct DfsIterator<'a> {
    dag: &'a QueryDag,
    stack: Vec<usize>,
    visited: HashSet<usize>,
}

impl<'a> DfsIterator<'a> {
    pub(crate) fn new(dag: &'a QueryDag, start: usize) -> Self {
        let mut stack = Vec::new();
        let visited = HashSet::new();

        if dag.get_node(start).is_some() {
            stack.push(start);
        }

        Self {
            dag,
            stack,
            visited,
        }
    }
}

impl<'a> Iterator for DfsIterator<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(node) = self.stack.pop() {
            if self.visited.insert(node) {
                // Add children to stack (in reverse order so they're processed in order)
                if let Some(children) = self.dag.edges.get(&node) {
                    for &child in children.iter().rev() {
                        if !self.visited.contains(&child) {
                            self.stack.push(child);
                        }
                    }
                }
                return Some(node);
            }
        }
        None
    }
}

/// Iterator for breadth-first search traversal
pub struct BfsIterator<'a> {
    dag: &'a QueryDag,
    queue: VecDeque<usize>,
    visited: HashSet<usize>,
}

impl<'a> BfsIterator<'a> {
    pub(crate) fn new(dag: &'a QueryDag, start: usize) -> Self {
        let mut queue = VecDeque::new();
        let visited = HashSet::new();

        if dag.get_node(start).is_some() {
            queue.push_back(start);
        }

        Self {
            dag,
            queue,
            visited,
        }
    }
}

impl<'a> Iterator for BfsIterator<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(node) = self.queue.pop_front() {
            if self.visited.insert(node) {
                // Add children to queue
                if let Some(children) = self.dag.edges.get(&node) {
                    for &child in children {
                        if !self.visited.contains(&child) {
                            self.queue.push_back(child);
                        }
                    }
                }
                return Some(node);
            }
        }
        None
    }
}

impl QueryDag {
    /// Create an iterator for topological order traversal
    pub fn topological_iter(&self) -> Result<TopologicalIterator<'_>, DagError> {
        TopologicalIterator::new(self)
    }

    /// Create an iterator for depth-first search starting from a node
    pub fn dfs_iter(&self, start: usize) -> DfsIterator<'_> {
        DfsIterator::new(self, start)
    }

    /// Create an iterator for breadth-first search starting from a node
    pub fn bfs_iter(&self, start: usize) -> BfsIterator<'_> {
        BfsIterator::new(self, start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OperatorNode;

    fn create_test_dag() -> QueryDag {
        let mut dag = QueryDag::new();
        let id1 = dag.add_node(OperatorNode::seq_scan(0, "users"));
        let id2 = dag.add_node(OperatorNode::filter(0, "age > 18"));
        let id3 = dag.add_node(OperatorNode::sort(0, vec!["name".to_string()]));
        let id4 = dag.add_node(OperatorNode::limit(0, 10));

        dag.add_edge(id1, id2).unwrap();
        dag.add_edge(id2, id3).unwrap();
        dag.add_edge(id3, id4).unwrap();

        dag
    }

    #[test]
    fn test_topological_iterator() {
        let dag = create_test_dag();
        let nodes: Vec<usize> = dag.topological_iter().unwrap().collect();

        assert_eq!(nodes.len(), 4);

        // Check ordering constraints
        let pos: Vec<usize> = (0..4)
            .map(|i| nodes.iter().position(|&x| x == i).unwrap())
            .collect();

        assert!(pos[0] < pos[1]); // 0 before 1
        assert!(pos[1] < pos[2]); // 1 before 2
        assert!(pos[2] < pos[3]); // 2 before 3
    }

    #[test]
    fn test_dfs_iterator() {
        let dag = create_test_dag();
        let nodes: Vec<usize> = dag.dfs_iter(0).collect();

        assert_eq!(nodes.len(), 4);
        assert_eq!(nodes[0], 0); // Should start from node 0
    }

    #[test]
    fn test_bfs_iterator() {
        let dag = create_test_dag();
        let nodes: Vec<usize> = dag.bfs_iter(0).collect();

        assert_eq!(nodes.len(), 4);
        assert_eq!(nodes[0], 0); // Should start from node 0
    }

    #[test]
    fn test_branching_dag() {
        let mut dag = QueryDag::new();
        let root = dag.add_node(OperatorNode::seq_scan(0, "users"));
        let left1 = dag.add_node(OperatorNode::filter(0, "age > 18"));
        let left2 = dag.add_node(OperatorNode::project(0, vec!["name".to_string()]));
        let right1 = dag.add_node(OperatorNode::filter(0, "active = true"));
        let join = dag.add_node(OperatorNode::hash_join(0, "id"));

        dag.add_edge(root, left1).unwrap();
        dag.add_edge(left1, left2).unwrap();
        dag.add_edge(root, right1).unwrap();
        dag.add_edge(left2, join).unwrap();
        dag.add_edge(right1, join).unwrap();

        // BFS should visit level by level
        let bfs_nodes: Vec<usize> = dag.bfs_iter(root).collect();
        assert_eq!(bfs_nodes.len(), 5);

        // Topological sort should respect dependencies
        let topo_nodes = dag.topological_sort().unwrap();
        assert_eq!(topo_nodes.len(), 5);

        let pos_root = topo_nodes.iter().position(|&x| x == root).unwrap();
        let pos_join = topo_nodes.iter().position(|&x| x == join).unwrap();
        assert!(pos_root < pos_join);
    }
}
