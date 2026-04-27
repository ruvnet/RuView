/**
 * @file wasm_runtime.c
 * @brief ADR-040 Tier 3 — WASM3 runtime for hot-loadable sensing algorithms.
 *
 * Manages up to WASM_MAX_MODULES concurrent WASM modules, each executing
 * on_frame() after Tier 2 DSP completes.  Modules are stored in PSRAM and
 * executed on Core 1 (DSP task context).
 *
 * Host API bindings expose Tier 2 DSP results (phase, amplitude, variance,
 * vitals) to WASM code via imported functions in the "csi" namespace.
 */

#include "sdkconfig.h"
#include "wasm_runtime.h"
#include "nvs_config.h"
#include "csi_collector.h"  /* csi_collector_get_node_id() - defensive #390 */

extern nvs_config_t g_nvs_config;

#if defined(CONFIG_WASM_ENABLE) && defined(WASM3_AVAILABLE)

#include "rvf_parser.h"
#include "stream_sender.h"

#include <string.h>
#include <math.h>
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "esp_heap_caps.h"
#include "sdkconfig.h"

/* Include WASM3 headers. */
#include "wasm3.h"
#include "m3_env.h"

static const char *TAG = "wasm_rt";

/* ======================================================================
 * Module Slot
 * ====================================================================== */

typedef struct {
    wasm_module_state_t state;
    uint8_t            *binary;       /**< Points into fixed arena (PSRAM). */
    uint32_t            binary_size;
    uint8_t            *arena;        /**< Fixed PSRAM arena (WASM_ARENA_SIZE). */

    /* WASM3 objects. */
    IM3Runtime          runtime;
    IM3Module           module;
    IM3Function         fn_on_init;
    IM3Function         fn_on_frame;
    IM3Function         fn_on_timer;

    /* Counters and telemetry. */
    uint32_t            frame_count;
    uint32_t            event_count;
    uint32_t            error_count;
    uint32_t            total_us;     /**< Cumulative execution time. */
    uint32_t            max_us;       /**< Worst-case single frame. */
    uint32_t            budget_faults;/**< Budget exceeded count. */

    /* Pending output events for this frame. */
    wasm_event_t        events[WASM_MAX_EVENTS];
    uint8_t             n_events;

    /* RVF manifest metadata (zeroed if raw WASM load). */
    char                module_name[32];
    uint32_t            capabilities;
    uint32_t            manifest_budget_us; /**< 0 = use global default. */

    /* Dead-band filter: last emitted value per event type (for delta export). */
    float               last_emitted[WASM_MAX_EVENTS];
    bool                has_emitted[WASM_MAX_EVENTS];
} wasm_slot_t;

/* ======================================================================
 * Global State
 * ====================================================================== */

static IM3Environment s_env;
static wasm_slot_t    s_slots[WASM_MAX_MODULES];
static SemaphoreHandle_t s_mutex;

/* Current frame data (set before calling on_frame, read by host imports). */
static const float *s_cur_phases;
static const float *s_cur_amplitudes;
static const float *s_cur_variances;
static uint16_t     s_cur_n_sc;
static const edge_vitals_pkt_t *s_cur_vitals;
static uint8_t      s_cur_slot_id;  /**< Slot being executed (for emit_event). */

/* Phase history accessed via edge_processing.h accessors. */

/* ======================================================================
 * Capability check helper — returns true if the current slot has the cap.
 * If capabilities == 0 (raw WASM, no manifest), all caps are granted.
 * ====================================================================== */

static inline bool slot_has_cap(uint32_t cap)
{
    uint32_t caps = s_slots[s_cur_slot_id].capabilities;
    return (caps == 0) || ((caps & cap) != 0);
}

/* ======================================================================
 * Host API Imports (called by WASM modules)
 * ====================================================================== */

static m3ApiRawFunction(host_csi_get_phase)
{
    m3ApiReturnType(float);
    m3ApiGetArg(int32_t, subcarrier);

    float val = 0.0f;
    if (slot_has_cap(RVF_CAP_READ_PHASE) &&
        s_cur_phases && subcarrier >= 0 && subcarrier < (int32_t)s_cur_n_sc) {
        val = s_cur_phases[subcarrier];
    }
    m3ApiReturn(val);
}

