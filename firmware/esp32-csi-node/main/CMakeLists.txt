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

set(REQUIRES "")

# ADR-061: Mock CSI generator for QEMU testing + ADR-081 mock radio binding
if(CONFIG_CSI_MOCK_ENABLED)
    list(APPEND SRCS "mock_csi.c" "rv_radio_ops_mock.c")
endif()

# ADR-045: AMOLED display support (compile-time optional)
if(CONFIG_DISPLAY_ENABLE)
    list(APPEND SRCS "display_hal.c" "display_ui.c" "display_task.c")
    set(REQUIRES esp_lcd esp_lcd_touch lvgl)
endif()

idf_component_register(
    SRCS ${SRCS}
    INCLUDE_DIRS "."
    REQUIRES ${REQUIRES}
)
