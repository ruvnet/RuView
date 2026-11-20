/* Production NVS loader with only key reads controlled by the host. */
#include <assert.h>
#include <stdio.h>
#include <string.h>
/* The device libc supplies strlcpy; keep the host probe portable to libcs
 * which do not expose that BSD helper under strict C99. */
static size_t radar_test_strlcpy(char *dst, const char *src, size_t size) {
    size_t len=strlen(src);
    if(size) { size_t count=len<size-1?len:size-1;memcpy(dst,src,count);dst[count]='\0'; }
    return len;
}
#ifdef strlcpy
#undef strlcpy
#endif
#define strlcpy radar_test_strlcpy
#include "esp_stubs.h"
esp_err_t radar_nvs_open(const char *, int, nvs_handle_t *);
esp_err_t radar_nvs_get_u16(nvs_handle_t, const char *, uint16_t *);
#define nvs_open radar_nvs_open
#define nvs_get_u16 radar_nvs_get_u16
#include "../main/nvs_config.c"
static int radar_result=ESP_ERR_NVS_NOT_FOUND, open_result=ESP_OK;
static uint16_t radar_value;
esp_err_t radar_nvs_open(const char *name, int mode, nvs_handle_t *handle) {
    assert(strcmp(name,"csi_cfg")==0 && mode==NVS_READONLY);*handle=1;return open_result;
}
esp_err_t radar_nvs_get_u16(nvs_handle_t handle,const char *key,uint16_t *out) {
    assert(handle==1);
    if(strcmp(key,"target_port")==0){*out=55063;return ESP_OK;}
    if(strcmp(key,"radar_port")==0){if(radar_result==ESP_OK)*out=radar_value;return radar_result;}
    return ESP_ERR_NVS_NOT_FOUND;
}
int main(void) {
    nvs_config_t cfg;memset(&cfg,0xA5,sizeof(cfg));nvs_config_load(&cfg);
    assert(cfg.radar_fused_port==0 && cfg.target_port==55063);
    radar_result=ESP_OK;radar_value=5005;nvs_config_load(&cfg);assert(cfg.radar_fused_port==5005 && cfg.target_port==55063);
    radar_value=0;nvs_config_load(&cfg);assert(cfg.radar_fused_port==0);
    radar_value=1023;nvs_config_load(&cfg);assert(cfg.radar_fused_port==1023); /* Preserved invalid intent. */
    radar_value=55063;nvs_config_load(&cfg);assert(cfg.radar_fused_port==55063);
    radar_result=ESP_FAIL;nvs_config_load(&cfg);assert(cfg.radar_fused_port==1); /* Wrong type/read error blocks. */
    open_result=ESP_ERR_NVS_NOT_FOUND;nvs_config_load(&cfg);assert(cfg.radar_fused_port==0);
    open_result=ESP_FAIL;nvs_config_load(&cfg);assert(cfg.radar_fused_port==1);
    puts("PASS: production NVS defaults, explicit split, preserved invalid/wrong-type intent and absent namespace");
}
