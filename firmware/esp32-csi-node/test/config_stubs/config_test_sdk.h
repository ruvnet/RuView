#pragma once
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
typedef int esp_err_t;
#define ESP_OK 0
#define ESP_FAIL -1
#define ESP_ERR_NO_MEM 0x101
#define ESP_ERR_NVS_NOT_FOUND 0x1102
#define ESP_LOGE(...) ((void)0)
#define ESP_LOGW(...) ((void)0)
#define ESP_LOGI(...) ((void)0)
typedef int nvs_handle_t;
#define NVS_READONLY 0
#define NVS_READWRITE 1
esp_err_t nvs_open(const char *, int, nvs_handle_t *);
void nvs_close(nvs_handle_t);
esp_err_t nvs_commit(nvs_handle_t);
esp_err_t nvs_get_str(nvs_handle_t, const char *, char *, size_t *);
esp_err_t nvs_get_u8(nvs_handle_t, const char *, uint8_t *);
esp_err_t nvs_get_u16(nvs_handle_t, const char *, uint16_t *);
esp_err_t nvs_get_u32(nvs_handle_t, const char *, uint32_t *);
esp_err_t nvs_set_str(nvs_handle_t, const char *, const char *);
esp_err_t nvs_set_u8(nvs_handle_t, const char *, uint8_t);
esp_err_t nvs_set_u16(nvs_handle_t, const char *, uint16_t);
esp_err_t nvs_set_u32(nvs_handle_t, const char *, uint32_t);
esp_err_t nvs_erase_key(nvs_handle_t, const char *);
esp_err_t nvs_erase_all(nvs_handle_t);
typedef struct { const char *body; int content_len, offset, status; } httpd_req_t;
typedef void *httpd_handle_t;
typedef struct { const char *uri; int method; esp_err_t (*handler)(httpd_req_t *); } httpd_uri_t;
#define HTTP_GET 0
#define HTTP_POST 1
#define HTTPD_400_BAD_REQUEST 400
#define HTTPD_403_FORBIDDEN 403
#define HTTPD_500_INTERNAL_SERVER_ERROR 500
static inline int httpd_req_recv(httpd_req_t *r, char *b, size_t n) {
    if (n > (size_t)(r->content_len - r->offset)) n = r->content_len - r->offset;
    memcpy(b, r->body + r->offset, n); r->offset += n; return (int)n;
}
static inline esp_err_t httpd_resp_send_err(httpd_req_t *r, int s, const char *m) { (void)m; r->status = s; return ESP_OK; }
static inline esp_err_t httpd_resp_sendstr(httpd_req_t *r, const char *s) { (void)s; if (!r->status) r->status = 200; return ESP_OK; }
static inline esp_err_t httpd_resp_set_type(httpd_req_t *r, const char *s) { (void)r; (void)s; return ESP_OK; }
static inline esp_err_t httpd_resp_set_status(httpd_req_t *r, const char *s) { r->status = atoi(s); return ESP_OK; }
static inline esp_err_t httpd_register_uri_handler(httpd_handle_t h, const httpd_uri_t *u) { (void)h; (void)u; return ESP_OK; }
typedef void *esp_timer_handle_t;
typedef struct { void (*callback)(void *); const char *name; } esp_timer_create_args_t;
esp_err_t esp_timer_create(const esp_timer_create_args_t *, esp_timer_handle_t *);
esp_err_t esp_timer_start_once(esp_timer_handle_t, uint64_t);
static inline esp_err_t esp_timer_stop(esp_timer_handle_t h) { (void)h; return ESP_OK; }
static inline esp_err_t esp_timer_delete(esp_timer_handle_t h) { (void)h; return ESP_OK; }
static inline int64_t esp_timer_get_time(void) { return 100000000; }
void esp_restart(void);
static inline esp_err_t esp_wifi_disconnect(void) { return ESP_OK; }
static inline const char *esp_err_to_name(esp_err_t e) { (void)e; return "synthetic"; }
