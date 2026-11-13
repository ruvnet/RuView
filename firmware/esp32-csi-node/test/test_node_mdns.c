/* Node mDNS TXT/hostname formatting (pure logic, SYNTHETIC). */
#define NODE_MDNS_HOST_TEST 1
#include "../main/node_mdns.c"
#include <assert.h>
#include <stdio.h>
#include <string.h>

int main(void)
{
    char buf[48];
    const uint8_t mac[6] = {0x20, 0x6e, 0xf1, 0x17, 0x0e, 0x34};

    assert(node_mdns_format_hostname(mac, buf, sizeof(buf)) == strlen("ruview-node-170e34"));
    assert(strcmp(buf, "ruview-node-170e34") == 0);
    /* Truncation is refused, not silently shortened into a colliding name. */
    assert(node_mdns_format_hostname(mac, buf, 10) == 0 && buf[0] == '\0');
    assert(node_mdns_format_hostname(NULL, buf, sizeof(buf)) == 0);

    assert(node_mdns_format_target("192.168.1.32", 5005, buf, sizeof(buf)) > 0);
    assert(strcmp(buf, "192.168.1.32:5005") == 0);
    assert(node_mdns_format_target("", 5005, buf, sizeof(buf)) == 0 && buf[0] == '\0');
    assert(node_mdns_format_target(NULL, 5005, buf, sizeof(buf)) == 0);
    assert(node_mdns_format_target("192.168.1.32", 0, buf, sizeof(buf)) == 0);
    assert(node_mdns_format_target("192.168.1.32", 5005, buf, 8) == 0 && buf[0] == '\0');

    puts("PASS: node mDNS hostname and csi_target formatting");
    return 0;
}
