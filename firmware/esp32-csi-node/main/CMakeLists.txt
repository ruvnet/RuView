set(SRCS
    "main.c" "csi_collector.c" "stream_sender.c" "nvs_config.c"
    "edge_processing.c" "ota_update.c" "power_mgmt.c"
    "wasm_runtime.c" "wasm_upload.c" "rvf_parser.c"
    "mmwave_sensor.c"
    "swarm_bridge.c"
    # ADR-081 — adaptive CSI mesh firmware kernel
    "rv_radio_ops_esp32.c"
    "rv_feature_state.c"
    "rv_mesh.c"
    "adaptive_controller.c"
)

# ESP-IDF v6+: headers must resolve via explicit REQUIRES (no implicit deps).
set(REQUIRES
    esp_wifi
    esp_netif
    esp_event
    nvs_flash
    app_update
    esp_http_server
    esp_http_client
    esp_app_format
    esp_timer
    esp_pm
    esp_driver_uart
    esp_driver_gpio
    esp_driver_spi
    esp_driver_i2c
    driver
    lwip
    mbedtls
)

# ADR-061: Mock CSI generator for QEMU testing + ADR-081 mock radio binding
if(CONFIG_CSI_MOCK_ENABLED)
    list(APPEND SRCS "mock_csi.c" "rv_radio_ops_mock.c")
endif()

# ADR-045: AMOLED display support (compile-time optional)
if(CONFIG_DISPLAY_ENABLE)
    list(APPEND SRCS "display_hal.c" "display_ui.c" "display_task.c")
    list(APPEND REQUIRES esp_lcd esp_lcd_touch lvgl)
endif()

if(CONFIG_WASM_ENABLE)
    list(APPEND REQUIRES wasm3)
endif()

idf_component_register(
    SRCS ${SRCS}
    INCLUDE_DIRS "."
    REQUIRES ${REQUIRES}
)
