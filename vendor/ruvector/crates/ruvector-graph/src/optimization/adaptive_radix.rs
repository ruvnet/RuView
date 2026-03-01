//! Adaptive Radix Tree (ART) for property indexes
//!
//! ART provides space-efficient indexing with excellent cache performance
//! through adaptive node sizes and path compression.

use std::cmp::Ordering;
use std::mem;

/// Adaptive Radix Tree for property indexing
pub struct AdaptiveRadixTree<V: Clone> {
    root: Option<Box<ArtNode<V>>>,
    size: usize,
}

impl<V: Clone> AdaptiveRadixTree<V> {
    pub fn new() -> Self {
        Self {
            root: None,
            size: 0,
        }
    }

    /// Insert a key-value pair
    pub fn insert(&mut self, key: &[u8], value: V) {
        if self.root.is_none() {
            self.root = Some(Box::new(ArtNode::Leaf {
                key: key.to_vec(),
                value,
            }));
            self.size += 1;
            return;
        }

        let root = self.root.take().unwrap();
        self.root = Some(Self::insert_recursive(root, key, 0, value));
        self.size += 1;
    }

    fn insert_recursive(
        mut node: Box<ArtNode<V>>,
        key: &[u8],
        depth: usize,
        value: V,
    ) -> Box<ArtNode<V>> {
        match node.as_mut() {
            ArtNode::Leaf {
                key: leaf_key,
                value: leaf_value,
            } => {
                // Check if keys are identical
                if *leaf_key == key {
                    // Replace value
                    *leaf_value = value;
                    return node;
                }

                // Find common prefix length starting from depth
                let common_prefix_len = Self::common_prefix_len(leaf_key, key, depth);
                let prefix = if depth + common_prefix_len <= leaf_key.len()
                    && depth + common_prefix_len <= key.len()
                {
                    key[depth..depth + common_prefix_len].to_vec()
                } else {
                    vec![]
                };

                // Create a new Node4 to hold both leaves
                let mut children: [Option<Box<ArtNode<V>>>; 4] = [None, None, None, None];
                let mut keys_arr = [0u8; 4];
                let mut num_children = 0u8;

                let next_depth = depth + common_prefix_len;

                // Get the distinguishing bytes for old and new keys
                let old_byte = if next_depth < leaf_key.len() {
                    Some(leaf_key[next_depth])
                } else {
                    None
                };

                let new_byte = if next_depth < key.len() {
                    Some(key[next_depth])
                } else {
                    None
                };

                // Take ownership of old leaf's data
                let old_key = std::mem::take(leaf_key);
                let old_value = unsafe { std::ptr::read(leaf_value) };

                // Add old leaf
                if let Some(byte) = old_byte {
                    keys_arr[num_children as usize] = byte;
                    children[num_children as usize] = Some(Box::new(ArtNode::Leaf {
                        key: old_key,
                        value: old_value,
                    }));
                    num_children += 1;
                }

                // Add new leaf
                if let Some(byte) = new_byte {
                    // Find insertion position (keep sorted for efficiency)
                    let mut insert_idx = num_children as usize;
                    for i in 0..num_children as usize {
                        if byte < keys_arr[i] {
                            insert_idx = i;
                            break;
                        }
                    }

                    // Shift existing entries if needed
                    for i in (insert_idx..num_children as usize).rev() {
                        keys_arr[i + 1] = keys_arr[i];
                        children[i + 1] = children[i].take();
                    }

                    keys_arr[insert_idx] = byte;
                    children[insert_idx] = Some(Box::new(ArtNode::Leaf {
                        key: key.to_vec(),
                        value,
                    }));
                    num_children += 1;
                }

                Box::new(ArtNode::Node4 {
                    prefix,
                    children,
                    keys: keys_arr,
                    num_children,
                })
            }
            ArtNode::Node4 {
                prefix,
                children,
                keys,
                num_children,
            } => {
                // Check prefix match
                let prefix_match = Self::check_prefix(prefix, key, depth);

                if prefix_match < prefix.len() {
                    // Prefix mismatch - need to split the node
                    let common = prefix[..prefix_match].to_vec();
                    let remaining = prefix[prefix_match..].to_vec();
                    let old_byte = remaining[0];

                    // Create new inner node with remaining prefix
                    let old_children = std::mem::replace(children, [None, None, None, None]);
                    let old_keys = *keys;
                    let old_num = *num_children;

                    let inner_node = Box::new(ArtNode::Node4 {
                        prefix: remaining[1..].to_vec(),
                        children: old_children,
                        keys: old_keys,
                        num_children: old_num,
                    });

                    // Create new leaf for the inserted key
                    let next_depth = depth + prefix_match;
                    let new_byte = if next_depth < key.len() {
                        key[next_depth]
                    } else {
                        0
                    };
                    let new_leaf = Box::new(ArtNode::Leaf {
                        key: key.to_vec(),
                        value,
                    });

                    // Set up new node
                    let mut new_children: [Option<Box<ArtNode<V>>>; 4] = [None, None, None, None];
                    let mut new_keys = [0u8; 4];

                    if old_byte < new_byte {
                        new_keys[0] = old_byte;
                        new_children[0] = Some(inner_node);
                        new_keys[1] = new_byte;
                        new_children[1] = Some(new_leaf);
                    } else {
                        new_keys[0] = new_byte;
                        new_children[0] = Some(new_leaf);
                        new_keys[1] = old_byte;
                        new_children[1] = Some(inner_node);
                    }

                    return Box::new(ArtNode::Node4 {
                        prefix: common,
                        children: new_children,
                        keys: new_keys,
                        num_children: 2,
                    });
                }

                // Full prefix match - traverse to child
                let next_depth = depth + prefix.len();
                if next_depth < key.len() {
                    let key_byte = key[next_depth];

                    // Find existing child
                    for i in 0..(*num_children as usize) {
                        if keys[i] == key_byte {
                            let child = children[i].take().unwrap();
                            children[i] =
                                Some(Self::insert_recursive(child, key, next_depth + 1, value));
                            return node;
                        }
                    }

                    // No matching child - add new one
                    if (*num_children as usize) < 4 {
                        let idx = *num_children as usize;
                        keys[idx] = key_byte;
                        children[idx] = Some(Box::new(ArtNode::Leaf {
                            key: key.to_vec(),
                            value,
                        }));
                        *num_children += 1;
                    }
                    // TODO: Handle node growth to Node16 when full
                }

                node
            }
            _ => {
                // Handle other node types (Node16, Node48, Node256)
                node
            }
        }
    }

