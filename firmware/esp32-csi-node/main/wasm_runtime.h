/**
 * @file wasm_runtime.h
 * @brief ADR-040 Tier 3 — WASM programmable sensing runtime.
 *
 * Manages WASM3 interpreter instances for hot-loadable sensing algorithms.
 * WASM modules are compiled from Rust (wifi-densepose-wasm-edge crate) to
 * wasm32-unknown-unknown and executed on-device after Tier 2 DSP completes.
 *
 * Host API namespace "csi":
 *   csi_get_phase(subcarrier) -> f32
 *   csi_get_amplitude(subcarrier) -> f32
 *   csi_get_variance(subcarrier) -> f32
 *   csi_get_bpm_breathing() -> f32
 *   csi_get_bpm_heartrate() -> f32
 *   csi_get_presence() -> i32
 *   csi_get_motion_energy() -> f32
 *   csi_get_n_persons() -> i32
 *   csi_get_timestamp() -> i32
 *   csi_emit_event(event_type, value)
 *   csi_log(ptr, len)
 *   csi_get_phase_history(buf_ptr, max_len) -> i32
 *
 * Module lifecycle exports:
 *   on_init()          — called once when module is loaded
 *   on_frame(n_sc)     — called per CSI frame (~20 Hz)
 *   on_timer()         — called at configurable interval (default 1 s)
 */

#ifndef WASM_RUNTIME_H
#define WASM_RUNTIME_H

#include <stdint.h>
#include <stdbool.h>
#include "esp_err.h"
#include "edge_processing.h"

/* ---- Configuration ---- */
#ifdef CONFIG_WASM_MAX_MODULES
#define WASM_MAX_MODULES CONFIG_WASM_MAX_MODULES
#else
#define WASM_MAX_MODULES 4
#endif

#define WASM_MAX_MODULE_SIZE (128 * 1024)  /**< Max .wasm binary size (128 KB). */
#define WASM_STACK_SIZE      (8 * 1024)    /**< WASM execution stack (8 KB). */
#define WASM_OUTPUT_MAGIC    0xC5110004    /**< WASM output packet magic. */
#define WASM_MAX_EVENTS      16            /**< Max events per output packet. */

/* ---- WASM Event (5 bytes: u8 type + f32 value) ---- */
typedef struct __attribute__((packed)) {
    uint8_t event_type;
    float   value;
} wasm_event_t;

/* ---- WASM Output Packet ---- */
typedef struct __attribute__((packed)) {
    uint32_t magic;         /**< WASM_OUTPUT_MAGIC = 0xC5110004. */
    uint8_t  node_id;       /**< ESP32 node identifier. */
    uint8_t  module_id;     /**< Module slot index. */
    uint16_t event_count;   /**< Number of events in this packet. */
    wasm_event_t events[WASM_MAX_EVENTS];
} wasm_output_pkt_t;

/* ---- Module state ---- */
typedef enum {
    WASM_MODULE_EMPTY = 0,  /**< Slot is free. */
    WASM_MODULE_LOADED,     /**< Binary loaded, not yet started. */
    WASM_MODULE_RUNNING,    /**< Module is executing on each frame. */
    WASM_MODULE_STOPPED,    /**< Module stopped but binary still in memory. */
    WASM_MODULE_ERROR,      /**< Module encountered a fatal error. */
} wasm_module_state_t;

/* ---- Per-frame budget (microseconds) ---- */
#ifdef CONFIG_WASM_FRAME_BUDGET_US
#define WASM_FRAME_BUDGET_US CONFIG_WASM_FRAME_BUDGET_US
#else
#define WASM_FRAME_BUDGET_US 10000  /**< Default 10 ms per on_frame call. */
#endif

/* ---- Fixed arena size per module slot (PSRAM) ---- */
#define WASM_ARENA_SIZE (160 * 1024) /**< 160 KB per slot, pre-allocated at boot. */

