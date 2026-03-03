idf_component_register(
    SRCS "main.c" "csi_collector.c" "stream_sender.c" "nvs_config.c"
         "edge_processing.c" "ota_update.c" "power_mgmt.c"
         "wasm_runtime.c" "wasm_upload.c" "rvf_parser.c"
    INCLUDE_DIRS "."
)
