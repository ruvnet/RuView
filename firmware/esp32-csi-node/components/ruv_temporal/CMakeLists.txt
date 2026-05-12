# ESP-IDF component manifest for the ruv_temporal Rust staticlib (ADR-095).
#
# Build flow:
#   - When CONFIG_CSI_TEMPORAL_HEAD_ENABLED is OFF (default): register an
#     empty stub. main/temporal_task.c compiles the no-op shim path, no
#     cargo, no Rust toolchain dependency. Default firmware build is
#     unaffected.
#   - When CONFIG_CSI_TEMPORAL_HEAD_ENABLED is ON: invoke
#     `cargo +esp build --release --target xtensa-esp32s3-none-elf`,
#     register the resulting libruv_temporal.a, and expose include/.
#
# add_custom_command is intentionally placed AFTER idf_component_register
# because ESP-IDF runs every component's CMakeLists.txt twice — once in
# script mode for dependency discovery (where add_custom_command is
# forbidden), and once for the actual build.

if(NOT CONFIG_CSI_TEMPORAL_HEAD_ENABLED)
    # Feature disabled — register an empty component so the directory's
    # mere existence doesn't break the build, but do NOT invoke cargo
    # or pull include/ onto consumers' include paths (the C ABI header
    # would advertise capabilities we cannot honour).
    idf_component_register()
    return()
endif()

set(RUV_TEMPORAL_DIR "${CMAKE_CURRENT_SOURCE_DIR}")
set(RUV_TEMPORAL_TARGET "xtensa-esp32s3-none-elf")
set(RUV_TEMPORAL_PROFILE "release")
set(RUV_TEMPORAL_LIB
    "${RUV_TEMPORAL_DIR}/target/${RUV_TEMPORAL_TARGET}/${RUV_TEMPORAL_PROFILE}/libruv_temporal.a")

idf_component_register(
    SRCS "shim.c"
    INCLUDE_DIRS "include"
    PRIV_REQUIRES "esp_common"
)

# Custom command + target run only at build time, not in script mode.
add_custom_command(
    OUTPUT "${RUV_TEMPORAL_LIB}"
    WORKING_DIRECTORY "${RUV_TEMPORAL_DIR}"
    COMMAND cargo +esp build --release --target ${RUV_TEMPORAL_TARGET}
    COMMENT "Building ruv_temporal Rust staticlib for ${RUV_TEMPORAL_TARGET}"
    VERBATIM
)
add_custom_target(ruv_temporal_rust_build ALL DEPENDS "${RUV_TEMPORAL_LIB}")

add_dependencies(${COMPONENT_LIB} ruv_temporal_rust_build)
target_link_libraries(${COMPONENT_LIB} INTERFACE "${RUV_TEMPORAL_LIB}")
