/**
 * @file camera_node.c
 * @brief Onboard camera (Seeed XIAO ESP32S3 Sense, OV3660) + HTTP server.
 *
 * Pin map verified on hardware by the cam-probe firmware (2026-06): the
 * XIAO ESP32S3 Sense map succeeded with sensor PID 0x3660. QVGA JPEG with
 * fb_count=2 + CAMERA_GRAB_LATEST in PSRAM is the proven-reliable config
 * (SVGA with fb_count=1 overflowed: FB-OVF). JPEG quality 12 + QVGA keeps
 * per-frame airtime bounded so the CSI promiscuous callback isn't starved.
 *
 * The HTTP server runs on CONFIG_CAMERA_HTTP_PORT (default 8081 — the OTA
 * server owns 8032) with its tasks pinned to core 1, away from the WiFi/CSI
 * work on core 0. ctrl_port is offset from the default because the OTA
 * httpd instance already owns 32768.
 */

#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "esp_camera.h"
#include "esp_http_server.h"
#include "esp_log.h"
#include "sdkconfig.h"

#include "camera_node.h"

static const char *TAG = "camera_node";

static httpd_handle_t s_httpd = NULL;

/* Seeed XIAO ESP32S3 Sense pin map (cam-probe verified). */
static const camera_config_t s_cam_cfg = {
    .pin_pwdn = -1,
    .pin_reset = -1,
    .pin_xclk = 10,
    .pin_sccb_sda = 40,
    .pin_sccb_scl = 39,
    .pin_d7 = 48, .pin_d6 = 11, .pin_d5 = 12, .pin_d4 = 14,
    .pin_d3 = 16, .pin_d2 = 18, .pin_d1 = 17, .pin_d0 = 15,
    .pin_vsync = 38,
    .pin_href = 47,
    .pin_pclk = 13,
    .ledc_channel = LEDC_CHANNEL_0,
    .ledc_timer = LEDC_TIMER_0,
    .xclk_freq_hz = 20000000,
    .pixel_format = PIXFORMAT_JPEG,
    .frame_size = FRAMESIZE_QVGA,
    .jpeg_quality = 12,
    .fb_count = 2,
    .fb_location = CAMERA_FB_IN_PSRAM,
    .grab_mode = CAMERA_GRAB_LATEST,
};

/* GET /snap — one fresh JPEG. Discard one (possibly stale) framebuffer
 * first so the client always gets a current capture. */
static esp_err_t snap_handler(httpd_req_t *req)
{
    camera_fb_t *fb = esp_camera_fb_get();
    if (fb != NULL) {
        esp_camera_fb_return(fb);   /* discard stale frame */
    }
    fb = esp_camera_fb_get();
    if (fb == NULL) {
        ESP_LOGW(TAG, "/snap: capture failed");
        httpd_resp_send_err(req, HTTPD_500_INTERNAL_SERVER_ERROR, "camera capture failed");
        return ESP_FAIL;
    }

    httpd_resp_set_type(req, "image/jpeg");
    httpd_resp_set_hdr(req, "Cache-Control", "no-store, no-cache, must-revalidate");
    httpd_resp_set_hdr(req, "Pragma", "no-cache");
    esp_err_t err = httpd_resp_send(req, (const char *)fb->buf, fb->len);
    esp_camera_fb_return(fb);
    return err;
}

/* GET /stream — multipart MJPEG at ~5 fps (200 ms inter-frame delay).
 * Loops until the client disconnects (chunk send fails). */
#define STREAM_BOUNDARY "ruviewcamframe"

static esp_err_t stream_handler(httpd_req_t *req)
{
    httpd_resp_set_type(req, "multipart/x-mixed-replace;boundary=" STREAM_BOUNDARY);
    httpd_resp_set_hdr(req, "Cache-Control", "no-store");

    while (true) {
        camera_fb_t *fb = esp_camera_fb_get();
        if (fb == NULL) {
            ESP_LOGW(TAG, "/stream: capture failed — closing stream");
            break;
        }

        char part_hdr[96];
        int hdr_len = snprintf(part_hdr, sizeof(part_hdr),
                               "\r\n--" STREAM_BOUNDARY "\r\n"
                               "Content-Type: image/jpeg\r\n"
                               "Content-Length: %u\r\n\r\n",
                               (unsigned)fb->len);

        esp_err_t err = httpd_resp_send_chunk(req, part_hdr, hdr_len);
        if (err == ESP_OK) {
            err = httpd_resp_send_chunk(req, (const char *)fb->buf, fb->len);
        }
        esp_camera_fb_return(fb);

        if (err != ESP_OK) {
            break;   /* client disconnected */
        }
        vTaskDelay(pdMS_TO_TICKS(200));   /* ~5 fps */
    }

    httpd_resp_send_chunk(req, NULL, 0);   /* terminate chunked response */
    return ESP_OK;
}

esp_err_t camera_node_start(void)
{
    esp_err_t err = esp_camera_init(&s_cam_cfg);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "camera init failed: %s — camera disabled, CSI unaffected",
                 esp_err_to_name(err));
        return err;
    }

    sensor_t *sensor = esp_camera_sensor_get();
    ESP_LOGI(TAG, "camera up: sensor PID=0x%04x (QVGA JPEG, fb_count=2, PSRAM)",
             (sensor != NULL) ? sensor->id.PID : 0);

    httpd_config_t cfg = HTTPD_DEFAULT_CONFIG();
    cfg.server_port = CONFIG_CAMERA_HTTP_PORT;
    cfg.ctrl_port = 32769;        /* OTA httpd owns the default 32768 */
    cfg.core_id = 1;              /* keep capture/serve off core 0 (WiFi/CSI) */
    cfg.lru_purge_enable = true;

    err = httpd_start(&s_httpd, &cfg);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "camera httpd start failed: %s — camera disabled",
                 esp_err_to_name(err));
        esp_camera_deinit();
        return err;
    }

    const httpd_uri_t snap_uri = {
        .uri = "/snap", .method = HTTP_GET, .handler = snap_handler,
    };
    const httpd_uri_t stream_uri = {
        .uri = "/stream", .method = HTTP_GET, .handler = stream_handler,
    };
    httpd_register_uri_handler(s_httpd, &snap_uri);
    httpd_register_uri_handler(s_httpd, &stream_uri);

    ESP_LOGI(TAG, "camera HTTP server on port %d (/snap, /stream)",
             CONFIG_CAMERA_HTTP_PORT);
    return ESP_OK;
}
