/* SYNTHETIC: run the real HTTP handler and boot recovery against write-through
 * NVS fault injection. No device, credentials or RF data are used. */
#include "config_test_sdk.h"
#include "cJSON.h"
#include <assert.h>
#include "../main/config_api.c"

nvs_config_t g_nvs_config;
static cJSON *db[3];
static int live_sets, fail_set_at, restarts;
static bool fail_restore, fail_erase, fail_bank_clear, fail_read, fail_restore_open;
static bool fail_timer, fail_commit;

static void put(nvs_handle_t h, const char *key, cJSON *value)
{
    cJSON_DeleteItemFromObjectCaseSensitive(db[h], key);
    assert(cJSON_AddItemToObject(db[h], key, value));
}
esp_err_t nvs_open(const char *name, int mode, nvs_handle_t *h)
{
    *h = strcmp(name, "csi_cfg") == 0 ? 1 : 2;
    if (*h == 1 && mode == NVS_READWRITE && fail_restore_open && live_sets >= 2) return ESP_FAIL;
    return ESP_OK;
}
void nvs_close(nvs_handle_t h) { (void)h; }
esp_err_t nvs_commit(nvs_handle_t h) { return h == 1 && fail_commit ? ESP_FAIL : ESP_OK; }
esp_err_t nvs_get_str(nvs_handle_t h, const char *key, char *value, size_t *size)
{
    if (h == 1 && fail_read) return ESP_FAIL;
    cJSON *v = cJSON_GetObjectItemCaseSensitive(db[h], key);
    if (!cJSON_IsString(v)) return ESP_ERR_NVS_NOT_FOUND;
    size_t n = strlen(v->valuestring) + 1;
    if (value) { if (*size < n) return ESP_FAIL; memcpy(value, v->valuestring, n); }
    *size = n; return ESP_OK;
}
static esp_err_t get_num(nvs_handle_t h, const char *key, uint32_t *value)
{
    cJSON *v = cJSON_GetObjectItemCaseSensitive(db[h], key);
    if (!cJSON_IsNumber(v)) return ESP_ERR_NVS_NOT_FOUND;
    *value = (uint32_t)v->valuedouble; return ESP_OK;
}
esp_err_t nvs_get_u8(nvs_handle_t h, const char *k, uint8_t *v) { uint32_t n=0; int e=get_num(h,k,&n); *v=(uint8_t)n; return e; }
esp_err_t nvs_get_u16(nvs_handle_t h, const char *k, uint16_t *v) { uint32_t n=0; int e=get_num(h,k,&n); *v=(uint16_t)n; return e; }
esp_err_t nvs_get_u32(nvs_handle_t h, const char *k, uint32_t *v) { return get_num(h,k,v); }
static esp_err_t set_value(nvs_handle_t h, const char *k, cJSON *v)
{
    if (h == 1) {
        live_sets++;
        if ((fail_set_at && live_sets == fail_set_at) ||
            (fail_restore && live_sets > fail_set_at)) { cJSON_Delete(v); return ESP_FAIL; }
    }
    put(h,k,v); return ESP_OK; /* setters immediately change persistent state */
}
esp_err_t nvs_set_str(nvs_handle_t h, const char *k, const char *v) { return set_value(h,k,cJSON_CreateString(v)); }
esp_err_t nvs_set_u8(nvs_handle_t h, const char *k, uint8_t v) { return set_value(h,k,cJSON_CreateNumber(v)); }
esp_err_t nvs_set_u16(nvs_handle_t h, const char *k, uint16_t v) { return set_value(h,k,cJSON_CreateNumber(v)); }
esp_err_t nvs_set_u32(nvs_handle_t h, const char *k, uint32_t v) { return set_value(h,k,cJSON_CreateNumber(v)); }
esp_err_t nvs_erase_key(nvs_handle_t h, const char *k)
{
    if (fail_erase) return ESP_FAIL;
    if (!cJSON_GetObjectItemCaseSensitive(db[h],k)) return ESP_ERR_NVS_NOT_FOUND;
    cJSON_DeleteItemFromObjectCaseSensitive(db[h],k); return ESP_OK;
}
esp_err_t nvs_erase_all(nvs_handle_t h)
{
    if (h == 2 && fail_bank_clear) return ESP_FAIL;
    cJSON_Delete(db[h]); db[h]=cJSON_CreateObject(); return ESP_OK;
}
esp_err_t esp_timer_create(const esp_timer_create_args_t *a, esp_timer_handle_t *h) { (void)a; *h=(void *)1; return fail_timer ? ESP_FAIL : ESP_OK; }
esp_err_t esp_timer_start_once(esp_timer_handle_t h, uint64_t us) { (void)h; (void)us; return fail_timer ? ESP_FAIL : ESP_OK; }
void esp_restart(void) { restarts++; }
bool ota_auth_check(httpd_req_t *r) { (void)r; return true; }
void ota_update_log_httpd_stack(const char *s) { (void)s; }

