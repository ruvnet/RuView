//! Fixed-size min-heap for top-K tracking.
//!
//! Max K=16, stored in static memory. No allocator needed.

/// Maximum top-K value supported by the tile.
pub const MAX_K: usize = 16;

/// A heap entry: (distance, vector_id).
#[derive(Clone, Copy)]
struct HeapEntry {
    dist: f32,
    id: u64,
}

/// Static heap storage. Max-heap by distance — the largest distance
/// is at index 0 so we can efficiently evict it when a closer
/// candidate arrives.
static mut HEAP: [HeapEntry; MAX_K] = [HeapEntry {
    dist: f32::MAX,
    id: 0,
}; MAX_K];

/// Current number of elements in the heap.
static mut HEAP_SIZE: usize = 0;

/// Reset the heap to empty state.
pub fn heap_reset() {
    unsafe {
        HEAP_SIZE = 0;
        for entry in HEAP.iter_mut() {
            entry.dist = f32::MAX;
            entry.id = 0;
        }
    }
}

/// Insert a candidate into the max-heap if it improves the top-K set.
pub fn heap_insert(dist: f32, id: u64, k: usize) {
    let k = if k > MAX_K { MAX_K } else { k };

    unsafe {
        if HEAP_SIZE < k {
            let idx = HEAP_SIZE;
            HEAP[idx] = HeapEntry { dist, id };
            HEAP_SIZE += 1;
            sift_up(idx);
        } else if dist < HEAP[0].dist {
            HEAP[0] = HeapEntry { dist, id };
            sift_down(0, HEAP_SIZE);
        }
    }
}

/// Read sorted results (ascending by distance) into output buffer.
/// Format: for each result, 8 bytes id (u64 LE) then 4 bytes dist (f32 LE).
/// Returns number of results written.
pub fn heap_read_sorted(out_ptr: *mut u8) -> i32 {
    unsafe {
        let size = HEAP_SIZE;
        if size == 0 {
            return 0;
        }

        // Copy heap to temporary sort buffer
        let mut sorted: [HeapEntry; MAX_K] = [HeapEntry {
            dist: f32::MAX,
            id: 0,
        }; MAX_K];
        for i in 0..size {
            sorted[i] = HEAP[i];
        }

        // Insertion sort (K <= 16)
        for i in 1..size {
            let key = sorted[i];
            let mut j = i;
            while j > 0 && sorted[j - 1].dist > key.dist {
                sorted[j] = sorted[j - 1];
                j -= 1;
            }
            sorted[j] = key;
        }

        // Write to output
        for i in 0..size {
            let offset = i * 12;
            let id_bytes = sorted[i].id.to_le_bytes();
            let dist_bytes = sorted[i].dist.to_le_bytes();
            for b in 0..8 {
                *out_ptr.add(offset + b) = id_bytes[b];
            }
            for b in 0..4 {
                *out_ptr.add(offset + 8 + b) = dist_bytes[b];
            }
        }

        size as i32
    }
}

/// Sift up in a max-heap.
unsafe fn sift_up(mut idx: usize) {
    while idx > 0 {
        let parent = (idx - 1) / 2;
        if HEAP[idx].dist > HEAP[parent].dist {
            HEAP.swap(idx, parent);
            idx = parent;
        } else {
            break;
        }
    }
}

/// Sift down in a max-heap.
unsafe fn sift_down(mut idx: usize, size: usize) {
    loop {
        let left = 2 * idx + 1;
        let right = 2 * idx + 2;
        let mut largest = idx;

        if left < size && HEAP[left].dist > HEAP[largest].dist {
            largest = left;
        }
        if right < size && HEAP[right].dist > HEAP[largest].dist {
            largest = right;
        }

        if largest == idx {
            break;
        }

        HEAP.swap(idx, largest);
        idx = largest;
    }
}
