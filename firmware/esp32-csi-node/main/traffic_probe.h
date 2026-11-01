/* Unicast UDP CSI traffic source (#2287).
 *
 * The connected-STA CSI floor (#521/#954) pings the gateway. Some APs send
 * ICMP echo replies at 802.11b DSSS/CCK rates, which carry no LTF and so
 * produce no CSI: #2287 measured ~98% of replies at 11b and 2-9 CSI/s, while
 * ordinary unicast UDP from the same router arrived as HT (~42 CSI/s). This
 * probe sends a small DNS query to the gateway instead, so the replies are
 * unicast UDP. Opt-in via CONFIG_CSI_TRAFFIC_SOURCE_UDP_DNS.
 *
 * The query builder is a pure function kept in this header, like
 * ota_health.h, so the host test exercises the exact code the device runs.
 * traffic_probe.c owns the socket and the task. */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define TRAFFIC_PROBE_DNS_PORT  53
#define TRAFFIC_PROBE_NAME_MAX  253                   /* presentation form, RFC 1035 */
#define TRAFFIC_PROBE_QUERY_MAX (12 + 255 + 4)         /* header + encoded name + type/class */

static inline bool traffic_probe_label_char_ok(char c)
{
    return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
           (c >= '0' && c <= '9') || c == '-' || c == '_';
}

/* Write a standard DNS query for (qname, A, IN) into buf.
 *
 * RD (recursion desired) is left clear: the probe only wants a reply, so it
 * asks the gateway to answer from its own data rather than query upstream on
 * the node's behalf. Any reply works, including NXDOMAIN or REFUSED.
 *
 * qname is a dotted name of letters, digits, '-' and '_', with an optional
 * trailing dot. Returns the query length, or 0 if qname is empty, has an
 * empty label, a label over 63 bytes, a character outside that set, is over
 * 253 bytes, or the query does not fit in cap. The ID is bytes 0-1. */
static inline size_t traffic_probe_build_dns_query(uint8_t *buf, size_t cap,
                                                   uint16_t id, const char *qname)
{
    if (buf == NULL || qname == NULL) {
        return 0;
    }
    size_t name_len = 0;
    while (qname[name_len] != '\0') {
        if (++name_len > TRAFFIC_PROBE_NAME_MAX + 1) {
            return 0;
        }
    }
    if (name_len > 0 && qname[name_len - 1] == '.') {
        name_len--;                                    /* fully-qualified form */
    }
    if (name_len == 0 || name_len > TRAFFIC_PROBE_NAME_MAX) {
        return 0;
    }
    /* header 12 + (length byte per label + name bytes - dots) + root 1 + type/class 4 */
    if (cap < 12 + name_len + 2 + 4) {
        return 0;
    }

    buf[0] = (uint8_t)(id >> 8);
    buf[1] = (uint8_t)id;
    buf[2] = 0x00;          /* QR=0 query, opcode 0, AA/TC/RD clear */
    buf[3] = 0x00;
    buf[4] = 0x00; buf[5] = 0x01;                      /* QDCOUNT 1 */
    buf[6] = buf[7] = buf[8] = buf[9] = buf[10] = buf[11] = 0x00;

    size_t out = 12;
    size_t label_start = 0;
    for (size_t i = 0; i <= name_len; i++) {
        if (i == name_len || qname[i] == '.') {
            size_t label_len = i - label_start;
            if (label_len == 0 || label_len > 63) {
                return 0;
            }
            buf[out++] = (uint8_t)label_len;
            for (size_t k = label_start; k < i; k++) {
                if (!traffic_probe_label_char_ok(qname[k])) {
                    return 0;
                }
                buf[out++] = (uint8_t)qname[k];
            }
            label_start = i + 1;
        }
    }
    buf[out++] = 0x00;                                 /* root */
    buf[out++] = 0x00; buf[out++] = 0x01;              /* QTYPE A */
    buf[out++] = 0x00; buf[out++] = 0x01;              /* QCLASS IN */
    return out;
}

/* Start the UDP probe to gw_addr (IPv4, network byte order) every
 * interval_ms, querying qname. Idempotent: returns true if it is already
 * running. Returns false, with nothing started, if qname is invalid or the
 * socket or task cannot be created, so the caller can fall back to ICMP. */
bool traffic_probe_start(uint32_t gw_addr, uint32_t interval_ms, const char *qname);
