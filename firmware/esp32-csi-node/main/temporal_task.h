/* SPDX-License-Identifier: MIT
 *
 * temporal_task.h — On-device temporal head FreeRTOS task (ADR-095, #513).
 *
 * Owns the lifecycle of the `ruv_temporal_ctx_t` from
 * components/ruv_temporal/include/ruv_temporal.h. Exposes:
 *
 *   1. `temporal_task_start()` — spawn the task with its own 16 KB stack
 *      pinned to Core 1, allocate a feed queue. Caller (main.c) ignores
 *      ESP_ERR_NOT_SUPPORTED when CONFIG_CSI_TEMPORAL_HEAD_ENABLED is off.
 *   2. `temporal_task_push_frame()` — non-blocking enqueue from the
 *      adaptive_controller fast loop. Drops on full queue (logs once
 *      per second) — the temporal head is best-effort, the physics-only
 *      path keeps producing vitals regardless.
 *   3. `temporal_task_stop()` — cleanly tear down (currently used only
 *      for tests; production firmware never calls this).
 *
 * Thread safety: per ADR-095 §3.3 the temporal task itself is the
 * single owner of the underlying `ruv_temporal_ctx_t`. Callers
 * communicate exclusively via the FreeRTOS queue.
 *
 * Output: every ~1 s the task runs `ruv_temporal_classify` and emits a
 * `0xC5110007 RV_TEMPORAL_CLASSIFICATION` packet via stream_sender.
 */

#pragma once

#include <stdint.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

/* Magic for the classification packet (ADR-095 §3.5). 0xC5110001..0006
 * are taken; 0007 is the next free slot. */
#define RV_TEMPORAL_PKT_MAGIC 0xC5110007u

/* On-the-wire packet for one classification result. Little-endian.
 * Size: 40 bytes. CRC covers everything before it.
 *
 * Field layout (bytes):
 *   [00..04)  magic              4
 *   [04..06)  version            2
 *   [06..08)  n_classes          2
 *   [08..09)  node_id            1
 *   [09..0C)  reserved           3
 *   [0C..14)  ts_us              8
 *   [14..18)  seq                4
 *   [18..19)  argmax             1
 *   [19..1C)  reserved2          3
 *   [1C..20)  top_logit          4
 *   [20..24)  top1_minus_top2    4
 *   [24..28)  crc32              4
 *   total: 40
 */
typedef struct __attribute__((packed)) {
    uint32_t magic;          /* 0xC5110007 */
    uint16_t version;        /* 1 */
    uint16_t n_classes;      /* matches init() value */
    uint8_t  node_id;        /* csi_collector_get_node_id() */
    uint8_t  reserved[3];
    uint64_t ts_us;          /* esp_timer_get_time() at classify */
    uint32_t seq;             /* monotonic, increments per emit */
    uint8_t  argmax;          /* highest-logit class */
    uint8_t  reserved2[3];
    float    top_logit;       /* logits[argmax] */
    float    top1_minus_top2; /* margin — useful for downstream gating */
    uint32_t crc32;
} rv_temporal_pkt_t;

/* Build-time guard so the wire format never silently changes. */
_Static_assert(sizeof(rv_temporal_pkt_t) == 40,
               "rv_temporal_pkt_t must be 40 bytes (ADR-095 §3.5)");

/* Start the temporal task. Returns ESP_ERR_NOT_SUPPORTED when the
 * feature is compiled out — caller should treat that as a non-error
 * and continue. Returns ESP_OK on success.
 *
 * input_dim   : feature dimension per frame (e.g. 60 for rv_feature_state_t)
 * window_len  : rolling window in frames (e.g. 256)
 * n_classes   : number of output logits the model produces (e.g. 4)
 */
esp_err_t temporal_task_start(uint32_t input_dim,
                              uint32_t window_len,
                              uint32_t n_classes);

/* Non-blocking push from the adaptive_controller fast loop. Returns
 * ESP_OK on enqueue, ESP_ERR_NOT_FOUND if the task isn't running,
 * ESP_ERR_TIMEOUT if the queue was full. Never blocks the caller. */
esp_err_t temporal_task_push_frame(const float *frame, uint32_t frame_len);

/* Optional teardown — currently unit-test only. */
void temporal_task_stop(void);

#ifdef __cplusplus
}
#endif
