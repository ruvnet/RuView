#include <assert.h>
#include <stdio.h>

#include "c6_antenna_select.h"

int main(void)
{
    assert(c6_xiao_antenna_gpio14_level(C6_XIAO_ANTENNA_INTERNAL) == 0);
    assert(c6_xiao_antenna_gpio14_level(C6_XIAO_ANTENNA_EXTERNAL) == 1);

    puts("PASS: XIAO ESP32-C6 antenna selection truth table");
    return 0;
}
