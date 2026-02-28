// SPDX-License-Identifier: GPL-2.0
//
// RVF Socket Filter: Port-Based Access Control
//
// This BPF socket filter enforces a simple port allow-list for RVF
// deployments. Only packets destined for explicitly allowed ports are
// passed through; everything else is dropped.
//
// Allowed ports are stored in a BPF hash map so they can be updated at
// runtime from userspace without reloading the program.
//
// Default allowed ports (populated by userspace loader):
//   - 8080: RVF API / vector query endpoint
//   - 2222: SSH management access
//   - 9090: Prometheus metrics scraping
//   - 6379: Optional Redis sidecar for caching
//
// Attach point: SO_ATTACH_BPF on a raw socket, or cgroup/skb.

#include "vmlinux.h"

/* ── Configuration ───────────────────────────────────────────────── */

#define MAX_ALLOWED_PORTS 64

/* ── BPF maps ────────────────────────────────────────────────────── */

/* Hash map: allowed destination ports. Key = port number, value = 1 */
struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, MAX_ALLOWED_PORTS);
    __type(key, __u16);
    __type(value, __u8);
} allowed_ports SEC(".maps");

/* Per-CPU array: drop/pass counters for observability */
struct port_stats {
    __u64 passed;
    __u64 dropped;
};

struct {
    __uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
    __uint(max_entries, 1);
    __type(key, __u32);
    __type(value, struct port_stats);
} stats SEC(".maps");

/* ── Helpers ─────────────────────────────────────────────────────── */

static __always_inline void bump_stat(int is_pass)
{
    __u32 zero = 0;
    struct port_stats *s = bpf_map_lookup_elem(&stats, &zero);
    if (s) {
        if (is_pass)
            s->passed++;
        else
            s->dropped++;
    }
}

/* ── Socket filter entry point ───────────────────────────────────── */

SEC("socket")
int rvf_port_filter(struct __sk_buff *skb)
{
    /* Load the protocol field from the IP header.
     * For socket filters attached via SO_ATTACH_BPF, skb->data
     * starts at the IP header (no Ethernet header). */

    __u8 protocol = 0;
    /* IP protocol field is at byte offset 9 in the IPv4 header */
    bpf_skb_load_bytes(skb, 9, &protocol, 1);

    __u16 dport = 0;

    if (protocol == IPPROTO_TCP) {
        /* TCP dest port: IP header (ihl*4) + offset 2 in TCP header */
        __u8 ihl_byte = 0;
        bpf_skb_load_bytes(skb, 0, &ihl_byte, 1);
        __u32 ip_hdr_len = (ihl_byte & 0x0F) * 4;

        __be16 raw_port = 0;
        bpf_skb_load_bytes(skb, ip_hdr_len + 2, &raw_port, 2);
        dport = bpf_ntohs(raw_port);
    } else if (protocol == IPPROTO_UDP) {
        __u8 ihl_byte = 0;
        bpf_skb_load_bytes(skb, 0, &ihl_byte, 1);
        __u32 ip_hdr_len = (ihl_byte & 0x0F) * 4;

        __be16 raw_port = 0;
        bpf_skb_load_bytes(skb, ip_hdr_len + 2, &raw_port, 2);
        dport = bpf_ntohs(raw_port);
    } else {
        /* Non-TCP/UDP traffic: pass through (e.g. ICMP for health checks) */
        bump_stat(1);
        return skb->len;
    }

    /* Look up the destination port in the allow-list */
    __u8 *allowed = bpf_map_lookup_elem(&allowed_ports, &dport);
    if (allowed) {
        bump_stat(1);
        return skb->len;     /* Pass: return original packet length */
    }

    bump_stat(0);
    return 0;                 /* Drop: returning 0 truncates the packet */
}

char _license[] SEC("license") = "GPL";
