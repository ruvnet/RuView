set(SRCS
    "main.c" "csi_collector.c" "stream_sender.c" "nvs_config.c"
    "edge_processing.c" "ota_update.c" "power_mgmt.c"
    "wasm_runtime.c" "wasm_upload.c" "rvf_parser.c"
)

# ADR-045: AMOLED display support (compile-time optional)
if(CONFIG_DISPLAY_ENABLE)
    list(APPEND SRCS "display_hal.c" "display_ui.c" "display_task.c")
    set(PRIV_REQUIRES esp_lcd esp_lcd_touch lvgl)
else()
    set(PRIV_REQUIRES "")
endif()

idf_component_register(
    SRCS ${SRCS}
    INCLUDE_DIRS "."
    PRIV_REQUIRES ${PRIV_REQUIRES}
)
