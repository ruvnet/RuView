/**
 * @file ping_rate.h
 * @brief Self-ping (#954) session parameters -- pure C, no ESP-IDF.
 *
 * Split out of csi_collector.c so the reply-timeout rule can be host-tested
 * directly (tests/host/test_ping_rate.c) as well as through the production
 * session config (test/test_self_ping.c).
 */
#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Reply timeout for the self-ping session (CONFIG_CSI_SELF_PING_TIMEOUT_MS).
 *
 * @param interval_ms    probe interval, 1000 / CONFIG_CSI_SELF_PING_HZ.
 * @param configured_ms  CONFIG_CSI_SELF_PING_TIMEOUT_MS.
 * @return configured_ms, but never less than interval_ms: a timeout shorter
 *         than the interval would count replies lost that are still on time
 *         for the next probe slot.
 */
uint32_t ping_timeout_resolve(uint32_t interval_ms, uint32_t configured_ms);

#ifdef __cplusplus
}
#endif