static m3ApiRawFunction(host_csi_get_amplitude)
{
    m3ApiReturnType(float);
    m3ApiGetArg(int32_t, subcarrier);

    float val = 0.0f;
    if (slot_has_cap(RVF_CAP_READ_AMPLITUDE) &&
        s_cur_amplitudes && subcarrier >= 0 && subcarrier < (int32_t)s_cur_n_sc) {
        val = s_cur_amplitudes[subcarrier];
    }
    m3ApiReturn(val);
}

static m3ApiRawFunction(host_csi_get_variance)
{
    m3ApiReturnType(float);
    m3ApiGetArg(int32_t, subcarrier);

    float val = 0.0f;
    if (slot_has_cap(RVF_CAP_READ_VARIANCE) &&
        s_cur_variances && subcarrier >= 0 && subcarrier < (int32_t)s_cur_n_sc) {
        val = s_cur_variances[subcarrier];
    }
    m3ApiReturn(val);
}

static m3ApiRawFunction(host_csi_get_bpm_breathing)
{
    m3ApiReturnType(float);
    float val = 0.0f;
    if (slot_has_cap(RVF_CAP_READ_VITALS) && s_cur_vitals) {
        val = (float)s_cur_vitals->breathing_rate / 100.0f;
    }
    m3ApiReturn(val);
}

static m3ApiRawFunction(host_csi_get_bpm_heartrate)
{
    m3ApiReturnType(float);
    float val = 0.0f;
    if (slot_has_cap(RVF_CAP_READ_VITALS) && s_cur_vitals) {
        val = (float)s_cur_vitals->heartrate / 10000.0f;
    }
    m3ApiReturn(val);
}

static m3ApiRawFunction(host_csi_get_presence)
{
    m3ApiReturnType(int32_t);
    int32_t val = 0;
    if (slot_has_cap(RVF_CAP_READ_VITALS) &&
        s_cur_vitals && (s_cur_vitals->flags & 0x01)) {
        val = 1;
    }
    m3ApiReturn(val);
}

static m3ApiRawFunction(host_csi_get_motion_energy)
{
    m3ApiReturnType(float);
    float val = 0.0f;
    if (slot_has_cap(RVF_CAP_READ_VITALS) && s_cur_vitals) {
        val = s_cur_vitals->motion_energy;
    }
    m3ApiReturn(val);
}

static m3ApiRawFunction(host_csi_get_n_persons)
{
    m3ApiReturnType(int32_t);
    int32_t val = 0;
    if (slot_has_cap(RVF_CAP_READ_VITALS) && s_cur_vitals) {
        val = (int32_t)s_cur_vitals->n_persons;
    }
    m3ApiReturn(val);
}

static m3ApiRawFunction(host_csi_get_timestamp)
{
    m3ApiReturnType(int32_t);
    int32_t val = (int32_t)(esp_timer_get_time() / 1000);
    m3ApiReturn(val);
}

static m3ApiRawFunction(host_csi_emit_event)
{
    m3ApiGetArg(int32_t, event_type);
    m3ApiGetArg(float, value);

    if (!slot_has_cap(RVF_CAP_EMIT_EVENTS)) {
        m3ApiSuccess();
    }

    wasm_slot_t *slot = &s_slots[s_cur_slot_id];
    if (slot->n_events < WASM_MAX_EVENTS) {
        slot->events[slot->n_events].event_type = (uint8_t)event_type;
        slot->events[slot->n_events].value = value;
        slot->n_events++;
        slot->event_count++;
    }

    m3ApiSuccess();
}

