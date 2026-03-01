// SPDX-License-Identifier: GPL-2.0
//
// RVF XDP Vector Distance Computation
//
// Computes squared L2 distance between a query vector received in a
// UDP packet and stored vectors cached in a BPF LRU hash map. Results
// are written to a per-CPU array map for lock-free retrieval by
// userspace via bpf_map_lookup_elem.
//
// Wire format of an RVF query packet:
//   Ethernet | IPv4 | UDP (dst port RVF_PORT) | rvf_query_hdr | f32[dim]
//
// The program only handles packets destined for RVF_PORT and bearing
// the correct magic number. All other traffic is passed through
// unchanged (XDP_PASS).

#include "vmlinux.h"

#define MAX_DIM      512
#define MAX_K        64
#define RVF_PORT     8080
#define RVF_MAGIC    0x52564600  /* "RVF\0" in big-endian */

/* ── RVF query packet header (follows UDP) ───────────────────────── */

struct rvf_query_hdr {
    __u32 magic;       /* RVF_MAGIC */
    __u16 dimension;   /* vector dimension (network byte order) */
    __u16 k;           /* top-k neighbours requested */
    __u64 query_id;    /* caller-chosen query identifier */
} __attribute__((packed));

/* ── Per-query result structure ──────────────────────────────────── */

struct query_result {
    __u64 query_id;
    __u32 count;
    __u64 ids[MAX_K];
    __u32 distances[MAX_K]; /* squared L2, fixed-point */
};

/* ── BPF maps ────────────────────────────────────────────────────── */

/* LRU hash map: caches hot vectors (vector_id -> f32[MAX_DIM]) */
struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __uint(max_entries, 4096);
    __type(key, __u64);
    __type(value, __u8[MAX_DIM * 4]);
} vector_cache SEC(".maps");

/* Per-CPU array: one result slot per CPU for lock-free writes */
struct {
    __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
    __uint(max_entries, 1);
    __type(key, __u32);
    __type(value, struct query_result);
} results SEC(".maps");

/* Array map: list of cached vector IDs for iteration */
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(max_entries, 4096);
    __type(key, __u32);
    __type(value, __u64);
} vector_ids SEC(".maps");

/* Array map: single entry holding the count of populated IDs */
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(max_entries, 1);
    __type(key, __u32);
    __type(value, __u32);
} id_count SEC(".maps");

/* ── Helpers ─────────────────────────────────────────────────────── */

/*
 * Compute squared L2 distance between two vectors stored as raw bytes.
 *
 * Both `a` and `b` point to dim * 4 bytes of IEEE-754 f32 data.
 * We reinterpret each 4-byte group as a __u32 and use integer
 * subtraction as a rough fixed-point proxy -- this is an approximation
 * suitable for ranking, not exact float arithmetic, because the BPF
 * verifier does not support floating-point instructions.
 */
static __always_inline __u64 l2_distance_sq(
    const __u8 *a, const __u8 *b, __u16 dim)
{
    __u64 sum = 0;
    __u16 i;

    /* Bounded loop: the verifier requires a compile-time upper bound. */
    #pragma unroll
    for (i = 0; i < MAX_DIM; i++) {
        if (i >= dim)
            break;

        __u32 va, vb;
        __builtin_memcpy(&va, a + (__u32)i * 4, 4);
        __builtin_memcpy(&vb, b + (__u32)i * 4, 4);

        __s32 diff = (__s32)va - (__s32)vb;
        sum += (__u64)((__s64)diff * (__s64)diff);
    }
    return sum;
}

/*
 * Insert a (distance, id) pair into a max-heap of size k stored in the
 * result arrays. We keep the worst (largest) distance at index 0 so
 * eviction is O(1). This is a simplified sift-down for bounded k.
 */
static __always_inline void heap_insert(
    struct query_result *res, __u32 k, __u64 vid, __u32 dist)
{
    if (res->count < k) {
        __u32 idx = res->count;
        if (idx < MAX_K) {
            res->ids[idx] = vid;
            res->distances[idx] = dist;
            res->count++;
        }
        return;
    }

    /* Find the current worst (max) distance in the heap */
    __u32 worst_idx = 0;
    __u32 worst_dist = 0;
    __u32 i;

