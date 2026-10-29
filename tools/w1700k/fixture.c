/* SPDX-License-Identifier: MIT */
/* SYNTHETIC ABI fixture generator, not hardware evidence. */
#define _POSIX_C_SOURCE 200809L
#include "vendor/w1700k_stream.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(int argc,char **argv) {
 if(argc!=3) return 2;
 FILE *f=fopen(argv[1],"wb"); if(!f) return 2;
 struct w1700k_stream_header h={0};
 h.magic=W1700K_STREAM_MAGIC; h.version=1; h.header_bytes=4096; h.slice_samples=2048;
 h.ring_slices=8; h.tags_offset=4096; h.samples_offset=4160; h.total_bytes=69700-4;
 h.run=1; h.newest=4; h.radio_sample_rate=40000000; h.heartbeat_ns=w1700k_stream_now_ns();
 const char *kind=argv[2];
 if(!strcmp(kind,"magic")) h.magic=0;
 if(!strcmp(kind,"ring")) h.ring_slices=3;
 if(!strcmp(kind,"offset")) h.samples_offset=8;
 if(!strcmp(kind,"overflow")) h.tags_offset=UINT64_MAX-7;
 if(!strcmp(kind,"seqlock")) h.position_seq=1;
 if(!strcmp(kind,"stale")) h.heartbeat_ns=1;
 if(!strcmp(kind,"idle")) h.newest=0;
 if(!strcmp(kind,"run_overflow")) h.run=1ULL<<21;
 char *data=calloc(1,69696); if(!data) return 2;
 memcpy(data,&h,sizeof(h));
 uint64_t *tags=(uint64_t*)(data+4096);
 for(int i=0;i<4;i++) {
   tags[i]=w1700k_stream_tag(1,i);
   int16_t *iq=(int16_t*)(data+4160+i*8192);
   for(int j=0;j<2048;j++) { iq[j*2]=4096; iq[j*2+1]=0; }
 }
 if(!strcmp(kind,"gap")) tags[1]|=W1700K_STREAM_TAG_LOST;
 if(!strcmp(kind,"event")) tags[1]|=W1700K_STREAM_TAG_EVENT;
 if(!strcmp(kind,"badkind")) tags[1]|=W1700K_STREAM_TAG_EVENT|W1700K_STREAM_TAG_LOST;
 if(!strcmp(kind,"tag")) tags[1]=0;
 if(!strcmp(kind,"flat")) {
   tags[1]|=W1700K_STREAM_TAG_FLAT;
   int16_t *iq=(int16_t*)(data+4160+8192);
   for(int j=0;j<4096;j++) iq[j]=16384;
 }
 size_t size=!strcmp(kind,"truncated")?4096:69696;
 int ok=fwrite(data,1,size,f)==size; free(data); fclose(f); return ok?0:2;
}
