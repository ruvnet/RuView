//! Witness types for cut certification
//!
//! A witness represents a connected set U ⊆ V with its boundary δ(U).
//! The witness certifies that a proper cut exists with value |δ(U)|.
//!
//! # Representation
//!
//! Witnesses use an implicit representation for memory efficiency:
//! - **Seed vertex**: The starting vertex that defines the connected component
//! - **Membership bitmap**: Compressed bitmap indicating which vertices are in U
//! - **Boundary size**: Pre-computed value |δ(U)| for O(1) queries
//! - **Hash**: Fast equality checking without full comparison
//!
//! # Performance
//!
//! - `WitnessHandle` uses `Arc` for cheap cloning (O(1))
//! - `contains()` is O(1) via bitmap lookup
//! - `boundary_size()` is O(1) via cached value
//! - `materialize_partition()` is O(|V|) and should be used sparingly

use crate::graph::VertexId;
use roaring::RoaringBitmap;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Handle to a witness (cheap to clone)
///
/// This is the primary type for passing witnesses around. It uses an `Arc`
/// internally so cloning is O(1) and witnesses can be shared across threads.
///
/// # Examples
///
/// ```
/// use ruvector_mincut::instance::witness::WitnessHandle;
/// use roaring::RoaringBitmap;
///
/// let mut membership = RoaringBitmap::new();
/// membership.insert(1);
/// membership.insert(2);
/// membership.insert(3);
///
/// let witness = WitnessHandle::new(1, membership, 4);
/// assert!(witness.contains(1));
/// assert!(witness.contains(2));
/// assert!(!witness.contains(5));
/// assert_eq!(witness.boundary_size(), 4);
/// ```
#[derive(Debug, Clone)]
pub struct WitnessHandle {
    inner: Arc<ImplicitWitness>,
}

/// Implicit representation of a cut witness
///
/// The witness represents a connected set U ⊆ V where:
/// - U contains the seed vertex
/// - |δ(U)| = boundary_size
/// - membership\[v\] = true iff v ∈ U
#[derive(Debug)]
pub struct ImplicitWitness {
    /// Seed vertex that defines the cut (always in U)
    pub seed: VertexId,
    /// Membership bitmap (vertex v is in U iff bit v is set)
    pub membership: RoaringBitmap,
    /// Current boundary size |δ(U)|
    pub boundary_size: u64,
    /// Hash for quick equality checks
    pub hash: u64,
}

impl WitnessHandle {
    /// Create a new witness handle
    ///
    /// # Arguments
    ///
    /// * `seed` - The seed vertex defining this cut (must be in membership)
    /// * `membership` - Bitmap of vertices in the cut set U
    /// * `boundary_size` - The size of the boundary |δ(U)|
    ///
    /// # Panics
    ///
    /// Panics if the seed vertex is not in the membership set (debug builds only)
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut::instance::witness::WitnessHandle;
    /// use roaring::RoaringBitmap;
    ///
    /// let mut membership = RoaringBitmap::new();
    /// membership.insert(0);
    /// membership.insert(1);
    ///
    /// let witness = WitnessHandle::new(0, membership, 5);
    /// assert_eq!(witness.seed(), 0);
    /// ```
    pub fn new(seed: VertexId, membership: RoaringBitmap, boundary_size: u64) -> Self {
        debug_assert!(
            seed <= u32::MAX as u64,
            "Seed vertex {} exceeds u32::MAX",
            seed
        );
        debug_assert!(
            membership.contains(seed as u32),
            "Seed vertex {} must be in membership set",
            seed
        );

        let hash = Self::compute_hash(seed, &membership);

        Self {
            inner: Arc::new(ImplicitWitness {
                seed,
                membership,
                boundary_size,
                hash,
            }),
        }
    }

    /// Compute hash for a witness
    ///
    /// The hash combines the seed vertex and membership bitmap for fast equality checks.
    fn compute_hash(seed: VertexId, membership: &RoaringBitmap) -> u64 {
        let mut hasher = DefaultHasher::new();
        seed.hash(&mut hasher);

        // Hash the membership bitmap by iterating its values
        for vertex in membership.iter() {
            vertex.hash(&mut hasher);
        }

        hasher.finish()
    }

    /// Check if vertex is in the cut set U
    ///
    /// # Time Complexity
    ///
    /// O(1) via bitmap lookup
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut::instance::witness::WitnessHandle;
    /// use roaring::RoaringBitmap;
    ///
    /// let mut membership = RoaringBitmap::new();
    /// membership.insert(5);
    /// membership.insert(10);
    ///
    /// let witness = WitnessHandle::new(5, membership, 3);
    /// assert!(witness.contains(5));
    /// assert!(witness.contains(10));
    /// assert!(!witness.contains(15));
    /// ```
    #[inline]
    pub fn contains(&self, v: VertexId) -> bool {
        if v > u32::MAX as u64 {
            return false;
        }
        self.inner.membership.contains(v as u32)
    }

