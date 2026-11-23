#pragma once
#include <stdbool.h>

/* C5 SPI0/1 uses GPIO16..22. Never route a sensor onto the memory bus.
 * No generic C5 sensor pin assignment is safe without board wiring. */
static inline bool ruview_c5_mmwave_pins_safe(int tx, int rx, int console_tx, int console_rx)
{
    return tx >= 0 && tx <= 28 && rx >= 0 && rx <= 28 && tx != rx
        && !(tx >= 16 && tx <= 22) && !(rx >= 16 && rx <= 22)
        && tx != console_tx && tx != console_rx
        && rx != console_tx && rx != console_rx;
}