static m3ApiRawFunction(host_csi_log)
{
    m3ApiGetArg(int32_t, ptr);
    m3ApiGetArg(int32_t, len);

    if (!slot_has_cap(RVF_CAP_LOG)) {
        m3ApiSuccess();
    }

    /* Safety: bounds-check against WASM memory. */
    uint32_t mem_size = 0;
    uint8_t *mem = m3_GetMemory(runtime, &mem_size, 0);
    if (mem && ptr >= 0 && len > 0 && (uint32_t)(ptr + len) <= mem_size) {
        char log_buf[128];
        int copy_len = (len > 127) ? 127 : len;
        memcpy(log_buf, mem + ptr, copy_len);
        log_buf[copy_len] = '\0';
        ESP_LOGI(TAG, "WASM[%u]: %s", s_cur_slot_id, log_buf);
    }

    m3ApiSuccess();
}

static m3ApiRawFunction(host_csi_get_phase_history)
{
    m3ApiReturnType(int32_t);
    m3ApiGetArg(int32_t, buf_ptr);
    m3ApiGetArg(int32_t, max_len);

    int32_t copied = 0;

    if (!slot_has_cap(RVF_CAP_READ_HISTORY)) {
        m3ApiReturn(0);
    }

    uint32_t mem_size = 0;
    uint8_t *mem = m3_GetMemory(runtime, &mem_size, 0);

    if (mem && buf_ptr >= 0 && max_len > 0 &&
        (uint32_t)(buf_ptr + max_len * sizeof(float)) <= mem_size) {
        /* Get phase history via accessor. */
        const float *history_buf = NULL;
        uint16_t history_len = 0, history_idx = 0;
        edge_get_phase_history(&history_buf, &history_len, &history_idx);

        if (history_buf) {
            int32_t to_copy = (history_len < max_len) ? history_len : max_len;
            float *dst = (float *)(mem + buf_ptr);

            /* Copy history in chronological order. */
            for (int32_t i = 0; i < to_copy; i++) {
                uint16_t ri = (history_idx + EDGE_PHASE_HISTORY_LEN
                               - history_len + i) % EDGE_PHASE_HISTORY_LEN;
                dst[i] = history_buf[ri];
            }
            copied = to_copy;
        }
    }

    m3ApiReturn(copied);
}

/* ======================================================================
 * Link host imports to a module
 * ====================================================================== */

static M3Result link_host_api(IM3Module module)
{
    M3Result r;
    const char *ns = "csi";

    r = m3_LinkRawFunction(module, ns, "csi_get_phase",          "f(i)",  host_csi_get_phase);
    if (r && strcmp(r, m3Err_functionLookupFailed) != 0) return r;

    r = m3_LinkRawFunction(module, ns, "csi_get_amplitude",      "f(i)",  host_csi_get_amplitude);
    if (r && strcmp(r, m3Err_functionLookupFailed) != 0) return r;

    r = m3_LinkRawFunction(module, ns, "csi_get_variance",       "f(i)",  host_csi_get_variance);
    if (r && strcmp(r, m3Err_functionLookupFailed) != 0) return r;

    r = m3_LinkRawFunction(module, ns, "csi_get_bpm_breathing",  "f()",   host_csi_get_bpm_breathing);
    if (r && strcmp(r, m3Err_functionLookupFailed) != 0) return r;

    r = m3_LinkRawFunction(module, ns, "csi_get_bpm_heartrate",  "f()",   host_csi_get_bpm_heartrate);
    if (r && strcmp(r, m3Err_functionLookupFailed) != 0) return r;

    r = m3_LinkRawFunction(module, ns, "csi_get_presence",       "i()",   host_csi_get_presence);
    if (r && strcmp(r, m3Err_functionLookupFailed) != 0) return r;

    r = m3_LinkRawFunction(module, ns, "csi_get_motion_energy",  "f()",   host_csi_get_motion_energy);
    if (r && strcmp(r, m3Err_functionLookupFailed) != 0) return r;

    r = m3_LinkRawFunction(module, ns, "csi_get_n_persons",      "i()",   host_csi_get_n_persons);
    if (r && strcmp(r, m3Err_functionLookupFailed) != 0) return r;

    r = m3_LinkRawFunction(module, ns, "csi_get_timestamp",      "i()",   host_csi_get_timestamp);
    if (r && strcmp(r, m3Err_functionLookupFailed) != 0) return r;

    r = m3_LinkRawFunction(module, ns, "csi_emit_event",         "v(if)", host_csi_emit_event);
    if (r && strcmp(r, m3Err_functionLookupFailed) != 0) return r;

    r = m3_LinkRawFunction(module, ns, "csi_log",                "v(ii)", host_csi_log);
    if (r && strcmp(r, m3Err_functionLookupFailed) != 0) return r;

    r = m3_LinkRawFunction(module, ns, "csi_get_phase_history",  "i(ii)", host_csi_get_phase_history);
    if (r && strcmp(r, m3Err_functionLookupFailed) != 0) return r;

    return m3Err_none;
}

