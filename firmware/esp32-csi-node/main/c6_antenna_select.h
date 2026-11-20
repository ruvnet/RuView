#pragma once

typedef enum {
    C6_XIAO_ANTENNA_INTERNAL = 0,
    C6_XIAO_ANTENNA_EXTERNAL = 1,
} c6_xiao_antenna_t;

static inline int c6_xiao_antenna_gpio14_level(c6_xiao_antenna_t antenna)
{
    return antenna == C6_XIAO_ANTENNA_EXTERNAL ? 1 : 0;
}

#ifdef ESP_PLATFORM
#include "esp_err.h"

esp_err_t c6_xiao_antenna_apply(void);
#endif
