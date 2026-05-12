/**
 * @file temporal_task.c
 * @brief ADR-095 / #513 — On-device temporal head FreeRTOS task.
 *
 * Owns the only `ruv_temporal_ctx_t` in the firmware. Receives feature
 * frames from the adaptive_controller fast loop via a FreeRTOS queue,
 * pushes them into the rolling window, and at ~1 Hz runs a
 * classification forward through the Rust `ruvllm_sparse_attention`
 * staticlib (when built — see CONFIG_CSI_TEMPORAL_HEAD_ENABLED).
 *
 * The whole file compiles down to no-op shims when the feature is off,
 * so adaptive_controller.c can call `temporal_task_push_frame()`
 * unconditionally — the function returns ESP_ERR_NOT_SUPPORTED and
 * costs one nullable check.
 */

#include "temporal_task.h"

#include <string.h>
#include "esp_log.h"
#include "esp_timer.h"
#include "sdkconfig.h"

static const char *TAG = "temporal";

#ifdef CONFIG_CSI_TEMPORAL_HEAD_ENABLED

#include "freertos/FreeRTOS.h"
#include "freertos/queue.h"
#include "freertos/task.h"

#include "csi_collector.h"  /* node_id */
#include "stream_sender.h"
#include "ruv_temporal.h"   /* C ABI from components/ruv_temporal */

/* Queue depth — picked so that the adaptive controller's fast loop
 * (default 5 Hz) can't overrun the temporal task even if classify()
 * stalls for ~6 s. Drops beyond that are logged. */
#define TEMPORAL_QUEUE_DEPTH 32

/* Stack sized per ADR-095 §3.3. The kernel forward + intermediate
 * tensors are bounded by `forward_flash` tiling, but rv_feature_state
 * marshalling, logging, and stream_sender_send all share this stack. */
#define TEMPORAL_TASK_STACK 16384

/* Pinned to Core 1, like edge_dsp. WiFi runs on Core 0 — keep them
 * apart so the temporal forward doesn't compete with CSI capture. */
#define TEMPORAL_TASK_CORE 1

/* Classification cadence in milliseconds. 1 Hz is the ADR-095 §3 default. */
#ifndef CONFIG_TEMPORAL_CLASSIFY_PERIOD_MS
#define CONFIG_TEMPORAL_CLASSIFY_PERIOD_MS 1000
#endif

/* Maximum logits buffer — sized to the largest n_classes any of the
 * ADR-095 §4 use cases needs (anomaly = 2, fall = 3, gesture = 8). */
#define TEMPORAL_MAX_LOGITS 16

/* ---- Module state ----------------------------------------------------- */

typedef struct {
    float frame[TEMPORAL_MAX_LOGITS * 8]; /* generous; trimmed via input_dim */
    uint32_t frame_len;
} temporal_msg_t;

static QueueHandle_t        s_queue;
static TaskHandle_t         s_task;
static ruv_temporal_ctx_t  *s_ctx;
static uint32_t             s_input_dim;
static uint32_t             s_window_len;
static uint32_t             s_n_classes;
static uint32_t             s_seq;
static uint32_t             s_drop_count;
static uint64_t             s_last_drop_log_us;

/* Lightweight CRC32 (IEEE 802.3 polynomial 0xEDB88320), table-free.
 * Used only for the 36-byte classification packet — speed isn't
 * critical. Existing firmware has its own CRC32 in csi_collector.c
 * but we don't link against it from here to keep coupling narrow. */
static uint32_t crc32_ieee(const uint8_t *data, size_t len)
{
    uint32_t crc = 0xFFFFFFFFu;
    for (size_t i = 0; i < len; i++) {
        crc ^= data[i];
        for (int b = 0; b < 8; b++) {
            uint32_t mask = -(int32_t)(crc & 1u);
            crc = (crc >> 1) ^ (0xEDB88320u & mask);
        }
    }
    return ~crc;
}

