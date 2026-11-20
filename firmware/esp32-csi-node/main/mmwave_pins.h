#pragma once
#include <stdbool.h>
#include <stddef.h>

static inline void ruview_c6_mmwave_default_pins(bool xiao_mr60bha2,
                                                 int *tx, int *rx)
{
    if (tx != NULL) *tx = xiao_mr60bha2 ? 16 : 4;
    if (rx != NULL) *rx = xiao_mr60bha2 ? 17 : 5;
}

static inline bool ruview_mmwave_pins_overlap_console(int tx, int rx,
                                                      int console_tx,
                                                      int console_rx)
{
    return (console_tx >= 0 && (tx == console_tx || rx == console_tx))
        || (console_rx >= 0 && (tx == console_rx || rx == console_rx));
}

/* C5 SPI0/1 uses GPIO16..22. Never route a sensor onto the memory bus.
 * No generic C5 sensor pin assignment is safe without board wiring. */
static inline bool ruview_c5_mmwave_pins_safe(int tx, int rx, int console_tx, int console_rx)
{
    return tx >= 0 && tx <= 28 && rx >= 0 && rx <= 28 && tx != rx
        && !(tx >= 16 && tx <= 22) && !(rx >= 16 && rx <= 22)
        && tx != console_tx && tx != console_rx
        && rx != console_tx && rx != console_rx;
}