/* ======================================================================
 * Send output packet
 * ====================================================================== */

/** Dead-band threshold: only export events whose value changed by >5%. */
#define DEADBAND_RATIO 0.05f

static void send_wasm_output(uint8_t slot_id)
{
    wasm_slot_t *slot = &s_slots[slot_id];
    if (slot->n_events == 0) return;

    /* Dead-band filter: suppress events whose value hasn't changed significantly. */
    wasm_event_t filtered[WASM_MAX_EVENTS];
    uint8_t n_filtered = 0;

    for (uint8_t i = 0; i < slot->n_events; i++) {
        uint8_t et = slot->events[i].event_type;
        float val = slot->events[i].value;

        if (et < WASM_MAX_EVENTS && slot->has_emitted[et]) {
            float prev = slot->last_emitted[et];
            float abs_prev = (prev < 0.0f) ? -prev : prev;
            float abs_diff = ((val - prev) < 0.0f) ? -(val - prev) : (val - prev);

            /* Skip if within dead-band: |delta| < 5% of |previous|, and |previous| > epsilon. */
            if (abs_prev > 0.001f && abs_diff < DEADBAND_RATIO * abs_prev) {
                continue;
            }
        }

        /* Event passes filter — record and emit. */
        if (et < WASM_MAX_EVENTS) {
            slot->last_emitted[et] = val;
            slot->has_emitted[et] = true;
        }
        filtered[n_filtered++] = slot->events[i];
    }

    if (n_filtered == 0) {
        slot->n_events = 0;
        return;
    }

    wasm_output_pkt_t pkt;
    memset(&pkt, 0, sizeof(pkt));

    pkt.magic = WASM_OUTPUT_MAGIC;
    pkt.node_id = csi_collector_get_node_id();  /* #390: defensive copy */
    pkt.module_id = slot_id;
    pkt.event_count = n_filtered;

    memcpy(pkt.events, filtered, n_filtered * sizeof(wasm_event_t));

    /* Send header + events (not full struct with empty padding). */
    uint16_t pkt_size = 8 + n_filtered * sizeof(wasm_event_t);
    stream_sender_send((const uint8_t *)&pkt, pkt_size);

    ESP_LOGD(TAG, "WASM[%u] output: %u/%u events (after deadband)",
             slot_id, n_filtered, slot->n_events);

    slot->n_events = 0;
}

/* ======================================================================
 * Public API
 * ====================================================================== */

esp_err_t wasm_runtime_init(void)
{
    s_mutex = xSemaphoreCreateMutex();
    if (s_mutex == NULL) {
        ESP_LOGE(TAG, "Failed to create WASM runtime mutex");
        return ESP_ERR_NO_MEM;
    }

    s_env = m3_NewEnvironment();
    if (s_env == NULL) {
        ESP_LOGE(TAG, "Failed to create WASM3 environment");
        return ESP_ERR_NO_MEM;
    }

    memset(s_slots, 0, sizeof(s_slots));
    for (int i = 0; i < WASM_MAX_MODULES; i++) {
        s_slots[i].state = WASM_MODULE_EMPTY;

        /* Pre-allocate fixed PSRAM arena per slot to avoid fragmentation. */
        s_slots[i].arena = heap_caps_malloc(WASM_ARENA_SIZE,
                                            MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT);
        if (s_slots[i].arena == NULL) {
            ESP_LOGW(TAG, "Failed to allocate PSRAM arena for slot %d, falling back to heap", i);
        } else {
            ESP_LOGD(TAG, "PSRAM arena %d: %d KB at %p",
                     i, WASM_ARENA_SIZE / 1024, s_slots[i].arena);
        }
    }

    ESP_LOGI(TAG, "WASM runtime initialized (max_modules=%d, arena=%d KB/slot, "
             "budget=%d us/frame)",
             WASM_MAX_MODULES, WASM_ARENA_SIZE / 1024, WASM_FRAME_BUDGET_US);

    return ESP_OK;
}