static void emit_classification(const float *logits, uint32_t n)
{
    /* Find argmax + margin in one pass. */
    uint32_t argmax = 0;
    float top1 = logits[0];
    float top2 = -1e30f;
    for (uint32_t i = 1; i < n; i++) {
        float v = logits[i];
        if (v > top1) {
            top2 = top1;
            top1 = v;
            argmax = i;
        } else if (v > top2) {
            top2 = v;
        }
    }

    rv_temporal_pkt_t pkt;
    memset(&pkt, 0, sizeof(pkt));
    pkt.magic = RV_TEMPORAL_PKT_MAGIC;
    pkt.version = 1;
    pkt.n_classes = (uint16_t)n;
    pkt.node_id = csi_collector_get_node_id();
    pkt.ts_us = (uint64_t)esp_timer_get_time();
    pkt.seq = ++s_seq;
    pkt.argmax = (uint8_t)argmax;
    pkt.top_logit = top1;
    pkt.top1_minus_top2 = top1 - top2;
    pkt.crc32 = crc32_ieee((const uint8_t *)&pkt, sizeof(pkt) - sizeof(pkt.crc32));

    int sent = stream_sender_send((const uint8_t *)&pkt, sizeof(pkt));
    if (sent < 0) {
        ESP_LOGW(TAG, "classification emit failed");
    }
}

static void temporal_task_loop(void *arg)
{
    (void)arg;
    ESP_LOGI(TAG, "temporal task online (window=%u dim=%u classes=%u core=%d)",
             (unsigned)s_window_len, (unsigned)s_input_dim,
             (unsigned)s_n_classes, TEMPORAL_TASK_CORE);

    /* Self-test the kernel link before touching real frames. */
    if (ruv_temporal_kernel_self_test() != ESP_OK) {
        ESP_LOGE(TAG, "ruv_temporal_kernel_self_test FAILED — temporal head disabled");
        s_ctx = NULL;
        vTaskDelete(NULL);
        return;
    }

    uint64_t next_classify_us = esp_timer_get_time()
                              + (uint64_t)CONFIG_TEMPORAL_CLASSIFY_PERIOD_MS * 1000ull;
    float logits[TEMPORAL_MAX_LOGITS];

    for (;;) {
        temporal_msg_t msg;
        /* Block up to 100 ms for a frame, then check if it's time to
         * classify. This double-poll keeps the cadence honest even
         * during long quiet periods. */
        if (xQueueReceive(s_queue, &msg, pdMS_TO_TICKS(100)) == pdTRUE) {
            if (s_ctx != NULL) {
                (void)ruv_temporal_push(s_ctx, msg.frame);
            }
        }

        uint64_t now_us = esp_timer_get_time();
        if (now_us >= next_classify_us && s_ctx != NULL) {
            esp_err_t cret = ruv_temporal_classify(s_ctx, logits, s_n_classes);
            if (cret == ESP_OK) {
                emit_classification(logits, s_n_classes);
            } else {
                ESP_LOGW(TAG, "classify returned 0x%x", (unsigned)cret);
            }
            next_classify_us = now_us
                             + (uint64_t)CONFIG_TEMPORAL_CLASSIFY_PERIOD_MS * 1000ull;
        }

        /* Coalesce drop-count logs to once per second so a backlog
         * doesn't flood the serial console. */
        if (s_drop_count > 0 && now_us - s_last_drop_log_us > 1000000ull) {
            ESP_LOGW(TAG, "queue full — dropped %u feature frames",
                     (unsigned)s_drop_count);
            s_drop_count = 0;
            s_last_drop_log_us = now_us;
        }
    }
}