static void reset(void)
{
    for (int i=1;i<=2;i++) { cJSON_Delete(db[i]); db[i]=cJSON_CreateObject(); }
    put(1,"ssid",cJSON_CreateString("old-network"));
    put(1,"edge_tier",cJSON_CreateNumber(0));
    live_sets=fail_set_at=restarts=0;
    fail_restore=fail_erase=fail_bank_clear=fail_read=fail_restore_open=fail_timer=fail_commit=false;
    s_pending_reboot=s_trial_armed=false; s_revert_timer=NULL;
}
static int request(const char *body)
{
    httpd_req_t req={.body=body,.content_len=(int)strlen(body)};
    config_post_handler(&req); return req.status;
}
static bool bank_present(void) { return cJSON_IsString(cJSON_GetObjectItem(db[2],"bank")); }
static void assert_old(void) { assert(strcmp(cJSON_GetObjectItem(db[1],"ssid")->valuestring,"old-network")==0); }

int main(void)
{
    const char *change="{\"wifi_ssid\":\"new-network\",\"edge_tier\":1}";
    reset();
    assert(request("{\"wifi_ssid\":\"new-network\",\"edge_tier\":\"invalid\"}")==400);
    assert_old(); assert(live_sets==0 && !bank_present());
    reset(); fail_read=true;
    assert(request(change)==500); assert(live_sets==0); assert_old();
    reset();
    assert(request("{\"led_mode\":1,\"reboot\":false}")==200);
    assert(!s_pending_reboot && !bank_present() && g_nvs_config.led_mode==1);
    reset(); fail_set_at=2;
    assert(request(change)==500); assert_old();
    assert(!s_pending_reboot && !bank_present());
    for (int mode=0;mode<3;mode++) {
        reset(); fail_set_at=2;
        fail_restore=mode==0; fail_restore_open=mode==1; fail_bank_clear=mode==2;
        assert(request(change)==500);
        assert(bank_present() && s_pending_reboot);
        int previous=live_sets;
        assert(request("{\"led_mode\":1}")==409 && live_sets==previous);
        config_trial_notify_connected(); assert(bank_present());
        /* A reset must restore before association can promote mixed values. */
        s_pending_reboot=s_trial_armed=false;
        fail_restore=fail_restore_open=fail_bank_clear=false; fail_set_at=0;
        config_trial_boot_check();
        assert_old(); assert(restarts==1 && !bank_present());
    }
    reset(); fail_set_at=2; fail_restore=true;
    assert(request(change)==500);
    trial_revert(NULL); assert(bank_present() && restarts==0 && s_pending_reboot);
    reset(); fail_erase=true;
    cJSON *absent=cJSON_CreateNull();
    assert(!restore_key(1,config_find_key("led_mode"),absent)); cJSON_Delete(absent);
    reset(); fail_timer=true;
    assert(request(change)==200); assert(bank_present() && s_pending_reboot);
    assert(request("{\"led_mode\":1}")==409);
    reset();
    assert(request(change)==200); assert(bank_present() && s_pending_reboot);
    config_trial_notify_connected(); assert(bank_present()); /* old connection */
    s_pending_reboot=s_trial_armed=false;
    config_trial_boot_check(); assert(s_trial_armed && bank_present());
    config_trial_notify_connected(); assert(!s_trial_armed && !bank_present());
    reset(); fail_commit=true;
    assert(request(change)==500); assert(bank_present() && s_pending_reboot);
    for (int i=1;i<=2;i++) cJSON_Delete(db[i]);
    puts("PASS: production HTTP/NVS validation, rollback faults, write lock and boot recovery");
    return 0;
}