    /// Get boundary size |δ(U)|
    ///
    /// Returns the pre-computed boundary size for O(1) access.
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut::instance::witness::WitnessHandle;
    /// use roaring::RoaringBitmap;
    ///
    /// let witness = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2, 3]), 7);
    /// assert_eq!(witness.boundary_size(), 7);
    /// ```
    #[inline]
    pub fn boundary_size(&self) -> u64 {
        self.inner.boundary_size
    }

    /// Get the seed vertex
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut::instance::witness::WitnessHandle;
    /// use roaring::RoaringBitmap;
    ///
    /// let witness = WitnessHandle::new(42, RoaringBitmap::from_iter([42u32]), 1);
    /// assert_eq!(witness.seed(), 42);
    /// ```
    #[inline]
    pub fn seed(&self) -> VertexId {
        self.inner.seed
    }

    /// Get the witness hash
    ///
    /// Used for fast equality checks without comparing full membership sets.
    #[inline]
    pub fn hash(&self) -> u64 {
        self.inner.hash
    }

    /// Materialize full partition (U, V \ U)
    ///
    /// This is an expensive operation (O(|V|)) that converts the implicit
    /// representation into explicit sets. Use sparingly, primarily for
    /// debugging or verification.
    ///
    /// # Returns
    ///
    /// A tuple `(U, V_minus_U)` where:
    /// - `U` is the set of vertices in the cut
    /// - `V_minus_U` is the complement set
    ///
    /// # Note
    ///
    /// This method assumes vertices are numbered 0..max_vertex. For sparse
    /// graphs, V \ U may contain vertex IDs that don't exist in the graph.
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut::instance::witness::WitnessHandle;
    /// use roaring::RoaringBitmap;
    /// use std::collections::HashSet;
    ///
    /// let mut membership = RoaringBitmap::new();
    /// membership.insert(1);
    /// membership.insert(2);
    ///
    /// let witness = WitnessHandle::new(1, membership, 3);
    /// let (u, _v_minus_u) = witness.materialize_partition();
    ///
    /// assert!(u.contains(&1));
    /// assert!(u.contains(&2));
    /// assert!(!u.contains(&3));
    /// ```
    pub fn materialize_partition(&self) -> (HashSet<VertexId>, HashSet<VertexId>) {
        let u: HashSet<VertexId> = self.inner.membership.iter().map(|v| v as u64).collect();

        // Find the maximum vertex ID to determine graph size
        let max_vertex = self.inner.membership.max().unwrap_or(0) as u64;

        // Create complement set
        let v_minus_u: HashSet<VertexId> = (0..=max_vertex)
            .filter(|&v| !self.inner.membership.contains(v as u32))
            .collect();

        (u, v_minus_u)
    }

    /// Get the cardinality of the cut set U
    ///
    /// # Examples
    ///
    /// ```
    /// use ruvector_mincut::instance::witness::WitnessHandle;
    /// use roaring::RoaringBitmap;
    ///
    /// let witness = WitnessHandle::new(1, RoaringBitmap::from_iter([1u32, 2u32, 3u32]), 5);
    /// assert_eq!(witness.cardinality(), 3);
    /// ```
    #[inline]
    pub fn cardinality(&self) -> u64 {
        self.inner.membership.len()
    }
}

impl PartialEq for WitnessHandle {
    /// Fast equality check using hash
    ///
    /// First compares hashes (O(1)), then falls back to full comparison if needed.
    fn eq(&self, other: &Self) -> bool {
        // Fast path: compare hashes
        if self.inner.hash != other.inner.hash {
            return false;
        }

        // Slow path: compare actual membership
        self.inner.seed == other.inner.seed
            && self.inner.boundary_size == other.inner.boundary_size
            && self.inner.membership == other.inner.membership
    }
}

impl Eq for WitnessHandle {}

/// Trait for witness operations
///
/// This trait abstracts witness operations for generic programming.
/// The primary implementation is `WitnessHandle`.
pub trait Witness {
    /// Check if vertex is in the cut set U
    fn contains(&self, v: VertexId) -> bool;

    /// Get boundary size |δ(U)|
    fn boundary_size(&self) -> u64;

    /// Materialize full partition (expensive)
    fn materialize_partition(&self) -> (HashSet<VertexId>, HashSet<VertexId>);

    /// Get the seed vertex
    fn seed(&self) -> VertexId;

    /// Get cardinality of U
    fn cardinality(&self) -> u64;
}

impl Witness for WitnessHandle {
    #[inline]
    fn contains(&self, v: VertexId) -> bool {
        WitnessHandle::contains(self, v)
    }

    #[inline]
    fn boundary_size(&self) -> u64 {
        WitnessHandle::boundary_size(self)
    }

