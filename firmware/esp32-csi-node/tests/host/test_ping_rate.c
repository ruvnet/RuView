/*
 * Host unit tests for ping_rate.c: the self-ping reply timeout.
 *
 * Pure C, no ESP-IDF -- same pattern as the rest of tests/host/.
 *
 * Build + run (from this directory):
 *   make test_ping_rate
 *   ./test_ping_rate
 */
#include <stdio.h>

#include "ping_rate.h"

static int g_pass = 0, g_fail = 0;

#define CHECK(cond, msg) do {                                           \
    if (cond) { g_pass++; }                                             \
    else { g_fail++; printf("  FAIL: %s (line %d)\n", msg, __LINE__); } \
} while (0)

int main(void)
{
    printf("test: reply timeout is the Kconfig value, floored at one interval\n");
    CHECK(ping_timeout_resolve(20, 200) == 200, "50 Hz, default 200 ms kept");
    CHECK(ping_timeout_resolve(100, 200) == 200, "10 Hz, 200 ms kept");
    CHECK(ping_timeout_resolve(20, 20) == 20, "timeout equal to the interval kept");
    CHECK(ping_timeout_resolve(100, 20) == 100, "20 ms at 10 Hz raised to the 100 ms interval");
    CHECK(ping_timeout_resolve(40, 1000) == 1000, "the old IDF 1000 ms still expressible");
    CHECK(ping_timeout_resolve(20, 200) > 83,
          "default keeps the slowest measured working-gateway RTT (67-83 ms) a reply, not a loss");

    printf("%d passed, %d failed\n", g_pass, g_fail);
    return g_fail == 0 ? 0 : 1;
}