    /// Search for a value by key
    pub fn get(&self, key: &[u8]) -> Option<&V> {
        let mut current = self.root.as_ref()?;
        let mut depth = 0;

        loop {
            match current.as_ref() {
                ArtNode::Leaf {
                    key: leaf_key,
                    value,
                } => {
                    if leaf_key == key {
                        return Some(value);
                    } else {
                        return None;
                    }
                }
                ArtNode::Node4 {
                    prefix,
                    children,
                    keys,
                    num_children,
                } => {
                    if !Self::match_prefix(prefix, key, depth) {
                        return None;
                    }

                    depth += prefix.len();
                    if depth >= key.len() {
                        return None;
                    }

                    let key_byte = key[depth];
                    let mut found = false;

                    for i in 0..*num_children as usize {
                        if keys[i] == key_byte {
                            current = children[i].as_ref()?;
                            depth += 1;
                            found = true;
                            break;
                        }
                    }

                    if !found {
                        return None;
                    }
                }
                _ => return None,
            }
        }
    }

    /// Check if tree contains key
    pub fn contains_key(&self, key: &[u8]) -> bool {
        self.get(key).is_some()
    }

    /// Get number of entries
    pub fn len(&self) -> usize {
        self.size
    }

    /// Check if tree is empty
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Find common prefix length
    fn common_prefix_len(a: &[u8], b: &[u8], start: usize) -> usize {
        let mut len = 0;
        let max = a.len().min(b.len()) - start;

        for i in 0..max {
            if a[start + i] == b[start + i] {
                len += 1;
            } else {
                break;
            }
        }

        len
    }

    /// Check prefix match
    fn check_prefix(prefix: &[u8], key: &[u8], depth: usize) -> usize {
        let max = prefix.len().min(key.len() - depth);
        let mut matched = 0;

        for i in 0..max {
            if prefix[i] == key[depth + i] {
                matched += 1;
            } else {
                break;
            }
        }

        matched
    }