esp_err_t wasm_runtime_load(const uint8_t *wasm_data, uint32_t wasm_len,
                            uint8_t *module_id)
{
    if (wasm_data == NULL || wasm_len == 0) {
        return ESP_ERR_INVALID_ARG;
    }
    if (wasm_len > WASM_MAX_MODULE_SIZE) {
        ESP_LOGE(TAG, "WASM binary too large: %lu > %d",
                 (unsigned long)wasm_len, WASM_MAX_MODULE_SIZE);
        return ESP_ERR_INVALID_SIZE;
    }

    xSemaphoreTake(s_mutex, portMAX_DELAY);

    /* Find free slot. */
    int slot_id = -1;
    for (int i = 0; i < WASM_MAX_MODULES; i++) {
        if (s_slots[i].state == WASM_MODULE_EMPTY) {
            slot_id = i;
            break;
        }
    }

    if (slot_id < 0) {
        xSemaphoreGive(s_mutex);
        ESP_LOGE(TAG, "No free WASM module slots");
        return ESP_ERR_NO_MEM;
    }

    wasm_slot_t *slot = &s_slots[slot_id];

    /* Use pre-allocated fixed arena (avoids PSRAM fragmentation). */
    if (slot->arena != NULL) {
        if (wasm_len > WASM_ARENA_SIZE) {
            xSemaphoreGive(s_mutex);
            ESP_LOGE(TAG, "WASM binary %lu > arena %d", (unsigned long)wasm_len, WASM_ARENA_SIZE);
            return ESP_ERR_INVALID_SIZE;
        }
        slot->binary = slot->arena;
    } else {
        /* Fallback: dynamic allocation if arena failed at boot. */
        slot->binary = malloc(wasm_len);
        if (slot->binary == NULL) {
            xSemaphoreGive(s_mutex);
            ESP_LOGE(TAG, "Failed to allocate %lu bytes for WASM binary",
                     (unsigned long)wasm_len);
            return ESP_ERR_NO_MEM;
        }
    }

    memcpy(slot->binary, wasm_data, wasm_len);
    slot->binary_size = wasm_len;

    /* Create WASM3 runtime. */
    slot->runtime = m3_NewRuntime(s_env, WASM_STACK_SIZE, NULL);
    if (slot->runtime == NULL) {
        free(slot->binary);
        slot->binary = NULL;
        xSemaphoreGive(s_mutex);
        ESP_LOGE(TAG, "Failed to create WASM3 runtime for slot %d", slot_id);
        return ESP_ERR_NO_MEM;
    }

    /* Parse module. */
    M3Result result = m3_ParseModule(s_env, &slot->module,
                                      slot->binary, wasm_len);
    if (result) {
        ESP_LOGE(TAG, "WASM parse error (slot %d): %s", slot_id, result);
        m3_FreeRuntime(slot->runtime);
        free(slot->binary);
        memset(slot, 0, sizeof(wasm_slot_t));
        xSemaphoreGive(s_mutex);
        return ESP_ERR_INVALID_STATE;
    }

    /* Load module into runtime. */
    result = m3_LoadModule(slot->runtime, slot->module);
    if (result) {
        ESP_LOGE(TAG, "WASM load error (slot %d): %s", slot_id, result);
        m3_FreeRuntime(slot->runtime);
        free(slot->binary);
        memset(slot, 0, sizeof(wasm_slot_t));
        xSemaphoreGive(s_mutex);
        return ESP_ERR_INVALID_STATE;
    }

    /* Link host API. */
    result = link_host_api(slot->module);
    if (result) {
        ESP_LOGE(TAG, "WASM link error (slot %d): %s", slot_id, result);
        m3_FreeRuntime(slot->runtime);
        free(slot->binary);
        memset(slot, 0, sizeof(wasm_slot_t));
        xSemaphoreGive(s_mutex);
        return ESP_ERR_INVALID_STATE;
    }

    /* Find exported lifecycle functions. */
    m3_FindFunction(&slot->fn_on_init,  slot->runtime, "on_init");
    m3_FindFunction(&slot->fn_on_frame, slot->runtime, "on_frame");
    m3_FindFunction(&slot->fn_on_timer, slot->runtime, "on_timer");

    if (slot->fn_on_frame == NULL) {
        ESP_LOGW(TAG, "WASM[%d]: no on_frame export (module may be passive)", slot_id);
    }

    slot->state = WASM_MODULE_LOADED;
    slot->frame_count = 0;
    slot->event_count = 0;
    slot->error_count = 0;
    slot->n_events = 0;

    if (module_id) *module_id = (uint8_t)slot_id;

    ESP_LOGI(TAG, "WASM module loaded into slot %d (%lu bytes)",
             slot_id, (unsigned long)wasm_len);

    xSemaphoreGive(s_mutex);
    return ESP_OK;
}

