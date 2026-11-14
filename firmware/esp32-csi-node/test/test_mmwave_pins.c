#include <assert.h>
#include "mmwave_pins.h"

int main(void)
{
    int tx = -1, rx = -1;
    ruview_c6_mmwave_default_pins(false, &tx, &rx);
    assert(tx == 4 && rx == 5);
    ruview_c6_mmwave_default_pins(true, &tx, &rx);
    assert(tx == 16 && rx == 17);

    /* Native USB has no GPIO console assignment, so the assembled XIAO
     * carrier UART remains usable. A UART console on those pins still fails
     * closed. */
    assert(!ruview_mmwave_pins_overlap_console(16, 17, -1, -1));
    assert(ruview_mmwave_pins_overlap_console(16, 17, 16, 17));
    assert(ruview_mmwave_pins_overlap_console(4, 5, 5, 6));

    assert(!ruview_c5_mmwave_pins_safe(-1, -1, 11, 12));
    assert(!ruview_c5_mmwave_pins_safe(17, 18, 11, 12));
    for (int pin = 16; pin <= 22; pin++) {
        assert(!ruview_c5_mmwave_pins_safe(pin, 5, 11, 12));
        assert(!ruview_c5_mmwave_pins_safe(4, pin, 11, 12));
    }
    assert(!ruview_c5_mmwave_pins_safe(11, 5, 11, 12));
    assert(!ruview_c5_mmwave_pins_safe(4, 12, 11, 12));
    assert(!ruview_c5_mmwave_pins_safe(4, 4, 11, 12));
    assert(!ruview_c5_mmwave_pins_safe(29, 5, 11, 12));
    assert(ruview_c5_mmwave_pins_safe(4, 5, 11, 12));
    assert(!ruview_c5_mmwave_pins_safe(4, 5, 4, 5));
    return 0;
}
