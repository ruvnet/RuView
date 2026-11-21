/**
 * @file ping_rate.c
 * @brief See ping_rate.h.
 */
#include "ping_rate.h"

uint32_t ping_timeout_resolve(uint32_t interval_ms, uint32_t configured_ms)
{
    return configured_ms < interval_ms ? interval_ms : configured_ms;
}
