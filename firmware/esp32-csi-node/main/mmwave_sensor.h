/**
 * @file mmwave_sensor.h
 * @brief ADR-063: 60 GHz mmWave sensor auto-detection and UART driver.
 *
 * Supports:
 *   - Seeed MR60BHA2 (60 GHz, heart rate + breathing + presence)
 *   - HLK-LD2410  (24 GHz, presence + distance)
 *   - HLK-LD2450  (24 GHz, x/y of up to 3 targets)
 *
 * Auto-detects sensor type at boot by probing UART for known frame headers.
 * Runs a background task that parses incoming frames and updates shared state.
 */

#ifndef MMWAVE_SENSOR_H
#define MMWAVE_SENSOR_H

#include <stdint.h>
#include <stdbool.h>
#include "esp_err.h"
#include "mmwave_detect.h"

/* ---- Sensor type enumeration ---- */
typedef enum {
    MMWAVE_TYPE_NONE      = 0,  /**< No sensor detected. */
    MMWAVE_TYPE_MR60BHA2  = 1,  /**< Seeed MR60BHA2 (60 GHz, HR + BR). */
    MMWAVE_TYPE_LD2410    = 2,  /**< HLK-LD2410 (24 GHz, presence + range). */
    MMWAVE_TYPE_LD2450    = 3,  /**< HLK-LD2450 (24 GHz, x/y of up to 3 targets). */
    MMWAVE_TYPE_MOCK      = 99, /**< Mock sensor for QEMU testing. */
} mmwave_type_t;

/* ---- Capability flags ---- */
#define MMWAVE_CAP_HEART_RATE   (1 << 0)
#define MMWAVE_CAP_BREATHING    (1 << 1)
#define MMWAVE_CAP_PRESENCE     (1 << 2)
#define MMWAVE_CAP_DISTANCE     (1 << 3)
#define MMWAVE_CAP_FALL         (1 << 4)
#define MMWAVE_CAP_MULTI_TARGET (1 << 5)
#define MMWAVE_CAP_POSITION     (1 << 6)

/* ---- Shared mmWave state (updated by background task) ---- */
typedef struct {
    /* Detection */
    mmwave_type_t type;         /**< Detected sensor type. */
    uint16_t      capabilities; /**< Bitmask of MMWAVE_CAP_* flags. */
    bool          detected;     /**< True if sensor responded on UART. */

    /* Vital signs (MR60BHA2) */
    float    heart_rate_bpm;    /**< Heart rate in BPM (0 if unavailable). */
    float    breathing_rate;    /**< Breathing rate in breaths/min. */

    /* Presence and range (LD2410 / MR60BHA2) */
    bool     person_present;    /**< True if person detected. */
    float    distance_cm;       /**< Distance to nearest target in cm. */
    uint8_t  target_count;      /**< Number of detected targets. */

    /* Per-target position (LD2450). Packed to the front; target_count valid. */
    mmwave_ld2450_target_t targets[MMWAVE_LD2450_MAX_TARGETS];

    /* Quality metrics */
    uint32_t frame_count;       /**< Total parsed frames since boot. */
    uint32_t error_count;       /**< Parse errors / CRC failures. */
    int64_t  last_update_us;    /**< Timestamp of last valid frame. */
} mmwave_state_t;

/* ---- Radar targets packet (40 bytes, wire format) ----
 * Sent once per LD2450 frame (~10 Hz) so the host can pair radar positions
 * with CSI frames by arrival time (e.g. as localisation training labels).
 * The 1 Hz fused vitals packet only carries the nearest distance. */
#define MMWAVE_TARGETS_MAGIC  0xC5110008  /**< Radar targets packet magic. */

typedef struct __attribute__((packed)) {
    uint32_t magic;          /**< MMWAVE_TARGETS_MAGIC = 0xC5110008. */
    uint8_t  node_id;
    uint8_t  mmwave_type;    /**< mmwave_type_t enum. */
    uint8_t  n_targets;      /**< Valid entries in targets[] (0-3). */
    uint8_t  reserved;
    uint32_t timestamp_ms;   /**< Node uptime at frame parse. */
    uint32_t frame_seq;      /**< Radar frames parsed since boot. */
    mmwave_ld2450_target_t targets[MMWAVE_LD2450_MAX_TARGETS];
} mmwave_targets_pkt_t;

_Static_assert(sizeof(mmwave_ld2450_target_t) == 8, "radar target must be 8 bytes");
_Static_assert(sizeof(mmwave_targets_pkt_t) == 40, "radar targets packet must be 40 bytes");

/**
 * Initialize the mmWave sensor subsystem.
 *
 * Probes the configured UART for known sensor types. If a sensor is
 * detected, starts a background FreeRTOS task to parse incoming frames.
 *
 * @param uart_tx_pin  GPIO pin for UART TX (to sensor RX). Use -1 for default.
 * @param uart_rx_pin  GPIO pin for UART RX (from sensor TX). Use -1 for default.
 * @return ESP_OK if sensor detected, ESP_ERR_NOT_FOUND if no sensor.
 */
esp_err_t mmwave_sensor_init(int uart_tx_pin, int uart_rx_pin);

/**
 * Get a snapshot of the current mmWave state (thread-safe copy).
 *
 * @param state  Output state struct.
 * @return true if valid data is available (sensor detected and running).
 */
bool mmwave_sensor_get_state(mmwave_state_t *state);

/**
 * Get the detected sensor type name as a string.
 */
const char *mmwave_type_name(mmwave_type_t type);

#endif /* MMWAVE_SENSOR_H */