esp_err_t wasm_runtime_start(uint8_t module_id)
{
    if (module_id >= WASM_MAX_MODULES) return ESP_ERR_INVALID_ARG;

    xSemaphoreTake(s_mutex, portMAX_DELAY);

    wasm_slot_t *slot = &s_slots[module_id];
    if (slot->state != WASM_MODULE_LOADED && slot->state != WASM_MODULE_STOPPED) {
        xSemaphoreGive(s_mutex);
        return ESP_ERR_INVALID_STATE;
    }

    /* Call on_init if available. */
    if (slot->fn_on_init) {
        M3Result result = m3_CallV(slot->fn_on_init);
        if (result) {
            ESP_LOGE(TAG, "WASM[%u] on_init failed: %s", module_id, result);
            slot->state = WASM_MODULE_ERROR;
            slot->error_count++;
            xSemaphoreGive(s_mutex);
            return ESP_ERR_INVALID_STATE;
        }
    }

    slot->state = WASM_MODULE_RUNNING;
    ESP_LOGI(TAG, "WASM module %u started", module_id);

    xSemaphoreGive(s_mutex);
    return ESP_OK;
}

esp_err_t wasm_runtime_stop(uint8_t module_id)
{
    if (module_id >= WASM_MAX_MODULES) return ESP_ERR_INVALID_ARG;

    xSemaphoreTake(s_mutex, portMAX_DELAY);

    wasm_slot_t *slot = &s_slots[module_id];
    if (slot->state != WASM_MODULE_RUNNING) {
        xSemaphoreGive(s_mutex);
        return ESP_ERR_INVALID_STATE;
    }

    slot->state = WASM_MODULE_STOPPED;
    ESP_LOGI(TAG, "WASM module %u stopped (frames=%lu, events=%lu)",
             module_id, (unsigned long)slot->frame_count,
             (unsigned long)slot->event_count);

    xSemaphoreGive(s_mutex);
    return ESP_OK;
}

esp_err_t wasm_runtime_unload(uint8_t module_id)
{
    if (module_id >= WASM_MAX_MODULES) return ESP_ERR_INVALID_ARG;

    xSemaphoreTake(s_mutex, portMAX_DELAY);

    wasm_slot_t *slot = &s_slots[module_id];
    if (slot->state == WASM_MODULE_EMPTY) {
        xSemaphoreGive(s_mutex);
        return ESP_ERR_INVALID_STATE;
    }

    if (slot->runtime) {
        m3_FreeRuntime(slot->runtime);
    }

    /* Keep the arena allocated (fixed, reusable). Only free dynamic fallback. */
    uint8_t *arena_save = slot->arena;
    if (slot->binary && slot->binary != slot->arena) {
        free(slot->binary);
    }

    ESP_LOGI(TAG, "WASM module %u unloaded", module_id);
    memset(slot, 0, sizeof(wasm_slot_t));
    slot->state = WASM_MODULE_EMPTY;
    slot->arena = arena_save;  /* Restore arena pointer. */

    xSemaphoreGive(s_mutex);
    return ESP_OK;
}

