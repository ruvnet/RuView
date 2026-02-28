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
 * Close the UDP sender socket.
 */
void stream_sender_deinit(void);

#endif /* STREAM_SENDER_H */
