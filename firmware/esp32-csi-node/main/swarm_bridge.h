/**
 * @file swarm_bridge.h
 * @brief ADR-066: ESP32 Swarm Bridge — Cognitum Seed coordinator client.
 *
 * Registers this node with a Cognitum Seed, sends periodic heartbeats,
 * and pushes happiness vectors for cross-zone analytics.
 * Runs as a FreeRTOS task on Core 0.
 */

#ifndef SWARM_BRIDGE_H
#define SWARM_BRIDGE_H

#include <stdint.h>
#include "esp_err.h"
#include "edge_processing.h"

/** Happiness vector dimension. */
#define SWARM_VECTOR_DIM  8

/** Swarm bridge configuration. */
typedef struct {
    char     seed_url[64];     /**< Cognitum Seed base URL (e.g. "http://192.168.1.10:8080"). */
    char     seed_token[64];   /**< Bearer token for Seed WiFi API auth (from pairing). */
    char     zone_name[16];    /**< Zone name for this node (e.g. "bedroom"). */
    uint16_t heartbeat_sec;    /**< Heartbeat interval in seconds (default 30). */
    uint16_t ingest_sec;       /**< Happiness ingest interval in seconds (default 5). */
    uint8_t  enabled;          /**< 1 = bridge active, 0 = disabled. */
} swarm_config_t;

/**
 * Initialize the swarm bridge and start the background task.
 * Registers this node with the Cognitum Seed on first successful POST.
 *
 * @param cfg      Swarm bridge configuration.
 * @param node_id  This node's identifier (from NVS).
 * @return ESP_OK on success, ESP_ERR_INVALID_ARG if seed_url is empty.
 */
esp_err_t swarm_bridge_init(const swarm_config_t *cfg, uint8_t node_id);

/**
 * Feed the latest vitals packet into the swarm bridge.
 * Called from the main loop whenever new vitals are available.
 *
 * @param vitals  Pointer to the latest vitals packet.
 */
void swarm_bridge_update_vitals(const edge_vitals_pkt_t *vitals);

/**
 * Update the happiness vector to be pushed at the next ingest cycle.
 *
 * @param vector  Float array of happiness values.
 * @param dim     Number of elements (clamped to SWARM_VECTOR_DIM).
 */
void swarm_bridge_update_happiness(const float *vector, uint8_t dim);

/**
 * Get cumulative bridge statistics.
 *
 * @param regs        Output: number of successful registrations.
 * @param heartbeats  Output: number of successful heartbeats sent.
 * @param ingests     Output: number of successful happiness ingests sent.
 * @param errors      Output: number of HTTP errors encountered.
 */
void swarm_bridge_get_stats(uint32_t *regs, uint32_t *heartbeats,
                            uint32_t *ingests, uint32_t *errors);

#endif /* SWARM_BRIDGE_H */