void wasm_runtime_on_frame(const float *phases, const float *amplitudes,
                           const float *variances, uint16_t n_sc,
                           const edge_vitals_pkt_t *vitals)
{
    /* Set current frame data for host imports. */
    s_cur_phases = phases;
    s_cur_amplitudes = amplitudes;
    s_cur_variances = variances;
    s_cur_n_sc = n_sc;
    s_cur_vitals = vitals;

    for (uint8_t i = 0; i < WASM_MAX_MODULES; i++) {
        wasm_slot_t *slot = &s_slots[i];
        if (slot->state != WASM_MODULE_RUNNING || slot->fn_on_frame == NULL) {
            continue;
        }

        s_cur_slot_id = i;
        slot->n_events = 0;

        /* Budget guard: measure execution time. */
        int64_t t_start = esp_timer_get_time();

        M3Result result = m3_CallV(slot->fn_on_frame, (int32_t)n_sc);

        int64_t t_elapsed = esp_timer_get_time() - t_start;
        uint32_t elapsed_us = (uint32_t)(t_elapsed & 0xFFFFFFFF);

        if (result) {
            slot->error_count++;
            if (slot->error_count <= 5) {
                ESP_LOGW(TAG, "WASM[%u] on_frame error: %s", i, result);
            }
            if (slot->error_count >= 100) {
                ESP_LOGE(TAG, "WASM[%u] too many errors, stopping", i);
                slot->state = WASM_MODULE_ERROR;
            }
            continue;
        }

        /* Update telemetry. */
        slot->frame_count++;
        slot->total_us += elapsed_us;
        if (elapsed_us > slot->max_us) {
            slot->max_us = elapsed_us;
        }

        /* Budget enforcement: use per-slot budget from RVF manifest, or global. */
        uint32_t budget = (slot->manifest_budget_us > 0)
                        ? slot->manifest_budget_us : WASM_FRAME_BUDGET_US;
        if (elapsed_us > budget) {
            slot->budget_faults++;
            ESP_LOGW(TAG, "WASM[%u] budget exceeded: %lu us > %lu us (fault #%lu)",
                     i, (unsigned long)elapsed_us, (unsigned long)budget,
                     (unsigned long)slot->budget_faults);
            if (slot->budget_faults >= 10) {
                ESP_LOGE(TAG, "WASM[%u] stopped: 10 consecutive budget faults", i);
                slot->state = WASM_MODULE_ERROR;
                continue;
            }
        } else {
            /* Reset consecutive fault counter on a good frame. */
            if (slot->budget_faults > 0 && elapsed_us < budget / 2) {
                slot->budget_faults = 0;
            }
        }

        /* Send output if events were emitted. */
        if (slot->n_events > 0) {
            send_wasm_output(i);
        }
    }

    /* Clear references. */
    s_cur_phases = NULL;
    s_cur_amplitudes = NULL;
    s_cur_variances = NULL;
    s_cur_vitals = NULL;
}

void wasm_runtime_on_timer(void)
{
    for (uint8_t i = 0; i < WASM_MAX_MODULES; i++) {
        wasm_slot_t *slot = &s_slots[i];
        if (slot->state != WASM_MODULE_RUNNING || slot->fn_on_timer == NULL) {
            continue;
        }

        s_cur_slot_id = i;
        slot->n_events = 0;

        M3Result result = m3_CallV(slot->fn_on_timer);
        if (result) {
            slot->error_count++;
            ESP_LOGW(TAG, "WASM[%u] on_timer error: %s", i, result);
        }

        if (slot->n_events > 0) {
            send_wasm_output(i);
        }
    }
}

