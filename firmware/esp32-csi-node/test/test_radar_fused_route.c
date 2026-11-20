/* Actual production sender with captured socket calls; no network traffic. */
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <math.h>
#define socket route_test_socket
#define sendto route_test_sendto
#define close route_test_close
#include "../main/stream_sender.c"
static int sends, error_mode;
static int64_t test_now;
static uint16_t observed_port;
static uint32_t observed_ip;
static uint8_t observed[512];
static size_t observed_len;
int route_test_socket(int domain, int type, int protocol) {
    assert(domain==AF_INET && type==SOCK_DGRAM && protocol==IPPROTO_UDP); return 7;
}
int route_test_close(int fd) { assert(fd==7); return 0; }
ssize_t route_test_sendto(int fd, const void *data, size_t len, int flags,
                         const struct sockaddr *destination, socklen_t address_len) {
    assert(fd==7 && flags==0 && address_len==sizeof(struct sockaddr_in));
    sends++; const struct sockaddr_in *a=(const struct sockaddr_in *)destination;
    observed_port=ntohs(a->sin_port); observed_ip=a->sin_addr.s_addr;
    assert(len<=sizeof(observed)); memcpy(observed,data,len); observed_len=len;
    if(error_mode) { errno=ENOMEM; return -1; } return (ssize_t)len;
}
int64_t esp_timer_get_time(void) { return test_now; }
static void fresh_tick(void) { test_now+=500000; }
int main(void) {
    uint8_t packet[48]={4,0,0x11,0xC5,60}; packet[14]=1;
    uint8_t raw[64]={1,0,0x11,0xC5,60};
    assert(stream_sender_init_with("192.168.1.3",55063)==0);
    assert(stream_sender_configure_radar(0,60)==0);
    assert(stream_sender_send_radar_fused(packet,48)==48 && observed_port==55063);
    int blocked_before=sends;
    assert(stream_sender_configure_radar(1023,60)==-1);
    assert(stream_sender_send_radar_fused(packet,48)==-1 && sends==blocked_before);
    assert(stream_sender_configure_radar(55063,60)==-1);
    assert(stream_sender_send_radar_fused(packet,48)==-1 && sends==blocked_before);
    assert(stream_sender_configure_radar(5005,60)==0);
    fresh_tick(); assert(stream_sender_send_radar_fused(packet,48)==48);
    assert(observed_port==5005 && observed_len==48 && memcmp(observed,packet,48)==0);
    struct in_addr expected; assert(inet_pton(AF_INET,"192.168.1.3",&expected)==1);
    assert(observed_ip==expected.s_addr); int before=sends;
    assert(stream_sender_send_radar_fused(packet,48)==-1 && sends==before); /* <=2Hz. */
    fresh_tick(); assert(stream_sender_send(raw,sizeof(raw))==(int)sizeof(raw) && observed_port==55063);
    assert(stream_sender_send_priority(raw,sizeof(raw))==(int)sizeof(raw) && observed_port==55063);
    before=sends;
    assert(stream_sender_send_radar_fused(raw,48)==-1 && sends==before);
    assert(stream_sender_send_radar_fused(packet,32)==-1 && sends==before);
    assert(stream_sender_send_radar_fused(NULL,48)==-1 && sends==before);
    packet[4]=21; assert(stream_sender_send_radar_fused(packet,48)==-1 && sends==before);packet[4]=60;
    packet[14]=2; assert(stream_sender_send_radar_fused(packet,48)==-1 && sends==before);packet[14]=1;
    float bad=NAN;memcpy(packet+28,&bad,4);assert(stream_sender_send_radar_fused(packet,48)==-1 && sends==before);
    bad=251;memcpy(packet+28,&bad,4);assert(stream_sender_send_radar_fused(packet,48)==-1 && sends==before);
    bad=0;memcpy(packet+28,&bad,4);packet[5]=8;assert(stream_sender_send_radar_fused(packet,48)==-1 && sends==before);packet[5]=0;
    error_mode=1;assert(stream_sender_send_radar_fused(packet,48)==-1 && sends==before+1);
    assert(observed_port==5005); /* Failed send does NOT fall back to bulk. */
    error_mode=0;assert(stream_sender_send_radar_fused(packet,48)==48 && observed_port==5005);
    fresh_tick(); assert(stream_sender_configure_radar(0,60)==0);
    assert(stream_sender_send_radar_fused(packet,48)==48 && observed_port==55063);
    stream_sender_deinit(); assert(stream_sender_send_radar_fused(packet,48)==-1);
    before=sends; assert(stream_sender_configure_radar(5005,60)==-1);
    assert(stream_sender_send_radar_fused(packet,48)==-1 && sends==before);
    puts("PASS: explicit MR60 split route, byte identity, legacy defaults, source identity, bounds, rate and no failure fallback");
}
