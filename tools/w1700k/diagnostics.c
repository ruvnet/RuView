/* SPDX-License-Identifier: MIT
 * RuView advisory W1700K diagnostics; no CSI or presence inference.
 * Local trusted producer only. pread avoids SIGBUS on concurrent truncation.
 */
#define _POSIX_C_SOURCE 200809L
#include "vendor/w1700k_stream.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <limits.h>
#include <inttypes.h>
#include <stdatomic.h>
#include <signal.h>
#define MAX_RING 1048576ULL
#define MAX_RECORDS 4096
static volatile sig_atomic_t stop;
static void stopped(int sig) { (void)sig; stop=1; }
static void die(const char *s) { fprintf(stderr,"w1700k-diagnostics: %s\n",s); exit(2); }
static void rd(int fd, void *p, size_t n, uint64_t off) {
    size_t got=0;
    while(got<n) {
        ssize_t k=pread(fd,(char*)p+got,n-got,(off_t)(off+got));
        if(k<0 && errno==EINTR) continue;
        if(k<=0) die("short read or input changed");
        got+=(size_t)k;
    }
    atomic_thread_fence(memory_order_acquire);
}
static uint64_t u64(int fd, uint64_t off) { uint64_t x; rd(fd,&x,8,off); return x; }
static uint64_t number(const char *s, uint64_t max) {
    if(!s[0]) die("empty numeric argument");
    for(const char *p=s;*p;p++) if(*p<'0'||*p>'9') die("invalid numeric argument");
    errno=0; char *end; unsigned long long n=strtoull(s,&end,10);
    if(errno || *end || n>max) die("numeric argument out of range");
    return n;
}
static void header(int fd, struct w1700k_stream_header *h) {
    struct stat st;
    if(fstat(fd,&st) || !S_ISREG(st.st_mode)) die("input must be regular file or POSIX SHM");
    rd(fd,h,sizeof(*h),0);
    if(h->magic!=W1700K_STREAM_MAGIC || h->version!=1 ||
       h->header_bytes!=4096 || h->slice_samples!=2048) die("unsupported ABI header");
    uint64_t r=h->ring_slices;
    if(!r || r>MAX_RING || (r&(r-1))) die("invalid ring size");
    if(h->tags_offset<4096 || h->tags_offset%8 ||
       h->tags_offset>UINT64_MAX-r*8 ||
       h->samples_offset<h->tags_offset+r*8 || h->samples_offset%8 ||
       h->samples_offset>UINT64_MAX-r*8192 ||
       h->total_bytes!=h->samples_offset+r*8192 ||
       h->total_bytes>(uint64_t)INT64_MAX || st.st_size<0 ||
       h->total_bytes!=(uint64_t)st.st_size) die("invalid layout or truncated file");
}
static void position(int fd,uint64_t *run,uint64_t *newest) {
    for(int i=0;i<64;i++) {
        uint64_t s=u64(fd,offsetof(struct w1700k_stream_header,position_seq));
        *run=u64(fd,offsetof(struct w1700k_stream_header,run));
        *newest=u64(fd,offsetof(struct w1700k_stream_header,newest));
        uint64_t t=u64(fd,offsetof(struct w1700k_stream_header,position_seq));
        if(!(s&1) && s==t) {
            if((!*run && *newest) || *run>=(1ULL<<21) || *newest>=(1ULL<<40)) die("tag identity overflow");
            return;
        }
    }
    die("position seqlock did not stabilize");
}
static int stale(int fd) {
    uint64_t hb=u64(fd,offsetof(struct w1700k_stream_header,heartbeat_ns));
    uint64_t now=w1700k_stream_now_ns();
    return hb>now || now-hb>=1000000000ULL;
}
static void emit(const char *source,uint64_t run,uint64_t index,uint64_t rate,
                 uint64_t center,const char *reason,uint64_t count,double power,
                 double rail,uint64_t missing,uint64_t events,uint64_t overrun) {
    printf("{\"schema\":\"ruview.rf_diagnostics.v1\",\"source_kind\":\"%s\","
           "\"source_id\":\"w1700k\",\"observed_monotonic_ns\":%" PRIu64 ","
           "\"run\":%" PRIu64 ",\"slot_index\":%" PRIu64 ",\"sample_start\":null,"
           "\"sample_rate_hz\":%" PRIu64 ",\"center_frequency_hz\":",
           source,w1700k_stream_now_ns(),run,index,rate);
    if(center) printf("%" PRIu64,center); else printf("null");
    printf(",\"config_verified\":false,\"valid\":false,"
           "\"invalid_reasons\":[\"configuration_unverified\"");
    if(reason) printf(",\"%s\"",reason);
    printf("],\"sample_count\":%" PRIu64 ",\"rms_dbfs\":",count);
    if(count && power>0) printf("%.6f",10*log10(power)); else printf("null");
    printf(",\"rail_fraction\":");
    if(count) printf("%.8f",rail); else printf("null");
    printf(",\"missing_samples\":%" PRIu64 ",\"event_count\":%" PRIu64 ","
           "\"overrun_slices\":%" PRIu64 ",\"ttl_ms\":1000}\n",missing,events,overrun);
    if(fflush(stdout) || ferror(stdout)) die("output failed");
}
int main(int argc,char **argv) {
    const char *path="/dev/shm/w1700k-stream",*source="live";
    uint64_t center=0,limit=64,timeout=10; int follow=0;
    for(int i=1;i<argc;i++) {
        if(!strcmp(argv[i],"--once")) { follow=0; continue; }
        if(!strcmp(argv[i],"--follow")) { follow=1; continue; }
        if(!strcmp(argv[i],"--help")) {
            puts("w1700k-diagnostics [--input PATH] [--source-kind live|synthetic|captured_replay] [--once|--follow] [--max-records 1..4096] [--timeout-seconds 1..3600] [--center-frequency-hz HZ]\nRead-only advisory diagnostics. Configuration is always unverified. Source kind is caller asserted. No CSI, presence or spectrum bins.");
            return 0;
        }
        if(i+1>=argc) die("missing option value");
        const char *key=argv[i],*value=argv[++i];
        if(!strcmp(key,"--input")) path=value;
        else if(!strcmp(key,"--source-kind")) source=value;
        else if(!strcmp(key,"--center-frequency-hz")) center=number(value,100000000000ULL);
        else if(!strcmp(key,"--max-records")) limit=number(value,MAX_RECORDS);
        else if(!strcmp(key,"--timeout-seconds")) timeout=number(value,3600);
        else die("unknown option");
    }
    if(strcmp(source,"live") && strcmp(source,"synthetic") && strcmp(source,"captured_replay"))
        die("invalid source kind");
    if(!limit || !timeout) die("zero limit");
    if(center && (center<2000000000ULL || center>8000000000ULL)) die("center frequency outside RF bands");
    int fd=open(path,O_RDONLY|O_CLOEXEC|O_NOFOLLOW|O_NONBLOCK);
    if(fd<0) die("cannot open input");
    struct w1700k_stream_header h; header(fd,&h);
    uint64_t run=0,next=0,newest=0,emitted=0;
    position(fd,&run,&newest);
    uint64_t backlog=newest<h.ring_slices?newest:h.ring_slices;
    if(backlog>limit) backlog=limit;
    next=newest-backlog;
    uint64_t deadline=w1700k_stream_now_ns()+timeout*1000000000ULL;
    signal(SIGINT,stopped); signal(SIGTERM,stopped);
    while(!stop && emitted<limit && w1700k_stream_now_ns()<deadline) {
        struct w1700k_stream_header check; header(fd,&check);
        if(memcmp(&h,&check,offsetof(struct w1700k_stream_header,service_pid)))
            die("layout changed");
        uint64_t current; position(fd,&current,&newest);
        uint64_t rate=u64(fd,offsetof(struct w1700k_stream_header,radio_sample_rate));
        if(rate && rate!=40000000 && rate!=80000000 && rate!=160000000 && rate!=320000000) die("unsupported sample rate");
        if(!strcmp(source,"live") && stale(fd)) {
            emit(source,current,next,rate,center,"stale",0,0,0,0,0,0); close(fd); return 3;
        }
        if(current!=run) {
            emit(source,current,0,rate,center,"run_change",0,0,0,0,0,0);
            emitted++; run=current; next=newest; continue;
        }
        if(newest<next) die("frontier moved backward");
        if(newest-next>h.ring_slices) {
            uint64_t lost=newest-next-h.ring_slices;
            emit(source,run,next,rate,center,"overrun",0,0,0,0,0,lost);
            emitted++; next=newest-h.ring_slices; continue;
        }
        if(next==newest) {
            if(!follow) break;
            struct timespec pause={0,10000000}; nanosleep(&pause,NULL); continue;
        }
        uint64_t slot=next&(h.ring_slices-1),tag=u64(fd,h.tags_offset+slot*8);
        uint64_t expected=w1700k_stream_tag(run,next),count=0,missing=0,events=0;
        double power=0,rail=0; const char *reason=NULL;
        if((tag&~W1700K_STREAM_TAG_KIND)!=expected) reason="tag_mismatch";
        else if((tag&W1700K_STREAM_TAG_KIND) && ((tag&W1700K_STREAM_TAG_KIND)&((tag&W1700K_STREAM_TAG_KIND)-1))) reason="tag_mismatch";
        else if(tag&W1700K_STREAM_TAG_EVENT) { reason="event"; events=1; }
        else if(tag&W1700K_STREAM_TAG_LOST) { reason="lost"; missing=2048; }
        else {
            int16_t iq[4096]; rd(fd,iq,sizeof(iq),h.samples_offset+slot*8192);
            uint64_t after=u64(fd,h.tags_offset+slot*8);
            if(tag!=after) reason="tag_mismatch";
            else {
                count=2048;
                for(int j=0;j<4096;j++) {
                    double x=(double)iq[j]/8192.0; power+=x*x;
                    if(iq[j]==INT16_MIN || iq[j]==INT16_MAX) rail++;
                }
                power/=2048; rail/=4096;
            }
        }
        if(tag!=u64(fd,h.tags_offset+slot*8)) {
            reason="tag_mismatch"; count=0; missing=0; events=0;
        }
        emit(source,run,next,rate,center,reason,count,power,rail,missing,events,0);
        next++; emitted++;
    }
    close(fd); return 0;
}