void wasm_runtime_get_info(wasm_module_info_t *info, uint8_t *count)
{
    xSemaphoreTake(s_mutex, portMAX_DELAY);

    uint8_t n = 0;
    for (uint8_t i = 0; i < WASM_MAX_MODULES; i++) {
        info[i].id = i;
        info[i].state = s_slots[i].state;
        info[i].binary_size = s_slots[i].binary_size;
        info[i].frame_count = s_slots[i].frame_count;
        info[i].event_count = s_slots[i].event_count;
        info[i].error_count = s_slots[i].error_count;
        info[i].total_us = s_slots[i].total_us;
        info[i].max_us = s_slots[i].max_us;
        info[i].budget_faults = s_slots[i].budget_faults;
        memcpy(info[i].module_name, s_slots[i].module_name, 32);
        info[i].capabilities = s_slots[i].capabilities;
        info[i].manifest_budget_us = s_slots[i].manifest_budget_us;
        if (s_slots[i].state != WASM_MODULE_EMPTY) n++;
    }
    if (count) *count = n;

    xSemaphoreGive(s_mutex);
}

esp_err_t wasm_runtime_set_manifest(uint8_t module_id, const char *module_name,
                                     uint32_t capabilities, uint32_t max_frame_us)
{
    if (module_id >= WASM_MAX_MODULES) return ESP_ERR_INVALID_ARG;

    xSemaphoreTake(s_mutex, portMAX_DELAY);

    wasm_slot_t *slot = &s_slots[module_id];
    if (slot->state == WASM_MODULE_EMPTY) {
        xSemaphoreGive(s_mutex);
        return ESP_ERR_INVALID_STATE;
    }

    if (module_name) {
        strncpy(slot->module_name, module_name, 31);
        slot->module_name[31] = '\0';
    }
    slot->capabilities = capabilities;
    slot->manifest_budget_us = max_frame_us;

    ESP_LOGI(TAG, "WASM[%u] manifest applied: name=\"%s\" caps=0x%04lx budget=%lu us",
             module_id, slot->module_name,
             (unsigned long)capabilities, (unsigned long)max_frame_us);

    xSemaphoreGive(s_mutex);
    return ESP_OK;
}

#else /* !CONFIG_WASM_ENABLE || !WASM3_AVAILABLE */

/* ======================================================================
 * No-op stubs when WASM3 is not available.
 * All functions return success or do nothing so the rest of the
 * firmware compiles and runs without the Tier 3 WASM layer.
 * ====================================================================== */

#include <string.h>
#include "esp_log.h"

static const char *TAG = "wasm_rt";

esp_err_t wasm_runtime_init(void)
{
    ESP_LOGW(TAG, "WASM Tier 3 disabled (WASM3 not available)");
    return ESP_OK;
}

esp_err_t wasm_runtime_load(const uint8_t *binary, uint32_t size, uint8_t *out_id)
{
    (void)binary; (void)size; (void)out_id;
    return ESP_ERR_NOT_SUPPORTED;
}

esp_err_t wasm_runtime_start(uint8_t module_id)
{
    (void)module_id;
    return ESP_ERR_NOT_SUPPORTED;
}

esp_err_t wasm_runtime_stop(uint8_t module_id)
{
    (void)module_id;
    return ESP_ERR_NOT_SUPPORTED;
}

esp_err_t wasm_runtime_unload(uint8_t module_id)
{
    (void)module_id;
    return ESP_ERR_NOT_SUPPORTED;
}

void wasm_runtime_on_frame(const float *phases, const float *amplitudes,
                           const float *variances, uint16_t n_sc,
                           const edge_vitals_pkt_t *vitals)
{
    (void)phases; (void)amplitudes; (void)variances; (void)n_sc; (void)vitals;
}

void wasm_runtime_on_timer(void) { }

void wasm_runtime_get_info(wasm_module_info_t *info, uint8_t *count)
{
    memset(info, 0, sizeof(wasm_module_info_t) * WASM_MAX_MODULES);
    *count = 0;
}

esp_err_t wasm_runtime_set_manifest(uint8_t module_id, const char *module_name,
                                     uint32_t capabilities, uint32_t max_frame_us)
{
    (void)module_id; (void)module_name; (void)capabilities; (void)max_frame_us;
    return ESP_ERR_NOT_SUPPORTED;
}

#endif /* CONFIG_WASM_ENABLE && WASM3_AVAILABLE */
