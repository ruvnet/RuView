#include <assert.h>
#include "mmwave_pins.h"

int main(void)
{
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
