/* SPDX-License-Identifier: MIT
 *
 * ESP32-S3 on-device temporal head — public C ABI (ADR-095, #513).
 *
 * Consumed by edge_processing.c / adaptive_controller.c. Backed by a
 * Rust staticlib that wraps `ruvllm_sparse_attention`. See
 * components/ruv_temporal/src/lib.rs for the implementation.
 *
 * Threading: NOT internally synchronised. Per ADR-095 §3.3 callers run
 * a single dedicated FreeRTOS task that owns the context and
 * serialises push() and classify(). init() and destroy() are NOT safe
 * against concurrent push/classify on the same handle.
 */

#pragma once

#include <stddef.h>
#include <stdint.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct RuvTemporalCtx ruv_temporal_ctx_t;

/* Allocate a temporal-head context.
 *
 * weights      — flat-buffer of model weights (Phase 5 wires the format),
 *                may be NULL during Phase 4 scaffolding.
 * weights_len  — bytes of `weights`, 0 if weights is NULL.
 * input_dim    — feature dimension per frame (e.g. 60 for rv_feature_state_t).
 * window_len   — number of frames in the rolling window (e.g. 256).
 * n_classes    — output logit count (e.g. 4 for gesture, 3 for fall).
 * out_ctx      — receives the new context pointer on ESP_OK.
 *
 * Returns ESP_OK on success, ESP_ERR_INVALID_ARG for null/zero inputs,
 * ESP_ERR_NO_MEM if buffer allocation fails.
 */
esp_err_t ruv_temporal_init(const uint8_t *weights,
                            size_t weights_len,
                            uint32_t input_dim,
                            uint32_t window_len,
                            uint32_t n_classes,
                            ruv_temporal_ctx_t **out_ctx);

/* Push one feature frame into the rolling window. Hot path — cheap,
 * no allocation. `frame` must point to at least `input_dim` floats.
 */
esp_err_t ruv_temporal_push(ruv_temporal_ctx_t *ctx, const float *frame);

/* Run the temporal-head forward and write `n_classes` class logits
 * into the caller-owned `logits` buffer (must be at least n_classes
 * floats). `n_classes` must match the value passed to init().
 */
esp_err_t ruv_temporal_classify(ruv_temporal_ctx_t *ctx,
                                float *logits,
                                uint32_t n_classes);

/* Release a context allocated by ruv_temporal_init. Safe on NULL. */
void ruv_temporal_destroy(ruv_temporal_ctx_t *ctx);

/* Self-test — proves the upstream sparse-attention kernel links and
 * runs. Returns ESP_OK on success. Useful as a smoke check on first
 * boot before allocating a real context.
 */
esp_err_t ruv_temporal_kernel_self_test(void);

#ifdef __cplusplus
}
#endif
