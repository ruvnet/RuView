/* Exercises the production parser/snapshot with a fake clock and host task stubs. */
#include <assert.h>
#include <stdio.h>
#include <stdint.h>
#define CONFIG_CSI_MOCK_ENABLED 1
typedef int portMUX_TYPE;
#define portMUX_INITIALIZER_UNLOCKED 0
#define portENTER_CRITICAL(lock) ((void)(lock))
#define portEXIT_CRITICAL(lock) ((void)(lock))
static int64_t test_now;
static inline void vTaskDelete(void *task) { (void)task; }
#include "../main/mmwave_sensor.c"
int64_t esp_timer_get_time(void) { return test_now; }
static void reset(void) {
    memset(&s_state, 0, sizeof(s_state)); memset(&s_field_times, 0, sizeof(s_field_times));
    memset(&s_mr60, 0, sizeof(s_mr60)); s_state.type = MMWAVE_TYPE_MR60BHA2;
    s_state.detected = true; test_now = 1000000;
}
static void frame(uint16_t type, const void *data, uint16_t length) {
    uint8_t h[] = {1,0,1,(uint8_t)(length>>8),(uint8_t)length,(uint8_t)(type>>8),(uint8_t)type,0};
    h[7] = mr60_calc_checksum(h, 7);
    for (unsigned i=0;i<sizeof(h);++i) mr60_feed_byte(h[i]);
    for (unsigned i=0;i<length;++i) mr60_feed_byte(((const uint8_t *)data)[i]);
    mr60_feed_byte(mr60_calc_checksum(data,length));
}
static void presence(bool v) { uint8_t p=v; frame(MR60_TYPE_PRESENCE,&p,1); }
static void distance(void) { uint8_t p[8]; uint32_t flag=1; float v=123;
    memcpy(p,&flag,4); memcpy(p+4,&v,4); frame(MR60_TYPE_DISTANCE,p,8); }
static void heart(float v) { frame(MR60_TYPE_HEARTRATE,&v,4); }
static void breathing(float v) { frame(MR60_TYPE_BREATHING,&v,4); }
static void complete(void) { presence(true); distance(); heart(72); breathing(16); }
int main(void) {
    mmwave_state_t out; reset(); assert(!mmwave_sensor_get_state(&out));
    test_now=0; presence(true); distance(); assert(!mmwave_sensor_get_state(&out));
    test_now=1000000;
    complete(); assert(mmwave_sensor_get_state(&out));
    assert(out.person_present && out.target_count==1 && out.heart_rate_bpm==72);
    uint8_t nonzero_presence=255; frame(MR60_TYPE_PRESENCE,&nonzero_presence,1);
    assert(mmwave_sensor_get_state(&out) && out.person_present); /* Upstream bool conversion. */
    test_now+=MMWAVE_DATA_MAX_AGE_US; assert(mmwave_sensor_get_state(&out));
    ++test_now; assert(!mmwave_sensor_get_state(&out) && !out.detected && s_state.detected);
    presence(false); assert(mmwave_sensor_get_state(&out) && !out.person_present);
    assert(out.distance_cm==0 && out.heart_rate_bpm==0 && out.breathing_rate==0);
    heart(80); assert(mmwave_sensor_get_state(&out) && out.heart_rate_bpm==0);
    reset(); complete(); test_now+=MMWAVE_DATA_MAX_AGE_US+1;
    heart(80); assert(!mmwave_sensor_get_state(&out)); /* HR cannot renew presence. */
    presence(true); assert(!mmwave_sensor_get_state(&out)); /* Or expired range. */
    distance(); assert(mmwave_sensor_get_state(&out) && out.heart_rate_bpm==80 && out.breathing_rate==0);
    reset(); complete(); test_now+=MMWAVE_DATA_MAX_AGE_US+1;
    presence(true); distance(); assert(mmwave_sensor_get_state(&out));
    assert(out.heart_rate_bpm==0 && out.breathing_rate==0);
    reset(); complete(); int64_t accepted=s_state.last_update_us;
    test_now+=1000; uint8_t bytes[8]={0};
    frame(MR60_TYPE_PHASE,bytes,4); assert(s_state.last_update_us==accepted);
    heart(NAN); assert(s_state.last_update_us==accepted && s_state.heart_rate_bpm==0);
    breathing(INFINITY); assert(s_state.last_update_us==accepted);
    frame(MR60_TYPE_HEARTRATE,bytes,1); assert(s_state.last_update_us==accepted);
    frame(MR60_TYPE_DISTANCE,bytes,8); assert(!mmwave_sensor_get_state(&out));
    distance(); assert(mmwave_sensor_get_state(&out));
    test_now=1; assert(!mmwave_sensor_get_state(&out)); assert(!mmwave_sensor_get_state(NULL));
    reset(); uint8_t ld[12]={2,0xAA,1,100,0,0,0,0,0,0,0,0};
    ld2410_process_frame(ld,sizeof(ld)); assert(mmwave_sensor_get_state(&out) && out.distance_cm==100);
    accepted=s_state.last_update_us; test_now+=MMWAVE_DATA_MAX_AGE_US+1;
    ld[0]=99; ld2410_process_frame(ld,sizeof(ld));
    assert(s_state.last_update_us==accepted && !mmwave_sensor_get_state(&out));
    puts("PASS: UART silence, independent field expiry, invalid frames, range withholding, absence, recovery, clock boundaries");
}
