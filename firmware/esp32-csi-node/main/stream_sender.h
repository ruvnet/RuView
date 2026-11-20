/**
 * @file stream_sender.h
 * @brief UDP stream sender for CSI frames.
 */

#ifndef STREAM_SENDER_H
#define STREAM_SENDER_H

#include <stdint.h>
#include <stddef.h>

/**
 * Initialize the UDP sender.
 * Creates a UDP socket targeting the configured aggregator.
 *
 * @return 0 on success, -1 on error.
 */
int stream_sender_init(void);

/**
 * Initialize the UDP sender with explicit IP and port.
 * Used when configuration is loaded from NVS at runtime.
 *
 * @param ip   Aggregator IP address string (e.g. "192.168.1.20").
 * @param port Aggregator UDP port.
 * @return 0 on success, -1 on error.
 */
int stream_sender_init_with(const char *ip, uint16_t port);

/**
 * Send a serialized CSI frame over UDP.
 *
 * @param data Frame data buffer.
 * @param len  Length of data to send.
 * @return Number of bytes sent, or -1 on error.
 */
int stream_sender_send(const uint8_t *data, size_t len);

/**
 * Send a low-rate control packet, bypassing the ENOMEM backoff gate (#1183).
 *
 * Intended for ≤48 B, ≤1 Hz control traffic (feature_state, HEALTH, mesh
 * sync) that must not be starved by the global backoff the high-rate CSI
 * stream triggers. An ENOMEM on this path is reported quietly and does NOT
 * extend or reset the global backoff streak.
 *
 * @param data Frame data buffer.
 * @param len  Length of data to send.
 * @return Number of bytes sent, or -1 on error.
 */
int stream_sender_send_priority(const uint8_t *data, size_t len);

/**
 * esp_timer timestamp (microseconds) of the last sendto() the network stack
 * accepted, on either the bulk or the priority path. Returns 0 if the node has
 * not successfully sent anything since boot.
 *
 * This reports that the datagram was accepted for transmission, NOT that the
 * aggregator received it -- UDP gives no such signal. Used by the uplink
 * watchdog to detect a node that is powered and capturing but cannot reach the
 * network.
 */
int64_t stream_sender_last_success_us(void);

/**
 * Close the UDP sender socket.
 */
void stream_sender_deinit(void);

#endif /* STREAM_SENDER_H */