    fn materialize_partition(&self) -> (HashSet<VertexId>, HashSet<VertexId>) {
        WitnessHandle::materialize_partition(self)
    }

    #[inline]
    fn seed(&self) -> VertexId {
        WitnessHandle::seed(self)
    }

    #[inline]
    fn cardinality(&self) -> u64 {
        WitnessHandle::cardinality(self)
    }
}

/// Recipe for constructing a witness lazily
///
/// Instead of computing the full membership bitmap upfront, this struct
/// stores the parameters needed to construct it on demand. This is useful
/// when you need to store many potential witnesses but only access a few.
///
/// # Lazy Evaluation
///
/// The membership bitmap is only computed when:
/// - `materialize()` is called to get a full `WitnessHandle`
/// - `contains()` is called to check vertex membership
/// - Any other operation that requires the full witness
///
/// # Memory Savings
///
/// A `LazyWitness` uses only ~32 bytes vs potentially kilobytes for a
/// `WitnessHandle` with a large membership bitmap.
///
/// # Example
///
/// ```
/// use ruvector_mincut::instance::witness::LazyWitness;
///
/// // Create a lazy witness recipe
/// let lazy = LazyWitness::new(42, 10, 5);
///
/// // No computation happens until materialized
/// assert_eq!(lazy.seed(), 42);
/// assert_eq!(lazy.boundary_size(), 5);
///
/// // Calling with_adjacency materializes the witness
/// // (requires adjacency data from the graph)
/// ```
#[derive(Debug, Clone)]
pub struct LazyWitness {
    /// Seed vertex that defines the cut
    seed: VertexId,
    /// Radius of the local search that found this witness
    radius: usize,
    /// Pre-computed boundary size
    boundary_size: u64,
    /// Cached materialized witness (computed on first access)
    cached: std::sync::OnceLock<WitnessHandle>,
}

impl LazyWitness {
    /// Create a new lazy witness
    ///
    /// # Arguments
    ///
    /// * `seed` - Seed vertex defining the cut
    /// * `radius` - Radius of local search used
    /// * `boundary_size` - Pre-computed boundary size
    pub fn new(seed: VertexId, radius: usize, boundary_size: u64) -> Self {
        Self {
            seed,
            radius,
            boundary_size,
            cached: std::sync::OnceLock::new(),
        }
    }

    /// Get the seed vertex
    #[inline]
    pub fn seed(&self) -> VertexId {
        self.seed
    }

    /// Get the search radius
    #[inline]
    pub fn radius(&self) -> usize {
        self.radius
    }

    /// Get boundary size |δ(U)|
    #[inline]
    pub fn boundary_size(&self) -> u64 {
        self.boundary_size
    }

    /// Check if the witness has been materialized
    #[inline]
    pub fn is_materialized(&self) -> bool {
        self.cached.get().is_some()
    }

    /// Materialize the witness with adjacency information
    ///
    /// This performs a BFS from the seed vertex up to the given radius
    /// to construct the full membership bitmap.
    ///
    /// # Arguments
    ///
    /// * `adjacency` - Function to get neighbors of a vertex
    ///
    /// # Returns
    ///
    /// A fully materialized `WitnessHandle`
    pub fn materialize<F>(&self, adjacency: F) -> WitnessHandle
    where
        F: Fn(VertexId) -> Vec<VertexId>,
    {
        self.cached
            .get_or_init(|| {
                // BFS from seed up to radius
                let mut membership = RoaringBitmap::new();
                let mut visited = HashSet::new();
                let mut queue = std::collections::VecDeque::new();

                queue.push_back((self.seed, 0usize));
                visited.insert(self.seed);
                membership.insert(self.seed as u32);

                while let Some((vertex, dist)) = queue.pop_front() {
                    if dist >= self.radius {
                        continue;
                    }

                    for neighbor in adjacency(vertex) {
                        if visited.insert(neighbor) {
                            membership.insert(neighbor as u32);
                            queue.push_back((neighbor, dist + 1));
                        }
                    }
                }

                WitnessHandle::new(self.seed, membership, self.boundary_size)
            })
            .clone()
    }

    /// Set a pre-computed witness (for cases where we already have it)
    pub fn set_materialized(&self, witness: WitnessHandle) {
        let _ = self.cached.set(witness);
    }

    /// Get the cached witness if already materialized
    pub fn get_cached(&self) -> Option<&WitnessHandle> {
        self.cached.get()
    }
}

/// Batch of lazy witnesses for efficient storage
///
/// Stores multiple lazy witnesses compactly and tracks which
/// have been materialized.
#[derive(Debug, Default)]
pub struct LazyWitnessBatch {
    /// Lazy witnesses in this batch
    witnesses: Vec<LazyWitness>,
    /// Count of materialized witnesses
    materialized_count: std::sync::atomic::AtomicUsize,
}