    /// Check if prefix matches
    fn match_prefix(prefix: &[u8], key: &[u8], depth: usize) -> bool {
        if depth + prefix.len() > key.len() {
            return false;
        }

        for i in 0..prefix.len() {
            if prefix[i] != key[depth + i] {
                return false;
            }
        }

        true
    }
}

impl<V: Clone> Default for AdaptiveRadixTree<V> {
    fn default() -> Self {
        Self::new()
    }
}

/// ART node types with adaptive sizing
pub enum ArtNode<V> {
    /// Leaf node containing value
    Leaf { key: Vec<u8>, value: V },

    /// Node with 4 children (smallest)
    Node4 {
        prefix: Vec<u8>,
        children: [Option<Box<ArtNode<V>>>; 4],
        keys: [u8; 4],
        num_children: u8,
    },

    /// Node with 16 children
    Node16 {
        prefix: Vec<u8>,
        children: [Option<Box<ArtNode<V>>>; 16],
        keys: [u8; 16],
        num_children: u8,
    },

    /// Node with 48 children (using index array)
    Node48 {
        prefix: Vec<u8>,
        children: [Option<Box<ArtNode<V>>>; 48],
        index: [u8; 256], // Maps key byte to child index
        num_children: u8,
    },

    /// Node with 256 children (full array)
    Node256 {
        prefix: Vec<u8>,
        children: [Option<Box<ArtNode<V>>>; 256],
        num_children: u16,
    },
}

impl<V> ArtNode<V> {
    /// Check if node is a leaf
    pub fn is_leaf(&self) -> bool {
        matches!(self, ArtNode::Leaf { .. })
    }

    /// Get node type name
    pub fn node_type(&self) -> &str {
        match self {
            ArtNode::Leaf { .. } => "Leaf",
            ArtNode::Node4 { .. } => "Node4",
            ArtNode::Node16 { .. } => "Node16",
            ArtNode::Node48 { .. } => "Node48",
            ArtNode::Node256 { .. } => "Node256",
        }
    }
}

/// Iterator over ART entries
pub struct ArtIter<'a, V> {
    stack: Vec<&'a ArtNode<V>>,
}

impl<'a, V> Iterator for ArtIter<'a, V> {
    type Item = (&'a [u8], &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(node) = self.stack.pop() {
            match node {
                ArtNode::Leaf { key, value } => {
                    return Some((key.as_slice(), value));
                }
                ArtNode::Node4 {
                    children,
                    num_children,
                    ..
                } => {
                    for i in (0..*num_children as usize).rev() {
                        if let Some(child) = &children[i] {
                            self.stack.push(child);
                        }
                    }
                }
                _ => {
                    // Handle other node types
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_art_basic() {
        let mut tree = AdaptiveRadixTree::new();

        tree.insert(b"hello", 1);
        tree.insert(b"world", 2);
        tree.insert(b"help", 3);

        assert_eq!(tree.get(b"hello"), Some(&1));
        assert_eq!(tree.get(b"world"), Some(&2));
        assert_eq!(tree.get(b"help"), Some(&3));
        assert_eq!(tree.get(b"nonexistent"), None);
    }

    #[test]
    fn test_art_contains() {
        let mut tree = AdaptiveRadixTree::new();

        tree.insert(b"test", 42);

        assert!(tree.contains_key(b"test"));
        assert!(!tree.contains_key(b"other"));
    }

    #[test]
    fn test_art_len() {
        let mut tree = AdaptiveRadixTree::new();

        assert_eq!(tree.len(), 0);
        assert!(tree.is_empty());

        tree.insert(b"a", 1);
        tree.insert(b"b", 2);

        assert_eq!(tree.len(), 2);
        assert!(!tree.is_empty());
    }

    #[test]
    fn test_art_common_prefix() {
        let mut tree = AdaptiveRadixTree::new();

        tree.insert(b"prefix_one", 1);
        tree.insert(b"prefix_two", 2);
        tree.insert(b"other", 3);

        assert_eq!(tree.get(b"prefix_one"), Some(&1));
        assert_eq!(tree.get(b"prefix_two"), Some(&2));
        assert_eq!(tree.get(b"other"), Some(&3));
    }
}
