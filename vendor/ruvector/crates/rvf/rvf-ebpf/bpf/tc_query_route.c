// SPDX-License-Identifier: GPL-2.0
//
// RVF TC Query Router: Priority-Based Query Classification
//
// This TC (Traffic Control) classifier inspects incoming UDP packets
// destined for the RVF query port and classifies them into priority
// tiers based on the query type encoded in the RVF protocol header.
//
// Classification tiers (set via skb->tc_classid):
//   TC_H_MAKE(1, 1) = "hot" queries   (low-latency, cached vectors)
//   TC_H_MAKE(1, 2) = "warm" queries  (standard priority)
//   TC_H_MAKE(1, 3) = "cold" queries  (batch/bulk, best-effort)
//
// The query type is determined by inspecting the flags field in the
// RVF query header that follows the UDP payload.
//
// Attach: tc filter add dev <iface> ingress bpf da obj tc_query_route.o

#include "vmlinux.h"

/* ── Configuration ───────────────────────────────────────────────── */

#define RVF_PORT     8080
#define RVF_MAGIC    0x52564600  /* "RVF\0" big-endian */

/* TC classid helpers: major:minor */
#define TC_H_MAKE(maj, min)  (((maj) << 16) | (min))

/* Priority classes */
#define CLASS_HOT    TC_H_MAKE(1, 1)
#define CLASS_WARM   TC_H_MAKE(1, 2)
#define CLASS_COLD   TC_H_MAKE(1, 3)

/* RVF query flag bits (in the flags field of the extended header) */
#define RVF_FLAG_HOT_CACHE   0x01  /* Request L0 (BPF map) cache lookup */
#define RVF_FLAG_BATCH       0x02  /* Batch query mode */
#define RVF_FLAG_PREFETCH    0x04  /* Prefetch hint for warming cache */
#define RVF_FLAG_PRIORITY    0x08  /* Caller-requested high priority */

/* ── RVF query header (same as xdp_distance.c) ──────────────────── */

struct rvf_query_hdr {
    __u32 magic;       /* RVF_MAGIC */
    __u16 dimension;   /* vector dimension (network byte order) */
    __u16 k;           /* top-k requested */
    __u64 query_id;    /* caller-chosen query identifier */
    __u32 flags;       /* query flags (network byte order) */
} __attribute__((packed));

/* ── BPF maps ────────────────────────────────────────────────────── */

/* Per-CPU counters for each priority class */
struct class_stats {
    __u64 hot;
    __u64 warm;
    __u64 cold;
    __u64 passthrough;
};

struct {
    __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
    __uint(max_entries, 1);
    __type(key, __u32);
    __type(value, struct class_stats);
} tc_stats SEC(".maps");

/* ── Helpers ─────────────────────────────────────────────────────── */

static __always_inline void bump_class(int class_idx)
{
    __u32 zero = 0;
    struct class_stats *s = bpf_map_lookup_elem(&tc_stats, &zero);
    if (!s)
        return;

    switch (class_idx) {
    case 0: s->hot++;         break;
    case 1: s->warm++;        break;
    case 2: s->cold++;        break;
    default: s->passthrough++; break;
    }
}

/* ── TC classifier entry point ───────────────────────────────────── */

SEC("tc")
int rvf_query_classify(struct __sk_buff *skb)
{
    /* ── Parse IP protocol and header length ─────────────────────── */
    __u8 ihl_byte = 0;
    if (bpf_skb_load_bytes(skb, 0, &ihl_byte, 1) < 0)
        return TC_ACT_OK;

    __u32 ip_hdr_len = (__u32)(ihl_byte & 0x0F) * 4;
    if (ip_hdr_len < 20)
        return TC_ACT_OK;

    __u8 protocol = 0;
    if (bpf_skb_load_bytes(skb, 9, &protocol, 1) < 0)
        return TC_ACT_OK;

    if (protocol != IPPROTO_UDP) {
        bump_class(3);
        return TC_ACT_OK;
    }

    /* ── Parse UDP destination port ──────────────────────────────── */
    __be16 raw_dport = 0;
    if (bpf_skb_load_bytes(skb, ip_hdr_len + 2, &raw_dport, 2) < 0)
        return TC_ACT_OK;

    __u16 dport = bpf_ntohs(raw_dport);
    if (dport != RVF_PORT) {
        bump_class(3);
        return TC_ACT_OK;
    }

    /* ── Parse RVF query header (after 8-byte UDP header) ────────── */
    __u32 rvf_offset = ip_hdr_len + 8; /* IP hdr + UDP hdr */

    struct rvf_query_hdr qhdr;
    __bpf_memset(&qhdr, 0, sizeof(qhdr));
    if (bpf_skb_load_bytes(skb, rvf_offset, &qhdr, sizeof(qhdr)) < 0) {
        bump_class(3);
        return TC_ACT_OK;
    }

    if (qhdr.magic != bpf_htonl(RVF_MAGIC)) {
        bump_class(3);
        return TC_ACT_OK;
    }

    /* ── Classify based on flags ─────────────────────────────────── */
    __u32 flags = bpf_ntohl(qhdr.flags);

    if (flags & RVF_FLAG_PRIORITY || flags & RVF_FLAG_HOT_CACHE) {
        /* Hot path: low-latency cached query */
        skb->tc_classid = CLASS_HOT;
        bump_class(0);
        return TC_ACT_OK;
    }

    if (flags & RVF_FLAG_BATCH) {
        /* Cold path: bulk/batch query, best-effort */
        skb->tc_classid = CLASS_COLD;
        bump_class(2);
        return TC_ACT_OK;
    }

    /* Default: warm / standard priority */
    skb->tc_classid = CLASS_WARM;
    bump_class(1);
    return TC_ACT_OK;
}

char _license[] SEC("license") = "GPL";
