/**
 * @file node_mdns.h
 * @brief Advertise this CSI node on the LAN via mDNS (packaging gap, ADR-423 follow-up).
 *
 * Announces `_ruview-node._tcp` on the OTA/control port with TXT records
 * describing the node (id, firmware, chip) and where it is sending CSI. A host
 * can then enumerate nodes and spot a misconfigured CSI target without serial
 * access. Advertising only: the node never follows a discovered server, so a
 * LAN host cannot redirect person data by answering mDNS.
 */
#ifndef NODE_MDNS_H
#define NODE_MDNS_H

#include <stddef.h>
#include <stdint.h>
#include "esp_err.h"

/** mDNS service type advertised by every node. */
#define NODE_MDNS_SERVICE "_ruview-node"
#define NODE_MDNS_PROTO   "_tcp"

/**
 * Format the node's mDNS hostname from its base MAC: "ruview-node-xxxxxx"
 * (last three MAC bytes). node_id is not used because provisioned ids can
 * collide; the MAC suffix cannot.
 *
 * @return chars written (excluding NUL), or 0 if the buffer is too small.
 */
size_t node_mdns_format_hostname(const uint8_t mac[6], char *out, size_t out_len);

/**
 * Format "ip:port" for the csi_target TXT record. Rejects an empty IP.
 *
 * @return chars written (excluding NUL), or 0 on invalid input / short buffer.
 */
size_t node_mdns_format_target(const char *ip, uint16_t port, char *out, size_t out_len);

/**
 * Start mDNS and advertise the node. Safe to call before the STA has an IP:
 * the responder announces once the interface comes up. Failure is logged and
 * returned but is never fatal to CSI capture.
 */
esp_err_t node_mdns_start(uint8_t node_id, const char *target_ip, uint16_t target_port,
                          uint16_t service_port);

#endif /* NODE_MDNS_H */