esp_err_t temporal_task_start(uint32_t input_dim,
                              uint32_t window_len,
                              uint32_t n_classes)
{
    if (s_task != NULL) {
        return ESP_OK;  /* idempotent */
    }
    if (input_dim == 0 || window_len == 0 || n_classes == 0) {
        return ESP_ERR_INVALID_ARG;
    }
    if (n_classes > TEMPORAL_MAX_LOGITS) {
        ESP_LOGE(TAG, "n_classes=%u exceeds TEMPORAL_MAX_LOGITS=%d",
                 (unsigned)n_classes, TEMPORAL_MAX_LOGITS);
        return ESP_ERR_INVALID_SIZE;
    }

    /* Allocate the kernel context. Phase 4 stub returns ESP_OK without
     * weights; Phase 5b will accept a real weights blob. */
    esp_err_t ret = ruv_temporal_init(NULL, 0, input_dim, window_len, n_classes,
                                      &s_ctx);
    if (ret != ESP_OK) {
        ESP_LOGE(TAG, "ruv_temporal_init failed: 0x%x", (unsigned)ret);
        return ret;
    }

    s_input_dim = input_dim;
    s_window_len = window_len;
    s_n_classes = n_classes;
    s_seq = 0;
    s_drop_count = 0;
    s_last_drop_log_us = 0;

    s_queue = xQueueCreate(TEMPORAL_QUEUE_DEPTH, sizeof(temporal_msg_t));
    if (s_queue == NULL) {
        ESP_LOGE(TAG, "queue create failed");
        ruv_temporal_destroy(s_ctx);
        s_ctx = NULL;
        return ESP_ERR_NO_MEM;
    }

    BaseType_t ok = xTaskCreatePinnedToCore(
        temporal_task_loop, "ruv_temporal", TEMPORAL_TASK_STACK,
        NULL, 4 /* priority, below edge_dsp */,
        &s_task, TEMPORAL_TASK_CORE);
    if (ok != pdPASS) {
        ESP_LOGE(TAG, "task create failed");
        vQueueDelete(s_queue);
        s_queue = NULL;
        ruv_temporal_destroy(s_ctx);
        s_ctx = NULL;
        return ESP_ERR_NO_MEM;
    }
    return ESP_OK;
}

esp_err_t temporal_task_push_frame(const float *frame, uint32_t frame_len)
{
    if (frame == NULL || frame_len == 0) {
        return ESP_ERR_INVALID_ARG;
    }
    if (s_queue == NULL) {
        return ESP_ERR_NOT_FOUND;
    }
    temporal_msg_t msg;
    uint32_t cap = (uint32_t)(sizeof(msg.frame) / sizeof(msg.frame[0]));
    uint32_t n = (frame_len < cap) ? frame_len : cap;
    if (n < s_input_dim) {
        /* Pad short frames with zeros so the rolling window stays
         * dimension-stable from the kernel's perspective. */
        memcpy(msg.frame, frame, n * sizeof(float));
        memset(&msg.frame[n], 0, (s_input_dim - n) * sizeof(float));
        msg.frame_len = s_input_dim;
    } else {
        memcpy(msg.frame, frame, s_input_dim * sizeof(float));
        msg.frame_len = s_input_dim;
    }

    /* Non-blocking — temporal head is best-effort. */
    if (xQueueSend(s_queue, &msg, 0) != pdPASS) {
        s_drop_count++;
        return ESP_ERR_TIMEOUT;
    }
    return ESP_OK;
}

void temporal_task_stop(void)
{
    if (s_task != NULL) {
        vTaskDelete(s_task);
        s_task = NULL;
    }
    if (s_queue != NULL) {
        vQueueDelete(s_queue);
        s_queue = NULL;
    }
    if (s_ctx != NULL) {
        ruv_temporal_destroy(s_ctx);
        s_ctx = NULL;
    }
}

#else  /* !CONFIG_CSI_TEMPORAL_HEAD_ENABLED */

esp_err_t temporal_task_start(uint32_t input_dim,
                              uint32_t window_len,
                              uint32_t n_classes)
{
    (void)input_dim;
    (void)window_len;
    (void)n_classes;
    return ESP_ERR_NOT_SUPPORTED;
}

esp_err_t temporal_task_push_frame(const float *frame, uint32_t frame_len)
{
    (void)frame;
    (void)frame_len;
    return ESP_ERR_NOT_SUPPORTED;
}

void temporal_task_stop(void) {}

#endif /* CONFIG_CSI_TEMPORAL_HEAD_ENABLED */