/* ---- Module info (for listing) ---- */
typedef struct {
    uint8_t             id;         /**< Slot index. */
    wasm_module_state_t state;      /**< Current state. */
    uint32_t            binary_size;/**< .wasm binary size in bytes. */
    uint32_t            frame_count;/**< Frames processed since start. */
    uint32_t            event_count;/**< Total events emitted. */
    uint32_t            error_count;/**< Runtime errors encountered. */
    uint32_t            total_us;   /**< Cumulative execution time (us). */
    uint32_t            max_us;     /**< Worst-case single frame (us). */
    uint32_t            budget_faults; /**< Times frame budget was exceeded. */
    /* RVF manifest metadata (zeroed if loaded as raw WASM). */
    char                module_name[32]; /**< From RVF manifest. */
    uint32_t            capabilities;    /**< RVF_CAP_* bitmask. */
    uint32_t            manifest_budget_us; /**< Budget from manifest (0=default). */
} wasm_module_info_t;

/**
 * Initialize the WASM runtime.
 * Allocates WASM3 environment and module slots in PSRAM.
 *
 * @return ESP_OK on success.
 */
esp_err_t wasm_runtime_init(void);

/**
 * Load a WASM binary into the next available slot.
 *
 * @param wasm_data  Pointer to .wasm binary data.
 * @param wasm_len   Length of the binary in bytes (max WASM_MAX_MODULE_SIZE).
 * @param module_id  Output: assigned slot index.
 * @return ESP_OK on success.
 */
esp_err_t wasm_runtime_load(const uint8_t *wasm_data, uint32_t wasm_len,
                            uint8_t *module_id);

/**
 * Start a loaded module (calls on_init export).
 *
 * @param module_id  Slot index from wasm_runtime_load().
 * @return ESP_OK on success.
 */
esp_err_t wasm_runtime_start(uint8_t module_id);

/**
 * Stop a running module.
 *
 * @param module_id  Slot index.
 * @return ESP_OK on success.
 */
esp_err_t wasm_runtime_stop(uint8_t module_id);

/**
 * Unload a module and free its memory.
 *
 * @param module_id  Slot index.
 * @return ESP_OK on success.
 */
esp_err_t wasm_runtime_unload(uint8_t module_id);

/**
 * Call on_frame(n_subcarriers) on all running modules.
 * Called from the DSP task (Core 1) after Tier 2 processing.
 *
 * @param phases      Current phase array (read by csi_get_phase).
 * @param amplitudes  Current amplitude array (read by csi_get_amplitude).
 * @param variances   Welford variance array (read by csi_get_variance).
 * @param n_sc        Number of subcarriers.
 * @param vitals      Current Tier 2 vitals (read by csi_get_bpm_* etc).
 */
void wasm_runtime_on_frame(const float *phases, const float *amplitudes,
                           const float *variances, uint16_t n_sc,
                           const edge_vitals_pkt_t *vitals);

/**
 * Call on_timer() on all running modules.
 * Called from the main loop at the configured timer interval.
 */
void wasm_runtime_on_timer(void);

/**
 * Get info for all module slots.
 *
 * @param info   Output array (must be WASM_MAX_MODULES elements).
 * @param count  Output: number of populated slots.
 */
void wasm_runtime_get_info(wasm_module_info_t *info, uint8_t *count);

/**
 * Apply RVF manifest metadata to a loaded module slot.
 *
 * Stores the module name, capabilities, and overrides the per-slot
 * frame budget with the manifest's max_frame_us (if nonzero).
 * Call after wasm_runtime_load(), before wasm_runtime_start().
 *
 * @param module_id     Slot index from wasm_runtime_load().
 * @param module_name   Null-terminated name (max 31 chars).
 * @param capabilities  RVF_CAP_* bitmask.
 * @param max_frame_us  Per-frame budget override (0 = use global default).
 * @return ESP_OK on success.
 */
esp_err_t wasm_runtime_set_manifest(uint8_t module_id, const char *module_name,
                                     uint32_t capabilities, uint32_t max_frame_us);

#endif /* WASM_RUNTIME_H */