impl LazyWitnessBatch {
    /// Create a new empty batch
    pub fn new() -> Self {
        Self {
            witnesses: Vec::new(),
            materialized_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Create batch with capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            witnesses: Vec::with_capacity(capacity),
            materialized_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Add a lazy witness to the batch
    pub fn push(&mut self, witness: LazyWitness) {
        self.witnesses.push(witness);
    }

    /// Get witness by index
    pub fn get(&self, index: usize) -> Option<&LazyWitness> {
        self.witnesses.get(index)
    }

    /// Number of witnesses in batch
    pub fn len(&self) -> usize {
        self.witnesses.len()
    }

    /// Check if batch is empty
    pub fn is_empty(&self) -> bool {
        self.witnesses.is_empty()
    }

    /// Count of materialized witnesses
    pub fn materialized_count(&self) -> usize {
        self.materialized_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Materialize a specific witness
    pub fn materialize<F>(&self, index: usize, adjacency: F) -> Option<WitnessHandle>
    where
        F: Fn(VertexId) -> Vec<VertexId>,
    {
        self.witnesses.get(index).map(|lazy| {
            let was_materialized = lazy.is_materialized();
            let handle = lazy.materialize(adjacency);
            if !was_materialized {
                self.materialized_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            handle
        })
    }

    /// Find witness with smallest boundary (materializes only as needed)
    pub fn find_smallest_boundary(&self) -> Option<&LazyWitness> {
        self.witnesses.iter().min_by_key(|w| w.boundary_size())
    }

    /// Iterate over all lazy witnesses
    pub fn iter(&self) -> impl Iterator<Item = &LazyWitness> {
        self.witnesses.iter()
    }
}

#[cfg(test)]
mod lazy_tests {
    use super::*;

    #[test]
    fn test_lazy_witness_new() {
        let lazy = LazyWitness::new(42, 5, 10);
        assert_eq!(lazy.seed(), 42);
        assert_eq!(lazy.radius(), 5);
        assert_eq!(lazy.boundary_size(), 10);
        assert!(!lazy.is_materialized());
    }

    #[test]
    fn test_lazy_witness_materialize() {
        let lazy = LazyWitness::new(0, 2, 3);

        // Simple adjacency: linear graph 0-1-2-3-4
        let adjacency = |v: VertexId| -> Vec<VertexId> {
            match v {
                0 => vec![1],
                1 => vec![0, 2],
                2 => vec![1, 3],
                3 => vec![2, 4],
                4 => vec![3],
                _ => vec![],
            }
        };

        let handle = lazy.materialize(adjacency);

        // With radius 2 from vertex 0, should include 0, 1, 2
        assert!(handle.contains(0));
        assert!(handle.contains(1));
        assert!(handle.contains(2));
        assert!(!handle.contains(3)); // Beyond radius 2
        assert!(lazy.is_materialized());
    }

    #[test]
    fn test_lazy_witness_caching() {
        let lazy = LazyWitness::new(0, 1, 5);

        let call_count = std::sync::atomic::AtomicUsize::new(0);
        let adjacency = |v: VertexId| -> Vec<VertexId> {
            call_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if v == 0 {
                vec![1, 2]
            } else {
                vec![]
            }
        };

        // First materialization
        let _h1 = lazy.materialize(&adjacency);
        let first_count = call_count.load(std::sync::atomic::Ordering::Relaxed);

        // Second materialization should use cache
        let _h2 = lazy.materialize(&adjacency);
        let second_count = call_count.load(std::sync::atomic::Ordering::Relaxed);

        // Adjacency should only be called during first materialization
        assert_eq!(first_count, second_count);
    }

    #[test]
    fn test_lazy_witness_batch() {
        let mut batch = LazyWitnessBatch::with_capacity(3);

        batch.push(LazyWitness::new(0, 2, 5));
        batch.push(LazyWitness::new(1, 3, 3)); // Smallest boundary
        batch.push(LazyWitness::new(2, 1, 7));

        assert_eq!(batch.len(), 3);
        assert_eq!(batch.materialized_count(), 0);

        // Find smallest boundary
        let smallest = batch.find_smallest_boundary().unwrap();
        assert_eq!(smallest.seed(), 1);
        assert_eq!(smallest.boundary_size(), 3);
    }

    #[test]
    fn test_batch_materialize() {
        let mut batch = LazyWitnessBatch::new();
        batch.push(LazyWitness::new(0, 1, 5));

        let adjacency = |_v: VertexId| -> Vec<VertexId> { vec![1, 2] };

        let handle = batch.materialize(0, adjacency).unwrap();
        assert!(handle.contains(0));
        assert!(handle.contains(1));
        assert!(handle.contains(2));

        assert_eq!(batch.materialized_count(), 1);
    }
}
