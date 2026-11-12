/* Execute production serial handlers with synthetic clock/identity/NVS ports. */
#include <assert.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include "serial_test_platform.h"
#define RUVIEW_ONBOARDING_IDENTITY_H
#define RUVIEW_ONBOARDING_CONSOLE_H
static int captured_printf(const char *format, ...);
#define printf captured_printf
#include "../main/serial_onboarding.c"
#undef printf

static char output[4096];
static int writes, restarts, open_error, set_error, commit_error, read_error, mismatch, raw_writes, node_writes;
static int64_t clock_us;
static nvs_config_t saved;
static const char *device = "0123456789abcdef";
static const char *claim = "00112233445566778899aabbccddeeff";
static int captured_printf(const char *format, ...) {
    va_list args; va_start(args, format);
    const size_t used = strlen(output);
    const int size = vsnprintf(output + used, sizeof(output) - used, format, args);
    va_end(args); return size;
}
const esp_app_desc_t *esp_app_get_description(void) { static const esp_app_desc_t app = {"synthetic-mr60"}; return &app; }
void ruview_onboarding_device_digest(char out[17]) { memcpy(out, device, 17); }
esp_err_t ruview_onboarding_console_prepare(void) { return ESP_OK; }
int64_t esp_timer_get_time(void) { return clock_us; }
const char *esp_err_to_name(esp_err_t value) { (void)value; return "synthetic_error"; }
void esp_restart(void) { restarts++; }
void vTaskDelay(uint32_t ticks) { (void)ticks; }
BaseType_t xTaskCreate(void (*task)(void *), const char *name, unsigned stack, void *arg, unsigned priority, void *handle) {
    (void)task;(void)name;(void)stack;(void)arg;(void)priority;(void)handle;return pdPASS;
}
esp_err_t nvs_open(const char *name, int mode, nvs_handle_t *handle) { assert(!strcmp(name,"csi_cfg"));(void)mode;*handle=1;return open_error?ESP_FAIL:ESP_OK; }
void nvs_close(nvs_handle_t handle) { assert(handle==1); }
esp_err_t nvs_set_str(nvs_handle_t h,const char *key,const char *value) {
    assert(h==1);writes++;if(set_error)return ESP_FAIL;
    if(!strcmp(key,"target_ip"))snprintf(saved.target_ip,sizeof(saved.target_ip),"%s",value);
    else if(!strcmp(key,"ssid"))snprintf(saved.wifi_ssid,sizeof(saved.wifi_ssid),"%s",value);
    else if(!strcmp(key,"password"))snprintf(saved.wifi_password,sizeof(saved.wifi_password),"%s",value);
    else assert(!"unexpected NVS key");return ESP_OK;
}
esp_err_t nvs_set_u16(nvs_handle_t h,const char *key,uint16_t value) {
    assert(h==1);writes++;if(set_error)return ESP_FAIL;
    if(!strcmp(key,"radar_port"))saved.radar_fused_port=value;
    else if(!strcmp(key,"target_port")){raw_writes++;saved.target_port=value;}
    else assert(!"unexpected NVS key");return ESP_OK;
}
esp_err_t nvs_set_u8(nvs_handle_t h,const char *key,uint8_t value) { assert(h==1&&!strcmp(key,"node_id"));node_writes++;saved.node_id=value;return ESP_OK; }
esp_err_t nvs_commit(nvs_handle_t handle) { assert(handle==1);return commit_error?ESP_FAIL:ESP_OK; }
esp_err_t nvs_get_str(nvs_handle_t h,const char *key,char *value,size_t *length) {
    assert(h==1);if(read_error)return ESP_FAIL;
    const char *source=!strcmp(key,"target_ip")?saved.target_ip:!strcmp(key,"ssid")?saved.wifi_ssid:saved.wifi_password;
    const size_t size=strlen(source)+1;if(size>*length)return ESP_FAIL;memcpy(value,source,size);*length=size;return ESP_OK;
}
esp_err_t nvs_get_u16(nvs_handle_t h,const char *key,uint16_t *value) { assert(h==1);if(read_error)return ESP_FAIL;*value=!strcmp(key,"radar_port")?(uint16_t)(saved.radar_fused_port+(mismatch?1:0)):saved.target_port;return ESP_OK; }
esp_err_t nvs_get_u8(nvs_handle_t h,const char *key,uint8_t *value) { assert(h==1&&!strcmp(key,"node_id"));*value=saved.node_id;return ESP_OK; }
static void reset(void) {
    memset(&saved,0,sizeof(saved));saved.node_id=60;saved.target_port=55063;saved.radar_fused_port=5005;
    strcpy(saved.target_ip,"192.168.1.3");strcpy(saved.wifi_ssid,"Synthetic WiFi");strcpy(saved.wifi_password,"synthetic-secret");
    s_current_config=saved;clock_us=100;writes=restarts=open_error=set_error=commit_error=read_error=mismatch=raw_writes=node_writes=0;
    strcpy(s_issued_nonce,claim);s_nonce_issued_at_us=100;output[0]='\0';device="0123456789abcdef";
}
static void send(const char *identity,unsigned node,unsigned port,const char *network,const char *secret) {
    char command[384];snprintf(command,sizeof(command),"RUVIEW_RADAR_CONFIG_V1 %s %s %u 192.168.1.9 %u %s %s",claim,identity,node,port,network,secret);process_line(command);
}
int main(void) {
    reset();emit_hello(claim);
#if defined(CONFIG_C6_XIAO_MR60BHA2) && CONFIG_C6_XIAO_MR60BHA2
    assert(strstr(output,"RUVIEW_RADAR_CAP_V1")&&strstr(output,"device_id=0123456789abcdef"));
    output[0]='\0';send(device,60,5006,"KEEP","KEEP");
    assert(restarts==1&&writes==2&&raw_writes==0&&node_writes==0&&saved.target_port==55063&&saved.node_id==60);
    assert(!strcmp(saved.wifi_password,"synthetic-secret")&&!strcmp(saved.wifi_ssid,"Synthetic WiFi"));
    assert(strstr(output,"radar_port=5006 raw_port=55063 rebooting=1")&&!strstr(output,"synthetic-secret"));
    send(device,60,5006,"KEEP","KEEP");assert(restarts==1&&writes==2); /* consumed nonce */
    reset();send(device,60,5005,"U3ludGhldGlj","c3ludGhldGljLXBhc3M=");
    assert(restarts==1&&!strcmp(saved.wifi_ssid,"Synthetic")&&!strcmp(saved.wifi_password,"synthetic-pass"));
    assert(!strstr(output,"synthetic-pass")&&!strstr(output,"c3ludGhldGlj"));
    for(int mode=0;mode<6;mode++) {
        reset();if(mode==0)clock_us+=60000001;if(mode==1)clock_us=99;if(mode==2)s_issued_nonce[0]='\0';
        send(mode==3?"abcdef0123456789":device,mode==4?61:60,mode==5?55063:5005,"KEEP","KEEP");
        assert(writes==0&&restarts==0&&!strstr(output,"CONFIG_OK"));
    }
    for(int mode=0;mode<5;mode++) {
        reset();if(mode==0)open_error=1;if(mode==1)set_error=1;if(mode==2)commit_error=1;if(mode==3)read_error=1;if(mode==4)mismatch=1;
        send(device,60,5005,"KEEP","KEEP");assert(restarts==0&&!strstr(output,"CONFIG_OK"));
    }
    reset();device="0000000000000000";emit_hello(claim);assert(!strstr(output,"RADAR_CAP"));send(device,60,5005,"KEEP","KEEP");assert(writes==0);
    reset();char generic[384];snprintf(generic,sizeof(generic),"RUVIEW_CONFIG_V1 %s 60 192.168.1.9 5005 KEEP KEEP",claim);
    process_line(generic);assert(writes==0&&strstr(output,"radar_requires_dedicated_config"));
#else
    assert(!strstr(output,"RUVIEW_RADAR_CAP_V1"));send(device,60,5005,"KEEP","KEEP");
    assert(writes==0&&restarts==0&&strstr(output,"unsupported_controller"));
#endif
    puts("PASS: production radar serial handler, identity/nonce, NVS readback/preservation and refusal controls (synthetic host ports)");
    return 0;
}