    #pragma unroll
    for (i = 0; i < MAX_K; i++) {
        if (i >= res->count)
            break;
        if (res->distances[i] > worst_dist) {
            worst_dist = res->distances[i];
            worst_idx = i;
        }
    }

    /* Evict the worst if the new distance is better */
    if (dist < worst_dist && worst_idx < MAX_K) {
        res->ids[worst_idx] = vid;
        res->distances[worst_idx] = dist;
    }
}

/* ── XDP entry point ─────────────────────────────────────────────── */

SEC("xdp")
int xdp_vector_distance(struct xdp_md *ctx)
{
    void *data     = (void *)(__u64)ctx->data;
    void *data_end = (void *)(__u64)ctx->data_end;

    /* ── Parse Ethernet ──────────────────────────────────────────── */
    struct ethhdr *eth = data;
    if ((void *)(eth + 1) > data_end)
        return XDP_PASS;

    if (eth->h_proto != bpf_htons(ETH_P_IP))
        return XDP_PASS;

    /* ── Parse IPv4 ──────────────────────────────────────────────── */
    struct iphdr *iph = (void *)(eth + 1);
    if ((void *)(iph + 1) > data_end)
        return XDP_PASS;

    if (iph->protocol != IPPROTO_UDP)
        return XDP_PASS;

    /* ── Parse UDP ───────────────────────────────────────────────── */
    struct udphdr *udph = (void *)iph + (iph->ihl * 4);
    if ((void *)(udph + 1) > data_end)
        return XDP_PASS;

    if (bpf_ntohs(udph->dest) != RVF_PORT)
        return XDP_PASS;

    /* ── Parse RVF query header ──────────────────────────────────── */
    struct rvf_query_hdr *qhdr = (void *)(udph + 1);
    if ((void *)(qhdr + 1) > data_end)
        return XDP_PASS;

    if (qhdr->magic != bpf_htonl(RVF_MAGIC))
        return XDP_PASS;

    __u16 dim = bpf_ntohs(qhdr->dimension);
    __u16 k   = bpf_ntohs(qhdr->k);

    if (dim == 0 || dim > MAX_DIM)
        return XDP_PASS;
    if (k == 0 || k > MAX_K)
        return XDP_PASS;

    /* ── Bounds-check the query vector payload ───────────────────── */
    __u8 *query_vec = (__u8 *)(qhdr + 1);
    if ((void *)(query_vec + (__u32)dim * 4) > data_end)
        return XDP_PASS;

    /* ── Get the result slot for this CPU ────────────────────────── */
    __u32 zero = 0;
    struct query_result *result = bpf_map_lookup_elem(&results, &zero);
    if (!result)
        return XDP_PASS;

    result->query_id = qhdr->query_id;
    result->count = 0;

    /* ── Get the number of cached vectors ────────────────────────── */
    __u32 *cnt_ptr = bpf_map_lookup_elem(&id_count, &zero);
    __u32 vec_count = cnt_ptr ? *cnt_ptr : 0;
    if (vec_count > 4096)
        vec_count = 4096;

    /* ── Scan cached vectors, maintaining a top-k heap ───────────── */
    __u32 idx;
    #pragma unroll
    for (idx = 0; idx < 256; idx++) {
        if (idx >= vec_count)
            break;

        __u64 *vid_ptr = bpf_map_lookup_elem(&vector_ids, &idx);
        if (!vid_ptr)
            continue;

        __u64 vid = *vid_ptr;
        __u8 *stored = bpf_map_lookup_elem(&vector_cache, &vid);
        if (!stored)
            continue;

        __u64 dist_sq = l2_distance_sq(query_vec, stored, dim);
        /* Truncate to u32 for storage (upper bits are rarely needed
         * for ranking among cached vectors). */
        __u32 dist32 = (dist_sq > 0xFFFFFFFF) ? 0xFFFFFFFF : (__u32)dist_sq;

        heap_insert(result, k, vid, dist32);
    }

    /* Let the packet continue to userspace for full-index search.
     * The XDP path only accelerates the L0 cache lookup; userspace
     * merges the BPF result with the full RVF index result. */
    return XDP_PASS;
}

char _license[] SEC("license") = "GPL";
