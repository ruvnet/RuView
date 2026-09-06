set(SRCS
    "main.c" "csi_collector.c" "config_api.c"
    "thermal.c" "stream_sender.c" "nvs_config.c"
    "edge_processing.c" "ota_update.c" "power_mgmt.c"
    "wasm_runtime.c" "wasm_upload.c" "rvf_parser.c"
    "mmwave_sensor.c"
    "swarm_bridge.c"
    # ADR-081 — adaptive CSI mesh firmware kernel
    "rv_radio_ops_esp32.c"
    "rv_feature_state.c"
    "rv_mesh.c"
    "adaptive_controller.c"
    # ADR-110 — ESP32-C6 capability modules (no-op stubs on other targets via #ifdef)
    "c6_twt.c"
    "c6_timesync.c"
    "c6_lp_core.c"
    # ADR-110 D1 workaround — ESP-NOW cross-node sync (works on S3+C6)
    "c6_sync_espnow.c"
    # ADR-110 B1/B2 unblock — soft-AP HE/TWT (C6-only when enabled)
    "c6_softap_he.c"
)

# ESP-IDF v6+: headers must resolve via explicit REQUIRES (no implicit deps).
set(REQUIRES
    json
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

# ADR-110: C6-only components — pulled in when building for esp32c6.
# Note: CONFIG_* symbols are not available in main CMakeLists.txt evaluation —
# we use the IDF_TARGET variable that idf.py sets from sdkconfig.defaults / set-target.
if(IDF_TARGET STREQUAL "esp32c6")
    list(APPEND REQUIRES ieee802154 ulp esp_hw_support)
endif()

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

# ADR-110 P5 (full): embed the LP-core motion-gate program when enabled.
# `ulp_embed_binary` compiles lp_core/main.c with the RISC-V LP toolchain
# and links the resulting binary into the HP image, exposing shared symbols
# via the auto-generated `ulp_main.h` header.
if(IDF_TARGET STREQUAL "esp32c6" AND CONFIG_C6_LP_CORE_ENABLE)
    set(ulp_app_name ulp_main)
    set(ulp_sources "lp_core/main.c")
    # Source files in the HP component that include the generated ulp_main.h
    set(ulp_exp_dep_srcs "c6_lp_core.c")
    ulp_embed_binary(${ulp_app_name} "${ulp_sources}" "${ulp_exp_dep_srcs}")
endif()
